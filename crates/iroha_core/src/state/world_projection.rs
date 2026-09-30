//! State-owned net changes from the actual World overlay journals.
//!
//! This private projection covers the centralized World field inventory and the
//! trigger owner's semantic stores. It binds before/after values, not mutable
//! access history, and never uses caller access hints. Fixed V1 bare Norito is
//! streamed into domain-separated hashes; no encoded change list is retained.
//!
//! A net delta binds changes only, not a read witness. The complete World state
//! commitment bound in the execution result `R` is [`WorldStateAccumulator`],
//! which consumes the same visitor. State-owned membership/runtime fields,
//! process event delivery and post-finality effects have separate owners.
//! TODO(S9): State-level canonical fields outside World (transaction membership,
//! canonical runtime policy, commit topologies) are not in the World accumulator;
//! accelerated restoration must authenticate them separately.

use super::{CellField, StorageField, World, WorldBlock};
use iroha_crypto::Hash;
use iroha_data_model::musubi::{
    ArchiveId, MusubiArchiveAvailabilityV1, MusubiOrderedPackageEntryV1, MusubiPackageSelectorV1,
    MusubiReleaseIdV1, MusubiResolverReleaseRowV1,
};
use mv::{Key, Value};
use norito::codec::Encode;

#[cfg(test)]
const VALUE_DOMAIN: &[u8] = b"iroha:world-net-delta:value:bare-v1\0";
const START_DOMAIN: &[u8] = b"iroha:world-net-delta:start:v1\0";
const PUBLICATION_START_DOMAIN: &[u8] = b"iroha:world-publication-journal:start:v1\0";
const FIELD_DOMAIN: &[u8] = b"iroha:world-net-delta:field:v1\0";
const ENTRY_DOMAIN: &[u8] = b"iroha:world-net-delta:entry:v1\0";
const END_FIELD_DOMAIN: &[u8] = b"iroha:world-net-delta:end-field:v1\0";
const FINISH_DOMAIN: &[u8] = b"iroha:world-net-delta:finish:v1\0";

/// Exact private net-delta binding produced from actual borrowed overlay entries.
/// It contains no copied values and cannot authorize a complete State commitment.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct WorldNetDelta {
    root: Hash,
    changed_values: u64,
    fields: u64,
}

/// Exact pending current/undo changes, including same-value journal touches.
/// Unlike the semantic net delta, this binds the undo representation published
/// by the actual journals without visiting untouched historical values.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::state) struct WorldPublicationDelta(WorldNetDelta);

impl WorldNetDelta {
    /// Number of distinct keys/cells whose canonical before/after values differ.
    #[cfg(test)]
    pub(crate) fn changed_values(&self) -> u64 {
        self.changed_values
    }
}

/// Streaming fold shared with the trigger owner, which owns its private stores.
/// Construction is not publication authority; State retains its own actual fold.
pub(crate) struct WorldDeltaBuilder {
    accumulator: Hash,
    changed_values: u64,
    fields: u64,
    open_field: bool,
    retain_noop_touches: bool,
}

/// Hash one canonical semantic value without allocating its encoded payload.
/// The domain fixes the bare Norito V1 layout and binds its exact encoded length.
pub(crate) fn hash_value<T: Encode>(value: &T) -> Result<Hash, String> {
    iroha_data_model::sumeragi_finality::world_state_value_hash_v1(value)
        .map_err(|error| error.to_string())
}

impl WorldDeltaBuilder {
    /// Start one field-ordered fold. Empty fields remain part of its schema.
    pub(crate) fn new() -> Self {
        Self {
            accumulator: Hash::new(START_DOMAIN),
            changed_values: 0,
            fields: 0,
            open_field: false,
            retain_noop_touches: false,
        }
    }

    fn for_publication() -> Self {
        Self {
            accumulator: Hash::new(PUBLICATION_START_DOMAIN),
            retain_noop_touches: true,
            ..Self::new()
        }
    }

    fn begin_field(&mut self, name: &'static str, kind: u8) -> Result<(), String> {
        if self.open_field {
            return Err("World net-delta fold retains an incomplete field".into());
        }
        // Retain this latch through an error or unwind; no partial fold can finish.
        self.open_field = true;
        let len = u64::try_from(name.len()).map_err(|_| "World field name exceeds u64")?;
        self.fields = self
            .fields
            .checked_add(1)
            .ok_or("World field count overflow")?;
        self.accumulator = Hash::new_from_chunks(&[
            FIELD_DOMAIN,
            self.accumulator.as_ref(),
            &[kind],
            &len.to_le_bytes(),
            name.as_bytes(),
        ]);
        Ok(())
    }

    fn append_change(
        &mut self,
        key: Option<Hash>,
        before: Option<Hash>,
        after: Option<Hash>,
    ) -> Result<(), String> {
        self.changed_values = self
            .changed_values
            .checked_add(1)
            .ok_or("World changed-value count overflow")?;
        // Presence is explicit. An absent value and an encoded empty value
        // therefore remain distinct even if a hash equals the zero filler.
        let zero = [0_u8; Hash::LENGTH];
        self.accumulator = Hash::new_from_chunks(&[
            ENTRY_DOMAIN,
            self.accumulator.as_ref(),
            &[u8::from(key.is_some())],
            key.as_ref()
                .map_or(zero.as_slice(), |hash| hash.as_ref().as_slice()),
            &[u8::from(before.is_some())],
            before
                .as_ref()
                .map_or(zero.as_slice(), |hash| hash.as_ref().as_slice()),
            &[u8::from(after.is_some())],
            after
                .as_ref()
                .map_or(zero.as_slice(), |hash| hash.as_ref().as_slice()),
        ]);
        Ok(())
    }

    fn end_field(&mut self, before: u64) {
        self.accumulator = Hash::new_from_chunks(&[
            END_FIELD_DOMAIN,
            self.accumulator.as_ref(),
            &(self.changed_values - before).to_le_bytes(),
        ]);
        self.open_field = false;
    }

    /// Append an original field's exact borrowed net changes in either readable phase.
    /// Its owner supplies the value projection without regaining execution authority.
    /// No `is_dirty` shortcut can discard an explicit absent-to-absent journal row.
    pub(crate) fn append_storage_with<
        K: Key + Encode,
        V: Value,
        M: mv::storage::StorageMode<K, V>,
    >(
        &mut self,
        name: &'static str,
        storage: &StorageField<'_, K, V, M>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), String> {
        self.begin_field(name, 0)?;
        let start = self.changed_values;
        for entry in storage.touched_entries() {
            let before = entry.before.map(&encode).transpose()?;
            let after = entry.after.map(&encode).transpose()?;
            if self.retain_noop_touches || before != after {
                self.append_change(Some(hash_value(entry.key)?), before, after)?;
            }
        }
        self.end_field(start);
        Ok(())
    }

    fn append_cell_with<V: Value, C: Send + Sync + 'static>(
        &mut self,
        name: &'static str,
        cell: &CellField<'_, V, C>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), String> {
        self.begin_field(name, 1)?;
        let start = self.changed_values;
        if let Some(value) = cell.touched_value() {
            let before = encode(value.before)?;
            let after = encode(value.after)?;
            if self.retain_noop_touches || before != after {
                self.append_change(None, Some(before), Some(after))?;
            }
        }
        self.end_field(start);
        Ok(())
    }

    /// Consume a completed fold; errors and unwind cannot produce a partial root.
    pub(crate) fn finish(self) -> Result<WorldNetDelta, String> {
        if self.open_field {
            return Err("World net-delta fold retains an incomplete field".into());
        }
        Ok(WorldNetDelta {
            root: Hash::new_from_chunks(&[
                FINISH_DOMAIN,
                self.accumulator.as_ref(),
                &self.changed_values.to_le_bytes(),
                &self.fields.to_le_bytes(),
            ]),
            changed_values: self.changed_values,
            fields: self.fields,
        })
    }
}

#[path = "world_projection/visitor.rs"]
mod visitor;
use visitor::AppendWorldField;
pub(crate) use visitor::WorldProjection;

macro_rules! append_world_field {
    ($world:ident, $builder:ident, executor) => {
        $builder.append_cell_with(
            "executor",
            &$world.executor,
            crate::executor::executor_norito::net_state_hash,
        )?;
    };
    ($world:ident, $builder:ident, triggers) => {
        $world.triggers.append_world_projection(&mut $builder)?;
    };
    ($world:ident, $builder:ident, musubi_archive_availability) => {
        $builder.append_musubi_archive_availability(&$world.musubi_archive_availability)?;
    };
    ($world:ident, $builder:ident, musubi_resolver_index) => {
        $builder.append_musubi_resolver_index(&$world.musubi_resolver_index)?;
    };
    ($world:ident, $builder:ident, musubi_public_directory) => {
        $builder.append_musubi_public_directory(&$world.musubi_public_directory)?;
    };
    ($world:ident, $builder:ident, $field:ident) => {
        $world
            .$field
            .append_world_field(stringify!($field), &mut $builder)?;
    };
}

macro_rules! append_world_fields {
    ($world:ident, $builder:ident; [$($prefix:ident),* $(,)?]
        [$($privacy:ident),* $(,)?] [$($suffix:ident),* $(,)?]) => {
        $(append_world_field!($world, $builder, $prefix);)*
        $(append_world_field!($world, $builder, $privacy);)*
        $(append_world_field!($world, $builder, $suffix);)*
    };
}

macro_rules! append_publication_identity {
    ($world:ident, $expected:ident, $mode:ident, $identities:ident, triggers) => {
        $world.triggers.append_world_publication_identities(
            &$expected.triggers,
            $mode,
            &mut $identities,
        )?;
    };
    ($world:ident, $expected:ident, $mode:ident, $identities:ident, $field:ident) => {
        if !$world.$field.belongs_to(&$expected.$field) || $world.$field.mode() != $mode {
            return Err(concat!(
                "publication World journal has foreign owner or mode: ",
                stringify!($field)
            )
            .into());
        }
        $identities.push($world.$field.publication_identity());
    };
}

macro_rules! append_publication_identities {
    ($world:ident, $expected:ident, $mode:ident, $identities:ident;
        [$($prefix:ident),* $(,)?] [$($privacy:ident),* $(,)?] [$($suffix:ident),* $(,)?]) => {
        $(append_publication_identity!($world, $expected, $mode, $identities, $prefix);)*
        $(append_publication_identity!($world, $expected, $mode, $identities, $privacy);)*
        $(append_publication_identity!($world, $expected, $mode, $identities, $suffix);)*
    };
}

impl WorldBlock<'_> {
    /// Bind each actual original journal to its expected World owner and mode.
    /// The fixed field inventory visits no values and exposes no publication power.
    pub(in crate::state) fn publication_identities(
        &self,
        expected: &World,
        mode: mv::BlockMode,
    ) -> Result<Vec<mv::BlockPublicationIdentity>, String> {
        let mut identities = Vec::new();
        with_world_overlay_fields!(
            append_publication_identities,
            self,
            expected,
            mode,
            identities
        );
        Ok(identities)
    }

    fn project_world<P: WorldProjection>(&self, mut builder: &mut P) -> Result<(), P::Error> {
        with_world_overlay_fields!(append_world_fields, self, builder);
        Ok(())
    }

    /// Capture all centralized World fields from their actual first preimages.
    /// Runtime caches are projected by their owners; derived State indexes remain
    /// visible because they can change execution. Process events are not a WSV leaf.
    pub(in crate::state) fn net_state_delta(&self) -> Result<WorldNetDelta, String> {
        let mut builder = WorldDeltaBuilder::new();
        self.project_world(&mut builder)?;
        builder.finish()
    }

    /// Bind every touched preimage and current value, including no-op writes.
    /// Canonical snapshots retain map undo entries even when values are equal.
    pub(in crate::state) fn publication_state_delta(
        &self,
    ) -> Result<WorldPublicationDelta, String> {
        let mut builder = WorldDeltaBuilder::for_publication();
        self.project_world(&mut builder)?;
        builder.finish().map(WorldPublicationDelta)
    }
}

#[cfg(test)]
#[path = "world_projection_tests.rs"]
mod tests;

#[path = "world_state_accumulator.rs"]
pub(crate) mod world_state_accumulator;
pub(crate) use world_state_accumulator::WorldStateAccumulator;
