//! Actual native signed execution retains its UID decoder bank across World rollback.
use super::*;
use crate::state::asset_uid::binding_custody::retry::observation::Observer;
use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
use iroha_data_model::{
    account::Account,
    asset::{
        AssetBalancePolicy, AssetDefinition, AssetDefinitionId, AssetId,
        uid::{AssetUidBindingV1, AssetUidDailyWindowV1, AssetUidPolicyV1},
    },
    domain::Domain,
    isi::{Mint, Register, Transfer},
};
use iroha_model_base::domain::DomainId;
use mv::storage::StorageReadOnly as _;

fn definition() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("uidretry", "universal").unwrap(),
        "token".parse().unwrap(),
    )
}
fn chain() -> CertifiedTestChain {
    let mut config = TestChainConfig::new(crate::state::World::new(), 1_000);
    let owner = AccountId::new(config.genesis_key.public_key().clone());
    let recipient = crate::smartcontracts::isi::sccp::test_support::authority(13);
    config.genesis_instructions = vec![
        Register::account(Account::new(recipient)).into(),
        Register::domain(Domain::new(
            DomainId::try_new("uidretry", "universal").unwrap(),
        ))
        .into(),
        Register::asset_definition(AssetDefinition::numeric(
            definition(),
            "UID retry",
            AssetBalancePolicy::Global,
            None,
        ))
        .into(),
        Mint::asset_quantity(10_u32, AssetId::of(definition(), owner.clone())).into(),
    ];
    let binding = AssetUidBindingV1 {
        asset_definition_id: definition(),
        account_id: owner.clone(),
        uid: [9; 32],
    };
    let policy = AssetUidPolicyV1 {
        asset_definition_id: definition(),
        uid: binding.uid,
        revision: 1,
        accounts: std::collections::BTreeSet::from([owner.clone()]),
        holding_limit: None,
        daily_window: AssetUidDailyWindowV1::UtcDay,
        daily_spend_limit: None,
        daily_outgoing_limit: None,
    };
    config.world.smart_contract_state.insert(
        crate::state::asset_uid::binding_key(&definition(), &owner),
        norito::encode_canonical(&binding).unwrap(),
    );
    config.world.smart_contract_state.insert(
        crate::state::asset_uid::policy_key(&definition(), &binding.uid),
        norito::encode_canonical(&policy).unwrap(),
    );
    CertifiedTestChain::start(config).expect("actual signed genesis with canonical UID membership")
}

#[test]
fn native_worker_binding_leaf_refusal_keeps_original_bank_and_reuses_controls_after_rollback() {
    publication_tests::with_worker_from(
        chain,
        ConsensusMode::Permissioned,
        |chain, worker, _blocks, events| {
            let _retirement = crossbeam_epoch::pin();
            let owner = chain.genesis_account().clone();
            let recipient = crate::smartcontracts::isi::sccp::test_support::authority(13);
            let key =
                iroha_crypto::KeyPair::from_seed(vec![0xCE; 32], iroha_crypto::Algorithm::Ed25519);
            let block =
                publication_tests::proposal_with_transaction(chain, worker, |chain, time| {
                    chain.sign(
                        &key,
                        [Transfer::asset_quantity(
                            AssetId::of(definition(), owner.clone()),
                            1_u32,
                            recipient.clone(),
                        )
                        .into()],
                        time,
                    )
                });
            let hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
            let observer = Observer::occupy_after_capture();
            let Err(PublicationError::Deferred(reason)) = worker.run_execution(&block, hash, false)
            else {
                panic!("actual native UID leaf capacity must defer before any balance publication");
            };
            assert!(
                matches!(reason.allocation_refusal(), Some(iroha_allocation::AllocationRefusal::Capacity { requested_bytes, .. }) if *requested_bytes == owner.try_signatory().unwrap().retained_allocation_layout().size())
            );
            assert_eq!(observer.snapshot(), (1, false));
            assert!(
                worker
                    .signature_decode
                    .as_ref()
                    .is_some_and(|attempt| attempt.uid_bindings.is_some()),
                "a refused native binding must retain its original execution bank across World rollback"
            );
            let original = worker
                .signature_decode
                .as_ref()
                .unwrap()
                .uid_bindings
                .as_ref()
                .unwrap()
                .clone();
            assert!(worker.live.is_none());
            assert!(worker.finishing.is_none());
            assert!(worker.results.is_empty());
            assert!(events.try_recv().is_err());
            assert_eq!(
                chain
                    .state()
                    .view()
                    .world()
                    .assets()
                    .get(&AssetId::of(definition(), owner.clone()))
                    .unwrap()
                    .as_ref(),
                &iroha_primitives::numeric::Quantity::from(10_u32)
            );
            observer.release_and_refuse_replacement();
            let retry = worker.run_execution(&block, hash, false);
            assert!(
                matches!(retry, Ok(Some(_))),
                "same native source must resume the original binding controls: {retry:?}"
            );
            assert_eq!(
                observer.snapshot(),
                (2, false),
                "original binding control allocations cannot be replaced after actual World rollback"
            );
            // Successful finishing moves the original decoder and bank into Live.
            assert!(worker.signature_decode.is_none());
            let live = worker.live.as_ref().expect("completed original execution");
            assert!(live.overlay.is_some());
            assert!(
                original.same_owner(
                    live.signature_decode
                        .uid_bindings
                        .as_ref()
                        .expect("Live retains the original binding bank")
                )
            );
            assert_eq!(chain.state().view().height(), 1);
        },
    );
}

/// Diagnostic: actual original-pool release readiness controls every repeated worker attempt.
///
/// The external occupancy is held for the whole test. A repeated immediate dispatch
/// therefore exposes credits returned by this execution's own rolled-back prefix.
#[test]
fn native_worker_binding_rollback_cannot_repeatedly_dispatch_without_external_release() {
    use crate::sumeragi::driver::{
        exec::{ExecDone, ExecOp, ExecSched, ExecutionRegistrations},
        persist::Backoff,
    };
    use iroha_allocation::{AllocationBudget, AllocationRefusal, release::ReleaseRegistration};
    use std::task::{Context, Poll, Waker};

    publication_tests::with_worker_from(
        chain,
        ConsensusMode::Permissioned,
        |chain, worker, _blocks, events| {
            // Keep unrelated deferred epoch reclamation outside the observation window.
            let _retirement = crossbeam_epoch::pin();
            let owner = chain.genesis_account().clone();
            let recipient = crate::smartcontracts::isi::sccp::test_support::authority(13);
            let key =
                iroha_crypto::KeyPair::from_seed(vec![0xCE; 32], iroha_crypto::Algorithm::Ed25519);
            let block =
                publication_tests::proposal_with_transaction(chain, worker, |chain, time| {
                    chain.sign(
                        &key,
                        [Transfer::asset_quantity(
                            AssetId::of(definition(), owner.clone()),
                            1_u32,
                            recipient.clone(),
                        )
                        .into()],
                        time,
                    )
                });
            let hash = block.hash(&**worker.context.crypto.as_ref().unwrap());
            let pool = worker
                .state
                .world
                .sccp_committee_generations
                .allocation_budget()
                .clone();
            // Observer and scheduler controls never consume or refund execution credit.
            let watch_pool = AllocationBudget::new(
                ExecutionRegistrations::admission_bytes()
                    + ReleaseRegistration::allocation_layout().size(),
            );
            let registrations = ExecutionRegistrations::admit(&watch_pool).unwrap();
            let mut scheduler = ExecSched::new(
                u64::try_from(chain.state().view().height()).unwrap(),
                Backoff::default(),
                registrations,
            );
            scheduler.bind_release_thread(&std::thread::current());
            let mut credit = watch_pool
                .try_reserve(ReleaseRegistration::allocation_layout())
                .unwrap();
            let mut registration = ReleaseRegistration::from_reservation(&mut credit).unwrap();
            registration.bind_thread(std::thread::current());
            scheduler.execute(976, hash, block, false);
            let mut next = scheduler.next(0).expect("initial actual execution job");
            let observer = Observer::occupy_after_capture();
            let expected_leaf = owner
                .try_signatory()
                .unwrap()
                .retained_allocation_layout()
                .size();
            let mut observations = [None; 3];
            let mut original_body = None;
            let mut original_bank = None;
            for attempt in 0..3 {
                let ExecOp::Execute {
                    block,
                    block_hash,
                    certified,
                } = next
                else {
                    panic!("the same held execution must be the only scheduled operation");
                };
                assert_eq!(block_hash, hash);
                assert!(!certified);
                if let Some(original) = &original_body {
                    assert!(std::sync::Arc::ptr_eq(original, &block));
                } else {
                    original_body = Some(std::sync::Arc::clone(&block));
                }
                let error = match worker.run_execution(&block, block_hash, certified) {
                    Err(error) => error,
                    result => {
                        // Finite progress is distinct from a repeated refusal loop. Keep
                        // this explicit diagnostic result; do not fabricate a scheduler
                        // success event or a fixed post-state expectation from a hash.
                        eprintln!(
                            "UID rollback diagnostic: execution progressed at attempt {} with fixed external occupancy: {result:?}; prior observations={observations:?}",
                            attempt + 1,
                        );
                        assert!(matches!(result, Ok(Some(_))));
                        assert_eq!(chain.state().view().height(), 1);
                        return;
                    }
                };
                eprintln!(
                    "UID rollback diagnostic original refusal: attempt={}, payload_bytes={}, error={error:#?}",
                    attempt + 1,
                    block.payload().as_slice().len(),
                );
                let PublicationError::Deferred(reason) = &error else {
                    panic!("original local execution refusal: {error:?}");
                };
                let Some(AllocationRefusal::Capacity {
                    requested_bytes, ..
                }) = reason.allocation_refusal()
                else {
                    panic!("original finite-pool refusal: {error:?}");
                };
                if attempt == 0 {
                    assert_eq!(*requested_bytes, expected_leaf);
                    assert_eq!(observer.snapshot(), (1, false));
                }
                let retained = worker
                    .signature_decode
                    .as_ref()
                    .and_then(|attempt| attempt.uid_bindings.as_ref())
                    .expect("original binding preparation survives every rollback");
                if let Some(original) = &original_bank {
                    assert!(retained.same_owner(original));
                } else {
                    original_bank = Some(retained.clone());
                }
                // This is the original refusal returned by this exact worker call,
                // polled only after all its disposable World state has rolled back.
                let release = reason.release_wait().expect("original capacity source");
                let release_ready = registration
                    .poll_wait(release, &mut Context::from_waker(Waker::noop()))
                    == Poll::Ready(());
                observations[attempt] = Some((
                    *requested_bytes,
                    release_ready,
                    observer.snapshot().0,
                    pool.reserved_bytes(),
                ));
                eprintln!(
                    "UID rollback diagnostic attempt {}: (requested_bytes, original_ready, binding_visits, reserved_bytes)={:?}",
                    attempt + 1,
                    observations[attempt],
                );
                assert!(worker.live.is_none());
                assert!(worker.finishing.is_none());
                assert!(worker.results.is_empty());
                assert!(events.try_recv().is_err());
                assert_eq!(chain.state().view().height(), 1);
                assert_eq!(
                    chain
                        .state()
                        .view()
                        .world()
                        .assets()
                        .get(&AssetId::of(definition(), owner.clone()))
                        .unwrap()
                        .as_ref(),
                    &iroha_primitives::numeric::Quantity::from(10_u32)
                );
                // Move the exact error into the actual scheduler. No reobservation,
                // substitute error, timer advance or external release is introduced.
                scheduler.done(0, ExecDone::Executed(Err(error)));
                assert_eq!(scheduler.outstanding(), 1);
                assert!(scheduler.take_events().is_empty());
                let scheduled = scheduler.next(0);
                if let Some(retry) = scheduled {
                    assert!(
                        release_ready,
                        "RetryGate cannot dispatch before its original capacity observation"
                    );
                    assert!(
                        attempt < 2,
                        "whole execution rollback repeatedly wakes the same native job without external credit return: {observations:?}"
                    );
                    next = retry;
                } else {
                    eprintln!(
                        "UID rollback diagnostic parked after {} refusal(s); original_ready={release_ready}; observations={observations:?}",
                        attempt + 1,
                    );
                    return;
                }
            }
            unreachable!("third refusal either parks or exposes repeated immediate dispatch");
        },
    );
}
