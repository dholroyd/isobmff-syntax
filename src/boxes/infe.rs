//! Item Info Entry Box (infe) parsing and serialization.
//!
//! The Item Info Entry Box contains information about a single item.
//!
//! ```text
//! aligned(8) class ItemInfoEntry
//!    extends FullBox('infe', version, flags) {
//!    if ((version == 0) || (version == 1)) {
//!       unsigned int(16) item_ID;
//!       unsigned int(16) item_protection_index;
//!       utf8string item_name;
//!       utf8string content_type;
//!       utf8string content_encoding; //optional
//!    }
//!    if (version == 1) {
//!       unsigned int(32) extension_type; //optional
//!       ItemInfoExtension(extension_type); //optional
//!    }
//!    if (version >= 2) {
//!       if (version == 2) {
//!          unsigned int(16) item_ID;
//!       } else if (version == 3) {
//!          unsigned int(32) item_ID;
//!       }
//!       unsigned int(16) item_protection_index;
//!       unsigned int(32) item_type;
//!       utf8string item_name;
//!       if (item_type=='mime') {
//!          utf8string content_type;
//!          utf8string content_encoding; //optional
//!       } else if (item_type=='uri ') {
//!          utf8string item_uri_type;
//!       }
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for ItemInfoEntryBox.
pub const BOX_TYPE: BoxCode = BoxCode::INFE;

/// Common interface for accessing ItemInfoEntryBox data.
pub trait ItemInfoEntryBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the item ID.
    fn item_id(&self) -> u32;

    /// Returns the item protection index.
    fn item_protection_index(&self) -> u16;

    /// Returns the item type (4 bytes, version >= 2).
    fn item_type(&self) -> Option<FourCC>;

    /// Returns the item name.
    fn item_name(&self) -> &[u8];

    /// Returns the content type string (without null terminator).
    ///
    /// For version 0/1: always present after item_name.
    /// For version >= 2: present only when item_type is 'mime'.
    /// Returns empty slice if not present.
    fn content_type(&self) -> &[u8];

    /// Returns the content encoding string (without null terminator).
    ///
    /// For version 0/1: optionally present after content_type.
    /// For version >= 2: optionally present after content_type when item_type is 'mime'.
    /// Returns empty slice if not present.
    fn content_encoding(&self) -> &[u8];

    /// Returns the item URI type string (without null terminator).
    ///
    /// Present only for version >= 2 when item_type is 'uri '.
    /// Returns empty slice if not present.
    fn item_uri_type(&self) -> &[u8];
}

/// Finds a null-terminated string in data starting at `start`.
///
/// Returns `(offset_after_null, string_bytes_without_null)`.
/// Returns `Err` if no null terminator is found.
fn find_null_terminated<'a>(
    data: &'a [u8],
    start: usize,
    field: &'static str,
) -> Result<(usize, &'a [u8]), ParseError> {
    let slice = &data[start..];
    let end = slice
        .iter()
        .position(|&b| b == 0)
        .ok_or(ParseError::MissingNullTerminator { field })?;
    let after_null = start
        .checked_add(end)
        .and_then(|v| v.checked_add(1))
        .ok_or(ParseError::BufferTooShort {
            expected: usize::MAX,
            found: data.len(),
        })?;
    Ok((after_null, &slice[..end]))
}

/// A borrowing view over raw ItemInfoEntryBox bytes.
#[derive(Clone, Copy)]
pub struct ItemInfoEntryBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
    /// Offset of item_name string start, and offset after its null terminator.
    item_name_range: (usize, usize),
}

impl<'a> ItemInfoEntryBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let version = header.version;

        // Minimum size depends on version
        let min_payload = match version {
            0 | 1 => 4, // item_id(2) + protection_index(2)
            2 => 8,     // item_id(2) + protection_index(2) + item_type(4)
            _ => 10,    // item_id(4) + protection_index(2) + item_type(4)
        };
        let fullbox_offset = header.validate(data, BOX_TYPE, None, min_payload)?;

        // Validate null-terminated strings
        let item_name_start = match version {
            0 | 1 => fullbox_offset + 8,  // version/flags(4) + item_id(2) + protection(2)
            2 => fullbox_offset + 12,     // version/flags(4) + item_id(2) + protection(2) + item_type(4)
            _ => fullbox_offset + 14,     // version/flags(4) + item_id(4) + protection(2) + item_type(4)
        };

        let (after_name, _) = find_null_terminated(data, item_name_start, "item_name")?;

        let mut offset = after_name;

        if version <= 1 {
            // content_type is mandatory for v0/v1
            if offset < data.len() {
                (offset, _) = find_null_terminated(data, offset, "content_type")?;
                // content_encoding is optional - validate only if data remains
                if offset < data.len() {
                    (offset, _) = find_null_terminated(data, offset, "content_encoding")?;
                }
            }
        } else {
            // version >= 2: strings depend on item_type
            let type_offset = fullbox_offset + 4 + if version >= 3 { 6 } else { 4 };
            let item_type = [data[type_offset], data[type_offset + 1], data[type_offset + 2], data[type_offset + 3]];
            if item_type == *b"mime" && offset < data.len() {
                (offset, _) = find_null_terminated(data, offset, "content_type")?;
                if offset < data.len() {
                    (offset, _) = find_null_terminated(data, offset, "content_encoding")?;
                }
            } else if item_type == *b"uri " && offset < data.len() {
                (offset, _) = find_null_terminated(data, offset, "item_uri_type")?;
            }
        }
        let _ = offset;

        Ok(Self {
            data,
            fullbox_offset,
            version,
            item_name_range: (item_name_start, after_name),
        })
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

    fn item_name_offset(&self) -> usize {
        self.item_name_range.0
    }

    /// Returns the offset immediately after item_name's null terminator.
    fn after_item_name_offset(&self) -> usize {
        self.item_name_range.1
    }

    /// Returns true if the version >= 2 item_type is 'mime'.
    fn is_mime_type(&self) -> bool {
        if let Some(ft) = ItemInfoEntryBox::item_type(self) {
            ft.0 == *b"mime"
        } else {
            false
        }
    }

    /// Returns true if the version >= 2 item_type is 'uri '.
    fn is_uri_type(&self) -> bool {
        if let Some(ft) = ItemInfoEntryBox::item_type(self) {
            ft.0 == *b"uri "
        } else {
            false
        }
    }
}

impl<'a> ItemInfoEntryBox for ItemInfoEntryBoxView<'a> {
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

    fn item_id(&self) -> u32 {
        let o = self.payload_offset();
        if self.version >= 3 {
            BigEndian::read_u32(&self.data[o..o + 4])
        } else {
            BigEndian::read_u16(&self.data[o..o + 2]) as u32
        }
    }

    fn item_protection_index(&self) -> u16 {
        let o = self.payload_offset() + if self.version >= 3 { 4 } else { 2 };
        BigEndian::read_u16(&self.data[o..o + 2])
    }

    fn item_type(&self) -> Option<FourCC> {
        if self.version >= 2 {
            let o = self.payload_offset() + if self.version >= 3 { 6 } else { 4 };
            Some(FourCC([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]]))
        } else {
            None
        }
    }

    fn item_name(&self) -> &[u8] {
        // Null terminator validated in constructor
        let start = self.item_name_offset();
        let end = self.data[start..].iter().position(|&b| b == 0).unwrap();
        &self.data[start..start + end]
    }

    fn content_type(&self) -> &[u8] {
        let has_content_type = match self.version {
            0 | 1 => true,
            _ => self.is_mime_type(),
        };
        if !has_content_type {
            return &[];
        }
        let after_name = self.after_item_name_offset();
        if after_name >= self.data.len() {
            return &[];
        }
        // Null terminator validated in constructor
        let end = self.data[after_name..].iter().position(|&b| b == 0).unwrap();
        &self.data[after_name..after_name + end]
    }

    fn content_encoding(&self) -> &[u8] {
        let has_encoding = match self.version {
            0 | 1 => true,
            _ => self.is_mime_type(),
        };
        if !has_encoding {
            return &[];
        }
        let after_name = self.after_item_name_offset();
        if after_name >= self.data.len() {
            return &[];
        }
        // Skip past content_type (validated in constructor)
        let ct_end = self.data[after_name..].iter().position(|&b| b == 0).unwrap();
        let after_ct = after_name + ct_end + 1;
        if after_ct >= self.data.len() {
            return &[];
        }
        // Null terminator validated in constructor
        let ce_end = self.data[after_ct..].iter().position(|&b| b == 0).unwrap();
        &self.data[after_ct..after_ct + ce_end]
    }

    fn item_uri_type(&self) -> &[u8] {
        if self.version >= 2 && self.is_uri_type() {
            let after_name = self.after_item_name_offset();
            if after_name >= self.data.len() {
                return &[];
            }
            // Null terminator validated in constructor
            let end = self.data[after_name..].iter().position(|&b| b == 0).unwrap();
            &self.data[after_name..after_name + end]
        } else {
            &[]
        }
    }
}

impl std::fmt::Debug for ItemInfoEntryBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemInfoEntryBoxView")
            .field("version", &self.version())
            .field("item_id", &self.item_id())
            .field("item_type", &self.item_type())
            .field("item_name", &String::from_utf8_lossy(self.item_name()))
            .field("content_type", &String::from_utf8_lossy(self.content_type()))
            .field("content_encoding", &String::from_utf8_lossy(self.content_encoding()))
            .field("item_uri_type", &String::from_utf8_lossy(self.item_uri_type()))
            .finish()
    }
}

/// An owned representation of ItemInfoEntryBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemInfoEntryBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Item ID.
    pub item_id: u32,
    /// Item protection index.
    pub item_protection_index: u16,
    /// Item type (4cc, for version >= 2).
    pub item_type: FourCC,
    /// Item name.
    pub item_name: Vec<u8>,
    /// Content type (for v0/v1, or v>=2 with item_type='mime').
    pub content_type: Vec<u8>,
    /// Content encoding (optional, for v0/v1, or v>=2 with item_type='mime').
    pub content_encoding: Vec<u8>,
    /// Item URI type (for v>=2 with item_type='uri ').
    pub item_uri_type: Vec<u8>,
}

impl ItemInfoEntryBoxOwned {
    /// Creates a new ItemInfoEntryBoxOwned.
    pub fn new(item_id: u32, item_type: FourCC) -> Self {
        Self {
            flags: 0,
            item_id,
            item_protection_index: 0,
            item_type,
            item_name: Vec::new(),
            content_type: Vec::new(),
            content_encoding: Vec::new(),
            item_uri_type: Vec::new(),
        }
    }

    fn version(&self) -> u8 {
        if self.item_id > u16::MAX as u32 {
            3
        } else {
            2
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let version = self.version();
        let fixed_payload: usize = match version {
            2 => 8,  // item_id(2) + protection(2) + type(4)
            _ => 10, // item_id(4) + protection(2) + type(4)
        };
        // item_name + null terminator
        let mut payload = fixed_payload
            .checked_add(self.item_name.len())
            .and_then(|v| v.checked_add(1))
            .expect("overflow computing infe payload size");

        if self.item_type.0 == *b"mime" {
            // content_type + null terminator
            payload = payload
                .checked_add(self.content_type.len())
                .and_then(|v| v.checked_add(1))
                .expect("overflow computing infe payload size");
            // content_encoding + null terminator (always written; empty string if not present)
            payload = payload
                .checked_add(self.content_encoding.len())
                .and_then(|v| v.checked_add(1))
                .expect("overflow computing infe payload size");
        } else if self.item_type.0 == *b"uri " {
            // item_uri_type + null terminator
            payload = payload
                .checked_add(self.item_uri_type.len())
                .and_then(|v| v.checked_add(1))
                .expect("overflow computing infe payload size");
        }

        let payload = payload as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let version = self.version();
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, version, self.flags)?;

        if version >= 3 {
            writer.write_u32::<BigEndian>(self.item_id)?;
        } else {
            writer.write_u16::<BigEndian>(self.item_id as u16)?;
        }

        writer.write_u16::<BigEndian>(self.item_protection_index)?;
        writer.write_all(&self.item_type.0)?;
        writer.write_all(&self.item_name)?;
        writer.write_u8(0)?; // null terminator

        if self.item_type.0 == *b"mime" {
            writer.write_all(&self.content_type)?;
            writer.write_u8(0)?; // null terminator
            writer.write_all(&self.content_encoding)?;
            writer.write_u8(0)?; // null terminator
        } else if self.item_type.0 == *b"uri " {
            writer.write_all(&self.item_uri_type)?;
            writer.write_u8(0)?; // null terminator
        }

        Ok(())
    }
}

impl Default for ItemInfoEntryBoxOwned {
    fn default() -> Self {
        Self::new(1, FourCC(*b"mime"))
    }
}

impl ItemInfoEntryBox for ItemInfoEntryBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.version()
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn item_id(&self) -> u32 {
        self.item_id
    }

    fn item_protection_index(&self) -> u16 {
        self.item_protection_index
    }

    fn item_type(&self) -> Option<FourCC> {
        Some(self.item_type)
    }

    fn item_name(&self) -> &[u8] {
        &self.item_name
    }

    fn content_type(&self) -> &[u8] {
        &self.content_type
    }

    fn content_encoding(&self) -> &[u8] {
        &self.content_encoding
    }

    fn item_uri_type(&self) -> &[u8] {
        &self.item_uri_type
    }
}

impl<T: ItemInfoEntryBox> From<&T> for ItemInfoEntryBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            item_id: source.item_id(),
            item_protection_index: source.item_protection_index(),
            item_type: source.item_type().unwrap_or(FourCC(*b"\0\0\0\0")),
            item_name: source.item_name().to_vec(),
            content_type: source.content_type().to_vec(),
            content_encoding: source.content_encoding().to_vec(),
            item_uri_type: source.item_uri_type().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_infe_v2() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&21u32.to_be_bytes()); // size = 8 + 4 + 8 + 1
        data.extend_from_slice(b"infe");
        data.push(2); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1u16.to_be_bytes()); // item_id
        data.extend_from_slice(&0u16.to_be_bytes()); // item_protection_index
        data.extend_from_slice(b"hvc1"); // item_type
        data.push(0); // empty item_name with null terminator
        data
    }

    #[test]
    fn parse_infe_v2() {
        let data = make_infe_v2();
        let view = ItemInfoEntryBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 2);
        assert_eq!(view.item_id(), 1);
        assert_eq!(view.item_protection_index(), 0);
        assert_eq!(view.item_type(), Some(FourCC(*b"hvc1")));
        assert_eq!(view.item_name(), b"");
        assert_eq!(view.content_type(), b"");
        assert_eq!(view.content_encoding(), b"");
        assert_eq!(view.item_uri_type(), b"");
    }

    #[test]
    fn roundtrip_v2() {
        let data = make_infe_v2();
        let view = ItemInfoEntryBoxView::new(&data).unwrap();
        let owned = ItemInfoEntryBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    fn make_infe_v2_mime() -> Vec<u8> {
        let content_type = b"image/jpeg";
        let content_encoding = b"gzip";
        let item_name = b"photo";
        // size = 8(header) + 4(fullbox) + 2(item_id) + 2(prot_idx)
        //      + 4(item_type) + 6(item_name+null) + 11(content_type+null) + 5(content_encoding+null)
        //      = 42
        let size: u32 = 8 + 4 + 2 + 2 + 4 + (item_name.len() as u32 + 1)
            + (content_type.len() as u32 + 1)
            + (content_encoding.len() as u32 + 1);
        let mut data = Vec::new();
        data.extend_from_slice(&size.to_be_bytes());
        data.extend_from_slice(b"infe");
        data.push(2); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&5u16.to_be_bytes()); // item_id
        data.extend_from_slice(&0u16.to_be_bytes()); // item_protection_index
        data.extend_from_slice(b"mime"); // item_type
        data.extend_from_slice(item_name);
        data.push(0); // null terminator for item_name
        data.extend_from_slice(content_type);
        data.push(0); // null terminator for content_type
        data.extend_from_slice(content_encoding);
        data.push(0); // null terminator for content_encoding
        data
    }

    #[test]
    fn parse_infe_v2_mime() {
        let data = make_infe_v2_mime();
        let view = ItemInfoEntryBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 2);
        assert_eq!(view.item_id(), 5);
        assert_eq!(view.item_type(), Some(FourCC(*b"mime")));
        assert_eq!(view.item_name(), b"photo");
        assert_eq!(view.content_type(), b"image/jpeg");
        assert_eq!(view.content_encoding(), b"gzip");
        assert_eq!(view.item_uri_type(), b"");
    }

    #[test]
    fn roundtrip_v2_mime() {
        let data = make_infe_v2_mime();
        let view = ItemInfoEntryBoxView::new(&data).unwrap();
        let owned = ItemInfoEntryBoxOwned::from(&view);

        assert_eq!(owned.content_type, b"image/jpeg");
        assert_eq!(owned.content_encoding, b"gzip");

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    fn make_infe_v2_uri() -> Vec<u8> {
        let item_uri_type = b"http://example.com/type";
        let size: u32 = 8 + 4 + 2 + 2 + 4 + 1 + (item_uri_type.len() as u32 + 1);
        let mut data = Vec::new();
        data.extend_from_slice(&size.to_be_bytes());
        data.extend_from_slice(b"infe");
        data.push(2); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&3u16.to_be_bytes()); // item_id
        data.extend_from_slice(&0u16.to_be_bytes()); // item_protection_index
        data.extend_from_slice(b"uri "); // item_type
        data.push(0); // empty item_name with null terminator
        data.extend_from_slice(item_uri_type);
        data.push(0); // null terminator for item_uri_type
        data
    }

    #[test]
    fn parse_infe_v2_uri() {
        let data = make_infe_v2_uri();
        let view = ItemInfoEntryBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 2);
        assert_eq!(view.item_id(), 3);
        assert_eq!(view.item_type(), Some(FourCC(*b"uri ")));
        assert_eq!(view.item_name(), b"");
        assert_eq!(view.content_type(), b"");
        assert_eq!(view.content_encoding(), b"");
        assert_eq!(view.item_uri_type(), b"http://example.com/type");
    }

    #[test]
    fn roundtrip_v2_uri() {
        let data = make_infe_v2_uri();
        let view = ItemInfoEntryBoxView::new(&data).unwrap();
        let owned = ItemInfoEntryBoxOwned::from(&view);

        assert_eq!(owned.item_uri_type, b"http://example.com/type");

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn mime_no_encoding() {
        // mime type with content_type but empty content_encoding
        let content_type = b"text/plain";
        let size: u32 = 8 + 4 + 2 + 2 + 4 + 1 + (content_type.len() as u32 + 1) + 1;
        let mut data = Vec::new();
        data.extend_from_slice(&size.to_be_bytes());
        data.extend_from_slice(b"infe");
        data.push(2);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(b"mime");
        data.push(0); // empty item_name
        data.extend_from_slice(content_type);
        data.push(0); // null terminator for content_type
        data.push(0); // empty content_encoding with null terminator

        let view = ItemInfoEntryBoxView::new(&data).unwrap();
        assert_eq!(view.content_type(), b"text/plain");
        assert_eq!(view.content_encoding(), b"");

        let owned = ItemInfoEntryBoxOwned::from(&view);
        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }
}
