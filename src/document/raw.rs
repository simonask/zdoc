#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use bytemuck::{cast_slice, pod_align_to};
use core::mem::offset_of;

use crate::{ValidationError, ValidationErrorKind};

use super::codec;
use codec::Header;

#[cfg(feature = "alloc")]
#[derive(Clone, Default)]
pub struct RawDocumentBuffer {
    /// Buffer including initial padding bytes achieve the correct alignment.
    buffer: Vec<u8>,
    /// If the buffer was not well-aligned, this is the number of bytes that
    /// were inserted at the beginning of `bytes` to produce the correct
    /// alignment for the header.
    ///
    /// On real-world allocators, this will always be zero, because the allocate
    /// with 16-bytes alignment (typically), but the Miri allocator does not,
    /// for instance, and there's no guarantee that all future allocators do.
    adjust_alignment: usize,
}

#[cfg(feature = "alloc")]
impl RawDocumentBuffer {
    /// Create a raw document from a buffer.
    ///
    /// This does *NOT* perform any validity checks, but only makes sure that
    /// the alignment of `buffer` is correct.
    #[inline]
    #[must_use]
    pub fn from_buffer(mut buffer: Vec<u8>) -> RawDocumentBuffer {
        #[inline]
        fn unaligned_prefix(bytes: &[u8]) -> usize {
            let (unaligned_prefix, _, _) = pod_align_to::<u8, Header>(bytes);
            unaligned_prefix.len()
        }

        #[cfg_attr(coverage, coverage(off))] // This function is unreachable outside of Miri.
        fn align_buffer(mut buffer: Vec<u8>) -> (Vec<u8>, usize) {
            // Preallocate the new buffer so there is space for the alignment,
            // and then recompute the alignment adjustment. We need to do this
            // in two steps to avoid the call to `resize()` invalidating the
            // unaligned prefix that we already computed.
            buffer.reserve(align_of::<Header>());

            // Check if call to `reserve()` coincidentally produced the correct
            // alignment.
            let adjust_alignment = unaligned_prefix(&buffer);
            if adjust_alignment != 0 {
                let len = buffer.len();
                buffer.resize(len + adjust_alignment, 0);
                buffer.copy_within(0..len, adjust_alignment);
            }
            (buffer, adjust_alignment)
        }

        // Check the alignment of the buffer. This check will always succeed on
        // most real-world allocators, which return 16-byte aligned buffers, but
        // the Miri allocator for instance does not.
        let mut adjust_alignment = unaligned_prefix(&buffer);

        // This condition is always false outside Miri.
        if adjust_alignment != 0 {
            (buffer, adjust_alignment) = align_buffer(buffer);
        }

        debug_assert_eq!(
            unaligned_prefix(&buffer[adjust_alignment..]),
            0,
            "buffer must be 4-byte aligned",
        );

        // SAFETY: We manually aligned the buffer.
        RawDocumentBuffer {
            buffer,
            adjust_alignment,
        }
    }

    #[inline]
    #[must_use]
    pub fn as_document(&self) -> &RawDocument {
        unsafe {
            // SAFETY: We checked the alignment.
            RawDocument::from_slice_unchecked(self.buffer.get_unchecked(self.adjust_alignment..))
        }
    }
}

#[cfg(feature = "alloc")]
impl core::ops::Deref for RawDocumentBuffer {
    type Target = RawDocument;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_document()
    }
}

/// Unsafe wrapper representing a document.
///
/// All methods on this are unsafe to call, because it does not perform any
/// validation on the document, exception in the `check_*` methods.
// Note: Implementing `PartialEq` and `Eq` in terms of bytewise equality will
// *mostly* work, because of the invariants in the layout of documents, which
// results in one basically canonical representation. However, it is possible to
// manually construct documents that are logically equal, but not bytewise
// equal, because nodes and arguments are not guaranteed to be ordered
// identically in the binary format.
#[derive(PartialEq, Eq, bytemuck::TransparentWrapper)]
#[repr(transparent)]
pub struct RawDocument {
    /// SAFETY INVARIANT: Must be 4-byte aligned and at least
    /// `size_of::<Header>()` bytes long.
    bytes: [u8],
}

impl RawDocument {
    #[inline]
    #[must_use]
    pub const fn empty() -> &'static RawDocument {
        unsafe {
            // SAFETY: Header is Pod.
            &*(core::ptr::from_ref(core::slice::from_raw_parts(
                (&raw const codec::DEFAULT_HEADER).cast::<u8>(),
                size_of::<Header>(),
            )) as *const Self)
        }
    }

    /// Create a new `RawDocument` from a byte slice.
    ///
    /// # Panics
    ///
    /// - The byte slice must be 4-byte aligned.
    /// - The length of the byte slice must be either zero or at least 64 bytes.
    ///
    /// # Safety
    ///
    /// This function is safe to call, but other unsafe methods are not safe to
    /// call unless `bytes` is a valid document.
    #[inline]
    #[must_use]
    pub fn from_slice(bytes: &[u8]) -> &Self {
        if bytes.is_empty() {
            return Self::empty();
        }

        let (unaligned_prefix, _, _) = pod_align_to::<_, Header>(bytes);
        assert!(
            unaligned_prefix.is_empty(),
            "document must be 4-byte aligned, prefix is {} bytes",
            unaligned_prefix.len()
        );

        bytemuck::TransparentWrapper::wrap_ref(bytes)
    }

    /// Create a new `RawDocument` from a byte slice without checking the
    /// alignment.
    ///
    /// # Safety
    ///
    /// This function is safe to call if:
    ///
    /// - The slice is 4-byte aligned.
    /// - The slice is either empty or at least 64 bytes long.
    #[inline]
    #[must_use]
    pub unsafe fn from_slice_unchecked(aligned_bytes: &[u8]) -> &Self {
        if aligned_bytes.is_empty() {
            return Self::empty();
        }

        let (unaligned_prefix, _, _) = pod_align_to::<_, Header>(aligned_bytes);
        debug_assert!(
            unaligned_prefix.is_empty(),
            "document must be 4-byte aligned"
        );
        debug_assert!(aligned_bytes.len() >= size_of::<Header>());
        unsafe { &*(core::ptr::from_ref(aligned_bytes) as *const Self) }
    }

    /// Get the header of the document.
    ///
    /// It does *not* check the header magic or version, or that any other
    ///   fields in the header are valid.
    ///
    /// # Panics
    ///
    /// - This function panics if the document size is greater than 0 but
    ///   smaller than 64 bytes.
    #[inline]
    #[must_use]
    #[expect(clippy::cast_ptr_alignment, clippy::ptr_as_ptr)]
    pub fn header(&self) -> &Header {
        unsafe {
            // SAFETY: The invariant of `self.bytes` is that it is well-aligned
            // and at least `size_of::<Header>()` bytes long.
            &*(self.bytes.as_ptr() as *const Header)
        }
    }

    /// Get the nodes in the document.
    ///
    /// # Panics
    ///
    /// - This function panics if the document does not start with a valid
    ///   header.
    /// - This function panics if the header's `nodes_offset` or `nodes_len` is
    ///   out of bounds.
    #[inline]
    #[must_use]
    pub fn nodes(&self) -> &[codec::Node] {
        let header = self.header();
        let start = header.nodes_offset as usize;
        let end = start + header.nodes_len as usize * size_of::<codec::Node>();
        cast_slice(&self.bytes[start..end])
    }

    /// Get the nodes in the document, without checking that the header's node
    /// information is valid.
    ///
    /// # Safety
    ///
    /// This is safe to call when [`check_header()`] has returned `Ok(())`.
    #[inline]
    #[must_use]
    pub unsafe fn nodes_unchecked(&self) -> &[codec::Node] {
        let header = self.header();
        unsafe {
            // SAFETY: Invariants of this function.
            self.nodes_unchecked_with_header(header)
        }
    }

    #[inline]
    unsafe fn nodes_unchecked_with_header(&self, header: &Header) -> &[codec::Node] {
        let bytes_start = header.nodes_offset as usize;
        let len = header.nodes_len as usize;
        let bytes_end = bytes_start + len * size_of::<codec::Node>();
        unsafe {
            // SAFETY: Invariants of this function.
            let bytes = self.bytes.get_unchecked(bytes_start..bytes_end);
            let ptr = bytes.as_ptr().cast();
            core::slice::from_raw_parts(ptr, len)
        }
    }

    #[inline]
    #[must_use]
    pub fn args(&self) -> &[codec::Arg] {
        let header = self.header();

        // // This check is necessary because the empty document is not guaranteed
        // // to be well-aligned.
        // if header.args_len == 0 {
        //     return &[];
        // }

        let start = header.args_offset as usize;
        let end = start + size_of::<codec::Arg>() * header.args_len as usize;
        cast_slice(&self.bytes[start..end])
    }

    /// Get all node arguments in this document.
    ///
    /// # Safety
    ///
    /// This is safe to call when [`check_header()`] has returned `Ok(())`.
    #[inline]
    #[must_use]
    pub unsafe fn args_unchecked(&self) -> &[codec::Arg] {
        let header = self.header();
        unsafe {
            // SAFETY: Invariants of this function.
            self.args_unchecked_with_header(header)
        }
    }

    #[inline]
    unsafe fn args_unchecked_with_header(&self, header: &Header) -> &[codec::Arg] {
        let bytes_start = header.args_offset as usize;
        let len = header.args_len as usize;

        // // This check is necessary because the empty document is not guaranteed
        // // to be well-aligned.
        // if len == 0 {
        //     return &[];
        // }

        let bytes_end = bytes_start + len * size_of::<codec::Arg>();
        unsafe {
            // SAFETY: Invariants of this function.
            let bytes = self.bytes.get_unchecked(bytes_start..bytes_end);
            let ptr = bytes.as_ptr().cast();
            core::slice::from_raw_parts(ptr, len)
        }
    }

    /// Get the root node.
    ///
    /// # Safety
    ///
    /// This function is safe to call when `check_header()` has returned `Ok(())`.
    #[inline]
    #[must_use]
    pub unsafe fn root_unchecked(&self) -> RawNodeRef {
        let header = self.header();
        let root_offset = header.root_node_index;

        if root_offset == 0 && header.nodes_len == 0 {
            // No root node, and no nodes in the document.
            return RawNodeRef {
                doc: self,
                header,
                node: &codec::Node::EMPTY,
            };
        }

        unsafe {
            // SAFETY: Invariants of this function.
            self.get_node_unchecked_with_header(header, root_offset)
        }
    }

    /// Get a node at the given index.
    ///
    /// # Safety
    ///
    /// `index` must be less than the number of nodes in the document, and
    /// [`check_header()`] and [`check_nodes()`] must have returned `Ok(())`.
    #[inline]
    #[must_use]
    pub unsafe fn get_node_unchecked(&self, index: u32) -> RawNodeRef {
        unsafe {
            // SAFETY: Invariants of this function.
            let header = self.header();
            self.get_node_unchecked_with_header(header, index)
        }
    }

    #[inline]
    #[must_use]
    unsafe fn get_node_unchecked_with_header<'a>(
        &'a self,
        header: &'a Header,
        index: u32,
    ) -> RawNodeRef<'a> {
        unsafe {
            // SAFETY: Invariants of this function.
            let nodes = self.nodes_unchecked_with_header(header);
            let node = nodes.get_unchecked(index as usize);
            RawNodeRef {
                doc: self,
                header,
                node,
            }
        }
    }

    /// Get an argument at the given index.
    ///
    /// The index is relative to the total list of arguments of all nodes in the
    /// document, not the argument index of a particular node.
    ///
    /// # Safety
    ///
    /// `index` must be less than the number of arguments in the document, and
    /// [`check_header()`] and [`check_args()`] must have returned `Ok(())`.
    #[inline]
    #[must_use]
    pub unsafe fn get_arg_unchecked(&self, index: u32) -> RawArgRef {
        unsafe {
            // SAFETY: Invariants of this function.
            let values = self.args_unchecked();
            let codec_arg = values.get_unchecked(index as usize);
            let name = codec_arg.name;
            let value = codec::RawValue::try_from(codec_arg.value).unwrap_unchecked();
            RawArgRef {
                doc: self,
                name,
                value,
            }
        }
    }

    /// Get a string value at the given byte range.
    ///
    /// # Safety
    ///
    /// `range` must refer to a range of valid UTF-8 bytes in the document,
    /// relative to `header.strings_offset`.
    #[inline]
    #[must_use]
    pub unsafe fn get_string_unchecked(&self, range: codec::StringRange) -> &str {
        let header = self.header();
        let start = header.strings_offset as usize + range.start as usize;
        let end = start + range.len as usize;
        unsafe {
            // SAFETY: Invariants of this function.
            let bytes = self.bytes.get_unchecked(start..end);
            core::str::from_utf8_unchecked(bytes)
        }
    }

    /// Get a binary value at the given byte range.
    ///
    /// # Safety
    ///
    /// `range` must be in bounds of the binary section of the document, relative to
    /// `header.binary_offset`.
    #[inline]
    #[must_use]
    pub unsafe fn get_binary_unchecked(&self, range: codec::BinaryRange) -> &[u8] {
        let header = self.header();
        let start = header.binary_offset as usize + range.start as usize;
        let end = start + range.len as usize;
        unsafe {
            // SAFETY: Invariants of this function.
            self.bytes.get_unchecked(start..end)
        }
    }

    /// Check all safety invariants.
    ///
    /// When this returns `Ok(())`, `root_unchecked()` is safe to call, and all
    /// nodes and values reachable from the root node are safe to access.
    ///
    /// # Errors
    ///
    /// If the bytes are not a valid document, this returns an error.
    #[inline]
    pub fn check(&self) -> Result<(), ValidationError> {
        self.check_header()?;
        self.check_nodes()?;
        self.check_args()?;
        self.check_strings()?;
        Ok(())
    }

    /// Check the header of the document.
    ///
    /// This checks that the offsets and lengths of the document's sections are
    /// within bounds.
    ///
    /// # Errors
    ///
    /// If the document header is not valid, this returns an error.
    #[expect(clippy::too_many_lines)]
    pub fn check_header(&self) -> Result<(), ValidationError> {
        #[inline]
        const fn is_overlapping(a: core::ops::Range<usize>, b: core::ops::Range<usize>) -> bool {
            (a.start < b.end) & (b.start < a.end)
        }

        // Check the safety invariants.
        debug_assert!(self.bytes.len() >= size_of::<Header>());
        #[cfg(debug_assertions)]
        {
            let (unaligned, _, _) = pod_align_to::<_, Header>(&self.bytes);
            debug_assert!(unaligned.is_empty(), "document is unaligned");
        }

        let Header {
            magic,
            version,
            root_node_index,
            size,
            nodes_offset,
            nodes_len,
            args_offset,
            args_len,
            strings_offset,
            strings_len,
            binary_offset,
            binary_len,
            reserved1,
            reserved2,
            reserved3,
        } = *self.header();

        if magic != codec::MAGIC {
            return Err(ValidationErrorKind::HeaderMagic.at_offset(0usize));
        }
        if version != codec::VERSION {
            return Err(
                ValidationErrorKind::HeaderVersion(version).at_offset(offset_of!(Header, version))
            );
        }
        if size as usize != self.bytes.len() {
            return Err(ValidationErrorKind::HeaderSize.at_offset(offset_of!(Header, size)));
        }

        if nodes_offset % 4 != 0 {
            return Err(
                ValidationErrorKind::HeaderNodesOffset.at_offset(offset_of!(Header, nodes_offset))
            );
        }
        if nodes_offset != 0 && nodes_offset < size_of::<Header>() as u32 {
            return Err(
                ValidationErrorKind::HeaderNodesOffset.at_offset(offset_of!(Header, nodes_offset))
            );
        }
        if nodes_offset as usize > self.bytes.len() {
            return Err(
                ValidationErrorKind::HeaderNodesOffset.at_offset(offset_of!(Header, nodes_offset))
            );
        }

        let Some(nodes_end) = (nodes_len as usize)
            .checked_mul(size_of::<codec::Node>())
            .and_then(|len| len.checked_add(nodes_offset as usize))
            .filter(|end| *end <= self.bytes.len())
        else {
            return Err(
                ValidationErrorKind::HeaderNodesLen.at_offset(offset_of!(Header, nodes_len))
            );
        };

        if args_offset % 4 != 0 {
            return Err(
                ValidationErrorKind::HeaderArgsOffset.at_offset(offset_of!(Header, args_offset))
            );
        }
        if args_offset != 0 && args_offset < size_of::<Header>() as u32 {
            return Err(
                ValidationErrorKind::HeaderArgsOffset.at_offset(offset_of!(Header, args_offset))
            );
        }
        if args_offset as usize > self.bytes.len() {
            return Err(
                ValidationErrorKind::HeaderArgsOffset.at_offset(offset_of!(Header, args_offset))
            );
        }
        let Some(args_end) = (args_len as usize)
            .checked_mul(size_of::<codec::Arg>())
            .and_then(|len| len.checked_add(args_offset as usize))
            .filter(|end| *end <= self.bytes.len())
        else {
            return Err(ValidationErrorKind::HeaderArgsLen.at_offset(offset_of!(Header, args_len)));
        };

        if strings_offset as usize > self.bytes.len() {
            return Err(ValidationErrorKind::HeaderStringsOffset
                .at_offset(offset_of!(Header, strings_offset)));
        }
        if strings_offset != 0 && strings_offset < size_of::<Header>() as u32 {
            return Err(ValidationErrorKind::HeaderStringsOffset
                .at_offset(offset_of!(Header, strings_offset)));
        }
        let Some(strings_end) = (strings_offset as usize)
            .checked_add(strings_len as usize)
            .filter(|end| *end <= self.bytes.len())
        else {
            return Err(
                ValidationErrorKind::HeaderStringsLen.at_offset(offset_of!(Header, strings_len))
            );
        };

        if binary_offset as usize > self.bytes.len() {
            return Err(ValidationErrorKind::HeaderBinaryOffset
                .at_offset(offset_of!(Header, binary_offset)));
        }
        if binary_offset != 0 && binary_offset < size_of::<Header>() as u32 {
            return Err(ValidationErrorKind::HeaderBinaryOffset
                .at_offset(offset_of!(Header, binary_offset)));
        }
        let Some(binary_end) = (binary_offset as usize)
            .checked_add(binary_len as usize)
            .filter(|end| *end <= self.bytes.len())
        else {
            return Err(
                ValidationErrorKind::HeaderBinaryLen.at_offset(offset_of!(Header, binary_len))
            );
        };

        if root_node_index != 0 && root_node_index >= nodes_len {
            return Err(ValidationErrorKind::HeaderRootNodeOutOfBounds
                .at_offset(offset_of!(Header, root_node_index)));
        }

        if is_overlapping(
            nodes_offset as usize..nodes_end,
            args_offset as usize..args_end,
        ) {
            return Err(ValidationErrorKind::HeaderSectionsOverlap
                .at_offset(offset_of!(Header, args_offset)));
        }

        if is_overlapping(
            nodes_offset as usize..nodes_end,
            strings_offset as usize..strings_end,
        ) {
            return Err(ValidationErrorKind::HeaderSectionsOverlap
                .at_offset(offset_of!(Header, strings_offset)));
        }

        if is_overlapping(
            nodes_offset as usize..nodes_end,
            binary_offset as usize..binary_end,
        ) {
            return Err(ValidationErrorKind::HeaderSectionsOverlap
                .at_offset(offset_of!(Header, binary_offset)));
        }

        if is_overlapping(
            args_offset as usize..args_end,
            strings_offset as usize..strings_end,
        ) {
            return Err(ValidationErrorKind::HeaderSectionsOverlap
                .at_offset(offset_of!(Header, strings_offset)));
        }

        if is_overlapping(
            args_offset as usize..args_end,
            binary_offset as usize..binary_end,
        ) {
            return Err(ValidationErrorKind::HeaderSectionsOverlap
                .at_offset(offset_of!(Header, binary_offset)));
        }

        if is_overlapping(
            strings_offset as usize..strings_end,
            binary_offset as usize..binary_end,
        ) {
            return Err(ValidationErrorKind::HeaderSectionsOverlap
                .at_offset(offset_of!(Header, binary_offset)));
        }

        if reserved1 != 0 {
            return Err(ValidationErrorKind::HeaderReservedFieldsMustBeZero
                .at_offset(offset_of!(Header, reserved1)));
        }
        if reserved2 != 0 {
            return Err(ValidationErrorKind::HeaderReservedFieldsMustBeZero
                .at_offset(offset_of!(Header, reserved2)));
        }
        if reserved3 != 0 {
            return Err(ValidationErrorKind::HeaderReservedFieldsMustBeZero
                .at_offset(offset_of!(Header, reserved3)));
        }

        Ok(())
    }

    /// Check the strings in the document.
    ///
    /// This checks that the strings in the document are valid UTF-8.
    ///
    /// # Errors
    ///
    /// If the strings are not valid UTF-8, this returns an error.
    #[inline]
    pub fn check_strings(&self) -> Result<(), ValidationError> {
        let header = self.header();
        let start = header.strings_offset as usize;
        let end = start + header.strings_len as usize;
        core::str::from_utf8(&self.bytes[start..end])
            .map(|_| ())
            .map_err(|_| ValidationErrorKind::InvalidUtf8.at_offset(header.strings_offset))
    }

    /// Get a string value from the document, checking whether it is in bounds
    /// and valid UTF-8.
    ///
    /// # Errors
    ///
    /// If the range is out of bounds, or the string is not valid UTF-8, this
    /// returns an error.
    #[inline]
    pub fn get_string(&self, range: codec::StringRange) -> Result<&str, ValidationError> {
        let header = self.header();
        let start = header.strings_offset as usize + range.start as usize;
        let end = start + range.len as usize;
        let bytes = self.bytes.get(start..end).ok_or_else(|| {
            ValidationErrorKind::StringOutOfBounds.at_offset(header.strings_offset)
        })?;
        core::str::from_utf8(bytes)
            .map_err(|_| ValidationErrorKind::InvalidUtf8.at_offset(header.strings_offset))
    }

    /// Check the nodes in the document.
    ///
    /// This checks that the nodes in the document are valid, including
    /// references between nodes. It does *not* check that node arguments are
    /// valid.
    ///
    /// # Errors
    ///
    /// If the nodes are not valid, this returns an error.
    #[inline]
    pub fn check_nodes(&self) -> Result<(), ValidationError> {
        let header = self.header();
        if header.nodes_len == 0 {
            return Ok(());
        }
        let nodes = self.nodes();
        for (index, node) in nodes.iter().enumerate() {
            Self::check_node(header, index as u32, node)?;
        }
        Ok(())
    }

    #[inline]
    fn check_node(header: &Header, index: u32, node: &codec::Node) -> Result<(), ValidationError> {
        let offset = header.nodes_offset + index * size_of::<codec::Node>() as u32;
        let name_offset = offset + offset_of!(codec::Node, name) as u32;
        let ty_offset = offset + offset_of!(codec::Node, ty) as u32;
        let args_offset = offset + offset_of!(codec::Node, args) as u32;
        let children_offset = offset + offset_of!(codec::Node, children) as u32;

        // Invariant: Node name and type must be valid strings.
        Self::check_string(header, name_offset, node.name)?;
        Self::check_string(header, ty_offset, node.ty)?;

        // Invariant: Node arguments must be valid.
        Self::check_arg_range(header, args_offset, node.args)?;

        // Invariant: All children of a node must come after the node itself.
        // This ensures that there are no circular references, and also helps
        // with cache locality on depth-first traversals.
        if node.children.len != 0 && node.children.start <= index {
            return Err(ValidationErrorKind::ChildrenBeforeParent.at_offset(children_offset));
        }

        // Invariant: Children of the node must be valid.
        Self::check_node_range(header, children_offset, node.children)
    }

    #[inline]
    fn check_arg_range(
        header: &Header,
        offset: u32,
        range: codec::ArgRange,
    ) -> Result<(), ValidationError> {
        let end = header.args_len;
        let range_end = range
            .start
            .checked_add(range.len)
            .ok_or(ValidationErrorKind::LengthOverflow.at_offset(offset))?;
        if range.start <= end && range_end <= end {
            return Ok(());
        }
        Err(ValidationErrorKind::ArgumentsOutOfBounds.at_offset(offset))
    }

    #[inline]
    fn check_node_range(
        header: &Header,
        offset: u32,
        range: codec::NodeRange,
    ) -> Result<(), ValidationError> {
        let end = header.nodes_len;
        let range_end = range
            .start
            .checked_add(range.len)
            .ok_or(ValidationErrorKind::LengthOverflow.at_offset(offset))?;
        if range.start <= end && range_end <= end {
            return Ok(());
        }
        Err(ValidationErrorKind::ChildrenOutOfBounds.at_offset(offset))
    }

    /// Check the integrity of all values in the document.
    ///
    /// When this is `Ok(())`, it means that all string ranges and binary ranges
    /// are valid (in bounds).
    ///
    /// # Errors
    ///
    /// If the values are not valid, this returns an error.
    #[inline]
    pub fn check_args(&self) -> Result<(), ValidationError> {
        let args = self.args();
        let header = self.header();
        for (index, arg) in args.iter().enumerate() {
            Self::check_arg(header, index as u32, arg)?;
        }
        Ok(())
    }

    #[inline]
    fn check_arg(header: &Header, index: u32, arg: &codec::Arg) -> Result<(), ValidationError> {
        let offset = header.args_offset + index * size_of::<codec::Arg>() as u32;
        let name_offset = offset + offset_of!(codec::Arg, name) as u32;
        let value_offset = offset + offset_of!(codec::Arg, value) as u32;

        Self::check_string(header, name_offset, arg.name)?;
        let value = arg.value;
        Self::check_value(header, value_offset, value)
    }

    #[inline]
    fn check_value(
        header: &Header,
        offset: u32,
        value: codec::Value,
    ) -> Result<(), ValidationError> {
        let ty_offset = offset + offset_of!(codec::Value, ty) as u32;
        let payload_offset = offset + offset_of!(codec::Value, payload) as u32;
        match value.try_into() {
            Err(err) => Err(err.at_offset(ty_offset)),
            Ok(codec::RawValue::String(range)) => Self::check_string(header, payload_offset, range),
            Ok(codec::RawValue::Binary(range)) => Self::check_binary(header, payload_offset, range),
            _ => Ok(()),
        }
    }

    #[inline]
    fn check_string(
        header: &Header,
        offset: u32,
        range: codec::StringRange,
    ) -> Result<(), ValidationError> {
        let len = header.strings_len;
        let range_end = range
            .start
            .checked_add(range.len)
            .ok_or(ValidationErrorKind::LengthOverflow.at_offset(offset))?;

        if range.start <= len && range_end <= len {
            return Ok(());
        }
        Err(ValidationErrorKind::StringOutOfBounds.at_offset(offset))
    }

    #[inline]
    fn check_binary(
        header: &Header,
        offset: u32,
        range: codec::BinaryRange,
    ) -> Result<(), ValidationError> {
        let len = header.binary_len;
        let range_end = range
            .start
            .checked_add(range.len)
            .ok_or(ValidationErrorKind::LengthOverflow.at_offset(offset))?;

        if range.start <= len && range_end <= len {
            return Ok(());
        }
        Err(ValidationErrorKind::BinaryOutOfBounds.at_offset(offset))
    }

    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Copy)]
pub struct RawNodeRef<'a> {
    doc: &'a RawDocument,
    header: &'a Header,
    node: &'a codec::Node,
}

impl<'a> RawNodeRef<'a> {
    /// The offset of this node in the document's node block.
    ///
    /// # Safety
    ///
    /// This `RawNodeRef` must come from a valid document.
    #[must_use]
    pub unsafe fn raw_index(&self) -> usize {
        let node_ptr = core::ptr::from_ref(self.node);
        let start_ptr = unsafe {
            // SAFETY: Invariants of this function.
            self.doc.nodes_unchecked().as_ptr()
        };

        let offset = node_ptr as usize;
        let start = start_ptr as usize;
        let distance = offset - start;
        distance / size_of::<codec::Node>()
    }

    #[inline]
    #[must_use]
    pub fn encoded(&self) -> &'a codec::Node {
        self.node
    }

    /// Get the name of the node.
    ///
    /// If the node is unnamed, this returns the empty string.
    ///
    /// # Safety
    ///
    /// This is safe to call when `check_nodes()` and `check_strings()` has
    /// returned `Ok(())`.
    #[inline]
    #[must_use]
    pub unsafe fn name_unchecked(&self) -> &'a str {
        unsafe {
            // SAFETY: Invariants of this function.
            self.doc.get_string_unchecked(self.node.name)
        }
    }

    /// Get the type of the node.
    ///
    /// If the node has no type, this returns the empty string.
    ///
    /// # Safety
    ///
    /// This is safe to call when `check_nodes()` and `check_strings()` has
    /// returned `Ok(())`.
    #[inline]
    #[must_use]
    pub unsafe fn ty_unchecked(&self) -> &'a str {
        unsafe {
            // SAFETY: Invariants of this function.
            self.doc.get_string_unchecked(self.node.ty)
        }
    }

    #[inline]
    #[must_use]
    pub fn children_range(&self) -> codec::NodeRange {
        self.node.children
    }

    #[inline]
    #[must_use]
    pub fn children(&self) -> RawNodeChildren<'a> {
        RawNodeChildren {
            doc: self.doc,
            header: self.header,
            node: self.node,
        }
    }

    #[inline]
    #[must_use]
    pub fn args(&self) -> RawNodeArgs<'a> {
        RawNodeArgs {
            doc: self.doc,
            node: self.node,
        }
    }
}

#[derive(Clone, Copy)]
pub struct RawNodeChildren<'a> {
    doc: &'a RawDocument,
    header: &'a Header,
    node: &'a codec::Node,
}

impl<'a> RawNodeChildren<'a> {
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.node.children.len == 0
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.node.children.len as usize
    }

    /// Get child at index.
    ///
    /// # Safety
    ///
    /// `index` must be less than the number of children in the node, and
    /// [`check_nodes()`] must have returned `Ok(())`.
    #[inline]
    #[must_use]
    pub unsafe fn get_unchecked(&self, index: usize) -> RawNodeRef<'a> {
        let children_start = self.node.children.start;
        let child_offset = children_start + index as u32;
        unsafe {
            // SAFETY: Invariants of this function.
            self.doc
                .get_node_unchecked_with_header(self.header, child_offset)
        }
    }
}

#[derive(Clone, Copy)]
pub struct RawNodeArgs<'a> {
    doc: &'a RawDocument,
    node: &'a codec::Node,
}

impl<'a> RawNodeArgs<'a> {
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.node.args.len == 0
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.node.args.len as usize
    }

    /// Get argument at index.
    ///
    /// # Safety
    ///
    /// `index` must be less than the number of arguments in the node, and
    /// [`check_args()`] must have returned `Ok(())`.
    #[inline]
    #[must_use]
    pub unsafe fn get_unchecked(&self, index: usize) -> RawArgRef<'a> {
        let args_start = self.node.args.start;
        let arg_offset = args_start + index as u32;
        unsafe {
            // SAFETY: Invariants of this function.
            self.doc.get_arg_unchecked(arg_offset)
        }
    }
}

#[derive(Clone, Copy)]
pub struct RawArgRef<'a> {
    doc: &'a RawDocument,
    name: codec::StringRange,
    value: codec::RawValue,
}

impl<'a> RawArgRef<'a> {
    /// Get the name of the argument.
    ///
    /// If the argument is unnamed, this returns the empty string.
    ///
    /// # Safety
    ///
    /// This is safe to call when `check_args()` and `check_strings()` has
    /// returned `Ok(())`.
    #[must_use]
    pub unsafe fn name_unchecked(&self) -> &'a str {
        unsafe {
            // SAFETY: Invariants of this function.
            self.doc.get_string_unchecked(self.name)
        }
    }

    /// Get the value of the argument.
    ///
    /// # Safety
    ///
    /// This is safe to call when `check_args()` and `check_strings()` has
    /// returned `Ok(())`.
    #[must_use]
    pub unsafe fn get_unchecked(&self) -> ValueRef<'a> {
        match self.value {
            codec::RawValue::Null => ValueRef::Null,
            codec::RawValue::Bool(value) => ValueRef::Bool(value),
            codec::RawValue::Int(value) => ValueRef::Int(value),
            codec::RawValue::Uint(value) => ValueRef::Uint(value),
            codec::RawValue::Float(value) => ValueRef::Float(value),
            codec::RawValue::String(range) => unsafe {
                // SAFETY: Invariants of this function.
                ValueRef::String(self.doc.get_string_unchecked(range))
            },
            codec::RawValue::Binary(range) => unsafe {
                // SAFETY: Invariants of this function.
                ValueRef::Binary(self.doc.get_binary_unchecked(range))
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ValueRef<'a> {
    Null,
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    String(&'a str),
    Binary(&'a [u8]),
}

impl<'a> ValueRef<'a> {
    /// Interpret a raw argument reference and convert it to an enum.
    ///
    /// # Safety
    ///
    /// This is safe to call when `check_args()` and `check_strings()` has
    /// returned `Ok(())`. `raw` must come from a valid document.
    #[inline]
    #[must_use]
    pub unsafe fn from_raw(raw: RawArgRef<'a>) -> Self {
        unsafe { raw.get_unchecked() }
    }
}

impl core::fmt::Debug for ValueRef<'_> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ValueRef::Null => f.write_str("null"),
            ValueRef::Bool(value) => write!(f, "{value}"),
            ValueRef::Int(value) => write!(f, "{value}"),
            ValueRef::Uint(value) => write!(f, "{value}"),
            ValueRef::Float(value) => write!(f, "{value}"),
            ValueRef::String(value) => write!(f, "\"{}\"", value.escape_debug()),
            ValueRef::Binary(value) => write!(f, "({} bytes)", value.len()),
        }
    }
}
