//! Repeated Data Size Box (drep) parsing and serialization.
//!
//! The Repeated Data Size Box contains the bytes of repeated data.
//!
//! ```text
//! aligned(8) class hintrepeatedBytesSent extends Box('drep') {
//!    uint(64) bytessent; // total bytes in repeated packets
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for RepeatedDataSizeBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"drep");

/// Common interface for accessing RepeatedDataSizeBox data.
pub trait RepeatedDataSizeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the repeated bytes.
    fn repeated_bytes(&self) -> u64;
}

/// A borrowing view over raw RepeatedDataSizeBox bytes.
#[derive(Clone, Copy)]
pub struct RepeatedDataSizeBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> RepeatedDataSizeBoxView<'a> {
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

impl<'a> RepeatedDataSizeBox for RepeatedDataSizeBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn repeated_bytes(&self) -> u64 {
        BigEndian::read_u64(&self.data[self.header_size..self.header_size + 8])
    }
}

impl std::fmt::Debug for RepeatedDataSizeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepeatedDataSizeBoxView")
            .field("repeated_bytes", &self.repeated_bytes())
            .finish()
    }
}

/// An owned representation of RepeatedDataSizeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepeatedDataSizeBoxOwned {
    /// Repeated bytes.
    pub repeated_bytes: u64,
}

impl RepeatedDataSizeBoxOwned {
    /// Creates a new RepeatedDataSizeBoxOwned.
    pub fn new(repeated_bytes: u64) -> Self {
        Self { repeated_bytes }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(8) + 8 // 8 + 8
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_u64::<BigEndian>(self.repeated_bytes)?;

        Ok(())
    }
}

impl Default for RepeatedDataSizeBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl RepeatedDataSizeBox for RepeatedDataSizeBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn repeated_bytes(&self) -> u64 {
        self.repeated_bytes
    }
}

impl<T: RepeatedDataSizeBox> From<&T> for RepeatedDataSizeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            repeated_bytes: source.repeated_bytes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_drep() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes()); // 8 + 8
        data.extend_from_slice(b"drep");
        data.extend_from_slice(&250u64.to_be_bytes());
        data
    }

    #[test]
    fn parse_drep() {
        let data = make_drep();
        let view = RepeatedDataSizeBoxView::new(&data).unwrap();
        assert_eq!(view.repeated_bytes(), 250);
    }

    #[test]
    fn roundtrip() {
        let data = make_drep();
        let view = RepeatedDataSizeBoxView::new(&data).unwrap();
        let owned = RepeatedDataSizeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
