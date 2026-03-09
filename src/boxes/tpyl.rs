//! Total RTP Payload Bytes Box (tpyl) parsing and serialization.
//!
//! The Total RTP Payload Bytes Box contains the total bytes sent in RTP payloads.
//!
//! ```text
//! aligned(8) class hintBytesSent extends Box('tpyl') {
//!    uint(64) bytessent; // total bytes sent, not including RTP headers
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TotalRTPBytesBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"tpyl");

/// Common interface for accessing TotalRTPBytesBox data.
pub trait TotalRTPBytesBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the total bytes sent.
    fn bytes_sent(&self) -> u64;
}

/// A borrowing view over raw TotalRTPBytesBox bytes.
#[derive(Clone, Copy)]
pub struct TotalRTPBytesBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> TotalRTPBytesBoxView<'a> {
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

impl<'a> TotalRTPBytesBox for TotalRTPBytesBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn bytes_sent(&self) -> u64 {
        BigEndian::read_u64(&self.data[self.header_size..self.header_size + 8])
    }
}

impl std::fmt::Debug for TotalRTPBytesBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TotalRTPBytesBoxView")
            .field("bytes_sent", &self.bytes_sent())
            .finish()
    }
}

/// An owned representation of TotalRTPBytesBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TotalRTPBytesBoxOwned {
    /// Total bytes sent.
    pub bytes_sent: u64,
}

impl TotalRTPBytesBoxOwned {
    /// Creates a new TotalRTPBytesBoxOwned.
    pub fn new(bytes_sent: u64) -> Self {
        Self { bytes_sent }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(8) + 8 // 8 + 8
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_u64::<BigEndian>(self.bytes_sent)?;

        Ok(())
    }
}

impl Default for TotalRTPBytesBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl TotalRTPBytesBox for TotalRTPBytesBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn bytes_sent(&self) -> u64 {
        self.bytes_sent
    }
}

impl<T: TotalRTPBytesBox> From<&T> for TotalRTPBytesBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            bytes_sent: source.bytes_sent(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tpyl() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes()); // 8 + 8
        data.extend_from_slice(b"tpyl");
        data.extend_from_slice(&1000u64.to_be_bytes());
        data
    }

    #[test]
    fn parse_tpyl() {
        let data = make_tpyl();
        let view = TotalRTPBytesBoxView::new(&data).unwrap();
        assert_eq!(view.bytes_sent(), 1000);
    }

    #[test]
    fn roundtrip() {
        let data = make_tpyl();
        let view = TotalRTPBytesBoxView::new(&data).unwrap();
        let owned = TotalRTPBytesBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
