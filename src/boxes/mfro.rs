//! Movie Fragment Random Access Offset Box (mfro) parsing and serialization.
//!
//! The Movie Fragment Random Access Offset Box provides the size of the
//! enclosing mfra box.
//!
//! ```text
//! aligned(8) class MovieFragmentRandomAccessOffsetBox
//!    extends FullBox('mfro', version, 0) {
//!    unsigned int(32) parent_size;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MovieFragmentRandomAccessOffsetBox.
pub const BOX_TYPE: BoxCode = BoxCode::MFRO;

/// Common interface for accessing MovieFragmentRandomAccessOffsetBox data.
pub trait MovieFragmentRandomAccessOffsetBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the size of the enclosing mfra box.
    fn mfra_size(&self) -> u32;
}

/// A borrowing view over raw MovieFragmentRandomAccessOffsetBox bytes.
#[derive(Clone, Copy)]
pub struct MovieFragmentRandomAccessOffsetBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> MovieFragmentRandomAccessOffsetBoxView<'a> {
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
}

impl MovieFragmentRandomAccessOffsetBox for MovieFragmentRandomAccessOffsetBoxView<'_> {
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

    fn mfra_size(&self) -> u32 {
        let o = self.payload_offset();
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for MovieFragmentRandomAccessOffsetBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MovieFragmentRandomAccessOffsetBoxView")
            .field("mfra_size", &self.mfra_size())
            .finish()
    }
}

/// An owned representation of MovieFragmentRandomAccessOffsetBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovieFragmentRandomAccessOffsetBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Size of the enclosing mfra box.
    pub mfra_size: u32,
}

impl MovieFragmentRandomAccessOffsetBoxOwned {
    /// Creates a new MovieFragmentRandomAccessOffsetBoxOwned.
    pub fn new(mfra_size: u32) -> Self {
        Self { flags: 0, mfra_size }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(4) + 4 // 8 + 4 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.mfra_size)?;
        Ok(())
    }
}

impl Default for MovieFragmentRandomAccessOffsetBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl MovieFragmentRandomAccessOffsetBox for MovieFragmentRandomAccessOffsetBoxOwned {
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

    fn mfra_size(&self) -> u32 {
        self.mfra_size
    }
}

impl<T: MovieFragmentRandomAccessOffsetBox> From<&T> for MovieFragmentRandomAccessOffsetBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            mfra_size: source.mfra_size(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mfro(mfra_size: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"mfro");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&mfra_size.to_be_bytes());
        data
    }

    #[test]
    fn parse_mfro() {
        let data = make_mfro(1000);
        let view = MovieFragmentRandomAccessOffsetBoxView::new(&data).unwrap();

        assert_eq!(view.mfra_size(), 1000);
    }

    #[test]
    fn roundtrip() {
        let data = make_mfro(2000);
        let view = MovieFragmentRandomAccessOffsetBoxView::new(&data).unwrap();
        let owned = MovieFragmentRandomAccessOffsetBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
