//! Level Assignment Box (leva) parsing and serialization.
//!
//! The Level Assignment Box assigns levels to track fragments.
//!
//! ```text
//! aligned(8) class LevelAssignmentBox extends FullBox('leva', 0, 0) {
//!    unsigned int(8) level_count;
//!    for (j=1; j <= level_count; j++) {
//!       unsigned int(32) track_ID;
//!       unsigned int(1) padding_flag;
//!       unsigned int(7) assignment_type;
//!       if (assignment_type == 0) {
//!          unsigned int(32) grouping_type;
//!       }
//!       else if (assignment_type == 1) {
//!          unsigned int(32) grouping_type;
//!          unsigned int(32) grouping_type_parameter;
//!       }
//!       else if (assignment_type == 2) {}
//!       else if (assignment_type == 3) {}
//!       else if (assignment_type == 4) {
//!          unsigned int(32) sub_track_ID;
//!       }
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for LevelAssignmentBox.
pub const BOX_TYPE: BoxCode = BoxCode::LEVA;

/// A level assignment entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelAssignmentEntry {
    /// Track ID.
    pub track_id: u32,
    /// Padding flag.
    pub padding_flag: bool,
    /// Assignment type.
    pub assignment_type: u8,
    /// Grouping type (if assignment_type == 0 or 1).
    pub grouping_type: Option<FourCC>,
    /// Grouping type parameter (if assignment_type == 1).
    pub grouping_type_parameter: Option<u32>,
    /// Sub-track ID (if assignment_type == 4).
    pub sub_track_id: Option<u32>,
}

/// Common interface for accessing LevelAssignmentBox data.
pub trait LevelAssignmentBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the level count.
    fn level_count(&self) -> u8;

    /// Returns an iterator over all entries.
    fn entries(&self) -> impl Iterator<Item = Result<LevelAssignmentEntry, ParseError>> + '_;
}

/// A borrowing view over raw LevelAssignmentBox bytes.
#[derive(Clone, Copy)]
pub struct LevelAssignmentBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    level_count: u8,
}

impl<'a> LevelAssignmentBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 1)?;
        let level_count = data[fullbox_offset + 4];
        Ok(Self { data, fullbox_offset, level_count })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

}

impl<'a> LevelAssignmentBox for LevelAssignmentBoxView<'a> {
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

    fn level_count(&self) -> u8 {
        self.level_count
    }

    fn entries(&self) -> impl Iterator<Item = Result<LevelAssignmentEntry, ParseError>> + '_ {
        let mut offset = self.fullbox_offset + 5;
        let mut remaining = self.level_count;
        let mut errored = false;

        std::iter::from_fn(move || {
            if errored || remaining == 0 {
                return None;
            }
            remaining -= 1;

            if offset + 5 > self.data.len() {
                errored = true;
                return Some(Err(ParseError::BufferTooShort {
                    expected: offset + 5,
                    found: self.data.len(),
                }));
            }

            let track_id = BigEndian::read_u32(&self.data[offset..offset + 4]);
            offset += 4;

            let flags_byte = self.data[offset];
            offset += 1;

            let padding_flag = (flags_byte >> 7) != 0;
            let assignment_type = flags_byte & 0x7F;

            let grouping_type = if assignment_type == 0 || assignment_type == 1 {
                if offset + 4 > self.data.len() {
                    errored = true;
                    return Some(Err(ParseError::BufferTooShort {
                        expected: offset + 4,
                        found: self.data.len(),
                    }));
                }
                let gt = FourCC([self.data[offset], self.data[offset + 1], self.data[offset + 2], self.data[offset + 3]]);
                offset += 4;
                Some(gt)
            } else {
                None
            };

            let grouping_type_parameter = if assignment_type == 1 {
                if offset + 4 > self.data.len() {
                    errored = true;
                    return Some(Err(ParseError::BufferTooShort {
                        expected: offset + 4,
                        found: self.data.len(),
                    }));
                }
                let gtp = BigEndian::read_u32(&self.data[offset..offset + 4]);
                offset += 4;
                Some(gtp)
            } else {
                None
            };

            let sub_track_id = if assignment_type == 4 {
                if offset + 4 > self.data.len() {
                    errored = true;
                    return Some(Err(ParseError::BufferTooShort {
                        expected: offset + 4,
                        found: self.data.len(),
                    }));
                }
                let stid = BigEndian::read_u32(&self.data[offset..offset + 4]);
                offset += 4;
                Some(stid)
            } else {
                None
            };

            Some(Ok(LevelAssignmentEntry {
                track_id,
                padding_flag,
                assignment_type,
                grouping_type,
                grouping_type_parameter,
                sub_track_id,
            }))
        })
    }
}

impl std::fmt::Debug for LevelAssignmentBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LevelAssignmentBoxView")
            .field("level_count", &self.level_count())
            .finish()
    }
}

/// An owned representation of LevelAssignmentBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct LevelAssignmentBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Level assignment entries.
    pub entries: Vec<LevelAssignmentEntry>,
}

impl LevelAssignmentBoxOwned {
    /// Creates a new LevelAssignmentBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let mut payload = 1u64; // level_count
        for entry in &self.entries {
            payload += 5; // track_id(4) + flags(1)
            if entry.assignment_type == 0 || entry.assignment_type == 1 {
                payload += 4; // grouping_type
            }
            if entry.assignment_type == 1 {
                payload += 4; // grouping_type_parameter
            }
            if entry.assignment_type == 4 {
                payload += 4; // sub_track_id
            }
        }
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u8(self.entries.len() as u8)?;

        for entry in &self.entries {
            writer.write_u32::<BigEndian>(entry.track_id)?;
            let flags_byte = ((entry.padding_flag as u8) << 7) | (entry.assignment_type & 0x7F);
            writer.write_u8(flags_byte)?;

            if entry.assignment_type == 0 || entry.assignment_type == 1 {
                writer.write_all(&entry.grouping_type.unwrap_or(FourCC([0; 4])).0)?;
            }
            if entry.assignment_type == 1 {
                writer.write_u32::<BigEndian>(entry.grouping_type_parameter.unwrap_or(0))?;
            }
            if entry.assignment_type == 4 {
                writer.write_u32::<BigEndian>(entry.sub_track_id.unwrap_or(0))?;
            }
        }

        Ok(())
    }
}


impl LevelAssignmentBox for LevelAssignmentBoxOwned {
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

    fn level_count(&self) -> u8 {
        self.entries.len() as u8
    }

    fn entries(&self) -> impl Iterator<Item = Result<LevelAssignmentEntry, ParseError>> + '_ {
        self.entries.iter().copied().map(Ok)
    }
}

impl TryFrom<&LevelAssignmentBoxView<'_>> for LevelAssignmentBoxOwned {
    type Error = ParseError;

    fn try_from(source: &LevelAssignmentBoxView<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            flags: source.flags(),
            entries: source.entries().collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_leva() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 1 + 5 + 4 = 22 bytes (assignment_type=0, has grouping_type)
        data.extend_from_slice(&22u32.to_be_bytes());
        data.extend_from_slice(b"leva");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.push(1); // level_count
        data.extend_from_slice(&1u32.to_be_bytes()); // track_id
        data.push(0x00); // padding_flag=0, assignment_type=0
        data.extend_from_slice(b"roll"); // grouping_type
        data
    }

    #[test]
    fn parse_leva() {
        let data = make_leva();
        let view = LevelAssignmentBoxView::new(&data).unwrap();

        assert_eq!(view.level_count(), 1);
        let entries: Vec<_> = view.entries().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].track_id, 1);
        assert!(!entries[0].padding_flag);
        assert_eq!(entries[0].assignment_type, 0);
        assert_eq!(entries[0].grouping_type, Some(FourCC(*b"roll")));
    }

    #[test]
    fn roundtrip() {
        let data = make_leva();
        let view = LevelAssignmentBoxView::new(&data).unwrap();
        let owned = LevelAssignmentBoxOwned::try_from(&view).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
