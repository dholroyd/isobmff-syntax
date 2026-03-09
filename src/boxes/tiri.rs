//! Tier Information Box (tiri) parsing and serialization.
//!
//! The Tier Information Box provides tier information for SVC scalable groups.
//!
//! ```text
//! aligned(8) class TierInfoBox extends Box('tiri') {
//!    unsigned int(16) tierID;
//!    unsigned int(8) profileIndication;
//!    unsigned int(8) profileCompatibility;
//!    unsigned int(8) levelIndication;
//!    unsigned int(16) visualWidth;
//!    unsigned int(16) visualHeight;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TierInfoBox.
pub const BOX_TYPE: BoxCode = BoxCode::TIRI;

const PAYLOAD_SIZE: u64 = 9; // 2+1+1+1+2+2

/// Common interface for accessing TierInfoBox data.
pub trait TierInfoBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;
    /// Returns the box type.
    fn box_type(&self) -> BoxCode;
    /// Returns the tier ID.
    fn tier_id(&self) -> u16;
    /// Returns the profile indication.
    fn profile_indication(&self) -> u8;
    /// Returns the profile compatibility.
    fn profile_compatibility(&self) -> u8;
    /// Returns the level indication.
    fn level_indication(&self) -> u8;
    /// Returns the visual width.
    fn visual_width(&self) -> u16;
    /// Returns the visual height.
    fn visual_height(&self) -> u16;
}

/// A borrowing view over raw TierInfoBox bytes.
#[derive(Clone, Copy)]
pub struct TierInfoBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> TierInfoBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, PAYLOAD_SIZE as usize)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl TierInfoBox for TierInfoBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn tier_id(&self) -> u16 {
        let o = self.header_size;
        BigEndian::read_u16(&self.data[o..o + 2])
    }

    fn profile_indication(&self) -> u8 {
        self.data[self.header_size + 2]
    }

    fn profile_compatibility(&self) -> u8 {
        self.data[self.header_size + 3]
    }

    fn level_indication(&self) -> u8 {
        self.data[self.header_size + 4]
    }

    fn visual_width(&self) -> u16 {
        let o = self.header_size + 5;
        BigEndian::read_u16(&self.data[o..o + 2])
    }

    fn visual_height(&self) -> u16 {
        let o = self.header_size + 7;
        BigEndian::read_u16(&self.data[o..o + 2])
    }
}

impl std::fmt::Debug for TierInfoBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TierInfoBoxView")
            .field("tier_id", &self.tier_id())
            .field("visual_width", &self.visual_width())
            .field("visual_height", &self.visual_height())
            .finish()
    }
}

/// An owned representation of TierInfoBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TierInfoBoxOwned {
    /// Tier ID.
    pub tier_id: u16,
    /// Profile indication.
    pub profile_indication: u8,
    /// Profile compatibility.
    pub profile_compatibility: u8,
    /// Level indication.
    pub level_indication: u8,
    /// Visual width.
    pub visual_width: u16,
    /// Visual height.
    pub visual_height: u16,
}

impl TierInfoBoxOwned {
    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(PAYLOAD_SIZE) + PAYLOAD_SIZE
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        writer.write_u16::<BigEndian>(self.tier_id)?;
        writer.write_u8(self.profile_indication)?;
        writer.write_u8(self.profile_compatibility)?;
        writer.write_u8(self.level_indication)?;
        writer.write_u16::<BigEndian>(self.visual_width)?;
        writer.write_u16::<BigEndian>(self.visual_height)?;
        Ok(())
    }
}

impl TierInfoBox for TierInfoBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn tier_id(&self) -> u16 {
        self.tier_id
    }

    fn profile_indication(&self) -> u8 {
        self.profile_indication
    }

    fn profile_compatibility(&self) -> u8 {
        self.profile_compatibility
    }

    fn level_indication(&self) -> u8 {
        self.level_indication
    }

    fn visual_width(&self) -> u16 {
        self.visual_width
    }

    fn visual_height(&self) -> u16 {
        self.visual_height
    }
}

impl<T: TierInfoBox> From<&T> for TierInfoBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            tier_id: source.tier_id(),
            profile_indication: source.profile_indication(),
            profile_compatibility: source.profile_compatibility(),
            level_indication: source.level_indication(),
            visual_width: source.visual_width(),
            visual_height: source.visual_height(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tiri() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&17u32.to_be_bytes()); // size = 8 + 9
        data.extend_from_slice(b"tiri");
        data.extend_from_slice(&100u16.to_be_bytes()); // tier_id
        data.push(66); // profile_indication
        data.push(0xC0); // profile_compatibility
        data.push(31); // level_indication
        data.extend_from_slice(&1920u16.to_be_bytes()); // visual_width
        data.extend_from_slice(&1080u16.to_be_bytes()); // visual_height
        data
    }

    #[test]
    fn parse_tiri() {
        let data = make_tiri();
        let view = TierInfoBoxView::new(&data).unwrap();
        assert_eq!(view.tier_id(), 100);
        assert_eq!(view.profile_indication(), 66);
        assert_eq!(view.profile_compatibility(), 0xC0);
        assert_eq!(view.level_indication(), 31);
        assert_eq!(view.visual_width(), 1920);
        assert_eq!(view.visual_height(), 1080);
    }

    #[test]
    fn roundtrip() {
        let data = make_tiri();
        let view = TierInfoBoxView::new(&data).unwrap();
        let owned = TierInfoBoxOwned::from(&view);
        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }
}
