use alloc::borrow::{Cow, ToOwned as _};
use facet_core::{Opaque, StructDef, StructKind};
use facet_reflect::{
    PokeEnumNoVariant, PokeListUninit, PokeMapUninit, PokeOptionUninit, PokeStruct, PokeUninit,
    PokeValueUninit,
};

use super::Error;
use crate::{
    ValueRef,
    access::{self, ArgRef as _},
};

fn deserialize_entry<'mem, 'a, Arg: access::ArgRef<'a>, Child: access::NodeRef<'a>>(
    poke: PokeUninit<'mem>,
    entry: &access::EntryRef<Arg, Child>,
) -> Result<Opaque<'mem>, Error> {
    match entry {
        access::EntryRef::Arg(arg) => deserialize_value(poke, arg.value()),
        access::EntryRef::Child(child) => deserialize_node(poke, child),
    }
}

// The reference implementation in `facet-json-read` uses a manual stack to
// avoid recursion, but let's not care about that for now.
pub fn deserialize_node<'mem, 'a, N: access::NodeRef<'a>>(
    poke: PokeUninit<'mem>,
    node: &N,
) -> Result<Opaque<'mem>, Error> {
    match poke {
        PokeUninit::Scalar(scalar) => {
            let first_arg = node.args().next().ok_or(Error::ExpectedValue)?;
            deserialize_value_as_scalar(scalar, first_arg.value())
        }
        PokeUninit::List(list) => deserialize_node_as_list(list, node),
        PokeUninit::Map(map) => deserialize_node_as_map(map, node),
        PokeUninit::Struct(struct_) => deserialize_node_as_struct(struct_, node),
        PokeUninit::Enum(enum_) => deserialize_node_as_enum(enum_, node),
        PokeUninit::Option(option) => deserialize_node_as_option(option, node),
        _ => Err(Error::UnsupportedPoke(poke.shape())),
    }
}

fn deserialize_node_as_list<'mem, 'a, N: access::NodeRef<'a>>(
    list: PokeListUninit<'mem>,
    node: &N,
) -> Result<Opaque<'mem>, Error> {
    let shape = list.shape();
    let len = node.entries().len();
    let mut list = list
        .init(Some(len))
        .map_err(|_| Error::UnsupportedPoke(shape))?;
    let item_shape = list.def().t;
    for entry in node.entries() {
        let (item_poke, _item_guard) = PokeUninit::alloc_shape(item_shape);
        let item = deserialize_entry(item_poke, &entry)?;
        unsafe {
            list.push(item);
        }
    }
    Ok(list.build_in_place())
}

fn deserialize_node_as_map<'mem, 'a, N: access::NodeRef<'a>>(
    map: PokeMapUninit<'mem>,
    node: &N,
) -> Result<Opaque<'mem>, Error> {
    let shape = map.shape();
    let len = node.entries().len();
    let mut map = map
        .init(Some(len))
        .map_err(|_| Error::UnsupportedPoke(shape))?;
    let key_shape = map.def().k;
    let value_shape = map.def().v;

    for entry in node.entries() {
        let (value_poke, _value_guard) = PokeUninit::alloc_shape(value_shape);
        let (key_poke, _key_guard) = PokeUninit::alloc_shape(key_shape);
        let key = deserialize_value(key_poke, ValueRef::String(entry.name()))?;
        let value = deserialize_entry(value_poke, &entry)?;
        unsafe {
            // Transfers ownership.
            map.insert(key, value);
        }
    }

    Ok(map.build_in_place())
}

fn deserialize_node_as_struct<'mem, 'a, N: access::NodeRef<'a>>(
    mut poke: PokeStruct<'mem>,
    node: &N,
) -> Result<Opaque<'mem>, Error> {
    let shape = poke.shape();
    match poke.def().kind {
        StructKind::Struct => {
            for entry in node.entries() {
                let (field_index, field) = poke
                    .field_by_name(entry.name())
                    .map_err(|err| Error::Field(err, shape))?;
                deserialize_entry(field, &entry)?;
                unsafe {
                    poke.mark_initialized(field_index);
                }
            }

            // Note: This panics if not all fields have been populated.
            Ok(poke.build_in_place())
        }
        StructKind::TupleStruct | StructKind::Tuple => {
            for (field_index, entry) in node.entries().enumerate() {
                let field = poke
                    .field(field_index)
                    .map_err(|err| Error::Field(err, shape))?;
                deserialize_entry(field, &entry)?;
                unsafe {
                    poke.mark_initialized(field_index);
                }
            }
            // Note: This panics if not all fields have been populated.
            Ok(poke.build_in_place())
        }
        StructKind::Unit => {
            if !node.is_empty() {
                return Err(Error::UnexpectedShape(shape));
            }
            Ok(poke.build_in_place())
        }
        _ => Err(Error::UnsupportedPoke(poke.shape())),
    }
}

fn deserialize_node_as_enum<'mem, 'a, N: access::NodeRef<'a>>(
    poke: PokeEnumNoVariant<'mem>,
    node: &N,
) -> Result<Opaque<'mem>, Error> {
    let shape = poke.shape();
    let mut poke = poke
        .set_variant_by_name(node.ty())
        .map_err(|err| Error::Field(err, shape))?;
    for (index, entry) in node.entries().enumerate() {
        let name = entry.name();
        let value = if name.is_empty() {
            poke.tuple_field(index)
        } else {
            poke.field_by_name(name).map(|(_, field)| field)
        }
        .map_err(|err| Error::Field(err, shape))?;
        deserialize_entry(value, &entry)?;
        unsafe { poke.mark_initialized(index) };
    }

    Ok(poke.build_in_place())
}

fn deserialize_node_as_option<'mem, 'a, N: access::NodeRef<'a>>(
    poke: PokeOptionUninit<'mem>,
    node: &N,
) -> Result<Opaque<'mem>, Error> {
    let po = unsafe { poke.init_none() };
    if node.is_empty() {
        Ok(po.build_in_place())
    } else {
        let some_shape = po.def().t;
        let (some_poke, _some_guard) = PokeUninit::alloc_shape(some_shape);
        let opaque = deserialize_node(some_poke, node)?;
        Ok(po
            .replace_with_some_opaque(opaque.as_const())
            .build_in_place())
    }
}

fn deserialize_value<'mem>(
    poke: PokeUninit<'mem>,
    value: ValueRef<'_>,
) -> Result<Opaque<'mem>, Error> {
    match poke {
        PokeUninit::Scalar(scalar) => deserialize_value_as_scalar(scalar, value),
        PokeUninit::List(list) => deserialize_value_as_list(list, value),
        PokeUninit::Map(map) => Err(Error::UnexpectedShape(map.shape())),
        PokeUninit::Struct(struct_) => deserialize_value_as_struct(struct_, value),
        PokeUninit::Enum(enum_) => deserialize_value_as_enum(enum_, value),
        PokeUninit::Option(option_) => deserialize_value_as_option(option_, value),
        _ => Err(Error::UnsupportedPoke(poke.shape())),
    }
}

/// Deserialize a value into a list with a single item.
fn deserialize_value_as_list<'mem>(
    poke: PokeListUninit<'mem>,
    value: ValueRef<'_>,
) -> Result<Opaque<'mem>, Error> {
    let shape = poke.shape();

    let mut list = poke
        .init(Some(1))
        .map_err(|_| Error::UnsupportedPoke(shape))?;

    let (item_poke, _item_guard) = PokeUninit::alloc_shape(shape);
    let item = deserialize_value(item_poke, value)?;
    unsafe {
        list.push(item);
    }

    Ok(list.build_in_place())
}

/// Deserialize a value into a newtype struct, or a single-element tuple, or the
/// unit type or a unit struct if the value is null.
fn deserialize_value_as_struct<'mem>(
    poke: PokeStruct<'mem>,
    value: ValueRef<'_>,
) -> Result<Opaque<'mem>, Error> {
    match poke.def().kind {
        // Only structs with unnamed fields are supported.
        StructKind::TupleStruct | StructKind::Unit | StructKind::Tuple => (),
        _ => return Err(Error::UnexpectedShape(poke.shape())),
    }

    if poke.def().fields.is_empty() {
        if let ValueRef::Null = value {
            return Ok(poke.build_in_place());
        }
        return Err(Error::ExpectedValue);
    }

    if poke.def().fields.len() != 1 {
        return Err(Error::UnexpectedShape(poke.shape()));
    }

    let field = poke
        .field(0)
        .map_err(|err| Error::Field(err, poke.shape()))?;
    deserialize_value(field, value)?;
    Ok(poke.build_in_place())
}

/// Deserialize a value into an enum variant. This expects the value to be a
/// string (the enum variant), and the enum variant must be a unit variant.
fn deserialize_value_as_enum<'mem>(
    poke: PokeEnumNoVariant<'mem>,
    value: ValueRef<'_>,
) -> Result<Opaque<'mem>, Error> {
    let ValueRef::String(variant) = value else {
        return Err(Error::ExpectedString(poke.shape()));
    };
    let shape = poke.shape();
    let variant = poke
        .set_variant_by_name(variant)
        .map_err(|err| Error::Field(err, shape))?;
    match variant.shape().def {
        facet_core::Def::Struct(StructDef {
            kind: StructKind::Unit,
            ..
        }) => Ok(variant.build_in_place()),
        _ => Err(Error::UnexpectedShape(variant.shape())),
    }
}

fn deserialize_value_as_option<'mem>(
    poke: PokeOptionUninit<'mem>,
    value: ValueRef<'_>,
) -> Result<Opaque<'mem>, Error> {
    let po = unsafe { poke.init_none() };
    if let ValueRef::Null = value {
        Ok(po.build_in_place())
    } else {
        let some_shape = po.def().t;
        let (some_poke, _some_guard) = PokeUninit::alloc_shape(some_shape);
        let opaque = deserialize_value(some_poke, value)?;
        Ok(po
            .replace_with_some_opaque(opaque.as_const())
            .build_in_place())
    }
}

#[expect(clippy::too_many_lines)]
fn deserialize_value_as_scalar<'mem, 'a>(
    poke: PokeValueUninit<'mem>,
    value: ValueRef<'a>,
) -> Result<Opaque<'mem>, Error> {
    let shape = poke.shape();
    match value {
        ValueRef::Null => Err(Error::ExpectedValue),
        ValueRef::Bool(value) => {
            if shape.is_type::<bool>() {
                Ok(poke.put(value))
            } else {
                Err(Error::ExpectedScalar("bool", shape))
            }
        }
        ValueRef::Int(value) => {
            if shape.is_type::<i8>() {
                return Ok(poke.put::<i8>(value.try_into().map_err(|_| Error::IntOverflow)?));
            }
            if shape.is_type::<i16>() {
                return Ok(poke.put::<i16>(value.try_into().map_err(|_| Error::IntOverflow)?));
            }
            if shape.is_type::<i32>() {
                return Ok(poke.put::<i32>(value.try_into().map_err(|_| Error::IntOverflow)?));
            }
            if shape.is_type::<i64>() {
                return Ok(poke.put(value));
            }
            if shape.is_type::<i128>() {
                return Ok(poke.put(value as i128));
            }
            if value >= 0 {
                if shape.is_type::<u8>() {
                    return Ok(poke.put::<u8>(value.try_into().map_err(|_| Error::IntOverflow)?));
                }
                if shape.is_type::<u16>() {
                    return Ok(poke.put::<u16>(value.try_into().map_err(|_| Error::IntOverflow)?));
                }
                if shape.is_type::<u32>() {
                    return Ok(poke.put::<u32>(value.try_into().map_err(|_| Error::IntOverflow)?));
                }
                if shape.is_type::<u64>() {
                    return Ok(poke.put(value));
                }
                #[allow(clippy::cast_sign_loss)] // false positive: checked sign already
                if shape.is_type::<u128>() {
                    return Ok(poke.put(value as u128));
                }
            }
            Err(Error::ExpectedScalar("signed int", shape))
        }
        ValueRef::Uint(value) => {
            if shape.is_type::<u8>() {
                return Ok(poke.put::<u8>(value.try_into().map_err(|_| Error::IntOverflow)?));
            }
            if shape.is_type::<u16>() {
                return Ok(poke.put::<u16>(value.try_into().map_err(|_| Error::IntOverflow)?));
            }
            if shape.is_type::<u32>() {
                return Ok(poke.put::<u32>(value.try_into().map_err(|_| Error::IntOverflow)?));
            }
            if shape.is_type::<u64>() {
                return Ok(poke.put(value));
            }
            if shape.is_type::<u128>() {
                return Ok(poke.put(value as u128));
            }
            if shape.is_type::<i8>() {
                return Ok(poke.put::<i8>(value.try_into().map_err(|_| Error::IntOverflow)?));
            }
            if shape.is_type::<i16>() {
                return Ok(poke.put::<i16>(value.try_into().map_err(|_| Error::IntOverflow)?));
            }
            if shape.is_type::<i32>() {
                return Ok(poke.put::<i32>(value.try_into().map_err(|_| Error::IntOverflow)?));
            }
            if shape.is_type::<i64>() {
                return Ok(poke.put(value));
            }
            if shape.is_type::<i128>() {
                return Ok(poke.put(value as i128));
            }
            Err(Error::ExpectedScalar("unsigned int", shape))
        }
        ValueRef::Float(value) => {
            if shape.is_type::<f32>() {
                return Ok(poke.put::<f32>(value as _));
            }
            if shape.is_type::<f64>() {
                return Ok(poke.put(value));
            }
            Err(Error::ExpectedScalar("float", shape))
        }
        ValueRef::String(value) => {
            if shape.is_type::<alloc::string::String>() {
                return Ok(poke.put(value.to_owned()));
            }
            if shape.is_type::<Cow<'a, str>>() {
                return Ok(poke.put(Cow::Borrowed(value)));
            }
            Err(Error::ExpectedScalar("string", shape))
        }
        ValueRef::Binary(value) => {
            if shape.is_type::<alloc::vec::Vec<u8>>() {
                return Ok(poke.put(value.to_owned()));
            }
            Err(Error::ExpectedScalar("binary", shape))
        }
    }
}
