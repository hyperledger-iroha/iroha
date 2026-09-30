//! Bounded complete World-element snapshots authenticated against certified execution.
//!
//! Every element includes its field/key/value hash preimage. An opaque remainder of
//! LtHash lanes is never a membership proof: it can be algebraically manufactured.
//! Native publication must capture all canonical elements at the same applied cut.
//! Target values additionally require their real canonical typed preimages.

use super::{FinalityError, VerifiedSumeragiBlock};
use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::{
    Decode, Encode,
    derive::{JsonDeserialize, JsonSerialize},
};
use std::{collections::BTreeSet, io::Write as _, sync::Arc};

/// Number of little-endian u16 lanes in the existing complete World accumulator.
pub const WORLD_STATE_ACCUMULATOR_LANES_V1: usize = 1024;
/// Portable reader bound, not a native consensus invalidity verdict.
pub const MAX_WORLD_STATE_SNAPSHOT_ENTRIES_V1: usize = 131_072;
/// Maximum original canonical snapshot frame accepted by the portable reader.
pub const MAX_WORLD_STATE_SNAPSHOT_BYTES_V1: usize = 32 * 1024 * 1024;
const ELEMENT_CONTEXT: &str = "iroha 2026-09-30 world-state lthash16 element v1";
const PATH: &[u8] = b"iroha:world-state:path:v1\0";
const ROOT: &[u8] = b"iroha:world-state:root:v1\0";
const VALUE: &[u8] = b"iroha:world-net-delta:value:bare-v1\0";

/// Stable canonical World field kinds used by the native authority registry.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(tag = "kind", content = "value", deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::WorldStateElementKindV1")]
pub enum WorldStateElementKindV1 {
    /// A keyed table entry, stable discriminant zero.
    Table,
    /// The sole value of a canonical cell, stable discriminant one.
    Cell,
}
impl WorldStateElementKindV1 {
    fn tag(self) -> u8 {
        match self {
            Self::Table => 0,
            Self::Cell => 1,
        }
    }
}

/// Complete hash preimage of one canonical World element, without unrelated values.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::WorldStateSnapshotEntryV1")]
pub struct WorldStateSnapshotEntryV1 {
    /// Full native registry identity, including `world.` and any owner path.
    pub field_id: String,
    /// Exact native registry kind.
    pub kind: WorldStateElementKindV1,
    /// Bare canonical key hash for a table; explicit absence for a cell.
    #[norito(required)]
    pub key_hash: Option<Hash>,
    /// Canonical semantic value hash supplied by the field owner.
    pub value_hash: Hash,
}

/// Data-only full snapshot; callers cannot select authority by decoding it.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(
    name = "iroha_data_model::sumeragi_finality::WorldStateSnapshotV1",
    frame = "iroha.sumeragi.world-state-snapshot.v1"
)]
pub struct WorldStateSnapshotV1 {
    /// Actual native registry schema commitment, itself bound by the certified root.
    pub schema_hash: Hash,
    /// Every canonical element, in strict field/kind/key-hash order.
    pub entries: Vec<WorldStateSnapshotEntryV1>,
}

/// Snapshot authenticated against one opaque certified execution decision.
/// This is ledger-content evidence; it does not grant deployment or release custody.
#[derive(Debug, Clone)]
pub struct VerifiedWorldStateSnapshotV1 {
    height: u64,
    context_id: Hash,
    world_root: Hash,
    entries: Arc<[WorldStateSnapshotEntryV1]>,
}

fn fail(reason: &str) -> FinalityError {
    FinalityError(reason.into())
}
fn valid_hash(hash: Hash) -> bool {
    let bytes = hash.as_ref();
    bytes[31] & 1 == 1 && !(bytes[..31].iter().all(|byte| *byte == 0) && bytes[31] == 1)
}

/// Exact existing path hash; the kind is part of the field identity.
/// # Errors
/// Invalid or unbounded canonical field identity.
pub fn world_state_path_hash_v1(
    field_id: &str,
    kind: WorldStateElementKindV1,
) -> Result<Hash, FinalityError> {
    if !field_id.starts_with("world.")
        || field_id.len() <= 6
        || field_id.len() > 192
        || !field_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.'))
    {
        return Err(fail("World snapshot field identity is invalid"));
    }
    let len = field_id.len() as u64;
    Ok(Hash::new_from_chunks(&[
        PATH,
        &[kind.tag()],
        &len.to_le_bytes(),
        field_id.as_bytes(),
    ]))
}

/// Expand a complete canonical element preimage using the existing LtHash context.
/// This arithmetic helper supplies no membership or finality authority.
#[must_use]
pub fn world_state_element_v1(
    path: &Hash,
    key: Option<&Hash>,
    value: &Hash,
) -> [u8; 2 * WORLD_STATE_ACCUMULATOR_LANES_V1] {
    let mut hasher = blake3::Hasher::new_derive_key(ELEMENT_CONTEXT);
    hasher.update(path.as_ref());
    hasher.update(&[u8::from(key.is_some())]);
    hasher.update(key.map_or(&[0; Hash::LENGTH], |key| key.as_ref()));
    hasher.update(value.as_ref());
    let mut lanes = [0; 2 * WORLD_STATE_ACCUMULATOR_LANES_V1];
    hasher.finalize_xof().fill(&mut lanes);
    lanes
}

/// Hash the existing complete accumulator; no root can authenticate its own inputs.
#[must_use]
pub fn world_state_root_from_accumulator_v1(
    schema: Hash,
    entries: u64,
    lanes: &[u16; WORLD_STATE_ACCUMULATOR_LANES_V1],
) -> Hash {
    let mut bytes = [0; 2 * WORLD_STATE_ACCUMULATOR_LANES_V1];
    for (encoded, lane) in bytes.chunks_exact_mut(2).zip(lanes) {
        encoded.copy_from_slice(&lane.to_le_bytes());
    }
    Hash::new_from_chunks(&[ROOT, schema.as_ref(), &entries.to_le_bytes(), &bytes])
}

/// Exact existing semantic-value hash with bare Norito V1 payload and encoded length.
/// Cache-bearing native fields must supply their registry-declared semantic projection.
/// # Errors
/// Canonical payload encoding or streaming hash failure.
pub fn world_state_value_hash_v1<T: norito::codec::Encode>(
    value: &T,
) -> Result<Hash, FinalityError> {
    Hash::new_from_writer(|mut writer| {
        writer.write_all(VALUE)?;
        let len = norito::codec::encode_adaptive_into(value, &mut writer)
            .map_err(std::io::Error::other)?;
        let len = u64::try_from(len)
            .map_err(|_| std::io::Error::other("World value length exceeds u64"))?;
        writer.write_all(&len.to_le_bytes())
    })
    .map_err(|_| fail("World snapshot canonical value encoding failed"))
}

impl WorldStateSnapshotV1 {
    /// Decode only an exact bounded original canonical native frame.
    /// # Errors
    /// Oversized, noncanonical, truncated, invalid or duplicate-entry snapshot.
    pub fn decode_bounded_canonical(wire: &[u8]) -> Result<Self, FinalityError> {
        if wire.is_empty() || wire.len() > MAX_WORLD_STATE_SNAPSHOT_BYTES_V1 {
            return Err(fail("World snapshot original exceeds its byte bound"));
        }
        let value: Self = norito::decode_canonical_with_limits(
            wire,
            norito::canonical_decode_limits(MAX_WORLD_STATE_SNAPSHOT_BYTES_V1),
        )
        .map_err(|_| fail("World snapshot original is not canonical bounded Norito"))?;
        value.root()?;
        Ok(value)
    }

    /// Recompute the complete World root from every full element preimage.
    /// This result remains untrusted until compared with certified execution.
    /// # Errors
    /// Bounds, invalid kind/key/hash, duplicate identity or noncanonical order.
    pub fn root(&self) -> Result<Hash, FinalityError> {
        if self.entries.len() > MAX_WORLD_STATE_SNAPSHOT_ENTRIES_V1 || !valid_hash(self.schema_hash)
        {
            return Err(fail("World snapshot entries or schema exceed their bound"));
        }
        let mut lanes = [0u16; WORLD_STATE_ACCUMULATOR_LANES_V1];
        let mut previous = None;
        let mut fields = BTreeSet::new();
        for entry in &self.entries {
            if !valid_hash(entry.value_hash)
                || entry.key_hash.is_some_and(|key| !valid_hash(key))
                || (entry.kind == WorldStateElementKindV1::Table) != entry.key_hash.is_some()
            {
                return Err(fail(
                    "World snapshot element kind or canonical hashes differ",
                ));
            }
            let identity = (&entry.field_id, entry.kind, entry.key_hash);
            if previous.as_ref().is_some_and(|prior| prior >= &identity) {
                return Err(fail(
                    "World snapshot repeats or reorders an element identity",
                ));
            }
            previous = Some(identity);
            // One canonical field cannot simultaneously be a table and a cell.
            if fields.contains(&(
                entry.field_id.as_str(),
                match entry.kind {
                    WorldStateElementKindV1::Table => WorldStateElementKindV1::Cell,
                    WorldStateElementKindV1::Cell => WorldStateElementKindV1::Table,
                },
            )) {
                return Err(fail("World snapshot gives one field incompatible kinds"));
            }
            fields.insert((entry.field_id.as_str(), entry.kind));
            let path = world_state_path_hash_v1(&entry.field_id, entry.kind)?;
            let element = world_state_element_v1(&path, entry.key_hash.as_ref(), &entry.value_hash);
            for (lane, bytes) in lanes.iter_mut().zip(element.chunks_exact(2)) {
                *lane = lane.wrapping_add(u16::from_le_bytes([bytes[0], bytes[1]]));
            }
        }
        Ok(world_state_root_from_accumulator_v1(
            self.schema_hash,
            self.entries.len() as u64,
            &lanes,
        ))
    }

    /// Authenticate the complete snapshot against an opaque certified successor block.
    /// The caller must independently select the verifier's real root and fresh tip.
    /// # Errors
    /// Genesis-only execution, invalid snapshot, or a different certified World root.
    pub fn authenticate(
        &self,
        block: &VerifiedSumeragiBlock,
    ) -> Result<VerifiedWorldStateSnapshotV1, FinalityError> {
        let world_root = self.root()?;
        if block.height() < 2 || world_root != block.execution().world_state_root {
            return Err(fail(
                "World snapshot differs from certified successor execution",
            ));
        }
        Ok(VerifiedWorldStateSnapshotV1 {
            height: block.height(),
            context_id: block.context_id(),
            world_root,
            entries: Arc::from(self.entries.clone()),
        })
    }
}

impl VerifiedWorldStateSnapshotV1 {
    /// Height of the same certified applied-cut World root.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.height
    }
    /// Exact certified consensus decision used to authenticate these values.
    #[must_use]
    pub fn context_id(&self) -> Hash {
        self.context_id
    }
    /// Certified complete World root, distinct from the ordinary write-witness root.
    #[must_use]
    pub fn world_root(&self) -> Hash {
        self.world_root
    }

    /// Prove that an exact native asset-definition key is absent from the complete
    /// certified World snapshot. The field and key type are fixed by the native
    /// authority registry; derived alias/domain indexes are not absence authority.
    ///
    /// The caller must select the real network, qualified schema and fresh applied
    /// cut independently. HTTP errors, an unavailable asset-specific publisher,
    /// unverified snapshot data or failed membership checks cannot call this API.
    ///
    /// # Errors
    /// Canonical key encoding fails, the exact definition exists, or the selected
    /// native field has an incompatible kind.
    pub fn verify_asset_definition_absent(
        &self,
        asset: &crate::asset::AssetDefinitionId,
    ) -> Result<(), FinalityError> {
        const FIELD: &str = "world.asset_definitions";
        let key = world_state_value_hash_v1(asset)?;
        if self
            .entries
            .iter()
            .any(|entry| entry.field_id == FIELD && entry.kind != WorldStateElementKindV1::Table)
        {
            return Err(fail("Asset definition field is not a native table"));
        }
        if self
            .entries
            .binary_search_by(|entry| {
                (entry.field_id.as_str(), entry.kind, entry.key_hash).cmp(&(
                    FIELD,
                    WorldStateElementKindV1::Table,
                    Some(key),
                ))
            })
            .is_ok()
        {
            return Err(fail(
                "Certified World snapshot contains the asset definition",
            ));
        }
        Ok(())
    }

    /// Verify exact canonical key and semantic value preimages for a native table row.
    /// # Errors
    /// Encoding error, absent row or changed canonical value.
    pub fn verify_table_value<K: norito::codec::Encode, V: norito::codec::Encode>(
        &self,
        field: &str,
        key: &K,
        value: &V,
    ) -> Result<(), FinalityError> {
        self.verify_value(
            field,
            WorldStateElementKindV1::Table,
            Some(world_state_value_hash_v1(key)?),
            world_state_value_hash_v1(value)?,
        )
    }
    /// Verify an exact canonical semantic cell value (e.g. the governed release registry).
    /// # Errors
    /// Encoding error, absent cell or changed canonical value.
    pub fn verify_cell_value<V: norito::codec::Encode>(
        &self,
        field: &str,
        value: &V,
    ) -> Result<(), FinalityError> {
        self.verify_value(
            field,
            WorldStateElementKindV1::Cell,
            None,
            world_state_value_hash_v1(value)?,
        )
    }
    fn verify_value(
        &self,
        field: &str,
        kind: WorldStateElementKindV1,
        key: Option<Hash>,
        value: Hash,
    ) -> Result<(), FinalityError> {
        world_state_path_hash_v1(field, kind)?;
        let index = self
            .entries
            .binary_search_by(|entry| {
                (entry.field_id.as_str(), entry.kind, entry.key_hash).cmp(&(field, kind, key))
            })
            .map_err(|_| fail("World snapshot does not contain the selected native value"))?;
        if self.entries[index].value_hash != value {
            return Err(fail("World snapshot canonical value preimage differs"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
