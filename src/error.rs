#[derive(Debug, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("the target format cannot represent this value: {0}")]
    UnrepresentableInt(i64),
    #[error("the target format cannot represent this value: {0}")]
    UnrepresentableUint(u64),
    #[error("the target format cannot represent this value: {0}")]
    UnrepresentableFloat(f64),
    #[error("the target format cannot represent binary data")]
    UnrepresentableBinary,
    #[error("the string is not valid UTF-8")]
    UnrepresentableString,
    #[error("special field in the target format was clobbered by a child or argument of a node")]
    ClobberedField,
    #[cfg(feature = "alloc")]
    #[error("{0}")]
    Custom(alloc::string::String),
}

impl Error {
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn custom<E>(error: E) -> Self
    where
        E: core::error::Error + Send + Sync + 'static,
    {
        use alloc::string::ToString as _;
        Self::Custom(error.to_string())
    }

    #[cfg(feature = "alloc")]
    pub fn msg<T: core::fmt::Display>(msg: T) -> Self {
        use alloc::string::ToString as _;
        Self::Custom(msg.to_string())
    }
}

/// Validation error when checking the integrity of the binary encoding of a
/// document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{error}, offset {offset}")]
pub struct ValidationError {
    /// Byte offset in the file where the error occurred.
    pub offset: usize,
    /// Kind of error that occurred.
    pub error: ValidationErrorKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ValidationErrorKind {
    #[error("header magic bytes are invalid")]
    HeaderMagic,
    #[error("header version field indicates an unsupported version: {0}")]
    HeaderVersion(u32),
    #[error("header size field does not match the actual size of the document")]
    HeaderSize,
    #[error("header nodes offset field is invalid")]
    HeaderNodesOffset,
    #[error("header nodes length field is invalid")]
    HeaderNodesLen,
    #[error("header args offset field is invalid")]
    HeaderArgsOffset,
    #[error("header args length field is invalid")]
    HeaderArgsLen,
    #[error("header strings offset field is invalid")]
    HeaderStringsOffset,
    #[error("header strings length field is invalid")]
    HeaderStringsLen,
    #[error("header binary offset field is invalid")]
    HeaderBinaryOffset,
    #[error("header binary length field is invalid")]
    HeaderBinaryLen,
    #[error("header root node index is out of bounds")]
    HeaderRootNodeOutOfBounds,
    #[error("header reserved fields must be zero")]
    HeaderReservedFieldsMustBeZero,

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

impl ValidationErrorKind {
    #[inline]
    #[must_use]
    pub(crate) fn at_offset(self, offset: impl Offset) -> ValidationError {
        ValidationError {
            offset: offset.offset(),
            error: self,
        }
    }
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

pub(crate) trait Offset {
    fn offset(self) -> usize;
}
impl Offset for usize {
    #[inline]
    fn offset(self) -> usize {
        self
    }
}
impl Offset for u32 {
    #[inline]
    fn offset(self) -> usize {
        self as usize
    }
}
