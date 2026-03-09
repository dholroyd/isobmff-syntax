//! Operating Points Information Box (oinf) parsing and serialization.
//!
//! The Operating Points Information Box provides information about AV1 operating points.

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for OperatingPointsInformationBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"oinf");

/// Common interface for accessing OperatingPointsInformationBox data.
pub trait OperatingPointsInformationBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the operating points data.
    fn operating_points_data(&self) -> &[u8];
}

/// A borrowing view over raw OperatingPointsInformationBox bytes.
#[derive(Clone, Copy)]
pub struct OperatingPointsInformationBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> OperatingPointsInformationBoxView<'a> {
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

    /// Returns the operating points data.
    pub fn operating_points_data(&self) -> &'a [u8] {
        let start = self.fullbox_offset + 4;
        if start < self.data.len() {
            &self.data[start..]
        } else {
            &[]
        }
    }
}

impl<'a> OperatingPointsInformationBox for OperatingPointsInformationBoxView<'a> {
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

    fn operating_points_data(&self) -> &[u8] {
        self.operating_points_data()
    }
}

impl std::fmt::Debug for OperatingPointsInformationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperatingPointsInformationBoxView")
            .field("data_len", &self.operating_points_data().len())
            .finish()
    }
}

/// An owned representation of OperatingPointsInformationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct OperatingPointsInformationBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Operating points data.
    pub data: Vec<u8>,
}

impl OperatingPointsInformationBoxOwned {
    /// Creates a new OperatingPointsInformationBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.data.len()) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_all(&self.data)?;

        Ok(())
    }
}


impl OperatingPointsInformationBox for OperatingPointsInformationBoxOwned {
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

    fn operating_points_data(&self) -> &[u8] {
        &self.data
    }
}

impl<T: OperatingPointsInformationBox> From<&T> for OperatingPointsInformationBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            data: source.operating_points_data().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_oinf() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"oinf");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data
    }

    #[test]
    fn parse_oinf() {
        let data = make_oinf();
        let view = OperatingPointsInformationBoxView::new(&data).unwrap();
        assert_eq!(view.version(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_oinf();
        let view = OperatingPointsInformationBoxView::new(&data).unwrap();
        let owned = OperatingPointsInformationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
