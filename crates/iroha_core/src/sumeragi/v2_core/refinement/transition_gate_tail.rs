fn volatile_summary_is_well_formed(summary: VolatileSummary, validator_count: u64) -> bool {
    volatile_summary_well_formed_body!(summary, validator_count)
}
fn signed_message_class_is_valid(facts: TransitionFacts) -> bool {
    signed_message_class_body!(facts)
}
fn stutter_action_is_valid(facts: TransitionFacts) -> bool {
    stutter_action_body!(facts)
}
fn begin_wal_action_is_valid(facts: TransitionFacts) -> bool {
    begin_wal_action_body!(facts)
}
fn acknowledge_wal_action_is_valid(facts: TransitionFacts) -> bool {
    acknowledge_wal_action_body!(facts)
}
fn validation_completed_action_is_valid(facts: TransitionFacts) -> bool {
    validation_completed_action_body!(facts, effect_count)
}
fn body_progress_action_is_valid(facts: TransitionFacts) -> bool {
    body_progress_action_body!(facts, validation_completed_action_is_valid)
}
fn volatile_protocol_action_is_valid(facts: TransitionFacts) -> bool {
    volatile_protocol_action_body!(facts)
}
fn complete_application_action_is_valid(facts: TransitionFacts) -> bool {
    complete_application_action_body!(facts)
}
fn resume_after_replay_action_is_valid(facts: TransitionFacts) -> bool {
    resume_after_replay_action_body!(facts, effect_count)
}
fn action_kind_is_valid(facts: TransitionFacts) -> bool {
    action_kind_relation_body!(
        facts,
        stutter_action_is_valid,
        begin_wal_action_is_valid,
        acknowledge_wal_action_is_valid,
        body_progress_action_is_valid,
        volatile_protocol_action_is_valid,
        complete_application_action_is_valid,
        resume_after_replay_action_is_valid,
    )
}
fn named_action_is_valid(facts: TransitionFacts) -> bool {
    production_action_relation_body!(facts, signed_message_class_is_valid, action_kind_is_valid,)
}
fn effect_slots_are_authorized(trace: EffectTrace) -> bool {
    effect_slots_authorized_body!(trace)
}
fn effect_count(trace: EffectTrace, kind: u8) -> u64 {
    effect_count_body!(trace, kind)
}
#[allow(clippy::too_many_arguments)]
fn effect_order_constraints(
    trace: EffectTrace,
    event_kind: u8,
    persist_count: u64,
    fetch_count: u64,
    store_count: u64,
    validate_count: u64,
    sign_count: u64,
    apply_count: u64,
    enter_count: u64,
) -> bool {
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
fn effect_order_is_valid(trace: EffectTrace, event_kind: u8) -> bool {
    effect_ordering_gate_body!(trace, event_kind, effect_count, effect_order_constraints)
}
fn effect_trace_accepts(trace: EffectTrace, event_kind: u8) -> bool {
    effect_trace_gate_body!(
        trace,
        event_kind,
        effect_slots_are_authorized,
        effect_order_is_valid,
    )
}
#[allow(clippy::too_many_arguments)]
fn transition_branch_constraints(
    facts: TransitionFacts,
    persist_count: u64,
    fetch_count: u64,
    sign_count: u64,
    apply_count: u64,
    enter_count: u64,
) -> bool {
    transition_branch_constraints_body!(
        facts,
        persist_count,
        fetch_count,
        sign_count,
        apply_count,
        enter_count,
    )
}
fn transition_branch_accepts(facts: TransitionFacts) -> bool {
    transition_branch_gate_body!(facts, effect_count, transition_branch_constraints)
}
fn accepts_facts(facts: TransitionFacts) -> bool {
    production_transition_gate_body!(
        facts,
        volatile_summary_is_well_formed,
        named_action_is_valid,
        effect_trace_accepts,
        transition_branch_accepts,
    )
}
/// Derive the complete checked relation from concrete primitive projections.
fn transition_facts(projection: TransitionProjection<'_>) -> TransitionFacts {
    transition_facts_from_projection_body!(
        projection,
        TransitionFacts,
        TransitionClassificationFacts,
        TransitionDeltaFacts,
    )
}
/// Predicate-level evidence for a transition rejected by [`accepts`].
///
/// This is diagnostic-only: every field is derived from the same primitive
/// projection and production predicates as the commit gate. It neither grants
/// capabilities nor participates in acceptance.
#[derive(Clone, Copy)]
pub(crate) struct TransitionDiagnostic {
    facts: TransitionFacts,
    safety_before: SafetyProjection,
    safety_after: SafetyProjection,
    volatile_before_well_formed: bool,
    volatile_after_well_formed: bool,
    signed_message_class_valid: bool,
    selected_action_valid: bool,
    effect_slots_authorized: bool,
    effect_order_valid: bool,
    transition_branch_valid: bool,
    enter_view_effective_lock_valid: bool,
}
impl fmt::Debug for TransitionDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransitionDiagnostic")
            .field("facts", &self.facts)
            .field("safety_before", &self.safety_before)
            .field("safety_after", &self.safety_after)
            .field(
                "volatile_before_well_formed",
                &self.volatile_before_well_formed,
            )
            .field(
                "volatile_after_well_formed",
                &self.volatile_after_well_formed,
            )
            .field(
                "signed_message_class_valid",
                &self.signed_message_class_valid,
            )
            .field("selected_action_valid", &self.selected_action_valid)
            .field("effect_slots_authorized", &self.effect_slots_authorized)
            .field("effect_order_valid", &self.effect_order_valid)
            .field("transition_branch_valid", &self.transition_branch_valid)
            .field(
                "enter_view_effective_lock_valid",
                &self.enter_view_effective_lock_valid,
            )
            .finish()
    }
}
/// Derive predicate-level diagnostics without weakening the production gate.
#[must_use]
pub(crate) fn diagnose(projection: TransitionProjection<'_>) -> TransitionDiagnostic {
    let facts = transition_facts(projection);
    let enter_view_effective_lock_valid = if projection.enter_view.active {
        let enter_view = projection.enter_view;
        let protected_after = u64::from(enter_view.durable_lock_after.present);
        let ownership_after = u64::from(enter_view.following_fetch_lock.present);
        let trace = EffectiveLockTraceProjection {
            kind: EFFECTIVE_LOCK_TRACE_ENTER_VIEW,
            relation_exact: enter_view_projection_gate_body!(enter_view)
                && enter_view.enter_count == effect_count_body!(projection.effects, 8u8)
                && enter_view.fetch_count == effect_count_body!(projection.effects, 2u8),
            protected_before: protected_after,
            protected_after: u64::from(enter_view.effect_protected_lock.present),
            owner_before: enter_view.fetch_count,
            owner_after: ownership_after,
            owner_reused: false,
            ready_before: 0,
            retired_retained: 0,
            retired_ready: 0,
            ready_after: 0,
            store_before: 0,
            retired_store: 0,
            store_after: 0,
            cursor_before: 0,
            completion_ready: false,
            progress_ready: false,
            normal_ready: false,
            selected: 0,
            cursor_after: 0,
        };
        production_enter_view_uses_post_install_effective_lock_kernel(trace, enter_view)
    } else {
        true
    };
    TransitionDiagnostic {
        facts,
        safety_before: projection.safety_before,
        safety_after: projection.safety_after,
        volatile_before_well_formed: volatile_summary_is_well_formed(
            facts.volatile_before,
            facts.validator_count,
        ),
        volatile_after_well_formed: volatile_summary_is_well_formed(
            facts.volatile_after,
            facts.validator_count,
        ),
        signed_message_class_valid: signed_message_class_is_valid(facts),
        selected_action_valid: action_kind_is_valid(facts),
        effect_slots_authorized: effect_slots_are_authorized(facts.effects),
        effect_order_valid: effect_order_is_valid(facts.effects, facts.event_kind),
        transition_branch_valid: transition_branch_accepts(facts),
        enter_view_effective_lock_valid,
    }
}
/// Execute the verified transition kernel used as the production commit gate.
///
/// No caller-provided authorization or action-exactness boolean crosses this
/// boundary.  The kernel derives them from requested/granted capability keys,
/// exact pre/post state identities, event tags, and invariant violation counts.
#[must_use = "checked reducer evidence must be consumed before installing candidate state"]
pub(crate) fn check(
    projection: TransitionProjection<'_>,
) -> Option<CheckedProductionTransition<TransitionProjection<'_>>> {
    if projection.enter_view.active {
        let enter_view = projection.enter_view;
        let protected_after = u64::from(enter_view.durable_lock_after.present);
        let ownership_after = u64::from(enter_view.following_fetch_lock.present);
        let trace = EffectiveLockTraceProjection {
            kind: EFFECTIVE_LOCK_TRACE_ENTER_VIEW,
            relation_exact: enter_view_projection_gate_body!(enter_view)
                && enter_view.enter_count == effect_count_body!(projection.effects, 8u8)
                && enter_view.fetch_count == effect_count_body!(projection.effects, 2u8),
            protected_before: protected_after,
            protected_after: u64::from(enter_view.effect_protected_lock.present),
            owner_before: enter_view.fetch_count,
            owner_after: ownership_after,
            owner_reused: false,
            ready_before: 0,
            retired_retained: 0,
            retired_ready: 0,
            ready_after: 0,
            store_before: 0,
            retired_store: 0,
            store_after: 0,
            cursor_before: 0,
            completion_ready: false,
            progress_ready: false,
            normal_ready: false,
            selected: 0,
            cursor_after: 0,
        };
        // The inner evidence establishes the Verus-checked effective-lock
        // claim. Consuming it here is safe because the returned outer token
        // retains the complete borrowed transition and is the only evidence
        // production can consume at the reducer's state-install boundary.
        let checked_effective_lock =
            check_production_enter_view_effective_lock_transition(trace, enter_view)?;
        let _authorized_effective_lock = checked_effective_lock.into_projection();
    }
    if accepts_facts(transition_facts(projection)) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Report whether the complete production transition relation accepts.
///
/// Production uses [`check`] and consumes its opaque evidence at state
/// installation. This boolean facade remains for pure diagnostics and
/// mutation-focused unit tests which do not cross a mutation boundary.
#[must_use]
pub fn accepts(projection: TransitionProjection<'_>) -> bool {
    check(projection).is_some()
}
