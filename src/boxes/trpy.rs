//! Total RTP Bytes Box (trpy) parsing and serialization.
//!
//! The Total RTP Bytes Box contains the total RTP bytes (including headers).
//!
//! ```text
//! aligned(8) class hintBytesSent extends Box('trpy') {
//!    uint(64) bytessent; // total bytes sent, including 12-byte RTP headers
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TotalRTPBytesWithHeaderBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"trpy");

/// Common interface for accessing TotalRTPBytesWithHeaderBox data.
pub trait TotalRTPBytesWithHeaderBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the total RTP bytes.
    fn rtp_bytes(&self) -> u64;
}

/// A borrowing view over raw TotalRTPBytesWithHeaderBox bytes.
#[derive(Clone, Copy)]
pub struct TotalRTPBytesWithHeaderBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> TotalRTPBytesWithHeaderBoxView<'a> {
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

impl<'a> TotalRTPBytesWithHeaderBox for TotalRTPBytesWithHeaderBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn rtp_bytes(&self) -> u64 {
        BigEndian::read_u64(&self.data[self.header_size..self.header_size + 8])
    }
}

impl std::fmt::Debug for TotalRTPBytesWithHeaderBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TotalRTPBytesWithHeaderBoxView")
            .field("rtp_bytes", &self.rtp_bytes())
            .finish()
    }
}

/// An owned representation of TotalRTPBytesWithHeaderBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TotalRTPBytesWithHeaderBoxOwned {
    /// Total RTP bytes.
    pub rtp_bytes: u64,
}

impl TotalRTPBytesWithHeaderBoxOwned {
    /// Creates a new TotalRTPBytesWithHeaderBoxOwned.
    pub fn new(rtp_bytes: u64) -> Self {
        Self { rtp_bytes }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(8) + 8 // 8 + 8
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_u64::<BigEndian>(self.rtp_bytes)?;

        Ok(())
    }
}

impl Default for TotalRTPBytesWithHeaderBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl TotalRTPBytesWithHeaderBox for TotalRTPBytesWithHeaderBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn rtp_bytes(&self) -> u64 {
        self.rtp_bytes
    }
}

impl<T: TotalRTPBytesWithHeaderBox> From<&T> for TotalRTPBytesWithHeaderBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            rtp_bytes: source.rtp_bytes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trpy() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes()); // 8 + 8
        data.extend_from_slice(b"trpy");
        data.extend_from_slice(&2000u64.to_be_bytes());
        data
    }

    #[test]
    fn parse_trpy() {
        let data = make_trpy();
        let view = TotalRTPBytesWithHeaderBoxView::new(&data).unwrap();
        assert_eq!(view.rtp_bytes(), 2000);
    }

    #[test]
    fn roundtrip() {
        let data = make_trpy();
        let view = TotalRTPBytesWithHeaderBoxView::new(&data).unwrap();
        let owned = TotalRTPBytesWithHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
