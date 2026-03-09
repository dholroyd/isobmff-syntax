//! Track Encryption Box (tenc) parsing and serialization.
//!
//! The Track Encryption Box specifies the default encryption parameters
//! for a protected track.
//!
//! ```text
//! aligned(8) class TrackEncryptionBox
//!    extends FullBox('tenc', version, 0) {
//!    unsigned int(8) reserved = 0;
//!    if (version==0) {
//!       unsigned int(8) reserved = 0;
//!    } else { // version >= 1
//!       unsigned int(4) default_crypt_byte_block;
//!       unsigned int(4) default_skip_byte_block;
//!    }
//!    unsigned int(8) default_isProtected;
//!    unsigned int(8) default_Per_Sample_IV_Size;
//!    unsigned int(8)[16] default_KID;
//!    if (default_isProtected ==1 && default_Per_Sample_IV_Size == 0) {
//!       unsigned int(8) default_constant_IV_size;
//!       unsigned int(8)[default_constant_IV_size] default_constant_IV;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TrackEncryptionBox.
pub const BOX_TYPE: BoxCode = BoxCode::TENC;

/// Common interface for accessing TrackEncryptionBox data.
pub trait TrackEncryptionBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the default crypt byte block (version >= 1).
    fn default_crypt_byte_block(&self) -> u8;

    /// Returns the default skip byte block (version >= 1).
    fn default_skip_byte_block(&self) -> u8;

    /// Returns whether the track is encrypted by default.
    fn is_encrypted(&self) -> u8;

    /// Returns the IV size.
    fn default_iv_size(&self) -> u8;

    /// Returns the default key ID.
    fn default_kid(&self) -> [u8; 16];

    /// Returns the default constant IV (if IV size is 0).
    fn default_constant_iv(&self) -> Option<&[u8]>;
}

/// A borrowing view over raw TrackEncryptionBox bytes.
#[derive(Clone, Copy)]
pub struct TrackEncryptionBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> TrackEncryptionBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        // reserved(1) + crypt/skip byte block(1) + isProtected(1) + IV size(1)
        // + default_KID(16); the constant IV that may follow is optional.
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 20)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the default constant IV (if isProtected == 1 and IV size is 0).
    pub fn default_constant_iv(&self) -> Option<&'a [u8]> {
        if self.is_encrypted() == 1 && self.default_iv_size() == 0 {
            let start = self.fullbox_offset + 4 + 4 + 16;
            if start < self.data.len() {
                let iv_size = self.data[start] as usize;
                let iv_start = start + 1;
                if iv_start + iv_size <= self.data.len() {
                    return Some(&self.data[iv_start..iv_start + iv_size]);
                }
            }
        }
        None
    }
}

impl<'a> TrackEncryptionBox for TrackEncryptionBoxView<'a> {
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

    fn default_crypt_byte_block(&self) -> u8 {
        if self.version() >= 1 {
            (self.data[self.fullbox_offset + 5] >> 4) & 0x0F
        } else {
            0
        }
    }

    fn default_skip_byte_block(&self) -> u8 {
        if self.version() >= 1 {
            self.data[self.fullbox_offset + 5] & 0x0F
        } else {
            0
        }
    }

    fn is_encrypted(&self) -> u8 {
        self.data[self.fullbox_offset + 6]
    }

    fn default_iv_size(&self) -> u8 {
        self.data[self.fullbox_offset + 7]
    }

    fn default_kid(&self) -> [u8; 16] {
        let mut kid = [0u8; 16];
        kid.copy_from_slice(&self.data[self.fullbox_offset + 8..self.fullbox_offset + 24]);
        kid
    }

    fn default_constant_iv(&self) -> Option<&[u8]> {
        self.default_constant_iv()
    }
}

impl std::fmt::Debug for TrackEncryptionBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackEncryptionBoxView")
            .field("is_encrypted", &self.is_encrypted())
            .field("default_iv_size", &self.default_iv_size())
            .finish()
    }
}

/// An owned representation of TrackEncryptionBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackEncryptionBoxOwned {
    /// Version (0 or 1).
    pub version: u8,
    /// Flags.
    pub flags: u32,
    /// Default crypt byte block (version >= 1, 4 bits).
    pub default_crypt_byte_block: u8,
    /// Default skip byte block (version >= 1, 4 bits).
    pub default_skip_byte_block: u8,
    /// Is encrypted flag.
    pub is_encrypted: u8,
    /// Default IV size.
    pub default_iv_size: u8,
    /// Default Key ID.
    pub default_kid: [u8; 16],
    /// Default constant IV (when iv_size is 0).
    pub default_constant_iv: Option<Vec<u8>>,
}

impl TrackEncryptionBoxOwned {
    /// Creates a new TrackEncryptionBoxOwned.
    pub fn new(default_kid: [u8; 16]) -> Self {
        Self {
            version: 0,
            flags: 0,
            default_crypt_byte_block: 0,
            default_skip_byte_block: 0,
            is_encrypted: 1,
            default_iv_size: 8,
            default_kid,
            default_constant_iv: None,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = if let Some(ref iv) = self.default_constant_iv {
            (4 + 16 + 1 + iv.len()) as u64
        } else {
            (4 + 16) as u64
        };
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, self.version, self.flags)?;
        writer.write_u8(0)?; // first reserved byte
        if self.version >= 1 {
            writer.write_u8((self.default_crypt_byte_block & 0x0F) << 4 | (self.default_skip_byte_block & 0x0F))?;
        } else {
            writer.write_u8(0)?; // second reserved byte
        }
        writer.write_u8(self.is_encrypted)?;
        writer.write_u8(self.default_iv_size)?;
        writer.write_all(&self.default_kid)?;
        if let Some(ref iv) = self.default_constant_iv {
            writer.write_u8(iv.len() as u8)?;
            writer.write_all(iv)?;
        }

        Ok(())
    }
}

impl Default for TrackEncryptionBoxOwned {
    fn default() -> Self {
        Self::new([0u8; 16])
    }
}

impl TrackEncryptionBox for TrackEncryptionBoxOwned {
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

    fn default_crypt_byte_block(&self) -> u8 {
        self.default_crypt_byte_block
    }

    fn default_skip_byte_block(&self) -> u8 {
        self.default_skip_byte_block
    }

    fn is_encrypted(&self) -> u8 {
        self.is_encrypted
    }

    fn default_iv_size(&self) -> u8 {
        self.default_iv_size
    }

    fn default_kid(&self) -> [u8; 16] {
        self.default_kid
    }

    fn default_constant_iv(&self) -> Option<&[u8]> {
        self.default_constant_iv.as_deref()
    }
}

impl<T: TrackEncryptionBox> From<&T> for TrackEncryptionBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            version: source.version(),
            flags: source.flags(),
            default_crypt_byte_block: source.default_crypt_byte_block(),
            default_skip_byte_block: source.default_skip_byte_block(),
            is_encrypted: source.is_encrypted(),
            default_iv_size: source.default_iv_size(),
            default_kid: source.default_kid(),
            default_constant_iv: source.default_constant_iv().map(|v| v.to_vec()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tenc() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&32u32.to_be_bytes()); // 8 + 4 + 4 + 16
        data.extend_from_slice(b"tenc");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.push(0); // first reserved byte
        data.push(0); // second reserved byte (v0)
        data.push(1); // is_encrypted
        data.push(8); // default_iv_size
        data.extend_from_slice(&[0u8; 16]); // default_kid
        data
    }

    #[test]
    fn parse_tenc() {
        let data = make_tenc();
        let view = TrackEncryptionBoxView::new(&data).unwrap();
        assert_eq!(view.is_encrypted(), 1);
        assert_eq!(view.default_iv_size(), 8);
    }

    #[test]
    fn roundtrip() {
        let data = make_tenc();
        let view = TrackEncryptionBoxView::new(&data).unwrap();
        let owned = TrackEncryptionBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
