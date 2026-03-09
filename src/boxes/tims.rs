//! Time Scale Entry Box (tims) parsing and serialization.
//!
//! The Time Scale Entry Box specifies the time scale for a track.
//!
//! ```text
//! class timescaleentry() extends Box('tims') {
//!    uint(32) timescale;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TimeScaleEntryBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"tims");

/// Common interface for accessing TimeScaleEntryBox data.
pub trait TimeScaleEntryBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the time scale.
    fn time_scale(&self) -> u32;
}

/// A borrowing view over raw TimeScaleEntryBox bytes.
#[derive(Clone, Copy)]
pub struct TimeScaleEntryBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> TimeScaleEntryBoxView<'a> {
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

impl<'a> TimeScaleEntryBox for TimeScaleEntryBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn time_scale(&self) -> u32 {
        let o = self.header_size;
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for TimeScaleEntryBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimeScaleEntryBoxView")
            .field("time_scale", &self.time_scale())
            .finish()
    }
}

/// An owned representation of TimeScaleEntryBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeScaleEntryBoxOwned {
    /// Time scale.
    pub time_scale: u32,
}

impl TimeScaleEntryBoxOwned {
    /// Creates a new TimeScaleEntryBoxOwned.
    pub fn new(time_scale: u32) -> Self {
        Self { time_scale }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(4) + 4 // 8 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        writer.write_u32::<BigEndian>(self.time_scale)?;

        Ok(())
    }
}

impl Default for TimeScaleEntryBoxOwned {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl TimeScaleEntryBox for TimeScaleEntryBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn time_scale(&self) -> u32 {
        self.time_scale
    }
}

impl<T: TimeScaleEntryBox> From<&T> for TimeScaleEntryBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            time_scale: source.time_scale(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tims() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes());
        data.extend_from_slice(b"tims");
        data.extend_from_slice(&90000u32.to_be_bytes()); // time_scale
        data
    }

    #[test]
    fn parse_tims() {
        let data = make_tims();
        let view = TimeScaleEntryBoxView::new(&data).unwrap();
        assert_eq!(view.time_scale(), 90000);
    }

    #[test]
    fn roundtrip() {
        let data = make_tims();
        let view = TimeScaleEntryBoxView::new(&data).unwrap();
        let owned = TimeScaleEntryBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
