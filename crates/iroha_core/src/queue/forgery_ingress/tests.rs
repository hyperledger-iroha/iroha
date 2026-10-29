//! Exact bounded ingress-state controls; native source tests below use real certificates.

use super::*;
use crate::smartcontracts::isi::sccp::store;
use crate::state::sccp_forgery_custody::OffenceKey;
use iroha_primitives::time::TimeSource;
use mv::storage::StorageReadOnly;
use std::{alloc::Layout, sync::Arc, time::Duration};

const REMOTE: SubmissionOrigin = SubmissionOrigin::Remote;

fn triple(index: u64) -> IngressKey {
    let mut message = [0; 32];
    message[..8].copy_from_slice(&index.to_be_bytes());
    IngressKey {
        generation: 1,
        message,
        signers: 7,
        signature: [0; 32],
    }
}

fn queue() -> Queue {
    let (_, time) = TimeSource::new_mock(Duration::default());
    Queue::test(iroha_config::parameters::actual::Queue::default(), &time)
}

#[test]
fn exact_burst_and_fractional_refill_are_eight_pairings_per_second() {
    let pool = AllocationBudget::new(1 << 20);
    let now = Instant::now();
    let mut state = ForgeryIngressState::new(&pool, now).unwrap();
    for index in 0..32 {
        state.observe(triple(index), now).unwrap();
    }
    assert!(matches!(
        state.observe(triple(32), now + Duration::from_nanos(TOKEN_NS - 1)),
        Err(ForgeryIngressError::RateLimited)
    ));
    state
        .observe(triple(33), now + Duration::from_nanos(TOKEN_NS))
        .unwrap();
    assert_eq!(state.credit_ns, 0);
    for index in 34..42 {
        state
            .observe(
                triple(index),
                now + Duration::from_nanos(TOKEN_NS) + Duration::from_secs(1),
            )
            .unwrap();
    }
    assert!(matches!(
        state.observe(
            triple(42),
            now + Duration::from_nanos(TOKEN_NS) + Duration::from_secs(1)
        ),
        Err(ForgeryIngressError::RateLimited)
    ));
    for index in 43..75 {
        state
            .observe(triple(index), now + Duration::from_secs(100))
            .unwrap();
    }
    assert!(matches!(
        state.observe(triple(75), now + Duration::from_secs(100)),
        Err(ForgeryIngressError::RateLimited)
    ));
}

#[test]
fn backward_clock_never_refills_or_rewinds_the_bucket() {
    let pool = AllocationBudget::new(1 << 20);
    let earlier = Instant::now();
    let now = earlier + Duration::from_secs(1);
    let mut state = ForgeryIngressState::new(&pool, now).unwrap();
    for index in 0..32 {
        state.observe(triple(index), now).unwrap();
    }
    assert!(matches!(
        state.observe(triple(32), earlier),
        Err(ForgeryIngressError::RateLimited)
    ));
    assert_eq!(state.last_refill, now);
    assert!(matches!(
        state.observe(triple(33), now + Duration::from_nanos(TOKEN_NS - 1)),
        Err(ForgeryIngressError::RateLimited)
    ));
    state
        .observe(triple(34), now + Duration::from_nanos(TOKEN_NS))
        .unwrap();
}

#[test]
fn rate_refusals_are_never_remembered() {
    let pool = AllocationBudget::new(1 << 20);
    let now = Instant::now();
    let mut state = ForgeryIngressState::new(&pool, now).unwrap();
    for index in 0..32 {
        state.observe(triple(index), now).unwrap();
    }
    for _ in 0..2 {
        assert!(
            matches!(
                state.observe(triple(32), now),
                Err(ForgeryIngressError::RateLimited)
            ),
            "a rate-refused submission must stay unseen"
        );
    }
    state
        .observe(triple(32), now + Duration::from_nanos(TOKEN_NS))
        .expect("a rate-refused submission reaches the pairing once a token refills");
    assert!(matches!(
        state.observe(triple(32), now + Duration::from_secs(5)),
        Err(ForgeryIngressError::Seen)
    ));
}

#[test]
fn lru_touches_repeats_before_eviction() {
    let pool = AllocationBudget::new(1 << 20);
    let now = Instant::now();
    let at = |index: u64| now + Duration::from_nanos(TOKEN_NS * index);
    let mut state = ForgeryIngressState::new(&pool, now).unwrap();
    for index in 0..CACHE_CAPACITY as u64 {
        state.observe(triple(index), at(index)).unwrap();
    }
    assert_eq!(state.seen.as_slice().len(), CACHE_CAPACITY);
    let full = at(CACHE_CAPACITY as u64);
    let credit = state.credit_ns;
    assert!(matches!(
        state.observe(triple(0), full),
        Err(ForgeryIngressError::Seen)
    ));
    assert_eq!(state.credit_ns, credit, "a repeat consumes no token");
    state.observe(triple(CACHE_CAPACITY as u64), full).unwrap();
    assert!(matches!(
        state.observe(triple(0), full),
        Err(ForgeryIngressError::Seen)
    ));
    state
        .observe(triple(1), full)
        .expect("the untouched oldest key was evicted");
}

#[test]
fn cache_key_includes_generation_digest_signers_and_signature_exactly() {
    let pool = AllocationBudget::new(1 << 20);
    let now = Instant::now();
    let mut state = ForgeryIngressState::new(&pool, now).unwrap();
    let original = triple(0);
    let mut generation = original;
    generation.generation = 2;
    let mut digest = original;
    digest.message[31] = 1;
    let mut signers = original;
    signers.signers = 11;
    let mut signature = original;
    signature.signature[0] = 1;
    for key in [original, generation, digest, signers, signature] {
        state.observe(key, now).unwrap();
    }
    for key in [original, generation, digest, signers, signature] {
        assert!(matches!(
            state.observe(key, now),
            Err(ForgeryIngressError::Seen)
        ));
    }
}

#[test]
fn queue_handles_share_cache_and_original_pool_until_last_drop() {
    let pool = AllocationBudget::new(1 << 20);
    let other = AllocationBudget::new(1 << 20);
    let queue = Arc::new(queue());
    let second_handle = queue.clone();
    let now = Instant::now();
    queue.observe_forgery(triple(1), &pool, now).unwrap();
    let backing = Layout::array::<IngressKey>(CACHE_CAPACITY).unwrap().size();
    assert_eq!(pool.reserved_bytes(), backing);
    assert!(matches!(
        second_handle.observe_forgery(triple(1), &pool, now),
        Err(ForgeryIngressError::Seen)
    ));
    queue.clear_all();
    assert!(matches!(
        second_handle.observe_forgery(triple(1), &pool, now),
        Err(ForgeryIngressError::Seen)
    ));
    assert!(matches!(
        queue.observe_forgery(triple(2), &other, now),
        Err(ForgeryIngressError::PoolMismatch)
    ));
    assert_eq!(other.reserved_bytes(), 0);
    drop(queue);
    assert_eq!(pool.reserved_bytes(), backing);
    drop(second_handle);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn concurrent_shared_queue_observations_cannot_double_spend_a_token_or_triple() {
    let pool = AllocationBudget::new(1 << 20);
    let queue = Arc::new(queue());
    let now = Instant::now();
    let wins = std::thread::scope(|scope| {
        let mut threads = Vec::with_capacity(64);
        for _ in 0..64 {
            let queue = queue.clone();
            let pool = pool.clone();
            threads.push(scope.spawn(move || queue.observe_forgery(triple(7), &pool, now).is_ok()));
        }
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .filter(|won| *won)
            .count()
    });
    assert_eq!(wins, 1);
    assert_eq!(
        queue.forgery_ingress.lock().as_ref().unwrap().credit_ns,
        BURST_NS - TOKEN_NS
    );
}

#[test]
fn original_cache_capacity_refusal_is_typed_and_does_not_install_seen_state() {
    let backing = Layout::array::<IngressKey>(CACHE_CAPACITY).unwrap().size();
    let pool = AllocationBudget::new(backing);
    let occupied = pool.try_reserve_bytes(1).unwrap();
    let queue = queue();
    let now = Instant::now();
    let error = queue.observe_forgery(triple(1), &pool, now).unwrap_err();
    let ForgeryIngressError::Allocation(original) = error else {
        panic!("exact original capacity refusal");
    };
    assert!(matches!(original, AllocationRefusal::Capacity { .. }));
    let Error::Deferred(owner) = ForgeryIngressError::Allocation(original.clone()).into_queue()
    else {
        panic!("typed local refusal");
    };
    assert_eq!(owner.allocation_refusal(), Some(&original));
    assert!(queue.forgery_ingress.lock().is_none());
    drop(occupied);
    queue.observe_forgery(triple(1), &pool, now).unwrap();
}

fn signed_evidence(
    view: &StateView<'_>,
    evidence: SubmitSccpForgeryEvidenceV1,
) -> SignedTransaction {
    use iroha_data_model::{
        account::AccountId,
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    let key =
        iroha_crypto::KeyPair::try_from_seed(vec![0x25; 32], iroha_crypto::Algorithm::Ed25519)
            .unwrap();
    let mut builder = TransactionBuilder::new(
        *view.network_id(),
        AccountId::new(key.public_key().clone()),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([evidence]);
    builder.set_creation_time(Duration::from_millis(3_000));
    builder.sign(key.private_key())
}

#[test]
fn actual_view_admission_verifies_native_evidence_and_never_publishes_it() {
    use crate::{
        queue::{EagerAdmissionStateAccess, QueueAdmissionStateAccess},
        state::WorldReadOnly,
    };
    use forgery::tests::{active_member_chain, future};
    let _retirement_pin = crossbeam_epoch::pin();
    let (chain, keys) = active_member_chain();
    let evidence = future(&chain, &keys);
    let view = chain.state().view();
    let precheck = forgery::prepare_committed(&view, &evidence, 3_000).unwrap();
    let parts = (
        precheck.generation().generation,
        *precheck.message(),
        precheck.qc().signers,
    );
    let transaction = signed_evidence(&view, evidence);
    let pool = view.execution_budget();
    let baseline = pool.reserved_bytes();
    let queue = queue();
    let mut access = EagerAdmissionStateAccess::new(&view, chain.height() + 1, 2_000);
    let checked = access
        .sccp_exempt_admission(&queue, &transaction)
        .unwrap()
        .unwrap();
    assert_eq!(
        checked,
        SccpAdmissionKeysV1::new(SccpExemptClassV1::ForgeryEvidence)
            .with_exclusive(&parts)
            .with_content(&parts)
    );
    assert!(matches!(
        access.sccp_exempt_admission(&queue, &transaction),
        Err(Error::SccpForgeryIngress(ForgeryIngressError::Seen))
    ));
    assert_eq!(
        queue.active_len(),
        0,
        "preverification creates no queue publication by itself"
    );
    assert!(view.world().sccp_forgery_offences().is_empty());
    assert_eq!(store::forgery_max_generation(view.world()), None);
    assert_eq!(
        pool.reserved_bytes(),
        baseline + Layout::array::<IngressKey>(CACHE_CAPACITY).unwrap().size()
    );
    drop(queue);
    assert_eq!(pool.reserved_bytes(), baseline);
}

#[test]
fn plain_world_classify_cannot_bypass_original_native_queue_owner() {
    use forgery::tests::{active_member_chain, future};
    let (chain, keys) = active_member_chain();
    let evidence = future(&chain, &keys);
    let view = chain.state().view();
    let transaction = signed_evidence(&view, evidence);
    assert!(
        crate::smartcontracts::isi::sccp::admission::classify(
            view.world(),
            &view,
            chain.height() + 1,
            &transaction
        )
        .is_err()
    );
}

#[test]
fn genuine_and_cheaply_invalid_sources_never_install_seen_state() {
    use forgery::tests::{active_member_chain, change_header, future, original};
    let (chain, keys) = active_member_chain();
    let genuine = original(&chain);
    let mut foreign = future(&chain, &keys);
    change_header(&mut foreign, |fields| fields.network_id[0] ^= 1);
    let view = chain.state().view();
    let queue = queue();
    for evidence in [genuine, foreign] {
        assert!(matches!(
            queue.admit_forgery(&view, &signed_evidence(&view, evidence), REMOTE),
            Err(Error::SccpForgeryIngress(
                ForgeryIngressError::Verification(ForgeryVerificationError::NotEvidence)
            ))
        ));
        assert!(queue.forgery_ingress.lock().is_none());
    }
}

#[test]
fn no_new_slashable_g6_member_is_duplicate_before_lru_and_pairing() {
    use crate::state::WorldReadOnly as _;
    use forgery::tests::{active_member_chain_with_penalty_delay, future};
    let (mut chain, keys, ordinary) = active_member_chain_with_penalty_delay(2);
    let evidence = future(&chain, &keys);
    let created_ms = chain
        .state()
        .view()
        .native_execution_tip()
        .unwrap()
        .creation_time_ms()
        + 1;
    let signed = chain.sign(&ordinary, [evidence.into()], created_ms);
    assert_eq!(chain.commit(vec![signed]), [true]);
    assert_eq!(
        chain.state().view().world().sccp_forgery_offences().len(),
        3,
        "the genuine exact quorum already has permanent offences"
    );

    // All G6 members are registered by the signed genesis. Record their actual
    // offences first, then prove that another claim by the same quorum is cheap
    // Duplicate even with an invalid signature, before LRU admission or pairing.
    let mut evidence = future(&chain, &keys);
    evidence.aggregate_signature = [0; 96];
    let created_ms = chain
        .state()
        .view()
        .native_execution_tip()
        .unwrap()
        .creation_time_ms()
        + 1;
    let transaction = chain.sign(&ordinary, [evidence.into()], created_ms);
    let view = chain.state().view();
    let queue = queue();
    assert!(matches!(
        queue.admit_forgery(&view, &transaction, REMOTE),
        Err(Error::SccpForgeryIngress(ForgeryIngressError::Duplicate))
    ));
    assert!(queue.forgery_ingress.lock().is_none());
}

#[test]
fn refused_signature_never_shadows_the_genuine_certificate() {
    use crate::state::WorldReadOnly;
    use forgery::tests::{active_member_chain, future};
    let (chain, keys) = active_member_chain();
    let valid = future(&chain, &keys);
    let mut invalid = valid.clone();
    invalid.aggregate_signature = [0; 96];
    let view = chain.state().view();
    let queue = queue();
    assert!(matches!(
        queue.admit_forgery(&view, &signed_evidence(&view, invalid.clone()), REMOTE),
        Err(Error::SccpForgeryIngress(
            ForgeryIngressError::Verification(ForgeryVerificationError::BadSignature)
        ))
    ));
    assert!(matches!(
        queue.admit_forgery(&view, &signed_evidence(&view, invalid), REMOTE),
        Err(Error::SccpForgeryIngress(ForgeryIngressError::Seen))
    ));
    assert!(
        queue
            .admit_forgery(&view, &signed_evidence(&view, valid.clone()), REMOTE)
            .is_ok(),
        "a refused signature must not shadow the genuine certificate of the same triple"
    );
    assert!(matches!(
        queue.admit_forgery(&view, &signed_evidence(&view, valid), REMOTE),
        Err(Error::SccpForgeryIngress(ForgeryIngressError::Seen))
    ));
    assert!(view.world().sccp_forgery_offences().is_empty());
    assert_eq!(store::forgery_max_generation(view.world()), None);
    assert_eq!(queue.active_len(), 0);
}

#[test]
fn post_insertion_original_scratch_refusal_is_forgotten_and_preserves_release_owner() {
    use forgery::tests::{active_member_chain, future};
    use iroha_crypto::bls::consensus::ConsensusAggregateScratch;
    let _retirement_pin = crossbeam_epoch::pin();
    let (chain, keys) = active_member_chain();
    let evidence = future(&chain, &keys);
    let view = chain.state().view();
    let transaction = signed_evidence(&view, evidence);
    let budget = view.execution_budget();
    let queue = queue();
    queue
        .observe_forgery(triple(999), &budget, Instant::now())
        .unwrap();
    let scratch = ConsensusAggregateScratch::<()>::backing_bytes();
    let generation = store::committee_generations::get(view.world(), &1).unwrap();
    let member = generation.members[0].as_ref().unwrap();
    let baseline = budget.reserved_bytes();
    let probe = OffenceKey::try_copy(1, member.lane_id, &member.validator, &budget).unwrap();
    assert!(
        budget.reserved_bytes() - baseline < scratch,
        "the earlier original query fits while exact pairing backing does not"
    );
    drop(probe);
    let occupied = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes() - (scratch - 1))
        .unwrap();
    let error = queue
        .admit_forgery(&view, &transaction, REMOTE)
        .unwrap_err();
    let Error::Deferred(owner) = error else {
        panic!("exact original scratch capacity owner: {error:?}")
    };
    assert!(
        matches!(owner.allocation_refusal(), Some(AllocationRefusal::Capacity { requested_bytes, .. }) if *requested_bytes == scratch)
    );
    drop(occupied);
    assert!(
        queue.admit_forgery(&view, &transaction, REMOTE).is_ok(),
        "a local refusal is no verdict: the retry reaches the pairing once capacity returns"
    );
    assert_eq!(queue.active_len(), 0);
}

#[test]
fn non_retryable_original_allocation_errors_keep_their_typed_category() {
    for original in [
        AllocationRefusal::DemandOverflow,
        AllocationRefusal::ExceedsLimit {
            requested_bytes: 9,
            limit_bytes: 8,
        },
    ] {
        let Error::SccpForgeryIngress(ForgeryIngressError::Allocation(retained)) =
            ForgeryIngressError::Allocation(original.clone()).into_queue()
        else {
            panic!("local recovery category")
        };
        assert_eq!(retained, original);
    }
    assert!(matches!(
        ForgeryIngressError::Allocator { bytes: 17 }.into_queue(),
        Error::SccpForgeryIngress(ForgeryIngressError::Allocator { bytes: 17 })
    ));
}

#[test]
fn drained_pairing_bucket_refuses_remote_evidence_but_admits_the_local_keeper() {
    use crate::tx::AcceptedTransaction;
    use forgery::tests::{active_member_chain, future};
    let _retirement_pin = crossbeam_epoch::pin();
    let (chain, keys) = active_member_chain();
    let evidence = future(&chain, &keys);
    let state = chain.state();
    let (_, time) = TimeSource::new_mock(Duration::from_millis(3_000));
    let queue = Queue::test(iroha_config::parameters::actual::Queue::default(), &time);
    let mut forged = evidence.clone();
    forged.aggregate_signature = [0; 96];
    let (genuine, forged) = {
        let view = state.view();
        // A distributed flood of fabricated evidence keeps the node-wide bucket empty. Its
        // last observation lies ahead of the wall clock, so no token refills meanwhile.
        let flood = Instant::now() + Duration::from_secs(3_600);
        for index in 0..32 {
            queue
                .observe_forgery(triple(index), &view.execution_budget(), flood)
                .unwrap();
        }
        let parameters = state.transaction_admission_parameters();
        let accept = |evidence| {
            AcceptedTransaction::accept_with_time_source(
                signed_evidence(&view, evidence),
                state.network_id_ref(),
                parameters.sumeragi().max_clock_drift(),
                parameters.transaction(),
                parameters.require_crypto_policy().unwrap(),
                &time,
            )
            .unwrap()
        };
        (accept(evidence), accept(forged))
    };
    let remote = queue
        .push_with_lane_with_state(genuine.clone(), state)
        .unwrap_err();
    assert!(
        matches!(
            remote.err,
            Error::SccpForgeryIngress(ForgeryIngressError::RateLimited)
        ),
        "{:?}",
        remote.err
    );
    // The keeper still pays the full pairing: a refused signature stays refused.
    let refused = queue
        .push_local_keeper_with_state(forged, state)
        .unwrap_err();
    assert!(
        matches!(
            refused.err,
            Error::SccpForgeryIngress(ForgeryIngressError::Verification(
                ForgeryVerificationError::BadSignature
            ))
        ),
        "{:?}",
        refused.err
    );
    queue
        .push_local_keeper_with_state(genuine, state)
        .expect("the keeper's genuine evidence must not wait behind a remote flood");
    assert_eq!(queue.active_len(), 1);
    // Neither keeper submission took a token or entered the remote repeat cache.
    let ingress = queue.forgery_ingress.lock();
    let ingress = ingress.as_ref().unwrap();
    assert_eq!(ingress.credit_ns, 0);
    assert_eq!(ingress.seen.as_slice().len(), 32);
}

#[test]
fn unresolved_signers_reach_the_pairing_until_their_generation_is_recorded() {
    use crate::state::WorldReadOnly as _;
    use forgery::tests::unresolved_member_chain;
    use iroha_data_model::block::consensus::EvidencePenaltyStatus;
    let (mut chain, _, ordinary, evidence) = unresolved_member_chain();
    let created_ms = |chain: &crate::sumeragi::test_chain::CertifiedTestChain| {
        chain
            .state()
            .view()
            .native_execution_tip()
            .unwrap()
            .creation_time_ms()
            + 1
    };
    {
        // No signer is slashable and no record names generation 1: not a duplicate.
        let transaction = chain.sign(&ordinary, [evidence.clone().into()], created_ms(&chain));
        let view = chain.state().view();
        assert!(queue().admit_forgery(&view, &transaction, REMOTE).is_ok());
    }
    let signed = chain.sign(&ordinary, [evidence.clone().into()], created_ms(&chain));
    assert_eq!(chain.commit(vec![signed]), [true]);
    let recorded_at = chain.height();
    {
        let view = chain.state().view();
        assert_eq!(store::forgery_max_generation(view.world()), Some(1));
        assert!(view.world().sccp_forgery_offences().is_empty());
        let (_, record) = view.world().consensus_evidence().iter().next().unwrap();
        assert_eq!(record.attribution.offenders, []);
        assert_eq!(record.penalty_status, EvidencePenaltyStatus::Pending);
        // The recorded generation makes a resubmission cheap Duplicate, before LRU and pairing.
        let transaction = chain.sign(&ordinary, [evidence.into()], created_ms(&chain));
        let queue = queue();
        assert!(matches!(
            queue.admit_forgery(&view, &transaction, REMOTE),
            Err(Error::SccpForgeryIngress(ForgeryIngressError::Duplicate))
        ));
        assert!(queue.forgery_ingress.lock().is_none());
    }
    // The penalty pipeline marks the record without offenders Applied and slashes nothing.
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    let view = chain.state().view();
    let (_, record) = view.world().consensus_evidence().iter().next().unwrap();
    assert_eq!(
        record.penalty_status,
        EvidencePenaltyStatus::Applied {
            height: recorded_at + 2
        }
    );
}
