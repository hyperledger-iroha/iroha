// Production-kernel gate and refinement proof tail.
//
// Included lexically by `verus_proofs` so public item paths remain unchanged.

verus! {

/// Exact executable volatile-cardinality checker used by production.
pub fn verified_volatile_summary_gate(
    summary: ProductionVolatileSummaryProjection,
    validator_count: u64,
) -> (accepted: bool)
    ensures
        accepted ==> production_volatile_summary_well_formed(summary, validator_count),
{
    volatile_summary_well_formed_body!(summary, validator_count)
}

/// Exact executable signed-completion classifier used by production.
pub fn verified_signed_message_class_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_signed_message_class_relation(facts),
{
    let accepted = signed_message_class_body!(facts);
    proof {
        reveal(production_signed_message_class_relation);
    }
    accepted
}

/// Exact executable stutter-action checker used by production.
pub fn verified_stutter_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_stutter_action_relation(facts),
{
    let accepted = stutter_action_body!(facts);
    proof {
        reveal(production_stutter_action_relation);
    }
    accepted
}

/// Exact executable begin-WAL action checker used by production.
pub fn verified_begin_wal_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_begin_wal_action_relation(facts),
{
    let accepted = begin_wal_action_body!(facts);
    proof {
        reveal(production_begin_wal_action_relation);
    }
    accepted
}

/// Exact executable acknowledge-WAL action checker used by production.
pub fn verified_acknowledge_wal_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_acknowledge_wal_action_relation(facts),
{
    let accepted = acknowledge_wal_action_body!(facts);
    proof {
        reveal(production_acknowledge_wal_action_relation);
    }
    accepted
}

/// Exact executable successful-validation effect checker used by production.
pub fn verified_validation_completed_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_validation_completed_action_relation(facts),
{
    let accepted = validation_completed_action_body!(facts, verified_effect_count_gate);
    proof {
        reveal(production_validation_completed_action_relation);
    }
    accepted
}

/// Exact executable body-progress action checker used by production.
pub fn verified_body_progress_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_body_progress_action_relation(facts),
{
    let accepted = body_progress_action_body!(facts, verified_validation_completed_action_gate);
    proof {
        reveal(production_body_progress_action_relation);
    }
    accepted
}

/// Exact executable volatile-protocol action checker used by production.
#[verifier::spinoff_prover]
pub fn verified_volatile_protocol_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_volatile_protocol_action_relation(facts),
{
    let accepted = volatile_protocol_action_body!(facts);
    proof {
        reveal(production_volatile_protocol_action_relation);
    }
    accepted
}

/// Exact executable application-completion action checker used by production.
pub fn verified_complete_application_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_complete_application_action_relation(facts),
{
    let accepted = complete_application_action_body!(facts);
    proof {
        reveal(production_complete_application_action_relation);
    }
    accepted
}

/// Exact executable replay-resumption checker used by production.
pub fn verified_resume_after_replay_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_resume_after_replay_action_relation(facts),
{
    let accepted = resume_after_replay_action_body!(facts, verified_effect_count_gate);
    proof {
        reveal(production_resume_after_replay_action_relation);
    }
    accepted
}

/// Exact executable action-discriminant checker used by production.
pub fn verified_action_kind_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_action_kind_relation(facts),
{
    let accepted = action_kind_relation_body!(
        facts,
        verified_stutter_action_gate,
        verified_begin_wal_action_gate,
        verified_acknowledge_wal_action_gate,
        verified_body_progress_action_gate,
        verified_volatile_protocol_action_gate,
        verified_complete_application_action_gate,
        verified_resume_after_replay_action_gate,
    );
    proof {
        reveal(production_action_kind_relation);
    }
    accepted
}

/// Exact executable action/WAL/signature-class checker used by production.
pub fn verified_named_action_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_named_action_relation(facts),
{
    let accepted = production_action_relation_body!(
        facts,
        verified_signed_message_class_gate,
        verified_action_kind_gate,
    );
    proof {
        reveal(production_named_action_relation);
    }
    accepted
}

/// Exact executable per-slot authorization checker used by production.
pub fn verified_effect_slots_gate(
    trace: ProductionEffectTraceProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_effect_slots_authorized(trace),
{
    effect_slots_authorized_body!(trace)
}

/// Exact executable effect-discriminant counter used by production.
pub fn verified_effect_count_gate(
    trace: ProductionEffectTraceProjection,
    kind: u8,
) -> (count: u64)
    ensures
        count == production_effect_count(trace, kind),
{
    effect_count_body!(trace, kind)
}

/// Exact executable order constraints used by production.
pub fn verified_effect_order_constraints_gate(
    trace: ProductionEffectTraceProjection,
    event_kind: u8,
    persist_count: u64,
    fetch_count: u64,
    store_count: u64,
    validate_count: u64,
    sign_count: u64,
    apply_count: u64,
    enter_count: u64,
) -> (accepted: bool)
    ensures
        accepted ==> production_effect_order_constraints(
            trace,
            event_kind,
            persist_count,
            fetch_count,
            store_count,
            validate_count,
            sign_count,
            apply_count,
            enter_count,
        ),
{
    effect_order_constraints_body!(
        trace,
        event_kind,
        persist_count,
        fetch_count,
        store_count,
        validate_count,
        sign_count,
        apply_count,
        enter_count,
    )
}

/// Exact executable vector-order checker used by production.
pub fn verified_effect_order_gate(
    trace: ProductionEffectTraceProjection,
    event_kind: u8,
) -> (accepted: bool)
    ensures
        accepted ==> production_effect_order_relation(trace, event_kind),
{
    effect_ordering_gate_body!(
        trace,
        event_kind,
        verified_effect_count_gate,
        verified_effect_order_constraints_gate,
    )
}

/// Exact executable combined effect checker used by the production gate.
pub fn verified_effect_trace_gate(
    trace: ProductionEffectTraceProjection,
    event_kind: u8,
) -> (accepted: bool)
    ensures
        accepted ==> production_effect_trace_relation(trace, event_kind),
{
    effect_trace_gate_body!(
        trace,
        event_kind,
        verified_effect_slots_gate,
        verified_effect_order_gate,
    )
}

/// Exact executable branch constraints used by the production gate.
pub fn verified_transition_branch_constraints_gate(
    facts: ProductionTransitionFactsProjection,
    persist_count: u64,
    fetch_count: u64,
    sign_count: u64,
    apply_count: u64,
    enter_count: u64,
) -> (accepted: bool)
    ensures
        accepted ==> production_transition_branch_constraints(
            facts,
            persist_count,
            fetch_count,
            sign_count,
            apply_count,
            enter_count,
        ),
{
    transition_branch_constraints_body!(
        facts,
        persist_count,
        fetch_count,
        sign_count,
        apply_count,
        enter_count,
    )
}

/// Exact executable branch checker used by the production gate.
pub fn verified_transition_branch_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_transition_branch_relation(facts),
{
    transition_branch_gate_body!(
        facts,
        verified_effect_count_gate,
        verified_transition_branch_constraints_gate,
    )
}

/// Exact executable decision procedure used by `refinement::accepts` in a
/// normal build.  The body is one shared macro expansion, not a transcription.
pub fn verified_production_transition_gate(
    facts: ProductionTransitionFactsProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_transition_action_relation(facts),
{
    let accepted = production_transition_gate_body!(
        facts,
        verified_volatile_summary_gate,
        verified_named_action_gate,
        verified_effect_trace_gate,
        verified_transition_branch_gate,
    );
    proof {
        reveal(production_transition_action_relation);
    }
    accepted
}

/// Safety relation of the exact primitive production kernel.
pub closed spec fn production_kernel_relation(
    projection: ProductionTransitionProjection,
) -> bool {
    production_transition_action_relation(production_facts_from_projection(projection))
}

/// An accepted active persisted-TC transition selects the maximum of the
/// pre-transition lock and the TC's highest `PrepareQC`, carries that exact
/// durable lock in `EnterView`, and emits one immediately following recovery
/// fetch exactly when the selected lock is present.
///
/// This is a serialized transition theorem only. It intentionally makes no
/// claim that asynchronous transport or executor work is eventually serviced.
pub proof fn accepted_core_enter_view_projection_selects_post_install_lock(
    projection: ProductionTransitionProjection,
)
    requires
        production_kernel_relation(projection),
        projection.enter_view.active,
    ensures
        production_enter_view_projection_relation(projection.enter_view),
        production_enter_view_preserves_locked_prepare_qc_identity(projection.enter_view),
        production_enter_view_retains_high_prepare_qc_identity(projection.enter_view),
        certificate_identity_equal_body!(
            projection.enter_view.durable_lock_after,
            production_enter_view_selected_lock(projection.enter_view)
        ),
        certificate_identity_equal_body!(
            projection.enter_view.effect_protected_lock,
            projection.enter_view.durable_lock_after
        ),
        projection.enter_view.durable_lock_after.present
            <==> production_enter_view_has_exact_following_fetch(projection.enter_view),
        !projection.enter_view.durable_lock_after.present
            ==> projection.enter_view.fetch_count == 0
                && !projection.enter_view.following_fetch_lock.present,
{
    reveal(production_kernel_relation);
    reveal(production_transition_action_relation);
    reveal(production_facts_from_projection);
    reveal(production_enter_view_projection_relation);
    reveal(production_enter_view_preserves_locked_prepare_qc_identity);
    reveal(production_enter_view_retains_high_prepare_qc_identity);
    reveal(production_enter_view_selected_lock);
    reveal(production_enter_view_has_exact_following_fetch);
}

/// An accepted active persisted-TC transition exposes the complete exact
/// `EnterView` fact consumed by the effective-lock production refinement.
pub proof fn accepted_core_enter_view_has_exact_fact(
    projection: ProductionTransitionProjection,
)
    requires
        production_kernel_relation(projection),
        projection.enter_view.active,
    ensures
        production_enter_view_exact_fact(projection),
{
    reveal(production_kernel_relation);
    reveal(production_transition_action_relation);
    reveal(production_facts_from_projection);
    reveal(production_enter_view_exact_fact);
}

/// Exact executable kernel called conceptually by `refinement::accepts`:
/// derive facts from concrete primitives first, then evaluate the established
/// transition gate.  No authorization or action-exactness boolean is an input.
pub fn verified_production_kernel(
    projection: ProductionTransitionProjection,
) -> (accepted: bool)
    ensures
        accepted ==> production_kernel_relation(projection),
{
    let facts = verified_facts_from_projection(projection);
    let accepted = verified_production_transition_gate(facts);
    proof {
        reveal(production_kernel_relation);
    }
    accepted
}

/// A nonzero invariant-violation counter cannot be hidden by the fact
/// derivation and therefore cannot pass the production kernel.
pub proof fn production_kernel_rejects_invalid_before_state(
    projection: ProductionTransitionProjection,
)
    requires
        projection.safety_before.durable_identity_mismatches > 0
            || projection.safety_before.asynchronous_fence_conflicts > 0
            || projection.safety_before.invalid_highest_prepare > 0
            || projection.safety_before.invalid_lock > 0
            || projection.safety_before.invalid_timeout > 0
            || projection.safety_before.invalid_decision > 0
            || projection.safety_before.invalid_pending_append > 0
            || projection.safety_before.unauthorized_signables > 0
            || projection.safety_before.invalid_application > 0,
    ensures
        !production_kernel_relation(projection),
{
    reveal(production_kernel_relation);
    reveal(production_transition_action_relation);
}

/// A counterfeit grant whose primitive key differs from the requested key in
/// any active slot cannot authorize the effect vector.
pub proof fn production_effect_slot_rejects_counterfeit_grant(
    slot: ProductionEffectSlotProjection,
)
    requires
        1 <= slot.kind <= 9,
        slot.requested.kind == slot.kind,
        slot.granted.kind == slot.kind,
        !capability_key_equal_body!(slot.requested, slot.granted),
    ensures
        !production_effect_slot_authorized(slot),
{
}

/// Acceptance refines to the production action relation and exposes the
/// durable invariants and complete per-slot effect authorization on which the
/// reducer's commit depends.
pub proof fn accepted_production_transition_refines_action(
    facts: ProductionTransitionFactsProjection,
)
    requires
        production_transition_action_relation(facts),
    ensures
        facts.before_invariant,
        facts.after_invariant,
        facts.context_unchanged,
        production_volatile_summary_well_formed(
            facts.volatile_before,
            facts.validator_count,
        ),
        production_volatile_summary_well_formed(
            facts.volatile_after,
            facts.validator_count,
        ),
        production_named_action_relation(facts),
        facts.enter_view_exact,
        production_effect_trace_relation(facts.effects, facts.event_kind),
        production_transition_branch_relation(facts),
        (!facts.tag_matches || !facts.busy_fence_open)
            ==> facts.whole_state_unchanged,
        facts.effects.len <= 8,
        facts.effects.len > 0 ==> production_effect_slot_authorized(facts.effects.slot0),
        facts.effects.len > 1 ==> production_effect_slot_authorized(facts.effects.slot1),
        facts.effects.len > 2 ==> production_effect_slot_authorized(facts.effects.slot2),
        facts.effects.len > 3 ==> production_effect_slot_authorized(facts.effects.slot3),
        facts.effects.len > 4 ==> production_effect_slot_authorized(facts.effects.slot4),
        facts.effects.len > 5 ==> production_effect_slot_authorized(facts.effects.slot5),
        facts.effects.len > 6 ==> production_effect_slot_authorized(facts.effects.slot6),
        facts.effects.len > 7 ==> production_effect_slot_authorized(facts.effects.slot7),
        effect_count_body!(facts.effects, 1u8) > 0
            ==> effect_count_body!(facts.effects, 5u8) == 0
                && effect_count_body!(facts.effects, 7u8) == 0
                && effect_count_body!(facts.effects, 8u8) == 0,
        effect_count_body!(facts.effects, 5u8) > 0
            ==> match facts.effects.len {
                1 => facts.effects.slot0.kind == 5,
                2 => facts.effects.slot1.kind == 5,
                3 => facts.effects.slot2.kind == 5,
                4 => facts.effects.slot3.kind == 5,
                5 => facts.effects.slot4.kind == 5,
                6 => facts.effects.slot5.kind == 5,
                7 => facts.effects.slot6.kind == 5,
                8 => facts.effects.slot7.kind == 5,
                _ => false,
            },
        effect_count_body!(facts.effects, 8u8) > 0
            ==> facts.acknowledge_persist_exact
                && facts.acknowledgement_continuation == 2,
{
    reveal(production_transition_action_relation);
}

} // verus!
