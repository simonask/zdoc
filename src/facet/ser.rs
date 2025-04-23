use alloc::borrow::ToOwned as _;
use facet_core::{Def, Facet, Field, StructKind};
use facet_reflect::{Peek, PeekList, PeekMap, PeekOption};

use crate::{
    ValueRef,
    builder::{Arg, Entry, Node, Value},
};

use super::Error;

/// Serialize anything into a node. This is used when the result should always
/// be a node, such as when serializing the root.
pub fn serialize_as_node(peek: Peek) -> Result<Node, Error> {
    match peek.shape().def {
        Def::Scalar(_) => serialize_value_as_value(peek).map(|value| Node::from_args([value])),
        Def::Struct(_) => serialize_struct_as_node(peek),
        Def::Enum(_) => serialize_enum_as_node(peek),
        Def::Map(_) => serialize_map_as_node(peek.into_map()?),
        Def::List(_) | Def::Array(_) | Def::Slice(_) => serialize_list_as_node(peek.into_list()?),
        Def::Option(_) => serialize_option_as_node(peek.into_option()?),
        _ => Err(Error::UnexpectedShape(peek.shape())),
    }
}

/// Serialize anything as either a child node or an argument.
pub fn serialize_as_entry(peek: Peek) -> Result<Entry, Error> {
    match peek.shape().def {
        Def::Scalar(_) => serialize_value_as_entry(peek),
        Def::Struct(_) => serialize_struct_as_entry(peek),
        Def::Enum(_) => serialize_enum_as_entry(peek),
        Def::Map(_) => serialize_map_as_node(peek.into_map()?).map(Entry::Child),
        Def::List(_) => serialize_list_as_node(peek.into_list()?).map(Entry::Child),
        Def::Array(_) | Def::Slice(_) => {
            serialize_list_as_node(peek.into_list()?).map(Entry::Child)
        }
        Def::Option(_) => serialize_option_as_entry(peek.into_option()?),
        _ => Err(Error::UnexpectedShape(peek.shape())),
    }
}

fn serialize_value_as_entry(peek: Peek<'_>) -> Result<Entry<'_>, Error> {
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

fn serialize_struct_as_entry(peek: Peek<'_>) -> Result<Entry<'_>, Error> {
    let shape = peek.shape();
    let peek = peek.into_struct()?;
    match peek.def().kind {
        StructKind::Unit => Ok(Entry::null()),
        StructKind::TupleStruct | StructKind::Tuple => {
            // Handle newtype structs as "transparent".
            if peek.def().fields.len() == 1 {
                return serialize_as_entry(peek.fields().next().unwrap().1);
            }
            serialize_tuple_struct_fields_as_node(peek.fields()).map(Entry::Child)
        }
        StructKind::Struct => {
            serialize_named_struct_fields_as_node(peek.fields_for_serialize()).map(Entry::Child)
        }
        _ => Err(Error::UnexpectedShape(shape)),
    }
}

fn serialize_struct_as_node(peek: Peek<'_>) -> Result<Node<'_>, Error> {
    let shape = peek.shape();
    let peek = peek.into_struct()?;
    match peek.def().kind {
        StructKind::Unit => Ok(Node::empty()),
        StructKind::TupleStruct | StructKind::Tuple => {
            // Handle newtype structs as "transparent".
            if peek.def().fields.len() == 1 {
                return serialize_as_node(peek.fields().next().unwrap().1);
            }
            serialize_tuple_struct_fields_as_node(peek.fields())
        }
        StructKind::Struct => serialize_named_struct_fields_as_node(peek.fields_for_serialize()),
        _ => Err(Error::UnexpectedShape(shape)),
    }
}

fn serialize_named_struct_fields_as_node<'mem>(
    fields: impl Iterator<Item = (&'static Field, Peek<'mem>)>,
) -> Result<Node<'mem>, Error> {
    let mut node = Node::empty();
    for (field, value) in fields {
        let mut serialized_value = serialize_as_entry(value)?;
        serialized_value.set_name(field.name);
        node.push(serialized_value);
    }
    Ok(node)
}

fn serialize_tuple_struct_fields_as_node<'mem>(
    fields: impl Iterator<Item = (&'static Field, Peek<'mem>)>,
) -> Result<Node<'mem>, Error> {
    let mut node = Node::empty();
    for (_field, value) in fields {
        let serialized_field = serialize_as_entry(value)?;
        node.push_ordered(serialized_field);
    }
    Ok(node)
}

fn serialize_enum_as_entry(peek: Peek<'_>) -> Result<Entry<'_>, Error> {
    let peek_enum = peek.into_enum()?;
    let variant = peek_enum.active_variant();
    match variant.data.kind {
        // Unit variants are serialized as a string.
        StructKind::Unit => Ok(Entry::Arg(Arg::unnamed(ValueRef::String(variant.name)))),
        _ => serialize_enum_as_node(peek).map(Entry::Child),
    }
}

fn serialize_enum_as_node(peek: Peek<'_>) -> Result<Node<'_>, Error> {
    let shape = peek.shape();
    let peek = peek.into_enum()?;
    let variant = peek.active_variant();
    match variant.data.kind {
        // Unit variants are serialized an empty node with a type.
        StructKind::Unit => Ok(Node::empty().with_ty(variant.name)),
        StructKind::TupleStruct | StructKind::Tuple => {
            let mut node = serialize_tuple_struct_fields_as_node(peek.fields())?;
            node.set_ty(variant.name);
            Ok(node)
        }
        StructKind::Struct => {
            let mut node = serialize_named_struct_fields_as_node(peek.fields_for_serialize())?;
            node.set_ty(variant.name);
            Ok(node)
        }
        _ => Err(Error::UnexpectedShape(shape)),
    }
}

fn serialize_option_as_node(peek: PeekOption<'_>) -> Result<Node<'_>, Error> {
    if let Some(value) = peek.value() {
        serialize_as_node(value)
    } else {
        Ok(Node::empty())
    }
}

fn serialize_option_as_entry(peek: PeekOption<'_>) -> Result<Entry<'_>, Error> {
    if let Some(value) = peek.value() {
        serialize_as_entry(value)
    } else {
        Ok(Entry::Arg(Arg::unnamed(ValueRef::Null)))
    }
}

fn serialize_value_as_value(peek: Peek) -> Result<Value, Error> {
    let shape = peek.shape();

    if let Some(value) = try_get::<i8>(&peek) {
        return Ok(Value::Int(*value as _));
    }
    if let Some(value) = try_get::<u8>(&peek) {
        return Ok(Value::Uint(*value as _));
    }
    if let Some(value) = try_get::<i16>(&peek) {
        return Ok(Value::Int(*value as _));
    }
    if let Some(value) = try_get::<u16>(&peek) {
        return Ok(Value::Uint(*value as _));
    }
    if let Some(value) = try_get::<i32>(&peek) {
        return Ok(Value::Int(*value as _));
    }
    if let Some(value) = try_get::<u32>(&peek) {
        return Ok(Value::Uint(*value as _));
    }
    if let Some(value) = try_get::<i64>(&peek) {
        return Ok(Value::Int(*value as _));
    }
    if let Some(value) = try_get::<u64>(&peek) {
        return Ok(Value::Uint(*value as _));
    }
    if let Some(value) = try_get::<f32>(&peek) {
        return Ok(Value::Float(*value as _));
    }
    if let Some(value) = try_get::<f64>(&peek) {
        return Ok(Value::Float(*value as _));
    }
    if let Some(value) = try_get::<bool>(&peek) {
        return Ok(Value::Bool(*value as _));
    }

    if let Some(value) = try_get::<alloc::string::String>(&peek) {
        return Ok(Value::String(value.clone().into()));
    }
    if let Some(value) = try_get::<alloc::borrow::Cow<'_, str>>(&peek) {
        return Ok(Value::String(alloc::borrow::Cow::Owned(
            value.clone().into_owned(),
        )));
    }
    if let Some(value) = try_get::<&str>(&peek) {
        return Ok(Value::String(value.to_owned().into()));
    }

    Err(Error::UnexpectedShape(shape))
}

// TODO: Feels like this is missing as a utility method in `PeekValue`.
fn try_get<'a, T: Facet>(value: &'a Peek<'_>) -> Option<&'a T> {
    value.get().ok()
}
