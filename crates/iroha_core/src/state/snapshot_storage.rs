//! Exact current and predecessor records for stores with compound binary keys.

use super::*;
use norito::codec::DecodeAll;
use norito::json::JsonSerialize as _;

/// Validate every retained manifest against both storage coordinates and its bytes.
pub(super) fn manifest_set_matches_key(
    uaid: &UniversalAccountId,
    manifests: &SpaceDirectoryManifestSet,
) -> bool {
    manifests.iter().all(|(dataspace, record)| {
        record.uaid() == *uaid
            && record.dataspace() == *dataspace
            && record.manifest_hash == Hash::from(HashOf::new(&record.manifest))
    })
}

/// Mandatory current/undo envelope; first-release snapshots have one wire shape.
#[derive(JsonSerialize, JsonDeserialize)]
pub(crate) struct SnapshotStorage {
    revert: Vec<SnapshotStorageUndo>,
    blocks: Vec<SnapshotStorageEntry>,
}

#[derive(JsonSerialize, JsonDeserialize)]
struct SnapshotStorageEntry {
    key: SnapshotNoritoBlob,
    value: SnapshotNoritoBlob,
}

#[derive(JsonSerialize, JsonDeserialize)]
struct SnapshotStorageUndo {
    key: SnapshotNoritoBlob,
    value: Option<SnapshotNoritoBlob>,
}

fn blob(value: &impl Encode) -> SnapshotNoritoBlob {
    SnapshotNoritoBlob {
        encoded_hex: hex::encode(value.encode()),
    }
}

fn serialize_maps<'a, K: mv::Key + Encode, V: mv::Value + Encode>(
    revert: &BTreeMap<K, Option<V>>,
    current: impl Iterator<Item = (&'a K, &'a V)>,
    out: &mut String,
) {
    out.push_str("{\"revert\":[");
    for (index, (key, value)) in revert.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        SnapshotStorageUndo {
            key: blob(key),
            value: value.as_ref().map(blob),
        }
        .json_serialize(out);
    }
    out.push_str("],\"blocks\":[");
    for (index, (key, value)) in current.enumerate() {
        if index != 0 {
            out.push(',');
        }
        SnapshotStorageEntry {
            key: blob(key),
            value: blob(value),
        }
        .json_serialize(out);
    }
    out.push_str("]}");
}

/// Encode borrowed MV maps under the caller's State publication generation fence.
pub(crate) fn serialize<K: mv::Key + Encode, V: mv::Value + Encode>(
    store: &Storage<K, V>,
    out: &mut String,
) {
    let snapshot = store.snapshot();
    serialize_maps(snapshot.revert_map(), snapshot.current().iter(), out);
}

/// Encode the exact staged post-commit maps without losing their original undo.
pub(crate) fn serialize_block<K: mv::Key + Encode, V: mv::Value + Encode>(
    store: &mv::storage::Block<'_, K, V>,
    out: &mut String,
) {
    serialize_maps(store.revert_map(), store.iter(), out);
}

pub(super) fn decode_blob<T: DecodeAll + Encode>(
    record: SnapshotNoritoBlob,
    field: &str,
) -> Result<T, json::Error> {
    let invalid = |message| json::Error::InvalidField {
        field: field.to_owned(),
        message,
    };
    let bytes = hex::decode(&record.encoded_hex)
        .map_err(|error| invalid(format!("invalid Norito hex: {error}")))?;
    if hex::encode(&bytes) != record.encoded_hex {
        return Err(invalid("noncanonical Norito hex".to_owned()));
    }
    let decoded = T::decode_all(&mut bytes.as_slice())
        .map_err(|error| invalid(format!("invalid Norito record: {error}")))?;
    if decoded.encode() != bytes {
        return Err(invalid("record is not canonical Norito".to_owned()));
    }
    Ok(decoded)
}

impl SnapshotStorage {
    pub(super) fn decode<K, V>(
        self,
        field: &str,
        matches_key: impl Fn(&K, &V) -> bool,
    ) -> Result<Storage<K, V>, json::Error>
    where
        K: mv::Key + DecodeAll + Encode,
        V: mv::Value + DecodeAll + Encode,
    {
        let invalid = |message| json::Error::InvalidField {
            field: field.to_owned(),
            message,
        };
        let mut revert = BTreeMap::new();
        for record in self.revert {
            let key: K = decode_blob(record.key, field)?;
            if revert
                .last_key_value()
                .is_some_and(|(last, _)| last >= &key)
            {
                return Err(invalid(
                    "undo records are not in strict semantic key order".to_owned(),
                ));
            }
            let value = record
                .value
                .map(|value| decode_blob::<V>(value, field))
                .transpose()?;
            if value
                .as_ref()
                .is_some_and(|value| !matches_key(&key, value))
            {
                return Err(invalid("undo record does not match its key".to_owned()));
            }
            revert.insert(key, value);
        }
        let mut blocks = BTreeMap::new();
        for record in self.blocks {
            let key: K = decode_blob(record.key, field)?;
            if blocks
                .last_key_value()
                .is_some_and(|(last, _)| last >= &key)
            {
                return Err(invalid(
                    "current records are not in strict semantic key order".to_owned(),
                ));
            }
            let value: V = decode_blob(record.value, field)?;
            if !matches_key(&key, &value) {
                return Err(invalid("current record does not match its key".to_owned()));
            }
            blocks.insert(key, value);
        }
        Ok(Storage::from_snapshot_parts(blocks, revert))
    }
}

#[cfg(test)]
#[path = "snapshot_storage_tests.rs"]
mod tests;
