mod raw;
pub use raw::*;

use core::mem;

use alloc::{borrow::Cow, string::String, vec, vec::Vec};
use hashbrown::{HashMap, hash_map};

use crate::{Document, DocumentBuffer, ValueRef, access, codec::StringRange, document};

/// Builder for [`Document`](crate::Document)s.
///
/// The builder can be used as a mutable document, i.e. it also supports
/// reading and deserialization.
#[derive(Clone, Debug)]
pub struct Builder<'a> {
    root: Node<'a>,
    auto_intern_limit: usize,
}

impl Default for Builder<'_> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Builder<'a> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            root: Node::empty(),
            auto_intern_limit: 128,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.root = Node::empty();
    }

    /// Create a mutable builder from an immutable document.
    #[must_use]
    pub fn from_document(doc: &'a Document) -> Self {
        let root = Node::from_document(doc.root());
        let mut builder = Self::new();
        builder.root = root;
        builder
    }

    #[must_use]
    pub fn auto_intern_limit(&self) -> usize {
        self.auto_intern_limit
    }

    #[inline]
    pub fn set_auto_intern_limit(&mut self, limit: usize) -> &mut Self {
        self.auto_intern_limit = limit;
        self
    }

    #[inline]
    pub fn set_root(&mut self, node: Node<'a>) {
        self.root = node;
    }

    #[inline]
    #[must_use]
    pub fn root(&self) -> &Node<'a> {
        &self.root
    }

    #[inline]
    pub fn root_mut(&mut self) -> &mut Node<'a> {
        &mut self.root
    }

    pub fn with_root(&mut self, f: impl FnOnce(&mut Node<'a>)) -> &mut Self {
        f(&mut self.root);
        self
    }

    #[must_use]
    pub fn build(&self) -> DocumentBuffer {
        let mut cache = BuildCache::default();
        self.build_with_cache(&mut cache)
    }

    pub fn build_with_cache(&self, cache: &mut BuildCache) -> DocumentBuffer {
        let root = &self.root;
        if root.is_empty() {
            return DocumentBuffer::default();
        }

        // This recursively serializes the document to the binary format.
        cache.raw.set_root(root);

        cache.raw.build()
    }
}

/// Allocation cache for [`Builder`], to amortize allocations between calls to
/// [`build()`](Builder::build).
#[derive(Clone, Default)]
pub struct BuildCache {
    raw: RawBuilder,
}

impl BuildCache {
    #[inline]
    pub fn reset(&mut self) {
        self.raw.clear();
    }

    /// Reset the cache and reclaim any memory that it is currently consuming.
    #[inline]
    pub fn deallocate(&mut self) {
        *self = Self::default();
    }
}

/// Owned node in a document.
///
/// This corresponds to the non-owning [`Node`](crate::Node) type, but may be
/// owned by a [`Builder`].
#[derive(Clone, Default)]
pub struct Node<'a> {
    pub children: Cow<'a, [Node<'a>]>,
    pub args: Cow<'a, [Arg<'a>]>,
    pub name: Cow<'a, str>,
    pub ty: Cow<'a, str>,
}

impl<'a> Node<'a> {
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Node {
            children: Cow::Borrowed(&[]),
            args: Cow::Borrowed(&[]),
            name: Cow::Borrowed(""),
            ty: Cow::Borrowed(""),
        }
    }

    #[must_use]
    pub fn from_values(args: impl IntoIterator<Item: Into<Value<'a>>>) -> Self {
        let args = args
            .into_iter()
            .map(Into::into)
            .map(Arg::from)
            .collect::<Vec<_>>();
        let mut node = Self::empty();
        node.set_args(args);
        node
    }

    #[must_use]
    pub fn from_args(args: impl IntoIterator<Item: Into<Arg<'a>>>) -> Self {
        let args = args.into_iter().collect::<Vec<_>>();
        let mut node = Self::empty();
        node.set_args(args);
        node
    }

    #[must_use]
    pub fn from_children(children: impl IntoIterator<Item: Into<Node<'a>>>) -> Self {
        let children = children.into_iter().map(Into::into).collect::<Vec<_>>();
        let mut node = Self::empty();
        node.set_children(children);
        node
    }

    pub fn from_entries(entries: impl IntoIterator<Item: IntoEntry<'a>>) -> Self {
        let mut node = Self::empty();
        for entry in entries {
            node.push(entry);
        }
        node
    }

    /// Create a node from a serialized node coming from a
    /// [`Document`](crate::Document).
    ///
    /// Strings and binary data will be borrowed from `node` rather than copied.
    pub fn from_document(node: document::Node<'a>) -> Self {
        let children = node
            .children()
            .into_iter()
            .map(Self::from_document)
            .collect();
        let args = node.args().into_iter().map(Arg::from_document).collect();
        let name = Cow::Borrowed(node.name().unwrap_or_default());
        let ty = Cow::Borrowed(node.ty().unwrap_or_default());
        Self {
            children,
            args,
            name,
            ty,
        }
    }

    /// Push an argument to the node.
    pub fn push_arg(&mut self, arg: Arg<'a>) -> &mut Self {
        self.args_mut().push(arg);
        self
    }

    /// Push an unnamed argument to the node.
    pub fn push_unnamed_arg(&mut self, value: impl Into<Value<'a>>) -> &mut Self {
        self.push_arg(Arg {
            name: None,
            value: value.into(),
        })
    }

    /// Push a named arg to the node.
    pub fn push_named_arg(
        &mut self,
        name: impl Into<Cow<'a, str>>,
        value: impl Into<Value<'a>>,
    ) -> &mut Self {
        self.push_arg(Arg {
            name: Some(name.into()),
            value: value.into(),
        })
    }

    pub fn add_unnamed_arg(&mut self, value: impl Into<Value<'a>>) -> &mut Self {
        self.push_arg(Arg {
            name: None,
            value: value.into(),
        })
    }

    /// Add a child node, calling `f` to build the child.
    pub fn add_child_with(&mut self, f: impl FnOnce(&mut Node<'a>)) -> &mut Self {
        let mut child = Node::empty();
        f(&mut child);
        self.children_mut().push(child);
        self
    }

    /// Insert a child node, calling `f` to build the child.
    ///
    /// `index` must be `<= len`.
    pub fn insert_child_with(&mut self, index: usize, f: impl FnOnce(&mut Node)) -> &mut Self {
        let mut child = Node::empty();
        f(&mut child);
        self.children_mut().insert(index, child);
        self
    }

    /// Create an unnamed node with a single value argument or child.
    ///
    /// A "list-like" node will have only unnamed children.
    pub fn unnamed(value: impl IntoEntry<'a>) -> Self {
        value.into_entry().into()
    }

    /// Create a key-value node, which is a named node with a single child.
    ///
    /// A "dictionary-like" node will have only key-value children.
    ///
    /// If the value is a primitive, use [`key_value()`] instead.
    pub fn key_value_with(
        key: impl Into<Cow<'a, str>>,
        with_child: impl FnOnce(&mut Node<'a>),
    ) -> Self {
        let mut node = Self::empty();
        node.set_name(key).add_child_with(with_child);
        node
    }

    /// Add a key-value child to this node.
    pub fn push_named_with(
        &mut self,
        name: impl Into<Cow<'a, str>>,
        with: impl FnOnce(&mut Node<'a>),
    ) -> &mut Self {
        let mut node = Node::empty();
        node.set_name(name);
        with(&mut node);
        self.push(node);
        self
    }

    /// Push an unnamed child, treating this node as a list.
    pub fn push(&mut self, value: impl IntoEntry<'a>) -> &mut Self {
        match value.into_entry() {
            Entry::Arg(arg) => self.args.to_mut().push(arg),
            Entry::Child(node) => self.children.to_mut().push(node),
        }
        self
    }

    #[must_use]
    pub fn with_entry(mut self, entry: impl IntoEntry<'a>) -> Self {
        self.push(entry);
        self
    }

    /// Push an unnamed child, treating this node as a list.
    ///
    /// This maintains the order of existing arguments and children, which means
    /// that if the node has children, `value` will always be added as a child,
    /// and if `value` is a child node, but the node has arguments, those
    /// arguments will be converted to children.
    pub fn push_ordered(&mut self, value: impl IntoEntry<'a>) -> &mut Self {
        match value.into_entry() {
            Entry::Arg(arg) => {
                if self.children().is_empty() {
                    self.args_mut().push(arg);
                } else {
                    for arg in mem::take(self.args_mut()) {
                        self.children_mut().push(arg.into_key_value_node());
                    }
                    self.children_mut().push(arg.into_key_value_node());
                }
            }
            Entry::Child(node) => {
                // Convert existing arguments to children in order to maintain
                // their order.
                for arg in mem::take(self.args_mut()) {
                    self.children_mut().push(arg.into_key_value_node());
                }
                self.children_mut().push(node);
            }
        }
        self
    }

    /// Returns `true` if the node has a named argument with the given name.
    ///
    /// Note: This is a linear search.
    #[must_use]
    pub fn contains_named_argument(&self, name: &str) -> bool {
        self.args()
            .iter()
            .any(|arg| arg.name.as_deref() == Some(name))
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
            && self.args.is_empty()
            && self.name.is_empty()
            && self.ty.is_empty()
    }

    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    #[must_use]
    pub fn ty(&self) -> &str {
        &self.ty
    }

    pub fn set_name(&mut self, name: impl Into<Cow<'a, str>>) -> &mut Self {
        self.name = name.into();
        self
    }

    pub fn set_ty(&mut self, ty: impl Into<Cow<'a, str>>) -> &mut Self {
        self.ty = ty.into();
        self
    }

    pub fn set_children(&mut self, children: impl IntoIterator<Item = Node<'a>>) -> &mut Self {
        // Note: When passed a `Vec`, this is guaranteed to not reallocate.
        self.children = children.into_iter().collect();
        self
    }

    #[inline]
    pub fn set_children_borrowed(&mut self, children: &'a [Node<'a>]) -> &mut Self {
        self.children = Cow::Borrowed(children);
        self
    }

    #[inline]
    #[must_use]
    pub fn children(&self) -> &[Node<'a>] {
        &self.children
    }

    #[inline]
    pub fn children_mut(&mut self) -> &mut Vec<Node<'a>> {
        self.children.to_mut()
    }

    pub fn set_args(&mut self, args: impl IntoIterator<Item: Into<Arg<'a>>>) -> &mut Self {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    #[inline]
    pub fn set_args_borrowed(&mut self, args: &'a [Arg<'a>]) -> &Self {
        self.args = Cow::Borrowed(args);
        self
    }

    #[inline]
    #[must_use]
    pub fn args(&self) -> &[Arg<'a>] {
        &self.args
    }

    #[inline]
    pub fn args_mut(&mut self) -> &mut Vec<Arg<'a>> {
        self.args.to_mut()
    }
}

impl<'a> From<crate::Node<'a>> for Node<'a> {
    fn from(value: crate::Node<'a>) -> Self {
        let mut node = Node::empty();
        node.set_ty(value.ty().unwrap_or_default())
            .set_name(value.name().unwrap_or_default())
            .set_args(value.args().into_iter().map(Arg::from))
            .set_children(value.children().into_iter().map(Node::from));
        node
    }
}

impl<'a> core::fmt::Debug for Node<'a> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::debug::debug_entry(&access::EntryRef::<&Arg<'a>, _>::Child(self)).fmt(f)
    }
}

#[derive(Clone, Default)]
pub struct Arg<'a> {
    pub name: Option<Cow<'a, str>>,
    pub value: Value<'a>,
}

impl<'a> Arg<'a> {
    #[inline]
    pub fn from_document(arg: document::Arg<'a>) -> Self {
        let name = arg.name.map(Cow::Borrowed);
        let value = Value::from_document(arg.value);
        Self { name, value }
    }

    #[inline]
    pub fn new(name: &'a str, value: impl Into<Value<'a>>) -> Self {
        Self {
            name: Some(Cow::Borrowed(name)),
            value: value.into(),
        }
    }

    #[inline]
    pub fn unnamed(value: impl Into<Value<'a>>) -> Self {
        Self {
            name: None,
            value: value.into(),
        }
    }

    #[inline]
    #[must_use]
    pub fn into_key_value_node(self) -> Node<'a> {
        Node {
            children: Cow::Borrowed(&[]),
            args: Cow::Owned(vec![Arg {
                name: None,
                value: self.value,
            }]),
            name: self.name.unwrap_or_default(),
            ty: Cow::Borrowed(""),
        }
    }
}

impl<'a> From<(&'a str, Value<'a>)> for Arg<'a> {
    #[inline]
    fn from(value: (&'a str, Value<'a>)) -> Self {
        Arg {
            name: Some(Cow::Borrowed(value.0)),
            value: value.1,
        }
    }
}

impl<'a> From<Value<'a>> for Arg<'a> {
    #[inline]
    fn from(value: Value<'a>) -> Self {
        Arg { name: None, value }
    }
}

impl<'a> From<crate::Arg<'a>> for Arg<'a> {
    #[inline]
    fn from(value: crate::Arg<'a>) -> Self {
        Arg {
            name: value.name.map(Cow::Borrowed),
            value: value.value.into(),
        }
    }
}

impl<'a> core::fmt::Debug for Arg<'a> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::debug::debug_entry(&access::EntryRef::<_, &Node<'a>>::Arg(self)).fmt(f)
    }
}

/// Argument or child of a [`Node`] belonging to a [`Builder`].
pub enum Entry<'a> {
    Arg(Arg<'a>),
    Child(Node<'a>),
}

impl<'a> Entry<'a> {
    #[inline]
    #[must_use]
    pub fn null() -> Self {
        Self::Arg(Arg {
            name: None,
            value: Value::Null,
        })
    }

    #[inline]
    pub fn set_name(&mut self, name: impl Into<Cow<'a, str>>) {
        let name = name.into();
        match self {
            Entry::Arg(arg) => {
                arg.name = if name.is_empty() { None } else { Some(name) };
            }
            Entry::Child(node) => {
                node.set_name(name);
            }
        }
    }

    #[inline]
    pub fn reset_as_value(&mut self) -> &mut Value<'a> {
        match self {
            Entry::Arg(arg) => &mut arg.value,
            Entry::Child(_) => {
                *self = Entry::Arg(Arg {
                    name: None,
                    value: Value::Null,
                });
                let Entry::Arg(arg) = self else {
                    unreachable!()
                };
                &mut arg.value
            }
        }
    }

    #[inline]
    pub fn reset_as_node(&mut self) -> &mut Node<'a> {
        match self {
            Entry::Arg(_) => {
                *self = Entry::Child(Node::empty());
                let Entry::Child(node) = self else {
                    unreachable!()
                };
                node
            }
            Entry::Child(node) => node,
        }
    }
}
impl<'a> From<Arg<'a>> for Entry<'a> {
    #[inline]
    fn from(value: Arg<'a>) -> Self {
        Entry::Arg(value)
    }
}

impl<'a> From<Value<'a>> for Entry<'a> {
    #[inline]
    fn from(value: Value<'a>) -> Self {
        Entry::Arg(Arg { name: None, value })
    }
}

impl<'a> From<Node<'a>> for Entry<'a> {
    #[inline]
    fn from(value: Node<'a>) -> Self {
        Entry::Child(value)
    }
}

impl<'a> From<Entry<'a>> for Node<'a> {
    #[inline]
    fn from(value: Entry<'a>) -> Self {
        match value {
            Entry::Arg(arg) => arg.into_key_value_node(),
            Entry::Child(node) => node,
        }
    }
}

impl core::fmt::Debug for Entry<'_> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::debug::debug_entry(&self.into()).fmt(f)
    }
}

/// Possibly owned value.
///
/// This corresponds to [`ValueRef`](crate::ValueRef), but may be owned by a
/// [`Builder`].
#[derive(Clone, Default, Debug)]
pub enum Value<'a> {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    String(Cow<'a, str>),
    Binary(Cow<'a, [u8]>),
}

impl<'a> Value<'a> {
    #[inline]
    #[must_use]
    pub fn from_document(value: ValueRef<'a>) -> Self {
        value.into()
    }
}

impl<'a, T: Into<Value<'a>>> From<Option<T>> for Value<'a> {
    #[inline]
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => value.into(),
            None => Value::Null,
        }
    }
}

impl<'a> From<ValueRef<'a>> for Value<'a> {
    #[inline]
    fn from(value: ValueRef<'a>) -> Self {
        match value {
            ValueRef::Null => Self::Null,
            ValueRef::Bool(v) => Self::Bool(v),
            ValueRef::Int(v) => Self::Int(v),
            ValueRef::Uint(v) => Self::Uint(v),
            ValueRef::Float(v) => Self::Float(v),
            ValueRef::String(s) => Self::String(Cow::Borrowed(s)),
            ValueRef::Binary(b) => Self::Binary(Cow::Borrowed(b)),
        }
    }
}

impl<'a> From<&'a Value<'a>> for ValueRef<'a> {
    fn from(value: &'a Value<'a>) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(v) => Self::Bool(*v),
            Value::Int(v) => Self::Int(*v),
            Value::Uint(v) => Self::Uint(*v),
            Value::Float(v) => Self::Float(*v),
            Value::String(s) => Self::String(s),
            Value::Binary(b) => Self::Binary(b),
        }
    }
}

impl From<i32> for Value<'_> {
    #[inline]
    fn from(value: i32) -> Self {
        Value::Int(value as _)
    }
}

impl From<u32> for Value<'_> {
    #[inline]
    fn from(value: u32) -> Self {
        Value::Uint(value as _)
    }
}

impl From<i64> for Value<'_> {
    #[inline]
    fn from(value: i64) -> Self {
        Value::Int(value)
    }
}

impl From<u64> for Value<'_> {
    #[inline]
    fn from(value: u64) -> Self {
        Value::Uint(value)
    }
}

impl From<f64> for Value<'_> {
    #[inline]
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<bool> for Value<'_> {
    #[inline]
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl<'a> From<Cow<'a, str>> for Value<'a> {
    #[inline]
    fn from(value: Cow<'a, str>) -> Self {
        Value::String(value)
    }
}

impl<'a> From<&'a str> for Value<'a> {
    #[inline]
    fn from(value: &'a str) -> Self {
        Value::String(Cow::Borrowed(value))
    }
}

impl From<String> for Value<'_> {
    #[inline]
    fn from(value: String) -> Self {
        Value::String(Cow::Owned(value))
    }
}

impl<'a> From<&'a Value<'_>> for Value<'a> {
    #[inline]
    fn from(value: &'a Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(v) => Self::Bool(*v),
            Value::Int(v) => Self::Int(*v),
            Value::Uint(v) => Self::Uint(*v),
            Value::Float(v) => Self::Float(*v),
            Value::String(s) => Self::String(Cow::Borrowed(s)),
            Value::Binary(b) => Self::Binary(Cow::Borrowed(b)),
        }
    }
}

pub trait IntoEntry<'a> {
    fn into_entry(self) -> Entry<'a>;
}

impl<'a, F: FnOnce(&mut Node<'a>)> IntoEntry<'a> for F {
    fn into_entry(self) -> Entry<'a> {
        let mut child = Node::empty();
        self(&mut child);
        Entry::Child(child)
    }
}

impl<'a> IntoEntry<'a> for (&'a str, Value<'a>) {
    fn into_entry(self) -> Entry<'a> {
        let (name, value) = self;
        Entry::Arg(Arg {
            name: if name.is_empty() {
                None
            } else {
                Some(Cow::Borrowed(name))
            },
            value,
        })
    }
}

impl<'a, T: IntoEntry<'a>> IntoEntry<'a> for (&'a str, Vec<T>) {
    fn into_entry(self) -> Entry<'a> {
        let (name, values) = self;
        let mut node = Node::empty();
        for value in values {
            node.push(value.into_entry());
        }
        node.set_name(name);
        Entry::Child(node)
    }
}

impl<'a> IntoEntry<'a> for Node<'a> {
    fn into_entry(self) -> Entry<'a> {
        Entry::Child(self)
    }
}

impl<'a> IntoEntry<'a> for Arg<'a> {
    #[inline]
    fn into_entry(self) -> Entry<'a> {
        Entry::Arg(self)
    }
}

impl<'a> IntoEntry<'a> for Value<'a> {
    #[inline]
    fn into_entry(self) -> Entry<'a> {
        Entry::Arg(self.into())
    }
}

impl<'a> IntoEntry<'a> for Entry<'a> {
    #[inline]
    fn into_entry(self) -> Entry<'a> {
        self
    }
}

impl<'a> IntoEntry<'a> for i32 {
    #[inline]
    fn into_entry(self) -> Entry<'a> {
        Entry::from(Value::Int(self as _))
    }
}

impl<'a> IntoEntry<'a> for u32 {
    #[inline]
    fn into_entry(self) -> Entry<'a> {
        Entry::from(Value::Uint(self as _))
    }
}

impl<'a> IntoEntry<'a> for i64 {
    #[inline]
    fn into_entry(self) -> Entry<'a> {
        Entry::from(Value::Int(self))
    }
}

impl<'a> IntoEntry<'a> for u64 {
    #[inline]
    fn into_entry(self) -> Entry<'a> {
        Entry::from(Value::Uint(self))
    }
}

impl<'a> IntoEntry<'a> for f64 {
    #[inline]
    fn into_entry(self) -> Entry<'a> {
        Entry::from(Value::Float(self))
    }
}

impl<'a> IntoEntry<'a> for bool {
    #[inline]
    fn into_entry(self) -> Entry<'a> {
        Entry::from(Value::Bool(self))
    }
}

impl<'a> IntoEntry<'a> for &'a str {
    #[inline]
    fn into_entry(self) -> Entry<'a> {
        Entry::from(Value::String(Cow::Borrowed(self)))
    }
}

impl<'a> IntoEntry<'a> for String {
    #[inline]
    fn into_entry(self) -> Entry<'a> {
        Entry::from(Value::String(Cow::Owned(self)))
    }
}

#[derive(Default, Clone)]
struct Strings {
    buffer: String,
    interned: HashMap<String, StringRange>,
    limit: usize,
}

impl Strings {
    #[inline]
    fn clear(&mut self) {
        self.buffer.clear();
        self.interned.clear();
    }

    #[inline]
    fn add_string(&mut self, s: &str) -> StringRange {
        if s.is_empty() {
            return StringRange::EMPTY;
        }
        if s.len() <= self.limit {
            return self.add_string_intern(s);
        }

        let start = self.buffer.len() as u32;
        let len = s.len() as u32;
        self.buffer.push_str(s);
        StringRange { start, len }
    }

    #[inline]
    fn add_string_intern(&mut self, s: &str) -> StringRange {
        if s.is_empty() {
            return StringRange::EMPTY;
        }

        match self.interned.entry_ref(s) {
            hash_map::EntryRef::Occupied(entry) => *entry.get(),
            hash_map::EntryRef::Vacant(entry) => {
                let start = self.buffer.len() as u32;
                let len = s.len() as u32;
                self.buffer.push_str(s);
                *entry.insert(StringRange { start, len })
            }
        }
    }
}

impl<'a, 'c: 'a> access::NodeRef<'a> for &'a Node<'c> {
    type ChildrenIter<'b>
        = core::slice::Iter<'b, Node<'c>>
    where
        Self: 'b,
        'c: 'b;

    type ArgsIter<'b>
        = core::slice::Iter<'b, Arg<'c>>
    where
        Self: 'b,
        'c: 'b;

    #[inline]
    fn name(&self) -> &'a str {
        &self.name
    }

    #[inline]
    fn ty(&self) -> &'a str {
        &self.ty
    }

    #[inline]
    fn children(&self) -> Self::ChildrenIter<'a> {
        self.children.iter()
    }

    #[inline]
    fn args(&self) -> Self::ArgsIter<'a> {
        self.args.iter()
    }
}

impl<'a> access::ArgRef<'a> for &'a Arg<'_> {
    #[inline]
    fn name(&self) -> &'a str {
        self.name.as_deref().unwrap_or_default()
    }

    #[inline]
    fn value(&self) -> ValueRef<'a> {
        (&self.value).into()
    }
}

impl<'a, 'b> From<&'a Entry<'b>> for access::EntryRef<&'a Arg<'b>, &'a Node<'b>> {
    #[inline]
    fn from(value: &'a Entry<'b>) -> Self {
        match value {
            Entry::Arg(arg) => access::EntryRef::Arg(arg),
            Entry::Child(node) => access::EntryRef::Child(node),
        }
    }
}
