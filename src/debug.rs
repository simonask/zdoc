use crate::{ClassifyNode, access};

pub(crate) fn debug_entry<'a, 'b, Arg: access::ArgRef<'a> + 'b, Child: access::NodeRef<'a> + 'b>(
    entry: &'b access::EntryRef<Arg, Child>,
) -> impl core::fmt::Debug + 'b {
    let printer: &'b DebugEntryWithName<access::EntryRef<Arg, Child>> =
        bytemuck::TransparentWrapper::wrap_ref(entry);
    printer
}

/// Debug an argument collection.
pub(crate) fn debug_args<
    'a,
    'b,
    Arg: access::ArgRef<'a> + 'b,
    I: Iterator<Item = Arg> + Clone + 'b,
>(
    args: I,
) -> impl core::fmt::Debug + 'b {
    let classify = crate::classify::classify_naming(args.clone().map(|arg| arg.name()));
    DebugEntries(
        classify.into(),
        args.map(|arg| access::EntryRef::<Arg, crate::Node<'a>>::Arg(arg)),
    )
}

/// Debug a child collection.
pub(crate) fn debug_children<
    'a,
    'b,
    Child: access::NodeRef<'a> + 'b,
    I: Iterator<Item = Child> + Clone + 'b,
>(
    children: I,
) -> impl core::fmt::Debug + 'b {
    let classify = crate::classify::classify_naming(children.clone().map(|arg| arg.name()));
    DebugEntries(
        classify.into(),
        children.map(|child| access::EntryRef::<crate::Arg<'a>, _>::Child(child)),
    )
}

pub(crate) fn debug_entries<'a, 'b, Arg: access::ArgRef<'a>, Child: access::NodeRef<'a>>(
    entries: impl Iterator<Item = access::EntryRef<Arg, Child>> + Clone + 'b,
) -> impl core::fmt::Debug + 'b {
    let classify = crate::classify::classify_naming(entries.clone().map(|entry| entry.name()));
    DebugEntries(classify.into(), entries)
}

#[derive(bytemuck::TransparentWrapper)]
#[repr(transparent)]
struct DebugEntryWithName<N>(N);
impl<'a, Arg: access::ArgRef<'a>, Child: access::NodeRef<'a>> core::fmt::Debug
    for DebugEntryWithName<access::EntryRef<Arg, Child>>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = self.0.name();
        if !name.is_empty() {
            write!(f, "{name} = ")?;
        }
        let without_name: &DebugEntryWithoutName<_> =
            bytemuck::TransparentWrapper::wrap_ref(&self.0);
        without_name.fmt(f)
    }
}

#[derive(bytemuck::TransparentWrapper)]
#[repr(transparent)]
struct DebugEntryWithoutName<N>(N);
impl<'a, Arg: access::ArgRef<'a>, Child: access::NodeRef<'a>> core::fmt::Debug
    for DebugEntryWithoutName<access::EntryRef<Arg, Child>>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let node = &self.0;
        let ty = node.ty();

        match node {
            access::EntryRef::Arg(arg) => arg.value().fmt(f)?,
            access::EntryRef::Child(child) => {
                let classify = child.classify();

                // Write the type name, choosing the style based on the name type.
                let mut terminator = "";
                if !ty.is_empty() {
                    match classify {
                        // Will be followed by a bracketed expression.
                        ClassifyNode::StructVariant
                        | ClassifyNode::MixedVariant
                        | ClassifyNode::SeqVariant => write!(f, "{ty} ")?,
                        // Write the type as a newtype.
                        ClassifyNode::ValueVariant => {
                            write!(f, "{ty}(")?;
                            terminator = ")";
                        }
                        // Unit enum variant, just write the type.
                        ClassifyNode::UnitVariant => write!(f, "{ty}")?,
                        _ => (),
                    }
                }
                DebugEntries(child.classify(), child.entries()).fmt(f)?;
                f.write_str(terminator)?;
            }
        }

        Ok(())
    }
}

struct DebugEntries<I>(ClassifyNode, I);
impl<
    'a,
    Arg: access::ArgRef<'a>,
    Child: access::NodeRef<'a>,
    I: Iterator<Item = access::EntryRef<Arg, Child>> + Clone,
> core::fmt::Debug for DebugEntries<I>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut iter = self.1.clone();

        match self.0 {
            ClassifyNode::Struct
            | ClassifyNode::StructVariant
            | ClassifyNode::Mixed
            | ClassifyNode::MixedVariant => {
                let mut debug_map = f.debug_map();
                for entry in iter {
                    debug_map.entry(&entry.name(), &DebugEntryWithoutName(entry));
                }
                debug_map.finish()
            }
            ClassifyNode::Seq | ClassifyNode::SeqVariant => {
                let mut debug_list = f.debug_list();
                for entry in iter {
                    debug_list.entry(&DebugEntryWithoutName(entry));
                }
                debug_list.finish()
            }
            ClassifyNode::Value | ClassifyNode::ValueVariant => match iter.next() {
                Some(entry) => DebugEntryWithoutName(entry).fmt(f),
                None => f.write_str("()"),
            },
            ClassifyNode::Unit => f.write_str("()"),
            ClassifyNode::UnitVariant => {
                // Wrote the type name.
                Ok(())
            }
        }
    }
}
