//! Explicit offline Native transcripts with genuine four-validator signatures.
//!
//! Each distinct route contributes at most one input per carrier. Context writes,
//! admission attestations, lane Decisions, RS16 manifests and global finality are
//! authenticated by their actual producers. Opaque State roots and successful
//! outputs are structural fixtures; these tests make no State execution claim.

use super::*;
use iroha_core::state::{
    native_context_evidence_for_testing, native_lane_instance_for_testing,
    native_lane_manifest_for_testing,
};
use iroha_core::torii_proxy::{
    new_queue_plan_admission_binding, queue_plan_admission_attestation_signing_bytes_v1,
};
use iroha_crypto::{Algorithm, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    block::{
        BlockExecutionContextBundle, BlockSignature,
        builder::BlockBuilder,
        consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
            QuorumCertificate, ValidatorPower, Vote, finality::V2FinalityArtifact,
        },
        execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1},
        lane_admission::*,
        lane_consensus::*,
        lane_decision_batch::LaneDecisionBatchV1,
        lane_input::*,
        output_budget::ExecutionOutputLimits,
    },
    bridge::BRIDGE_FINALITY_PROOF_VERSION_V2,
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterV1,
        KagemushaMintFinalityValidatorKeysV1,
    },
    transaction::{
        FeePaymentIntent, TransactionAdmissionIntent, TransactionBuilder, signed::TransactionResult,
    },
    trigger::DataTriggerSequence,
};
use iroha_model_base::peer::PeerId;
use norito::codec::{DecodeAll as _, Encode as _};
use std::num::NonZeroU64;

pub(super) fn h(label: &str) -> Hash {
    Hash::new(label.as_bytes())
}
fn keys() -> Vec<KeyPair> {
    let mut keys: Vec<_> = (1..=4)
        .map(|n| KeyPair::try_from_seed(vec![n; 32], Algorithm::BlsNormal).unwrap())
        .collect();
    keys.sort_by_key(|k| PeerId::new(k.public_key().clone()));
    keys
}
fn peers(keys: &[KeyPair]) -> Vec<PeerId> {
    keys.iter()
        .map(|k| PeerId::new(k.public_key().clone()))
        .collect()
}
fn aggregate(keys: &[KeyPair], message: &[u8]) -> Vec<u8> {
    let shares: Vec<_> = keys
        .iter()
        .take(3)
        .map(|k| {
            Signature::try_new(k.private_key(), message)
                .unwrap()
                .payload()
                .to_vec()
        })
        .collect();
    iroha_crypto::bls_normal_aggregate_signatures(
        &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .unwrap()
}
fn pops(keys: &[KeyPair]) -> Vec<Vec<u8>> {
    keys.iter()
        .map(|k| iroha_crypto::bls_normal_pop_prove(k.private_key()).unwrap())
        .collect()
}
fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(h(
        "canonical workload genesis pin",
    )))
}
pub(super) fn context(keys: &[KeyPair]) -> HeightContext {
    let roster: Vec<_> = peers(keys)
        .into_iter()
        .map(|validator| ValidatorPower {
            validator,
            power: 1,
        })
        .collect();
    let mint = KagemushaMintFinalityEpochRosterV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id: network(),
        epoch: 0,
        validators: roster
            .iter()
            .enumerate()
            .map(|(i, v)| KagemushaMintFinalityValidatorKeysV1 {
                validator: v.validator.clone(),
                eq_proof_public_key: [u8::try_from(i + 1).unwrap(); 32],
                ep_proof_public_key: [u8::try_from(i + 17).unwrap(); 32],
            })
            .collect(),
    };
    HeightContext {
        network_id: network(),
        protocol_version: PROTOCOL_VERSION,
        height: 1,
        epoch: 0,
        kagemusha_mint_finality_epoch_id: mint.finality_epoch_id().unwrap(),
        kagemusha_mint_finality_epoch_roster: mint,
        epoch_end_height: 2048,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).unwrap(),
        roster,
        nexus_amx_context_hash: h("nexus execution context"),
        execution_policy_hash: h("runtime execution policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 8192,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 2 * 1024 * 1024,
            max_chunk_count: 512,
        },
        leader_seed: [0xA5; 32],
    }
}
pub(super) fn signed_proof(
    keys: &[KeyPair],
    context: HeightContext,
    block: &SignedBlock,
    ordinary_writes_root: Hash,
) -> BridgeFinalityProof {
    let wire = block.encode_wire().unwrap();
    let execution = ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
        h("pre state"),
        h("post state"),
        ordinary_writes_root,
        wire.len() as u64,
        Hash::new(&wire),
    );
    let subject = BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: block.canonical_proposal_wire_hash().unwrap(),
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: block.header().view_change_index(),
    };
    let vote = Vote {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment: execution,
        signer: 0,
        signature: Vec::new(),
    };
    let qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment: execution,
        signers: vec![0, 1, 2],
        aggregate_signature: aggregate(keys, &vote.signature_preimage()),
    };
    let artifact = V2FinalityArtifact::new(context, subject, qc, pops(keys));
    artifact.verify().expect("actual 3-of-4 global finality");
    artifact.validate_for_header(&block.header()).unwrap();
    BridgeFinalityProof {
        version: BRIDGE_FINALITY_PROOF_VERSION_V2,
        block_header: block.header(),
        finality_artifact: artifact,
    }
}

pub(super) fn binding(index: usize) -> NativeWorkloadLane {
    NativeWorkloadLane {
        lane_id: LaneId::new(index as u32),
        dataspace_id: DataSpaceId::new(index as u64),
        incarnation: h(&format!("incarnation {index}")),
        activation_height: 1,
    }
}

pub(super) fn attach_outputs(
    block: &mut SignedBlock,
    outputs: Vec<ExecutionOutputV1>,
    keys: &[KeyPair],
) {
    let fragments = outputs
        .iter()
        .filter(|output| output.result().0.is_ok())
        .count() as u64;
    block
        .set_execution_outputs(
            outputs,
            fragments,
            BTreeMap::new(),
            Vec::new(),
            Default::default(),
            BTreeSet::new(),
            Vec::new(),
            &ExecutionOutputLimits {
                max_outputs: 1024,
                max_output_bytes: 1024 * 1024,
                max_total_output_bytes: 4 * 1024 * 1024,
                max_executed_wire_bytes: 32 * 1024 * 1024,
            },
        )
        .unwrap();
    block
        .replace_signatures(
            [BlockSignature::new(
                0,
                SignatureOf::from_hash(keys[0].private_key(), block.hash()),
            )]
            .into_iter()
            .collect(),
        )
        .unwrap();
}

pub(super) fn successful_outputs(block: &SignedBlock) -> Vec<ExecutionOutputV1> {
    (0..block.network_entrypoint_count())
        .map(|index| {
            ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                input_index: index as u32,
                result: TransactionResult::from(Ok(DataTriggerSequence::default())),
                completions: Vec::new(),
            })
        })
        .collect()
}

#[derive(Clone)]
pub(super) struct Height {
    pub block: SignedBlock,
    pub proof: BridgeFinalityProof,
    pub contexts: LaneConsensusContextsV1,
    pub evidence: Vec<u8>,
}
impl Height {
    pub fn queries(&self) -> Vec<Vec<u8>> {
        if self
            .block
            .execution_context()
            .and_then(|context| context.native_lane_decisions.as_ref())
            .is_none()
        {
            return Vec::new();
        }
        self.block
            .network_entrypoints()
            .enumerate()
            .map(|(index, entrypoint)| {
                let (output_index, _) = self.block.network_output_at(index as u32).unwrap();
                let output = self.block.execution_outputs()[output_index as usize].clone();
                norito::encode_canonical(&CommittedTransaction {
                    block_hash: self.block.hash(),
                    entrypoint_hash: entrypoint.hash(),
                    entrypoint_proof: self.block.network_input_proof(index as u32).unwrap(),
                    entrypoint: entrypoint.clone(),
                    output_hash: HashOf::new(&output),
                    output_proof: self.block.output_proof(output_index).unwrap(),
                    output,
                })
                .unwrap()
            })
            .collect()
    }
    pub fn push(&self, verifier: &mut ScalingProofVerifier) -> Result<()> {
        let queries = self.queries();
        verifier.push_height(
            &norito::encode_canonical(&self.proof).unwrap(),
            &self.block.encode_wire().unwrap(),
            &self.evidence,
            &queries.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
    }
    pub fn resign(&mut self, keys: &[KeyPair]) {
        let (evidence, root) = native_context_evidence_for_testing(
            network(),
            self.proof.block_header.height().get(),
            self.contexts.clone(),
        )
        .unwrap();
        self.evidence = evidence;
        self.proof = signed_proof(
            keys,
            self.proof.finality_artifact.height_context.clone(),
            &self.block,
            root,
        );
    }
}

pub(super) struct Fixture {
    pub keys: Vec<KeyPair>,
    pub heights: Vec<Height>,
    pub lane_count: usize,
    pub requests: Vec<(String, SignedTransaction, RoutingDecision, WorkloadPhase)>,
}
impl Fixture {
    pub fn new(lane_count: usize) -> Self {
        Self::with_request_count(lane_count, 8)
    }
    pub fn with_request_count(lane_count: usize, request_count: usize) -> Self {
        assert!(matches!(lane_count, 1 | 4));
        assert!((8..=1024).contains(&request_count));
        let keys = keys();
        let roster = peers(&keys);
        let mut requests = Vec::new();
        let mut inputs = Vec::new();
        for index in 0..request_count {
            let owner =
                KeyPair::try_from_seed(vec![80 + (index % 4) as u8; 32], Algorithm::Ed25519)
                    .unwrap();
            let authority = AccountId::new(owner.public_key().clone());
            let logical = format!("{index:064x}");
            let binding = binding(index % lane_count);
            let route = RoutingDecision::new(binding.lane_id, binding.dataspace_id);
            let mut builder = TransactionBuilder::new(
                network(),
                authority.clone(),
                FeePaymentIntent::authority(Vec::new(), None),
            );
            builder.set_creation_time(std::time::Duration::from_millis(index as u64));
            let tx = builder
                .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
                .with_executable(expected_executable(&authority, &logical).unwrap())
                .sign(owner.private_key());
            let entrypoint = TransactionEntrypoint::External(tx.clone());
            let routing = RoutingPlan::single(route);
            let context = QueuePlanAdmissionContextV1 {
                version: QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V1,
                authority_height: 0,
                proposal_height: 1,
                predecessor_block_hash: None,
                routing_plan_digest: routing.digest(),
                route_incarnations: vec![QueuePlanRouteIncarnationV1 {
                    leg: routing.coordinator_leg(),
                    lane_incarnation: binding.incarnation,
                    validator_set_hash_version: 1,
                    validator_set_hash: HashOf::new(&roster),
                    validator_set: roster.clone(),
                    validator_count: 4,
                    durability_threshold: 2,
                }],
            };
            let binding = new_queue_plan_admission_binding(
                &network(),
                &entrypoint,
                &routing,
                context,
                index as u64,
            )
            .unwrap();
            let attestations = keys
                .iter()
                .take(2)
                .enumerate()
                .map(|(index, key)| QueuePlanAdmissionAttestationV1 {
                    version: QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V1,
                    validator_index: index as u16,
                    signature: Signature::new(
                        key.private_key(),
                        &queue_plan_admission_attestation_signing_bytes_v1(
                            binding.canonical_hash(),
                            index as u16,
                        )
                        .unwrap(),
                    ),
                })
                .collect();
            inputs.push(LaneAdmittedInputV1 {
                entrypoint,
                certificate: QueuePlanAdmissionCertificateV1 {
                    version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V1,
                    binding,
                    attestations,
                },
            });
            requests.push((
                logical,
                tx,
                route,
                if index < 4 {
                    WorkloadPhase::Warmup
                } else {
                    WorkloadPhase::Measurement
                },
            ));
        }
        inputs.sort_by_key(|input| input.certificate.binding.registry_key());
        let pending: Vec<Vec<usize>> = (0..lane_count)
            .map(|lane| {
                inputs
                    .iter()
                    .enumerate()
                    .filter_map(|(index, input)| {
                        (input
                            .routing_plan()
                            .unwrap()
                            .coordinator_leg()
                            .route
                            .lane_id
                            == LaneId::new(lane as u32))
                        .then_some(index)
                    })
                    .collect()
            })
            .collect();
        let mut offsets = vec![0usize; lane_count];
        let mut frontiers = vec![(0u64, None, 0u64); lane_count];
        let mut heights = Vec::new();
        let mut global_context = context(&keys);
        let admission_bytes: Vec<_> = inputs
            .iter()
            .map(|input| norito::encode_canonical(input).unwrap())
            .collect();
        let mut builder = BlockBuilder::new(BlockHeader::new(
            NonZeroU64::new(1).unwrap(),
            None,
            None,
            0,
            0,
        ));
        builder.set_execution_context(Some(
            BlockExecutionContextBundle::default()
                .with_queue_plan_admissions(admission_bytes.clone()),
        ));
        let mut block = builder.build(BTreeSet::new());
        attach_outputs(&mut block, Vec::new(), &keys);
        loop {
            let height = block.header().height().get();
            let contexts = LaneConsensusContextsV1::new(
                (0..lane_count)
                    .filter_map(|lane| {
                        let index = *pending[lane].get(offsets[lane])?;
                        let input = &inputs[index];
                        let binding = binding(lane);
                        let (previous, hash, applied) = frontiers[lane];
                        Some(FrozenLaneConsensusContextV1 {
                            network_id: network(),
                            protocol_version: PROTOCOL_VERSION,
                            opening_global_height: height,
                            opening_global_context_id: global_context.id(),
                            admitted_binding_hash: input.certificate.binding.canonical_hash(),
                            admission_priority: QueuePlanAdmissionPriorityV1::new(1, index)
                                .unwrap(),
                            epoch: 0,
                            mode: ConsensusMode::Permissioned,
                            lane_id: binding.lane_id,
                            dataspace_id: binding.dataspace_id,
                            lane_incarnation: binding.incarnation,
                            next_lane_height: previous + 1,
                            predecessor_height: previous,
                            predecessor_hash: hash,
                            predecessor_applied_global_height: applied,
                            committee: roster.clone(),
                            validator_set_pops: pops(&keys),
                            nexus_amx_context_hash: global_context.nexus_amx_context_hash,
                            execution_policy_hash: global_context.execution_policy_hash,
                            da_layout: global_context.da_layout,
                            leader_seed: global_context.leader_seed,
                        })
                    })
                    .collect(),
            )
            .unwrap();
            let (evidence, root) =
                native_context_evidence_for_testing(network(), height, contexts.clone()).unwrap();
            let proof = signed_proof(&keys, global_context.clone(), &block, root);
            heights.push(Height {
                block: block.clone(),
                proof: proof.clone(),
                contexts: contexts.clone(),
                evidence,
            });
            if contexts.contexts.is_empty() {
                break;
            }
            let mut groups = Vec::new();
            for frozen in &contexts.contexts {
                let lane = frozen.lane_id.as_u32() as usize;
                let index = pending[lane][offsets[lane]];
                let input = inputs[index].clone();
                let instance =
                    native_lane_instance_for_testing(frozen.clone(), &proof.finality_artifact)
                        .unwrap();
                let payload = LaneInputPayloadV1 {
                    input,
                    descriptor: LaneInputDescriptorV1 {
                        version: LANE_INPUT_VERSION_V1,
                        admission_priority: frozen.admission_priority,
                        admission_carrier_hash: heights[0].block.hash(),
                        admitted_input_hash: Hash::new(&admission_bytes[index]),
                        slots: vec![LaneInputRouteSlotV1 {
                            route: RoutingDecision::new(frozen.lane_id, frozen.dataspace_id),
                            lane_incarnation: frozen.lane_incarnation,
                            instance_id: instance,
                            lane_height: frozen.next_lane_height,
                        }],
                    },
                };
                let manifest = native_lane_manifest_for_testing(
                    frozen.clone(),
                    &proof.finality_artifact,
                    &payload,
                    0,
                )
                .unwrap();
                let statement = LaneVoteStatementV1 {
                    round: LaneRoundV1 {
                        instance_id: instance,
                        lane_height: frozen.next_lane_height,
                        voting_view: 0,
                    },
                    phase: LanePhaseV1::Commit,
                    value: manifest.value,
                };
                let shares = keys
                    .iter()
                    .take(3)
                    .enumerate()
                    .map(|(index, key)| LaneSignatureShareV1 {
                        signer: index as u32,
                        signature: Signature::try_new(
                            key.private_key(),
                            &statement.signature_preimage().unwrap(),
                        )
                        .unwrap()
                        .payload()
                        .to_vec(),
                    })
                    .collect();
                frontiers[lane] = (
                    frozen.next_lane_height,
                    Some(payload.descriptor.canonical_hash().unwrap()),
                    height + 1,
                );
                offsets[lane] += 1;
                groups.push(LaneDecisionGroupV1 {
                    payload,
                    decisions: vec![LaneDecisionV1 {
                        manifest,
                        commit_qc: LaneQcV1 { statement, shares },
                    }],
                });
            }
            groups.sort_by_key(|group| group.payload.descriptor.admission_priority);
            let batch = LaneDecisionBatchV1 {
                base_state_height: height,
                base_state_hash: HashOf::from_untyped_unchecked(h(&format!(
                    "explicit offline State cut {height}"
                ))),
                groups,
            };
            let mut builder = BlockBuilder::new(BlockHeader::new(
                NonZeroU64::new(height + 1).unwrap(),
                Some(block.hash()),
                None,
                height * 100,
                0,
            ));
            builder.set_execution_context(Some(
                BlockExecutionContextBundle::default().with_native_lane_decisions(batch),
            ));
            block = builder.build(BTreeSet::new());
            let outputs = successful_outputs(&block);
            attach_outputs(&mut block, outputs, &keys);
            global_context.height = height + 1;
            global_context.parent_commit_qc = Some(proof.finality_artifact.commit_qc);
        }
        Self {
            keys,
            heights,
            lane_count,
            requests,
        }
    }
    pub fn plan(&self) -> TrustedRunPlan {
        TrustedRunPlan {
            network_id: network(),
            first_context: self.heights[0].proof.finality_artifact.context_id(),
            first_height: 1,
            last_height: self.heights.len() as u64,
            nexus_amx_context_hash: context(&self.keys).nexus_amx_context_hash,
            execution_policy_hash: context(&self.keys).execution_policy_hash,
            active_lanes: (0..self.lane_count).map(binding).collect(),
            lane_authorities: MergeLaneAuthorityCatalogV1::from_lane_committees(
                &vec![peers(&self.keys); self.lane_count],
            )
            .unwrap(),
            scheduled: self
                .requests
                .iter()
                .map(|(logical, tx, route, phase)| ScheduledRequest {
                    logical_id: logical.clone(),
                    phase: *phase,
                    signed_transaction: norito::encode_canonical(tx).unwrap(),
                    route: *route,
                })
                .collect(),
        }
    }
    pub fn start(&self, plan: TrustedRunPlan, limits: VerificationLimits) -> ScalingProofVerifier {
        let mut verifier = ScalingProofVerifier::new(plan, limits).unwrap();
        self.heights[0].push(&mut verifier).unwrap();
        verifier
    }
    pub fn push(&self, verifier: &mut ScalingProofVerifier) -> Result<()> {
        for height in &self.heights[1..] {
            height.push(verifier)?;
        }
        Ok(())
    }
}
pub(super) fn limits() -> VerificationLimits {
    VerificationLimits {
        admitted_proof_bytes: 64 * 1024 * 1024,
        input_bytes: 48 * 1024 * 1024,
        output_bytes: 16 * 1024 * 1024,
        heights: 1025,
        requests: 1024,
        leaves_per_carrier: 1024,
    }
}

// Test-only raw wire projection permits adverse full-output and source mutations.
// It is decoded through SignedBlock again; production has no unchecked setter.
#[derive(norito::Encode, norito::Decode)]
pub(super) struct RawBlock {
    pub signatures: BTreeSet<BlockSignature>,
    pub payload: iroha_data_model::block::BlockPayload,
    pub result: Option<iroha_data_model::block::BlockResult>,
}
pub(super) fn mutate_height(
    height: &mut Height,
    keys: &[KeyPair],
    change: impl FnOnce(&mut RawBlock),
) {
    let mut raw = RawBlock::decode_all(&mut height.block.encode().as_slice()).unwrap();
    change(&mut raw);
    raw.payload
        .header
        .set_execution_context_hash(raw.payload.execution_context.as_ref().map(HashOf::new));
    raw.signatures = [BlockSignature::new(
        0,
        SignatureOf::from_hash(keys[0].private_key(), raw.payload.header.hash()),
    )]
    .into_iter()
    .collect();
    height.block = SignedBlock::decode_all(&mut raw.encode().as_slice()).unwrap();
    height.resign(keys);
}

pub(super) fn successor(
    keys: &[KeyPair],
    parent: &Height,
    execution: Option<BlockExecutionContextBundle>,
    contexts: LaneConsensusContextsV1,
) -> Height {
    let number = parent.block.header().height().get() + 1;
    let mut builder = BlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(number).unwrap(),
        Some(parent.block.hash()),
        None,
        number * 100,
        0,
    ));
    builder.set_execution_context(execution);
    let mut block = builder.build(BTreeSet::new());
    let outputs = successful_outputs(&block);
    attach_outputs(&mut block, outputs, keys);
    let mut context = parent.proof.finality_artifact.height_context.clone();
    context.height = number;
    context.parent_commit_qc = Some(parent.proof.finality_artifact.commit_qc.clone());
    let (evidence, root) =
        native_context_evidence_for_testing(network(), number, contexts.clone()).unwrap();
    let proof = signed_proof(keys, context, &block, root);
    Height {
        block,
        proof,
        contexts,
        evidence,
    }
}

pub(super) fn resign_group(
    group: &mut LaneDecisionGroupV1,
    frozen: &FrozenLaneConsensusContextV1,
    opening: &V2FinalityArtifact,
    keys: &[KeyPair],
) {
    let manifest =
        native_lane_manifest_for_testing(frozen.clone(), opening, &group.payload, 0).unwrap();
    let statement = LaneVoteStatementV1 {
        round: LaneRoundV1 {
            instance_id: manifest.value.instance_id,
            lane_height: frozen.next_lane_height,
            voting_view: 0,
        },
        phase: LanePhaseV1::Commit,
        value: manifest.value,
    };
    let shares = keys
        .iter()
        .take(3)
        .enumerate()
        .map(|(index, key)| LaneSignatureShareV1 {
            signer: index as u32,
            signature: Signature::try_new(
                key.private_key(),
                &statement.signature_preimage().unwrap(),
            )
            .unwrap()
            .payload()
            .to_vec(),
        })
        .collect();
    group.decisions = vec![LaneDecisionV1 {
        manifest,
        commit_qc: LaneQcV1 { statement, shares },
    }];
}

pub(super) fn resign_claimed_subject(height: &mut Height, keys: &[KeyPair]) {
    let artifact = &mut height.proof.finality_artifact;
    artifact.commit_qc.subject = artifact.subject;
    let qc = &mut artifact.commit_qc;
    let vote = Vote {
        round: qc.round,
        proposal_round: qc.proposal_round,
        phase: qc.phase,
        subject: qc.subject,
        execution_commitment: qc.execution_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    qc.aggregate_signature = aggregate(keys, &vote.signature_preimage());
    artifact.verify().unwrap();
}
