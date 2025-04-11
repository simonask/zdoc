#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("keys in maps must serialize as strings")]
    NonStringMapKey,
    #[error("cannot serialize a compound type as a plain value")]
    CompoundValue,
    #[cfg(feature = "alloc")]
    #[error("{0}")]
    Custom(alloc::string::String),
    #[cfg(not(feature = "alloc"))]
    #[error("custom error")]
    Custom,
}

#[cfg(feature = "alloc")]
impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: core::fmt::Display,
    {
        use alloc::string::ToString as _;
        Error::Custom(msg.to_string())
    }
}

#[cfg(feature = "alloc")]
impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: core::fmt::Display,
    {
        use alloc::string::ToString as _;
        Error::Custom(msg.to_string())
    }
}

#[cfg(not(feature = "alloc"))]
impl serde::de::Error for Error {
    fn custom<T>(_msg: T) -> Self
    where
        T: core::fmt::Display,
    {
        Error::Custom
    }
}
