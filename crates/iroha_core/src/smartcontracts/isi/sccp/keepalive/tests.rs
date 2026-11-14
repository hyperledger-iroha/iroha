//! Deterministic shared-carrier clock, original-parent and admission regressions.

use super::*;
use crate::{
    smartcontracts::isi::sccp::{
        admission::{SccpExemptClassV1, SccpPendingIndexV1},
        committee_storage::CommitteeFixtureStorage as _,
        eligibility::BlockParent,
        test_support::{authority, sample_generation, sample_signed_transaction},
    },
    state::World,
};
use iroha_data_model::{sccp::params::SccpParametersV1, transaction::FeePaymentIntent};

fn world(degraded: u32) -> (World, u64) {
    let world = World::new();
    let parameters = SccpParametersV1::taira_default();
    let generation = sample_generation(1);
    let deadline = generation.start_timestamp_ms + parameters.committee_heartbeat_ms;
    let mut block = world.block();
    *block.sccp_parameters.get_mut() = Some(parameters);
    *block.sccp_committee_current.get_mut() = 1;
    *block.sccp_degraded_run.get_mut() = degraded;
    block.sccp_committee_generations.insert(1, generation);
    block.commit();
    (world, deadline)
}

fn payload(time: u64) -> TransactionPayload {
    let mut payload = sample_signed_transaction().payload().clone();
    payload.authority = authority(31);
    payload.instructions = Executable::Instructions(vec![Keepalive {}.into()].into());
    payload.creation_time_ms = time;
    payload.fee_payment = FeePaymentIntent::authority(Vec::new(), None);
    payload
}

#[test]
fn keepalive_k1_counts_carriers_independently_of_outcomes() {
    let mixed = Executable::Instructions(vec![Keepalive {}.into(), Keepalive {}.into()].into());
    assert_eq!(count(&mixed), 2);
    assert!(
        check_count(count(&mixed)).is_err(),
        "K1 must reject every second Keepalive"
    );
    assert_eq!(check_count(0), Ok(()));
    assert_eq!(check_count(1), Ok(()));
}

#[test]
fn keepalive_k2_uses_parent_heartbeat_and_degraded_run() {
    let (world, deadline) = world(1);
    let view = world.view();
    assert!(!heartbeat_due(&Current(&view), deadline - 1));
    assert!(heartbeat_due(&Current(&view), deadline));
    assert_eq!(next_heartbeat_at_ms(&view), Some(deadline));
    drop(view);
    let mut block = world.block();
    *block.sccp_degraded_run.get_mut() = 2;
    assert!(
        !heartbeat_due(&Current(&block), deadline),
        "K2 must stop after two degraded parent heights"
    );
    assert!(
        heartbeat_due(&BlockParent(&block), deadline),
        "this height's failed finalizer cannot revoke parent heartbeat validity"
    );
    assert_eq!(
        due_from(&Current(&block), None, 2, deadline, || Ok(false)),
        Ok(false)
    );
}

#[test]
fn keepalive_progress_keeps_reconciliation_due_after_degraded_heartbeats_stop() {
    use iroha_data_model::{
        bridge::SccpNetworkV1, sccp::reconciliation::SccpReconciliationStateV1,
    };

    let (world, heartbeat) = world(0);
    assert_eq!(next_progress_at_ms(&world.view(), None, 2), Some(heartbeat));
    assert_eq!(next_reconciliation_at_ms(&world.view()), None);
    let reconciliation = heartbeat + 10_000;
    let key = (SccpNetworkV1::EthereumMainnet, 1);
    let mut block = world.block();
    block
        .sccp_reconciliations
        .try_insert_admitted(
            key,
            SccpReconciliationStateV1 {
                opened_at_ms: heartbeat,
                closes_at_ms: reconciliation,
                forged_supply: 0,
                rho: None,
                settled_face: 0,
            },
        )
        .unwrap();
    block
        .sccp_reconciliation_due
        .try_insert_admitted((reconciliation, key.0, key.1), ())
        .unwrap();
    assert_eq!(next_progress_at_ms(&block, None, 2), Some(heartbeat));
    *block.sccp_degraded_run.get_mut() = 2;
    assert_eq!(next_heartbeat_at_ms(&block), None);
    assert_eq!(next_progress_at_ms(&block, None, 2), Some(reconciliation));
    assert_eq!(next_reconciliation_at_ms(&block), Some(reconciliation));
    assert_eq!(due(&block, None, 2, reconciliation - 1), Ok(false));
    assert_eq!(due(&block, None, 2, reconciliation), Ok(true));
    assert_eq!(next_progress_at_ms(&World::new().view(), None, 2), None);
    // A World that is no global chain with the anonymous family has no Parliament deadline.
    assert_eq!(next_governance_at_ms(&block, None, 2), Ok(None));
    assert_eq!(
        next_governance_at_ms(&World::new().view(), Some(0), 2),
        Ok(None)
    );
}

#[test]
fn keepalive_pause_conclusion_stays_due_through_expiry_and_reads_original_parent() {
    use crate::smartcontracts::isi::sccp::{
        pause_record::RetainedSccpFastPauseRound, test_support::sample_fast_pause_round,
    };
    use iroha_data_model::{bridge::SccpNetworkV1, sccp::fast_pause::SccpPanelRoleV1};
    let (world, time) = world(2);
    let key = (SccpNetworkV1::TonMainnet, SccpPanelRoleV1::Backup);
    let mut block = world.block();
    let mut source = sample_fast_pause_round(1);
    source.certify_due = true;
    let expired = source.closes_at_ms.max(time) + 1;
    let round = RetainedSccpFastPauseRound::admit(
        &source,
        &iroha_allocation::AllocationBudget::new(1 << 20),
    )
    .unwrap();
    block.sccp_fast_pause_rounds.insert(key, round);
    assert_eq!(
        due_from(&BlockParent(&block), None, 2, expired, || Ok(false)),
        Ok(false),
        "new fast-pause work cannot replace the original parent due-work observation"
    );
    assert_eq!(
        due_from(&Current(&block), None, 2, expired, || Ok(false)),
        Ok(true)
    );
    assert_eq!(next_progress_at_ms(&block, None, 2), Some(0));
    block.commit();
    let mut block = world.block();
    block.sccp_fast_pause_rounds.remove(key);
    assert_eq!(
        due_from(&BlockParent(&block), None, 3, expired, || Ok(false)),
        Ok(true),
        "an expired original-parent round stays due until its NoResult is processed"
    );
    assert_eq!(
        due_from(&Current(&block), None, 3, expired, || Ok(false)),
        Ok(false)
    );
}

#[test]
fn keepalive_k3_k5_require_standalone_ed25519_due_time_and_empty_fee() {
    let (world, deadline) = world(0);
    let view = world.view();
    let mut payload = payload(deadline - 1);
    let keys = admit(&view, &payload, None, 2, deadline).unwrap().unwrap();
    assert_eq!(keys.class, SccpExemptClassV1::Keepalive);
    assert!(crate::tx::allows_unregistered_authority(
        &payload.instructions,
        &payload.authority
    ));
    let hash = iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([1; 32]));
    let mut pending = SccpPendingIndexV1::default();
    pending.claim(hash, keys.clone()).unwrap();
    assert_eq!(pending.keepalive_holder(&payload.authority), Some(hash));
    assert!(
        pending
            .validate_claim(
                &iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
                    [2; 32]
                )),
                &keys
            )
            .is_err()
    );
    pending.release(&hash);
    assert_eq!(pending.keepalive_holder(&payload.authority), None);
    payload.fee_payment = FeePaymentIntent::authority(Vec::new(), std::num::NonZeroU64::new(1));
    assert!(
        !valid_shape(&payload),
        "K3 must reject fee-bearing Keepalive"
    );
    payload.fee_payment = FeePaymentIntent::authority(Vec::new(), None);
    let pair = iroha_crypto::KeyPair::try_from_seed(vec![4; 32], Algorithm::Secp256k1).unwrap();
    payload.authority = AccountId::new(pair.public_key().clone());
    assert!(!valid_shape(&payload));
    payload.authority = authority(31);
    payload.creation_time_ms = deadline - 2;
    assert!(admit(&view, &payload, None, 2, deadline).is_err());
    payload.creation_time_ms = deadline + view.parameters().sumeragi().max_clock_drift_ms() + 1;
    assert!(admit(&view, &payload, None, 2, deadline).is_err());
}

#[test]
fn keepalive_k4_refreshes_after_half_lag_and_preserves_boundary() {
    assert!(!needs_fresh_carrier(1_000, 31_000, 60_000));
    assert!(
        needs_fresh_carrier(1_000, 31_001, 60_000),
        "K4 must replace a carrier older than half the maximum lag"
    );
    assert!(!needs_fresh_carrier(31_000, 31_001, 60_000));
}

#[test]
fn keepalive_ct5_stale_due_work_is_a_local_refusal() {
    let (world, deadline) = world(0);
    let view = world.view();
    assert_eq!(
        stale_due_work(&view, None, 2, deadline, deadline + 60_000, false),
        Ok(false)
    );
    assert_eq!(
        stale_due_work(&view, None, 2, deadline, deadline + 60_001, false),
        Ok(true),
        "CT5 must refuse stale uncertified due work"
    );
    assert_eq!(
        stale_due_work(&view, None, 2, deadline - 1, deadline + 60_001, false),
        Ok(false)
    );
}

#[test]
fn keepalive_ct5_certified_execution_ignores_the_local_wall_clock() {
    let (world, deadline) = world(0);
    assert_eq!(
        stale_due_work(&world.view(), None, 2, deadline, u64::MAX, true),
        Ok(false),
        "CT5 must exempt genuine certified execution"
    );
}

#[test]
fn heartbeat_due_rejects_early_progress_and_includes_the_exact_deadline() {
    let (world, deadline) = world(0);
    let view = world.view();
    assert!(
        !heartbeat_due(&Current(&view), deadline - 1),
        "SC7 heartbeat must wait for the original inclusive deadline"
    );
    assert_eq!(due(&view, None, 2, deadline - 1), Ok(false));
    assert!(heartbeat_due(&Current(&view), deadline));
    assert_eq!(due(&view, None, 2, deadline), Ok(true));
    assert!(
        admit(&view, &payload(deadline - 1), None, 2, deadline)
            .unwrap()
            .is_some()
    );
}

#[test]
fn governance_due_keeps_the_original_fast_pause_leg_when_heartbeat_is_disabled() {
    use crate::smartcontracts::isi::sccp::{
        pause_record::RetainedSccpFastPauseRound, test_support::sample_fast_pause_round,
    };
    use iroha_data_model::{bridge::SccpNetworkV1, sccp::fast_pause::SccpPanelRoleV1};
    let (world, time) = world(2);
    let key = (SccpNetworkV1::EthereumMainnet, SccpPanelRoleV1::Primary);
    let mut source = sample_fast_pause_round(1);
    source.certify_due = true;
    let expired = source.closes_at_ms.max(time) + 1;
    let mut block = world.block();
    block.sccp_fast_pause_rounds.insert(
        key,
        RetainedSccpFastPauseRound::admit(
            &source,
            &iroha_allocation::AllocationBudget::new(1 << 20),
        )
        .unwrap(),
    );
    block.commit();
    let mut block = world.block();
    block.sccp_fast_pause_rounds.remove(key);
    assert!(!heartbeat_due(&BlockParent(&block), expired));
    assert_eq!(
        due_from(&BlockParent(&block), None, 2, expired, || Ok(false)),
        Ok(true),
        "SC8 due work must retain the original fast-pause conclusion leg"
    );
    assert_eq!(
        due_from(&Current(&block), None, 2, expired, || Ok(false)),
        Ok(false)
    );
}

#[test]
fn keepalive_admission_requires_established_due_work_before_assigning_exemption() {
    let (world, deadline) = world(0);
    let view = world.view();
    let early = payload(deadline - 2);
    assert!(valid_shape(&early));
    assert!(!heartbeat_due(&Current(&view), early.creation_time_ms + 1));
    assert_eq!(due(&view, None, 2, early.creation_time_ms + 1), Ok(false));
    assert!(
        admit(&view, &early, None, 2, deadline).is_err(),
        "SC16 Keepalive admission must reject a carrier without established due work"
    );
    let due = payload(deadline - 1);
    assert!(admit(&view, &due, None, 2, deadline).unwrap().is_some());
    drop(view);
    let mut block = world.block();
    *block.sccp_degraded_run.get_mut() = 2;
    assert!(admit(&block, &due, None, 2, deadline).is_err());
}

#[test]
fn scheduled_time_carrier_uses_exact_parent_without_sccp_or_client_work() {
    use crate::smartcontracts::isi::triggers::specialized::{
        SpecializedAction, SpecializedTrigger,
    };
    use iroha_data_model::{
        events::time::Schedule,
        prelude::{ExecutionTime, Json, Level, Log, Repeats, TimeEventFilter},
    };

    let world = World::new();
    let mut block = world.block();
    let mut tx = block.triggers.transaction();
    let mut action = SpecializedAction::new(
        Executable::Instructions(vec![Log::new(Level::INFO, "scheduled".to_owned()).into()].into()),
        Repeats::Exactly(1),
        authority(31),
        TimeEventFilter(ExecutionTime::Schedule(Schedule {
            start_ms: 100,
            period_ms: None,
        })),
    )
    .unwrap();
    action.metadata.insert(
        "__registered_block_height".parse().unwrap(),
        Json::new(1_u64),
    );
    action.metadata.insert(
        crate::smartcontracts::isi::triggers::progress::DECLARATION_KEY
            .parse()
            .unwrap(),
        Json::new(1_u32),
    );
    crate::smartcontracts::isi::triggers::progress::initialize(&mut action.metadata, 1);
    tx.add_time_trigger(SpecializedTrigger::new(
        "scheduled".parse().unwrap(),
        action,
    ))
    .unwrap();
    tx.apply();
    assert_eq!(
        due_from(&BlockParent(&block), Some(0), 2, 101, || Ok(false)),
        Ok(false),
        "a registration in the carrier block cannot manufacture eligibility"
    );
    block.commit();

    let view = world.view();
    assert_eq!(next_progress_at_ms(&view, Some(0), 2), Some(101));
    assert_eq!(due(&view, Some(0), 2, 100), Ok(false));
    assert_eq!(due(&view, Some(0), 2, 101), Ok(true));
    assert_eq!(due(&view, None, 2, 101), Ok(false));
    assert!(
        admit(&view, &payload(100), Some(0), 2, 100)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        super::super::fees::exempt_class(&view, &payload(100), Some(0), 0),
        Some(SccpExemptClassV1::Keepalive)
    );
    assert_eq!(
        super::super::fees::exempt_class(&view, &payload(100), Some(101), 0),
        None,
        "a rejected one-shot without retry cannot continue creating blocks"
    );
    drop(view);

    let mut block = world.block();
    let mut tx = block.triggers.transaction();
    assert!(tx.remove(&"scheduled".parse().unwrap()));
    tx.apply();
    assert_eq!(
        due_from(&BlockParent(&block), Some(0), 2, 101, || Ok(false)),
        Ok(true),
        "candidate removal cannot revoke the captured carrier admission"
    );
    assert_eq!(
        due_from(&Current(&block), Some(0), 2, 101, || Ok(false)),
        Ok(false)
    );
}

#[test]
fn parliament_terms_are_asked_only_when_no_sccp_term_holds_and_stay_unavailable() {
    let (world, deadline) = world(0);
    let view = world.view();
    let source = Current(&view);
    // A due heartbeat needs no Parliament observation.
    assert_eq!(
        due_from(&source, None, 2, deadline, || panic!(
            "a due SCCP term must not ask Parliament"
        )),
        Ok(true)
    );
    // Before the heartbeat deadline the answer is Parliament's, unavailability included.
    for governance in [Ok(true), Ok(false), Err(DueWorkUnavailable)] {
        assert_eq!(
            due_from(&source, None, 2, deadline - 1, || governance),
            governance,
            "Parliament's due terms decide when no SCCP term holds"
        );
    }
}

#[test]
fn an_authenticated_global_root_observes_due_work_without_decoding_its_committed_scope() {
    use crate::sumeragi::lanes::routing::{read_routing_root_scope, test_support};
    use iroha_data_model::block::consensus::SumeragiRootScope;

    let world = test_support::world(SumeragiRootScope::Global);
    let view = world.view();
    let accounting = || {
        norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            64,
        ))
    };
    // The committed scope is JSON metadata: reading it is a decode charged to its caller.
    let committed_scope = accounting();
    assert_eq!(
        committed_scope
            .with(|| read_routing_root_scope(&view))
            .expect("an unrefused scope read"),
        Some(SumeragiRootScope::Global)
    );
    assert!(committed_scope.consumed_allocated_bytes() > 0);
    // The observation that reads the committed scope answers first, outside the accounting
    // below: a global root with no SCCP or Parliament work has nothing due.
    let committed_due = due(&view, None, 2, 1_000);
    assert_eq!(committed_due, Ok(false));
    let committed = ProgressClock::capture(&view, None, 2, 1_000).expect("clock parameters");
    // An executor that authenticated its root asks Parliament on that root directly, so
    // its routing read stays the first and only decode of that metadata.
    let authenticated = accounting();
    let (observed, clock) = authenticated.with(|| {
        (
            due_on_global_root(&view, None, 2, 1_000),
            ProgressClock::capture_on_global_root(&view, None, 2, 1_000),
        )
    });
    assert_eq!(
        authenticated.consumed_allocated_bytes(),
        0,
        "the authenticated observation must not decode the committed root scope"
    );
    assert_eq!(observed, committed_due);
    let clock = clock.expect("the same clock parameters");
    for (wall_ms, certified) in [(1_000, false), (u64::MAX, false), (u64::MAX, true)] {
        assert_eq!(
            clock.is_stale(wall_ms, certified),
            committed.is_stale(wall_ms, certified)
        );
        assert_eq!(clock.is_stale(wall_ms, certified), Ok(false));
    }
}

#[test]
fn the_global_root_observation_keeps_the_heartbeat_term_and_the_staleness_guard() {
    let (world, deadline) = world(0);
    let view = world.view();
    // The heartbeat leg is the same parent read as the committed-scope observation.
    assert_eq!(due_on_global_root(&view, None, 2, deadline - 1), Ok(false));
    assert_eq!(due_on_global_root(&view, None, 2, deadline), Ok(true));
    let clock = ProgressClock::capture_on_global_root(&view, None, 2, deadline)
        .expect("committed clock parameters");
    assert_eq!(clock.is_stale(deadline + 60_000, false), Ok(false));
    assert_eq!(
        clock.is_stale(deadline + 60_001, false),
        Ok(true),
        "CT5 must refuse stale uncertified due work on an authenticated global root"
    );
    assert_eq!(clock.is_stale(u64::MAX, true), Ok(false));
}
