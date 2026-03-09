//! Null Media Header Box (nmhd) parsing and serialization.
//!
//! The Null Media Header Box is used for tracks that do not require specific
//! media header information (e.g., timed metadata tracks).
//!
//! ```text
//! aligned(8) class NullMediaHeaderBox
//!    extends FullBox('nmhd', version = 0, flags) {
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for NullMediaHeaderBox.
pub const BOX_TYPE: BoxCode = BoxCode::NMHD;

/// Common interface for accessing NullMediaHeaderBox data.
pub trait NullMediaHeaderBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;
}

/// A borrowing view over raw NullMediaHeaderBox bytes.
#[derive(Clone, Copy)]
pub struct NullMediaHeaderBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> NullMediaHeaderBoxView<'a> {
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
}

impl NullMediaHeaderBox for NullMediaHeaderBoxView<'_> {
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
}

impl std::fmt::Debug for NullMediaHeaderBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NullMediaHeaderBoxView")
            .field("box_size", &self.box_size())
            .field("version", &self.version())
            .field("flags", &self.flags())
            .finish()
    }
}

/// An owned representation of NullMediaHeaderBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct NullMediaHeaderBoxOwned {
    /// Flags.
    pub flags: u32,
}

impl NullMediaHeaderBoxOwned {
    /// Creates a new NullMediaHeaderBoxOwned with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(0)
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, 0, self.flags)?;
        Ok(())
    }
}


impl NullMediaHeaderBox for NullMediaHeaderBoxOwned {
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
}

impl<T: NullMediaHeaderBox> From<&T> for NullMediaHeaderBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_nmhd() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes());
        data.extend_from_slice(b"nmhd");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data
    }

    #[test]
    fn parse_nmhd() {
        let data = make_nmhd();
        let view = NullMediaHeaderBoxView::new(&data).unwrap();

        assert_eq!(view.box_size(), 12);
        assert_eq!(view.flags(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_nmhd();
        let view = NullMediaHeaderBoxView::new(&data).unwrap();
        let owned = NullMediaHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
