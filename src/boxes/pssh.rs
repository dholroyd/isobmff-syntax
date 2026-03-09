//! Protection System Specific Header Box (pssh) parsing and serialization.
//!
//! The Protection System Specific Header Box contains system-specific
//! information for content protection.
//!
//! ```text
//! aligned(8) class ProtectionSystemSpecificHeaderBox
//!    extends FullBox('pssh', version, 0) {
//!    unsigned int(8)[16] SystemID;
//!    if (version > 0) {
//!       unsigned int(32) KID_count;
//!       {
//!          unsigned int(8)[16] KID;
//!       } [KID_count];
//!    }
//!    unsigned int(32) DataSize;
//!    unsigned int(8)[DataSize] Data;
//! }
//! ```

use crate::entries::FixedSizeEntries;
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ProtectionSystemSpecificHeaderBox.
pub const BOX_TYPE: BoxCode = BoxCode::PSSH;

/// Common interface for accessing ProtectionSystemSpecificHeaderBox data.
pub trait ProtectionSystemSpecificHeaderBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the system ID (16 bytes).
    fn system_id(&self) -> [u8; 16];

    /// Returns the number of key IDs.
    ///
    /// Always zero for version 0, which has no `KID` array.
    fn kid_count(&self) -> u32;

    /// Returns the key ID at the given index (0-based), or `None` if out of range.
    fn key_id(&self, index: usize) -> Option<[u8; 16]>;

    /// Returns an iterator over the key IDs.
    ///
    /// Yields nothing for version 0, which has no `KID` array.
    fn key_ids(&self) -> impl Iterator<Item = [u8; 16]> + '_;

    /// Returns the data portion.
    fn pssh_data(&self) -> &[u8];
}

/// A borrowing view over raw ProtectionSystemSpecificHeaderBox bytes.
#[derive(Clone, Copy)]
pub struct ProtectionSystemSpecificHeaderBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    key_ids: FixedSizeEntries<'a, 16>,
    pssh_data: &'a [u8],
}

impl<'a> ProtectionSystemSpecificHeaderBoxView<'a> {
    /// Creates a new view over the given bytes.
    ///
    /// The `KID` array and `Data` extents are validated here, so the accessors
    /// cannot subsequently fail.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        // v0: system_id(16) + data_size(4) = 20
        // v1: system_id(16) + KID_count(4) + data_size(4) = 24
        let min_payload = if header.version >= 1 { 24 } else { 20 };
        let fullbox_offset = header.validate(data, BOX_TYPE, None, min_payload)?;

        // `fullbox_offset` addresses the version/flags word, so the SystemID
        // starts 4 bytes further on.
        let after_system_id = fullbox_offset + 4 + 16;

        let (key_ids, data_size_offset) = if header.version >= 1 {
            let kid_count = BigEndian::read_u32(&data[after_system_id..after_system_id + 4]);
            let kid_start = after_system_id + 4;
            let key_ids = FixedSizeEntries::new(&data[kid_start..], kid_count)?;
            let data_size_offset = (key_ids.count() as usize)
                .checked_mul(16)
                .and_then(|len| len.checked_add(kid_start))
                .ok_or(ParseError::UnexpectedEndOfData { context: "pssh KID array" })?;
            (key_ids, data_size_offset)
        } else {
            (FixedSizeEntries::new(&[], 0)?, after_system_id)
        };

        let data_start = data_size_offset
            .checked_add(4)
            .ok_or(ParseError::UnexpectedEndOfData { context: "pssh DataSize" })?;
        if data_start > data.len() {
            return Err(ParseError::BufferTooShort {
                expected: data_start,
                found: data.len(),
            });
        }
        let data_size = BigEndian::read_u32(&data[data_size_offset..data_start]) as usize;
        let data_end = data_start
            .checked_add(data_size)
            .ok_or(ParseError::UnexpectedEndOfData { context: "pssh Data" })?;
        if data_end > data.len() {
            return Err(ParseError::BufferTooShort {
                expected: data_end,
                found: data.len(),
            });
        }

        Ok(Self {
            data,
            fullbox_offset,
            key_ids,
            pssh_data: &data[data_start..data_end],
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns an iterator over the key IDs.
    ///
    /// Yields nothing for version 0, which has no `KID` array.
    pub fn key_ids(&self) -> impl Iterator<Item = [u8; 16]> + 'a {
        self.key_ids.iter().copied()
    }

    /// Returns the data portion.
    pub fn pssh_data(&self) -> &'a [u8] {
        self.pssh_data
    }
}

impl<'a> ProtectionSystemSpecificHeaderBox for ProtectionSystemSpecificHeaderBoxView<'a> {
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

    fn system_id(&self) -> [u8; 16] {
        let mut id = [0u8; 16];
        id.copy_from_slice(&self.data[self.fullbox_offset + 4..self.fullbox_offset + 20]);
        id
    }

    fn kid_count(&self) -> u32 {
        self.key_ids.count()
    }

    fn key_id(&self, index: usize) -> Option<[u8; 16]> {
        self.key_ids.get(index).copied()
    }

    fn key_ids(&self) -> impl Iterator<Item = [u8; 16]> + '_ {
        self.key_ids()
    }

    fn pssh_data(&self) -> &[u8] {
        self.pssh_data()
    }
}

impl std::fmt::Debug for ProtectionSystemSpecificHeaderBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtectionSystemSpecificHeaderBoxView")
            .field("version", &self.version())
            .field("system_id", &self.system_id())
            .field("kid_count", &self.kid_count())
            .finish()
    }
}

/// An owned representation of ProtectionSystemSpecificHeaderBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtectionSystemSpecificHeaderBoxOwned {
    /// Version (0 or 1).
    pub version: u8,
    /// Flags.
    pub flags: u32,
    /// System ID (16 bytes).
    pub system_id: [u8; 16],
    /// Key IDs. Only serialized, and only reported by the accessors, when
    /// `version >= 1`; version 0 has no `KID` array.
    pub key_ids: Vec<[u8; 16]>,
    /// Data.
    pub data: Vec<u8>,
}

impl ProtectionSystemSpecificHeaderBoxOwned {
    /// Creates a new ProtectionSystemSpecificHeaderBoxOwned.
    pub fn new(system_id: [u8; 16]) -> Self {
        Self {
            version: 0,
            flags: 0,
            system_id,
            key_ids: Vec::new(),
            data: Vec::new(),
        }
    }

    /// Returns the key IDs that this box will serialize, which is nothing at
    /// version 0.
    fn stored_key_ids(&self) -> &[[u8; 16]] {
        if self.version >= 1 { &self.key_ids } else { &[] }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = if self.version >= 1 {
            (16 + 4 + self.key_ids.len() * 16 + 4 + self.data.len()) as u64
        } else {
            (16 + 4 + self.data.len()) as u64
        };
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, self.version, self.flags)?;
        writer.write_all(&self.system_id)?;
        if self.version >= 1 {
            writer.write_u32::<BigEndian>(self.key_ids.len() as u32)?;
            for kid in &self.key_ids {
                writer.write_all(kid)?;
            }
        }
        writer.write_u32::<BigEndian>(self.data.len() as u32)?;
        writer.write_all(&self.data)?;

        Ok(())
    }
}

impl Default for ProtectionSystemSpecificHeaderBoxOwned {
    fn default() -> Self {
        Self::new([0u8; 16])
    }
}

impl ProtectionSystemSpecificHeaderBox for ProtectionSystemSpecificHeaderBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.version
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn system_id(&self) -> [u8; 16] {
        self.system_id
    }

    fn kid_count(&self) -> u32 {
        self.stored_key_ids().len() as u32
    }

    fn key_id(&self, index: usize) -> Option<[u8; 16]> {
        self.stored_key_ids().get(index).copied()
    }

    fn key_ids(&self) -> impl Iterator<Item = [u8; 16]> + '_ {
        self.stored_key_ids().iter().copied()
    }

    fn pssh_data(&self) -> &[u8] {
        &self.data
    }
}

impl<T: ProtectionSystemSpecificHeaderBox> From<&T> for ProtectionSystemSpecificHeaderBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            version: source.version(),
            flags: source.flags(),
            system_id: source.system_id(),
            key_ids: source.key_ids().collect(),
            data: source.pssh_data().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pssh_v0() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&32u32.to_be_bytes()); // 8 + 4 + 16 + 4
        data.extend_from_slice(b"pssh");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&[0u8; 16]); // system_id
        data.extend_from_slice(&0u32.to_be_bytes()); // data_size
        data
    }

    fn make_pssh_v1(kids: &[[u8; 16]], pssh_data: &[u8]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&[0u8; 16]); // system_id
        payload.extend_from_slice(&(kids.len() as u32).to_be_bytes());
        for kid in kids {
            payload.extend_from_slice(kid);
        }
        payload.extend_from_slice(&(pssh_data.len() as u32).to_be_bytes());
        payload.extend_from_slice(pssh_data);

        let mut data = Vec::new();
        data.extend_from_slice(&(12 + payload.len() as u32).to_be_bytes());
        data.extend_from_slice(b"pssh");
        data.push(1); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&payload);
        data
    }

    #[test]
    fn parse_pssh_v0() {
        let data = make_pssh_v0();
        let view = ProtectionSystemSpecificHeaderBoxView::new(&data).unwrap();
        assert_eq!(view.version(), 0);
        assert_eq!(view.kid_count(), 0);
        assert_eq!(view.key_ids().count(), 0);
        assert_eq!(ProtectionSystemSpecificHeaderBox::key_id(&view, 0), None);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_pssh_v0();
        let view = ProtectionSystemSpecificHeaderBoxView::new(&data).unwrap();
        let owned = ProtectionSystemSpecificHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn parse_pssh_v1_key_ids() {
        let kids = [[0xaa; 16], [0xbb; 16]];
        let data = make_pssh_v1(&kids, &[1, 2, 3]);
        let view = ProtectionSystemSpecificHeaderBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 1);
        assert_eq!(view.kid_count(), 2);
        assert_eq!(view.key_ids().collect::<Vec<_>>(), kids);
        assert_eq!(
            ProtectionSystemSpecificHeaderBox::key_id(&view, 1),
            Some([0xbb; 16])
        );
        assert_eq!(ProtectionSystemSpecificHeaderBox::key_id(&view, 2), None);
        assert_eq!(view.pssh_data(), &[1, 2, 3]);
    }

    #[test]
    fn roundtrip_v1() {
        let data = make_pssh_v1(&[[0xaa; 16], [0xbb; 16]], &[1, 2, 3]);
        let view = ProtectionSystemSpecificHeaderBoxView::new(&data).unwrap();
        let owned = ProtectionSystemSpecificHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn owned_ignores_key_ids_at_version_0() {
        let mut owned = ProtectionSystemSpecificHeaderBoxOwned::new([0u8; 16]);
        owned.key_ids.push([0xaa; 16]);

        assert_eq!(owned.kid_count(), 0);
        assert_eq!(owned.key_ids().count(), 0);
        assert_eq!(owned.box_size(), 32);
    }

    #[test]
    fn reject_kid_count_exceeding_box() {
        let mut data = make_pssh_v1(&[[0xaa; 16]], &[]);
        // Claim 100 KIDs where only one is present.
        let kid_count_offset = 12 + 16;
        data[kid_count_offset..kid_count_offset + 4].copy_from_slice(&100u32.to_be_bytes());

        assert!(matches!(
            ProtectionSystemSpecificHeaderBoxView::new(&data),
            Err(ParseError::InvalidEntryCount { count: 100, .. })
        ));
    }

    #[test]
    fn reject_data_size_exceeding_box() {
        let mut data = make_pssh_v1(&[[0xaa; 16]], &[1, 2, 3]);
        let data_size_offset = 12 + 16 + 4 + 16;
        data[data_size_offset..data_size_offset + 4].copy_from_slice(&99u32.to_be_bytes());

        assert!(matches!(
            ProtectionSystemSpecificHeaderBoxView::new(&data),
            Err(ParseError::BufferTooShort { .. })
        ));
    }
}
