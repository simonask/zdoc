use alloc::{borrow::Cow, vec};

use super::{Node, Value};

#[derive(Clone, Default)]
pub struct Arg<'a> {
    pub name: Option<Cow<'a, str>>,
    pub value: Value<'a>,
}

impl<'a> Arg<'a> {
    #[inline]
    pub fn from_document(arg: crate::Arg<'a>) -> Self {
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

macro_rules! impl_from_via_value {
    ($($t:ty)*) => {
        $(
            impl<'a> From<$t> for Arg<'a> {
                #[inline]
                fn from(value: $t) -> Self {
                    Arg {
                        name: None,
                        value: Value::from(value),
                    }
                }
            }
        )*
    };
}

impl_from_via_value!(
    bool
    i8 i16 i32 i64
    u8 u16 u32 u64
    f32 f64
    &'a str
    alloc::string::String
    Cow<'a, str>
);

impl<'a> core::fmt::Debug for Arg<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::debug::debug_entry(&crate::access::EntryRef::<_, &Node<'a>>::Arg(self)).fmt(f)
    }
}
