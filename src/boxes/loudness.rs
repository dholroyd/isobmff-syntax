//! Shared types and parsing for LoudnessBaseBox content.
//!
//! Used by TrackLoudnessInfoBox (tlou) and AlbumLoudnessInfoBox (alou),
//! which both extend LoudnessBaseBox as defined in ISO/IEC 14496-12 §12.2.7.
//!
//! ```text
//! aligned(8) class LoudnessBaseBox(loudnessType, version, flags=0) {
//!   if (version >= 2) {
//!     unsigned int(2) loudness_info_type;
//!     unsigned int(6) loudness_base_count;
//!     if (loudness_info_type == 1 || loudness_info_type == 2) {
//!       unsigned int(1) reserved = 0;
//!       unsigned int(7) mae_group_ID;
//!     }
//!     else if (loudness_info_type == 3) {
//!       unsigned int(3) reserved = 0;
//!       unsigned int(5) mae_group_preset_ID;
//!     }
//!   }
//!   else if (version == 1) {
//!     unsigned int(2) reserved = 0;
//!     unsigned int(6) loudness_base_count;
//!   } else {
//!     int loudness_base_count = 1;
//!   }
//!   for (a=1; a<=loudness_base_count; a++) {
//!     if (version >= 1) {
//!       unsigned int(2) reserved = 0;
//!       unsigned int(6) EQ_set_ID;
//!     }
//!     unsigned int(3) reserved = 0;
//!     unsigned int(7) downmix_ID;
//!     unsigned int(6) DRC_set_ID;
//!     signed int(12)  bs_sample_peak_level;
//!     signed int(12)  bs_true_peak_level;
//!     unsigned int(4) measurement_system_for_TP;
//!     unsigned int(4) reliability_for_TP;
//!     unsigned int(8) measurement_count;
//!     for (i=1; i<=measurement_count; i++) {
//!       unsigned int(8) method_definition;
//!       unsigned int(8) method_value;
//!       unsigned int(4) measurement_system;
//!       unsigned int(4) reliability;
//!     }
//!   }
//! }
//! ```

use crate::error::ParseError;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use std::io::{self, Write};

/// A single loudness measurement within a loudness entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoudnessMeasurement {
    /// Identifies the loudness measurement method.
    pub method_definition: u8,
    /// The measured loudness value.
    pub method_value: u8,
    /// The measurement system used (4 bits).
    pub measurement_system: u8,
    /// The reliability of the measurement (4 bits).
    pub reliability: u8,
}

/// A single loudness base entry containing peak levels, DRC info, and measurements.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoudnessBaseEntry {
    /// EQ set identifier (6 bits). Only meaningful for version >= 1.
    pub eq_set_id: u8,
    /// Downmix identifier (7 bits).
    pub downmix_id: u8,
    /// DRC set identifier (6 bits).
    pub drc_set_id: u8,
    /// Sample peak level as defined in ISO/IEC 23003-4 (signed 12-bit).
    pub bs_sample_peak_level: i16,
    /// True peak level as defined in ISO/IEC 23003-4 (signed 12-bit).
    pub bs_true_peak_level: i16,
    /// Measurement system for the true peak (4 bits).
    pub measurement_system_for_tp: u8,
    /// Reliability of the true peak measurement (4 bits).
    pub reliability_for_tp: u8,
    /// Individual loudness measurements.
    pub measurements: Vec<LoudnessMeasurement>,
}

/// Parsed content of a LoudnessBaseBox (the payload after version/flags).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoudnessData {
    /// The type of loudness information (2 bits). Only meaningful for version >= 2.
    pub loudness_info_type: u8,
    /// MAE group identifier (7 bits). Present when version >= 2 and
    /// loudness_info_type is 1 or 2.
    pub mae_group_id: Option<u8>,
    /// MAE group preset identifier (5 bits). Present when version >= 2 and
    /// loudness_info_type is 3.
    pub mae_group_preset_id: Option<u8>,
    /// The loudness base entries.
    pub entries: Vec<LoudnessBaseEntry>,
}

impl Default for LoudnessData {
    fn default() -> Self {
        Self {
            loudness_info_type: 0,
            mae_group_id: None,
            mae_group_preset_id: None,
            entries: vec![LoudnessBaseEntry {
                eq_set_id: 0,
                downmix_id: 0,
                drc_set_id: 0,
                bs_sample_peak_level: 0,
                bs_true_peak_level: 0,
                measurement_system_for_tp: 0,
                reliability_for_tp: 0,
                measurements: Vec::new(),
            }],
        }
    }
}

/// Parses the LoudnessBaseBox payload (bytes after version/flags).
pub fn parse_loudness_payload(version: u8, payload: &[u8]) -> Result<LoudnessData, ParseError> {
    let mut offset = 0usize;

    let loudness_info_type: u8;
    let loudness_base_count: u8;
    let mut mae_group_id = None;
    let mut mae_group_preset_id = None;

    if version >= 2 {
        check_len(payload, offset, 1)?;
        loudness_info_type = (payload[offset] >> 6) & 0x03;
        loudness_base_count = payload[offset] & 0x3F;
        offset += 1;

        if loudness_info_type == 1 || loudness_info_type == 2 {
            check_len(payload, offset, 1)?;
            mae_group_id = Some(payload[offset] & 0x7F);
            offset += 1;
        } else if loudness_info_type == 3 {
            check_len(payload, offset, 1)?;
            mae_group_preset_id = Some(payload[offset] & 0x1F);
            offset += 1;
        }
    } else if version == 1 {
        check_len(payload, offset, 1)?;
        loudness_info_type = 0;
        loudness_base_count = payload[offset] & 0x3F;
        offset += 1;
    } else {
        loudness_info_type = 0;
        loudness_base_count = 1;
    }

    let mut entries = Vec::with_capacity(loudness_base_count as usize);

    for _ in 0..loudness_base_count {
        let eq_set_id = if version >= 1 {
            check_len(payload, offset, 1)?;
            let id = payload[offset] & 0x3F;
            offset += 1;
            id
        } else {
            0
        };

        // 7 bytes: downmix/DRC(2) + peaks(3) + TP(1) + measurement_count(1)
        check_len(payload, offset, 7)?;

        let downmix_drc = BigEndian::read_u16(&payload[offset..offset + 2]);
        let downmix_id = ((downmix_drc >> 6) & 0x7F) as u8;
        let drc_set_id = (downmix_drc & 0x3F) as u8;
        offset += 2;

        let b0 = payload[offset] as i16;
        let b1 = payload[offset + 1] as i16;
        let b2 = payload[offset + 2] as i16;
        let sample_raw = (b0 << 4) | (b1 >> 4);
        let true_raw = ((b1 & 0x0F) << 8) | b2;
        // Sign extend from 12 bits
        let bs_sample_peak_level = (sample_raw << 4) >> 4;
        let bs_true_peak_level = (true_raw << 4) >> 4;
        offset += 3;

        let tp_byte = payload[offset];
        let measurement_system_for_tp = (tp_byte >> 4) & 0x0F;
        let reliability_for_tp = tp_byte & 0x0F;
        offset += 1;

        let measurement_count = payload[offset] as usize;
        offset += 1;

        let measurements_size = measurement_count.checked_mul(3).ok_or(
            ParseError::BufferTooShort {
                expected: usize::MAX,
                found: payload.len(),
            },
        )?;
        check_len(payload, offset, measurements_size)?;

        let mut measurements = Vec::with_capacity(measurement_count);
        for _ in 0..measurement_count {
            measurements.push(LoudnessMeasurement {
                method_definition: payload[offset],
                method_value: payload[offset + 1],
                measurement_system: (payload[offset + 2] >> 4) & 0x0F,
                reliability: payload[offset + 2] & 0x0F,
            });
            offset += 3;
        }

        entries.push(LoudnessBaseEntry {
            eq_set_id,
            downmix_id,
            drc_set_id,
            bs_sample_peak_level,
            bs_true_peak_level,
            measurement_system_for_tp,
            reliability_for_tp,
            measurements,
        });
    }

    Ok(LoudnessData {
        loudness_info_type,
        mae_group_id,
        mae_group_preset_id,
        entries,
    })
}

/// Returns the serialized size of the loudness payload (after version/flags).
pub fn loudness_payload_size(version: u8, data: &LoudnessData) -> u64 {
    let mut size = 0u64;

    if version >= 2 {
        size += 1; // loudness_info_type(2) + loudness_base_count(6)
        if data.loudness_info_type == 1 || data.loudness_info_type == 2 {
            size += 1; // mae_group_id
        } else if data.loudness_info_type == 3 {
            size += 1; // mae_group_preset_id
        }
    } else if version == 1 {
        size += 1; // reserved(2) + loudness_base_count(6)
    }

    for entry in &data.entries {
        if version >= 1 {
            size += 1; // eq_set_id
        }
        size += 7; // fixed entry fields
        size += entry.measurements.len() as u64 * 3;
    }

    size
}

/// Writes the loudness payload (after version/flags) to the given writer.
pub fn write_loudness_payload<W: Write>(
    version: u8,
    data: &LoudnessData,
    writer: &mut W,
) -> io::Result<()> {
    let count = data.entries.len() as u8;

    if version >= 2 {
        writer.write_u8((data.loudness_info_type << 6) | (count & 0x3F))?;
        if data.loudness_info_type == 1 || data.loudness_info_type == 2 {
            writer.write_u8(data.mae_group_id.unwrap_or(0) & 0x7F)?;
        } else if data.loudness_info_type == 3 {
            writer.write_u8(data.mae_group_preset_id.unwrap_or(0) & 0x1F)?;
        }
    } else if version == 1 {
        writer.write_u8(count & 0x3F)?;
    }

    for entry in &data.entries {
        if version >= 1 {
            writer.write_u8(entry.eq_set_id & 0x3F)?;
        }

        let downmix_drc: u16 =
            ((entry.downmix_id as u16 & 0x7F) << 6) | (entry.drc_set_id as u16 & 0x3F);
        writer.write_u16::<BigEndian>(downmix_drc)?;

        // Pack two signed 12-bit values into 3 bytes
        let sample = (entry.bs_sample_peak_level as u16) & 0x0FFF;
        let true_peak = (entry.bs_true_peak_level as u16) & 0x0FFF;
        writer.write_u8((sample >> 4) as u8)?;
        writer.write_u8((((sample & 0x0F) << 4) | (true_peak >> 8)) as u8)?;
        writer.write_u8((true_peak & 0xFF) as u8)?;

        writer.write_u8(
            (entry.measurement_system_for_tp << 4) | (entry.reliability_for_tp & 0x0F),
        )?;
        writer.write_u8(entry.measurements.len() as u8)?;

        for m in &entry.measurements {
            writer.write_u8(m.method_definition)?;
            writer.write_u8(m.method_value)?;
            writer.write_u8((m.measurement_system << 4) | (m.reliability & 0x0F))?;
        }
    }

    Ok(())
}

/// Parsed header of a LoudnessBaseBox payload (before the entries).
pub struct LoudnessPayloadHeader {
    /// The type of loudness information (2 bits). Only meaningful for version >= 2.
    pub loudness_info_type: u8,
    /// MAE group identifier (7 bits). Present when version >= 2 and
    /// loudness_info_type is 1 or 2.
    pub mae_group_id: Option<u8>,
    /// MAE group preset identifier (5 bits). Present when version >= 2 and
    /// loudness_info_type is 3.
    pub mae_group_preset_id: Option<u8>,
    /// Number of loudness base entries.
    pub entry_count: usize,
    /// Byte offset within the payload where entries begin.
    pub entries_offset: usize,
}

/// Parses the header portion of a LoudnessBaseBox payload (before entries).
///
/// Returns the header metadata including the offset where entries start.
pub fn parse_loudness_header(version: u8, payload: &[u8]) -> Result<LoudnessPayloadHeader, ParseError> {
    let mut offset = 0usize;

    let loudness_info_type: u8;
    let entry_count: usize;
    let mut mae_group_id = None;
    let mut mae_group_preset_id = None;

    if version >= 2 {
        check_len(payload, offset, 1)?;
        loudness_info_type = (payload[offset] >> 6) & 0x03;
        entry_count = (payload[offset] & 0x3F) as usize;
        offset += 1;

        if loudness_info_type == 1 || loudness_info_type == 2 {
            check_len(payload, offset, 1)?;
            mae_group_id = Some(payload[offset] & 0x7F);
            offset += 1;
        } else if loudness_info_type == 3 {
            check_len(payload, offset, 1)?;
            mae_group_preset_id = Some(payload[offset] & 0x1F);
            offset += 1;
        }
    } else if version == 1 {
        check_len(payload, offset, 1)?;
        loudness_info_type = 0;
        entry_count = (payload[offset] & 0x3F) as usize;
        offset += 1;
    } else {
        loudness_info_type = 0;
        entry_count = 1;
    }

    Ok(LoudnessPayloadHeader {
        loudness_info_type,
        mae_group_id,
        mae_group_preset_id,
        entry_count,
        entries_offset: offset,
    })
}

/// Iterator that parses `LoudnessBaseEntry` values on demand from a byte slice.
///
/// The caller must ensure the payload has been validated (e.g. via
/// [`parse_loudness_payload`]) before constructing this iterator.
#[derive(Clone)]
pub(crate) struct LoudnessEntryIter<'a> {
    payload: &'a [u8],
    offset: usize,
    remaining: usize,
    version: u8,
}

impl<'a> LoudnessEntryIter<'a> {
    /// Creates a new iterator over loudness entries.
    ///
    /// `payload` is the loudness payload (after version/flags), `entries_offset`
    /// is the byte offset where entries begin (after the header), `entry_count`
    /// is the number of entries, and `version` is the box version.
    pub(crate) fn new(payload: &'a [u8], entries_offset: usize, entry_count: usize, version: u8) -> Self {
        Self {
            payload,
            offset: entries_offset,
            remaining: entry_count,
            version,
        }
    }
}

impl Iterator for LoudnessEntryIter<'_> {
    type Item = LoudnessBaseEntry;

    fn next(&mut self) -> Option<LoudnessBaseEntry> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;

        let eq_set_id = if self.version >= 1 {
            let id = self.payload[self.offset] & 0x3F;
            self.offset += 1;
            id
        } else {
            0
        };

        let downmix_drc = BigEndian::read_u16(&self.payload[self.offset..self.offset + 2]);
        let downmix_id = ((downmix_drc >> 6) & 0x7F) as u8;
        let drc_set_id = (downmix_drc & 0x3F) as u8;
        self.offset += 2;

        let b0 = self.payload[self.offset] as i16;
        let b1 = self.payload[self.offset + 1] as i16;
        let b2 = self.payload[self.offset + 2] as i16;
        let sample_raw = (b0 << 4) | (b1 >> 4);
        let true_raw = ((b1 & 0x0F) << 8) | b2;
        let bs_sample_peak_level = (sample_raw << 4) >> 4;
        let bs_true_peak_level = (true_raw << 4) >> 4;
        self.offset += 3;

        let tp_byte = self.payload[self.offset];
        let measurement_system_for_tp = (tp_byte >> 4) & 0x0F;
        let reliability_for_tp = tp_byte & 0x0F;
        self.offset += 1;

        let measurement_count = self.payload[self.offset] as usize;
        self.offset += 1;

        let mut measurements = Vec::with_capacity(measurement_count);
        for _ in 0..measurement_count {
            measurements.push(LoudnessMeasurement {
                method_definition: self.payload[self.offset],
                method_value: self.payload[self.offset + 1],
                measurement_system: (self.payload[self.offset + 2] >> 4) & 0x0F,
                reliability: self.payload[self.offset + 2] & 0x0F,
            });
            self.offset += 3;
        }

        Some(LoudnessBaseEntry {
            eq_set_id,
            downmix_id,
            drc_set_id,
            bs_sample_peak_level,
            bs_true_peak_level,
            measurement_system_for_tp,
            reliability_for_tp,
            measurements,
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for LoudnessEntryIter<'_> {}

fn check_len(data: &[u8], offset: usize, needed: usize) -> Result<(), ParseError> {
    let end = offset.checked_add(needed).ok_or(ParseError::BufferTooShort {
        expected: usize::MAX,
        found: data.len(),
    })?;
    if data.len() < end {
        return Err(ParseError::BufferTooShort {
            expected: end,
            found: data.len(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_v0_minimal() {
        let data = LoudnessData {
            loudness_info_type: 0,
            mae_group_id: None,
            mae_group_preset_id: None,
            entries: vec![LoudnessBaseEntry {
                eq_set_id: 0,
                downmix_id: 0,
                drc_set_id: 0,
                bs_sample_peak_level: 0,
                bs_true_peak_level: 0,
                measurement_system_for_tp: 0,
                reliability_for_tp: 0,
                measurements: Vec::new(),
            }],
        };

        let mut buf = Vec::new();
        write_loudness_payload(0, &data, &mut buf).unwrap();
        assert_eq!(buf.len(), 7);

        let parsed = parse_loudness_payload(0, &buf).unwrap();
        assert_eq!(parsed, data);
    }

    #[test]
    fn roundtrip_v0_with_measurements() {
        let data = LoudnessData {
            loudness_info_type: 0,
            mae_group_id: None,
            mae_group_preset_id: None,
            entries: vec![LoudnessBaseEntry {
                eq_set_id: 0,
                downmix_id: 5,
                drc_set_id: 3,
                bs_sample_peak_level: -100,
                bs_true_peak_level: 200,
                measurement_system_for_tp: 2,
                reliability_for_tp: 3,
                measurements: vec![
                    LoudnessMeasurement {
                        method_definition: 1,
                        method_value: 0xE4,
                        measurement_system: 1,
                        reliability: 2,
                    },
                    LoudnessMeasurement {
                        method_definition: 2,
                        method_value: 0xD0,
                        measurement_system: 1,
                        reliability: 3,
                    },
                ],
            }],
        };

        let mut buf = Vec::new();
        write_loudness_payload(0, &data, &mut buf).unwrap();
        assert_eq!(buf.len(), 7 + 6); // 7 fixed + 2*3 measurements

        let parsed = parse_loudness_payload(0, &buf).unwrap();
        assert_eq!(parsed, data);
    }

    #[test]
    fn roundtrip_v1_multiple_entries() {
        let data = LoudnessData {
            loudness_info_type: 0,
            mae_group_id: None,
            mae_group_preset_id: None,
            entries: vec![
                LoudnessBaseEntry {
                    eq_set_id: 1,
                    downmix_id: 0,
                    drc_set_id: 0,
                    bs_sample_peak_level: 0,
                    bs_true_peak_level: 0,
                    measurement_system_for_tp: 0,
                    reliability_for_tp: 0,
                    measurements: Vec::new(),
                },
                LoudnessBaseEntry {
                    eq_set_id: 2,
                    downmix_id: 7,
                    drc_set_id: 10,
                    bs_sample_peak_level: -2048,
                    bs_true_peak_level: 2047,
                    measurement_system_for_tp: 15,
                    reliability_for_tp: 15,
                    measurements: vec![LoudnessMeasurement {
                        method_definition: 0xFF,
                        method_value: 0x80,
                        measurement_system: 0x0F,
                        reliability: 0x0F,
                    }],
                },
            ],
        };

        let mut buf = Vec::new();
        write_loudness_payload(1, &data, &mut buf).unwrap();
        // 1 (count) + 2 * (1 eq_set + 7 fixed) + 1 * 3 measurements = 1 + 16 + 3 = 20
        assert_eq!(buf.len(), 20);

        let parsed = parse_loudness_payload(1, &buf).unwrap();
        assert_eq!(parsed, data);
    }

    #[test]
    fn roundtrip_v2_mae_group_id() {
        let data = LoudnessData {
            loudness_info_type: 1,
            mae_group_id: Some(42),
            mae_group_preset_id: None,
            entries: vec![LoudnessBaseEntry {
                eq_set_id: 0,
                downmix_id: 0,
                drc_set_id: 0,
                bs_sample_peak_level: 0,
                bs_true_peak_level: 0,
                measurement_system_for_tp: 0,
                reliability_for_tp: 0,
                measurements: Vec::new(),
            }],
        };

        let mut buf = Vec::new();
        write_loudness_payload(2, &data, &mut buf).unwrap();
        // 1 (info_type+count) + 1 (mae_group_id) + 1 (eq_set) + 7 (fixed) = 10
        assert_eq!(buf.len(), 10);

        let parsed = parse_loudness_payload(2, &buf).unwrap();
        assert_eq!(parsed, data);
    }

    #[test]
    fn roundtrip_v2_mae_preset() {
        let data = LoudnessData {
            loudness_info_type: 3,
            mae_group_id: None,
            mae_group_preset_id: Some(17),
            entries: vec![LoudnessBaseEntry {
                eq_set_id: 0,
                downmix_id: 0,
                drc_set_id: 0,
                bs_sample_peak_level: 0,
                bs_true_peak_level: 0,
                measurement_system_for_tp: 0,
                reliability_for_tp: 0,
                measurements: Vec::new(),
            }],
        };

        let mut buf = Vec::new();
        write_loudness_payload(2, &data, &mut buf).unwrap();

        let parsed = parse_loudness_payload(2, &buf).unwrap();
        assert_eq!(parsed, data);
    }

    #[test]
    fn signed_peak_levels() {
        // Verify sign extension works correctly for 12-bit signed values
        let data = LoudnessData {
            loudness_info_type: 0,
            mae_group_id: None,
            mae_group_preset_id: None,
            entries: vec![LoudnessBaseEntry {
                eq_set_id: 0,
                downmix_id: 0,
                drc_set_id: 0,
                bs_sample_peak_level: -1,   // 0xFFF in 12-bit
                bs_true_peak_level: -2048,  // 0x800 in 12-bit (min value)
                measurement_system_for_tp: 0,
                reliability_for_tp: 0,
                measurements: Vec::new(),
            }],
        };

        let mut buf = Vec::new();
        write_loudness_payload(0, &data, &mut buf).unwrap();

        let parsed = parse_loudness_payload(0, &buf).unwrap();
        assert_eq!(parsed.entries[0].bs_sample_peak_level, -1);
        assert_eq!(parsed.entries[0].bs_true_peak_level, -2048);
    }

    #[test]
    fn size_calculation() {
        let data = LoudnessData {
            loudness_info_type: 0,
            mae_group_id: None,
            mae_group_preset_id: None,
            entries: vec![LoudnessBaseEntry {
                eq_set_id: 0,
                downmix_id: 0,
                drc_set_id: 0,
                bs_sample_peak_level: 0,
                bs_true_peak_level: 0,
                measurement_system_for_tp: 0,
                reliability_for_tp: 0,
                measurements: vec![LoudnessMeasurement {
                    method_definition: 1,
                    method_value: 2,
                    measurement_system: 3,
                    reliability: 4,
                }],
            }],
        };

        let mut buf = Vec::new();
        write_loudness_payload(0, &data, &mut buf).unwrap();
        assert_eq!(buf.len() as u64, loudness_payload_size(0, &data));

        let mut buf = Vec::new();
        write_loudness_payload(1, &data, &mut buf).unwrap();
        assert_eq!(buf.len() as u64, loudness_payload_size(1, &data));

        let mut buf = Vec::new();
        write_loudness_payload(2, &data, &mut buf).unwrap();
        assert_eq!(buf.len() as u64, loudness_payload_size(2, &data));
    }

    #[test]
    fn truncated_payload_rejected() {
        // v0 requires at least 7 bytes
        assert!(parse_loudness_payload(0, &[0; 6]).is_err());

        // v1 requires at least 1 (count) byte
        assert!(parse_loudness_payload(1, &[]).is_err());

        // v1 with count=1 but too short for entry
        assert!(parse_loudness_payload(1, &[1]).is_err());
    }
}
