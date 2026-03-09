//! Camera Intrinsic Matrix Box (cmin) parsing and serialization.
//!
//! The Camera Intrinsic Matrix Box provides camera intrinsic parameters.
//!
//! ```text
//! aligned(8) class CameraIntrinsicMatrix
//!    extends ItemFullProperty('cmin', version = 0, 0) {
//!    unsigned int(32) focal_length_x;
//!    unsigned int(32) focal_length_y;
//!    unsigned int(32) principal_point_x;
//!    unsigned int(32) principal_point_y;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for CameraIntrinsicMatrixBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"cmin");

/// Common interface for accessing CameraIntrinsicMatrixBox data.
pub trait CameraIntrinsicMatrixBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the focal length X.
    fn focal_length_x(&self) -> u32;

    /// Returns the focal length Y.
    fn focal_length_y(&self) -> u32;

    /// Returns the principal point X.
    fn principal_point_x(&self) -> u32;

    /// Returns the principal point Y.
    fn principal_point_y(&self) -> u32;
}

/// A borrowing view over raw CameraIntrinsicMatrixBox bytes.
#[derive(Clone, Copy)]
pub struct CameraIntrinsicMatrixBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> CameraIntrinsicMatrixBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 16)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> CameraIntrinsicMatrixBox for CameraIntrinsicMatrixBoxView<'a> {
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

    fn focal_length_x(&self) -> u32 {
        let o = self.fullbox_offset + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn focal_length_y(&self) -> u32 {
        let o = self.fullbox_offset + 8;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn principal_point_x(&self) -> u32 {
        let o = self.fullbox_offset + 12;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn principal_point_y(&self) -> u32 {
        let o = self.fullbox_offset + 16;
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for CameraIntrinsicMatrixBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraIntrinsicMatrixBoxView")
            .field("focal_length", &(self.focal_length_x(), self.focal_length_y()))
            .field("principal_point", &(self.principal_point_x(), self.principal_point_y()))
            .finish()
    }
}

/// An owned representation of CameraIntrinsicMatrixBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraIntrinsicMatrixBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Focal length X.
    pub focal_length_x: u32,
    /// Focal length Y.
    pub focal_length_y: u32,
    /// Principal point X.
    pub principal_point_x: u32,
    /// Principal point Y.
    pub principal_point_y: u32,
}

impl CameraIntrinsicMatrixBoxOwned {
    /// Creates a new CameraIntrinsicMatrixBoxOwned.
    pub fn new(
        focal_length_x: u32,
        focal_length_y: u32,
        principal_point_x: u32,
        principal_point_y: u32,
    ) -> Self {
        Self {
            flags: 0,
            focal_length_x,
            focal_length_y,
            principal_point_x,
            principal_point_y,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(16) + 16 // 8 + 4 + 16
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.focal_length_x)?;
        writer.write_u32::<BigEndian>(self.focal_length_y)?;
        writer.write_u32::<BigEndian>(self.principal_point_x)?;
        writer.write_u32::<BigEndian>(self.principal_point_y)?;

        Ok(())
    }
}

impl Default for CameraIntrinsicMatrixBoxOwned {
    fn default() -> Self {
        Self::new(0, 0, 0, 0)
    }
}

impl CameraIntrinsicMatrixBox for CameraIntrinsicMatrixBoxOwned {
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

    fn focal_length_x(&self) -> u32 {
        self.focal_length_x
    }

    fn focal_length_y(&self) -> u32 {
        self.focal_length_y
    }

    fn principal_point_x(&self) -> u32 {
        self.principal_point_x
    }

    fn principal_point_y(&self) -> u32 {
        self.principal_point_y
    }
}

impl<T: CameraIntrinsicMatrixBox> From<&T> for CameraIntrinsicMatrixBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            focal_length_x: source.focal_length_x(),
            focal_length_y: source.focal_length_y(),
            principal_point_x: source.principal_point_x(),
            principal_point_y: source.principal_point_y(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cmin() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&28u32.to_be_bytes());
        data.extend_from_slice(b"cmin");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1000u32.to_be_bytes()); // focal_length_x
        data.extend_from_slice(&1000u32.to_be_bytes()); // focal_length_y
        data.extend_from_slice(&500u32.to_be_bytes()); // principal_point_x
        data.extend_from_slice(&500u32.to_be_bytes()); // principal_point_y
        data
    }

    #[test]
    fn parse_cmin() {
        let data = make_cmin();
        let view = CameraIntrinsicMatrixBoxView::new(&data).unwrap();

        assert_eq!(view.focal_length_x(), 1000);
        assert_eq!(view.focal_length_y(), 1000);
        assert_eq!(view.principal_point_x(), 500);
        assert_eq!(view.principal_point_y(), 500);
    }

    #[test]
    fn roundtrip() {
        let data = make_cmin();
        let view = CameraIntrinsicMatrixBoxView::new(&data).unwrap();
        let owned = CameraIntrinsicMatrixBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
