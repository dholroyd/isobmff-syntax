//! Track Header Box (tkhd) parsing and serialization.
//!
//! The Track Header Box specifies the characteristics of a single track.
//!
//! ```text
//! aligned(8) class TrackHeaderBox
//!    extends FullBox('tkhd', version, flags) {
//!    if (version==1) {
//!       unsigned int(64) creation_time;
//!       unsigned int(64) modification_time;
//!       unsigned int(32) track_ID;
//!       const unsigned int(32) reserved = 0;
//!       unsigned int(64) duration;
//!    } else { // version==0
//!       unsigned int(32) creation_time;
//!       unsigned int(32) modification_time;
//!       unsigned int(32) track_ID;
//!       const unsigned int(32) reserved = 0;
//!       unsigned int(32) duration;
//!    }
//!    const unsigned int(32)[2] reserved = 0;
//!    template int(16) layer = 0;
//!    template int(16) alternate_group = 0;
//!    template int(16) volume = {if track_is_audio 0x0100 else 0};
//!    const unsigned int(16) reserved = 0;
//!    template int(32)[9] matrix=
//!       { 0x00010000,0,0,0,0x00010000,0,0,0,0x40000000 };
//!       // unity matrix
//!    unsigned int(32) width;
//!    unsigned int(32) height;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use crate::types::{FixedPoint8_8, Matrix, UFixedPoint16_16};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TrackHeaderBox.
pub const BOX_TYPE: BoxCode = BoxCode::TKHD;

/// Track header flags.
pub mod flags {
    /// Track is enabled.
    pub const TRACK_ENABLED: u32 = 0x000001;
    /// Track is used in the presentation.
    pub const TRACK_IN_MOVIE: u32 = 0x000002;
    /// Track is used in preview.
    pub const TRACK_IN_PREVIEW: u32 = 0x000004;
    /// Track size is in aspect ratio.
    pub const TRACK_SIZE_IS_ASPECT_RATIO: u32 = 0x000008;
}

/// Common interface for accessing TrackHeaderBox data.
pub trait TrackHeaderBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box (0 or 1).
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the creation time as seconds since 1904-01-01.
    fn creation_time(&self) -> u64;

    /// Returns the modification time as seconds since 1904-01-01.
    fn modification_time(&self) -> u64;

    /// Returns the track ID.
    fn track_id(&self) -> u32;

    /// Returns the duration in movie timescale units.
    fn duration(&self) -> u64;

    /// Returns the front-to-back ordering of video tracks.
    fn layer(&self) -> i16;

    /// Returns the group or collection of tracks.
    fn alternate_group(&self) -> i16;

    /// Returns the track's relative audio volume.
    fn volume(&self) -> FixedPoint8_8;

    /// Returns the transformation matrix.
    fn matrix(&self) -> Matrix;

    /// Returns the track's visual presentation width.
    fn width(&self) -> UFixedPoint16_16;

    /// Returns the track's visual presentation height.
    fn height(&self) -> UFixedPoint16_16;

    /// Returns true if the track is enabled.
    fn is_enabled(&self) -> bool {
        self.flags() & flags::TRACK_ENABLED != 0
    }

    /// Returns true if the track is in movie.
    fn is_in_movie(&self) -> bool {
        self.flags() & flags::TRACK_IN_MOVIE != 0
    }

    /// Returns true if the track is in preview.
    fn is_in_preview(&self) -> bool {
        self.flags() & flags::TRACK_IN_PREVIEW != 0
    }
}

/// A borrowing view over raw TrackHeaderBox bytes.
#[derive(Clone, Copy)]
pub struct TrackHeaderBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> TrackHeaderBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let payload_size = if header.version == 0 { 80 } else { 92 };
        let fullbox_offset = header.validate(data, BOX_TYPE, Some(1), payload_size)?;
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

    /// Returns the 4-byte reserved field after track_id.
    ///
    /// Per the spec this field is `const unsigned int(32) reserved = 0`.
    pub fn reserved_after_track_id(&self) -> &'a [u8] {
        let o = self.payload_offset();
        let offset = if self.version() == 1 { 20 } else { 12 };
        &self.data[o + offset..o + offset + 4]
    }

    /// Returns the 8-byte reserved field after duration (two `const unsigned int(32) reserved = 0`).
    pub fn reserved_after_duration(&self) -> &'a [u8] {
        let o = self.payload_offset();
        let offset = if self.version() == 1 { 32 } else { 20 };
        &self.data[o + offset..o + offset + 8]
    }

    /// Returns the 2-byte reserved field after volume (`const unsigned int(16) reserved = 0`).
    pub fn reserved_after_volume(&self) -> &'a [u8] {
        let o = self.payload_offset();
        let offset = if self.version() == 1 { 46 } else { 34 };
        &self.data[o + offset..o + offset + 2]
    }
}

impl TrackHeaderBox for TrackHeaderBoxView<'_> {
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

    fn creation_time(&self) -> u64 {
        let o = self.payload_offset();
        if self.version() == 1 {
            BigEndian::read_u64(&self.data[o..o + 8])
        } else {
            BigEndian::read_u32(&self.data[o..o + 4]) as u64
        }
    }

    fn modification_time(&self) -> u64 {
        let o = self.payload_offset();
        if self.version() == 1 {
            BigEndian::read_u64(&self.data[o + 8..o + 16])
        } else {
            BigEndian::read_u32(&self.data[o + 4..o + 8]) as u64
        }
    }

    fn track_id(&self) -> u32 {
        let o = self.payload_offset();
        let offset = if self.version() == 1 { 16 } else { 8 };
        BigEndian::read_u32(&self.data[o + offset..o + offset + 4])
    }

    fn duration(&self) -> u64 {
        let o = self.payload_offset();
        if self.version() == 1 {
            // After creation(8)+modification(8)+track_id(4)+reserved(4)
            BigEndian::read_u64(&self.data[o + 24..o + 32])
        } else {
            // After creation(4)+modification(4)+track_id(4)+reserved(4)
            BigEndian::read_u32(&self.data[o + 16..o + 20]) as u64
        }
    }

    fn layer(&self) -> i16 {
        let o = self.payload_offset();
        let base = if self.version() == 1 { 40 } else { 28 };
        BigEndian::read_i16(&self.data[o + base..o + base + 2])
    }

    fn alternate_group(&self) -> i16 {
        let o = self.payload_offset();
        let base = if self.version() == 1 { 42 } else { 30 };
        BigEndian::read_i16(&self.data[o + base..o + base + 2])
    }

    fn volume(&self) -> FixedPoint8_8 {
        let o = self.payload_offset();
        let base = if self.version() == 1 { 44 } else { 32 };
        FixedPoint8_8::from_raw(BigEndian::read_i16(&self.data[o + base..o + base + 2]))
    }

    fn matrix(&self) -> Matrix {
        let o = self.payload_offset();
        let base = if self.version() == 1 { 48 } else { 36 };
        let mut values = [0i32; 9];
        for (i, value) in values.iter_mut().enumerate() {
            let start = o + base + i * 4;
            *value = BigEndian::read_i32(&self.data[start..start + 4]);
        }
        Matrix::from_raw(values)
    }

    fn width(&self) -> UFixedPoint16_16 {
        let o = self.payload_offset();
        let base = if self.version() == 1 { 84 } else { 72 };
        UFixedPoint16_16::from_raw(BigEndian::read_u32(&self.data[o + base..o + base + 4]))
    }

    fn height(&self) -> UFixedPoint16_16 {
        let o = self.payload_offset();
        let base = if self.version() == 1 { 88 } else { 76 };
        UFixedPoint16_16::from_raw(BigEndian::read_u32(&self.data[o + base..o + base + 4]))
    }
}

impl std::fmt::Debug for TrackHeaderBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackHeaderBoxView")
            .field("box_size", &self.box_size())
            .field("version", &self.version())
            .field("flags", &format!("0x{:06X}", self.flags()))
            .field("track_id", &self.track_id())
            .field("duration", &self.duration())
            .field("layer", &self.layer())
            .field("alternate_group", &self.alternate_group())
            .field("volume", &self.volume())
            .field("width", &self.width())
            .field("height", &self.height())
            .finish()
    }
}

/// An owned representation of TrackHeaderBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackHeaderBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Creation time.
    pub creation_time: u64,
    /// Modification time.
    pub modification_time: u64,
    /// Track ID.
    pub track_id: u32,
    /// Duration in movie timescale.
    pub duration: u64,
    /// Layer (front-to-back ordering).
    pub layer: i16,
    /// Alternate group.
    pub alternate_group: i16,
    /// Volume (for audio tracks).
    pub volume: FixedPoint8_8,
    /// Transformation matrix.
    pub matrix: Matrix,
    /// Track visual width.
    pub width: UFixedPoint16_16,
    /// Track visual height.
    pub height: UFixedPoint16_16,
}

impl TrackHeaderBoxOwned {
    /// Creates a new TrackHeaderBoxOwned with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the version required to represent this box's values.
    pub fn required_version(&self) -> u8 {
        if self.creation_time > u32::MAX as u64
            || self.modification_time > u32::MAX as u64
            || self.duration > u32::MAX as u64
        {
            1
        } else {
            0
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
let version = self.required_version();
        let payload = if version == 0 { 80u64 } else { 92u64 };
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let version = self.required_version();
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, version, self.flags)?;

        // Payload
        if version == 1 {
            writer.write_u64::<BigEndian>(self.creation_time)?;
            writer.write_u64::<BigEndian>(self.modification_time)?;
            writer.write_u32::<BigEndian>(self.track_id)?;
            writer.write_u32::<BigEndian>(0)?; // reserved
            writer.write_u64::<BigEndian>(self.duration)?;
        } else {
            writer.write_u32::<BigEndian>(self.creation_time as u32)?;
            writer.write_u32::<BigEndian>(self.modification_time as u32)?;
            writer.write_u32::<BigEndian>(self.track_id)?;
            writer.write_u32::<BigEndian>(0)?; // reserved
            writer.write_u32::<BigEndian>(self.duration as u32)?;
        }

        // Reserved (2x32-bit)
        writer.write_all(&[0u8; 8])?;

        writer.write_i16::<BigEndian>(self.layer)?;
        writer.write_i16::<BigEndian>(self.alternate_group)?;
        writer.write_i16::<BigEndian>(self.volume.raw())?;
        writer.write_u16::<BigEndian>(0)?; // reserved

        // Matrix
        for &value in self.matrix.as_raw() {
            writer.write_i32::<BigEndian>(value)?;
        }

        writer.write_u32::<BigEndian>(self.width.raw())?;
        writer.write_u32::<BigEndian>(self.height.raw())?;

        Ok(())
    }
}

impl Default for TrackHeaderBoxOwned {
    fn default() -> Self {
        Self {
            flags: flags::TRACK_ENABLED | flags::TRACK_IN_MOVIE | flags::TRACK_IN_PREVIEW,
            creation_time: 0,
            modification_time: 0,
            track_id: 1,
            duration: 0,
            layer: 0,
            alternate_group: 0,
            volume: FixedPoint8_8::ZERO, // 0 for video, 1.0 for audio
            matrix: Matrix::IDENTITY,
            width: UFixedPoint16_16::ZERO,
            height: UFixedPoint16_16::ZERO,
        }
    }
}

impl TrackHeaderBox for TrackHeaderBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.required_version()
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn creation_time(&self) -> u64 {
        self.creation_time
    }

    fn modification_time(&self) -> u64 {
        self.modification_time
    }

    fn track_id(&self) -> u32 {
        self.track_id
    }

    fn duration(&self) -> u64 {
        self.duration
    }

    fn layer(&self) -> i16 {
        self.layer
    }

    fn alternate_group(&self) -> i16 {
        self.alternate_group
    }

    fn volume(&self) -> FixedPoint8_8 {
        self.volume
    }

    fn matrix(&self) -> Matrix {
        self.matrix
    }

    fn width(&self) -> UFixedPoint16_16 {
        self.width
    }

    fn height(&self) -> UFixedPoint16_16 {
        self.height
    }
}

impl<T: TrackHeaderBox> From<&T> for TrackHeaderBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            creation_time: source.creation_time(),
            modification_time: source.modification_time(),
            track_id: source.track_id(),
            duration: source.duration(),
            layer: source.layer(),
            alternate_group: source.alternate_group(),
            volume: source.volume(),
            matrix: source.matrix(),
            width: source.width(),
            height: source.height(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_v0_tkhd() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&92u32.to_be_bytes()); // size
        data.extend_from_slice(b"tkhd");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0x07]); // flags (enabled|in_movie|in_preview)

        data.extend_from_slice(&1000u32.to_be_bytes()); // creation_time
        data.extend_from_slice(&2000u32.to_be_bytes()); // modification_time
        data.extend_from_slice(&1u32.to_be_bytes()); // track_id
        data.extend_from_slice(&0u32.to_be_bytes()); // reserved
        data.extend_from_slice(&48000u32.to_be_bytes()); // duration

        data.extend_from_slice(&[0u8; 8]); // reserved
        data.extend_from_slice(&0i16.to_be_bytes()); // layer
        data.extend_from_slice(&0i16.to_be_bytes()); // alternate_group
        data.extend_from_slice(&0x0100i16.to_be_bytes()); // volume = 1.0
        data.extend_from_slice(&0u16.to_be_bytes()); // reserved

        // Identity matrix
        let matrix = [0x10000i32, 0, 0, 0, 0x10000, 0, 0, 0, 0x40000000];
        for m in matrix {
            data.extend_from_slice(&m.to_be_bytes());
        }

        data.extend_from_slice(&(1920u32 << 16).to_be_bytes()); // width = 1920.0
        data.extend_from_slice(&(1080u32 << 16).to_be_bytes()); // height = 1080.0

        data
    }

    #[test]
    fn parse_v0() {
        let data = make_v0_tkhd();
        let view = TrackHeaderBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.flags(), 0x07);
        assert_eq!(view.track_id(), 1);
        assert_eq!(view.duration(), 48000);
        assert_eq!(view.volume(), FixedPoint8_8::ONE);
        assert_eq!(view.width().as_f32() as u32, 1920);
        assert_eq!(view.height().as_f32() as u32, 1080);
        assert!(view.is_enabled());
        assert!(view.is_in_movie());
    }

    #[test]
    fn roundtrip() {
        let data = make_v0_tkhd();
        let view = TrackHeaderBoxView::new(&data).unwrap();
        let owned = TrackHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
