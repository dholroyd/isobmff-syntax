//! Track Fragment Header Box (tfhd) parsing and serialization.
//!
//! The Track Fragment Header Box sets up information for this track fragment.
//!
//! ```text
//! aligned(8) class TrackFragmentHeaderBox
//!    extends FullBox('tfhd', 0, tf_flags) {
//!    unsigned int(32) track_ID;
//!    // all the following are optional fields
//!    unsigned int(64) base_data_offset;
//!    unsigned int(32) sample_description_index;
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

/// The box type identifier for TrackFragmentHeaderBox.
pub const BOX_TYPE: BoxCode = BoxCode::TFHD;

/// Track fragment header flags.
pub mod flags {
    /// Base data offset is present.
    pub const BASE_DATA_OFFSET_PRESENT: u32 = 0x000001;
    /// Sample description index is present.
    pub const SAMPLE_DESCRIPTION_INDEX_PRESENT: u32 = 0x000002;
    /// Default sample duration is present.
    pub const DEFAULT_SAMPLE_DURATION_PRESENT: u32 = 0x000008;
    /// Default sample size is present.
    pub const DEFAULT_SAMPLE_SIZE_PRESENT: u32 = 0x000010;
    /// Default sample flags is present.
    pub const DEFAULT_SAMPLE_FLAGS_PRESENT: u32 = 0x000020;
    /// Duration is empty.
    pub const DURATION_IS_EMPTY: u32 = 0x010000;
    /// Default base is moof.
    pub const DEFAULT_BASE_IS_MOOF: u32 = 0x020000;
}

/// Common interface for accessing TrackFragmentHeaderBox data.
pub trait TrackFragmentHeaderBox {
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

    /// Returns the base data offset, if present.
    fn base_data_offset(&self) -> Option<u64>;

    /// Returns the sample description index, if present.
    fn sample_description_index(&self) -> Option<u32>;

    /// Returns the default sample duration, if present.
    fn default_sample_duration(&self) -> Option<u32>;

    /// Returns the default sample size, if present.
    fn default_sample_size(&self) -> Option<u32>;

    /// Returns the default sample flags, if present.
    fn default_sample_flags(&self) -> Option<u32>;
}

/// A borrowing view over raw TrackFragmentHeaderBox bytes.
#[derive(Clone, Copy)]
pub struct TrackFragmentHeaderBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> TrackFragmentHeaderBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 4)?;

        // Compute required size from flags
        let fl = BigEndian::read_u24(&data[fullbox_offset + 1..fullbox_offset + 4]);
        let mut min_size = fullbox_offset + 8;
        if fl & flags::BASE_DATA_OFFSET_PRESENT != 0 {
            min_size += 8;
        }
        if fl & flags::SAMPLE_DESCRIPTION_INDEX_PRESENT != 0 {
            min_size += 4;
        }
        if fl & flags::DEFAULT_SAMPLE_DURATION_PRESENT != 0 {
            min_size += 4;
        }
        if fl & flags::DEFAULT_SAMPLE_SIZE_PRESENT != 0 {
            min_size += 4;
        }
        if fl & flags::DEFAULT_SAMPLE_FLAGS_PRESENT != 0 {
            min_size += 4;
        }
        if data.len() < min_size {
            return Err(ParseError::BufferTooShort {
                expected: min_size,
                found: data.len(),
            });
        }

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

    /// Returns the offset of optional fields based on flags.
    fn optional_field_offset(&self, target_flag: u32) -> Option<usize> {
        let fl = self.flags();
        let mut offset = self.payload_offset() + 4; // After track_id

        let ordered_flags = [
            (flags::BASE_DATA_OFFSET_PRESENT, 8),
            (flags::SAMPLE_DESCRIPTION_INDEX_PRESENT, 4),
            (flags::DEFAULT_SAMPLE_DURATION_PRESENT, 4),
            (flags::DEFAULT_SAMPLE_SIZE_PRESENT, 4),
            (flags::DEFAULT_SAMPLE_FLAGS_PRESENT, 4),
        ];

        for (flag, size) in ordered_flags {
            if flag == target_flag {
                if fl & flag != 0 {
                    return Some(offset);
                } else {
                    return None;
                }
            }
            if fl & flag != 0 {
                offset += size;
            }
        }
        None
    }
}

impl TrackFragmentHeaderBox for TrackFragmentHeaderBoxView<'_> {
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

    fn base_data_offset(&self) -> Option<u64> {
        self.optional_field_offset(flags::BASE_DATA_OFFSET_PRESENT)
            .map(|o| BigEndian::read_u64(&self.data[o..o + 8]))
    }

    fn sample_description_index(&self) -> Option<u32> {
        self.optional_field_offset(flags::SAMPLE_DESCRIPTION_INDEX_PRESENT)
            .map(|o| BigEndian::read_u32(&self.data[o..o + 4]))
    }

    fn default_sample_duration(&self) -> Option<u32> {
        self.optional_field_offset(flags::DEFAULT_SAMPLE_DURATION_PRESENT)
            .map(|o| BigEndian::read_u32(&self.data[o..o + 4]))
    }

    fn default_sample_size(&self) -> Option<u32> {
        self.optional_field_offset(flags::DEFAULT_SAMPLE_SIZE_PRESENT)
            .map(|o| BigEndian::read_u32(&self.data[o..o + 4]))
    }

    fn default_sample_flags(&self) -> Option<u32> {
        self.optional_field_offset(flags::DEFAULT_SAMPLE_FLAGS_PRESENT)
            .map(|o| BigEndian::read_u32(&self.data[o..o + 4]))
    }
}

impl std::fmt::Debug for TrackFragmentHeaderBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackFragmentHeaderBoxView")
            .field("track_id", &self.track_id())
            .field("flags", &format!("0x{:06X}", self.flags()))
            .field("base_data_offset", &self.base_data_offset())
            .field("default_sample_duration", &self.default_sample_duration())
            .finish()
    }
}

/// An owned representation of TrackFragmentHeaderBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackFragmentHeaderBoxOwned {
    /// Track ID.
    pub track_id: u32,
    /// Base data offset.
    pub base_data_offset: Option<u64>,
    /// Sample description index.
    pub sample_description_index: Option<u32>,
    /// Default sample duration.
    pub default_sample_duration: Option<u32>,
    /// Default sample size.
    pub default_sample_size: Option<u32>,
    /// Default sample flags.
    pub default_sample_flags: Option<u32>,
    /// Duration is empty flag.
    pub duration_is_empty: bool,
    /// Default base is moof flag.
    pub default_base_is_moof: bool,
}

impl TrackFragmentHeaderBoxOwned {
    /// Creates a new TrackFragmentHeaderBoxOwned with minimal fields.
    pub fn new(track_id: u32) -> Self {
        Self {
            track_id,
            ..Default::default()
        }
    }

    /// Computes the flags based on which optional fields are present.
    fn compute_flags(&self) -> u32 {
        let mut fl = 0u32;
        if self.base_data_offset.is_some() {
            fl |= flags::BASE_DATA_OFFSET_PRESENT;
        }
        if self.sample_description_index.is_some() {
            fl |= flags::SAMPLE_DESCRIPTION_INDEX_PRESENT;
        }
        if self.default_sample_duration.is_some() {
            fl |= flags::DEFAULT_SAMPLE_DURATION_PRESENT;
        }
        if self.default_sample_size.is_some() {
            fl |= flags::DEFAULT_SAMPLE_SIZE_PRESENT;
        }
        if self.default_sample_flags.is_some() {
            fl |= flags::DEFAULT_SAMPLE_FLAGS_PRESENT;
        }
        if self.duration_is_empty {
            fl |= flags::DURATION_IS_EMPTY;
        }
        if self.default_base_is_moof {
            fl |= flags::DEFAULT_BASE_IS_MOOF;
        }
        fl
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let mut payload = 4u64; // track_id
        if self.base_data_offset.is_some() {
            payload += 8;
        }
        if self.sample_description_index.is_some() {
            payload += 4;
        }
        if self.default_sample_duration.is_some() {
            payload += 4;
        }
        if self.default_sample_size.is_some() {
            payload += 4;
        }
        if self.default_sample_flags.is_some() {
            payload += 4;
        }
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        let fl = self.compute_flags();
        write_fullbox_header(writer, size, BOX_TYPE, 0, fl)?;
        writer.write_u32::<BigEndian>(self.track_id)?;

        if let Some(v) = self.base_data_offset {
            writer.write_u64::<BigEndian>(v)?;
        }
        if let Some(v) = self.sample_description_index {
            writer.write_u32::<BigEndian>(v)?;
        }
        if let Some(v) = self.default_sample_duration {
            writer.write_u32::<BigEndian>(v)?;
        }
        if let Some(v) = self.default_sample_size {
            writer.write_u32::<BigEndian>(v)?;
        }
        if let Some(v) = self.default_sample_flags {
            writer.write_u32::<BigEndian>(v)?;
        }

        Ok(())
    }
}

impl Default for TrackFragmentHeaderBoxOwned {
    fn default() -> Self {
        Self {
            track_id: 1,
            base_data_offset: None,
            sample_description_index: None,
            default_sample_duration: None,
            default_sample_size: None,
            default_sample_flags: None,
            duration_is_empty: false,
            default_base_is_moof: true,
        }
    }
}

impl TrackFragmentHeaderBox for TrackFragmentHeaderBoxOwned {
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
        self.compute_flags()
    }

    fn track_id(&self) -> u32 {
        self.track_id
    }

    fn base_data_offset(&self) -> Option<u64> {
        self.base_data_offset
    }

    fn sample_description_index(&self) -> Option<u32> {
        self.sample_description_index
    }

    fn default_sample_duration(&self) -> Option<u32> {
        self.default_sample_duration
    }

    fn default_sample_size(&self) -> Option<u32> {
        self.default_sample_size
    }

    fn default_sample_flags(&self) -> Option<u32> {
        self.default_sample_flags
    }
}

impl<T: TrackFragmentHeaderBox> From<&T> for TrackFragmentHeaderBoxOwned {
    fn from(source: &T) -> Self {
        let fl = source.flags();
        Self {
            track_id: source.track_id(),
            base_data_offset: source.base_data_offset(),
            sample_description_index: source.sample_description_index(),
            default_sample_duration: source.default_sample_duration(),
            default_sample_size: source.default_sample_size(),
            default_sample_flags: source.default_sample_flags(),
            duration_is_empty: fl & flags::DURATION_IS_EMPTY != 0,
            default_base_is_moof: fl & flags::DEFAULT_BASE_IS_MOOF != 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tfhd_minimal(track_id: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes()); // size
        data.extend_from_slice(b"tfhd");
        data.push(0);
        data.extend_from_slice(&[0x02, 0x00, 0x00]); // flags = default_base_is_moof
        data.extend_from_slice(&track_id.to_be_bytes());
        data
    }

    fn make_tfhd_with_duration(track_id: u32, duration: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_be_bytes()); // size
        data.extend_from_slice(b"tfhd");
        data.push(0);
        // flags = default_sample_duration_present | default_base_is_moof
        data.extend_from_slice(&[0x02, 0x00, 0x08]);
        data.extend_from_slice(&track_id.to_be_bytes());
        data.extend_from_slice(&duration.to_be_bytes());
        data
    }

    #[test]
    fn parse_tfhd_minimal() {
        let data = make_tfhd_minimal(1);
        let view = TrackFragmentHeaderBoxView::new(&data).unwrap();

        assert_eq!(view.track_id(), 1);
        assert!(view.base_data_offset().is_none());
        assert!(view.default_sample_duration().is_none());
    }

    #[test]
    fn parse_tfhd_with_duration() {
        let data = make_tfhd_with_duration(2, 1024);
        let view = TrackFragmentHeaderBoxView::new(&data).unwrap();

        assert_eq!(view.track_id(), 2);
        assert_eq!(view.default_sample_duration(), Some(1024));
    }

    #[test]
    fn roundtrip() {
        let data = make_tfhd_with_duration(1, 512);
        let view = TrackFragmentHeaderBoxView::new(&data).unwrap();
        let owned = TrackFragmentHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
