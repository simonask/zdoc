use bytemuck::{bytes_of, cast_slice, cast_slice_mut};
use zdoc::{
    Document, ValidationError, ValidationErrorKind,
    codec::{ArgRange, DEFAULT_HEADER, Header, Node, NodeRange, StringRange},
};

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
fn validate_header_magic() {
    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            magic: [0; 8],
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 0,
            error: ValidationErrorKind::HeaderMagic
        })
    );
}

#[test]
fn validate_header_version() {
    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // only version 1 is supported
            version: 0,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 8,
            error: ValidationErrorKind::HeaderVersion(0)
        })
    );
}

#[test]
fn validate_header_size() {
    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // size must include header size
            size: 0,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 16,
            error: ValidationErrorKind::HeaderSize
        })
    );
}

#[test]
fn validate_header_root_node_index() {
    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // out of bounds
            root_node_index: 1,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 12,
            error: ValidationErrorKind::HeaderRootNodeOutOfBounds,
        })
    );
}

#[test]
fn validate_header_nodes() {
    let mut buf = [0u32; 128];
    let buf: &mut [u8] = cast_slice_mut(&mut buf);
    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // aligned, in bounds, but inside header
            nodes_offset: 32,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 20,
            error: ValidationErrorKind::HeaderNodesOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // unaligned and out of bounds.
            nodes_offset: 65,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 20,
            error: ValidationErrorKind::HeaderNodesOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // aligned and out of bounds.
            nodes_offset: 68,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 20,
            error: ValidationErrorKind::HeaderNodesOffset,
        })
    );

    buf[0..64].copy_from_slice(bytes_of(&Header {
        // unaligned but in bounds
        nodes_offset: 65,
        size: 96,
        ..DEFAULT_HEADER
    }));
    assert_eq!(
        Document::from_slice(&buf[..96]),
        Err(ValidationError {
            offset: 20,
            error: ValidationErrorKind::HeaderNodesOffset,
        })
    );
    buf[0..64].copy_from_slice(bytes_of(&Header {
        // aligned and in bounds
        nodes_offset: 64,
        // out of bounds
        nodes_len: 2,
        size: 96,
        ..DEFAULT_HEADER
    }));
    assert_eq!(
        Document::from_slice(&buf[..96]),
        Err(ValidationError {
            offset: 24,
            error: ValidationErrorKind::HeaderNodesLen,
        })
    );
}

#[test]
fn validate_header_args() {
    let mut buf = [0u32; 128];
    let buf: &mut [u8] = cast_slice_mut(&mut buf);
    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // aligned, in bounds, but inside header
            args_offset: 32,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 28,
            error: ValidationErrorKind::HeaderArgsOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // unaligned and out of bounds.
            args_offset: 65,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 28,
            error: ValidationErrorKind::HeaderArgsOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // aligned and out of bounds.
            args_offset: 68,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 28,
            error: ValidationErrorKind::HeaderArgsOffset,
        })
    );

    buf[0..64].copy_from_slice(bytes_of(&Header {
        // unaligned but in bounds
        args_offset: 65,
        size: 80,
        ..DEFAULT_HEADER
    }));
    assert_eq!(
        Document::from_slice(&buf[..80]),
        Err(ValidationError {
            offset: 28,
            error: ValidationErrorKind::HeaderArgsOffset,
        })
    );
    buf[0..64].copy_from_slice(bytes_of(&Header {
        // unaligned but in bounds
        args_offset: 64,
        // out of bounds
        args_len: 2,
        size: 80,
        ..DEFAULT_HEADER
    }));
    assert_eq!(
        Document::from_slice(&buf[..80]),
        Err(ValidationError {
            offset: 32,
            error: ValidationErrorKind::HeaderArgsLen,
        })
    );
}

#[test]
fn validate_header_overlap() {
    let mut buf = [0u32; 128];
    let buf: &mut [u8] = cast_slice_mut(&mut buf);

    // nodes and args overlap
    buf[..64].copy_from_slice(bytes_of(&Header {
        nodes_offset: 64,
        nodes_len: 1,
        args_offset: 80,
        args_len: 1,
        size: 100,
        ..DEFAULT_HEADER
    }));
    assert_eq!(
        Document::from_slice(&buf[..100]),
        Err(ValidationError {
            offset: 28,
            error: ValidationErrorKind::HeaderSectionsOverlap,
        })
    );

    // nodes and strings overlap
    buf[..64].copy_from_slice(bytes_of(&Header {
        nodes_offset: 64,
        nodes_len: 1,
        strings_offset: 64,
        strings_len: 1,
        size: 96,
        ..DEFAULT_HEADER
    }));
    assert_eq!(
        Document::from_slice(&buf[..96]),
        Err(ValidationError {
            offset: 36,
            error: ValidationErrorKind::HeaderSectionsOverlap,
        })
    );

    // nodes and binary overlap
    buf[..64].copy_from_slice(bytes_of(&Header {
        nodes_offset: 64,
        nodes_len: 1,
        binary_offset: 64,
        binary_len: 1,
        size: 96,
        ..DEFAULT_HEADER
    }));
    assert_eq!(
        Document::from_slice(&buf[..96]),
        Err(ValidationError {
            offset: 44,
            error: ValidationErrorKind::HeaderSectionsOverlap,
        })
    );

    // args and strings overlap
    buf[..64].copy_from_slice(bytes_of(&Header {
        args_offset: 64,
        args_len: 1,
        strings_offset: 64,
        strings_len: 1,
        size: 84,
        ..DEFAULT_HEADER
    }));
    assert_eq!(
        Document::from_slice(&buf[..84]),
        Err(ValidationError {
            offset: 36,
            error: ValidationErrorKind::HeaderSectionsOverlap,
        })
    );

    // args and binary overlap
    buf[..64].copy_from_slice(bytes_of(&Header {
        args_offset: 64,
        args_len: 1,
        binary_offset: 64,
        binary_len: 1,
        size: 84,
        ..DEFAULT_HEADER
    }));
    assert_eq!(
        Document::from_slice(&buf[..84]),
        Err(ValidationError {
            offset: 44,
            error: ValidationErrorKind::HeaderSectionsOverlap,
        })
    );

    // strings and binary overlap
    buf[..64].copy_from_slice(bytes_of(&Header {
        strings_offset: 64,
        strings_len: 1,
        binary_offset: 64,
        binary_len: 1,
        size: 65,
        ..DEFAULT_HEADER
    }));
    assert_eq!(
        Document::from_slice(&buf[..65]),
        Err(ValidationError {
            offset: 44,
            error: ValidationErrorKind::HeaderSectionsOverlap,
        })
    );
}

#[test]
fn validate_header_reserved() {
    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            reserved1: 1,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 52,
            error: ValidationErrorKind::HeaderReservedFieldsMustBeZero,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            reserved2: 1,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 56,
            error: ValidationErrorKind::HeaderReservedFieldsMustBeZero,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            reserved3: 1,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 60,
            error: ValidationErrorKind::HeaderReservedFieldsMustBeZero,
        })
    );
}

#[test]
fn validate_header_strings() {
    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // within header
            strings_offset: 1,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 36,
            error: ValidationErrorKind::HeaderStringsOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // offset out of bounds
            strings_offset: 65,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 36,
            error: ValidationErrorKind::HeaderStringsOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // offset+len out of bounds
            strings_offset: 64,
            strings_len: 1,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 40,
            error: ValidationErrorKind::HeaderStringsLen,
        })
    );
}

#[test]
fn validate_header_binary() {
    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // within header
            binary_offset: 1,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 44,
            error: ValidationErrorKind::HeaderBinaryOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // offset out of bounds
            binary_offset: 65,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 44,
            error: ValidationErrorKind::HeaderBinaryOffset,
        })
    );

    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // offset+len out of bounds
            binary_offset: 64,
            binary_len: 1,
            ..DEFAULT_HEADER
        })),
        Err(ValidationError {
            offset: 48,
            error: ValidationErrorKind::HeaderBinaryLen,
        })
    );
    assert_eq!(
        Document::from_slice(bytes_of(&Header {
            // offset+len overflow
            binary_offset: 64,
            binary_len: u32::MAX - 63,
            ..DEFAULT_HEADER
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
    buf[0..64].copy_from_slice(bytes_of(&Header {
        strings_offset: 64,
        strings_len: 1,
        size: 65,
        ..DEFAULT_HEADER
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

#[test]
#[expect(clippy::too_many_lines)]
fn validate_nodes() {
    let mut buf = [0u32; 128];
    let buf: &mut [u8] = cast_slice_mut(&mut buf);
    buf[0..64].copy_from_slice(bytes_of(&Header {
        nodes_offset: 64,
        nodes_len: 1,
        size: 96,
        ..DEFAULT_HEADER
    }));

    // self-referential root
    buf[64..96].copy_from_slice(bytes_of(&Node {
        args: ArgRange::EMPTY,
        children: NodeRange { start: 0, len: 1 },
        name: StringRange::EMPTY,
        ty: StringRange::EMPTY,
    }));
    assert_eq!(
        Document::from_slice(&buf[..96]),
        Err(ValidationError {
            offset: 72,
            error: ValidationErrorKind::ChildrenBeforeParent,
        })
    );

    // children start out of bounds
    buf[64..96].copy_from_slice(bytes_of(&Node {
        args: ArgRange::EMPTY,
        children: NodeRange { start: 1, len: 1 },
        name: StringRange::EMPTY,
        ty: StringRange::EMPTY,
    }));
    assert_eq!(
        Document::from_slice(&buf[..96]),
        Err(ValidationError {
            offset: 72,
            error: ValidationErrorKind::ChildrenOutOfBounds,
        })
    );

    // children len out of bounds
    buf[..64].copy_from_slice(bytes_of(&Header {
        nodes_offset: 64,
        nodes_len: 2,
        size: 128,
        ..DEFAULT_HEADER
    }));
    buf[64..96].copy_from_slice(bytes_of(&Node {
        args: ArgRange::EMPTY,
        children: NodeRange { start: 1, len: 2 },
        name: StringRange::EMPTY,
        ty: StringRange::EMPTY,
    }));
    assert_eq!(
        Document::from_slice(&buf[..128]),
        Err(ValidationError {
            offset: 72,
            error: ValidationErrorKind::ChildrenOutOfBounds,
        })
    );

    // children len overflow
    buf[..64].copy_from_slice(bytes_of(&Header {
        nodes_offset: 64,
        nodes_len: 2,
        size: 128,
        ..DEFAULT_HEADER
    }));
    buf[64..96].copy_from_slice(bytes_of(&Node {
        args: ArgRange::EMPTY,
        children: NodeRange {
            start: 1,
            len: u32::MAX,
        },
        name: StringRange::EMPTY,
        ty: StringRange::EMPTY,
    }));
    assert_eq!(
        Document::from_slice(&buf[..128]),
        Err(ValidationError {
            offset: 72,
            error: ValidationErrorKind::LengthOverflow,
        })
    );

    // args start out of bounds
    buf[..64].copy_from_slice(bytes_of(&Header {
        nodes_offset: 64,
        nodes_len: 1,
        args_offset: 96,
        args_len: 1,
        size: 116,
        ..DEFAULT_HEADER
    }));
    buf[64..96].copy_from_slice(bytes_of(&Node {
        args: ArgRange { start: 1, len: 1 },
        children: NodeRange::EMPTY,
        name: StringRange::EMPTY,
        ty: StringRange::EMPTY,
    }));
    assert_eq!(
        Document::from_slice(&buf[..116]),
        Err(ValidationError {
            offset: 64,
            error: ValidationErrorKind::ArgumentsOutOfBounds,
        })
    );

    // args len out of bounds
    buf[64..96].copy_from_slice(bytes_of(&Node {
        args: ArgRange { start: 0, len: 2 },
        children: NodeRange::EMPTY,
        name: StringRange::EMPTY,
        ty: StringRange::EMPTY,
    }));
    assert_eq!(
        Document::from_slice(&buf[..116]),
        Err(ValidationError {
            offset: 64,
            error: ValidationErrorKind::ArgumentsOutOfBounds,
        })
    );
}
