//! Original signed genesis, actual lane admission/QCs and global economic execution.
//!
//! Positive bytes are captured from production StateExecutor/Kura and LaneExecutor/store
//! owners. Negative helpers mutate certified claims only after execution and never provide
//! a positive workload capture. This proves same-World lane merging, not cross-dataspace AMX.

use super::*;
use iroha_core::{
    query::native_context_archive::NativeContextArchive,
    state::{AllocationBudget, NativeLaneStateProjectionV1},
    sumeragi::test_chain::CertifiedTestChain,
};
use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::{
    block::{BlockSignature, CommitCertificate},
    sumeragi_finality::ExecutionResultCommitment,
};
use iroha_sumeragi::{
    message::{BlockHeader as NativeHeader, Qc},
    types::AggregateSignature,
};
use norito::codec::{DecodeAll as _, Encode as _};
use std::{num::NonZeroUsize, sync::Arc};

pub(crate) mod producer;
use super::lane_proof::LaneFrameV1;
use producer::{Deferred, LaneProducer};

pub(super) fn h(label: &str) -> Hash {
    Hash::new(label.as_bytes())
}

#[derive(Clone)]
pub(super) struct Height {
    pub block: SignedBlock,
    pub lane_evidence: LaneMergeEvidenceV1,
    pub evidence: Vec<u8>,
}
impl Height {
    pub(super) fn capture(
        chain: &CertifiedTestChain,
        height: u64,
        frames: Vec<LaneFrameV1>,
    ) -> Self {
        let committed = chain.committed(height);
        let block = committed.block().as_ref().clone();
        let archive = NativeContextArchive::open_read_only(
            chain.kura().store_root(),
            AllocationBudget::new(MAX_CONTEXT_BYTES),
            NonZeroUsize::new(MAX_CONTEXT_BYTES).unwrap(),
        )
        .unwrap();
        let original = archive.read_exact(height, block.hash()).unwrap();
        let state: NativeLaneStateProjectionV1 = canonical(original.as_slice()).unwrap();
        assert_eq!(state.carrier_hash, block.hash());
        assert_eq!(state.carrier_height, height);
        assert!(
            committed
                .commitment()
                .native_lanes
                .matches_state(chain.network_id(), height, &state.lanes)
                .unwrap()
        );
        archive.recheck_namespace().unwrap();
        let lane_evidence = LaneMergeEvidenceV1 { state, frames };
        let evidence = norito::encode_canonical(&lane_evidence).unwrap();
        Self {
            block,
            lane_evidence,
            evidence,
        }
    }
    pub fn commitment(&self) -> ExecutionResultCommitment {
        canonical(self.block.commit_certificate().unwrap().result_preimage()).unwrap()
    }
    pub fn queries(&self) -> Vec<Vec<u8>> {
        if self.block.header().height().get() == 1 {
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
            &self.block.encode_wire().unwrap(),
            &self.evidence,
            &queries.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
    }
    pub fn refresh_evidence(&mut self) {
        self.lane_evidence.state.carrier_height = self.block.header().height().get();
        self.lane_evidence.state.carrier_hash = self.block.hash();
        self.evidence = norito::encode_canonical(&self.lane_evidence).unwrap();
    }
    pub fn native_parts(&self) -> (NativeHeader, Qc) {
        iroha_core::sumeragi::block_store::decode_certificate(
            self.block.commit_certificate().unwrap(),
        )
        .unwrap()
    }
    pub fn change_certificate(
        &mut self,
        change: impl FnOnce(&mut NativeHeader, &mut Qc, &mut ExecutionResultCommitment),
    ) {
        let (mut header, mut qc) = self.native_parts();
        let mut result = self.commitment();
        change(&mut header, &mut qc, &mut result);
        self.block
            .set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
                norito::encode_canonical(&header).unwrap(),
                norito::encode_canonical(&qc).unwrap(),
                norito::encode_canonical(&result).unwrap(),
            )));
    }
    // Negative control only. Original state claims remain unchanged, while exact wire and
    // transaction roots are recomputed so semantic output/source checks still run after QC.
    pub fn resign(&mut self, keys: &[KeyPair]) {
        let (mut header, mut qc) = self.native_parts();
        let mut result = self.commitment();
        assert!(
            !header.attest && !qc.attest,
            "negative controls cannot forge Pasta custody"
        );
        let mut executed = self.block.clone();
        executed.set_commit_certificate(None);
        let wire = executed.encode_wire().unwrap();
        result.execution.executed_block_wire_len = wire.len() as u64;
        result.execution.executed_block_wire_hash = Hash::new(&wire);
        result.execution.transaction_input_commitment =
            self.block.network_input_merkle_commitment();
        result.execution.transaction_output_commitment = self.block.output_merkle_commitment();
        let payload = self
            .block
            .canonical_resultless_proposal()
            .encode_wire()
            .unwrap();
        let crypto = iroha_core::sumeragi::crypto::BlsCrypto::new();
        header.payload_hash = iroha_sumeragi::preimage::payload_hash(&crypto, &payload);
        header.payload_len = payload.len().try_into().unwrap();
        qc.block_hash = header.hash(&crypto);
        qc.result = result.result().unwrap();
        let shares = keys
            .iter()
            .take(3)
            .map(|key| {
                Signature::new(key.private_key(), &qc.preimage())
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        qc.agg_sig = AggregateSignature(
            iroha_crypto::bls_normal_aggregate_signatures(
                &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .unwrap()
            .try_into()
            .unwrap(),
        );
        self.block
            .set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
                norito::encode_canonical(&header).unwrap(),
                norito::encode_canonical(&qc).unwrap(),
                result.preimage().unwrap(),
            )));
        self.refresh_evidence();
    }
}

pub(super) struct Fixture {
    pub keys: Vec<KeyPair>,
    pub heights: Vec<Height>,
    pub lane_count: usize,
    pub chain: CertifiedTestChain,
    pub requests: Vec<(String, SignedTransaction, RoutingDecision, WorkloadPhase)>,
    policy: SumeragiLanePolicy,
}
impl Fixture {
    pub fn new(lane_count: usize) -> Self {
        Self::with_request_count(lane_count, 8)
    }
    pub fn with_request_count(lane_count: usize, request_count: usize) -> Self {
        Self::seeded(lane_count, request_count, false)
    }
    pub fn global_rescue(lane_count: usize) -> Self {
        Self::seeded(lane_count, 8, true)
    }
    fn seeded(lane_count: usize, request_count: usize, rescue: bool) -> Self {
        assert!(matches!(lane_count, 1 | 4));
        assert!((8..=1024).contains(&request_count));
        let producer = LaneProducer::start(lane_count);
        let requests = (0..request_count)
            .map(|index| {
                let logical = format!("{index:064x}");
                let transaction = producer.request(index % producer.users.len(), &logical);
                (
                    logical,
                    transaction,
                    RoutingDecision::new(
                        LaneId::new((index % lane_count) as u32),
                        DataSpaceId::UNIVERSAL,
                    ),
                    if index < 4 {
                        WorkloadPhase::Warmup
                    } else {
                        WorkloadPhase::Measurement
                    },
                )
            })
            .collect();
        Self::from_requests(producer, requests, rescue)
    }
    pub fn from_generated_genesis(
        chain: CertifiedTestChain,
        deferred: Arc<Deferred>,
        keys: Vec<KeyPair>,
        authority: &crate::genesis::StagedNativeGenesis,
        scheduled: &[ScheduledRequest],
    ) -> Self {
        assert_eq!(chain.genesis().hash(), authority.genesis().hash());
        assert_eq!(
            keys.iter()
                .map(|key| iroha_model_base::peer::PeerId::new(key.public_key().clone()))
                .collect::<Vec<_>>(),
            authority
                .epoch()
                .committee
                .iter()
                .map(|member| member.validator.clone())
                .collect::<Vec<_>>()
        );
        let producer = LaneProducer::from_chain(
            chain,
            deferred,
            authority.lane_policy().unwrap().clone(),
            Vec::new(),
            keys,
        );
        let requests = scheduled
            .iter()
            .map(|row| {
                (
                    row.logical_id.clone(),
                    canonical(&row.signed_transaction).unwrap(),
                    row.route,
                    row.phase,
                )
            })
            .collect();
        Self::from_requests(producer, requests, false)
    }
    fn from_requests(
        mut producer: LaneProducer,
        requests: Vec<(String, SignedTransaction, RoutingDecision, WorkloadPhase)>,
        rescue: bool,
    ) -> Self {
        let lane_count = producer.policy.fixed.len() + 1;
        let mut heights = (1..=3)
            .map(|height| Height::capture(&producer.chain, height, Vec::new()))
            .collect::<Vec<_>>();
        let mut offsets = vec![0; lane_count];
        let pending = (0..lane_count)
            .map(|lane| {
                requests
                    .iter()
                    .filter(|row| row.2.lane_id.as_u32() as usize == lane)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        loop {
            let mut direct = Vec::new();
            let mut frames = Vec::new();
            let mut any = false;
            for lane in 0..lane_count {
                if let Some(row) = pending[lane].get(offsets[lane]) {
                    any = true;
                    offsets[lane] += 1;
                    if lane == 0 || rescue {
                        direct.push(row.1.clone());
                    } else {
                        frames.push(producer.certify(lane, vec![row.1.clone()]));
                    }
                }
            }
            if !any {
                break;
            }
            assert!(producer.chain.commit(direct).into_iter().all(|ok| ok));
            let height = Height::capture(&producer.chain, producer.chain.height(), frames);
            for index in 0..height.block.network_entrypoint_count() {
                assert!(
                    height
                        .block
                        .network_output_at(index as u32)
                        .unwrap()
                        .1
                        .result
                        .is_ok(),
                    "actual global execution must succeed"
                );
            }
            heights.push(height);
        }
        Self {
            keys: producer.keys,
            heights,
            lane_count,
            chain: producer.chain,
            requests,
            policy: producer.policy,
        }
    }
    pub fn plan(&self) -> TrustedRunPlan {
        TrustedRunPlan {
            network_id: self.chain.network_id(),
            chain_id: self.chain.state().chain_id_ref().clone(),
            genesis_epoch_context_id: iroha_data_model::sumeragi_finality::genesis_epoch(
                self.chain.genesis(),
            )
            .unwrap()
            .context_id()
            .unwrap(),
            first_height: 1,
            last_height: self.heights.len() as u64,
            lane_policy: self.policy.clone(),
            active_lanes: (0..self.lane_count)
                .map(|lane| NativeWorkloadLane {
                    lane_id: LaneId::new(lane as u32),
                    dataspace_id: DataSpaceId::UNIVERSAL,
                })
                .collect(),
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
        for height in &self.heights[..3] {
            height.push(&mut verifier).unwrap();
        }
        verifier
    }
    pub fn push(&self, verifier: &mut ScalingProofVerifier) -> Result<()> {
        for height in &self.heights[3..] {
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
        heights: 1027,
        requests: 1024,
        leaves_per_carrier: 1024,
    }
}
// Exact test-only raw projection; every mutation is decoded back through the real model.
#[derive(norito::Encode, norito::Decode)]
pub(super) struct RawBlock {
    pub signatures: BTreeSet<BlockSignature>,
    pub payload: iroha_data_model::block::BlockPayload,
    pub result: Option<iroha_data_model::block::BlockResult>,
    pub commit_certificate: Option<CommitCertificate>,
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
    // Native proposals are unsigned: the sole native certificate authenticates their exact wire.
    raw.signatures.clear();
    height.block = SignedBlock::decode_all(&mut raw.encode().as_slice()).unwrap();
    height.resign(keys);
}

pub(super) fn attach_outputs(
    block: &mut SignedBlock,
    outputs: Vec<iroha_data_model::block::execution_output::ExecutionOutputV1>,
    _keys: &[KeyPair],
) {
    let certificate = block.commit_certificate().cloned();
    block.set_commit_certificate(None);
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
            &iroha_data_model::block::output_budget::ExecutionOutputLimits {
                max_outputs: 1024,
                max_output_bytes: 1024 * 1024,
                max_total_output_bytes: 4 * 1024 * 1024,
                max_executed_wire_bytes: 32 * 1024 * 1024,
            },
        )
        .unwrap();
    block.set_commit_certificate(certificate);
}

pub(super) fn resign_claimed_subject(height: &mut Height, keys: &[KeyPair]) {
    height.change_certificate(|header, qc, _| {
        qc.block_hash = header.hash(&iroha_core::sumeragi::crypto::BlsCrypto::new());
        qc.agg_sig = AggregateSignature(aggregate(keys, &qc.preimage()).try_into().unwrap());
    });
}

fn aggregate(keys: &[KeyPair], message: &[u8]) -> Vec<u8> {
    let shares = keys
        .iter()
        .take(3)
        .map(|key| {
            Signature::new(key.private_key(), message)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    iroha_crypto::bls_normal_aggregate_signatures(
        &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .unwrap()
}
