//! Number of RTP Packets Box (nump) parsing and serialization.
//!
//! The Number of RTP Packets Box contains the total number of RTP packets sent.
//!
//! ```text
//! aligned(8) class hintPacketsSent extends Box('nump') {
//!    uint(64) packetssent; // total packets sent
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for NumRTPPacketsBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"nump");

/// Common interface for accessing NumRTPPacketsBox data.
pub trait NumRTPPacketsBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the number of packets sent.
    fn packets_sent(&self) -> u64;
}

/// A borrowing view over raw NumRTPPacketsBox bytes.
#[derive(Clone, Copy)]
pub struct NumRTPPacketsBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> NumRTPPacketsBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 8)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> NumRTPPacketsBox for NumRTPPacketsBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn packets_sent(&self) -> u64 {
        BigEndian::read_u64(&self.data[self.header_size..self.header_size + 8])
    }
}

impl std::fmt::Debug for NumRTPPacketsBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NumRTPPacketsBoxView")
            .field("packets_sent", &self.packets_sent())
            .finish()
    }
}

/// An owned representation of NumRTPPacketsBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumRTPPacketsBoxOwned {
    /// Total packets sent.
    pub packets_sent: u64,
}

impl NumRTPPacketsBoxOwned {
    /// Creates a new NumRTPPacketsBoxOwned.
    pub fn new(packets_sent: u64) -> Self {
        Self { packets_sent }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(8) + 8 // 8 + 8
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_u64::<BigEndian>(self.packets_sent)?;

        Ok(())
    }
}

impl Default for NumRTPPacketsBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl NumRTPPacketsBox for NumRTPPacketsBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn packets_sent(&self) -> u64 {
        self.packets_sent
    }
}

impl<T: NumRTPPacketsBox> From<&T> for NumRTPPacketsBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            packets_sent: source.packets_sent(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_nump() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes()); // 8 + 8
        data.extend_from_slice(b"nump");
        data.extend_from_slice(&100u64.to_be_bytes());
        data
    }

    #[test]
    fn parse_nump() {
        let data = make_nump();
        let view = NumRTPPacketsBoxView::new(&data).unwrap();
        assert_eq!(view.packets_sent(), 100);
    }

    #[test]
    fn roundtrip() {
        let data = make_nump();
        let view = NumRTPPacketsBoxView::new(&data).unwrap();
        let owned = NumRTPPacketsBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
