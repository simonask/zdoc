//! Convert between YAML and [`Document`].
//!
//! Key-value nodes are represented as YAML mappings, and sequence-like nodes are
//! represented as YAML sequences.
//!
//! Nodes that have a "type" will be represented as YAML tags (`!type`). Nodes
//! that have a mix of named and unnamed children will gain an `$items` field,
//! which contains a YAML sequence of the unnamed items. Nodes that have a
//! single unnamed child will gain a special `$value` field, which contains the
//! unnamed child.
//!
//! **Note:** Conversion to and from YAML is lossy, because YAML does not
//! distinguish between attributes and children. All arguments and children of a
//! node are treated as fields (when named) or sequence items (when unnamed).
//!
//! Similarly, [`Document`] cannot distinguish between single-element sequences
//! and single-value nodes. For example, a YAML sequence with a single element
//! will be represented as just the element, losing the "array" information, so
//! a roundtrip through [`Document`] is not lossless.
//!
//! Due to the way that the [`Deserialize`](serde::Deserialize) trait is
//! implemented for [`Document`], converting YAML to a [`Document`] and
//! deserializing something from it should behave identially to `serde_yaml`.

use alloc::{
    borrow::{Cow, ToOwned as _},
    boxed::Box,
    string::ToString as _,
    vec::Vec,
};

use crate::{Builder, ClassifyNode, Document, DocumentBuffer, Error, Result, builder};

/// Settings for converting between YAML and [`Document`].
pub struct YamlSettings<'a> {
    /// When the children of a node are mixed named and unnamed, overflow
    /// unnamed children into a list with this name. Default is `"$items"`.
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

impl Default for YamlSettings<'_> {
    #[inline]
    fn default() -> Self {
        Self {
            items_tag: "$items",
            value_tag: "$value",
        }
    }
}

/// Convert [`Document`] to YAML.
///
/// # Errors
///
/// If the document cannot be represented as YAML, or if any fields in the
/// document conflict with the default YAML settings, this returns an error.
#[inline]
pub fn document_to_yaml_value(doc: &Document) -> Result<serde_yaml::Value> {
    document_to_yaml_value_with_settings(doc, &YamlSettings::default())
}

/// Convert [`Document`] to YAML.
///
/// # Errors
///
/// If the document cannot be represented as YAML, or if any fields in the
/// document conflict with the default YAML settings, this returns an error.
#[inline]
pub fn document_to_yaml(doc: &Document) -> Result<alloc::string::String> {
    document_to_yaml_with_settings(doc, &YamlSettings::default())
}

/// Convert [`Document`] to YAML.
///
/// # Errors
///
/// If the document cannot be represented as YAML, or if any fields in the
/// document conflict with the YAML settings, this returns an error.
#[inline]
pub fn document_to_yaml_value_with_settings(
    doc: &Document,
    settings: &YamlSettings,
) -> Result<serde_yaml::Value> {
    settings.node_to_yaml(&doc.root())
}

/// Convert [`Document`] to YAML.
///
/// # Errors
///
/// If the document cannot be represented as YAML, or if any fields in the
/// document conflict with the YAML settings, this returns an error.
#[inline]
pub fn document_to_yaml_with_settings(
    doc: &Document,
    settings: &YamlSettings,
) -> Result<alloc::string::String> {
    let value = settings.node_to_yaml(&doc.root())?;
    serde_yaml::to_string(&value).map_err(Error::custom)
}

/// Convert YAML to [`Document`].
///
/// This is an infallible conversion.
#[inline]
#[must_use]
pub fn document_from_yaml_value(value: &serde_yaml::Value) -> DocumentBuffer {
    document_from_yaml_value_with_settings(value, &YamlSettings::default())
}

/// Convert YAML to [`Document`].
///
/// # Errors
///
/// If `yaml` is not valid YAML syntax, this returns an error.
#[inline]
pub fn document_from_yaml(yaml: &str) -> Result<DocumentBuffer> {
    let yaml = serde_yaml::from_str(yaml).map_err(Error::custom)?;
    Ok(document_from_yaml_value_with_settings(
        &yaml,
        &YamlSettings::default(),
    ))
}

/// Convert YAML to [`Document`].
///
/// This is an infallible conversion.
#[inline]
#[must_use]
pub fn document_from_yaml_value_with_settings(
    value: &serde_yaml::Value,
    settings: &YamlSettings,
) -> DocumentBuffer {
    builder_from_yaml_value_with_settings(value, settings).build()
}

/// Convert YAML to [`Document`].
///
/// # Errors
///
/// If `yaml` is not valid YAML syntax, this returns an error.
#[inline]
pub fn document_from_yaml_with_settings(
    yaml: &str,
    settings: &YamlSettings,
) -> Result<DocumentBuffer> {
    let yaml = serde_yaml::from_str(yaml).map_err(Error::custom)?;
    Ok(document_from_yaml_value_with_settings(&yaml, settings))
}

/// Convert YAML to [`Builder`], which can be modified further.
///
/// Strings from the YAML value are borrowed, not cloned, so the YAML value must
/// outlive the returned builder.
///
/// This is an infallible conversion.
#[must_use]
#[inline]
pub fn builder_from_yaml_value(value: &serde_yaml::Value) -> Builder<'_> {
    builder_from_yaml_value_with_settings(value, &YamlSettings::default())
}

/// Convert YAML to [`Builder`] with default settings.
///
/// # Errors
///
/// If `yaml` is not valid YAML syntax, this returns an error.
#[inline]
pub fn builder_from_yaml(yaml: &str) -> Result<Builder<'static>> {
    builder_from_yaml_with_settings(yaml, &YamlSettings::default())
}

/// Convert YAML to [`Builder`], which can be modified further.
///
/// # Errors
///
/// If `yaml` is not valid YAML syntax, this returns an error.
#[inline]
pub fn builder_from_yaml_with_settings(
    yaml: &str,
    settings: &YamlSettings,
) -> Result<Builder<'static>> {
    let yaml = serde_yaml::from_str(yaml).map_err(Error::custom)?;
    Ok(builder_from_yaml_value_with_settings(&yaml, settings).into_static())
}

#[must_use]
#[inline]
pub fn builder_from_yaml_value_with_settings<'a>(
    value: &'a serde_yaml::Value,
    settings: &YamlSettings,
) -> Builder<'a> {
    settings.yaml_to_builder(value)
}

impl YamlSettings<'_> {
    #[inline]
    fn yaml_to_builder<'a>(&self, value: &'a serde_yaml::Value) -> Builder<'a> {
        let mut builder = Builder::new();
        builder.set_root(self.yaml_to_node(value));
        builder
    }

    fn node_to_yaml(&self, node: &crate::Node) -> Result<serde_yaml::Value> {
        let classification = node.classify();

        let value = match classification {
            ClassifyNode::Struct
            | ClassifyNode::StructVariant
            | ClassifyNode::Mixed
            | ClassifyNode::MixedVariant => self.node_to_yaml_mapping(node)?.into(),
            ClassifyNode::Seq | ClassifyNode::SeqVariant => {
                let mut items = Vec::with_capacity(node.children().len() + node.args().len());
                for arg in node.args() {
                    items.push(self.value_to_yaml(&arg.value)?);
                }
                for child in node.children() {
                    items.push(self.node_to_yaml(&child)?);
                }
                serde_yaml::Value::Sequence(items)
            }
            ClassifyNode::Value | ClassifyNode::ValueVariant => {
                if let Some(first_arg) = node.args().get(0) {
                    self.value_to_yaml(&first_arg.value)?
                } else if let Some(first_child) = node.children().get(0) {
                    self.node_to_yaml(&first_child)?
                } else {
                    serde_yaml::Value::Null
                }
            }
            ClassifyNode::Unit | ClassifyNode::UnitVariant => serde_yaml::Value::Null,
        };

        if let Some(ty) = node.ty() {
            Ok(serde_yaml::Value::Tagged(Box::new(
                serde_yaml::value::TaggedValue {
                    tag: serde_yaml::value::Tag::new(ty),
                    value,
                },
            )))
        } else {
            Ok(value)
        }
    }

    fn node_to_yaml_mapping(&self, node: &crate::Node) -> Result<serde_yaml::Mapping> {
        let mut obj = serde_yaml::Mapping::with_capacity(
            node.children().len() + node.args().len() + node.ty().is_some() as usize,
        );

        let mut items = Vec::new();

        for arg in node.args() {
            if let Some(name) = arg.name {
                obj.insert(name.to_owned().into(), self.value_to_yaml(&arg.value)?);
            } else {
                items.push(self.value_to_yaml(&arg.value)?);
            }
        }

        for child in node.children() {
            if let Some(name) = child.name() {
                obj.insert(name.to_owned().into(), self.node_to_yaml(&child)?);
            } else {
                items.push(self.node_to_yaml(&child)?);
            }
        }

        if !items.is_empty() {
            if items.len() == 1 {
                let had_value = obj
                    .insert(self.value_tag.to_owned().into(), items.remove(0))
                    .is_some();
                assert!(
                    !had_value,
                    "single value clobbered field: {}",
                    self.value_tag
                );
            } else {
                let had_items = obj
                    .insert(self.items_tag.to_owned().into(), items.into())
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
    fn value_to_yaml(&self, value: &crate::ValueRef) -> Result<serde_yaml::Value> {
        Ok(match value {
            crate::ValueRef::Null => serde_yaml::Value::Null,
            crate::ValueRef::Bool(value) => serde_yaml::Value::Bool(*value),
            crate::ValueRef::Int(value) => {
                serde_yaml::Value::Number(serde_yaml::Number::from(*value))
            }
            crate::ValueRef::Uint(value) => {
                serde_yaml::Value::Number(serde_yaml::Number::from(*value))
            }
            crate::ValueRef::Float(value) => {
                serde_yaml::Value::Number(serde_yaml::Number::from(*value))
            }
            crate::ValueRef::String(value) => serde_yaml::Value::String((*value).to_owned()),
            crate::ValueRef::Binary(_) => return Err(Error::UnrepresentableBinary),
        })
    }

    #[inline]
    fn yaml_to_node<'a>(&self, value: &'a serde_yaml::Value) -> builder::Node<'a> {
        self.yaml_to_entry(value).into()
    }

    fn yaml_to_entry<'a>(&self, value: &'a serde_yaml::Value) -> builder::Entry<'a> {
        match value {
            serde_yaml::Value::Null => builder::Value::Null.into(),
            serde_yaml::Value::Bool(value) => builder::Value::Bool(*value).into(),
            serde_yaml::Value::Number(number) => {
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
            serde_yaml::Value::String(s) => builder::Value::String(Cow::Borrowed(s)).into(),
            serde_yaml::Value::Sequence(values) => {
                let mut node = builder::Node::empty();
                for value in values {
                    node.push_ordered(self.yaml_to_entry(value));
                }
                builder::Entry::Child(node)
            }
            serde_yaml::Value::Mapping(map) => self.yaml_mapping_to_node(map).into(),
            serde_yaml::Value::Tagged(tagged) => {
                let mut node = self.yaml_to_node(&tagged.value);
                // The tag is a string, but `serde_yaml` does not provide a way
                // to access it directly.
                let tag = tagged.tag.to_string();
                let unbanged = tag.strip_prefix("!").unwrap_or(&tag);
                node.set_ty(unbanged.to_owned());
                builder::Entry::Child(node)
            }
        }
    }

    fn yaml_mapping_to_node<'a>(&self, map: &'a serde_yaml::Mapping) -> builder::Node<'a> {
        let mut node = builder::Node::empty();
        for (k, v) in map {
            if k == self.items_tag {
                if let Some(list) = v.as_sequence() {
                    for item in list {
                        node.push_ordered(self.yaml_to_entry(item));
                    }
                }
            } else if k == self.value_tag {
                let value = self.yaml_to_entry(v);
                node.push_ordered(value);
            } else if let Some(k) = k.as_str() {
                let mut value = self.yaml_to_entry(v);
                value.set_name(k);
                node.push(value);
            } else {
                panic!("non-string keys are not supported")
            }
        }

        node
    }
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
            // Unnamed children should end up in an `$items` field.
            root.push(builder::Value::String("child".into()));
            root.push(builder::Value::Int(123));
        });
        let doc = builder.build();

        let yaml_value = document_to_yaml_value(&doc).unwrap();
        let yaml = serde_yaml::to_string(&yaml_value).unwrap();
        assert_eq!(yaml, "!Root\nkey: value\n$items:\n- child\n- 123\n");

        let doc = document_from_yaml_value(&yaml_value);
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
