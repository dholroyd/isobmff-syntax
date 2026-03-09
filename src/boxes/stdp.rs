//! Degradation Priority Box (stdp) parsing and serialization.
//!
//! The Degradation Priority Box contains degradation priority for each sample.
//!
//! ```text
//! aligned(8) class DegradationPriorityBox
//!    extends FullBox('stdp', version = 0, 0) {
//!    for (i=0; i < sample_count; i++) {
//!       unsigned int(16) priority;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for DegradationPriorityBox.
pub const BOX_TYPE: BoxCode = BoxCode::STDP;

/// Common interface for accessing DegradationPriorityBox data.
pub trait DegradationPriorityBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the sample count (inferred from box size).
    fn sample_count(&self) -> usize;

    /// Returns all priorities.
    fn priorities(&self) -> impl Iterator<Item = u16> + '_;
}

/// A borrowing view over raw DegradationPriorityBox bytes.
#[derive(Clone, Copy)]
pub struct DegradationPriorityBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> DegradationPriorityBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 0)?;
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

    /// Returns the priority for the given sample (0-indexed).
    pub fn priority(&self, sample: usize) -> Option<u16> {
        if sample >= self.sample_count() {
            return None;
        }

        let offset = self.payload_offset() + sample * 2;
        if offset + 2 > self.data.len() {
            return None;
        }

        Some(BigEndian::read_u16(&self.data[offset..offset + 2]))
    }

}

impl<'a> DegradationPriorityBox for DegradationPriorityBoxView<'a> {
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

    fn sample_count(&self) -> usize {
        let payload_len = self.data.len() - self.payload_offset();
        payload_len / 2
    }

    fn priorities(&self) -> impl Iterator<Item = u16> + '_ {
        (0..self.sample_count()).filter_map(|i| self.priority(i))
    }
}

impl std::fmt::Debug for DegradationPriorityBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DegradationPriorityBoxView")
            .field("sample_count", &self.sample_count())
            .finish()
    }
}

/// An owned representation of DegradationPriorityBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct DegradationPriorityBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Degradation priorities for each sample.
    pub priorities: Vec<u16>,
}

impl DegradationPriorityBoxOwned {
    /// Creates a new DegradationPriorityBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.priorities.len() * 2) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;

        for priority in &self.priorities {
            writer.write_u16::<BigEndian>(*priority)?;
        }

        Ok(())
    }
}


impl DegradationPriorityBox for DegradationPriorityBoxOwned {
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

    fn sample_count(&self) -> usize {
        self.priorities.len()
    }

    fn priorities(&self) -> impl Iterator<Item = u16> + '_ {
        self.priorities.iter().copied()
    }
}

impl<T: DegradationPriorityBox> From<&T> for DegradationPriorityBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            priorities: source.priorities().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stdp() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 4 = 16 bytes (2 samples)
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"stdp");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&100u16.to_be_bytes()); // priority[0]
        data.extend_from_slice(&200u16.to_be_bytes()); // priority[1]
        data
    }

    #[test]
    fn parse_stdp() {
        let data = make_stdp();
        let view = DegradationPriorityBoxView::new(&data).unwrap();

        assert_eq!(view.sample_count(), 2);
        assert_eq!(view.priority(0), Some(100));
        assert_eq!(view.priority(1), Some(200));
    }

    #[test]
    fn roundtrip() {
        let data = make_stdp();
        let view = DegradationPriorityBoxView::new(&data).unwrap();
        let owned = DegradationPriorityBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
