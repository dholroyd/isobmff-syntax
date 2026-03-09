//! Smallest Relative Transmission Time Box (tmin) parsing and serialization.
//!
//! The Smallest Relative Transmission Time Box contains the smallest
//! relative transmission time in milliseconds.
//!
//! ```text
//! aligned(8) class hintminrelativetime extends Box('tmin') {
//!    int(32) time; // smallest relative transmission time, milliseconds
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SmallestRelativeTimeBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"tmin");

/// Common interface for accessing SmallestRelativeTimeBox data.
pub trait SmallestRelativeTimeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the smallest relative transmission time.
    fn min_time(&self) -> i32;
}

/// A borrowing view over raw SmallestRelativeTimeBox bytes.
#[derive(Clone, Copy)]
pub struct SmallestRelativeTimeBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> SmallestRelativeTimeBoxView<'a> {
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

impl<'a> SmallestRelativeTimeBox for SmallestRelativeTimeBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn min_time(&self) -> i32 {
        BigEndian::read_i32(&self.data[self.header_size..self.header_size + 4])
    }
}

impl std::fmt::Debug for SmallestRelativeTimeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmallestRelativeTimeBoxView")
            .field("min_time", &self.min_time())
            .finish()
    }
}

/// An owned representation of SmallestRelativeTimeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmallestRelativeTimeBoxOwned {
    /// Smallest relative transmission time.
    pub min_time: i32,
}

impl SmallestRelativeTimeBoxOwned {
    /// Creates a new SmallestRelativeTimeBoxOwned.
    pub fn new(min_time: i32) -> Self {
        Self { min_time }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(4) + 4 // 8 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_i32::<BigEndian>(self.min_time)?;

        Ok(())
    }
}

impl Default for SmallestRelativeTimeBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl SmallestRelativeTimeBox for SmallestRelativeTimeBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn min_time(&self) -> i32 {
        self.min_time
    }
}

impl<T: SmallestRelativeTimeBox> From<&T> for SmallestRelativeTimeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            min_time: source.min_time(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tmin() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"tmin");
        data.extend_from_slice(&(-10i32).to_be_bytes());
        data
    }

    #[test]
    fn parse_tmin() {
        let data = make_tmin();
        let view = SmallestRelativeTimeBoxView::new(&data).unwrap();
        assert_eq!(view.min_time(), -10);
    }

    #[test]
    fn roundtrip() {
        let data = make_tmin();
        let view = SmallestRelativeTimeBoxView::new(&data).unwrap();
        let owned = SmallestRelativeTimeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
