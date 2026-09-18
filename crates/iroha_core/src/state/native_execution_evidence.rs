//! Bounded offline authentication of Native inputs, Decisions and complete outputs.
//!
//! The anchored global chain and exact context-set write witnesses supply historical
//! authority. This reader performs no State/Kura I/O and never exports a current-set,
//! voting, execution, durability or publication capability.

use std::{collections::BTreeMap, sync::Arc};

use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    block::{
        SignedBlock,
        consensus::ExecWitness,
        consensus_v2::{HeightContextId, finality::V2FinalityArtifact},
        lane_admission::QueuePlanAdmissionRegistryKeyV1,
        lane_consensus::QueuePlanAdmissionPriorityV1,
    },
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
};

use super::{LaneConsensusContextsV1, LaneConsensusContextsWitnessV1, VerifiedLaneContext};
use crate::{
    sumeragi::{v2_lane_payload::encode_frozen_lane_input, v2_lane_wire::LaneAuthenticator},
    torii_proxy::decode_and_validate_lane_admitted_input_v1,
};

/// Complete post-carrier context values and their fixed-key ordinary-write proof.
/// Encoding or constructing this value does not authenticate its claimed root.
#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::state::NativeLaneContextsEvidenceV1")]
pub struct NativeLaneContextsEvidenceV1 {
    contexts: LaneConsensusContextsV1,
    witness: LaneConsensusContextsWitnessV1,
}
impl NativeLaneContextsEvidenceV1 {
    /// Capture a complete context projection from the actual execution witness.
    /// The resulting bytes require independently anchored global finality.
    /// # Errors
    /// Rejects missing, duplicate or inconsistent context-set writes.
    pub fn from_execution_witness(
        contexts: iroha_data_model::block::lane_consensus::LaneConsensusContextsV1,
        witness: &ExecWitness,
        network: NetworkId,
        height: u64,
    ) -> Result<Self, String> {
        let (witness, _) = LaneConsensusContextsWitnessV1::from_witness(witness)?;
        if !witness.matches_contexts(network, height, &contexts)? {
            return Err("offline contexts differ from the actual execution witness".into());
        }
        Ok(Self { contexts, witness })
    }
}

/// Finite canonical-byte and work limits for an independently selected interval.
/// These bound retained proof/body encodings and row counts, not allocator overhead.
#[derive(Debug, Clone, Copy)]
pub struct NativeExecutionEvidenceLimits {
    /// Maximum contiguous carriers, including admission-only and empty carriers.
    pub max_carriers: u64,
    /// Maximum complete SignedBlockWire bytes per carrier.
    pub max_carrier_bytes: u64,
    /// Maximum individual finality or context-witness encoding.
    pub max_proof_bytes: u64,
    /// Maximum cumulative retained carrier, finality and context-proof encodings.
    pub max_retained_bytes: u64,
}

struct RetainedCarrier {
    block: Arc<SignedBlock>,
    finality: V2FinalityArtifact,
}

/// Exact authenticated source/output carrier for offline queries only.
#[derive(Debug)]
pub struct VerifiedNativeExecutionCarrier {
    block: Arc<SignedBlock>,
    predecessor_contexts: Vec<VerifiedLaneContext>,
}
impl VerifiedNativeExecutionCarrier {
    /// Borrow the immutable carrier whose Native group authority was verified.
    pub fn block(&self) -> &SignedBlock {
        &self.block
    }

    /// Borrow the exact historical decision context, without its private capability.
    pub fn decision_context(
        &self,
        instance: Hash,
    ) -> Option<&iroha_data_model::block::lane_consensus::FrozenLaneConsensusContextV1> {
        self.predecessor_contexts
            .iter()
            .find(|context| Hash::from(context.instance_id().0) == instance)
            .map(VerifiedLaneContext::frozen)
    }
}

/// Consuming chronological evidence owner with independently anchored finality.
/// Any error or unwind poisons the interval; partial success cannot be reused as a new run.
pub struct NativeExecutionEvidenceVerifier {
    finality: BridgeFinalityVerifier,
    network: NetworkId,
    limits: NativeExecutionEvidenceLimits,
    retained_bytes: u64,
    carriers: BTreeMap<u64, RetainedCarrier>,
    contexts: Vec<VerifiedLaneContext>,
    first_admissions:
        BTreeMap<QueuePlanAdmissionRegistryKeyV1, (Hash, QueuePlanAdmissionPriorityV1)>,
    poisoned: bool,
}
impl NativeExecutionEvidenceVerifier {
    /// Bind the reader to the caller's independently trusted first context.
    /// # Errors
    /// Rejects zero or inconsistent canonical-byte limits.
    pub fn new(
        network: NetworkId,
        first_context: HeightContextId,
        limits: NativeExecutionEvidenceLimits,
    ) -> Result<Self, String> {
        if limits.max_carriers == 0
            || limits.max_carrier_bytes == 0
            || limits.max_proof_bytes == 0
            || limits.max_carrier_bytes > limits.max_retained_bytes
            || limits.max_proof_bytes > limits.max_retained_bytes
        {
            return Err("offline Native evidence limits must be finite and ordered".into());
        }
        Ok(Self {
            finality: BridgeFinalityVerifier::with_context(network, first_context),
            network,
            limits,
            retained_bytes: 0,
            carriers: BTreeMap::new(),
            contexts: Vec::new(),
            first_admissions: BTreeMap::new(),
            poisoned: false,
        })
    }

    /// Authenticate one complete carrier, its post-context witness and Native Decisions.
    /// Every opening and first admission must occur in this exact retained interval.
    /// # Errors
    /// Rejects changed/skipped finality, omitted or false context witnesses, missing
    /// openings/sources, altered complete wire, invalid Decisions/RS16, or bounds.
    pub fn push_height(
        &mut self,
        proof: &BridgeFinalityProof,
        block: SignedBlock,
        context_evidence: &[u8],
    ) -> Result<VerifiedNativeExecutionCarrier, String> {
        if self.poisoned {
            return Err("offline Native evidence interval is poisoned".into());
        }
        self.poisoned = true;
        let result = self.push_height_inner(proof, block, context_evidence);
        if result.is_ok() {
            self.poisoned = false;
        }
        result
    }

    fn push_height_inner(
        &mut self,
        proof: &BridgeFinalityProof,
        block: SignedBlock,
        context_evidence: &[u8],
    ) -> Result<VerifiedNativeExecutionCarrier, String> {
        let body_bytes =
            u64::try_from(norito::canonical_frame_len(&block).map_err(|error| error.to_string())?)
                .ok()
                .and_then(|bytes| bytes.checked_add(1))
                .ok_or("offline carrier byte length overflow")?;
        let proof_bytes =
            u64::try_from(norito::canonical_frame_len(proof).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        let context_bytes =
            u64::try_from(context_evidence.len()).map_err(|error| error.to_string())?;
        let retained = self
            .retained_bytes
            .checked_add(body_bytes)
            .and_then(|bytes| bytes.checked_add(proof_bytes))
            .and_then(|bytes| bytes.checked_add(context_bytes))
            .ok_or("offline evidence byte sum overflow")?;
        if self.carriers.len() as u64 >= self.limits.max_carriers
            || body_bytes > self.limits.max_carrier_bytes
            || proof_bytes > self.limits.max_proof_bytes
            || context_bytes == 0
            || context_bytes > self.limits.max_proof_bytes
            || retained > self.limits.max_retained_bytes
        {
            return Err("offline Native evidence exceeds its admitted interval".into());
        }
        block
            .validate_output_merkle_cache()
            .map_err(|error| error.to_string())?;
        let artifact = &proof.finality_artifact;
        let execution = &artifact.commit_qc.execution_commitment;
        if proof.block_header != block.header()
            || artifact.subject.payload_hash
                != block
                    .canonical_proposal_wire_hash()
                    .map_err(|error| error.to_string())?
            || execution.executed_block_wire_len != body_bytes
            || execution.executed_block_wire_hash
                != block
                    .executed_block_wire_hash()
                    .map_err(|error| error.to_string())?
            || execution.merge_carrier.is_some()
            || block
                .execution_context()
                .is_some_and(|context| context.merge_entry.is_some())
        {
            return Err("offline evidence is not this exact current Network carrier".into());
        }
        self.finality
            .verify(proof)
            .map_err(|error| error.to_string())?;
        let evidence: NativeLaneContextsEvidenceV1 = norito::decode_canonical_with_limits(
            context_evidence,
            norito::canonical_decode_limits(context_evidence.len()),
        )
        .map_err(|error| error.to_string())?;
        if !evidence.witness.verify(
            self.network,
            artifact.height,
            execution.ordinary_writes_root,
        ) || !evidence.witness.matches_contexts(
            self.network,
            artifact.height,
            &evidence.contexts,
        )? {
            return Err("offline complete context set differs from finalized writes".into());
        }
        if let Some(batch) = block
            .execution_context()
            .and_then(|context| context.native_lane_decisions.as_deref())
        {
            batch.validate_structure()?;
            for group in &batch.groups {
                let descriptor = &group.payload.descriptor;
                let binding = group.payload.input.certificate.binding.canonical_hash();
                if self
                    .first_admissions
                    .get(&group.payload.input.certificate.binding.registry_key())
                    != Some(&(binding, descriptor.admission_priority))
                {
                    return Err("Native source lacks its exact first finalized admission".into());
                }
                let source = self
                    .carriers
                    .get(&descriptor.admission_priority.carrier_height)
                    .ok_or("Native admission carrier is outside the authenticated interval")?;
                let bytes = source
                    .block
                    .execution_context()
                    .and_then(|context| {
                        context
                            .queue_plan_admissions
                            .get(descriptor.admission_priority.admission_index as usize)
                    })
                    .ok_or("Native admission position is absent")?;
                if source.block.hash() != descriptor.admission_carrier_hash
                    || Hash::new(bytes) != descriptor.admitted_input_hash
                    || norito::encode_canonical(&group.payload.input)
                        .map_err(|error| error.to_string())?
                        != *bytes
                {
                    return Err("Native source substitutes the exact first-carrier input".into());
                }
                let kind = group.payload.validate_structure()?;
                let payload_bytes =
                    norito::encode_canonical(&group.payload).map_err(|error| error.to_string())?;
                for (slot, decision) in descriptor.slots.iter().zip(&group.decisions) {
                    let lane = self
                        .contexts
                        .iter()
                        .find(|context| Hash::from(context.instance_id().0) == slot.instance_id)
                        .ok_or(
                            "Native Decision is absent from the previous complete context set",
                        )?;
                    LaneAuthenticator::new(lane)
                        .decision_certificate(decision)
                        .map_err(|error| error.to_string())?;
                    let expected = encode_frozen_lane_input(
                        lane,
                        &group.payload,
                        kind,
                        &payload_bytes,
                        decision.value().origin_view,
                    )?;
                    if expected.manifest() != &decision.manifest {
                        return Err(
                            "Native Decision differs from the exact RS16 input codeword".into()
                        );
                    }
                }
            }
        }
        let mut contexts = Vec::with_capacity(evidence.contexts.contexts.len());
        for frozen in evidence.contexts.contexts {
            let opening = if frozen.opening_global_height == artifact.height {
                artifact
            } else {
                &self
                    .carriers
                    .get(&frozen.opening_global_height)
                    .ok_or("offline lane opening witness is missing from the interval")?
                    .finality
            };
            // An old opening must retain the same exact context. A later proof may
            // carry an existing instance forward, but cannot invent its old set.
            if frozen.opening_global_height != artifact.height
                && !self
                    .contexts
                    .iter()
                    .any(|previous| previous.frozen() == &frozen)
            {
                return Err("offline context rewrites or resurrects an earlier opening".into());
            }
            contexts.push(VerifiedLaneContext::from_verified_opening(
                frozen,
                opening,
                opening.block_hash,
            )?);
        }
        if let Some(context) = block.execution_context() {
            let mut previous_key = None;
            for (index, bytes) in context.queue_plan_admissions.iter().enumerate() {
                let admitted = decode_and_validate_lane_admitted_input_v1(&self.network, bytes)?;
                let binding = admitted.certificate().binding_hash;
                let key = admitted.certificate().registry_key;
                if previous_key
                    .as_ref()
                    .is_some_and(|previous| previous >= &key)
                {
                    return Err(
                        "offline admissions are not in strict immutable registry order".into(),
                    );
                }
                previous_key = Some(key);
                let priority = QueuePlanAdmissionPriorityV1::new(artifact.height, index)
                    .map_err(|error| error.to_string())?;
                if let Some((first_binding, _)) = self.first_admissions.get(&key) {
                    if *first_binding != binding {
                        return Err("offline admission changes an immutable first binding".into());
                    }
                } else {
                    self.first_admissions.insert(key, (binding, priority));
                }
            }
        }
        let block = Arc::new(block);
        self.carriers.insert(
            artifact.height,
            RetainedCarrier {
                block: Arc::clone(&block),
                finality: artifact.clone(),
            },
        );
        let predecessor_contexts = std::mem::replace(&mut self.contexts, contexts);
        self.retained_bytes = retained;
        Ok(VerifiedNativeExecutionCarrier {
            block,
            predecessor_contexts,
        })
    }
}

/// Construct an explicit offline context-write fixture and its computed root.
/// This test support runs no State transition and grants no finality or live authority.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub fn native_context_evidence_for_testing(
    network: NetworkId,
    height: u64,
    contexts: iroha_data_model::block::lane_consensus::LaneConsensusContextsV1,
) -> Result<(Vec<u8>, Hash), String> {
    let commitment =
        super::LaneConsensusContextsCommitmentV1::from_contexts(network, height, &contexts)?;
    let witness = ExecWitness {
        writes: vec![iroha_data_model::block::consensus::ExecKv {
            key: super::LANE_CONSENSUS_CONTEXTS_WITNESS_KEY.to_vec(),
            value: norito::to_bytes(&commitment).map_err(|error| error.to_string())?,
        }],
        ..ExecWitness::default()
    };
    let (_, root) = LaneConsensusContextsWitnessV1::from_witness(&witness)?;
    let proof =
        NativeLaneContextsEvidenceV1::from_execution_witness(contexts, &witness, network, height)?;
    Ok((
        norito::encode_canonical(&proof).map_err(|error| error.to_string())?,
        root,
    ))
}

/// Derive the actual instance identity for explicit signed offline fixtures.
/// The test caller must still authenticate context membership and Decision signatures.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub fn native_lane_instance_for_testing(
    frozen: iroha_data_model::block::lane_consensus::FrozenLaneConsensusContextV1,
    opening: &V2FinalityArtifact,
) -> Result<Hash, String> {
    opening.verify().map_err(|error| error.to_string())?;
    let lane = VerifiedLaneContext::from_verified_opening(frozen, opening, opening.block_hash)?;
    Ok(Hash::from(lane.instance_id().0))
}

/// Reuse the exact live RS16/origin calculation for a bounded offline fixture.
/// Returning this unsigned manifest never authorizes a vote or actual execution.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub fn native_lane_manifest_for_testing(
    frozen: iroha_data_model::block::lane_consensus::FrozenLaneConsensusContextV1,
    opening: &V2FinalityArtifact,
    payload: &iroha_data_model::block::lane_input::LaneInputPayloadV1,
    origin_view: u64,
) -> Result<iroha_data_model::block::lane_consensus::LaneManifestV1, String> {
    opening.verify().map_err(|error| error.to_string())?;
    let lane = VerifiedLaneContext::from_verified_opening(frozen, opening, opening.block_hash)?;
    let bytes = norito::encode_canonical(payload).map_err(|error| error.to_string())?;
    Ok(encode_frozen_lane_input(
        &lane,
        payload,
        payload.validate_structure()?,
        &bytes,
        origin_view,
    )?
    .manifest()
    .clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::HashOf;
    use iroha_data_model::block::consensus::ExecKv;

    #[test]
    fn context_evidence_requires_one_exact_actual_write_and_declared_frame() {
        let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"offline witness test",
        )));
        let contexts = LaneConsensusContextsV1::default();
        let commitment =
            super::super::LaneConsensusContextsCommitmentV1::from_contexts(network, 1, &contexts)
                .unwrap();
        let write = ExecKv {
            key: super::super::LANE_CONSENSUS_CONTEXTS_WITNESS_KEY.to_vec(),
            value: norito::to_bytes(&commitment).unwrap(),
        };
        let witness = ExecWitness {
            writes: vec![write.clone()],
            ..ExecWitness::default()
        };
        let proof = NativeLaneContextsEvidenceV1::from_execution_witness(
            contexts.clone(),
            &witness,
            network,
            1,
        )
        .unwrap();
        let (bytes, root) =
            native_context_evidence_for_testing(network, 1, contexts.clone()).unwrap();
        assert_eq!(norito::encode_canonical(&proof).unwrap(), bytes);
        let decoded: NativeLaneContextsEvidenceV1 = norito::decode_canonical(&bytes).unwrap();
        assert!(decoded.witness.verify(network, 1, root));
        assert!(
            decoded
                .witness
                .matches_contexts(network, 1, &contexts)
                .unwrap()
        );
        assert!(
            !decoded
                .witness
                .verify(network, 1, Hash::new(b"wrong write root"))
        );
        assert!(
            NativeLaneContextsEvidenceV1::from_execution_witness(
                contexts.clone(),
                &ExecWitness::default(),
                network,
                1
            )
            .is_err()
        );
        let duplicate = ExecWitness {
            writes: vec![write.clone(), write],
            ..ExecWitness::default()
        };
        assert!(
            NativeLaneContextsEvidenceV1::from_execution_witness(
                contexts.clone(),
                &duplicate,
                network,
                1
            )
            .is_err()
        );
        assert!(
            NativeLaneContextsEvidenceV1::from_execution_witness(contexts, &witness, network, 2)
                .is_err()
        );
        assert!(
            norito::decode_canonical::<NativeLaneContextsEvidenceV1>(
                &bytes[norito::core::Header::SIZE..]
            )
            .is_err()
        );
    }
}
