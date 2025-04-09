//! Raw definitions of the document format.
//!
//! All structs are `#[repr(C)]`, and compile-time checks exist to verify that
//! the C representation of each struct is portable (i.e., the current
//! platform's C ABI struct representation corresponds to the expectations of
//! the binary format).
//!
//! # Endianness
//!
//! Only little-endian architectures are supported at this time.

use core::mem::offset_of;

use crate::ValidationErrorKind;

pub const MAGIC: [u8; 8] = *b"zdoc\0\0\0\0";
pub const VERSION: u32 = 1;

#[cfg(not(target_endian = "little"))]
compile_error!("Unsupported target endian");

/// Header of a zdoc document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C, align(4))]
pub struct Header {
    /// Magic bytes. Must be "zdoc\0\0\0\0" (8 bytes).
    pub magic: [u8; 8],
    /// Document format version, must be 1.
    pub version: u32,
    /// Index of the root node. Must be zero or less than `nodes_len`.
    pub root_node_index: u32,
    /// Size of the document in bytes, including the header.
    pub size: u32,
    /// Start of nodes (byte offset from beginning of file/buffer). This is
    /// usually the same as `size_of::<Header>()`, which is 64. Must be 4-byte
    /// aligned.
    pub nodes_offset: u32,
    /// Number nodes.
    pub nodes_len: u32,
    /// Start of arguments (byte offset from the beginning of file/buffer). Must
    /// be 4-byte aligned.
    pub args_offset: u32,
    /// Number arguments. Always a multiple of 32 (the value size).
    pub args_len: u32,
    /// Offset of the UTF-8 encoded part of the document. This is used to check
    /// the validity of all strings in the document up front. No alignment
    /// requirement.
    pub strings_offset: u32,
    /// Number of UTF-8 encoded bytes after `strings_offset`.
    pub strings_len: u32,
    /// Offset of arbitrary binary data block. No alignment requirement.
    pub binary_offset: u32,
    /// Length in bytes of the binary data block.
    pub binary_len: u32,
    /// Reserved.
    pub reserved1: u32,
    pub reserved2: u32,
    pub reserved3: u32,
}

const _: () = {
    assert!(
        size_of::<Header>() == 64,
        "Incompatible C ABI for this platform"
    );
    assert!(offset_of!(Header, magic) == 0, "unexpected offset");
    assert!(offset_of!(Header, version) == 8, "unexpected offset");
    assert!(
        offset_of!(Header, root_node_index) == 12,
        "unexpected offset"
    );
    assert!(offset_of!(Header, size) == 16, "unexpected offset");
    assert!(offset_of!(Header, nodes_offset) == 20, "unexpected offset");
    assert!(offset_of!(Header, nodes_len) == 24, "unexpected offset");
    assert!(offset_of!(Header, args_offset) == 28, "unexpected offset");
    assert!(offset_of!(Header, args_len) == 32, "unexpected offset");
    assert!(
        offset_of!(Header, strings_offset) == 36,
        "unexpected offset"
    );
    assert!(offset_of!(Header, strings_len) == 40, "unexpected offset");
    assert!(offset_of!(Header, binary_offset) == 44, "unexpected offset");
    assert!(offset_of!(Header, binary_len) == 48, "unexpected offset");
    assert!(offset_of!(Header, reserved1) == 52, "unexpected offset");
    assert!(offset_of!(Header, reserved2) == 56, "unexpected offset");
    assert!(offset_of!(Header, reserved3) == 60, "unexpected offset");
};

pub static DEFAULT_HEADER: Header = Header {
    magic: MAGIC,
    version: VERSION,
    size: size_of::<Header>() as u32,
    root_node_index: 0,
    nodes_offset: 0,
    nodes_len: 0,
    args_offset: 0,
    args_len: 0,
    strings_offset: 0,
    strings_len: 0,
    binary_offset: 0,
    binary_len: 0,
    reserved1: 0,
    reserved2: 0,
    reserved3: 0,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C, align(4))]
pub struct Node {
    pub args: ArgRange,
    pub children: NodeRange,
    pub name: StringRange,
    pub ty: StringRange,
}

const _: () = {
    assert!(
        size_of::<Node>() == 32,
        "Incompatible C ABI for this platform"
    );
    assert!(offset_of!(Node, args) == 0, "unexpected offset");
    assert!(offset_of!(Node, children) == 8, "unexpected offset");
    assert!(offset_of!(Node, name) == 16, "unexpected offset");
    assert!(offset_of!(Node, ty) == 24, "unexpected offset");
};

impl Node {
    pub const EMPTY: Node = Node {
        args: ArgRange::EMPTY,
        children: NodeRange::EMPTY,
        name: StringRange::EMPTY,
        ty: StringRange::EMPTY,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C, align(4))]
pub struct Arg {
    pub name: StringRange,
    pub value: Value,
}

const _: () = {
    assert!(
        size_of::<Arg>() == 20,
        "Incompatible C ABI for this platform"
    );
    assert!(offset_of!(Arg, name) == 0, "unexpected offset");
    assert!(offset_of!(Arg, value) == 8, "unexpected offset");
};

impl Arg {
    pub const EMPTY: Arg = Arg {
        name: StringRange::EMPTY,
        value: Value::NULL,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C, align(4))]
pub struct Value {
    pub ty: u32,
    pub payload: [u8; 8],
}

impl Value {
    pub const NULL: Value = Value {
        ty: 0,
        payload: [0; 8],
    };
}

const _: () = {
    assert!(
        size_of::<Value>() == 12,
        "Incompatible C ABI for this platform"
    );
    assert!(offset_of!(Value, ty) == 0, "unexpected offset");
    assert!(offset_of!(Value, payload) == 4, "unexpected offset");
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C, align(4))]
pub struct ArgRange {
    /// Start index of the value range within the `values` section of the file
    /// (`header.values_offset`).
    pub start: u32,
    /// Number of values in the range.
    pub len: u32,
}

impl ArgRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C, align(4))]
pub struct NodeRange {
    /// Start index of the node range within the `nodes` section of the file
    /// (`header.nodes_offset`).
    pub start: u32,
    /// Number of nodes in the range.
    pub len: u32,
}

impl NodeRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };
}

/// Byte range containing a string within a document.
///
/// In a valid document, the start and len are guaranteed to fall on valid UTF-8
/// char boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C, align(4))]
pub struct StringRange {
    /// Start of the string from `header.strings_offset`.
    pub start: u32,
    pub len: u32,
}

impl StringRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C, align(4))]
pub struct BinaryRange {
    pub start: u32,
    pub len: u32,
}

impl BinaryRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum RawValue {
    Null = 0,
    Bool(bool) = 1,
    Int(i64) = 2,
    Uint(u64) = 3,
    Float(f64) = 4,
    String(StringRange) = 5,
    Binary(BinaryRange) = 6,
}

impl TryFrom<Value> for RawValue {
    type Error = ValidationErrorKind;

    #[inline]
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(match value.ty {
            0 => RawValue::Null,
            1 => RawValue::Bool(value.payload[0] != 0),
            2 => RawValue::Int(i64::from_le_bytes(value.payload)),
            3 => RawValue::Uint(u64::from_le_bytes(value.payload)),
            4 => RawValue::Float(f64::from_le_bytes(value.payload)),
            5 => RawValue::String(bytemuck::cast(value.payload)),
            6 => RawValue::Binary(bytemuck::cast(value.payload)),
            _ => return Err(ValidationErrorKind::InvalidArgumentType),
        })
    }
}

impl From<RawValue> for Value {
    #[inline]
    fn from(value: RawValue) -> Self {
        match value {
            RawValue::Null => Value {
                ty: 0,
                payload: [0; 8],
            },
            RawValue::Bool(v) => Value {
                ty: 1,
                payload: if v { [1; 8] } else { [0; 8] },
            },
            RawValue::Int(v) => Value {
                ty: 2,
                payload: v.to_le_bytes(),
            },
            RawValue::Uint(v) => Value {
                ty: 3,
                payload: v.to_le_bytes(),
            },
            RawValue::Float(v) => Value {
                ty: 4,
                payload: v.to_le_bytes(),
            },
            RawValue::String(v) => Value {
                ty: 5,
                payload: bytemuck::cast(v),
            },
            RawValue::Binary(v) => Value {
                ty: 6,
                payload: bytemuck::cast(v),
            },
        }
    }
}
