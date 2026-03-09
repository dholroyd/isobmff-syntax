//! User Extension Box (uuid) parsing and serialization.
//!
//! The UUID Box provides a mechanism for extending the format.
//!
//! ```text
//! aligned(8) class UserExtensionBox extends Box('uuid', extended_type) {
//!    bit(8) data[];
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for UserExtensionBox.
pub const BOX_TYPE: BoxCode = BoxCode::UUID;

/// Common interface for accessing UserExtensionBox data.
pub trait UserExtensionBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the extended type UUID (16 bytes).
    fn extended_type(&self) -> [u8; 16];

    /// Returns the user data payload.
    fn user_data(&self) -> &[u8];
}

/// A borrowing view over raw UserExtensionBox bytes.
#[derive(Clone, Copy)]
pub struct UserExtensionBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> UserExtensionBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 16)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> UserExtensionBox for UserExtensionBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn extended_type(&self) -> [u8; 16] {
        let o = self.header_size;
        let mut uuid = [0u8; 16];
        uuid.copy_from_slice(&self.data[o..o + 16]);
        uuid
    }

    fn user_data(&self) -> &[u8] {
        let o = self.header_size + 16;
        &self.data[o..]
    }
}

impl std::fmt::Debug for UserExtensionBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let uuid = self.extended_type();
        let uuid_str = format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            uuid[0], uuid[1], uuid[2], uuid[3],
            uuid[4], uuid[5],
            uuid[6], uuid[7],
            uuid[8], uuid[9],
            uuid[10], uuid[11], uuid[12], uuid[13], uuid[14], uuid[15]
        );
        f.debug_struct("UserExtensionBoxView")
            .field("extended_type", &uuid_str)
            .field("user_data_len", &self.user_data().len())
            .finish()
    }
}

/// An owned representation of UserExtensionBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserExtensionBoxOwned {
    /// Extended type UUID.
    pub extended_type: [u8; 16],
    /// User data payload.
    pub user_data: Vec<u8>,
}

impl UserExtensionBoxOwned {
    /// Creates a new UserExtensionBoxOwned.
    pub fn new(extended_type: [u8; 16]) -> Self {
        Self {
            extended_type,
            user_data: Vec::new(),
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (16 + self.user_data.len()) as u64;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_all(&self.extended_type)?;
        writer.write_all(&self.user_data)?;

        Ok(())
    }
}

impl Default for UserExtensionBoxOwned {
    fn default() -> Self {
        Self::new([0; 16])
    }
}

impl UserExtensionBox for UserExtensionBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn extended_type(&self) -> [u8; 16] {
        self.extended_type
    }

    fn user_data(&self) -> &[u8] {
        &self.user_data
    }
}

impl<T: UserExtensionBox> From<&T> for UserExtensionBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            extended_type: source.extended_type(),
            user_data: source.user_data().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_uuid() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&32u32.to_be_bytes()); // size = 8 + 16 + 8
        data.extend_from_slice(b"uuid");
        // UUID: 6b6840f2-5f24-4fc5-ba39-a5192c7c23cf (example)
        data.extend_from_slice(&[
            0x6b, 0x68, 0x40, 0xf2, 0x5f, 0x24, 0x4f, 0xc5,
            0xba, 0x39, 0xa5, 0x19, 0x2c, 0x7c, 0x23, 0xcf,
        ]);
        data.extend_from_slice(b"testdata");
        data
    }

    #[test]
    fn parse_uuid() {
        let data = make_uuid();
        let view = UserExtensionBoxView::new(&data).unwrap();

        let expected_uuid = [
            0x6b, 0x68, 0x40, 0xf2, 0x5f, 0x24, 0x4f, 0xc5,
            0xba, 0x39, 0xa5, 0x19, 0x2c, 0x7c, 0x23, 0xcf,
        ];
        assert_eq!(view.extended_type(), expected_uuid);
        assert_eq!(view.user_data(), b"testdata");
    }

    #[test]
    fn roundtrip() {
        let data = make_uuid();
        let view = UserExtensionBoxView::new(&data).unwrap();
        let owned = UserExtensionBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
