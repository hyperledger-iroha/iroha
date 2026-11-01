//! Bech32 (BIP-173) and Bech32m (BIP-350) strings.
//!
//! Contract addresses are Bech32m strings and I105 account addresses carry a
//! Bech32m checksum. Decoding accepts either checksum, preserves the spelling of
//! the human-readable part (HRP) and drops trailing padding bits without
//! inspecting them. The borrowed decoder never allocates.

use core::{fmt, slice};

/// Checksum algorithm of a Bech32-family string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Variant {
    /// BIP-173 Bech32.
    Bech32,
    /// BIP-350 Bech32m.
    Bech32m,
}

impl Variant {
    /// Polymod residue that a valid checksum of this variant leaves.
    const fn target_residue(self) -> u32 {
        match self {
            Self::Bech32 => 1,
            Self::Bech32m => 0x2bc8_30a3,
        }
    }
}

/// Number of checksum characters at the end of every string.
pub(crate) const CHECKSUM_LEN: usize = 6;
/// Longest accepted string, in bytes, for both variants.
const CODE_LENGTH: usize = 1023;
/// Longest accepted HRP, in bytes.
const MAX_HRP_LEN: usize = 83;
/// Separator between the HRP and the data part; the last one in a string wins.
const SEPARATOR: char = '1';
/// Data characters in value order.
const CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
/// Marks an ASCII byte that is not a data character in [`CHARSET_VALUES`].
const NOT_A_VALUE: u8 = 0xFF;
/// Five-bit value of every ASCII byte in either case, or [`NOT_A_VALUE`].
const CHARSET_VALUES: [u8; 128] = {
    let mut values = [NOT_A_VALUE; 128];
    let mut value = 0;
    while value < CHARSET.len() {
        let lower = CHARSET[value];
        values[lower as usize] = value as u8;
        values[lower.to_ascii_uppercase() as usize] = value as u8;
        value += 1;
    }
    values
};
/// Generator coefficients of the BCH code shared by both variants.
const GENERATOR: [u32; 5] = [
    0x3b6a_57b2,
    0x2650_8e6d,
    0x1ea1_19fa,
    0x3d42_33dd,
    0x2a14_62b3,
];

/// Five-bit value of a data character in either case.
fn char_value(character: char) -> Option<u8> {
    let value = *CHARSET_VALUES.get(usize::try_from(u32::from(character)).ok()?)?;
    (value != NOT_A_VALUE).then_some(value)
}

/// Running BCH checksum over an expanded HRP and five-bit values.
#[derive(Clone, Copy)]
struct Polymod(u32);

impl Polymod {
    /// Start a checksum over `hrp`, lowercased as both variants require.
    fn with_hrp(hrp: &str) -> Self {
        let mut polymod = Self(1);
        for byte in hrp.bytes() {
            polymod.input(byte.to_ascii_lowercase() >> 5);
        }
        polymod.input(0);
        for byte in hrp.bytes() {
            polymod.input(byte.to_ascii_lowercase() & 0x1f);
        }
        polymod
    }

    /// Feed one five-bit value.
    fn input(&mut self, value: u8) {
        let top = self.0 >> 25;
        self.0 = ((self.0 & 0x01ff_ffff) << 5) ^ u32::from(value);
        for (index, generator) in GENERATOR.iter().enumerate() {
            if (top >> index) & 1 == 1 {
                self.0 ^= generator;
            }
        }
    }
}

/// Checksum values (each below 32) that the encoder appends to `hrp` and `data`.
pub(crate) fn create_checksum(
    variant: Variant,
    hrp: &str,
    data: impl IntoIterator<Item = u8>,
) -> [u8; CHECKSUM_LEN] {
    let mut polymod = Polymod::with_hrp(hrp);
    for value in data {
        polymod.input(value);
    }
    for _ in 0..CHECKSUM_LEN {
        polymod.input(0);
    }
    let residue = polymod.0 ^ variant.target_residue();
    let mut checksum = [0; CHECKSUM_LEN];
    for (index, value) in checksum.iter_mut().enumerate() {
        *value = ((residue >> (5 * (CHECKSUM_LEN - 1 - index))) & 0x1f) as u8;
    }
    checksum
}

/// Why an HRP was rejected; messages match the reference implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HrpError {
    /// The HRP has no characters.
    Empty,
    /// The HRP has more than [`MAX_HRP_LEN`] bytes.
    TooLong(usize),
    /// The HRP contains a non-ASCII character.
    NonAsciiChar(char),
    /// The HRP contains an ASCII byte outside `33..=126`.
    InvalidAsciiByte(u8),
    /// The HRP mixes upper and lower case letters.
    MixedCase,
}

impl fmt::Display for HrpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Empty => f.write_str("hrp is empty, must have at least 1 character"),
            Self::TooLong(len) => write!(
                f,
                "hrp is too long, found {len} characters, must be <= {MAX_HRP_LEN}"
            ),
            Self::NonAsciiChar(character) => write!(f, "found non-ASCII character: {character}"),
            Self::InvalidAsciiByte(byte) => {
                write!(f, "byte value is not valid US-ASCII: '{byte:x}'")
            }
            Self::MixedCase => f.write_str("hrp cannot mix upper and lower case"),
        }
    }
}

impl std::error::Error for HrpError {}

/// Validated human-readable part, borrowed with its original spelling.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Hrp<'a>(&'a str);

impl<'a> Hrp<'a> {
    /// Validate `hrp`: 1 to 83 printable ASCII characters in a single case.
    ///
    /// # Errors
    /// Returns the first violated rule, in the reference implementation's order.
    pub(crate) fn parse(hrp: &'a str) -> Result<Self, HrpError> {
        if hrp.is_empty() {
            return Err(HrpError::Empty);
        }
        if hrp.len() > MAX_HRP_LEN {
            return Err(HrpError::TooLong(hrp.len()));
        }
        let mut has_lower = false;
        let mut has_upper = false;
        for character in hrp.chars() {
            if !character.is_ascii() {
                return Err(HrpError::NonAsciiChar(character));
            }
            let byte = character as u8;
            if !(33..=126).contains(&byte) {
                return Err(HrpError::InvalidAsciiByte(byte));
            }
            if byte.is_ascii_lowercase() {
                if has_upper {
                    return Err(HrpError::MixedCase);
                }
                has_lower = true;
            } else if byte.is_ascii_uppercase() {
                if has_lower {
                    return Err(HrpError::MixedCase);
                }
                has_upper = true;
            }
        }
        Ok(Self(hrp))
    }

    /// The HRP exactly as spelled in its source.
    pub(crate) fn as_str(self) -> &'a str {
        self.0
    }
}

impl fmt::Display for Hrp<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Why a string failed to decode; messages match the reference implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecodeError {
    /// Bad characters, mixed case, no separator, or an invalid HRP.
    Parse,
    /// The string is too long, too short for a checksum, or neither checksum verifies.
    Checksum,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Parse => "parsing failed",
            Self::Checksum => "no valid bech32 or bech32m checksum",
        })
    }
}

impl std::error::Error for DecodeError {}

/// A checksum-verified string borrowed from its source text.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Decoded<'a> {
    hrp: Hrp<'a>,
    /// Data characters without the checksum.
    data: &'a [u8],
}

impl<'a> Decoded<'a> {
    /// The HRP exactly as spelled in the source.
    pub(crate) fn hrp(&self) -> Hrp<'a> {
        self.hrp
    }

    /// Payload bytes; trailing bits that do not fill a byte are dropped.
    pub(crate) fn bytes(&self) -> DataBytes<'a> {
        DataBytes {
            characters: self.data.iter(),
            buffer: 0,
            bits: 0,
        }
    }
}

/// Split and verify `text` without allocating.
///
/// Accepts a valid Bech32m or Bech32 checksum, as the reference decoder does.
///
/// # Errors
/// [`DecodeError::Parse`] for malformed text and [`DecodeError::Checksum`] for a
/// string longer than 1023 bytes, a data part shorter than the checksum, or a
/// checksum that verifies under neither variant.
pub(crate) fn decode_borrowed(text: &str) -> Result<Decoded<'_>, DecodeError> {
    let mut separator = None;
    let mut has_upper = false;
    let mut has_lower = false;
    for (index, character) in text.char_indices().rev() {
        if separator.is_none() {
            if character == SEPARATOR {
                separator = Some(index);
            } else if char_value(character).is_none() {
                return Err(DecodeError::Parse);
            }
        }
        if character.is_ascii_uppercase() {
            has_upper = true;
        } else if character.is_ascii_lowercase() {
            has_lower = true;
        }
    }
    if has_upper && has_lower {
        return Err(DecodeError::Parse);
    }
    let separator = separator.ok_or(DecodeError::Parse)?;
    let hrp = Hrp::parse(&text[..separator]).map_err(|_| DecodeError::Parse)?;
    let data = &text.as_bytes()[separator + 1..];
    if text.len() > CODE_LENGTH || data.len() < CHECKSUM_LEN {
        return Err(DecodeError::Checksum);
    }
    let mut polymod = Polymod::with_hrp(hrp.as_str());
    for &character in data {
        polymod.input(CHARSET_VALUES[usize::from(character)]);
    }
    if polymod.0 != Variant::Bech32m.target_residue()
        && polymod.0 != Variant::Bech32.target_residue()
    {
        return Err(DecodeError::Checksum);
    }
    Ok(Decoded {
        hrp,
        data: &data[..data.len() - CHECKSUM_LEN],
    })
}

/// Decode `text` into its HRP and payload bytes.
///
/// # Errors
/// As [`decode_borrowed`].
pub(crate) fn decode(text: &str) -> Result<(Hrp<'_>, Vec<u8>), DecodeError> {
    let decoded = decode_borrowed(text)?;
    Ok((decoded.hrp(), decoded.bytes().collect()))
}

/// Why a string could not be encoded; the message matches the reference implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EncodeError {
    /// The encoded string would exceed 1023 bytes.
    TooLong,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("encode error")
    }
}

impl std::error::Error for EncodeError {}

/// Encode `data` under `hrp` as a lowercase string with a `variant` checksum.
///
/// # Errors
/// [`EncodeError::TooLong`] when the string would exceed 1023 bytes.
pub(crate) fn encode(variant: Variant, hrp: Hrp<'_>, data: &[u8]) -> Result<String, EncodeError> {
    let values = BytesToValues::new(data);
    let len = hrp.as_str().len() + 1 + values.len() + CHECKSUM_LEN;
    if len > CODE_LENGTH {
        return Err(EncodeError::TooLong);
    }
    let mut text = String::with_capacity(len);
    text.extend(
        hrp.as_str()
            .chars()
            .map(|character| character.to_ascii_lowercase()),
    );
    text.push(SEPARATOR);
    text.extend(
        values
            .clone()
            .map(|value| char::from(CHARSET[usize::from(value)])),
    );
    let checksum = create_checksum(variant, hrp.as_str(), values);
    text.extend(checksum.map(|value| char::from(CHARSET[usize::from(value)])));
    Ok(text)
}

/// Payload bytes of verified data characters (five bits each).
#[derive(Clone, Debug)]
pub(crate) struct DataBytes<'a> {
    characters: slice::Iter<'a, u8>,
    buffer: u32,
    bits: u32,
}

impl Iterator for DataBytes<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        while self.bits < 8 {
            let character = *self.characters.next()?;
            self.buffer = (self.buffer << 5) | u32::from(CHARSET_VALUES[usize::from(character)]);
            self.bits += 5;
        }
        self.bits -= 8;
        let byte = (self.buffer >> self.bits) as u8;
        self.buffer &= (1 << self.bits) - 1;
        Some(byte)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = (self.characters.len() * 5 + self.bits as usize) / 8;
        (len, Some(len))
    }
}

impl ExactSizeIterator for DataBytes<'_> {}

/// Five-bit values of bytes, with the final value zero-padded.
#[derive(Clone, Debug)]
pub(crate) struct BytesToValues<'a> {
    bytes: slice::Iter<'a, u8>,
    buffer: u32,
    bits: u32,
}

impl<'a> BytesToValues<'a> {
    /// Values of `bytes`, eight bits regrouped into five.
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes: bytes.iter(),
            buffer: 0,
            bits: 0,
        }
    }
}

impl Iterator for BytesToValues<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        if self.bits < 5 {
            match self.bytes.next() {
                Some(&byte) => {
                    self.buffer = (self.buffer << 8) | u32::from(byte);
                    self.bits += 8;
                }
                None if self.bits == 0 => return None,
                None => {
                    let value = (self.buffer << (5 - self.bits)) & 0x1f;
                    self.bits = 0;
                    self.buffer = 0;
                    return Some(value as u8);
                }
            }
        }
        self.bits -= 5;
        let value = (self.buffer >> self.bits) & 0x1f;
        self.buffer &= (1 << self.bits) - 1;
        Some(value as u8)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = (self.bytes.len() * 8 + self.bits as usize).div_ceil(5);
        (len, Some(len))
    }
}

impl ExactSizeIterator for BytesToValues<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    // BIP-173 valid Bech32 and BIP-350 valid Bech32m strings.
    const VALID: &[&str] = &[
        "A12UEL5L",
        "a12uel5l",
        "an83characterlonghumanreadablepartthatcontainsthenumber1andtheexcludedcharactersbio1tt5tgs",
        "abcdef1qpzry9x8gf2tvdw0s3jn54khce6mua7lmqqqxw",
        "11qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqc8247j",
        "split1checkupstagehandshakeupstreamerranterredcaperred2y9e3w",
        "?1ezyfcl",
        "A1LQFN3A",
        "a1lqfn3a",
        "an83characterlonghumanreadablepartthatcontainsthetheexcludedcharactersbioandnumber11sg7hg6",
        "abcdef1l7aum6echk45nj3s0wdvt2fg8x9yrzpqzd3ryx",
        "11llllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllludsr8",
        "split1checkupstagehandshakeupstreamerranterredcaperredlc445v",
        "?1v759aa",
    ];

    // BIP-173 and BIP-350 invalid strings with the reference rejection class.
    const INVALID: &[(&str, DecodeError)] = &[
        ("\u{20}1nwldj5", DecodeError::Parse),
        ("\u{7f}1axkwrx", DecodeError::Parse),
        ("\u{80}1eym55h", DecodeError::Parse),
        (
            "an84characterslonghumanreadablepartthatcontainsthenumber1andtheexcludedcharactersbio1569pvx",
            DecodeError::Parse,
        ),
        ("pzry9x0s0muk", DecodeError::Parse),
        ("1pzry9x0s0muk", DecodeError::Parse),
        ("x1b4n0q5v", DecodeError::Parse),
        ("li1dgmt3", DecodeError::Checksum),
        ("de1lg7wt\u{ff}", DecodeError::Parse),
        ("A1G7SGD8", DecodeError::Checksum),
        ("10a06t8", DecodeError::Parse),
        ("1qzzfhee", DecodeError::Parse),
        ("M1VUXWEZ", DecodeError::Checksum),
        ("in1muywd", DecodeError::Checksum),
        ("mm1crxm3i", DecodeError::Parse),
        ("au1s5cgom", DecodeError::Parse),
        ("qyrz8wqd2c9m", DecodeError::Parse),
        ("y1b0jsk6g", DecodeError::Parse),
        ("lt1igcx5c0", DecodeError::Parse),
        (
            "Abcdef1qpzry9x8gf2tvdw0s3jn54khce6mua7lmqqqxw",
            DecodeError::Parse,
        ),
    ];

    #[test]
    fn bip_vectors_decode_with_reference_classification() {
        for text in VALID {
            let decoded = decode_borrowed(text).unwrap_or_else(|err| panic!("{text}: {err}"));
            let separator = text.rfind('1').expect("separator");
            assert_eq!(decoded.hrp().as_str(), &text[..separator]);
        }
        for (text, expected) in INVALID {
            assert_eq!(decode_borrowed(text).err(), Some(*expected), "{text:?}");
        }
        assert_eq!(DecodeError::Parse.to_string(), "parsing failed");
        assert_eq!(
            DecodeError::Checksum.to_string(),
            "no valid bech32 or bech32m checksum"
        );
    }

    #[test]
    fn encode_reproduces_bip_checksums() {
        let hrp = |text| Hrp::parse(text).expect("valid hrp");
        assert_eq!(
            encode(Variant::Bech32, hrp("a"), &[]).as_deref(),
            Ok("a12uel5l")
        );
        assert_eq!(
            encode(Variant::Bech32m, hrp("A"), &[]).as_deref(),
            Ok("a1lqfn3a")
        );
        let ascending: Vec<u8> = (0..32).collect();
        let descending: Vec<u8> = (0..32).rev().collect();
        let render = |values: &[u8], checksum: [u8; CHECKSUM_LEN]| -> String {
            values
                .iter()
                .chain(checksum.iter())
                .map(|value| char::from(CHARSET[usize::from(*value)]))
                .collect()
        };
        assert_eq!(
            render(
                &ascending,
                create_checksum(Variant::Bech32, "abcdef", ascending.iter().copied())
            ),
            "qpzry9x8gf2tvdw0s3jn54khce6mua7lmqqqxw"
        );
        assert_eq!(
            render(
                &descending,
                create_checksum(Variant::Bech32m, "abcdef", descending.iter().copied())
            ),
            "l7aum6echk45nj3s0wdvt2fg8x9yrzpqzd3ryx"
        );
        let too_long = vec![0_u8; 700];
        assert_eq!(
            encode(Variant::Bech32m, hrp("a"), &too_long),
            Err(EncodeError::TooLong)
        );
        assert_eq!(EncodeError::TooLong.to_string(), "encode error");
    }

    #[test]
    fn decode_round_trips_payload_bytes_and_ignores_padding_bits() {
        let hrp = Hrp::parse("irohac").expect("valid hrp");
        let payload: Vec<u8> = (0..29).collect();
        for variant in [Variant::Bech32m, Variant::Bech32] {
            let text = encode(variant, hrp, &payload).expect("encodable");
            assert_eq!(text.len(), 60);
            let (decoded_hrp, bytes) = decode(&text).expect("round trip");
            assert_eq!(decoded_hrp.as_str(), "irohac");
            assert_eq!(bytes, payload);
            let upper = text.to_ascii_uppercase();
            let decoded = decode_borrowed(&upper).expect("single-case upper text");
            assert_eq!(decoded.hrp().to_string(), "IROHAC");
            assert_eq!(decoded.bytes().len(), 29);
            assert!(decoded.bytes().eq(payload.iter().copied()));
        }
        // 47 values carry 29 bytes and three padding bits, which decoding drops.
        let mut values: Vec<u8> = BytesToValues::new(&payload).collect();
        *values.last_mut().expect("values") |= 0b111;
        let checksum = create_checksum(Variant::Bech32m, "irohac", values.iter().copied());
        let text: String = "irohac1"
            .chars()
            .chain(
                values
                    .iter()
                    .chain(&checksum)
                    .map(|v| char::from(CHARSET[usize::from(*v)])),
            )
            .collect();
        assert_eq!(decode(&text).map(|(_, bytes)| bytes), Ok(payload));
    }

    #[test]
    fn hrp_rules_and_messages_match_reference() {
        assert_eq!(Hrp::parse("irohac").map(Hrp::as_str), Ok("irohac"));
        assert_eq!(
            Hrp::parse("IROHAC").map(|hrp| hrp.to_string()),
            Ok("IROHAC".to_owned())
        );
        let cases = [
            ("", "hrp is empty, must have at least 1 character"),
            (
                &"a".repeat(84),
                "hrp is too long, found 84 characters, must be <= 83",
            ),
            ("ab\u{e9}", "found non-ASCII character: \u{e9}"),
            ("a b", "byte value is not valid US-ASCII: '20'"),
            ("aB", "hrp cannot mix upper and lower case"),
        ];
        for (text, message) in cases {
            assert_eq!(
                Hrp::parse(text)
                    .map_err(|err| err.to_string())
                    .err()
                    .as_deref(),
                Some(message)
            );
        }
    }

    #[test]
    fn value_conversions_regroup_bits() {
        assert_eq!(
            BytesToValues::new(&[]).collect::<Vec<_>>(),
            Vec::<u8>::new()
        );
        assert_eq!(BytesToValues::new(&[0x00]).collect::<Vec<_>>(), vec![0, 0]);
        assert_eq!(
            BytesToValues::new(&[0xff]).collect::<Vec<_>>(),
            vec![31, 28]
        );
        assert_eq!(BytesToValues::new(&[0xff; 5]).len(), 8);
        let decoded = decode_borrowed("a1llllllcy5v0gm").expect("valid five-byte payload");
        assert_eq!(decoded.bytes().collect::<Vec<_>>(), vec![0xff; 4]);
        assert_eq!(char_value('Q'), Some(0));
        assert_eq!(char_value('l'), Some(31));
        assert_eq!(char_value('b'), None);
        assert_eq!(char_value('\u{e9}'), None);
    }
}
