//! Clean Aperture Box (clap) parsing and serialization.
//!
//! The Clean Aperture Box specifies the clean aperture of the video.
//!
//! ```text
//! class CleanApertureBox extends Box('clap'){
//!    unsigned int(32) cleanApertureWidthN;
//!    unsigned int(32) cleanApertureWidthD;
//!    unsigned int(32) cleanApertureHeightN;
//!    unsigned int(32) cleanApertureHeightD;
//!    unsigned int(32) horizOffN;
//!    unsigned int(32) horizOffD;
//!    unsigned int(32) vertOffN;
//!    unsigned int(32) vertOffD;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for CleanApertureBox.
pub const BOX_TYPE: BoxCode = BoxCode::CLAP;

/// Common interface for accessing CleanApertureBox data.
pub trait CleanApertureBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the clean aperture width numerator.
    fn clean_aperture_width_n(&self) -> u32;

    /// Returns the clean aperture width denominator.
    fn clean_aperture_width_d(&self) -> u32;

    /// Returns the clean aperture height numerator.
    fn clean_aperture_height_n(&self) -> u32;

    /// Returns the clean aperture height denominator.
    fn clean_aperture_height_d(&self) -> u32;

    /// Returns the horizontal offset numerator.
    fn horiz_off_n(&self) -> u32;

    /// Returns the horizontal offset denominator.
    fn horiz_off_d(&self) -> u32;

    /// Returns the vertical offset numerator.
    fn vert_off_n(&self) -> u32;

    /// Returns the vertical offset denominator.
    fn vert_off_d(&self) -> u32;
}

/// A borrowing view over raw CleanApertureBox bytes.
#[derive(Clone, Copy)]
pub struct CleanApertureBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> CleanApertureBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 32)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> CleanApertureBox for CleanApertureBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn clean_aperture_width_n(&self) -> u32 {
        let o = self.header_size;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn clean_aperture_width_d(&self) -> u32 {
        let o = self.header_size + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn clean_aperture_height_n(&self) -> u32 {
        let o = self.header_size + 8;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn clean_aperture_height_d(&self) -> u32 {
        let o = self.header_size + 12;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn horiz_off_n(&self) -> u32 {
        let o = self.header_size + 16;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn horiz_off_d(&self) -> u32 {
        let o = self.header_size + 20;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn vert_off_n(&self) -> u32 {
        let o = self.header_size + 24;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn vert_off_d(&self) -> u32 {
        let o = self.header_size + 28;
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for CleanApertureBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CleanApertureBoxView")
            .field("clean_aperture_width", &format!("{}/{}", self.clean_aperture_width_n(), self.clean_aperture_width_d()))
            .field("clean_aperture_height", &format!("{}/{}", self.clean_aperture_height_n(), self.clean_aperture_height_d()))
            .finish()
    }
}

/// An owned representation of CleanApertureBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanApertureBoxOwned {
    /// Clean aperture width numerator.
    pub clean_aperture_width_n: u32,
    /// Clean aperture width denominator.
    pub clean_aperture_width_d: u32,
    /// Clean aperture height numerator.
    pub clean_aperture_height_n: u32,
    /// Clean aperture height denominator.
    pub clean_aperture_height_d: u32,
    /// Horizontal offset numerator.
    pub horiz_off_n: u32,
    /// Horizontal offset denominator.
    pub horiz_off_d: u32,
    /// Vertical offset numerator.
    pub vert_off_n: u32,
    /// Vertical offset denominator.
    pub vert_off_d: u32,
}

impl CleanApertureBoxOwned {
    /// Creates a new CleanApertureBoxOwned with integer dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            clean_aperture_width_n: width,
            clean_aperture_width_d: 1,
            clean_aperture_height_n: height,
            clean_aperture_height_d: 1,
            horiz_off_n: 0,
            horiz_off_d: 1,
            vert_off_n: 0,
            vert_off_d: 1,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(32) + 32 // 8 + 32
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        writer.write_u32::<BigEndian>(self.clean_aperture_width_n)?;
        writer.write_u32::<BigEndian>(self.clean_aperture_width_d)?;
        writer.write_u32::<BigEndian>(self.clean_aperture_height_n)?;
        writer.write_u32::<BigEndian>(self.clean_aperture_height_d)?;
        writer.write_u32::<BigEndian>(self.horiz_off_n)?;
        writer.write_u32::<BigEndian>(self.horiz_off_d)?;
        writer.write_u32::<BigEndian>(self.vert_off_n)?;
        writer.write_u32::<BigEndian>(self.vert_off_d)?;

        Ok(())
    }
}

impl Default for CleanApertureBoxOwned {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl CleanApertureBox for CleanApertureBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn clean_aperture_width_n(&self) -> u32 {
        self.clean_aperture_width_n
    }

    fn clean_aperture_width_d(&self) -> u32 {
        self.clean_aperture_width_d
    }

    fn clean_aperture_height_n(&self) -> u32 {
        self.clean_aperture_height_n
    }

    fn clean_aperture_height_d(&self) -> u32 {
        self.clean_aperture_height_d
    }

    fn horiz_off_n(&self) -> u32 {
        self.horiz_off_n
    }

    fn horiz_off_d(&self) -> u32 {
        self.horiz_off_d
    }

    fn vert_off_n(&self) -> u32 {
        self.vert_off_n
    }

    fn vert_off_d(&self) -> u32 {
        self.vert_off_d
    }
}

impl<T: CleanApertureBox> From<&T> for CleanApertureBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            clean_aperture_width_n: source.clean_aperture_width_n(),
            clean_aperture_width_d: source.clean_aperture_width_d(),
            clean_aperture_height_n: source.clean_aperture_height_n(),
            clean_aperture_height_d: source.clean_aperture_height_d(),
            horiz_off_n: source.horiz_off_n(),
            horiz_off_d: source.horiz_off_d(),
            vert_off_n: source.vert_off_n(),
            vert_off_d: source.vert_off_d(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_clap() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&40u32.to_be_bytes());
        data.extend_from_slice(b"clap");
        data.extend_from_slice(&1920u32.to_be_bytes()); // width_n
        data.extend_from_slice(&1u32.to_be_bytes()); // width_d
        data.extend_from_slice(&1080u32.to_be_bytes()); // height_n
        data.extend_from_slice(&1u32.to_be_bytes()); // height_d
        data.extend_from_slice(&0u32.to_be_bytes()); // horiz_off_n
        data.extend_from_slice(&1u32.to_be_bytes()); // horiz_off_d
        data.extend_from_slice(&0u32.to_be_bytes()); // vert_off_n
        data.extend_from_slice(&1u32.to_be_bytes()); // vert_off_d
        data
    }

    #[test]
    fn parse_clap() {
        let data = make_clap();
        let view = CleanApertureBoxView::new(&data).unwrap();

        assert_eq!(view.clean_aperture_width_n(), 1920);
        assert_eq!(view.clean_aperture_height_n(), 1080);
    }

    #[test]
    fn roundtrip() {
        let data = make_clap();
        let view = CleanApertureBoxView::new(&data).unwrap();
        let owned = CleanApertureBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
