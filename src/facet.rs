use facet_core::{Facet, FieldError, Shape};
use facet_reflect::ReflectError;

use crate::Document;

mod de;
mod ser;

#[derive(thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("expected a value")]
    ExpectedValue,
    #[error("expected scalar of type `{0}`, got: {1}")]
    ExpectedScalar(&'static str, &'static Shape),
    #[error("expected a list")]
    ExpectedList,
    #[error("expected a string to initialize {0}")]
    ExpectedString(&'static Shape),
    #[error("expected enum")]
    ExpectedEnum(&'static Shape),
    #[error("map keys must be strings, got: {0}")]
    NonStringMapKey(&'static Shape),
    #[error("unsupported value type: {0}")]
    UnsupportedValue(&'static Shape),
    #[error("{0}: {1}")]
    Field(FieldError, &'static Shape),
    #[error(transparent)]
    Reflect(#[from] ReflectError),
    #[error("unexpected shape: {0}")]
    UnexpectedShape(&'static Shape),
}

impl core::fmt::Debug for Error {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self, f)
    }
}

/// Deserialize a document into a facet type.
///
/// # Errors
///
/// If the document does not match the shape of `T`, this returns an error.
pub fn from_document<T: Facet>(doc: &Document) -> Result<T, Error> {
    from_document_node(doc.root())
}

/// Deserialize a document node into a facet type.
///
/// # Errors
///
/// If the node does not match the shape of `T`, this returns an error.
pub fn from_document_node<T: Facet>(node: crate::Node) -> Result<T, Error> {
    let wip = de::deserialize_node(facet_reflect::Wip::alloc::<T>(), &node)?;
    wip.build()
        .and_then(facet_reflect::HeapValue::materialize)
        .map_err(Into::into)
}

/// Deserialize a builder into a facet type.
///
/// # Errors
///
/// If the document does not match the shape of `T`, this returns an error.
#[cfg(feature = "alloc")]
pub fn from_builder<T: Facet>(builder: &crate::Builder) -> Result<T, Error> {
    from_builder_node(builder.root())
}

/// Deserialize a builder node into a facet type.
///
/// # Errors
///
/// If the node does not match the shape of `T`, this returns an error.
#[cfg(feature = "alloc")]
pub fn from_builder_node<T: Facet>(node: &crate::builder::Node) -> Result<T, Error> {
    let wip = de::deserialize_node(facet_reflect::Wip::alloc::<T>(), &node)?;
    wip.build()
        .and_then(facet_reflect::HeapValue::materialize)
        .map_err(Into::into)
}

/// Serialize a facet type into a builder that can be modified further.
///
/// # Errors
///
/// If the facet type could not be serialized as a builder, this returns an error.
#[cfg(feature = "alloc")]
pub fn to_builder<T: Facet>(value: &T) -> Result<crate::Builder, Error> {
    let mut builder = crate::Builder::new();
    builder.set_root(to_builder_node(value)?);
    Ok(builder)
}

/// Serialize a facet type into a document.
///
/// # Errors
///
/// If the facet type could not be serialized as a builder node, this returns an
/// error.
#[cfg(feature = "alloc")]
pub fn to_builder_node<T: Facet>(value: &T) -> Result<crate::builder::Node, Error> {
    let peek = facet_reflect::Peek::new(value);
    ser::serialize_as_node(peek)
}

/// Serialize a facet type into a document.
///
/// # Errors
///
/// If the facet type could not be serialized as a builder, this returns an error.
#[cfg(feature = "alloc")]
pub fn to_document<T: Facet>(value: &T) -> Result<crate::DocumentBuffer, Error> {
    let builder = to_builder(value)?;
    Ok(builder.build())
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use crate::builder::{self, Arg, Value};

    use super::*;
    use alloc::{borrow::Cow, collections::btree_map::BTreeMap, string::String, vec, vec::Vec};
    use facet::Facet;

    #[derive(Facet, Debug, PartialEq)]
    #[repr(u8)]
    enum Enum {
        UnitVariant,
        NewTypeValue(i32),
        // Nested(Box<Enum>),
        Struct { int: i32, string: String },
        Tuple(i32, String),
    }

    #[derive(Facet, Debug, PartialEq)]
    struct Struct {
        string: String,
        int: i32,
        enum_: Enum,
        vec: Vec<i32>,
    }

    #[test]
    fn unit_variant() {
        let doc = to_document(&Enum::UnitVariant).unwrap();
        assert_eq!(doc.as_bytes().len(), 107);
        assert_eq!(doc.root().ty(), Some("UnitVariant"));
        let de = from_document::<Enum>(&doc).unwrap();
        assert_eq!(de, Enum::UnitVariant);
    }

    #[test]
    fn newtype_variant() {
        let doc = to_document(&Enum::NewTypeValue(123)).unwrap();
        assert_eq!(
            doc.root(),
            builder::Node::from_args([Value::Int(123)]).with_ty("NewTypeValue")
        );
        let de = from_document::<Enum>(&doc).unwrap();
        assert_eq!(de, Enum::NewTypeValue(123));
    }

    #[test]
    fn struct_variant() {
        let doc = to_document(&Enum::Struct {
            int: 123,
            string: String::from("hello"),
        })
        .unwrap();
        assert_eq!(
            doc.root(),
            builder::Node::from_args([
                Arg {
                    name: Some("int".into()),
                    value: Value::Int(123),
                },
                Arg {
                    name: Some("string".into()),
                    value: Value::String("hello".into())
                }
            ])
            .with_ty("Struct")
        );
        let de = from_document::<Enum>(&doc).unwrap();
        assert_eq!(
            de,
            Enum::Struct {
                int: 123,
                string: String::from("hello")
            }
        );
    }

    #[test]
    fn tuple_variant() {
        let doc = to_document(&Enum::Tuple(123, String::from("hello"))).unwrap();
        assert_eq!(
            doc.root(),
            builder::Node::from_args([Value::Int(123), Value::String("hello".into())])
                .with_ty("Tuple")
        );
        let de = from_document::<Enum>(&doc).unwrap();
        assert_eq!(de, Enum::Tuple(123, String::from("hello")));
    }

    #[test]
    fn struct_with_unit_variant() {
        let struct_ = Struct {
            string: String::from("hello"),
            int: 123,
            enum_: Enum::UnitVariant,
            vec: vec![1, 2, 3],
        };
        let doc = to_document(&struct_).unwrap();
        assert_eq!(
            doc.root(),
            &*builder::Node::from_args([
                Arg {
                    name: Some("string".into()),
                    value: Value::String("hello".into())
                },
                Arg {
                    name: Some("int".into()),
                    value: Value::Int(123),
                },
                Arg {
                    name: Some("enum_".into()),
                    value: Value::String("UnitVariant".into())
                }
            ])
            .add_child_with(|child| {
                child.set_name("vec").set_args([1i32, 2, 3]);
            })
        );
        let de = from_document::<Struct>(&doc).unwrap();
        assert_eq!(de, struct_);
    }

    #[test]
    fn tuple() {
        let tuple = (123i32, Enum::UnitVariant, String::from("hello"));
        let doc = to_document(&tuple).unwrap();
        assert_eq!(
            doc.root(),
            &*builder::Node::empty()
                .push_arg(123i32)
                .push_arg("UnitVariant")
                .push_ordered("hello")
        );
        let de: (i32, Enum, String) = from_document(&doc).unwrap();
        assert_eq!(de, tuple);
    }

    #[test]
    fn mapping_newtype_key() {
        #[derive(Facet, PartialEq, Eq, PartialOrd, Ord, Debug)]
        struct Key(String);
        let mut map = BTreeMap::new();
        map.insert(Key(String::from("a")), 1i32);
        map.insert(Key(String::from("b")), 2i32);
        map.insert(Key(String::from("c")), 3i32);
        let builder = to_builder(&map).unwrap();
        let doc = builder.build();
        assert_eq!(doc.root(), builder.root());
        assert_eq!(
            doc.root(),
            builder::Node {
                children: Cow::Borrowed(&[]),
                args: Cow::Borrowed(&[
                    builder::Arg {
                        name: Some("a".into()),
                        value: builder::Value::Int(1),
                    },
                    builder::Arg {
                        name: Some("b".into()),
                        value: builder::Value::Int(2),
                    },
                    builder::Arg {
                        name: Some("c".into()),
                        value: builder::Value::Int(3),
                    }
                ]),
                name: "".into(),
                ty: "".into()
            }
        );
        let map1: BTreeMap<Key, i32> = from_document(&doc).unwrap();
        let map2: BTreeMap<Key, i32> = from_builder(&builder).unwrap();
        assert_eq!(map1, map);
        assert_eq!(map2, map);
    }
}
