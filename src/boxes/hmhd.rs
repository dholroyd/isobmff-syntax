//! Hint Media Header Box (hmhd) parsing and serialization.
//!
//! The Hint Media Header Box contains general information for hint tracks.
//!
//! ```text
//! aligned(8) class HintMediaHeaderBox
//!    extends FullBox('hmhd', version = 0, 0) {
//!    unsigned int(16) maxPDUsize;
//!    unsigned int(16) avgPDUsize;
//!    unsigned int(32) maxbitrate;
//!    unsigned int(32) avgbitrate;
//!    unsigned int(32) reserved;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for HintMediaHeaderBox.
pub const BOX_TYPE: BoxCode = BoxCode::HMHD;

/// Common interface for accessing HintMediaHeaderBox data.
pub trait HintMediaHeaderBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the maximum PDU size.
    fn max_pdu_size(&self) -> u16;

    /// Returns the average PDU size.
    fn avg_pdu_size(&self) -> u16;

    /// Returns the maximum bit rate.
    fn max_bitrate(&self) -> u32;

    /// Returns the average bit rate.
    fn avg_bitrate(&self) -> u32;
}

/// A borrowing view over raw HintMediaHeaderBox bytes.
#[derive(Clone, Copy)]
pub struct HintMediaHeaderBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> HintMediaHeaderBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 16)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    #[inline]
    fn payload_offset(&self) -> usize {
        self.fullbox_offset + 4
    }
}

impl HintMediaHeaderBox for HintMediaHeaderBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.data[self.fullbox_offset]
    }

    fn flags(&self) -> u32 {
        BigEndian::read_u24(&self.data[self.fullbox_offset + 1..self.fullbox_offset + 4])
    }

    fn max_pdu_size(&self) -> u16 {
        let o = self.payload_offset();
        BigEndian::read_u16(&self.data[o..o + 2])
    }

    fn avg_pdu_size(&self) -> u16 {
        let o = self.payload_offset();
        BigEndian::read_u16(&self.data[o + 2..o + 4])
    }

    fn max_bitrate(&self) -> u32 {
        let o = self.payload_offset();
        BigEndian::read_u32(&self.data[o + 4..o + 8])
    }

    fn avg_bitrate(&self) -> u32 {
        let o = self.payload_offset();
        BigEndian::read_u32(&self.data[o + 8..o + 12])
    }
}

impl std::fmt::Debug for HintMediaHeaderBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HintMediaHeaderBoxView")
            .field("box_size", &self.box_size())
            .field("max_pdu_size", &self.max_pdu_size())
            .field("avg_pdu_size", &self.avg_pdu_size())
            .field("max_bitrate", &self.max_bitrate())
            .field("avg_bitrate", &self.avg_bitrate())
            .finish()
    }
}

/// An owned representation of HintMediaHeaderBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct HintMediaHeaderBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Maximum PDU size.
    pub max_pdu_size: u16,
    /// Average PDU size.
    pub avg_pdu_size: u16,
    /// Maximum bit rate.
    pub max_bitrate: u32,
    /// Average bit rate.
    pub avg_bitrate: u32,
}

impl HintMediaHeaderBoxOwned {
    /// Creates a new HintMediaHeaderBoxOwned with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(16) + 16 // 8 + 4 + 16
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, 0, self.flags)?;
        writer.write_u16::<BigEndian>(self.max_pdu_size)?;
        writer.write_u16::<BigEndian>(self.avg_pdu_size)?;
        writer.write_u32::<BigEndian>(self.max_bitrate)?;
        writer.write_u32::<BigEndian>(self.avg_bitrate)?;
        writer.write_u32::<BigEndian>(0)?; // reserved
        Ok(())
    }
}


impl HintMediaHeaderBox for HintMediaHeaderBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        0
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn max_pdu_size(&self) -> u16 {
        self.max_pdu_size
    }

    fn avg_pdu_size(&self) -> u16 {
        self.avg_pdu_size
    }

    fn max_bitrate(&self) -> u32 {
        self.max_bitrate
    }

    fn avg_bitrate(&self) -> u32 {
        self.avg_bitrate
    }
}

impl<T: HintMediaHeaderBox> From<&T> for HintMediaHeaderBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            max_pdu_size: source.max_pdu_size(),
            avg_pdu_size: source.avg_pdu_size(),
            max_bitrate: source.max_bitrate(),
            avg_bitrate: source.avg_bitrate(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hmhd() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&28u32.to_be_bytes());
        data.extend_from_slice(b"hmhd");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1400u16.to_be_bytes()); // max_pdu_size
        data.extend_from_slice(&1200u16.to_be_bytes()); // avg_pdu_size
        data.extend_from_slice(&128000u32.to_be_bytes()); // max_bitrate
        data.extend_from_slice(&96000u32.to_be_bytes()); // avg_bitrate
        data.extend_from_slice(&0u32.to_be_bytes()); // reserved
        data
    }

    #[test]
    fn parse_hmhd() {
        let data = make_hmhd();
        let view = HintMediaHeaderBoxView::new(&data).unwrap();

        assert_eq!(view.max_pdu_size(), 1400);
        assert_eq!(view.avg_pdu_size(), 1200);
        assert_eq!(view.max_bitrate(), 128000);
        assert_eq!(view.avg_bitrate(), 96000);
    }

    #[test]
    fn roundtrip() {
        let data = make_hmhd();
        let view = HintMediaHeaderBoxView::new(&data).unwrap();
        let owned = HintMediaHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
