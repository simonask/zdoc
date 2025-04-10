use core::iter::FusedIterator;

use crate::{access, internal};

use super::{ValueRef, codec, raw};

/// Node in a [`Document`](crate::Document).
#[derive(Clone, Copy)]
pub struct Node<'a> {
    /// SAFETY INVARIANT: Must come from a valid (internally consistent)
    /// document.
    raw: raw::RawNodeRef<'a>,
}

impl<'a> Node<'a> {
    /// Wrap a [`RawNodeRef`](raw::RawNodeRef).
    ///
    /// # Safety
    ///
    /// The node must come from a valid document.
    #[inline]
    #[must_use]
    pub unsafe fn from_raw(raw: raw::RawNodeRef<'a>) -> Self {
        Self { raw }
    }

    #[inline]
    #[must_use]
    pub fn encoded(&self) -> &codec::Node {
        self.raw.encoded()
    }

    /// The position of the node in the document's block of nodes.
    ///
    /// The root node is usually at index 0.
    #[inline]
    #[must_use]
    pub fn raw_index(&self) -> usize {
        unsafe {
            // SAFETY: This node is valid.
            self.raw.raw_index()
        }
    }

    /// If the node has a name, return the name of the node.
    ///
    /// Nodes with unnamed children may be considered "array-like", and named
    /// children may be considered "map-like" or "object-like".
    ///
    /// Having multiple children with the same name is allowed, and mixing named
    /// and unnamed children is allowed.
    ///
    /// Note that all formats do not support mixing named and unnamed children,
    /// and that some formats do not support duplicate children with the same
    /// name.
    #[inline]
    #[must_use]
    pub fn name(&self) -> Option<&'a str> {
        unsafe {
            let name = self.raw.name_unchecked();
            if name.is_empty() { None } else { Some(name) }
        }
    }

    /// If the node has a "type" (an identifier string), return the identifier.
    ///
    /// Note that zdocument does not perform any validation of identifiers, and
    /// it may be an arbitrary string.
    #[inline]
    #[must_use]
    pub fn ty(&self) -> Option<&'a str> {
        unsafe {
            let ty = self.raw.ty_unchecked();
            if ty.is_empty() { None } else { Some(ty) }
        }
    }

    /// Get the children of this node, if any.
    #[inline]
    #[must_use]
    pub fn children(&self) -> Children<'a> {
        Children {
            raw: self.raw.children(),
        }
    }

    /// Get the arguments and children of this node, if any.
    #[inline]
    #[must_use]
    pub fn entries(&self) -> Entries<'a> {
        Entries {
            args: self.args(),
            children: self.children(),
        }
    }

    /// Get the key-value arguments (key optional) of this node.
    #[inline]
    #[must_use]
    pub fn args(&self) -> Args<'a> {
        Args {
            raw: self.raw.args(),
        }
    }

    #[inline]
    #[must_use]
    pub fn classify(&self) -> crate::ClassifyNode {
        crate::classify::classify_node(self)
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.args().is_empty() && self.children().is_empty()
    }

    #[inline]
    #[must_use]
    pub fn is_dictionary_like(&self) -> bool {
        self.classify().is_dictionary_like()
    }

    #[inline]
    #[must_use]
    pub fn is_list_like(&self) -> bool {
        self.classify().is_list_like()
    }

    #[inline]
    #[must_use]
    pub fn is_mixed(&self) -> bool {
        self.classify() == crate::ClassifyNode::Mixed
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<Entry<'a>> {
        self.args()
            .get_by_name(key)
            .map(Entry::Arg)
            .or_else(|| self.children().get_by_name(key).map(Entry::Child))
    }

    /// Get the first argument of this node.
    ///
    /// This is mainly useful for key-value like entries (nodes) in a
    /// dictionary.
    #[must_use]
    pub fn value(&self) -> Option<ValueRef<'a>> {
        self.args().into_iter().next().map(|arg| arg.value)
    }
}

/// Argument or child of a [`Node`].
#[derive(Clone, Copy)]
pub enum Entry<'a> {
    Arg(Arg<'a>),
    Child(Node<'a>),
}

impl<'a> Entry<'a> {
    #[inline]
    #[must_use]
    pub fn name(&self) -> Option<&'a str> {
        match self {
            Entry::Arg(arg) => arg.name,
            Entry::Child(node) => node.name(),
        }
    }

    #[inline]
    #[must_use]
    pub fn ty(&self) -> Option<&'a str> {
        match self {
            Entry::Arg(_) => None,
            Entry::Child(node) => node.ty(),
        }
    }

    #[inline]
    #[must_use]
    pub fn value(&self) -> Option<ValueRef<'a>> {
        match self {
            Entry::Arg(arg) => Some(arg.value),
            Entry::Child(node) => node.value(),
        }
    }
}

impl<'a> From<ValueRef<'a>> for Entry<'a> {
    #[inline]
    fn from(value: ValueRef<'a>) -> Self {
        Arg::from(value).into()
    }
}

impl<'a> From<Arg<'a>> for Entry<'a> {
    #[inline]
    fn from(arg: Arg<'a>) -> Self {
        Entry::Arg(arg)
    }
}

impl<'a> From<Node<'a>> for Entry<'a> {
    #[inline]
    fn from(node: Node<'a>) -> Self {
        Entry::Child(node)
    }
}

impl core::fmt::Debug for Entry<'_> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Entry::Arg(arg) => arg.fmt(f),
            Entry::Child(node) => node.fmt(f),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Entries<'a> {
    children: Children<'a>,
    args: Args<'a>,
}

impl<'a> IntoIterator for Entries<'a> {
    type Item = Entry<'a>;
    type IntoIter = EntriesIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        EntriesIter {
            args: self.args.into_iter(),
            children: self.children.into_iter(),
        }
    }
}

#[derive(Clone)]
pub struct EntriesIter<'a> {
    // Note: `ArgsIter` is a `FusedIterator`.
    args: ArgsIter<'a>,
    children: ChildrenIter<'a>,
}

impl<'a> Iterator for EntriesIter<'a> {
    type Item = Entry<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.args
            .next()
            .map(Entry::Arg)
            .or_else(|| self.children.next().map(Entry::Child))
    }
}

impl DoubleEndedIterator for EntriesIter<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.children
            .next_back()
            .map(Entry::Child)
            .or_else(|| self.args.next_back().map(Entry::Arg))
    }
}

impl ExactSizeIterator for EntriesIter<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.args.len() + self.children.len()
    }
}

impl FusedIterator for EntriesIter<'_> {}

/// Children of a [`Node`].
#[derive(Clone, Copy)]
pub struct Children<'a> {
    raw: raw::RawNodeChildren<'a>,
}

impl<'a> Children<'a> {
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.raw.len()
    }

    #[must_use]
    pub fn get<'b>(&self, key: impl Into<internal::IndexOrString<'b>>) -> Option<Node<'a>> {
        match key.into() {
            internal::IndexOrString::Index(index) => self.get_by_index(index),
            internal::IndexOrString::String(name) => self.get_by_name(name),
        }
    }

    #[inline]
    #[must_use]
    fn get_by_index(&self, index: usize) -> Option<Node<'a>> {
        if index < self.len() {
            unsafe {
                // SAFETY: Checked bounds.
                Some(Node::from_raw(self.raw.get_unchecked(index)))
            }
        } else {
            None
        }
    }

    /// Get a child by name.
    ///
    /// If multiple children have the same name, this returns the *last* child
    /// with that name.
    #[inline]
    fn get_by_name(&self, name: &str) -> Option<Node<'a>> {
        self.into_iter().rfind(|child| child.name() == Some(name))
    }
}

impl<'a> IntoIterator for Children<'a> {
    type Item = Node<'a>;
    type IntoIter = ChildrenIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let len = self.raw.len();
        ChildrenIter {
            raw: self.raw,
            range: 0..len,
        }
    }
}

#[derive(Clone)]
pub struct ChildrenIter<'a> {
    raw: raw::RawNodeChildren<'a>,
    range: core::ops::Range<usize>,
}

impl<'a> Iterator for ChildrenIter<'a> {
    type Item = Node<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(index) = self.range.next() {
            unsafe { Some(Node::from_raw(self.raw.get_unchecked(index))) }
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl DoubleEndedIterator for ChildrenIter<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(index) = self.range.next_back() {
            unsafe { Some(Node::from_raw(self.raw.get_unchecked(index))) }
        } else {
            None
        }
    }
}

impl FusedIterator for ChildrenIter<'_> {}

impl ExactSizeIterator for ChildrenIter<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.range.len()
    }
}

/// Value arguments of a [`Node`].
#[derive(Clone, Copy)]
pub struct Args<'a> {
    raw: raw::RawNodeArgs<'a>,
}

impl<'a> Args<'a> {
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.raw.len()
    }

    /// Get an argument at index.
    ///
    /// # Safety
    ///
    /// The index must be in bounds.
    #[inline]
    #[must_use]
    pub unsafe fn get_unchecked(&self, index: usize) -> Arg<'a> {
        debug_assert!(
            index < self.len(),
            "Index out of bounds: {index} >= {}",
            self.len()
        );
        unsafe {
            // SAFETY: Checked bounds.
            Arg::from_raw(self.raw.get_unchecked(index))
        }
    }

    #[inline]
    #[must_use]
    pub fn get<'b>(&self, index: impl Into<internal::IndexOrString<'b>>) -> Option<Arg<'a>> {
        match index.into() {
            internal::IndexOrString::Index(index) => self.get_by_index(index),
            internal::IndexOrString::String(name) => self.get_by_name(name),
        }
    }

    #[inline]
    #[must_use]
    pub fn get_by_index(&self, index: usize) -> Option<Arg<'a>> {
        if index < self.len() {
            unsafe {
                // SAFETY: Checked bounds.
                Some(self.get_unchecked(index))
            }
        } else {
            None
        }
    }

    #[inline]
    #[must_use]
    pub fn get_by_name(&self, name: &str) -> Option<Arg<'a>> {
        self.into_iter().rfind(|arg| arg.name == Some(name))
    }
}

impl<'a> IntoIterator for Args<'a> {
    type Item = Arg<'a>;
    type IntoIter = ArgsIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let len = self.raw.len();
        ArgsIter {
            raw: self.raw,
            range: 0..len,
        }
    }
}

#[derive(Clone)]
pub struct ArgsIter<'a> {
    raw: raw::RawNodeArgs<'a>,
    range: core::ops::Range<usize>,
}

impl<'a> Iterator for ArgsIter<'a> {
    type Item = Arg<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(index) = self.range.next() {
            unsafe { Some(Arg::from_raw(self.raw.get_unchecked(index))) }
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl DoubleEndedIterator for ArgsIter<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(index) = self.range.next_back() {
            unsafe { Some(Arg::from_raw(self.raw.get_unchecked(index))) }
        } else {
            None
        }
    }
}

impl FusedIterator for ArgsIter<'_> {}

impl ExactSizeIterator for ArgsIter<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.range.len()
    }
}

/// Value argument of a [`Node`].
#[derive(Clone, Copy, PartialEq)]
pub struct Arg<'a> {
    pub name: Option<&'a str>,
    pub value: ValueRef<'a>,
}

impl<'a> Arg<'a> {
    /// Wrap a [`RawArgRef`](raw::RawArgRef).
    ///
    /// # Safety
    ///
    /// The argument must come from a valid document.
    #[inline]
    #[must_use]
    pub unsafe fn from_raw(arg: raw::RawArgRef<'a>) -> Self {
        unsafe {
            let name = arg.name_unchecked();
            let name = if name.is_empty() { None } else { Some(name) };
            let value = arg.get_unchecked();
            Self { name, value }
        }
    }
}

impl<'a> From<ValueRef<'a>> for Arg<'a> {
    #[inline]
    fn from(value: ValueRef<'a>) -> Self {
        Arg { name: None, value }
    }
}

impl core::fmt::Debug for Node<'_> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::debug::debug_entry(&access::EntryRef::<Arg, _>::Child(*self)).fmt(f)
    }
}

impl core::fmt::Debug for Arg<'_> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::debug::debug_entry(&access::EntryRef::<_, Node>::Arg(*self)).fmt(f)
    }
}

impl core::fmt::Debug for Children<'_> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::debug::debug_children(self.into_iter()).fmt(f)
    }
}

impl core::fmt::Debug for Args<'_> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::debug::debug_args(self.into_iter()).fmt(f)
    }
}

impl core::fmt::Debug for Entries<'_> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::debug::debug_entries(self.into_iter().map(Into::into)).fmt(f)
    }
}

impl<'a> access::NodeRef<'a> for Node<'a> {
    type ChildrenIter<'b>
        = ChildrenIter<'b>
    where
        Self: 'b;

    type ArgsIter<'b>
        = ArgsIter<'b>
    where
        Self: 'b;

    #[inline]
    fn name(&self) -> &'a str {
        Node::name(self).unwrap_or("")
    }

    #[inline]
    fn ty(&self) -> &'a str {
        Node::ty(self).unwrap_or("")
    }

    #[inline]
    fn children(&self) -> Self::ChildrenIter<'a> {
        Node::children(self).into_iter()
    }

    #[inline]
    fn args(&self) -> Self::ArgsIter<'a> {
        Node::args(self).into_iter()
    }
}

impl<'a> access::ArgRef<'a> for Arg<'a> {
    #[inline]
    fn name(&self) -> &'a str {
        self.name.unwrap_or("")
    }

    #[inline]
    fn value(&self) -> ValueRef<'a> {
        self.value
    }
}

impl<'a> From<Entry<'a>> for access::EntryRef<Arg<'a>, Node<'a>> {
    #[inline]
    fn from(value: Entry<'a>) -> Self {
        match value {
            Entry::Arg(arg) => access::EntryRef::Arg(arg),
            Entry::Child(node) => access::EntryRef::Child(node),
        }
    }
}
