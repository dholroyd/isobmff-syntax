//! Priority Range Box (svpr) parsing and serialization.
//!
//! The Priority Range Box specifies the range of priority IDs
//! for SVC scalable and MVC multiview groups.
//!
//! ```text
//! aligned(8) class PriorityRangeBox extends Box('svpr') {
//!    unsigned int(6) minPriorityId;
//!    unsigned int(2) reserved = 0;
//!    unsigned int(6) maxPriorityId;
//!    unsigned int(2) reserved = 0;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for PriorityRangeBox.
pub const BOX_TYPE: BoxCode = BoxCode::SVPR;

const PAYLOAD_SIZE: u64 = 2;

/// Common interface for accessing PriorityRangeBox data.
pub trait PriorityRangeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;
    /// Returns the box type.
    fn box_type(&self) -> BoxCode;
    /// Returns the minimum priority ID (6 bits).
    fn min_priority_id(&self) -> u8;
    /// Returns the maximum priority ID (6 bits).
    fn max_priority_id(&self) -> u8;
}

/// A borrowing view over raw PriorityRangeBox bytes.
#[derive(Clone, Copy)]
pub struct PriorityRangeBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> PriorityRangeBoxView<'a> {
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

impl PriorityRangeBox for PriorityRangeBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn min_priority_id(&self) -> u8 {
        (self.data[self.header_size] >> 2) & 0x3F
    }

    fn max_priority_id(&self) -> u8 {
        (self.data[self.header_size + 1] >> 2) & 0x3F
    }
}

impl std::fmt::Debug for PriorityRangeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PriorityRangeBoxView")
            .field("min_priority_id", &self.min_priority_id())
            .field("max_priority_id", &self.max_priority_id())
            .finish()
    }
}

/// An owned representation of PriorityRangeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriorityRangeBoxOwned {
    /// Minimum priority ID (6 bits).
    pub min_priority_id: u8,
    /// Maximum priority ID (6 bits).
    pub max_priority_id: u8,
}

impl PriorityRangeBoxOwned {
    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(PAYLOAD_SIZE) + PAYLOAD_SIZE
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        let b0 = (self.min_priority_id & 0x3F) << 2;
        let b1 = (self.max_priority_id & 0x3F) << 2;
        writer.write_all(&[b0, b1])?;
        Ok(())
    }
}

impl PriorityRangeBox for PriorityRangeBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn min_priority_id(&self) -> u8 {
        self.min_priority_id
    }

    fn max_priority_id(&self) -> u8 {
        self.max_priority_id
    }
}

impl<T: PriorityRangeBox> From<&T> for PriorityRangeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            min_priority_id: source.min_priority_id(),
            max_priority_id: source.max_priority_id(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_svpr() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&10u32.to_be_bytes()); // size = 8 + 2
        data.extend_from_slice(b"svpr");
        // min_priority_id=10(001010), reserved(00) => 0b00101000 = 0x28
        data.push(0x28);
        // max_priority_id=40(101000), reserved(00) => 0b10100000 = 0xA0
        data.push(0xA0);
        data
    }

    #[test]
    fn parse_svpr() {
        let data = make_svpr();
        let view = PriorityRangeBoxView::new(&data).unwrap();
        assert_eq!(view.min_priority_id(), 10);
        assert_eq!(view.max_priority_id(), 40);
    }

    #[test]
    fn roundtrip() {
        let data = make_svpr();
        let view = PriorityRangeBoxView::new(&data).unwrap();
        let owned = PriorityRangeBoxOwned::from(&view);
        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }
}
