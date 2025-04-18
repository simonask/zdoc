#![doc = include_str!("../README.md")]
#![no_std]
#![cfg_attr(coverage, feature(coverage_attribute))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(feature = "alloc")]
extern crate alloc;

pub(crate) mod access;
#[cfg(feature = "alloc")]
pub mod builder;
pub(crate) mod classify;
mod compare;
pub(crate) mod debug;
mod document;
mod error;
#[cfg(feature = "facet")]
pub mod facet;
#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "kdl")]
pub mod kdl;
#[cfg(feature = "rkyv")]
pub mod rkyv;
#[cfg(feature = "serde")]
pub mod serde;
#[cfg(feature = "xml")]
pub mod xml;
#[cfg(feature = "yaml")]
pub mod yaml;

#[cfg(feature = "alloc")]
pub use builder::Builder;
pub use classify::ClassifyNode;
pub use document::*;
pub use error::*;

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
