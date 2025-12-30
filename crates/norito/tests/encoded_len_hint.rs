#![allow(clippy::manual_div_ceil)]
//! Tests for encoded_len_hint to ensure capacity reservations are reasonable.
use std::io::Write;

use iroha_schema::IntoSchema;
use norito::core::{Error, NoritoSerialize};

#[test]
fn primitives_hint_sizes() {
    assert_eq!(7u8.encoded_len_hint(), Some(1));
    assert_eq!((-1i8).encoded_len_hint(), Some(1));
    assert_eq!(0x1234u16.encoded_len_hint(), Some(2));
    assert_eq!(0x12345678u32.encoded_len_hint(), Some(4));
    assert_eq!(0x123456789abcdef0u64.encoded_len_hint(), Some(8));
}

#[test]
fn string_hint_size() {
    let s = String::from("hello");
    assert_eq!(s.encoded_len_hint(), Some(8 + 5));
}

#[test]
fn vec_hint_size() {
    let v = vec![1u16, 2u16, 3u16];
    assert_eq!(v.encoded_len_hint(), None);
}

// Custom type that provides no size hint regardless of content
struct NoHint(u32);
impl NoritoSerialize for NoHint {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        // Serialize as a fixed 4-byte little-endian value
        writer.write_all(&self.0.to_le_bytes())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        None
    }
}

// Custom type that advertises a very large hint to exercise saturating math
struct BigHint;
impl NoritoSerialize for BigHint {
    fn serialize<W: Write>(&self, _writer: W) -> Result<(), Error> {
        // Minimal payload; not used in these tests
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(usize::MAX)
    }
}

#[test]
fn vec_hint_empty() {
    let v: Vec<u8> = Vec::new();
    assert_eq!(v.encoded_len_hint(), None);
}

#[test]
fn vec_hint_strings() {
    let v = vec![String::from("a"), String::from("bc")];
    assert_eq!(v.encoded_len_hint(), None);
}

#[test]
fn vec_hint_nested_vecs() {
    let v = vec![vec![1u8, 2u8], vec![3u8]];
    assert_eq!(v.encoded_len_hint(), None);
}

#[test]
fn vec_hint_nohint_element() {
    let v = vec![NoHint(1), NoHint(2)];
    // If any element lacks a hint, the vector reports None (both layouts)
    assert_eq!(v.encoded_len_hint(), None);
}

#[test]
fn vec_hint_saturating_big() {
    let v = vec![BigHint];
    assert_eq!(v.encoded_len_hint(), None);
}

#[derive(IntoSchema, norito::NoritoSerialize, norito::NoritoDeserialize)]
struct Pair {
    a: u32,
    b: u64,
}

#[test]
fn derive_struct_hint() {
    let p = Pair { a: 1, b: 2 };
    // Per-field: 8 prefix + field size; sum both
    assert_eq!(p.encoded_len_hint(), Some(8 + 4 + 8 + 8));
}
