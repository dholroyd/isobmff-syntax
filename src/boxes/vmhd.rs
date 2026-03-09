//! Video Media Header Box (vmhd) parsing and serialization.
//!
//! The Video Media Header Box contains general presentation information for video.
//!
//! ```text
//! aligned(8) class VideoMediaHeaderBox
//!    extends FullBox('vmhd', version = 0, 1) {
//!    template unsigned int(16) graphicsmode = 0; // copy, see below
//!    template unsigned int(16)[3] opcolor = {0, 0, 0};
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for VideoMediaHeaderBox.
pub const BOX_TYPE: BoxCode = BoxCode::VMHD;

/// Common interface for accessing VideoMediaHeaderBox data.
pub trait VideoMediaHeaderBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the graphics mode.
    fn graphics_mode(&self) -> u16;

    /// Returns the op color (RGB).
    fn op_color(&self) -> [u16; 3];
}

/// A borrowing view over raw VideoMediaHeaderBox bytes.
#[derive(Clone, Copy)]
pub struct VideoMediaHeaderBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> VideoMediaHeaderBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 8)?;
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
}

impl VideoMediaHeaderBox for VideoMediaHeaderBoxView<'_> {
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

    fn graphics_mode(&self) -> u16 {
        let o = self.payload_offset();
        BigEndian::read_u16(&self.data[o..o + 2])
    }

    fn op_color(&self) -> [u16; 3] {
        let o = self.payload_offset() + 2;
        [
            BigEndian::read_u16(&self.data[o..o + 2]),
            BigEndian::read_u16(&self.data[o + 2..o + 4]),
            BigEndian::read_u16(&self.data[o + 4..o + 6]),
        ]
    }
}

impl std::fmt::Debug for VideoMediaHeaderBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoMediaHeaderBoxView")
            .field("box_size", &self.box_size())
            .field("graphics_mode", &self.graphics_mode())
            .field("op_color", &self.op_color())
            .finish()
    }
}

/// An owned representation of VideoMediaHeaderBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoMediaHeaderBoxOwned {
    /// Flags (should be 0x000001).
    pub flags: u32,
    /// Graphics mode.
    pub graphics_mode: u16,
    /// Op color (RGB).
    pub op_color: [u16; 3],
}

impl VideoMediaHeaderBoxOwned {
    /// Creates a new VideoMediaHeaderBoxOwned with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(8) + 8 // 8 + 4 + 8
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, 0, self.flags)?;
        writer.write_u16::<BigEndian>(self.graphics_mode)?;
        writer.write_u16::<BigEndian>(self.op_color[0])?;
        writer.write_u16::<BigEndian>(self.op_color[1])?;
        writer.write_u16::<BigEndian>(self.op_color[2])?;
        Ok(())
    }
}

impl Default for VideoMediaHeaderBoxOwned {
    fn default() -> Self {
        Self {
            flags: 0x000001, // no lean ahead
            graphics_mode: 0,
            op_color: [0, 0, 0],
        }
    }
}

impl VideoMediaHeaderBox for VideoMediaHeaderBoxOwned {
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

    fn graphics_mode(&self) -> u16 {
        self.graphics_mode
    }

    fn op_color(&self) -> [u16; 3] {
        self.op_color
    }
}

impl<T: VideoMediaHeaderBox> From<&T> for VideoMediaHeaderBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            graphics_mode: source.graphics_mode(),
            op_color: source.op_color(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vmhd() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"vmhd");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 1]); // flags = 1
        data.extend_from_slice(&0u16.to_be_bytes()); // graphics_mode
        data.extend_from_slice(&0u16.to_be_bytes()); // op_color[0]
        data.extend_from_slice(&0u16.to_be_bytes()); // op_color[1]
        data.extend_from_slice(&0u16.to_be_bytes()); // op_color[2]
        data
    }

    #[test]
    fn parse_vmhd() {
        let data = make_vmhd();
        let view = VideoMediaHeaderBoxView::new(&data).unwrap();

        assert_eq!(view.flags(), 1);
        assert_eq!(view.graphics_mode(), 0);
        assert_eq!(view.op_color(), [0, 0, 0]);
    }

    #[test]
    fn roundtrip() {
        let data = make_vmhd();
        let view = VideoMediaHeaderBoxView::new(&data).unwrap();
        let owned = VideoMediaHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
