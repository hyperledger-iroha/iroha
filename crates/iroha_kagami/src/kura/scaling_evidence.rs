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
mod lane_proof;
pub use lane_proof::AuthenticatedLaneSourceV1;
use lane_proof::{LaneMergeEvidenceV1, LaneProofState};

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
    block::{BlockHeader, SignedBlock, decode_versioned_signed_block},
    isi::{InstructionBox, SetKeyValue},
    query::CommittedTransaction,
    sumeragi_lanes::SumeragiLanePolicy,
    transaction::{Executable, SignedTransaction, signed::TransactionEntrypoint},
};
use iroha_model_base::{
    chain::ChainId,
    topology::{DataSpaceId, LaneId},
};
use iroha_primitives::json::Json;

const MAX_PROOF_BYTES: u64 = 256 * 1024 * 1024;
const MAX_CONTEXT_BYTES: usize = 9 * 1024 * 1024;
const MAX_CARRIER_BYTES: usize = 32 * 1024 * 1024;
const MAX_TRANSACTION_BYTES: usize = 1024 * 1024;
const MAX_REQUESTS: usize = 1_000_000;
const ROW_RESERVATION: u64 = 2048;

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
}

/// Independently supplied launch facts, never learned from a proof roster.
pub struct TrustedRunPlan {
    /// Exact genesis-derived network identity.
    pub network_id: NetworkId,
    /// Independently selected consensus chain identity.
    pub chain_id: ChainId,
    /// Independently authenticated complete native genesis epoch context digest.
    pub genesis_epoch_context_id: [u8; 32],
    /// First height in the required contiguous finality interval.
    pub first_height: u64,
    /// Last height in that interval; a successful prefix is insufficient.
    pub last_height: u64,
    /// Exact original signed genesis lane policy, independently retained by launch setup.
    pub lane_policy: SumeragiLanePolicy,
    /// Exact sorted one- or four-lane lifecycle bindings.
    pub active_lanes: Vec<NativeWorkloadLane>,
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
    /// Original certified lane batch source; absent only for lane zero's direct global input.
    pub lane_source: Option<AuthenticatedLaneSourceV1>,
    /// Network input index, distinct from the typed output index.
    pub leaf_index: u32,
    /// Actual executed lane.
    pub lane_id: LaneId,
    /// Actual executed dataspace.
    pub dataspace_id: DataSpaceId,
}

/// Complete authenticated run; construction is private to successful `finish`.
pub struct AuthenticatedRun {
    rows: Vec<AuthenticatedRequest>,
    canonical: Vec<u8>,
    input_bytes: u64,
}
impl AuthenticatedRun {
    /// Complete rows in independently supplied schedule order.
    #[cfg(test)]
    pub fn rows(&self) -> &[AuthenticatedRequest] {
        &self.rows
    }
    /// Canonical bounded result bytes; these are not a replacement for proof inputs.
    #[cfg(test)]
    pub fn canonical_rows(&self) -> &[u8] {
        &self.canonical
    }
    /// Total exact canonical input bytes consumed.
    #[cfg(test)]
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
    lane_proofs: LaneProofState,
    pending_genesis: Option<(
        SignedBlock,
        iroha_data_model::sumeragi_lanes::SumeragiLaneState,
    )>,
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
            plan.first_height == 1
                && plan.last_height >= 2
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
        plan.lane_policy.validate()?;
        ensure!(
            plan.lane_policy.fixed.len() <= 64
                && plan
                    .lane_policy
                    .fixed
                    .iter()
                    .all(|fixed| fixed.committee.len() <= 64),
            "unbounded lane policy"
        );
        for binding in &plan.active_lanes {
            if binding.lane_id.as_u32() == 0 {
                ensure!(
                    binding.dataspace_id.as_u64() == 0,
                    "global lane has another dataspace"
                );
            } else {
                let fixed = plan
                    .lane_policy
                    .fixed_lane(binding.lane_id)
                    .ok_or_else(|| eyre!("workload lane is absent from pinned fixed policy"))?;
                ensure!(
                    fixed.dataspace == binding.dataspace_id,
                    "workload dataspace differs from pinned policy"
                );
            }
        }
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
            plan.chain_id.clone(),
            plan.network_id,
            NativeExecutionEvidenceLimits {
                max_carriers: limits.heights,
                max_carrier_bytes: limits.input_bytes.min(MAX_CARRIER_BYTES as u64),
                max_context_bytes: limits.input_bytes.min(MAX_CONTEXT_BYTES as u64),
                max_retained_bytes: limits.input_bytes,
            },
        )
        .map_err(|error| eyre!(error))?;
        Ok(Self {
            plan,
            limits,
            native,
            lane_proofs: LaneProofState::default(),
            pending_genesis: None,
            expected,
            by_hash,
            rows,
            next_height,
            input_bytes: bytes,
            poisoned: false,
        })
    }

    /// Consume one exact successor with its complete lane-state and original lane-frame proof.
    /// Queries cover every actual Network input; scheduled nonzero-lane work must
    /// occur in the authenticated merged suffix, and lane zero uses the global input.
    pub fn push_height(
        &mut self,
        carrier: &[u8],
        lane_evidence: &[u8],
        queries: &[&[u8]],
    ) -> Result<()> {
        ensure!(!self.poisoned, "proof owner is poisoned");
        self.poisoned = true;
        self.consume_height(carrier, lane_evidence, queries)?;
        self.poisoned = false;
        Ok(())
    }

    fn consume_height(
        &mut self,
        carrier: &[u8],
        lane_evidence: &[u8],
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
            (carrier, MAX_CARRIER_BYTES),
            (lane_evidence, MAX_CONTEXT_BYTES),
        ] {
            bounded(bytes, maximum)?;
            self.input_bytes = charged(self.input_bytes, bytes.len(), self.limits.input_bytes)?;
        }
        for bytes in queries {
            bounded(bytes, MAX_TRANSACTION_BYTES)?;
            self.input_bytes = charged(self.input_bytes, bytes.len(), self.limits.input_bytes)?;
        }
        let block = norito::with_decode_limits_scope(decode_limits(carrier.len()), || {
            decode_versioned_signed_block(carrier)
        })?;
        ensure!(
            block.encode_wire()?.as_slice() == carrier,
            "noncanonical carrier wire"
        );
        ensure!(
            block.header().height().get() == self.next_height,
            "noncontiguous carrier height"
        );
        validate_carrier(&block, self.limits.leaves_per_carrier)?;
        if self.next_height == 1 {
            let epoch = iroha_data_model::sumeragi_finality::genesis_epoch(&block)
                .map_err(|error| eyre!(error))?;
            ensure!(
                epoch.context_id().map_err(|error| eyre!(error))?
                    == self.plan.genesis_epoch_context_id,
                "genesis native epoch differs from independently retained launch authority"
            );
            ensure!(
                lane_proof::signed_genesis_policy(&block)? == self.plan.lane_policy,
                "signed genesis lane policy differs from independently retained launch plan"
            );
            ensure!(
                queries.is_empty() && block.lane_merge().is_none(),
                "genesis cannot supply workload outputs or lane merges"
            );
        }
        let evidence: LaneMergeEvidenceV1 = canonical(lane_evidence)?;
        ensure!(
            evidence.frames.len() <= self.limits.leaves_per_carrier,
            "lane frame work bound exceeded"
        );
        let state_bytes = norito::encode_canonical(&evidence.state)?;
        if self.next_height == 1 {
            ensure!(evidence.frames.is_empty(), "genesis has lane frames");
            // This retained value is quarantined until NativeExecutionEvidenceVerifier
            // authenticates the original H1 result with the actual H2 certificate.
            self.pending_genesis = Some((block.clone(), evidence.state.lanes.clone()));
        }
        let Some(verified) = self
            .native
            .push_height(block, &state_bytes)
            .map_err(|error| eyre!(error))?
        else {
            ensure!(
                self.next_height == 1,
                "only genesis may await its successor anchor"
            );
            self.next_height += 1;
            return Ok(());
        };
        if let Some((genesis, lanes)) = self.pending_genesis.take() {
            self.lane_proofs.anchor_genesis(&genesis, lanes)?;
        }
        let sources = self.lane_proofs.verify(
            verified.block(),
            verified.lanes(),
            &evidence.frames,
            &self.plan.lane_policy,
            self.plan.network_id,
            &self.plan.chain_id,
        )?;
        self.consume_network(&verified, queries, sources)?;
        self.next_height += 1;
        Ok(())
    }

    fn consume_network(
        &mut self,
        verified: &VerifiedNativeExecutionCarrier,
        queries: &[&[u8]],
        mut sources: BTreeMap<usize, lane_proof::ProvenSource>,
    ) -> Result<()> {
        let block = verified.block();
        ensure!(
            block.network_entrypoint_count() == queries.len(),
            "missing or extra Network query"
        );
        let mut unique = BTreeSet::new();
        for (index, (entrypoint, bytes)) in block.network_entrypoints().zip(queries).enumerate() {
            ensure!(
                unique.insert(entrypoint.hash()),
                "duplicate carrier entrypoint"
            );
            let queried: CommittedTransaction = canonical(bytes)?;
            ensure!(
                queried.verify_inclusion_in_block(block)
                    && usize::try_from(queried.entrypoint_proof.leaf_index())? == index
                    && queried.entrypoint == *entrypoint,
                "queried leaf is not this exact typed Network output"
            );
            let source = sources.remove(&index);
            let Some(&expected_index) = self.by_hash.get(&entrypoint.hash()) else {
                ensure!(source.is_none(), "extra unscheduled merged entrypoint");
                if let TransactionEntrypoint::External(tx) = entrypoint
                    && let Executable::Instructions(instructions) = tx.instructions()
                {
                    ensure!(!instructions.iter().any(|instruction| matches!(
                        instruction.as_any().downcast_ref::<iroha_data_model::isi::SetKeyValueBox>(),
                        Some(iroha_data_model::isi::SetKeyValueBox::Account(set)) if set.key.as_ref().starts_with("gscale_")
                    )), "extra unscheduled workload effect");
                }
                continue;
            };
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
            let route = expected.request.route;
            let context = block
                .execution_context()
                .and_then(|bundle| {
                    bundle
                        .external
                        .iter()
                        .find(|context| context.entrypoint_hash == entrypoint.hash())
                })
                .ok_or_else(|| eyre!("scheduled transaction lacks its actual execution context"))?;
            ensure!(
                context
                    == &iroha_data_model::block::ExternalExecutionContext::new(
                        entrypoint.hash(),
                        route.lane_id,
                        route.dataspace_id
                    ),
                "actual execution context differs from the independently planned single route"
            );
            let lane_source = if route.lane_id.as_u32() == 0 {
                ensure!(
                    source.is_none(),
                    "global lane cannot claim a separate lane instance"
                );
                None
            } else {
                let source = source.ok_or_else(|| {
                    eyre!(
                        "scheduled lane work used global rescue instead of a certified lane merge"
                    )
                })?;
                ensure!(
                    source.lane == route.lane_id && source.dataspace == route.dataspace_id,
                    "original lane source differs from the actual scheduled route"
                );
                Some(source.source)
            };
            let row = AuthenticatedRequest {
                logical_id: expected.request.logical_id.clone(),
                phase: expected.request.phase,
                authority: signed.authority().clone(),
                entrypoint_hash: entrypoint.hash(),
                carrier_height: self.next_height,
                carrier_hash: block.hash(),
                lane_source,
                leaf_index: u32::try_from(index)?,
                lane_id: route.lane_id,
                dataspace_id: route.dataspace_id,
            };
            ensure!(
                u64::try_from(norito::encode_canonical(&row)?.len())? <= ROW_RESERVATION,
                "authenticated row exceeds reserved output"
            );
            self.rows[expected_index] = Some(row);
        }
        ensure!(sources.is_empty(), "unmatched original merged source");
        Ok(())
    }

    /// Complete exactly the independently selected interval and successful cohort.
    ///
    /// Admission rejection, execution rejection, missing transactions and missing
    /// lanes fail this qualification; the caller must not filter such trace rows.
    pub fn finish(self) -> Result<AuthenticatedRun> {
        self.finish_with_plan().map(|(run, _plan)| run)
    }

    /// Complete the same proof boundary while returning its original launch authority.
    /// Retained scheduled requests are moved back without replacing or reconstructing them.
    pub(crate) fn finish_with_plan(mut self) -> Result<(AuthenticatedRun, TrustedRunPlan)> {
        ensure!(
            !self.poisoned
                && self.pending_genesis.is_none()
                && self.next_height == self.plan.last_height + 1,
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
        self.plan.scheduled = self
            .expected
            .into_iter()
            .map(|expected| expected.request)
            .collect();
        Ok((
            AuthenticatedRun {
                rows,
                canonical,
                input_bytes: self.input_bytes,
            },
            self.plan,
        ))
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
#[allow(dead_code, reason = "fixture is shared by focused test suites")]
#[path = "scaling_evidence/fixture.rs"]
mod fixture;

#[cfg(test)]
#[path = "scaling_evidence/tests.rs"]
mod tests;
