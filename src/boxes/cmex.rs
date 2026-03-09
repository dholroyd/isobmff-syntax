//! Camera Extrinsic Matrix Box (cmex) parsing and serialization.
//!
//! The Camera Extrinsic Matrix Box provides camera extrinsic parameters.
//!
//! ```text
//! aligned(8) class CameraExtrinsicMatrix
//!    extends ItemFullProperty('cmex', version = 0, flags) {
//!    if (flags & 1) { // pos_present
//!       signed int(32) pos_x;
//!       signed int(32) pos_y;
//!       signed int(32) pos_z;
//!    }
//!    if (flags & 2) { // orient_present
//!       unsigned int(32) quat_x;
//!       unsigned int(32) quat_y;
//!       unsigned int(32) quat_z;
//!    }
//!    if (flags & 4) { // world_coordinate_system_present
//!       unsigned int(32) world_coordinate_system_id;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for CameraExtrinsicMatrixBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"cmex");

/// Camera extrinsic matrix flags.
pub mod flags {
    /// Position fields (pos_x, pos_y, pos_z) are present.
    pub const POS_PRESENT: u32 = 0x01;
    /// Orientation fields (quat_x, quat_y, quat_z) are present.
    pub const ORIENT_PRESENT: u32 = 0x02;
    /// World coordinate system ID is present.
    pub const WORLD_COORDINATE_SYSTEM_PRESENT: u32 = 0x04;
}

/// Common interface for accessing CameraExtrinsicMatrixBox data.
pub trait CameraExtrinsicMatrixBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the position X coordinate, if present (flags & 1).
    fn pos_x(&self) -> Option<i32>;

    /// Returns the position Y coordinate, if present (flags & 1).
    fn pos_y(&self) -> Option<i32>;

    /// Returns the position Z coordinate, if present (flags & 1).
    fn pos_z(&self) -> Option<i32>;

    /// Returns the quaternion X component, if present (flags & 2).
    fn quat_x(&self) -> Option<u32>;

    /// Returns the quaternion Y component, if present (flags & 2).
    fn quat_y(&self) -> Option<u32>;

    /// Returns the quaternion Z component, if present (flags & 2).
    fn quat_z(&self) -> Option<u32>;

    /// Returns the world coordinate system ID, if present (flags & 4).
    fn world_coordinate_system_id(&self) -> Option<u32>;
}

/// A borrowing view over raw CameraExtrinsicMatrixBox bytes.
#[derive(Clone, Copy)]
pub struct CameraExtrinsicMatrixBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> CameraExtrinsicMatrixBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 0)?;

        // Compute required size from flags
        let fl = BigEndian::read_u24(&data[fullbox_offset + 1..fullbox_offset + 4]);
        let mut min_size = fullbox_offset + 4;
        if fl & flags::POS_PRESENT != 0 {
            min_size = min_size.checked_add(12).ok_or(ParseError::BufferTooShort {
                expected: usize::MAX,
                found: data.len(),
            })?;
        }
        if fl & flags::ORIENT_PRESENT != 0 {
            min_size = min_size.checked_add(12).ok_or(ParseError::BufferTooShort {
                expected: usize::MAX,
                found: data.len(),
            })?;
        }
        if fl & flags::WORLD_COORDINATE_SYSTEM_PRESENT != 0 {
            min_size = min_size.checked_add(4).ok_or(ParseError::BufferTooShort {
                expected: usize::MAX,
                found: data.len(),
            })?;
        }
        if data.len() < min_size {
            return Err(ParseError::BufferTooShort {
                expected: min_size,
                found: data.len(),
            });
        }

        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    #[inline]
    fn payload_offset(&self) -> usize {
        self.fullbox_offset + 4
    }

    /// Returns the offset of an optional field group based on flags.
    /// The fields are laid out sequentially: position (12 bytes if flag 1),
    /// orientation (12 bytes if flag 2), world_coordinate_system_id (4 bytes if flag 4).
    fn optional_field_offset(&self, target_flag: u32) -> Option<usize> {
        let fl = self.flags();
        let mut offset = self.payload_offset();

        let ordered_flags: [(u32, usize); 3] = [
            (flags::POS_PRESENT, 12),
            (flags::ORIENT_PRESENT, 12),
            (flags::WORLD_COORDINATE_SYSTEM_PRESENT, 4),
        ];

        for (flag, size) in ordered_flags {
            if flag == target_flag {
                if fl & flag != 0 {
                    return Some(offset);
                } else {
                    return None;
                }
            }
            if fl & flag != 0 {
                offset += size;
            }
        }
        None
    }
}

impl CameraExtrinsicMatrixBox for CameraExtrinsicMatrixBoxView<'_> {
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

    fn pos_x(&self) -> Option<i32> {
        self.optional_field_offset(flags::POS_PRESENT)
            .map(|o| BigEndian::read_i32(&self.data[o..o + 4]))
    }

    fn pos_y(&self) -> Option<i32> {
        self.optional_field_offset(flags::POS_PRESENT)
            .map(|o| BigEndian::read_i32(&self.data[o + 4..o + 8]))
    }

    fn pos_z(&self) -> Option<i32> {
        self.optional_field_offset(flags::POS_PRESENT)
            .map(|o| BigEndian::read_i32(&self.data[o + 8..o + 12]))
    }

    fn quat_x(&self) -> Option<u32> {
        self.optional_field_offset(flags::ORIENT_PRESENT)
            .map(|o| BigEndian::read_u32(&self.data[o..o + 4]))
    }

    fn quat_y(&self) -> Option<u32> {
        self.optional_field_offset(flags::ORIENT_PRESENT)
            .map(|o| BigEndian::read_u32(&self.data[o + 4..o + 8]))
    }

    fn quat_z(&self) -> Option<u32> {
        self.optional_field_offset(flags::ORIENT_PRESENT)
            .map(|o| BigEndian::read_u32(&self.data[o + 8..o + 12]))
    }

    fn world_coordinate_system_id(&self) -> Option<u32> {
        self.optional_field_offset(flags::WORLD_COORDINATE_SYSTEM_PRESENT)
            .map(|o| BigEndian::read_u32(&self.data[o..o + 4]))
    }
}

impl std::fmt::Debug for CameraExtrinsicMatrixBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraExtrinsicMatrixBoxView")
            .field("flags", &format!("0x{:06X}", self.flags()))
            .field("pos", &(self.pos_x(), self.pos_y(), self.pos_z()))
            .field("orientation", &(self.quat_x(), self.quat_y(), self.quat_z()))
            .field("world_coordinate_system_id", &self.world_coordinate_system_id())
            .finish()
    }
}

/// An owned representation of CameraExtrinsicMatrixBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct CameraExtrinsicMatrixBoxOwned {
    /// Position (pos_x, pos_y, pos_z), present when flags & 1.
    pub pos: Option<(i32, i32, i32)>,
    /// Orientation quaternion (quat_x, quat_y, quat_z), present when flags & 2.
    pub orientation: Option<(u32, u32, u32)>,
    /// World coordinate system ID, present when flags & 4.
    pub world_coordinate_system_id: Option<u32>,
}

impl CameraExtrinsicMatrixBoxOwned {
    /// Creates a new CameraExtrinsicMatrixBoxOwned with no fields present.
    pub fn new() -> Self {
        Self::default()
    }

    /// Computes the flags based on which optional fields are present.
    fn compute_flags(&self) -> u32 {
        let mut fl = 0u32;
        if self.pos.is_some() {
            fl |= flags::POS_PRESENT;
        }
        if self.orientation.is_some() {
            fl |= flags::ORIENT_PRESENT;
        }
        if self.world_coordinate_system_id.is_some() {
            fl |= flags::WORLD_COORDINATE_SYSTEM_PRESENT;
        }
        fl
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let mut payload = 0u64;
        if self.pos.is_some() {
            payload += 12;
        }
        if self.orientation.is_some() {
            payload += 12;
        }
        if self.world_coordinate_system_id.is_some() {
            payload += 4;
        }
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        let fl = self.compute_flags();
        write_fullbox_header(writer, size, BOX_TYPE, 0, fl)?;

        if let Some((px, py, pz)) = self.pos {
            writer.write_i32::<BigEndian>(px)?;
            writer.write_i32::<BigEndian>(py)?;
            writer.write_i32::<BigEndian>(pz)?;
        }
        if let Some((qx, qy, qz)) = self.orientation {
            writer.write_u32::<BigEndian>(qx)?;
            writer.write_u32::<BigEndian>(qy)?;
            writer.write_u32::<BigEndian>(qz)?;
        }
        if let Some(wcs_id) = self.world_coordinate_system_id {
            writer.write_u32::<BigEndian>(wcs_id)?;
        }

        Ok(())
    }
}


impl CameraExtrinsicMatrixBox for CameraExtrinsicMatrixBoxOwned {
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
        self.compute_flags()
    }

    fn pos_x(&self) -> Option<i32> {
        self.pos.map(|(x, _, _)| x)
    }

    fn pos_y(&self) -> Option<i32> {
        self.pos.map(|(_, y, _)| y)
    }

    fn pos_z(&self) -> Option<i32> {
        self.pos.map(|(_, _, z)| z)
    }

    fn quat_x(&self) -> Option<u32> {
        self.orientation.map(|(x, _, _)| x)
    }

    fn quat_y(&self) -> Option<u32> {
        self.orientation.map(|(_, y, _)| y)
    }

    fn quat_z(&self) -> Option<u32> {
        self.orientation.map(|(_, _, z)| z)
    }

    fn world_coordinate_system_id(&self) -> Option<u32> {
        self.world_coordinate_system_id
    }
}

impl<T: CameraExtrinsicMatrixBox> From<&T> for CameraExtrinsicMatrixBoxOwned {
    fn from(source: &T) -> Self {
        let pos = match (source.pos_x(), source.pos_y(), source.pos_z()) {
            (Some(x), Some(y), Some(z)) => Some((x, y, z)),
            _ => None,
        };
        let orientation = match (source.quat_x(), source.quat_y(), source.quat_z()) {
            (Some(x), Some(y), Some(z)) => Some((x, y, z)),
            _ => None,
        };
        Self {
            pos,
            orientation,
            world_coordinate_system_id: source.world_coordinate_system_id(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cmex_flags0() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"cmex");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags = 0
        data
    }

    fn make_cmex_all_flags() -> Vec<u8> {
        let mut data = Vec::new();
        // size = 8 (header) + 4 (version/flags) + 12 (pos) + 12 (orient) + 4 (wcs) = 40
        data.extend_from_slice(&40u32.to_be_bytes());
        data.extend_from_slice(b"cmex");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 7]); // flags = 7 (all present)
        // pos_x, pos_y, pos_z (signed)
        data.extend_from_slice(&100i32.to_be_bytes());
        data.extend_from_slice(&(-200i32).to_be_bytes());
        data.extend_from_slice(&300i32.to_be_bytes());
        // quat_x, quat_y, quat_z (unsigned)
        data.extend_from_slice(&1000u32.to_be_bytes());
        data.extend_from_slice(&2000u32.to_be_bytes());
        data.extend_from_slice(&3000u32.to_be_bytes());
        // world_coordinate_system_id
        data.extend_from_slice(&42u32.to_be_bytes());
        data
    }

    #[test]
    fn parse_cmex_no_fields() {
        let data = make_cmex_flags0();
        let view = CameraExtrinsicMatrixBoxView::new(&data).unwrap();
        assert_eq!(view.version(), 0);
        assert_eq!(view.flags(), 0);
        assert_eq!(view.pos_x(), None);
        assert_eq!(view.pos_y(), None);
        assert_eq!(view.pos_z(), None);
        assert_eq!(view.quat_x(), None);
        assert_eq!(view.quat_y(), None);
        assert_eq!(view.quat_z(), None);
        assert_eq!(view.world_coordinate_system_id(), None);
    }

    #[test]
    fn parse_cmex_all_fields() {
        let data = make_cmex_all_flags();
        let view = CameraExtrinsicMatrixBoxView::new(&data).unwrap();
        assert_eq!(view.version(), 0);
        assert_eq!(view.flags(), 7);
        assert_eq!(view.pos_x(), Some(100));
        assert_eq!(view.pos_y(), Some(-200));
        assert_eq!(view.pos_z(), Some(300));
        assert_eq!(view.quat_x(), Some(1000));
        assert_eq!(view.quat_y(), Some(2000));
        assert_eq!(view.quat_z(), Some(3000));
        assert_eq!(view.world_coordinate_system_id(), Some(42));
    }

    #[test]
    fn roundtrip_no_fields() {
        let data = make_cmex_flags0();
        let view = CameraExtrinsicMatrixBoxView::new(&data).unwrap();
        let owned = CameraExtrinsicMatrixBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_all_fields() {
        let data = make_cmex_all_flags();
        let view = CameraExtrinsicMatrixBoxView::new(&data).unwrap();
        let owned = CameraExtrinsicMatrixBoxOwned::from(&view);

        assert_eq!(owned.pos, Some((100, -200, 300)));
        assert_eq!(owned.orientation, Some((1000, 2000, 3000)));
        assert_eq!(owned.world_coordinate_system_id, Some(42));

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn parse_cmex_pos_only() {
        let mut data = Vec::new();
        // size = 8 + 4 + 12 = 24
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(b"cmex");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 1]); // flags = 1 (pos only)
        data.extend_from_slice(&(-10i32).to_be_bytes());
        data.extend_from_slice(&20i32.to_be_bytes());
        data.extend_from_slice(&(-30i32).to_be_bytes());
        let view = CameraExtrinsicMatrixBoxView::new(&data).unwrap();
        assert_eq!(view.pos_x(), Some(-10));
        assert_eq!(view.pos_y(), Some(20));
        assert_eq!(view.pos_z(), Some(-30));
        assert_eq!(view.quat_x(), None);
        assert_eq!(view.world_coordinate_system_id(), None);
    }

    #[test]
    fn parse_cmex_orient_and_wcs() {
        let mut data = Vec::new();
        // size = 8 + 4 + 12 + 4 = 28
        data.extend_from_slice(&28u32.to_be_bytes());
        data.extend_from_slice(b"cmex");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 6]); // flags = 6 (orient + wcs)
        // quat_x, quat_y, quat_z
        data.extend_from_slice(&500u32.to_be_bytes());
        data.extend_from_slice(&600u32.to_be_bytes());
        data.extend_from_slice(&700u32.to_be_bytes());
        // world_coordinate_system_id
        data.extend_from_slice(&99u32.to_be_bytes());
        let view = CameraExtrinsicMatrixBoxView::new(&data).unwrap();
        assert_eq!(view.pos_x(), None);
        assert_eq!(view.quat_x(), Some(500));
        assert_eq!(view.quat_y(), Some(600));
        assert_eq!(view.quat_z(), Some(700));
        assert_eq!(view.world_coordinate_system_id(), Some(99));
    }

    #[test]
    fn buffer_too_short_for_flags() {
        // flags = 7 but not enough data
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // size = 12 (too small for flags=7)
        data.extend_from_slice(b"cmex");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 7]); // flags = 7
        let err = CameraExtrinsicMatrixBoxView::new(&data).unwrap_err();
        match err {
            ParseError::BufferTooShort { .. } | ParseError::SizeMismatch { .. } => {}
            other => panic!("unexpected error: {:?}", other),
        }
    }
}
