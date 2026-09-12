//! Bounded offline authentication of a complete useful multilane workload.
//!
//! Expectations are supplied independently of the evidence. One anchored global
//! verifier survives the entire height interval. No result is exposed before
//! every scheduled request, including warmup, has a successful merged result.
//! Account query responses are observed postconditions, not state membership
//! proofs, and intentionally are not accepted as inputs to this owner.
//!
//! The export adapter combines this authentication with the read-only Core owner.
//! TODO: connect independently retained launcher inputs and secure CLI publication;
//! the typed adapter does not grant filesystem publication authority.

pub(crate) mod export;

use std::collections::{BTreeMap, BTreeSet};

use color_eyre::eyre::{Result, ensure, eyre};
use iroha_core::{
    merge::{
        merge_activation_root, merge_application_header_from_carrier,
        merge_execution_batch_commitments_match,
    },
    queue::{LaneQueueReservationKeyV1, RouteLegRole, RoutingDecision, RoutingPlan},
};
use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::{
        BlockHeader, SignedBlock,
        consensus::{CertPhase, LaneBlockProposalV1},
        consensus_v2::{HeightContextId, MergeCarrierCommitmentV1},
        decode_versioned_signed_block,
    },
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    isi::{InstructionBox, SetKeyValue},
    merge::{
        MAX_MERGE_LEDGER_ENTRY_BYTES, MergeLaneAuthorityCatalogV1, MergeLaneBinding,
        MergeLaneExecution, MergeLedgerEntry,
    },
    nexus::{LaneLifecycleIncarnationEntry, LaneLifecycleParameterV1},
    query::CommittedTransaction,
    transaction::{Executable, SignedTransaction, signed::TransactionEntrypoint},
};
use iroha_model_base::topology::{DataSpaceId, LaneId};
use iroha_primitives::json::Json;

const MAX_PROOF_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FINALITY_BYTES: usize = 9 * 1024 * 1024;
const MAX_CARRIER_BYTES: usize = 32 * 1024 * 1024;
const MAX_TRANSACTION_BYTES: usize = 1024 * 1024;
const MAX_REQUESTS: usize = 1_000_000;
const ROW_RESERVATION: u64 = 1024;

/// The two phases are independent of untrusted proof timestamps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
pub enum WorkloadPhase {
    /// Work offered before the measurement origin; still required to succeed.
    Warmup,
    /// Work offered during the measurement window.
    Measurement,
}

/// One independently retained signed request and its expected route.
pub struct ScheduledRequest {
    /// Canonical lowercase 64-character logical request identity.
    pub logical_id: String,
    /// Independently selected phase; no rejected-request exemption exists.
    pub phase: WorkloadPhase,
    /// Canonical framed Norito signed transaction retained before submission.
    pub signed_transaction: Vec<u8>,
    /// Exact lane/dataspace selected by the trusted routing setup.
    pub route: RoutingDecision,
}

/// Independently supplied launch facts, never learned from a proof roster.
pub struct TrustedRunPlan {
    /// Exact genesis-derived network identity.
    pub network_id: NetworkId,
    /// Independently authenticated context at `first_height`.
    pub first_context: HeightContextId,
    /// First height in the required contiguous finality interval.
    pub first_height: u64,
    /// Last height in that interval; a successful prefix is insufficient.
    pub last_height: u64,
    /// Exact active catalog identity retained by launch admission.
    pub lane_catalog_hash: Hash,
    /// Exact sorted one- or four-lane lifecycle bindings.
    pub active_lanes: Vec<MergeLaneBinding>,
    /// Exact committee catalog retained independently from deployment state.
    pub lane_authorities: MergeLaneAuthorityCatalogV1,
    /// Complete scheduled cohort in logical offer order, including warmup.
    pub scheduled: Vec<ScheduledRequest>,
}

/// All limits are required and checked before decoding or retaining proof data.
#[derive(Clone, Copy)]
pub struct VerificationLimits {
    /// Actual independently admitted per-run canonical-proof file allocation.
    pub admitted_proof_bytes: u64,
    /// Maximum cumulative canonical inputs, including retained signed requests.
    pub input_bytes: u64,
    /// Reserved canonical result bytes, including framing and one row per request.
    pub output_bytes: u64,
    /// Maximum contiguous global heights.
    pub heights: u64,
    /// Maximum scheduled requests.
    pub requests: usize,
    /// Maximum ordinary plus merged leaves decoded for one global carrier.
    pub leaves_per_carrier: usize,
}

/// Authenticated execution evidence for one exact scheduled request.
#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_kagami::scaling_evidence::AuthenticatedRequestV1")]
pub struct AuthenticatedRequest {
    /// Independently supplied logical identity matched to the exact instruction.
    pub logical_id: String,
    /// Independently supplied scheduled phase.
    pub phase: WorkloadPhase,
    /// Signed transaction authority, always a universal AccountId.
    pub authority: AccountId,
    /// Exact canonical entrypoint hash.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Globally certified application carrier height.
    pub carrier_height: u64,
    /// Globally certified application carrier header identity.
    pub carrier_hash: HashOf<BlockHeader>,
    /// Full merge-entry hash, authenticated by both reference and CommitQC.
    pub merge_entry_hash: HashOf<MergeLedgerEntry>,
    /// Exact certified merge epoch.
    pub merge_epoch: u64,
    /// Aligned entrypoint/result index in the full merge transcript.
    pub leaf_index: u32,
    /// Actual executed lane.
    pub lane_id: LaneId,
    /// Actual executed dataspace.
    pub dataspace_id: DataSpaceId,
    /// Actual executed incarnation.
    pub incarnation: Hash,
}

/// Complete authenticated run; construction is private to successful `finish`.
pub struct AuthenticatedRun {
    rows: Vec<AuthenticatedRequest>,
    canonical: Vec<u8>,
    input_bytes: u64,
}
impl AuthenticatedRun {
    /// Complete rows in independently supplied schedule order.
    pub fn rows(&self) -> &[AuthenticatedRequest] {
        &self.rows
    }
    /// Canonical bounded result bytes; these are not a replacement for proof inputs.
    pub fn canonical_rows(&self) -> &[u8] {
        &self.canonical
    }
    /// Total exact canonical input bytes consumed.
    pub fn input_bytes(&self) -> u64 {
        self.input_bytes
    }
}

struct Expected {
    request: ScheduledRequest,
    transaction: SignedTransaction,
}

/// Move-owned, fail-closed global chain and useful-effect authenticator.
pub struct ScalingProofVerifier {
    plan: TrustedRunPlan,
    limits: VerificationLimits,
    finality: BridgeFinalityVerifier,
    expected: Vec<Expected>,
    by_hash: BTreeMap<HashOf<TransactionEntrypoint>, usize>,
    rows: Vec<Option<AuthenticatedRequest>>,
    next_height: u64,
    input_bytes: u64,
    poisoned: bool,
}
impl ScalingProofVerifier {
    /// Validate the independently supplied bounded plan before consuming evidence.
    pub fn new(mut plan: TrustedRunPlan, limits: VerificationLimits) -> Result<Self> {
        ensure!(
            (1..=MAX_PROOF_BYTES).contains(&limits.admitted_proof_bytes),
            "invalid proof allocation"
        );
        ensure!(
            limits.input_bytes > 0
                && limits.output_bytes > 0
                && limits
                    .input_bytes
                    .checked_add(limits.output_bytes)
                    .is_some_and(|n| n <= limits.admitted_proof_bytes),
            "invalid proof reservations"
        );
        ensure!(
            (1..=65_536).contains(&limits.heights)
                && (1..=MAX_REQUESTS).contains(&limits.requests)
                && (1..=MAX_REQUESTS).contains(&limits.leaves_per_carrier),
            "invalid proof work bound"
        );
        let heights = plan
            .last_height
            .checked_sub(plan.first_height)
            .and_then(|n| n.checked_add(1));
        ensure!(
            plan.first_height > 0
                && plan.last_height < u64::MAX
                && heights.is_some_and(|n| n <= limits.heights),
            "invalid finality interval"
        );
        ensure!(
            matches!(plan.active_lanes.len(), 1 | 4)
                && plan
                    .active_lanes
                    .windows(2)
                    .all(|w| w[0].lane_id < w[1].lane_id),
            "invalid active lane geometry"
        );
        ensure!(
            plan.lane_authorities.rosters.len() <= 4
                && plan
                    .lane_authorities
                    .rosters
                    .iter()
                    .all(|r| r.validators.len() <= 64),
            "unbounded committee plan"
        );
        plan.lane_authorities
            .validate_for_active_lanes(plan.active_lanes.len())?;
        ensure!(
            !plan.scheduled.is_empty() && plan.scheduled.len() <= limits.requests,
            "invalid scheduled cohort count"
        );
        let reserved_output = u64::try_from(plan.scheduled.len())?
            .checked_mul(ROW_RESERVATION)
            .and_then(|n| n.checked_add(1024))
            .ok_or_else(|| eyre!("row reservation overflow"))?;
        ensure!(
            reserved_output <= limits.output_bytes,
            "insufficient result reservation"
        );
        let mut bytes = 0u64;
        for row in &plan.scheduled {
            ensure!(
                row.logical_id.len() == 64
                    && row
                        .logical_id
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "invalid logical identity"
            );
            bounded(&row.signed_transaction, MAX_TRANSACTION_BYTES)?;
            bytes = charged(bytes, row.signed_transaction.len(), limits.input_bytes)?;
            ensure!(
                plan.active_lanes
                    .iter()
                    .any(|b| b.lane_id == row.route.lane_id
                        && b.dataspace_id == row.route.dataspace_id),
                "scheduled route outside active lanes"
            );
        }
        let mut by_hash = BTreeMap::new();
        let mut logical = BTreeSet::new();
        let mut expected = Vec::with_capacity(plan.scheduled.len());
        for request in std::mem::take(&mut plan.scheduled) {
            ensure!(
                logical.insert(request.logical_id.clone()),
                "duplicate logical request"
            );
            let transaction: SignedTransaction = canonical(&request.signed_transaction)?;
            transaction.verify_signature()?;
            ensure!(
                transaction.network_id() == Some(&plan.network_id),
                "scheduled transaction network mismatch"
            );
            ensure!(
                transaction.instructions()
                    == &expected_executable(transaction.authority(), &request.logical_id)?,
                "scheduled request is not the exact useful effect"
            );
            ensure!(
                by_hash
                    .insert(transaction.hash_as_entrypoint(), expected.len())
                    .is_none(),
                "duplicate signed request"
            );
            expected.push(Expected {
                request,
                transaction,
            });
        }
        let rows = (0..expected.len()).map(|_| None).collect();
        let next_height = plan.first_height;
        let finality = BridgeFinalityVerifier::with_context(plan.network_id, plan.first_context);
        Ok(Self {
            plan,
            limits,
            finality,
            expected,
            by_hash,
            rows,
            next_height,
            input_bytes: bytes,
            poisoned: false,
        })
    }

    /// Consume one exact immediate successor; any error or unwind poisons this owner.
    ///
    /// Queries are canonical CommittedTransaction encodings for the entire merged
    /// cohort at this height. No ordinary-inclusion or unsigned-summary path exists.
    pub fn push_height(
        &mut self,
        finality: &[u8],
        carrier: &[u8],
        merge_entry: Option<&[u8]>,
        queries: &[&[u8]],
    ) -> Result<()> {
        ensure!(!self.poisoned, "proof owner is poisoned");
        self.poisoned = true;
        self.consume_height(finality, carrier, merge_entry, queries)?;
        self.poisoned = false;
        Ok(())
    }

    fn consume_height(
        &mut self,
        finality: &[u8],
        carrier: &[u8],
        merge_entry: Option<&[u8]>,
        queries: &[&[u8]],
    ) -> Result<()> {
        ensure!(
            self.next_height <= self.plan.last_height,
            "extra finality height"
        );
        ensure!(
            queries.len() <= self.limits.requests,
            "too many queried transactions"
        );
        for (bytes, max) in [(finality, MAX_FINALITY_BYTES), (carrier, MAX_CARRIER_BYTES)] {
            bounded(bytes, max)?;
            self.input_bytes = charged(self.input_bytes, bytes.len(), self.limits.input_bytes)?;
        }
        if let Some(bytes) = merge_entry {
            bounded(bytes, MAX_MERGE_LEDGER_ENTRY_BYTES)?;
            self.input_bytes = charged(self.input_bytes, bytes.len(), self.limits.input_bytes)?;
        }
        for bytes in queries {
            bounded(bytes, MAX_TRANSACTION_BYTES)?;
            self.input_bytes = charged(self.input_bytes, bytes.len(), self.limits.input_bytes)?;
        }
        let proof: BridgeFinalityProof = canonical(finality)?;
        ensure!(
            proof.block_header.height().get() == self.next_height,
            "noncontiguous carrier height"
        );
        self.finality.verify(&proof)?;
        let block = norito::with_decode_limits_scope(decode_limits(carrier.len()), || {
            decode_versioned_signed_block(carrier)
        })?;
        ensure!(
            block.encode_wire()?.as_slice() == carrier,
            "noncanonical carrier wire"
        );
        proof
            .finality_artifact
            .validate_for_header(&block.header())?;
        let commitment = &proof.finality_artifact.commit_qc.execution_commitment;
        ensure!(
            block.header() == proof.block_header
                && block.executed_block_wire_hash()? == commitment.executed_block_wire_hash
                && u64::try_from(carrier.len())? == commitment.executed_block_wire_len,
            "executed carrier commitment mismatch"
        );
        validate_carrier(&block, self.limits.leaves_per_carrier)?;
        let ordinary: BTreeSet<_> = block.entrypoint_hashes().collect();
        ensure!(
            ordinary.len() == block.entrypoint_hashes().len(),
            "duplicate ordinary entrypoint"
        );
        for hash in &ordinary {
            ensure!(
                !self.by_hash.contains_key(hash),
                "scheduled transaction used ordinary fallback"
            );
        }
        let reference = block
            .execution_context()
            .and_then(|c| c.merge_entry.as_ref());
        match (reference, merge_entry) {
            (None, None) => ensure!(
                commitment.merge_carrier.is_none() && queries.is_empty(),
                "ordinary fallback or missing merge reference"
            ),
            (Some(reference), Some(bytes)) => {
                let entry: MergeLedgerEntry = canonical(bytes)?;
                ensure!(
                    entry.version == MergeLedgerEntry::VERSION
                        && reference.matches_entry(&entry)
                        && commitment.merge_carrier
                            == Some(MergeCarrierCommitmentV1::new(entry.canonical_hash())),
                    "full merge reference or CommitQC mismatch"
                );
                let qc = &entry.merge_qc;
                ensure!(
                    qc.network_id == self.plan.network_id
                        && qc.carrier_height == self.next_height
                        && Some(qc.carrier_parent_hash) == block.header().prev_block_hash()
                        && qc.view == block.header().view_change_index()
                        && qc.epoch_id == entry.epoch_id,
                    "merge QC carrier identity mismatch"
                );
                self.consume_entry(&block, &entry, queries)?;
            }
            _ => return Err(eyre!("missing or extra full merge entry")),
        }
        self.next_height += 1;
        Ok(())
    }

    fn consume_entry(
        &mut self,
        block: &SignedBlock,
        entry: &MergeLedgerEntry,
        queries: &[&[u8]],
    ) -> Result<()> {
        ensure!(
            entry.active_lanes == self.plan.active_lanes
                && entry.lane_catalog_hash == self.plan.lane_catalog_hash
                && entry.lane_authority_catalog == self.plan.lane_authorities,
            "historical route authority differs from launch plan"
        );
        let incarnations: Vec<_> = entry
            .active_lanes
            .iter()
            .map(|b| LaneLifecycleIncarnationEntry {
                lane_id: b.lane_id,
                incarnation: b.incarnation,
            })
            .collect();
        ensure!(
            entry.activation_root == merge_activation_root(&entry.active_lanes)
                && entry.incarnation_root
                    == LaneLifecycleParameterV1::incarnation_root(&incarnations),
            "lifecycle roots mismatch"
        );
        let Some(batch) = entry.execution_batch.as_ref() else {
            ensure!(queries.is_empty(), "snapshot is not useful execution");
            return Ok(());
        };
        ensure!(
            batch.version == 1
                && !batch.lanes.is_empty()
                && batch.lanes.len() <= self.limits.leaves_per_carrier,
            "invalid lane execution count"
        );
        ensure!(
            batch
                .lanes
                .windows(2)
                .all(|w| lane_order(&w[0]) < lane_order(&w[1])),
            "noncanonical merge lane order"
        );
        let mut count = block.entrypoint_hashes().len();
        let mut unique: BTreeSet<_> = block.entrypoint_hashes().collect();
        ensure!(unique.len() == count, "duplicate ordinary entrypoint");
        for hash in &unique {
            ensure!(
                !self.by_hash.contains_key(hash),
                "scheduled transaction used ordinary fallback"
            );
        }
        for lane in &batch.lanes {
            validate_lane(lane, &self.plan, self.next_height)?;
            count = count
                .checked_add(lane.entrypoints.len())
                .ok_or_else(|| eyre!("leaf count overflow"))?;
            ensure!(
                count <= self.limits.leaves_per_carrier,
                "carrier leaf budget exceeded"
            );
            for entrypoint in &lane.entrypoints {
                ensure!(
                    unique.insert(entrypoint.hash()),
                    "duplicate carrier entrypoint"
                );
            }
        }
        ensure!(
            batch.entrypoint_count == u64::try_from(queries.len())?
                && batch.application_block_header
                    == merge_application_header_from_carrier(&block.header())
                && merge_execution_batch_commitments_match(batch),
            "merge transcript commitments mismatch"
        );
        // Hash the bounded full transcript once per carrier, never once per leaf.
        let merge_entry_hash = entry.canonical_hash();
        let mut flat = batch.lanes.iter().flat_map(|lane| {
            lane.entrypoints
                .iter()
                .zip(&lane.results)
                .map(move |pair| (lane, pair))
        });
        for (index, bytes) in queries.iter().enumerate() {
            let (lane, (entrypoint, result)) = flat
                .next()
                .ok_or_else(|| eyre!("missing aligned merge leaf"))?;
            let queried: CommittedTransaction = canonical(bytes)?;
            ensure!(
                queried.merge_inclusion.is_some()
                    && queried.verify_inclusion_in_block(block)
                    && usize::try_from(queried.entrypoint_proof.leaf_index())? == index
                    && queried.entrypoint == *entrypoint
                    && queried.result == *result,
                "queried leaf is not this exact full transcript leaf"
            );
            let expected_index = *self
                .by_hash
                .get(&entrypoint.hash())
                .ok_or_else(|| eyre!("extra unscheduled merge entrypoint"))?;
            ensure!(
                self.rows[expected_index].is_none(),
                "scheduled request executed more than once"
            );
            let expected = &self.expected[expected_index];
            let TransactionEntrypoint::External(signed) = entrypoint else {
                return Err(eyre!("nonexternal workload entrypoint"));
            };
            signed.verify_signature()?;
            ensure!(
                norito::encode_canonical(signed)? == expected.request.signed_transaction
                    && signed == &expected.transaction
                    && result.0.is_ok()
                    && result.1.is_empty(),
                "scheduled signed request failed or changed"
            );
            let descriptor = &lane.proposal.descriptor;
            ensure!(
                expected.request.route
                    == RoutingDecision::new(descriptor.lane_id, descriptor.dataspace_id),
                "actual route differs from planned route"
            );
            let row = AuthenticatedRequest {
                logical_id: expected.request.logical_id.clone(),
                phase: expected.request.phase,
                authority: signed.authority().clone(),
                entrypoint_hash: entrypoint.hash(),
                carrier_height: self.next_height,
                carrier_hash: block.hash(),
                merge_entry_hash,
                merge_epoch: entry.epoch_id,
                leaf_index: u32::try_from(index)?,
                lane_id: descriptor.lane_id,
                dataspace_id: descriptor.dataspace_id,
                incarnation: descriptor.lane_incarnation,
            };
            // Account controllers are variable-sized. Validate the actual row
            // before retaining it under the fixed per-request reservation.
            ensure!(
                u64::try_from(norito::encode_canonical(&row)?.len())? <= ROW_RESERVATION,
                "authenticated row exceeds reserved output"
            );
            self.rows[expected_index] = Some(row);
        }
        ensure!(flat.next().is_none(), "unqueried merge leaf");
        Ok(())
    }

    /// Complete exactly the independently selected interval and successful cohort.
    ///
    /// Admission rejection, execution rejection, missing transactions and missing
    /// lanes fail this qualification; the caller must not filter such trace rows.
    pub fn finish(self) -> Result<AuthenticatedRun> {
        ensure!(
            !self.poisoned && self.next_height == self.plan.last_height + 1,
            "incomplete or poisoned proof interval"
        );
        let rows = self
            .rows
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| eyre!("missing scheduled successful execution"))?;
        for binding in &self.plan.active_lanes {
            ensure!(
                rows.iter().any(|row| row.lane_id == binding.lane_id),
                "declared active lane has no useful work"
            );
        }
        let canonical = norito::encode_canonical(&rows)?;
        ensure!(
            u64::try_from(canonical.len())? <= self.limits.output_bytes,
            "result output allocation exceeded"
        );
        Ok(AuthenticatedRun {
            rows,
            canonical,
            input_bytes: self.input_bytes,
        })
    }
}

fn charged(current: u64, count: usize, maximum: u64) -> Result<u64> {
    let next = current
        .checked_add(u64::try_from(count)?)
        .ok_or_else(|| eyre!("proof byte count overflow"))?;
    ensure!(next <= maximum, "proof input allocation exceeded");
    Ok(next)
}
fn bounded(bytes: &[u8], maximum: usize) -> Result<()> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= maximum,
        "canonical input exceeds bound"
    );
    Ok(())
}
fn decode_limits(length: usize) -> norito::DecodeLimits {
    let standard = norito::canonical_decode_limits(length);
    norito::DecodeLimits::new(
        standard.max_sequence_elements(),
        standard.max_field_bytes(),
        standard.max_total_elements(),
        standard.max_total_allocated_bytes(),
        128,
    )
}
fn canonical<T>(bytes: &[u8]) -> Result<T>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    Ok(norito::decode_canonical_with_limits(
        bytes,
        decode_limits(bytes.len()),
    )?)
}
fn expected_executable(authority: &AccountId, logical: &str) -> Result<Executable> {
    Ok(Executable::Instructions(
        vec![InstructionBox::from(SetKeyValue::account(
            authority.clone(),
            format!("gscale_{logical}").parse()?,
            Json::try_new(logical)?,
        ))]
        .into(),
    ))
}
fn validate_carrier(block: &SignedBlock, maximum: usize) -> Result<()> {
    ensure!(block.has_results(), "carrier has no executed results");
    block.validate_entrypoint_merkle_cache()?;
    block.validate_result_merkle_cache()?;
    ensure!(
        block.entrypoint_hashes().len() <= maximum
            && block.entrypoint_hashes().len() == block.result_hashes().len(),
        "carrier entrypoint/result alignment mismatch"
    );
    ensure!(
        block.header().merkle_root()
            == MerkleTree::root_from_typed_leaves(
                block.external_entrypoints_cloned().map(|e| e.hash())
            )
            && block.header().result_merkle_root()
                == block.result_merkle_commitment().map(|c| *c.root())
            && block.header().execution_context_hash()
                == block.execution_context().map(HashOf::new),
        "carrier payload context or Merkle commitment mismatch"
    );
    Ok(())
}
fn validate_proposal(proposal: &LaneBlockProposalV1) -> Result<()> {
    ensure!(
        proposal.proposal_hash == proposal.computed_proposal_hash()
            && proposal.descriptor.descriptor_hash
                == proposal.descriptor.computed_descriptor_hash(),
        "lane proposal digest mismatch"
    );
    Ok(())
}
fn validate_lane(
    lane: &MergeLaneExecution,
    plan: &TrustedRunPlan,
    carrier_height: u64,
) -> Result<()> {
    let n = lane.entrypoints.len();
    ensure!(
        n > 0
            && [
                lane.entrypoint_hashes.len(),
                lane.results.len(),
                lane.result_hashes.len(),
                lane.routing_plans.len(),
                lane.reservation_keys.len(),
                lane.native_amx_receipts.len(),
                lane.authenticated_signed_replay_aliases.len()
            ]
            .into_iter()
            .all(|len| len == n),
        "unaligned lane transcript"
    );
    validate_proposal(&lane.proposal)?;
    validate_proposal(&lane.origin_proposal)?;
    let d = &lane.proposal.descriptor;
    let origin = &lane.origin_proposal.descriptor;
    let mut normalized_origin = origin.clone();
    normalized_origin.lane_block_view = d.lane_block_view;
    normalized_origin.descriptor_hash = normalized_origin.computed_descriptor_hash();
    ensure!(
        origin.lane_block_view == 0
            && normalized_origin == *d
            && lane.origin_proposal.payload_block_hint == lane.proposal.payload_block_hint,
        "origin proposal changed immutable fields"
    );
    ensure!(
        !lane.source_bundle.is_empty()
            && lane.source_bundle_hash
                == Hash::new_from_chunks(&[
                    b"iroha:nexus:autonomous-lane-merge-bundle:v1\0",
                    &lane.source_bundle
                ])
            && lane.settlement_hash == HashOf::new(&lane.settlement_commitment),
        "source/settlement hash mismatch"
    );
    let position = plan
        .active_lanes
        .iter()
        .position(|b| b.lane_id == d.lane_id)
        .ok_or_else(|| eyre!("undeclared execution lane"))?;
    let binding = &plan.active_lanes[position];
    let roster = plan.lane_authorities.roster_for_lane(position)?;
    ensure!(
        d.dataspace_id == binding.dataspace_id
            && d.lane_incarnation == binding.incarnation
            && d.proposal_height >= binding.activation_height
            && d.proposal_height < carrier_height
            && d.validator_set == roster.validators
            && d.validator_set_hash == roster.validator_set_hash
            && d.validator_set_hash_version == roster.validator_set_hash_version
            && usize::try_from(d.validator_count)? == roster.validators.len()
            && usize::try_from(d.min_quorum)? == (roster.validators.len() - 1) / 3 * 2 + 1,
        "lane authority/activation mismatch"
    );
    ensure!(
        lane.autonomous_network_id == plan.network_id
            && d.accepted_transaction_hashes == lane.entrypoint_hashes
            && d.accepted_candidate_indices.len() == n
            && d.accepted_candidate_indices.windows(2).all(|w| w[0] < w[1])
            && origin.lane_id == d.lane_id
            && origin.dataspace_id == d.dataspace_id
            && origin.lane_incarnation == d.lane_incarnation
            && origin.proposal_height == d.proposal_height
            && origin.lane_block_height == d.lane_block_height
            && origin.accepted_transaction_hashes == d.accepted_transaction_hashes,
        "lane origin/work identity mismatch"
    );
    ensure!(
        lane.prepare_qc.body == lane.proposal.vote_body(CertPhase::Prepare)
            && lane.commit_qc.body == lane.proposal.vote_body(CertPhase::Commit)
            && lane.prepare_qc.payload_availability_qc.is_some()
            && lane.commit_qc.payload_availability_qc.is_none(),
        "lane QC role/proposal mismatch"
    );
    for qc in [&lane.prepare_qc, &lane.commit_qc] {
        ensure!(
            qc.validator_set == roster.validators
                && qc.validator_set_hash == roster.validator_set_hash
                && qc.validator_set_hash_version == roster.validator_set_hash_version,
            "lane QC roster mismatch"
        );
    }
    for index in 0..n {
        let hash = lane.entrypoints[index].hash();
        ensure!(
            Hash::from(hash) == lane.entrypoint_hashes[index]
                && Hash::from(lane.results[index].hash()) == lane.result_hashes[index]
                && lane.authenticated_signed_replay_aliases[index].is_none()
                && lane.native_amx_receipts[index].is_none(),
            "lane leaf identity/role mismatch"
        );
        let routing: RoutingPlan = canonical(&lane.routing_plans[index])?;
        ensure!(
            routing == RoutingPlan::single(RoutingDecision::new(d.lane_id, d.dataspace_id)),
            "non-single coordinator route"
        );
        let key: LaneQueueReservationKeyV1 = canonical(&lane.reservation_keys[index])?;
        ensure!(
            key.version == LaneQueueReservationKeyV1::VERSION
                && key.entrypoint_hash == hash
                && key.coordinator_leg.role == RouteLegRole::Coordinator
                && key.coordinator_leg == routing.coordinator_leg()
                && key.routing_plan_digest == routing.digest()
                && key.lane_id == d.lane_id
                && key.dataspace_id == d.dataspace_id
                && key.lane_incarnation == d.lane_incarnation
                && key.proposal_height == d.proposal_height
                && key.lane_block_height == d.lane_block_height
                && key.lane_block_view == origin.lane_block_view,
            "reservation/route alignment mismatch"
        );
    }
    Ok(())
}

fn lane_order(lane: &MergeLaneExecution) -> (LaneId, DataSpaceId, Hash, u64, u64, u64, Hash, Hash) {
    let d = &lane.proposal.descriptor;
    (
        d.lane_id,
        d.dataspace_id,
        d.lane_incarnation,
        d.lane_block_height,
        d.proposal_height,
        d.lane_block_view,
        d.descriptor_hash,
        lane.proposal.proposal_hash,
    )
}

#[cfg(test)]
#[path = "scaling_evidence/tests.rs"]
mod tests;
