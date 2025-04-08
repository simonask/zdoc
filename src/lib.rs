#![doc = include_str!("../README.md")]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod builder;
mod document;
mod error;
#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "kdl")]
pub mod kdl;
#[cfg(feature = "rkyv")]
pub mod rkyv;
#[cfg(feature = "serde")]
mod serde_support;
#[cfg(feature = "xml")]
pub mod xml;
#[cfg(feature = "yaml")]
pub mod yaml;

#[cfg(feature = "alloc")]
pub use builder::Builder;
pub use document::*;
pub use error::*;

#[cfg(feature = "serde")]
pub use serde_support::{from_document, to_document, to_document_builder};

mod internal {
    pub enum IndexOrString<'a> {
        Index(usize),
        String(&'a str),
    }

    impl From<usize> for IndexOrString<'_> {
        #[inline]
        fn from(value: usize) -> Self {
            Self::Index(value)
        }
    }

    impl<'a> From<&'a str> for IndexOrString<'a> {
        #[inline]
        fn from(value: &'a str) -> Self {
            Self::String(value)
        }
    }
}
