//! Complete semantic policies from the validated original frozen publication.
//!
//! Runtime and parameters retain their own current/undo identities. The captured
//! execution settings describe the predecessor block; committed mutable policy
//! is always selected from the exact parameter/runtime version being encoded.

use super::{CaptureError, Collector, SourceIdentity, bare};
use crate::state::{
    StateBlockFields,
    authority_registry::{
        content_policy, governance_policy, nexus_policy, oracle_policy, pipeline_policy,
        policy_borrowed::OrderError, runtime_lane_policy, settlement_policy, zk_policy,
    },
};

fn order(error: OrderError) -> CaptureError {
    match error {
        OrderError::Allocation(error) => CaptureError::Allocation(error),
        OrderError::SourceCount => CaptureError::Changed("policy ordering source count"),
        OrderError::Codec(error) => CaptureError::Codec(error),
    }
}

pub(super) fn append(
    fields: &StateBlockFields<'_>,
    output: &mut Collector,
) -> Result<(), CaptureError> {
    let runtime = &fields.canonical_runtime;
    let parameters = &fields.world.parameters;
    let mode = parameters.mode();
    let (current_runtime, before_runtime) = output.cell_values(
        "state.canonical_runtime",
        runtime,
        &fields.state_ref.canonical_runtime,
        mode,
    )?;
    let (current_parameters, before_parameters) = output.cell_values(
        "world.parameters",
        parameters,
        &fields.state_ref.world.parameters,
        mode,
    )?;
    if current_parameters.root_fees().is_none()
        || (fields._curr_block.height().get() > 1 && before_parameters.root_fees().is_none())
    {
        return Err(CaptureError::Changed(
            "committed root fee policy is missing",
        ));
    }
    if current_parameters.crypto_policy().is_none()
        || (fields._curr_block.height().get() > 1 && before_parameters.crypto_policy().is_none())
    {
        return Err(CaptureError::Changed(
            "committed crypto admission policy is missing",
        ));
    }
    // world::append streams the exact current/undo Parameters owner, including
    // its committed crypto policy. Local implementation settings in the immutable
    // execution snapshot are not a second canonical policy cell.
    // OriginalCut retains StatePublication's validated frozen phase. Its fields
    // cannot be mutated, rebound or replaced while this same cut is alive.
    let source = || SourceIdentity::Policy(std::ptr::from_ref(fields) as usize);
    let parameters_touched = parameters.touched_value().is_some();
    let runtime_touched = runtime.touched_value().is_some();
    macro_rules! immutable {
        ($id:literal, $value:expr) => {
            if !output.skip_cell_projection($id, source(), false)? {
                let value = $value;
                output.projected_cell(
                    $id,
                    source(),
                    false,
                    |writer| bare(&value, writer),
                    |writer| bare(&value, writer),
                )?;
            }
        };
    }
    immutable!("state.oracle", oracle_policy::borrowed(&fields.oracle));
    immutable!("state.gov", governance_policy::borrowed(&fields.gov));
    // The router inputs and the content limits are committed policy of a root that
    // recognizes them (`runtime::committed_policy`): each side encodes the effective value
    // of its own parameters and selection versions over the installed constants, so a root
    // that recognizes neither keeps the constant bytes.
    let selection = &fields.world.runtime_selection;
    let (current_selection, before_selection) = output.cell_values(
        "world.runtime_selection",
        selection,
        &fields.state_ref.world.runtime_selection,
        selection.mode(),
    )?;
    let committed_touched = parameters_touched || selection.touched_value().is_some();
    if !output.skip_cell_projection("state.settlement", source(), committed_touched)? {
        let current = crate::runtime::committed_policy::effective_settlement(
            &fields.state_ref.settlement,
            current_selection,
            current_parameters,
        );
        let before = crate::runtime::committed_policy::effective_settlement(
            &fields.state_ref.settlement,
            before_selection,
            before_parameters,
        );
        let (current, before) = (
            settlement_policy::borrowed(&current),
            settlement_policy::borrowed(&before),
        );
        output.projected_cell(
            "state.settlement",
            source(),
            committed_touched,
            |writer| bare(&current, writer),
            |writer| bare(&before, writer),
        )?;
    }
    if !output.skip_cell_projection("state.content", source(), committed_touched)? {
        let current = crate::runtime::committed_policy::effective_content(
            &fields.state_ref.content,
            current_selection,
            current_parameters,
        );
        let before = crate::runtime::committed_policy::effective_content(
            &fields.state_ref.content,
            before_selection,
            before_parameters,
        );
        let (current, before) = (
            content_policy::borrowed(&current),
            content_policy::borrowed(&before),
        );
        output.projected_cell(
            "state.content",
            source(),
            committed_touched,
            |writer| bare(&current, writer),
            |writer| bare(&before, writer),
        )?;
    }

    if !output.skip_cell_projection("state.pipeline", source(), parameters_touched)? {
        let current = pipeline_policy::BorrowedPipelinePolicy::new(
            current_parameters.pipeline_policy(),
            current_parameters.gas(),
            output.budget(),
        )
        .map_err(order)?;
        let before = pipeline_policy::BorrowedPipelinePolicy::new(
            before_parameters.pipeline_policy(),
            before_parameters.gas(),
            output.budget(),
        )
        .map_err(order)?;
        output.projected_cell(
            "state.pipeline",
            source(),
            parameters_touched,
            |writer| bare(&current, writer),
            |writer| bare(&before, writer),
        )?;
    }
    if !output.skip_cell_projection("state.zk", source(), parameters_touched)? {
        let current = zk_policy::borrowed(current_parameters.zk_policy());
        let before = zk_policy::borrowed(before_parameters.zk_policy());
        output.projected_cell(
            "state.zk",
            source(),
            parameters_touched,
            |writer| bare(&current, writer),
            |writer| bare(&before, writer),
        )?;
    }
    if !output.skip_cell_projection(
        "runtime.lanes",
        SourceIdentity::Journal(runtime.publication_identity()),
        runtime_touched,
    )? {
        let current = runtime_lane_policy::borrowed(&current_runtime.lanes)
            .map_err(|error| CaptureError::Source(error.to_string()))?;
        let before = runtime_lane_policy::borrowed(&before_runtime.lanes)
            .map_err(|error| CaptureError::Source(error.to_string()))?;
        output.projected_cell(
            "runtime.lanes",
            SourceIdentity::Journal(runtime.publication_identity()),
            runtime_touched,
            |writer| bare(&current, writer),
            |writer| bare(&before, writer),
        )?;
    }
    let current_compliance = fields.lane_compliance.as_deref();
    let before_compliance = fields.runtime_policy.compliance.as_deref();
    let compliance_touched = match (&fields.lane_compliance, &fields.runtime_policy.compliance) {
        (Some(current), Some(before)) => !std::sync::Arc::ptr_eq(current, before),
        (None, None) => false,
        _ => true,
    };
    if !output.skip_cell_projection("state.lane_compliance", source(), compliance_touched)? {
        let current = crate::compliance::authority::BorrowedCompliance(current_compliance);
        let before = crate::compliance::authority::BorrowedCompliance(before_compliance);
        output.projected_cell(
            "state.lane_compliance",
            source(),
            compliance_touched,
            |writer| bare(&current, writer),
            |writer| bare(&before, writer),
        )?;
    }
    let current_manifests = fields.lane_manifests.as_ref();
    let before_manifests = fields.runtime_policy.manifests.as_ref();
    let manifests_touched =
        !std::sync::Arc::ptr_eq(&fields.lane_manifests, &fields.runtime_policy.manifests);
    // Materialized manifest validation and bytes also depend on the exact lane
    // catalog. A runtime touch, even an equal one, must retain that preimage and
    // cannot skip the original catalog binding check.
    let manifest_projection_touched = manifests_touched
        || (runtime_touched && !cfg!(all(test, sumeragi_core_mutation = "HC457")));
    if !output.skip_cell_projection(
        "state.lane_manifests",
        source(),
        manifest_projection_touched,
    )? {
        let current = current_manifests
            .borrowed_materialized_for_frozen_catalog(
                current_runtime.lane_count,
                &current_runtime.lanes,
            )
            .map_err(CaptureError::Codec)?;
        let before = before_manifests
            .borrowed_materialized_for_frozen_catalog(
                before_runtime.lane_count,
                &before_runtime.lanes,
            )
            .map_err(CaptureError::Codec)?;
        output.projected_cell(
            "state.lane_manifests",
            source(),
            manifest_projection_touched,
            |writer| bare(&current, writer),
            |writer| bare(&before, writer),
        )?;
    }
    let nexus_touched =
        parameters_touched || runtime_touched || compliance_touched || manifests_touched;
    if !output.skip_cell_projection("state.nexus", source(), nexus_touched)? {
        let current_digest = current_compliance
            .map(crate::compliance::authority::borrowed_digest)
            .transpose()
            .map_err(CaptureError::Codec)?;
        let before_digest = before_compliance
            .map(crate::compliance::authority::borrowed_digest)
            .transpose()
            .map_err(CaptureError::Codec)?;
        let current = nexus_policy::BorrowedNexusPolicy::new(
            &fields.runtime_policy.nexus,
            current_runtime,
            current_parameters.root_fees(),
            current_digest,
            Some(current_manifests.baseline_consensus_policy_digest()),
        )
        .map_err(CaptureError::Codec)?;
        let before = nexus_policy::BorrowedNexusPolicy::new(
            &fields.runtime_policy.nexus,
            before_runtime,
            before_parameters.root_fees(),
            before_digest,
            Some(before_manifests.baseline_consensus_policy_digest()),
        )
        .map_err(CaptureError::Codec)?;
        output.projected_cell(
            "state.nexus",
            source(),
            nexus_touched,
            |writer| bare(&current, writer),
            |writer| bare(&before, writer),
        )?;
    }
    Ok(())
}
