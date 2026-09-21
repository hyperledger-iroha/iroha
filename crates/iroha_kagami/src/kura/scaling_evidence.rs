//! Bounded offline authentication of a complete useful multilane workload.
//!
//! Expectations are supplied independently of the evidence. One anchored global
//! verifier survives the entire height interval. No result is exposed before
//! every scheduled request, including warmup, has a successful Native Network output.
//! Account query responses are observed postconditions, not state membership
//! proofs, and intentionally are not accepted as inputs to this owner.
//!
//! The export adapter combines this authentication with the read-only Core owner.
//! The command and retained filesystem owners bind independently selected launch
//! inputs through secure publication and replay; this typed adapter alone does
//! not grant filesystem publication authority or qualify a scaling release.

pub(crate) mod command;
pub(crate) mod export;

#[cfg(test)]
mod fixture;

use std::collections::{BTreeMap, BTreeSet};

use color_eyre::eyre::{Result, ensure, eyre};
use iroha_core::{
    queue::{RoutingDecision, RoutingPlan},
    state::{
        NativeExecutionEvidenceLimits, NativeExecutionEvidenceVerifier,
        VerifiedNativeExecutionCarrier,
    },
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::{
        BlockHeader, SignedBlock, consensus_v2::HeightContextId, decode_versioned_signed_block,
    },
    bridge::BridgeFinalityProof,
    isi::{InstructionBox, SetKeyValue},
    merge::MergeLaneAuthorityCatalogV1,
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

/// Independently pinned Native route and incarnation used by this workload.
/// Physical catalog layout is not asserted by execution evidence.
#[derive(Clone, Debug, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_kagami::scaling_evidence::NativeWorkloadLane")]
pub struct NativeWorkloadLane {
    /// Exact planned lane.
    pub lane_id: LaneId,
    /// Exact planned dataspace.
    pub dataspace_id: DataSpaceId,
    /// Exact incarnation admitted by launch setup.
    pub incarnation: Hash,
    /// Earliest permitted opening under the independently retained launch policy.
    pub activation_height: u64,
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
    /// Independently retained Nexus/AMX execution-context identity.
    pub nexus_amx_context_hash: Hash,
    /// Independently retained deterministic execution policy identity.
    pub execution_policy_hash: Hash,
    /// Exact sorted one- or four-lane lifecycle bindings.
    pub active_lanes: Vec<NativeWorkloadLane>,
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
    /// Maximum Network input and output rows decoded for one global carrier.
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
    /// Exact first finalized admission carrier containing the complete input.
    pub admission_carrier_hash: HashOf<BlockHeader>,
    /// Exact selected input descriptor authenticated by the Native Decision.
    pub input_descriptor_hash: Hash,
    /// Frozen lane instance that authenticated this Decision.
    pub instance_id: Hash,
    /// Network input index, distinct from the typed output index.
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
    native: NativeExecutionEvidenceVerifier,
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
        let native = NativeExecutionEvidenceVerifier::new(
            plan.network_id,
            plan.first_context,
            NativeExecutionEvidenceLimits {
                max_carriers: limits.heights,
                max_carrier_bytes: limits.input_bytes.min(MAX_CARRIER_BYTES as u64),
                max_proof_bytes: limits.input_bytes.min(MAX_FINALITY_BYTES as u64),
                max_retained_bytes: limits.input_bytes,
            },
        )
        .map_err(|error| eyre!(error))?;
        Ok(Self {
            plan,
            limits,
            native,
            expected,
            by_hash,
            rows,
            next_height,
            input_bytes: bytes,
            poisoned: false,
        })
    }

    /// Consume one exact successor with its complete context-write proof.
    /// Queries must cover every Native Network input at this height. Account
    /// observations and ordinary execution cannot substitute for Native Decisions.
    pub fn push_height(
        &mut self,
        finality: &[u8],
        carrier: &[u8],
        contexts: &[u8],
        queries: &[&[u8]],
    ) -> Result<()> {
        ensure!(!self.poisoned, "proof owner is poisoned");
        self.poisoned = true;
        self.consume_height(finality, carrier, contexts, queries)?;
        self.poisoned = false;
        Ok(())
    }

    fn consume_height(
        &mut self,
        finality: &[u8],
        carrier: &[u8],
        contexts: &[u8],
        queries: &[&[u8]],
    ) -> Result<()> {
        ensure!(
            self.next_height <= self.plan.last_height,
            "extra finality height"
        );
        ensure!(
            queries.len() <= self.limits.leaves_per_carrier,
            "too many queried transactions"
        );
        for (bytes, maximum) in [
            (finality, MAX_FINALITY_BYTES),
            (carrier, MAX_CARRIER_BYTES),
            (contexts, MAX_FINALITY_BYTES),
        ] {
            bounded(bytes, maximum)?;
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
        let block = norito::with_decode_limits_scope(decode_limits(carrier.len()), || {
            decode_versioned_signed_block(carrier)
        })?;
        ensure!(
            block.encode_wire()?.as_slice() == carrier,
            "noncanonical carrier wire"
        );
        validate_carrier(&block, self.limits.leaves_per_carrier)?;
        let verified = self
            .native
            .push_height(&proof, block, contexts)
            .map_err(|error| eyre!(error))?;
        let block = verified.block();
        if block
            .execution_context()
            .and_then(|context| context.native_lane_decisions.as_ref())
            .is_some()
        {
            self.consume_native(&verified, &proof, queries)?;
        } else {
            ensure!(
                queries.is_empty(),
                "ordinary fallback is not Native execution evidence"
            );
            let mut unique = BTreeSet::new();
            for hash in block.network_input_hashes() {
                ensure!(unique.insert(hash), "duplicate ordinary entrypoint");
                ensure!(
                    !self.by_hash.contains_key(&hash),
                    "scheduled transaction used ordinary fallback"
                );
            }
        }
        self.next_height += 1;
        Ok(())
    }

    fn consume_native(
        &mut self,
        verified: &VerifiedNativeExecutionCarrier,
        proof: &BridgeFinalityProof,
        queries: &[&[u8]],
    ) -> Result<()> {
        let block = verified.block();
        let batch = block
            .execution_context()
            .and_then(|context| context.native_lane_decisions.as_deref())
            .ok_or_else(|| eyre!("missing Native Decision batch"))?;
        ensure!(
            batch.groups.len() == queries.len(),
            "missing or extra Native query"
        );
        let mut unique = BTreeSet::new();
        for (index, (group, bytes)) in batch.groups.iter().zip(queries).enumerate() {
            let input = &group.payload.input;
            let entrypoint = &input.entrypoint;
            ensure!(
                unique.insert(entrypoint.hash()),
                "duplicate carrier entrypoint"
            );
            let queried: CommittedTransaction = canonical(bytes)?;
            ensure!(
                queried.verify_inclusion_in_authenticated_execution(
                    block,
                    &proof.finality_artifact.commit_qc.execution_commitment
                ) && usize::try_from(queried.entrypoint_proof.leaf_index())? == index
                    && queried.entrypoint == *entrypoint,
                "queried leaf is not this exact typed Network output"
            );
            let expected_index = *self
                .by_hash
                .get(&entrypoint.hash())
                .ok_or_else(|| eyre!("extra unscheduled Native entrypoint"))?;
            ensure!(
                self.rows[expected_index].is_none(),
                "scheduled request executed more than once"
            );
            let expected = &self.expected[expected_index];
            let TransactionEntrypoint::External(signed) = entrypoint else {
                return Err(eyre!("nonexternal workload entrypoint"));
            };
            signed.verify_signature()?;
            let result = queried.result();
            ensure!(
                norito::encode_canonical(signed)? == expected.request.signed_transaction
                    && signed == &expected.transaction
                    && result.0.is_ok()
                    && result.1.is_empty(),
                "scheduled signed request failed or changed"
            );
            ensure!(
                group.payload.descriptor.slots.len() == 1 && group.decisions.len() == 1,
                "workload requires one coordinator route"
            );
            let slot = &group.payload.descriptor.slots[0];
            let frozen = verified
                .decision_context(slot.instance_id)
                .ok_or_else(|| eyre!("missing exact Decision authority"))?;
            let route = RoutingDecision::new(slot.route.lane_id, slot.route.dataspace_id);
            ensure!(
                expected.request.route == route
                    && input.routing_plan().map_err(|error| eyre!(error))?
                        == RoutingPlan::single(route),
                "actual route differs from planned single coordinator route"
            );
            let position = self
                .plan
                .active_lanes
                .iter()
                .position(|binding| binding.lane_id == route.lane_id)
                .ok_or_else(|| eyre!("undeclared execution lane"))?;
            let binding = &self.plan.active_lanes[position];
            let roster = self.plan.lane_authorities.roster_for_lane(position)?;
            ensure!(
                frozen.network_id == self.plan.network_id
                    && frozen.dataspace_id == binding.dataspace_id
                    && frozen.lane_incarnation == binding.incarnation
                    && frozen.opening_global_height >= binding.activation_height
                    && frozen.opening_global_height < self.next_height
                    && frozen.committee == roster.validators
                    && HashOf::new(&frozen.committee) == roster.validator_set_hash
                    && roster.validator_set_hash_version == 1
                    && frozen.nexus_amx_context_hash == self.plan.nexus_amx_context_hash
                    && frozen.execution_policy_hash == self.plan.execution_policy_hash,
                "Native authority/activation differs from launch plan"
            );
            let row = AuthenticatedRequest {
                logical_id: expected.request.logical_id.clone(),
                phase: expected.request.phase,
                authority: signed.authority().clone(),
                entrypoint_hash: entrypoint.hash(),
                carrier_height: self.next_height,
                carrier_hash: block.hash(),
                admission_carrier_hash: group.payload.descriptor.admission_carrier_hash,
                input_descriptor_hash: group
                    .payload
                    .descriptor
                    .canonical_hash()
                    .map_err(|error| eyre!(error))?,
                instance_id: slot.instance_id,
                leaf_index: u32::try_from(index)?,
                lane_id: route.lane_id,
                dataspace_id: route.dataspace_id,
                incarnation: slot.lane_incarnation,
            };
            ensure!(
                u64::try_from(norito::encode_canonical(&row)?.len())? <= ROW_RESERVATION,
                "authenticated row exceeds reserved output"
            );
            self.rows[expected_index] = Some(row);
        }
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
    ensure!(block.has_results(), "carrier has no executed outputs");
    block
        .validate_proposal_commitments()
        .map_err(|error| eyre!(error))?;
    block.validate_output_merkle_cache()?;
    ensure!(
        block.network_entrypoint_count() <= maximum && block.execution_outputs().len() <= maximum,
        "carrier input/output budget exceeded"
    );
    Ok(())
}

#[cfg(test)]
#[path = "scaling_evidence/fixture.rs"]
mod fixture;

#[cfg(test)]
#[path = "scaling_evidence/tests.rs"]
mod tests;
