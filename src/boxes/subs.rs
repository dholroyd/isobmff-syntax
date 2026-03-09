//! Sub-Sample Information Box (subs) parsing and serialization.
//!
//! The Sub-Sample Information Box provides information about sub-samples.
//!
//! ```text
//! aligned(8) class SubSampleInformationBox
//!    extends FullBox('subs', version, flags) {
//!    unsigned int(32) entry_count;
//!    for (i=1; i <= entry_count; i++) {
//!       unsigned int(32) sample_delta;
//!       unsigned int(16) subsample_count;
//!       if (subsample_count > 0) {
//!          for (j=1; j <= subsample_count; j++) {
//!             if(version == 1) {
//!                unsigned int(32) subsample_size;
//!             } else {
//!                unsigned int(16) subsample_size;
//!             }
//!             unsigned int(8) subsample_priority;
//!             unsigned int(8) discardable;
//!             unsigned int(32) codec_specific_parameters;
//!          }
//!       }
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SubSampleInformationBox.
pub const BOX_TYPE: BoxCode = BoxCode::SUBS;

/// Information about a sub-sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubSampleEntry {
    /// Size of this sub-sample.
    pub subsample_size: u32,
    /// Priority of this sub-sample.
    pub subsample_priority: u8,
    /// Whether this sub-sample is discardable.
    pub discardable: u8,
    /// Codec-specific parameters.
    pub codec_specific_parameters: u32,
}

/// Shared interface over a subs entry describing the sub-samples of one sample.
///
/// Implemented by both the borrowing [`SampleSubSampleEntryView`] and by
/// `&SampleSubSampleEntryOwned`.
pub trait SampleSubSampleEntry {
    /// Returns the delta from the previous sample with sub-sample information.
    fn sample_delta(&self) -> u32;

    /// Returns the number of sub-samples.
    fn subsample_count(&self) -> usize;

    /// Returns an iterator over the sub-samples.
    fn sub_samples(&self) -> impl Iterator<Item = SubSampleEntry> + '_;

    /// Materialises an owned copy of this entry.
    fn to_owned(&self) -> SampleSubSampleEntryOwned {
        SampleSubSampleEntryOwned {
            sample_delta: self.sample_delta(),
            sub_samples: self.sub_samples().collect(),
        }
    }
}

/// A borrowing view over a single `subs` entry's raw bytes.
#[derive(Clone, Copy)]
pub struct SampleSubSampleEntryView<'a> {
    sample_delta: u32,
    sub_samples_data: &'a [u8],
    subsample_count: usize,
    version: u8,
}

impl<'a> SampleSubSampleEntry for SampleSubSampleEntryView<'a> {
    fn sample_delta(&self) -> u32 {
        self.sample_delta
    }

    fn subsample_count(&self) -> usize {
        self.subsample_count
    }

    fn sub_samples(&self) -> impl Iterator<Item = SubSampleEntry> + '_ {
        let version = self.version;
        let size_bytes: usize = if version == 1 { 4 } else { 2 };
        let stride = size_bytes + 6;
        self.sub_samples_data
            .chunks_exact(stride)
            .map(move |chunk| {
                let subsample_size = if size_bytes == 4 {
                    BigEndian::read_u32(&chunk[..4])
                } else {
                    BigEndian::read_u16(&chunk[..2]) as u32
                };
                SubSampleEntry {
                    subsample_size,
                    subsample_priority: chunk[size_bytes],
                    discardable: chunk[size_bytes + 1],
                    codec_specific_parameters: BigEndian::read_u32(
                        &chunk[size_bytes + 2..size_bytes + 6],
                    ),
                }
            })
    }
}

/// An owned `subs` entry describing the sub-samples of one sample.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleSubSampleEntryOwned {
    /// Delta from previous sample with sub-sample information.
    pub sample_delta: u32,
    /// Sub-samples for this sample.
    pub sub_samples: Vec<SubSampleEntry>,
}

impl SampleSubSampleEntry for &SampleSubSampleEntryOwned {
    fn sample_delta(&self) -> u32 {
        self.sample_delta
    }

    fn subsample_count(&self) -> usize {
        self.sub_samples.len()
    }

    fn sub_samples(&self) -> impl Iterator<Item = SubSampleEntry> + '_ {
        self.sub_samples.iter().copied()
    }

    fn to_owned(&self) -> SampleSubSampleEntryOwned {
        (*self).clone()
    }
}

/// Common interface for accessing SubSampleInformationBox data.
pub trait SubSampleInformationBox {
    /// The concrete entry type yielded by [`Self::entries`].
    type Entry<'a>: SampleSubSampleEntry
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the entry count.
    fn entry_count(&self) -> u32;

    /// Returns an iterator over all entries.
    fn entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_;
}

/// A borrowing view over raw SubSampleInformationBox bytes.
#[derive(Clone, Copy)]
pub struct SubSampleInformationBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
    entry_count: u32,
}

impl<'a> SubSampleInformationBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 4)?;

        let version = header.version;
        let entry_count = BigEndian::read_u32(&data[fullbox_offset + 4..fullbox_offset + 8]);

        Ok(Self {
            data,
            fullbox_offset,
            version,
            entry_count,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

}

impl<'a> SubSampleInformationBox for SubSampleInformationBoxView<'a> {
    type Entry<'b> = SampleSubSampleEntryView<'b> where Self: 'b;

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

    fn entry_count(&self) -> u32 {
        self.entry_count
    }

    fn entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_ {
        let subsample_size_bytes: usize = if self.version == 1 { 4 } else { 2 };
        let per_subsample = subsample_size_bytes + 6;
        let mut offset = self.fullbox_offset + 8;
        let mut remaining = self.entry_count;
        let mut errored = false;

        std::iter::from_fn(move || {
            if errored || remaining == 0 {
                return None;
            }
            remaining -= 1;

            if offset + 6 > self.data.len() {
                errored = true;
                return Some(Err(ParseError::BufferTooShort {
                    expected: offset + 6,
                    found: self.data.len(),
                }));
            }

            let sample_delta = BigEndian::read_u32(&self.data[offset..offset + 4]);
            offset += 4;

            let subsample_count = BigEndian::read_u16(&self.data[offset..offset + 2]) as usize;
            offset += 2;

            // Validate all sub-samples fit
            let subsamples_total = match subsample_count.checked_mul(per_subsample) {
                Some(total) => total,
                None => {
                    errored = true;
                    return Some(Err(ParseError::BufferTooShort {
                        expected: usize::MAX,
                        found: self.data.len(),
                    }));
                }
            };
            if offset + subsamples_total > self.data.len() {
                errored = true;
                return Some(Err(ParseError::BufferTooShort {
                    expected: offset + subsamples_total,
                    found: self.data.len(),
                }));
            }

            let sub_samples_data = &self.data[offset..offset + subsamples_total];
            offset += subsamples_total;

            Some(Ok(SampleSubSampleEntryView {
                sample_delta,
                sub_samples_data,
                subsample_count,
                version: self.version,
            }))
        })
    }
}

impl std::fmt::Debug for SubSampleInformationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubSampleInformationBoxView")
            .field("version", &self.version())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of SubSampleInformationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct SubSampleInformationBoxOwned {
    /// Version (0 or 1).
    pub version: u8,
    /// Flags.
    pub flags: u32,
    /// Entries.
    pub entries: Vec<SampleSubSampleEntryOwned>,
}

impl SubSampleInformationBoxOwned {
    /// Creates a new SubSampleInformationBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let mut payload = 4u64; // entry_count
        for entry in &self.entries {
            payload += 6; // sample_delta(4) + subsample_count(2)
            let subsample_size_bytes: u64 = if self.version == 1 { 4 } else { 2 };
            payload += entry.sub_samples.len() as u64 * (subsample_size_bytes + 6);
        }
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_u8(self.version)?;
        writer.write_u24::<BigEndian>(self.flags)?;
        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

        for entry in &self.entries {
            writer.write_u32::<BigEndian>(entry.sample_delta)?;
            writer.write_u16::<BigEndian>(entry.sub_samples.len() as u16)?;

            for subsample in &entry.sub_samples {
                if self.version == 1 {
                    writer.write_u32::<BigEndian>(subsample.subsample_size)?;
                } else {
                    writer.write_u16::<BigEndian>(subsample.subsample_size as u16)?;
                }
                writer.write_u8(subsample.subsample_priority)?;
                writer.write_u8(subsample.discardable)?;
                writer.write_u32::<BigEndian>(subsample.codec_specific_parameters)?;
            }
        }

        Ok(())
    }
}


impl SubSampleInformationBox for SubSampleInformationBoxOwned {
    type Entry<'a> = &'a SampleSubSampleEntryOwned where Self: 'a;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.version
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn entry_count(&self) -> u32 {
        self.entries.len() as u32
    }

    fn entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_ {
        self.entries.iter().map(Ok)
    }
}

impl TryFrom<&SubSampleInformationBoxView<'_>> for SubSampleInformationBoxOwned {
    type Error = ParseError;

    fn try_from(source: &SubSampleInformationBoxView<'_>) -> Result<Self, Self::Error> {
        let entries = SubSampleInformationBox::entries(source)
            .map(|res| res.map(|v| SampleSubSampleEntry::to_owned(&v)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            version: source.version(),
            flags: source.flags(),
            entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_subs_v0() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 4 + 6 + 8 = 30 bytes
        data.extend_from_slice(&30u32.to_be_bytes());
        data.extend_from_slice(b"subs");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1u32.to_be_bytes()); // entry_count
        data.extend_from_slice(&1u32.to_be_bytes()); // sample_delta
        data.extend_from_slice(&1u16.to_be_bytes()); // subsample_count
        data.extend_from_slice(&1000u16.to_be_bytes()); // subsample_size
        data.push(1); // priority
        data.push(0); // discardable
        data.extend_from_slice(&0u32.to_be_bytes()); // codec_specific_parameters
        data
    }

    #[test]
    fn parse_subs_v0() {
        let data = make_subs_v0();
        let view = SubSampleInformationBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.entry_count(), 1);

        let entries: Vec<_> =
            SubSampleInformationBox::entries(&view).collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].sample_delta(), 1);
        assert_eq!(entries[0].subsample_count(), 1);
        let subs: Vec<_> = entries[0].sub_samples().collect();
        assert_eq!(subs[0].subsample_size, 1000);
        assert_eq!(subs[0].subsample_priority, 1);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_subs_v0();
        let view = SubSampleInformationBoxView::new(&data).unwrap();
        let owned = SubSampleInformationBoxOwned::try_from(&view).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
