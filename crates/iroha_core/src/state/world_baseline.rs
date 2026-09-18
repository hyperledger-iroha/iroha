//! Private persistent commitment to the complete World value projection.
//!
//! Cold capture visits all values, including untouched values and stores omitted
//! by recovery JSON. Subsequent versions consume only the actual MV journals.
//! Field/schema identity, physical keys and cell/storage kind are authenticated.
//! No loaded executor/trigger cache or MV undo history becomes a semantic leaf.
//!
//! This owner is not a complete State commitment or publication permission.
//! A lifecycle owner must retain the baseline of the SAME actual predecessor;
//! changed-key preimage checks cannot detect unrelated untouched-state drift.
//! TODO: bind it to the complete State commit-preparation capsule, predecessor
//! restoration and aggregate allocation admission before production cutover.

use super::{CellBlock, Hash, StorageBlock, WorldBlock, WorldProjection, hash_value};
use iroha_crypto::MerkleMap;
use mv::{Key, Value, storage::StorageReadOnly};
use norito::codec::Encode;

const SCHEMA_START: &[u8] = b"iroha:world-state:schema:start:v1\0";
const SCHEMA_FIELD: &[u8] = b"iroha:world-state:schema:field:v1\0";
const PATH: &[u8] = b"iroha:world-state:path:v1\0";
const ROOT: &[u8] = b"iroha:world-state:root:v1\0";

/// Immutable-node baseline, constructed only by visiting actual World stores.
/// Cloning retains one version; updates publish a new version only on success.
#[derive(Clone)]
pub(in crate::state) struct WorldStateBaseline {
    values: MerkleMap,
    schema: Hash,
    fields: u64,
}

impl WorldStateBaseline {
    /// Cold capture of the current overlay's complete World values, in O(N).
    /// This is an initialization/recovery operation, not a per-block hot path.
    pub(in crate::state) fn capture_current(world: &WorldBlock<'_>) -> Result<Self, String> {
        let mut builder = BaselineBuilder::new(MerkleMap::new(), Direction::Capture);
        world.project_world(&mut builder)?;
        Ok(builder.finish())
    }

    /// Cold capture of this overlay's exact World predecessor, including after
    /// MV replacement undo. Constructor/transaction touches are reversed using
    /// their actual first preimages. No current runtime metadata is inferred.
    pub(in crate::state) fn capture_predecessor(world: &WorldBlock<'_>) -> Result<Self, String> {
        Self::capture_current(world)?.transform(world, Direction::Reverse)
    }

    /// Apply the actual block delta to its retained predecessor baseline.
    ///
    /// Only touched entries are encoded. Every touched preimage, including a
    /// no-op, must match. Encoding, schema or preimage failure leaves `self`
    /// unchanged. The State lifecycle owner must guarantee untouched predecessor
    /// identity; this method cannot authenticate a caller-selected foreign World.
    pub(in crate::state) fn apply_block(&self, world: &WorldBlock<'_>) -> Result<Self, String> {
        self.transform(world, Direction::Forward)
    }

    /// Commitment to all current World values and the exhaustive field schema.
    pub(in crate::state) fn root(&self) -> Hash {
        Hash::new_from_chunks(&[
            ROOT,
            self.schema.as_ref(),
            &self.fields.to_le_bytes(),
            self.values.root().as_ref(),
        ])
    }

    fn transform(&self, world: &WorldBlock<'_>, direction: Direction) -> Result<Self, String> {
        let mut builder = BaselineBuilder::new(self.values.clone(), direction);
        world.project_world(&mut builder)?;
        if builder.schema != self.schema || builder.fields != self.fields {
            return Err("World baseline schema does not match the actual field registry".into());
        }
        Ok(builder.finish())
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Capture,
    Forward,
    Reverse,
}

struct BaselineBuilder {
    values: MerkleMap,
    schema: Hash,
    fields: u64,
    direction: Direction,
}

impl BaselineBuilder {
    fn new(values: MerkleMap, direction: Direction) -> Self {
        Self {
            values,
            schema: Hash::new(SCHEMA_START),
            fields: 0,
            direction,
        }
    }

    fn field(&mut self, name: &'static str, kind: u8) -> Result<Hash, String> {
        let len = u64::try_from(name.len()).map_err(|_| "World field name exceeds u64")?;
        self.fields = self
            .fields
            .checked_add(1)
            .ok_or("World baseline field count overflow")?;
        let field = Hash::new_from_chunks(&[PATH, &[kind], &len.to_le_bytes(), name.as_bytes()]);
        self.schema = Hash::new_from_chunks(&[SCHEMA_FIELD, self.schema.as_ref(), field.as_ref()]);
        Ok(field)
    }

    fn replace(
        &mut self,
        field: Hash,
        key: Option<Hash>,
        before: Option<Hash>,
        after: Option<Hash>,
    ) -> Result<(), String> {
        let key = match key {
            Some(key) => Hash::new_from_chunks(&[PATH, field.as_ref(), &[1], key.as_ref()]),
            None => Hash::new_from_chunks(&[PATH, field.as_ref(), &[0]]),
        };
        let (expected, value) = match self.direction {
            Direction::Reverse => (after, before),
            Direction::Capture | Direction::Forward => (before, after),
        };
        self.values
            .replace(key, expected, value)
            .map_err(|error| format!("World baseline update failed: {error}"))?;
        Ok(())
    }

    fn finish(self) -> WorldStateBaseline {
        WorldStateBaseline {
            values: self.values,
            schema: self.schema,
            fields: self.fields,
        }
    }
}

impl WorldProjection for BaselineBuilder {
    fn append_storage_with<K: Key + Encode, V: Value>(
        &mut self,
        name: &'static str,
        storage: &StorageBlock<'_, K, V>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), String> {
        let field = self.field(name, 0)?;
        match self.direction {
            Direction::Capture => {
                for (key, value) in storage.iter() {
                    self.replace(field, Some(hash_value(key)?), None, Some(encode(value)?))?;
                }
            }
            Direction::Forward | Direction::Reverse => {
                for entry in storage.touched_entries() {
                    self.replace(
                        field,
                        Some(hash_value(entry.key)?),
                        entry.before.map(&encode).transpose()?,
                        entry.after.map(&encode).transpose()?,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn append_cell_with<V: Value>(
        &mut self,
        name: &'static str,
        cell: &CellBlock<'_, V>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), String> {
        let field = self.field(name, 1)?;
        match self.direction {
            Direction::Capture => self.replace(field, None, None, Some(encode(cell.get())?))?,
            Direction::Forward | Direction::Reverse => {
                if let Some(value) = cell.touched_value() {
                    self.replace(
                        field,
                        None,
                        Some(encode(value.before)?),
                        Some(encode(value.after)?),
                    )?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "world_baseline_tests.rs"]
mod tests;
