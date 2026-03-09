//! FD Session Group Box (segr) parsing and serialization.
//!
//! The FD Session Group Box specifies session groups for file delivery.
//!
//! ```text
//! aligned(8) class FDSessionGroupBox extends Box('segr') {
//!    unsigned int(16) num_session_groups;
//!    for(i=0; i < num_session_groups; i++) {
//!       unsigned int(8) entry_count;
//!       for (j=0; j < entry_count; j++) {
//!          unsigned int(32) group_ID;
//!       }
//!       unsigned int(16) num_channels_in_session_group;
//!       for(k=0; k < num_channels_in_session_group; k++) {
//!          unsigned int(32) hint_track_ID;
//!       }
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for FDSessionGroupBox.
pub const BOX_TYPE: BoxCode = BoxCode::SEGR;

/// A session group entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionGroupEntry {
    /// Group IDs in this session group.
    pub entry_ids: Vec<u32>,
    /// Hint track IDs in this session group.
    pub hint_track_ids: Vec<u32>,
}

/// Common interface for accessing FDSessionGroupBox data.
pub trait FDSessionGroupBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the number of session groups.
    fn num_session_groups(&self) -> u16;

    /// Returns all session groups.
    fn session_groups(&self) -> Vec<SessionGroupEntry>;
}

/// Parses session groups from the given data starting at the given offset.
fn parse_session_groups(data: &[u8], start: usize, count: u16) -> Vec<SessionGroupEntry> {
    let mut result = Vec::new();
    let mut offset = start;

    for _ in 0..count {
        if offset >= data.len() {
            break;
        }

        // Read entry_count (u8) and group_IDs
        let entry_count = data[offset] as usize;
        offset += 1;

        let mut entry_ids = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            if offset + 4 > data.len() {
                break;
            }
            entry_ids.push(BigEndian::read_u32(&data[offset..offset + 4]));
            offset += 4;
        }

        // Read num_channels_in_session_group (u16) and hint_track_IDs
        let mut hint_track_ids = Vec::new();
        if offset + 2 <= data.len() {
            let num_channels = BigEndian::read_u16(&data[offset..offset + 2]) as usize;
            offset += 2;

            hint_track_ids.reserve(num_channels);
            for _ in 0..num_channels {
                if offset + 4 > data.len() {
                    break;
                }
                hint_track_ids.push(BigEndian::read_u32(&data[offset..offset + 4]));
                offset += 4;
            }
        }

        result.push(SessionGroupEntry {
            entry_ids,
            hint_track_ids,
        });
    }

    result
}

/// A borrowing view over raw FDSessionGroupBox bytes.
#[derive(Clone, Copy)]
pub struct FDSessionGroupBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
    num_session_groups: u16,
}

impl<'a> FDSessionGroupBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 2)?;
        let header_size = header.header_size as usize;

        let num_session_groups = BigEndian::read_u16(&data[header_size..header_size + 2]);

        Ok(Self {
            data,
            header_size,
            num_session_groups,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns all session groups.
    pub fn session_groups(&self) -> Vec<SessionGroupEntry> {
        parse_session_groups(self.data, self.header_size + 2, self.num_session_groups)
    }
}

impl<'a> FDSessionGroupBox for FDSessionGroupBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn num_session_groups(&self) -> u16 {
        self.num_session_groups
    }

    fn session_groups(&self) -> Vec<SessionGroupEntry> {
        self.session_groups()
    }
}

impl std::fmt::Debug for FDSessionGroupBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FDSessionGroupBoxView")
            .field("num_session_groups", &self.num_session_groups())
            .finish()
    }
}

/// An owned representation of FDSessionGroupBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct FDSessionGroupBoxOwned {
    /// Session groups.
    pub session_groups: Vec<SessionGroupEntry>,
}

impl FDSessionGroupBoxOwned {
    /// Creates a new FDSessionGroupBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let mut payload: usize = 2; // num_session_groups
        for group in &self.session_groups {
            payload += 1 + group.entry_ids.len() * 4; // entry_count + group_IDs
            payload += 2 + group.hint_track_ids.len() * 4; // num_channels + hint_track_IDs
        }
        let payload = payload as u64;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_u16::<BigEndian>(self.session_groups.len() as u16)?;

        for group in &self.session_groups {
            writer.write_u8(group.entry_ids.len() as u8)?;
            for &id in &group.entry_ids {
                writer.write_u32::<BigEndian>(id)?;
            }
            writer.write_u16::<BigEndian>(group.hint_track_ids.len() as u16)?;
            for &id in &group.hint_track_ids {
                writer.write_u32::<BigEndian>(id)?;
            }
        }

        Ok(())
    }
}


impl FDSessionGroupBox for FDSessionGroupBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn num_session_groups(&self) -> u16 {
        self.session_groups.len() as u16
    }

    fn session_groups(&self) -> Vec<SessionGroupEntry> {
        self.session_groups.clone()
    }
}

impl<T: FDSessionGroupBox> From<&T> for FDSessionGroupBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            session_groups: source.session_groups(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_segr() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 2 = 10 bytes (empty, no session groups)
        data.extend_from_slice(&10u32.to_be_bytes());
        data.extend_from_slice(b"segr");
        data.extend_from_slice(&0u16.to_be_bytes()); // num_session_groups
        data
    }

    fn make_segr_with_entries() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 2 + (1 + 4 + 2 + 4) = 21 bytes
        data.extend_from_slice(&21u32.to_be_bytes());
        data.extend_from_slice(b"segr");
        data.extend_from_slice(&1u16.to_be_bytes()); // num_session_groups = 1
        data.push(1); // entry_count = 1
        data.extend_from_slice(&42u32.to_be_bytes()); // group_ID
        data.extend_from_slice(&1u16.to_be_bytes()); // num_channels_in_session_group = 1
        data.extend_from_slice(&99u32.to_be_bytes()); // hint_track_ID
        data
    }

    #[test]
    fn parse_segr() {
        let data = make_segr();
        let view = FDSessionGroupBoxView::new(&data).unwrap();
        assert_eq!(view.num_session_groups(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_segr();
        let view = FDSessionGroupBoxView::new(&data).unwrap();
        let owned = FDSessionGroupBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn parse_segr_with_entries() {
        let data = make_segr_with_entries();
        let view = FDSessionGroupBoxView::new(&data).unwrap();
        assert_eq!(view.num_session_groups(), 1);
        let groups = view.session_groups();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].entry_ids, vec![42]);
        assert_eq!(groups[0].hint_track_ids, vec![99]);
    }

    #[test]
    fn roundtrip_with_entries() {
        let data = make_segr_with_entries();
        let view = FDSessionGroupBoxView::new(&data).unwrap();
        let owned = FDSessionGroupBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
