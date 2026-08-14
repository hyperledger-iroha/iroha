//! Strict deterministic-CBOR reader for the closed Vega mDL profile.
use core::{cmp::Ordering, ops::Range, str};
use thiserror::Error;
const MAX_CBOR_DEPTH_V1: usize = 16;
const MAX_CBOR_CONTAINER_ITEMS_V1: usize = 256;
const MAX_CBOR_TOTAL_ITEMS_V1: usize = 1_024;
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub(super) enum CborError {
    #[error("canonical CBOR input is empty or truncated")]
    UnexpectedEnd,
    #[error("canonical CBOR input contains trailing bytes")]
    TrailingBytes,
    #[error("CBOR additional-information value is reserved")]
    ReservedAdditionalInformation,
    #[error("indefinite-length CBOR is not admitted")]
    IndefiniteLength,
    #[error("CBOR integer or length does not use its shortest encoding")]
    NonMinimalArgument,
    #[error("CBOR length does not fit the native address space")]
    LengthOverflow,
    #[error("CBOR nesting exceeds the closed profile depth")]
    DepthLimit,
    #[error("CBOR container exceeds the closed profile item bound")]
    ContainerLimit,
    #[error("CBOR document exceeds the closed profile total-item bound")]
    ItemLimit,
    #[error("CBOR text string is not valid UTF-8")]
    InvalidUtf8,
    #[error("CBOR map keys are duplicated or not in deterministic order")]
    NonCanonicalMapOrder,
    #[error("floating-point and unassigned CBOR simple values are not admitted")]
    UnsupportedSimpleValue,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CborNode<'a> {
    source: &'a [u8],
    range: Range<usize>,
    value: CborValue<'a>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CborValue<'a> {
    Unsigned(u64),
    Negative(u64),
    Bytes(&'a [u8]),
    Text(&'a str),
    Array(Vec<CborNode<'a>>),
    Map(Vec<(CborNode<'a>, CborNode<'a>)>),
    Tag(u64, Box<CborNode<'a>>),
    Boolean(bool),
    Null,
}
impl<'a> CborNode<'a> {
    pub(super) fn parse_exact(bytes: &'a [u8]) -> Result<Self, CborError> {
        let mut parser = Parser {
            bytes,
            offset: 0,
            items: 0,
        };
        let node = parser.parse_node(0)?;
        if parser.offset != bytes.len() {
            return Err(CborError::TrailingBytes);
        }
        Ok(node)
    }
    pub(super) fn encoded(&self) -> &'a [u8] {
        &self.source[self.range.clone()]
    }
    pub(super) fn range(&self) -> Range<usize> {
        self.range.clone()
    }
    pub(super) fn as_map(&self) -> Option<&[(CborNode<'a>, CborNode<'a>)]> {
        match &self.value {
            CborValue::Map(entries) => Some(entries),
            _ => None,
        }
    }
    pub(super) fn as_array(&self) -> Option<&[CborNode<'a>]> {
        match &self.value {
            CborValue::Array(values) => Some(values),
            _ => None,
        }
    }
    pub(super) const fn as_bytes(&self) -> Option<&'a [u8]> {
        match self.value {
            CborValue::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }
    pub(super) fn as_bytes_with_range(&self) -> Option<(&'a [u8], Range<usize>)> {
        let bytes = self.as_bytes()?;
        let source_start = self.source.as_ptr() as usize;
        let bytes_start = bytes.as_ptr() as usize;
        let start = bytes_start.checked_sub(source_start)?;
        let end = start.checked_add(bytes.len())?;
        (end <= self.source.len()).then_some((bytes, start..end))
    }
    pub(super) const fn as_text(&self) -> Option<&'a str> {
        match self.value {
            CborValue::Text(text) => Some(text),
            _ => None,
        }
    }
    pub(super) const fn as_unsigned(&self) -> Option<u64> {
        match self.value {
            CborValue::Unsigned(value) => Some(value),
            _ => None,
        }
    }
    pub(super) fn tagged(&self, expected: u64) -> Option<&CborNode<'a>> {
        match &self.value {
            CborValue::Tag(tag, value) if *tag == expected => Some(value),
            _ => None,
        }
    }
    pub(super) fn integer_equals(&self, expected: i64) -> bool {
        match self.value {
            CborValue::Unsigned(value) => u64::try_from(expected) == Ok(value),
            CborValue::Negative(argument) => {
                expected < 0 && u64::try_from(-(i128::from(expected)) - 1) == Ok(argument)
            }
            _ => false,
        }
    }
    pub(super) fn map_get_text(&self, key: &str) -> Option<&CborNode<'a>> {
        self.map_entry_text(key).map(|(_, value)| value)
    }
    pub(super) fn map_get_integer(&self, key: i64) -> Option<&CborNode<'a>> {
        self.map_entry_integer(key).map(|(_, value)| value)
    }
    pub(super) fn map_entry_text(&self, key: &str) -> Option<(&CborNode<'a>, &CborNode<'a>)> {
        self.as_map()?.iter().find_map(|(candidate, value)| {
            (candidate.as_text() == Some(key)).then_some((candidate, value))
        })
    }
    pub(super) fn map_entry_integer(&self, key: i64) -> Option<(&CborNode<'a>, &CborNode<'a>)> {
        self.as_map()?.iter().find_map(|(candidate, value)| {
            candidate.integer_equals(key).then_some((candidate, value))
        })
    }
    pub(super) fn map_entry_unsigned(&self, key: u64) -> Option<(&CborNode<'a>, &CborNode<'a>)> {
        self.as_map()?.iter().find_map(|(candidate, value)| {
            (candidate.as_unsigned() == Some(key)).then_some((candidate, value))
        })
    }
}
struct Parser<'a> {
    bytes: &'a [u8],
    offset: usize,
    items: usize,
}
impl<'a> Parser<'a> {
    fn parse_node(&mut self, depth: usize) -> Result<CborNode<'a>, CborError> {
        if depth > MAX_CBOR_DEPTH_V1 {
            return Err(CborError::DepthLimit);
        }
        self.items = self.items.checked_add(1).ok_or(CborError::ItemLimit)?;
        if self.items > MAX_CBOR_TOTAL_ITEMS_V1 {
            return Err(CborError::ItemLimit);
        }
        let start = self.offset;
        let initial = self.take_byte()?;
        let major = initial >> 5;
        let additional = initial & 0x1f;
        let value = match major {
            0 => CborValue::Unsigned(self.read_argument(additional)?),
            1 => CborValue::Negative(self.read_argument(additional)?),
            2 => {
                let length = self.read_length(additional)?;
                CborValue::Bytes(self.take(length)?)
            }
            3 => {
                let length = self.read_length(additional)?;
                let bytes = self.take(length)?;
                CborValue::Text(str::from_utf8(bytes).map_err(|_| CborError::InvalidUtf8)?)
            }
            4 => {
                let length = self.read_container_length(additional, false)?;
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    values.push(self.parse_node(depth + 1)?);
                }
                CborValue::Array(values)
            }
            5 => {
                let length = self.read_container_length(additional, true)?;
                let mut entries = Vec::with_capacity(length);
                let mut previous_key: Option<&[u8]> = None;
                for _ in 0..length {
                    let key = self.parse_node(depth + 1)?;
                    if previous_key.is_some_and(|previous| {
                        deterministic_key_cmp(previous, key.encoded()) != Ordering::Less
                    }) {
                        return Err(CborError::NonCanonicalMapOrder);
                    }
                    previous_key = Some(key.encoded());
                    let value = self.parse_node(depth + 1)?;
                    entries.push((key, value));
                }
                CborValue::Map(entries)
            }
            6 => {
                let tag = self.read_argument(additional)?;
                let value = self.parse_node(depth + 1)?;
                CborValue::Tag(tag, Box::new(value))
            }
            7 => match additional {
                20 => CborValue::Boolean(false),
                21 => CborValue::Boolean(true),
                22 => CborValue::Null,
                _ => return Err(CborError::UnsupportedSimpleValue),
            },
            _ => unreachable!("CBOR major type occupies three bits"),
        };
        Ok(CborNode {
            source: self.bytes,
            range: start..self.offset,
            value,
        })
    }
    fn read_container_length(&mut self, additional: u8, map: bool) -> Result<usize, CborError> {
        let length = self.read_length(additional)?;
        if length > MAX_CBOR_CONTAINER_ITEMS_V1 {
            return Err(CborError::ContainerLimit);
        }
        let minimum_bytes = if map {
            length.checked_mul(2).ok_or(CborError::LengthOverflow)?
        } else {
            length
        };
        if minimum_bytes > self.remaining() {
            return Err(CborError::UnexpectedEnd);
        }
        Ok(length)
    }
    fn read_length(&mut self, additional: u8) -> Result<usize, CborError> {
        usize::try_from(self.read_argument(additional)?).map_err(|_| CborError::LengthOverflow)
    }
    fn read_argument(&mut self, additional: u8) -> Result<u64, CborError> {
        match additional {
            value @ 0..=23 => Ok(u64::from(value)),
            24 => {
                let value = u64::from(self.take_byte()?);
                if value < 24 {
                    return Err(CborError::NonMinimalArgument);
                }
                Ok(value)
            }
            25 => {
                let value = u64::from(u16::from_be_bytes(self.take_array()?));
                if value <= u64::from(u8::MAX) {
                    return Err(CborError::NonMinimalArgument);
                }
                Ok(value)
            }
            26 => {
                let value = u64::from(u32::from_be_bytes(self.take_array()?));
                if value <= u64::from(u16::MAX) {
                    return Err(CborError::NonMinimalArgument);
                }
                Ok(value)
            }
            27 => {
                let value = u64::from_be_bytes(self.take_array()?);
                if value <= u64::from(u32::MAX) {
                    return Err(CborError::NonMinimalArgument);
                }
                Ok(value)
            }
            31 => Err(CborError::IndefiniteLength),
            _ => Err(CborError::ReservedAdditionalInformation),
        }
    }
    fn take_array<const N: usize>(&mut self) -> Result<[u8; N], CborError> {
        let mut value = [0_u8; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }
    fn take_byte(&mut self) -> Result<u8, CborError> {
        let byte = *self
            .bytes
            .get(self.offset)
            .ok_or(CborError::UnexpectedEnd)?;
        self.offset += 1;
        Ok(byte)
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], CborError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(CborError::LengthOverflow)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(CborError::UnexpectedEnd)?;
        self.offset = end;
        Ok(bytes)
    }
    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }
}
fn deterministic_key_cmp(left: &[u8], right: &[u8]) -> Ordering {
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_map_parses_and_exposes_integer_keys() {
        let node = CborNode::parse_exact(&[0xa2, 0x01, 0x02, 0x20, 0x01]).expect("canonical map");
        assert_eq!(
            node.map_get_integer(1).and_then(CborNode::as_unsigned),
            Some(2)
        );
        assert_eq!(
            node.map_get_integer(-1).and_then(CborNode::as_unsigned),
            Some(1)
        );
    }
    #[test]
    fn rejects_non_minimal_indefinite_duplicate_and_unsorted_encodings() {
        for malformed in [
            &[0x18, 0x17][..],
            &[0x9f, 0xff],
            &[0xa2, 0x01, 0x00, 0x01, 0x01],
            &[0xa2, 0x20, 0x01, 0x01, 0x02],
            &[0xf9, 0x00, 0x00],
        ] {
            assert!(CborNode::parse_exact(malformed).is_err(), "{malformed:x?}");
        }
    }
    #[test]
    fn rejects_truncation_trailing_bytes_and_excessive_depth() {
        assert_eq!(
            CborNode::parse_exact(&[0x58, 0x20, 0]),
            Err(CborError::UnexpectedEnd)
        );
        assert_eq!(
            CborNode::parse_exact(&[0x01, 0x02]),
            Err(CborError::TrailingBytes)
        );
        let mut nested = vec![0xc0; MAX_CBOR_DEPTH_V1 + 1];
        nested.push(0xf6);
        assert_eq!(CborNode::parse_exact(&nested), Err(CborError::DepthLimit));
    }
}
