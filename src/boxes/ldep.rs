//! Tier Dependency Box (ldep) parsing and serialization.
//!
//! The Tier Dependency Box specifies tier dependencies for MVC multiview groups.
//!
//! ```text
//! aligned(8) class TierDependencyBox extends Box('ldep') {
//!    unsigned int(16) numTierDependencies;
//!    for (i = 0; i < numTierDependencies; i++) {
//!       unsigned int(16) dependencyTierId;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TierDependencyBox.
pub const BOX_TYPE: BoxCode = BoxCode::LDEP;

/// Common interface for accessing TierDependencyBox data.
pub trait TierDependencyBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;
    /// Returns the box type.
    fn box_type(&self) -> BoxCode;
    /// Returns the number of dependency tier IDs.
    fn num_tier_dependencies(&self) -> u16;
    /// Returns the dependency tier ID at the given index.
    fn dependency_tier_id(&self, index: usize) -> Option<u16>;
    /// Returns an iterator over all dependency tier IDs.
    fn dependency_tier_ids(&self) -> impl Iterator<Item = u16> + '_;
}

/// A borrowing view over raw TierDependencyBox bytes.
#[derive(Clone, Copy)]
pub struct TierDependencyBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
    num_tier_dependencies: u16,
}

impl<'a> TierDependencyBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 2)?;
        let header_size = header.header_size as usize;

        let num_tier_dependencies = BigEndian::read_u16(&data[header_size..header_size + 2]);

        let expected_size = header_size
            .checked_add(2)
            .and_then(|s| s.checked_add((num_tier_dependencies as usize).checked_mul(2)?))
            .ok_or(ParseError::BufferTooShort {
                expected: usize::MAX,
                found: data.len(),
            })?;
        if data.len() < expected_size {
            return Err(ParseError::BufferTooShort {
                expected: expected_size,
                found: data.len(),
            });
        }

        Ok(Self {
            data,
            header_size,
            num_tier_dependencies,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl TierDependencyBox for TierDependencyBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn num_tier_dependencies(&self) -> u16 {
        self.num_tier_dependencies
    }

    fn dependency_tier_id(&self, index: usize) -> Option<u16> {
        if index >= self.num_tier_dependencies as usize {
            return None;
        }
        let o = self.header_size + 2 + index * 2;
        Some(BigEndian::read_u16(&self.data[o..o + 2]))
    }

    fn dependency_tier_ids(&self) -> impl Iterator<Item = u16> + '_ {
        let start = self.header_size + 2;
        let end = start + self.num_tier_dependencies as usize * 2;
        self.data[start..end]
            .chunks_exact(2)
            .map(BigEndian::read_u16)
    }
}

impl std::fmt::Debug for TierDependencyBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TierDependencyBoxView")
            .field("num_tier_dependencies", &self.num_tier_dependencies())
            .finish()
    }
}

/// An owned representation of TierDependencyBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TierDependencyBoxOwned {
    /// Dependency tier IDs.
    pub dependency_tier_ids: Vec<u16>,
}

impl TierDependencyBoxOwned {
    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = 2u64 + (self.dependency_tier_ids.len() as u64) * 2;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        writer.write_u16::<BigEndian>(self.dependency_tier_ids.len() as u16)?;
        for &id in &self.dependency_tier_ids {
            writer.write_u16::<BigEndian>(id)?;
        }
        Ok(())
    }
}

impl TierDependencyBox for TierDependencyBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn num_tier_dependencies(&self) -> u16 {
        self.dependency_tier_ids.len() as u16
    }

    fn dependency_tier_id(&self, index: usize) -> Option<u16> {
        self.dependency_tier_ids.get(index).copied()
    }

    fn dependency_tier_ids(&self) -> impl Iterator<Item = u16> + '_ {
        self.dependency_tier_ids.iter().copied()
    }
}

impl<T: TierDependencyBox> From<&T> for TierDependencyBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            dependency_tier_ids: source.dependency_tier_ids().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ldep() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&14u32.to_be_bytes()); // size = 8 + 2 + 4
        data.extend_from_slice(b"ldep");
        data.extend_from_slice(&2u16.to_be_bytes()); // numTierDependencies
        data.extend_from_slice(&100u16.to_be_bytes()); // dependency_tier_id[0]
        data.extend_from_slice(&200u16.to_be_bytes()); // dependency_tier_id[1]
        data
    }

    #[test]
    fn parse_ldep() {
        let data = make_ldep();
        let view = TierDependencyBoxView::new(&data).unwrap();
        assert_eq!(view.num_tier_dependencies(), 2);
        assert_eq!(view.dependency_tier_id(0), Some(100));
        assert_eq!(view.dependency_tier_id(1), Some(200));
        assert_eq!(view.dependency_tier_id(2), None);
    }

    #[test]
    fn roundtrip() {
        let data = make_ldep();
        let view = TierDependencyBoxView::new(&data).unwrap();
        let owned = TierDependencyBoxOwned::from(&view);
        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }
}
