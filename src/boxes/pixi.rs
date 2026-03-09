//! Pixel Information Box (pixi) parsing and serialization.
//!
//! The Pixel Information Box specifies the number and bit depth of color components.
//!
//! ```text
//! aligned(8) class PixelInformationProperty
//!    extends ItemFullProperty('pixi', version = 0, 0) {
//!    unsigned int(8) num_channels;
//!    for (i=0; i<num_channels; i++) {
//!       unsigned int(8) bits_per_channel;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for PixelInformationBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"pixi");

/// Common interface for accessing PixelInformationBox data.
pub trait PixelInformationBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the number of channels.
    fn num_channels(&self) -> u8;

    /// Returns all bits per channel values.
    fn bits_per_channels(&self) -> Vec<u8>;
}

/// A borrowing view over raw PixelInformationBox bytes.
#[derive(Clone, Copy)]
pub struct PixelInformationBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    num_channels: u8,
}

impl<'a> PixelInformationBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 1)?;
        let num_channels = data[fullbox_offset + 4];
        Ok(Self { data, fullbox_offset, num_channels })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the bits per channel for the given channel index.
    pub fn bits_per_channel(&self, channel: usize) -> Option<u8> {
        if channel >= self.num_channels as usize {
            return None;
        }

        let offset = self.fullbox_offset + 5 + channel;
        if offset < self.data.len() {
            Some(self.data[offset])
        } else {
            None
        }
    }

    /// Returns all bits per channel values.
    pub fn bits_per_channels(&self) -> Vec<u8> {
        (0..self.num_channels as usize)
            .filter_map(|i| self.bits_per_channel(i))
            .collect()
    }
}

impl<'a> PixelInformationBox for PixelInformationBoxView<'a> {
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

    fn num_channels(&self) -> u8 {
        self.num_channels
    }

    fn bits_per_channels(&self) -> Vec<u8> {
        self.bits_per_channels()
    }
}

impl std::fmt::Debug for PixelInformationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PixelInformationBoxView")
            .field("num_channels", &self.num_channels())
            .field("bits_per_channels", &self.bits_per_channels())
            .finish()
    }
}

/// An owned representation of PixelInformationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PixelInformationBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Bits per channel for each channel.
    pub bits_per_channels: Vec<u8>,
}

impl PixelInformationBoxOwned {
    /// Creates a new PixelInformationBoxOwned.
    pub fn new(bits_per_channels: Vec<u8>) -> Self {
        Self {
            flags: 0,
            bits_per_channels,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (1 + self.bits_per_channels.len()) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u8(self.bits_per_channels.len() as u8)?;
        writer.write_all(&self.bits_per_channels)?;

        Ok(())
    }
}

impl Default for PixelInformationBoxOwned {
    fn default() -> Self {
        Self::new(vec![8, 8, 8]) // RGB 8-bit
    }
}

impl PixelInformationBox for PixelInformationBoxOwned {
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

    fn num_channels(&self) -> u8 {
        self.bits_per_channels.len() as u8
    }

    fn bits_per_channels(&self) -> Vec<u8> {
        self.bits_per_channels.clone()
    }
}

impl<T: PixelInformationBox> From<&T> for PixelInformationBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            bits_per_channels: source.bits_per_channels(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pixi() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 1 + 3 = 16 bytes
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"pixi");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.push(3); // num_channels
        data.push(8); // bits_per_channel[0]
        data.push(8); // bits_per_channel[1]
        data.push(8); // bits_per_channel[2]
        data
    }

    #[test]
    fn parse_pixi() {
        let data = make_pixi();
        let view = PixelInformationBoxView::new(&data).unwrap();

        assert_eq!(view.num_channels(), 3);
        assert_eq!(view.bits_per_channels(), vec![8, 8, 8]);
    }

    #[test]
    fn roundtrip() {
        let data = make_pixi();
        let view = PixelInformationBoxView::new(&data).unwrap();
        let owned = PixelInformationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
