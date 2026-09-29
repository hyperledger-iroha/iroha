//! Private persistent commitment to the complete World value projection.
//!
//! Cold capture visits every authoritative value, including untouched values
//! and stores omitted by recovery JSON. Derived indexes and local delivery
//! buffers are excluded according to the exhaustive World authority registry.
//! Subsequent versions consume only the actual MV journals. Field identity,
//! physical keys and cell/storage kind are authenticated.
//! No loaded executor/trigger cache or MV undo history becomes a semantic leaf.
//!
//! This owner is not a complete State commitment or publication permission.
//! A lifecycle owner must retain the baseline of the SAME actual predecessor;
//! changed-key preimage checks cannot detect unrelated untouched-state drift.
//! TODO: validate every excluded derived index against its canonical sources,
//! then bind this baseline to the complete State commit-preparation capsule,
//! predecessor restoration and remaining aggregate allocation admission before cutover.

use super::{CellBlock, Hash, StorageBlock, WorldBlock, WorldProjection, hash_value};
use crate::state::authority_registry::{Canonical, Role, Schema, WORLD_FIELDS};
use iroha_crypto::{MerkleMap, MerkleMapError};
use iroha_data_model::musubi::{
    ArchiveId, MusubiArchiveAvailabilityV1, MusubiOrderedPackageEntryV1, MusubiPackageSelectorV1,
    MusubiReleaseIdV1, MusubiResolverReleaseRowV1,
};
use mv::allocation::AllocationBudget;
use mv::{Key, Value, storage::StorageReadOnly};
use norito::codec::Encode;

const SCHEMA_START: &[u8] = b"iroha:world-state:schema:start:v1\0";
const SCHEMA_FIELD: &[u8] = b"iroha:world-state:schema:field:v1\0";
const SCHEMA_VALUE: &[u8] = b"iroha:world-state:schema:value:v1\0";
const SCHEMA_NO_KEY: &[u8] = b"iroha:world-state:schema:no-key:v1\0";
const PATH: &[u8] = b"iroha:world-state:path:v1\0";
const ROOT: &[u8] = b"iroha:world-state:root:v1\0";

/// Owner-specific projection rejection or original local allocation failure.
#[derive(Debug, thiserror::Error)]
pub(in crate::state) enum WorldBaselineError {
    /// Existing semantic or codec diagnostics preserve their text and priority.
    #[error("{0}")]
    Semantic(String),
    /// A rejected update retains the exact original node-admission failure.
    #[error("World baseline update failed: {0}")]
    Update(#[from] MerkleMapError),
}

impl From<String> for WorldBaselineError {
    fn from(error: String) -> Self {
        Self::Semantic(error)
    }
}

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
    /// The lifecycle owner supplies its original pool; derived versions retain it.
    pub(in crate::state) fn capture_current(
        world: &WorldBlock<'_>,
        budget: &AllocationBudget,
    ) -> Result<Self, WorldBaselineError> {
        let mut builder = BaselineBuilder::for_world(MerkleMap::new(budget), Direction::Capture);
        world.project_world(&mut builder)?;
        Ok(builder.finish())
    }

    /// Cold capture of this overlay's exact World predecessor, including after
    /// MV replacement undo. Constructor/transaction touches are reversed using
    /// their actual first preimages. No current runtime metadata is inferred.
    pub(in crate::state) fn capture_predecessor(
        world: &WorldBlock<'_>,
        budget: &AllocationBudget,
    ) -> Result<Self, WorldBaselineError> {
        Self::capture_current(world, budget)?.transform(world, Direction::Reverse)
    }

    /// Apply the actual block delta to its retained predecessor baseline.
    ///
    /// Only touched entries are encoded. Every touched preimage, including a
    /// no-op, must match. Encoding, schema or preimage failure leaves `self`
    /// unchanged. The State lifecycle owner must guarantee untouched predecessor
    /// identity; this method cannot authenticate a caller-selected foreign World.
    pub(in crate::state) fn apply_block(
        &self,
        world: &WorldBlock<'_>,
    ) -> Result<Self, WorldBaselineError> {
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

    fn transform(
        &self,
        world: &WorldBlock<'_>,
        direction: Direction,
    ) -> Result<Self, WorldBaselineError> {
        let mut builder = BaselineBuilder::for_world(self.values.clone(), direction);
        world.project_world(&mut builder)?;
        if builder.schema != self.schema || builder.fields != self.fields {
            return Err(WorldBaselineError::Semantic(
                "World baseline schema does not match the actual field registry".into(),
            ));
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

fn authority_schemas(name: &str, kind: u8) -> Result<Option<(Option<Schema>, Schema)>, String> {
    let fields = if name.starts_with("triggers.") {
        crate::smartcontracts::isi::triggers::set::AUTHORITY_FIELDS
    } else {
        WORLD_FIELDS
    };
    let field = fields
        .iter()
        .find(|field| {
            if name.starts_with("triggers.") {
                field.id == name
            } else {
                field.id.strip_prefix("world.") == Some(name)
            }
        })
        .ok_or_else(|| format!("unclassified World baseline field: {name}"))?;
    match field.role {
        Role::Canonical(Canonical::Table { key, value }) if kind == 0 => {
            Ok(Some((Some(key), value)))
        }
        Role::Canonical(Canonical::Cell(value)) if kind == 1 => Ok(Some((None, value))),
        Role::Canonical(_) => Err(format!("World baseline field kind mismatch: {name}")),
        Role::Derived { .. } | Role::Local(_) => Ok(None),
        Role::History { .. } => Err(format!(
            "historical World field has no baseline owner: {name}"
        )),
    }
}

#[cfg(test)]
fn is_world_authority(name: &str, kind: u8) -> Result<bool, String> {
    authority_schemas(name, kind).map(|schemas| schemas.is_some())
}

fn schema_fingerprint(schema: Schema) -> Result<Hash, String> {
    let (kind, identity, layout) = match schema {
        Schema::Norito {
            nominal_name,
            layout,
        } => (0_u8, nominal_name(), layout),
        Schema::Semantic {
            identity, layout, ..
        } => (1_u8, identity.to_owned(), layout),
        Schema::Required { identity, .. } => {
            return Err(format!(
                "World baseline cannot authenticate unresolved schema {identity}"
            ));
        }
    };
    let length = u64::try_from(identity.len()).map_err(|_| "World schema identity exceeds u64")?;
    Ok(Hash::new_from_chunks(&[
        SCHEMA_VALUE,
        &[kind, layout.major, layout.minor, layout.flags],
        &length.to_le_bytes(),
        identity.as_bytes(),
    ]))
}

struct BaselineBuilder {
    values: MerkleMap,
    schema: Hash,
    fields: u64,
    direction: Direction,
    authoritative_only: bool,
}

impl BaselineBuilder {
    fn new(values: MerkleMap, direction: Direction) -> Self {
        Self {
            values,
            schema: Hash::new(SCHEMA_START),
            fields: 0,
            direction,
            authoritative_only: false,
        }
    }

    fn for_world(values: MerkleMap, direction: Direction) -> Self {
        Self {
            authoritative_only: true,
            ..Self::new(values, direction)
        }
    }

    fn field(&mut self, name: &'static str, kind: u8) -> Result<Option<Hash>, String> {
        let schemas = if self.authoritative_only {
            match authority_schemas(name, kind)? {
                Some(schemas) => Some(schemas),
                None => return Ok(None),
            }
        } else {
            None
        };
        let len = u64::try_from(name.len()).map_err(|_| "World field name exceeds u64")?;
        self.fields = self
            .fields
            .checked_add(1)
            .ok_or("World baseline field count overflow")?;
        let field = Hash::new_from_chunks(&[PATH, &[kind], &len.to_le_bytes(), name.as_bytes()]);
        let (key_schema, value_schema) = match schemas {
            Some((key, value)) => (
                key.map(schema_fingerprint)
                    .transpose()?
                    .unwrap_or_else(|| Hash::new(SCHEMA_NO_KEY)),
                schema_fingerprint(value)?,
            ),
            None => (Hash::new(SCHEMA_NO_KEY), Hash::new(SCHEMA_NO_KEY)),
        };
        self.schema = Hash::new_from_chunks(&[
            SCHEMA_FIELD,
            self.schema.as_ref(),
            field.as_ref(),
            key_schema.as_ref(),
            value_schema.as_ref(),
        ]);
        Ok(Some(field))
    }

    fn replace(
        &mut self,
        field: Hash,
        key: Option<Hash>,
        before: Option<Hash>,
        after: Option<Hash>,
    ) -> Result<(), WorldBaselineError> {
        let key = match key {
            Some(key) => Hash::new_from_chunks(&[PATH, field.as_ref(), &[1], key.as_ref()]),
            None => Hash::new_from_chunks(&[PATH, field.as_ref(), &[0]]),
        };
        let (expected, value) = match self.direction {
            Direction::Reverse => (after, before),
            Direction::Capture | Direction::Forward => (before, after),
        };
        self.values.replace(key, expected, value)?;
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
    type Error = WorldBaselineError;
    fn append_musubi_archive_availability(
        &mut self,
        storage: &StorageBlock<'_, ArchiveId, MusubiArchiveAvailabilityV1>,
    ) -> Result<(), Self::Error> {
        self.append_storage_with("musubi_archive_availability", storage, |row| {
            let anchor = crate::state::authority_registry::world::musubi_availability_policy::MusubiAvailabilityAuthorityV1::from_record(row);
            hash_value(&anchor)
        })
    }

    fn append_musubi_resolver_index(
        &mut self,
        storage: &StorageBlock<'_, MusubiReleaseIdV1, MusubiResolverReleaseRowV1>,
    ) -> Result<(), Self::Error> {
        self.append_storage_with("musubi_resolver_index", storage, |row| {
            let authority = crate::state::authority_registry::world::musubi_universal_policy::MusubiResolverAuthorityV1::from_record(row);
            hash_value(&authority)
        })
    }

    fn append_musubi_public_directory(
        &mut self,
        storage: &StorageBlock<'_, MusubiPackageSelectorV1, MusubiOrderedPackageEntryV1>,
    ) -> Result<(), Self::Error> {
        self.append_storage_with("musubi_public_directory", storage, |row| {
            let authority = crate::state::authority_registry::world::musubi_universal_policy::MusubiDirectoryAuthorityV1::from_record(row);
            hash_value(&authority)
        })
    }

    fn append_storage_with<K: Key + Encode, V: Value, M: mv::storage::StorageMode<K, V>>(
        &mut self,
        name: &'static str,
        storage: &StorageBlock<'_, K, V, M>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), Self::Error> {
        let Some(field) = self.field(name, 0)? else {
            return Ok(());
        };
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

    fn append_cell_with<V: Value, C: Send + Sync + 'static>(
        &mut self,
        name: &'static str,
        cell: &CellBlock<'_, V, C>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), Self::Error> {
        let Some(field) = self.field(name, 1)? else {
            return Ok(());
        };
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

#[cfg(test)]
#[path = "world_baseline/funding_tests.rs"]
mod funding_tests;
