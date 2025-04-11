#![cfg(feature = "serde")]

use bytemuck::{bytes_of, offset_of};
use zdocument::{Document, codec, serde::from_document};

#[derive(Clone, Copy)]
#[repr(C)]
struct Bin {
    header: codec::Header,
    nodes: [codec::Node; 3],
    args: [codec::Arg; 3],
    strings: [u8; 43],
    binary: [u8; 0],
}

unsafe impl bytemuck::Zeroable for Bin {}
unsafe impl bytemuck::Pod for Bin {}

impl Default for Bin {
    fn default() -> Self {
        Self {
            header: codec::Header::default(),
            nodes: Default::default(),
            args: Default::default(),
            strings: [0; 43],
            binary: Default::default(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
struct Struct<'a> {
    string: &'a str,
    int: i32,
    enum_1: Enum<'a>,
    enum_2: Enum<'a>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
enum Enum<'a> {
    UnitVariant,
    NewTypeValue(&'a str),
    Struct { int: i32 },
}

#[test]
fn de_no_alloc() {
    let bin = Bin {
        header: codec::Header {
            magic: codec::MAGIC,
            version: 1,
            root_node_index: 0,
            size: 264,
            nodes_offset: offset_of!(Bin, nodes) as u32,
            nodes_len: 3,
            args_offset: offset_of!(Bin, args) as u32,
            args_len: 3,
            strings_offset: offset_of!(Bin, strings) as u32,
            strings_len: 43,
            binary_offset: offset_of!(Bin, binary) as u32,
            binary_len: 0,
            reserved1: 0,
            reserved2: 0,
            reserved3: 0,
        },
        nodes: [
            codec::Node {
                // string and int
                args: (0..2).into(),
                children: (1..3).into(),
                name: codec::StringRange::EMPTY,
                ty: codec::StringRange::EMPTY,
            },
            // enum_1 (UnitVariant)
            codec::Node {
                args: codec::ArgRange::EMPTY,
                children: codec::NodeRange::EMPTY,
                // enum_1
                name: (9..15).into(),
                // UnitVariant
                ty: (21..32).into(),
            },
            // enum_2 (Struct)
            codec::Node {
                args: (2..3).into(),
                children: codec::NodeRange::EMPTY,
                // enum_2
                name: (15..21).into(),
                // Struct
                ty: (32..38).into(),
            },
        ],
        args: [
            // string
            codec::Arg {
                name: (0..6).into(),
                // "hello"
                value: codec::RawValue::String((38..43).into()).into(),
            },
            // int
            codec::Arg {
                name: (6..9).into(),
                value: codec::RawValue::Int(123).into(),
            },
            // int
            codec::Arg {
                name: (6..9).into(),
                value: codec::RawValue::Int(456).into(),
            },
        ],
        strings: *b"stringintenum_1enum_2UnitVariantStructhello",
        binary: [],
    };

    let bytes = bytes_of(&bin);
    let doc = Document::from_slice(bytes).unwrap();
    let s: Struct = from_document(doc).unwrap();

    assert_eq!(
        s,
        Struct {
            string: "hello",
            int: 123,
            enum_1: Enum::UnitVariant,
            enum_2: Enum::Struct { int: 456 },
        }
    );
}
