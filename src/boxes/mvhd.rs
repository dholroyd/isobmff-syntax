//! Movie Header Box (mvhd) parsing and serialization.
//!
//! The Movie Header Box contains overall information about the presentation,
//! including timescale, duration, and transformation matrix.
//!
//! ```text
//! aligned(8) class MovieHeaderBox
//!    extends FullBox('mvhd', version, 0) {
//!    if (version==1) {
//!       unsigned int(64) creation_time;
//!       unsigned int(64) modification_time;
//!       unsigned int(32) timescale;
//!       unsigned int(64) duration;
//!    } else { // version==0
//!       unsigned int(32) creation_time;
//!       unsigned int(32) modification_time;
//!       unsigned int(32) timescale;
//!       unsigned int(32) duration;
//!    }
//!    template int(32) rate = 0x00010000; // typically 1.0
//!    template int(16) volume = 0x0100; // typically, full volume
//!    const bit(16) reserved = 0;
//!    const unsigned int(32)[2] reserved = 0;
//!    template int(32)[9] matrix =
//!       { 0x00010000,0,0,0,0x00010000,0,0,0,0x40000000 };
//!       // Unity matrix
//!    bit(32)[6] pre_defined = 0;
//!    unsigned int(32) next_track_ID;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use crate::types::{FixedPoint16_16, FixedPoint8_8, Matrix};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MovieHeaderBox.
pub const BOX_TYPE: BoxCode = BoxCode::MVHD;

/// Common interface for accessing MovieHeaderBox data.
///
/// This trait is implemented by both [`MovieHeaderBoxView`] (borrowing) and
/// [`MovieHeaderBoxOwned`] (owning) representations.
pub trait MovieHeaderBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type (always `mvhd`).
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box (0 or 1).
    fn version(&self) -> u8;

    /// Returns the flags (24-bit value stored in lower 24 bits).
    fn flags(&self) -> u32;

    /// Returns the creation time as seconds since 1904-01-01 00:00:00 UTC.
    fn creation_time(&self) -> u64;

    /// Returns the modification time as seconds since 1904-01-01 00:00:00 UTC.
    fn modification_time(&self) -> u64;

    /// Returns the timescale (time units per second).
    fn timescale(&self) -> u32;

    /// Returns the duration in timescale units.
    fn duration(&self) -> u64;

    /// Returns the preferred playback rate as a 16.16 fixed-point number.
    fn rate(&self) -> FixedPoint16_16;

    /// Returns the preferred volume as an 8.8 fixed-point number.
    fn volume(&self) -> FixedPoint8_8;

    /// Returns the transformation matrix.
    fn matrix(&self) -> Matrix;

    /// Returns the next track ID to be used.
    fn next_track_id(&self) -> u32;
}

/// A borrowing view over raw MovieHeaderBox bytes.
///
/// This type wraps a byte slice containing a complete MovieHeaderBox and
/// provides accessor methods that decode fields on demand.
///
/// # Example
///
/// ```ignore
/// let view = MovieHeaderBoxView::new(bytes)?;
/// println!("Duration: {} / {}", view.duration(), view.timescale());
/// ```
#[derive(Clone, Copy)]
pub struct MovieHeaderBoxView<'a> {
    data: &'a [u8],
    /// Offset to the FullBox header (version/flags), after size/type/largesize
    fullbox_offset: usize,
}

impl<'a> MovieHeaderBoxView<'a> {
    /// Creates a new view over the given bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The buffer is too short
    /// - The box type is not `mvhd`
    /// - The version is not 0 or 1
    /// - The declared size doesn't match the buffer length
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let payload_size = if header.version == 0 { 96 } else { 108 };
        let fullbox_offset = header.validate(data, BOX_TYPE, Some(1), payload_size)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns true if this box uses extended (64-bit) size.
    #[inline]
    pub fn has_extended_size(&self) -> bool {
        self.fullbox_offset == 16
    }

    /// Helper to get the offset to the payload (after version/flags).
    #[inline]
    fn payload_offset(&self) -> usize {
        self.fullbox_offset + 4
    }

    /// Returns the 10-byte reserved region after the volume field.
    ///
    /// Per the spec this consists of `const bit(16) reserved = 0` followed by
    /// `const unsigned int(32)[2] reserved = 0`.
    pub fn reserved_after_volume(&self) -> &'a [u8] {
        let o = self.payload_offset();
        let offset = if self.version() == 1 { 34 } else { 22 };
        &self.data[o + offset..o + offset + 10]
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(self.data)
    }
}

impl MovieHeaderBox for MovieHeaderBoxView<'_> {
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
        let o = self.fullbox_offset + 1;
        BigEndian::read_u24(&self.data[o..o + 3])
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

    fn timescale(&self) -> u32 {
        let o = self.payload_offset();
        let offset = if self.version() == 1 { 16 } else { 8 };
        BigEndian::read_u32(&self.data[o + offset..o + offset + 4])
    }

    fn duration(&self) -> u64 {
        let o = self.payload_offset();
        if self.version() == 1 {
            BigEndian::read_u64(&self.data[o + 20..o + 28])
        } else {
            BigEndian::read_u32(&self.data[o + 12..o + 16]) as u64
        }
    }

    fn rate(&self) -> FixedPoint16_16 {
        let o = self.payload_offset();
        let offset = if self.version() == 1 { 28 } else { 16 };
        FixedPoint16_16::from_raw(BigEndian::read_i32(&self.data[o + offset..o + offset + 4]))
    }

    fn volume(&self) -> FixedPoint8_8 {
        let o = self.payload_offset();
        let offset = if self.version() == 1 { 32 } else { 20 };
        FixedPoint8_8::from_raw(BigEndian::read_i16(&self.data[o + offset..o + offset + 2]))
    }

    fn matrix(&self) -> Matrix {
        let o = self.payload_offset();
        let base_offset = if self.version() == 1 { 44 } else { 32 };
        let mut values = [0i32; 9];
        for (i, value) in values.iter_mut().enumerate() {
            let start = o + base_offset + i * 4;
            *value = BigEndian::read_i32(&self.data[start..start + 4]);
        }
        Matrix::from_raw(values)
    }

    fn next_track_id(&self) -> u32 {
        let o = self.payload_offset();
        let offset = if self.version() == 1 { 104 } else { 92 };
        BigEndian::read_u32(&self.data[o + offset..o + offset + 4])
    }
}

impl std::fmt::Debug for MovieHeaderBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MovieHeaderBoxView")
            .field("box_size", &self.box_size())
            .field("version", &self.version())
            .field("flags", &self.flags())
            .field("creation_time", &self.creation_time())
            .field("modification_time", &self.modification_time())
            .field("timescale", &self.timescale())
            .field("duration", &self.duration())
            .field("rate", &self.rate())
            .field("volume", &self.volume())
            .field("matrix", &self.matrix())
            .field("next_track_id", &self.next_track_id())
            .finish()
    }
}

/// An owned representation of MovieHeaderBox data.
///
/// This type stores the payload fields directly and calculates
/// box header fields (size, type, version) dynamically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovieHeaderBoxOwned {
    /// Flags (24-bit value, stored in lower 24 bits).
    pub flags: u32,
    /// Creation time as seconds since 1904-01-01 00:00:00 UTC.
    pub creation_time: u64,
    /// Modification time as seconds since 1904-01-01 00:00:00 UTC.
    pub modification_time: u64,
    /// Time units per second.
    pub timescale: u32,
    /// Duration in timescale units.
    pub duration: u64,
    /// Preferred playback rate (16.16 fixed-point).
    pub rate: FixedPoint16_16,
    /// Preferred volume (8.8 fixed-point).
    pub volume: FixedPoint8_8,
    /// Transformation matrix.
    pub matrix: Matrix,
    /// Next track ID to be used.
    pub next_track_id: u32,
}

impl MovieHeaderBoxOwned {
    /// Creates a new MovieHeaderBoxOwned with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the version required to represent this box's values.
    ///
    /// Returns 0 if all timestamp and duration values fit in 32 bits,
    /// otherwise returns 1.
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
        let payload_size = if version == 0 { 96u64 } else { 108u64 };
        fullbox_header_size_for_payload(payload_size) + payload_size
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let version = self.required_version();
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, version, self.flags)?;

        // Write payload
        if version == 1 {
            writer.write_u64::<BigEndian>(self.creation_time)?;
            writer.write_u64::<BigEndian>(self.modification_time)?;
            writer.write_u32::<BigEndian>(self.timescale)?;
            writer.write_u64::<BigEndian>(self.duration)?;
        } else {
            writer.write_u32::<BigEndian>(self.creation_time as u32)?;
            writer.write_u32::<BigEndian>(self.modification_time as u32)?;
            writer.write_u32::<BigEndian>(self.timescale)?;
            writer.write_u32::<BigEndian>(self.duration as u32)?;
        }

        writer.write_i32::<BigEndian>(self.rate.raw())?;
        writer.write_i16::<BigEndian>(self.volume.raw())?;

        // Reserved fields
        writer.write_all(&[0u8; 2])?;  // 16-bit reserved
        writer.write_all(&[0u8; 8])?;  // 2x 32-bit reserved

        // Matrix
        for &value in self.matrix.as_raw() {
            writer.write_i32::<BigEndian>(value)?;
        }

        // Pre-defined (6x 32-bit zeros)
        writer.write_all(&[0u8; 24])?;

        // Next track ID
        writer.write_u32::<BigEndian>(self.next_track_id)?;

        Ok(())
    }
}

impl Default for MovieHeaderBoxOwned {
    fn default() -> Self {
        Self {
            flags: 0,
            creation_time: 0,
            modification_time: 0,
            timescale: 1000,
            duration: 0,
            rate: FixedPoint16_16::ONE,
            volume: FixedPoint8_8::ONE,
            matrix: Matrix::IDENTITY,
            next_track_id: 1,
        }
    }
}

impl MovieHeaderBox for MovieHeaderBoxOwned {
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

    fn timescale(&self) -> u32 {
        self.timescale
    }

    fn duration(&self) -> u64 {
        self.duration
    }

    fn rate(&self) -> FixedPoint16_16 {
        self.rate
    }

    fn volume(&self) -> FixedPoint8_8 {
        self.volume
    }

    fn matrix(&self) -> Matrix {
        self.matrix
    }

    fn next_track_id(&self) -> u32 {
        self.next_track_id
    }
}

impl<T: MovieHeaderBox> From<&T> for MovieHeaderBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            creation_time: source.creation_time(),
            modification_time: source.modification_time(),
            timescale: source.timescale(),
            duration: source.duration(),
            rate: source.rate(),
            volume: source.volume(),
            matrix: source.matrix(),
            next_track_id: source.next_track_id(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_v0_box() -> Vec<u8> {
        let mut data = Vec::new();

        // Size (108 bytes total for v0)
        data.extend_from_slice(&108u32.to_be_bytes());
        // Type
        data.extend_from_slice(b"mvhd");
        // Version and flags
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags

        // Payload (v0)
        data.extend_from_slice(&1000u32.to_be_bytes()); // creation_time
        data.extend_from_slice(&2000u32.to_be_bytes()); // modification_time
        data.extend_from_slice(&48000u32.to_be_bytes()); // timescale
        data.extend_from_slice(&96000u32.to_be_bytes()); // duration (2 seconds)

        data.extend_from_slice(&0x00010000i32.to_be_bytes()); // rate = 1.0
        data.extend_from_slice(&0x0100i16.to_be_bytes()); // volume = 1.0

        data.extend_from_slice(&[0u8; 2]); // reserved
        data.extend_from_slice(&[0u8; 8]); // reserved

        // Identity matrix
        let matrix = [0x10000i32, 0, 0, 0, 0x10000, 0, 0, 0, 0x40000000];
        for m in matrix {
            data.extend_from_slice(&m.to_be_bytes());
        }

        data.extend_from_slice(&[0u8; 24]); // pre_defined
        data.extend_from_slice(&1u32.to_be_bytes()); // next_track_id

        data
    }

    fn make_v1_box() -> Vec<u8> {
        let mut data = Vec::new();

        // Size (120 bytes total for v1)
        data.extend_from_slice(&120u32.to_be_bytes());
        // Type
        data.extend_from_slice(b"mvhd");
        // Version and flags
        data.push(1); // version
        data.extend_from_slice(&[0, 0, 0]); // flags

        // Payload (v1)
        data.extend_from_slice(&0x100000000u64.to_be_bytes()); // creation_time (> u32::MAX)
        data.extend_from_slice(&0x100000001u64.to_be_bytes()); // modification_time
        data.extend_from_slice(&48000u32.to_be_bytes()); // timescale
        data.extend_from_slice(&0x200000000u64.to_be_bytes()); // duration

        data.extend_from_slice(&0x00010000i32.to_be_bytes()); // rate = 1.0
        data.extend_from_slice(&0x0100i16.to_be_bytes()); // volume = 1.0

        data.extend_from_slice(&[0u8; 2]); // reserved
        data.extend_from_slice(&[0u8; 8]); // reserved

        // Identity matrix
        let matrix = [0x10000i32, 0, 0, 0, 0x10000, 0, 0, 0, 0x40000000];
        for m in matrix {
            data.extend_from_slice(&m.to_be_bytes());
        }

        data.extend_from_slice(&[0u8; 24]); // pre_defined
        data.extend_from_slice(&42u32.to_be_bytes()); // next_track_id

        data
    }

    #[test]
    fn parse_v0_box() {
        let data = make_v0_box();
        let view = MovieHeaderBoxView::new(&data).unwrap();

        assert_eq!(view.box_size(), 108);
        assert_eq!(view.box_type(), BOX_TYPE);
        assert_eq!(view.version(), 0);
        assert_eq!(view.flags(), 0);
        assert_eq!(view.creation_time(), 1000);
        assert_eq!(view.modification_time(), 2000);
        assert_eq!(view.timescale(), 48000);
        assert_eq!(view.duration(), 96000);
        assert_eq!(view.rate(), FixedPoint16_16::ONE);
        assert_eq!(view.volume(), FixedPoint8_8::ONE);
        assert_eq!(view.matrix(), Matrix::IDENTITY);
        assert_eq!(view.next_track_id(), 1);
    }

    #[test]
    fn parse_v1_box() {
        let data = make_v1_box();
        let view = MovieHeaderBoxView::new(&data).unwrap();

        assert_eq!(view.box_size(), 120);
        assert_eq!(view.version(), 1);
        assert_eq!(view.creation_time(), 0x100000000);
        assert_eq!(view.modification_time(), 0x100000001);
        assert_eq!(view.timescale(), 48000);
        assert_eq!(view.duration(), 0x200000000);
        assert_eq!(view.next_track_id(), 42);
    }

    #[test]
    fn invalid_box_type() {
        let mut data = make_v0_box();
        data[4..8].copy_from_slice(b"trak");

        let result = MovieHeaderBoxView::new(&data);
        assert!(matches!(result, Err(ParseError::InvalidBoxType { .. })));
    }

    #[test]
    fn invalid_version() {
        let mut data = make_v0_box();
        data[8] = 2; // invalid version

        let result = MovieHeaderBoxView::new(&data);
        assert!(matches!(result, Err(ParseError::InvalidVersion(2))));
    }

    #[test]
    fn buffer_too_short() {
        // Buffer too short to read basic header
        let result = MovieHeaderBoxView::new(&[0u8; 4]);
        assert!(matches!(result, Err(ParseError::BufferTooShort { .. })));
    }

    #[test]
    fn size_mismatch() {
        let mut data = make_v0_box();
        // Change declared size to 200
        data[0..4].copy_from_slice(&200u32.to_be_bytes());

        let result = MovieHeaderBoxView::new(&data);
        assert!(matches!(result, Err(ParseError::SizeMismatch { .. })));
    }

    #[test]
    fn owned_default() {
        let owned = MovieHeaderBoxOwned::new();
        assert_eq!(owned.required_version(), 0);
        assert_eq!(owned.timescale, 1000);
        assert_eq!(owned.rate, FixedPoint16_16::ONE);
    }

    #[test]
    fn owned_from_view() {
        let data = make_v0_box();
        let view = MovieHeaderBoxView::new(&data).unwrap();
        let owned = MovieHeaderBoxOwned::from(&view);

        assert_eq!(owned.creation_time, view.creation_time());
        assert_eq!(owned.modification_time, view.modification_time());
        assert_eq!(owned.timescale, view.timescale());
        assert_eq!(owned.duration, view.duration());
    }

    #[test]
    fn owned_requires_v1() {
        let mut owned = MovieHeaderBoxOwned::new();
        assert_eq!(owned.required_version(), 0);

        owned.creation_time = 0x100000000;
        assert_eq!(owned.required_version(), 1);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_v0_box();
        let view = MovieHeaderBoxView::new(&data).unwrap();
        let owned = MovieHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_v1() {
        let data = make_v1_box();
        let view = MovieHeaderBoxView::new(&data).unwrap();
        let owned = MovieHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn trait_generic() {
        fn print_duration(header: &impl MovieHeaderBox) -> f64 {
            header.duration() as f64 / header.timescale() as f64
        }

        let data = make_v0_box();
        let view = MovieHeaderBoxView::new(&data).unwrap();
        let owned = MovieHeaderBoxOwned::from(&view);

        let view_duration = print_duration(&view);
        let owned_duration = print_duration(&owned);

        assert!((view_duration - 2.0).abs() < 0.001);
        assert!((owned_duration - 2.0).abs() < 0.001);
    }
}
