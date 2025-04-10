use alloc::{
    borrow::{Cow, ToOwned},
    string::{String, ToString as _},
};
use serde::{Serialize as _, ser::Impossible};

use crate::{
    builder::{Arg, Entry, Node, Value},
    serde::Error,
};

macro_rules! fwd_ser_entry {
    ($function:ident, $value:ty) => {
        #[inline]
        #[allow(unused_mut)]
        fn $function(mut self, v: $value) -> Result<Self::Ok, Self::Error> {
            *self = v.serialize(ValueSerializer)?.into();
            Ok(())
        }
    };
}

impl<'a, 'b> serde::Serializer for &'a mut Entry<'b> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = SeqSerializer<'a, 'b>;
    type SerializeTuple = SeqSerializer<'a, 'b>;
    type SerializeTupleStruct = SeqSerializer<'a, 'b>;
    type SerializeTupleVariant = SeqSerializer<'a, 'b>;
    type SerializeMap = MapSerializer<'a, 'b>;
    type SerializeStruct = &'a mut Node<'b>;
    type SerializeStructVariant = &'a mut Node<'b>;

    fwd_ser_entry!(serialize_bool, bool);
    fwd_ser_entry!(serialize_i8, i8);
    fwd_ser_entry!(serialize_i16, i16);
    fwd_ser_entry!(serialize_i32, i32);
    fwd_ser_entry!(serialize_i64, i64);
    fwd_ser_entry!(serialize_u8, u8);
    fwd_ser_entry!(serialize_u16, u16);
    fwd_ser_entry!(serialize_u32, u32);
    fwd_ser_entry!(serialize_u64, u64);
    fwd_ser_entry!(serialize_f32, f32);
    fwd_ser_entry!(serialize_f64, f64);
    fwd_ser_entry!(serialize_char, char);
    fwd_ser_entry!(serialize_str, &str);
    fwd_ser_entry!(serialize_bytes, &[u8]);

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        *self = Entry::Arg(Arg {
            name: None,
            value: Value::Null,
        });
        Ok(())
    }

    #[inline]
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        *self = Entry::Child(Node::empty());
        Ok(())
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        *self = ValueSerializer
            .serialize_unit_variant(name, variant_index, variant)?
            .into();
        Ok(())
    }

    #[inline]
    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.reset_as_node()
            .serialize_newtype_variant(name, variant_index, variant, value)
    }

    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.reset_as_node().serialize_seq(len)
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.reset_as_node().serialize_tuple(len)
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.reset_as_node().serialize_tuple_struct(name, len)
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.reset_as_node()
            .serialize_tuple_variant(name, variant_index, variant, len)
    }

    #[inline]
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.reset_as_node().serialize_map(len)
    }

    #[inline]
    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.reset_as_node().serialize_struct(name, len)
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.reset_as_node()
            .serialize_struct_variant(name, variant_index, variant, len)
    }
}

macro_rules! fwd_ser_node {
    ($function:ident, $value:ty) => {
        #[inline]
        #[allow(unused_mut)]
        fn $function(mut self, v: $value) -> Result<Self::Ok, Self::Error> {
            *self = Node::from_values(Some(ValueSerializer.$function(v)?));
            Ok(())
        }
    };
}

/// Serialize as a document node.
///
/// For plain values, this produces a single, unnamed node with the given value
/// as the only argument.
///
/// When serializing named types (structs and enums), this will only ever set
/// the `type` of the node - not the name.
impl<'a, 'b: 'a> serde::Serializer for &'a mut Node<'b> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = SeqSerializer<'a, 'b>;
    type SerializeTuple = SeqSerializer<'a, 'b>;
    type SerializeTupleStruct = SeqSerializer<'a, 'b>;
    type SerializeTupleVariant = SeqSerializer<'a, 'b>;
    type SerializeMap = MapSerializer<'a, 'b>;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fwd_ser_node!(serialize_bool, bool);
    fwd_ser_node!(serialize_i8, i8);
    fwd_ser_node!(serialize_i16, i16);
    fwd_ser_node!(serialize_i32, i32);
    fwd_ser_node!(serialize_i64, i64);
    fwd_ser_node!(serialize_u8, u8);
    fwd_ser_node!(serialize_u16, u16);
    fwd_ser_node!(serialize_u32, u32);
    fwd_ser_node!(serialize_u64, u64);
    fwd_ser_node!(serialize_f32, f32);
    fwd_ser_node!(serialize_f64, f64);
    fwd_ser_node!(serialize_char, char);
    fwd_ser_node!(serialize_str, &str);
    fwd_ser_node!(serialize_bytes, &[u8]);

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        *self = Node::from_values(Some(Value::Null));
        Ok(())
    }

    #[inline]
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.set_ty(variant);
        Ok(())
    }

    #[inline]
    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let mut entry = Entry::null();
        value.serialize(&mut entry)?;
        self.set_ty(variant);
        self.push(entry);
        Ok(())
    }

    #[inline]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SeqSerializer { seq: self })
    }

    #[inline]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(SeqSerializer { seq: self })
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(SeqSerializer { seq: self })
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.set_ty(variant);
        Ok(SeqSerializer { seq: self })
    }

    #[inline]
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.children_mut().reserve(len.unwrap_or(0));
        Ok(MapSerializer {
            node: self,
            pending_key: None,
        })
    }

    #[inline]
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        // Not preallocating; we don't know if the fields will be serialized as
        // arguments or children.
        Ok(self)
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        // Not preallocating; we don't know if the fields will be serialized as
        // arguments or children.
        self.set_ty(variant);
        Ok(self)
    }
}

pub struct SeqSerializer<'a, 'b> {
    seq: &'a mut Node<'b>,
}

impl serde::ser::SerializeSeq for SeqSerializer<'_, '_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let mut child = Entry::null();
        value.serialize(&mut child)?;
        self.seq.push_ordered(child);
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeTuple for SeqSerializer<'_, '_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeTupleStruct for SeqSerializer<'_, '_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeTupleVariant for SeqSerializer<'_, '_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

struct MapKey<'a>(Cow<'a, str>);

impl From<String> for MapKey<'_> {
    #[inline]
    fn from(value: String) -> Self {
        MapKey(value.into())
    }
}

impl From<&'static str> for MapKey<'_> {
    #[inline]
    fn from(value: &'static str) -> Self {
        MapKey(value.into())
    }
}

pub struct MapSerializer<'a, 'b> {
    node: &'a mut Node<'b>,
    pending_key: Option<MapKey<'b>>,
}

impl serde::ser::SerializeMap for MapSerializer<'_, '_> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        assert!(
            self.pending_key.is_none(),
            "unbalanced calls to serialize_key()/serialize_value()"
        );
        let mut pending_key = MapKey(Cow::Borrowed(""));
        key.serialize(&mut pending_key)?;
        self.pending_key = Some(pending_key);
        Ok(())
    }

    fn serialize_value<T>(&mut self, serialize_value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let mut value = Entry::null();
        serialize_value.serialize(&mut value)?;
        let key = self
            .pending_key
            .take()
            .expect("unbalanced calls to serialize_key()/serialize_value()");
        value.set_name(key.0);
        self.node.push_ordered(value);
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeStruct for &mut Node<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let mut entry = Entry::null();
        value.serialize(&mut entry)?;
        entry.set_name(key);
        self.push(entry);

        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl serde::ser::SerializeStructVariant for &mut Node<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        serde::ser::SerializeStruct::serialize_field(self, key, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

macro_rules! must_be_string_key {
    ($func:ident, $ty:ty) => {
        #[inline]
        fn $func(self, _key: $ty) -> Result<Self::Ok, Self::Error> {
            Err(Error::NonStringMapKey)
        }
    };
}

impl<'a, 'b: 'a> serde::Serializer for &'a mut MapKey<'b> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = SeqSerializer<'a, 'b>;
    type SerializeTuple = SeqSerializer<'a, 'b>;
    type SerializeTupleStruct = SeqSerializer<'a, 'b>;
    type SerializeTupleVariant = SeqSerializer<'a, 'b>;
    type SerializeMap = MapSerializer<'a, 'b>;
    type SerializeStruct = &'a mut Node<'b>;
    type SerializeStructVariant = &'a mut Node<'b>;

    must_be_string_key!(serialize_bool, bool);
    must_be_string_key!(serialize_i8, i8);
    must_be_string_key!(serialize_i16, i16);
    must_be_string_key!(serialize_i32, i32);
    must_be_string_key!(serialize_i64, i64);
    must_be_string_key!(serialize_u8, u8);
    must_be_string_key!(serialize_u16, u16);
    must_be_string_key!(serialize_u32, u32);
    must_be_string_key!(serialize_u64, u64);
    must_be_string_key!(serialize_f32, f32);
    must_be_string_key!(serialize_f64, f64);
    must_be_string_key!(serialize_bytes, &[u8]);

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        *self = MapKey(Cow::Owned(v.to_string()));
        Ok(())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        *self = MapKey(Cow::Owned(v.to_owned()));
        Ok(())
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::NonStringMapKey)
    }

    #[inline]
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::NonStringMapKey)
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_none()
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        *self = MapKey(Cow::Borrowed(variant));
        Ok(())
    }

    #[inline]
    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        Err(Error::NonStringMapKey)
    }

    #[inline]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Error::NonStringMapKey)
    }

    #[inline]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(Error::NonStringMapKey)
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Error::NonStringMapKey)
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::NonStringMapKey)
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Error::NonStringMapKey)
    }

    #[inline]
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(Error::NonStringMapKey)
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::NonStringMapKey)
    }
}

struct ValueSerializer;
impl serde::Serializer for ValueSerializer {
    type Ok = Value<'static>;
    type Error = Error;
    type SerializeSeq = Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    #[inline]
    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Bool(v))
    }

    #[inline]
    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(v.into()))
    }

    #[inline]
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(v.into()))
    }

    #[inline]
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(v.into()))
    }

    #[inline]
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(v))
    }

    #[inline]
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Uint(v.into()))
    }

    #[inline]
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Uint(v.into()))
    }

    #[inline]
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Uint(v.into()))
    }

    #[inline]
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Uint(v))
    }

    #[inline]
    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Float(v.into()))
    }

    #[inline]
    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Float(v))
    }

    #[inline]
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        Ok(Value::String(v.to_string().into()))
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::String(v.to_owned().into()))
    }

    #[inline]
    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Binary(v.to_owned().into()))
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    #[inline]
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(Value::String(Cow::Borrowed(variant)))
    }

    #[inline]
    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        Err(Error::CompoundValue)
    }

    #[inline]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Error::CompoundValue)
    }

    #[inline]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(Error::CompoundValue)
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Error::CompoundValue)
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::CompoundValue)
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Error::CompoundValue)
    }

    #[inline]
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(Error::CompoundValue)
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::CompoundValue)
    }
}
