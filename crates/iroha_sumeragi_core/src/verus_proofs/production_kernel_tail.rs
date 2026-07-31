// Production-kernel gate and refinement proof tail.
//
// Included lexically by `verus_proofs` so public item paths remain unchanged.

verus! {

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
