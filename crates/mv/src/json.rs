//! Norito JSON helpers for MV storage types.
//!
//! This module provides thin conversion helpers so that downstream crates can
//! encode/decode MV `Storage`/`Cell` instances using the Norito JSON machinery
//! without pulling Serde into the dependency graph. The former `mv::serde`
//! shim has been removed; callers should import these helpers directly.

use core::{fmt, marker::PhantomData};
use std::{collections::BTreeMap, ops::Deref};

use concread::{
    bptree::{BptreeMap, BptreeMapReadTxn},
    ebrcell::EbrCell,
};
use norito::json::{self, JsonDeserialize, JsonSerialize};

use crate::{Key, Value, cell::Cell, storage::Storage};

/// Helper interface for parsing JSON object keys into typed values.
pub trait KeySeed: Clone {
    /// Target key type.
    type Key;

    /// Convert a borrowed JSON object key into `Self::Key`.
    fn parse_key(&self, key: &str) -> Result<Self::Key, json::Error>;
}

/// Helper interface for parsing JSON values into typed payloads.
pub trait ValueSeed: Clone {
    /// Target value type.
    type Value;

    /// Convert a Norito JSON `Value` into `Self::Value`.
    fn parse_value(&self, value: json::Value) -> Result<Self::Value, json::Error>;
}

/// Helper trait for converting storage keys to and from their JSON string representation.
pub trait JsonKeyCodec: Sized {
    /// Write the canonical JSON string representation of this key to `out`.
    fn encode_json_key(&self, out: &mut String);

    /// Parse a key from a JSON string representation.
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error>;
}

const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";

fn append_hex_upper(bytes: &[u8], out: &mut String) {
    out.reserve(bytes.len() * 2);
    for &byte in bytes {
        let hi = (byte >> 4) as usize;
        let lo = (byte & 0x0F) as usize;
        out.push(HEX_DIGITS[hi] as char);
        out.push(HEX_DIGITS[lo] as char);
    }
}

fn decode_hex_nibble(byte: u8) -> Result<u8, json::Error> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(json::Error::Message(format!(
            "invalid hex digit `{}`",
            byte as char
        ))),
    }
}

fn decode_hex_array<const N: usize>(encoded: &str) -> Result<[u8; N], json::Error> {
    if encoded.len() != N * 2 {
        return Err(json::Error::Message(format!(
            "expected {len} hex digits, got {}",
            encoded.len(),
            len = N * 2
        )));
    }
    let mut out = [0u8; N];
    let bytes = encoded.as_bytes();
    for (i, chunk) in bytes.chunks_exact(2).enumerate() {
        let hi = decode_hex_nibble(chunk[0])?;
        let lo = decode_hex_nibble(chunk[1])?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

impl JsonKeyCodec for String {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(self, out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        Ok(encoded.to_owned())
    }
}

impl JsonKeyCodec for u64 {
    fn encode_json_key(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .parse::<u64>()
            .map_err(|err| json::Error::Message(format!("invalid map key `{encoded}`: {err}")))
    }
}

impl<const N: usize> JsonKeyCodec for [u8; N] {
    fn encode_json_key(&self, out: &mut String) {
        let mut buf = String::new();
        append_hex_upper(self, &mut buf);
        json::write_json_string(&buf, out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        decode_hex_array(encoded)
    }
}

/// Delimiter for tuple keys (chosen outside the ASCII printable range to avoid collisions).
const TUPLE_KEY_SEPARATOR: char = '\u{1f}';

impl JsonKeyCodec for (String, String) {
    fn encode_json_key(&self, out: &mut String) {
        let mut buf = String::with_capacity(self.0.len() + self.1.len() + 1);
        buf.push_str(&self.0);
        buf.push(TUPLE_KEY_SEPARATOR);
        buf.push_str(&self.1);
        json::write_json_string(&buf, out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        encoded
            .split_once(TUPLE_KEY_SEPARATOR)
            .map(|(left, right)| (left.to_owned(), right.to_owned()))
            .ok_or_else(|| {
                json::Error::Message("expected contract tuple key to contain unit separator".into())
            })
    }
}

impl JsonKeyCodec for (String, u32) {
    fn encode_json_key(&self, out: &mut String) {
        let mut buf = String::with_capacity(self.0.len() + 11 + 1);
        buf.push_str(&self.0);
        buf.push(TUPLE_KEY_SEPARATOR);
        buf.push_str(&self.1.to_string());
        json::write_json_string(&buf, out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let (left, right) = encoded.split_once(TUPLE_KEY_SEPARATOR).ok_or_else(|| {
            json::Error::Message("expected circuit tuple key to contain unit separator".into())
        })?;
        let version = right.parse::<u32>().map_err(|err| {
            json::Error::Message(format!("invalid circuit version `{right}`: {err}"))
        })?;
        Ok((left.to_owned(), version))
    }
}

/// Key seed that relies on `FromStr` with deterministic error reporting.
#[derive(Debug, Default)]
pub struct KeyFromStr<T>(PhantomData<T>);

impl<T> KeyFromStr<T> {
    /// Construct a new seed (type inference helper).
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Clone for KeyFromStr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for KeyFromStr<T> {}

impl<T> KeySeed for KeyFromStr<T>
where
    T: core::str::FromStr,
    T::Err: fmt::Display,
{
    type Key = T;

    fn parse_key(&self, key: &str) -> Result<Self::Key, json::Error> {
        key.parse::<T>()
            .map_err(|err| json::Error::Message(format!("invalid map key `{key}`: {err}")))
    }
}

/// Key seed that delegates to [`JsonKeyCodec`] implementations.
#[derive(Debug, Default)]
pub struct CodecKeySeed<K>(PhantomData<K>);

impl<K> CodecKeySeed<K> {
    /// Construct a new seed (type inference helper).
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<K> Clone for CodecKeySeed<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for CodecKeySeed<K> {}

impl<K> KeySeed for CodecKeySeed<K>
where
    K: JsonKeyCodec,
{
    type Key = K;

    fn parse_key(&self, key: &str) -> Result<Self::Key, json::Error> {
        K::decode_json_key(key)
    }
}

/// Value seed that delegates to `JsonDeserialize` via the in-memory DOM.
#[derive(Debug, Default)]
pub struct ValueFromJson<T>(PhantomData<T>);

impl<T> ValueFromJson<T> {
    /// Construct a new seed (type inference helper).
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Clone for ValueFromJson<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ValueFromJson<T> {}

impl<T> ValueSeed for ValueFromJson<T>
where
    T: JsonDeserialize,
{
    type Value = T;

    fn parse_value(&self, value: json::Value) -> Result<Self::Value, json::Error> {
        json::value::from_value(value)
    }
}

/// Deserialize `Storage<K, V>` using custom key/value seeds.
#[derive(Debug, Clone)]
pub struct StorageSeeded<KS, VS> {
    /// Seed used to parse object keys.
    pub kseed: KS,
    /// Seed used to parse object values.
    pub vseed: VS,
}

impl<KS, VS> StorageSeeded<KS, VS>
where
    KS: KeySeed,
    VS: ValueSeed,
    KS::Key: Key + Ord,
    VS::Value: Value,
{
    /// Parse `Storage` from `parser`, expecting `{ "revert": {..}, "blocks": {..} }`.
    pub fn deserialize(
        &self,
        parser: &mut json::Parser<'_>,
    ) -> Result<Storage<KS::Key, VS::Value>, json::Error> {
        let mut map = json::MapVisitor::new(parser)?;
        let mut revert: Option<BTreeMap<KS::Key, Option<VS::Value>>> = None;
        let mut blocks: Option<BptreeMap<KS::Key, VS::Value>> = None;

        while let Some(key) = map.next_key()? {
            match key.as_str() {
                "revert" => {
                    if revert.is_some() {
                        return Err(json::MapVisitor::duplicate_field("revert"));
                    }
                    let value = map.parse_value::<json::Value>()?;
                    revert = Some(self.parse_revert(value)?);
                }
                "blocks" => {
                    if blocks.is_some() {
                        return Err(json::MapVisitor::duplicate_field("blocks"));
                    }
                    let value = map.parse_value::<json::Value>()?;
                    blocks = Some(self.parse_blocks(value)?);
                }
                other => {
                    return Err(json::MapVisitor::unknown_field(other));
                }
            }
        }
        map.finish()?;

        let revert = revert.ok_or_else(|| json::MapVisitor::missing_field("revert"))?;
        let blocks = blocks.ok_or_else(|| json::MapVisitor::missing_field("blocks"))?;

        Ok(Storage {
            revert: EbrCell::new(revert),
            blocks,
        })
    }

    fn parse_revert(
        &self,
        value: json::Value,
    ) -> Result<BTreeMap<KS::Key, Option<VS::Value>>, json::Error> {
        let json::Value::Object(map) = value else {
            return Err(json::Error::Message("expected object for `revert`".into()));
        };

        let mut out = BTreeMap::new();
        for (key_str, raw_value) in map.into_iter() {
            let key = self.kseed.parse_key(&key_str)?;
            let parsed = if matches!(raw_value, json::Value::Null) {
                None
            } else {
                Some(self.vseed.parse_value(raw_value)?)
            };
            if out.insert(key, parsed).is_some() {
                return Err(json::Error::DuplicateField { field: key_str });
            }
        }
        Ok(out)
    }

    fn parse_blocks(
        &self,
        value: json::Value,
    ) -> Result<BptreeMap<KS::Key, VS::Value>, json::Error> {
        let json::Value::Object(map) = value else {
            return Err(json::Error::Message("expected object for `blocks`".into()));
        };

        let mut entries = Vec::with_capacity(map.len());
        for (key_str, raw_value) in map.into_iter() {
            let key = self.kseed.parse_key(&key_str)?;
            let value = self.vseed.parse_value(raw_value.clone())?;
            entries.push((key, value));
        }
        Ok(BptreeMap::from_iter(entries))
    }
}

/// Deserialize `Cell<V>` using a custom seed.
#[derive(Debug, Clone)]
pub struct CellSeeded<S> {
    /// Seed used to parse the inner value.
    pub seed: S,
}

impl<S> CellSeeded<S>
where
    S: ValueSeed,
    S::Value: Value,
{
    /// Parse `Cell` from `parser`, expecting `{ "revert": <Option>, "blocks": <Value> }`.
    pub fn deserialize(
        &self,
        parser: &mut json::Parser<'_>,
    ) -> Result<Cell<S::Value>, json::Error> {
        let mut map = json::MapVisitor::new(parser)?;
        let mut revert: Option<Option<S::Value>> = None;
        let mut blocks: Option<S::Value> = None;

        while let Some(key) = map.next_key()? {
            match key.as_str() {
                "revert" => {
                    if revert.is_some() {
                        return Err(json::MapVisitor::duplicate_field("revert"));
                    }
                    let value = map.parse_value::<json::Value>()?;
                    revert = Some(self.parse_optional(value)?);
                }
                "blocks" => {
                    if blocks.is_some() {
                        return Err(json::MapVisitor::duplicate_field("blocks"));
                    }
                    let value = map.parse_value::<json::Value>()?;
                    blocks = Some(self.seed.parse_value(value)?);
                }
                other => {
                    return Err(json::MapVisitor::unknown_field(other));
                }
            }
        }
        map.finish()?;

        let revert = revert.ok_or_else(|| json::MapVisitor::missing_field("revert"))?;
        let blocks = blocks.ok_or_else(|| json::MapVisitor::missing_field("blocks"))?;

        Ok(Cell {
            revert: EbrCell::new(revert),
            blocks: EbrCell::new(blocks),
        })
    }

    fn parse_optional(&self, value: json::Value) -> Result<Option<S::Value>, json::Error> {
        if matches!(value, json::Value::Null) {
            Ok(None)
        } else {
            self.seed.parse_value(value).map(Some)
        }
    }
}

impl<K, V> JsonSerialize for Storage<K, V>
where
    K: JsonKeyCodec + Key,
    V: JsonSerialize + Value,
{
    fn json_serialize(&self, out: &mut String) {
        let revert = self.revert.read();
        let blocks = self.blocks.read();

        out.push('{');
        out.push_str("\"revert\":");
        write_revert(revert.deref(), out);
        out.push(',');
        out.push_str("\"blocks\":");
        write_blocks(&blocks, out);
        out.push('}');
    }
}

impl<V> JsonSerialize for Cell<V>
where
    V: JsonSerialize + Value,
{
    fn json_serialize(&self, out: &mut String) {
        let revert = self.revert.read();
        let blocks = self.blocks.read();

        out.push('{');
        out.push_str("\"revert\":");
        JsonSerialize::json_serialize(revert.deref(), out);
        out.push(',');
        out.push_str("\"blocks\":");
        JsonSerialize::json_serialize(blocks.deref(), out);
        out.push('}');
    }
}

impl<K, V> JsonDeserialize for Storage<K, V>
where
    K: JsonKeyCodec + Key + Ord,
    V: JsonSerialize + JsonDeserialize + Value,
{
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        StorageSeeded {
            kseed: CodecKeySeed::<K>::new(),
            vseed: ValueFromJson::<V>::new(),
        }
        .deserialize(parser)
    }
}

impl<V> JsonDeserialize for Cell<V>
where
    V: JsonSerialize + JsonDeserialize + Value,
    ValueFromJson<V>: ValueSeed<Value = V>,
{
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        CellSeeded {
            seed: ValueFromJson::<V>::new(),
        }
        .deserialize(parser)
    }
}

fn write_revert<K, V>(map: &BTreeMap<K, Option<V>>, out: &mut String)
where
    K: JsonKeyCodec,
    V: JsonSerialize,
{
    out.push('{');
    let mut iter = map.iter();
    if let Some((key, value)) = iter.next() {
        key.encode_json_key(out);
        out.push(':');
        value.json_serialize(out);
        for (key, value) in iter {
            out.push(',');
            key.encode_json_key(out);
            out.push(':');
            value.json_serialize(out);
        }
    }
    out.push('}');
}

fn write_blocks<K, V>(blocks: &BptreeMapReadTxn<'_, K, V>, out: &mut String)
where
    K: JsonKeyCodec + Clone + Ord + fmt::Debug + Send + Sync + 'static,
    V: JsonSerialize + Clone + Send + Sync + 'static,
{
    out.push('{');
    let mut iter = blocks.iter();
    if let Some((key, value)) = iter.next() {
        key.encode_json_key(out);
        out.push(':');
        value.json_serialize(out);
        for (key, value) in iter {
            out.push(',');
            key.encode_json_key(out);
            out.push(':');
            value.json_serialize(out);
        }
    }
    out.push('}');
}

#[cfg(test)]
mod tests {
    use norito::json::{from_json, to_json};

    use super::*;
    use crate::storage::StorageReadOnly;

    fn sample_storage() -> Storage<String, i32> {
        let storage = Storage::default();
        {
            let mut block = storage.block();
            block.insert("foo".to_owned(), 1);
            block.insert("bar".to_owned(), 2);
            block.commit();
        }
        storage
    }

    #[test]
    fn storage_roundtrip() {
        let storage = sample_storage();
        let json = to_json(&storage).expect("serialize");
        let decoded: Storage<String, i32> = from_json(&json).expect("deserialize");
        let storage_view = storage.view();
        let decoded_view = decoded.view();
        assert_eq!(decoded_view.len(), storage_view.len());
        for (k, v) in storage_view.iter() {
            let decoded_v = decoded_view.get(k).expect("present");
            assert_eq!(decoded_v, v);
        }
    }

    #[test]
    fn cell_roundtrip() {
        let cell = Cell::new(42i32);
        let json = to_json(&cell).expect("serialize");
        let decoded: Cell<i32> = from_json(&json).expect("deserialize");
        assert_eq!(*decoded.view(), 42);
    }

    #[test]
    fn storage_roundtrip_array_key() {
        let storage: Storage<[u8; 4], i32> = Storage::new();
        {
            let mut block = storage.block();
            block.insert([0xAA, 0xBB, 0xCC, 0xDD], 7);
            block.commit();
        }
        let json = to_json(&storage).expect("serialize hex key");
        let decoded: Storage<[u8; 4], i32> = from_json(&json).expect("deserialize hex key");
        assert_eq!(decoded.view().get(&[0xAA, 0xBB, 0xCC, 0xDD]), Some(&7));
    }

    #[test]
    fn storage_roundtrip_tuple_key() {
        let storage: Storage<(String, String), i32> = Storage::new();
        {
            let mut block = storage.block();
            block.insert(("namespace".into(), "contract".into()), 11);
            block.commit();
        }
        let json = to_json(&storage).expect("serialize tuple key");
        let decoded: Storage<(String, String), i32> =
            from_json(&json).expect("deserialize tuple key");
        assert_eq!(
            decoded
                .view()
                .get(&(String::from("namespace"), String::from("contract"))),
            Some(&11)
        );
    }
}
