//! Track Fragment Run Box (trun) parsing and serialization.
//!
//! The Track Fragment Run Box contains the sample information for a run of samples.
//!
//! ```text
//! aligned(8) class TrackRunBox
//!    extends FullBox('trun', version, tr_flags) {
//!    unsigned int(32) sample_count;
//!    // the following are optional fields
//!    signed int(32) data_offset;
//!    unsigned int(32) first_sample_flags;
//!    // all fields in the following array are optional
//!    {
//!       unsigned int(32) sample_duration;
//!       unsigned int(32) sample_size;
//!       unsigned int(32) sample_flags
//!       if (version == 0)
//!          { unsigned int(32) sample_composition_time_offset; }
//!       else
//!          { signed int(32) sample_composition_time_offset; }
//!    }[ sample_count ]
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TrackRunBox.
pub const BOX_TYPE: BoxCode = BoxCode::TRUN;

/// Track run flags.
pub mod flags {
    /// Data offset is present.
    pub const DATA_OFFSET_PRESENT: u32 = 0x000001;
    /// First sample flags is present.
    pub const FIRST_SAMPLE_FLAGS_PRESENT: u32 = 0x000004;
    /// Sample duration is present.
    pub const SAMPLE_DURATION_PRESENT: u32 = 0x000100;
    /// Sample size is present.
    pub const SAMPLE_SIZE_PRESENT: u32 = 0x000200;
    /// Sample flags is present.
    pub const SAMPLE_FLAGS_PRESENT: u32 = 0x000400;
    /// Sample composition time offset is present.
    pub const SAMPLE_COMPOSITION_TIME_OFFSETS_PRESENT: u32 = 0x000800;
}

/// A single sample entry in a track run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct TrackRunSample {
    /// Sample duration, if present.
    pub sample_duration: Option<u32>,
    /// Sample size, if present.
    pub sample_size: Option<u32>,
    /// Sample flags, if present.
    pub sample_flags: Option<u32>,
    /// Sample composition time offset, if present.
    pub sample_composition_time_offset: Option<i64>,
}


/// Common interface for accessing TrackRunBox data.
pub trait TrackRunBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the sample count.
    fn sample_count(&self) -> u32;

    /// Returns the data offset, if present.
    fn data_offset(&self) -> Option<i32>;

    /// Returns the first sample flags, if present.
    fn first_sample_flags(&self) -> Option<u32>;

    /// Returns an iterator over all samples.
    fn samples(&self) -> impl Iterator<Item = TrackRunSample> + '_;
}

/// A borrowing view over raw TrackRunBox bytes.
#[derive(Clone, Copy)]
pub struct TrackRunBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
    fl: u32,
    sample_count: u32,
    samples_offset: usize,
}

impl<'a> TrackRunBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let version = header.version;
        let fl = header.flags;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 4)?;

        let sample_count = BigEndian::read_u32(&data[fullbox_offset + 4..fullbox_offset + 8]);

        // Optional header fields
        let mut offset = fullbox_offset + 8;
        if fl & flags::DATA_OFFSET_PRESENT != 0 {
            offset += 4;
        }
        if fl & flags::FIRST_SAMPLE_FLAGS_PRESENT != 0 {
            offset += 4;
        }

        let samples_offset = offset;

        if data.len() < samples_offset {
            return Err(ParseError::BufferTooShort {
                expected: samples_offset,
                found: data.len(),
            });
        }

        // Validate per-sample data fits in the remaining box bytes.
        let sample_entry_size = Self::sample_entry_size(fl, version);
        let available = data.len() - samples_offset;
        let total_sample_data = (sample_count as usize)
            .checked_mul(sample_entry_size)
            .ok_or(ParseError::InvalidEntryCount {
                count: sample_count,
                max_possible: if sample_entry_size > 0 {
                    (available / sample_entry_size) as u32
                } else {
                    0
                },
            })?;
        if total_sample_data > available {
            let max_possible = if sample_entry_size > 0 {
                available / sample_entry_size
            } else {
                0
            };
            return Err(ParseError::InvalidEntryCount {
                count: sample_count,
                max_possible: max_possible as u32,
            });
        }

        Ok(Self {
            data,
            fullbox_offset,
            version,
            fl,
            sample_count,
            samples_offset,
        })
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

    fn sample_entry_size(fl: u32, _version: u8) -> usize {
        let mut size = 0;
        if fl & flags::SAMPLE_DURATION_PRESENT != 0 {
            size += 4;
        }
        if fl & flags::SAMPLE_SIZE_PRESENT != 0 {
            size += 4;
        }
        if fl & flags::SAMPLE_FLAGS_PRESENT != 0 {
            size += 4;
        }
        if fl & flags::SAMPLE_COMPOSITION_TIME_OFFSETS_PRESENT != 0 {
            size += 4; // Always 4 bytes (signed for v1, unsigned for v0)
        }
        size
    }

    /// Returns the sample at the given index.
    pub fn sample(&self, index: usize) -> Option<TrackRunSample> {
        if index >= self.sample_count as usize {
            return None;
        }

        let entry_size = Self::sample_entry_size(self.fl, self.version);
        let mut offset = self.samples_offset + index * entry_size;

        let sample_duration = if self.fl & flags::SAMPLE_DURATION_PRESENT != 0 {
            let v = BigEndian::read_u32(&self.data[offset..offset + 4]);
            offset += 4;
            Some(v)
        } else {
            None
        };

        let sample_size = if self.fl & flags::SAMPLE_SIZE_PRESENT != 0 {
            let v = BigEndian::read_u32(&self.data[offset..offset + 4]);
            offset += 4;
            Some(v)
        } else {
            None
        };

        let sample_flags = if self.fl & flags::SAMPLE_FLAGS_PRESENT != 0 {
            let v = BigEndian::read_u32(&self.data[offset..offset + 4]);
            offset += 4;
            Some(v)
        } else {
            None
        };

        let sample_composition_time_offset =
            if self.fl & flags::SAMPLE_COMPOSITION_TIME_OFFSETS_PRESENT != 0 {
                if self.version == 1 {
                    Some(BigEndian::read_i32(&self.data[offset..offset + 4]) as i64)
                } else {
                    Some(BigEndian::read_u32(&self.data[offset..offset + 4]) as i64)
                }
            } else {
                None
            };

        Some(TrackRunSample {
            sample_duration,
            sample_size,
            sample_flags,
            sample_composition_time_offset,
        })
    }

    /// Returns the offset to optional header fields.
    fn optional_field_offset(&self, target_flag: u32) -> Option<usize> {
        let mut offset = self.payload_offset() + 4; // After sample_count

        let ordered_flags = [
            flags::DATA_OFFSET_PRESENT,
            flags::FIRST_SAMPLE_FLAGS_PRESENT,
        ];

        for flag in ordered_flags {
            if flag == target_flag {
                if self.fl & flag != 0 {
                    return Some(offset);
                } else {
                    return None;
                }
            }
            if self.fl & flag != 0 {
                offset += 4;
            }
        }
        None
    }
}

impl TrackRunBox for TrackRunBoxView<'_> {
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
        self.fl
    }

    fn sample_count(&self) -> u32 {
        self.sample_count
    }

    fn data_offset(&self) -> Option<i32> {
        self.optional_field_offset(flags::DATA_OFFSET_PRESENT)
            .map(|o| BigEndian::read_i32(&self.data[o..o + 4]))
    }

    fn first_sample_flags(&self) -> Option<u32> {
        self.optional_field_offset(flags::FIRST_SAMPLE_FLAGS_PRESENT)
            .map(|o| BigEndian::read_u32(&self.data[o..o + 4]))
    }

    fn samples(&self) -> impl Iterator<Item = TrackRunSample> + '_ {
        // When sample_entry_size is 0, there is no per-sample data to read,
        // so the iterator yields nothing regardless of sample_count. This
        // prevents a malicious sample_count from causing billions of no-op
        // iterations. Callers can still read sample_count() for the declared
        // count when per-sample defaults are provided via tfhd/trex.
        let entry_size = Self::sample_entry_size(self.fl, self.version);
        let count = if entry_size > 0 {
            self.sample_count as usize
        } else {
            0
        };

        // Precompute field offsets within each entry so we don't re-check
        // flags per sample.
        let mut off = 0usize;
        let duration_off = if self.fl & flags::SAMPLE_DURATION_PRESENT != 0 {
            let o = off; off += 4; Some(o)
        } else {
            None
        };
        let size_off = if self.fl & flags::SAMPLE_SIZE_PRESENT != 0 {
            let o = off; off += 4; Some(o)
        } else {
            None
        };
        let flags_off = if self.fl & flags::SAMPLE_FLAGS_PRESENT != 0 {
            let o = off; off += 4; Some(o)
        } else {
            None
        };
        let ctts_off = if self.fl & flags::SAMPLE_COMPOSITION_TIME_OFFSETS_PRESENT != 0 {
            Some(off)
        } else {
            None
        };
        let version = self.version;

        // entry_size may be 0 when no per-sample flags are set; use 1 to
        // avoid a chunks_exact(0) panic.  The slice is empty in that case
        // (count is 0), so the chunk size is irrelevant.
        let chunk_size = entry_size.max(1);
        let end = self.samples_offset + count * entry_size;
        self.data[self.samples_offset..end]
            .chunks_exact(chunk_size)
            .map(move |chunk| TrackRunSample {
                sample_duration: duration_off
                    .map(|o| u32::from_be_bytes(chunk[o..o + 4].try_into().unwrap())),
                sample_size: size_off
                    .map(|o| u32::from_be_bytes(chunk[o..o + 4].try_into().unwrap())),
                sample_flags: flags_off
                    .map(|o| u32::from_be_bytes(chunk[o..o + 4].try_into().unwrap())),
                sample_composition_time_offset: ctts_off.map(|o| {
                    if version == 1 {
                        i32::from_be_bytes(chunk[o..o + 4].try_into().unwrap()) as i64
                    } else {
                        u32::from_be_bytes(chunk[o..o + 4].try_into().unwrap()) as i64
                    }
                }),
            })
    }
}

impl std::fmt::Debug for TrackRunBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackRunBoxView")
            .field("sample_count", &self.sample_count())
            .field("flags", &format!("0x{:06X}", self.flags()))
            .field("data_offset", &self.data_offset())
            .finish()
    }
}

/// An owned representation of TrackRunBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct TrackRunBoxOwned {
    /// Data offset.
    pub data_offset: Option<i32>,
    /// First sample flags.
    pub first_sample_flags: Option<u32>,
    /// Samples.
    pub samples: Vec<TrackRunSample>,
    /// Use version 1 (signed composition offsets).
    pub use_signed_composition_offsets: bool,
}

impl TrackRunBoxOwned {
    /// Creates a new empty TrackRunBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Computes the flags based on which optional fields are present.
    fn compute_flags(&self) -> u32 {
        let mut fl = 0u32;
        if self.data_offset.is_some() {
            fl |= flags::DATA_OFFSET_PRESENT;
        }
        if self.first_sample_flags.is_some() {
            fl |= flags::FIRST_SAMPLE_FLAGS_PRESENT;
        }
        for sample in &self.samples {
            if sample.sample_duration.is_some() {
                fl |= flags::SAMPLE_DURATION_PRESENT;
            }
            if sample.sample_size.is_some() {
                fl |= flags::SAMPLE_SIZE_PRESENT;
            }
            if sample.sample_flags.is_some() {
                fl |= flags::SAMPLE_FLAGS_PRESENT;
            }
            if sample.sample_composition_time_offset.is_some() {
                fl |= flags::SAMPLE_COMPOSITION_TIME_OFFSETS_PRESENT;
            }
        }
        fl
    }

    /// Returns the version to use.
    fn version(&self) -> u8 {
        if self.use_signed_composition_offsets { 1 } else { 0 }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let fl = self.compute_flags();
        let mut payload = 4u64; // sample_count

        if fl & flags::DATA_OFFSET_PRESENT != 0 {
            payload += 4;
        }
        if fl & flags::FIRST_SAMPLE_FLAGS_PRESENT != 0 {
            payload += 4;
        }

        let sample_entry_size = TrackRunBoxView::sample_entry_size(fl, self.version());
        payload += self.samples.len() as u64 * sample_entry_size as u64;

        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        let fl = self.compute_flags();
        let version = self.version();
        write_fullbox_header(writer, size, BOX_TYPE, version, fl)?;
        writer.write_u32::<BigEndian>(self.samples.len() as u32)?;

        if let Some(v) = self.data_offset {
            writer.write_i32::<BigEndian>(v)?;
        }
        if let Some(v) = self.first_sample_flags {
            writer.write_u32::<BigEndian>(v)?;
        }

        for sample in &self.samples {
            if fl & flags::SAMPLE_DURATION_PRESENT != 0 {
                writer.write_u32::<BigEndian>(sample.sample_duration.unwrap_or(0))?;
            }
            if fl & flags::SAMPLE_SIZE_PRESENT != 0 {
                writer.write_u32::<BigEndian>(sample.sample_size.unwrap_or(0))?;
            }
            if fl & flags::SAMPLE_FLAGS_PRESENT != 0 {
                writer.write_u32::<BigEndian>(sample.sample_flags.unwrap_or(0))?;
            }
            if fl & flags::SAMPLE_COMPOSITION_TIME_OFFSETS_PRESENT != 0 {
                let offset = sample.sample_composition_time_offset.unwrap_or(0);
                if version == 1 {
                    writer.write_i32::<BigEndian>(offset as i32)?;
                } else {
                    writer.write_u32::<BigEndian>(offset as u32)?;
                }
            }
        }

        Ok(())
    }
}


impl TrackRunBox for TrackRunBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.version()
    }

    fn flags(&self) -> u32 {
        self.compute_flags()
    }

    fn sample_count(&self) -> u32 {
        self.samples.len() as u32
    }

    fn data_offset(&self) -> Option<i32> {
        self.data_offset
    }

    fn first_sample_flags(&self) -> Option<u32> {
        self.first_sample_flags
    }

    fn samples(&self) -> impl Iterator<Item = TrackRunSample> + '_ {
        self.samples.iter().copied()
    }
}

impl<T: TrackRunBox> From<&T> for TrackRunBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            data_offset: source.data_offset(),
            first_sample_flags: source.first_sample_flags(),
            samples: source.samples().collect(),
            use_signed_composition_offsets: source.version() == 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trun_minimal() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_be_bytes()); // size = 8 + 4 + 4 + 4
        data.extend_from_slice(b"trun");
        data.push(0); // version
        data.extend_from_slice(&[0x00, 0x00, 0x01]); // flags = data_offset_present
        data.extend_from_slice(&2u32.to_be_bytes()); // sample_count
        data.extend_from_slice(&100i32.to_be_bytes()); // data_offset
        data
    }

    fn make_trun_with_sizes() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&28u32.to_be_bytes()); // size
        data.extend_from_slice(b"trun");
        data.push(0); // version
        data.extend_from_slice(&[0x00, 0x02, 0x01]); // flags = data_offset + sample_size
        data.extend_from_slice(&2u32.to_be_bytes()); // sample_count
        data.extend_from_slice(&100i32.to_be_bytes()); // data_offset
        data.extend_from_slice(&1000u32.to_be_bytes()); // sample 0 size
        data.extend_from_slice(&2000u32.to_be_bytes()); // sample 1 size
        data
    }

    #[test]
    fn parse_trun_minimal() {
        let data = make_trun_minimal();
        let view = TrackRunBoxView::new(&data).unwrap();

        assert_eq!(view.sample_count(), 2);
        assert_eq!(view.data_offset(), Some(100));
        assert!(view.first_sample_flags().is_none());
    }

    #[test]
    fn parse_trun_with_sizes() {
        let data = make_trun_with_sizes();
        let view = TrackRunBoxView::new(&data).unwrap();

        assert_eq!(view.sample_count(), 2);
        assert_eq!(view.data_offset(), Some(100));

        let sample0 = view.sample(0).unwrap();
        assert_eq!(sample0.sample_size, Some(1000));

        let sample1 = view.sample(1).unwrap();
        assert_eq!(sample1.sample_size, Some(2000));
    }

    #[test]
    fn compute_flags_checks_all_samples() {
        let owned = TrackRunBoxOwned {
            data_offset: None,
            first_sample_flags: None,
            use_signed_composition_offsets: false,
            samples: vec![
                TrackRunSample {
                    sample_duration: None,
                    sample_size: None,
                    sample_flags: None,
                    sample_composition_time_offset: None,
                },
                TrackRunSample {
                    sample_duration: Some(1024),
                    sample_size: Some(512),
                    sample_flags: None,
                    sample_composition_time_offset: None,
                },
            ],
        };

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        let view = TrackRunBoxView::new(&output).unwrap();
        assert_eq!(view.sample_count(), 2);

        let s0 = view.sample(0).unwrap();
        assert_eq!(s0.sample_duration, Some(0));
        assert_eq!(s0.sample_size, Some(0));

        let s1 = view.sample(1).unwrap();
        assert_eq!(s1.sample_duration, Some(1024));
        assert_eq!(s1.sample_size, Some(512));
    }

    /// Regression test for fuzz-discovered DoS: a trun with no per-sample
    /// flags but a huge sample_count (0x6F6D0020 ≈ 1.87 billion) must not
    /// cause samples() to busy-loop.  If the fix regresses, .count() will
    /// attempt ~1.87 billion iterations and the test will hang/timeout.
    #[test]
    fn huge_sample_count_with_zero_entry_size_does_not_hang() {
        // Construct a trun where no per-sample flags are set (entry_size = 0)
        // but sample_count is enormous.
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes()); // size = 8 header + 4 ver/flags + 4 count
        data.extend_from_slice(b"trun");
        data.push(0);                                 // version
        data.extend_from_slice(&[0x00, 0x00, 0x00]);  // flags = 0 (no per-sample fields)
        data.extend_from_slice(&0x6F6D0020u32.to_be_bytes()); // sample_count ≈ 1.87 billion

        let view = TrackRunBoxView::new(&data).unwrap();

        // The declared count is preserved for callers that need it.
        assert_eq!(view.sample_count(), 0x6F6D0020);

        // The iterator must yield nothing - there is no per-sample data to
        // read.  Using .next() rather than .count() so that a regression
        // fails instantly (returns Some) instead of looping for minutes.
        assert!(view.samples().next().is_none());
    }

    #[test]
    fn roundtrip() {
        let data = make_trun_with_sizes();
        let view = TrackRunBoxView::new(&data).unwrap();
        let owned = TrackRunBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
