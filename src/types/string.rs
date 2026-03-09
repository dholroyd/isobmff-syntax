//! Null-terminated string handling for ISOBMFF structures.

use std::fmt;

/// A null-terminated string as used in various ISOBMFF boxes.
///
/// This type wraps a byte slice that ends with a null terminator.
/// The string content is expected to be valid UTF-8, though this
/// is not strictly enforced during parsing.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Default)]
pub struct NullTerminatedString<'a> {
    /// The string content without the null terminator.
    data: &'a [u8],
}

impl<'a> NullTerminatedString<'a> {
    /// Creates a new null-terminated string from bytes.
    ///
    /// The slice should NOT include the null terminator.
    #[inline]
    pub const fn from_bytes(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Parses a null-terminated string from a byte slice.
    ///
    /// Returns the string and the number of bytes consumed (including the null terminator).
    /// Returns `None` if no null terminator is found.
    pub fn parse(data: &'a [u8]) -> Option<(Self, usize)> {
        let pos = data.iter().position(|&b| b == 0)?;
        Some((Self::from_bytes(&data[..pos]), pos + 1))
    }

    /// Returns the string content as bytes (without null terminator).
    #[inline]
    pub const fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the string as a UTF-8 string slice.
    ///
    /// Returns `None` if the content is not valid UTF-8.
    #[inline]
    pub fn as_str(&self) -> Option<&'a str> {
        std::str::from_utf8(self.data).ok()
    }

    /// Returns the length of the string (not including null terminator).
    #[inline]
    pub const fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the string is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the total bytes needed including null terminator.
    #[inline]
    pub const fn encoded_len(&self) -> usize {
        self.data.len() + 1
    }
}


impl fmt::Debug for NullTerminatedString<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_str() {
            Some(s) => write!(f, "NullTerminatedString({:?})", s),
            None => write!(f, "NullTerminatedString({:?})", self.data),
        }
    }
}

impl fmt::Display for NullTerminatedString<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_str() {
            Some(s) => write!(f, "{}", s),
            None => write!(f, "{}", String::from_utf8_lossy(self.data)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_null_terminated() {
        let data = b"hello\0world";
        let (s, len) = NullTerminatedString::parse(data).unwrap();
        assert_eq!(s.as_str(), Some("hello"));
        assert_eq!(len, 6);
    }

    #[test]
    fn parse_empty() {
        let data = b"\0";
        let (s, len) = NullTerminatedString::parse(data).unwrap();
        assert!(s.is_empty());
        assert_eq!(len, 1);
    }

    #[test]
    fn parse_no_terminator() {
        let data = b"hello";
        assert!(NullTerminatedString::parse(data).is_none());
    }
}
