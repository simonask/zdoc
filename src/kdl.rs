use core::{mem, str::FromStr};

use alloc::{
    borrow::{Cow, ToOwned},
    string::{String, ToString as _},
};
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};

use crate::{
    Builder, Document, DocumentBuffer, Error, Result, ValueRef,
    builder::{Arg, Node, Value},
};

/// Convert a document to a KDL document.
///
/// # Errors
///
/// If `doc` contains binary data (unrepresentable in KDL), this returns an
/// error.
#[inline]
pub fn document_to_kdl_document(doc: &Document) -> Result<KdlDocument> {
    KdlDocument::try_from(doc)
}

/// Convert a document to a KDL string.
///
/// # Errors
///
/// If `doc` contains binary data (unrepresentable in KDL), this returns an
/// error.
#[inline]
pub fn document_to_kdl(doc: &Document) -> Result<String> {
    KdlDocument::try_from(doc).map(|doc| doc.to_string())
}

#[inline]
#[must_use]
pub fn document_from_kdl_document(kdl: &KdlDocument) -> DocumentBuffer {
    DocumentBuffer::from(kdl)
}

/// Convert a KDL document to a document.
///
/// # Errors
///
/// This function will return an error if `kdl` is not valid KDL syntax.
#[inline]
pub fn document_from_kdl(kdl: &str) -> Result<DocumentBuffer> {
    let kdl = KdlDocument::from_str(kdl).map_err(Error::custom)?;
    Ok(document_from_kdl_document(&kdl))
}

/// Create a builder from a KDL document.
///
/// This version borrows strings from the KDL document.
#[inline]
#[must_use]
pub fn builder_from_kdl_document(kdl: &KdlDocument) -> Builder<'_> {
    Builder::from(kdl)
}

/// Create a builder from a KDL document.
///
/// This version borrows strings from the KDL document.
///
/// # Errors
///
/// If `kdl` is not valid KDL syntax, this function will return an error.
#[inline]
pub fn builder_from_kdl(kdl: &str) -> Result<Builder<'static>> {
    Ok(
        builder_from_kdl_document(&KdlDocument::from_str(kdl).map_err(Error::custom)?)
            .into_static(),
    )
}

impl<'a> From<&'a KdlDocument> for Builder<'a> {
    fn from(value: &'a KdlDocument) -> Self {
        let mut builder = Builder::new();

        let root = builder.root_mut();
        for node in value.nodes() {
            root.push(Node::from(node));
        }

        builder
    }
}

impl<'a> From<&'a KdlDocument> for DocumentBuffer {
    #[inline]
    fn from(value: &'a KdlDocument) -> Self {
        Builder::from(value).build()
    }
}

impl<'a> TryFrom<&'a Document> for KdlDocument {
    type Error = Error;

    fn try_from(value: &'a Document) -> Result<Self> {
        let mut doc = KdlDocument::new();

        let root = value.root();
        // KDL doesn't support arguments to the root, so push them to the root
        // node as children.
        for arg in root.args() {
            let name = arg.name.unwrap_or("-");
            let mut node = KdlNode::new(name);
            let value: KdlValue = arg.value.try_into()?;
            node.entries_mut().push(value.into());
        }

        for child in root.children() {
            doc.nodes_mut().push(KdlNode::try_from(child)?);
        }

        Ok(doc)
    }
}

impl<'a> From<&'a KdlNode> for Node<'a> {
    fn from(value: &'a KdlNode) -> Self {
        let mut node = Node::empty();

        // Handle conventionally nameless nodes.
        let mut name = value.name().repr().unwrap_or("-");
        if name == "-" {
            name = "";
        }

        node.set_name(name);
        node.set_ty(value.ty().and_then(|ty| ty.repr()).unwrap_or(""));

        for entry in value.entries() {
            node.push_arg(entry);
        }

        if let Some(children) = value.children() {
            for child in children.nodes() {
                node.push(Node::from(child));
            }
        }

        node
    }
}

impl TryFrom<Node<'_>> for KdlNode {
    type Error = Error;

    fn try_from(mut value: Node) -> Result<Self> {
        // Produce conventionally nameless nodes.
        let name = if value.name.is_empty() {
            "-"
        } else {
            value.name.as_ref()
        };

        let mut node = KdlNode::new(name);
        if !value.ty.is_empty() {
            node.set_ty(&*value.ty);
        }

        for arg in mem::take(value.args_mut()) {
            node.entries_mut().push(arg.try_into()?);
        }

        if !value.children().is_empty() {
            let children = node.children_mut().get_or_insert_default();
            for child in mem::take(value.children_mut()) {
                children.nodes_mut().push(child.try_into()?);
            }
        }

        Ok(node)
    }
}

impl TryFrom<crate::Node<'_>> for KdlNode {
    type Error = Error;

    fn try_from(value: crate::Node<'_>) -> Result<Self> {
        // Produce conventionally nameless nodes.
        let name = match value.name() {
            None => "-",
            Some(name) => name,
        };
        let mut node = KdlNode::new(name);

        if let Some(ty) = value.ty() {
            node.set_ty(ty);
        }

        for arg in value.args() {
            node.entries_mut().push(arg.try_into()?);
        }

        if !value.children().is_empty() {
            let children = node.children_mut().get_or_insert_default();
            for child in value.children() {
                children.nodes_mut().push(child.try_into()?);
            }
        }

        Ok(node)
    }
}

impl<'a> From<&'a KdlEntry> for Arg<'a> {
    #[inline]
    fn from(value: &'a KdlEntry) -> Self {
        Arg {
            name: value.name().and_then(|name| name.repr()).map(Cow::Borrowed),
            value: value.value().into(),
        }
    }
}

impl TryFrom<Arg<'_>> for KdlEntry {
    type Error = Error;

    #[inline]
    fn try_from(value: Arg) -> Result<Self> {
        let kdl_value: KdlValue = value.value.try_into()?;
        Ok(if let Some(name) = value.name {
            KdlEntry::new_prop(&*name, kdl_value)
        } else {
            KdlEntry::new(kdl_value)
        })
    }
}

impl TryFrom<crate::Arg<'_>> for KdlEntry {
    type Error = Error;

    #[inline]
    fn try_from(value: crate::Arg<'_>) -> Result<Self> {
        let kdl_value: KdlValue = value.value.try_into()?;
        Ok(if let Some(name) = value.name {
            KdlEntry::new_prop(name, kdl_value)
        } else {
            KdlEntry::new(kdl_value)
        })
    }
}

impl<'a> From<&'a KdlValue> for ValueRef<'a> {
    #[inline]
    fn from(value: &'a KdlValue) -> Self {
        match value {
            KdlValue::String(s) => ValueRef::String(s),
            KdlValue::Integer(value) => {
                ValueRef::Int((*value).try_into().expect("Integer value too large"))
            }
            KdlValue::Float(value) => ValueRef::Float(*value),
            KdlValue::Bool(value) => ValueRef::Bool(*value),
            KdlValue::Null => ValueRef::Null,
        }
    }
}

impl TryFrom<ValueRef<'_>> for KdlValue {
    type Error = Error;

    #[inline]
    fn try_from(value: ValueRef<'_>) -> Result<Self> {
        Ok(match value {
            ValueRef::String(s) => KdlValue::String(s.to_owned()),
            ValueRef::Binary(_) => return Err(Error::UnrepresentableBinary),
            ValueRef::Int(i) => KdlValue::Integer(i as _),
            ValueRef::Uint(u) => KdlValue::Integer(u as _),
            ValueRef::Float(f) => KdlValue::Float(f),
            ValueRef::Bool(b) => KdlValue::Bool(b),
            ValueRef::Null => KdlValue::Null,
        })
    }
}

impl<'a> From<&'a KdlValue> for Value<'a> {
    #[inline]
    fn from(value: &'a KdlValue) -> Self {
        match value {
            KdlValue::String(s) => Value::String(Cow::Borrowed(s)),
            KdlValue::Integer(value) => {
                Value::Int((*value).try_into().expect("Integer value too large"))
            }
            KdlValue::Float(value) => Value::Float(*value),
            KdlValue::Bool(value) => Value::Bool(*value),
            KdlValue::Null => Value::Null,
        }
    }
}

impl TryFrom<Value<'_>> for KdlValue {
    type Error = Error;

    #[inline]
    fn try_from(value: Value<'_>) -> Result<Self> {
        Ok(match value {
            Value::String(s) => KdlValue::String(s.into_owned()),
            Value::Binary(_) => return Err(Error::UnrepresentableBinary),
            Value::Int(i) => KdlValue::Integer(i as _),
            Value::Uint(u) => KdlValue::Integer(u as _),
            Value::Float(f) => KdlValue::Float(f),
            Value::Bool(b) => KdlValue::Bool(b),
            Value::Null => KdlValue::Null,
        })
    }
}
