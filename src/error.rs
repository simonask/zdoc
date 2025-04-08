#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("invalid magic bytes in document")]
    InvalidMagic,
    #[error("invalid document version")]
    InvalidVersion(u32),
    #[error("document is too small")]
    InvalidSize,
    #[error("invalid header")]
    InvalidHeader,
    #[error("string section contains invalid UTF-8")]
    InvalidUtf8,
    #[error(transparent)]
    Corrupt(#[from] CorruptError),
    #[error("the target format cannot represent this value: {0}")]
    UnrepresentableInt(i64),
    #[error("the target format cannot represent this value: {0}")]
    UnrepresentableUint(u64),
    #[error("the target format cannot represent this value: {0}")]
    UnrepresentableFloat(f64),
    #[error("the target format cannot represent binary data")]
    UnrepresentableBinary,
    #[error("special field in the target format was clobbered by a child or argument of a node")]
    ClobberedField,
    #[cfg(feature = "alloc")]
    #[error(transparent)]
    Custom(alloc::boxed::Box<dyn core::error::Error + Send + Sync>),
}

impl Error {
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn custom<E>(error: E) -> Self
    where
        E: core::error::Error + Send + Sync + 'static,
    {
        Self::Custom(alloc::boxed::Box::new(error))
    }

    #[cfg(feature = "alloc")]
    pub fn msg<T: core::fmt::Display>(msg: T) -> Self {
        use alloc::string::ToString as _;
        #[derive(Debug)]
        struct Message(alloc::string::String);
        impl core::error::Error for Message {}
        impl core::fmt::Display for Message {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(&self.0)
            }
        }
        Self::Custom(alloc::boxed::Box::new(Message(msg.to_string())))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{error}, offset {offset}")]
pub struct CorruptError {
    /// Byte offset in the file where the error occurred.
    pub offset: u32,
    /// Kind of error that occurred.
    pub error: CorruptErrorKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CorruptErrorKind {
    #[error("range length overflow")]
    LengthOverflow,
    #[error("children node range out of bounds")]
    ChildrenOutOfBounds,
    #[error("node argument range out of bounds")]
    ArgumentsOutOfBounds,
    #[error("string out of bounds")]
    StringOutOfBounds,
    #[error("binary out of bounds")]
    BinaryOutOfBounds,
    #[error("string blob contains invalid UTF-8")]
    InvalidUtf8,
    #[error("invalid argument type")]
    InvalidArgumentType,
    #[error(
        "children of node come before the node; all children of a node must come after the node itself"
    )]
    ChildrenBeforeParent,
}

impl CorruptErrorKind {
    #[inline]
    #[must_use]
    pub(crate) fn with_offset(self, offset: u32) -> CorruptError {
        CorruptError {
            offset,
            error: self,
        }
    }
}

pub type Result<T, E = Error> = core::result::Result<T, E>;
