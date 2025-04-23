use alloc::borrow::{Cow, ToOwned as _};
use facet_core::{Def, Facet, Struct, StructKind};
use facet_reflect::{ReflectError, Wip};

use super::Error;
use crate::{
    ValueRef,
    access::{self, ArgRef as _},
};

fn deserialize_entry<'mem, 'a, Arg: access::ArgRef<'a>, Child: access::NodeRef<'a>>(
    wip: Wip<'mem>,
    entry: &access::EntryRef<Arg, Child>,
) -> Result<Wip<'mem>, Error> {
    match entry {
        access::EntryRef::Arg(arg) => deserialize_value(wip, arg.value()),
        access::EntryRef::Child(child) => deserialize_node(wip, child),
    }
}

// The reference implementation in `facet-json-read` uses a manual stack to
// avoid recursion, but let's not care about that for now.
pub fn deserialize_node<'mem, 'a, N: access::NodeRef<'a>>(
    wip: Wip<'mem>,
    node: &N,
) -> Result<Wip<'mem>, Error> {
    match wip.shape().def {
        Def::Scalar(_) => deserialize_value_as_scalar(
            wip,
            node.args().next().ok_or(Error::ExpectedValue)?.value(),
        )
        .map_err(Into::into),
        Def::Struct(s) => deserialize_node_as_struct(wip, node, s),
        Def::Enum(_) => deserialize_node_as_enum(wip, node),
        Def::Map(_) => deserialize_node_as_map(wip, node),
        Def::List(_) | Def::Array(_) | Def::Slice(_) => deserialize_node_as_list(wip, node),
        Def::Option(_) => deserialize_node_as_option(wip, node),
        // TODO: Can probably support Box.
        _ => Err(Error::UnexpectedShape(wip.shape())),
    }
}

fn deserialize_node_as_list<'mem, 'a, N: access::NodeRef<'a>>(
    list: Wip<'mem>,
    node: &N,
) -> Result<Wip<'mem>, Error> {
    let mut wip = list.put_empty_list()?;
    for entry in node.entries() {
        wip = deserialize_entry(wip.push()?, &entry)?.pop()?;
    }
    Ok(wip)
}

fn deserialize_node_as_map<'mem, 'a, N: access::NodeRef<'a>>(
    map: Wip<'mem>,
    node: &N,
) -> Result<Wip<'mem>, Error> {
    let mut wip = map.put_empty_map()?;
    for entry in node.entries() {
        wip = deserialize_value(wip.push_map_key()?, ValueRef::String(entry.name()))?;
        wip = deserialize_entry(wip.push_map_value()?, &entry)?.pop()?;
    }
    Ok(wip)
}

fn deserialize_node_as_struct<'mem, 'a, N: access::NodeRef<'a>>(
    mut wip: Wip<'mem>,
    node: &N,
    def: Struct,
) -> Result<Wip<'mem>, Error> {
    let shape = wip.shape();
    match def.kind {
        StructKind::Unit => Ok(wip),
        StructKind::TupleStruct | StructKind::Tuple => {
            // Handle newtype structs as "transparent".
            if def.fields.len() == 1 {
                return deserialize_node(wip.field(0)?, node)?
                    .pop()
                    .map_err(Into::into);
            }

            for (index, entry) in node.entries().enumerate() {
                wip = deserialize_entry(wip.field(index)?, &entry)?.pop()?;
            }
            Ok(wip)
        }
        StructKind::Struct => {
            for entry in node.entries() {
                wip = deserialize_entry(wip.field_named(entry.name())?, &entry)?.pop()?;
            }
            Ok(wip)
        }
        _ => Err(Error::UnexpectedShape(shape)),
    }
}

fn deserialize_node_as_enum<'mem, 'a, N: access::NodeRef<'a>>(
    wip: Wip<'mem>,
    node: &N,
) -> Result<Wip<'mem>, Error> {
    if node.ty().is_empty() {
        // If the node is just a single string, assume this is a unit variant.
        if let Some(first_arg) = node.args().next() {
            if let ValueRef::String(s) = first_arg.value() {
                return Ok(wip.variant_named(s)?);
            }
        }

        Err(Error::ExpectedEnum(wip.shape()))
    } else {
        let wip = wip.variant_named(node.ty())?;
        let variant = wip.selected_variant().unwrap();
        deserialize_node_as_struct(wip, node, variant.data)
    }
}

fn deserialize_node_as_option<'mem, 'a, N: access::NodeRef<'a>>(
    wip: Wip<'mem>,
    node: &N,
) -> Result<Wip<'mem>, Error> {
    if node.is_empty() {
        wip.put_default().map_err(Into::into)
    } else {
        deserialize_node(wip.push_some()?, node)?
            .pop()
            .map_err(Into::into)
    }
}

fn deserialize_value<'mem>(wip: Wip<'mem>, value: ValueRef<'_>) -> Result<Wip<'mem>, Error> {
    match wip.shape().def {
        Def::Scalar(_) => deserialize_value_as_scalar(wip, value).map_err(Into::into),
        Def::Struct(s) => deserialize_value_as_struct(wip, value, s),
        Def::Enum(_) => deserialize_value_as_enum(wip, value),
        Def::List(_) | Def::Array(_) | Def::Slice(_) => deserialize_value_as_list(wip, value),
        Def::Option(_) => deserialize_value_as_option(wip, value),
        _ => Err(Error::UnexpectedShape(wip.shape())),
    }
}

/// Deserialize a value into a list with a single item.
fn deserialize_value_as_list<'mem>(
    wip: Wip<'mem>,
    value: ValueRef<'_>,
) -> Result<Wip<'mem>, Error> {
    deserialize_value(wip.put_empty_list()?.push()?, value)?
        .pop()
        .map_err(Into::into)
}

/// Deserialize a value into a newtype struct, or a single-element tuple, or the
/// unit type or a unit struct if the value is null.
fn deserialize_value_as_struct<'mem>(
    wip: Wip<'mem>,
    value: ValueRef<'_>,
    s: Struct,
) -> Result<Wip<'mem>, Error> {
    match s.kind {
        // Only structs with unnamed fields are supported.
        StructKind::TupleStruct | StructKind::Unit | StructKind::Tuple => (),
        _ => return Err(Error::UnexpectedShape(wip.shape())),
    }

    deserialize_value(wip.field(0)?, value)?
        .pop()
        .map_err(Into::into)
}

/// Deserialize a value into an enum variant. This expects the value to be a
/// string (the enum variant), and the enum variant must be a unit variant.
fn deserialize_value_as_enum<'mem>(
    wip: Wip<'mem>,
    value: ValueRef<'_>,
) -> Result<Wip<'mem>, Error> {
    let ValueRef::String(variant) = value else {
        return Err(Error::ExpectedString(wip.shape()));
    };
    wip.variant_named(variant).map_err(Into::into)
}

fn deserialize_value_as_option<'mem>(
    wip: Wip<'mem>,
    value: ValueRef<'_>,
) -> Result<Wip<'mem>, Error> {
    if let ValueRef::Null = value {
        wip.put_default().map_err(Into::into)
    } else {
        deserialize_value(wip.push_some()?, value)?
            .pop()
            .map_err(Into::into)
    }
}

#[expect(clippy::too_many_lines)]
fn deserialize_value_as_scalar<'mem, 'a>(
    wip: Wip<'mem>,
    value: ValueRef<'a>,
) -> Result<Wip<'mem>, ReflectError> {
    fn try_put_int<T: TryFrom<i64> + Facet + 'static>(
        wip: Wip<'_>,
        value: i64,
    ) -> Result<Wip<'_>, ReflectError> {
        T::try_from(value)
            .map_err(|_| ReflectError::OperationFailed {
                shape: T::SHAPE,
                operation: "integer overflow",
            })
            .and_then(move |value| wip.put(value))
    }
    fn try_put_uint<T: TryFrom<u64> + Facet + 'static>(
        wip: Wip<'_>,
        value: u64,
    ) -> Result<Wip<'_>, ReflectError> {
        T::try_from(value)
            .map_err(|_| ReflectError::OperationFailed {
                shape: T::SHAPE,
                operation: "integer overflow",
            })
            .and_then(move |value| wip.put(value))
    }

    let shape = wip.shape();

    match value {
        ValueRef::Null => Err(ReflectError::WrongShape {
            expected: <Option<()> as Facet>::SHAPE,
            actual: shape,
        }),
        ValueRef::Bool(value) => {
            if shape.is_type::<bool>() {
                wip.put(value)
            } else {
                Err(ReflectError::WrongShape {
                    expected: <bool as Facet>::SHAPE,
                    actual: shape,
                })
            }
        }
        ValueRef::Int(value) => {
            if shape.is_type::<i8>() {
                return try_put_int::<i8>(wip, value);
            }
            if shape.is_type::<i16>() {
                return try_put_int::<i16>(wip, value);
            }
            if shape.is_type::<i32>() {
                return try_put_int::<i32>(wip, value);
            }
            if shape.is_type::<i64>() {
                return wip.put(value);
            }
            if shape.is_type::<i128>() {
                return wip.put(value as i128);
            }
            if value >= 0 {
                if shape.is_type::<u8>() {
                    return try_put_int::<u8>(wip, value);
                }
                if shape.is_type::<u16>() {
                    return try_put_int::<u16>(wip, value);
                }
                if shape.is_type::<u32>() {
                    return try_put_int::<u32>(wip, value);
                }
                if shape.is_type::<u64>() {
                    return try_put_int::<u64>(wip, value);
                }
                #[allow(clippy::cast_sign_loss)] // false positive: checked sign already
                if shape.is_type::<u128>() {
                    return try_put_int::<u128>(wip, value);
                }
            }
            Err(ReflectError::WrongShape {
                expected: <i64 as Facet>::SHAPE,
                actual: shape,
            })
        }
        ValueRef::Uint(value) => {
            if shape.is_type::<u8>() {
                return try_put_uint::<u8>(wip, value);
            }
            if shape.is_type::<u16>() {
                return try_put_uint::<u16>(wip, value);
            }
            if shape.is_type::<u32>() {
                return try_put_uint::<u32>(wip, value);
            }
            if shape.is_type::<u64>() {
                return wip.put(value);
            }
            if shape.is_type::<u128>() {
                return wip.put(value as u128);
            }
            if shape.is_type::<i8>() {
                return try_put_uint::<i8>(wip, value);
            }
            if shape.is_type::<i16>() {
                return try_put_uint::<i16>(wip, value);
            }
            if shape.is_type::<i32>() {
                return try_put_uint::<i32>(wip, value);
            }
            if shape.is_type::<i64>() {
                return try_put_uint::<i64>(wip, value);
            }
            if shape.is_type::<i128>() {
                return wip.put(value as i128);
            }
            Err(ReflectError::WrongShape {
                expected: <u64 as Facet>::SHAPE,
                actual: shape,
            })
        }
        ValueRef::Float(value) => {
            if shape.is_type::<f32>() {
                return wip.put::<f32>(value as _);
            }
            if shape.is_type::<f64>() {
                return wip.put(value);
            }
            Err(ReflectError::WrongShape {
                expected: f64::SHAPE,
                actual: shape,
            })
        }
        ValueRef::String(value) => {
            if shape.is_type::<alloc::string::String>() {
                return wip.put(value.to_owned());
            }
            if shape.is_type::<Cow<'a, str>>() {
                return wip.put(Cow::Owned(value.to_owned()));
            }
            Err(ReflectError::WrongShape {
                expected: alloc::string::String::SHAPE,
                actual: shape,
            })
        }
        ValueRef::Binary(value) => {
            if shape.is_type::<alloc::vec::Vec<u8>>() {
                return wip.put(value.to_owned());
            }
            Err(ReflectError::WrongShape {
                expected: alloc::vec::Vec::<u8>::SHAPE,
                actual: shape,
            })
        }
    }
}
