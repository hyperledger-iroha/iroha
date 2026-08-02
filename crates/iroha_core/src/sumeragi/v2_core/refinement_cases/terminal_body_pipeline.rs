//! Terminal body-pipeline and replay refinement cases.

use super::*;

#[test]
fn decision_ack_retires_competing_owners_and_keeps_one_body_pipeline() {
    let mut terminal = base_facts();
    terminal.action_kind = ACTION_ACKNOWLEDGE_WAL;
    terminal.wal_record_kind = WAL_RECORD_DECISION;
    terminal.event_kind = EVENT_PERSISTED;
    terminal.pending_unchanged = false;
    terminal.acknowledge_persist_exact = true;
    terminal.acknowledgement_continuation = CONTINUATION_DECIDE;
    terminal.volatile_before.body_work = 2;
    terminal.volatile_after.body_work = 1;
    terminal.volatile_after.outbound_control = 1;
    terminal.volatile_after.durable_signable_limit = 0;
    assert!(accepts_facts(terminal));

    let mut stale_pipeline = terminal;
    stale_pipeline.volatile_after.body_work = 2;
    assert!(!accepts_facts(stale_pipeline));

    let mut stale_candidate = terminal;
    stale_candidate.volatile_after.candidate_present = true;
    assert!(!accepts_facts(stale_candidate));

    let mut stale_signature = terminal;
    stale_signature.volatile_after.signature_queue = 1;
    stale_signature.volatile_after.durable_signable_limit = 1;
    assert!(!accepts_facts(stale_signature));

    let mut missing_pipeline = terminal;
    missing_pipeline.volatile_before.body_work = 0;
    assert!(!accepts_facts(missing_pipeline));

    let mut dropped_pipeline = terminal;
    dropped_pipeline.volatile_after.body_work = 0;
    assert!(!accepts_facts(dropped_pipeline));
}

#[test]
fn body_pipeline_classifier_rejects_non_pipeline_effects() {
    let mut stored = base_facts();
    stored.action_kind = ACTION_BODY_PROGRESS;
    stored.event_kind = EVENT_BODY_AVAILABLE;
    assert!(push_authorized(&mut stored.effects, EFFECT_STORE));
    assert!(accepts_facts(stored));

    let mut validated = base_facts();
    validated.action_kind = ACTION_BODY_PROGRESS;
    validated.event_kind = 10;
    assert!(push_authorized(&mut validated.effects, EFFECT_REPORT));
    assert!(accepts_facts(validated));

    let mut invented_broadcast = validated;
    invented_broadcast.effects = EffectTrace::empty();
    assert!(push_authorized(
        &mut invented_broadcast.effects,
        EFFECT_BROADCAST
    ));
    assert!(!accepts_facts(invented_broadcast));

    let mut invented_fetch = validated;
    invented_fetch.effects = EffectTrace::empty();
    assert!(push_authorized(&mut invented_fetch.effects, EFFECT_FETCH));
    assert!(!accepts_facts(invented_fetch));
}

#[test]
fn retransmit_may_reconstruct_one_final_decision_body_stage() {
    let mut store_retry = base_facts();
    store_retry.action_kind = ACTION_VOLATILE_PROTOCOL;
    store_retry.event_kind = 7;
    for _ in 0..7 {
        assert!(push_authorized(&mut store_retry.effects, EFFECT_BROADCAST));
    }
    assert!(push_authorized(&mut store_retry.effects, EFFECT_STORE));
    assert!(accepts_facts(store_retry));

    let mut validate_retry = base_facts();
    validate_retry.action_kind = ACTION_VOLATILE_PROTOCOL;
    validate_retry.event_kind = 7;
    assert!(push_authorized(
        &mut validate_retry.effects,
        EFFECT_BROADCAST
    ));
    assert!(push_authorized(
        &mut validate_retry.effects,
        EFFECT_VALIDATE
    ));
    assert!(accepts_facts(validate_retry));

    let mut not_final = validate_retry;
    not_final.effects = EffectTrace::empty();
    assert!(push_authorized(&mut not_final.effects, EFFECT_VALIDATE));
    assert!(push_authorized(&mut not_final.effects, EFFECT_BROADCAST));
    assert!(!accepts_facts(not_final));

    let mut mixed_stages = validate_retry;
    mixed_stages.effects = EffectTrace::empty();
    assert!(push_authorized(&mut mixed_stages.effects, EFFECT_STORE));
    assert!(push_authorized(&mut mixed_stages.effects, EFFECT_VALIDATE));
    assert!(!accepts_facts(mixed_stages));

    let mut fetch_and_store = validate_retry;
    fetch_and_store.effects = EffectTrace::empty();
    assert!(push_authorized(&mut fetch_and_store.effects, EFFECT_FETCH));
    assert!(push_authorized(&mut fetch_and_store.effects, EFFECT_STORE));
    assert!(!accepts_facts(fetch_and_store));

    let mut report_and_store = validate_retry;
    report_and_store.effects = EffectTrace::empty();
    assert!(push_authorized(
        &mut report_and_store.effects,
        EFFECT_REPORT
    ));
    assert!(push_authorized(&mut report_and_store.effects, EFFECT_STORE));
    assert!(!accepts_facts(report_and_store));

    let mut apply_and_fetch = validate_retry;
    apply_and_fetch.effects = EffectTrace::empty();
    assert!(push_authorized(&mut apply_and_fetch.effects, EFFECT_APPLY));
    assert!(push_authorized(&mut apply_and_fetch.effects, EFFECT_FETCH));
    assert!(!accepts_facts(apply_and_fetch));

    let mut fetch_not_final = validate_retry;
    fetch_not_final.effects = EffectTrace::empty();
    assert!(push_authorized(&mut fetch_not_final.effects, EFFECT_FETCH));
    assert!(push_authorized(
        &mut fetch_not_final.effects,
        EFFECT_BROADCAST
    ));
    assert!(!accepts_facts(fetch_not_final));

    let mut wrong_event = validate_retry;
    wrong_event.event_kind = 6;
    assert!(!accepts_facts(wrong_event));
}

#[test]
fn signed_classifier_and_inactive_slots_are_canonical() {
    let mut invented_signed_transition = base_facts();
    invented_signed_transition.event_kind = EVENT_SIGNED;
    invented_signed_transition.action_kind = ACTION_VOLATILE_PROTOCOL;
    assert!(!accepts_facts(invented_signed_transition));

    let mut noncanonical_empty = base_facts();
    noncanonical_empty.effects.slot0 = EffectSlotProjection {
        kind: EFFECT_BROADCAST,
        requested: EffectCapabilityKey::none(),
        granted: EffectCapabilityKey::none(),
    };
    assert!(!accepts_facts(noncanonical_empty));

    let mut impossible_roster = base_facts();
    impossible_roster.validator_count = u64::MAX / 2 + 1;
    assert!(!accepts_facts(impossible_roster));
}

#[test]
fn replay_resume_has_a_distinct_one_shot_effect_relation() {
    let mut resumed = base_facts();
    resumed.event_kind = EVENT_RESUME_AFTER_REPLAY;
    resumed.action_kind = ACTION_RESUME_AFTER_REPLAY;
    resumed.replay_effect_kind = REPLAY_EFFECT_PREPARE;
    resumed.volatile_after.replay_resumed = true;
    resumed.volatile_after.awaiting_signature = true;
    assert!(push_authorized(&mut resumed.effects, EFFECT_SIGN));
    assert!(accepts_facts(resumed));

    let mut stale_did_work = resumed;
    stale_did_work.tag_matches = false;
    assert!(!accepts_facts(stale_did_work));

    let mut replayed_twice = resumed;
    replayed_twice.volatile_before.replay_resumed = true;
    assert!(!accepts_facts(replayed_twice));

    let mut decision_fetch = base_facts();
    decision_fetch.event_kind = EVENT_RESUME_AFTER_REPLAY;
    decision_fetch.action_kind = ACTION_RESUME_AFTER_REPLAY;
    decision_fetch.replay_effect_kind = REPLAY_EFFECT_DECISION;
    decision_fetch.volatile_after.replay_resumed = true;
    decision_fetch.volatile_after.body_work = 1;
    assert!(push_authorized(&mut decision_fetch.effects, EFFECT_FETCH));
    assert!(accepts_facts(decision_fetch));
}
