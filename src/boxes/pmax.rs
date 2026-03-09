//! Largest Packet Size Box (pmax) parsing and serialization.
//!
//! The Largest Packet Size Box contains the largest packet size in bytes.
//!
//! ```text
//! aligned(8) class hintlargestpacket extends Box('pmax') {
//!    uint(32) bytes; // largest packet sent, including RTP header
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for LargestPacketSizeBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"pmax");

/// Common interface for accessing LargestPacketSizeBox data.
pub trait LargestPacketSizeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the largest packet size.
    fn max_packet_size(&self) -> u32;
}

/// A borrowing view over raw LargestPacketSizeBox bytes.
#[derive(Clone, Copy)]
pub struct LargestPacketSizeBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> LargestPacketSizeBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 4)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> LargestPacketSizeBox for LargestPacketSizeBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn max_packet_size(&self) -> u32 {
        BigEndian::read_u32(&self.data[self.header_size..self.header_size + 4])
    }
}

impl std::fmt::Debug for LargestPacketSizeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LargestPacketSizeBoxView")
            .field("max_packet_size", &self.max_packet_size())
            .finish()
    }
}

/// An owned representation of LargestPacketSizeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LargestPacketSizeBoxOwned {
    /// Largest packet size.
    pub max_packet_size: u32,
}

impl LargestPacketSizeBoxOwned {
    /// Creates a new LargestPacketSizeBoxOwned.
    pub fn new(max_packet_size: u32) -> Self {
        Self { max_packet_size }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(4) + 4 // 8 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_u32::<BigEndian>(self.max_packet_size)?;

        Ok(())
    }
}

impl Default for LargestPacketSizeBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl LargestPacketSizeBox for LargestPacketSizeBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn max_packet_size(&self) -> u32 {
        self.max_packet_size
    }
}

impl<T: LargestPacketSizeBox> From<&T> for LargestPacketSizeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            max_packet_size: source.max_packet_size(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pmax() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"pmax");
        data.extend_from_slice(&1500u32.to_be_bytes());
        data
    }

    #[test]
    fn parse_pmax() {
        let data = make_pmax();
        let view = LargestPacketSizeBoxView::new(&data).unwrap();
        assert_eq!(view.max_packet_size(), 1500);
    }

    #[test]
    fn roundtrip() {
        let data = make_pmax();
        let view = LargestPacketSizeBoxView::new(&data).unwrap();
        let owned = LargestPacketSizeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
