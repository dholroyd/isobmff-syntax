//! Track Loudness Info Box (tlou) parsing and serialization.
//!
//! The Track Loudness Info Box contains loudness information for a track.
//!
//! ```text
//! aligned(8) class TrackLoudnessInfo
//!    extends LoudnessBaseBox('tlou') {
//! }
//! ```

use crate::boxes::loudness::{
    LoudnessBaseEntry, LoudnessData, loudness_payload_size, parse_loudness_payload,
    write_loudness_payload,
};
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TrackLoudnessInfoBox.
pub const BOX_TYPE: BoxCode = BoxCode::TLOU;

/// Common interface for accessing TrackLoudnessInfoBox data.
pub trait TrackLoudnessInfoBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the parsed loudness data.
    fn loudness_data(&self) -> &LoudnessData;

    /// Returns the loudness base entries.
    fn entries(&self) -> &[LoudnessBaseEntry] {
        &self.loudness_data().entries
    }
}

/// A borrowing view over raw TrackLoudnessInfoBox bytes.
#[derive(Clone)]
pub struct TrackLoudnessInfoBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    parsed: LoudnessData,
}

impl<'a> TrackLoudnessInfoBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 0)?;

        let version = data[fullbox_offset];
        let payload = &data[fullbox_offset + 4..];
        let parsed = parse_loudness_payload(version, payload)?;

        Ok(Self {
            data,
            fullbox_offset,
            parsed,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> TrackLoudnessInfoBox for TrackLoudnessInfoBoxView<'a> {
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

    fn loudness_data(&self) -> &LoudnessData {
        &self.parsed
    }
}

impl std::fmt::Debug for TrackLoudnessInfoBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackLoudnessInfoBoxView")
            .field("version", &self.version())
            .field("entry_count", &self.parsed.entries.len())
            .finish()
    }
}

/// An owned representation of TrackLoudnessInfoBox data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackLoudnessInfoBoxOwned {
    /// Box version.
    pub version: u8,
    /// Flags.
    pub flags: u32,
    /// Parsed loudness data.
    pub loudness: LoudnessData,
}

impl TrackLoudnessInfoBoxOwned {
    /// Creates a new TrackLoudnessInfoBoxOwned with default v0 data.
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

impl TrackLoudnessInfoBox for TrackLoudnessInfoBoxOwned {
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

    fn loudness_data(&self) -> &LoudnessData {
        &self.loudness
    }
}

impl<T: TrackLoudnessInfoBox> From<&T> for TrackLoudnessInfoBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            version: source.version(),
            flags: source.flags(),
            loudness: source.loudness_data().clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boxes::loudness::LoudnessMeasurement;

    /// Build a v0 tlou box with one entry, no measurements.
    fn make_tlou_v0() -> Vec<u8> {
        let loudness = LoudnessData::default();
        let payload_size = loudness_payload_size(0, &loudness);
        let total = 12 + payload_size as usize; // 8 (box header) + 4 (version/flags) + payload

        let mut data = Vec::with_capacity(total);
        data.extend_from_slice(&(total as u32).to_be_bytes());
        data.extend_from_slice(b"tlou");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags

        // Write the v0 loudness payload: 7 bytes of zeros
        let mut payload = Vec::new();
        write_loudness_payload(0, &loudness, &mut payload).unwrap();
        data.extend_from_slice(&payload);

        data
    }

    /// Build a v0 tlou box with measurements.
    fn make_tlou_v0_with_measurements() -> Vec<u8> {
        let loudness = LoudnessData {
            loudness_info_type: 0,
            mae_group_id: None,
            mae_group_preset_id: None,
            entries: vec![LoudnessBaseEntry {
                eq_set_id: 0,
                downmix_id: 3,
                drc_set_id: 1,
                bs_sample_peak_level: -50,
                bs_true_peak_level: 100,
                measurement_system_for_tp: 2,
                reliability_for_tp: 1,
                measurements: vec![
                    LoudnessMeasurement {
                        method_definition: 1,
                        method_value: 0xE4,
                        measurement_system: 1,
                        reliability: 2,
                    },
                ],
            }],
        };

        let payload_size = loudness_payload_size(0, &loudness);
        let total = 12 + payload_size as usize;

        let mut data = Vec::with_capacity(total);
        data.extend_from_slice(&(total as u32).to_be_bytes());
        data.extend_from_slice(b"tlou");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags

        let mut payload = Vec::new();
        write_loudness_payload(0, &loudness, &mut payload).unwrap();
        data.extend_from_slice(&payload);

        data
    }

    #[test]
    fn parse_v0() {
        let data = make_tlou_v0();
        let view = TrackLoudnessInfoBoxView::new(&data).unwrap();
        assert_eq!(view.version(), 0);
        assert_eq!(view.entries().len(), 1);
        assert_eq!(view.entries()[0].downmix_id, 0);
    }

    #[test]
    fn parse_v0_with_measurements() {
        let data = make_tlou_v0_with_measurements();
        let view = TrackLoudnessInfoBoxView::new(&data).unwrap();
        assert_eq!(view.version(), 0);
        assert_eq!(view.entries().len(), 1);
        assert_eq!(view.entries()[0].downmix_id, 3);
        assert_eq!(view.entries()[0].drc_set_id, 1);
        assert_eq!(view.entries()[0].bs_sample_peak_level, -50);
        assert_eq!(view.entries()[0].bs_true_peak_level, 100);
        assert_eq!(view.entries()[0].measurements.len(), 1);
        assert_eq!(view.entries()[0].measurements[0].method_definition, 1);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_tlou_v0();
        let view = TrackLoudnessInfoBoxView::new(&data).unwrap();
        let owned = TrackLoudnessInfoBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_v0_with_measurements() {
        let data = make_tlou_v0_with_measurements();
        let view = TrackLoudnessInfoBoxView::new(&data).unwrap();
        let owned = TrackLoudnessInfoBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
