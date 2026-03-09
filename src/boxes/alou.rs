//! Album Loudness Info Box (alou) parsing and serialization.
//!
//! The Album Loudness Info Box contains album-level loudness information.
//!
//! ```text
//! aligned(8) class AlbumLoudnessInfo
//!    extends LoudnessBaseBox('alou') {
//! }
//! ```

use crate::boxes::loudness::{
    LoudnessBaseEntry, LoudnessData, LoudnessEntryIter, LoudnessPayloadHeader,
    loudness_payload_size, parse_loudness_header, parse_loudness_payload, write_loudness_payload,
};
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for AlbumLoudnessInfoBox.
pub const BOX_TYPE: BoxCode = BoxCode::ALOU;

/// Common interface for accessing AlbumLoudnessInfoBox data.
pub trait AlbumLoudnessInfoBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the loudness info type (2 bits). Only meaningful for version >= 2.
    fn loudness_info_type(&self) -> u8;

    /// Returns the MAE group identifier (7 bits), if present.
    fn mae_group_id(&self) -> Option<u8>;

    /// Returns the MAE group preset identifier (5 bits), if present.
    fn mae_group_preset_id(&self) -> Option<u8>;

    /// Returns the number of loudness base entries.
    fn entry_count(&self) -> usize;

    /// Returns an iterator over the loudness base entries.
    fn entries(&self) -> impl Iterator<Item = LoudnessBaseEntry> + '_;
}

/// A borrowing view over raw AlbumLoudnessInfoBox bytes.
#[derive(Clone, Copy)]
pub struct AlbumLoudnessInfoBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> AlbumLoudnessInfoBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 0)?;

        let version = data[fullbox_offset];
        let payload = &data[fullbox_offset + 4..];

        // Validate the full payload by parsing it; discard the result.
        let _ = parse_loudness_payload(version, payload)?;

        Ok(Self {
            data,
            fullbox_offset,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the loudness payload (bytes after version/flags).
    #[inline]
    fn payload(&self) -> &'a [u8] {
        &self.data[self.fullbox_offset + 4..]
    }

    /// Parses the loudness payload header on demand.
    fn loudness_header(&self) -> LoudnessPayloadHeader {
        // Safety of unwrap: new() validated the full payload.
        parse_loudness_header(self.version(), self.payload()).unwrap()
    }
}

impl<'a> AlbumLoudnessInfoBox for AlbumLoudnessInfoBoxView<'a> {
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

    fn loudness_info_type(&self) -> u8 {
        self.loudness_header().loudness_info_type
    }

    fn mae_group_id(&self) -> Option<u8> {
        self.loudness_header().mae_group_id
    }

    fn mae_group_preset_id(&self) -> Option<u8> {
        self.loudness_header().mae_group_preset_id
    }

    fn entry_count(&self) -> usize {
        self.loudness_header().entry_count
    }

    fn entries(&self) -> impl Iterator<Item = LoudnessBaseEntry> + '_ {
        let hdr = self.loudness_header();
        LoudnessEntryIter::new(self.payload(), hdr.entries_offset, hdr.entry_count, self.version())
    }
}

impl std::fmt::Debug for AlbumLoudnessInfoBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlbumLoudnessInfoBoxView")
            .field("version", &self.version())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of AlbumLoudnessInfoBox data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AlbumLoudnessInfoBoxOwned {
    /// Box version.
    pub version: u8,
    /// Flags.
    pub flags: u32,
    /// Parsed loudness data.
    pub loudness: LoudnessData,
}

impl AlbumLoudnessInfoBoxOwned {
    /// Creates a new AlbumLoudnessInfoBoxOwned with default v0 data.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = loudness_payload_size(self.version, &self.loudness);
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, self.version, self.flags)?;
        write_loudness_payload(self.version, &self.loudness, writer)?;
        Ok(())
    }
}

impl AlbumLoudnessInfoBox for AlbumLoudnessInfoBoxOwned {
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

    fn loudness_info_type(&self) -> u8 {
        self.loudness.loudness_info_type
    }

    fn mae_group_id(&self) -> Option<u8> {
        self.loudness.mae_group_id
    }

    fn mae_group_preset_id(&self) -> Option<u8> {
        self.loudness.mae_group_preset_id
    }

    fn entry_count(&self) -> usize {
        self.loudness.entries.len()
    }

    fn entries(&self) -> impl Iterator<Item = LoudnessBaseEntry> + '_ {
        self.loudness.entries.iter().cloned()
    }
}

impl<T: AlbumLoudnessInfoBox> From<&T> for AlbumLoudnessInfoBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            version: source.version(),
            flags: source.flags(),
            loudness: LoudnessData {
                loudness_info_type: source.loudness_info_type(),
                mae_group_id: source.mae_group_id(),
                mae_group_preset_id: source.mae_group_preset_id(),
                entries: source.entries().collect(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boxes::loudness::LoudnessMeasurement;

    /// Build a v0 alou box with one entry, no measurements.
    fn make_alou_v0() -> Vec<u8> {
        let loudness = LoudnessData::default();
        let payload_size = loudness_payload_size(0, &loudness);
        let total = 12 + payload_size as usize;

        let mut data = Vec::with_capacity(total);
        data.extend_from_slice(&(total as u32).to_be_bytes());
        data.extend_from_slice(b"alou");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags

        let mut payload = Vec::new();
        write_loudness_payload(0, &loudness, &mut payload).unwrap();
        data.extend_from_slice(&payload);

        data
    }

    /// Build a v1 alou box with two entries.
    fn make_alou_v1() -> Vec<u8> {
        let loudness = LoudnessData {
            loudness_info_type: 0,
            mae_group_id: None,
            mae_group_preset_id: None,
            entries: vec![
                LoudnessBaseEntry {
                    eq_set_id: 0,
                    downmix_id: 0,
                    drc_set_id: 0,
                    bs_sample_peak_level: 0,
                    bs_true_peak_level: 0,
                    measurement_system_for_tp: 0,
                    reliability_for_tp: 0,
                    measurements: Vec::new(),
                },
                LoudnessBaseEntry {
                    eq_set_id: 5,
                    downmix_id: 1,
                    drc_set_id: 2,
                    bs_sample_peak_level: -100,
                    bs_true_peak_level: 50,
                    measurement_system_for_tp: 1,
                    reliability_for_tp: 3,
                    measurements: vec![LoudnessMeasurement {
                        method_definition: 7,
                        method_value: 0xAB,
                        measurement_system: 2,
                        reliability: 1,
                    }],
                },
            ],
        };

        let payload_size = loudness_payload_size(1, &loudness);
        let total = 12 + payload_size as usize;

        let mut data = Vec::with_capacity(total);
        data.extend_from_slice(&(total as u32).to_be_bytes());
        data.extend_from_slice(b"alou");
        data.push(1); // version
        data.extend_from_slice(&[0, 0, 0]); // flags

        let mut payload = Vec::new();
        write_loudness_payload(1, &loudness, &mut payload).unwrap();
        data.extend_from_slice(&payload);

        data
    }

    #[test]
    fn parse_v0() {
        let data = make_alou_v0();
        let view = AlbumLoudnessInfoBoxView::new(&data).unwrap();
        assert_eq!(view.version(), 0);
        assert_eq!(view.entry_count(), 1);
        let entries: Vec<_> = view.entries().collect();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn parse_v1() {
        let data = make_alou_v1();
        let view = AlbumLoudnessInfoBoxView::new(&data).unwrap();
        assert_eq!(view.version(), 1);
        assert_eq!(view.entry_count(), 2);
        let entries: Vec<_> = view.entries().collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].eq_set_id, 5);
        assert_eq!(entries[1].downmix_id, 1);
        assert_eq!(entries[1].measurements.len(), 1);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_alou_v0();
        let view = AlbumLoudnessInfoBoxView::new(&data).unwrap();
        let owned = AlbumLoudnessInfoBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_v1() {
        let data = make_alou_v1();
        let view = AlbumLoudnessInfoBoxView::new(&data).unwrap();
        let owned = AlbumLoudnessInfoBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
