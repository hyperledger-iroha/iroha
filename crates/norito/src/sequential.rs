//! Sequential (feature-independent) Norito helpers.
//!
//! These utilities provide a deterministic, compact layout for common
//! collections without depending on compile-time feature flags. They always
//! emit little-endian 64-bit length headers followed by element payloads
//! encoded with [`NoritoSerialize`], and decode the inverse layout.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    convert::TryInto,
    hash::Hash,
};

use byteorder::{LittleEndian, WriteBytesExt};

use crate::core::{Error, NoritoDeserialize, NoritoSerialize, decode_field_canonical};

fn write_len_u64(writer: &mut Vec<u8>, len: usize) -> Result<(), Error> {
    let len_u64 = u64::try_from(len).map_err(|_| Error::LengthMismatch)?;
    writer.write_u64::<LittleEndian>(len_u64)?;
    Ok(())
}

fn read_len_u64(bytes: &[u8], cursor: &mut usize) -> Result<usize, Error> {
    let next = cursor.checked_add(8).ok_or(Error::LengthMismatch)?;
    if next > bytes.len() {
        return Err(Error::LengthMismatch);
    }
    let len_slice: [u8; 8] = bytes[*cursor..next]
        .try_into()
        .map_err(|_| Error::LengthMismatch)?;
    let len = u64::from_le_bytes(len_slice);
    *cursor = next;
    usize::try_from(len).map_err(|_| Error::LengthMismatch)
}

fn serialize_element<T: NoritoSerialize>(value: &T) -> Result<Vec<u8>, Error> {
    let mut buf = Vec::new();
    value.serialize(&mut buf)?;
    Ok(buf)
}

fn deserialize_element<T>(bytes: &[u8]) -> Result<T, Error>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    let (value, used) = decode_field_canonical::<T>(bytes)?;
    if used != bytes.len() {
        return Err(Error::LengthMismatch);
    }
    Ok(value)
}

pub fn serialize_vec<T: NoritoSerialize>(values: &[T]) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    write_len_u64(&mut out, values.len())?;
    for value in values {
        let payload = serialize_element(value)?;
        write_len_u64(&mut out, payload.len())?;
        out.extend_from_slice(&payload);
    }
    Ok(out)
}

pub fn deserialize_vec<T>(bytes: &[u8]) -> Result<Vec<T>, Error>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    let mut cursor = 0usize;
    let len = read_len_u64(bytes, &mut cursor)?;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        let elem_len = read_len_u64(bytes, &mut cursor)?;
        let end = cursor.checked_add(elem_len).ok_or(Error::LengthMismatch)?;
        if end > bytes.len() {
            return Err(Error::LengthMismatch);
        }
        let elem_slice = &bytes[cursor..end];
        out.push(deserialize_element::<T>(elem_slice)?);
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err(Error::LengthMismatch);
    }
    Ok(out)
}

pub fn serialize_btreemap<K, V>(map: &BTreeMap<K, V>) -> Result<Vec<u8>, Error>
where
    K: NoritoSerialize,
    V: NoritoSerialize,
{
    let mut out = Vec::new();
    write_len_u64(&mut out, map.len())?;
    for (key, value) in map.iter() {
        let key_payload = serialize_element(key)?;
        let value_payload = serialize_element(value)?;
        write_len_u64(&mut out, key_payload.len())?;
        out.extend_from_slice(&key_payload);
        write_len_u64(&mut out, value_payload.len())?;
        out.extend_from_slice(&value_payload);
    }
    Ok(out)
}

pub fn deserialize_btreemap<K, V>(bytes: &[u8]) -> Result<BTreeMap<K, V>, Error>
where
    K: for<'de> NoritoDeserialize<'de> + Ord + NoritoSerialize,
    V: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    let mut cursor = 0usize;
    let len = read_len_u64(bytes, &mut cursor)?;
    let mut map = BTreeMap::new();
    for _ in 0..len {
        let key_len = read_len_u64(bytes, &mut cursor)?;
        let key_end = cursor.checked_add(key_len).ok_or(Error::LengthMismatch)?;
        if key_end > bytes.len() {
            return Err(Error::LengthMismatch);
        }
        let key_slice = &bytes[cursor..key_end];
        let key = deserialize_element::<K>(key_slice)?;
        cursor = key_end;

        let value_len = read_len_u64(bytes, &mut cursor)?;
        let value_end = cursor.checked_add(value_len).ok_or(Error::LengthMismatch)?;
        if value_end > bytes.len() {
            return Err(Error::LengthMismatch);
        }
        let value_slice = &bytes[cursor..value_end];
        let value = deserialize_element::<V>(value_slice)?;
        cursor = value_end;

        map.insert(key, value);
    }
    if cursor != bytes.len() {
        return Err(Error::LengthMismatch);
    }
    Ok(map)
}

pub fn serialize_hashmap<K, V>(map: &HashMap<K, V>) -> Result<Vec<u8>, Error>
where
    K: NoritoSerialize + Ord + Hash + Eq,
    V: NoritoSerialize,
{
    let mut ordered: Vec<_> = map.iter().collect();
    ordered.sort_by(|(ka, _), (kb, _)| ka.cmp(kb));
    let mut out = Vec::new();
    write_len_u64(&mut out, ordered.len())?;
    for (key, value) in ordered.into_iter() {
        let key_payload = serialize_element(key)?;
        let value_payload = serialize_element(value)?;
        write_len_u64(&mut out, key_payload.len())?;
        out.extend_from_slice(&key_payload);
        write_len_u64(&mut out, value_payload.len())?;
        out.extend_from_slice(&value_payload);
    }
    Ok(out)
}

pub fn deserialize_hashmap<K, V>(bytes: &[u8]) -> Result<HashMap<K, V>, Error>
where
    K: for<'de> NoritoDeserialize<'de> + NoritoSerialize + Eq + Hash + Ord,
    V: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    let ordered = deserialize_btreemap::<K, V>(bytes)?;
    Ok(ordered.into_iter().collect())
}

pub fn serialize_btreeset<T: NoritoSerialize + Ord>(set: &BTreeSet<T>) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    write_len_u64(&mut out, set.len())?;
    for item in set.iter() {
        let payload = serialize_element(item)?;
        write_len_u64(&mut out, payload.len())?;
        out.extend_from_slice(&payload);
    }
    Ok(out)
}

pub fn deserialize_btreeset<T>(bytes: &[u8]) -> Result<BTreeSet<T>, Error>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize + Ord,
{
    let values = deserialize_vec::<T>(bytes)?;
    Ok(values.into_iter().collect())
}

pub fn serialize_hashset<T>(set: &HashSet<T>) -> Result<Vec<u8>, Error>
where
    T: NoritoSerialize + Ord + Hash + Eq,
{
    let mut ordered: Vec<_> = set.iter().collect();
    ordered.sort();
    let mut out = Vec::new();
    write_len_u64(&mut out, ordered.len())?;
    for item in ordered {
        let payload = serialize_element(item)?;
        write_len_u64(&mut out, payload.len())?;
        out.extend_from_slice(&payload);
    }
    Ok(out)
}

pub fn deserialize_hashset<T>(bytes: &[u8]) -> Result<HashSet<T>, Error>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize + Eq + Hash + Ord,
{
    let ordered = deserialize_btreeset::<T>(bytes)?;
    Ok(ordered.into_iter().collect())
}
