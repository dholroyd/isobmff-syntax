//! Box header parsing utilities.
//!
//! This module provides types for parsing ISOBMFF box headers.

use crate::error::ParseError;
use byteorder::{BigEndian, ByteOrder};
use mp4ra_rust::BoxCode;

/// The minimum size of a basic box header (size + type).
pub const MIN_HEADER_SIZE: usize = 8;

/// The size of an extended box header (size + type + largesize).
pub const EXTENDED_HEADER_SIZE: usize = 16;

/// A parsed box header containing size and type information.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoxHeader {
    /// The total size of the box including the header.
    pub size: u64,
    /// The box type code.
    pub box_type: BoxCode,
    /// The size of the header itself (8 or 16 bytes).
    pub header_size: u8,
}

impl BoxHeader {
    /// Parses a box header from the given data.
    ///
    /// Returns the parsed header. The `available_size` parameter should be
    /// the total number of bytes available (used when size == 0 means
    /// "box extends to end of file").
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is too short to contain a valid header.
    #[inline]
    pub fn parse(data: &[u8], available_size: usize) -> Result<Self, ParseError> {
        if data.len() < MIN_HEADER_SIZE {
            return Err(ParseError::BufferTooShort {
                expected: MIN_HEADER_SIZE,
                found: data.len(),
            });
        }

        let size_field = BigEndian::read_u32(&data[0..4]);
        let box_type = BoxCode::new([data[4], data[5], data[6], data[7]]);

        let (size, header_size) = match size_field {
            0 => {
                // Box extends to end of available data
                (available_size as u64, 8)
            }
            1 => {
                // Extended size in next 8 bytes
                if data.len() < EXTENDED_HEADER_SIZE {
                    return Err(ParseError::BufferTooShort {
                        expected: EXTENDED_HEADER_SIZE,
                        found: data.len(),
                    });
                }
                let largesize = BigEndian::read_u64(&data[8..16]);
                if largesize < EXTENDED_HEADER_SIZE as u64 {
                    return Err(ParseError::InvalidBoxSize {
                        box_type,
                        size: largesize,
                    });
                }
                (largesize, 16)
            }
            _ => {
                if (size_field as usize) < MIN_HEADER_SIZE {
                    return Err(ParseError::InvalidBoxSize {
                        box_type,
                        size: size_field as u64,
                    });
                }
                (size_field as u64, 8)
            }
        };

        Ok(Self {
            size,
            box_type,
            header_size,
        })
    }

    /// Validates common box header fields against the provided data.
    ///
    /// Checks that:
    /// - The box type matches `expected_type`
    /// - The declared size matches `data.len()`
    /// - The buffer is large enough for `header_size + min_payload`
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::InvalidBoxType`], [`ParseError::SizeMismatch`],
    /// or [`ParseError::BufferTooShort`] on failure.
    pub fn validate(
        &self,
        data: &[u8],
        expected_type: BoxCode,
        min_payload: usize,
    ) -> Result<(), ParseError> {
        if self.box_type != expected_type {
            return Err(ParseError::InvalidBoxType {
                expected: expected_type,
                found: self.box_type,
            });
        }

        if self.size != data.len() as u64 {
            return Err(ParseError::SizeMismatch {
                declared: self.size,
                actual: data.len(),
            });
        }

        let expected_size = self.header_size as usize + min_payload;
        if data.len() < expected_size {
            return Err(ParseError::BufferTooShort {
                expected: expected_size,
                found: data.len(),
            });
        }

        Ok(())
    }

    /// Returns the box type as a 4-byte array.
    #[inline]
    pub fn box_type_bytes(&self) -> [u8; 4] {
        self.box_type.0 .0
    }

    /// Returns the size of the box payload (total size minus header size).
    #[inline]
    pub fn payload_size(&self) -> u64 {
        self.size.saturating_sub(self.header_size as u64)
    }

    /// Returns true if this box uses extended (64-bit) size.
    #[inline]
    pub fn has_extended_size(&self) -> bool {
        self.header_size == 16
    }
}

/// A parsed FullBox header containing version and flags in addition to size and type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FullBoxHeader {
    /// The basic box header.
    pub box_header: BoxHeader,
    /// The version of the box (0-255).
    pub version: u8,
    /// The flags (24-bit value).
    pub flags: u32,
}

impl FullBoxHeader {
    /// Parses a FullBox header from the given data.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is too short.
    #[inline]
    pub fn parse(data: &[u8], available_size: usize) -> Result<Self, ParseError> {
        let box_header = BoxHeader::parse(data, available_size)?;

        let fullbox_start = box_header.header_size as usize;
        if data.len() < fullbox_start + 4 {
            return Err(ParseError::BufferTooShort {
                expected: fullbox_start + 4,
                found: data.len(),
            });
        }

        let version = data[fullbox_start];
        let flags = BigEndian::read_u24(&data[fullbox_start + 1..fullbox_start + 4]);

        Ok(Self {
            box_header,
            version,
            flags,
        })
    }

    /// Validates common FullBox header fields against the provided data.
    ///
    /// Checks that:
    /// - The box type matches `expected_type`
    /// - The declared size matches `data.len()`
    /// - If `max_version` is `Some(n)`, the version is at most `n`
    /// - The buffer is large enough for the header + version/flags + `min_payload`
    ///
    /// Returns the fullbox offset (the byte offset to the version/flags field)
    /// on success.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::InvalidBoxType`], [`ParseError::SizeMismatch`],
    /// [`ParseError::InvalidVersion`], or [`ParseError::BufferTooShort`] on failure.
    pub fn validate(
        &self,
        data: &[u8],
        expected_type: BoxCode,
        max_version: Option<u8>,
        min_payload: usize,
    ) -> Result<usize, ParseError> {
        if self.box_type() != expected_type {
            return Err(ParseError::InvalidBoxType {
                expected: expected_type,
                found: self.box_header.box_type,
            });
        }

        if self.size() != data.len() as u64 {
            return Err(ParseError::SizeMismatch {
                declared: self.size(),
                actual: data.len(),
            });
        }

        if let Some(max) = max_version
            && self.version > max
        {
            return Err(ParseError::InvalidVersion(self.version));
        }

        let fullbox_offset = self.box_header.header_size as usize;
        let expected_size = fullbox_offset + 4 + min_payload;
        if data.len() < expected_size {
            return Err(ParseError::BufferTooShort {
                expected: expected_size,
                found: data.len(),
            });
        }

        Ok(fullbox_offset)
    }

    /// Returns the total header size including version and flags.
    #[inline]
    pub fn total_header_size(&self) -> usize {
        self.box_header.header_size as usize + 4
    }

    /// Returns the offset to the payload (after version/flags).
    #[inline]
    pub fn payload_offset(&self) -> usize {
        self.total_header_size()
    }

    /// Returns the size of the box payload (total size minus full header size).
    #[inline]
    pub fn payload_size(&self) -> u64 {
        self.box_header
            .size
            .saturating_sub(self.total_header_size() as u64)
    }

    /// Returns the box type.
    #[inline]
    pub fn box_type(&self) -> BoxCode {
        self.box_header.box_type
    }

    /// Returns the total box size.
    #[inline]
    pub fn size(&self) -> u64 {
        self.box_header.size
    }
}

/// Returns the box header size (8 or 16) needed for a given payload size.
///
/// If the total box size (header + payload) exceeds `u32::MAX`, a 16-byte
/// extended header is required; otherwise an 8-byte header suffices.
#[inline]
pub fn header_size_for_payload(payload_size: u64) -> u64 {
    let needs_extended = payload_size
        .checked_add(MIN_HEADER_SIZE as u64)
        .is_none_or(|total| total > u32::MAX as u64);
    if needs_extended {
        EXTENDED_HEADER_SIZE as u64
    } else {
        MIN_HEADER_SIZE as u64
    }
}

/// Returns the full box header size (12 or 20) needed for a given payload size.
///
/// `payload_size` should **not** include the 4-byte version/flags field;
/// those bytes are accounted for internally.
#[inline]
pub fn fullbox_header_size_for_payload(payload_size: u64) -> u64 {
    // The +4 accounts for the version/flags field that FullBox adds.
    // If payload_size + 4 overflows u64, the payload vastly exceeds u32::MAX,
    // so using u64::MAX as the fallback is safe: header_size_for_payload will
    // select the extended (16-byte) header, which is the correct choice for
    // any payload that cannot fit in a standard 32-bit size field.
    header_size_for_payload(payload_size.saturating_add(4)) + 4
}

/// Writes a box header to a writer.
pub fn write_box_header<W: std::io::Write>(
    writer: &mut W,
    size: u64,
    box_type: BoxCode,
) -> std::io::Result<()> {
    use byteorder::WriteBytesExt;

    if size > u32::MAX as u64 {
        // Extended size
        writer.write_u32::<BigEndian>(1)?;
        writer.write_all(&box_type.0 .0)?;
        writer.write_u64::<BigEndian>(size)?;
    } else {
        writer.write_u32::<BigEndian>(size as u32)?;
        writer.write_all(&box_type.0 .0)?;
    }
    Ok(())
}

/// Writes a FullBox header to a writer.
pub fn write_fullbox_header<W: std::io::Write>(
    writer: &mut W,
    size: u64,
    box_type: BoxCode,
    version: u8,
    flags: u32,
) -> std::io::Result<()> {
    use byteorder::WriteBytesExt;

    write_box_header(writer, size, box_type)?;
    writer.write_u8(version)?;
    writer.write_u24::<BigEndian>(flags)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_header() {
        let data = [
            0x00, 0x00, 0x00, 0x20, // size = 32
            b'm', b'o', b'o', b'v', // type = moov
        ];
        let header = BoxHeader::parse(&data, data.len()).unwrap();
        assert_eq!(header.size, 32);
        assert_eq!(header.box_type, BoxCode::MOOV);
        assert_eq!(header.header_size, 8);
    }

    #[test]
    fn parse_extended_header() {
        let data = [
            0x00, 0x00, 0x00, 0x01, // size = 1 (extended)
            b'm', b'd', b'a', b't', // type = mdat
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, // largesize = 4GB
        ];
        let header = BoxHeader::parse(&data, data.len()).unwrap();
        assert_eq!(header.size, 0x0000_0001_0000_0000);
        assert_eq!(header.box_type, BoxCode::MDAT);
        assert_eq!(header.header_size, 16);
    }

    #[test]
    fn parse_zero_size() {
        let data = [
            0x00, 0x00, 0x00, 0x00, // size = 0 (extends to EOF)
            b'f', b'r', b'e', b'e', // type = free
        ];
        let header = BoxHeader::parse(&data, 1000).unwrap();
        assert_eq!(header.size, 1000);
        assert_eq!(header.header_size, 8);
    }

    #[test]
    fn parse_fullbox_header() {
        let data = [
            0x00, 0x00, 0x00, 0x6C, // size = 108
            b'm', b'v', b'h', b'd', // type = mvhd
            0x00,             // version = 0
            0x00, 0x00, 0x00, // flags = 0
        ];
        let header = FullBoxHeader::parse(&data, data.len()).unwrap();
        assert_eq!(header.box_header.size, 108);
        assert_eq!(header.box_header.box_type, BoxCode::MVHD);
        assert_eq!(header.version, 0);
        assert_eq!(header.flags, 0);
        assert_eq!(header.total_header_size(), 12);
    }

    #[test]
    fn write_basic_header() {
        let mut buf = Vec::new();
        write_box_header(&mut buf, 32, BoxCode::MOOV).unwrap();
        assert_eq!(buf, [0x00, 0x00, 0x00, 0x20, b'm', b'o', b'o', b'v']);
    }

    #[test]
    fn write_extended_header() {
        let mut buf = Vec::new();
        write_box_header(&mut buf, 0x0000_0001_0000_0000, BoxCode::MDAT).unwrap();
        assert_eq!(
            buf,
            [
                0x00, 0x00, 0x00, 0x01, // size = 1 (extended)
                b'm', b'd', b'a', b't', // type = mdat
                0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, // largesize
            ]
        );
    }

    #[test]
    fn test_write_fullbox_header() {
        let mut buf = Vec::new();
        super::write_fullbox_header(&mut buf, 108, BoxCode::MVHD, 0, 0).unwrap();
        assert_eq!(
            buf,
            [
                0x00, 0x00, 0x00, 0x6C, // size = 108
                b'm', b'v', b'h', b'd', // type = mvhd
                0x00,             // version = 0
                0x00, 0x00, 0x00, // flags = 0
            ]
        );
    }

    #[test]
    fn header_size_for_small_payload() {
        // payload=100 => total=108 => fits in u32 => 8-byte header
        assert_eq!(header_size_for_payload(100), 8);
    }

    #[test]
    fn header_size_at_boundary() {
        // payload = u32::MAX - 8 => total = u32::MAX exactly => fits in u32 => 8-byte header
        let payload = u32::MAX as u64 - MIN_HEADER_SIZE as u64;
        assert_eq!(header_size_for_payload(payload), 8);

        // payload = u32::MAX - 7 => total = u32::MAX + 1 => needs extended => 16-byte header
        let payload = u32::MAX as u64 - MIN_HEADER_SIZE as u64 + 1;
        assert_eq!(header_size_for_payload(payload), 16);
    }

    #[test]
    fn header_size_for_large_payload() {
        assert_eq!(header_size_for_payload(u64::MAX / 2), 16);
    }

    #[test]
    fn fullbox_header_size_for_small_payload() {
        // payload=80 => total with fullbox header = 12+80=92 => 12
        assert_eq!(fullbox_header_size_for_payload(80), 12);
    }

    #[test]
    fn fullbox_header_size_at_boundary() {
        // payload (excl v/f) = u32::MAX - 12 => total = u32::MAX exactly => 12-byte fullbox header
        let payload = u32::MAX as u64 - 12;
        assert_eq!(fullbox_header_size_for_payload(payload), 12);

        // payload (excl v/f) = u32::MAX - 11 => total = u32::MAX + 1 => 20-byte fullbox header
        let payload = u32::MAX as u64 - 11;
        assert_eq!(fullbox_header_size_for_payload(payload), 20);
    }
}
