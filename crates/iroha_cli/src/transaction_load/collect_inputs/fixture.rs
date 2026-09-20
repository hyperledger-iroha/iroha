//! Offline Native collection transcripts with exact four-validator signatures.
//!
//! Public Core producers authenticate admitted inputs, RS16 manifests, Native
//! Decisions, global finality and context-write witnesses. Opaque State roots and
//! fixture outputs establish no runtime execution or performance claim.

use eyre::{Result, ensure, eyre};
use iroha::http::{HttpTransport, Method, Response, TransportFuture, TransportRequest};
use iroha_core::{
    kura::{
        BlockStore, CanonicalKuraEvidenceComplete, CanonicalKuraEvidenceLimits,
        CanonicalKuraEvidenceReader,
    },
    queue::{RoutingDecision, RoutingPlan},
    state::{
        FinalizedNativeContextV1, NativeLaneContextsEvidenceV1,
        native_context_evidence_for_testing, native_lane_instance_for_testing,
        native_lane_manifest_for_testing,
    },
    torii_proxy::{
        new_queue_plan_admission_binding, queue_plan_admission_attestation_signing_bytes_v1,
    },
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::{
        BlockExecutionContextBundle, BlockHeader, BlockSignature, SignedBlock,
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
    bridge::{BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeFinalityProof, BridgeFinalityVerifier},
    isi::{
        InstructionBox, SetKeyValue,
        kagemusha_v1::{
            KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterV1,
            KagemushaMintFinalityValidatorKeysV1,
        },
    },
    query::CommittedTransaction,
    transaction::{
        Executable, FeePaymentIntent, SignedTransaction, TransactionAdmissionIntent,
        TransactionBuilder,
        signed::{TransactionEntrypoint, TransactionResult},
    },
    trigger::DataTriggerSequence,
};
use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};
use iroha_primitives::json::Json;
use norito::codec::{DecodeAll as _, Encode as _};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    num::NonZeroU64,
    path::PathBuf,
    sync::{Arc, Mutex},
};

fn h(label: &str) -> Hash {
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
fn context(keys: &[KeyPair], network_id: NetworkId) -> HeightContext {
    let roster: Vec<_> = peers(keys)
        .into_iter()
        .map(|validator| ValidatorPower {
            validator,
            power: 1,
        })
        .collect();
    let mint = KagemushaMintFinalityEpochRosterV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id,
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
        network_id,
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
fn signed_proof(
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

#[derive(Clone)]
struct Lane {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    incarnation: Hash,
}
fn binding(index: usize) -> Lane {
    Lane {
        lane_id: LaneId::new(index as u32),
        dataspace_id: DataSpaceId::UNIVERSAL,
        incarnation: h(&format!("incarnation {index}")),
    }
}

fn attach_outputs(block: &mut SignedBlock, outputs: Vec<ExecutionOutputV1>, keys: &[KeyPair]) {
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

fn successful_outputs(block: &SignedBlock) -> Vec<ExecutionOutputV1> {
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
    pub fn queries(&self) -> Vec<CommittedTransaction> {
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
                CommittedTransaction {
                    block_hash: self.block.hash(),
                    entrypoint_hash: entrypoint.hash(),
                    entrypoint_proof: self.block.network_input_proof(index as u32).unwrap(),
                    entrypoint: entrypoint.clone(),
                    output_hash: HashOf::new(&output),
                    output_proof: self.block.output_proof(output_index).unwrap(),
                    output,
                }
            })
            .collect()
    }
    pub fn resign(&mut self, keys: &[KeyPair]) {
        let (evidence, root) = native_context_evidence_for_testing(
            self.proof.finality_artifact.height_context.network_id,
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

fn write_archive(heights: &[Height]) -> Result<tempfile::TempDir> {
    use std::{
        fs,
        os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    };
    let archive = tempfile::tempdir()?;
    let root = archive.path().canonicalize()?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let mut store = BlockStore::new(&root);
    store.create_files_if_they_do_not_exist()?;
    for height in heights {
        store.append_block_to_chain(&height.block)?;
    }
    drop(store);
    for name in [
        "blocks.data",
        "blocks.index",
        "blocks.hashes",
        "blocks.count.norito",
    ] {
        let path = root.join(name);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        fs::File::open(path)?.sync_all()?;
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root.join("merge.log"))?
        .sync_all()?;
    fs::File::open(&root)?.sync_all()?;
    Ok(archive)
}

/// Original offer order, independent of chronological Native input order.
pub(super) struct Request {
    pub logical_id: String,
    pub signed: SignedTransaction,
    pub route: RoutingDecision,
    pub warmup: bool,
}
/// An independently signed chronological Native transcript with original storage custody.
pub(super) struct Fixture {
    _archive: tempfile::TempDir,
    pub keys: Vec<KeyPair>,
    pub network_id: NetworkId,
    pub heights: Vec<Height>,
    pub lane_count: usize,
    pub requests: Vec<Request>,
}
impl Fixture {
    pub fn new(lane_count: usize, rejected_logical_index: Option<usize>) -> Self {
        assert!(matches!(lane_count, 1 | 4));
        assert!(rejected_logical_index.is_none_or(|index| index < 8));
        let keys = keys();
        let mut genesis = BlockBuilder::new(BlockHeader::new(
            NonZeroU64::new(1).unwrap(),
            None,
            None,
            0,
            0,
        ))
        .build(BTreeSet::new());
        attach_outputs(&mut genesis, Vec::new(), &keys);
        let network_id = NetworkId::from_genesis_hash(genesis.hash());
        let mut global_context = context(&keys, network_id);
        let bindings: Vec<_> = (0..lane_count).map(binding).collect();
        let mut requests = Vec::new();
        for index in 0..8 {
            let owner_key =
                KeyPair::try_from_seed(vec![80 + (index % 4) as u8; 32], Algorithm::Ed25519)
                    .unwrap();
            let authority = AccountId::new(owner_key.public_key().clone());
            let logical = format!("{index:064x}");
            let lane = &bindings[index % lane_count];
            let route = RoutingDecision::new(lane.lane_id, lane.dataspace_id);
            let mut builder = TransactionBuilder::new(
                network_id,
                authority.clone(),
                FeePaymentIntent::authority(Vec::new(), None),
            );
            builder.set_creation_time(std::time::Duration::from_millis(index as u64));
            let tx = builder
                .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
                .with_executable(Executable::Instructions(
                    vec![InstructionBox::from(SetKeyValue::account(
                        authority,
                        format!("gscale_{logical}").parse().unwrap(),
                        Json::try_new(logical.as_str()).unwrap(),
                    ))]
                    .into(),
                ))
                .sign(owner_key.private_key());
            requests.push(Request {
                logical_id: logical,
                signed: tx,
                route,
                warmup: index < 4,
            });
        }
        let genesis = Some(genesis);
        let network_id = global_context.network_id;
        let lane_count = bindings.len();
        let roster = peers(&keys);
        let admission_height = if genesis.is_some() { 2 } else { 1 };
        let parent_hash = genesis.as_ref().map(SignedBlock::hash);
        let mut inputs = Vec::new();
        for (index, request) in requests.iter().enumerate() {
            let tx = &request.signed;
            let route = &request.route;
            let binding = bindings
                .iter()
                .find(|lane| {
                    lane.lane_id == route.lane_id && lane.dataspace_id == route.dataspace_id
                })
                .unwrap();
            let entrypoint = TransactionEntrypoint::External(tx.clone());
            let routing = RoutingPlan::single(*route);
            let context = QueuePlanAdmissionContextV1 {
                version: QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V1,
                authority_height: admission_height - 1,
                proposal_height: admission_height,
                predecessor_block_hash: parent_hash,
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
                &network_id,
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
                            == bindings[lane].lane_id)
                            .then_some(index)
                    })
                    .collect()
            })
            .collect();
        let mut offsets = vec![0usize; lane_count];
        let mut frontiers = vec![(0u64, None, 0u64); lane_count];
        let mut heights = Vec::new();
        if let Some(genesis) = genesis {
            let contexts = LaneConsensusContextsV1::default();
            let (evidence, root) =
                native_context_evidence_for_testing(network_id, 1, contexts.clone()).unwrap();
            let proof = signed_proof(&keys, global_context.clone(), &genesis, root);
            heights.push(Height {
                block: genesis,
                proof: proof.clone(),
                contexts,
                evidence,
            });
            global_context.height = admission_height;
            global_context.parent_commit_qc = Some(proof.finality_artifact.commit_qc);
        }
        let admission_bytes: Vec<_> = inputs
            .iter()
            .map(|input| norito::encode_canonical(input).unwrap())
            .collect();
        let mut builder = BlockBuilder::new(BlockHeader::new(
            NonZeroU64::new(admission_height).unwrap(),
            parent_hash,
            None,
            admission_height * 100,
            0,
        ));
        builder.set_execution_context(Some(
            BlockExecutionContextBundle::default()
                .with_queue_plan_admissions(admission_bytes.clone()),
        ));
        let mut block = builder.build(BTreeSet::new());
        attach_outputs(&mut block, Vec::new(), &keys);
        let admission_carrier_hash = block.hash();
        loop {
            let height = block.header().height().get();
            let contexts = LaneConsensusContextsV1::new(
                (0..lane_count)
                    .filter_map(|lane| {
                        let index = *pending[lane].get(offsets[lane])?;
                        let input = &inputs[index];
                        let binding = &bindings[lane];
                        let (previous, hash, applied) = frontiers[lane];
                        Some(FrozenLaneConsensusContextV1 {
                            network_id,
                            protocol_version: PROTOCOL_VERSION,
                            opening_global_height: height,
                            opening_global_context_id: global_context.id(),
                            admitted_binding_hash: input.certificate.binding.canonical_hash(),
                            admission_priority: QueuePlanAdmissionPriorityV1::new(
                                admission_height,
                                index,
                            )
                            .unwrap(),
                            epoch: global_context.epoch,
                            mode: global_context.mode,
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
                native_context_evidence_for_testing(network_id, height, contexts.clone()).unwrap();
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
                let lane = bindings
                    .iter()
                    .position(|binding| binding.lane_id == frozen.lane_id)
                    .unwrap();
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
                        admission_carrier_hash,
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
            let mut outputs = successful_outputs(&block);
            if let Some(rejected) = rejected_logical_index {
                let hash = requests[rejected].signed.hash_as_entrypoint();
                if let Some(index) = block
                    .network_entrypoints()
                    .position(|entry| entry.hash() == hash)
                {
                    let ExecutionOutputV1::Network(output) = &mut outputs[index] else {
                        unreachable!()
                    };
                    output.result = TransactionResult::from(Err(
                        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                            iroha_data_model::ValidationFail::NotPermitted("committed collection fixture rejection".into()),
                        ),
                    ));
                }
            }
            attach_outputs(&mut block, outputs, &keys);
            global_context.height = height + 1;
            global_context.parent_commit_qc = Some(proof.finality_artifact.commit_qc);
        }
        let archive = write_archive(&heights).expect("public offline Native block store");
        Self {
            _archive: archive,
            keys,
            network_id,
            heights,
            lane_count,
            requests,
        }
    }
    pub fn queries(&self) -> Vec<CommittedTransaction> {
        self.heights.iter().flat_map(Height::queries).collect()
    }
    pub fn native_contexts(&self) -> Vec<NativeLaneContextsEvidenceV1> {
        self.heights
            .iter()
            .map(|height| norito::decode_canonical(&height.evidence).unwrap())
            .collect()
    }
    pub fn finalized_contexts(&self) -> Vec<FinalizedNativeContextV1> {
        self.heights
            .iter()
            .zip(self.native_contexts())
            .map(|(height, contexts)| FinalizedNativeContextV1 {
                finality: height.proof.clone(),
                contexts,
            })
            .collect()
    }
    /// Independently selected config for command and injected SDK admission.
    pub fn config(&self) -> iroha::config::Config {
        use iroha_service_model::{
            sorafs::*,
            soranet::{AnonymityPolicy, RolloutPhase},
        };
        use std::time::Duration;
        let key_pair = KeyPair::try_from_seed(vec![80; 32], Algorithm::Ed25519).unwrap();
        iroha::config::Config {
            chain: iroha_model_base::chain::ChainId::from("00000000-0000-0000-0000-000000000001"),
            network_id: self.network_id,
            account: AccountId::new(key_pair.public_key().clone()),
            key_pair,
            account_chain_discriminant: iroha_torii_shared::MINAMOTO_CHAIN_DISCRIMINANT,
            torii_api_url: "http://127.0.0.1:1/".parse().unwrap(),
            torii_request_timeout: Duration::from_secs(2),
            basic_auth: None,
            transaction_ttl: Duration::from_secs(5),
            transaction_status_timeout: Duration::from_secs(2),
            transaction_add_nonce: false,
            sorafs_alias_cache: sorafs_manifest::alias_cache::AliasCachePolicy::new(
                Duration::from_secs(DEFAULT_ALIAS_POSITIVE_TTL_SECS),
                Duration::from_secs(DEFAULT_ALIAS_REFRESH_WINDOW_SECS),
                Duration::from_secs(DEFAULT_ALIAS_HARD_EXPIRY_SECS),
                Duration::from_secs(DEFAULT_ALIAS_NEGATIVE_TTL_SECS),
                Duration::from_secs(DEFAULT_ALIAS_REVOCATION_TTL_SECS),
                Duration::from_secs(DEFAULT_ALIAS_ROTATION_MAX_AGE_SECS),
                Duration::from_secs(DEFAULT_ALIAS_SUCCESSOR_GRACE_SECS),
                Duration::from_secs(DEFAULT_ALIAS_GOVERNANCE_GRACE_SECS),
            ),
            sorafs_anonymity_policy: AnonymityPolicy::GuardPq,
            sorafs_rollout_phase: RolloutPhase::Canary,
        }
    }
    /// Validated SDK context using one of the exact independently signed request keys.
    pub fn client(&self, transport: Arc<dyn HttpTransport>) -> iroha::client::Client {
        iroha::client::Client::builder(self.config())
            .http_transport(transport)
            .build()
            .unwrap()
    }
    /// Valid signatures and exact header, but a deliberately incorrect executed-wire tuple.
    pub fn height_with_wrong_executed_wire(&self, index: usize) -> BridgeFinalityProof {
        let mut proof = self.heights[index].proof.clone();
        let qc = &mut proof.finality_artifact.commit_qc;
        qc.execution_commitment.executed_block_wire_hash = h("wrong but signed executed wire");
        qc.execution_commitment.executed_block_wire_len += 1;
        let vote = Vote {
            round: qc.round,
            proposal_round: qc.proposal_round,
            phase: qc.phase,
            subject: qc.subject,
            execution_commitment: qc.execution_commitment,
            signer: 0,
            signature: Vec::new(),
        };
        qc.aggregate_signature = aggregate(&self.keys, &vote.signature_preimage());
        proof.finality_artifact.verify().unwrap();
        proof
            .finality_artifact
            .validate_for_header(&self.heights[index].block.header())
            .unwrap();
        proof
    }
    /// Re-sign a bounded malformed Native carrier and its global descendants.
    /// Original Native Decisions remain unchanged so source/order mutations fail authentication.
    pub fn rewrite_native_for_test(
        &mut self,
        height_index: usize,
        mutate: impl FnOnce(&mut LaneDecisionBatchV1),
    ) -> Result<()> {
        ensure!(
            height_index >= 2 && height_index < self.heights.len(),
            "Native fixture height bound"
        );
        let mut heights = self.heights.clone();
        let mut execution = heights[height_index]
            .block
            .execution_context()
            .cloned()
            .ok_or_else(|| eyre!("Native context missing"))?;
        let batch = execution
            .native_lane_decisions
            .as_mut()
            .ok_or_else(|| eyre!("Native batch missing"))?;
        mutate(batch);
        ensure!(
            (1..=4).contains(&batch.groups.len()),
            "Native fixture group bound"
        );
        for index in height_index..heights.len() {
            let selected = if index == height_index {
                execution.clone()
            } else {
                heights[index]
                    .block
                    .execution_context()
                    .cloned()
                    .unwrap_or_default()
            };
            let mut raw = RawBlock::decode_all(&mut heights[index].block.encode().as_slice())?;
            raw.payload.execution_context = Some(selected);
            raw.payload.header = BlockHeader::new(
                NonZeroU64::new(index as u64 + 1).unwrap(),
                Some(heights[index - 1].block.hash()),
                None,
                index as u64 * 100,
                0,
            );
            raw.payload.header.set_execution_context_hash(
                raw.payload.execution_context.as_ref().map(HashOf::new),
            );
            raw.signatures = [BlockSignature::new(
                0,
                SignatureOf::from_hash(self.keys[0].private_key(), raw.payload.header.hash()),
            )]
            .into_iter()
            .collect();
            let block = SignedBlock::decode_all(&mut raw.encode().as_slice())?;
            ensure!(
                block.network_entrypoint_count() == block.execution_outputs().len(),
                "Native fixture output bound"
            );
            heights[index].block = block;
            heights[index]
                .proof
                .finality_artifact
                .height_context
                .parent_commit_qc =
                Some(heights[index - 1].proof.finality_artifact.commit_qc.clone());
            heights[index].resign(&self.keys);
        }
        let archive = write_archive(&heights)?;
        self.heights = heights;
        self._archive = archive;
        Ok(())
    }
    /// Actual canonicalized offline archive paths retained through the fixture lifetime.
    pub fn paths(&self) -> (PathBuf, PathBuf) {
        let root = self._archive.path().canonicalize().unwrap();
        (root.clone(), root.join("merge.log"))
    }
    /// Independently fixed reader budget for the complete chronological archive.
    pub fn limits(&self) -> CanonicalKuraEvidenceLimits {
        CanonicalKuraEvidenceLimits {
            first_height: 1,
            last_height: self.heights.len() as u64,
            max_committed_blocks: self.heights.len() as u64,
            max_store_data_bytes: 4 * 1024 * 1024,
            max_carrier_bytes: 1024 * 1024,
            max_merge_log_bytes: 8 * 1024 * 1024,
            max_merge_frames: 2,
            max_output_bytes: 16 * 1024 * 1024,
            max_decode_allocation_bytes: 64 * 1024 * 1024,
            owner_uid: rustix::process::getuid().as_raw(),
        }
    }
    pub fn open_reader(&self) -> CanonicalKuraEvidenceReader {
        let (blocks, merge) = self.paths();
        CanonicalKuraEvidenceReader::open(&blocks, &merge, self.limits())
            .expect("real archive admission")
    }
    /// Retain this capability and original archive through collection/publication.
    pub fn complete(&self) -> CanonicalKuraEvidenceComplete {
        let mut reader = self.open_reader();
        for height in &self.heights {
            assert_eq!(
                reader
                    .read_carrier(height.block.header().height().get())
                    .unwrap(),
                height.block.encode_wire().unwrap()
            );
        }
        reader
            .scan_merge_entries(&[], |_, _, _| {
                unreachable!("Native archive has no merge entries")
            })
            .unwrap();
        reader
            .finish()
            .expect("all carriers and the complete empty merge log consumed")
    }
}

// Test-only wire projection for authenticated malformed-source controls.
#[derive(norito::Encode, norito::Decode)]
struct RawBlock {
    signatures: BTreeSet<BlockSignature>,
    payload: iroha_data_model::block::BlockPayload,
    result: Option<iroha_data_model::block::BlockResult>,
}

/// An exact ordered SDK response, selected by the test before Client admission.
#[derive(Debug)]
pub(super) struct Reply {
    pub method: Method,
    pub path: String,
    pub media_type: &'static str,
    /// Expected signed request selection, retained separately from mutable response bytes.
    pub expected_query_hash: Option<HashOf<TransactionEntrypoint>>,
    pub body: Vec<u8>,
}
impl Reply {
    /// Exercise the public compatibility handshake using the actual nominal schema.
    pub fn capabilities() -> Self {
        Self {
            method: Method::GET, path: "/v1/node/capabilities".into(), media_type: "application/json", expected_query_hash: None,
            body: norito::json::to_vec(&norito::json!({
                "data_model_version": (iroha_data_model::DATA_MODEL_VERSION),
                "signed_transaction_schema_hash_hex": (hex::encode(norito::schema::identity::frame_hash::<SignedTransaction>())),
            })).unwrap(),
        }
    }
    pub fn finality(proof: &BridgeFinalityProof) -> Self {
        Self {
            method: Method::GET,
            path: iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY
                .path()
                .replace("{height}", &proof.block_header.height().get().to_string()),
            media_type: "application/x-norito",
            expected_query_hash: None,
            body: norito::encode_canonical(proof).unwrap(),
        }
    }
    pub fn details(transaction: CommittedTransaction) -> Self {
        Self {
            method: Method::POST,
            path: iroha_torii_shared::uri::TRANSACTION_DETAILS.into(),
            media_type: "application/x-norito",
            expected_query_hash: Some(transaction.entrypoint_hash),
            body: norito::encode_canonical(
                &iroha_torii_shared::PipelineTransactionDetailsResponse {
                    hash: transaction.entrypoint_hash.to_string(),
                    transaction,
                },
            )
            .unwrap(),
        }
    }
}
/// Decode and authenticate the exact SDK query, including its native payload selectors.
fn validate_details_request(
    request: &TransportRequest,
    expected: HashOf<TransactionEntrypoint>,
) -> Result<()> {
    use iroha_data_model::query::{
        CommittedTxFilters, Query, QueryRequest, SignedQuery,
        dsl::{CompoundPredicate, SelectorTuple},
        parameters::QueryParams,
        transaction::prelude::FindTransactions,
    };
    use iroha_version::codec::DecodeVersioned as _;
    use norito::codec::Decode as _;
    ensure!(
        request.url.query().is_none(),
        "unexpected details URL query"
    );
    ensure!(
        request.max_response_bytes == 64 * 1024 * 1024,
        "details response cap"
    );
    for name in ["accept", "content-type"] {
        let values = request
            .headers
            .iter()
            .filter(|(key, _)| key.as_str().eq_ignore_ascii_case(name))
            .collect::<Vec<_>>();
        ensure!(
            values.len() == 1 && values[0].1.to_str()? == "application/x-norito",
            "details representation header"
        );
    }
    let signed = SignedQuery::decode_all_versioned(&request.body)?;
    signed.verify_signature()?;
    let QueryRequest::Start(query) = signed.request() else {
        return Err(eyre!("details query is not Start"));
    };
    let find = FindTransactions::new();
    ensure!(
        query.query_payload == find.dyn_encode()
            && query.item == find.query_item_kind()
            && query.params == QueryParams::default(),
        "exact details query owner"
    );
    let mut cursor = std::io::Cursor::new(query.predicate_bytes.as_slice());
    let predicate = CompoundPredicate::<CommittedTransaction>::decode(&mut cursor)?;
    ensure!(
        cursor.position() as usize == query.predicate_bytes.len(),
        "predicate trailing bytes"
    );
    ensure!(
        predicate.committed_tx_filters()
            == Some(CommittedTxFilters {
                entry_eq: Some(expected),
                ..CommittedTxFilters::default()
            }),
        "exact queried entrypoint hash"
    );
    let mut cursor = std::io::Cursor::new(query.selector_bytes.as_slice());
    let selector = SelectorTuple::<CommittedTransaction>::decode(&mut cursor)?;
    ensure!(
        cursor.position() as usize == query.selector_bytes.len()
            && selector == SelectorTuple::<CommittedTransaction>::default(),
        "exact details selector"
    );
    Ok(())
}
/// Public HTTP injection with no sockets, retries, fallback responses or test-only SDK API.
#[derive(Debug)]
pub(super) struct ScriptedTransport {
    replies: Mutex<VecDeque<Reply>>,
    consumed: Mutex<usize>,
}
impl ScriptedTransport {
    pub fn new(replies: Vec<Reply>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            consumed: Mutex::new(0),
        })
    }
    pub fn consumed(&self) -> usize {
        *self.consumed.lock().unwrap()
    }
    pub fn assert_drained(&self, expected: usize) {
        assert!(self.replies.lock().unwrap().is_empty());
        assert_eq!(*self.consumed.lock().unwrap(), expected);
    }
}
impl HttpTransport for ScriptedTransport {
    fn send_blocking(&self, request: TransportRequest) -> Result<Response<Vec<u8>>> {
        let reply = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| eyre!("unexpected SDK request"))?;
        *self.consumed.lock().unwrap() += 1;
        ensure!(
            request.method == reply.method,
            "unexpected SDK request method"
        );
        ensure!(
            request.url.path() == reply.path,
            "unexpected SDK request path"
        );
        if let Some(expected) = reply.expected_query_hash {
            validate_details_request(&request, expected)?;
        }
        ensure!(
            reply.body.len() <= request.max_response_bytes,
            "SDK response byte cap"
        );
        Ok(Response::builder()
            .status(200)
            .header("content-type", reply.media_type)
            .body(reply.body)?)
    }
    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move { self.send_blocking(request) })
    }
}

#[cfg(test)]
mod tests;
