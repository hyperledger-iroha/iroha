//! TL (Type Language) primitives of the TON ADNL and `liteServer.*` schemas
//! (spec §7.2, §8).
//!
//! TL is little-endian: `int` is 4 bytes, `long` 8 bytes, `int256` 32 raw
//! bytes. `bytes` and `string` carry a length prefix (one byte below 254,
//! `0xFE` and a 3-byte length below 2^24, `0xFF` and a 7-byte length above)
//! and are zero-padded to a multiple of 4 bytes including the prefix. Boxed
//! values start with the 4-byte constructor id, the IEEE CRC-32 of the
//! normalized schema line. `Bool` is the boxed `boolTrue` or `boolFalse`.
//!
//! [`TlReader`] mirrors the reference parser: padding bytes are skipped
//! unchecked, and a message must be consumed exactly ([`TlReader::finish`]).
//! Every length is checked against the remaining input before anything is
//! allocated, so a hostile liteserver cannot make the reader allocate more than
//! it sent.

use std::fmt;

/// `boolTrue = Bool`.
pub const BOOL_TRUE: u32 = 0x9972_75b5;
/// `boolFalse = Bool`.
pub const BOOL_FALSE: u32 = 0xbc79_9737;

/// Largest length a one-byte `bytes` prefix encodes.
const SHORT_LENGTH_LIMIT: usize = 254;
/// Prefix byte of a 3-byte `bytes` length.
const MEDIUM_LENGTH_TAG: u8 = 0xFE;
/// Prefix byte of a 7-byte `bytes` length.
const LONG_LENGTH_TAG: u8 = 0xFF;
/// Lengths from here on need the 7-byte form.
const MEDIUM_LENGTH_LIMIT: usize = 1 << 24;

/// Why a TL message could not be read or written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlError {
    /// Byte offset in the message where reading failed.
    pub offset: usize,
    /// What is wrong.
    pub kind: TlErrorKind,
}

/// What is wrong with a TL message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlErrorKind {
    /// The message ends before the value.
    Truncated {
        /// Bytes the value needs.
        needed: usize,
        /// Bytes left.
        remaining: usize,
    },
    /// A boxed value starts with another constructor.
    UnexpectedConstructor {
        /// What the schema allows here.
        expected: &'static str,
        /// The constructor id found.
        found: u32,
    },
    /// A vector announces more elements than the message can hold or the
    /// caller accepts.
    VectorTooLong {
        /// Announced element count.
        count: u32,
        /// Largest accepted count.
        limit: usize,
    },
    /// A `string` is not UTF-8.
    InvalidUtf8,
    /// Bytes are left after the value.
    TrailingBytes {
        /// Unread bytes.
        remaining: usize,
    },
    /// A value is too long for the TL length prefix.
    TooLong {
        /// The value length.
        length: usize,
    },
}

impl fmt::Display for TlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let offset = self.offset;
        match &self.kind {
            TlErrorKind::Truncated { needed, remaining } => write!(
                formatter,
                "TL message truncated at byte {offset}: {needed} bytes needed, {remaining} left"
            ),
            TlErrorKind::UnexpectedConstructor { expected, found } => write!(
                formatter,
                "unexpected TL constructor {found:#010x} at byte {offset}; expected {expected}"
            ),
            TlErrorKind::VectorTooLong { count, limit } => write!(
                formatter,
                "TL vector at byte {offset} announces {count} elements; at most {limit} fit"
            ),
            TlErrorKind::InvalidUtf8 => {
                write!(formatter, "TL string at byte {offset} is not UTF-8")
            }
            TlErrorKind::TrailingBytes { remaining } => write!(
                formatter,
                "{remaining} unread bytes after the TL value at byte {offset}"
            ),
            TlErrorKind::TooLong { length } => {
                write!(
                    formatter,
                    "a {length}-byte value exceeds the TL length prefix"
                )
            }
        }
    }
}

impl std::error::Error for TlError {}

/// Serializes TL values in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TlWriter {
    buffer: Vec<u8>,
}

impl TlWriter {
    /// An empty message.
    pub fn new() -> Self {
        Self::default()
    }

    /// A message starting with the boxed constructor `id`.
    pub fn boxed(id: u32) -> Self {
        let mut writer = Self::new();
        writer.u32(id);
        writer
    }

    /// Appends a 4-byte little-endian unsigned `int` (or constructor id).
    pub fn u32(&mut self, value: u32) -> &mut Self {
        self.buffer.extend_from_slice(&value.to_le_bytes());
        self
    }

    /// Appends a 4-byte little-endian signed `int`.
    pub fn i32(&mut self, value: i32) -> &mut Self {
        self.buffer.extend_from_slice(&value.to_le_bytes());
        self
    }

    /// Appends an 8-byte little-endian unsigned `long`.
    pub fn u64(&mut self, value: u64) -> &mut Self {
        self.buffer.extend_from_slice(&value.to_le_bytes());
        self
    }

    /// Appends an 8-byte little-endian signed `long`.
    pub fn i64(&mut self, value: i64) -> &mut Self {
        self.buffer.extend_from_slice(&value.to_le_bytes());
        self
    }

    /// Appends a raw `int256`.
    pub fn int256(&mut self, value: &[u8; 32]) -> &mut Self {
        self.buffer.extend_from_slice(value);
        self
    }

    /// Appends a boxed `Bool`.
    pub fn bool(&mut self, value: bool) -> &mut Self {
        self.u32(if value { BOOL_TRUE } else { BOOL_FALSE })
    }

    /// Appends length-prefixed, zero-padded `bytes`.
    ///
    /// # Errors
    /// [`TlErrorKind::TooLong`] for values of 2^56 bytes or more.
    pub fn bytes(&mut self, value: &[u8]) -> Result<&mut Self, TlError> {
        let start = self.buffer.len();
        let length = value.len();
        if length < SHORT_LENGTH_LIMIT {
            self.buffer.push(u8::try_from(length).unwrap_or(u8::MAX));
        } else if length < MEDIUM_LENGTH_LIMIT {
            self.buffer.push(MEDIUM_LENGTH_TAG);
            self.buffer.extend_from_slice(&length.to_le_bytes()[..3]);
        } else if u64::try_from(length).is_ok_and(|length| length < 1 << 56) {
            self.buffer.push(LONG_LENGTH_TAG);
            self.buffer
                .extend_from_slice(&(length as u64).to_le_bytes()[..7]);
        } else {
            return Err(TlError {
                offset: start,
                kind: TlErrorKind::TooLong { length },
            });
        }
        self.buffer.extend_from_slice(value);
        let padding = (4 - (self.buffer.len() - start) % 4) % 4;
        self.buffer.extend(std::iter::repeat_n(0, padding));
        Ok(self)
    }

    /// Appends already serialized TL.
    pub fn raw(&mut self, value: &[u8]) -> &mut Self {
        self.buffer.extend_from_slice(value);
        self
    }

    /// Bytes written so far.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Whether nothing was written.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// The serialized message.
    pub fn finish(self) -> Vec<u8> {
        self.buffer
    }
}

/// Reads TL values in order from one message.
#[derive(Debug, Clone)]
pub struct TlReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> TlReader<'a> {
    /// A reader at the start of `data`.
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    /// Current byte offset.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Bytes not read yet.
    pub fn remaining(&self) -> usize {
        self.data.len() - self.offset
    }

    fn error(&self, kind: TlErrorKind) -> TlError {
        TlError {
            offset: self.offset,
            kind,
        }
    }

    /// Takes the next `length` bytes.
    ///
    /// # Errors
    /// [`TlErrorKind::Truncated`] if fewer are left.
    pub fn take(&mut self, length: usize) -> Result<&'a [u8], TlError> {
        let remaining = self.remaining();
        if length > remaining {
            return Err(self.error(TlErrorKind::Truncated {
                needed: length,
                remaining,
            }));
        }
        let value = &self.data[self.offset..self.offset + length];
        self.offset += length;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], TlError> {
        let mut out = [0_u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    /// Reads a 4-byte unsigned `int` (or constructor id).
    ///
    /// # Errors
    /// If the message is truncated.
    pub fn u32(&mut self) -> Result<u32, TlError> {
        self.array().map(u32::from_le_bytes)
    }

    /// Reads a 4-byte signed `int`.
    ///
    /// # Errors
    /// If the message is truncated.
    pub fn i32(&mut self) -> Result<i32, TlError> {
        self.array().map(i32::from_le_bytes)
    }

    /// Reads an 8-byte unsigned `long`.
    ///
    /// # Errors
    /// If the message is truncated.
    pub fn u64(&mut self) -> Result<u64, TlError> {
        self.array().map(u64::from_le_bytes)
    }

    /// Reads an 8-byte signed `long`.
    ///
    /// # Errors
    /// If the message is truncated.
    pub fn i64(&mut self) -> Result<i64, TlError> {
        self.array().map(i64::from_le_bytes)
    }

    /// Reads an `int256`.
    ///
    /// # Errors
    /// If the message is truncated.
    pub fn int256(&mut self) -> Result<[u8; 32], TlError> {
        self.array()
    }

    /// Reads a constructor id and checks it is `id`.
    ///
    /// # Errors
    /// If the message is truncated or another constructor follows.
    pub fn expect_constructor(&mut self, id: u32, name: &'static str) -> Result<(), TlError> {
        let start = self.offset;
        let found = self.u32()?;
        if found == id {
            Ok(())
        } else {
            Err(TlError {
                offset: start,
                kind: TlErrorKind::UnexpectedConstructor {
                    expected: name,
                    found,
                },
            })
        }
    }

    /// Reads a boxed `Bool`.
    ///
    /// # Errors
    /// If the message is truncated or the value is neither `boolTrue` nor
    /// `boolFalse`.
    pub fn bool(&mut self) -> Result<bool, TlError> {
        let start = self.offset;
        match self.u32()? {
            BOOL_TRUE => Ok(true),
            BOOL_FALSE => Ok(false),
            found => Err(TlError {
                offset: start,
                kind: TlErrorKind::UnexpectedConstructor {
                    expected: "Bool",
                    found,
                },
            }),
        }
    }

    /// Reads length-prefixed `bytes` and skips its padding.
    ///
    /// # Errors
    /// If the message is truncated.
    pub fn bytes(&mut self) -> Result<&'a [u8], TlError> {
        let start = self.offset;
        let tag = self.take(1)?[0];
        let length = match tag {
            MEDIUM_LENGTH_TAG => {
                let mut length = [0_u8; 8];
                length[..3].copy_from_slice(self.take(3)?);
                u64::from_le_bytes(length)
            }
            LONG_LENGTH_TAG => {
                let mut length = [0_u8; 8];
                length[..7].copy_from_slice(self.take(7)?);
                u64::from_le_bytes(length)
            }
            short => u64::from(short),
        };
        let length = usize::try_from(length).unwrap_or(usize::MAX);
        let value = self.take(length)?;
        let padding = (4 - (self.offset - start) % 4) % 4;
        self.take(padding)?;
        Ok(value)
    }

    /// Reads `bytes` into an owned vector.
    ///
    /// # Errors
    /// If the message is truncated.
    pub fn bytes_vec(&mut self) -> Result<Vec<u8>, TlError> {
        self.bytes().map(<[u8]>::to_vec)
    }

    /// Reads a UTF-8 `string`.
    ///
    /// # Errors
    /// If the message is truncated or the value is not UTF-8.
    pub fn string(&mut self) -> Result<&'a str, TlError> {
        let start = self.offset;
        let value = self.bytes()?;
        std::str::from_utf8(value).map_err(|_| TlError {
            offset: start,
            kind: TlErrorKind::InvalidUtf8,
        })
    }

    /// Reads a vector length whose elements take at least `min_element_bytes`
    /// each, accepting at most `limit` elements.
    ///
    /// # Errors
    /// [`TlErrorKind::VectorTooLong`] if the count exceeds `limit` or the
    /// remaining input.
    pub fn vector_len(&mut self, min_element_bytes: usize, limit: usize) -> Result<usize, TlError> {
        let start = self.offset;
        let count = self.u32()?;
        let fits = self.remaining() / min_element_bytes.max(1);
        let limit = limit.min(fits);
        let length = usize::try_from(count).unwrap_or(usize::MAX);
        if length > limit {
            return Err(TlError {
                offset: start,
                kind: TlErrorKind::VectorTooLong { count, limit },
            });
        }
        Ok(length)
    }

    /// Checks that the whole message was read.
    ///
    /// # Errors
    /// [`TlErrorKind::TrailingBytes`] otherwise.
    pub fn finish(&self) -> Result<(), TlError> {
        let remaining = self.remaining();
        if remaining == 0 {
            Ok(())
        } else {
            Err(self.error(TlErrorKind::TrailingBytes { remaining }))
        }
    }
}

/// The TL constructor id of a normalized schema line: its IEEE CRC-32.
///
/// The line must already be normalized as the TL compiler does (explicit
/// `#id` removed, `(vector T)` written `vector T`, no trailing `;`).
pub fn constructor_id(schema: &str) -> u32 {
    crc32_ieee(schema.as_bytes())
}

/// IEEE 802.3 CRC-32 (reflected polynomial `0xEDB88320`).
fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_reference_check_value() {
        assert_eq!(crc32_ieee(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32_ieee(b""), 0);
    }

    #[test]
    fn constructor_ids_match_the_published_schema() {
        assert_eq!(constructor_id("boolTrue = Bool"), BOOL_TRUE);
        assert_eq!(constructor_id("boolFalse = Bool"), BOOL_FALSE);
        assert_eq!(
            constructor_id("liteServer.query data:bytes = Object"),
            0x798c_06df
        );
    }

    #[test]
    fn integers_are_little_endian() {
        let mut writer = TlWriter::boxed(0x0102_0304);
        writer
            .i32(-2)
            .u64(0x1122_3344_5566_7788)
            .i64(-1)
            .int256(&[7; 32])
            .bool(true)
            .bool(false);
        assert_eq!(writer.len(), 4 + 4 + 8 + 8 + 32 + 8);
        assert!(!writer.is_empty());
        let bytes = writer.finish();
        assert_eq!(&bytes[..8], &[4, 3, 2, 1, 0xfe, 0xff, 0xff, 0xff]);
        let mut reader = TlReader::new(&bytes);
        assert_eq!(reader.u32().expect("id"), 0x0102_0304);
        assert_eq!(reader.i32().expect("int"), -2);
        assert_eq!(reader.u64().expect("long"), 0x1122_3344_5566_7788);
        assert_eq!(reader.i64().expect("long"), -1);
        assert_eq!(reader.int256().expect("int256"), [7; 32]);
        assert!(reader.bool().expect("true"));
        assert!(!reader.bool().expect("false"));
        assert_eq!(reader.offset(), bytes.len());
        reader.finish().expect("consumed");
    }

    #[test]
    fn bytes_use_the_three_length_forms_and_padding() {
        for (length, header) in [
            (0_usize, 1_usize),
            (1, 1),
            (3, 1),
            (253, 1),
            (254, 4),
            (70_000, 4),
        ] {
            let value: Vec<u8> = (0..length)
                .map(|index| u8::try_from(index % 251).expect("below 251"))
                .collect();
            let mut writer = TlWriter::new();
            writer.bytes(&value).expect("encodes");
            let bytes = writer.finish();
            assert_eq!(bytes.len() % 4, 0, "{length}");
            assert_eq!(bytes.len(), (header + length).div_ceil(4) * 4, "{length}");
            let mut reader = TlReader::new(&bytes);
            assert_eq!(reader.bytes().expect("decodes"), value.as_slice());
            reader.finish().expect("consumed");
        }
        let mut writer = TlWriter::new();
        writer.bytes(&[0xAA; 254]).expect("encodes");
        assert_eq!(&writer.finish()[..4], &[0xFE, 254, 0, 0]);

        // The 7-byte form is read like the reference parser does.
        let mut long = vec![LONG_LENGTH_TAG, 5, 0, 0, 0, 0, 0, 0];
        long.extend_from_slice(b"hello");
        long.extend_from_slice(&[0, 0, 0]);
        let mut reader = TlReader::new(&long);
        assert_eq!(reader.bytes().expect("long form"), b"hello");
        reader.finish().expect("consumed");
    }

    #[test]
    fn strings_must_be_utf8() {
        let mut writer = TlWriter::new();
        writer.bytes(b"liteserver").expect("encodes");
        writer.bytes(&[0xff, 0xfe]).expect("encodes");
        let bytes = writer.finish();
        let mut reader = TlReader::new(&bytes);
        assert_eq!(reader.string().expect("utf8"), "liteserver");
        assert_eq!(
            reader.string().expect_err("not utf8").kind,
            TlErrorKind::InvalidUtf8
        );
    }

    #[test]
    fn truncation_and_trailing_bytes_are_reported() {
        let mut reader = TlReader::new(&[1, 2, 3]);
        let error = reader.u32().expect_err("short");
        assert_eq!(
            error.kind,
            TlErrorKind::Truncated {
                needed: 4,
                remaining: 3
            }
        );
        assert!(error.to_string().contains("truncated"));
        // A length larger than the message is refused before allocating.
        let mut reader = TlReader::new(&[0xFE, 0xFF, 0xFF, 0x7F]);
        assert!(matches!(
            reader.bytes().expect_err("oversized").kind,
            TlErrorKind::Truncated { .. }
        ));
        // Padding is part of the value.
        let mut reader = TlReader::new(&[2, 9, 9]);
        assert!(reader.bytes().is_err());
        let reader = TlReader::new(&[0]);
        let error = reader.finish().expect_err("trailing");
        assert_eq!(error.kind, TlErrorKind::TrailingBytes { remaining: 1 });
        assert!(error.to_string().contains("unread"));
    }

    #[test]
    fn constructors_and_bools_are_checked() {
        let bytes = 0x1234_5678_u32.to_le_bytes();
        let mut reader = TlReader::new(&bytes);
        let error = reader
            .expect_constructor(0x0bad_cafe, "test.value")
            .expect_err("mismatch");
        assert_eq!(error.offset, 0);
        assert!(error.to_string().contains("test.value"));
        let mut reader = TlReader::new(&bytes);
        reader
            .expect_constructor(0x1234_5678, "test.value")
            .expect("match");
        let mut reader = TlReader::new(&bytes);
        assert!(matches!(
            reader.bool().expect_err("not a Bool").kind,
            TlErrorKind::UnexpectedConstructor {
                expected: "Bool",
                ..
            }
        ));
    }

    #[test]
    fn vector_lengths_are_bounded_by_input_and_limit() {
        let mut writer = TlWriter::new();
        writer.u32(3).u32(1).u32(2).u32(3);
        let bytes = writer.finish();
        assert_eq!(TlReader::new(&bytes).vector_len(4, 16).expect("fits"), 3);
        assert!(matches!(
            TlReader::new(&bytes)
                .vector_len(4, 2)
                .expect_err("limit")
                .kind,
            TlErrorKind::VectorTooLong { count: 3, limit: 2 }
        ));
        assert!(matches!(
            TlReader::new(&bytes)
                .vector_len(8, 16)
                .expect_err("input")
                .kind,
            TlErrorKind::VectorTooLong { count: 3, limit: 1 }
        ));
        let huge = u32::MAX.to_le_bytes();
        let error = TlReader::new(&huge)
            .vector_len(0, usize::MAX)
            .expect_err("empty input");
        assert!(error.to_string().contains("announces"));
    }

    #[test]
    fn raw_appends_verbatim() {
        let mut writer = TlWriter::new();
        writer.raw(&[1, 2, 3]).u32(4);
        assert_eq!(writer.finish(), vec![1, 2, 3, 4, 0, 0, 0]);
        let mut reader = TlReader::new(&[5, 6]);
        assert_eq!(reader.take(2).expect("take"), &[5, 6]);
        assert_eq!(reader.remaining(), 0);
    }
}
