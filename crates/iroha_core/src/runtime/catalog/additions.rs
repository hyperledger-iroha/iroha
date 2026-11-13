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
//! therefore activated in declaration order, except the explicitly independent additions (the
//! repair, authority, restricted-read, anonymous Parliament and runtime lifecycle additions)
//! whose dependencies are exactly the baseline. Published dependency
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
/// A World field an addition adds is a top-level World member or a member of a nested owner
/// whose snapshot reader applies the selection: snapshots leave it out, and a restore reads it
/// empty, until a change set installs its storage
/// ([`crate::runtime::uninstalled_world_members`]).
pub const ADDITIONS: &[Addition] = &[
    ENDORSED_RUNTIME_UPGRADE_PROPOSAL,
    GAME_PARTITIONS,
    // Keep account signing keys independent of the Taira-only repair boundary.
    ACCOUNT_SIGNING_KEYS,
    SORAFS_REPAIR,
    SORAFS_AUTHORITY,
    RESTRICTED_DATASPACE_READ,
    // Independent of every addition before it, like the three above.
    PARLIAMENT_ANONYMOUS,
    // These independent additions are excluded from the ordered dependency chain.
    // Later additions must explicitly compose with their selected services and tables.
    RUNTIME_LIFECYCLE,
    DEFERRED_CONSENT,
    NATIVE_ESCROW_CAPTURE,
    NATIVE_AMX_CAPTURE,
    NATIVE_GAME_CAPTURE,
    CONTRACT_LIFECYCLE,
    RUNTIME_RETIREMENT,
];

/// Permanent runtime history and certified exclusion receipts, independently governed.
pub const RUNTIME_RETIREMENT: Addition = Addition {
    module: "iroha.runtime.retirement.v1",
    instructions: &["iroha.runtime.retirement.finalize.v1"],
    rebinds: &[],
    storage: &[StorageAddition::Table("world.runtime_retirements")],
};

/// Reserved native game transfers retain their original certified completion closure.
pub const NATIVE_GAME_CAPTURE: Addition = Addition {
    module: "iroha.runtime.native_game_capture.v1",
    instructions: &[
        "iroha.game_partition.request_export.v1",
        "iroha.game_partition.reserve_import.v1",
        "iroha.game_partition.export_reserved.v1",
        "iroha.game_partition.import_reserved.v1",
        "iroha.game_partition.acknowledge_import.v1",
        "iroha.game_partition.cancel_export_request.v1",
        "iroha.game_partition.cancel_import_reservation.v1",
    ],
    rebinds: &[
        "iroha.game_partition.export.v1",
        "iroha.game_partition.import.v1",
    ],
    storage: &[StorageAddition::Table("world.runtime_game_transfers")],
};

/// Native AMX original decision/settlement captures, including the fixed deadline finalizer.
pub const NATIVE_AMX_CAPTURE: Addition = Addition {
    module: "iroha.runtime.native_amx_capture.v1",
    instructions: &[],
    rebinds: &[
        "iroha.sumeragi.amx.begin.v1",
        "iroha.sumeragi.amx.prepare.v1",
    ],
    storage: &[StorageAddition::Table("world.runtime_amx_captures")],
};

/// Kernel-owned native escrow captures, installed independently after earlier custody drains.
pub const NATIVE_ESCROW_CAPTURE: Addition = Addition {
    module: "iroha.runtime.native_escrow_capture.v1",
    instructions: &[],
    rebinds: &["iroha.instruction.v1::escrow::OpenAssetEscrow"],
    storage: &[StorageAddition::Table("world.runtime_escrow_captures")],
};

/// Deferred execution consent, independently installed through a new services profile.
/// Existing proposal and trigger layouts stay unchanged; the kernel owns their captures.
pub const DEFERRED_CONSENT: Addition = Addition {
    module: "iroha.runtime.deferred_consent.v1",
    instructions: &[
        "iroha.runtime.consent.trigger.current.v1",
        "iroha.runtime.consent.multisig.current.v1",
    ],
    rebinds: &[],
    storage: &[StorageAddition::Table("world.runtime_deferred_consents")],
};

/// The kernel's lifecycle State (`specs/live_runtime_upgrades.md` §5.4, §7.2, §7.3, §13):
///
/// - `world.runtime_obligations`, the registry of obligations
///   (`crate::runtime::obligations::registry`): `RegisterRuntimeObligationV1` commits an owner's
///   terminal effects with their captured completion closure, and `CompleteRuntimeObligationV1`
///   ends the obligation;
/// - `world.runtime_migrations`, the committed cursor of every migration
///   (`crate::runtime::migration`);
/// - `world.runtime_boundaries`, the boundaries of the root's history by height
///   (`crate::runtime::checkpoint`): `RatifyRuntimeAdoptionAnchorV1` commits the adoption anchor
///   and `RaiseRuntimeStartupFloorV1` a startup floor, each with every validator's statement.
///
/// A root that has activated it also admits what needs that State: a migration, the retirement
/// of drained storage and the committed checkpoint policy. Until a change set activates the
/// addition the tables stay empty and out of snapshots, the instructions stay inactive, and the
/// root executes exactly what the frozen baseline executes. Its dependencies are exactly the
/// frozen baseline.
pub const RUNTIME_LIFECYCLE: Addition = Addition {
    module: "iroha.runtime.lifecycle.v1",
    instructions: &[
        "iroha.runtime.obligation.register.v1",
        "iroha.runtime.obligation.complete.v1",
        "iroha.runtime.boundary.anchor.v1",
        "iroha.runtime.boundary.floor.v1",
    ],
    rebinds: &[],
    storage: &[
        StorageAddition::Table("world.runtime_obligations"),
        StorageAddition::Table("world.runtime_migrations"),
        StorageAddition::Table("world.runtime_boundaries"),
    ],
};

/// Exact restricted data space reads delegated by their active native SNS owner.
/// This independent successor preserves the baseline's genesis-only grant policy until the
/// four permission/role mutation interfaces are explicitly rebound by governance.
pub const RESTRICTED_DATASPACE_READ: Addition = Addition {
    module: "iroha.dataspace.restricted_read_delegation.v1",
    instructions: &[],
    rebinds: &[
        "iroha.grant",
        "iroha.revoke",
        "iroha.register",
        "iroha.unregister",
    ],
    storage: &[],
};

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
/// The original unreserved export does not capture destination completion. Local release
/// qualification therefore refuses this addition on its own; `NATIVE_GAME_CAPTURE` installs
/// these immutable dependencies atomically with its certified transfer and cancellation service.
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

/// The anonymous Parliament family (`crate::governance::parliament_anonymous`,
/// `specs/parliament_private_ballot_design.md` §8.1 and §12.3). Its interfaces are
/// `CreateAnonymousParliamentAttemptV1`, which creates an attempt below the admission cap,
/// `SubmitAnonymousParliamentTransitionV1`, which records an account-signed fact or intent of
/// one, and `RegisterParliamentCredentialV1`, which accepts a binding seat together with its
/// verified credential. Its tables are `world.parliament_anonymous_attempts`, the State of
/// each attempt, `world.parliament_anonymous_registrations`, the credential of each accepted
/// binding assignment, and `world.parliament_anonymous_boards`, the board of each sealed
/// binding jury. Until a change set activates it the instructions stay inactive and the
/// tables stay empty and out of snapshots. Its dependencies are exactly the frozen baseline.
///
/// TODO(`specs/parliament_private_ballot_design.md` §12.3): the family is activatable only
/// after its fourth slice (the positioned block-start pass and signerless ballots), and on a
/// root that runs the kernel's lifecycle services profile only with a composition for it.
/// Governance must not activate it on a network before then.
pub const PARLIAMENT_ANONYMOUS: Addition = Addition {
    module: "iroha.governance.parliament.anonymous.v1",
    instructions: &[
        "iroha.governance.parliament.anonymous.attempt.create.v1",
        "iroha.governance.parliament.anonymous.transition.submit.v1",
        "iroha.governance.parliament.credential.register.v1",
    ],
    rebinds: &[],
    storage: &[
        StorageAddition::Table("world.parliament_anonymous_attempts"),
        StorageAddition::Table("world.parliament_anonymous_registrations"),
        StorageAddition::Table("world.parliament_anonymous_boards"),
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
    /// The cell [`LOG`] adds, a top-level World member.
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

    /// The table [`NESTED`] adds: a member of the trigger set, a nested World owner.
    pub const NESTED_TABLE: &str = "triggers.contracts";
    /// The member [`NESTED_TABLE`] is written as inside the World's `triggers` member.
    pub const NESTED_MEMBER: &str = "contracts";

    /// A release that adds the `Log` instruction and one table inside a nested World owner.
    pub const NESTED: Addition = Addition {
        module: "iroha.test.addition.nested",
        instructions: &[LOG_WIRE_ID],
        rebinds: &[],
        storage: &[StorageAddition::Table(NESTED_TABLE)],
    };

    /// A release that adds the `Log` instruction and two tables a migration moves rows between
    /// (`super::super::migrations::test_migrations`). No baseline definition declares either
    /// table, so the old one can be retired without replacing the baseline bundle.
    pub const MIGRATED: Addition = Addition {
        module: "iroha.test.addition.migrated",
        instructions: &[LOG_WIRE_ID],
        rebinds: &[],
        storage: &[
            StorageAddition::Table("world.tx_sequences"),
            StorageAddition::Table(TABLE),
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

/// Authenticated executing-contract registration under a separately named ABI V1 host profile.
/// The governed draft requires the existing kernel lifecycle storage; it adds no storage layout.
pub const CONTRACT_LIFECYCLE: Addition = Addition {
    module: "iroha.runtime.contract_lifecycle.v1",
    instructions: &["iroha.runtime.obligation.contract.register.v1"],
    rebinds: &[],
    storage: &[],
};
