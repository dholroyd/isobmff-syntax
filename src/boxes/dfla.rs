//! FLAC Specific Box (dfLa) parsing and serialization.
//!
//! The FLAC Specific Box contains FLAC codec configuration.

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for FLACSpecificBox.
pub const BOX_TYPE: BoxCode = BoxCode::DFLA;

/// Common interface for accessing FLACSpecificBox data.
pub trait FLACSpecificBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the FLAC metadata blocks.
    fn metadata_blocks(&self) -> &[u8];
}

/// A borrowing view over raw FLACSpecificBox bytes.
#[derive(Clone, Copy)]
pub struct FLACSpecificBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> FLACSpecificBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 0)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the FLAC metadata blocks.
    pub fn metadata_blocks(&self) -> &'a [u8] {
        let start = self.fullbox_offset + 4;
        if start < self.data.len() {
            &self.data[start..]
        } else {
            &[]
        }
    }
}

impl<'a> FLACSpecificBox for FLACSpecificBoxView<'a> {
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

    fn metadata_blocks(&self) -> &[u8] {
        self.metadata_blocks()
    }
}

impl std::fmt::Debug for FLACSpecificBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FLACSpecificBoxView")
            .field("metadata_blocks_len", &self.metadata_blocks().len())
            .finish()
    }
}

/// An owned representation of FLACSpecificBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FLACSpecificBoxOwned {
    /// Flags.
    pub flags: u32,
    /// FLAC metadata blocks.
    pub metadata_blocks: Vec<u8>,
}

impl FLACSpecificBoxOwned {
    /// Creates a new FLACSpecificBoxOwned.
    pub fn new(metadata_blocks: Vec<u8>) -> Self {
        Self {
            flags: 0,
            metadata_blocks,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.metadata_blocks.len()) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_all(&self.metadata_blocks)?;

        Ok(())
    }
}

impl Default for FLACSpecificBoxOwned {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl FLACSpecificBox for FLACSpecificBoxOwned {
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

    fn metadata_blocks(&self) -> &[u8] {
        &self.metadata_blocks
    }
}

impl<T: FLACSpecificBox> From<&T> for FLACSpecificBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            metadata_blocks: source.metadata_blocks().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dfla() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"dfLa");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data
    }

    #[test]
    fn parse_dfla() {
        let data = make_dfla();
        let view = FLACSpecificBoxView::new(&data).unwrap();
        assert_eq!(view.version(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_dfla();
        let view = FLACSpecificBoxView::new(&data).unwrap();
        let owned = FLACSpecificBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
