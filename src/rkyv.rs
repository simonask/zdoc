use crate::{Document, codec};
use rkyv::{primitive::ArchivedUsize, ser::WriterExt as _};

impl rkyv::ArchiveUnsized for Document {
    type Archived = Document;

    #[inline]
    fn archived_metadata(&self) -> rkyv::ArchivedMetadata<Self> {
        ArchivedUsize::from_native(rkyv::ptr_meta::metadata(self) as _)
    }
}

// SAFETY: The `Document` type is a thin wrapper around a raw pointer to a
// `RawDocument`, which is a thin wrapper around a byte slice.
unsafe impl rkyv::ptr_meta::Pointee for Document {
    type Metadata = usize;
}

// SAFETY: All codec types are `#[repr(C)]`, and we check at compile time that
// the layout matches expectations.
unsafe impl rkyv::Portable for Document {}

impl rkyv::traits::ArchivePointee for Document {
    type ArchivedMetadata = ArchivedUsize;

    fn pointer_metadata(
        archived: &Self::ArchivedMetadata,
    ) -> <Self as rkyv::ptr_meta::Pointee>::Metadata {
        <[u8]>::pointer_metadata(archived)
    }
}

impl<S> rkyv::SerializeUnsized<S> for Document
where
    S: rkyv::rancor::Fallible + rkyv::ser::Allocator + rkyv::ser::Writer + ?Sized,
{
    fn serialize_unsized(&self, serializer: &mut S) -> Result<usize, S::Error> {
        let result = serializer.align_for::<codec::Header>();
        serializer.write(self.as_bytes())?;
        result
    }
}

impl<D> rkyv::DeserializeUnsized<Document, D> for Document
where
    D: rkyv::rancor::Fallible,
{
    unsafe fn deserialize_unsized(&self, _: &mut D, out: *mut Document) -> Result<(), D::Error> {
        unsafe {
            // SAFETY: `Document` is #[repr(transparent)].
            // SAFETY: `RawDocument` is #[repr(transparent)].
            let dst = out.cast::<u8>();
            let src = self.as_bytes();
            let src_ptr = src.as_ptr();
            let len = src.len();
            core::ptr::copy_nonoverlapping(src_ptr, dst, len);
        }

        Ok(())
    }

    #[inline]
    fn deserialize_metadata(&self) -> <Document as rkyv::ptr_meta::Pointee>::Metadata {
        rkyv::ptr_meta::metadata(self)
    }
}
