use crate::access;

/// Inferred classification of a node.
///
/// This is used during deserialization (in serde, to support
/// `deserialize_any()`) to guess how a node should be deserialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClassifyNode {
    /// Key-value pairs, where all keys are strings.
    ///
    /// Conditions: All children and arguments are named, and the node does not
    /// have a type.
    ///
    /// Serialized structs fall in this category.
    Struct,
    /// Key-value pairs, where all keys are strings and the node has a type.
    ///
    /// Conditions: All children and arguments are named, and the node has a
    /// type.
    StructVariant,
    /// Sequence of arguments or children.
    ///
    /// Conditions: The node has at least 2 arguments and children, all children
    /// and arguments are unnamed, and the node does not have a type.
    Seq,
    /// Sequence of arguments or children with a type.
    ///
    /// Conditions: The node has at least 2 arguments and children, all children
    /// and arguments are unnamed, and the node has a type.
    SeqVariant,
    /// Single unnamed entry (argument xor child).
    ///
    /// Conditions: The argument is unnamed, and the node does not have a type.
    Value,
    /// Single unnamed entry with a type. This is used during
    /// serialization/deserialization to represent newtype enum variants.
    ///
    /// Conditions: The argument is unnamed, and the node has a type.
    ValueVariant,
    /// Empty node without a type.
    Unit,
    /// Empty node with a type.
    UnitVariant,
    /// The node has a mixed bag of named and unnamed children and arguments.
    Mixed,
    /// The node has a mixed bag of named and unnamed children and arguments,
    /// and it has a type.
    MixedVariant,
}

impl ClassifyNode {
    /// True if the node can be viewed as a list of unnamed items.
    ///
    /// This returns `true` for empty nodes and single-value nodes.
    #[inline]
    #[must_use]
    pub fn is_list_like(&self) -> bool {
        matches!(
            self,
            ClassifyNode::Seq
                | ClassifyNode::SeqVariant
                | ClassifyNode::Value
                | ClassifyNode::ValueVariant
                | ClassifyNode::Unit
                | ClassifyNode::UnitVariant
                | ClassifyNode::Mixed
                | ClassifyNode::MixedVariant
        )
    }

    /// True if the node can be viewed as a map of named items.
    ///
    /// This returns `true` for empty nodes.
    #[inline]
    #[must_use]
    pub fn is_dictionary_like(&self) -> bool {
        matches!(
            self,
            ClassifyNode::Struct
                | ClassifyNode::StructVariant
                | ClassifyNode::Unit
                | ClassifyNode::UnitVariant
                | ClassifyNode::Mixed
                | ClassifyNode::MixedVariant
        )
    }
}

pub(crate) fn classify_node<'a>(node: &(impl access::NodeRef<'a> + 'a)) -> ClassifyNode {
    use access::{ArgRef as _, NodeRef as _};

    let has_type = !node.ty().is_empty();
    let child_naming = classify_naming(node.children().map(|child| child.name()));
    let arg_naming = classify_naming(node.args().map(|arg| arg.name()));
    let naming = match (child_naming, arg_naming) {
        (ClassifyNaming::Empty, ClassifyNaming::Empty) => ClassifyNaming::Empty,
        (
            ClassifyNaming::AllNamed | ClassifyNaming::Empty,
            ClassifyNaming::AllNamed | ClassifyNaming::Empty,
        ) => ClassifyNaming::AllNamed,
        (
            ClassifyNaming::AllUnnamed | ClassifyNaming::Empty,
            ClassifyNaming::AllUnnamed | ClassifyNaming::Empty,
        ) => ClassifyNaming::AllUnnamed,
        _ => ClassifyNaming::Mixed,
    };

    match naming {
        ClassifyNaming::Empty => {
            if has_type {
                return ClassifyNode::UnitVariant;
            }
            ClassifyNode::Unit
        }
        ClassifyNaming::AllNamed => {
            if has_type {
                return ClassifyNode::StructVariant;
            }
            ClassifyNode::Struct
        }
        ClassifyNaming::AllUnnamed => {
            let total_len = node.children().len() + node.args().len();
            if total_len == 0 {
                unreachable!() // caught by classify_naming()
            } else if total_len == 1 {
                if has_type {
                    return ClassifyNode::ValueVariant;
                }
                return ClassifyNode::Value;
            } else if has_type {
                return ClassifyNode::SeqVariant;
            }
            ClassifyNode::Seq
        }
        ClassifyNaming::Mixed => {
            if has_type {
                return ClassifyNode::MixedVariant;
            }
            ClassifyNode::Mixed
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClassifyNaming {
    Empty,
    AllNamed,
    AllUnnamed,
    Mixed,
}

pub(crate) fn classify_naming<'a>(names: impl Iterator<Item = &'a str>) -> ClassifyNaming {
    let (named_count, unnamed_count) = names.fold((0, 0), |(named_count, unnamed_count), name| {
        if name.is_empty() {
            (named_count, unnamed_count + 1)
        } else {
            (named_count + 1, unnamed_count)
        }
    });

    if named_count == 0 && unnamed_count == 0 {
        ClassifyNaming::Empty
    } else if named_count == 0 {
        ClassifyNaming::AllUnnamed
    } else if unnamed_count == 0 {
        ClassifyNaming::AllNamed
    } else {
        ClassifyNaming::Mixed
    }
}

impl From<ClassifyNaming> for ClassifyNode {
    #[inline]
    fn from(value: ClassifyNaming) -> Self {
        match value {
            ClassifyNaming::Empty => ClassifyNode::Unit,
            ClassifyNaming::AllNamed => ClassifyNode::Struct,
            ClassifyNaming::AllUnnamed => ClassifyNode::Seq,
            ClassifyNaming::Mixed => ClassifyNode::Mixed,
        }
    }
}
