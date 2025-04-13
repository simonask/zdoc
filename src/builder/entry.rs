use alloc::{borrow::Cow, vec::Vec};

use super::{Arg, Node, Value};

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

macro_rules! impl_primitive_into_entry {
    ($($t:ty)*) => {
        $(
            impl<'a> IntoEntry<'a> for $t {
                #[inline]
                fn into_entry(self) -> Entry<'a> {
                    Entry::Arg(Arg::from(self))
                }
            }
        )*
    };
}

impl_primitive_into_entry!(
    bool
    i8 i16 i32 i64
    u8 u16 u32 u64
    f32 f64
    &'a str
    alloc::string::String
    Cow<'a, str>
);
