//! Convert between [`Document`] and JSON.
//!
//! Key-value nodes are represented as JSON objects, and sequence-like nodes are
//! represented as JSON arrays.
//!
//! Nodes that have a "type" will gain a special `"$type"` field in the JSON
//! representation. Nodes that have a mix of named and unnamed
//! children/arguments will gain an `"$items"` field, which contains a JSON
//! array of the unnamed items. Nodes that have a single unnamed child will gain
//! a special `"$value"` field containing the argument.
//!
//! **Note:** Conversion to and from JSON is lossy, because JSON does not
//! distinguish between attributes and children. All arguments and children of a
//! node are treated as fields (when named) or sequence items (when unnamed).
//!
//! Similarly, [`Document`] cannot distinguish between single-value lists and
//! single-value nodes. For example, a JSON list with a single element will be
//! represented as just the element, losing the "array" information, so a
//! roundtrip through [`Document`] is not lossless.
//!
//! Due to the way that the [`Deserialize`](serde::Deserialize) trait is
//! implemented for [`Document`], converting JSON to a [`Document`] and
//! deserializing something from it should behave identially to `serde_json`.

use alloc::{
    borrow::{Cow, ToOwned as _},
    string::{String, ToString},
    vec::Vec,
};

use crate::{Builder, ClassifyNode, Document, DocumentBuffer, Error, Result, builder};

/// Settings for converting between JSON and [`Document`].
pub struct JsonSettings<'a> {
    /// For nodes that have a type, using this key will add a field to the JSON
    /// object with the type name. Default is `"$type"`. When empty, type
    /// information is omitted.
    pub type_tag: &'a str,
    /// When the children of a node are mixed named and unnamed, or the node has
    /// a type (and `type_tag` is not empty), overflow unnamed children into a
    /// list with this name. Default is `"$items"`.
    ///
    /// Note that sequence-like nodes with a single element are
    /// indistinguishable from single-value nodes, so single-element sequences
    /// will not have an `$items` field, but instead a `$value` field.
    pub items_tag: &'a str,
    /// In objects representing a single value (e.g. a node that contains a
    /// single unnamed element, but the node has a type), put the value in a
    /// field with this name. Default is `"$value"`.
    ///
    /// Note that sequence-like nodes with a single element are
    /// indistinguishable from single-value nodes, so single-element sequences
    /// will not have an `$items` field, but instead a `$value` field.
    pub value_tag: &'a str,
}

impl Default for JsonSettings<'_> {
    #[inline]
    fn default() -> Self {
        Self {
            type_tag: "$type",
            items_tag: "$items",
            value_tag: "$value",
        }
    }
}

/// Convert [`Document`] to JSON.
///
/// # Errors
///
/// If the document cannot be represented as JSON, or if any fields in the
/// document conflict with the default JSON settings, this returns an error.
#[inline]
pub fn document_to_json_value(doc: &Document) -> Result<serde_json::Value> {
    document_to_json_value_with_settings(doc, &JsonSettings::default())
}

/// Convert [`Document`] to JSON.
///
/// # Errors
///
/// If the document cannot be represented as JSON, or if any fields in the
/// document conflict with the default JSON settings, this returns an error.
#[inline]
pub fn document_to_json(doc: &Document) -> Result<alloc::string::String> {
    let value = document_to_json_value(doc)?;
    Ok(value.to_string())
}

/// Convert [`Document`] to JSON.
///
/// # Errors
///
/// If the document cannot be represented as JSON, or if any fields in the
/// document conflict with the JSON settings, this returns an error.
#[inline]
pub fn document_to_json_value_with_settings(
    doc: &Document,
    settings: &JsonSettings,
) -> Result<serde_json::Value> {
    settings.node_to_json(&doc.root())
}

/// Convert [`Document`] to JSON.
///
/// # Errors
///
/// If the document cannot be represented as JSON, or if any fields in the
/// document conflict with the JSON settings, this returns an error.
#[inline]
pub fn document_to_json_with_settings(
    doc: &Document,
    settings: &JsonSettings,
) -> Result<alloc::string::String> {
    settings
        .node_to_json(&doc.root())
        .map(|value| value.to_string())
}

/// Convert JSON to [`Document`].
///
/// This is an infallible conversion.
#[inline]
#[must_use]
pub fn document_from_json_value(value: &serde_json::Value) -> DocumentBuffer {
    document_from_json_value_with_settings(value, &JsonSettings::default())
}

/// Convert JSON to [`Document`].
///
/// # Errors
///
/// If the string is not a valid JSON document, this returns an error.
#[inline]
pub fn document_from_json(json: &str) -> Result<DocumentBuffer> {
    document_from_json_with_settings(json, &JsonSettings::default())
}

/// Convert JSON to [`Document`].
///
/// This is an infallible conversion.
#[inline]
#[must_use]
pub fn document_from_json_value_with_settings(
    value: &serde_json::Value,
    settings: &JsonSettings,
) -> DocumentBuffer {
    builder_from_json_value_with_settings(value, settings).build()
}

/// Convert JSON to [`Document`].
///
/// # Errors
///
/// If the string is not a valid JSON document, this returns an error.
#[inline]
pub fn document_from_json_with_settings(
    json: &str,
    settings: &JsonSettings,
) -> Result<DocumentBuffer> {
    let json = serde_json::from_str(json).map_err(Error::custom)?;
    Ok(builder_from_json_value_with_settings(&json, settings).build())
}

/// Convert JSON to [`Builder`], which can be modified further.
///
/// Strings from the JSON value are borrowed, not cloned, so the JSON value must
/// outlive the returned builder.
///
/// This is an infallible conversion.
#[must_use]
#[inline]
pub fn builder_from_json_value(value: &serde_json::Value) -> Builder<'_> {
    builder_from_json_value_with_settings(value, &JsonSettings::default())
}

#[must_use]
#[inline]
pub fn builder_from_json_value_with_settings<'a>(
    value: &'a serde_json::Value,
    settings: &JsonSettings,
) -> Builder<'a> {
    settings.json_to_builder(value)
}

impl JsonSettings<'_> {
    #[inline]
    fn json_to_builder<'a>(&self, value: &'a serde_json::Value) -> Builder<'a> {
        let mut builder = Builder::new();
        builder.set_root(self.json_to_node(value));
        builder
    }

    fn node_to_json(&self, node: &crate::Node) -> Result<serde_json::Value> {
        let classification = node.classify();

        if let ClassifyNode::Struct
        | ClassifyNode::StructVariant
        | ClassifyNode::SeqVariant
        | ClassifyNode::ValueVariant
        | ClassifyNode::UnitVariant
        | ClassifyNode::Mixed
        | ClassifyNode::MixedVariant = classification
        {
            // The node must be converted to a JSON object.
            return self.node_to_json_object(node).map(Into::into);
        }

        match classification {
            ClassifyNode::Seq => {
                let mut items = Vec::with_capacity(node.children().len() + node.args().len());
                for arg in node.args() {
                    items.push(self.value_to_json(&arg.value)?);
                }
                for child in node.children() {
                    items.push(self.node_to_json(&child)?);
                }
                Ok(serde_json::Value::Array(items))
            }
            ClassifyNode::Value => {
                if let Some(first_arg) = node.args().get(0) {
                    return self.value_to_json(&first_arg.value);
                } else if let Some(first_child) = node.children().get(0) {
                    return self.node_to_json(&first_child);
                }
                Ok(serde_json::Value::Null)
            }
            ClassifyNode::Unit => Ok(serde_json::Value::Null),
            _ => unreachable!(), // Handled above
        }
    }

    fn node_to_json_object(
        &self,
        node: &crate::Node,
    ) -> Result<serde_json::Map<String, serde_json::Value>> {
        let mut obj = serde_json::Map::with_capacity(
            node.children().len() + node.args().len() + node.ty().is_some() as usize,
        );

        let mut items = Vec::new();

        for arg in node.args() {
            if let Some(name) = arg.name {
                obj.insert(name.to_owned(), self.value_to_json(&arg.value)?);
            } else {
                items.push(self.value_to_json(&arg.value)?);
            }
        }

        for child in node.children() {
            if let Some(name) = child.name() {
                obj.insert(name.to_owned(), self.node_to_json(&child)?);
            } else {
                items.push(self.node_to_json(&child)?);
            }
        }

        if let Some(ty) = node.ty() {
            let had_ty = obj
                .insert(self.type_tag.to_owned(), ty.to_owned().into())
                .is_some();
            assert!(!had_ty, "type tag clobbered field: {}", self.type_tag);
        }

        if !items.is_empty() {
            if items.len() == 1 {
                let had_value = obj
                    .insert(self.value_tag.to_owned(), items.remove(0))
                    .is_some();
                assert!(
                    !had_value,
                    "single value clobbered field: {}",
                    self.value_tag
                );
            } else {
                let had_items = obj
                    .insert(self.items_tag.to_owned(), items.into())
                    .is_some();
                assert!(
                    !had_items,
                    "unnamed items overflow clobbered field: {}",
                    self.items_tag
                );
            }
        }

        Ok(obj)
    }

    #[expect(clippy::unused_self)]
    fn value_to_json(&self, value: &crate::ValueRef) -> Result<serde_json::Value> {
        Ok(match value {
            crate::ValueRef::Null => serde_json::Value::Null,
            crate::ValueRef::Bool(value) => serde_json::Value::Bool(*value),
            crate::ValueRef::Int(value) => serde_json::Value::Number(
                serde_json::Number::from_i128(*value as _)
                    .ok_or(Error::UnrepresentableInt(*value))?,
            ),
            crate::ValueRef::Uint(value) => serde_json::Value::Number(
                serde_json::Number::from_u128(*value as _)
                    .ok_or(Error::UnrepresentableUint(*value))?,
            ),
            crate::ValueRef::Float(value) => serde_json::Value::Number(
                serde_json::Number::from_f64(*value).ok_or(Error::UnrepresentableFloat(*value))?,
            ),
            crate::ValueRef::String(value) => serde_json::Value::String((*value).to_owned()),
            crate::ValueRef::Binary(_) => return Err(Error::UnrepresentableBinary),
        })
    }

    #[inline]
    fn json_to_node<'a>(&self, value: &'a serde_json::Value) -> builder::Node<'a> {
        self.json_to_entry(value).into()
    }

    fn json_to_entry<'a>(&self, value: &'a serde_json::Value) -> builder::Entry<'a> {
        match value {
            serde_json::Value::Null => builder::Value::Null.into(),
            serde_json::Value::Bool(value) => builder::Value::Bool(*value).into(),
            serde_json::Value::Number(number) => {
                if let Some(int) = number.as_u64() {
                    builder::Value::Uint(int).into()
                } else if let Some(uint) = number.as_i64() {
                    builder::Value::Int(uint).into()
                } else if let Some(float) = number.as_f64() {
                    builder::Value::Float(float).into()
                } else {
                    panic!("no suitable representation for number: {}", number);
                }
            }
            serde_json::Value::String(s) => builder::Value::String(Cow::Borrowed(s)).into(),
            serde_json::Value::Array(values) => {
                let mut node = builder::Node::empty();
                for value in values {
                    node.push_ordered(self.json_to_entry(value));
                }
                builder::Entry::Child(node)
            }
            serde_json::Value::Object(map) => self.json_object_to_node(map).into(),
        }
    }

    fn json_object_to_node<'a>(
        &self,
        map: &'a serde_json::Map<String, serde_json::Value>,
    ) -> builder::Node<'a> {
        let mut node = builder::Node::empty();
        for (k, v) in map {
            if k == self.type_tag {
                if let Some(ty) = v.as_str() {
                    node.set_ty(ty);
                }
            } else if k == self.items_tag {
                if let Some(list) = v.as_array() {
                    for item in list {
                        node.push_ordered(self.json_to_entry(item));
                    }
                }
            } else if k == self.value_tag {
                let value = self.json_to_entry(v);
                node.push_ordered(value);
            } else {
                let mut value = self.json_to_entry(v);
                value.set_name(k);
                node.push(value);
            }
        }

        node
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString as _;

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
            // Unnamed children should end up in an `$items` field.
            root.push(builder::Value::String("child".into()));
            root.push(builder::Value::Int(123));
        });
        let doc = builder.build();

        let json_value = document_to_json_value(&doc).unwrap();
        let json = json_value.to_string();
        assert_eq!(
            json,
            r#"{"$items":["child",123],"$type":"Root","key":"value"}"#
        );

        let doc = document_from_json_value(&json_value);
        let root = doc.root();
        assert_eq!(root.ty(), Some("Root"));
        assert_eq!(root.args().len(), 3);
        // Note: `serde_json` is not built with `preserve_order` enabled, and
        // fields will be deserialized in the order they appear in the JSON.
        assert_eq!(root.args().get(0).unwrap().name, None);
        assert_eq!(
            root.args().get(0).unwrap().value,
            crate::ValueRef::String("child")
        );
        assert_eq!(root.args().get(1).unwrap().name, None);
        assert_eq!(
            root.args().get(1).unwrap().value,
            // Since the number is representable as a `u64`, it will be
            // converted.
            crate::ValueRef::Uint(123)
        );
        assert_eq!(root.args().get(2).unwrap().name, Some("key"));
        assert_eq!(
            root.args().get(2).unwrap().value,
            crate::ValueRef::String("value")
        );
    }
}
