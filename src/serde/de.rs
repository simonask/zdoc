use core::iter;

use serde::{
    Deserializer as _,
    de::{Unexpected, value::BorrowedStrDeserializer},
    forward_to_deserialize_any,
};

use crate::{
    ClassifyNode, ValueRef,
    access::{self, ArgRef as _},
};

use super::Error;

pub struct DeNode<N>(pub N);

impl<'de, N: access::NodeRef<'de>> serde::de::IntoDeserializer<'de, Error> for DeNode<N> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

impl<'de, N: access::NodeRef<'de>> serde::Deserializer<'de> for DeNode<N> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self.0.classify() {
            ClassifyNode::Struct | ClassifyNode::Mixed => visitor.visit_map(MapAccess::new(self.0)),
            ClassifyNode::Seq => visitor.visit_seq(SeqAccess::new(self.0)),
            ClassifyNode::Value => {
                let Some(arg) = self.0.args().next() else {
                    unreachable!()
                };
                arg.value().deserialize_any(visitor)
            }
            ClassifyNode::Unit => visitor.visit_unit(),
            ClassifyNode::StructVariant
            | ClassifyNode::SeqVariant
            | ClassifyNode::ValueVariant
            | ClassifyNode::UnitVariant
            | ClassifyNode::MixedVariant => visitor.visit_enum(EnumAccess { node: self.0 }),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf
        unit identifier ignored_any
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if self.0.is_empty() {
            visitor.visit_none()
        } else if let Some(ValueRef::Null) = self.0.args().next().map(|arg| arg.value()) {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(SeqAccess::new(self.0))
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(SeqAccess::new(self.0))
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(SeqAccess::new(self.0))
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_map(MapAccess::new(self.0))
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_map(MapAccess::new(self.0))
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_enum(EnumAccess { node: self.0 })
    }
}

struct MapAccess<Args: Iterator, Children: Iterator> {
    entries: access::EntryRefIter<Args, Children>,
    current: Option<<access::EntryRefIter<Args, Children> as Iterator>::Item>,
}

impl<'de, Args: Iterator, Children: Iterator> MapAccess<Args, Children> {
    #[inline]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new<N: access::NodeRef<'de, ArgsIter<'de> = Args, ChildrenIter<'de> = Children>>(
        node: N,
    ) -> Self {
        Self {
            entries: node.entries(),
            current: None,
        }
    }
}

impl<'de, Args, Children> serde::de::MapAccess<'de> for MapAccess<Args, Children>
where
    Args: Iterator<Item: access::ArgRef<'de>>,
    Children: Iterator<Item: access::NodeRef<'de>>,
{
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        loop {
            if let Some(next) = self.entries.next() {
                let name = next.name();
                if !name.is_empty() {
                    self.current = Some(next);
                    return seed.deserialize(MapKeyDeserializer(name)).map(Some);
                }
            } else {
                return Ok(None);
            }
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let Some(entry) = self.current.take() else {
            panic!("unbalanced map access; call next_key() first")
        };
        match entry {
            access::EntryRef::Arg(arg) => seed.deserialize(arg.value()),
            access::EntryRef::Child(node) => seed.deserialize(DeNode(node)),
        }
    }
}

struct MapKeyDeserializer<'de>(&'de str);
impl<'de> serde::de::Deserializer<'de> for MapKeyDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.0)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf map struct unit unit_struct identifier ignored_any
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if self.0.is_empty() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(ValueSeq(Some(ValueRef::String(self.0))))
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(ValueSeq(Some(ValueRef::String(self.0))))
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(ValueSeq(Some(ValueRef::String(self.0))))
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_enum(self)
    }
}

struct SeqAccess<Args, Children> {
    entries: access::EntryRefIter<Args, Children>,
}

impl<'de, Args, Children> SeqAccess<Args, Children> {
    #[inline]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new<N: access::NodeRef<'de, ArgsIter<'de> = Args, ChildrenIter<'de> = Children>>(
        node: N,
    ) -> Self {
        Self {
            entries: node.entries(),
        }
    }
}

impl<'de, Args, Children> serde::de::SeqAccess<'de> for SeqAccess<Args, Children>
where
    Args: Iterator<Item: access::ArgRef<'de>>,
    Children: Iterator<Item: access::NodeRef<'de>>,
{
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        if let Some(entry) = self.entries.next() {
            match entry {
                access::EntryRef::Arg(arg) => seed.deserialize(arg.value()).map(Some),
                access::EntryRef::Child(node) => seed.deserialize(DeNode(node)).map(Some),
            }
        } else {
            Ok(None)
        }
    }
}

struct EnumAccess<N> {
    node: N,
}

impl<'de, N: access::NodeRef<'de>> serde::de::EnumAccess<'de> for EnumAccess<N> {
    type Error = Error;
    type Variant = VariantAccess<N>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let ty = self.node.ty();
        if ty.is_empty() {
            if let Some(first_arg) = self.node.args().next() {
                if let ValueRef::String(variant) = first_arg.value() {
                    let variant = seed.deserialize(BorrowedStrDeserializer::new(variant))?;
                    return Ok((variant, VariantAccess::Unit));
                }
            }
        }

        let variant = seed.deserialize(BorrowedStrDeserializer::new(ty))?;
        Ok((variant, VariantAccess::TypedNode(self.node)))
    }
}

enum VariantAccess<N> {
    Unit,
    TypedNode(N),
}

impl<'de, N: access::NodeRef<'de>> serde::de::VariantAccess<'de> for VariantAccess<N> {
    type Error = Error;

    #[inline]
    fn unit_variant(self) -> Result<(), Self::Error> {
        match self {
            VariantAccess::Unit => Ok(()),
            VariantAccess::TypedNode(node) => {
                if !node.is_empty() {
                    // We're returning an error - might as well perform an
                    // "expensive" calculation to provide the most accurate
                    // error information.
                    let classify = node.classify();
                    return Err(serde::de::Error::invalid_type(
                        match classify {
                            ClassifyNode::Struct
                            | ClassifyNode::StructVariant
                            | ClassifyNode::Mixed
                            | ClassifyNode::MixedVariant => Unexpected::StructVariant,
                            ClassifyNode::Seq | ClassifyNode::SeqVariant => {
                                if node.entries().len() == 1 {
                                    Unexpected::NewtypeVariant
                                } else {
                                    Unexpected::TupleVariant
                                }
                            }
                            ClassifyNode::Value | ClassifyNode::ValueVariant => {
                                Unexpected::NewtypeVariant
                            }
                            ClassifyNode::Unit | ClassifyNode::UnitVariant => unreachable!(),
                        },
                        &"unit variant",
                    ));
                }

                Ok(())
            }
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        let Self::TypedNode(node) = self else {
            return Err(serde::de::Error::invalid_type(
                Unexpected::UnitVariant,
                &"newtype variant",
            ));
        };
        let mut entries = node.entries();
        if let Some(first) = entries.next() {
            if entries.next().is_some() {
                return Err(serde::de::Error::invalid_length(node.entries().len(), &"1"));
            }
            match first {
                access::EntryRef::Arg(arg) => seed.deserialize(arg.value()),
                access::EntryRef::Child(node) => seed.deserialize(DeNode(node)),
            }
        } else {
            Err(serde::de::Error::invalid_type(
                Unexpected::UnitVariant,
                &"newtype variant",
            ))
        }
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let Self::TypedNode(node) = self else {
            // Deserialize the empty sequence.
            return serde::de::value::SeqDeserializer::new(iter::empty::<DeNode<N>>())
                .deserialize_seq(visitor);
        };

        DeNode(node).deserialize_seq(visitor)
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let Self::TypedNode(node) = self else {
            // Deserialize the empty map.
            return serde::de::value::MapDeserializer::new(iter::empty::<(&str, DeNode<N>)>())
                .deserialize_map(visitor);
        };
        DeNode(node).deserialize_map(visitor)
    }
}

impl<'de> serde::Deserializer<'de> for ValueRef<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self {
            ValueRef::Null => visitor.visit_none(),
            ValueRef::Bool(value) => visitor.visit_bool(value),
            ValueRef::Int(value) => visitor.visit_i64(value),
            ValueRef::Uint(value) => visitor.visit_u64(value),
            ValueRef::Float(value) => visitor.visit_f64(value),
            ValueRef::String(value) => visitor.visit_borrowed_str(value),
            ValueRef::Binary(value) => visitor.visit_borrowed_bytes(value),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf
        map struct identifier ignored_any
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let ValueRef::Null = self {
            visitor.visit_unit()
        } else {
            self.deserialize_any(visitor)
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let ValueRef::Null = self {
            visitor.visit_unit()
        } else {
            self.deserialize_any(visitor)
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if let ValueRef::Null = self {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(ValueSeq(Some(self)))
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(ValueSeq(Some(self)))
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(ValueSeq(Some(self)))
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_enum(self)
    }
}

struct ValueSeq<'a>(Option<ValueRef<'a>>);

impl<'a> serde::de::SeqAccess<'a> for ValueSeq<'a> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'a>,
    {
        let Some(value) = self.0.take() else {
            return Ok(None);
        };
        seed.deserialize(value).map(Some)
    }
}

mod internal {
    use serde::de::Unexpected;

    pub struct UnitOnly;

    impl<'a> serde::de::VariantAccess<'a> for UnitOnly {
        type Error = super::Error;

        fn unit_variant(self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value, Self::Error>
        where
            T: serde::de::DeserializeSeed<'a>,
        {
            Err(serde::de::Error::invalid_type(
                Unexpected::UnitVariant,
                &"newtype variant",
            ))
        }

        fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
        where
            V: serde::de::Visitor<'a>,
        {
            Err(serde::de::Error::invalid_type(
                Unexpected::UnitVariant,
                &"tuple variant",
            ))
        }

        fn struct_variant<V>(
            self,
            _fields: &'static [&'static str],
            _visitor: V,
        ) -> Result<V::Value, Self::Error>
        where
            V: serde::de::Visitor<'a>,
        {
            Err(serde::de::Error::invalid_type(
                Unexpected::UnitVariant,
                &"struct variant",
            ))
        }
    }
}

impl<'a> serde::de::EnumAccess<'a> for ValueRef<'a> {
    type Error = Error;
    type Variant = internal::UnitOnly;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: serde::de::DeserializeSeed<'a>,
    {
        let variant = seed.deserialize(self)?;
        Ok((variant, internal::UnitOnly))
    }
}

impl<'a> serde::de::EnumAccess<'a> for MapKeyDeserializer<'a> {
    type Error = Error;
    type Variant = internal::UnitOnly;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: serde::de::DeserializeSeed<'a>,
    {
        let variant = seed.deserialize(self)?;
        Ok((variant, internal::UnitOnly))
    }
}
