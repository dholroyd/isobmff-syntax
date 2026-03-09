//! Payload Type Box (payt) parsing and serialization.
//!
//! The Payload Type Box contains the RTP payload type mapping.
//!
//! ```text
//! aligned(8) class hintpayloadID extends Box('payt') {
//!    uint(32) payloadID;    // payload ID used in RTP packets
//!    uint(8) count;
//!    char rtpmap_string[count];
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for PayloadTypeBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"payt");

/// Common interface for accessing PayloadTypeBox data.
pub trait PayloadTypeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the payload type.
    fn payload_type(&self) -> u32;

    /// Returns the RTP map string (if present).
    fn rtp_map(&self) -> &[u8];
}

/// A borrowing view over raw PayloadTypeBox bytes.
#[derive(Clone, Copy)]
pub struct PayloadTypeBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> PayloadTypeBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 4)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the RTP map string (if present).
    pub fn rtp_map(&self) -> &'a [u8] {
        let start = self.header_size + 4 + 1; // after payload_type + count byte
        if self.header_size + 4 < self.data.len() {
            let count = self.data[self.header_size + 4] as usize;
            if start + count <= self.data.len() {
                return &self.data[start..start + count];
            }
        }
        &[]
    }
}

impl<'a> PayloadTypeBox for PayloadTypeBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn payload_type(&self) -> u32 {
        BigEndian::read_u32(&self.data[self.header_size..self.header_size + 4])
    }

    fn rtp_map(&self) -> &[u8] {
        self.rtp_map()
    }
}

impl std::fmt::Debug for PayloadTypeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PayloadTypeBoxView")
            .field("payload_type", &self.payload_type())
            .finish()
    }
}

/// An owned representation of PayloadTypeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PayloadTypeBoxOwned {
    /// Payload type.
    pub payload_type: u32,
    /// RTP map string.
    pub rtp_map: Vec<u8>,
}

impl PayloadTypeBoxOwned {
    /// Creates a new PayloadTypeBoxOwned.
    pub fn new(payload_type: u32) -> Self {
        Self {
            payload_type,
            rtp_map: Vec::new(),
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (4 + 1 + self.rtp_map.len()) as u64;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_u32::<BigEndian>(self.payload_type)?;
        writer.write_u8(self.rtp_map.len() as u8)?;
        writer.write_all(&self.rtp_map)?;

        Ok(())
    }
}

impl Default for PayloadTypeBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl PayloadTypeBox for PayloadTypeBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn payload_type(&self) -> u32 {
        self.payload_type
    }

    fn rtp_map(&self) -> &[u8] {
        &self.rtp_map
    }
}

impl<T: PayloadTypeBox> From<&T> for PayloadTypeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            payload_type: source.payload_type(),
            rtp_map: source.rtp_map().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_payt() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&13u32.to_be_bytes()); // 8 + 4 + 1
        data.extend_from_slice(b"payt");
        data.extend_from_slice(&96u32.to_be_bytes()); // payload_type
        data.push(0); // rtp_map count
        data
    }

    #[test]
    fn parse_payt() {
        let data = make_payt();
        let view = PayloadTypeBoxView::new(&data).unwrap();
        assert_eq!(view.payload_type(), 96);
    }

    #[test]
    fn roundtrip() {
        let data = make_payt();
        let view = PayloadTypeBoxView::new(&data).unwrap();
        let owned = PayloadTypeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
