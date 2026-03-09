//! Sample Description Box (stsd) parsing and serialization.
//!
//! The Sample Description Box contains information about the coding type used
//! and any initialization information needed for that coding.
//!
//! ```text
//! aligned(8) class SampleDescriptionBox (unsigned int(32) handler_type)
//!    extends FullBox('stsd', version, 0) {
//!    int i;
//!    unsigned int(32) entry_count;
//!    for (i = 1 ; i <= entry_count ; i++){
//!       SampleEntry();  // an instance of a class derived from SampleEntry
//!    }
//! }
//! ```

use crate::boxes::sample_entry::{
    self, AudioSampleEntryOwned, AudioSampleEntryView, SampleEntryKind, VisualSampleEntryOwned,
    VisualSampleEntryView,
};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SampleDescriptionBox.
pub const BOX_TYPE: BoxCode = BoxCode::STSD;

/// A typed child of a SampleDescriptionBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SampleDescriptionChild {
    /// A visual sample entry (avc1, hvc1, etc.).
    Visual(VisualSampleEntryOwned),
    /// An audio sample entry (mp4a, ac-3, etc.).
    Audio(AudioSampleEntryOwned),
    /// An unknown or unrecognized sample entry.
    Other(OpaqueBoxOwned),
}

impl ChildBox for SampleDescriptionChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Visual(v) => sample_entry::SampleEntry::box_type(v),
            Self::Audio(a) => sample_entry::SampleEntry::box_type(a),
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Visual(v) => sample_entry::SampleEntry::box_size(v),
            Self::Audio(a) => sample_entry::SampleEntry::box_size(a),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl SampleDescriptionChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Visual(v) => v.write_to(writer),
            Self::Audio(a) => a.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for SampleDescriptionChild {
    fn from(raw: RawBox<'_>) -> Self {
        match SampleEntryKind::from_box_type(raw.box_type()) {
            SampleEntryKind::Visual => match VisualSampleEntryView::new(raw.data()) {
                Ok(v) => Self::Visual(VisualSampleEntryOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            SampleEntryKind::Audio => match AudioSampleEntryView::new(raw.data()) {
                Ok(v) => Self::Audio(AudioSampleEntryOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&SampleDescriptionChild> for SampleDescriptionChild {
    fn from(source: &SampleDescriptionChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing SampleDescriptionBox data.
pub trait SampleDescriptionBox {
    /// The type of child items yielded by the entries iterator.
    type Child<'a>: ChildBox + Into<SampleDescriptionChild>
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

    /// Returns the number of sample entries.
    fn entry_count(&self) -> u32;

    /// Returns an iterator over the sample entries.
    fn entries(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw SampleDescriptionBox bytes.
#[derive(Clone, Copy)]
pub struct SampleDescriptionBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> SampleDescriptionBoxView<'a> {
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

    #[inline]
    fn payload_offset(&self) -> usize {
        self.fullbox_offset + 4
    }

    /// Returns an iterator over the sample entry boxes.
    pub fn entries(&self) -> BoxIterator<'a> {
        let o = self.payload_offset() + 4;
        BoxIterator::new(&self.data[o..])
    }
}

impl<'a> SampleDescriptionBox for SampleDescriptionBoxView<'a> {
    type Child<'b> = RawBox<'b> where Self: 'b;

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

    fn entry_count(&self) -> u32 {
        let o = self.payload_offset();
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn entries(&self) -> impl Iterator<Item = RawBox<'_>> {
        SampleDescriptionBoxView::entries(self)
    }
}

impl std::fmt::Debug for SampleDescriptionBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleDescriptionBoxView")
            .field("box_size", &SampleDescriptionBox::box_size(self))
            .field("entry_count", &SampleDescriptionBox::entry_count(self))
            .finish()
    }
}

/// An owned representation of SampleDescriptionBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SampleDescriptionBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Typed sample entries.
    pub entries: Vec<SampleDescriptionChild>,
}

impl SampleDescriptionBoxOwned {
    /// Creates a new empty SampleDescriptionBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = 4 + self.entries.iter().map(|c| c.box_size()).sum::<u64>();
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;
        for child in &self.entries {
            child.write_to(writer)?;
        }
        Ok(())
    }
}

impl SampleDescriptionBox for SampleDescriptionBoxOwned {
    type Child<'a> = &'a SampleDescriptionChild;

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

    fn entry_count(&self) -> u32 {
        self.entries.len() as u32
    }

    fn entries(&self) -> impl Iterator<Item = &SampleDescriptionChild> {
        self.entries.iter()
    }
}

impl<T: SampleDescriptionBox> From<&T> for SampleDescriptionBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            entries: source.entries().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boxes::sample_entry::AUDIO_HEADER_SIZE;

    fn make_stsd(entry_data: &[u8], entry_count: u32) -> Vec<u8> {
        let size = 8 + 4 + 4 + entry_data.len();
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"stsd");
        data.push(0);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&entry_count.to_be_bytes());
        data.extend_from_slice(entry_data);
        data
    }

    fn make_audio_entry() -> Vec<u8> {
        // Build a valid audio sample entry (mp4a) with AUDIO_HEADER_SIZE bytes
        let size = AUDIO_HEADER_SIZE as u32;
        let mut entry = vec![0u8; AUDIO_HEADER_SIZE];
        entry[0..4].copy_from_slice(&size.to_be_bytes());
        entry[4..8].copy_from_slice(b"mp4a");
        // reserved (6 bytes) at 8-13 = zeros
        // data_reference_index at 14-15
        entry[14..16].copy_from_slice(&1u16.to_be_bytes());
        // reserved (8 bytes) at 16-23 = zeros
        // channel_count at 24-25
        entry[24..26].copy_from_slice(&2u16.to_be_bytes());
        // sample_size at 26-27
        entry[26..28].copy_from_slice(&16u16.to_be_bytes());
        // pre_defined at 28-29 = zeros
        // reserved at 30-31 = zeros
        // sample_rate at 32-35 (44100 << 16)
        entry[32..36].copy_from_slice(&((44100u32) << 16).to_be_bytes());
        entry
    }

    #[test]
    fn parse_stsd() {
        let entry = make_audio_entry();
        let data = make_stsd(&entry, 1);
        let view = SampleDescriptionBoxView::new(&data).unwrap();

        assert_eq!(view.entry_count(), 1);
        let entries: Vec<_> = view.entries().collect();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn roundtrip() {
        let entry = make_audio_entry();
        let data = make_stsd(&entry, 1);
        let view = SampleDescriptionBoxView::new(&data).unwrap();
        let owned = SampleDescriptionBoxOwned::from(&view);

        assert_eq!(owned.entries.len(), 1);
        assert!(matches!(owned.entries[0], SampleDescriptionChild::Audio(_)));

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        // Re-parse the output to verify it produces an identical view
        let view2 = SampleDescriptionBoxView::new(&output).unwrap();
        assert_eq!(view2.entry_count(), view.entry_count());
        assert_eq!(output, data);
    }
}
