//! Convert between XML and [`Document`].
//!
//! Since both XML and [`Document`] are node-based, the conversion has higher
//! fidelity than JSON or YAML, but XML is more strict because it requires all
//! elements to have a "type" (the node's type).
//!
//! When nodes have a type, the XML tag is set to the type name. If a node has no
//! type, the XML tag is set to the value of [`XmlSettings::untyped_node_tag`].
//!
//! # Type inference
//!
//! Attributes in XML are not typed, so the type of a node is inferred from the
//! value of the attribute. If the value is `true` or `false`, the type is
//! `bool`. The string `"null"` is parsed as the null value. Otherwise, we try
//! to parse the value as an integer. If that fails, we try to parse it as a
//! float. If that fails, we assume the value is a string.

use alloc::{
    borrow::{Cow, ToOwned},
    string::{String, ToString as _},
    vec::Vec,
};
use quick_xml::{
    Reader, Writer,
    escape::{escape, unescape},
    events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event, attributes::Attribute},
    name::QName,
};

use crate::{Builder, Document, DocumentBuffer, Error, Result, ValueRef, builder};

pub struct XmlSettings<'a> {
    /// The XML tag to use for nodes without a type. Defaults to `<item .../>`.
    pub untyped_node_tag: &'a str,
    /// The XML attribute for the name of children that have a name. Defaults to
    /// `name`.
    pub name_attribute: &'a str,
    /// The XML tag to use for unnamed node arguments. Defaults to `arg`, so an
    /// element will be created as `<arg value="..." />`.
    pub unnamed_argument_tag: &'a str,
    /// The XML tag to use for the value of unnamed node arguments. Defaults to
    /// `value`, so `<item value="..." />`.
    pub unnamed_argument_attribute: &'a str,
}

impl Default for XmlSettings<'_> {
    #[inline]
    fn default() -> Self {
        Self {
            untyped_node_tag: "item",
            name_attribute: "name",
            unnamed_argument_tag: "arg",
            unnamed_argument_attribute: "value",
        }
    }
}

/// Convert a [`Document`] to XML.
///
/// # Errors
///
/// If any fields in the document would clobber the XML format, this returns an
/// error.
#[inline]
pub fn document_to_xml(doc: &Document) -> Result<String> {
    document_to_xml_with_settings(doc, &XmlSettings::default())
}

/// Convert a [`Document`] to XML.
///
/// # Errors
///
/// If any fields in the document would clobber the special fields defined in
/// `settings`, this returns an error.
#[inline]
pub fn document_to_xml_with_settings(doc: &Document, settings: &XmlSettings<'_>) -> Result<String> {
    settings.write_document(doc)
}

/// Convert XML to a [`DocumentBuffer`].
///
/// # Errors
///
/// If `xml` is not valid XML, this returns an error.
#[inline]
pub fn document_from_xml(xml: &str) -> Result<DocumentBuffer> {
    let builder = builder_from_xml(xml)?;
    Ok(builder.build())
}

/// Convert XML to a [`Builder`], which can be modified further.
///
/// # Errors
///
/// If `xml` is not valid XML, this returns an error.
#[inline]
pub fn builder_from_xml(xml: &str) -> Result<Builder> {
    builder_from_xml_with_settings(xml, &XmlSettings::default())
}

/// Convert XML to a [`Builder`], which can be modified further.
///
/// # Errors
///
/// If `xml` is not valid XML, this returns an error.
#[inline]
pub fn builder_from_xml_with_settings<'a>(
    xml: &'a str,
    settings: &XmlSettings<'_>,
) -> Result<Builder<'a>> {
    settings.read_document(xml)
}

impl XmlSettings<'_> {
    fn read_document<'a>(&self, xml: &'a str) -> Result<Builder<'a>> {
        let mut reader = Reader::from_str(xml);
        let mut eof = false;
        let root = loop {
            match reader.read_event().map_err(Error::custom)? {
                Event::Start(tag) => break self.read_node_with_body(&tag, &mut reader)?,
                Event::End(_) => return Err(Error::msg("unexpected end tag")),
                Event::Empty(tag) => break self.read_node_attributes(&tag)?,
                Event::Text(bytes_text) => {
                    let mut node = builder::Node::empty();
                    node.push_unnamed_arg(bytes_text.unescape().map_err(Error::custom)?);
                    break node;
                }
                Event::CData(_) => return Err(Error::msg("unexpected CDATA")),

                Event::Decl(decl) => {
                    if let Some(encoding) = decl.encoding() {
                        let encoding = encoding.map_err(Error::custom)?;
                        if !(*encoding == *b"UTF-8" || *encoding == *b"utf-8") {
                            return Err(Error::msg(format_args!(
                                "unsupported encoding: {}",
                                core::str::from_utf8(&encoding).unwrap_or("<invalid UTF-8>")
                            )));
                        }
                    }
                }
                Event::Comment(_) | Event::PI(_) | Event::DocType(_) => (),
                Event::Eof => {
                    eof = true;
                    break builder::Node::empty();
                }
            }
        };

        if !eof {
            let last_event = reader.read_event().map_err(Error::custom)?;
            if !matches!(last_event, Event::Eof) {
                return Err(Error::msg("trailing data after root node"));
            }
        }

        let mut builder = Builder::new();
        builder.set_root(root);
        Ok(builder)
    }

    fn read_node_attributes<'a>(&self, tag: &BytesStart<'a>) -> Result<builder::Node<'a>> {
        let mut node = builder::Node::empty();
        let xml_name = qname_to_string(tag.name())?;
        if xml_name != self.untyped_node_tag {
            // Typed node, so we need to set the type to the tag name.
            node.set_ty(xml_name);
        }

        for attr in tag.attributes() {
            let attr = attr.map_err(Error::custom)?;
            let name = qname_to_string(attr.key)?;
            if name == self.name_attribute {
                // This is the name attribute, so set the name of the node.
                let value = string_from_bytes(&attr.value)?;
                node.set_name(value);
                continue;
            }

            // This is a named argument, so set it as an attribute.
            let value = self.read_value(&attr.value)?;
            node.push_named_arg(name, value);
        }

        Ok(node)
    }

    fn read_node_with_body<'a>(
        &self,
        tag: &BytesStart<'a>,
        reader: &mut Reader<&'a [u8]>,
    ) -> Result<builder::Node<'a>> {
        let mut node = self.read_node_attributes(tag)?;

        // Read children until we see the end tag.
        loop {
            match reader.read_event().map_err(Error::custom)? {
                Event::Start(tag) => {
                    let child = self.read_node_with_body(&tag, reader)?;
                    node.push(child);
                }
                Event::End(end_tag) => {
                    if end_tag.name() == tag.name() {
                        break;
                    }
                    return Err(Error::msg(format_args!(
                        "unexpected end tag: {}",
                        qname_ref_to_string(end_tag.name())?
                    )));
                }
                Event::Text(text) => {
                    let text = text.unescape().map_err(Error::custom)?;
                    let mut child = builder::Node::empty();
                    child.push_unnamed_arg(text);
                    node.push(child);
                }
                Event::Empty(tag) => {
                    let name = qname_ref_to_string(tag.name())?;
                    if name == self.unnamed_argument_tag {
                        if let Some(attr) = tag
                            .try_get_attribute(self.unnamed_argument_attribute)
                            .map_err(Error::custom)?
                        {
                            node.push_unnamed_arg(self.read_value(&attr.value)?);
                            continue;
                        }
                    }

                    let child = self.read_node_attributes(&tag)?;
                    node.push(child);
                }
                Event::CData(cdata) => {
                    let inner = cdata.into_inner();
                    let text = alloc::string::String::from_utf8(inner.into_owned())
                        .map_err(|_| Error::UnrepresentableString)?;
                    let mut child = builder::Node::empty();
                    child.push_unnamed_arg(text);
                    node.push(child);
                }
                Event::Decl(_) | Event::Comment(_) | Event::PI(_) | Event::DocType(_) => (),
                Event::Eof => return Err(Error::msg("unexpected EOF")),
            }
        }

        Ok(node)
    }

    // Unfortunately, `BytesStart` does not allow us to borrow the bytes of the
    // attributes from the original string.
    #[allow(clippy::same_functions_in_if_condition, clippy::unused_self)] // false positive
    fn read_value(&self, bytes: &[u8]) -> Result<builder::Value<'static>> {
        let string = string_from_bytes(bytes)?;

        Ok(match &*string {
            "null" => builder::Value::Null,
            "false" => builder::Value::Bool(false),
            "true" => builder::Value::Bool(true),
            string => {
                if let Ok(uint) = string.parse() {
                    builder::Value::Uint(uint)
                } else if let Ok(int) = string.parse() {
                    builder::Value::Int(int)
                } else if let Ok(float) = string.parse() {
                    builder::Value::Float(float)
                } else {
                    // This is a string, so we need to unescape it.
                    let escaped = unescape(string).map_err(Error::custom)?.into_owned();
                    builder::Value::String(escaped.into())
                }
            }
        })
    }

    fn write_document(&self, doc: &Document) -> Result<String> {
        let mut out = Vec::new();
        let mut writer = Writer::new(&mut out);
        writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
            .map_err(Error::custom)?;
        self.write_node(&mut writer, doc.root())?;
        alloc::string::String::from_utf8(out).map_err(|_| Error::UnrepresentableString)
    }

    fn write_node(&self, writer: &mut Writer<&mut Vec<u8>>, node: crate::Node) -> Result<()> {
        let ty = node.ty().unwrap_or(self.untyped_node_tag);

        let mut start = BytesStart::new(ty);

        if let Some(name) = node.name() {
            start.push_attribute(value_to_attribute(
                self.name_attribute,
                ValueRef::String(name),
            )?);
        }

        let mut has_unnamed_args = false;
        // Write named arguments as attributes.
        for arg in node.args() {
            let Some(name) = arg.name else {
                has_unnamed_args = true;
                continue;
            };
            start.push_attribute(value_to_attribute(name, arg.value)?);
        }

        if has_unnamed_args || !node.children().is_empty() {
            writer
                .write_event(Event::Start(start))
                .map_err(Error::custom)?;

            for arg in node.args() {
                if arg.name.is_some() {
                    // Already written as an attribute.
                    continue;
                }

                let mut attr = BytesStart::new(self.unnamed_argument_tag);
                attr.push_attribute(value_to_attribute(
                    self.unnamed_argument_attribute,
                    arg.value,
                )?);
                writer
                    .write_event(Event::Empty(attr))
                    .map_err(Error::custom)?;
            }

            for child in node.children() {
                // If the child is unnamed, untyped, and has a single string
                // argument, emit it as text.
                if child.name().is_none() && child.ty().is_none() && child.args().len() == 1 {
                    if let Some(crate::Arg {
                        name: None,
                        value: ValueRef::String(text),
                    }) = child.args().get(0)
                    {
                        writer
                            .write_event(Event::Text(BytesText::new(text)))
                            .map_err(Error::custom)?;
                        continue;
                    }
                }

                self.write_node(writer, child)?;
            }

            writer
                .write_event(Event::End(BytesEnd::new(ty)))
                .map_err(Error::custom)
        } else {
            writer
                .write_event(Event::Empty(start))
                .map_err(Error::custom)
        }
    }
}

#[inline]
fn value_to_attribute<'a>(name: &'a str, value: ValueRef<'a>) -> Result<Attribute<'a>> {
    let key = QName(name.as_bytes());
    let value: Cow<'_, [u8]> = match value {
        ValueRef::Null => Cow::Borrowed(b"null"),
        ValueRef::Bool(false) => Cow::Borrowed(b"false"),
        ValueRef::Bool(true) => Cow::Borrowed(b"true"),
        ValueRef::Int(int) => Cow::Owned(int.to_string().into_bytes()),
        ValueRef::Uint(uint) => Cow::Owned(uint.to_string().into_bytes()),
        ValueRef::Float(float) => Cow::Owned(float.to_string().into_bytes()),
        ValueRef::String(string) => match escape(string) {
            Cow::Borrowed(string) => Cow::Borrowed(string.as_bytes()),
            Cow::Owned(string) => Cow::Owned(string.into_bytes()),
        },
        ValueRef::Binary(_) => return Err(Error::UnrepresentableBinary),
    };
    Ok(Attribute { key, value })
}

#[inline]
fn string_from_bytes(bytes: &[u8]) -> Result<Cow<'static, str>> {
    core::str::from_utf8(bytes)
        .map(|s| Cow::Owned(s.to_owned()))
        .map_err(|_| Error::UnrepresentableString)
}

#[inline]
fn qname_to_string(qname: QName<'_>) -> Result<Cow<'static, str>> {
    string_from_bytes(qname.0)
}

#[inline]
fn qname_ref_to_string(qname: QName<'_>) -> Result<&'_ str> {
    core::str::from_utf8(qname.0).map_err(|_| Error::UnrepresentableString)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_roundtrip() {
        let mut builder = Builder::new();
        builder.with_root(|root| {
            root.set_ty("Root");
            root.push(builder::Arg::new(
                "key",
                builder::Value::String("value".into()),
            ));
            // Unnamed arguments should end up as "<arg>" elements.
            root.push(builder::Value::String("child".into()));
            root.push(builder::Value::Int(123));
        });
        let doc = builder.build();

        let xml = document_to_xml(&doc).unwrap();
        assert_eq!(
            xml,
            r#"<?xml version="1.0" encoding="UTF-8"?><Root key="value"><arg value="child"/><arg value="123"/></Root>"#
        );

        let doc = document_from_xml(&xml).unwrap();
        let root = doc.root();
        assert_eq!(root.ty(), Some("Root"));
        assert_eq!(root.args().len(), 3);
        assert_eq!(root.args().get(0).unwrap().name, Some("key"));
        assert_eq!(
            root.args().get(0).unwrap().value,
            crate::ValueRef::String("value")
        );
        assert_eq!(root.args().get(1).unwrap().name, None);
        assert_eq!(
            root.args().get(1).unwrap().value,
            crate::ValueRef::String("child")
        );
        assert_eq!(root.args().get(2).unwrap().name, None);
        assert_eq!(
            root.args().get(2).unwrap().value,
            // Since the number is representable as a `u64`, it will be
            // converted.
            crate::ValueRef::Uint(123)
        );
    }
}
