//! Image Rotation Box (irot) parsing and serialization.
//!
//! The Image Rotation Box specifies the rotation angle in multiples of 90 degrees.
//!
//! ```text
//! aligned(8) class ImageRotation
//!    extends ItemProperty('irot') {
//!    unsigned int(6) reserved = 0;
//!    unsigned int(2) angle;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::WriteBytesExt;
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ImageRotationBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"irot");

/// Common interface for accessing ImageRotationBox data.
pub trait ImageRotationBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the rotation angle code (0-3).
    /// 0 = 0°, 1 = 90° CCW, 2 = 180°, 3 = 270° CCW (or 90° CW)
    fn angle(&self) -> u8;

    /// Returns the rotation angle in degrees.
    fn angle_degrees(&self) -> u16 {
        match self.angle() & 0x03 {
            0 => 0,
            1 => 90,
            2 => 180,
            3 => 270,
            _ => 0,
        }
    }
}

/// A borrowing view over raw ImageRotationBox bytes.
#[derive(Clone, Copy)]
pub struct ImageRotationBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> ImageRotationBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 1)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> ImageRotationBox for ImageRotationBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn angle(&self) -> u8 {
        self.data[self.header_size] & 0x03
    }
}

impl std::fmt::Debug for ImageRotationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageRotationBoxView")
            .field("angle", &self.angle())
            .field("angle_degrees", &self.angle_degrees())
            .finish()
    }
}

/// An owned representation of ImageRotationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRotationBoxOwned {
    /// Rotation angle code (0-3).
    pub angle: u8,
}

impl ImageRotationBoxOwned {
    /// Creates a new ImageRotationBoxOwned.
    pub fn new(angle: u8) -> Self {
        Self { angle: angle & 0x03 }
    }

    /// Creates a new ImageRotationBoxOwned from degrees.
    pub fn from_degrees(degrees: u16) -> Self {
        let angle = match degrees % 360 {
            0..=44 => 0,
            45..=134 => 1,
            135..=224 => 2,
            225..=314 => 3,
            _ => 0,
        };
        Self::new(angle)
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(1) + 1 // 8 + 1
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        writer.write_u8(self.angle & 0x03)?;

        Ok(())
    }
}

impl Default for ImageRotationBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl ImageRotationBox for ImageRotationBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn angle(&self) -> u8 {
        self.angle & 0x03
    }
}

impl<T: ImageRotationBox> From<&T> for ImageRotationBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            angle: source.angle(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_irot() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&9u32.to_be_bytes());
        data.extend_from_slice(b"irot");
        data.push(1); // 90 degrees CCW
        data
    }

    #[test]
    fn parse_irot() {
        let data = make_irot();
        let view = ImageRotationBoxView::new(&data).unwrap();

        assert_eq!(view.angle(), 1);
        assert_eq!(view.angle_degrees(), 90);
    }

    #[test]
    fn roundtrip() {
        let data = make_irot();
        let view = ImageRotationBoxView::new(&data).unwrap();
        let owned = ImageRotationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
