//! Number of Packets Box (npck) parsing and serialization.
//!
//! The Number of Packets Box contains the total number of packets.
//!
//! ```text
//! aligned(8) class hintPacketsSent extends Box('npck') {
//!    uint(32) packetssent; // total packets sent
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for NumPacketsBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"npck");

/// Common interface for accessing NumPacketsBox data.
pub trait NumPacketsBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the number of packets.
    fn num_packets(&self) -> u32;
}

/// A borrowing view over raw NumPacketsBox bytes.
#[derive(Clone, Copy)]
pub struct NumPacketsBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> NumPacketsBoxView<'a> {
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

impl<'a> NumPacketsBox for NumPacketsBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn num_packets(&self) -> u32 {
        BigEndian::read_u32(&self.data[self.header_size..self.header_size + 4])
    }
}

impl std::fmt::Debug for NumPacketsBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NumPacketsBoxView")
            .field("num_packets", &self.num_packets())
            .finish()
    }
}

/// An owned representation of NumPacketsBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumPacketsBoxOwned {
    /// Number of packets.
    pub num_packets: u32,
}

impl NumPacketsBoxOwned {
    /// Creates a new NumPacketsBoxOwned.
    pub fn new(num_packets: u32) -> Self {
        Self { num_packets }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(4) + 4 // 8 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_u32::<BigEndian>(self.num_packets)?;

        Ok(())
    }
}

impl Default for NumPacketsBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl NumPacketsBox for NumPacketsBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn num_packets(&self) -> u32 {
        self.num_packets
    }
}

impl<T: NumPacketsBox> From<&T> for NumPacketsBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            num_packets: source.num_packets(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_npck() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"npck");
        data.extend_from_slice(&50u32.to_be_bytes());
        data
    }

    #[test]
    fn parse_npck() {
        let data = make_npck();
        let view = NumPacketsBoxView::new(&data).unwrap();
        assert_eq!(view.num_packets(), 50);
    }

    #[test]
    fn roundtrip() {
        let data = make_npck();
        let view = NumPacketsBoxView::new(&data).unwrap();
        let owned = NumPacketsBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
