//! Movie Extends Header Box (mehd) parsing and serialization.
//!
//! The Movie Extends Header Box provides the overall duration of a fragmented movie.
//!
//! ```text
//! aligned(8) class MovieExtendsHeaderBox
//!    extends FullBox('mehd', version, 0) {
//!    if (version==1) {
//!       unsigned int(64) fragment_duration;
//!    } else { // version==0
//!       unsigned int(32) fragment_duration;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MovieExtendsHeaderBox.
pub const BOX_TYPE: BoxCode = BoxCode::MEHD;

/// Common interface for accessing MovieExtendsHeaderBox data.
pub trait MovieExtendsHeaderBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the fragment duration.
    fn fragment_duration(&self) -> u64;
}

/// A borrowing view over raw MovieExtendsHeaderBox bytes.
#[derive(Clone, Copy)]
pub struct MovieExtendsHeaderBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
}

impl<'a> MovieExtendsHeaderBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let payload_size = if header.version == 1 { 8 } else { 4 };
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

impl MovieExtendsHeaderBox for MovieExtendsHeaderBoxView<'_> {
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

    fn fragment_duration(&self) -> u64 {
        let o = self.payload_offset();
        if self.version == 1 {
            BigEndian::read_u64(&self.data[o..o + 8])
        } else {
            BigEndian::read_u32(&self.data[o..o + 4]) as u64
        }
    }
}

impl std::fmt::Debug for MovieExtendsHeaderBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MovieExtendsHeaderBoxView")
            .field("version", &self.version())
            .field("fragment_duration", &self.fragment_duration())
            .finish()
    }
}

/// An owned representation of MovieExtendsHeaderBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovieExtendsHeaderBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Fragment duration.
    pub fragment_duration: u64,
}

impl MovieExtendsHeaderBoxOwned {
    /// Creates a new MovieExtendsHeaderBoxOwned.
    pub fn new(fragment_duration: u64) -> Self {
        Self {
            flags: 0,
            fragment_duration,
        }
    }

    /// Returns whether version 1 is required.
    fn requires_v1(&self) -> bool {
        self.fragment_duration > u32::MAX as u64
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = if self.requires_v1() { 8u64 } else { 4u64 };
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let v1 = self.requires_v1();
        let version = if v1 { 1u8 } else { 0u8 };

        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, version, self.flags)?;

        if v1 {
            writer.write_u64::<BigEndian>(self.fragment_duration)?;
        } else {
            writer.write_u32::<BigEndian>(self.fragment_duration as u32)?;
        }

        Ok(())
    }
}

impl Default for MovieExtendsHeaderBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl MovieExtendsHeaderBox for MovieExtendsHeaderBoxOwned {
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

    fn fragment_duration(&self) -> u64 {
        self.fragment_duration
    }
}

impl<T: MovieExtendsHeaderBox> From<&T> for MovieExtendsHeaderBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            fragment_duration: source.fragment_duration(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mehd_v0(duration: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"mehd");
        data.push(0);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&duration.to_be_bytes());
        data
    }

    fn make_mehd_v1(duration: u64) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"mehd");
        data.push(1);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&duration.to_be_bytes());
        data
    }

    #[test]
    fn parse_mehd_v0() {
        let data = make_mehd_v0(10000);
        let view = MovieExtendsHeaderBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.fragment_duration(), 10000);
    }

    #[test]
    fn parse_mehd_v1() {
        let data = make_mehd_v1(5_000_000_000);
        let view = MovieExtendsHeaderBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 1);
        assert_eq!(view.fragment_duration(), 5_000_000_000);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_mehd_v0(20000);
        let view = MovieExtendsHeaderBoxView::new(&data).unwrap();
        let owned = MovieExtendsHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_v1() {
        let data = make_mehd_v1(10_000_000_000);
        let view = MovieExtendsHeaderBoxView::new(&data).unwrap();
        let owned = MovieExtendsHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
