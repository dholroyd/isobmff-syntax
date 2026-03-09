//! Composition to Decode Timeline Mapping Box (cslg) parsing and serialization.
//!
//! The Composition to Decode Timeline Mapping Box provides the offset
//! between composition time and decode time.
//!
//! ```text
//! aligned(8) class CompositionToDecodeBox
//!    extends FullBox('cslg', version, 0) {
//!    if (version == 0) {
//!       signed int(32) compositionToDTSShift;
//!       signed int(32) leastDecodeToDisplayDelta;
//!       signed int(32) greatestDecodeToDisplayDelta;
//!       signed int(32) compositionStartTime;
//!       signed int(32) compositionEndTime;
//!    } else {
//!       signed int(64) compositionToDTSShift;
//!       signed int(64) leastDecodeToDisplayDelta;
//!       signed int(64) greatestDecodeToDisplayDelta;
//!       signed int(64) compositionStartTime;
//!       signed int(64) compositionEndTime;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for CompositionToDecodeBox.
pub const BOX_TYPE: BoxCode = BoxCode::CSLG;

/// Common interface for accessing CompositionToDecodeBox data.
pub trait CompositionToDecodeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the composition to DTS shift.
    fn composition_to_dts_shift(&self) -> i64;

    /// Returns the least decode to display delta.
    fn least_decode_to_display_delta(&self) -> i64;

    /// Returns the greatest decode to display delta.
    fn greatest_decode_to_display_delta(&self) -> i64;

    /// Returns the composition start time.
    fn composition_start_time(&self) -> i64;

    /// Returns the composition end time.
    fn composition_end_time(&self) -> i64;
}

/// A borrowing view over raw CompositionToDecodeBox bytes.
#[derive(Clone, Copy)]
pub struct CompositionToDecodeBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
}

impl<'a> CompositionToDecodeBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let payload_size = if header.version == 1 { 40 } else { 20 };
        let fullbox_offset = header.validate(data, BOX_TYPE, None, payload_size)?;
        let version = header.version;
        Ok(Self { data, fullbox_offset, version })
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
}

impl CompositionToDecodeBox for CompositionToDecodeBoxView<'_> {
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

    fn composition_to_dts_shift(&self) -> i64 {
        let o = self.payload_offset();
        if self.version == 1 {
            BigEndian::read_i64(&self.data[o..o + 8])
        } else {
            BigEndian::read_i32(&self.data[o..o + 4]) as i64
        }
    }

    fn least_decode_to_display_delta(&self) -> i64 {
        let o = self.payload_offset() + if self.version == 1 { 8 } else { 4 };
        if self.version == 1 {
            BigEndian::read_i64(&self.data[o..o + 8])
        } else {
            BigEndian::read_i32(&self.data[o..o + 4]) as i64
        }
    }

    fn greatest_decode_to_display_delta(&self) -> i64 {
        let o = self.payload_offset() + if self.version == 1 { 16 } else { 8 };
        if self.version == 1 {
            BigEndian::read_i64(&self.data[o..o + 8])
        } else {
            BigEndian::read_i32(&self.data[o..o + 4]) as i64
        }
    }

    fn composition_start_time(&self) -> i64 {
        let o = self.payload_offset() + if self.version == 1 { 24 } else { 12 };
        if self.version == 1 {
            BigEndian::read_i64(&self.data[o..o + 8])
        } else {
            BigEndian::read_i32(&self.data[o..o + 4]) as i64
        }
    }

    fn composition_end_time(&self) -> i64 {
        let o = self.payload_offset() + if self.version == 1 { 32 } else { 16 };
        if self.version == 1 {
            BigEndian::read_i64(&self.data[o..o + 8])
        } else {
            BigEndian::read_i32(&self.data[o..o + 4]) as i64
        }
    }
}

impl std::fmt::Debug for CompositionToDecodeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositionToDecodeBoxView")
            .field("version", &self.version())
            .field("composition_to_dts_shift", &self.composition_to_dts_shift())
            .finish()
    }
}

/// An owned representation of CompositionToDecodeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct CompositionToDecodeBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Composition to DTS shift.
    pub composition_to_dts_shift: i64,
    /// Least decode to display delta.
    pub least_decode_to_display_delta: i64,
    /// Greatest decode to display delta.
    pub greatest_decode_to_display_delta: i64,
    /// Composition start time.
    pub composition_start_time: i64,
    /// Composition end time.
    pub composition_end_time: i64,
}

impl CompositionToDecodeBoxOwned {
    /// Creates a new CompositionToDecodeBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether version 1 is required.
    fn requires_v1(&self) -> bool {
        self.composition_to_dts_shift > i32::MAX as i64
            || self.composition_to_dts_shift < i32::MIN as i64
            || self.least_decode_to_display_delta > i32::MAX as i64
            || self.least_decode_to_display_delta < i32::MIN as i64
            || self.greatest_decode_to_display_delta > i32::MAX as i64
            || self.greatest_decode_to_display_delta < i32::MIN as i64
            || self.composition_start_time > i32::MAX as i64
            || self.composition_start_time < i32::MIN as i64
            || self.composition_end_time > i32::MAX as i64
            || self.composition_end_time < i32::MIN as i64
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = if self.requires_v1() { 40u64 } else { 20u64 };
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let v1 = self.requires_v1();
        let version = if v1 { 1u8 } else { 0u8 };

        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, version, self.flags)?;

        if v1 {
            writer.write_i64::<BigEndian>(self.composition_to_dts_shift)?;
            writer.write_i64::<BigEndian>(self.least_decode_to_display_delta)?;
            writer.write_i64::<BigEndian>(self.greatest_decode_to_display_delta)?;
            writer.write_i64::<BigEndian>(self.composition_start_time)?;
            writer.write_i64::<BigEndian>(self.composition_end_time)?;
        } else {
            writer.write_i32::<BigEndian>(self.composition_to_dts_shift as i32)?;
            writer.write_i32::<BigEndian>(self.least_decode_to_display_delta as i32)?;
            writer.write_i32::<BigEndian>(self.greatest_decode_to_display_delta as i32)?;
            writer.write_i32::<BigEndian>(self.composition_start_time as i32)?;
            writer.write_i32::<BigEndian>(self.composition_end_time as i32)?;
        }

        Ok(())
    }
}


impl CompositionToDecodeBox for CompositionToDecodeBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        if self.requires_v1() { 1 } else { 0 }
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn composition_to_dts_shift(&self) -> i64 {
        self.composition_to_dts_shift
    }

    fn least_decode_to_display_delta(&self) -> i64 {
        self.least_decode_to_display_delta
    }

    fn greatest_decode_to_display_delta(&self) -> i64 {
        self.greatest_decode_to_display_delta
    }

    fn composition_start_time(&self) -> i64 {
        self.composition_start_time
    }

    fn composition_end_time(&self) -> i64 {
        self.composition_end_time
    }
}

impl<T: CompositionToDecodeBox> From<&T> for CompositionToDecodeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            composition_to_dts_shift: source.composition_to_dts_shift(),
            least_decode_to_display_delta: source.least_decode_to_display_delta(),
            greatest_decode_to_display_delta: source.greatest_decode_to_display_delta(),
            composition_start_time: source.composition_start_time(),
            composition_end_time: source.composition_end_time(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cslg_v0() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&32u32.to_be_bytes()); // size
        data.extend_from_slice(b"cslg");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&100i32.to_be_bytes()); // composition_to_dts_shift
        data.extend_from_slice(&(-50i32).to_be_bytes()); // least_decode_to_display_delta
        data.extend_from_slice(&200i32.to_be_bytes()); // greatest_decode_to_display_delta
        data.extend_from_slice(&0i32.to_be_bytes()); // composition_start_time
        data.extend_from_slice(&1000i32.to_be_bytes()); // composition_end_time
        data
    }

    #[test]
    fn parse_cslg_v0() {
        let data = make_cslg_v0();
        let view = CompositionToDecodeBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.composition_to_dts_shift(), 100);
        assert_eq!(view.least_decode_to_display_delta(), -50);
        assert_eq!(view.greatest_decode_to_display_delta(), 200);
        assert_eq!(view.composition_start_time(), 0);
        assert_eq!(view.composition_end_time(), 1000);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_cslg_v0();
        let view = CompositionToDecodeBoxView::new(&data).unwrap();
        let owned = CompositionToDecodeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
