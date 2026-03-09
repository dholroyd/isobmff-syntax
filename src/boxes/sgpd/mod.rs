//! Sample Group Description Box (sgpd) parsing and serialization.
//!
//! The Sample Group Description Box provides descriptions for sample groups.
//!
//! ```text
//! aligned(8) class SampleGroupDescriptionBox ()
//!    extends FullBox('sgpd', version, flags) {
//!    unsigned int(32) grouping_type;
//!    if (version>=1) { unsigned int(32) default_length; }
//!    if (version>=2) {
//!       unsigned int(32) default_group_description_index;
//!    }
//!    unsigned int(32) entry_count;
//!    for (i = 1 ; i <= entry_count ; i++){
//!       if (version>=1) {
//!          if (default_length==0) {
//!             unsigned int(32) description_length;
//!          }
//!       }
//!       SampleGroupDescriptionEntry (grouping_type);
//!    }
//! }
//! ```

pub mod entries;

pub use entries::SampleGroupEntry;

use crate::error::{validate_entry_count, ParseError};
use crate::header::{fullbox_header_size_for_payload, write_fullbox_header, FullBoxHeader};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use entries::parse_entry;
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for SampleGroupDescriptionBox.
pub const BOX_TYPE: BoxCode = BoxCode::SGPD;

/// Common interface for accessing SampleGroupDescriptionBox data.
pub trait SampleGroupDescriptionBox {
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

    /// Returns the default length (version >= 1).
    fn default_length(&self) -> Option<u32>;

    /// Returns the default sample description index (version >= 2).
    fn default_sample_description_index(&self) -> Option<u32>;

    /// Returns the entry count.
    fn entry_count(&self) -> u32;

    /// Returns a typed iterator over parsed sample group entries.
    fn entries(&self) -> Vec<SampleGroupEntry>;
}

/// A borrowing view over raw SampleGroupDescriptionBox bytes.
#[derive(Clone, Copy)]
pub struct SampleGroupDescriptionBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
    entry_count: u32,
    entries_offset: usize,
    default_length: u32,
}

impl<'a> SampleGroupDescriptionBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;

        if header.box_type() != BOX_TYPE {
            return Err(ParseError::InvalidBoxType {
                expected: BOX_TYPE,
                found: header.box_header.box_type,
            });
        }

        if header.size() != data.len() as u64 {
            return Err(ParseError::SizeMismatch {
                declared: header.size(),
                actual: data.len(),
            });
        }

        let version = header.version;
        let fullbox_offset = header.box_header.header_size as usize;

        // Calculate header size based on version
        let (header_payload_size, default_length) = match version {
            0 => (8, 0u32), // grouping_type(4) + entry_count(4)
            1 => {
                let min = fullbox_offset + 12;
                if data.len() < min {
                    return Err(ParseError::BufferTooShort {
                        expected: min,
                        found: data.len(),
                    });
                }
                let dl = BigEndian::read_u32(&data[fullbox_offset + 8..fullbox_offset + 12]);
                (12, dl) // grouping_type(4) + default_length(4) + entry_count(4)
            }
            _ => {
                let min = fullbox_offset + 16;
                if data.len() < min {
                    return Err(ParseError::BufferTooShort {
                        expected: min,
                        found: data.len(),
                    });
                }
                let dl = BigEndian::read_u32(&data[fullbox_offset + 8..fullbox_offset + 12]);
                (16, dl) // grouping_type(4) + default_length(4) + default_sdi(4) + entry_count(4)
            }
        };

        let min_size = fullbox_offset + 4 + header_payload_size;
        if data.len() < min_size {
            return Err(ParseError::BufferTooShort {
                expected: min_size,
                found: data.len(),
            });
        }

        let entry_count_offset = fullbox_offset + 4 + header_payload_size - 4;
        let entry_count = BigEndian::read_u32(&data[entry_count_offset..entry_count_offset + 4]);
        let entries_offset = fullbox_offset + 4 + header_payload_size;

        // Bound `entry_count` by the bytes actually available, so that a hostile
        // count cannot make `entries()` iterate billions of times. The smallest
        // an entry can be depends on how its length is expressed: a fixed
        // `default_length`, or a 4-byte per-entry length prefix. Version 0 states
        // neither, so all that can be said is that an entry takes up at least a
        // byte; `entry_data` cannot locate version 0 entries in any case.
        let min_entry_size = if default_length > 0 {
            default_length as usize
        } else if version >= 1 {
            4
        } else {
            1
        };
        validate_entry_count(data, entries_offset, entry_count, min_entry_size)?;

        Ok(Self {
            data,
            fullbox_offset,
            version,
            entry_count,
            entries_offset,
            default_length,
        })
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

    /// Returns the raw entry data at the given index.
    ///
    /// This is an inherent method (not on the trait) providing raw byte access
    /// to individual entries, supporting both fixed-size and variable-size entries.
    pub fn entry_data(&self, index: usize) -> Option<&'a [u8]> {
        if index >= self.entry_count as usize {
            return None;
        }

        if self.default_length > 0 {
            // Fixed-size entries
            let entry_size = self.default_length as usize;
            let start = self.entries_offset.checked_add(index.checked_mul(entry_size)?)?;
            let end = start.checked_add(entry_size)?;
            if end <= self.data.len() {
                Some(&self.data[start..end])
            } else {
                None
            }
        } else if self.version >= 1 {
            // Variable-size entries (each prefixed with description_length)
            let mut offset = self.entries_offset;
            for i in 0..=index {
                if offset + 4 > self.data.len() {
                    return None;
                }
                let entry_len = BigEndian::read_u32(&self.data[offset..offset + 4]) as usize;
                offset += 4;
                if i == index {
                    let end = match offset.checked_add(entry_len) {
                        Some(e) if e <= self.data.len() => e,
                        _ => return None,
                    };
                    return Some(&self.data[offset..end]);
                }
                offset = offset.checked_add(entry_len)?;
            }
            None
        } else {
            // Version 0: no default_length field, entries run to end of box
            // Cannot determine individual entry boundaries without knowing the grouping type
            None
        }
    }

    /// Returns the raw entries data bytes (all entries, starting after entry_count).
    pub fn entries_raw_data(&self) -> &'a [u8] {
        &self.data[self.entries_offset..]
    }
}

impl SampleGroupDescriptionBox for SampleGroupDescriptionBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.version
    }

    fn flags(&self) -> u32 {
        BigEndian::read_u24(&self.data[self.fullbox_offset + 1..self.fullbox_offset + 4])
    }

    fn grouping_type(&self) -> FourCC {
        let o = self.payload_offset();
        FourCC([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]])
    }

    fn default_length(&self) -> Option<u32> {
        if self.version >= 1 {
            Some(self.default_length)
        } else {
            None
        }
    }

    fn default_sample_description_index(&self) -> Option<u32> {
        if self.version >= 2 {
            let o = self.payload_offset() + 8;
            Some(BigEndian::read_u32(&self.data[o..o + 4]))
        } else {
            None
        }
    }

    fn entry_count(&self) -> u32 {
        self.entry_count
    }

    fn entries(&self) -> Vec<SampleGroupEntry> {
        let gt = self.grouping_type();
        (0..self.entry_count as usize)
            .filter_map(|i| {
                let data = self.entry_data(i)?;
                parse_entry(&gt, data).ok()
            })
            .collect()
    }
}

impl std::fmt::Debug for SampleGroupDescriptionBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleGroupDescriptionBoxView")
            .field("version", &self.version())
            .field("grouping_type", &self.grouping_type())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of SampleGroupDescriptionBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleGroupDescriptionBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Grouping type.
    pub grouping_type: FourCC,
    /// Default sample description index (version 2+).
    pub default_sample_description_index: Option<u32>,
    /// Typed entries.
    pub entries: Vec<SampleGroupEntry>,
}

impl SampleGroupDescriptionBoxOwned {
    /// Creates a new SampleGroupDescriptionBoxOwned with no entries.
    pub fn new(grouping_type: FourCC) -> Self {
        Self {
            flags: 0,
            grouping_type,
            default_sample_description_index: None,
            entries: Vec::new(),
        }
    }

    fn version(&self) -> u8 {
        if self.default_sample_description_index.is_some() {
            2
        } else {
            1 // Use v1 by default for new boxes
        }
    }

    /// Computes the default_length: if all entries have the same serialized size,
    /// returns that size; otherwise returns 0 (variable-length).
    fn computed_default_length(&self) -> u32 {
        if self.entries.is_empty() {
            return 0;
        }
        let first_size = self.entries[0].serialized_size();
        if self.entries.iter().all(|e| e.serialized_size() == first_size) {
            first_size as u32
        } else {
            0
        }
    }

    /// Returns the serialized size of the entry data portion.
    fn entries_data_size(&self) -> u64 {
        let dl = self.computed_default_length();
        if dl > 0 {
            // Fixed-size: no length prefixes
            self.entries.len() as u64 * dl as u64
        } else {
            // Variable-size: each entry prefixed with 4-byte description_length
            self.entries
                .iter()
                .map(|e| 4u64 + e.serialized_size() as u64)
                .sum()
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let version = self.version();
        let version_fields: u64 = match version {
            0 => 8,
            1 => 12,
            _ => 16,
        };
        let payload = version_fields + self.entries_data_size();
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let version = self.version();
        let size = self.serialized_size();
        let default_length = self.computed_default_length();
        write_fullbox_header(writer, size, BOX_TYPE, version, self.flags)?;
        writer.write_all(&self.grouping_type.0)?;

        if version >= 1 {
            writer.write_u32::<BigEndian>(default_length)?;
        }
        if version >= 2 {
            writer.write_u32::<BigEndian>(
                self.default_sample_description_index.unwrap_or(0),
            )?;
        }

        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

        for entry in &self.entries {
            if default_length == 0 && version >= 1 {
                writer.write_u32::<BigEndian>(entry.serialized_size() as u32)?;
            }
            entry.write_to(writer)?;
        }

        Ok(())
    }
}

impl SampleGroupDescriptionBox for SampleGroupDescriptionBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.version()
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn grouping_type(&self) -> FourCC {
        self.grouping_type
    }

    fn default_length(&self) -> Option<u32> {
        if self.version() >= 1 {
            Some(self.computed_default_length())
        } else {
            None
        }
    }

    fn default_sample_description_index(&self) -> Option<u32> {
        self.default_sample_description_index
    }

    fn entry_count(&self) -> u32 {
        self.entries.len() as u32
    }

    fn entries(&self) -> Vec<SampleGroupEntry> {
        self.entries.clone()
    }
}

impl<T: SampleGroupDescriptionBox> From<&T> for SampleGroupDescriptionBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            grouping_type: source.grouping_type(),
            default_sample_description_index: source.default_sample_description_index(),
            entries: source.entries(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use entries::RollRecoveryEntry;

    fn make_sgpd_v1_roll() -> Vec<u8> {
        let mut data = Vec::new();
        // Roll recovery entry is 2 bytes (roll_distance as i16)
        data.extend_from_slice(&26u32.to_be_bytes()); // size = 8 + 4 + 12 + 2
        data.extend_from_slice(b"sgpd");
        data.push(1); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(b"roll"); // grouping_type
        data.extend_from_slice(&2u32.to_be_bytes()); // default_length
        data.extend_from_slice(&1u32.to_be_bytes()); // entry_count
        data.extend_from_slice(&(-2i16).to_be_bytes()); // roll_distance = -2
        data
    }

    #[test]
    fn parse_sgpd_v1() {
        let data = make_sgpd_v1_roll();
        let view = SampleGroupDescriptionBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 1);
        assert_eq!(view.grouping_type(), FourCC(*b"roll"));
        assert_eq!(view.default_length(), Some(2));
        assert_eq!(view.entry_count(), 1);

        // Raw entry data access
        let entry = view.entry_data(0).unwrap();
        assert_eq!(entry.len(), 2);
        assert_eq!(BigEndian::read_i16(entry), -2);

        // Typed entry access
        let entries = view.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0],
            SampleGroupEntry::RollRecovery(RollRecoveryEntry { roll_distance: -2 })
        );
    }

    #[test]
    fn roundtrip_v1() {
        let data = make_sgpd_v1_roll();
        let view = SampleGroupDescriptionBoxView::new(&data).unwrap();
        let owned = SampleGroupDescriptionBoxOwned::from(&view);

        assert_eq!(owned.entries.len(), 1);
        assert_eq!(
            owned.entries[0],
            SampleGroupEntry::RollRecovery(RollRecoveryEntry { roll_distance: -2 })
        );

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn owned_construction() {
        let mut owned = SampleGroupDescriptionBoxOwned::new(FourCC(*b"roll"));
        owned
            .entries
            .push(SampleGroupEntry::RollRecovery(RollRecoveryEntry {
                roll_distance: -3,
            }));
        owned
            .entries
            .push(SampleGroupEntry::RollRecovery(RollRecoveryEntry {
                roll_distance: -1,
            }));

        assert_eq!(owned.entry_count(), 2);
        assert_eq!(owned.default_length(), Some(2));

        // Roundtrip through serialization
        let mut buf = Vec::new();
        owned.write_to(&mut buf).unwrap();
        let view = SampleGroupDescriptionBoxView::new(&buf).unwrap();
        assert_eq!(view.entry_count(), 2);

        let parsed_entries = view.entries();
        assert_eq!(parsed_entries, owned.entries);
    }

    fn make_sgpd_v1_rap() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&25u32.to_be_bytes()); // size = 8 + 4 + 12 + 1
        data.extend_from_slice(b"sgpd");
        data.push(1); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(b"rap "); // grouping_type
        data.extend_from_slice(&1u32.to_be_bytes()); // default_length
        data.extend_from_slice(&1u32.to_be_bytes()); // entry_count
        // num_leading_samples_known=1, num_leading_samples=5
        data.push(0x85); // 1_0000101
        data
    }

    #[test]
    fn parse_rap_entry() {
        let data = make_sgpd_v1_rap();
        let view = SampleGroupDescriptionBoxView::new(&data).unwrap();
        let entries = view.entries();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            SampleGroupEntry::VisualRandomAccess(e) => {
                assert!(e.num_leading_samples_known);
                assert_eq!(e.num_leading_samples, 5);
            }
            _ => panic!("expected VisualRandomAccess"),
        }

        // Roundtrip
        let owned = SampleGroupDescriptionBoxOwned::from(&view);
        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    fn make_sgpd_v1_seig() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&44u32.to_be_bytes()); // size = 8 + 4 + 12 + 20
        data.extend_from_slice(b"sgpd");
        data.push(1); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(b"seig"); // grouping_type
        data.extend_from_slice(&20u32.to_be_bytes()); // default_length
        data.extend_from_slice(&1u32.to_be_bytes()); // entry_count
        // seig entry: crypt_byte_block=1, skip_byte_block=9, is_protected=1, per_sample_iv_size=8
        data.push(0x01); // reserved(4) + crypt_byte_block(4)
        data.push(0x09); // reserved(4) + skip_byte_block(4)
        data.push(1); // is_protected
        data.push(8); // per_sample_iv_size
        data.extend_from_slice(&[
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ]); // KID
        data
    }

    #[test]
    fn parse_seig_entry() {
        let data = make_sgpd_v1_seig();
        let view = SampleGroupDescriptionBoxView::new(&data).unwrap();
        let entries = view.entries();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            SampleGroupEntry::CencSampleEncryption(e) => {
                assert_eq!(e.crypt_byte_block, 1);
                assert_eq!(e.skip_byte_block, 9);
                assert_eq!(e.is_protected, 1);
                assert_eq!(e.per_sample_iv_size, 8);
                assert_eq!(e.kid[0], 0x01);
                assert_eq!(e.kid[15], 0x10);
                assert!(e.constant_iv.is_none());
            }
            _ => panic!("expected CencSampleEncryption"),
        }

        // Roundtrip
        let owned = SampleGroupDescriptionBoxOwned::from(&view);
        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    fn make_sgpd_v1_sync() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&25u32.to_be_bytes()); // size = 8 + 4 + 12 + 1
        data.extend_from_slice(b"sgpd");
        data.push(1); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(b"sync"); // grouping_type
        data.extend_from_slice(&1u32.to_be_bytes()); // default_length
        data.extend_from_slice(&1u32.to_be_bytes()); // entry_count
        data.push(5); // nal_unit_type = 5
        data
    }

    #[test]
    fn parse_sync_entry() {
        let data = make_sgpd_v1_sync();
        let view = SampleGroupDescriptionBoxView::new(&data).unwrap();
        let entries = view.entries();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            SampleGroupEntry::SyncSample(e) => {
                assert_eq!(e.nal_unit_type, 5);
            }
            _ => panic!("expected SyncSample"),
        }

        // Roundtrip
        let owned = SampleGroupDescriptionBoxOwned::from(&view);
        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn variable_length_entries() {
        use entries::SampleToMetadataItemEntry;

        // Build an sgpd with variable-length stmi entries
        let mut owned = SampleGroupDescriptionBoxOwned::new(FourCC(*b"stmi"));
        owned.entries.push(SampleGroupEntry::SampleToMetadataItem(
            SampleToMetadataItemEntry {
                meta_box_handler_type: FourCC(*b"pict"),
                item_ids: vec![1, 2],
            },
        ));
        owned.entries.push(SampleGroupEntry::SampleToMetadataItem(
            SampleToMetadataItemEntry {
                meta_box_handler_type: FourCC(*b"pict"),
                item_ids: vec![10],
            },
        ));

        // These have different sizes (16 vs 12), so default_length should be 0
        assert_eq!(owned.computed_default_length(), 0);

        let mut buf = Vec::new();
        owned.write_to(&mut buf).unwrap();

        let view = SampleGroupDescriptionBoxView::new(&buf).unwrap();
        assert_eq!(view.entry_count(), 2);
        assert_eq!(view.default_length(), Some(0));

        let parsed = view.entries();
        assert_eq!(parsed, owned.entries);
    }
}
