//! View Identifier Box (vwid) parsing and serialization.
//!
//! The View Identifier Box specifies view identifiers for MVC multiview groups.
//!
//! ```text
//! aligned(8) class ViewIdentifierBox extends FullBox('vwid', 0, 0) {
//!    unsigned int(3) minTemporalId;
//!    unsigned int(3) maxTemporalId;
//!    unsigned int(2) reserved = 0;
//!    unsigned int(16) numViews;
//!    for (i = 0; i < numViews; i++) {
//!       unsigned int(10) viewId;
//!       unsigned int(10) viewOrderIndex;
//!       unsigned int(1) textureInStream;
//!       unsigned int(1) textureInTrack;
//!       unsigned int(1) depthInStream;
//!       unsigned int(1) depthInTrack;
//!       unsigned int(2) baseViewType;
//!       unsigned int(6) numRefViews;
//!       for (j = 0; j < numRefViews; j++) {
//!          unsigned int(10) refViewId;
//!          unsigned int(6) reserved = 0;
//!       }
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ViewIdentifierBox.
pub const BOX_TYPE: BoxCode = BoxCode::VWID;

/// Shared interface over a single vwid view entry.
///
/// Implemented by both the borrowing [`ViewEntryView`] and by
/// `&ViewEntryOwned`.
pub trait ViewEntry {
    /// View ID (10 bits).
    fn view_id(&self) -> u16;
    /// View order index (10 bits).
    fn view_order_index(&self) -> u16;
    /// Whether texture is in the stream.
    fn texture_in_stream(&self) -> bool;
    /// Whether texture is in the track.
    fn texture_in_track(&self) -> bool;
    /// Whether depth is in the stream.
    fn depth_in_stream(&self) -> bool;
    /// Whether depth is in the track.
    fn depth_in_track(&self) -> bool;
    /// Base view type (2 bits).
    fn base_view_type(&self) -> u8;
    /// Returns the number of reference view IDs.
    fn num_ref_views(&self) -> usize;
    /// Returns an iterator over the reference view IDs (10 bits each).
    fn ref_view_ids(&self) -> impl Iterator<Item = u16> + '_;

    /// Materialises an owned copy of this entry.
    fn to_owned(&self) -> ViewEntryOwned {
        ViewEntryOwned {
            view_id: self.view_id(),
            view_order_index: self.view_order_index(),
            texture_in_stream: self.texture_in_stream(),
            texture_in_track: self.texture_in_track(),
            depth_in_stream: self.depth_in_stream(),
            depth_in_track: self.depth_in_track(),
            base_view_type: self.base_view_type(),
            ref_view_ids: self.ref_view_ids().collect(),
        }
    }
}

/// A borrowing view over a single vwid view entry's raw bytes.
///
/// The first 4 bytes are the packed header word; the remainder is the
/// `ref_view_ids` array (2 bytes per id).
#[derive(Clone, Copy)]
pub struct ViewEntryView<'a> {
    data: &'a [u8],
}

impl<'a> ViewEntryView<'a> {
    #[inline]
    fn w0(&self) -> u32 {
        BigEndian::read_u32(&self.data[..4])
    }
}

impl<'a> ViewEntry for ViewEntryView<'a> {
    fn view_id(&self) -> u16 { ((self.w0() >> 22) & 0x3FF) as u16 }
    fn view_order_index(&self) -> u16 { ((self.w0() >> 12) & 0x3FF) as u16 }
    fn texture_in_stream(&self) -> bool { (self.w0() >> 11) & 1 == 1 }
    fn texture_in_track(&self) -> bool { (self.w0() >> 10) & 1 == 1 }
    fn depth_in_stream(&self) -> bool { (self.w0() >> 9) & 1 == 1 }
    fn depth_in_track(&self) -> bool { (self.w0() >> 8) & 1 == 1 }
    fn base_view_type(&self) -> u8 { ((self.w0() >> 6) & 0x03) as u8 }
    fn num_ref_views(&self) -> usize { (self.w0() & 0x3F) as usize }

    fn ref_view_ids(&self) -> impl Iterator<Item = u16> + '_ {
        self.data[4..]
            .chunks_exact(2)
            .map(|c| (BigEndian::read_u16(c) >> 6) & 0x3FF)
    }
}

/// An owned vwid view entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewEntryOwned {
    /// View ID (10 bits).
    pub view_id: u16,
    /// View order index (10 bits).
    pub view_order_index: u16,
    /// Whether texture is in the stream.
    pub texture_in_stream: bool,
    /// Whether texture is in the track.
    pub texture_in_track: bool,
    /// Whether depth is in the stream.
    pub depth_in_stream: bool,
    /// Whether depth is in the track.
    pub depth_in_track: bool,
    /// Base view type (2 bits).
    pub base_view_type: u8,
    /// Reference view IDs (10 bits each).
    pub ref_view_ids: Vec<u16>,
}

impl ViewEntry for &ViewEntryOwned {
    fn view_id(&self) -> u16 { self.view_id }
    fn view_order_index(&self) -> u16 { self.view_order_index }
    fn texture_in_stream(&self) -> bool { self.texture_in_stream }
    fn texture_in_track(&self) -> bool { self.texture_in_track }
    fn depth_in_stream(&self) -> bool { self.depth_in_stream }
    fn depth_in_track(&self) -> bool { self.depth_in_track }
    fn base_view_type(&self) -> u8 { self.base_view_type }
    fn num_ref_views(&self) -> usize { self.ref_view_ids.len() }

    fn ref_view_ids(&self) -> impl Iterator<Item = u16> + '_ {
        self.ref_view_ids.iter().copied()
    }

    fn to_owned(&self) -> ViewEntryOwned {
        (*self).clone()
    }
}

/// Common interface for accessing ViewIdentifierBox data.
pub trait ViewIdentifierBox {
    /// The concrete entry type yielded by [`Self::view_entries`].
    type Entry<'a>: ViewEntry
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;
    /// Returns the box type.
    fn box_type(&self) -> BoxCode;
    /// Returns the version.
    fn version(&self) -> u8;
    /// Returns the flags.
    fn flags(&self) -> u32;
    /// Returns the minimum temporal ID (3 bits).
    fn min_temporal_id(&self) -> u8;
    /// Returns the maximum temporal ID (3 bits).
    fn max_temporal_id(&self) -> u8;
    /// Returns the number of views.
    fn num_views(&self) -> u16;
    /// Returns an iterator over the view entries.
    fn view_entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_;
}

/// A borrowing view over raw ViewIdentifierBox bytes.
#[derive(Clone, Copy)]
pub struct ViewIdentifierBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> ViewIdentifierBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 3)?;
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

    /// Returns an iterator over the view entries, decoding each on demand.
    pub fn view_entries(&self) -> ViewEntryIter<'a> {
        ViewEntryIter {
            data: self.data,
            offset: self.payload_offset() + 3,
            remaining: self.num_views() as usize,
            done: false,
        }
    }
}

/// Lazy iterator over view entries in a [`ViewIdentifierBoxView`].
pub struct ViewEntryIter<'a> {
    data: &'a [u8],
    offset: usize,
    remaining: usize,
    done: bool,
}

impl<'a> Iterator for ViewEntryIter<'a> {
    type Item = Result<ViewEntryView<'a>, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done || self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;

        if self.offset + 4 > self.data.len() {
            self.done = true;
            return Some(Err(ParseError::UnexpectedEndOfData {
                context: "vwid view entry",
            }));
        }

        let w0 = BigEndian::read_u32(&self.data[self.offset..self.offset + 4]);
        let num_ref_views = (w0 & 0x3F) as usize;

        let ref_bytes_needed = match num_ref_views.checked_mul(2) {
            Some(v) => v,
            None => {
                self.done = true;
                return Some(Err(ParseError::UnexpectedEndOfData {
                    context: "vwid ref views overflow",
                }));
            }
        };
        let end = self.offset + 4 + ref_bytes_needed;
        if end > self.data.len() {
            self.done = true;
            return Some(Err(ParseError::UnexpectedEndOfData {
                context: "vwid ref views",
            }));
        }

        let entry = ViewEntryView {
            data: &self.data[self.offset..end],
        };
        self.offset = end;

        Some(Ok(entry))
    }
}

impl<'a> ViewIdentifierBox for ViewIdentifierBoxView<'a> {
    type Entry<'b> = ViewEntryView<'b> where Self: 'b;

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

    fn min_temporal_id(&self) -> u8 {
        let o = self.payload_offset();
        (self.data[o] >> 5) & 0x07
    }

    fn max_temporal_id(&self) -> u8 {
        let o = self.payload_offset();
        (self.data[o] >> 2) & 0x07
    }

    fn num_views(&self) -> u16 {
        let o = self.payload_offset() + 1;
        BigEndian::read_u16(&self.data[o..o + 2])
    }

    fn view_entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_ {
        ViewIdentifierBoxView::view_entries(self)
    }
}

impl std::fmt::Debug for ViewIdentifierBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewIdentifierBoxView")
            .field("num_views", &self.num_views())
            .finish()
    }
}

/// An owned representation of ViewIdentifierBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewIdentifierBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Minimum temporal ID (3 bits).
    pub min_temporal_id: u8,
    /// Maximum temporal ID (3 bits).
    pub max_temporal_id: u8,
    /// View entries.
    pub views: Vec<ViewEntryOwned>,
}

impl ViewIdentifierBoxOwned {
    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let mut payload: u64 = 3; // temporal IDs byte(1) + numViews(2)
        for view in &self.views {
            payload += 4; // fixed view header
            payload += view.ref_view_ids.len() as u64 * 2;
        }
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;

        // Temporal IDs byte: minTemporalId(3) + maxTemporalId(3) + reserved(2)
        let temporal_byte =
            ((self.min_temporal_id & 0x07) << 5) | ((self.max_temporal_id & 0x07) << 2);
        writer.write_u8(temporal_byte)?;
        writer.write_u16::<BigEndian>(self.views.len() as u16)?;

        for view in &self.views {
            let num_ref = view.ref_view_ids.len() as u32;
            let w0: u32 = ((view.view_id as u32 & 0x3FF) << 22)
                | ((view.view_order_index as u32 & 0x3FF) << 12)
                | (u32::from(view.texture_in_stream) << 11)
                | (u32::from(view.texture_in_track) << 10)
                | (u32::from(view.depth_in_stream) << 9)
                | (u32::from(view.depth_in_track) << 8)
                | ((view.base_view_type as u32 & 0x03) << 6)
                | (num_ref & 0x3F);
            writer.write_u32::<BigEndian>(w0)?;

            for &ref_id in &view.ref_view_ids {
                let w = (ref_id & 0x3FF) << 6;
                writer.write_u16::<BigEndian>(w)?;
            }
        }

        Ok(())
    }
}

impl ViewIdentifierBox for ViewIdentifierBoxOwned {
    type Entry<'a> = &'a ViewEntryOwned where Self: 'a;

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

    fn min_temporal_id(&self) -> u8 {
        self.min_temporal_id
    }

    fn max_temporal_id(&self) -> u8 {
        self.max_temporal_id
    }

    fn num_views(&self) -> u16 {
        self.views.len() as u16
    }

    fn view_entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_ {
        self.views.iter().map(Ok)
    }
}

impl TryFrom<&ViewIdentifierBoxView<'_>> for ViewIdentifierBoxOwned {
    type Error = ParseError;

    fn try_from(source: &ViewIdentifierBoxView<'_>) -> Result<Self, ParseError> {
        let views = ViewIdentifierBox::view_entries(source)
            .map(|res| res.map(|v| ViewEntry::to_owned(&v)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            flags: source.flags(),
            min_temporal_id: source.min_temporal_id(),
            max_temporal_id: source.max_temporal_id(),
            views,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vwid() -> Vec<u8> {
        let mut data = Vec::new();
        // size = 12 (fullbox header) + 1 (temporal) + 2 (numViews) + 4 (view entry) = 19
        data.extend_from_slice(&19u32.to_be_bytes());
        data.extend_from_slice(b"vwid");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        // temporal IDs: min=1(001), max=3(011), reserved(00) => 0b00101100 = 0x2C
        data.push(0x2C);
        data.extend_from_slice(&1u16.to_be_bytes()); // numViews = 1
        // View entry: viewId=5(10 bits), viewOrderIndex=0(10 bits),
        // textureInStream=1, textureInTrack=0, depthInStream=0, depthInTrack=0,
        // baseViewType=0(2 bits), numRefViews=0(6 bits)
        // = 0b0000000101_0000000000_1_0_0_0_00_000000
        // = 0x01400800
        #[allow(clippy::identity_op)]
        let w0: u32 = (5 << 22) | (0 << 12) | (1 << 11) | (0 << 10) | (0 << 9) | (0 << 8)
            | (0 << 6) | 0;
        data.extend_from_slice(&w0.to_be_bytes());
        data
    }

    #[test]
    fn parse_vwid() {
        let data = make_vwid();
        let view = ViewIdentifierBoxView::new(&data).unwrap();
        assert_eq!(view.min_temporal_id(), 1);
        assert_eq!(view.max_temporal_id(), 3);
        assert_eq!(view.num_views(), 1);

        let entries: Vec<_> =
            ViewIdentifierBox::view_entries(&view).collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].view_id(), 5);
        assert_eq!(entries[0].view_order_index(), 0);
        assert!(entries[0].texture_in_stream());
        assert!(!entries[0].texture_in_track());
        assert_eq!(entries[0].num_ref_views(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_vwid();
        let view = ViewIdentifierBoxView::new(&data).unwrap();
        let owned = ViewIdentifierBoxOwned::try_from(&view).unwrap();
        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }
}
