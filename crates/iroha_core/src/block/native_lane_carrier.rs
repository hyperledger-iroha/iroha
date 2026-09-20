//! Complete supported Native input shape retained by live and finalized source owners.
//!
//! This is a pure borrowed projection, not a source/finality/acceptance token.
//! Network, Pipeline and Time outputs share the common actual producer.
//! QueuePlan and consensus effects require recorded execution with verified context.
//! TODO: compose DA/pin/SCCP controls and full State publication before removing
//! the production Native inactive gate.

use iroha_crypto::HashOf;
use iroha_data_model::block::{SignedBlock, lane_decision_batch::LaneDecisionBatchV1};

/// Borrow Native inputs whose independent controls have concrete execution owners.
/// Native/global signatures, canonical inclusion, execution and publication are
/// independent boundaries. Historical callers must authenticate inclusion first.
pub(crate) fn native_lane_batch_for_execution(
    carrier: &SignedBlock,
) -> Result<&LaneDecisionBatchV1, String> {
    let header = carrier.header();
    let bundle = carrier
        .execution_context()
        .ok_or_else(|| "carrier has no native execution context".to_owned())?;
    if header.execution_context_hash() != Some(HashOf::new(bundle))
        || !carrier.external_entrypoints_slice().is_empty()
        || header.merkle_root().is_some()
    {
        return Err("native carrier has mixed or unbound economic inputs".into());
    }
    bundle.validate_native_lane_decisions_shape()?;
    let batch = bundle
        .native_lane_decisions
        .as_deref()
        .ok_or_else(|| "carrier contains no native economic batch".to_owned())?;
    // The mandatory proof-policy snapshot describes read-only DA policy, not
    // additional State-changing work. Bind its bytes here; the shared replay
    // kernel checks the exact active policy at this height against pre-State.
    if header.da_proof_policies_hash() != carrier.da_proof_policies().map(HashOf::new) {
        return Err("native carrier has an unbound DA proof-policy snapshot".into());
    }
    if carrier.da_commitments().is_some()
        || carrier.da_pin_intents().is_some()
        || header.da_commitments_hash().is_some()
        || header.da_pin_intents_hash().is_some()
        || header.sccp_commitment_root().is_some()
    {
        return Err(
            "native execution does not support additional carrier controls (DA, pin or SCCP)"
                .into(),
        );
    }
    if header.npos_effects_hash() != carrier.npos_consensus_effects().map(HashOf::new) {
        return Err("native carrier has unbound NPoS controls".into());
    }
    if batch.base_state_height.checked_add(1) != Some(header.height().get())
        || header.prev_block_hash().is_none()
        || header.creation_time().is_zero()
    {
        return Err("native batch is bound to another applying carrier height".into());
    }
    Ok(batch)
}

/// Scratch has no authenticated control/context consumer. Refuse the complete
/// carrier before State acquisition instead of silently dropping its controls.
pub(crate) fn native_lane_batch_for_scratch(
    carrier: &SignedBlock,
) -> Result<&LaneDecisionBatchV1, String> {
    let batch = native_lane_batch_for_execution(carrier)?;
    if !carrier
        .execution_context()
        .expect("checked Native shape")
        .queue_plan_admissions
        .is_empty()
        || carrier.npos_consensus_effects().is_some()
        || carrier.header().npos_effects_hash().is_some()
    {
        return Err("native scratch replay does not support additional carrier controls".into());
    }
    Ok(batch)
}
