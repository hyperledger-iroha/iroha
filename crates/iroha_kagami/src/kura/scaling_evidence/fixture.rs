//! Real four-validator signatures over deliberately explicit offline transcripts.
//!
//! This fixture proves authentication and transcript reconciliation. It does not
//! run State, synthesize a state membership proof, or claim runtime execution of
//! its opaque retained source bundle. All global, merge, lane and READY quorum
//! signatures use exactly three of the same four BLS-normal validators.

use super::*;
use iroha_core::merge::{
    MergeLedgerCandidate, merge_execution_batch_hash, merge_execution_entrypoint_merkle_root,
    merge_execution_result_merkle_root, merge_execution_root, merge_expected_post_state_hash,
    merge_qc_message_digest,
};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    block::{
        BlockExecutionContextBundle, CertifiedMergeLedgerReference,
        builder::BlockBuilder,
        consensus::{
            LaneBlockCommitment, LaneBlockDescriptorV1, LaneBlockQcV1,
            LanePayloadAvailabilityBodyV1, LanePayloadAvailabilityQcV1,
        },
        consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
            QuorumCertificate, ValidatorPower, Vote, finality::V2FinalityArtifact,
        },
    },
    bridge::BRIDGE_FINALITY_PROOF_VERSION_V2,
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterV1,
        KagemushaMintFinalityValidatorKeysV1,
    },
    merge::{MergeExecutionBatch, MergeLaneSignerProof, MergeQuorumCertificate, MergeSignerProof},
    query::CertifiedMergeTransactionInclusion,
    transaction::{FeePaymentIntent, TransactionBuilder, signed::TransactionResult},
    trigger::DataTriggerSequence,
};
use iroha_model_base::peer::PeerId;
use std::num::NonZeroU64;

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
fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(h(
        "canonical workload genesis pin",
    )))
}
fn context(keys: &[KeyPair]) -> HeightContext {
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
        epoch_end_height: 100,
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
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0xA5; 32],
    }
}
fn signed_proof(
    keys: &[KeyPair],
    context: HeightContext,
    block: &SignedBlock,
    merge: Option<HashOf<MergeLedgerEntry>>,
) -> BridgeFinalityProof {
    let wire = block.encode_wire().unwrap();
    let mut execution = ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
        h("pre state"),
        h("post state"),
        h("ordinary writes"),
        wire.len() as u64,
        Hash::new(&wire),
    );
    execution.merge_carrier = merge.map(MergeCarrierCommitmentV1::new);
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
fn binding(index: usize) -> MergeLaneBinding {
    MergeLaneBinding {
        lane_id: LaneId::new(index as u32),
        dataspace_id: DataSpaceId::new(index as u64),
        lane_config_hash: h(&format!("lane config {index}")),
        incarnation: h(&format!("incarnation {index}")),
        activation_height: 1,
    }
}
fn lane(
    keys: &[KeyPair],
    binding: &MergeLaneBinding,
    txs: Vec<TransactionEntrypoint>,
) -> MergeLaneExecution {
    let validators = peers(keys);
    let hashes: Vec<_> = txs.iter().map(|t| Hash::from(t.hash())).collect();
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id: binding.lane_id,
        dataspace_id: binding.dataspace_id,
        lane_incarnation: binding.incarnation,
        proposal_height: 1,
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_height: 1,
        lane_block_view: 0,
        subject_hash: h("lane subject"),
        payload_ownership_hash: h("payload owner"),
        rbc_instance_hash: h("rbc instance"),
        accepted_candidate_indices: (0..txs.len() as u64).collect(),
        accepted_transaction_hashes: hashes.clone(),
        validator_set_hash_version: 1,
        validator_set_hash: HashOf::new(&validators),
        validator_set: validators.clone(),
        validator_count: 4,
        min_quorum: 3,
        qc_mode_tag: "permissioned:canonical-authentication-fixture".to_owned(),
        descriptor_hash: h("unset descriptor"),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: h("unset proposal"),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let d = &proposal.descriptor;
    let payload_hash = h("retained autonomous payload identity");
    let ready = LanePayloadAvailabilityBodyV1 {
        version: 1,
        network_id: network(),
        epoch: 0,
        lane_id: d.lane_id,
        dataspace_id: d.dataspace_id,
        lane_incarnation: d.lane_incarnation,
        proposal_height: 1,
        lane_block_height: 1,
        origin_lane_block_view: 0,
        origin_proposal_hash: proposal.proposal_hash,
        origin_descriptor_hash: d.descriptor_hash,
        current_lane_block_view: 0,
        current_proposal_hash: proposal.proposal_hash,
        current_descriptor_hash: d.descriptor_hash,
        current_subject_hash: d.subject_hash,
        current_payload_ownership_hash: d.payload_ownership_hash,
        current_rbc_instance_hash: d.rbc_instance_hash,
        executable_payload_hash: payload_hash,
        validator_set_hash_version: 1,
        validator_set_hash: d.validator_set_hash,
        validator_count: 4,
        min_quorum: 3,
        qc_mode_tag: d.qc_mode_tag.clone(),
    };
    let ready_qc = LanePayloadAvailabilityQcV1 {
        body: ready.clone(),
        validator_set_hash_version: 1,
        validator_set_hash: d.validator_set_hash,
        validator_set: validators.clone(),
        validator_set_pops: pops(keys),
        signers_bitmap: vec![7],
        bls_aggregate_signature: aggregate(keys, &ready.signature_preimage()),
    };
    let qc = |phase| {
        let body = proposal.vote_body(phase);
        LaneBlockQcV1 {
            bls_aggregate_signature: aggregate(keys, &body.signature_preimage()),
            body,
            validator_set_hash_version: 1,
            validator_set_hash: d.validator_set_hash,
            validator_set: validators.clone(),
            signers_bitmap: vec![7],
            payload_availability_qc: (phase == CertPhase::Prepare).then(|| ready_qc.clone()),
        }
    };
    let routing = RoutingPlan::single(RoutingDecision::new(d.lane_id, d.dataspace_id));
    let reservations: Vec<_> = txs
        .iter()
        .map(|tx| {
            norito::encode_canonical(&LaneQueueReservationKeyV1 {
                version: 1,
                entrypoint_hash: tx.hash(),
                queue_plan_admission_binding_hash: h("signed queue admission binding"),
                routing_plan_digest: routing.digest(),
                coordinator_leg: routing.coordinator_leg(),
                lane_id: d.lane_id,
                dataspace_id: d.dataspace_id,
                lane_incarnation: d.lane_incarnation,
                proposal_height: 1,
                lane_block_height: 1,
                lane_block_view: 0,
                reservation_owner_hash: h("reservation owner"),
                proposal_identity_hash: proposal.proposal_hash,
            })
            .unwrap()
        })
        .collect();
    let results: Vec<_> = txs
        .iter()
        .map(|_| TransactionResult::from(Ok(DataTriggerSequence::default())))
        .collect();
    let settlement = LaneBlockCommitment {
        block_height: 1,
        lane_id: d.lane_id,
        lane_incarnation: d.lane_incarnation,
        dataspace_id: d.dataspace_id,
        tx_count: txs.len() as u64,
        total_local_amount: 0u32.into(),
        total_xor_due: 0u32.into(),
        total_xor_after_haircut: 0u32.into(),
        total_xor_variance: 0u32.into(),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    // Authentication consumer treats source bytes as globally authenticated opaque
    // evidence. Runtime producer/State replay is a separate owner and fixture.
    let source_bundle = norito::encode_canonical(&txs).unwrap();
    MergeLaneExecution {
        source_bundle_hash: Hash::new_from_chunks(&[
            b"iroha:nexus:autonomous-lane-merge-bundle:v1\0",
            &source_bundle,
        ]),
        source_bundle,
        prepare_qc: qc(CertPhase::Prepare),
        commit_qc: qc(CertPhase::Commit),
        signer_proofs: keys
            .iter()
            .take(3)
            .map(|k| MergeLaneSignerProof {
                public_key: k.public_key().clone(),
                proof_of_possession: iroha_crypto::bls_normal_pop_prove(k.private_key()).unwrap(),
            })
            .collect(),
        autonomous_network_id: network(),
        autonomous_epoch: 0,
        autonomous_payload_hash: payload_hash,
        entrypoint_hashes: hashes,
        authenticated_signed_replay_aliases: vec![None; txs.len()],
        reservation_keys: reservations,
        routing_plans: vec![norito::encode_canonical(&routing).unwrap(); txs.len()],
        native_amx_receipts: vec![None; txs.len()],
        result_hashes: results.iter().map(|r| Hash::from(r.hash())).collect(),
        entrypoints: txs,
        results,
        settlement_hash: HashOf::new(&settlement),
        settlement_commitment: settlement,
        origin_proposal: proposal.clone(),
        proposal,
        fastpq_transcripts: Default::default(),
    }
}
pub(super) fn rehash_batch(batch: &mut MergeExecutionBatch) {
    batch.entrypoint_count = batch.lanes.iter().map(|l| l.entrypoints.len() as u64).sum();
    batch.entrypoint_merkle_root = merge_execution_entrypoint_merkle_root(&batch.lanes).unwrap();
    batch.result_merkle_root = merge_execution_result_merkle_root(&batch.lanes).unwrap();
    batch.execution_root = merge_execution_root(&batch.lanes);
    batch.expected_post_state_hash = merge_expected_post_state_hash(
        batch.base_state_height,
        batch.base_state_hash,
        batch.write_set_root,
    );
    batch.batch_hash = merge_execution_batch_hash(batch);
}
fn sign_merge(entry: &mut MergeLedgerEntry, keys: &[KeyPair]) {
    let digest = merge_qc_message_digest(
        &network(),
        &MergeLedgerCandidate::from(&*entry),
        1,
        entry.merge_qc.validator_set_hash,
    );
    entry.merge_qc.message_digest = digest;
    entry.merge_qc.aggregate_signature = aggregate(keys, digest.as_ref());
}
pub(super) struct Fixture {
    pub keys: Vec<KeyPair>,
    pub genesis: SignedBlock,
    pub first: BridgeFinalityProof,
    pub carrier: SignedBlock,
    pub second: BridgeFinalityProof,
    pub entry: MergeLedgerEntry,
    pub requests: Vec<(String, SignedTransaction, RoutingDecision, WorkloadPhase)>,
}
impl Fixture {
    pub fn new(lane_count: usize) -> Self {
        Self::with_request_count(lane_count, 8)
    }
    pub fn with_request_count(lane_count: usize, request_count: usize) -> Self {
        assert!((8..=1024).contains(&request_count));
        let keys = keys();
        let genesis = BlockBuilder::new(BlockHeader::new(
            NonZeroU64::new(1).unwrap(),
            None,
            None,
            None,
            0,
            0,
        ))
        .build_with_signature(0, keys[0].private_key());
        let first = signed_proof(&keys, context(&keys), &genesis, None);
        let bindings: Vec<_> = (0..lane_count).map(binding).collect();
        let authorities =
            MergeLaneAuthorityCatalogV1::from_lane_committees(&vec![peers(&keys); lane_count])
                .unwrap();
        let mut requests = Vec::new();
        let mut groups = vec![Vec::new(); lane_count];
        for index in 0..request_count {
            let owner_key =
                KeyPair::try_from_seed(vec![80 + (index % 4) as u8; 32], Algorithm::Ed25519)
                    .unwrap();
            let authority = AccountId::new(owner_key.public_key().clone());
            let logical = format!("{index:064x}");
            let route = RoutingDecision::new(
                bindings[index % lane_count].lane_id,
                bindings[index % lane_count].dataspace_id,
            );
            let tx = TransactionBuilder::new(
                network(),
                authority.clone(),
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_executable(expected_executable(&authority, &logical).unwrap())
            .sign(owner_key.private_key());
            groups[index % lane_count].push(TransactionEntrypoint::External(tx.clone()));
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
        let header = BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            Some(genesis.hash()),
            None,
            None,
            100,
            0,
        );
        let mut batch = MergeExecutionBatch {
            version: 1,
            base_state_height: 1,
            base_state_hash: genesis.hash(),
            application_block_header: header,
            lanes: bindings
                .iter()
                .zip(groups)
                .map(|(b, txs)| lane(&keys, b, txs))
                .collect(),
            entrypoint_count: 0,
            entrypoint_merkle_root: HashOf::from_untyped_unchecked(h("unset entry root")),
            result_merkle_root: HashOf::from_untyped_unchecked(h("unset result root")),
            execution_root: h("unset execution"),
            application_write_set_root: h("application writes"),
            write_set_root: h("full writes"),
            expected_post_state_hash: HashOf::from_untyped_unchecked(h("unset post state")),
            batch_hash: h("unset batch"),
        };
        rehash_batch(&mut batch);
        let validators = peers(&keys);
        let mut entry = MergeLedgerEntry {
            version: 3,
            epoch_id: 1,
            lane_catalog_hash: h("independently pinned catalog"),
            incarnation_root: LaneLifecycleParameterV1::incarnation_root(
                &bindings
                    .iter()
                    .map(|b| LaneLifecycleIncarnationEntry {
                        lane_id: b.lane_id,
                        incarnation: b.incarnation,
                    })
                    .collect::<Vec<_>>(),
            ),
            activation_root: merge_activation_root(&bindings),
            active_lanes: bindings,
            lane_authority_catalog: authorities,
            lane_snapshots: Vec::new(),
            global_state_root: h("global reduction"),
            execution_batch: Some(batch),
            lane_drain_certificates: Vec::new(),
            merge_qc: MergeQuorumCertificate::new(
                0,
                1,
                2,
                genesis.hash(),
                network(),
                1,
                HashOf::new(&validators),
                validators,
                vec![7],
                pops(&keys)
                    .into_iter()
                    .take(3)
                    .enumerate()
                    .map(|(signer, proof_of_possession)| MergeSignerProof {
                        signer: signer as u32,
                        proof_of_possession,
                    })
                    .collect(),
                Vec::new(),
                h("unset merge signature"),
            ),
        };
        sign_merge(&mut entry, &keys);
        let mut fixture = Self {
            keys,
            genesis,
            first: first.clone(),
            carrier: BlockBuilder::new(BlockHeader::new(
                NonZeroU64::new(2).unwrap(),
                Some(first.block_header.hash()),
                None,
                None,
                100,
                0,
            ))
            .build(BTreeSet::new()),
            second: first,
            entry,
            requests,
        };
        fixture.rebuild_carrier();
        fixture
    }
    pub fn rebuild_carrier(&mut self) {
        sign_merge(&mut self.entry, &self.keys);
        let mut builder = BlockBuilder::new(BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            Some(self.genesis.hash()),
            None,
            None,
            100,
            0,
        ));
        builder.set_execution_context(Some(
            BlockExecutionContextBundle::new(Vec::new())
                .with_merge_entry(CertifiedMergeLedgerReference::new(&self.entry)),
        ));
        self.carrier = builder.build_with_signature(0, self.keys[0].private_key());
        self.resign_carrier();
    }
    pub fn resign_carrier(&mut self) {
        self.resign_carrier_with_merge(Some(self.entry.canonical_hash()));
    }
    pub fn resign_carrier_with_merge(&mut self, merge: Option<HashOf<MergeLedgerEntry>>) {
        let mut next = self.first.finality_artifact.height_context.clone();
        next.height = 2;
        next.parent_commit_qc = Some(self.first.finality_artifact.commit_qc.clone());
        self.second = signed_proof(&self.keys, next, &self.carrier, merge);
    }
    pub fn plan(&self) -> TrustedRunPlan {
        TrustedRunPlan {
            network_id: network(),
            first_context: self.first.finality_artifact.context_id(),
            first_height: 1,
            last_height: 2,
            lane_catalog_hash: h("independently pinned catalog"),
            active_lanes: (0..self.entry.active_lanes.len()).map(binding).collect(),
            lane_authorities: MergeLaneAuthorityCatalogV1::from_lane_committees(&vec![
                peers(
                    &self.keys
                );
                self.entry
                    .active_lanes
                    .len()
            ])
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
    pub fn queries(&self) -> Vec<Vec<u8>> {
        let batch = self.entry.execution_batch.as_ref().unwrap();
        let entries: Vec<_> = batch
            .lanes
            .iter()
            .flat_map(|l| l.entrypoints.iter())
            .collect();
        let results: Vec<_> = batch.lanes.iter().flat_map(|l| l.results.iter()).collect();
        let et: MerkleTree<TransactionEntrypoint> = entries.iter().map(|e| e.hash()).collect();
        let rt: MerkleTree<TransactionResult> = results.iter().map(|r| r.hash()).collect();
        entries
            .iter()
            .zip(results)
            .enumerate()
            .map(|(i, (e, r))| {
                norito::encode_canonical(&CommittedTransaction {
                    block_hash: self.carrier.hash(),
                    entrypoint_hash: e.hash(),
                    entrypoint_proof: et.get_proof(i as u32).unwrap(),
                    entrypoint: (*e).clone(),
                    result_hash: r.hash(),
                    result_proof: rt.get_proof(i as u32).unwrap(),
                    result: r.clone(),
                    merge_inclusion: Some(CertifiedMergeTransactionInclusion {
                        version: 1,
                        merge_entry_hash: self.entry.canonical_hash(),
                        merge_epoch_id: self.entry.epoch_id,
                        execution_batch_hash: batch.batch_hash,
                        entrypoint_count: batch.entrypoint_count,
                        entrypoint_merkle_root: batch.entrypoint_merkle_root,
                        result_merkle_root: batch.result_merkle_root,
                    }),
                })
                .unwrap()
            })
            .collect()
    }
    pub fn start(&self, plan: TrustedRunPlan, limits: VerificationLimits) -> ScalingProofVerifier {
        let mut verifier = ScalingProofVerifier::new(plan, limits).unwrap();
        verifier
            .push_height(
                &norito::encode_canonical(&self.first).unwrap(),
                &self.genesis.encode_wire().unwrap(),
                None,
                &[],
            )
            .unwrap();
        verifier
    }
    pub fn push(&self, verifier: &mut ScalingProofVerifier) -> Result<()> {
        let queries = self.queries();
        verifier.push_height(
            &norito::encode_canonical(&self.second).unwrap(),
            &self.carrier.encode_wire().unwrap(),
            Some(&self.entry.canonical_bytes()),
            &queries.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
    }
}
pub(super) fn limits() -> VerificationLimits {
    VerificationLimits {
        admitted_proof_bytes: 4 * 1024 * 1024,
        input_bytes: 3 * 1024 * 1024,
        output_bytes: 1024 * 1024,
        heights: 8,
        requests: 64,
        leaves_per_carrier: 64,
    }
}
