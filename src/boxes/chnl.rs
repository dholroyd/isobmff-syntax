//! Channel Layout Box (chnl) parsing and serialization.
//!
//! The Channel Layout Box specifies the speaker positions for audio channels.
//!
//! ```text
//! aligned(8) class ChannelLayout
//!    extends FullBox('chnl', version, flags=0) {
//!    if (version == 0) {
//!       unsigned int(8) stream_structure;
//!       if (stream_structure & channelStructured) {
//!          unsigned int(8) definedLayout;
//!          if (definedLayout==0) {
//!             for (i = 1; i <= layout_channel_count; i++) {
//!                unsigned int(8) speaker_position;
//!                if (speaker_position == 126) {
//!                   signed int(16) azimuth;
//!                   signed int(8) elevation;
//!                }
//!             }
//!          } else {
//!             unsigned int(64) omittedChannelsMap;
//!          }
//!       }
//!       if (stream_structure & objectStructured) {
//!          unsigned int(8) object_count;
//!       }
//!    } else {
//!       unsigned int(4) stream_structure;
//!       unsigned int(4) format_ordering;
//!       unsigned int(8) baseChannelCount;
//!       if (stream_structure & channelStructured) {
//!          unsigned int(8) definedLayout;
//!          if (definedLayout==0) {
//!             unsigned int(8) layout_channel_count;
//!             for (i = 1; i <= layout_channel_count; i++) {
//!                unsigned int(8) speaker_position;
//!                if (speaker_position == 126) {
//!                   signed int(16) azimuth;
//!                   signed int(8) elevation;
//!                }
//!             }
//!          } else {
//!             int(4) reserved = 0;
//!             unsigned int(3) channel_order_definition;
//!             unsigned int(1) omitted_channels_present;
//!             if (omitted_channels_present == 1) {
//!                unsigned int(64) omittedChannelsMap;
//!             }
//!          }
//!       }
//!       if (stream_structure & objectStructured) {
//!          // object_count is derived from baseChannelCount
//!       }
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ChannelLayoutBox.
pub const BOX_TYPE: BoxCode = BoxCode::CHNL;

/// Stream structure flag: channel structured.
pub const CHANNEL_STRUCTURED: u8 = 0x01;
/// Stream structure flag: object structured.
pub const OBJECT_STRUCTURED: u8 = 0x02;

/// Speaker position code indicating explicit azimuth/elevation follow.
pub const SPEAKER_POSITION_EXPLICIT: u8 = 126;

/// A speaker position entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeakerPosition {
    /// Speaker position code (0-125 are predefined, 126 = explicit azimuth/elevation).
    pub position: u8,
    /// Azimuth in degrees (present when position == 126).
    pub azimuth: Option<i16>,
    /// Elevation in degrees (present when position == 126).
    pub elevation: Option<i8>,
}

/// Channel layout information for a defined layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChannelLayoutInfo {
    /// A predefined layout with an optional omitted channels map.
    DefinedLayout {
        /// Defined layout index (1-255).
        layout: u8,
        /// Omitted channels bitmap (v0: always present for defined layouts;
        /// v1: depends on channel_order_definition and omitted_channels_present).
        omitted_channels_map: Option<u64>,
        /// Channel order definition (v1 only, 0 for v0).
        channel_order_definition: u8,
    },
    /// Custom layout with explicit speaker positions.
    CustomLayout {
        /// Speaker positions for each channel.
        speakers: Vec<SpeakerPosition>,
    },
}

/// Common interface for accessing ChannelLayoutBox data.
pub trait ChannelLayoutBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the stream structure flags.
    fn stream_structure(&self) -> u8;

    /// Returns the format ordering (v1 only, 0 for v0).
    fn format_ordering(&self) -> u8;

    /// Returns the base channel count (v1 only, 0 for v0).
    fn base_channel_count(&self) -> u8;

    /// Returns the channel layout info (if stream_structure has channelStructured flag).
    fn channel_layout(&self) -> Option<ChannelLayoutInfo>;

    /// Returns the object count (if stream_structure has objectStructured flag).
    fn object_count(&self) -> Option<u8>;
}

/// A borrowing view over raw ChannelLayoutBox bytes.
#[derive(Clone, Copy)]
pub struct ChannelLayoutBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
}

impl<'a> ChannelLayoutBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let version = header.version;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 1)?;
        Ok(Self { data, fullbox_offset, version })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the offset of the first byte after the stream_structure field.
    fn channel_data_offset(&self) -> usize {
        if self.version == 0 {
            self.fullbox_offset + 4 + 1 // version/flags + stream_structure
        } else {
            self.fullbox_offset + 4 + 1 + 1 // version/flags + stream_structure|format_ordering + baseChannelCount
        }
    }

    /// Parses speaker positions from `start` up to `end`, reading at most `max_count` entries.
    fn parse_speakers(&self, start: usize, end: usize, max_count: usize) -> Vec<SpeakerPosition> {
        let mut speakers = Vec::new();
        let mut offset = start;
        while offset < end && speakers.len() < max_count {
            let position = self.data[offset];
            offset += 1;
            if position == SPEAKER_POSITION_EXPLICIT && offset + 3 <= end {
                let azimuth = BigEndian::read_i16(&self.data[offset..offset + 2]);
                let elevation = self.data[offset + 2] as i8;
                offset += 3;
                speakers.push(SpeakerPosition { position, azimuth: Some(azimuth), elevation: Some(elevation) });
            } else {
                speakers.push(SpeakerPosition { position, azimuth: None, elevation: None });
            }
        }
        speakers
    }
}

impl<'a> ChannelLayoutBox for ChannelLayoutBoxView<'a> {
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

    fn stream_structure(&self) -> u8 {
        if self.version == 0 {
            self.data[self.fullbox_offset + 4]
        } else {
            // Upper 4 bits of the byte
            self.data[self.fullbox_offset + 4] >> 4
        }
    }

    fn format_ordering(&self) -> u8 {
        if self.version == 0 {
            0
        } else {
            self.data[self.fullbox_offset + 4] & 0x0F
        }
    }

    fn base_channel_count(&self) -> u8 {
        if self.version == 0 {
            0
        } else if self.data.len() > self.fullbox_offset + 5 {
            self.data[self.fullbox_offset + 5]
        } else {
            0
        }
    }

    fn channel_layout(&self) -> Option<ChannelLayoutInfo> {
        if self.stream_structure() & CHANNEL_STRUCTURED == 0 {
            return None;
        }
        let offset = self.channel_data_offset();
        if offset >= self.data.len() {
            return None;
        }
        let defined_layout = self.data[offset];
        if defined_layout == 0 {
            // Custom layout with explicit speaker positions
            if self.version == 0 {
                // v0: layout_channel_count is implicit (parse until end of box minus
                // possible object_count byte)
                let has_objects = self.stream_structure() & OBJECT_STRUCTURED != 0;
                let end = if has_objects { self.data.len() - 1 } else { self.data.len() };
                let speakers = self.parse_speakers(offset + 1, end, usize::MAX);
                Some(ChannelLayoutInfo::CustomLayout { speakers })
            } else {
                // v1: explicit layout_channel_count field
                let count_offset = offset + 1;
                if count_offset >= self.data.len() {
                    return None;
                }
                let layout_channel_count = self.data[count_offset] as usize;
                let speakers = self.parse_speakers(count_offset + 1, self.data.len(), layout_channel_count);
                Some(ChannelLayoutInfo::CustomLayout { speakers })
            }
        } else {
            // Defined layout
            if self.version == 0 {
                // v0: omittedChannelsMap always follows
                let map_offset = offset + 1;
                let omitted = if map_offset + 8 <= self.data.len() {
                    Some(BigEndian::read_u64(&self.data[map_offset..map_offset + 8]))
                } else {
                    None
                };
                Some(ChannelLayoutInfo::DefinedLayout {
                    layout: defined_layout,
                    omitted_channels_map: omitted,
                    channel_order_definition: 0,
                })
            } else {
                // v1: packed byte with reserved(4) + channel_order_definition(3) + omitted_channels_present(1)
                let flags_offset = offset + 1;
                if flags_offset >= self.data.len() {
                    return None;
                }
                let flags_byte = self.data[flags_offset];
                let channel_order_definition = (flags_byte >> 1) & 0x07;
                let omitted_channels_present = flags_byte & 0x01;
                let omitted = if omitted_channels_present == 1 {
                    let map_offset = flags_offset + 1;
                    if map_offset + 8 <= self.data.len() {
                        Some(BigEndian::read_u64(&self.data[map_offset..map_offset + 8]))
                    } else {
                        None
                    }
                } else {
                    None
                };
                Some(ChannelLayoutInfo::DefinedLayout {
                    layout: defined_layout,
                    omitted_channels_map: omitted,
                    channel_order_definition,
                })
            }
        }
    }

    fn object_count(&self) -> Option<u8> {
        if self.stream_structure() & OBJECT_STRUCTURED == 0 {
            return None;
        }
        if self.version == 0 {
            // Last byte of the box
            Some(*self.data.last().unwrap_or(&0))
        } else {
            // v1: object_count is derived from baseChannelCount
            Some(self.base_channel_count())
        }
    }
}

impl std::fmt::Debug for ChannelLayoutBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelLayoutBoxView")
            .field("version", &self.version())
            .field("stream_structure", &self.stream_structure())
            .field("channel_layout", &self.channel_layout())
            .field("object_count", &self.object_count())
            .finish()
    }
}

/// An owned representation of ChannelLayoutBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelLayoutBoxOwned {
    /// Version of the box (0 or 1).
    pub version: u8,
    /// Flags.
    pub flags: u32,
    /// Stream structure flags.
    pub stream_structure: u8,
    /// Format ordering (v1 only, 0 for v0).
    pub format_ordering: u8,
    /// Base channel count (v1 only, 0 for v0).
    pub base_channel_count: u8,
    /// Channel layout info (present if channelStructured flag is set).
    pub channel_layout: Option<ChannelLayoutInfo>,
    /// Object count (present if objectStructured flag is set, v0 only - v1 derives from base_channel_count).
    pub object_count: Option<u8>,
}

impl ChannelLayoutBoxOwned {
    /// Creates a new ChannelLayoutBoxOwned.
    pub fn new(version: u8, stream_structure: u8) -> Self {
        Self {
            version,
            flags: 0,
            stream_structure,
            format_ordering: 0,
            base_channel_count: 0,
            channel_layout: None,
            object_count: None,
        }
    }

    /// Returns the serialized payload size.
    fn payload_size(&self) -> usize {
        let mut size: usize = if self.version == 0 { 1 } else { 2 }; // stream_structure (+ baseChannelCount for v1)

        if let Some(ref layout) = self.channel_layout {
            size += 1; // definedLayout
            match layout {
                ChannelLayoutInfo::CustomLayout { speakers } => {
                    if self.version != 0 {
                        size += 1; // layout_channel_count
                    }
                    for sp in speakers {
                        size += 1; // speaker_position
                        if sp.position == SPEAKER_POSITION_EXPLICIT {
                            size += 3; // azimuth(2) + elevation(1)
                        }
                    }
                }
                ChannelLayoutInfo::DefinedLayout { omitted_channels_map, .. } => {
                    if self.version == 0 {
                        size += 8; // omittedChannelsMap always present in v0
                    } else {
                        size += 1; // flags byte (reserved + channel_order_definition + omitted_channels_present)
                        if omitted_channels_map.is_some() {
                            size += 8;
                        }
                    }
                }
            }
        }

        if self.version == 0 && self.object_count.is_some() {
            size += 1; // object_count
        }

        size
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = self.payload_size() as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, self.version, self.flags)?;

        if self.version == 0 {
            writer.write_u8(self.stream_structure)?;
        } else {
            writer.write_u8((self.stream_structure << 4) | (self.format_ordering & 0x0F))?;
            writer.write_u8(self.base_channel_count)?;
        }

        if let Some(ref layout) = self.channel_layout {
            match layout {
                ChannelLayoutInfo::CustomLayout { speakers } => {
                    writer.write_u8(0)?; // definedLayout = 0
                    if self.version != 0 {
                        writer.write_u8(speakers.len() as u8)?; // layout_channel_count
                    }
                    for sp in speakers {
                        writer.write_u8(sp.position)?;
                        if sp.position == SPEAKER_POSITION_EXPLICIT {
                            writer.write_i16::<BigEndian>(sp.azimuth.unwrap_or(0))?;
                            writer.write_i8(sp.elevation.unwrap_or(0))?;
                        }
                    }
                }
                ChannelLayoutInfo::DefinedLayout { layout, omitted_channels_map, channel_order_definition } => {
                    writer.write_u8(*layout)?;
                    if self.version == 0 {
                        writer.write_u64::<BigEndian>(omitted_channels_map.unwrap_or(0))?;
                    } else {
                        let omitted_present = if omitted_channels_map.is_some() { 1u8 } else { 0u8 };
                        writer.write_u8((*channel_order_definition << 1) | omitted_present)?;
                        if let Some(map) = omitted_channels_map {
                            writer.write_u64::<BigEndian>(*map)?;
                        }
                    }
                }
            }
        }

        if self.version == 0
            && let Some(count) = self.object_count {
                writer.write_u8(count)?;
            }

        Ok(())
    }
}

impl Default for ChannelLayoutBoxOwned {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl ChannelLayoutBox for ChannelLayoutBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.version
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn stream_structure(&self) -> u8 {
        self.stream_structure
    }

    fn format_ordering(&self) -> u8 {
        self.format_ordering
    }

    fn base_channel_count(&self) -> u8 {
        self.base_channel_count
    }

    fn channel_layout(&self) -> Option<ChannelLayoutInfo> {
        self.channel_layout.clone()
    }

    fn object_count(&self) -> Option<u8> {
        if self.stream_structure & OBJECT_STRUCTURED != 0 {
            if self.version == 0 {
                self.object_count
            } else {
                Some(self.base_channel_count)
            }
        } else {
            None
        }
    }
}

impl<T: ChannelLayoutBox> From<&T> for ChannelLayoutBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            version: source.version(),
            flags: source.flags(),
            stream_structure: source.stream_structure(),
            format_ordering: source.format_ordering(),
            base_channel_count: source.base_channel_count(),
            channel_layout: source.channel_layout(),
            object_count: source.object_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chnl_empty() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&13u32.to_be_bytes()); // 8 + 4 + 1
        data.extend_from_slice(b"chnl");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.push(0); // stream_structure = 0 (neither channel nor object)
        data
    }

    fn make_chnl_v0_defined() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 1 + 1 + 8 = 22
        data.extend_from_slice(&22u32.to_be_bytes());
        data.extend_from_slice(b"chnl");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.push(CHANNEL_STRUCTURED); // stream_structure
        data.push(2); // definedLayout = 2 (stereo)
        data.extend_from_slice(&0u64.to_be_bytes()); // omittedChannelsMap
        data
    }

    fn make_chnl_v0_custom() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 1 + 1 + 2 speakers = 16
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"chnl");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.push(CHANNEL_STRUCTURED); // stream_structure
        data.push(0); // definedLayout = 0 (custom)
        data.push(1); // speaker_position = 1 (front left)
        data.push(2); // speaker_position = 2 (front right)
        data
    }

    #[test]
    fn parse_chnl_empty() {
        let data = make_chnl_empty();
        let view = ChannelLayoutBoxView::new(&data).unwrap();
        assert_eq!(view.stream_structure(), 0);
        assert!(view.channel_layout().is_none());
        assert!(view.object_count().is_none());
    }

    #[test]
    fn parse_chnl_v0_defined() {
        let data = make_chnl_v0_defined();
        let view = ChannelLayoutBoxView::new(&data).unwrap();
        assert_eq!(view.stream_structure(), CHANNEL_STRUCTURED);
        let layout = view.channel_layout().unwrap();
        assert!(matches!(layout, ChannelLayoutInfo::DefinedLayout { layout: 2, omitted_channels_map: Some(0), .. }));
    }

    #[test]
    fn parse_chnl_v0_custom() {
        let data = make_chnl_v0_custom();
        let view = ChannelLayoutBoxView::new(&data).unwrap();
        let layout = view.channel_layout().unwrap();
        match layout {
            ChannelLayoutInfo::CustomLayout { speakers } => {
                assert_eq!(speakers.len(), 2);
                assert_eq!(speakers[0].position, 1);
                assert_eq!(speakers[1].position, 2);
            }
            _ => panic!("expected custom layout"),
        }
    }

    #[test]
    fn roundtrip_empty() {
        let data = make_chnl_empty();
        let view = ChannelLayoutBoxView::new(&data).unwrap();
        let owned = ChannelLayoutBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_v0_defined() {
        let data = make_chnl_v0_defined();
        let view = ChannelLayoutBoxView::new(&data).unwrap();
        let owned = ChannelLayoutBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_v0_custom() {
        let data = make_chnl_v0_custom();
        let view = ChannelLayoutBoxView::new(&data).unwrap();
        let owned = ChannelLayoutBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
