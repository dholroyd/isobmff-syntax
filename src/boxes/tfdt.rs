//! Track Fragment Decode Time Box (tfdt) parsing and serialization.
//!
//! The Track Fragment Decode Time Box provides the absolute decode time of the first sample.
//!
//! ```text
//! aligned(8) class TrackFragmentBaseMediaDecodeTimeBox
//!    extends FullBox('tfdt', version, 0) {
//!    if (version==1) {
//!       unsigned int(64) baseMediaDecodeTime;
//!    } else { // version==0
//!       unsigned int(32) baseMediaDecodeTime;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TrackFragmentBaseMediaDecodeTimeBox.
pub const BOX_TYPE: BoxCode = BoxCode::TFDT;

/// Common interface for accessing TrackFragmentBaseMediaDecodeTimeBox data.
pub trait TrackFragmentBaseMediaDecodeTimeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the base media decode time.
    fn base_media_decode_time(&self) -> u64;
}

/// A borrowing view over raw TrackFragmentBaseMediaDecodeTimeBox bytes.
#[derive(Clone, Copy)]
pub struct TrackFragmentBaseMediaDecodeTimeBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
}

impl<'a> TrackFragmentBaseMediaDecodeTimeBoxView<'a> {
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

impl TrackFragmentBaseMediaDecodeTimeBox for TrackFragmentBaseMediaDecodeTimeBoxView<'_> {
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

    fn base_media_decode_time(&self) -> u64 {
        let o = self.payload_offset();
        if self.version == 1 {
            BigEndian::read_u64(&self.data[o..o + 8])
        } else {
            BigEndian::read_u32(&self.data[o..o + 4]) as u64
        }
    }
}

impl std::fmt::Debug for TrackFragmentBaseMediaDecodeTimeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackFragmentBaseMediaDecodeTimeBoxView")
            .field("version", &self.version())
            .field("base_media_decode_time", &self.base_media_decode_time())
            .finish()
    }
}

/// An owned representation of TrackFragmentBaseMediaDecodeTimeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackFragmentBaseMediaDecodeTimeBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Base media decode time.
    pub base_media_decode_time: u64,
}

impl TrackFragmentBaseMediaDecodeTimeBoxOwned {
    /// Creates a new TrackFragmentBaseMediaDecodeTimeBoxOwned.
    pub fn new(base_media_decode_time: u64) -> Self {
        Self {
            flags: 0,
            base_media_decode_time,
        }
    }

    /// Returns whether version 1 is required.
    fn requires_v1(&self) -> bool {
        self.base_media_decode_time > u32::MAX as u64
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
            writer.write_u64::<BigEndian>(self.base_media_decode_time)?;
        } else {
            writer.write_u32::<BigEndian>(self.base_media_decode_time as u32)?;
        }

        Ok(())
    }
}

impl Default for TrackFragmentBaseMediaDecodeTimeBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl TrackFragmentBaseMediaDecodeTimeBox for TrackFragmentBaseMediaDecodeTimeBoxOwned {
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

    fn base_media_decode_time(&self) -> u64 {
        self.base_media_decode_time
    }
}

impl<T: TrackFragmentBaseMediaDecodeTimeBox> From<&T> for TrackFragmentBaseMediaDecodeTimeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            base_media_decode_time: source.base_media_decode_time(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tfdt_v0(time: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"tfdt");
        data.push(0);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&time.to_be_bytes());
        data
    }

    fn make_tfdt_v1(time: u64) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"tfdt");
        data.push(1);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&time.to_be_bytes());
        data
    }

    #[test]
    fn parse_tfdt_v0() {
        let data = make_tfdt_v0(1000);
        let view = TrackFragmentBaseMediaDecodeTimeBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.base_media_decode_time(), 1000);
    }

    #[test]
    fn parse_tfdt_v1() {
        let data = make_tfdt_v1(5_000_000_000);
        let view = TrackFragmentBaseMediaDecodeTimeBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 1);
        assert_eq!(view.base_media_decode_time(), 5_000_000_000);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_tfdt_v0(2000);
        let view = TrackFragmentBaseMediaDecodeTimeBoxView::new(&data).unwrap();
        let owned = TrackFragmentBaseMediaDecodeTimeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_v1() {
        let data = make_tfdt_v1(10_000_000_000);
        let view = TrackFragmentBaseMediaDecodeTimeBoxView::new(&data).unwrap();
        let owned = TrackFragmentBaseMediaDecodeTimeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
