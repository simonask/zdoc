use bytemuck::{bytes_of, cast_slice, cast_slice_mut};
use zdocument::{Document, ValidationError, ValidationErrorKind, codec};

#[test]
fn empty() {
    assert_eq!(Document::from_slice(b""), Ok(Document::empty()));
    let empty = [0u32; 16];
    assert_eq!(
        Document::from_slice(cast_slice(&empty)),
        Err(ValidationError {
            offset: 0,
            error: ValidationErrorKind::HeaderMagic,
        })
    );
}

#[test]
#[expect(clippy::too_many_lines)]
fn validate_header() {
    let mut buf = [0u32; 128];
    let buf: &mut [u8] = cast_slice_mut(&mut buf);

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            magic: [0; 8],
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 0,
            error: ValidationErrorKind::HeaderMagic
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // only version 1 is supported
            version: 0,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 8,
            error: ValidationErrorKind::HeaderVersion(0)
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // size must include header size
            size: 0,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 16,
            error: ValidationErrorKind::HeaderSize
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // out of bounds
            root_node_index: 1,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 12,
            error: ValidationErrorKind::HeaderRootNodeOutOfBounds,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // aligned, in bounds, but inside header
            nodes_offset: 32,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 20,
            error: ValidationErrorKind::HeaderNodesOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // unaligned and out of bounds.
            nodes_offset: 65,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 20,
            error: ValidationErrorKind::HeaderNodesOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // aligned and out of bounds.
            nodes_offset: 68,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 20,
            error: ValidationErrorKind::HeaderNodesOffset,
        })
    );

    buf[0..64].copy_from_slice(bytes_of(&codec::Header {
        // unaligned but in bounds
        nodes_offset: 65,
        size: 96,
        ..codec::DEFAULT_HEADER
    }));
    assert_eq!(
        Document::from_slice(&buf[..96]),
        Err(ValidationError {
            offset: 20,
            error: ValidationErrorKind::HeaderNodesOffset,
        })
    );
    buf[0..64].copy_from_slice(bytes_of(&codec::Header {
        // aligned and in bounds
        nodes_offset: 64,
        // out of bounds
        nodes_len: 2,
        size: 96,
        ..codec::DEFAULT_HEADER
    }));
    assert_eq!(
        Document::from_slice(&buf[..96]),
        Err(ValidationError {
            offset: 24,
            error: ValidationErrorKind::HeaderNodesLen,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // aligned, in bounds, but inside header
            args_offset: 32,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 28,
            error: ValidationErrorKind::HeaderArgsOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // unaligned and out of bounds.
            args_offset: 65,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 28,
            error: ValidationErrorKind::HeaderArgsOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // aligned and out of bounds.
            args_offset: 68,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 28,
            error: ValidationErrorKind::HeaderArgsOffset,
        })
    );

    buf[0..64].copy_from_slice(bytes_of(&codec::Header {
        // unaligned but in bounds
        args_offset: 65,
        size: 80,
        ..codec::DEFAULT_HEADER
    }));
    assert_eq!(
        Document::from_slice(&buf[..80]),
        Err(ValidationError {
            offset: 28,
            error: ValidationErrorKind::HeaderArgsOffset,
        })
    );
    buf[0..64].copy_from_slice(bytes_of(&codec::Header {
        // unaligned but in bounds
        args_offset: 64,
        // out of bounds
        args_len: 2,
        size: 80,
        ..codec::DEFAULT_HEADER
    }));
    assert_eq!(
        Document::from_slice(&buf[..80]),
        Err(ValidationError {
            offset: 32,
            error: ValidationErrorKind::HeaderArgsLen,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // within header
            strings_offset: 1,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 36,
            error: ValidationErrorKind::HeaderStringsOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // offset out of bounds
            strings_offset: 65,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 36,
            error: ValidationErrorKind::HeaderStringsOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // offset+len out of bounds
            strings_offset: 64,
            strings_len: 1,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 40,
            error: ValidationErrorKind::HeaderStringsLen,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // within header
            binary_offset: 1,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 44,
            error: ValidationErrorKind::HeaderBinaryOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // offset out of bounds
            binary_offset: 65,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 44,
            error: ValidationErrorKind::HeaderBinaryOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // offset+len out of bounds
            binary_offset: 64,
            binary_len: 1,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 48,
            error: ValidationErrorKind::HeaderBinaryLen,
        })
    );
    assert_eq!(
        Document::from_slice(bytes_of(&codec::Header {
            // offset+len overflow
            binary_offset: 64,
            binary_len: u32::MAX - 63,
            ..codec::DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 48,
            error: ValidationErrorKind::HeaderBinaryLen,
        })
    );
}

#[test]
fn validate_strings() {
    let mut buf = [0u32; 128];
    let buf: &mut [u8] = cast_slice_mut(&mut buf);
    buf[0..64].copy_from_slice(bytes_of(&codec::Header {
        strings_offset: 64,
        strings_len: 1,
        size: 65,
        ..codec::DEFAULT_HEADER
    }));
    buf[64] = 0xFF;
    assert_eq!(
        Document::from_slice(&buf[..65]),
        Err(ValidationError {
            offset: 64,
            error: ValidationErrorKind::InvalidUtf8
        })
    );
}
