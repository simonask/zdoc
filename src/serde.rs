mod de;
mod error;
mod ser;

pub use error::*;

/// Serialize a value to a [`Builder`](crate::Builder), which can be used to
/// make further modifications.
///
/// # Conventions
///
/// zdoc is a very flexible format, so for the purposes of serialization, we are
/// choosing a couple of conventional behaviors. These mostly match the
/// conventions of the KDL language.
///
/// zdoc represents "values" as arguments to "nodes". Compound values can
/// consist of both arguments and children with arguments, but arguments cannot
/// themselves be compound (like properties in XML).
///
/// ## Lists and tuples
///
/// 1. The list itself is always a child node, because it is compound.
/// 2. If every entry in a list/tuple is a simple value, all of its entries are
///    serialized as unnamed arguments.
/// 3. If *any* entry is a compound value (a list, map, or typed struct), *all*
///    entries in the list or tuple are represented as (unnamed) children of the
///    list instead of unnamed arguments.
///
/// ## Maps (dictionaries)
///
/// 1. The map itself is always a full node, because it is compound.
/// 2. All keys in the map must be serializable as strings. If any key is not a string,
///    this returns an error.
/// 3. If all values in the map are simple values, all of its entries are
///    serialized as named arguments, where the name of each argument is the
///    key.
/// 4. If *any* value is a compound value (a list, map, or typed struct), *all*
///    entries in the map are represented as (named) children of the map
/// 5. The order of map entries is always preserved.
///
/// ## Structs with named fields
///
/// Structs are like maps, but they differ in the following ways:
///
/// 1. All keys are guaranteed to be strings.
/// 2. Fields of the struct are serialized as a mix of named arguments and named
///    children, depending on the type of the field, which implies that the
///    natural ordering between fields is not preserved, but it results in a
///    more compact document.
///
/// ## Other
///
/// 1. `None` and the unit type `()` are serialized as a null value. When
///    represented as a child node, it is represented by the empty node.
/// 3. Empty enum variants (unit variants) are serialized as a string with the
///    name of the variant.
/// 4. Newtype structs (`struct Foo(Bar)`) are serialized as the inner value of
///    the struct. The `type` field is *not* set.
/// 5. Newtype enum variants (`enum Foo { Bar(Baz) }`) are serialized as the
///    inner value of the variant. The `type` field is set to the name of the
///    variant.
/// 6. Tuple structs are serialized as normal tuples. The `type` field is *not*
///    set.
/// 7. Tuple enum variants are serialized as lists, but the `type` field is set
///    to the name of the variant.
/// 8. Struct enum variants are serialized as structs, but the `type` field is
///    set to the name of the variant.
///
/// # Errors
///
/// If the value cannot be serialized, this returns an error.
pub fn to_builder<'a, T: serde::Serialize>(value: &T) -> Result<crate::Builder<'a>, Error> {
    let mut builder = crate::Builder::new();
    value.serialize(builder.root_mut())?;
    Ok(builder)
}

/// Serialize a value to a linear, immutable
/// [`DocumentBuffer`](crate::DocumentBuffer).
///
/// # Errors
///
/// If the value cannot be serialized, this returns an error.
pub fn to_document<T: serde::Serialize>(value: &T) -> Result<crate::DocumentBuffer, Error> {
    to_builder(value).map(|builder| builder.build())
}

/// Deserialize a [`Builder`](crate::Builder) into a value of type `T`,
/// borrowing strings and binary data from the document.
///
/// See [`to_builder`] for the conventions and assumptions of the structure of
/// serialized data.
///
/// # Errors
///
/// If the document cannot be deserialized into `T`, this returns an error.
pub fn from_builder<'a, T: serde::Deserialize<'a>>(
    builder: &'a crate::Builder,
) -> Result<T, Error> {
    serde::de::Deserialize::deserialize(de::DeNode(builder.root()))
}

/// Deserialize a [`Document`](crate::Document) into a value of type `T`,
/// borrowing strings and binary data from the document.
///
/// See [`to_document_builder`] for the conventions and assumptions of the
/// structure of serialized data.
///
/// # Errors
///
/// If the document cannot be deserialized into `T`, this returns an error.
pub fn from_document<'a, T: serde::Deserialize<'a>>(
    document: &'a crate::Document,
) -> Result<T, Error> {
    serde::de::Deserialize::deserialize(de::DeNode(document.root()))
}

impl<'de> serde::de::IntoDeserializer<'de, Error> for crate::Node<'de> {
    type Deserializer = de::DeNode<Self>;

    #[inline]
    fn into_deserializer(self) -> Self::Deserializer {
        de::DeNode(self)
    }
}

impl<'de> serde::de::IntoDeserializer<'de, Error> for &'de crate::builder::Node<'de> {
    type Deserializer = de::DeNode<Self>;

    #[inline]
    fn into_deserializer(self) -> Self::Deserializer {
        de::DeNode(self)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use alloc::{
        borrow::{Cow, ToOwned as _},
        boxed::Box,
        collections::btree_map::BTreeMap,
        string::{String, ToString as _},
        vec,
        vec::Vec,
    };
    use serde::{Deserialize, de::IntoDeserializer};

    use crate::{Arg, ValueRef, builder};

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Bytes(Vec<u8>);
    impl serde::Serialize for Bytes {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_bytes(&self.0)
        }
    }
    impl<'de> serde::Deserialize<'de> for Bytes {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            struct Visitor;
            impl serde::de::Visitor<'_> for Visitor {
                type Value = Bytes;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    write!(formatter, "expected binary")
                }

                fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    Ok(Bytes(v.to_owned()))
                }
            }

            deserializer.deserialize_bytes(Visitor)
        }
    }

    #[test]
    fn unit() {
        let doc = to_document(&()).unwrap();
        assert_eq!(doc.as_bytes().len(), 64); // just the header
        assert!(doc.is_empty());
        let _: () = from_document(&doc).unwrap();
    }

    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    enum Enum {
        UnitVariant,
        NewTypeValue(i32),
        Nested(Box<Enum>),
    }

    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
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
        assert!(!doc.is_empty());
        assert_eq!(doc.root().ty(), Some("UnitVariant"));
        assert!(doc.root().is_empty());
        let de = from_document::<Enum>(&doc).unwrap();
        assert_eq!(de, Enum::UnitVariant);
    }

    #[test]
    fn newtype_variant() {
        let doc = to_document(&Enum::NewTypeValue(123)).unwrap();
        assert_eq!(doc.as_bytes().len(), 128);
        assert!(!doc.is_empty());
        assert_eq!(doc.root().ty(), Some("NewTypeValue"));
        assert!(doc.root().args().len() == 1);
        assert_eq!(
            doc.root().args().get(0),
            Some(Arg::from(ValueRef::Int(123)))
        );
        let de = from_document::<Enum>(&doc).unwrap();
        assert_eq!(de, Enum::NewTypeValue(123));
    }

    #[test]
    fn nested_unit_variant() {
        let doc = to_document(&Enum::Nested(Box::new(Enum::UnitVariant))).unwrap();
        assert_eq!(doc.as_bytes().len(), 133);
        assert!(!doc.is_empty());
        assert_eq!(doc.root().ty(), Some("Nested"));
        assert!(doc.root().args().len() == 1);
        let nested = doc.root().args().get(0).unwrap().value;
        assert_eq!(nested, ValueRef::String("UnitVariant"));
        let de = from_document::<Enum>(&doc).unwrap();
        assert_eq!(de, Enum::Nested(Box::new(Enum::UnitVariant)));
    }

    #[test]
    fn nested_newtype_variant() {
        let doc = to_document(&Enum::Nested(Box::new(Enum::NewTypeValue(123)))).unwrap();
        assert_eq!(doc.as_bytes().len(), 166);
        assert!(!doc.is_empty());
        assert_eq!(doc.root().ty(), Some("Nested"));
        assert!(doc.root().children().len() == 1);
        let nested = doc.root().children().get(0).unwrap();
        assert_eq!(nested.ty(), Some("NewTypeValue"));
        assert_eq!(nested.args().len(), 1);
        assert_eq!(nested.args().get(0), Some(Arg::from(ValueRef::Int(123))));
        let de = from_document::<Enum>(&doc).unwrap();
        assert_eq!(de, Enum::Nested(Box::new(Enum::NewTypeValue(123))));
    }

    #[test]
    fn nested_nested_newtype_variant() {
        let doc = to_document(&Enum::Nested(Box::new(Enum::Nested(Box::new(
            Enum::NewTypeValue(123),
        )))))
        .unwrap();
        assert_eq!(doc.as_bytes().len(), 198);
        assert!(!doc.is_empty());
        assert_eq!(doc.root().ty(), Some("Nested"));
        assert!(doc.root().children().len() == 1);
        let nested1 = doc.root().children().get(0).unwrap();
        assert_eq!(nested1.ty(), Some("Nested"));
        assert!(nested1.children().len() == 1);
        let nested2 = nested1.children().get(0).unwrap();
        assert_eq!(nested2.ty(), Some("NewTypeValue"));
        assert_eq!(nested2.args().len(), 1);
        assert_eq!(nested2.args().get(0), Some(Arg::from(ValueRef::Int(123))));
        let de = from_document::<Enum>(&doc).unwrap();
        assert_eq!(
            de,
            Enum::Nested(Box::new(Enum::Nested(Box::new(Enum::NewTypeValue(123)))))
        );
    }

    #[test]
    fn struct_with_unit_variant() {
        let doc = to_document(&Struct {
            string: "hello".to_string(),
            int: 123,
            enum_: Enum::UnitVariant,
            vec: vec![],
        })
        .unwrap();
        assert_eq!(doc.as_bytes().len(), 221);
        let root = doc.root();
        assert!(!root.is_empty());
        assert_eq!(root.ty(), None);
        assert_eq!(root.args().len(), 3);
        assert_eq!(root.children().len(), 1);
        assert_eq!(
            root.args().get("string").unwrap().value,
            ValueRef::String("hello")
        );
        assert_eq!(root.args().get(0).unwrap().value, ValueRef::String("hello"));
        assert_eq!(root.args().get("int").unwrap().value, ValueRef::Int(123));
        assert_eq!(root.args().get(1).unwrap().value, ValueRef::Int(123));
        assert_eq!(
            root.args().get("enum_").unwrap().value,
            ValueRef::String("UnitVariant")
        );
        assert_eq!(
            root.args().get(2).unwrap().value,
            ValueRef::String("UnitVariant")
        );
        let de = from_document::<Struct>(&doc).unwrap();
        assert_eq!(
            de,
            Struct {
                string: "hello".to_string(),
                int: 123,
                enum_: Enum::UnitVariant,
                vec: vec![],
            }
        );
    }

    #[test]
    fn struct_with_newtype_variant() {
        let doc = to_document(&Struct {
            string: "hello".to_string(),
            int: 123,
            enum_: Enum::NewTypeValue(456),
            vec: vec![],
        })
        .unwrap();
        assert_eq!(doc.as_bytes().len(), 254);
        let root = doc.root();
        assert!(!root.is_empty());
        assert_eq!(root.ty(), None);
        assert_eq!(root.args().len(), 2);
        assert_eq!(root.children().len(), 2);
        assert_eq!(
            root.args().get("string").unwrap().value,
            ValueRef::String("hello")
        );
        assert_eq!(root.args().get(0).unwrap().value, ValueRef::String("hello"));
        assert_eq!(root.args().get("int").unwrap().value, ValueRef::Int(123));
        assert_eq!(root.args().get(1).unwrap().value, ValueRef::Int(123));
        let de = from_document::<Struct>(&doc).unwrap();
        assert_eq!(
            de,
            Struct {
                string: "hello".to_string(),
                int: 123,
                enum_: Enum::NewTypeValue(456),
                vec: vec![],
            }
        );
    }

    #[test]
    fn struct_with_vec1() {
        let doc = to_document(&Struct {
            string: "hello".to_string(),
            int: 123,
            enum_: Enum::UnitVariant,
            vec: vec![1],
        })
        .unwrap();
        assert_eq!(doc.as_bytes().len(), 241);
        let root = doc.root();
        assert!(!root.is_empty());
        assert_eq!(root.ty(), None);
        assert_eq!(root.args().len(), 3);
        assert_eq!(root.children().len(), 1);
        assert_eq!(
            root.args().get("string").unwrap().value,
            ValueRef::String("hello")
        );
        assert_eq!(root.args().get(0).unwrap().value, ValueRef::String("hello"));
        assert_eq!(root.args().get("int").unwrap().value, ValueRef::Int(123));
        assert_eq!(root.args().get(1).unwrap().value, ValueRef::Int(123));
        assert_eq!(
            root.args().get("enum_").unwrap().value,
            ValueRef::String("UnitVariant")
        );
        assert_eq!(
            root.args().get(2).unwrap().value,
            ValueRef::String("UnitVariant")
        );

        let list = root.children().get("vec").unwrap();
        assert_eq!(list.ty(), None);
        assert!(list.is_list_like());
        assert_eq!(list.args().len(), 1);
        assert_eq!(list.args().get(0).unwrap().value, ValueRef::Int(1));

        let de = from_document::<Struct>(&doc).unwrap();
        assert_eq!(
            de,
            Struct {
                string: "hello".to_string(),
                int: 123,
                enum_: Enum::UnitVariant,
                vec: vec![1],
            }
        );
    }

    #[test]
    fn struct_with_vec2() {
        let doc = to_document(&Struct {
            string: "hello".to_string(),
            int: 123,
            enum_: Enum::UnitVariant,
            vec: vec![1, 2],
        })
        .unwrap();
        let root = doc.root();
        assert_eq!(doc.as_bytes().len(), 261);
        assert!(!root.is_empty());
        assert_eq!(root.ty(), None);
        assert_eq!(root.args().len(), 3);
        assert_eq!(root.children().len(), 1);
        assert_eq!(
            root.args().get("string").unwrap().value,
            ValueRef::String("hello")
        );
        assert_eq!(root.args().get(0).unwrap().value, ValueRef::String("hello"));
        assert_eq!(root.args().get("int").unwrap().value, ValueRef::Int(123));
        assert_eq!(root.args().get(1).unwrap().value, ValueRef::Int(123));
        assert_eq!(
            root.args().get("enum_").unwrap().value,
            ValueRef::String("UnitVariant")
        );
        assert_eq!(
            root.args().get(2).unwrap().value,
            ValueRef::String("UnitVariant")
        );

        let list = root.children().get("vec").unwrap();
        assert_eq!(list.ty(), None);
        assert!(list.is_list_like());
        assert_eq!(list.args().len(), 2);
        assert_eq!(list.args().get(0).unwrap().value, ValueRef::Int(1));
        assert_eq!(list.args().get(1).unwrap().value, ValueRef::Int(2));

        let de = from_document::<Struct>(&doc).unwrap();
        assert_eq!(
            de,
            Struct {
                string: "hello".to_string(),
                int: 123,
                enum_: Enum::UnitVariant,
                vec: vec![1, 2],
            }
        );
    }

    #[test]
    fn primitive() {
        fn primitive_roundtrip<
            T: serde::Serialize + for<'de> serde::Deserialize<'de> + core::fmt::Debug + PartialEq,
        >(
            value: &T,
            expected: builder::Value<'_>,
        ) {
            let builder = to_builder(&value).unwrap();
            let doc = builder.build();
            assert_eq!(builder.root(), doc.root());
            assert_eq!(
                doc.root(),
                builder::Node {
                    children: Cow::Borrowed(&[]),
                    args: Cow::Borrowed(&[builder::Arg {
                        name: None,
                        value: expected,
                    }]),
                    name: "".into(),
                    ty: "".into(),
                }
            );
            let a = T::deserialize(doc.root().into_deserializer()).unwrap();
            let b = T::deserialize(builder.root().into_deserializer()).unwrap();
            assert_eq!(a, b);
        }

        primitive_roundtrip(&true, builder::Value::Bool(true));
        primitive_roundtrip(&false, builder::Value::Bool(false));
        primitive_roundtrip(&123i8, builder::Value::Int(123));
        primitive_roundtrip(&123i16, builder::Value::Int(123));
        primitive_roundtrip(&123i32, builder::Value::Int(123));
        primitive_roundtrip(&123i64, builder::Value::Int(123));
        primitive_roundtrip(&123u8, builder::Value::Uint(123));
        primitive_roundtrip(&123u16, builder::Value::Uint(123));
        primitive_roundtrip(&123u32, builder::Value::Uint(123));
        primitive_roundtrip(&123u64, builder::Value::Uint(123));
        primitive_roundtrip(&123.0f32, builder::Value::Float(123.0));
        primitive_roundtrip(&123.0f64, builder::Value::Float(123.0));
        primitive_roundtrip(
            &String::from("hello"),
            builder::Value::String("hello".into()),
        );
        primitive_roundtrip(
            &Bytes(Vec::from(&[1u8, 2, 3])),
            builder::Value::Binary((&[1, 2, 3]).into()),
        );

        primitive_roundtrip(&Some(true), builder::Value::Bool(true));
        primitive_roundtrip(&Some(false), builder::Value::Bool(false));
        primitive_roundtrip(&Some(123i8), builder::Value::Int(123));
        primitive_roundtrip(&Some(123i16), builder::Value::Int(123));
        primitive_roundtrip(&Some(123i32), builder::Value::Int(123));
        primitive_roundtrip(&Some(123i64), builder::Value::Int(123));
        primitive_roundtrip(&Some(123u8), builder::Value::Uint(123));
        primitive_roundtrip(&Some(123u16), builder::Value::Uint(123));
        primitive_roundtrip(&Some(123u32), builder::Value::Uint(123));
        primitive_roundtrip(&Some(123u64), builder::Value::Uint(123));
        primitive_roundtrip(&Some(123.0f32), builder::Value::Float(123.0));
        primitive_roundtrip(&Some(123.0f64), builder::Value::Float(123.0));
        primitive_roundtrip(
            &Some(String::from("hello")),
            builder::Value::String("hello".into()),
        );
        primitive_roundtrip(
            &Some(Bytes(Vec::from(&[1u8, 2, 3]))),
            builder::Value::Binary((&[1, 2, 3]).into()),
        );

        primitive_roundtrip(&None::<bool>, builder::Value::Null);
        primitive_roundtrip(&None::<i8>, builder::Value::Null);
        primitive_roundtrip(&None::<i16>, builder::Value::Null);
        primitive_roundtrip(&None::<i32>, builder::Value::Null);
        primitive_roundtrip(&None::<i64>, builder::Value::Null);
        primitive_roundtrip(&None::<u8>, builder::Value::Null);
        primitive_roundtrip(&None::<u16>, builder::Value::Null);
        primitive_roundtrip(&None::<u32>, builder::Value::Null);
        primitive_roundtrip(&None::<u64>, builder::Value::Null);
        primitive_roundtrip(&None::<f32>, builder::Value::Null);
        primitive_roundtrip(&None::<f64>, builder::Value::Null);
        primitive_roundtrip(&None::<String>, builder::Value::Null);
        primitive_roundtrip(&None::<Bytes>, builder::Value::Null);

        primitive_roundtrip(&Some(Some(123i32)), builder::Value::Int(123));
        primitive_roundtrip(&Some(None::<i32>), builder::Value::Null);
    }

    #[test]
    fn newtype_struct() {
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct Foo(String);
        let foo = Foo("hello".to_string());
        let builder = to_builder(&foo).unwrap();
        let doc = builder.build();
        assert_eq!(
            doc.root(),
            builder::Node {
                children: Cow::Borrowed(&[]),
                args: Cow::Borrowed(&[builder::Arg {
                    name: None,
                    value: builder::Value::String("hello".into()),
                }]),
                name: "".into(),
                ty: "".into(),
            }
        );
        assert_eq!(doc.root(), builder.root());
        let foo1 = Foo::deserialize(doc.root().into_deserializer()).unwrap();
        let foo2 = Foo::deserialize(builder.root().into_deserializer()).unwrap();
        assert_eq!(foo1, foo2);
    }

    #[test]
    fn tuple_struct() {
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct Foo(String, i32);
        let foo = Foo("hello".to_string(), 123);
        let builder = to_builder(&foo).unwrap();
        let doc = builder.build();
        assert_eq!(
            doc.root(),
            builder::Node {
                children: Cow::Borrowed(&[]),
                args: Cow::Borrowed(&[
                    builder::Arg {
                        name: None,
                        value: builder::Value::String("hello".into()),
                    },
                    builder::Arg {
                        name: None,
                        value: builder::Value::Int(123)
                    }
                ]),
                name: "".into(),
                ty: "".into(),
            }
        );
        assert_eq!(doc.root(), builder.root());
        let foo1 = Foo::deserialize(doc.root().into_deserializer()).unwrap();
        let foo2 = Foo::deserialize(builder.root().into_deserializer()).unwrap();
        assert_eq!(foo1, foo2);
    }

    #[test]
    fn mapping_newtype_key() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
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
        let map1 = BTreeMap::deserialize(doc.root().into_deserializer()).unwrap();
        let map2 = BTreeMap::deserialize(builder.root().into_deserializer()).unwrap();
        assert_eq!(map1, map);
        assert_eq!(map2, map);
    }

    #[test]
    fn mapping_enum_key() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
        enum Key {
            A,
            B,
            C,
        }

        let mut map = BTreeMap::new();
        map.insert(Key::A, 1i32);
        map.insert(Key::B, 2i32);
        map.insert(Key::C, 3i32);
        let builder = to_builder(&map).unwrap();
        let doc = builder.build();
        assert_eq!(doc.root(), builder.root());
        assert_eq!(
            doc.root(),
            builder::Node {
                children: Cow::Borrowed(&[]),
                args: Cow::Borrowed(&[
                    builder::Arg {
                        name: Some("A".into()),
                        value: builder::Value::Int(1),
                    },
                    builder::Arg {
                        name: Some("B".into()),
                        value: builder::Value::Int(2),
                    },
                    builder::Arg {
                        name: Some("C".into()),
                        value: builder::Value::Int(3),
                    }
                ]),
                name: "".into(),
                ty: "".into()
            }
        );
        let map1 = BTreeMap::deserialize(doc.root().into_deserializer()).unwrap();
        let map2 = BTreeMap::deserialize(builder.root().into_deserializer()).unwrap();
        assert_eq!(map1, map);
        assert_eq!(map2, map);
    }

    #[test]
    fn tuple_1_arg() {
        let foo = &(1i32,);
        let builder = to_builder(&foo).unwrap();
        let doc = builder.build();
        assert_eq!(doc.root(), builder.root());
        assert_eq!(
            doc.root(),
            builder::Node::from_values([builder::Value::Int(1)])
        );
        let bar: (i32,) = Deserialize::deserialize(doc.root().into_deserializer()).unwrap();
        assert_eq!(*foo, bar);
    }

    #[test]
    fn tuple_1_child() {
        let foo = &(Struct {
            string: String::from("hello"),
            int: 123,
            enum_: Enum::UnitVariant,
            vec: vec![1, 2],
        },);
        let builder = to_builder(&foo).unwrap();
        let doc = builder.build();
        assert_eq!(doc.root(), builder.root());
        assert_eq!(
            doc.root(),
            builder::Node::from_entries([builder::Node::from_entries([
                ("string", builder::Value::String("hello".into())),
                ("int", builder::Value::Int(123)),
                ("enum_", builder::Value::String("UnitVariant".into())),
            ])
            .with_entry(("vec", vec![1i32, 2]))])
        );
        let bar: (Struct,) = Deserialize::deserialize(doc.root().into_deserializer()).unwrap();
        assert_eq!(*foo, bar);
    }
}
