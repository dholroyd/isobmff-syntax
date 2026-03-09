//! Sampling Rate Box (srat) parsing and serialization.
//!
//! The Sampling Rate Box specifies the sampling rate of an audio track.
//!
//! ```text
//! aligned(8) class SamplingRateBox
//!    extends FullBox('srat', version = 0, 0) {
//!    unsigned int(32) sampling_rate;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SamplingRateBox.
pub const BOX_TYPE: BoxCode = BoxCode::SRAT;

/// Common interface for accessing SamplingRateBox data.
pub trait SamplingRateBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the sampling rate.
    fn sampling_rate(&self) -> u32;
}

/// A borrowing view over raw SamplingRateBox bytes.
#[derive(Clone, Copy)]
pub struct SamplingRateBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> SamplingRateBoxView<'a> {
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
}

impl<'a> SamplingRateBox for SamplingRateBoxView<'a> {
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

    fn sampling_rate(&self) -> u32 {
        let o = self.fullbox_offset + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for SamplingRateBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SamplingRateBoxView")
            .field("sampling_rate", &self.sampling_rate())
            .finish()
    }
}

/// An owned representation of SamplingRateBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SamplingRateBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Sampling rate.
    pub sampling_rate: u32,
}

impl SamplingRateBoxOwned {
    /// Creates a new SamplingRateBoxOwned.
    pub fn new(sampling_rate: u32) -> Self {
        Self {
            flags: 0,
            sampling_rate,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(4) + 4 // 8 + 4 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.sampling_rate)?;

        Ok(())
    }
}

impl Default for SamplingRateBoxOwned {
    fn default() -> Self {
        Self::new(48000)
    }
}

impl SamplingRateBox for SamplingRateBoxOwned {
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

    fn sampling_rate(&self) -> u32 {
        self.sampling_rate
    }
}

impl<T: SamplingRateBox> From<&T> for SamplingRateBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            sampling_rate: source.sampling_rate(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_srat() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"srat");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&48000u32.to_be_bytes()); // sampling_rate
        data
    }

    #[test]
    fn parse_srat() {
        let data = make_srat();
        let view = SamplingRateBoxView::new(&data).unwrap();
        assert_eq!(view.sampling_rate(), 48000);
    }

    #[test]
    fn roundtrip() {
        let data = make_srat();
        let view = SamplingRateBoxView::new(&data).unwrap();
        let owned = SamplingRateBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
