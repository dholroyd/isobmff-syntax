//! Largest Relative Transmission Time Box (tmax) parsing and serialization.
//!
//! The Largest Relative Transmission Time Box contains the largest
//! relative transmission time in milliseconds.
//!
//! ```text
//! aligned(8) class hintmaxrelativetime extends Box('tmax') {
//!    int(32) time; // largest relative transmission time, milliseconds
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for LargestRelativeTimeBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"tmax");

/// Common interface for accessing LargestRelativeTimeBox data.
pub trait LargestRelativeTimeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the largest relative transmission time.
    fn max_time(&self) -> i32;
}

/// A borrowing view over raw LargestRelativeTimeBox bytes.
#[derive(Clone, Copy)]
pub struct LargestRelativeTimeBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> LargestRelativeTimeBoxView<'a> {
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

impl<'a> LargestRelativeTimeBox for LargestRelativeTimeBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn max_time(&self) -> i32 {
        BigEndian::read_i32(&self.data[self.header_size..self.header_size + 4])
    }
}

impl std::fmt::Debug for LargestRelativeTimeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LargestRelativeTimeBoxView")
            .field("max_time", &self.max_time())
            .finish()
    }
}

/// An owned representation of LargestRelativeTimeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LargestRelativeTimeBoxOwned {
    /// Largest relative transmission time.
    pub max_time: i32,
}

impl LargestRelativeTimeBoxOwned {
    /// Creates a new LargestRelativeTimeBoxOwned.
    pub fn new(max_time: i32) -> Self {
        Self { max_time }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(4) + 4 // 8 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_i32::<BigEndian>(self.max_time)?;

        Ok(())
    }
}

impl Default for LargestRelativeTimeBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl LargestRelativeTimeBox for LargestRelativeTimeBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn max_time(&self) -> i32 {
        self.max_time
    }
}

impl<T: LargestRelativeTimeBox> From<&T> for LargestRelativeTimeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            max_time: source.max_time(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tmax() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"tmax");
        data.extend_from_slice(&100i32.to_be_bytes());
        data
    }

    #[test]
    fn parse_tmax() {
        let data = make_tmax();
        let view = LargestRelativeTimeBoxView::new(&data).unwrap();
        assert_eq!(view.max_time(), 100);
    }

    #[test]
    fn roundtrip() {
        let data = make_tmax();
        let view = LargestRelativeTimeBoxView::new(&data).unwrap();
        let owned = LargestRelativeTimeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
