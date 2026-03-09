//! Primary Item Box (pitm) parsing and serialization.
//!
//! The Primary Item Box indicates the primary item.
//!
//! ```text
//! aligned(8) class PrimaryItemBox
//!    extends FullBox('pitm', version, 0) {
//!    if (version == 0) {
//!       unsigned int(16) item_ID;
//!    } else {
//!       unsigned int(32) item_ID;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for PrimaryItemBox.
pub const BOX_TYPE: BoxCode = BoxCode::PITM;

/// Common interface for accessing PrimaryItemBox data.
pub trait PrimaryItemBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the item ID of the primary item.
    fn item_id(&self) -> u32;
}

/// A borrowing view over raw PrimaryItemBox bytes.
#[derive(Clone, Copy)]
pub struct PrimaryItemBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
}

impl<'a> PrimaryItemBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let payload_size = if header.version == 0 { 2 } else { 4 };
        let fullbox_offset = header.validate(data, BOX_TYPE, None, payload_size)?;
        let version = header.version;
        Ok(Self { data, fullbox_offset, version })
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
}

impl PrimaryItemBox for PrimaryItemBoxView<'_> {
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

    fn item_id(&self) -> u32 {
        let o = self.payload_offset();
        if self.version == 0 {
            BigEndian::read_u16(&self.data[o..o + 2]) as u32
        } else {
            BigEndian::read_u32(&self.data[o..o + 4])
        }
    }
}

impl std::fmt::Debug for PrimaryItemBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimaryItemBoxView")
            .field("version", &self.version())
            .field("item_id", &self.item_id())
            .finish()
    }
}

/// An owned representation of PrimaryItemBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrimaryItemBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Item ID of the primary item.
    pub item_id: u32,
}

impl PrimaryItemBoxOwned {
    /// Creates a new PrimaryItemBoxOwned.
    pub fn new(item_id: u32) -> Self {
        Self { flags: 0, item_id }
    }

    fn requires_v1(&self) -> bool {
        self.item_id > u16::MAX as u32
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = if self.requires_v1() { 4u64 } else { 2u64 };
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let v1 = self.requires_v1();
        let version = if v1 { 1u8 } else { 0u8 };

        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, version, self.flags)?;

        if v1 {
            writer.write_u32::<BigEndian>(self.item_id)?;
        } else {
            writer.write_u16::<BigEndian>(self.item_id as u16)?;
        }

        Ok(())
    }
}

impl Default for PrimaryItemBoxOwned {
    fn default() -> Self {
        Self::new(1)
    }
}

impl PrimaryItemBox for PrimaryItemBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        if self.requires_v1() { 1 } else { 0 }
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn item_id(&self) -> u32 {
        self.item_id
    }
}

impl<T: PrimaryItemBox> From<&T> for PrimaryItemBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            item_id: source.item_id(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pitm_v0(item_id: u16) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&14u32.to_be_bytes());
        data.extend_from_slice(b"pitm");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&item_id.to_be_bytes());
        data
    }

    #[test]
    fn parse_pitm_v0() {
        let data = make_pitm_v0(1);
        let view = PrimaryItemBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.item_id(), 1);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_pitm_v0(5);
        let view = PrimaryItemBoxView::new(&data).unwrap();
        let owned = PrimaryItemBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
