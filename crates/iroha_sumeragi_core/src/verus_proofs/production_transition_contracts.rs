verus! {
/// Complete fixed-vector effect relation.
pub open spec fn production_effect_trace_relation(
    trace: ProductionEffectTraceProjection,
    event_kind: u8,
) -> bool {
    production_effect_slots_authorized(trace)
        && production_effect_order_relation(trace, event_kind)
}
/// Branch relation after the exact effect trace has passed its independent
/// authorization and ordering check.
pub open spec fn production_transition_branch_constraints(
    facts: ProductionTransitionFactsProjection,
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
/// Branch relation after the exact effect trace has passed its independent
/// authorization and ordering check.
pub open spec fn production_transition_branch_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    production_transition_branch_constraints(
        facts,
        production_effect_count(facts.effects, 1),
        production_effect_count(facts.effects, 2),
        production_effect_count(facts.effects, 5),
        production_effect_count(facts.effects, 7),
        production_effect_count(facts.effects, 8),
    )
}
pub closed spec fn production_transition_action_relation(
    facts: ProductionTransitionFactsProjection,
) -> bool {
    production_transition_gate_body!(
        facts,
        production_volatile_summary_well_formed,
        production_named_action_relation,
        production_effect_trace_relation,
        production_transition_branch_relation,
    )
}
/// Every accepted production action has an explicit TLA+ macro-step name and
/// its durable-boundary name permits exactly the projected state delta.
pub proof fn production_action_has_named_tla_mapping(
    facts: ProductionTransitionFactsProjection,
)
    requires
        production_transition_action_relation(facts),
    ensures
        production_tla_boundary_delta(
            facts,
            production_tla_macro_step(facts).boundary,
        ),
        production_tla_macro_step(facts).boundary
                == TlaActionNameProjection::NoAction
            ==> facts.action_kind == 3 || facts.action_kind == 4 || facts.action_kind == 6,
        facts.action_kind == 6 && facts.replay_effect_kind != 0
            ==> production_tla_macro_step(facts).source
                != TlaActionNameProjection::NoAction,
{
    reveal(production_transition_action_relation);
    reveal(production_named_action_relation);
    reveal(production_action_kind_relation);
    reveal(production_stutter_action_relation);
    reveal(production_begin_wal_action_relation);
    reveal(production_acknowledge_wal_action_relation);
    reveal(production_body_progress_action_relation);
    reveal(production_volatile_protocol_action_relation);
    reveal(production_complete_application_action_relation);
    reveal(production_resume_after_replay_action_relation);
    match facts.action_kind {
        0 => {},
        1 | 2 => {
            match facts.wal_record_kind {
                1 | 2 | 3 | 4 | 5 | 6 | 7 => {},
                _ => {},
            }
        },
        3 | 4 | 5 => {},
        6 => {
            match facts.replay_effect_kind {
                0 | 1 | 2 | 3 | 4 | 5 => {},
                _ => {},
            }
        },
        _ => {},
    }
}
/// The executable gate bounds the safety-relevant volatile structures on both
/// sides of every committed step and makes bounded persisted-TC retention explicit.
pub proof fn production_action_preserves_volatile_bounds(
    facts: ProductionTransitionFactsProjection,
)
    requires
        production_transition_action_relation(facts),
    ensures
        facts.volatile_after.vote_pools <= 2,
        facts.volatile_after.vote_entries <= facts.validator_count * 2,
        facts.volatile_after.timeout_vote_pools <= 2,
        facts.volatile_after.timeout_vote_entries <= facts.validator_count * 2,
        facts.volatile_after.formed_certificates <= 2,
        facts.volatile_after.formed_timeouts <= 2,
        facts.volatile_after.outbound_control <= 7,
        facts.volatile_after.pending_prepare <= facts.volatile_after.known_prepare,
        facts.volatile_after.known_prepare - facts.volatile_after.pending_prepare <= 2,
        facts.volatile_after.signature_queue
            <= facts.volatile_after.durable_signable_limit,
        facts.volatile_after.awaiting_signature
            ==> facts.volatile_after.signature_queue
                < facts.volatile_after.durable_signable_limit,
        facts.action_kind == 2 && facts.wal_record_kind == 6
            ==> !facts.volatile_after.candidate_present
                && facts.volatile_after.body_work <= 1
                && facts.volatile_after.pending_prepare == 0
                && facts.volatile_after.vote_pools == 0
                && facts.volatile_after.vote_entries == 0
                && (if facts.install_view_unchanged {
                    facts.timeout_vote_pool_unchanged
                        && facts.volatile_after.timeout_vote_pools
                            == facts.volatile_before.timeout_vote_pools
                        && facts.volatile_after.timeout_vote_entries
                            == facts.volatile_before.timeout_vote_entries
                } else {
                    facts.timeout_evidence_after_in_installed_window
                        && facts.volatile_after.timeout_vote_pools
                            <= facts.volatile_before.timeout_vote_pools
                        && facts.volatile_after.timeout_vote_entries
                            <= facts.volatile_before.timeout_vote_entries
                })
                && facts.volatile_after.formed_certificates == 0
                && (if facts.install_view_unchanged {
                    facts.formed_timeouts_unchanged
                        && facts.volatile_after.formed_timeouts
                            == facts.volatile_before.formed_timeouts
                } else {
                    facts.volatile_after.formed_timeouts
                        <= facts.volatile_before.formed_timeouts
                })
                && (if facts.install_view_unchanged {
                    facts.timeout_control_unchanged
                } else {
                    facts.timeout_control_after_absent
                })
                && facts.volatile_after.known_prepare <= 2
                && facts.volatile_after.outbound_control <= 4,
{
    reveal(production_transition_action_relation);
}
} // verus!
