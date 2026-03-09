//! Producer Reference Time Box (prft) parsing and serialization.
//!
//! The Producer Reference Time Box provides producer timing information.
//!
//! ```text
//! aligned(8) class ProducerReferenceTimeBox
//!    extends FullBox('prft', version, flags) {
//!    unsigned int(32) reference_track_ID;
//!    unsigned int(64) ntp_timestamp;
//!    if (version==0) {
//!       unsigned int(32) media_time;
//!    } else {
//!       unsigned int(64) media_time;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ProducerReferenceTimeBox.
pub const BOX_TYPE: BoxCode = BoxCode::PRFT;

/// Common interface for accessing ProducerReferenceTimeBox data.
pub trait ProducerReferenceTimeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the reference track ID.
    fn reference_track_id(&self) -> u32;

    /// Returns the NTP timestamp.
    fn ntp_timestamp(&self) -> u64;

    /// Returns the media time.
    fn media_time(&self) -> u64;
}

/// A borrowing view over raw ProducerReferenceTimeBox bytes.
#[derive(Clone, Copy)]
pub struct ProducerReferenceTimeBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
}

impl<'a> ProducerReferenceTimeBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let payload_size = if header.version == 1 { 20 } else { 16 };
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

impl ProducerReferenceTimeBox for ProducerReferenceTimeBoxView<'_> {
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

    fn reference_track_id(&self) -> u32 {
        let o = self.payload_offset();
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn ntp_timestamp(&self) -> u64 {
        let o = self.payload_offset() + 4;
        BigEndian::read_u64(&self.data[o..o + 8])
    }

    fn media_time(&self) -> u64 {
        let o = self.payload_offset() + 12;
        if self.version == 1 {
            BigEndian::read_u64(&self.data[o..o + 8])
        } else {
            BigEndian::read_u32(&self.data[o..o + 4]) as u64
        }
    }
}

impl std::fmt::Debug for ProducerReferenceTimeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProducerReferenceTimeBoxView")
            .field("version", &self.version())
            .field("reference_track_id", &self.reference_track_id())
            .field("ntp_timestamp", &self.ntp_timestamp())
            .field("media_time", &self.media_time())
            .finish()
    }
}

/// An owned representation of ProducerReferenceTimeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProducerReferenceTimeBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Reference track ID.
    pub reference_track_id: u32,
    /// NTP timestamp.
    pub ntp_timestamp: u64,
    /// Media time.
    pub media_time: u64,
}

impl ProducerReferenceTimeBoxOwned {
    /// Creates a new ProducerReferenceTimeBoxOwned.
    pub fn new(reference_track_id: u32) -> Self {
        Self {
            flags: 0,
            reference_track_id,
            ntp_timestamp: 0,
            media_time: 0,
        }
    }

    /// Returns whether version 1 is required.
    fn requires_v1(&self) -> bool {
        self.media_time > u32::MAX as u64
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = if self.requires_v1() { 20u64 } else { 16u64 };
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let v1 = self.requires_v1();
        let version = if v1 { 1u8 } else { 0u8 };

        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, version, self.flags)?;
        writer.write_u32::<BigEndian>(self.reference_track_id)?;
        writer.write_u64::<BigEndian>(self.ntp_timestamp)?;

        if v1 {
            writer.write_u64::<BigEndian>(self.media_time)?;
        } else {
            writer.write_u32::<BigEndian>(self.media_time as u32)?;
        }

        Ok(())
    }
}

impl Default for ProducerReferenceTimeBoxOwned {
    fn default() -> Self {
        Self::new(1)
    }
}

impl ProducerReferenceTimeBox for ProducerReferenceTimeBoxOwned {
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

    fn reference_track_id(&self) -> u32 {
        self.reference_track_id
    }

    fn ntp_timestamp(&self) -> u64 {
        self.ntp_timestamp
    }

    fn media_time(&self) -> u64 {
        self.media_time
    }
}

impl<T: ProducerReferenceTimeBox> From<&T> for ProducerReferenceTimeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            reference_track_id: source.reference_track_id(),
            ntp_timestamp: source.ntp_timestamp(),
            media_time: source.media_time(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_prft_v0() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&28u32.to_be_bytes()); // size
        data.extend_from_slice(b"prft");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1u32.to_be_bytes()); // reference_track_id
        data.extend_from_slice(&0x12345678_9ABCDEF0u64.to_be_bytes()); // ntp_timestamp
        data.extend_from_slice(&1000u32.to_be_bytes()); // media_time
        data
    }

    #[test]
    fn parse_prft_v0() {
        let data = make_prft_v0();
        let view = ProducerReferenceTimeBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.reference_track_id(), 1);
        assert_eq!(view.ntp_timestamp(), 0x12345678_9ABCDEF0);
        assert_eq!(view.media_time(), 1000);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_prft_v0();
        let view = ProducerReferenceTimeBoxView::new(&data).unwrap();
        let owned = ProducerReferenceTimeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
