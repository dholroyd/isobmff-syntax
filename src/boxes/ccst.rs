//! Coding Constraints Box (ccst) parsing and serialization.
//!
//! The Coding Constraints Box specifies coding constraints for the track.
//!
//! ```text
//! aligned(8) class CodingConstraintsBox
//!    extends ItemFullProperty('ccst', version = 0, 0) {
//!    unsigned int(1) all_ref_pics_intra;
//!    unsigned int(1) intra_pred_used;
//!    unsigned int(4) max_ref_per_pic;
//!    unsigned int(26) reserved;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for CodingConstraintsBox.
pub const BOX_TYPE: BoxCode = BoxCode::CCST;

/// Common interface for accessing CodingConstraintsBox data.
pub trait CodingConstraintsBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns true if all reference pictures are intra coded.
    fn all_ref_pics_intra(&self) -> bool;

    /// Returns true if intra prediction may not extend across tile boundaries.
    fn intra_pred_used(&self) -> bool;

    /// Returns the maximum number of reference pictures.
    fn max_ref_per_pic(&self) -> u8;

    /// Returns the raw constraint word.
    fn constraint_word(&self) -> u32;
}

/// A borrowing view over raw CodingConstraintsBox bytes.
#[derive(Clone, Copy)]
pub struct CodingConstraintsBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> CodingConstraintsBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 4)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the constraint word.
    fn constraint_word(&self) -> u32 {
        let o = self.fullbox_offset + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl<'a> CodingConstraintsBox for CodingConstraintsBoxView<'a> {
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

    fn all_ref_pics_intra(&self) -> bool {
        (self.constraint_word() >> 31) & 1 != 0
    }

    fn intra_pred_used(&self) -> bool {
        (self.constraint_word() >> 30) & 1 != 0
    }

    fn max_ref_per_pic(&self) -> u8 {
        ((self.constraint_word() >> 26) & 0x0F) as u8
    }

    fn constraint_word(&self) -> u32 {
        let o = self.fullbox_offset + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for CodingConstraintsBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodingConstraintsBoxView")
            .field("all_ref_pics_intra", &self.all_ref_pics_intra())
            .field("intra_pred_used", &self.intra_pred_used())
            .field("max_ref_per_pic", &self.max_ref_per_pic())
            .finish()
    }
}

/// An owned representation of CodingConstraintsBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct CodingConstraintsBoxOwned {
    /// Flags.
    pub flags: u32,
    /// All reference pictures are intra coded.
    pub all_ref_pics_intra: bool,
    /// Intra prediction used flag.
    pub intra_pred_used: bool,
    /// Maximum number of reference pictures.
    pub max_ref_per_pic: u8,
    /// Reserved bits.
    pub reserved: u32,
}

impl CodingConstraintsBoxOwned {
    /// Creates a new CodingConstraintsBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(4) + 4 // 8 + 4 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, 0, self.flags)?;

        let constraint_word = ((self.all_ref_pics_intra as u32) << 31)
            | ((self.intra_pred_used as u32) << 30)
            | (((self.max_ref_per_pic & 0x0F) as u32) << 26)
            | (self.reserved & 0x03FF_FFFF);
        writer.write_u32::<BigEndian>(constraint_word)?;

        Ok(())
    }
}


impl CodingConstraintsBox for CodingConstraintsBoxOwned {
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

    fn all_ref_pics_intra(&self) -> bool {
        self.all_ref_pics_intra
    }

    fn intra_pred_used(&self) -> bool {
        self.intra_pred_used
    }

    fn max_ref_per_pic(&self) -> u8 {
        self.max_ref_per_pic
    }

    fn constraint_word(&self) -> u32 {
        ((self.all_ref_pics_intra as u32) << 31)
            | ((self.intra_pred_used as u32) << 30)
            | (((self.max_ref_per_pic & 0x0F) as u32) << 26)
            | (self.reserved & 0x03FF_FFFF)
    }
}

impl<T: CodingConstraintsBox> From<&T> for CodingConstraintsBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            all_ref_pics_intra: source.all_ref_pics_intra(),
            intra_pred_used: source.intra_pred_used(),
            max_ref_per_pic: source.max_ref_per_pic(),
            reserved: source.constraint_word() & 0x03FF_FFFF,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ccst() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"ccst");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        // constraint_word: all_ref=1, intra_pred=0, max_ref=0
        data.extend_from_slice(&0x8000_0000u32.to_be_bytes());
        data
    }

    #[test]
    fn parse_ccst() {
        let data = make_ccst();
        let view = CodingConstraintsBoxView::new(&data).unwrap();

        assert!(view.all_ref_pics_intra());
        assert!(!view.intra_pred_used());
        assert_eq!(view.max_ref_per_pic(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_ccst();
        let view = CodingConstraintsBoxView::new(&data).unwrap();
        let owned = CodingConstraintsBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
