//! Opus Specific Box (dOps) parsing and serialization.
//!
//! The Opus Specific Box contains Opus codec configuration.

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{ByteOrder, LittleEndian, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for OpusSpecificBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"dOps");

/// Common interface for accessing OpusSpecificBox data.
pub trait OpusSpecificBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version.
    fn version(&self) -> u8;

    /// Returns the output channel count.
    fn output_channel_count(&self) -> u8;

    /// Returns the pre-skip.
    fn pre_skip(&self) -> u16;

    /// Returns the input sample rate.
    fn input_sample_rate(&self) -> u32;

    /// Returns the output gain.
    fn output_gain(&self) -> i16;

    /// Returns the channel mapping family.
    fn channel_mapping_family(&self) -> u8;

    /// Returns the channel mapping data if present.
    fn channel_mapping(&self) -> Option<&[u8]>;
}

/// A borrowing view over raw OpusSpecificBox bytes.
#[derive(Clone, Copy)]
pub struct OpusSpecificBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> OpusSpecificBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 11)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the channel mapping data if present.
    pub fn channel_mapping(&self) -> Option<&'a [u8]> {
        if self.channel_mapping_family() == 0 {
            None
        } else {
            let start = self.header_size + 11;
            if start < self.data.len() {
                Some(&self.data[start..])
            } else {
                None
            }
        }
    }
}

impl<'a> OpusSpecificBox for OpusSpecificBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.data[self.header_size]
    }

    fn output_channel_count(&self) -> u8 {
        self.data[self.header_size + 1]
    }

    fn pre_skip(&self) -> u16 {
        let o = self.header_size + 2;
        LittleEndian::read_u16(&self.data[o..o + 2])
    }

    fn input_sample_rate(&self) -> u32 {
        let o = self.header_size + 4;
        LittleEndian::read_u32(&self.data[o..o + 4])
    }

    fn output_gain(&self) -> i16 {
        let o = self.header_size + 8;
        LittleEndian::read_i16(&self.data[o..o + 2])
    }

    fn channel_mapping_family(&self) -> u8 {
        self.data[self.header_size + 10]
    }

    fn channel_mapping(&self) -> Option<&[u8]> {
        self.channel_mapping()
    }
}

impl std::fmt::Debug for OpusSpecificBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpusSpecificBoxView")
            .field("version", &self.version())
            .field("output_channel_count", &self.output_channel_count())
            .field("input_sample_rate", &self.input_sample_rate())
            .finish()
    }
}

/// An owned representation of OpusSpecificBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpusSpecificBoxOwned {
    /// Version (should be 0).
    pub version: u8,
    /// Output channel count.
    pub output_channel_count: u8,
    /// Pre-skip.
    pub pre_skip: u16,
    /// Input sample rate.
    pub input_sample_rate: u32,
    /// Output gain.
    pub output_gain: i16,
    /// Channel mapping family.
    pub channel_mapping_family: u8,
    /// Channel mapping data.
    pub channel_mapping: Vec<u8>,
}

impl OpusSpecificBoxOwned {
    /// Creates a new OpusSpecificBoxOwned.
    pub fn new(channels: u8, sample_rate: u32) -> Self {
        Self {
            version: 0,
            output_channel_count: channels,
            pre_skip: 0,
            input_sample_rate: sample_rate,
            output_gain: 0,
            channel_mapping_family: 0,
            channel_mapping: Vec::new(),
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let mapping_size = if self.channel_mapping_family == 0 {
            0
        } else {
            self.channel_mapping.len()
        };
        let payload = (11 + mapping_size) as u64;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;

        writer.write_u8(self.version)?;
        writer.write_u8(self.output_channel_count)?;
        writer.write_u16::<LittleEndian>(self.pre_skip)?;
        writer.write_u32::<LittleEndian>(self.input_sample_rate)?;
        writer.write_i16::<LittleEndian>(self.output_gain)?;
        writer.write_u8(self.channel_mapping_family)?;

        if self.channel_mapping_family != 0 {
            writer.write_all(&self.channel_mapping)?;
        }

        Ok(())
    }
}

impl Default for OpusSpecificBoxOwned {
    fn default() -> Self {
        Self::new(2, 48000)
    }
}

impl OpusSpecificBox for OpusSpecificBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.version
    }

    fn output_channel_count(&self) -> u8 {
        self.output_channel_count
    }

    fn pre_skip(&self) -> u16 {
        self.pre_skip
    }

    fn input_sample_rate(&self) -> u32 {
        self.input_sample_rate
    }

    fn output_gain(&self) -> i16 {
        self.output_gain
    }

    fn channel_mapping_family(&self) -> u8 {
        self.channel_mapping_family
    }

    fn channel_mapping(&self) -> Option<&[u8]> {
        if self.channel_mapping_family == 0 {
            None
        } else {
            Some(&self.channel_mapping)
        }
    }
}

impl TryFrom<&OpusSpecificBoxView<'_>> for OpusSpecificBoxOwned {
    type Error = ParseError;

    fn try_from(source: &OpusSpecificBoxView<'_>) -> Result<Self, Self::Error> {
        let channel_mapping = if source.channel_mapping_family() != 0 {
            source.channel_mapping().ok_or(ParseError::BufferTooShort {
                expected: source.header_size + 12,
                found: source.data.len(),
            })?.to_vec()
        } else {
            Vec::new()
        };

        Ok(Self {
            version: source.version(),
            output_channel_count: source.output_channel_count(),
            pre_skip: source.pre_skip(),
            input_sample_rate: source.input_sample_rate(),
            output_gain: source.output_gain(),
            channel_mapping_family: source.channel_mapping_family(),
            channel_mapping,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dops() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&19u32.to_be_bytes()); // 8 + 11
        data.extend_from_slice(b"dOps");
        data.push(0); // version
        data.push(2); // output_channel_count
        data.extend_from_slice(&312u16.to_le_bytes()); // pre_skip
        data.extend_from_slice(&48000u32.to_le_bytes()); // input_sample_rate
        data.extend_from_slice(&0i16.to_le_bytes()); // output_gain
        data.push(0); // channel_mapping_family
        data
    }

    #[test]
    fn parse_dops() {
        let data = make_dops();
        let view = OpusSpecificBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.output_channel_count(), 2);
        assert_eq!(view.input_sample_rate(), 48000);
    }

    #[test]
    fn roundtrip() {
        let data = make_dops();
        let view = OpusSpecificBoxView::new(&data).unwrap();
        let owned = OpusSpecificBoxOwned::try_from(&view).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
