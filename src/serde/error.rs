use alloc::string::{String, ToString as _};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("keys in maps must serialize as strings")]
    NonStringMapKey,
    #[error("cannot serialize a compound type as a plain value")]
    CompoundValue,
    #[error("{0}")]
    Custom(String),
}

impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: core::fmt::Display,
    {
        Error::Custom(msg.to_string())
    }
}

impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: core::fmt::Display,
    {
        Error::Custom(msg.to_string())
    }
}
