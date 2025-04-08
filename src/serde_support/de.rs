use serde::{Deserializer as _, de::value::BorrowedStrDeserializer, forward_to_deserialize_any};

use crate::{ClassifyNode, EntriesIter, Entry, Node, ValueRef};

use super::Error;

impl<'de> serde::Deserializer<'de> for Node<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self.classify() {
            ClassifyNode::Struct | ClassifyNode::Mixed => visitor.visit_map(MapAccess::new(self)),
            ClassifyNode::Seq => visitor.visit_seq(SeqAccess::new(self)),
            ClassifyNode::Value => {
                let Some(arg) = self.args().get(0) else {
                    unreachable!()
                };
                arg.value.deserialize_any(visitor)
            }
            ClassifyNode::Unit => visitor.visit_unit(),
            ClassifyNode::StructVariant
            | ClassifyNode::SeqVariant
            | ClassifyNode::ValueVariant
            | ClassifyNode::UnitVariant
            | ClassifyNode::MixedVariant => visitor.visit_enum(EnumAccess { node: self }),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option newtype_struct
        unit unit_struct identifier ignored_any
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(SeqAccess::new(self))
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(SeqAccess::new(self))
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
        visitor.visit_seq(SeqAccess::new(self))
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_map(MapAccess::new(self))
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
        visitor.visit_map(MapAccess::new(self))
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
        visitor.visit_enum(EnumAccess { node: self })
    }
}

struct MapAccess<'de> {
    entries: EntriesIter<'de>,
    current: Option<Entry<'de>>,
}

impl<'de> MapAccess<'de> {
    #[inline]
    pub fn new(node: Node<'de>) -> Self {
        Self {
            entries: node.entries().into_iter(),
            current: None,
        }
    }
}

impl<'de> serde::de::MapAccess<'de> for MapAccess<'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        loop {
            if let Some(next) = self.entries.next() {
                if let Some(name) = next.name() {
                    self.current = Some(next);
                    return seed
                        .deserialize(BorrowedStrDeserializer::new(name))
                        .map(Some);
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
            Entry::Arg(arg) => seed.deserialize(arg.value),
            Entry::Child(node) => seed.deserialize(node),
        }
    }
}

struct SeqAccess<'de> {
    entries: EntriesIter<'de>,
}

impl<'de> SeqAccess<'de> {
    #[inline]
    pub fn new(node: Node<'de>) -> Self {
        Self {
            entries: node.entries().into_iter(),
        }
    }
}

impl<'de> serde::de::SeqAccess<'de> for SeqAccess<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        if let Some(entry) = self.entries.next() {
            match entry {
                Entry::Arg(arg) => seed.deserialize(arg.value).map(Some),
                Entry::Child(node) => seed.deserialize(node).map(Some),
            }
        } else {
            Ok(None)
        }
    }
}

struct EnumAccess<'de> {
    node: Node<'de>,
}

impl<'de> serde::de::EnumAccess<'de> for EnumAccess<'de> {
    type Error = Error;
    type Variant = VariantAccess<'de>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let ty = self.node.ty().unwrap_or("");
        let variant = seed.deserialize(BorrowedStrDeserializer::new(ty))?;
        Ok((variant, VariantAccess { node: self.node }))
    }
}

struct VariantAccess<'de> {
    node: Node<'de>,
}

impl<'de> serde::de::VariantAccess<'de> for VariantAccess<'de> {
    type Error = Error;

    #[inline]
    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        if let Some(first) = self.node.entries().into_iter().next() {
            match first {
                Entry::Arg(arg) => seed.deserialize(arg.value),
                Entry::Child(node) => seed.deserialize(node),
            }
        } else {
            Err(serde::de::Error::custom(
                "variant has no arguments or children",
            ))
        }
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.node.deserialize_seq(visitor)
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.node.deserialize_map(visitor)
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

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any
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
