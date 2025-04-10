//! Private interface to generalize implementations of various things between
//! `Document` nodes and `Builder` nodes, which have a different representation,
//! but are semantically the same.

use core::iter::FusedIterator;

use crate::{ClassifyNode, ValueRef};

#[allow(unused)]
pub trait NodeRef<'a>: Sized + 'a {
    type ChildrenIter<'b>: ExactSizeIterator<Item: NodeRef<'b>> + Clone + 'b
    where
        Self: 'b;
    type ArgsIter<'b>: ExactSizeIterator<Item: ArgRef<'b>> + Clone + 'b
    where
        Self: 'b;

    fn name(&self) -> &'a str;
    fn ty(&self) -> &'a str;
    fn children(&self) -> Self::ChildrenIter<'a>;
    fn args(&self) -> Self::ArgsIter<'a>;
    fn entries(&self) -> EntryRefIter<Self::ArgsIter<'a>, Self::ChildrenIter<'a>> {
        EntryRefIter::new(self.args(), self.children())
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.entries().next().is_none()
    }

    #[inline]
    fn classify(&self) -> ClassifyNode {
        crate::classify::classify_node(self)
    }
}

pub trait ArgRef<'a> {
    fn name(&self) -> &'a str;
    fn value(&self) -> ValueRef<'a>;
}

/// Generic entry (argument or child). This can be used to access the contents
/// of a document in a generic way (builder or serialized).
///
/// Also, this can be used to hook into the general `Debug` implementation.
pub enum EntryRef<Arg, Child> {
    Arg(Arg),
    Child(Child),
}

impl<'a, Arg: ArgRef<'a>, Child: NodeRef<'a>> EntryRef<Arg, Child> {
    pub fn name(&self) -> &'a str {
        match self {
            EntryRef::Arg(arg) => arg.name(),
            EntryRef::Child(child) => child.name(),
        }
    }

    pub fn ty(&self) -> &'a str {
        match self {
            EntryRef::Arg(_) => "",
            EntryRef::Child(child) => child.ty(),
        }
    }
}

impl<'a, Arg: ArgRef<'a>, Child: NodeRef<'a>> core::fmt::Debug for EntryRef<Arg, Child> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::debug::debug_entry(self).fmt(f)
    }
}

#[derive(Clone, Copy)]
pub struct EntryRefIter<ArgIter, ChildrenIter> {
    args: ArgIter,
    children: ChildrenIter,
}

impl<ArgIter, ChildrenIter> EntryRefIter<ArgIter, ChildrenIter> {
    pub fn new(args: ArgIter, children: ChildrenIter) -> Self {
        Self { args, children }
    }
}

impl<ArgIter: Iterator, ChildrenIter: Iterator> Iterator for EntryRefIter<ArgIter, ChildrenIter> {
    type Item = EntryRef<ArgIter::Item, ChildrenIter::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        self.args
            .next()
            .map(EntryRef::Arg)
            .or_else(|| self.children.next().map(EntryRef::Child))
    }
}

impl<ArgIter: DoubleEndedIterator, ChildrenIter: DoubleEndedIterator> DoubleEndedIterator
    for EntryRefIter<ArgIter, ChildrenIter>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.children
            .next_back()
            .map(EntryRef::Child)
            .or_else(|| self.args.next_back().map(EntryRef::Arg))
    }
}

impl<ArgIter: ExactSizeIterator, ChildrenIter: ExactSizeIterator> ExactSizeIterator
    for EntryRefIter<ArgIter, ChildrenIter>
{
    fn len(&self) -> usize {
        self.args.len() + self.children.len()
    }
}

impl<ArgIter: FusedIterator, ChildrenIter: FusedIterator> FusedIterator
    for EntryRefIter<ArgIter, ChildrenIter>
{
}
