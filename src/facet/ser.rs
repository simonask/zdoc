use facet_core::Facet;
use facet_reflect::{Peek, PeekEnum, PeekList, PeekMap, PeekOption, PeekStruct, PeekValue};

use crate::{
    ValueRef,
    builder::{Arg, Entry, Node, Value},
};

use super::Error;

pub fn serialize_as_node(peek: Peek) -> Result<Node, Error> {
    let entry = serialize_as_entry(peek)?;
    Ok(match entry {
        Entry::Arg(arg) => arg.into_key_value_node(),
        Entry::Child(node) => node,
    })
}

pub fn serialize_as_entry(peek: Peek) -> Result<Entry, Error> {
    match peek {
        Peek::Value(peek) => serialize_value_as_entry(peek),
        Peek::List(peek) => serialize_list_as_node(peek).map(Entry::Child),
        Peek::Map(peek) => serialize_map_as_node(peek).map(Entry::Child),
        Peek::Struct(peek) => serialize_struct_as_entry(peek),
        Peek::Enum(peek) => serialize_enum_as_entry(peek),
        Peek::Option(peek) => serialize_option_as_entry(peek),
        _ => Err(Error::UnexpectedShape(peek.shape())),
    }
}

fn serialize_value_as_entry(peek: PeekValue<'_>) -> Result<Entry<'_>, Error> {
    serialize_value_as_value(peek)
        .map(Arg::unnamed)
        .map(Entry::Arg)
}

fn serialize_list_as_node(peek: PeekList<'_>) -> Result<Node<'_>, Error> {
    let mut node = Node::empty();
    for item in peek.iter() {
        let item = serialize_as_entry(item)?;
        node.push_ordered(item);
    }
    Ok(node)
}

fn serialize_map_as_node(peek: PeekMap<'_>) -> Result<Node<'_>, Error> {
    let mut node = Node::empty();
    for (key, value) in peek.iter() {
        let serialized_key = serialize_as_entry(key)?;
        let Entry::Arg(Arg {
            name: None,
            value: Value::String(key),
        }) = serialized_key
        else {
            return Err(Error::NonStringMapKey(key.shape()));
        };
        let mut serialized_value = serialize_as_entry(value)?;
        serialized_value.set_name(key);
        node.push_ordered(serialized_value);
    }
    Ok(node)
}

fn serialize_struct_as_entry(peek: PeekStruct<'_>) -> Result<Entry<'_>, Error> {
    match peek.def().kind {
        facet_core::StructKind::Struct => {
            // Struct with named fields.
            let mut node = Node::empty();
            for (name, value) in peek.fields() {
                let mut serialized_value = serialize_as_entry(value)?;
                serialized_value.set_name(name);
                node.push(serialized_value);
            }
            Ok(Entry::Child(node))
        }
        facet_core::StructKind::Unit => Ok(Entry::Child(Node::empty())),
        facet_core::StructKind::TupleStruct | facet_core::StructKind::Tuple => {
            let mut node = Node::empty();
            for index in 0..peek.field_count() {
                let field = peek.field_value(index).expect("field out of bounds");
                let serialized_field = serialize_as_entry(field)?;
                node.push_ordered(serialized_field);
            }
            Ok(Entry::Child(node))
        }
        _ => Err(Error::UnsupportedPoke(peek.shape())),
    }
}

fn serialize_enum_as_entry(peek: PeekEnum<'_>) -> Result<Entry<'_>, Error> {
    let variant = peek.active_variant();
    match variant.kind {
        facet_core::VariantKind::Unit => {
            let mut node = Node::empty();
            node.set_ty(variant.name);
            Ok(Entry::Child(node))
        }
        facet_core::VariantKind::Tuple { fields } => {
            // TODO: We don't get direct access to `variant_data()`, so we can't
            // reuse `serialize_list_as_node` :(.
            let mut node = Node::empty();
            node.set_ty(variant.name);
            for i in 0..fields.len() {
                let field = peek.tuple_field(i).expect("tuple field out of bounds");
                let field = serialize_as_entry(field)?;
                node.push_ordered(field);
            }
            Ok(Entry::Child(node))
        }
        facet_core::VariantKind::Struct { fields } => {
            // TODO: We don't get direct access to `variant_data()`, so we can't
            // reuse `serialize_struct_as_node` :(.
            let mut node = Node::empty();
            node.set_ty(variant.name);
            for field in fields {
                let value = peek.field(field.name).expect("field not found");
                let mut serialized_field = serialize_as_entry(value)?;
                serialized_field.set_name(field.name);
                node.push(serialized_field);
            }
            Ok(Entry::Child(node))
        }
        _ => Err(Error::UnexpectedShape(peek.shape())),
    }
}

fn serialize_option_as_entry(peek: PeekOption<'_>) -> Result<Entry<'_>, Error> {
    if let Some(value) = peek.value() {
        serialize_as_entry(value)
    } else {
        Ok(Entry::Arg(Arg::unnamed(ValueRef::Null)))
    }
}

fn serialize_value_as_value(peek: PeekValue) -> Result<ValueRef, Error> {
    let shape = peek.shape();

    if let Some(value) = try_get::<i8>(peek) {
        return Ok(ValueRef::Int(*value as _));
    }
    if let Some(value) = try_get::<u8>(peek) {
        return Ok(ValueRef::Uint(*value as _));
    }
    if let Some(value) = try_get::<i16>(peek) {
        return Ok(ValueRef::Int(*value as _));
    }
    if let Some(value) = try_get::<u16>(peek) {
        return Ok(ValueRef::Uint(*value as _));
    }
    if let Some(value) = try_get::<i32>(peek) {
        return Ok(ValueRef::Int(*value as _));
    }
    if let Some(value) = try_get::<u32>(peek) {
        return Ok(ValueRef::Uint(*value as _));
    }
    if let Some(value) = try_get::<i64>(peek) {
        return Ok(ValueRef::Int(*value as _));
    }
    if let Some(value) = try_get::<u64>(peek) {
        return Ok(ValueRef::Uint(*value as _));
    }
    if let Some(value) = try_get::<f32>(peek) {
        return Ok(ValueRef::Float(*value as _));
    }
    if let Some(value) = try_get::<f64>(peek) {
        return Ok(ValueRef::Float(*value as _));
    }
    if let Some(value) = try_get::<bool>(peek) {
        return Ok(ValueRef::Bool(*value as _));
    }

    #[cfg(feature = "alloc")]
    if let Some(value) = try_get::<alloc::string::String>(peek) {
        return Ok(ValueRef::String(value));
    }
    #[cfg(feature = "alloc")]
    if let Some(value) = try_get::<alloc::borrow::Cow<'_, str>>(peek) {
        return Ok(ValueRef::String(value));
    }
    if let Some(value) = try_get::<&str>(peek) {
        return Ok(ValueRef::String(value));
    }

    Err(Error::UnexpectedShape(shape))
}

// TODO: Feels like this is missing as a utility method in `PeekValue`.
fn try_get<T: Facet>(value: PeekValue) -> Option<&T> {
    if value.shape().is_type::<T>() {
        Some(unsafe { value.data().as_ref::<T>() })
    } else {
        None
    }
}
