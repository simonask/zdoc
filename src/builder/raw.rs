use alloc::vec::Vec;
use bytemuck::{bytes_of, cast_slice};

use crate::{DocumentBuffer, ValueRef, codec, raw::RawDocumentBuffer};

use super::{Strings, Value};

/// Raw builder.
///
/// This builder cannot add or remove children and arguments of a node after the
/// node has been created, but it allocates way less than
/// [`Builder`](super::Builder), and allocations may be amortized when building
/// multiple documents.
///
/// This is useful when converting between formats where the number of children
/// and arguments of each node are known in advance, such as KDL, JSON, or XML.
#[derive(Clone, Default)]
pub struct RawBuilder {
    nodes: Vec<codec::Node>,
    args: Vec<codec::Arg>,
    strings: Strings,
    binary: Vec<u8>,
}

impl RawBuilder {
    #[inline]
    #[must_use]
    pub fn file_size(&self) -> usize {
        let nodes_size = if self.nodes.len() == 1 && self.nodes[0] == codec::Node::EMPTY {
            0
        } else {
            self.nodes.len() * size_of::<codec::Node>()
        };

        size_of::<codec::Header>()
            + nodes_size
            + size_of::<codec::Arg>() * self.args.len()
            + self.strings.buffer.len()
            + self.binary.len()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.args.clear();
        self.strings.clear();
        self.binary.clear();
    }

    /// Clear the builder and set the root node.
    pub fn set_root(&mut self, build: impl BuildRawNode) {
        self.clear();
        self.nodes.push(codec::Node::EMPTY);
        build.build(self, 0);
    }

    fn build_children(
        &mut self,
        children: impl ExactSizeIterator<Item: BuildRawNode>,
    ) -> codec::NodeRange {
        let len = children.len() as u32;
        if len == 0 {
            return codec::NodeRange::EMPTY;
        }

        let start = self.nodes.len() as u32;
        let end = start.checked_add(len).expect("too many nodes");
        self.nodes.resize(end as usize, codec::Node::EMPTY);
        for (index, build_child) in children.enumerate() {
            let index = start + index as u32;
            build_child.build(self, index);
        }
        codec::NodeRange { start, len }
    }

    fn build_args(&mut self, args: impl ExactSizeIterator<Item: BuildRawArg>) -> codec::ArgRange {
        let len = args.len() as u32;
        if len == 0 {
            return codec::ArgRange::EMPTY;
        }

        let start = self.args.len() as u32;
        let end = start.checked_add(len).expect("too many args");
        self.args.resize(end as usize, codec::Arg::EMPTY);
        for (index, build_arg) in args.enumerate() {
            let index = start + index as u32;
            build_arg.build(self, index);
        }
        codec::ArgRange { start, len }
    }

    #[inline]
    fn node_mut(&mut self, index: u32) -> &mut codec::Node {
        &mut self.nodes[index as usize]
    }

    #[inline]
    fn arg_mut(&mut self, index: u32) -> &mut codec::Arg {
        &mut self.args[index as usize]
    }

    #[inline]
    fn add_string(&mut self, s: &str) -> codec::StringRange {
        self.strings.add_string(s)
    }

    #[inline]
    fn add_string_intern(&mut self, s: &str) -> codec::StringRange {
        self.strings.add_string_intern(s)
    }

    #[inline]
    fn add_binary(&mut self, data: &[u8]) -> codec::BinaryRange {
        let start = self.binary.len() as u32;
        let len = data.len() as u32;
        let _end = start.checked_add(len).expect("too much binary data");
        self.binary.extend_from_slice(data);
        codec::BinaryRange { start, len }
    }

    #[inline]
    fn add_value(&mut self, value: ValueRef<'_>) -> codec::Value {
        match value {
            ValueRef::Null => codec::RawValue::Null,
            ValueRef::Bool(value) => codec::RawValue::Bool(value),
            ValueRef::Int(value) => codec::RawValue::Int(value),
            ValueRef::Uint(value) => codec::RawValue::Uint(value),
            ValueRef::Float(value) => codec::RawValue::Float(value),
            ValueRef::String(value) => {
                let range = self.add_string(value);
                codec::RawValue::String(range)
            }
            ValueRef::Binary(value) => {
                let range = self.add_binary(value);
                codec::RawValue::Binary(range)
            }
        }
        .into()
    }

    #[inline]
    fn add_arg(&mut self, name: &str, value: ValueRef) -> codec::Arg {
        let name = self.add_string_intern(name);
        let value = self.add_value(value);
        codec::Arg { name, value }
    }

    /// Build the document.
    ///
    /// This simply concatenates internal buffers and adds the file header.
    ///
    /// # Panics
    ///
    /// This panics if the size of the document would exceed 4 GiB.
    #[must_use]
    pub fn build(&self) -> DocumentBuffer {
        let nodes_len = if self.nodes.len() == 1 && self.nodes[0] == codec::Node::EMPTY {
            return DocumentBuffer::default();
        } else {
            self.nodes.len() as u32
        };

        let size = self
            .file_size()
            .try_into()
            .expect("document would be too large (> 4 GiB)");

        let args_offset =
            size_of::<codec::Header>() as u32 + nodes_len * size_of::<codec::Node>() as u32;
        let args_len = self.args.len() as u32;
        let strings_offset = args_offset + args_len * size_of::<codec::Arg>() as u32;
        let strings_len = self.strings.buffer.len() as u32;
        let binary_offset = strings_offset + strings_len;
        let binary_len = self.binary.len() as u32;

        let header = codec::Header {
            magic: codec::MAGIC,
            version: codec::VERSION,
            root_node_index: 0,
            size,
            nodes_offset: size_of::<codec::Header>() as u32,
            nodes_len,
            args_offset,
            args_len,
            strings_offset,
            strings_len,
            binary_offset,
            binary_len,
            reserved1: 0,
            reserved2: 0,
            reserved3: 0,
        };

        let mut buffer = Vec::with_capacity(size as usize);
        buffer.extend_from_slice(bytes_of(&header));
        debug_assert_eq!(buffer.len(), size_of::<codec::Header>());
        buffer.extend_from_slice(cast_slice(&self.nodes));
        debug_assert_eq!(buffer.len(), args_offset as usize);
        buffer.extend_from_slice(cast_slice(&self.args));
        debug_assert_eq!(buffer.len(), strings_offset as usize);
        buffer.extend_from_slice(self.strings.buffer.as_bytes());
        debug_assert_eq!(buffer.len(), binary_offset as usize);
        buffer.extend_from_slice(&self.binary);

        unsafe {
            // SAFETY: We just built a valid document.
            let raw = RawDocumentBuffer::from_buffer(buffer);
            if cfg!(debug_assertions) {
                raw.check().unwrap();
            }
            DocumentBuffer::from_raw_unchecked(raw)
        }
    }
}

/// Raw node builder, agnostic about the type of its children and arguments.
pub struct RawNode<'a, Children, Args> {
    pub ty: Option<&'a str>,
    pub name: Option<&'a str>,
    pub children: Children,
    pub args: Args,
}

pub trait BuildRawNode {
    fn build(self, builder: &mut RawBuilder, index: u32);
}

impl BuildRawNode for () {
    fn build(self, _builder: &mut RawBuilder, _index: u32) {}
}

impl<Children, Args> BuildRawNode for RawNode<'_, Children, Args>
where
    Children: IntoIterator<IntoIter: ExactSizeIterator, Item: BuildRawNode>,
    Args: IntoIterator<IntoIter: ExactSizeIterator, Item: BuildRawArg>,
{
    fn build(self, builder: &mut RawBuilder, index: u32) {
        let args = builder.build_args(self.args.into_iter());
        let children = builder.build_children(self.children.into_iter());
        let ty = builder.add_string_intern(self.ty.unwrap_or(""));
        let name = builder.add_string_intern(self.name.unwrap_or(""));
        let node = builder.node_mut(index);
        node.args = args;
        node.children = children;
        node.ty = ty;
        node.name = name;
    }
}

impl BuildRawNode for crate::Node<'_> {
    fn build(self, builder: &mut RawBuilder, index: u32) {
        let args = builder.build_args(self.args().into_iter());
        let children = builder.build_children(self.children().into_iter());
        let ty = builder.add_string_intern(self.ty().unwrap_or(""));
        let name = builder.add_string_intern(self.name().unwrap_or(""));
        let node = builder.node_mut(index);
        node.args = args;
        node.children = children;
        node.ty = ty;
        node.name = name;
    }
}

impl BuildRawNode for &super::Node<'_> {
    fn build(self, builder: &mut RawBuilder, index: u32) {
        let args = builder.build_args(self.args().iter());
        let children = builder.build_children(self.children().iter());
        let ty = builder.add_string_intern(&self.ty);
        let name = builder.add_string_intern(&self.name);
        let node = builder.node_mut(index);
        node.args = args;
        node.children = children;
        node.ty = ty;
        node.name = name;
    }
}

impl BuildRawNode for super::Node<'_> {
    #[inline]
    fn build(self, builder: &mut RawBuilder, index: u32) {
        BuildRawNode::build(&self, builder, index);
    }
}

pub trait BuildRawArg {
    fn build(self, builder: &mut RawBuilder, index: u32);
}

impl BuildRawArg for () {
    #[inline]
    fn build(self, _builder: &mut RawBuilder, _index: u32) {}
}

impl BuildRawArg for ValueRef<'_> {
    #[inline]
    fn build(self, builder: &mut RawBuilder, index: u32) {
        builder.arg_mut(index).value = builder.add_value(self);
    }
}

impl BuildRawArg for &Value<'_> {
    #[inline]
    fn build(self, builder: &mut RawBuilder, index: u32) {
        builder.arg_mut(index).value = builder.add_value(self.into());
    }
}

impl BuildRawArg for Value<'_> {
    #[inline]
    fn build(self, builder: &mut RawBuilder, index: u32) {
        builder.arg_mut(index).value = builder.add_value((&self).into());
    }
}

impl BuildRawArg for super::Arg<'_> {
    #[inline]
    fn build(self, builder: &mut RawBuilder, index: u32) {
        *builder.arg_mut(index) =
            builder.add_arg(self.name.as_deref().unwrap_or(""), (&self.value).into());
    }
}

impl BuildRawArg for &super::Arg<'_> {
    #[inline]
    fn build(self, builder: &mut RawBuilder, index: u32) {
        *builder.arg_mut(index) =
            builder.add_arg(self.name.as_deref().unwrap_or(""), (&self.value).into());
    }
}

impl BuildRawArg for crate::Arg<'_> {
    #[inline]
    fn build(self, builder: &mut RawBuilder, index: u32) {
        *builder.arg_mut(index) = builder.add_arg(self.name.unwrap_or(""), self.value);
    }
}

impl BuildRawArg for &crate::Arg<'_> {
    #[inline]
    fn build(self, builder: &mut RawBuilder, index: u32) {
        *builder.arg_mut(index) = builder.add_arg(self.name.unwrap_or(""), self.value);
    }
}
