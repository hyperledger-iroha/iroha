//! Original State and runtime cells outside the centralized World journal.

use super::*;
use crate::state::SnapshotNexusRuntime;

pub(super) fn append(block: &StateBlock<'_>, output: &mut Collector) -> Result<(), CaptureError> {
    let fields = block.fields.as_ref().ok_or(CaptureError::NotFrozen)?;
    append_fields(fields, output)
}

pub(super) fn append_fields(
    fields: &crate::state::StateBlockFields<'_>,
    output: &mut Collector,
) -> Result<(), CaptureError> {
    let expected = fields.state_ref;
    let mode = fields.world.parameters.mode();
    output.cell(
        "state.commit_topology",
        &fields.commit_topology,
        &expected.commit_topology,
        mode,
        bare,
    )?;
    output.cell(
        "state.prev_commit_topology",
        &fields.prev_commit_topology,
        &expected.prev_commit_topology,
        mode,
        bare,
    )?;
    let source = || SourceIdentity::Policy(std::ptr::from_ref(fields) as usize);
    output.projected_cell(
        "state.chain_id",
        source(),
        false,
        |writer| bare(&fields.chain_id, writer),
        |writer| bare(&fields.chain_id, writer),
    )?;
    output.projected_cell(
        "state.network_id",
        source(),
        false,
        |writer| bare(&fields.network_id, writer),
        |writer| bare(&fields.network_id, writer),
    )?;
    let original = &fields.canonical_runtime;
    let (current, before) = output.cell_values(
        "state.canonical_runtime",
        original,
        &expected.canonical_runtime,
        mode,
    )?;
    // Exhaustive destructuring prevents silent runtime schema growth. The semantic
    // lane vector is encoded by its existing policy owner in policies::append.
    let SnapshotNexusRuntime {
        version: _,
        owner_policy: _,
        lane_count: _,
        lanes: _,
        lane_incarnation_lineage: _,
        autoscale_last_transition_height: _,
        autoscale_scale_out_window_blocks: _,
        autoscale_scale_in_window_blocks: _,
        autoscale_sample_history_cap: _,
        autoscale_sample_history: _,
    } = current;
    macro_rules! cell {
        ($field:ident) => {
            output.projected_cell(
                concat!("runtime.", stringify!($field)),
                SourceIdentity::Journal(original.publication_identity()),
                original.touched_value().is_some(),
                |writer| bare(&current.$field, writer),
                |writer| bare(&before.$field, writer),
            )?;
        };
    }
    cell!(version);
    cell!(owner_policy);
    cell!(lane_count);
    cell!(lane_incarnation_lineage);
    cell!(autoscale_last_transition_height);
    cell!(autoscale_scale_out_window_blocks);
    cell!(autoscale_scale_in_window_blocks);
    cell!(autoscale_sample_history_cap);
    cell!(autoscale_sample_history);
    Ok(())
}
