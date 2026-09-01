//! Governance slashing and restitution flows for plain ballots and manual appeals.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
use iroha_core::{
    block::BlockBuilder,
    governance::manifest::LaneManifestRegistry,
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_data_model::{
    Registrable,
    asset::{Asset, AssetDefinition},
    block::BlockHeader,
    domain::{Domain, DomainId},
    events::data::governance::GovernanceSlashReason,
    permission::Permission,
    prelude::{AssetDefinitionId, AssetId, Grant},
    transaction::{
        FeePaymentIntent, TransactionBuilder, TransactionEntrypoint,
        signed::{
            SealedTransactionCommitmentPayload, SealedTransactionReveal,
            SignedSealedTransactionCommitment, compute_sealed_transaction_commitment,
        },
    },
};
use iroha_executor_data_model::permission::governance::{
    CanRestituteGovernanceLock, CanSlashGovernanceLock, CanSubmitGovernanceBallot,
};
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, gen_account_in};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;
use std::{borrow::Cow, sync::Arc};
fn governance_state_with_accounts(
    voting_asset_id: AssetDefinitionId,
    escrow_account: &iroha_data_model::account::AccountId,
    slash_account: &iroha_data_model::account::AccountId,
) -> State {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain");
    let domain = Domain::new(domain_id.clone()).build(escrow_account);
    let alice_account =
        iroha_data_model::account::Account::new(ALICE_ID.clone()).build(escrow_account);
    let escrow =
        iroha_data_model::account::Account::new(escrow_account.clone()).build(escrow_account);
    let slash =
        iroha_data_model::account::Account::new(slash_account.clone()).build(escrow_account);
    let asset_def = AssetDefinition::numeric(
        voting_asset_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(escrow_account);
    // Seed balances: Alice 1_000, escrow 0, slash 0.
    let alice_asset = Asset::new(
        AssetId::new(voting_asset_id.clone(), ALICE_ID.clone()),
        Quantity::from(1_000_u64),
    );
    let escrow_asset = Asset::new(
        AssetId::new(voting_asset_id.clone(), escrow_account.clone()),
        Quantity::from(0_u64),
    );
    let slash_asset = Asset::new(
        AssetId::new(voting_asset_id, slash_account.clone()),
        Quantity::from(0_u64),
    );
    let world = World::with_assets(
        [domain],
        [alice_account, escrow, slash],
        [asset_def],
        [alice_asset, escrow_asset, slash_asset],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    State::new_for_testing(world, kura, query_handle)
}
fn seed_slash_snapshot(
    state: &mut State,
    rid: &str,
    escrow_asset_id: &AssetId,
    slash_asset_id: &AssetId,
) {
    let mut seed_block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
    let mut seed_tx = seed_block.transaction();
    seed_tx.world.put_governance_referendum_for_testing(
        rid.to_owned(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 1,
            h_end: 100,
            status: iroha_core::state::GovernanceReferendumStatus::Open,
            final_tally: None,
        },
    );
    let mut locks = iroha_core::state::GovernanceLocksForReferendum::default();
    locks.locks.insert(
        ALICE_ID.clone(),
        iroha_core::state::GovernanceLockRecord {
            owner: ALICE_ID.clone(),
            amount: 60_u64.into(),
            slashed: 40_u64.into(),
            expiry_height: 100,
            direction: iroha_data_model::isi::governance::GovernancePlainBallotDirectionV1::Aye,
            duration_blocks: 99,
            custody: iroha_core::state::GovernanceLockCustody {
                escrowed: true,
                asset_definition_id: escrow_asset_id.definition().clone(),
                bond_escrow_account: escrow_asset_id.account().clone(),
                slash_receiver_account: slash_asset_id.account().clone(),
            },
        },
    );
    seed_tx
        .world
        .governance_locks_mut()
        .insert(rid.to_string(), locks);
    let mut ledger = iroha_core::state::GovernanceSlashLedger::default();
    ledger.slashes.insert(
        ALICE_ID.clone(),
        iroha_core::state::GovernanceSlashEntry {
            total_slashed: 40_u64.into(),
            total_restituted: 0_u64.into(),
            last_reason: GovernanceSlashReason::DoubleVote,
            last_height: 1,
        },
    );
    seed_tx
        .world
        .governance_slashes_mut()
        .insert(rid.to_string(), ledger);
    **seed_tx
        .world
        .asset_mut(escrow_asset_id)
        .expect("escrow asset") = Quantity::from(60_u64);
    **seed_tx
        .world
        .asset_mut(slash_asset_id)
        .expect("slash asset") = Quantity::from(40_u64);
    seed_tx.apply();
    let _ = seed_block.commit_empty_block_for_testing();
}
#[test]
#[allow(clippy::too_many_lines)]
fn double_vote_slashes_plain_lock() {
    let def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let (escrow_id, _) = gen_account_in("wonderland");
    let (slash_id, _) = gen_account_in("wonderland");
    let mut state = governance_state_with_accounts(def_id.clone(), &escrow_id, &slash_id);
    let alice = ALICE_ID.clone();
    let mut gov_cfg = state.gov.clone();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.voting_asset_id = def_id.clone();
    gov_cfg.min_bond_amount = 10_u64.into();
    gov_cfg.bond_escrow_account = escrow_id.clone();
    gov_cfg.slash_receiver_account = slash_id.clone();
    gov_cfg.slash_double_vote_bps = 2_000; // 20%
    state.set_gov(gov_cfg);
    let nexus = state.nexus_snapshot();
    state.install_lane_manifests(&Arc::new(
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
    ));
    // Block 1: seed referendum and cast initial ballot.
    let rid = "rid-slash-plain".to_string();
    {
        let header1 = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut sblock1 = state.block(header1);
        let mut stx1 = sblock1.transaction();
        stx1.world.put_governance_referendum_for_testing(
            rid.clone(),
            iroha_core::state::GovernanceReferendumRecord {
                h_start: 1,
                h_end: 50,
                status: iroha_core::state::GovernanceReferendumStatus::Open,
                final_tally: None,
            },
        );
        let perm: Permission = CanSubmitGovernanceBallot {
            referendum_id: rid.clone(),
        }
        .into();
        Grant::account_permission(perm, ALICE_ID.clone())
            .execute(&ALICE_ID, &mut stx1)
            .expect("grant ballot permission");
        let ballot_ok = iroha_data_model::isi::governance::CastPlainBallot {
            referendum_id: rid.clone(),
            direction: iroha_data_model::isi::governance::GovernancePlainBallotDirectionV1::Aye,
            lock: iroha_data_model::isi::governance::GovernanceParticipationLockV1 {
                amount: 20_u64.into(),
                duration_blocks: core::num::NonZeroU64::new(200).expect("non-zero lock duration"),
            },
        };
        ballot_ok
            .execute(&ALICE_ID, &mut stx1)
            .expect("first ballot should succeed");
        stx1.apply();
        let _ = sblock1.commit_empty_block_for_testing();
    }
    // Block 2: commit the sealed carrier for the conflicting ballot.
    let ballot_conflict = iroha_data_model::isi::governance::CastPlainBallot {
        referendum_id: rid.clone(),
        direction: iroha_data_model::isi::governance::GovernancePlainBallotDirectionV1::Nay,
        lock: iroha_data_model::isi::governance::GovernanceParticipationLockV1 {
            amount: 30_u64.into(),
            duration_blocks: core::num::NonZeroU64::new(200).expect("non-zero lock duration"),
        },
    };
    let transaction = TransactionBuilder::new(
        *state.network_id_ref(),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([ballot_conflict])
    .sign(ALICE_KEYPAIR.private_key());
    let salt = [0xA5; 32];
    let reveal_deadline_height = 10;
    let commitment = compute_sealed_transaction_commitment(
        state.network_id_ref(),
        &transaction,
        salt,
        reveal_deadline_height,
    );
    let sealed_commitment = SignedSealedTransactionCommitment::sign(
        SealedTransactionCommitmentPayload::new(
            *state.network_id_ref(),
            ALICE_ID.clone(),
            commitment,
            3,
            reveal_deadline_height,
            None,
        ),
        ALICE_KEYPAIR.private_key(),
    );
    let parent_hash = state
        .view()
        .block_hashes()
        .last()
        .copied()
        .expect("synthetic first block hash");
    let block = BlockBuilder::new(vec![AcceptedTransaction::new_unchecked_entrypoint(
        Cow::Owned(TransactionEntrypoint::SealedCommitment(sealed_commitment)),
    )])
    .chain_with_parent_hash(0, 1, parent_hash)
    .sign(ALICE_KEYPAIR.private_key())
    .unpack(|_| {});
    let mut state_block = state.block(block.header());
    let valid = block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    let commitment_results = valid.as_ref().entrypoint_results().collect::<Vec<_>>();
    assert_eq!(commitment_results.len(), 1);
    assert!(
        commitment_results[0].2.0.is_ok(),
        "sealed commitment must be retained before reveal: {:?}",
        commitment_results[0].2.0
    );
    let committed = valid.commit_unchecked().unpack(|_| {});
    let _ = state_block.apply_without_execution(&committed, Vec::new());
    state_block
        .commit()
        .expect("commit sealed ballot commitment");

    // Block 3: the sealed reveal enters the shared sequential corridor. The
    // ballot remains rejected while its prevalidated slash commits separately.
    let reveal_entrypoint = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
        commitment,
        transaction.clone(),
        salt,
    ));
    let reveal_hash = reveal_entrypoint.hash();
    let parent_hash = state
        .view()
        .block_hashes()
        .last()
        .copied()
        .expect("sealed commitment block hash");
    let block = BlockBuilder::new(vec![AcceptedTransaction::new_unchecked_entrypoint(
        Cow::Owned(reveal_entrypoint),
    )])
    .chain_with_parent_hash(0, 2, parent_hash)
    .sign(ALICE_KEYPAIR.private_key())
    .unpack(|_| {});
    let mut state_block = state.block(block.header());
    let valid = block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    let reveal_results = valid.as_ref().entrypoint_results().collect::<Vec<_>>();
    assert_eq!(reveal_results.len(), 1);
    let rejection = reveal_results[0]
        .2
        .0
        .as_ref()
        .expect_err("conflicting sealed ballot must remain rejected");
    assert!(
        format!("{rejection:?}").contains("re-vote cannot change direction"),
        "unexpected rejection: {rejection:?}"
    );
    let committed = valid.commit_unchecked().unpack(|_| {});
    let _ = state_block.apply_without_execution(&committed, Vec::new());
    state_block
        .commit()
        .expect("commit rejected sealed-ballot penalty");
    assert!(
        state.has_committed_entrypoint(reveal_hash),
        "the exact rejected sealed carrier must be replay protected"
    );
    assert!(
        state.has_committed_entrypoint(transaction.hash_as_entrypoint()),
        "the rejected reveal's enclosed signed intent must be replay protected"
    );
    // Escrow should now hold 16 (20 - 20% slash), slash receiver 4.
    let view = state.view();
    let escrow_asset_id = AssetId::new(def_id.clone(), escrow_id);
    let slash_asset_id = AssetId::new(def_id.clone(), slash_id);
    let lock = view
        .world()
        .governance_locks()
        .get(&rid)
        .and_then(|locks| locks.locks.get(&alice))
        .expect("lock present after slash");
    assert_eq!(lock.amount, Quantity::from(16_u64));
    assert_eq!(lock.slashed, Quantity::from(4_u64));
    let escrow_balance = view
        .world()
        .asset(&escrow_asset_id)
        .expect("escrow asset exists")
        .as_ref()
        .clone();
    let slash_balance = view
        .world()
        .asset(&slash_asset_id)
        .expect("slash receiver asset exists")
        .as_ref()
        .clone();
    assert_eq!(escrow_balance.clone(), Quantity::from(16_u64));
    assert_eq!(slash_balance.clone(), Quantity::from(4_u64));
    drop(view);
    let header3 = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut sblock3 = state.block(header3);
    let mut stx3 = sblock3.transaction();
    let unresolved_revote = iroha_data_model::isi::governance::CastPlainBallot {
        referendum_id: rid.clone(),
        direction: iroha_data_model::isi::governance::GovernancePlainBallotDirectionV1::Aye,
        lock: iroha_data_model::isi::governance::GovernanceParticipationLockV1 {
            amount: 20_u64.into(),
            duration_blocks: core::num::NonZeroU64::new(200).expect("non-zero lock duration"),
        },
    }
    .execute(&ALICE_ID, &mut stx3)
    .expect_err("a re-vote must not overwrite unresolved slash accounting");
    assert!(
        unresolved_revote
            .to_string()
            .contains("re-vote requires prior restitution")
    );
    let retained = stx3
        .world
        .governance_locks()
        .get(&rid)
        .and_then(|locks| locks.locks.get(&alice))
        .expect("rejected re-vote retains the slashed lock");
    assert_eq!(retained.amount, Quantity::from(16_u64));
    assert_eq!(retained.slashed, Quantity::from(4_u64));
    assert_eq!(
        stx3.world
            .asset(&escrow_asset_id)
            .expect("escrow remains after rejected re-vote")
            .as_ref()
            .clone(),
        Quantity::from(16_u64)
    );
    assert_eq!(
        stx3.world
            .asset(&slash_asset_id)
            .expect("slash receiver remains after rejected re-vote")
            .as_ref()
            .clone(),
        Quantity::from(4_u64)
    );
}
#[test]
fn restitution_restores_slashed_balance() {
    let def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let (escrow_id, _) = gen_account_in("wonderland");
    let (slash_id, _) = gen_account_in("wonderland");
    let mut state = governance_state_with_accounts(def_id.clone(), &escrow_id, &slash_id);
    let alice = ALICE_ID.clone();
    let mut gov_cfg = state.gov.clone();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.voting_asset_id = def_id.clone();
    gov_cfg.bond_escrow_account = escrow_id.clone();
    gov_cfg.slash_receiver_account = slash_id.clone();
    state.set_gov(gov_cfg);
    let rid = "rid-restitute".to_string();
    let escrow_asset_id = AssetId::new(def_id.clone(), escrow_id.clone());
    let slash_asset_id = AssetId::new(def_id.clone(), slash_id.clone());
    // Pre-seed a lock with a recorded slash (amount=60 active, 40 slashed) and matching balances.
    seed_slash_snapshot(&mut state, &rid, &escrow_asset_id, &slash_asset_id);
    {
        let header = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
        let mut sblock = state.block(header);
        let mut stx = sblock.transaction();
        // Grant restitution permission to ALICE.
        let perm: Permission = CanRestituteGovernanceLock {
            referendum_id: rid.clone(),
        }
        .into();
        Grant::account_permission(perm, ALICE_ID.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("grant restitution permission");
        iroha_data_model::isi::governance::RestituteGovernanceLock {
            referendum_id: rid.clone(),
            owner: ALICE_ID.clone(),
            amount: 30_u64.into(),
            reason: "appeal_upheld".to_string(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("restitution should succeed");
        let events = stx.world.take_external_events();
        assert!(events.iter().any(|ev| {
            matches!(
                ev.as_data_event(),
                Some(iroha_data_model::events::data::DataEvent::Governance(
                    iroha_data_model::events::data::governance::GovernanceEvent::LockRestituted(payload)
                )) if payload.amount == Quantity::from(30_u64)
                    && payload.reason == GovernanceSlashReason::Restitution
                    && payload.note == "appeal_upheld"
            )
        }));
        stx.apply();
        let _ = sblock.commit_world_overlay_for_testing();
    }
    let view = state.view();
    let lock = view
        .world()
        .governance_locks()
        .get(&rid)
        .and_then(|locks| locks.locks.get(&alice))
        .expect("lock present after restitution");
    assert_eq!(lock.amount, Quantity::from(90_u64));
    assert_eq!(lock.slashed, Quantity::from(10_u64));
    let escrow_balance = view
        .world()
        .asset(&escrow_asset_id)
        .expect("escrow asset exists")
        .as_ref()
        .clone();
    let slash_balance = view
        .world()
        .asset(&slash_asset_id)
        .expect("slash receiver asset exists")
        .as_ref()
        .clone();
    assert_eq!(escrow_balance.clone(), Quantity::from(90_u64));
    assert_eq!(slash_balance.clone(), Quantity::from(10_u64));
}
#[test]
fn restitution_preflight_leaves_custody_untouched_when_slash_ledger_is_missing() {
    let def_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "xor".parse().expect("asset name"),
    );
    let (escrow_id, _) = gen_account_in("wonderland");
    let (slash_id, _) = gen_account_in("wonderland");
    let mut state = governance_state_with_accounts(def_id.clone(), &escrow_id, &slash_id);
    let mut gov_cfg = state.gov.clone();
    gov_cfg.voting_asset_id = def_id.clone();
    gov_cfg.bond_escrow_account = escrow_id.clone();
    gov_cfg.slash_receiver_account = slash_id.clone();
    state.set_gov(gov_cfg);
    let referendum_id = "restitution-missing-ledger";
    let escrow_asset_id = AssetId::new(def_id.clone(), escrow_id);
    let slash_asset_id = AssetId::new(def_id, slash_id);
    seed_slash_snapshot(&mut state, referendum_id, &escrow_asset_id, &slash_asset_id);
    {
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut state_transaction = block.transaction();
        state_transaction
            .world
            .governance_slashes_mut()
            .remove(referendum_id.to_owned());
        Grant::account_permission(
            Permission::from(CanRestituteGovernanceLock {
                referendum_id: referendum_id.to_owned(),
            }),
            ALICE_ID.clone(),
        )
        .execute(&ALICE_ID, &mut state_transaction)
        .expect("grant restitution permission");
        state_transaction.apply();
        let _ = block.commit_empty_block_for_testing();
    }
    let header = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut state_transaction = block.transaction();
    let error = iroha_data_model::isi::governance::RestituteGovernanceLock {
        referendum_id: referendum_id.to_owned(),
        owner: ALICE_ID.clone(),
        amount: 30_u64.into(),
        reason: "missing_ledger".to_owned(),
    }
    .execute(&ALICE_ID, &mut state_transaction)
    .expect_err("missing slash ledger must reject restitution");
    assert!(error.to_string().contains("slash ledger missing"));
    // Deliberately apply the errored overlay to prove the helper itself did not
    // stage any custody or lock mutation before its ledger preflight failed.
    state_transaction.apply();
    let _ = block.commit_empty_block_for_testing();
    let view = state.view();
    let lock = view
        .world()
        .governance_locks()
        .get(referendum_id)
        .and_then(|locks| locks.locks.get(&ALICE_ID))
        .expect("rejected restitution retains the lock");
    assert_eq!(lock.amount, Quantity::from(60_u64));
    assert_eq!(lock.slashed, Quantity::from(40_u64));
    assert_eq!(
        view.world()
            .asset(&escrow_asset_id)
            .expect("escrow asset")
            .as_ref(),
        &Quantity::from(60_u64)
    );
    assert_eq!(
        view.world()
            .asset(&slash_asset_id)
            .expect("slash receiver asset")
            .as_ref(),
        &Quantity::from(40_u64)
    );
}
#[test]
fn slash_and_restitution_use_stored_custody_after_governance_config_change() {
    let domain_id = DomainId::try_new("wonderland", "universal").expect("domain");
    let old_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "old_xor".parse().expect("old asset name"),
    );
    let live_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "live_xor".parse().expect("live asset name"),
    );
    let alice = ALICE_ID.clone();
    let (old_escrow, _) = gen_account_in("wonderland");
    let (old_receiver, _) = gen_account_in("wonderland");
    let (live_escrow, _) = gen_account_in("wonderland");
    let (live_receiver, _) = gen_account_in("wonderland");
    let old_escrow_asset_id = AssetId::new(old_definition_id.clone(), old_escrow.clone());
    let old_receiver_asset_id = AssetId::new(old_definition_id.clone(), old_receiver.clone());
    let live_escrow_asset_id = AssetId::new(live_definition_id.clone(), live_escrow.clone());
    let live_receiver_asset_id = AssetId::new(live_definition_id.clone(), live_receiver.clone());
    let world = World::with_assets(
        [Domain::new(domain_id).build(&alice)],
        [
            iroha_data_model::account::Account::new(alice.clone()).build(&alice),
            iroha_data_model::account::Account::new(old_escrow.clone()).build(&alice),
            iroha_data_model::account::Account::new(old_receiver.clone()).build(&alice),
            iroha_data_model::account::Account::new(live_escrow.clone()).build(&alice),
            iroha_data_model::account::Account::new(live_receiver.clone()).build(&alice),
        ],
        [
            AssetDefinition::numeric(
                old_definition_id.clone(),
                "old_xor".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .build(&alice),
            AssetDefinition::numeric(
                live_definition_id.clone(),
                "live_xor".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .build(&alice),
        ],
        [
            Asset::new(old_escrow_asset_id.clone(), Quantity::from(10_u64)),
            Asset::new(old_receiver_asset_id.clone(), Quantity::from(5_u64)),
            Asset::new(live_escrow_asset_id.clone(), Quantity::from(11_u64)),
            Asset::new(live_receiver_asset_id.clone(), Quantity::from(13_u64)),
        ],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);
    let mut live_governance = state.gov.clone();
    live_governance.voting_asset_id = live_definition_id;
    live_governance.bond_escrow_account = live_escrow;
    live_governance.slash_receiver_account = live_receiver;
    state.set_gov(live_governance);
    let referendum_id = "stored-custody-slash-restitution";
    let stored_custody = iroha_core::state::GovernanceLockCustody {
        escrowed: true,
        asset_definition_id: old_definition_id,
        bond_escrow_account: old_escrow,
        slash_receiver_account: old_receiver,
    };
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    tx.world.put_governance_referendum_for_testing(
        referendum_id.to_owned(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 99,
            status: iroha_core::state::GovernanceReferendumStatus::Open,
            final_tally: None,
        },
    );
    for permission in [
        Permission::from(CanSlashGovernanceLock {
            referendum_id: referendum_id.to_owned(),
        }),
        Permission::from(CanRestituteGovernanceLock {
            referendum_id: referendum_id.to_owned(),
        }),
    ] {
        Grant::account_permission(permission, alice.clone())
            .execute(&alice, &mut tx)
            .expect("grant governance custody permission");
    }
    let mut locks = iroha_core::state::GovernanceLocksForReferendum::default();
    locks.locks.insert(
        alice.clone(),
        iroha_core::state::GovernanceLockRecord {
            owner: alice.clone(),
            amount: Quantity::from(10_u64),
            slashed: Quantity::zero(),
            expiry_height: 100,
            direction: iroha_data_model::isi::governance::GovernancePlainBallotDirectionV1::Aye,
            duration_blocks: 100,
            custody: stored_custody.clone(),
        },
    );
    tx.world
        .governance_locks_mut()
        .insert(referendum_id.to_owned(), locks);
    iroha_data_model::isi::governance::SlashGovernanceLock {
        referendum_id: referendum_id.to_owned(),
        owner: alice.clone(),
        amount: Quantity::from(4_u64),
        reason: "stored custody regression".to_owned(),
    }
    .execute(&alice, &mut tx)
    .expect("slash must use the lock's stored custody");
    let lock_after_slash = tx
        .world
        .governance_locks()
        .get(referendum_id)
        .and_then(|locks| locks.locks.get(&alice))
        .expect("lock after slash");
    assert_eq!(lock_after_slash.amount, Quantity::from(6_u64));
    assert_eq!(lock_after_slash.slashed, Quantity::from(4_u64));
    assert_eq!(lock_after_slash.custody, stored_custody);
    assert_eq!(
        tx.world
            .asset(&old_escrow_asset_id)
            .expect("stored escrow asset after slash")
            .as_ref()
            .clone(),
        Quantity::from(6_u64)
    );
    assert_eq!(
        tx.world
            .asset(&old_receiver_asset_id)
            .expect("stored slash receiver asset after slash")
            .as_ref()
            .clone(),
        Quantity::from(9_u64)
    );
    assert_eq!(
        tx.world
            .asset(&live_escrow_asset_id)
            .expect("live escrow asset after slash")
            .as_ref()
            .clone(),
        Quantity::from(11_u64)
    );
    assert_eq!(
        tx.world
            .asset(&live_receiver_asset_id)
            .expect("live slash receiver asset after slash")
            .as_ref()
            .clone(),
        Quantity::from(13_u64)
    );
    let ledger_after_slash = tx
        .world
        .governance_slashes()
        .get(referendum_id)
        .and_then(|ledger| ledger.slashes.get(&alice))
        .expect("slash ledger entry");
    assert_eq!(ledger_after_slash.total_slashed, Quantity::from(4_u64));
    assert_eq!(ledger_after_slash.total_restituted, Quantity::zero());
    assert_eq!(
        ledger_after_slash.last_reason,
        GovernanceSlashReason::Manual
    );
    assert_eq!(ledger_after_slash.last_height, 1);
    iroha_data_model::isi::governance::RestituteGovernanceLock {
        referendum_id: referendum_id.to_owned(),
        owner: alice.clone(),
        amount: Quantity::from(4_u64),
        reason: "stored custody appeal".to_owned(),
    }
    .execute(&alice, &mut tx)
    .expect("restitution must use the lock's stored custody");
    let lock_after_restitution = tx
        .world
        .governance_locks()
        .get(referendum_id)
        .and_then(|locks| locks.locks.get(&alice))
        .expect("lock after restitution");
    assert_eq!(lock_after_restitution.amount, Quantity::from(10_u64));
    assert_eq!(lock_after_restitution.slashed, Quantity::zero());
    assert_eq!(lock_after_restitution.custody, stored_custody);
    assert_eq!(
        tx.world
            .asset(&old_escrow_asset_id)
            .expect("stored escrow asset after restitution")
            .as_ref()
            .clone(),
        Quantity::from(10_u64)
    );
    assert_eq!(
        tx.world
            .asset(&old_receiver_asset_id)
            .expect("stored slash receiver asset after restitution")
            .as_ref()
            .clone(),
        Quantity::from(5_u64)
    );
    assert_eq!(
        tx.world
            .asset(&live_escrow_asset_id)
            .expect("live escrow asset after restitution")
            .as_ref()
            .clone(),
        Quantity::from(11_u64)
    );
    assert_eq!(
        tx.world
            .asset(&live_receiver_asset_id)
            .expect("live slash receiver asset after restitution")
            .as_ref()
            .clone(),
        Quantity::from(13_u64)
    );
    let ledger_after_restitution = tx
        .world
        .governance_slashes()
        .get(referendum_id)
        .and_then(|ledger| ledger.slashes.get(&alice))
        .expect("slash ledger after restitution");
    assert_eq!(
        ledger_after_restitution.total_slashed,
        Quantity::from(4_u64)
    );
    assert_eq!(
        ledger_after_restitution.total_restituted,
        Quantity::from(4_u64)
    );
    assert_eq!(
        ledger_after_restitution.last_reason,
        GovernanceSlashReason::Restitution
    );
    assert_eq!(ledger_after_restitution.last_height, 1);
}
