//! Authority table readers bind original persisted values without conferring authority.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::isi::sorafs_authority::{
        SorafsAuthorityHeadV1, SorafsAuthorityLeaseV1, SorafsAuthorityProposalRecordV1,
    },
    state::{GovernanceProposalStatus, World},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    governance::types::{GovernanceAttemptId, GovernanceCertificateId},
    isi::sorafs_authority::{
        SorafsAuthorityActionV1, SorafsAuthorityProposalV1, SorafsAuthorityTermGridV1,
        SorafsOperatingAuthorityV1,
    },
    runtime::lifecycle::RootScopeV1,
};
use native_test_support::{capture_controls, cloned, fixture_insert, fixture_remove, limits};

fn proposal() -> SorafsAuthorityProposalV1 {
    SorafsAuthorityProposalV1 {
        grid: SorafsAuthorityTermGridV1 {
            epoch_length_blocks: 100,
        },
        term: 1,
        network: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
            [0x71; 32],
        ))),
        root: RootScopeV1::Universal,
        account: iroha_test_samples::ALICE_ID.clone(),
        authority: SorafsOperatingAuthorityV1::SetReservePolicy,
        action: SorafsAuthorityActionV1::Grant,
        expected_revision: 0,
        completion_policy: None,
    }
}

fn record() -> SorafsAuthorityProposalRecordV1 {
    let proposal = proposal();
    SorafsAuthorityProposalRecordV1 {
        proposer: proposal.account.clone(),
        proposal,
        created_height: 1,
        status: GovernanceProposalStatus::Proposed,
        deadline_closure: None,
    }
}

fn head() -> SorafsAuthorityHeadV1 {
    let proposal = proposal();
    let content = proposal.proposal_content_id();
    // These typed storage fixtures exercise canonical bytes, not an admitted
    // vote, execution receipt, finality certificate or activated selection.
    SorafsAuthorityHeadV1 {
        plan_id: proposal.plan_id(),
        term: proposal.term,
        expires_at_height: proposal.grid.window(proposal.term).unwrap().end_height,
        authority: proposal.authority,
        account: proposal.account,
        revision: 2,
        granted: false,
        proposal_content_id: content,
        governance_attempt_id: GovernanceAttemptId::derive_v1(content, 0),
        certificate_id: GovernanceCertificateId::new([0x71; 32]),
        enacted_at_height: 1,
    }
}

fn lease() -> SorafsAuthorityLeaseV1 {
    SorafsAuthorityLeaseV1 {
        plan: proposal(),
        grant_approval: None,
        terminal_approval: None,
        activation: None,
        completion: None,
        start_processed: false,
    }
}

capture_controls!(
    sorafs_authority_proposal_capture_binds_original_value_and_rejects_substitution,
    sorafs_authority_proposals,
    capture_sorafs_authority_proposals_once,
    ([0x71; 32], record()),
    |value: &mut SorafsAuthorityProposalRecordV1| value.created_height += 1
);
capture_controls!(
    sorafs_authority_head_capture_binds_original_value_and_rejects_substitution,
    sorafs_authority_heads,
    capture_sorafs_authority_heads_once,
    ([0x71; 32], head()),
    |value: &mut SorafsAuthorityHeadV1| value.revision += 1
);
capture_controls!(
    sorafs_authority_lease_capture_binds_original_value_and_rejects_substitution,
    sorafs_authority_leases,
    capture_sorafs_authority_leases_once,
    ([0x71; 32], lease()),
    |value: &mut SorafsAuthorityLeaseV1| value.start_processed = true
);

#[test]
fn sorafs_authority_catalog_preserves_original_pool_refusal_and_publication_retry() {
    let _constructor_retirement = crossbeam_epoch::pin();
    let state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    fixture_insert!(state, sorafs_authority_proposals, [0x71; 32], record());
    fixture_insert!(state, sorafs_authority_heads, [0x71; 32], head());
    fixture_insert!(state, sorafs_authority_leases, [0x71; 32], lease());
    let original = state.ivm_execution_budget();
    let resident = original.reserved_bytes();
    let original_limit = original.limit_bytes();
    for id in [
        "world.sorafs_authority_proposals",
        "world.sorafs_authority_heads",
        "world.sorafs_authority_leases",
    ] {
        let TableMaterializer::Native { capture, .. } = TABLE_MATERIALIZERS
            .iter()
            .find(|owner| owner.table_ids().any(|candidate| candidate == id))
            .expect("declared original authority table")
        else {
            panic!("one declaration must own committed and frozen original readers")
        };
        original.set_limit_bytes(0);
        assert!(matches!(
            capture(&state, limits()),
            Err(LeafError::Admission(_) | LeafError::OrderedRange(_))
        ));
        assert_eq!(original.reserved_bytes(), resident);
        let mut publication = state.state_view_publication();
        let guard = publication.begin();
        assert!(capture(&state, limits()).unwrap().is_none());
        drop(guard);
        original.set_limit_bytes(original_limit);
        let snapshot = capture(&state, limits()).unwrap().unwrap();
        assert_eq!(snapshot.table_id(), id);
        assert_eq!(snapshot.row_count(), 1);
        assert!(original.reserved_bytes() > resident);
        drop(snapshot);
        assert_eq!(original.reserved_bytes(), resident);
    }
}

#[test]
fn sorafs_authority_frozen_readers_keep_original_replacement_images_and_undo() {
    use crate::state::block_field::BlockField;
    use iroha_data_model::block::BlockHeader;
    use mv::BlockRetirement as _;
    use std::num::NonZeroU64;

    // These are native storage-image controls, not governed activation fixtures.
    // Each new declaration must bind both its cold reader and its exact frozen
    // replacement image. Undo stays in the original owner, not in the row root.
    macro_rules! check {
        ($field:ident, $capture:ident, $value:expr, $change:expr) => {{
            let state = State::new_for_testing(
                World::new(),
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let key = [0x72; 32];
            let removed = [0x73; 32];
            let absent = [0x74; 32];
            let value = $value;
            let mut changed = value.clone();
            ($change)(&mut changed);
            {
                let mut original = state.world.$field.block();
                original.insert(key, value.clone());
                original.insert(removed, value.clone());
                original.commit();
            }
            fixture_insert!(state, $field, key, changed.clone());
            let committed = $capture(&state, limits()).unwrap().unwrap();
            let mut block =
                state.block_and_revert(BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0));
            assert_eq!(block.world.$field.get(&key), Some(&value));
            block.world.$field.remove(removed);
            block.world.$field.remove(absent);
            block.world.begin_freeze();
            block.world.finish_freeze();
            block.world.retire_frozen_cleanup();
            let rows = block.world.$field.frozen_images().unwrap();
            assert_eq!(rows.mode(), mv::BlockMode::Replace);
            assert_eq!(
                rows.undo_entries().find(|(k, _)| *k == &removed).unwrap().1,
                &Some(value.clone())
            );
            assert_eq!(
                rows.undo_entries().find(|(k, _)| *k == &absent).unwrap().1,
                &None
            );
            let table = concat!("world.", stringify!($field));
            let retained = frozen::capture_original_table_once(&block, table, limits(), 100_000)
                .unwrap()
                .unwrap();
            let expected = State::new_for_testing(
                World::new(),
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            fixture_insert!(expected, $field, key, value);
            let cold = $capture(&expected, limits()).unwrap().unwrap();
            assert_eq!(retained.table_id(), table);
            assert_eq!(retained.row_count(), 1);
            assert_eq!(retained.root(), cold.root());
            assert_eq!(retained.lookup_root(), cold.lookup_root());
            assert_eq!(retained.ordered_root(), cold.ordered_root());
            assert_ne!(retained.root(), committed.root());
            let identity = block.world.$field.publication_identity();
            fixture_insert!(state, $field, key, changed);
            let same_original =
                frozen::capture_original_table_once(&block, table, limits(), 100_000)
                    .unwrap()
                    .unwrap();
            assert_eq!(same_original.root(), retained.root());
            assert_eq!(same_original.lookup_root(), retained.lookup_root());
            assert_eq!(same_original.ordered_root(), retained.ordered_root());
            assert_eq!(block.world.$field.publication_identity(), identity);
        }};
    }
    check!(
        sorafs_authority_proposals,
        capture_sorafs_authority_proposals_once,
        record(),
        |value: &mut SorafsAuthorityProposalRecordV1| value.created_height += 1
    );
    check!(
        sorafs_authority_heads,
        capture_sorafs_authority_heads_once,
        head(),
        |value: &mut SorafsAuthorityHeadV1| value.revision += 1
    );
    check!(
        sorafs_authority_leases,
        capture_sorafs_authority_leases_once,
        lease(),
        |value: &mut SorafsAuthorityLeaseV1| value.start_processed = true
    );
}
