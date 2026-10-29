//! Network upgrade checkpoint: this release taking over a running network at the last block its
//! predecessor release executed (`snapshot.upgrade_checkpoint`).
//!
//! Blocks at or below the checkpoint are final history. This release never re-executes them: it
//! restores the World at the checkpoint from the predecessor's signed snapshot, proves that World
//! against the checkpoint block's certified World root and executes from the next block on.
//!
//! The certified root binds the World registry of the release that computed it (`S` of
//! [`super::world_projection::WorldStateAccumulator`]). The predecessor this release accepts is
//! iroha f24908b4b3, the Taira release of 2026-10-25. Its canonical World held the cell
//! `privacy_exact12_qualification` and the tables `privacy_activations` and `runtime_upgrades`,
//! which this release retired, and lacked the tables `privacy_protocol_policies`,
//! `runtime_change_sets` and `runtime_containments` ([`ADDED_WORLD_FIELDS`]). An import accepts
//! a predecessor World only when every retired field holds its empty value, so nothing is
//! dropped, and starts the added tables empty.
//!
//! The predecessor also predates the live-upgrade foundation (`specs/live_runtime_upgrades.md`
//! §13): it has no runtime definitions or selection. The import founds them on this release's
//! baseline, exactly as a new World of this release is founded.

use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::block::BlockHeader;

/// A validated upgrade checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeCheckpointV1 {
    /// Height of the last block the predecessor release executed.
    pub height: u64,
    /// Hash of that block.
    pub block_hash: HashOf<BlockHeader>,
    /// Key that signed the checkpoint snapshot when it is not the node's own snapshot key.
    pub signer: Option<PublicKey>,
}

impl UpgradeCheckpointV1 {
    /// The checkpoint of a validated node configuration.
    #[must_use]
    pub fn from_config(checkpoint: &iroha_config::parameters::user::UpgradeCheckpoint) -> Self {
        Self {
            height: checkpoint.height,
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed(checkpoint.block_hash)),
            signer: checkpoint.signer.clone(),
        }
    }

    /// Whether `hashes`, Kura's block hashes through `height`, end at this checkpoint.
    pub(crate) fn is_tip_of(&self, height: u64, hashes: &[HashOf<BlockHeader>]) -> bool {
        self.height == height && hashes.last() == Some(&self.block_hash)
    }
}

/// A canonical World field of the predecessor release's registry.
pub(super) struct PredecessorField {
    /// Registry identity.
    pub(super) id: &'static str,
    /// Snapshot JSON field name.
    pub(super) name: &'static str,
    /// Nominal key schema of a table; `None` for a cell.
    pub(super) key: Option<&'static str>,
    /// Nominal value schema.
    pub(super) value: &'static str,
    /// The only snapshot JSON value an import accepts for it.
    pub(super) empty_json: &'static str,
}

/// One table this release added, in place of the predecessor fields it retired, if any. An
/// import admits the retired fields only empty and starts the added table empty.
pub(super) struct AddedWorldField {
    /// Registry identity of the added field.
    pub(super) id: &'static str,
    /// Its snapshot JSON name.
    pub(super) name: &'static str,
    /// The predecessor's fields it replaced, in the predecessor's World order; empty for a
    /// table without a predecessor.
    pub(super) retired: &'static [PredecessorField],
}

/// Every table this release added, in this release's World order.
pub(super) const ADDED_WORLD_FIELDS: [AddedWorldField; 3] = [
    AddedWorldField {
        id: "world.runtime_change_sets",
        name: "runtime_change_sets",
        retired: &[PredecessorField {
            id: "world.runtime_upgrades",
            name: "runtime_upgrades",
            key: Some("iroha_data_model::runtime::RuntimeUpgradeId"),
            value: "iroha_data_model::runtime::RuntimeUpgradeRecord",
            empty_json: r#"{"revert":{},"blocks":{}}"#,
        }],
    },
    AddedWorldField {
        id: "world.runtime_containments",
        name: "runtime_containments",
        retired: &[],
    },
    AddedWorldField {
        id: "world.privacy_protocol_policies",
        name: "privacy_protocol_policies",
        retired: &[
            PredecessorField {
                id: "world.privacy_exact12_qualification",
                name: "privacy_exact12_qualification",
                key: None,
                value: "core::option::Option<iroha_data_model::privacy::release_manifest::PrivacyExact12QualificationRecordV1>",
                empty_json: r#"{"revert":null,"blocks":null}"#,
            },
            PredecessorField {
                id: "world.privacy_activations",
                name: "privacy_activations",
                key: Some("iroha_core::privacy_state::PrivacyActivationKeyV1"),
                value: "iroha_data_model::privacy::PrivacyProtocolActivationRecordV1",
                empty_json: r#"{"revert":{},"blocks":{}}"#,
            },
        ],
    },
];

/// The predecessor's retired fields, in the predecessor's World order.
pub(super) fn retired_world_fields() -> impl Iterator<Item = &'static PredecessorField> {
    ADDED_WORLD_FIELDS
        .iter()
        .flat_map(|added| added.retired.iter())
}

/// The added table with the snapshot JSON name `name`.
pub(super) fn added_world_field(name: &str) -> Option<&'static AddedWorldField> {
    ADDED_WORLD_FIELDS.iter().find(|added| added.name == name)
}

/// The fields the predecessor lacked that an import founds on this release's baseline:
/// registry identity and snapshot JSON name.
pub(super) const FOUNDED_WORLD_FIELDS: [(&str, &str); 2] = [
    ("world.runtime_definitions", "runtime_definitions"),
    ("world.runtime_selection", "runtime_selection"),
];

/// Whether the import founds the snapshot field `name`.
pub(super) fn is_founded_world_field(name: &str) -> bool {
    FOUNDED_WORLD_FIELDS
        .iter()
        .any(|(_, founded)| *founded == name)
}

/// The predecessor release's World snapshot field order: `current` with the retired fields in
/// place of the added one and without the founded ones.
pub(super) fn predecessor_world_field_order(current: &[&'static str]) -> Vec<&'static str> {
    let mut order = Vec::with_capacity(current.len() + 1);
    for field in current {
        if is_founded_world_field(field) {
            continue;
        }
        if let Some(added) = added_world_field(field) {
            order.extend(added.retired.iter().map(|retired| retired.name));
        } else {
            order.push(*field);
        }
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predecessor_order_puts_the_retired_fields_where_the_added_field_is() {
        let current: &'static [&'static str] = &[
            "poseidon_params",
            "runtime_change_sets",
            "runtime_containments",
            "runtime_definitions",
            "runtime_selection",
            "privacy_consensus_policy",
            "privacy_protocol_policies",
            "kagemusha_wallet_ledger",
        ];
        assert_eq!(
            predecessor_world_field_order(current),
            [
                "poseidon_params",
                "runtime_upgrades",
                "privacy_consensus_policy",
                "privacy_exact12_qualification",
                "privacy_activations",
                "kagemusha_wallet_ledger",
            ]
        );
    }

    #[test]
    fn retired_fields_admit_only_the_empty_values_of_their_kind() {
        for retired in retired_world_fields() {
            let value: norito::json::Value =
                norito::json::from_str(retired.empty_json).expect("empty value is JSON");
            let empty = if retired.key.is_some() {
                r#"{"revert":{},"blocks":{}}"#
            } else {
                r#"{"revert":null,"blocks":null}"#
            };
            assert_eq!(
                value,
                norito::json::from_str::<norito::json::Value>(empty).unwrap()
            );
            assert_eq!(retired.id, format!("world.{}", retired.name));
        }
        for added in &ADDED_WORLD_FIELDS {
            assert_eq!(added.id, format!("world.{}", added.name));
            assert_eq!(
                added_world_field(added.name).map(|found| found.id),
                Some(added.id)
            );
        }
        for (id, name) in FOUNDED_WORLD_FIELDS {
            assert_eq!(id, format!("world.{name}"));
            assert!(is_founded_world_field(name));
        }
    }

    #[test]
    fn checkpoint_is_the_tip_only_at_its_height_and_hash() {
        let hash = HashOf::from_untyped_unchecked(Hash::prehashed([7; 32]));
        let other = HashOf::from_untyped_unchecked(Hash::prehashed([8; 32]));
        let checkpoint = UpgradeCheckpointV1 {
            height: 2,
            block_hash: hash,
            signer: None,
        };
        assert!(checkpoint.is_tip_of(2, &[other, hash]));
        assert!(!checkpoint.is_tip_of(2, &[hash, other]));
        assert!(!checkpoint.is_tip_of(3, &[other, other, hash]));
    }
}
