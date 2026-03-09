//! Sub Track Sample Group Box (stsg) parsing and serialization.
//!
//! The Sub Track Sample Group Box specifies which samples belong to a sub-track.
//!
//! ```text
//! aligned(8) class SubTrackSampleGroupBox
//!    extends FullBox('stsg', 0, 0) {
//!    unsigned int(32) grouping_type;
//!    unsigned int(16) item_count;
//!    for(i = 0; i< item_count; i++)
//!       unsigned int(32) group_description_index;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for SubTrackSampleGroupBox.
pub const BOX_TYPE: BoxCode = BoxCode::STSG;

/// Common interface for accessing SubTrackSampleGroupBox data.
pub trait SubTrackSampleGroupBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the grouping type.
    fn grouping_type(&self) -> FourCC;

    /// Returns the item count.
    fn item_count(&self) -> u16;

    /// Returns all group description indices.
    fn group_description_indices(&self) -> Vec<u32>;
}

/// A borrowing view over raw SubTrackSampleGroupBox bytes.
#[derive(Clone, Copy)]
pub struct SubTrackSampleGroupBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    item_count: u16,
}

impl<'a> SubTrackSampleGroupBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 6)?;
        let item_count = BigEndian::read_u16(&data[fullbox_offset + 8..fullbox_offset + 10]);
        Ok(Self { data, fullbox_offset, item_count })
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

    /// Returns the group description index at the given index.
    pub fn group_description_index(&self, index: usize) -> Option<u32> {
        if index >= self.item_count as usize {
            return None;
        }

        let offset = self.fullbox_offset + 10 + index * 4;
        if offset + 4 > self.data.len() {
            return None;
        }

        Some(BigEndian::read_u32(&self.data[offset..offset + 4]))
    }

    /// Returns all group description indices.
    pub fn group_description_indices(&self) -> Vec<u32> {
        (0..self.item_count as usize)
            .filter_map(|i| self.group_description_index(i))
            .collect()
    }
}

impl<'a> SubTrackSampleGroupBox for SubTrackSampleGroupBoxView<'a> {
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

    fn grouping_type(&self) -> FourCC {
        let o = self.payload_offset();
        FourCC([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]])
    }

    fn item_count(&self) -> u16 {
        self.item_count
    }

    fn group_description_indices(&self) -> Vec<u32> {
        self.group_description_indices()
    }
}

impl std::fmt::Debug for SubTrackSampleGroupBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubTrackSampleGroupBoxView")
            .field("grouping_type", &self.grouping_type())
            .field("item_count", &self.item_count())
            .finish()
    }
}

/// An owned representation of SubTrackSampleGroupBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubTrackSampleGroupBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Grouping type.
    pub grouping_type: FourCC,
    /// Group description indices.
    pub group_description_indices: Vec<u32>,
}

impl SubTrackSampleGroupBoxOwned {
    /// Creates a new SubTrackSampleGroupBoxOwned.
    pub fn new(grouping_type: FourCC) -> Self {
        Self {
            flags: 0,
            grouping_type,
            group_description_indices: Vec::new(),
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (4 + 2 + self.group_description_indices.len() * 4) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_all(&self.grouping_type.0)?;
        writer.write_u16::<BigEndian>(self.group_description_indices.len() as u16)?;

        for index in &self.group_description_indices {
            writer.write_u32::<BigEndian>(*index)?;
        }

        Ok(())
    }
}

impl Default for SubTrackSampleGroupBoxOwned {
    fn default() -> Self {
        Self::new(FourCC(*b"roll"))
    }
}

impl SubTrackSampleGroupBox for SubTrackSampleGroupBoxOwned {
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

    fn grouping_type(&self) -> FourCC {
        self.grouping_type
    }

    fn item_count(&self) -> u16 {
        self.group_description_indices.len() as u16
    }

    fn group_description_indices(&self) -> Vec<u32> {
        self.group_description_indices.clone()
    }
}

impl<T: SubTrackSampleGroupBox> From<&T> for SubTrackSampleGroupBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            grouping_type: source.grouping_type(),
            group_description_indices: source.group_description_indices(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stsg() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 4 + 2 + 4 = 22 bytes
        data.extend_from_slice(&22u32.to_be_bytes());
        data.extend_from_slice(b"stsg");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(b"roll"); // grouping_type
        data.extend_from_slice(&1u16.to_be_bytes()); // item_count
        data.extend_from_slice(&1u32.to_be_bytes()); // group_description_index
        data
    }

    #[test]
    fn parse_stsg() {
        let data = make_stsg();
        let view = SubTrackSampleGroupBoxView::new(&data).unwrap();

        assert_eq!(view.grouping_type(), FourCC(*b"roll"));
        assert_eq!(view.item_count(), 1);
        assert_eq!(view.group_description_index(0), Some(1));
    }

    #[test]
    fn roundtrip() {
        let data = make_stsg();
        let view = SubTrackSampleGroupBoxView::new(&data).unwrap();
        let owned = SubTrackSampleGroupBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
