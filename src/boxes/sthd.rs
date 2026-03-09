//! Subtitle Media Header Box (sthd) parsing and serialization.
//!
//! The Subtitle Media Header Box is used for subtitle tracks.
//!
//! ```text
//! aligned(8) class SubtitleMediaHeaderBox
//!    extends FullBox('sthd', version = 0, flags = 0) {
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SubtitleMediaHeaderBox.
pub const BOX_TYPE: BoxCode = BoxCode::STHD;

/// Common interface for accessing SubtitleMediaHeaderBox data.
pub trait SubtitleMediaHeaderBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;
}

/// A borrowing view over raw SubtitleMediaHeaderBox bytes.
#[derive(Clone, Copy)]
pub struct SubtitleMediaHeaderBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> SubtitleMediaHeaderBoxView<'a> {
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

impl<'a> SubtitleMediaHeaderBox for SubtitleMediaHeaderBoxView<'a> {
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

impl std::fmt::Debug for SubtitleMediaHeaderBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubtitleMediaHeaderBoxView")
            .field("version", &self.version())
            .finish()
    }
}

/// An owned representation of SubtitleMediaHeaderBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct SubtitleMediaHeaderBoxOwned {
    /// Flags.
    pub flags: u32,
}

impl SubtitleMediaHeaderBoxOwned {
    /// Creates a new SubtitleMediaHeaderBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(0)
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;

        Ok(())
    }
}


impl SubtitleMediaHeaderBox for SubtitleMediaHeaderBoxOwned {
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

impl<T: SubtitleMediaHeaderBox> From<&T> for SubtitleMediaHeaderBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sthd() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"sthd");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data
    }

    #[test]
    fn parse_sthd() {
        let data = make_sthd();
        let view = SubtitleMediaHeaderBoxView::new(&data).unwrap();
        assert_eq!(view.version(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_sthd();
        let view = SubtitleMediaHeaderBoxView::new(&data).unwrap();
        let owned = SubtitleMediaHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
