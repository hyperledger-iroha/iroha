//! Whether the lifecycle successor's V2 terminal envelope covers the committed output policy.
//!
//! The lifecycle successor emits larger V2 terminal rows. Before a node schedules, preflights or
//! activates it, the committed `ExecutionOutputPolicyV1` and `FastpqSourcePolicyV1` must admit
//! every candidate the unchanged source profile allows. Both genesis profiles are immutable, so
//! checking the maximum allowed growth keeps the answer valid until activation. This is a local
//! support check, never a ledger rejection or wire field.

use iroha_data_model::{
    block::output_budget::ExecutionOutputTerminalCeilings,
    parameter::{ExecutionOutputPolicyV1, FastpqSourcePolicyV1},
};

/// Apply the output support check to the exact compiled selected service semantics.
/// Baseline and Ironwood-only selections keep their predecessor output policy.
/// # Errors
/// An unsupported service profile/closure or insufficient lifecycle terminal capacity.
pub(crate) fn selected_output_support(
    selection: &iroha_data_model::runtime::definition::RootSelectionV1,
    policy: ExecutionOutputPolicyV1,
    source: FastpqSourcePolicyV1,
) -> Result<(), String> {
    if super::trigger_lifecycle_enabled(selection).map_err(|error| error.to_string())? {
        lifecycle_output_support(policy, source)?;
    }
    Ok(())
}

/// Check that the lifecycle successor's terminal capacity covers the source candidate capacity.
///
/// # Errors
/// A diagnostic when either policy is invalid or the successor's capacity is smaller.
pub(crate) fn lifecycle_output_support(
    policy: ExecutionOutputPolicyV1,
    source: FastpqSourcePolicyV1,
) -> Result<(), String> {
    policy.validate()?;
    source.validate(policy)?;
    let candidate_capacity = source.maximum_network_inputs(policy)?;
    let capacity = ExecutionOutputTerminalCeilings::derive_for_lifecycle(true)?
        .envelope(policy.max_pipeline_triggers, policy.max_time_invocations)
        .maximum_network_inputs(&policy.limits())?;
    if capacity < candidate_capacity {
        return Err(format!(
            "committed lifecycle terminal capacity {capacity} does not cover source candidate capacity {candidate_capacity}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{
        block::output_budget::{ExecutionOutputPhase as Phase, ExecutionOutputTerminalCeilings},
        parameter::{ExecutionOutputPolicyV1, FastpqSourcePolicyV1},
    };

    use super::lifecycle_output_support;

    #[test]
    fn refuses_v1_only_row_or_aggregate_capacity() {
        let old = ExecutionOutputTerminalCeilings::derive().unwrap();
        let new = ExecutionOutputTerminalCeilings::derive_for_lifecycle(true).unwrap();
        let old_max = old
            .for_phase(Phase::Network)
            .max(old.for_phase(Phase::Time));
        let new_max = new
            .for_phase(Phase::Network)
            .max(new.for_phase(Phase::Time));
        assert!(new_max > old_max);
        let old_sum = old.for_phase(Phase::Network) + old.for_phase(Phase::Time);
        for row_deficit in [false, true] {
            let mut policy = ExecutionOutputPolicyV1::bootstrap();
            policy.max_pipeline_triggers = 0;
            policy.max_time_triggers = 1;
            policy.max_time_invocations = 1;
            policy.max_outputs = 2;
            if row_deficit {
                policy.max_output_bytes = old_max;
            } else {
                policy.max_output_bytes = new_max;
                policy.max_total_output_bytes = old_sum.max(new_max);
            }
            policy.validate().unwrap();
            assert!(lifecycle_output_support(policy, FastpqSourcePolicyV1::bootstrap()).is_err());
        }
    }

    #[test]
    fn checks_maximum_growth_and_accepts_exact_successor_capacity() {
        let source = FastpqSourcePolicyV1::bootstrap();
        let old = ExecutionOutputTerminalCeilings::derive().unwrap();
        let new = ExecutionOutputTerminalCeilings::derive_for_lifecycle(true).unwrap();
        let mut policy = ExecutionOutputPolicyV1::bootstrap();
        policy.max_pipeline_triggers = 2;
        policy.max_time_triggers = 4;
        policy.max_time_invocations = 4;
        policy.max_outputs = 9; // One Network, two Pipeline events of two, four Time.
        policy.max_output_bytes = [Phase::Network, Phase::Pipeline, Phase::Time]
            .into_iter()
            .map(|phase| new.for_phase(phase))
            .max()
            .unwrap();
        let plan_bytes = |bounds: ExecutionOutputTerminalCeilings| {
            bounds.for_phase(Phase::Network)
                + 4 * bounds.for_phase(Phase::Pipeline)
                + 4 * bounds.for_phase(Phase::Time)
        };
        policy.max_total_output_bytes = plan_bytes(old);
        policy.validate().unwrap();
        assert!(lifecycle_output_support(policy, source).is_err());
        policy.max_total_output_bytes = plan_bytes(new);
        lifecycle_output_support(policy, source).unwrap();
        lifecycle_output_support(ExecutionOutputPolicyV1::bootstrap(), source).unwrap();
    }

    #[test]
    fn covers_the_unchanged_candidate_cap() {
        let old = ExecutionOutputTerminalCeilings::derive().unwrap();
        let new = ExecutionOutputTerminalCeilings::derive_for_lifecycle(true).unwrap();
        let mut policy = ExecutionOutputPolicyV1::bootstrap();
        policy.max_pipeline_triggers = 0;
        policy.max_time_triggers = 1;
        policy.max_time_invocations = 1;
        policy.max_outputs = 3;
        policy.max_output_bytes = new
            .for_phase(Phase::Network)
            .max(new.for_phase(Phase::Time));
        policy.max_total_output_bytes = new.for_phase(Phase::Network) + new.for_phase(Phase::Time);
        assert!(
            policy.max_total_output_bytes
                >= 2 * old.for_phase(Phase::Network) + old.for_phase(Phase::Time)
        );
        policy.validate().unwrap();
        assert_eq!(policy.maximum_terminal_network_inputs().unwrap(), 2);
        assert_eq!(
            new.envelope(0, 1)
                .maximum_network_inputs(&policy.limits())
                .unwrap(),
            1,
            "one feasible successor root is insufficient if the source admits two"
        );
        let bootstrap_source = FastpqSourcePolicyV1::bootstrap();
        let source_for = |count| {
            FastpqSourcePolicyV1::from_sizing(
                policy,
                bootstrap_source.intrinsic,
                bootstrap_source.mandatory,
                count,
            )
            .unwrap()
        };
        assert!(lifecycle_output_support(policy, source_for(2)).is_err());
        lifecycle_output_support(policy, source_for(1)).unwrap();
    }
}
