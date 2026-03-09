//! Sample Scale Box (stsl) parsing and serialization.
//!
//! The Sample Scale Box provides scaling information for video samples.
//!
//! ```text
//! aligned(8) class SampleScaleBox
//!    extends FullBox('stsl', version = 0, 0) {
//!    unsigned int(7) reserved = 0;
//!    unsigned int(1) constraint_flag;
//!    unsigned int(8) scale_method;
//!    signed int(16) display_center_x;
//!    signed int(16) display_center_y;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SampleScaleBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"stsl");

/// Common interface for accessing SampleScaleBox data.
pub trait SampleScaleBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the constraint flag.
    fn constraint_flag(&self) -> u8;

    /// Returns the scale method.
    fn scale_method(&self) -> u8;

    /// Returns the display center X.
    fn display_center_x(&self) -> i16;

    /// Returns the display center Y.
    fn display_center_y(&self) -> i16;
}

/// A borrowing view over raw SampleScaleBox bytes.
#[derive(Clone, Copy)]
pub struct SampleScaleBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> SampleScaleBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        // reserved(7) + constraint_flag(1) packed in one byte, then
        // scale_method(1), display_center_x(2) and display_center_y(2).
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 6)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> SampleScaleBox for SampleScaleBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.data[self.fullbox_offset]
    }

    fn flags(&self) -> u32 {
        BigEndian::read_u24(&self.data[self.fullbox_offset + 1..self.fullbox_offset + 4])
    }

    fn constraint_flag(&self) -> u8 {
        (self.data[self.fullbox_offset + 4] >> 7) & 0x01
    }

    fn scale_method(&self) -> u8 {
        self.data[self.fullbox_offset + 5]
    }

    fn display_center_x(&self) -> i16 {
        let o = self.fullbox_offset + 6;
        BigEndian::read_i16(&self.data[o..o + 2])
    }

    fn display_center_y(&self) -> i16 {
        let o = self.fullbox_offset + 8;
        BigEndian::read_i16(&self.data[o..o + 2])
    }
}

impl std::fmt::Debug for SampleScaleBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleScaleBoxView")
            .field("constraint_flag", &self.constraint_flag())
            .field("scale_method", &self.scale_method())
            .field("display_center_x", &self.display_center_x())
            .field("display_center_y", &self.display_center_y())
            .finish()
    }
}

/// An owned representation of SampleScaleBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct SampleScaleBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Constraint flag.
    pub constraint_flag: u8,
    /// Scale method.
    pub scale_method: u8,
    /// Display center X.
    pub display_center_x: i16,
    /// Display center Y.
    pub display_center_y: i16,
}

impl SampleScaleBoxOwned {
    /// Creates a new SampleScaleBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(6) + 6 // 8 + 4 + 1 + 1 + 2 + 2
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u8((self.constraint_flag & 0x01) << 7)?;
        writer.write_u8(self.scale_method)?;
        writer.write_i16::<BigEndian>(self.display_center_x)?;
        writer.write_i16::<BigEndian>(self.display_center_y)?;

        Ok(())
    }
}


impl SampleScaleBox for SampleScaleBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        0
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn constraint_flag(&self) -> u8 {
        self.constraint_flag
    }

    fn scale_method(&self) -> u8 {
        self.scale_method
    }

    fn display_center_x(&self) -> i16 {
        self.display_center_x
    }

    fn display_center_y(&self) -> i16 {
        self.display_center_y
    }
}

impl<T: SampleScaleBox> From<&T> for SampleScaleBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            constraint_flag: source.constraint_flag(),
            scale_method: source.scale_method(),
            display_center_x: source.display_center_x(),
            display_center_y: source.display_center_y(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stsl() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&18u32.to_be_bytes()); // 8 + 4 + 1 + 1 + 2 + 2
        data.extend_from_slice(b"stsl");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.push(0); // constraint_flag
        data.push(0); // scale_method
        data.extend_from_slice(&0i16.to_be_bytes()); // display_center_x
        data.extend_from_slice(&0i16.to_be_bytes()); // display_center_y
        data
    }

    #[test]
    fn parse_stsl() {
        let data = make_stsl();
        let view = SampleScaleBoxView::new(&data).unwrap();
        assert_eq!(view.scale_method(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_stsl();
        let view = SampleScaleBoxView::new(&data).unwrap();
        let owned = SampleScaleBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
