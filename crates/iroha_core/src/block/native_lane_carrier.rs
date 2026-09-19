//! One supported native carrier shape shared by live and finalized scratch replay.
//!
//! This is a pure borrowed projection, not a source/finality/acceptance token.
//! Network, Pipeline and Time outputs share the common actual producer.
//! The canonical owner may compose one independently authenticated pristine
//! beacon pulse. Scratch readers reject controls they cannot replay completely.
//! TODO: activate the sole runtime native replay/Apply consumer before removing
//! the production native inactive gate.

use iroha_crypto::HashOf;
use iroha_data_model::block::{SignedBlock, lane_decision_batch::LaneDecisionBatchV1};

/// Borrow the sole native input batch only if scratch replay covers the whole input.
/// Native/global signatures, canonical inclusion, execution and publication are
/// independent boundaries. Historical callers must authenticate inclusion first.
pub(crate) fn native_lane_batch_for_scratch(
    carrier: &SignedBlock,
) -> Result<&LaneDecisionBatchV1, String> {
    let batch = native_lane_batch_for_execution(carrier)?;
    if carrier.npos_consensus_effects().is_some() || carrier.header().npos_effects_hash().is_some()
    {
        return Err("native scratch replay does not support additional carrier controls".into());
    }
    Ok(batch)
}

/// Project source-only native execution with an optional pristine beacon pulse.
/// This checks shape only; ValidBlock must authenticate and apply the pulse on
/// the exact parent before shared start hooks and native economics.
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
    if !bundle.queue_plan_admissions.is_empty()
        || carrier.da_commitments().is_some()
        || carrier.da_pin_intents().is_some()
        || header.da_commitments_hash().is_some()
        || header.da_pin_intents_hash().is_some()
        || header.sccp_commitment_root().is_some()
    {
        return Err("native scratch replay does not support additional carrier controls".into());
    }
    if header.npos_effects_hash() != carrier.npos_consensus_effects().map(HashOf::new) {
        return Err("native carrier has unbound NPoS controls".into());
    }
    if let Some(effects) = carrier.npos_consensus_effects()
        && (effects.finalized_global_beacon_pulse.is_none()
            || !effects.penalty_actions.is_empty()
            || !effects.v2_evidence_admissions.is_empty())
    {
        return Err(
            "native carrier requires one exact beacon pulse without mixed NPoS effects".into(),
        );
    }
    if batch.base_state_height.checked_add(1) != Some(header.height().get())
        || header.prev_block_hash().is_none()
        || header.creation_time().is_zero()
    {
        return Err("native batch is bound to another applying carrier height".into());
    }
    Ok(batch)
}
