//! Real State retained-reserve rollback, sweep retry, and current/undo restoration.

use super::*;
use crate::{
    fastpq::source_reservation::SourceUsage,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{
        GovernanceLockRecord, GovernanceReferendumMode, GovernanceReferendumRecord,
        GovernanceReferendumStatus, State, StateBlock, World,
    },
};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::Account,
    asset::{Asset, AssetBalancePolicy, AssetDefinition, AssetId},
    block::BlockHeader,
    domain::Domain,
    governance::conviction::{PlainVotingContextV1, PlainVotingResultV1},
};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use std::collections::{BTreeMap, BTreeSet};

fn referendum() -> GovernanceReferendumRecord {
    GovernanceReferendumRecord {
        h_start: 1,
        h_end: 10,
        status: GovernanceReferendumStatus::Open,
        mode: GovernanceReferendumMode::Zk,
        plain_context: PlainVotingContextV1::NotApplicable,
        plain_result: PlainVotingResultV1::NotApplicable,
    }
}

fn retained(
    custody: GovernanceLockCustody,
    amount: u32,
    expiry: u64,
) -> GovernanceLocksForReferendum {
    GovernanceLocksForReferendum {
        locks: BTreeMap::from([(
            ALICE_ID.clone(),
            GovernanceLockRecord {
                owner: ALICE_ID.clone(),
                amount: Quantity::from(amount),
                slashed: Quantity::zero(),
                expiry_height: expiry,
                direction: 0,
                duration_blocks: 1,
                custody,
            },
        )]),
    }
}

fn header(height: u64) -> BlockHeader {
    BlockHeader::new(height.try_into().unwrap(), None, None, 0, 0)
}

fn assert_single_mandatory_release(block: &StateBlock<'_>) {
    let (ordinary, mandatory) = block.fastpq_source_usage_for_testing();
    assert_eq!(ordinary, SourceUsage::ZERO);
    assert_eq!(
        (
            mandatory.executed_entries,
            mandatory.transcripts,
            mandatory.deltas
        ),
        (1, 1, 1)
    );
    let (hash, transcripts) = block.fastpq_transcripts.iter().next().unwrap();
    let exact = measure_fastpq_source_entry_frame_usage(
        *hash,
        transcripts.iter(),
        limits(FastpqSourcePolicyV1::bootstrap().block).unwrap(),
    )
    .unwrap();
    assert_eq!(
        mandatory.input_transcript_bytes,
        exact.input_transcript_bytes as u64
    );
    assert_eq!(
        mandatory.max_statement_bytes,
        exact.max_statement_bytes as u64
    );
    assert_eq!(
        mandatory.total_statement_bytes,
        exact.total_statement_bytes as u64
    );
}

#[test]
fn state_preflight_refusal_and_transaction_rollback_preserve_the_entire_reserve() {
    let custody = super::tests::custody();
    let mut world = World::default();
    let cap = FastpqSourcePolicyV1::bootstrap()
        .mandatory
        .max_retained_obligations;
    for index in 0..cap - 1 {
        let id = format!("retained-{index}");
        world.governance_referenda.insert(id.clone(), referendum());
        world
            .governance_locks
            .insert(id, retained(custody.clone(), 0, u64::MAX));
    }
    world.rebuild_governance_read_indexes().unwrap();
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(header(2));
    let before = norito::encode_canonical(
        &block
            .world
            .governance_locks
            .iter()
            .map(|(id, locks)| (id.clone(), locks.clone()))
            .collect::<BTreeMap<_, _>>(),
    )
    .unwrap();
    {
        let mut tx = block.transaction();
        tx.validate_fastpq_governance_lock("last", &ALICE_ID, &custody)
            .unwrap();
        tx.world
            .put_governance_locks("last".into(), retained(custody.clone(), 0, u64::MAX));
        assert!(
            tx.validate_fastpq_governance_lock("overflow", &ALICE_ID, &custody)
                .unwrap_err()
                .contains("global retained")
        );
        tx.validate_fastpq_governance_lock("last", &ALICE_ID, &custody)
            .unwrap();
        assert!(tx.pending_transfer_transcripts.is_empty());
        assert!(
            tx.pending_fastpq_source_captures
                .sources()
                .unwrap()
                .is_empty()
        );
        // Dropping the actual World transaction restores the reservation owner.
    }
    assert_eq!(
        norito::encode_canonical(
            &block
                .world
                .governance_locks
                .iter()
                .map(|(id, locks)| (id.clone(), locks.clone()))
                .collect::<BTreeMap<_, _>>()
        )
        .unwrap(),
        before
    );
    assert_eq!(
        block.fastpq_source_usage_for_testing(),
        (SourceUsage::ZERO, SourceUsage::ZERO)
    );
    let tx = block.transaction();
    tx.validate_fastpq_governance_lock("last", &ALICE_ID, &custody)
        .unwrap();
}

#[test]
fn actual_sweep_releases_zero_and_successful_records_but_retains_failed_retry() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let custody = super::tests::custody();
    let domain =
        iroha_model_base::domain::DomainId::try_new("source-mandatory", "universal").unwrap();
    let definition = custody.asset_definition_id.clone();
    let escrow_asset = AssetId::new(definition.clone(), BOB_ID.clone());
    let owner_asset = AssetId::new(definition.clone(), ALICE_ID.clone());
    let mut world = World::with_assets(
        [Domain::new(domain.clone()).build(&ALICE_ID)],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(BOB_ID.clone()).build(&ALICE_ID),
        ],
        [AssetDefinition::numeric(
            definition,
            "asset",
            AssetBalancePolicy::Global,
            Some(domain),
        )
        .build(&ALICE_ID)],
        [Asset::new(escrow_asset.clone(), Quantity::from(3_u32))],
        [],
    );
    // Exact same escrow: first release succeeds, second cannot debit its exhausted balance.
    for (id, amount) in [("a-success", 3), ("b-failed", 1), ("c-zero", 0)] {
        world.governance_referenda.insert(id.into(), referendum());
        world
            .governance_locks
            .insert(id.into(), retained(custody.clone(), amount, 1));
    }
    world.rebuild_governance_read_indexes().unwrap();
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    // Bootstrap the real genesis-only asset incarnation before this height-two
    // sweep. Expiry one is inclusive, so the height-one start hooks release none.
    let genesis = state.block(header(1));
    assert_eq!(genesis.world.governance_locks.len(), 3);
    assert!(genesis.fastpq_transcripts.is_empty());
    genesis.commit_world_overlay_for_testing().unwrap();
    let block = state.block(header(2));
    assert!(block.world.governance_locks.get("a-success").is_none());
    assert!(block.world.governance_locks.get("c-zero").is_none());
    assert!(block.world.governance_locks.get("b-failed").is_some());
    assert!(block.world.assets.get(&escrow_asset).is_none());
    assert_eq!(
        block
            .world
            .assets
            .get(&owner_asset)
            .map(|value| value.as_ref().clone()),
        Some(Quantity::from(3_u32))
    );
    assert_eq!(block.fastpq_transcripts.len(), 1);
    assert_eq!(block.captured_fastpq_transcript_sources().unwrap().len(), 1);
    assert_single_mandatory_release(&block);
    assert_eq!(
        block.world.governance_unlock_stats.get().expired_locks_now,
        1
    );
    block.commit_world_overlay_for_testing().unwrap();

    // A later sweep fails again; the same record continues to reserve its future release.
    let retry = state.block(header(3));
    assert!(retry.world.governance_locks.get("b-failed").is_some());
    assert!(retry.fastpq_transcripts.is_empty());
    assert_eq!(
        retry.fastpq_source_usage_for_testing(),
        (SourceUsage::ZERO, SourceUsage::ZERO)
    );
    assert!(
        retry
            .captured_fastpq_transcript_sources()
            .unwrap()
            .is_empty()
    );
    let mut one = FastpqSourcePolicyV1::bootstrap();
    one.mandatory.max_retained_obligations = 1;
    assert!(
        validate_retained(
            one,
            retry.world.governance_locks.iter(),
            Some(("new", &ALICE_ID, &custody)),
            None
        )
        .unwrap_err()
        .contains("global retained")
    );
    drop(retry);
    // Restore the actual escrow balance and prove the retained obligation is
    // released only when a later mandatory transfer really applies.
    {
        let mut world = state.world.block();
        let (id, value) = Asset::new(escrow_asset, Quantity::from(1_u32)).into_key_value();
        world.assets.insert(id, value);
        world.commit();
    }
    let success = state.block(header(4));
    assert!(success.world.governance_locks.get("b-failed").is_none());
    assert_eq!(success.fastpq_transcripts.len(), 1);
    assert_single_mandatory_release(&success);
    assert_eq!(
        success.captured_fastpq_transcript_sources().unwrap().len(),
        1
    );
    assert_eq!(
        success
            .world
            .assets
            .get(&owner_asset)
            .map(|value| value.as_ref().clone()),
        Some(Quantity::from(4_u32))
    );
    validate_retained(
        one,
        success.world.governance_locks.iter(),
        Some(("new", &ALICE_ID, &custody)),
        None,
    )
    .unwrap();
}

#[test]
fn restore_rejects_over_capacity_current_and_undo_without_publishing_partial_index() {
    let custody = super::tests::custody();
    let cap = FastpqSourcePolicyV1::bootstrap()
        .mandatory
        .max_retained_obligations;
    let sentinel = BTreeMap::from([(
        77_u64,
        BTreeSet::from([("sentinel".into(), ALICE_ID.clone())]),
    )]);
    let mut world = World::default();
    for index in 0..=cap {
        let id = format!("retained-{index}");
        world.governance_referenda.insert(id.clone(), referendum());
        world
            .governance_locks
            .insert(id, retained(custody.clone(), 0, 1));
    }
    world.governance_lock_expiry_index = sentinel.clone().into_iter().collect();
    assert!(
        world
            .rebuild_governance_read_indexes()
            .unwrap_err()
            .contains("global retained")
    );
    // Current corpus now fits; its undo value still contains the over-capacity corpus.
    {
        let mut locks = world.governance_locks.block();
        locks.remove(format!("retained-{cap}"));
        locks.commit();
    }
    assert_eq!(world.governance_locks.view().len(), cap as usize);
    assert!(
        world
            .rebuild_governance_read_indexes()
            .unwrap_err()
            .contains("global retained")
    );
    assert_eq!(
        world
            .governance_lock_expiry_index
            .view()
            .iter()
            .map(|(height, entries)| (*height, entries.clone()))
            .collect::<BTreeMap<_, _>>(),
        sentinel
    );
    assert_eq!(
        world.governance_locks.block_and_revert().len(),
        cap as usize + 1
    );
}
