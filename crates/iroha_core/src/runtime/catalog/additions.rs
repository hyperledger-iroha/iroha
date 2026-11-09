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
//! therefore activated in declaration order, except the explicitly independent repair and
//! authority additions whose dependencies are exactly the baseline. Published dependency
//! descriptors never change after a selection contains them.

use crate::state::WorldTransaction;

/// One native implementation a release adds after the frozen baseline.
#[derive(Clone, Copy, Debug)]
pub struct Addition {
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
        reason = "a release declares its World-cell additions here; this release adds only a table"
    )
)]
pub enum StorageAddition {
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
pub struct CellAddition {
    /// Registry identity of the World field.
    pub(crate) field: &'static str,
    /// Canonical Norito bytes of the initial value, published in the storage definition.
    pub(crate) initial: fn() -> Result<Vec<u8>, String>,
    /// Decode the published initial bytes into the typed cell and write them.
    pub(crate) install: CellInstaller,
}

/// Writes a cell's published initial bytes into the World when its storage is installed.
pub type CellInstaller = fn(&mut WorldTransaction<'_, '_>, &[u8]) -> Result<(), String>;

/// What this release adds after its frozen baseline, in activation order.
///
/// A World field an addition adds must be a top-level World member: snapshots leave it out, and a
/// restore reads it empty, until a change set installs its storage
/// ([`crate::runtime::uninstalled_world_members`]).
pub const ADDITIONS: &[Addition] = &[
    ENDORSED_RUNTIME_UPGRADE_PROPOSAL,
    GAME_PARTITIONS,
    // Keep account signing keys independent of the Taira-only repair boundary.
    ACCOUNT_SIGNING_KEYS,
    SORAFS_REPAIR,
    SORAFS_AUTHORITY,
];

/// Parliament-owned nondelegable SoraFS authorities, independent of game partitions.
pub const SORAFS_AUTHORITY: Addition = Addition {
    module: "iroha.sorafs.authority_governance.v1",
    instructions: &[
        "iroha.sorafs.authority.propose.v1",
        "iroha.sorafs.authority.attempt.create.v1",
    ],
    rebinds: &["iroha.governance.parliament.transition.submit.v1"],
    storage: &[
        StorageAddition::Table("world.sorafs_authority_proposals"),
        StorageAddition::Table("world.sorafs_authority_heads"),
        StorageAddition::Table("world.sorafs_authority_leases"),
    ],
};

/// The owner-authorized, all-four-certified Taira initialization. Its dependency closure is
/// exactly the frozen baseline, independently of the unfinished game-partition addition.
/// Its separate kernel boundary is specified in `specs/sorafs/taira_repair_v1.md`.
pub const SORAFS_REPAIR: Addition = Addition {
    module: "iroha.sorafs.bootstrap.repair.v1",
    instructions: &["iroha.sorafs.bootstrap.repair.apply.v1"],
    rebinds: &[],
    storage: &[],
};

/// Runtime-upgrade proposals that carry the trusted release signers' endorsements
/// (`specs/runtime_upgrades.md`, "Provenance policy"). The baseline
/// `ProposeRuntimeUpgradeProposal` keeps its frozen layout and semantics, which check no
/// provenance; a root that activates this addition can then restrict the baseline proposal so
/// every scheduled change set carries endorsements.
pub const ENDORSED_RUNTIME_UPGRADE_PROPOSAL: Addition = Addition {
    module: "iroha.runtime.endorsed_change_set_proposal.v1",
    instructions: &["iroha.instruction.v1::governance::ProposeEndorsedRuntimeUpgradeProposal"],
    rebinds: &[],
    storage: &[],
};

/// Native game partitions of independent roots (`crate::smartcontracts::isi::game_partition`):
/// partition installation, peer registration, unit seeding and stepping, and certified unit
/// export and import, with the `world.game_partitions` table they keep. Until a change set
/// activates it the instructions stay inactive and the table stays empty and out of snapshots.
///
/// TODO(`specs/live_runtime_upgrades.md` §7): an export removes the unit at its source while the
/// destination's import is still pending, but runtime obligations track only escrow and AMX
/// State. Make an in-flight export a live obligation, so admission closure, completion-only
/// restrictions and retirement cannot strand its import, before governance activates this
/// addition on a live network.
pub const GAME_PARTITIONS: Addition = Addition {
    module: "iroha.game_partition.v1",
    instructions: &[
        "iroha.game_partition.install.v1",
        "iroha.game_partition.register_peer.v1",
        "iroha.game_partition.seed.v1",
        "iroha.game_partition.advance.v1",
        "iroha.game_partition.export.v1",
        "iroha.game_partition.import.v1",
    ],
    rebinds: &[],
    storage: &[StorageAddition::Table("world.game_partitions")],
};

/// Off-chain signing keys of accounts (`crate::smartcontracts::isi::account_signing_key`,
/// `specs/account_signing_keys.md`): `SetAccountSigningKeyV1` installs, rotates and revokes
/// the key an account signs off-chain messages with, separate from the controller that signs
/// its transactions, with the `world.account_signing_keys` table and the
/// `world.account_signing_key_reservations` table of keys that can never be reused. Until a
/// change set activates it the instruction stays inactive, both tables stay empty and out of
/// snapshots, and Torii authenticates every account with its controller exactly as before.
///
/// Rekey and recovery need no rebind. A signing key is bound to the `AccountId`, which is the
/// controller, so a controller change yields an account without a signing key, while the old
/// key stays reserved.
pub const ACCOUNT_SIGNING_KEYS: Addition = Addition {
    module: "iroha.account.signing_key.v1",
    instructions: &["iroha.account.signing_key.set.v1"],
    rebinds: &[],
    storage: &[
        StorageAddition::Table("world.account_signing_keys"),
        StorageAddition::Table("world.account_signing_key_reservations"),
    ],
};

/// Additions of a hypothetical later release, claiming compiled items the real baseline holds.
#[cfg(test)]
pub mod test_additions {
    use super::{Addition, CellAddition, StorageAddition};

    /// Module of [`LOG`].
    pub const LOG_MODULE: &str = "iroha.test.addition.log";
    /// Wire ID [`LOG`] adds.
    pub const LOG_WIRE_ID: &str = "iroha.log";
    /// The table [`LOG`] adds.
    pub const TABLE: &str = "world.viral_bonus_paid";
    /// The cell [`LOG`] adds: a top-level World member, as every addition's field must be.
    pub const CELL: &str = "world.soracloud_sequence_watermark";
    /// The value the cell [`LOG`] adds starts at.
    pub const CELL_INITIAL: u64 = 7;

    /// A release that adds the `Log` instruction, one table and one cell after its baseline.
    pub const LOG: Addition = Addition {
        module: LOG_MODULE,
        instructions: &[LOG_WIRE_ID],
        rebinds: &[],
        storage: &[
            StorageAddition::Table(TABLE),
            StorageAddition::Cell(CellAddition {
                field: CELL,
                initial: || norito::encode_canonical(&CELL_INITIAL).map_err(|e| e.to_string()),
                install: |world, bytes| {
                    *world.soracloud_sequence_watermark.get_mut() =
                        norito::decode_canonical::<u64>(bytes).map_err(|e| e.to_string())?;
                    Ok(())
                },
            }),
        ],
    };

    /// A later addition that rebinds `Log`, which [`LOG`] adds, to its own handler.
    pub const REBIND_LOG: Addition = Addition {
        module: "iroha.test.addition.log_fix",
        instructions: &[],
        rebinds: &[LOG_WIRE_ID],
        storage: &[],
    };
}
