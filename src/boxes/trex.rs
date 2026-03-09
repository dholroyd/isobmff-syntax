//! Track Extends Box (trex) parsing and serialization.
//!
//! The Track Extends Box sets up default values used in track fragments.
//!
//! ```text
//! aligned(8) class TrackExtendsBox
//!    extends FullBox('trex', 0, 0) {
//!    unsigned int(32) track_ID;
//!    unsigned int(32) default_sample_description_index;
//!    unsigned int(32) default_sample_duration;
//!    unsigned int(32) default_sample_size;
//!    unsigned int(32) default_sample_flags;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TrackExtendsBox.
pub const BOX_TYPE: BoxCode = BoxCode::TREX;

/// Common interface for accessing TrackExtendsBox data.
pub trait TrackExtendsBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the track ID.
    fn track_id(&self) -> u32;

    /// Returns the default sample description index.
    fn default_sample_description_index(&self) -> u32;

    /// Returns the default sample duration.
    fn default_sample_duration(&self) -> u32;

    /// Returns the default sample size.
    fn default_sample_size(&self) -> u32;

    /// Returns the default sample flags.
    fn default_sample_flags(&self) -> u32;
}

/// A borrowing view over raw TrackExtendsBox bytes.
#[derive(Clone, Copy)]
pub struct TrackExtendsBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> TrackExtendsBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 20)?;
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

impl TrackExtendsBox for TrackExtendsBoxView<'_> {
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

    fn track_id(&self) -> u32 {
        let o = self.payload_offset();
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn default_sample_description_index(&self) -> u32 {
        let o = self.payload_offset() + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn default_sample_duration(&self) -> u32 {
        let o = self.payload_offset() + 8;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn default_sample_size(&self) -> u32 {
        let o = self.payload_offset() + 12;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn default_sample_flags(&self) -> u32 {
        let o = self.payload_offset() + 16;
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for TrackExtendsBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackExtendsBoxView")
            .field("track_id", &self.track_id())
            .field("default_sample_duration", &self.default_sample_duration())
            .field("default_sample_size", &self.default_sample_size())
            .finish()
    }
}

/// An owned representation of TrackExtendsBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackExtendsBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Track ID.
    pub track_id: u32,
    /// Default sample description index.
    pub default_sample_description_index: u32,
    /// Default sample duration.
    pub default_sample_duration: u32,
    /// Default sample size.
    pub default_sample_size: u32,
    /// Default sample flags.
    pub default_sample_flags: u32,
}

impl TrackExtendsBoxOwned {
    /// Creates a new TrackExtendsBoxOwned with default values.
    pub fn new(track_id: u32) -> Self {
        Self {
            track_id,
            ..Default::default()
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(20) + 20 // 8 + 4 + 20
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.track_id)?;
        writer.write_u32::<BigEndian>(self.default_sample_description_index)?;
        writer.write_u32::<BigEndian>(self.default_sample_duration)?;
        writer.write_u32::<BigEndian>(self.default_sample_size)?;
        writer.write_u32::<BigEndian>(self.default_sample_flags)?;
        Ok(())
    }
}

impl Default for TrackExtendsBoxOwned {
    fn default() -> Self {
        Self {
            flags: 0,
            track_id: 1,
            default_sample_description_index: 1,
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
        }
    }
}

impl TrackExtendsBox for TrackExtendsBoxOwned {
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

    fn track_id(&self) -> u32 {
        self.track_id
    }

    fn default_sample_description_index(&self) -> u32 {
        self.default_sample_description_index
    }

    fn default_sample_duration(&self) -> u32 {
        self.default_sample_duration
    }

    fn default_sample_size(&self) -> u32 {
        self.default_sample_size
    }

    fn default_sample_flags(&self) -> u32 {
        self.default_sample_flags
    }
}

impl<T: TrackExtendsBox> From<&T> for TrackExtendsBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            track_id: source.track_id(),
            default_sample_description_index: source.default_sample_description_index(),
            default_sample_duration: source.default_sample_duration(),
            default_sample_size: source.default_sample_size(),
            default_sample_flags: source.default_sample_flags(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trex() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&32u32.to_be_bytes());
        data.extend_from_slice(b"trex");
        data.push(0);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&1u32.to_be_bytes()); // track_id
        data.extend_from_slice(&1u32.to_be_bytes()); // default_sample_description_index
        data.extend_from_slice(&1024u32.to_be_bytes()); // default_sample_duration
        data.extend_from_slice(&0u32.to_be_bytes()); // default_sample_size
        data.extend_from_slice(&0u32.to_be_bytes()); // default_sample_flags
        data
    }

    #[test]
    fn parse_trex() {
        let data = make_trex();
        let view = TrackExtendsBoxView::new(&data).unwrap();

        assert_eq!(view.track_id(), 1);
        assert_eq!(view.default_sample_duration(), 1024);
    }

    #[test]
    fn roundtrip() {
        let data = make_trex();
        let view = TrackExtendsBoxView::new(&data).unwrap();
        let owned = TrackExtendsBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
