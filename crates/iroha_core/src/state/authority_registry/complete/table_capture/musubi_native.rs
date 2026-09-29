//! Actual readers for Musubi's four remaining native-Norito authority tables.
//!
//! These capture typed rows only; they do not establish finalized governance,
//! alias validity or coherent cross-table ownership. The availability, resolver
//! and public-directory semantic readers remain absent from the catalog.
//! TODO: give their live/universal validators caller-owned, fallibly admitted
//! scratch, including provider-attestation preimages and package accumulators,
//! before projecting all three tables from the same borrowed World cut.

use super::*;

capture_world_table_once!(
    pub(super) capture_musubi_resolver_index_checkpoints_once,
    musubi_resolver_index_checkpoints,
    "world.musubi_resolver_index_checkpoints"
);
capture_world_table_once!(
    pub(super) capture_musubi_aliases_once,
    musubi_aliases,
    "world.musubi_aliases"
);
capture_world_table_once!(
    pub(super) capture_musubi_alias_history_once,
    musubi_alias_history,
    "world.musubi_alias_history"
);
capture_world_table_once!(
    pub(super) capture_musubi_governance_decisions_once,
    musubi_governance_decisions,
    "world.musubi_governance_decisions"
);

#[cfg(test)]
mod tests {
    use super::super::native_test_support::{
        capture_controls, cloned, fixture_insert, fixture_remove, limits,
    };
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{MusubiResolverIndexRevisionV1, World},
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{account::AccountId, musubi::*};
    use iroha_model_base::topology::DataSpaceId;

    fn package() -> MusubiPackageIdV1 {
        MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            "capture".parse().expect("package name"),
        )
    }

    capture_controls!(
        checkpoint_reader_rejects_changed_anchor_and_omitted_row,
        musubi_resolver_index_checkpoints,
        capture_musubi_resolver_index_checkpoints_once,
        {
            let row = MusubiRegistrySnapshotV1 {
                finalized_height: 7,
                finalized_block_hash: [0x11; 32],
                index_revision: 9,
            };
            row.validate().unwrap();
            (MusubiResolverIndexRevisionV1::new(9).unwrap(), row)
        },
        |row: &mut MusubiRegistrySnapshotV1| row.finalized_block_hash[0] ^= 1
    );

    capture_controls!(
        alias_reader_rejects_changed_registration_and_omitted_row,
        musubi_aliases,
        capture_musubi_aliases_once,
        {
            let alias = "capture".parse::<MusubiAliasNameV1>().unwrap();
            let policy = MusubiAliasPricingPolicyV1::GENESIS;
            let publisher = AccountId::new(
                KeyPair::from_seed(b"musubi-native-capture".to_vec(), Algorithm::Ed25519)
                    .public_key()
                    .clone(),
            );
            let row = MusubiAliasRecordV1 {
                alias: alias.clone(),
                target: package(),
                registered_by: publisher,
                pricing_revision: policy.revision,
                paid_xor: policy.price_for(&alias),
                registered_at_height: 7,
                history_revision: 1,
            };
            row.validate(&policy).unwrap();
            (alias, row)
        },
        |row: &mut MusubiAliasRecordV1| row.registered_at_height += 1
    );

    capture_controls!(
        alias_history_reader_rejects_changed_height_and_omitted_row,
        musubi_alias_history,
        capture_musubi_alias_history_once,
        {
            let row = MusubiAliasHistoryEntryV1 {
                alias: "capture".parse().unwrap(),
                revision: 1,
                action: MusubiAliasHistoryActionV1::Registered,
                previous_target: None,
                target: package(),
                governance_action: None,
                finalized_height: 7,
            };
            row.validate().unwrap();
            (row.key(), row)
        },
        |row: &mut MusubiAliasHistoryEntryV1| row.finalized_height += 1
    );

    capture_controls!(
        decision_reader_rejects_changed_consumption_and_omitted_row,
        musubi_governance_decisions,
        capture_musubi_governance_decisions_once,
        {
            let row = MusubiGovernanceDecisionConsumptionV1 {
                decision: MusubiGovernanceDecisionV1 {
                    decision_id: [0x12; 32],
                    action_digest: MusubiGovernanceActionDigestV1::new([0x13; 32]),
                    enacted_at_height: 3,
                    execute_after_height: 7,
                },
                minimum_enactment_delay: 4,
                consumed_at_height: 7,
            };
            row.validate().unwrap();
            (row.decision.decision_id, row)
        },
        |row: &mut MusubiGovernanceDecisionConsumptionV1| row.consumed_at_height += 1
    );
}
