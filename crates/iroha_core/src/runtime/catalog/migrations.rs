//! Native migrations this release compiles (`specs/live_runtime_upgrades.md` §5.4).
//!
//! A migration moves the rows of old World storage into successor storage while the chain keeps
//! committing. The kernel ([`crate::runtime::migration`]) drives it; the release supplies the
//! code that knows the two layouts:
//!
//! - The stage copies at most a bounded number of old rows after the cursor
//!   into the successor and returns the new cursor;
//! - The barrier mirrors, at the end of every block, the old rows that block
//!   touched at or before the cursor, so the staged part of the successor stays exact;
//! - The drain deletes a bounded number of old rows after the cutover;
//! - The stale-write check reports a write to the old storage after the cutover.
//!
//! The catalog publishes one migration definition per declaration and execution services
//! profile it compiles, so a root running any of them can activate the migration. The successor
//! storage is storage an addition adds ([`super::additions`]); this release declares none.
//! Callbacks receive only pre-bound storage capabilities from an engine-owned typed adapter
//! ([`crate::runtime::migration::storage`]). The first adapter supports one source and one
//! successor table. Declaring another field or shape fails before the code can run; no callback
//! receives the World or an arbitrary projection, and no table can be opened after writes start.

use crate::runtime::migration::storage::MigrationProgram;

/// One native migration between World fields.
#[derive(Clone, Copy, Debug)]
pub struct MigrationAddition {
    /// Native module of the migration code; unique among migrations and additions.
    pub(crate) module: &'static str,
    /// Registry identities of the old World fields it reads and later drains.
    pub(crate) from: &'static [&'static str],
    /// Registry identities of the successor World fields it fills.
    pub(crate) to: &'static [&'static str],
    /// Codec identity of the committed cursor.
    pub(crate) cursor: &'static str,
    /// Most old rows one stage copies or drains.
    pub(crate) rows_per_stage: u32,
    /// Sealed engine adapter issuing only this declaration's typed storage capabilities.
    pub(crate) code: &'static dyn MigrationProgram,
}

/// What one copying stage did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageOutcome {
    /// The cursor after the stage: every old row at or before it is staged.
    pub cursor: Vec<u8>,
    /// Old rows the stage copied.
    pub rows: u32,
    /// Whether no old row lies after the cursor.
    pub done: bool,
}

/// What one draining stage did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrainOutcome {
    /// Old rows the stage deleted.
    pub rows: u32,
    /// Whether the old storage is empty.
    pub empty: bool,
}

/// The native migrations of this release.
pub const MIGRATIONS: &[MigrationAddition] = &[];

/// A migration of a hypothetical later release, over World fields the real baseline holds.
#[cfg(test)]
pub mod test_migrations {
    use super::{DrainOutcome, MigrationAddition, StageOutcome};
    use crate::runtime::migration::MigrationResult;
    use crate::runtime::migration::storage::{
        BarrierStorage, PointAndScan, SEQUENCE_TABLES, SourceChanges, SourceDrain, StageStorage,
        TableMigration,
    };
    use iroha_crypto::Hash;
    use iroha_data_model::account::AccountId;

    /// Module of [`SEQUENCES`].
    pub const MODULE: &str = "iroha.test.migration.sequences";
    /// The old table [`SEQUENCES`] reads. Like [`TO`], it is storage the test addition
    /// [`super::super::additions::test_additions::MIGRATED`] adds.
    pub const FROM: &str = "world.tx_sequences";
    /// The successor table [`SEQUENCES`] fills.
    pub const TO: &str = "world.viral_bonus_paid";

    /// The successor row of one old row: the hash of the account, and whether its sequence is
    /// odd. Every sequence has a successor, including the copying stage's fault sentinel, so
    /// later source writes remain representable while the barrier holds.
    pub fn successor(account: &AccountId, sequence: u64) -> MigrationResult<(Hash, bool)> {
        let key = codec(|| norito::encode_canonical(account))?;
        Ok((Hash::new(key), sequence % 2 == 1))
    }

    fn codec<T>(operation: impl FnOnce() -> Result<T, norito::Error>) -> MigrationResult<T> {
        norito::core::classify_decode_attempt(operation).map_err(|original| {
            crate::execution_attempt::canonical_decode_attempt_error(original, |error| {
                error.to_string()
            })
        })
    }

    fn cursor_account(cursor: &[u8]) -> MigrationResult<Option<AccountId>> {
        if cursor.is_empty() {
            return Ok(None);
        }
        codec(|| norito::decode_canonical(cursor)).map(Some)
    }

    pub(crate) fn stage(
        mut storage: StageStorage<'_, '_, PointAndScan, AccountId, u64, Hash, bool>,
        cursor: &[u8],
        limit: u32,
    ) -> MigrationResult<StageOutcome> {
        let after = cursor_account(cursor)?;
        let limit = usize::try_from(limit).map_err(|error| error.to_string())?;
        // Borrow original source accounts: only fixed row slots are owned here.
        let capacity = storage.source.scan_after(after.as_ref(), limit).count();
        let mut batch = storage.scratch.buffer(capacity)?;
        let mut done = true;
        for (account, sequence) in storage
            .source
            .scan_after(after.as_ref(), limit.saturating_add(1))
        {
            if batch.as_slice().len() == limit {
                done = false;
                break;
            }
            // Inject a deterministic copying-stage failure before any successor write. This
            // tests stage atomicity without making the live barrier's row transform partial.
            if *sequence == u64::MAX {
                return Err(format!("{account} holds the copying-stage fault sentinel").into());
            }
            batch.push_reserved((account, successor(account, *sequence)?));
        }
        let cursor = match batch.as_slice().last() {
            Some((account, _)) => codec(|| norito::encode_canonical(*account))?,
            None => cursor.to_vec(),
        };
        let rows = u32::try_from(batch.as_slice().len()).map_err(|error| error.to_string())?;
        for (_, (key, value)) in batch.as_slice() {
            storage.successor.insert(*key, *value);
        }
        Ok(StageOutcome { cursor, rows, done })
    }

    pub(crate) fn barrier(
        mut storage: BarrierStorage<'_, '_, '_, AccountId, u64, Hash, bool>,
        cursor: &[u8],
    ) -> MigrationResult<()> {
        let Some(staged) = cursor_account(cursor)? else {
            return Ok(());
        };
        // Every codec operation finishes before the first mutation. Neither malformed
        // cursor nor a locally unavailable transform becomes an ignored touched row.
        let capacity = storage
            .source
            .touched_entries()
            .filter(|entry| *entry.key <= staged)
            .count();
        let mut mirrored = storage.scratch.buffer(capacity)?;
        for entry in storage
            .source
            .touched_entries()
            .filter(|entry| *entry.key <= staged)
        {
            let (key, value) = successor(entry.key, entry.after.copied().unwrap_or(0))?;
            mirrored.push_reserved((key, entry.after.map(|_| value)));
        }
        for &(key, value) in mirrored.as_slice() {
            match value {
                Some(value) => storage.successor.insert(key, value),
                None => storage.successor.remove(key),
            }
        }
        Ok(())
    }

    pub(crate) fn drain(
        mut source: SourceDrain<'_, '_, AccountId, u64>,
        limit: u32,
    ) -> MigrationResult<DrainOutcome> {
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        let batch: Vec<AccountId> = source.keys(limit).cloned().collect();
        let rows = u32::try_from(batch.len()).unwrap_or(u32::MAX);
        for account in batch {
            source.remove(account);
        }
        Ok(DrainOutcome {
            rows,
            empty: source.is_empty(),
        })
    }

    fn stale(source: SourceChanges<'_, '_, AccountId, u64>) -> bool {
        source.touched_entries().any(|entry| entry.after.is_some())
    }

    static CODE: TableMigration<PointAndScan, AccountId, u64, Hash, bool> =
        SEQUENCE_TABLES.migration(stage, barrier, drain, stale);

    /// Moves `world.tx_sequences` into `world.viral_bonus_paid`, two rows a stage.
    pub const SEQUENCES: MigrationAddition = MigrationAddition {
        module: MODULE,
        from: &[FROM],
        to: &[TO],
        cursor: "norito:iroha_data_model::account::AccountId@test",
        rows_per_stage: 2,
        code: &CODE,
    };
}
