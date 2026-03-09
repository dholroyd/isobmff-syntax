//! Stereo Video Box (stvi) parsing and serialization.
//!
//! The Stereo Video Box specifies stereo video configuration.
//!
//! ```text
//! aligned(8) class StereoVideoBox extends
//!    extends FullBox('stvi', version = 0, 0) {
//!    template unsigned int(30) reserved = 0;
//!    unsigned int(2) single_view_allowed;
//!    unsigned int(32) stereo_scheme;
//!    unsigned int(32) length;
//!    unsigned int(8)[length] stereo_indication_type;
//!    Box[] any_box; // optional
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for StereoVideoBox.
pub const BOX_TYPE: BoxCode = BoxCode::STVI;

/// Common interface for accessing StereoVideoBox data.
pub trait StereoVideoBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the single view allowed flag.
    fn single_view_allowed(&self) -> u8;

    /// Returns the stereo scheme.
    fn stereo_scheme(&self) -> u32;

    /// Returns the stereo indication type data.
    fn stereo_indication_type(&self) -> &[u8];
}

/// A borrowing view over raw StereoVideoBox bytes.
#[derive(Clone, Copy)]
pub struct StereoVideoBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> StereoVideoBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        // reserved(30) + single_view_allowed(2) packed in one u32, then
        // stereo_scheme(4) and the length(4) of the stereo_indication_type that
        // follows it.
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 12)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the length of the stereo indication type data.
    pub fn stereo_indication_type_length(&self) -> u32 {
        let o = self.fullbox_offset + 4 + 4 + 4; // after version/flags + reserved_and_single_view + stereo_scheme
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    /// Returns the stereo indication type data.
    pub fn stereo_indication_type(&self) -> &'a [u8] {
        let length = self.stereo_indication_type_length() as usize;
        let start = self.fullbox_offset + 4 + 4 + 4 + 4; // after version/flags + reserved_and_single_view + stereo_scheme + length
        let end = start.saturating_add(length).min(self.data.len());
        &self.data[start..end]
    }
}

impl<'a> StereoVideoBox for StereoVideoBoxView<'a> {
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

    fn single_view_allowed(&self) -> u8 {
        let o = self.fullbox_offset + 4; // reserved(30) + single_view_allowed(2) packed in one u32
        (BigEndian::read_u32(&self.data[o..o + 4]) & 0x03) as u8
    }

    fn stereo_scheme(&self) -> u32 {
        let o = self.fullbox_offset + 4 + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn stereo_indication_type(&self) -> &[u8] {
        self.stereo_indication_type()
    }
}

impl std::fmt::Debug for StereoVideoBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StereoVideoBoxView")
            .field("single_view_allowed", &self.single_view_allowed())
            .field("stereo_scheme", &self.stereo_scheme())
            .finish()
    }
}

/// An owned representation of StereoVideoBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct StereoVideoBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Single view allowed (2 bits).
    pub single_view_allowed: u8,
    /// Stereo scheme.
    pub stereo_scheme: u32,
    /// Stereo indication type data.
    pub stereo_indication_type: Vec<u8>,
}

impl StereoVideoBoxOwned {
    /// Creates a new StereoVideoBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (4 + 4 + 4 + self.stereo_indication_type.len()) as u64; // combined_word + stereo_scheme + length + data
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.single_view_allowed as u32 & 0x03)?; // reserved(30) + single_view_allowed(2)
        writer.write_u32::<BigEndian>(self.stereo_scheme)?;
        writer.write_u32::<BigEndian>(self.stereo_indication_type.len() as u32)?; // length
        writer.write_all(&self.stereo_indication_type)?;

        Ok(())
    }
}


impl StereoVideoBox for StereoVideoBoxOwned {
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

    fn single_view_allowed(&self) -> u8 {
        self.single_view_allowed
    }

    fn stereo_scheme(&self) -> u32 {
        self.stereo_scheme
    }

    fn stereo_indication_type(&self) -> &[u8] {
        &self.stereo_indication_type
    }
}

impl<T: StereoVideoBox> From<&T> for StereoVideoBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            single_view_allowed: source.single_view_allowed(),
            stereo_scheme: source.stereo_scheme(),
            stereo_indication_type: source.stereo_indication_type().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stvi() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&24u32.to_be_bytes()); // 8 + 4 + 4 + 4 + 4
        data.extend_from_slice(b"stvi");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&0u32.to_be_bytes()); // reserved(30) + single_view_allowed(2)
        data.extend_from_slice(&0u32.to_be_bytes()); // stereo_scheme
        data.extend_from_slice(&0u32.to_be_bytes()); // length (0 = no stereo_indication_type data)
        data
    }

    #[test]
    fn parse_stvi() {
        let data = make_stvi();
        let view = StereoVideoBoxView::new(&data).unwrap();
        assert_eq!(view.single_view_allowed(), 0);
        assert_eq!(view.stereo_scheme(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_stvi();
        let view = StereoVideoBoxView::new(&data).unwrap();
        let owned = StereoVideoBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
