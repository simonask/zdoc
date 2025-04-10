use crate::access::{self, ArgRef as _};

#[cfg(feature = "alloc")]
use crate::builder;

fn node_partial_eq<'a, 'b, A: access::NodeRef<'a>, B: access::NodeRef<'b>>(
    lhs: &A,
    rhs: &B,
) -> bool {
    let lhs_args = lhs.args();
    let rhs_args = rhs.args();
    let lhs_children = lhs.children();
    let rhs_children = rhs.children();

    // Start by comparing lengths, because it's probably cheapest.
    if lhs_args.len() != rhs_args.len() {
        return false;
    }
    if lhs_children.len() != rhs_children.len() {
        return false;
    }

    if lhs.name() != rhs.name() {
        return false;
    }
    if lhs.ty() != rhs.ty() {
        return false;
    }

    for (lhs_arg, rhs_arg) in lhs_args.zip(rhs_args) {
        if lhs_arg.name() != rhs_arg.name() {
            return false;
        }
        if lhs_arg.value() != rhs_arg.value() {
            return false;
        }
    }

    for (lhs_child, rhs_child) in lhs_children.zip(rhs_children) {
        if !node_partial_eq(&lhs_child, &rhs_child) {
            return false;
        }
    }

    true
}

impl<'a> PartialEq<crate::Node<'a>> for crate::Node<'_> {
    #[inline]
    fn eq(&self, other: &crate::Node<'a>) -> bool {
        node_partial_eq(self, other)
    }
}

impl<'a> PartialEq<&crate::Node<'a>> for crate::Node<'_> {
    #[inline]
    fn eq(&self, other: &&crate::Node<'a>) -> bool {
        node_partial_eq(self, *other)
    }
}

impl<'a> PartialEq<crate::Node<'a>> for &crate::Node<'_> {
    #[inline]
    fn eq(&self, other: &crate::Node<'a>) -> bool {
        node_partial_eq(*self, other)
    }
}

#[cfg(feature = "alloc")]
impl<'a> PartialEq<builder::Node<'a>> for builder::Node<'_> {
    #[inline]
    fn eq(&self, other: &builder::Node<'a>) -> bool {
        node_partial_eq(&self, &other)
    }
}

#[cfg(feature = "alloc")]
impl<'a> PartialEq<&builder::Node<'a>> for builder::Node<'_> {
    #[inline]
    fn eq(&self, other: &&builder::Node<'a>) -> bool {
        node_partial_eq(&self, other)
    }
}

#[cfg(feature = "alloc")]
impl<'a> PartialEq<builder::Node<'a>> for &builder::Node<'_> {
    #[inline]
    fn eq(&self, other: &builder::Node<'a>) -> bool {
        node_partial_eq(self, &other)
    }
}

#[cfg(feature = "alloc")]
impl<'a> PartialEq<crate::Node<'a>> for builder::Node<'_> {
    #[inline]
    fn eq(&self, other: &crate::Node<'a>) -> bool {
        node_partial_eq(&self, other)
    }
}

#[cfg(feature = "alloc")]
impl<'a> PartialEq<crate::Node<'a>> for &builder::Node<'_> {
    #[inline]
    fn eq(&self, other: &crate::Node<'a>) -> bool {
        node_partial_eq(self, other)
    }
}

#[cfg(feature = "alloc")]
impl<'a> PartialEq<builder::Node<'a>> for crate::Node<'_> {
    #[inline]
    fn eq(&self, other: &builder::Node<'a>) -> bool {
        node_partial_eq(self, &other)
    }
}

#[cfg(feature = "alloc")]
impl<'a> PartialEq<&builder::Node<'a>> for crate::Node<'_> {
    #[inline]
    fn eq(&self, other: &&builder::Node<'a>) -> bool {
        node_partial_eq(self, other)
    }
}

#[cfg(feature = "alloc")]
impl<'a> PartialEq<builder::Node<'a>> for &crate::Node<'_> {
    #[inline]
    fn eq(&self, other: &builder::Node<'a>) -> bool {
        node_partial_eq(*self, &other)
    }
}
