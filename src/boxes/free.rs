//! Free Space Box (free/skip) parsing and serialization.
//!
//! Free space boxes are used to reserve space in a file or to fill gaps.
//! The 'free' and 'skip' box types are functionally identical.
//!
//! ```text
//! aligned(8) class FreeSpaceBox extends Box(free_type) {
//!    unsigned int(8) data[];
//! }
//! ```

use crate::error::ParseError;
use crate::header::{header_size_for_payload, write_box_header, BoxHeader};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for FreeSpaceBox.
pub const BOX_TYPE_FREE: BoxCode = BoxCode::FREE;
pub const BOX_TYPE_SKIP: BoxCode = BoxCode::SKIP;

/// Common interface for accessing FreeSpaceBox data.
pub trait FreeSpaceBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type (free or skip).
    fn box_type(&self) -> BoxCode;

    /// Returns the size of the free space (payload size).
    fn free_space_size(&self) -> u64;
}

/// A borrowing view over raw FreeSpaceBox bytes.
#[derive(Clone, Copy)]
pub struct FreeSpaceBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> FreeSpaceBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;

        if header.box_type != BOX_TYPE_FREE && header.box_type != BOX_TYPE_SKIP {
            return Err(ParseError::InvalidBoxType {
                expected: BOX_TYPE_FREE,
                found: header.box_type,
            });
        }

        if header.size != data.len() as u64 {
            return Err(ParseError::SizeMismatch {
                declared: header.size,
                actual: data.len(),
            });
        }

        Ok(Self { data, header })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the padding data (usually zeros or garbage).
    #[inline]
    pub fn padding_data(&self) -> &'a [u8] {
        &self.data[self.header.header_size as usize..]
    }
}

impl FreeSpaceBox for FreeSpaceBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.header.size
    }

    fn box_type(&self) -> BoxCode {
        self.header.box_type
    }

    fn free_space_size(&self) -> u64 {
        self.header.payload_size()
    }
}

impl std::fmt::Debug for FreeSpaceBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FreeSpaceBoxView")
            .field("box_size", &self.box_size())
            .field("box_type", &String::from_utf8_lossy(&self.header.box_type_bytes()))
            .field("free_space_size", &self.free_space_size())
            .finish()
    }
}

/// An owned representation of FreeSpaceBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FreeSpaceBoxOwned {
    /// The box type (free or skip).
    pub box_type: BoxCode,
    /// The size of the free space in bytes.
    pub size: u64,
}

impl FreeSpaceBoxOwned {
    /// Creates a new FreeSpaceBoxOwned with the given size.
    pub fn new(size: u64) -> Self {
        Self {
            box_type: BOX_TYPE_FREE,
            size,
        }
    }

    /// Creates a new skip box with the given size.
    pub fn skip(size: u64) -> Self {
        Self {
            box_type: BOX_TYPE_SKIP,
            size,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(self.size) + self.size
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let total_size = self.serialized_size();
        write_box_header(writer, total_size, self.box_type)?;
        // Write zeros as padding in chunks to avoid large allocations
        let chunk = [0u8; 8192];
        let mut remaining = self.size;
        while remaining > 0 {
            let n = (remaining as usize).min(chunk.len());
            writer.write_all(&chunk[..n])?;
            remaining -= n as u64;
        }
        Ok(())
    }
}

impl Default for FreeSpaceBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl FreeSpaceBox for FreeSpaceBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        self.box_type
    }

    fn free_space_size(&self) -> u64 {
        self.size
    }
}

impl<T: FreeSpaceBox> From<&T> for FreeSpaceBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            box_type: source.box_type(),
            size: source.free_space_size(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_free(size: usize) -> Vec<u8> {
        let total = 8 + size;
        let mut data = Vec::with_capacity(total);
        data.extend_from_slice(&(total as u32).to_be_bytes());
        data.extend_from_slice(b"free");
        data.extend_from_slice(&vec![0u8; size]);
        data
    }

    #[test]
    fn parse_free() {
        let data = make_free(16);
        let view = FreeSpaceBoxView::new(&data).unwrap();

        assert_eq!(view.box_size(), 24);
        assert_eq!(view.box_type(), BOX_TYPE_FREE);
        assert_eq!(view.free_space_size(), 16);
    }

    #[test]
    fn parse_skip() {
        let mut data = make_free(8);
        data[4..8].copy_from_slice(b"skip");
        let view = FreeSpaceBoxView::new(&data).unwrap();

        assert_eq!(view.box_type(), BOX_TYPE_SKIP);
    }

    #[test]
    fn roundtrip() {
        let data = make_free(32);
        let view = FreeSpaceBoxView::new(&data).unwrap();
        let owned = FreeSpaceBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
