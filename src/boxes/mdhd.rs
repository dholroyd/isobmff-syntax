//! Media Header Box (mdhd) parsing and serialization.
//!
//! The Media Header Box declares overall information that is media-independent.
//!
//! ```text
//! aligned(8) class MediaHeaderBox
//!    extends FullBox('mdhd', version, 0) {
//!    if (version==1) {
//!       unsigned int(64) creation_time;
//!       unsigned int(64) modification_time;
//!       unsigned int(32) timescale;
//!       unsigned int(64) duration;
//!    } else { // version==0
//!       unsigned int(32) creation_time;
//!       unsigned int(32) modification_time;
//!       unsigned int(32) timescale;
//!       unsigned int(32) duration;
//!    }
//!    bit(1) pad = 0;
//!    unsigned int(5)[3] language; // ISO-639-2/T language code
//!    unsigned int(16) pre_defined = 0;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use crate::types::IsoLanguageCode;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MediaHeaderBox.
pub const BOX_TYPE: BoxCode = BoxCode::MDHD;

/// Common interface for accessing MediaHeaderBox data.
pub trait MediaHeaderBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box (0 or 1).
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the creation time as seconds since 1904-01-01.
    fn creation_time(&self) -> u64;

    /// Returns the modification time as seconds since 1904-01-01.
    fn modification_time(&self) -> u64;

    /// Returns the timescale (time units per second).
    fn timescale(&self) -> u32;

    /// Returns the duration in timescale units.
    fn duration(&self) -> u64;

    /// Returns the language code.
    fn language(&self) -> IsoLanguageCode;
}

/// A borrowing view over raw MediaHeaderBox bytes.
#[derive(Clone, Copy)]
pub struct MediaHeaderBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> MediaHeaderBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let payload_size = if header.version == 0 { 20 } else { 32 };
        let fullbox_offset = header.validate(data, BOX_TYPE, Some(1), payload_size)?;
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

impl MediaHeaderBox for MediaHeaderBoxView<'_> {
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

    fn creation_time(&self) -> u64 {
        let o = self.payload_offset();
        if self.version() == 1 {
            BigEndian::read_u64(&self.data[o..o + 8])
        } else {
            BigEndian::read_u32(&self.data[o..o + 4]) as u64
        }
    }

    fn modification_time(&self) -> u64 {
        let o = self.payload_offset();
        if self.version() == 1 {
            BigEndian::read_u64(&self.data[o + 8..o + 16])
        } else {
            BigEndian::read_u32(&self.data[o + 4..o + 8]) as u64
        }
    }

    fn timescale(&self) -> u32 {
        let o = self.payload_offset();
        let offset = if self.version() == 1 { 16 } else { 8 };
        BigEndian::read_u32(&self.data[o + offset..o + offset + 4])
    }

    fn duration(&self) -> u64 {
        let o = self.payload_offset();
        if self.version() == 1 {
            BigEndian::read_u64(&self.data[o + 20..o + 28])
        } else {
            BigEndian::read_u32(&self.data[o + 12..o + 16]) as u64
        }
    }

    fn language(&self) -> IsoLanguageCode {
        let o = self.payload_offset();
        let offset = if self.version() == 1 { 28 } else { 16 };
        IsoLanguageCode::from_raw(BigEndian::read_u16(&self.data[o + offset..o + offset + 2]))
    }
}

impl std::fmt::Debug for MediaHeaderBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaHeaderBoxView")
            .field("box_size", &self.box_size())
            .field("version", &self.version())
            .field("timescale", &self.timescale())
            .field("duration", &self.duration())
            .field("language", &self.language())
            .finish()
    }
}

/// An owned representation of MediaHeaderBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaHeaderBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Creation time.
    pub creation_time: u64,
    /// Modification time.
    pub modification_time: u64,
    /// Timescale (time units per second).
    pub timescale: u32,
    /// Duration in timescale units.
    pub duration: u64,
    /// Language code.
    pub language: IsoLanguageCode,
}

impl MediaHeaderBoxOwned {
    /// Creates a new MediaHeaderBoxOwned with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the version required to represent this box's values.
    pub fn required_version(&self) -> u8 {
        if self.creation_time > u32::MAX as u64
            || self.modification_time > u32::MAX as u64
            || self.duration > u32::MAX as u64
        {
            1
        } else {
            0
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
let version = self.required_version();
        let payload = if version == 0 { 20u64 } else { 32u64 };
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let version = self.required_version();
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, version, self.flags)?;

        // Payload
        if version == 1 {
            writer.write_u64::<BigEndian>(self.creation_time)?;
            writer.write_u64::<BigEndian>(self.modification_time)?;
            writer.write_u32::<BigEndian>(self.timescale)?;
            writer.write_u64::<BigEndian>(self.duration)?;
        } else {
            writer.write_u32::<BigEndian>(self.creation_time as u32)?;
            writer.write_u32::<BigEndian>(self.modification_time as u32)?;
            writer.write_u32::<BigEndian>(self.timescale)?;
            writer.write_u32::<BigEndian>(self.duration as u32)?;
        }

        writer.write_u16::<BigEndian>(self.language.raw())?;
        writer.write_u16::<BigEndian>(0)?; // pre_defined

        Ok(())
    }
}

impl Default for MediaHeaderBoxOwned {
    fn default() -> Self {
        Self {
            flags: 0,
            creation_time: 0,
            modification_time: 0,
            timescale: 1000,
            duration: 0,
            language: IsoLanguageCode::UNDETERMINED,
        }
    }
}

impl MediaHeaderBox for MediaHeaderBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.required_version()
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn creation_time(&self) -> u64 {
        self.creation_time
    }

    fn modification_time(&self) -> u64 {
        self.modification_time
    }

    fn timescale(&self) -> u32 {
        self.timescale
    }

    fn duration(&self) -> u64 {
        self.duration
    }

    fn language(&self) -> IsoLanguageCode {
        self.language
    }
}

impl<T: MediaHeaderBox> From<&T> for MediaHeaderBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            creation_time: source.creation_time(),
            modification_time: source.modification_time(),
            timescale: source.timescale(),
            duration: source.duration(),
            language: source.language(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_v0_mdhd() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&32u32.to_be_bytes()); // size
        data.extend_from_slice(b"mdhd");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags

        data.extend_from_slice(&1000u32.to_be_bytes()); // creation_time
        data.extend_from_slice(&2000u32.to_be_bytes()); // modification_time
        data.extend_from_slice(&48000u32.to_be_bytes()); // timescale
        data.extend_from_slice(&96000u32.to_be_bytes()); // duration (2 seconds)
        data.extend_from_slice(&IsoLanguageCode::from_chars(['e', 'n', 'g']).raw().to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes()); // pre_defined

        data
    }

    #[test]
    fn parse_v0() {
        let data = make_v0_mdhd();
        let view = MediaHeaderBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.timescale(), 48000);
        assert_eq!(view.duration(), 96000);
        assert_eq!(view.language().to_chars(), ['e', 'n', 'g']);
    }

    #[test]
    fn roundtrip() {
        let data = make_v0_mdhd();
        let view = MediaHeaderBoxView::new(&data).unwrap();
        let owned = MediaHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
