//! Error types for parsing ISOBMFF structures.

use mp4ra_rust::BoxCode;
use std::fmt;

/// Errors that can occur when parsing box data.
#[derive(Debug)]
pub enum ParseError {
    /// The box type in the header does not match the expected type.
    InvalidBoxType {
        expected: BoxCode,
        found: BoxCode,
    },
    /// The buffer is too short to contain the expected data.
    BufferTooShort {
        expected: usize,
        found: usize,
    },
    /// The version field contains an unsupported value.
    InvalidVersion(u8),
    /// The declared size in the box header does not match the buffer length.
    SizeMismatch {
        declared: u64,
        actual: usize,
    },
    /// The flags field contains invalid or unsupported values.
    InvalidFlags {
        flags: u32,
        reason: &'static str,
    },
    /// The entry count is invalid (e.g., too large for available data).
    InvalidEntryCount {
        count: u32,
        max_possible: u32,
    },
    /// The version is not supported for this box type.
    UnsupportedVersion {
        version: u8,
        supported: &'static [u8],
    },
    /// A required child box is missing from a container box.
    MissingRequiredBox {
        box_type: BoxCode,
    },
    /// Unexpected end of data while parsing.
    UnexpectedEndOfData {
        context: &'static str,
    },
    /// A string field contains invalid encoding.
    InvalidStringEncoding {
        context: &'static str,
    },
    /// Invalid field size configuration.
    InvalidFieldSize {
        field: &'static str,
        size: u8,
    },
    /// The box size is invalid (e.g., smaller than header size).
    InvalidBoxSize {
        box_type: BoxCode,
        size: u64,
    },
    /// The box is too large to load into memory.
    BoxTooLargeToLoad {
        size: u64,
        max: u64,
    },
    /// The container nesting depth exceeds the maximum allowed limit.
    NestingTooDeep {
        depth: usize,
        max: usize,
    },
    /// A null-terminated string field is missing its null terminator.
    MissingNullTerminator {
        field: &'static str,
    },
    /// A box's declared size exceeds the remaining bytes in its containing scope.
    ///
    /// The parent container's size is treated as the authoritative boundary.
    /// See the [`streaming`](crate::streaming) module documentation for the
    /// rationale behind this design decision.
    BoxExceedsScope {
        box_type: BoxCode,
        /// The size declared in the box header.
        box_size: u64,
        /// The number of bytes remaining in the containing scope.
        remaining: u64,
        /// The absolute file offset where the box header starts.
        box_offset: u64,
    },
}

/// Validates that `data[entries_offset..]` can hold `count` entries of
/// `entry_size` bytes each.
///
/// # Errors
///
/// Returns [`ParseError::InvalidEntryCount`] if the declared count exceeds
/// what the remaining buffer can hold.
pub fn validate_entry_count(
    data: &[u8],
    entries_offset: usize,
    count: u32,
    entry_size: usize,
) -> Result<(), ParseError> {
    let max_entries = (data.len() - entries_offset) / entry_size;
    if count as usize > max_entries {
        return Err(ParseError::InvalidEntryCount {
            count,
            max_possible: max_entries as u32,
        });
    }
    Ok(())
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidBoxType { expected, found } => {
                write!(
                    f,
                    "invalid box type: expected '{}', found '{}'",
                    expected, found
                )
            }
            ParseError::BufferTooShort { expected, found } => {
                write!(
                    f,
                    "buffer too short: expected at least {} bytes, found {}",
                    expected, found
                )
            }
            ParseError::InvalidVersion(v) => {
                write!(f, "invalid version: {}", v)
            }
            ParseError::SizeMismatch { declared, actual } => {
                write!(
                    f,
                    "size mismatch: header declares {} bytes, buffer contains {}",
                    declared, actual
                )
            }
            ParseError::InvalidFlags { flags, reason } => {
                write!(f, "invalid flags 0x{:06x}: {}", flags, reason)
            }
            ParseError::InvalidEntryCount { count, max_possible } => {
                write!(
                    f,
                    "invalid entry count: {} entries declared but at most {} possible",
                    count, max_possible
                )
            }
            ParseError::UnsupportedVersion { version, supported } => {
                write!(
                    f,
                    "unsupported version {}, supported versions: {:?}",
                    version, supported
                )
            }
            ParseError::MissingRequiredBox { box_type } => {
                write!(f, "missing required box '{}'", box_type)
            }
            ParseError::UnexpectedEndOfData { context } => {
                write!(f, "unexpected end of data while parsing {}", context)
            }
            ParseError::InvalidStringEncoding { context } => {
                write!(f, "invalid string encoding in {}", context)
            }
            ParseError::InvalidFieldSize { field, size } => {
                write!(f, "invalid size {} for field '{}'", size, field)
            }
            ParseError::InvalidBoxSize { box_type, size } => {
                write!(f, "invalid box size {} for box '{}'", size, box_type)
            }
            ParseError::BoxTooLargeToLoad { size, max } => {
                write!(
                    f,
                    "box size {} exceeds maximum load size {}",
                    size, max
                )
            }
            ParseError::NestingTooDeep { depth, max } => {
                write!(
                    f,
                    "container nesting depth {} exceeds maximum of {}",
                    depth, max
                )
            }
            ParseError::MissingNullTerminator { field } => {
                write!(f, "missing null terminator in field '{}'", field)
            }
            ParseError::BoxExceedsScope {
                box_type,
                box_size,
                remaining,
                box_offset,
            } => {
                write!(
                    f,
                    "box '{}' at offset {} declares size {} but only {} bytes remain in the containing scope",
                    box_type, box_offset, box_size, remaining
                )
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Errors from reading and parsing boxes from a stream.
///
/// This combines I/O errors (inability to read data) with parse errors
/// (malformed data). [`SliceBoxIterator`](crate::streaming::SliceBoxIterator)
/// methods return [`ParseError`] directly since no I/O is involved.
#[derive(Debug)]
pub enum BoxReadError {
    /// The data was read successfully but is malformed.
    Parse(ParseError),
    /// An I/O error prevented reading the data.
    Io(std::io::Error),
}

impl From<std::io::Error> for BoxReadError {
    fn from(err: std::io::Error) -> Self {
        BoxReadError::Io(err)
    }
}

impl From<ParseError> for BoxReadError {
    fn from(err: ParseError) -> Self {
        BoxReadError::Parse(err)
    }
}

impl fmt::Display for BoxReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BoxReadError::Parse(err) => write!(f, "{}", err),
            BoxReadError::Io(err) => write!(f, "I/O error: {}", err),
        }
    }
}

impl std::error::Error for BoxReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BoxReadError::Parse(err) => Some(err),
            BoxReadError::Io(err) => Some(err),
        }
    }
}
