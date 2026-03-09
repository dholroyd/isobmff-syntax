//! Sample Entry box parsing.
//!
//! Sample entries are contained within the Sample Description Box (stsd).
//! This module provides views for the base SampleEntry and its extensions:
//! VisualSampleEntry, AudioSampleEntry, and others.
//!
//! Each sample entry type can contain child boxes (e.g., avcC inside avc1,
//! esds inside mp4a).
//!
//! ```text
//! aligned(8) abstract class SampleEntry(codingname)
//!    extends Box(codingname) {
//!    const unsigned int(8)[6] reserved = 0;
//!    unsigned int(16) data_reference_index;
//! }
//!
//! class VisualSampleEntry(codingname) extends SampleEntry(codingname){
//!    unsigned int(16) pre_defined = 0;
//!    const unsigned int(16) reserved = 0;
//!    unsigned int(32)[3] pre_defined = 0;
//!    unsigned int(16) width;
//!    unsigned int(16) height;
//!    template unsigned int(32) horizresolution = 0x00480000; // 72 dpi
//!    template unsigned int(32) vertresolution = 0x00480000; // 72 dpi
//!    const unsigned int(32) reserved = 0;
//!    template unsigned int(16) frame_count = 1;
//!    uint(8)[32] compressorname;
//!    template unsigned int(16) depth = 0x0018;
//!    int(16) pre_defined = -1;
//!    // other boxes from derived specifications
//!    CleanApertureBox clap; // optional
//!    PixelAspectRatioBox pasp; // optional
//! }
//!
//! class AudioSampleEntry(codingname) extends SampleEntry(codingname){
//!    const unsigned int(32)[2] reserved = 0;
//!    template unsigned int(16) channelcount = 2;
//!    template unsigned int(16) samplesize = 16;
//!    unsigned int(16) pre_defined = 0;
//!    const unsigned int(16) reserved = 0;
//!    template unsigned int(32) samplerate = { default samplerate of media} << 16;
//! }
//! ```

use crate::container::{BoxIterator, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use crate::types::UFixedPoint16_16;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, HandlerCode, SampleEntryCode};
use std::io::{self, Write};

/// Base sample entry header size: 8 (box header) + 6 (reserved) + 2 (data_reference_index).
pub const BASE_HEADER_SIZE: usize = 16;

/// Visual sample entry header size: base (16) + visual fields (70) = 86 bytes.
pub const VISUAL_HEADER_SIZE: usize = 86;

/// Audio sample entry header size: base (16) + audio fields (20) = 36 bytes.
pub const AUDIO_HEADER_SIZE: usize = 36;

/// Text sample entry header size: base (16) + text fields (30) = 46 bytes (tx3g).
/// Fields: displayFlags(4) + justification(2) + background-color(4) +
/// default-text-box(8) + default-style(12) = 30 bytes.
pub const TEXT_HEADER_SIZE: usize = 46;

/// Determines the sample entry category based on box type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleEntryKind {
    /// Visual sample entry (avc1, hvc1, etc.) - 86 byte header.
    Visual,
    /// Audio sample entry (mp4a, ac-3, etc.) - 36 byte header.
    Audio,
    /// Text sample entry (tx3g) - 46 byte header.
    Text,
    /// Subtitle/Text sample entry (stxt, wvtt, etc.) - 16 byte header.
    SubtitleText,
    /// Other sample entry types - 16 byte header.
    Other,
}

impl SampleEntryKind {
    /// Determines the sample entry kind from a box type code.
    ///
    /// Uses mp4ra-rust's SampleEntryCode::handler() to determine the handler type,
    /// replacing hardcoded codec lists with the official MP4RA registry.
    pub fn from_box_type(box_type: BoxCode) -> Self {
        let sample_code = SampleEntryCode::new(box_type.0 .0);

        // tx3g has special handling - it uses TEXT handler but has a 46-byte header
        if sample_code == SampleEntryCode::TX3G {
            return Self::Text;
        }

        // stxt (Simple Text) is a subtitle format with base header
        if sample_code == SampleEntryCode::STXT {
            return Self::SubtitleText;
        }

        // Use mp4ra-rust to determine handler type
        match sample_code.handler() {
            Some(HandlerCode::VIDE) => Self::Visual,
            Some(HandlerCode::SOUN) => Self::Audio,
            Some(HandlerCode::TEXT) => Self::SubtitleText,
            Some(HandlerCode::SUBT) => Self::SubtitleText,
            _ => Self::Other,
        }
    }

    /// Returns the header size for this sample entry kind.
    pub fn header_size(self) -> usize {
        match self {
            Self::Visual => VISUAL_HEADER_SIZE,
            Self::Audio => AUDIO_HEADER_SIZE,
            Self::Text => TEXT_HEADER_SIZE,
            Self::SubtitleText | Self::Other => BASE_HEADER_SIZE,
        }
    }
}

/// Common interface for all sample entries.
pub trait SampleEntry {
    /// Returns the total size of the sample entry box.
    fn box_size(&self) -> u64;

    /// Returns the box type (codec identifier).
    fn box_type(&self) -> BoxCode;

    /// Returns the data reference index.
    fn data_reference_index(&self) -> u16;
}

/// Common interface for accessing visual sample entry fields.
///
/// Types implementing this trait should also implement [`SampleEntry`]
/// for the common `box_size`, `box_type`, and `data_reference_index` accessors.
pub trait VisualSampleEntry {
    /// Returns the video width in pixels.
    fn width(&self) -> u16;

    /// Returns the video height in pixels.
    fn height(&self) -> u16;

    /// Returns the horizontal resolution (pixels per inch) as unsigned fixed 16.16.
    fn horiz_resolution(&self) -> UFixedPoint16_16;

    /// Returns the vertical resolution (pixels per inch) as unsigned fixed 16.16.
    fn vert_resolution(&self) -> UFixedPoint16_16;

    /// Returns the frame count (typically 1).
    fn frame_count(&self) -> u16;

    /// Returns the compressor name.
    fn compressor_name(&self) -> &str;

    /// Returns the bit depth (typically 24 for color).
    fn depth(&self) -> u16;

    /// Returns the raw child box data after the visual sample entry header.
    fn children_data(&self) -> &[u8];
}

/// Common interface for accessing audio sample entry fields.
///
/// Types implementing this trait should also implement [`SampleEntry`]
/// for the common `box_size`, `box_type`, and `data_reference_index` accessors.
pub trait AudioSampleEntry {
    /// Returns the number of audio channels (template: 2).
    fn channel_count(&self) -> u16;

    /// Returns the sample size in bits (template: 16).
    fn sample_size(&self) -> u16;

    /// Returns the sample rate as unsigned fixed 16.16.
    fn sample_rate(&self) -> UFixedPoint16_16;

    /// Returns the raw child box data after the audio sample entry header.
    fn children_data(&self) -> &[u8];
}

/// A borrowing view over a sample entry box.
///
/// This view can parse any sample entry type and provides access to
/// child boxes within the sample entry.
#[derive(Clone, Copy)]
pub struct SampleEntryView<'a> {
    data: &'a [u8],
    header: BoxHeader,
    kind: SampleEntryKind,
}

impl<'a> SampleEntryView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;

        if header.size != data.len() as u64 {
            return Err(ParseError::SizeMismatch {
                declared: header.size,
                actual: data.len(),
            });
        }

        // Need at least base header size
        if data.len() < BASE_HEADER_SIZE {
            return Err(ParseError::BufferTooShort {
                expected: BASE_HEADER_SIZE,
                found: data.len(),
            });
        }

        let kind = SampleEntryKind::from_box_type(header.box_type);

        Ok(Self { data, header, kind })
    }

    /// Creates a view from a RawBox.
    pub fn from_raw_box(raw: &RawBox<'a>) -> Result<Self, ParseError> {
        Self::new(raw.data())
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the sample entry kind (Visual, Audio, or Other).
    #[inline]
    pub fn kind(&self) -> SampleEntryKind {
        self.kind
    }

    /// Returns the header size for this sample entry type.
    #[inline]
    pub fn header_size(&self) -> usize {
        self.kind.header_size()
    }

    /// Returns the payload data after the sample entry header (child boxes).
    pub fn children_data(&self) -> &'a [u8] {
        let header_size = self.header_size();
        if self.data.len() > header_size {
            &self.data[header_size..]
        } else {
            &[]
        }
    }
}

impl SampleEntry for SampleEntryView<'_> {
    fn box_size(&self) -> u64 {
        self.header.size
    }

    fn box_type(&self) -> BoxCode {
        self.header.box_type
    }

    fn data_reference_index(&self) -> u16 {
        // data_reference_index is at offset 14-15 (after 8 byte header + 6 reserved)
        BigEndian::read_u16(&self.data[14..16])
    }
}

impl<'a> SampleEntryView<'a> {
    /// Returns an iterator over child boxes.
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(self.children_data())
    }
}

impl std::fmt::Debug for SampleEntryView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleEntryView")
            .field("box_type", &String::from_utf8_lossy(&self.header.box_type.0 .0))
            .field("box_size", &self.box_size())
            .field("kind", &self.kind)
            .field("data_reference_index", &self.data_reference_index())
            .finish()
    }
}

// ============================================================================
// Visual Sample Entry
// ============================================================================

/// A borrowing view over a visual sample entry (avc1, hvc1, etc.).
///
/// Visual sample entries contain video codec information and child boxes
/// like avcC (AVC configuration), hvcC (HEVC configuration), etc.
#[derive(Clone, Copy)]
pub struct VisualSampleEntryView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> VisualSampleEntryView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;

        if header.size != data.len() as u64 {
            return Err(ParseError::SizeMismatch {
                declared: header.size,
                actual: data.len(),
            });
        }

        if data.len() < VISUAL_HEADER_SIZE {
            return Err(ParseError::BufferTooShort {
                expected: VISUAL_HEADER_SIZE,
                found: data.len(),
            });
        }

        // Validate compressor name: length byte must not exceed 31 (the fixed field size)
        let compressor_name_len = data[50] as usize;
        if compressor_name_len > 31 {
            return Err(ParseError::InvalidFieldSize {
                field: "compressor_name_length",
                size: data[50],
            });
        }
        // Validate compressor name is valid UTF-8
        if std::str::from_utf8(&data[51..51 + compressor_name_len]).is_err() {
            return Err(ParseError::InvalidStringEncoding {
                context: "compressor_name",
            });
        }

        Ok(Self { data, header })
    }

    /// Creates a view from a RawBox.
    pub fn from_raw_box(raw: &RawBox<'a>) -> Result<Self, ParseError> {
        Self::new(raw.data())
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the payload data after the visual sample entry header (child boxes).
    pub fn children_data(&self) -> &'a [u8] {
        if self.data.len() > VISUAL_HEADER_SIZE {
            &self.data[VISUAL_HEADER_SIZE..]
        } else {
            &[]
        }
    }
}

impl SampleEntry for VisualSampleEntryView<'_> {
    fn box_size(&self) -> u64 {
        self.header.size
    }

    fn box_type(&self) -> BoxCode {
        self.header.box_type
    }

    fn data_reference_index(&self) -> u16 {
        BigEndian::read_u16(&self.data[14..16])
    }
}

impl VisualSampleEntry for VisualSampleEntryView<'_> {
    fn width(&self) -> u16 {
        BigEndian::read_u16(&self.data[32..34])
    }

    fn height(&self) -> u16 {
        BigEndian::read_u16(&self.data[34..36])
    }

    fn horiz_resolution(&self) -> UFixedPoint16_16 {
        UFixedPoint16_16::from_raw(BigEndian::read_u32(&self.data[36..40]))
    }

    fn vert_resolution(&self) -> UFixedPoint16_16 {
        UFixedPoint16_16::from_raw(BigEndian::read_u32(&self.data[40..44]))
    }

    fn frame_count(&self) -> u16 {
        BigEndian::read_u16(&self.data[48..50])
    }

    fn compressor_name(&self) -> &str {
        // Length and UTF-8 validated in constructor
        let len = self.data[50] as usize;
        std::str::from_utf8(&self.data[51..51 + len]).unwrap()
    }

    fn depth(&self) -> u16 {
        BigEndian::read_u16(&self.data[82..84])
    }

    fn children_data(&self) -> &[u8] {
        self.children_data()
    }
}

impl<'a> VisualSampleEntryView<'a> {
    /// Returns an iterator over child boxes.
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(self.children_data())
    }
}

impl std::fmt::Debug for VisualSampleEntryView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VisualSampleEntryView")
            .field("box_type", &String::from_utf8_lossy(&self.header.box_type.0 .0))
            .field("width", &self.width())
            .field("height", &self.height())
            .field("depth", &self.depth())
            .finish()
    }
}

/// An owned representation of a visual sample entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisualSampleEntryOwned {
    /// The codec identifier (e.g. avc1, hvc1).
    pub box_type: BoxCode,
    /// Data reference index.
    pub data_reference_index: u16,
    /// Video width in pixels.
    pub width: u16,
    /// Video height in pixels.
    pub height: u16,
    /// Horizontal resolution (template: 72 dpi = 0x00480000).
    pub horiz_resolution: UFixedPoint16_16,
    /// Vertical resolution (template: 72 dpi = 0x00480000).
    pub vert_resolution: UFixedPoint16_16,
    /// Number of frames per sample (template: 1).
    pub frame_count: u16,
    /// Compressor name (up to 31 characters).
    pub compressor_name: String,
    /// Bit depth (template: 24).
    pub depth: u16,
    /// Raw child box data (e.g. avcC, hvcC, clap, pasp).
    pub children_data: Vec<u8>,
}

impl VisualSampleEntryOwned {
    /// Creates a new VisualSampleEntryOwned with default template values.
    pub fn new(box_type: BoxCode) -> Self {
        Self {
            box_type,
            ..Default::default()
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (VISUAL_HEADER_SIZE - 8) as u64 + self.children_data.len() as u64;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, self.box_type)?;
        // reserved (6 bytes)
        writer.write_all(&[0u8; 6])?;
        // data_reference_index
        writer.write_u16::<BigEndian>(self.data_reference_index)?;
        // pre_defined = 0
        writer.write_u16::<BigEndian>(0)?;
        // reserved = 0
        writer.write_u16::<BigEndian>(0)?;
        // pre_defined = 0 (12 bytes)
        writer.write_all(&[0u8; 12])?;
        // width
        writer.write_u16::<BigEndian>(self.width)?;
        // height
        writer.write_u16::<BigEndian>(self.height)?;
        // horizresolution
        writer.write_u32::<BigEndian>(self.horiz_resolution.raw())?;
        // vertresolution
        writer.write_u32::<BigEndian>(self.vert_resolution.raw())?;
        // reserved = 0
        writer.write_u32::<BigEndian>(0)?;
        // frame_count
        writer.write_u16::<BigEndian>(self.frame_count)?;
        // compressorname (32 bytes: 1 length byte + up to 31 chars + padding)
        let name_bytes = self.compressor_name.as_bytes();
        let name_len = name_bytes.len().min(31);
        writer.write_u8(name_len as u8)?;
        writer.write_all(&name_bytes[..name_len])?;
        for _ in 0..(31 - name_len) {
            writer.write_u8(0)?;
        }
        // depth
        writer.write_u16::<BigEndian>(self.depth)?;
        // pre_defined = -1
        writer.write_i16::<BigEndian>(-1)?;
        // children
        writer.write_all(&self.children_data)?;
        Ok(())
    }
}

impl Default for VisualSampleEntryOwned {
    fn default() -> Self {
        Self {
            box_type: BoxCode::new(*b"avc1"),
            data_reference_index: 1,
            width: 0,
            height: 0,
            horiz_resolution: UFixedPoint16_16::from_raw(0x00480000), // 72 dpi
            vert_resolution: UFixedPoint16_16::from_raw(0x00480000),  // 72 dpi
            frame_count: 1,
            compressor_name: String::new(),
            depth: 0x0018,
            children_data: Vec::new(),
        }
    }
}

impl SampleEntry for VisualSampleEntryOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        self.box_type
    }

    fn data_reference_index(&self) -> u16 {
        self.data_reference_index
    }
}

impl VisualSampleEntry for VisualSampleEntryOwned {
    fn width(&self) -> u16 {
        self.width
    }

    fn height(&self) -> u16 {
        self.height
    }

    fn horiz_resolution(&self) -> UFixedPoint16_16 {
        self.horiz_resolution
    }

    fn vert_resolution(&self) -> UFixedPoint16_16 {
        self.vert_resolution
    }

    fn frame_count(&self) -> u16 {
        self.frame_count
    }

    fn compressor_name(&self) -> &str {
        &self.compressor_name
    }

    fn depth(&self) -> u16 {
        self.depth
    }

    fn children_data(&self) -> &[u8] {
        &self.children_data
    }
}

impl<T: VisualSampleEntry + SampleEntry> From<&T> for VisualSampleEntryOwned {
    fn from(source: &T) -> Self {
        Self {
            box_type: source.box_type(),
            data_reference_index: source.data_reference_index(),
            width: source.width(),
            height: source.height(),
            horiz_resolution: source.horiz_resolution(),
            vert_resolution: source.vert_resolution(),
            frame_count: source.frame_count(),
            compressor_name: source.compressor_name().to_string(),
            depth: source.depth(),
            children_data: source.children_data().to_vec(),
        }
    }
}

// ============================================================================
// Audio Sample Entry
// ============================================================================

/// A borrowing view over an audio sample entry (mp4a, ac-3, etc.).
///
/// Audio sample entries contain audio codec information and child boxes
/// like esds (ES descriptor), dac3 (AC-3 config), etc.
#[derive(Clone, Copy)]
pub struct AudioSampleEntryView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> AudioSampleEntryView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;

        if header.size != data.len() as u64 {
            return Err(ParseError::SizeMismatch {
                declared: header.size,
                actual: data.len(),
            });
        }

        if data.len() < AUDIO_HEADER_SIZE {
            return Err(ParseError::BufferTooShort {
                expected: AUDIO_HEADER_SIZE,
                found: data.len(),
            });
        }

        Ok(Self { data, header })
    }

    /// Creates a view from a RawBox.
    pub fn from_raw_box(raw: &RawBox<'a>) -> Result<Self, ParseError> {
        Self::new(raw.data())
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the sample rate as a u32 (integer part only).
    pub fn sample_rate_integer(&self) -> u32 {
        BigEndian::read_u16(&self.data[32..34]) as u32
    }

    /// Returns the payload data after the audio sample entry header (child boxes).
    pub fn children_data(&self) -> &'a [u8] {
        if self.data.len() > AUDIO_HEADER_SIZE {
            &self.data[AUDIO_HEADER_SIZE..]
        } else {
            &[]
        }
    }
}

impl SampleEntry for AudioSampleEntryView<'_> {
    fn box_size(&self) -> u64 {
        self.header.size
    }

    fn box_type(&self) -> BoxCode {
        self.header.box_type
    }

    fn data_reference_index(&self) -> u16 {
        BigEndian::read_u16(&self.data[14..16])
    }
}

impl AudioSampleEntry for AudioSampleEntryView<'_> {
    fn channel_count(&self) -> u16 {
        BigEndian::read_u16(&self.data[24..26])
    }

    fn sample_size(&self) -> u16 {
        BigEndian::read_u16(&self.data[26..28])
    }

    fn sample_rate(&self) -> UFixedPoint16_16 {
        UFixedPoint16_16::from_raw(BigEndian::read_u32(&self.data[32..36]))
    }

    fn children_data(&self) -> &[u8] {
        self.children_data()
    }
}

impl<'a> AudioSampleEntryView<'a> {
    /// Returns an iterator over child boxes.
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(self.children_data())
    }
}

impl std::fmt::Debug for AudioSampleEntryView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioSampleEntryView")
            .field("box_type", &String::from_utf8_lossy(&self.header.box_type.0 .0))
            .field("channel_count", &self.channel_count())
            .field("sample_size", &self.sample_size())
            .field("sample_rate", &self.sample_rate_integer())
            .finish()
    }
}

/// An owned representation of an audio sample entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioSampleEntryOwned {
    /// The codec identifier (e.g. mp4a, ac-3).
    pub box_type: BoxCode,
    /// Data reference index.
    pub data_reference_index: u16,
    /// Number of audio channels (template: 2).
    pub channel_count: u16,
    /// Sample size in bits (template: 16).
    pub sample_size: u16,
    /// Sample rate as unsigned fixed 16.16.
    pub sample_rate: UFixedPoint16_16,
    /// Raw child box data (e.g. esds, dac3).
    pub children_data: Vec<u8>,
}

impl AudioSampleEntryOwned {
    /// Creates a new AudioSampleEntryOwned with default template values.
    pub fn new(box_type: BoxCode) -> Self {
        Self {
            box_type,
            ..Default::default()
        }
    }

    /// Returns the sample rate as a u32 (integer part only).
    pub fn sample_rate_integer(&self) -> u32 {
        self.sample_rate.raw() >> 16
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (AUDIO_HEADER_SIZE - 8) as u64 + self.children_data.len() as u64;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, self.box_type)?;
        // reserved (6 bytes)
        writer.write_all(&[0u8; 6])?;
        // data_reference_index
        writer.write_u16::<BigEndian>(self.data_reference_index)?;
        // reserved (8 bytes)
        writer.write_all(&[0u8; 8])?;
        // channelcount
        writer.write_u16::<BigEndian>(self.channel_count)?;
        // samplesize
        writer.write_u16::<BigEndian>(self.sample_size)?;
        // pre_defined = 0
        writer.write_u16::<BigEndian>(0)?;
        // reserved = 0
        writer.write_u16::<BigEndian>(0)?;
        // samplerate
        writer.write_u32::<BigEndian>(self.sample_rate.raw())?;
        // children
        writer.write_all(&self.children_data)?;
        Ok(())
    }
}

impl Default for AudioSampleEntryOwned {
    fn default() -> Self {
        Self {
            box_type: BoxCode::new(*b"mp4a"),
            data_reference_index: 1,
            channel_count: 2,
            sample_size: 16,
            sample_rate: UFixedPoint16_16::ZERO,
            children_data: Vec::new(),
        }
    }
}

impl SampleEntry for AudioSampleEntryOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        self.box_type
    }

    fn data_reference_index(&self) -> u16 {
        self.data_reference_index
    }
}

impl AudioSampleEntry for AudioSampleEntryOwned {
    fn channel_count(&self) -> u16 {
        self.channel_count
    }

    fn sample_size(&self) -> u16 {
        self.sample_size
    }

    fn sample_rate(&self) -> UFixedPoint16_16 {
        self.sample_rate
    }

    fn children_data(&self) -> &[u8] {
        &self.children_data
    }
}

impl<T: AudioSampleEntry + SampleEntry> From<&T> for AudioSampleEntryOwned {
    fn from(source: &T) -> Self {
        Self {
            box_type: source.box_type(),
            data_reference_index: source.data_reference_index(),
            channel_count: source.channel_count(),
            sample_size: source.sample_size(),
            sample_rate: source.sample_rate(),
            children_data: source.children_data().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_visual_sample_entry(box_type: &[u8; 4]) -> Vec<u8> {
        let mut data = vec![0u8; VISUAL_HEADER_SIZE + 20]; // Room for a child box
        let size = data.len() as u32;
        data[0..4].copy_from_slice(&size.to_be_bytes());
        data[4..8].copy_from_slice(box_type);
        // reserved (6 bytes) at 8-13
        // data_reference_index at 14-15
        data[14..16].copy_from_slice(&1u16.to_be_bytes());
        // pre_defined at 16-17
        // reserved at 18-19
        // pre_defined at 20-31 (12 bytes)
        // width at 32-33
        data[32..34].copy_from_slice(&1920u16.to_be_bytes());
        // height at 34-35
        data[34..36].copy_from_slice(&1080u16.to_be_bytes());
        // horizresolution at 36-39 (72.0 = 0x00480000)
        data[36..40].copy_from_slice(&0x00480000u32.to_be_bytes());
        // vertresolution at 40-43
        data[40..44].copy_from_slice(&0x00480000u32.to_be_bytes());
        // reserved at 44-47
        // frame_count at 48-49
        data[48..50].copy_from_slice(&1u16.to_be_bytes());
        // compressor name at 50-81 (32 bytes, first byte is length)
        data[50] = 4;
        data[51..55].copy_from_slice(b"Test");
        // depth at 82-83
        data[82..84].copy_from_slice(&24u16.to_be_bytes());
        // pre_defined = -1 at 84-85
        data[84..86].copy_from_slice(&(-1i16).to_be_bytes());

        // Add a child box (avcC placeholder)
        let child_offset = VISUAL_HEADER_SIZE;
        let child_size = 20u32;
        data[child_offset..child_offset + 4].copy_from_slice(&child_size.to_be_bytes());
        data[child_offset + 4..child_offset + 8].copy_from_slice(b"avcC");

        data
    }

    fn make_audio_sample_entry(box_type: &[u8; 4]) -> Vec<u8> {
        let mut data = vec![0u8; AUDIO_HEADER_SIZE + 16]; // Room for a child box
        let size = data.len() as u32;
        data[0..4].copy_from_slice(&size.to_be_bytes());
        data[4..8].copy_from_slice(box_type);
        // reserved (6 bytes) at 8-13
        // data_reference_index at 14-15
        data[14..16].copy_from_slice(&1u16.to_be_bytes());
        // reserved at 16-23 (8 bytes)
        // channel_count at 24-25
        data[24..26].copy_from_slice(&2u16.to_be_bytes());
        // sample_size at 26-27
        data[26..28].copy_from_slice(&16u16.to_be_bytes());
        // pre_defined at 28-29
        // reserved at 30-31
        // sample_rate at 32-35 (44100 << 16)
        data[32..36].copy_from_slice(&((44100u32) << 16).to_be_bytes());

        // Add a child box (esds placeholder)
        let child_offset = AUDIO_HEADER_SIZE;
        let child_size = 16u32;
        data[child_offset..child_offset + 4].copy_from_slice(&child_size.to_be_bytes());
        data[child_offset + 4..child_offset + 8].copy_from_slice(b"esds");

        data
    }

    #[test]
    fn parse_visual_sample_entry() {
        let data = make_visual_sample_entry(b"avc1");
        let view = VisualSampleEntryView::new(&data).unwrap();

        assert_eq!(view.box_type(), BoxCode::new(*b"avc1"));
        assert_eq!(view.data_reference_index(), 1);
        assert_eq!(view.width(), 1920);
        assert_eq!(view.height(), 1080);
        assert_eq!(view.frame_count(), 1);
        assert_eq!(view.depth(), 24);
        assert_eq!(view.compressor_name(), "Test");
    }

    #[test]
    fn visual_sample_entry_children() {
        let data = make_visual_sample_entry(b"avc1");
        let view = VisualSampleEntryView::new(&data).unwrap();

        let children: Vec<_> = view.children().collect();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].box_type(), BoxCode::AVCC);
    }

    #[test]
    fn parse_audio_sample_entry() {
        let data = make_audio_sample_entry(b"mp4a");
        let view = AudioSampleEntryView::new(&data).unwrap();

        assert_eq!(view.box_type(), BoxCode::new(*b"mp4a"));
        assert_eq!(view.data_reference_index(), 1);
        assert_eq!(view.channel_count(), 2);
        assert_eq!(view.sample_size(), 16);
        assert_eq!(view.sample_rate_integer(), 44100);
    }

    #[test]
    fn audio_sample_entry_children() {
        let data = make_audio_sample_entry(b"mp4a");
        let view = AudioSampleEntryView::new(&data).unwrap();

        let children: Vec<_> = view.children().collect();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].box_type(), BoxCode::ESDS);
    }

    #[test]
    fn generic_sample_entry_detects_kind() {
        let visual_data = make_visual_sample_entry(b"avc1");
        let visual = SampleEntryView::new(&visual_data).unwrap();
        assert_eq!(visual.kind(), SampleEntryKind::Visual);

        let audio_data = make_audio_sample_entry(b"mp4a");
        let audio = SampleEntryView::new(&audio_data).unwrap();
        assert_eq!(audio.kind(), SampleEntryKind::Audio);
    }

    #[test]
    fn generic_sample_entry_children() {
        let data = make_visual_sample_entry(b"avc1");
        let view = SampleEntryView::new(&data).unwrap();

        let children: Vec<_> = view.children().collect();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].box_type(), BoxCode::AVCC);
    }

    #[test]
    fn visual_sample_entry_roundtrip() {
        let data = make_visual_sample_entry(b"avc1");
        let view = VisualSampleEntryView::new(&data).unwrap();
        let owned = VisualSampleEntryOwned::from(&view);

        assert_eq!(owned.box_type, BoxCode::new(*b"avc1"));
        assert_eq!(owned.data_reference_index, 1);
        assert_eq!(owned.width, 1920);
        assert_eq!(owned.height, 1080);
        assert_eq!(owned.horiz_resolution, UFixedPoint16_16::from_raw(0x00480000));
        assert_eq!(owned.vert_resolution, UFixedPoint16_16::from_raw(0x00480000));
        assert_eq!(owned.frame_count, 1);
        assert_eq!(owned.compressor_name, "Test");
        assert_eq!(owned.depth, 24);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn audio_sample_entry_roundtrip() {
        let data = make_audio_sample_entry(b"mp4a");
        let view = AudioSampleEntryView::new(&data).unwrap();
        let owned = AudioSampleEntryOwned::from(&view);

        assert_eq!(owned.box_type, BoxCode::new(*b"mp4a"));
        assert_eq!(owned.data_reference_index, 1);
        assert_eq!(owned.channel_count, 2);
        assert_eq!(owned.sample_size, 16);
        assert_eq!(owned.sample_rate_integer(), 44100);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn visual_sample_entry_owned_default() {
        let owned = VisualSampleEntryOwned::default();
        assert_eq!(owned.horiz_resolution, UFixedPoint16_16::from_raw(0x00480000));
        assert_eq!(owned.vert_resolution, UFixedPoint16_16::from_raw(0x00480000));
        assert_eq!(owned.frame_count, 1);
        assert_eq!(owned.depth, 24);
    }

    #[test]
    fn audio_sample_entry_owned_default() {
        let owned = AudioSampleEntryOwned::default();
        assert_eq!(owned.channel_count, 2);
        assert_eq!(owned.sample_size, 16);
    }

    #[test]
    fn visual_sample_entry_no_children_roundtrip() {
        let mut data = vec![0u8; VISUAL_HEADER_SIZE];
        let size = data.len() as u32;
        data[0..4].copy_from_slice(&size.to_be_bytes());
        data[4..8].copy_from_slice(b"hvc1");
        data[14..16].copy_from_slice(&1u16.to_be_bytes());
        data[32..34].copy_from_slice(&3840u16.to_be_bytes());
        data[34..36].copy_from_slice(&2160u16.to_be_bytes());
        data[36..40].copy_from_slice(&0x00480000u32.to_be_bytes());
        data[40..44].copy_from_slice(&0x00480000u32.to_be_bytes());
        data[48..50].copy_from_slice(&1u16.to_be_bytes());
        data[82..84].copy_from_slice(&24u16.to_be_bytes());
        data[84..86].copy_from_slice(&(-1i16).to_be_bytes());

        let view = VisualSampleEntryView::new(&data).unwrap();
        let owned = VisualSampleEntryOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }
}
