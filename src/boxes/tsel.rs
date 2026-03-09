//! Track Selection Box (tsel) parsing and serialization.
//!
//! The Track Selection Box contains attributes for selecting a track.
//!
//! ```text
//! aligned(8) class TrackSelectionBox
//!    extends FullBox('tsel', version = 0, 0) {
//!    template int(32) switch_group = 0;
//!    unsigned int(32) attribute_list[]; // to end of the box
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for TrackSelectionBox.
pub const BOX_TYPE: BoxCode = BoxCode::TSEL;

/// Common interface for accessing TrackSelectionBox data.
pub trait TrackSelectionBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the switch group.
    fn switch_group(&self) -> u32;

    /// Returns the number of attribute lists.
    fn attribute_list_count(&self) -> usize;

    /// Returns an iterator over all attribute lists.
    fn attribute_lists(&self) -> impl Iterator<Item = FourCC> + '_;
}

/// A borrowing view over raw TrackSelectionBox bytes.
#[derive(Clone, Copy)]
pub struct TrackSelectionBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> TrackSelectionBoxView<'a> {
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

    #[inline]
    fn payload_offset(&self) -> usize {
        self.fullbox_offset + 4
    }

    /// Returns the attribute list at the given index.
    pub fn attribute_list(&self, index: usize) -> Option<FourCC> {
        if index >= self.attribute_list_count() {
            return None;
        }

        let offset = self.payload_offset().checked_add(4)?.checked_add(index.checked_mul(4)?)?;
        if offset + 4 > self.data.len() {
            return None;
        }

        Some(FourCC([self.data[offset], self.data[offset + 1], self.data[offset + 2], self.data[offset + 3]]))
    }

}

impl<'a> TrackSelectionBox for TrackSelectionBoxView<'a> {
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

    fn switch_group(&self) -> u32 {
        let o = self.payload_offset();
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn attribute_list_count(&self) -> usize {
        let remaining = self.data.len() - self.payload_offset() - 4;
        remaining / 4
    }

    fn attribute_lists(&self) -> impl Iterator<Item = FourCC> + '_ {
        (0..self.attribute_list_count()).filter_map(|i| self.attribute_list(i))
    }
}

impl std::fmt::Debug for TrackSelectionBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackSelectionBoxView")
            .field("switch_group", &self.switch_group())
            .field("attribute_list_count", &self.attribute_list_count())
            .finish()
    }
}

/// An owned representation of TrackSelectionBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackSelectionBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Switch group.
    pub switch_group: u32,
    /// Attribute lists (4-byte codes).
    pub attribute_lists: Vec<FourCC>,
}

impl TrackSelectionBoxOwned {
    /// Creates a new TrackSelectionBoxOwned.
    pub fn new(switch_group: u32) -> Self {
        Self {
            flags: 0,
            switch_group,
            attribute_lists: Vec::new(),
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (4 + self.attribute_lists.len() * 4) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.switch_group)?;

        for attr in &self.attribute_lists {
            writer.write_all(&attr.0)?;
        }

        Ok(())
    }
}

impl Default for TrackSelectionBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl TrackSelectionBox for TrackSelectionBoxOwned {
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

    fn switch_group(&self) -> u32 {
        self.switch_group
    }

    fn attribute_list_count(&self) -> usize {
        self.attribute_lists.len()
    }

    fn attribute_lists(&self) -> impl Iterator<Item = FourCC> + '_ {
        self.attribute_lists.iter().copied()
    }
}

impl<T: TrackSelectionBox> From<&T> for TrackSelectionBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            switch_group: source.switch_group(),
            attribute_lists: source.attribute_lists().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tsel() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 4 + 4 = 20 bytes
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"tsel");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1u32.to_be_bytes()); // switch_group
        data.extend_from_slice(b"cdec"); // attribute_list
        data
    }

    #[test]
    fn parse_tsel() {
        let data = make_tsel();
        let view = TrackSelectionBoxView::new(&data).unwrap();

        assert_eq!(view.switch_group(), 1);
        assert_eq!(view.attribute_list_count(), 1);
        assert_eq!(view.attribute_list(0), Some(FourCC(*b"cdec")));
    }

    #[test]
    fn roundtrip() {
        let data = make_tsel();
        let view = TrackSelectionBoxView::new(&data).unwrap();
        let owned = TrackSelectionBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
