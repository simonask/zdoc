#![cfg(feature = "alloc")]

use zdocument::{ValueRef, builder::Builder, codec};

#[test]
fn empty() {
    let doc = Builder::new().build();
    assert!(doc.is_empty());
    assert!(doc.as_bytes().is_empty());
}

#[test]
fn named_root() {
    let doc = Builder::new()
        .with_root(|root| {
            root.set_name("root");
        })
        .build();
    assert!(!doc.is_empty());
    assert_eq!(
        *doc.header(),
        codec::Header {
            magic: codec::MAGIC,
            version: codec::VERSION,
            root_node_index: 0,
            size: (size_of::<codec::Header>() + size_of::<codec::Node>() + 4) as u32,
            nodes_offset: size_of::<codec::Header>() as u32,
            nodes_len: 1,
            args_offset: (size_of::<codec::Header>() + size_of::<codec::Node>()) as u32,
            args_len: 0,
            strings_offset: (size_of::<codec::Header>() + size_of::<codec::Node>()) as u32,
            strings_len: 4,
            binary_offset: (size_of::<codec::Header>() + size_of::<codec::Node>() + 4) as u32,
            binary_len: 0,
            reserved1: 0,
            reserved2: 0,
            reserved3: 0,
        }
    );
    let bytes = doc.as_bytes();

    let nodes = doc.nodes();
    assert_eq!(nodes.len(), 1);
    assert_eq!(
        nodes[0],
        codec::Node {
            args: codec::ArgRange::EMPTY,
            children: codec::NodeRange::EMPTY,
            name: codec::StringRange { start: 0, len: 4 },
            ty: codec::StringRange::EMPTY,
        }
    );

    let strings_start = size_of::<codec::Header>() + size_of::<codec::Node>();
    let strings_end = strings_start + 4;
    let strings = core::str::from_utf8(&bytes[strings_start..strings_end]).unwrap();

    assert_eq!(strings, "root");
    assert_eq!(doc.root().name(), Some("root"));
}

#[test]
fn dictionary() {
    let doc = Builder::new()
        .with_root(|root| {
            root.push_named("key1", 123);
            root.push_named_with("dict", |dict| {
                dict.push_named("key", 456);
            });
            root.push_named_with("list", |list| {
                list.push(789);
                list.push(0);
            });
        })
        .build();

    let nodes = doc.nodes();
    assert_eq!(nodes.len(), 5);
    let root = doc.root();
    assert_eq!(root.raw_index(), 0);
    assert_eq!(root.children().len(), 3);
    assert!(root.is_dictionary_like());
    assert_eq!(root.name(), None);

    let key1 = root.get("key1").unwrap();
    assert_eq!(key1.name(), Some("key1"));
    assert_eq!(key1.value(), Some(ValueRef::Int(123)));

    let key1 = root.children().get(0).unwrap();
    assert_eq!(key1.name(), Some("key1"));
    assert_eq!(key1.value(), Some(ValueRef::Int(123)));

    let dict = root.children().get("dict").unwrap();
    assert_eq!(dict.name(), Some("dict"));
    assert_eq!(dict.children().len(), 1);
    assert_eq!(dict.value(), None);
    let dict_key = dict.get("key").unwrap();
    assert_eq!(dict_key.value(), Some(ValueRef::Int(456)));

    let list = root.children().get("list").unwrap();
    assert_eq!(list.children().len(), 0);
    assert_eq!(list.name(), Some("list"));
    assert_eq!(list.args().get(0).unwrap().value, ValueRef::Int(789));
    assert_eq!(list.args().get(1).unwrap().value, ValueRef::Int(0));
}
