//! Native migrations this release compiles (`specs/live_runtime_upgrades.md` §5.4).
//!
//! A migration moves the rows of old World storage into successor storage while the chain keeps
//! committing. The kernel ([`crate::runtime::migration`]) drives it; the release supplies the
//! code that knows the two layouts:
//!
//! - [`MigrationAddition::stage`] copies at most a bounded number of old rows after the cursor
//!   into the successor and returns the new cursor;
//! - [`MigrationAddition::barrier`] mirrors, at the end of every block, the old rows that block
//!   touched at or before the cursor, so the staged part of the successor stays exact;
//! - [`MigrationAddition::drain`] deletes a bounded number of old rows after the cutover;
//! - [`MigrationAddition::stale`] reports a write to the old storage after the cutover.
//!
//! The catalog publishes one migration definition per declaration and execution services
//! profile it compiles, so a root running any of them can activate the migration. The successor
//! storage is storage an addition adds ([`super::additions`]); this release declares none.

use crate::state::{WorldBlock, WorldTransaction};

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
    /// Copy at most `limit` old rows after `cursor` into the successor.
    pub(crate) stage: MigrationStage,
    /// Mirror the old rows the block touched at or before `cursor` into the successor.
    pub(crate) barrier: MigrationBarrier,
    /// Delete at most `limit` old rows.
    pub(crate) drain: MigrationDrain,
    /// Whether the block wrote a row of the old storage rather than deleting one.
    pub(crate) stale: MigrationStaleWrite,
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

/// Copies one bounded stage. It writes the successor only, and on an error it has written
/// nothing: it transforms the rows it will copy before it writes the first, because the kernel
/// keeps the committed cursor and the write barrier of a faulted migration for the replacement
/// that takes them over.
pub type MigrationStage =
    fn(&mut WorldTransaction<'_, '_>, &[u8], u32) -> Result<StageOutcome, String>;
/// Mirrors a block's writes of the old storage. It is total over the rows a stage accepts.
pub type MigrationBarrier = fn(&mut WorldBlock<'_>, &[u8]);
/// Drains one bounded stage of the old storage.
pub type MigrationDrain = fn(&mut WorldTransaction<'_, '_>, u32) -> DrainOutcome;
/// Reports a write that reached the old storage in the block.
pub type MigrationStaleWrite = fn(&WorldBlock<'_>) -> bool;

/// The native migrations of this release.
pub const MIGRATIONS: &[MigrationAddition] = &[];

/// A migration of a hypothetical later release, over World fields the real baseline holds.
#[cfg(test)]
pub mod test_migrations {
    use iroha_crypto::Hash;
    use iroha_data_model::account::AccountId;
    use mv::storage::StorageReadOnly as _;

    use super::{DrainOutcome, MigrationAddition, StageOutcome};
    use crate::state::{WorldBlock, WorldTransaction};

    /// Module of [`SEQUENCES`].
    pub const MODULE: &str = "iroha.test.migration.sequences";
    /// The old table [`SEQUENCES`] reads. Like [`TO`], it is storage the test addition
    /// [`super::super::additions::test_additions::MIGRATED`] adds.
    pub const FROM: &str = "world.tx_sequences";
    /// The successor table [`SEQUENCES`] fills.
    pub const TO: &str = "world.viral_bonus_paid";

    /// The successor row of one old row: the hash of the account, and whether its sequence is
    /// odd. A sequence of `u64::MAX` is a row no stage accepts.
    pub fn successor(account: &AccountId, sequence: u64) -> Result<(Hash, bool), String> {
        if sequence == u64::MAX {
            return Err(format!(
                "{account} holds a sequence no successor row represents"
            ));
        }
        let key = norito::encode_canonical(account).map_err(|error| error.to_string())?;
        Ok((Hash::new(key), sequence % 2 == 1))
    }

    fn cursor_account(cursor: &[u8]) -> Result<Option<AccountId>, String> {
        if cursor.is_empty() {
            return Ok(None);
        }
        norito::decode_canonical(cursor)
            .map(Some)
            .map_err(|error| error.to_string())
    }

    fn stage(
        world: &mut WorldTransaction<'_, '_>,
        cursor: &[u8],
        limit: u32,
    ) -> Result<StageOutcome, String> {
        let after = cursor_account(cursor)?;
        let limit = usize::try_from(limit).map_err(|error| error.to_string())?;
        let mut batch = Vec::new();
        let mut done = true;
        for (account, sequence) in world.tx_sequences.iter() {
            if after.as_ref().is_some_and(|after| account <= after) {
                continue;
            }
            if batch.len() == limit {
                done = false;
                break;
            }
            batch.push((account.clone(), successor(account, *sequence)?));
        }
        let cursor = match batch.last() {
            Some((account, _)) => {
                norito::encode_canonical(account).map_err(|error| error.to_string())?
            }
            None => cursor.to_vec(),
        };
        let rows = u32::try_from(batch.len()).map_err(|error| error.to_string())?;
        for (_, (key, value)) in batch {
            world.viral_bonus_paid.insert(key, value);
        }
        Ok(StageOutcome { cursor, rows, done })
    }

    fn barrier(world: &mut WorldBlock<'_>, cursor: &[u8]) {
        let Ok(Some(staged)) = cursor_account(cursor) else {
            return;
        };
        let mirrored: Vec<(Hash, Option<bool>)> = world
            .tx_sequences
            .touched_entries()
            .filter(|entry| *entry.key <= staged)
            .filter_map(|entry| {
                let (key, value) = successor(entry.key, entry.after.copied().unwrap_or(0)).ok()?;
                Some((key, entry.after.map(|_| value)))
            })
            .collect();
        for (key, value) in mirrored {
            match value {
                Some(value) => {
                    world.viral_bonus_paid.insert(key, value);
                }
                None => {
                    world.viral_bonus_paid.remove(key);
                }
            }
        }
    }

    fn drain(world: &mut WorldTransaction<'_, '_>, limit: u32) -> DrainOutcome {
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        let batch: Vec<AccountId> = world
            .tx_sequences
            .iter()
            .take(limit)
            .map(|(account, _)| account.clone())
            .collect();
        let rows = u32::try_from(batch.len()).unwrap_or(u32::MAX);
        for account in batch {
            world.tx_sequences.remove(account);
        }
        DrainOutcome {
            rows,
            empty: world.tx_sequences.iter().next().is_none(),
        }
    }

    fn stale(world: &WorldBlock<'_>) -> bool {
        world
            .tx_sequences
            .touched_entries()
            .any(|entry| entry.after.is_some())
    }

    /// Moves `world.tx_sequences` into `world.viral_bonus_paid`, two rows a stage.
    pub const SEQUENCES: MigrationAddition = MigrationAddition {
        module: MODULE,
        from: &[FROM],
        to: &[TO],
        cursor: "norito:iroha_data_model::account::AccountId@test",
        rows_per_stage: 2,
        stage,
        barrier,
        drain,
        stale,
    };
}
