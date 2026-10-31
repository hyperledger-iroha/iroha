//! What this release adds after its frozen baseline (`specs/live_runtime_upgrades.md` §3.1, §4).
//!
//! A running network that installed the baseline executes it unchanged forever, so a release that
//! adds an instruction or a World field declares it here instead of letting it join the baseline;
//! the pinned baseline inventory (`specs/runtime_baseline_catalog.json`) fails otherwise. Each
//! addition is one native implementation module:
//!
//! - every instruction it adds becomes a new interface it answers. Its wire ID is registered
//!   tolerantly (`built_in_wire_id!(T => "id", tolerant)`), so a frame the decoder refuses stays
//!   an opaque envelope, exactly as on a release without the addition (§5.2);
//! - every earlier wire ID it rebinds (a fix within that interface's contract, §4) is answered by
//!   its own handler, registered beside the native instruction table;
//! - every World field it adds becomes a new storage definition: a table starts empty, and a cell
//!   starts at the canonical value its [`CellAddition::initial`] encodes.
//!
//! The catalog publishes these definitions without installing or activating any of them. A
//! governed change set does that ([`super::ReleaseCatalog::activation`]): until it activates, a
//! node of this release executes exactly what a node of the previous release executes, and the
//! fields stay empty, uncommitted and out of snapshots.
//!
//! Like the baseline bundle, an addition declares a conservative superset of dependencies: every
//! storage definition and interface of the baseline and of the additions before it. Additions are
//! therefore activated in declaration order, and an addition never moves once a network has
//! activated it.

use crate::state::WorldTransaction;

/// One native implementation a release adds after the frozen baseline.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Addition {
    /// Native module of the implementation; unique in the release.
    pub(crate) module: &'static str,
    /// Wire IDs of the instructions it adds, each a new interface.
    pub(crate) instructions: &'static [&'static str],
    /// Wire IDs of earlier interfaces it rebinds to its own handlers.
    pub(crate) rebinds: &'static [&'static str],
    /// The canonical World fields it adds.
    pub(crate) storage: &'static [StorageAddition],
}

/// One canonical World field an addition adds.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "a release declares its World-field additions here; this release declares none"
    )
)]
pub(crate) enum StorageAddition {
    /// A table, installed empty: its registry identity.
    Table(&'static str),
    /// A cell, installed at its initial value.
    Cell(CellAddition),
}

impl StorageAddition {
    /// Registry identity of the World field.
    pub(crate) const fn field(&self) -> &'static str {
        match self {
            Self::Table(field) => field,
            Self::Cell(cell) => cell.field,
        }
    }
}

/// A World cell an addition adds, with the value its installation writes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CellAddition {
    /// Registry identity of the World field.
    pub(crate) field: &'static str,
    /// Canonical Norito bytes of the initial value, published in the storage definition.
    pub(crate) initial: fn() -> Result<Vec<u8>, String>,
    /// Decode the published initial bytes into the typed cell and write them.
    pub(crate) install: CellInstaller,
}

/// Writes a cell's published initial bytes into the World when its storage is installed.
pub(crate) type CellInstaller = fn(&mut WorldTransaction<'_, '_>, &[u8]) -> Result<(), String>;

/// What this release adds after its frozen baseline, in activation order.
///
/// TODO(`specs/live_runtime_upgrades.md` §5.5): before the first addition of a World field,
/// snapshot writers must omit, and readers default, every field the snapshot's selection does not
/// install, so a previous release keeps reading this release's snapshots until activation.
pub(crate) const ADDITIONS: &[Addition] = &[];

/// Additions of a hypothetical later release, claiming compiled items the real baseline holds.
#[cfg(test)]
pub(crate) mod test_additions {
    use super::{Addition, CellAddition, StorageAddition};

    /// Module of [`LOG`].
    pub(crate) const LOG_MODULE: &str = "iroha.test.addition.log";
    /// Wire ID [`LOG`] adds.
    pub(crate) const LOG_WIRE_ID: &str = "iroha.log";
    /// The table [`LOG`] adds.
    pub(crate) const TABLE: &str = "world.viral_bonus_paid";
    /// The cell [`LOG`] adds.
    pub(crate) const CELL: &str = "world.soradns_history_len";
    /// The value the cell [`LOG`] adds starts at.
    pub(crate) const CELL_INITIAL: u64 = 7;

    /// A release that adds the `Log` instruction, one table and one cell after its baseline.
    pub(crate) const LOG: Addition = Addition {
        module: LOG_MODULE,
        instructions: &[LOG_WIRE_ID],
        rebinds: &[],
        storage: &[
            StorageAddition::Table(TABLE),
            StorageAddition::Cell(CellAddition {
                field: CELL,
                initial: || norito::encode_canonical(&CELL_INITIAL).map_err(|e| e.to_string()),
                install: |world, bytes| {
                    *world.soradns_history_len.get_mut() =
                        norito::decode_canonical::<u64>(bytes).map_err(|e| e.to_string())?;
                    Ok(())
                },
            }),
        ],
    };

    /// A later addition that rebinds `Log`, which [`LOG`] adds, to its own handler.
    pub(crate) const REBIND_LOG: Addition = Addition {
        module: "iroha.test.addition.log_fix",
        instructions: &[],
        rebinds: &[LOG_WIRE_ID],
        storage: &[],
    };
}
