//! ISO-639-2/T language code packed as 3x5-bit values.
//!
//! Also supports legacy QuickTime language IDs which are converted to ISO codes.

use std::fmt;

/// A packed ISO-639-2/T language code stored in 16 bits.
///
/// The language code is stored as three 5-bit values representing characters
/// in the range 'a'-'z' (each character offset by 0x60). The first bit is
/// padding and should be 0.
///
/// For example, "eng" is stored as:
/// - 'e' - 0x60 = 5 (0b00101)
/// - 'n' - 0x60 = 14 (0b01110)
/// - 'g' - 0x60 = 7 (0b00111)
/// - Packed: 0b0_00101_01110_00111 = 0x15C7
///
/// Legacy QuickTime files may use numeric language IDs instead of ISO codes.
/// These are detected and converted when the decoded characters would be invalid.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IsoLanguageCode(u16);

impl IsoLanguageCode {
    /// Undetermined language code (ISO 639-2).
    pub const UNDETERMINED: Self = Self::from_chars(['u', 'n', 'd']);

    /// Creates a language code from a raw 16-bit value.
    #[inline]
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    /// Creates a language code from three characters.
    ///
    /// Each character should be in the range 'a'-'z'.
    #[inline]
    pub const fn from_chars(chars: [char; 3]) -> Self {
        let c0 = (chars[0] as u16).wrapping_sub(0x60) & 0x1F;
        let c1 = (chars[1] as u16).wrapping_sub(0x60) & 0x1F;
        let c2 = (chars[2] as u16).wrapping_sub(0x60) & 0x1F;
        Self((c0 << 10) | (c1 << 5) | c2)
    }

    /// Creates a language code from a 3-byte string.
    ///
    /// Returns `None` if the string is not exactly 3 characters or contains
    /// characters outside the range 'a'-'z'.
    pub fn parse(s: &str) -> Option<Self> {
        let bytes = s.as_bytes();
        if bytes.len() != 3 {
            return None;
        }
        for &b in bytes {
            if !b.is_ascii_lowercase() {
                return None;
            }
        }
        Some(Self::from_chars([
            bytes[0] as char,
            bytes[1] as char,
            bytes[2] as char,
        ]))
    }

    /// Returns the raw 16-bit representation.
    #[inline]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Decodes the language code to three characters.
    pub fn to_chars(self) -> [char; 3] {
        let c0 = (((self.0 >> 10) & 0x1F) + 0x60) as u8;
        let c1 = (((self.0 >> 5) & 0x1F) + 0x60) as u8;
        let c2 = ((self.0 & 0x1F) + 0x60) as u8;
        [c0 as char, c1 as char, c2 as char]
    }

    /// Returns true if this is a valid language code (all characters a-z).
    pub fn is_valid(&self) -> bool {
        let chars = self.to_chars();
        chars.iter().all(|&c| c.is_ascii_lowercase())
    }

    /// Returns true if this appears to be a QuickTime language ID rather than ISO code.
    ///
    /// QuickTime language IDs are small integers (typically < 150), while valid
    /// ISO 639-2/T codes have higher values due to the 5-bit character encoding.
    pub fn is_quicktime_id(&self) -> bool {
        // If the decoded characters include non-lowercase letters, it's likely a QT ID
        !self.is_valid()
    }

    /// Converts a QuickTime language ID to an ISO 639-2/T code.
    ///
    /// Returns the original code if no mapping is found.
    pub fn from_quicktime_id(id: u16) -> Self {
        // Common QuickTime language ID mappings
        // See: https://developer.apple.com/library/archive/documentation/QuickTime/QTFF/QTFFChap4/qtff4.html
        let iso_code = match id {
            0 => "eng",   // English
            1 => "fra",   // French
            2 => "deu",   // German
            3 => "ita",   // Italian
            4 => "nld",   // Dutch
            5 => "swe",   // Swedish
            6 => "spa",   // Spanish
            7 => "dan",   // Danish
            8 => "por",   // Portuguese
            9 => "nor",   // Norwegian
            10 => "heb",  // Hebrew
            11 => "jpn",  // Japanese
            12 => "ara",  // Arabic
            13 => "fin",  // Finnish
            14 => "ell",  // Greek
            15 => "isl",  // Icelandic
            16 => "mlt",  // Maltese
            17 => "tur",  // Turkish
            18 => "hrv",  // Croatian
            19 => "zho",  // Traditional Chinese
            20 => "urd",  // Urdu
            21 => "hin",  // Hindi
            22 => "tha",  // Thai
            23 => "kor",  // Korean
            24 => "lit",  // Lithuanian
            25 => "pol",  // Polish
            26 => "hun",  // Hungarian
            27 => "est",  // Estonian
            28 => "lav",  // Latvian
            29 => "smi",  // Sami
            30 => "fao",  // Faroese
            31 => "fas",  // Farsi/Persian
            32 => "rus",  // Russian
            33 => "zho",  // Simplified Chinese
            57 => "slk",  // Slovak
            85 => "hin",  // Hindi (another mapping)
            _ => return Self::from_raw(id),
        };

        Self::parse(iso_code).unwrap_or(Self::from_raw(id))
    }

    /// Decodes the language code, automatically detecting and converting
    /// QuickTime language IDs to ISO codes.
    pub fn to_string_auto(&self) -> String {
        // Raw value 0 means undetermined per ISO spec
        if self.0 == 0 {
            return "und".to_string();
        }
        if self.is_quicktime_id() {
            Self::from_quicktime_id(self.0).to_string()
        } else {
            self.to_string()
        }
    }
}

impl From<u16> for IsoLanguageCode {
    fn from(raw: u16) -> Self {
        Self(raw)
    }
}

impl From<IsoLanguageCode> for u16 {
    fn from(code: IsoLanguageCode) -> Self {
        code.0
    }
}

impl fmt::Debug for IsoLanguageCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let chars = self.to_chars();
        write!(
            f,
            "IsoLanguageCode(0x{:04X} = \"{}{}{}\")",
            self.0, chars[0], chars[1], chars[2]
        )
    }
}

impl fmt::Display for IsoLanguageCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let chars = self.to_chars();
        write!(f, "{}{}{}", chars[0], chars[1], chars[2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_eng() {
        let lang = IsoLanguageCode::from_chars(['e', 'n', 'g']);
        assert_eq!(lang.raw(), 0x15C7);
    }

    #[test]
    fn decode_eng() {
        let lang = IsoLanguageCode::from_raw(0x15C7);
        assert_eq!(lang.to_chars(), ['e', 'n', 'g']);
    }

    #[test]
    fn roundtrip() {
        let original = ['j', 'p', 'n'];
        let lang = IsoLanguageCode::from_chars(original);
        assert_eq!(lang.to_chars(), original);
    }

    #[test]
    fn parse_valid() {
        let lang = IsoLanguageCode::parse("fra").unwrap();
        assert_eq!(lang.to_chars(), ['f', 'r', 'a']);
    }

    #[test]
    fn parse_invalid() {
        assert!(IsoLanguageCode::parse("EN").is_none());
        assert!(IsoLanguageCode::parse("english").is_none());
        assert!(IsoLanguageCode::parse("").is_none());
    }

    #[test]
    fn undetermined() {
        let und = IsoLanguageCode::UNDETERMINED;
        assert_eq!(und.to_chars(), ['u', 'n', 'd']);
    }
}
