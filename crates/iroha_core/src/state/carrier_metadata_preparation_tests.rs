//! Structural metadata parity and refusal controls; no finality is constructed.

use super::*;
use crate::{block::ValidBlock, kura::Kura, query::store::LiveQueryStore};

#[inline(never)]
fn state() -> Box<State> {
    Box::new(State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ))
}

fn proposal() -> SignedBlock {
    let key = crate::state::checked_keypair();
    let mut header = BlockHeader::new(NonZeroU64::MIN, None, None, 3, 0);
    header.set_confidential_features(Some(
        iroha_data_model::confidential::DEFAULT_CONFIDENTIAL_FEATURE_DIGEST,
    ));
    let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
    builder.set_da_proof_policies(Some(crate::da::proof_policy_bundle(
        &iroha_config::parameters::actual::LaneConfig::default(),
    )));
    builder
        .try_build_with_signature(0, key.private_key())
        .unwrap()
}

fn carrier() -> SignedBlock {
    let mut block = proposal();
    block
        .set_execution_outputs(
            Vec::new(),
            0,
            BTreeMap::new(),
            Vec::new(),
            AxtPolicySnapshot::default(),
            BTreeSet::new(),
            Vec::new(),
            &crate::execution_output_test_support::structural_output_limits(),
        )
        .unwrap();
    block
}

fn topology() -> Vec<PeerId> {
    (0_u8..4)
        .map(|index| {
            let key = iroha_crypto::KeyPair::try_from_seed(
                vec![0x60 + index; 32],
                iroha_crypto::Algorithm::BlsNormal,
            )
            .unwrap();
            PeerId::new(key.public_key().clone())
        })
        .collect()
}

#[test]
fn deterministic_metadata_preparation_matches_existing_apply_without_publication() {
    let state = state();
    let carrier = carrier();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    for authority in [
        ApplyTopologyAuthority::V2Finality,
        ApplyTopologyAuthority::Fixture,
    ] {
        let prepared_bytes = {
            let mut prepared = state.block(carrier.header());
            let events_before = prepared.world.external_event_buf.len();
            prepared
                .prepare_deterministic_carrier_metadata(&carrier, topology(), authority.clone())
                .unwrap();
            assert_eq!(prepared.world.external_event_buf.len(), events_before);
            assert_eq!(prepared.block_hashes.last(), Some(&carrier.hash()));
            assert_eq!(prepared.world.musubi_resolver_index_checkpoints.len(), 1);
            assert_eq!(prepared.commit_topology.len(), 4);
            assert!(
                prepared
                    .canonical_carrier_commit_metadata_authorization
                    .is_none()
            );
            crate::snapshot::canonical_staged_state_snapshot_bytes(&prepared)
        };
        // Exercise only the pre-existing structural metadata wrapper. This
        // supplies no QC and does not publish State or Kura.
        let committed = ValidBlock::new_unverified_for_tests(carrier.clone())
            .commit_unchecked()
            .unpack(|_| {});
        let mut applied = state.block(carrier.header());
        let (events, result) =
            applied.apply_without_execution_inner(&committed, topology(), authority);
        result.unwrap();
        assert!(
            !events.is_empty(),
            "the wrapper retains Applied event delivery"
        );
        assert_eq!(
            crate::snapshot::canonical_staged_state_snapshot_bytes(&applied),
            prepared_bytes
        );
        drop(applied);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            before
        );
        assert_eq!(state.committed_height(), 0);
        assert_eq!(state.kura.blocks_count(), 0);
    }
}

#[test]
fn deterministic_metadata_preparation_keeps_npos_rejection_before_writes() {
    let state = state();
    let carrier = carrier();
    let mut prepared = state.block(carrier.header());
    prepared.applied_npos_consensus_effects_hash = Some(HashOf::from_untyped_unchecked(Hash::new(
        b"foreign NPoS application",
    )));
    let before = crate::snapshot::canonical_staged_state_snapshot_bytes(&prepared);
    assert!(
        prepared
            .prepare_deterministic_carrier_metadata(
                &carrier,
                topology(),
                ApplyTopologyAuthority::V2Finality,
            )
            .is_err()
    );
    assert_eq!(
        crate::snapshot::canonical_staged_state_snapshot_bytes(&prepared),
        before
    );
    assert!(prepared.block_hashes.is_empty());
    assert!(!prepared.transactions.has_staged_block());
}

#[test]
fn deterministic_metadata_preparation_cannot_resolve_output_publication_guard() {
    let state = state();
    let carrier = carrier();
    let mut prepared = state.block(carrier.header());
    prepared
        .reserve_ordinary_execution_outputs(&carrier)
        .unwrap();
    prepared
        .prepare_deterministic_carrier_metadata(
            &carrier,
            topology(),
            ApplyTopologyAuthority::V2Finality,
        )
        .unwrap();
    assert!(matches!(
        prepared.execution_output_plan.as_ref(),
        Some(output_capacity::ExecutionOutputPlanState::Reserved(_))
    ));
    assert_eq!(
        prepared.commit().unwrap_err(),
        TransactionsBlockError::ExecutionOutputCapacity
    );
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura.blocks_count(), 0);
}

#[test]
fn malformed_or_foreign_metadata_is_rejected_before_any_staged_write() {
    let state = state();
    let valid = carrier();
    let mut bad_sccp = valid.clone();
    bad_sccp.set_sccp_commitment_root(Some([0x49; 32]));
    for (source, scope_header) in [
        (proposal(), valid.header()),
        (bad_sccp.clone(), bad_sccp.header()),
        (
            valid.clone(),
            BlockHeader::new(NonZeroU64::MIN, None, None, 9, 0),
        ),
    ] {
        let mut scope = state.block(scope_header);
        let before = crate::snapshot::canonical_staged_state_snapshot_bytes(&scope);
        assert!(
            scope
                .prepare_deterministic_carrier_metadata(
                    &source,
                    topology(),
                    ApplyTopologyAuthority::V2Finality
                )
                .is_err()
        );
        assert_eq!(
            crate::snapshot::canonical_staged_state_snapshot_bytes(&scope),
            before
        );
        assert!(!scope.transactions.has_staged_block());
        assert!(scope.pending_da_pin_intents.is_none());
    }
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura.blocks_count(), 0);
}

#[test]
fn prepared_carrier_cannot_append_its_hash_twice() {
    let state = state();
    let source = carrier();
    let mut scope = state.block(source.header());
    scope
        .prepare_deterministic_carrier_metadata(
            &source,
            topology(),
            ApplyTopologyAuthority::V2Finality,
        )
        .unwrap();
    let before = crate::snapshot::canonical_staged_state_snapshot_bytes(&scope);
    let error = scope
        .prepare_deterministic_carrier_metadata(
            &source,
            topology(),
            ApplyTopologyAuthority::V2Finality,
        )
        .unwrap_err();
    assert!(error.to_string().contains("exact State predecessor"));
    assert_eq!(
        crate::snapshot::canonical_staged_state_snapshot_bytes(&scope),
        before
    );
    assert_eq!(scope.block_hashes.len(), 1);
}

#[test]
fn musubi_checkpoint_preparation_rejects_inconsistent_history_without_panicking() {
    let state = state();
    let source = carrier();
    let mut scope = state.block(source.header());
    assert!(
        scope
            .stage_musubi_resolver_index_checkpoint(1, source.hash())
            .is_err()
    );
    assert!(scope.world.musubi_resolver_index_checkpoints.is_empty());
    scope.block_hashes.push(source.hash());
    let future_revision = MusubiResolverIndexRevisionV1::new(2).unwrap();
    scope.world.musubi_resolver_index_checkpoints.insert(
        future_revision,
        MusubiRegistrySnapshotV1 {
            finalized_height: 1,
            finalized_block_hash: *source.hash().as_ref(),
            index_revision: 2,
        },
    );
    let before = crate::snapshot::canonical_staged_state_snapshot_bytes(&scope);
    assert!(
        scope
            .stage_musubi_resolver_index_checkpoint(1, source.hash())
            .is_err()
    );
    assert_eq!(
        crate::snapshot::canonical_staged_state_snapshot_bytes(&scope),
        before
    );
    scope
        .world
        .musubi_resolver_index_checkpoints
        .remove(future_revision);
    scope
        .stage_musubi_resolver_index_checkpoint(1, source.hash())
        .unwrap();
    let once = crate::snapshot::canonical_staged_state_snapshot_bytes(&scope);
    scope
        .stage_musubi_resolver_index_checkpoint(1, source.hash())
        .unwrap();
    assert_eq!(
        crate::snapshot::canonical_staged_state_snapshot_bytes(&scope),
        once
    );
}
