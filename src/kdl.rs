use core::mem;

use alloc::borrow::{Cow, ToOwned};
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};

use crate::{
    Builder, Document, DocumentBuffer, ValueRef,
    builder::{Arg, Node, Value},
};

impl<'a> From<&'a KdlDocument> for Builder<'a> {
    fn from(value: &'a KdlDocument) -> Self {
        let mut builder = Builder::new();

        let root = builder.root_mut();
        for node in value.nodes() {
            root.push(Node::from(node));
        }

        builder
    }
}

impl<'a> From<&'a KdlDocument> for DocumentBuffer {
    #[inline]
    fn from(value: &'a KdlDocument) -> Self {
        Builder::from(value).build()
    }
}

impl<'a> From<&'a Document> for KdlDocument {
    fn from(value: &'a Document) -> Self {
        let mut doc = KdlDocument::new();

        let root = value.root();
        // KDL doesn't support arguments to the root, so push them to the root
        // node as children.
        for arg in root.args() {
            let name = arg.name.unwrap_or("-");
            let mut node = KdlNode::new(name);
            node.entries_mut().push(arg.value.into());
        }

        for child in root.children() {
            doc.nodes_mut().push(KdlNode::from(child));
        }

        doc
    }
}

impl<'a> From<&'a KdlNode> for Node<'a> {
    fn from(value: &'a KdlNode) -> Self {
        let mut node = Node::empty();

        // Handle conventionally nameless nodes.
        let mut name = value.name().repr().unwrap_or("-");
        if name == "-" {
            name = "";
        }

        node.set_name(name);
        node.set_ty(value.ty().and_then(|ty| ty.repr()).unwrap_or(""));

        for entry in value.entries() {
            node.push_arg(entry.into());
        }

        if let Some(children) = value.children() {
            for child in children.nodes() {
                node.push(Node::from(child));
            }
        }

        node
    }
}

impl From<Node<'_>> for KdlNode {
    fn from(mut value: Node) -> Self {
        // Produce conventionally nameless nodes.
        let name = if value.name.is_empty() {
            "-"
        } else {
            value.name.as_ref()
        };

        let mut node = KdlNode::new(name);
        if !value.ty.is_empty() {
            node.set_ty(&*value.ty);
        }

        for arg in mem::take(value.args_mut()) {
            node.entries_mut().push(arg.into());
        }

        if !value.children().is_empty() {
            let children = node.children_mut().get_or_insert_default();
            for child in mem::take(value.children_mut()) {
                children.nodes_mut().push(child.into());
            }
        }

        node
    }
}

impl From<crate::Node<'_>> for KdlNode {
    fn from(value: crate::Node<'_>) -> Self {
        // Produce conventionally nameless nodes.
        let name = match value.name() {
            None => "-",
            Some(name) => name,
        };
        let mut node = KdlNode::new(name);

        if let Some(ty) = value.ty() {
            node.set_ty(ty);
        }

        for arg in value.args() {
            node.entries_mut().push(arg.into());
        }

        if !value.children().is_empty() {
            let children = node.children_mut().get_or_insert_default();
            for child in value.children() {
                children.nodes_mut().push(child.into());
            }
        }

        node
    }
}

impl<'a> From<&'a KdlEntry> for Arg<'a> {
    #[inline]
    fn from(value: &'a KdlEntry) -> Self {
        Arg {
            name: value.name().and_then(|name| name.repr()).map(Cow::Borrowed),
            value: value.value().into(),
        }
    }
}

impl From<Arg<'_>> for KdlEntry {
    #[inline]
    fn from(value: Arg) -> Self {
        if let Some(name) = value.name {
            KdlEntry::new_prop(&*name, value.value)
        } else {
            KdlEntry::new(value.value)
        }
    }
}

impl From<crate::Arg<'_>> for KdlEntry {
    #[inline]
    fn from(value: crate::Arg<'_>) -> Self {
        if let Some(name) = value.name {
            KdlEntry::new_prop(name, value.value)
        } else {
            KdlEntry::new(value.value)
        }
    }
}

impl<'a> From<&'a KdlValue> for ValueRef<'a> {
    #[inline]
    fn from(value: &'a KdlValue) -> Self {
        match value {
            KdlValue::String(s) => ValueRef::String(s),
            KdlValue::Integer(value) => {
                ValueRef::Int((*value).try_into().expect("Integer value too large"))
            }
            KdlValue::Float(value) => ValueRef::Float(*value),
            KdlValue::Bool(value) => ValueRef::Bool(*value),
            KdlValue::Null => ValueRef::Null,
        }
    }
}

impl From<ValueRef<'_>> for KdlValue {
    #[inline]
    fn from(value: ValueRef<'_>) -> Self {
        match value {
            ValueRef::String(s) => KdlValue::String(s.to_owned()),
            ValueRef::Binary(_) => panic!("Binary values are not supported in KDL"),
            ValueRef::Int(i) => KdlValue::Integer(i as _),
            ValueRef::Uint(u) => KdlValue::Integer(u as _),
            ValueRef::Float(f) => KdlValue::Float(f),
            ValueRef::Bool(b) => KdlValue::Bool(b),
            ValueRef::Null => KdlValue::Null,
        }
    }
}

impl<'a> From<&'a KdlValue> for Value<'a> {
    #[inline]
    fn from(value: &'a KdlValue) -> Self {
        match value {
            KdlValue::String(s) => Value::String(Cow::Borrowed(s)),
            KdlValue::Integer(value) => {
                Value::Int((*value).try_into().expect("Integer value too large"))
            }
            KdlValue::Float(value) => Value::Float(*value),
            KdlValue::Bool(value) => Value::Bool(*value),
            KdlValue::Null => Value::Null,
        }
    }
}

impl From<Value<'_>> for KdlValue {
    #[inline]
    fn from(value: Value<'_>) -> Self {
        match value {
            Value::String(s) => KdlValue::String(s.into_owned()),
            Value::Binary(_) => panic!("Binary values are not supported in KDL"),
            Value::Int(i) => KdlValue::Integer(i as _),
            Value::Uint(u) => KdlValue::Integer(u as _),
            Value::Float(f) => KdlValue::Float(f),
            Value::Bool(b) => KdlValue::Bool(b),
            Value::Null => KdlValue::Null,
        }
    }
}
