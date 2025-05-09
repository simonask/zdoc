use crate::ValidationError;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub mod codec;
mod node;
pub mod raw;

pub use node::*;
pub use raw::ValueRef;

/// Immutable document that owns its memory.
#[cfg(feature = "alloc")]
#[derive(Clone, Default)]
pub struct DocumentBuffer {
    // Note: Constructing this is unsafe, and must go through the unsafe
    // `DocumentBuffer::from_raw_unchecked`.
    raw: raw::RawDocumentBuffer,
}

#[cfg(feature = "alloc")]
impl DocumentBuffer {
    /// Create a document from a buffer, checking for validity.
    ///
    /// # Errors
    ///
    /// If the bytes in `buffer` are not a valid document, this returns an
    /// error. Note that an empty buffer is a valid document.
    #[inline]
    pub fn from_buffer(buffer: Vec<u8>) -> Result<Self, ValidationError> {
        let raw = raw::RawDocumentBuffer::from_buffer(buffer);
        raw.check().map(|()| unsafe {
            // SAFETY: Safety checks passed.
            Self::from_raw_unchecked(raw)
        })
    }

    /// Create a document from a buffer without checking for validity.
    ///
    /// `buffer` does not have to be well-aligned.
    ///
    /// # Safety
    ///
    /// The buffer must contain a valid document.
    #[must_use]
    #[inline]
    pub unsafe fn from_buffer_unchecked(buffer: Vec<u8>) -> Self {
        unsafe {
            // SAFETY: Invariants of this function.
            Self::from_raw_unchecked(raw::RawDocumentBuffer::from_buffer(buffer))
        }
    }

    /// Create a document from a [`RawDocumentBuffer`], checking the document
    /// for validity.
    ///
    /// # Errors
    ///
    /// If the document is invalid, this returns an error.
    #[inline]
    pub fn from_raw(raw: raw::RawDocumentBuffer) -> Result<Self, ValidationError> {
        raw.check()?;
        unsafe {
            // SAFETY: Safety checks passed.
            Ok(Self::from_raw_unchecked(raw))
        }
    }

    /// Create a document from a [`RawDocumentBuffer`] without checking for
    /// validity.
    ///
    /// # Safety
    ///
    /// The document must be valid.
    #[must_use]
    #[inline]
    pub unsafe fn from_raw_unchecked(raw: raw::RawDocumentBuffer) -> Self {
        Self { raw }
    }

    #[inline]
    #[must_use]
    pub fn as_document(&self) -> &Document {
        unsafe {
            // SAFETY: Invariants of Self.
            Document::from_raw_unchecked(&self.raw)
        }
    }
}

#[cfg(feature = "alloc")]
impl core::fmt::Debug for DocumentBuffer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_document().fmt(f)
    }
}

#[cfg(feature = "alloc")]
impl core::ops::Deref for DocumentBuffer {
    type Target = Document;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_document()
    }
}

#[cfg(feature = "alloc")]
impl TryFrom<raw::RawDocumentBuffer> for DocumentBuffer {
    type Error = ValidationError;

    #[inline]
    fn try_from(value: raw::RawDocumentBuffer) -> Result<Self, Self::Error> {
        Self::from_raw(value)
    }
}

/// Document containing arbitrary structured data.
///
/// This is an unsized reference type that can wrap any contiguous block of
/// bytes.
///
/// When wrapping a slice, the integrity of the document is checked up front.
/// All subsequent access to the contents of the document has zero validation
/// overhead.
#[repr(transparent)]
#[derive(PartialEq, Eq)]
pub struct Document {
    raw: raw::RawDocument,
}

impl Document {
    #[must_use]
    pub const fn empty() -> &'static Document {
        unsafe {
            // SAFETY: The empty document is always valid.
            Self::from_raw_unchecked(raw::RawDocument::empty())
        }
    }

    /// Validate a block of bytes as a document, and wrap the slice.
    ///
    /// Note that `slice` must contain a valid document. The empty slice is a
    /// valid document.
    ///
    /// # Errors
    ///
    /// If the bytes in `slice` are not a valid document, this returns an error.
    #[inline]
    pub fn from_slice(slice: &[u8]) -> Result<&Self, ValidationError> {
        let raw = raw::RawDocument::from_slice(slice);
        Self::try_from_raw(raw)
    }

    /// Unsafely create a document from a block of bytes.
    ///
    /// Note that the empty slice is a valid document.
    ///
    /// # Safety
    ///
    /// The bytes in `slice` must represent a valid document.
    #[inline]
    #[must_use]
    pub unsafe fn from_slice_unchecked(slice: &[u8]) -> &Self {
        unsafe {
            // SAFETY: Invariants of this function.
            Self::from_raw_unchecked(raw::RawDocument::from_slice(slice))
        }
    }

    /// Wrap a raw document, checking it for validity.
    ///
    /// # Errors
    ///
    /// If the document is invalid, this returns an error.
    #[inline]
    pub fn try_from_raw(raw: &raw::RawDocument) -> Result<&Self, ValidationError> {
        raw.check()?;

        unsafe {
            // SAFETY: Safety checks passed.
            Ok(Self::from_raw_unchecked(raw))
        }
    }

    /// Wrap a raw document.
    ///
    /// # Safety
    ///
    /// The document must be valid.
    #[inline]
    #[must_use]
    pub const unsafe fn from_raw_unchecked(raw: &raw::RawDocument) -> &Self {
        unsafe {
            // SAFETY: Invariants of this function.
            &*(core::ptr::from_ref::<raw::RawDocument>(raw) as *const Self)
        }
    }

    #[inline]
    #[must_use]
    pub fn header(&self) -> &codec::Header {
        self.raw.header()
    }

    /// All nodes in the document.
    #[inline]
    #[must_use]
    pub fn nodes(&self) -> &[codec::Node] {
        self.raw.nodes()
    }

    /// All arguments in the document.
    #[inline]
    #[must_use]
    pub fn args(&self) -> &[codec::Arg] {
        self.raw.args()
    }

    /// Get the root node.
    #[inline]
    #[must_use]
    pub fn root(&self) -> Node {
        unsafe {
            // SAFETY: Invariants of Self.
            Node::from_raw(self.raw.root_unchecked())
        }
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.raw.header().nodes_len == 0
    }

    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.raw.as_bytes()
    }

    #[inline]
    #[must_use]
    pub fn get_string(&self, range: codec::StringRange) -> Option<&str> {
        self.raw.get_string(range).ok()
    }
}

impl core::fmt::Debug for Document {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.root().fmt(f)
    }
}

impl<'a> TryFrom<&'a raw::RawDocument> for &'a Document {
    type Error = ValidationError;

    #[inline]
    fn try_from(value: &'a raw::RawDocument) -> Result<Self, Self::Error> {
        Document::try_from_raw(value)
    }
}
