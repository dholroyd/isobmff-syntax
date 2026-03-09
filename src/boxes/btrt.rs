//! Bitrate Box (btrt) parsing and serialization.
//!
//! The Bitrate Box specifies the bitrate information for a sample entry.
//!
//! ```text
//! aligned(8) class BitRateBox extends Box('btrt'){
//!    unsigned int(32) bufferSizeDB;
//!    unsigned int(32) maxBitrate;
//!    unsigned int(32) avgBitrate;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for BitrateBox.
pub const BOX_TYPE: BoxCode = BoxCode::BTRT;

/// Common interface for accessing BitrateBox data.
pub trait BitrateBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the buffer size in bytes.
    fn buffer_size_db(&self) -> u32;

    /// Returns the maximum bitrate.
    fn max_bitrate(&self) -> u32;

    /// Returns the average bitrate.
    fn avg_bitrate(&self) -> u32;
}

/// A borrowing view over raw BitrateBox bytes.
#[derive(Clone, Copy)]
pub struct BitrateBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> BitrateBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 12)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> BitrateBox for BitrateBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn buffer_size_db(&self) -> u32 {
        let o = self.header_size;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn max_bitrate(&self) -> u32 {
        let o = self.header_size + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn avg_bitrate(&self) -> u32 {
        let o = self.header_size + 8;
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for BitrateBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BitrateBoxView")
            .field("buffer_size_db", &self.buffer_size_db())
            .field("max_bitrate", &self.max_bitrate())
            .field("avg_bitrate", &self.avg_bitrate())
            .finish()
    }
}

/// An owned representation of BitrateBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitrateBoxOwned {
    /// Buffer size in bytes.
    pub buffer_size_db: u32,
    /// Maximum bitrate.
    pub max_bitrate: u32,
    /// Average bitrate.
    pub avg_bitrate: u32,
}

impl BitrateBoxOwned {
    /// Creates a new BitrateBoxOwned.
    pub fn new(buffer_size_db: u32, max_bitrate: u32, avg_bitrate: u32) -> Self {
        Self {
            buffer_size_db,
            max_bitrate,
            avg_bitrate,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(12) + 12 // 8 + 4 + 4 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        writer.write_u32::<BigEndian>(self.buffer_size_db)?;
        writer.write_u32::<BigEndian>(self.max_bitrate)?;
        writer.write_u32::<BigEndian>(self.avg_bitrate)?;

        Ok(())
    }
}

impl Default for BitrateBoxOwned {
    fn default() -> Self {
        Self::new(0, 0, 0)
    }
}

impl BitrateBox for BitrateBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn buffer_size_db(&self) -> u32 {
        self.buffer_size_db
    }

    fn max_bitrate(&self) -> u32 {
        self.max_bitrate
    }

    fn avg_bitrate(&self) -> u32 {
        self.avg_bitrate
    }
}

impl<T: BitrateBox> From<&T> for BitrateBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            buffer_size_db: source.buffer_size_db(),
            max_bitrate: source.max_bitrate(),
            avg_bitrate: source.avg_bitrate(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_btrt() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"btrt");
        data.extend_from_slice(&65536u32.to_be_bytes()); // buffer_size_db
        data.extend_from_slice(&5000000u32.to_be_bytes()); // max_bitrate
        data.extend_from_slice(&2500000u32.to_be_bytes()); // avg_bitrate
        data
    }

    #[test]
    fn parse_btrt() {
        let data = make_btrt();
        let view = BitrateBoxView::new(&data).unwrap();

        assert_eq!(view.buffer_size_db(), 65536);
        assert_eq!(view.max_bitrate(), 5000000);
        assert_eq!(view.avg_bitrate(), 2500000);
    }

    #[test]
    fn roundtrip() {
        let data = make_btrt();
        let view = BitrateBoxView::new(&data).unwrap();
        let owned = BitrateBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
