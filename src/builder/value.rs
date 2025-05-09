use alloc::{borrow::Cow, string::String};

use crate::ValueRef;

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

    #[inline]
    #[must_use]
    pub fn into_static(self) -> Value<'static> {
        match self {
            Value::Null => Value::Null,
            Value::Bool(v) => Value::Bool(v),
            Value::Int(v) => Value::Int(v),
            Value::Uint(v) => Value::Uint(v),
            Value::Float(v) => Value::Float(v),
            Value::String(s) => Value::String(Cow::Owned(s.into_owned())),
            Value::Binary(b) => Value::Binary(Cow::Owned(b.into_owned())),
        }
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

macro_rules! signed {
    ($($t:ty)*) => {
        $(
            impl From<$t> for Value<'_> {
                #[inline]
                fn from(value: $t) -> Self {
                    Value::Int(value as _)
                }
            }
        )*
    };
}

macro_rules! unsigned {
    ($($t:ty)*) => {
        $(
            impl From<$t> for Value<'_> {
                #[inline]
                fn from(value: $t) -> Self {
                    Value::Uint(value as _)
                }
            }
        )*
    };
}

macro_rules! float {
    ($($t:ty)*) => {
        $(
            impl From<$t> for Value<'_> {
                #[inline]
                fn from(value: $t) -> Self {
                    Value::Float(value as _)
                }
            }
        )*
    };
}

signed!(i8 i16 i32 i64);
unsigned!(u8 u16 u32 u64);
float!(f32 f64);

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
