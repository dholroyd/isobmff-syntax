//! Sub Track Information Box (stri) parsing and serialization.
//!
//! The Sub Track Information Box contains information about a sub-track.
//!
//! ```text
//! aligned(8) class SubTrackInformationBox
//!    extends FullBox('stri', version = 0, 0) {
//!    template int(16) switch_group = 0;
//!    template int(16) alternate_group = 0;
//!    template unsigned int(32) sub_track_ID = 0;
//!    unsigned int(32) attribute_list[]; // to the end of the box
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for SubTrackInformationBox.
pub const BOX_TYPE: BoxCode = BoxCode::STRI;

/// Common interface for accessing SubTrackInformationBox data.
pub trait SubTrackInformationBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the switch group.
    fn switch_group(&self) -> i16;

    /// Returns the alternate group.
    fn alternate_group(&self) -> i16;

    /// Returns the sub-track ID.
    fn sub_track_id(&self) -> u32;

    /// Returns the attribute list count.
    fn attribute_list_count(&self) -> usize;

    /// Returns all attribute lists.
    fn attribute_lists(&self) -> Vec<FourCC>;
}

/// A borrowing view over raw SubTrackInformationBox bytes.
#[derive(Clone, Copy)]
pub struct SubTrackInformationBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> SubTrackInformationBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 8)?;
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

    /// Returns the attribute list at the given index.
    pub fn attribute_list(&self, index: usize) -> Option<FourCC> {
        if index >= self.attribute_list_count() {
            return None;
        }

        let offset = self.payload_offset() + 8 + index * 4;
        if offset + 4 > self.data.len() {
            return None;
        }

        Some(FourCC([self.data[offset], self.data[offset + 1], self.data[offset + 2], self.data[offset + 3]]))
    }

    /// Returns all attribute lists.
    pub fn attribute_lists(&self) -> Vec<FourCC> {
        (0..self.attribute_list_count())
            .filter_map(|i| self.attribute_list(i))
            .collect()
    }
}

impl<'a> SubTrackInformationBox for SubTrackInformationBoxView<'a> {
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

    fn switch_group(&self) -> i16 {
        let o = self.payload_offset();
        BigEndian::read_i16(&self.data[o..o + 2])
    }

    fn alternate_group(&self) -> i16 {
        let o = self.payload_offset() + 2;
        BigEndian::read_i16(&self.data[o..o + 2])
    }

    fn sub_track_id(&self) -> u32 {
        let o = self.payload_offset() + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn attribute_list_count(&self) -> usize {
        let remaining = self.data.len() - self.payload_offset() - 8;
        remaining / 4
    }

    fn attribute_lists(&self) -> Vec<FourCC> {
        self.attribute_lists()
    }
}

impl std::fmt::Debug for SubTrackInformationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubTrackInformationBoxView")
            .field("sub_track_id", &self.sub_track_id())
            .field("attribute_list_count", &self.attribute_list_count())
            .finish()
    }
}

/// An owned representation of SubTrackInformationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubTrackInformationBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Switch group.
    pub switch_group: i16,
    /// Alternate group.
    pub alternate_group: i16,
    /// Sub-track ID.
    pub sub_track_id: u32,
    /// Attribute lists.
    pub attribute_lists: Vec<FourCC>,
}

impl SubTrackInformationBoxOwned {
    /// Creates a new SubTrackInformationBoxOwned.
    pub fn new(sub_track_id: u32) -> Self {
        Self {
            flags: 0,
            switch_group: 0,
            alternate_group: 0,
            sub_track_id,
            attribute_lists: Vec::new(),
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (8 + self.attribute_lists.len() * 4) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_i16::<BigEndian>(self.switch_group)?;
        writer.write_i16::<BigEndian>(self.alternate_group)?;
        writer.write_u32::<BigEndian>(self.sub_track_id)?;

        for attr in &self.attribute_lists {
            writer.write_all(&attr.0)?;
        }

        Ok(())
    }
}

impl Default for SubTrackInformationBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl SubTrackInformationBox for SubTrackInformationBoxOwned {
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

    fn switch_group(&self) -> i16 {
        self.switch_group
    }

    fn alternate_group(&self) -> i16 {
        self.alternate_group
    }

    fn sub_track_id(&self) -> u32 {
        self.sub_track_id
    }

    fn attribute_list_count(&self) -> usize {
        self.attribute_lists.len()
    }

    fn attribute_lists(&self) -> Vec<FourCC> {
        self.attribute_lists.clone()
    }
}

impl<T: SubTrackInformationBox> From<&T> for SubTrackInformationBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            switch_group: source.switch_group(),
            alternate_group: source.alternate_group(),
            sub_track_id: source.sub_track_id(),
            attribute_lists: source.attribute_lists(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stri() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 8 = 20 bytes
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"stri");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1i16.to_be_bytes()); // switch_group
        data.extend_from_slice(&2i16.to_be_bytes()); // alternate_group
        data.extend_from_slice(&100u32.to_be_bytes()); // sub_track_id
        data
    }

    #[test]
    fn parse_stri() {
        let data = make_stri();
        let view = SubTrackInformationBoxView::new(&data).unwrap();

        assert_eq!(view.switch_group(), 1);
        assert_eq!(view.alternate_group(), 2);
        assert_eq!(view.sub_track_id(), 100);
    }

    #[test]
    fn roundtrip() {
        let data = make_stri();
        let view = SubTrackInformationBoxView::new(&data).unwrap();
        let owned = SubTrackInformationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
