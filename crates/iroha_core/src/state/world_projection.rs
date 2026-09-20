//! State-owned net changes from the actual World overlay journals.
//!
//! This private projection covers the centralized World field inventory and the
//! trigger owner's semantic stores. It binds before/after values, not mutable
//! access history, and never uses caller access hints. Fixed V1 bare Norito is
//! streamed into domain-separated hashes; no encoded change list is retained.
//!
//! This is NOT a complete State root or a read witness. Untouched values require
//! an authenticated persistent baseline; State-owned membership/runtime fields,
//! process event delivery and post-finality effects have separate owners.
//! TODO: compose the complete canonical State tree, lifecycle and agreed resource
//! admission before changing consensus commitments or enabling publication.

use super::{CellBlock, StorageBlock, World, WorldBlock};
use iroha_crypto::Hash;
use mv::{Key, Value};
use norito::codec::Encode;

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
    Hash::new_from_writer(|mut writer| {
        writer.write_all(VALUE_DOMAIN)?;
        let len = norito::codec::encode_adaptive_into(value, &mut writer)
            .map_err(std::io::Error::other)?;
        let len = u64::try_from(len)
            .map_err(|_| std::io::Error::other("canonical value length exceeds u64"))?;
        writer.write_all(&len.to_le_bytes())
    })
    .map_err(|error| format!("World net-delta value encoding failed: {error}"))
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

    /// Append a storage's exact borrowed net changes, using its owner's value projection.
    /// No `is_dirty` shortcut can discard an explicit absent-to-absent journal row.
    pub(crate) fn append_storage_with<K: Key + Encode, V: Value>(
        &mut self,
        name: &'static str,
        storage: &StorageBlock<'_, K, V>,
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

    fn append_cell_with<V: Value>(
        &mut self,
        name: &'static str,
        cell: &CellBlock<'_, V>,
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

/// Exhaustive semantic World visitor shared by delta and persistent baseline owners.
/// Each owner supplies the hash of its actual borrowed value, excluding caches.
pub(crate) trait WorldProjection {
    fn append_storage_with<K: Key + Encode, V: Value>(
        &mut self,
        name: &'static str,
        storage: &StorageBlock<'_, K, V>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), String>;

    fn append_cell_with<V: Value>(
        &mut self,
        name: &'static str,
        cell: &CellBlock<'_, V>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), String>;
}

impl<T: WorldProjection> WorldProjection for &mut T {
    fn append_storage_with<K: Key + Encode, V: Value>(
        &mut self,
        name: &'static str,
        storage: &StorageBlock<'_, K, V>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), String> {
        (**self).append_storage_with(name, storage, encode)
    }
    fn append_cell_with<V: Value>(
        &mut self,
        name: &'static str,
        cell: &CellBlock<'_, V>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), String> {
        (**self).append_cell_with(name, cell, encode)
    }
}

impl WorldProjection for WorldDeltaBuilder {
    fn append_storage_with<K: Key + Encode, V: Value>(
        &mut self,
        name: &'static str,
        storage: &StorageBlock<'_, K, V>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), String> {
        Self::append_storage_with(self, name, storage, encode)
    }

    fn append_cell_with<V: Value>(
        &mut self,
        name: &'static str,
        cell: &CellBlock<'_, V>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), String> {
        Self::append_cell_with(self, name, cell, encode)
    }
}

trait AppendWorldField {
    fn append_world_field(
        &self,
        name: &'static str,
        builder: &mut impl WorldProjection,
    ) -> Result<(), String>;
}

impl<K: Key + Encode, V: Value + Encode> AppendWorldField for StorageBlock<'_, K, V> {
    fn append_world_field(
        &self,
        name: &'static str,
        builder: &mut impl WorldProjection,
    ) -> Result<(), String> {
        builder.append_storage_with(name, self, hash_value)
    }
}

impl<V: Value + Encode> AppendWorldField for CellBlock<'_, V> {
    fn append_world_field(
        &self,
        name: &'static str,
        builder: &mut impl WorldProjection,
    ) -> Result<(), String> {
        builder.append_cell_with(name, self, hash_value)
    }
}

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

    fn project_world(&self, mut builder: &mut impl WorldProjection) -> Result<(), String> {
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

#[path = "world_baseline.rs"]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TODO: connect retained journals to the consuming State publisher"
    )
)]
mod world_baseline;
pub(in crate::state) use world_baseline::WorldStateBaseline;
