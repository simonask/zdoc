use core::mem;

use alloc::{borrow::Cow, vec::Vec};

use super::{Arg, Entry, IntoEntry, Value};

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
    pub fn from_document(node: crate::Node<'a>) -> Self {
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
    pub fn push_arg(&mut self, arg: impl Into<Arg<'a>>) -> &mut Self {
        self.args_mut().push(arg.into());
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

    /// Push an argument or child node. This method does not maintain ordering
    /// between arguments and children.
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

    #[must_use]
    pub fn with_ty(mut self, ty: impl Into<Cow<'a, str>>) -> Self {
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
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::debug::debug_entry(&crate::access::EntryRef::<&Arg<'a>, _>::Child(self)).fmt(f)
    }
}
