//! Maximum Data Rate Box (maxr) parsing and serialization.
//!
//! The Maximum Data Rate Box contains the maximum data rate over a given period.
//!
//! ```text
//! aligned(8) class hintmaxrate extends Box('maxr') {
//!    uint(32) period;    // in milliseconds
//!    uint(32) bytes;     // max bytes sent in any period 'period' long
//!                        // including RTP headers
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MaxDataRateBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"maxr");

/// Common interface for accessing MaxDataRateBox data.
pub trait MaxDataRateBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the period in milliseconds.
    fn period(&self) -> u32;

    /// Returns the max bytes sent in any period 'period' long.
    fn bytes(&self) -> u32;
}

/// A borrowing view over raw MaxDataRateBox bytes.
#[derive(Clone, Copy)]
pub struct MaxDataRateBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> MaxDataRateBoxView<'a> {
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

impl<'a> MaxDataRateBox for MaxDataRateBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn period(&self) -> u32 {
        BigEndian::read_u32(&self.data[self.header_size..self.header_size + 4])
    }

    fn bytes(&self) -> u32 {
        BigEndian::read_u32(&self.data[self.header_size + 4..self.header_size + 8])
    }
}

impl std::fmt::Debug for MaxDataRateBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaxDataRateBoxView")
            .field("period", &self.period())
            .field("bytes", &self.bytes())
            .finish()
    }
}

/// An owned representation of MaxDataRateBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaxDataRateBoxOwned {
    /// Period in milliseconds.
    pub period: u32,
    /// Max bytes sent in any period 'period' long.
    pub bytes: u32,
}

impl MaxDataRateBoxOwned {
    /// Creates a new MaxDataRateBoxOwned.
    pub fn new(period: u32, bytes: u32) -> Self {
        Self {
            period,
            bytes,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(8) + 8 // 8 + 4 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_u32::<BigEndian>(self.period)?;
        writer.write_u32::<BigEndian>(self.bytes)?;

        Ok(())
    }
}

impl Default for MaxDataRateBoxOwned {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl MaxDataRateBox for MaxDataRateBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn period(&self) -> u32 {
        self.period
    }

    fn bytes(&self) -> u32 {
        self.bytes
    }
}

impl<T: MaxDataRateBox> From<&T> for MaxDataRateBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            period: source.period(),
            bytes: source.bytes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_maxr() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes()); // 8 + 4 + 4
        data.extend_from_slice(b"maxr");
        data.extend_from_slice(&1000u32.to_be_bytes()); // period
        data.extend_from_slice(&128000u32.to_be_bytes()); // bytes
        data
    }

    #[test]
    fn parse_maxr() {
        let data = make_maxr();
        let view = MaxDataRateBoxView::new(&data).unwrap();
        assert_eq!(view.period(), 1000);
        assert_eq!(view.bytes(), 128000);
    }

    #[test]
    fn roundtrip() {
        let data = make_maxr();
        let view = MaxDataRateBoxView::new(&data).unwrap();
        let owned = MaxDataRateBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
