//! SVC Dependency Range Box (svdr) parsing and serialization.
//!
//! The SVC Dependency Range Box specifies the range of SVC dependency,
//! temporal, and quality IDs for a scalable group.
//!
//! ```text
//! aligned(8) class SvcDependencyRangeBox extends Box('svdr') {
//!    unsigned int(3) minDependencyId;
//!    unsigned int(3) minTemporalId;
//!    unsigned int(2) reserved = 0;
//!    unsigned int(3) maxDependencyId;
//!    unsigned int(3) maxTemporalId;
//!    unsigned int(2) reserved = 0;
//!    unsigned int(4) minQualityId;
//!    unsigned int(4) maxQualityId;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SvcDependencyRangeBox.
pub const BOX_TYPE: BoxCode = BoxCode::SVDR;

const PAYLOAD_SIZE: u64 = 3;

/// Common interface for accessing SvcDependencyRangeBox data.
pub trait SvcDependencyRangeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;
    /// Returns the box type.
    fn box_type(&self) -> BoxCode;
    /// Returns the minimum dependency ID (3 bits).
    fn min_dependency_id(&self) -> u8;
    /// Returns the minimum temporal ID (3 bits).
    fn min_temporal_id(&self) -> u8;
    /// Returns the maximum dependency ID (3 bits).
    fn max_dependency_id(&self) -> u8;
    /// Returns the maximum temporal ID (3 bits).
    fn max_temporal_id(&self) -> u8;
    /// Returns the minimum quality ID (4 bits).
    fn min_quality_id(&self) -> u8;
    /// Returns the maximum quality ID (4 bits).
    fn max_quality_id(&self) -> u8;
}

/// A borrowing view over raw SvcDependencyRangeBox bytes.
#[derive(Clone, Copy)]
pub struct SvcDependencyRangeBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> SvcDependencyRangeBoxView<'a> {
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

impl SvcDependencyRangeBox for SvcDependencyRangeBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn min_dependency_id(&self) -> u8 {
        (self.data[self.header_size] >> 5) & 0x07
    }

    fn min_temporal_id(&self) -> u8 {
        (self.data[self.header_size] >> 2) & 0x07
    }

    fn max_dependency_id(&self) -> u8 {
        (self.data[self.header_size + 1] >> 5) & 0x07
    }

    fn max_temporal_id(&self) -> u8 {
        (self.data[self.header_size + 1] >> 2) & 0x07
    }

    fn min_quality_id(&self) -> u8 {
        (self.data[self.header_size + 2] >> 4) & 0x0F
    }

    fn max_quality_id(&self) -> u8 {
        self.data[self.header_size + 2] & 0x0F
    }
}

impl std::fmt::Debug for SvcDependencyRangeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvcDependencyRangeBoxView")
            .field("min_dependency_id", &self.min_dependency_id())
            .field("max_dependency_id", &self.max_dependency_id())
            .finish()
    }
}

/// An owned representation of SvcDependencyRangeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SvcDependencyRangeBoxOwned {
    /// Minimum dependency ID (3 bits).
    pub min_dependency_id: u8,
    /// Minimum temporal ID (3 bits).
    pub min_temporal_id: u8,
    /// Maximum dependency ID (3 bits).
    pub max_dependency_id: u8,
    /// Maximum temporal ID (3 bits).
    pub max_temporal_id: u8,
    /// Minimum quality ID (4 bits).
    pub min_quality_id: u8,
    /// Maximum quality ID (4 bits).
    pub max_quality_id: u8,
}

impl SvcDependencyRangeBoxOwned {
    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(PAYLOAD_SIZE) + PAYLOAD_SIZE
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        // Byte 0: min_dependency_id(3) + min_temporal_id(3) + reserved(2)
        let b0 = ((self.min_dependency_id & 0x07) << 5) | ((self.min_temporal_id & 0x07) << 2);
        writer.write_all(&[b0])?;
        // Byte 1: max_dependency_id(3) + max_temporal_id(3) + reserved(2)
        let b1 = ((self.max_dependency_id & 0x07) << 5) | ((self.max_temporal_id & 0x07) << 2);
        writer.write_all(&[b1])?;
        // Byte 2: min_quality_id(4) + max_quality_id(4)
        let b2 = ((self.min_quality_id & 0x0F) << 4) | (self.max_quality_id & 0x0F);
        writer.write_all(&[b2])?;
        Ok(())
    }
}

impl SvcDependencyRangeBox for SvcDependencyRangeBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn min_dependency_id(&self) -> u8 {
        self.min_dependency_id
    }

    fn min_temporal_id(&self) -> u8 {
        self.min_temporal_id
    }

    fn max_dependency_id(&self) -> u8 {
        self.max_dependency_id
    }

    fn max_temporal_id(&self) -> u8 {
        self.max_temporal_id
    }

    fn min_quality_id(&self) -> u8 {
        self.min_quality_id
    }

    fn max_quality_id(&self) -> u8 {
        self.max_quality_id
    }
}

impl<T: SvcDependencyRangeBox> From<&T> for SvcDependencyRangeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            min_dependency_id: source.min_dependency_id(),
            min_temporal_id: source.min_temporal_id(),
            max_dependency_id: source.max_dependency_id(),
            max_temporal_id: source.max_temporal_id(),
            min_quality_id: source.min_quality_id(),
            max_quality_id: source.max_quality_id(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_svdr() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&11u32.to_be_bytes()); // size = 8 + 3
        data.extend_from_slice(b"svdr");
        // min_dep=1(001), min_temp=2(010), reserved(00) => 0b00101000 = 0x28
        data.push(0x28);
        // max_dep=3(011), max_temp=4(100), reserved(00) => 0b01110000 = 0x70
        data.push(0x70);
        // min_qual=5(0101), max_qual=10(1010) => 0b01011010 = 0x5A
        data.push(0x5A);
        data
    }

    #[test]
    fn parse_svdr() {
        let data = make_svdr();
        let view = SvcDependencyRangeBoxView::new(&data).unwrap();
        assert_eq!(view.min_dependency_id(), 1);
        assert_eq!(view.min_temporal_id(), 2);
        assert_eq!(view.max_dependency_id(), 3);
        assert_eq!(view.max_temporal_id(), 4);
        assert_eq!(view.min_quality_id(), 5);
        assert_eq!(view.max_quality_id(), 10);
    }

    #[test]
    fn roundtrip() {
        let data = make_svdr();
        let view = SvcDependencyRangeBoxView::new(&data).unwrap();
        let owned = SvcDependencyRangeBoxOwned::from(&view);
        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }
}
