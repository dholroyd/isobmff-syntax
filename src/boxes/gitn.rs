//! Group Id To Name Box (gitn) parsing and serialization.
//!
//! The Group Id To Name Box maps group IDs to names for file delivery.
//!
//! ```text
//! aligned(8) class GroupIdToNameBox
//!    extends FullBox('gitn', version = 0, 0) {
//!    unsigned int(16) entry_count;
//!    for (i=1; i <= entry_count; i++) {
//!       unsigned int(32) group_ID;
//!       utf8string group_name;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for GroupIdToNameBox.
pub const BOX_TYPE: BoxCode = BoxCode::GITN;

/// Shared interface over a single gitn entry.
///
/// Implemented by both the borrowing [`GroupIdToNameView`] and by
/// `&GroupIdToNameOwned`.
pub trait GroupIdToName {
    /// Returns the group ID.
    fn group_id(&self) -> u32;

    /// Returns the group name (without the trailing null terminator).
    fn group_name(&self) -> &str;

    /// Materialises an owned copy of this entry.
    fn to_owned(&self) -> GroupIdToNameOwned {
        GroupIdToNameOwned {
            group_id: self.group_id(),
            group_name: self.group_name().to_owned(),
        }
    }
}

/// A borrowing view over a single gitn entry's raw bytes.
#[derive(Clone, Copy)]
pub struct GroupIdToNameView<'a> {
    group_id: u32,
    group_name: &'a str,
}

impl<'a> GroupIdToName for GroupIdToNameView<'a> {
    fn group_id(&self) -> u32 {
        self.group_id
    }

    fn group_name(&self) -> &str {
        self.group_name
    }
}

/// An owned gitn entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupIdToNameOwned {
    /// Group ID.
    pub group_id: u32,
    /// Group name (null-terminated in file).
    pub group_name: String,
}

impl GroupIdToName for &GroupIdToNameOwned {
    fn group_id(&self) -> u32 {
        self.group_id
    }

    fn group_name(&self) -> &str {
        &self.group_name
    }

    fn to_owned(&self) -> GroupIdToNameOwned {
        (*self).clone()
    }
}

/// Common interface for accessing GroupIdToNameBox data.
pub trait GroupIdToNameBox {
    /// The concrete entry type yielded by [`Self::entries`].
    type Entry<'a>: GroupIdToName
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the entry count.
    fn entry_count(&self) -> u16;

    /// Returns an iterator over all entries.
    fn entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_;
}

/// A borrowing view over raw GroupIdToNameBox bytes.
#[derive(Clone, Copy)]
pub struct GroupIdToNameBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    entry_count: u16,
}

impl<'a> GroupIdToNameBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 2)?;

        let entry_count = BigEndian::read_u16(&data[fullbox_offset + 4..fullbox_offset + 6]);

        Ok(Self {
            data,
            fullbox_offset,
            entry_count,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

}

impl<'a> GroupIdToNameBox for GroupIdToNameBoxView<'a> {
    type Entry<'b> = GroupIdToNameView<'b> where Self: 'b;

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

    fn entry_count(&self) -> u16 {
        self.entry_count
    }

    fn entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_ {
        let mut offset = self.fullbox_offset + 6;
        let mut remaining = self.entry_count;
        let mut errored = false;

        std::iter::from_fn(move || {
            if errored || remaining == 0 {
                return None;
            }
            remaining -= 1;

            if offset + 4 > self.data.len() {
                errored = true;
                return Some(Err(ParseError::BufferTooShort {
                    expected: offset + 4,
                    found: self.data.len(),
                }));
            }

            let group_id = BigEndian::read_u32(&self.data[offset..offset + 4]);
            offset += 4;

            // Find null terminator for name
            let name_start = offset;
            while offset < self.data.len() && self.data[offset] != 0 {
                offset += 1;
            }
            if offset >= self.data.len() {
                errored = true;
                return Some(Err(ParseError::UnexpectedEndOfData {
                    context: "gitn group_name (missing null terminator)",
                }));
            }
            let group_name = match std::str::from_utf8(&self.data[name_start..offset]) {
                Ok(s) => s,
                Err(_) => {
                    errored = true;
                    return Some(Err(ParseError::InvalidStringEncoding {
                        context: "gitn group_name",
                    }));
                }
            };
            offset += 1; // skip null terminator

            Some(Ok(GroupIdToNameView { group_id, group_name }))
        })
    }
}

impl std::fmt::Debug for GroupIdToNameBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GroupIdToNameBoxView")
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of GroupIdToNameBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct GroupIdToNameBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Entries.
    pub entries: Vec<GroupIdToNameOwned>,
}

impl GroupIdToNameBoxOwned {
    /// Creates a new GroupIdToNameBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let mut payload = 2u64; // entry_count
        for entry in &self.entries {
            payload += 4 + entry.group_name.len() as u64 + 1; // group_id + name + null
        }
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u16::<BigEndian>(self.entries.len() as u16)?;

        for entry in &self.entries {
            writer.write_u32::<BigEndian>(entry.group_id)?;
            writer.write_all(entry.group_name.as_bytes())?;
            writer.write_u8(0)?; // null terminator
        }

        Ok(())
    }
}


impl GroupIdToNameBox for GroupIdToNameBoxOwned {
    type Entry<'a> = &'a GroupIdToNameOwned where Self: 'a;

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

    fn entry_count(&self) -> u16 {
        self.entries.len() as u16
    }

    fn entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_ {
        self.entries.iter().map(Ok)
    }
}

impl TryFrom<&GroupIdToNameBoxView<'_>> for GroupIdToNameBoxOwned {
    type Error = ParseError;

    fn try_from(source: &GroupIdToNameBoxView<'_>) -> Result<Self, Self::Error> {
        let entries = GroupIdToNameBox::entries(source)
            .map(|res| res.map(|v| GroupIdToName::to_owned(&v)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            flags: source.flags(),
            entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_gitn() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 2 = 14 bytes (empty)
        data.extend_from_slice(&14u32.to_be_bytes());
        data.extend_from_slice(b"gitn");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&0u16.to_be_bytes()); // entry_count
        data
    }

    #[test]
    fn parse_gitn() {
        let data = make_gitn();
        let view = GroupIdToNameBoxView::new(&data).unwrap();
        assert_eq!(view.entry_count(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_gitn();
        let view = GroupIdToNameBoxView::new(&data).unwrap();
        let owned = GroupIdToNameBoxOwned::try_from(&view).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
