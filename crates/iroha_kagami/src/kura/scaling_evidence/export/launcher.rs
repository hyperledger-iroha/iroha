//! One bounded first-release request carrying independently retained launch facts.
//!
//! The caller owns the expected raw SHA-256 and byte admission. Neither a proof
//! artifact nor this request can provide its own trust pin. Decoding invokes the
//! ordinary schedule/binding admission; it creates no completed-proof authority.
//! TODO: connect this codec to retained request-file handles and the pinned CLI.

use super::*;

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_kagami::scaling_evidence::LauncherRequestV1")]
struct RequestV1 {
    version: u16,
    plan: PlanV1,
    limits: LimitsV1,
    bindings: Vec<BindingV1>,
}

#[derive(norito::Encode, norito::Decode)]
struct PlanV1 {
    network_id: NetworkId,
    first_context: HeightContextId,
    first_height: u64,
    last_height: u64,
    lane_catalog_hash: Hash,
    active_lanes: Vec<MergeLaneBinding>,
    lane_authorities: MergeLaneAuthorityCatalogV1,
    scheduled: Vec<ScheduledV1>,
}

#[derive(norito::Encode, norito::Decode)]
struct ScheduledV1 {
    logical_id: String,
    phase: WorkloadPhase,
    signed_transaction: Vec<u8>,
    route: RoutingDecision,
}

// Fixed-width wire integers do not depend on the host pointer width.
#[derive(norito::Encode, norito::Decode)]
struct LimitsV1 {
    admitted_proof_bytes: u64,
    input_bytes: u64,
    output_bytes: u64,
    heights: u64,
    requests: u64,
    leaves_per_carrier: u64,
}

#[derive(norito::Encode, norito::Decode)]
struct BindingV1 {
    height: u64,
    finality_hash: Hash,
    query_hashes: Vec<Hash>,
}

/// An externally digest-bound request accepted by the normal plan admission.
///
/// This value supplies caller authority, not verified execution. Export/replay
/// must independently authenticate every supplied height and exact signed effect.
pub(crate) struct BoundLauncherRequest {
    plan: TrustedRunPlan,
    limits: VerificationLimits,
    bindings: Vec<HeightInputBinding>,
}

impl BoundLauncherRequest {
    /// Move the admitted launch facts into the concrete export or replay owner.
    pub(crate) fn into_parts(
        self,
    ) -> (TrustedRunPlan, VerificationLimits, Vec<HeightInputBinding>) {
        (self.plan, self.limits, self.bindings)
    }
}

/// Encode an independently constructed launcher request with a complete preflight.
///
/// Canonical frame counting precedes allocation. Input facts are admitted by the
/// existing authentication owner; this function accepts no proof-derived facts.
pub(crate) fn encode(
    plan: TrustedRunPlan,
    limits: VerificationLimits,
    bindings: Vec<HeightInputBinding>,
    max_bytes: u64,
) -> Result<Vec<u8>> {
    ensure!(
        (1..=MAX_PROOF_BYTES).contains(&max_bytes),
        "invalid launcher byte admission"
    );
    let plan = admit_plan(plan, limits, &bindings)?;
    let request = RequestV1::from_parts(plan, limits, bindings)?;
    let count = u64::try_from(norito::canonical_frame_len(&request)?)?;
    ensure!(
        count > 0 && count <= max_bytes,
        "launcher request exceeds byte admission"
    );
    let encoded = norito::encode_canonical(&request)?;
    ensure!(
        u64::try_from(encoded.len())? == count,
        "launcher frame count changed"
    );
    Ok(encoded)
}

/// Decode exactly one uncompressed canonical V1 request under caller-owned bounds.
///
/// The independent raw digest is checked before any Norito decoding. Bare
/// payloads, other layouts, trailers and unknown versions have no fallback.
pub(crate) fn decode(
    bytes: &[u8],
    expected_sha256: [u8; 32],
    max_bytes: u64,
) -> Result<BoundLauncherRequest> {
    ensure!(
        (1..=MAX_PROOF_BYTES).contains(&max_bytes)
            && !bytes.is_empty()
            && u64::try_from(bytes.len())? <= max_bytes,
        "invalid launcher byte admission"
    );
    ensure!(
        iroha_crypto::sha256(bytes) == expected_sha256,
        "launcher request digest mismatch"
    );
    let standard = norito::canonical_decode_limits(bytes.len());
    // Cumulative decoder accounting charges alignment copies, sequence plans
    // and owned fields without refunding dropped temporaries. Preserve Norito's
    // frame-derived allowance; the independent file cap is not an allocation
    // amplification estimate. The global ceiling remains 512 MiB.
    let allocation = usize::try_from(MAX_PROOF_BYTES * 2)?;
    let request: RequestV1 = norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            standard.max_sequence_elements(),
            standard.max_field_bytes(),
            standard.max_total_elements(),
            standard.max_total_allocated_bytes().min(allocation),
            128,
        ),
    )?;
    let (plan, limits, bindings) = request.into_parts()?;
    let plan = admit_plan(plan, limits, &bindings)?;
    Ok(BoundLauncherRequest {
        plan,
        limits,
        bindings,
    })
}

// The authenticator moves schedule entries into Expected during admission.
// Return those exact moved requests, in order, without cloning or reconstructing
// signed bytes from evidence. No height has been consumed or proof completed.
fn admit_plan(
    plan: TrustedRunPlan,
    limits: VerificationLimits,
    bindings: &[HeightInputBinding],
) -> Result<TrustedRunPlan> {
    let ScalingProofVerifier {
        mut plan, expected, ..
    } = super::admit(plan, limits, bindings)?;
    plan.scheduled = expected.into_iter().map(|entry| entry.request).collect();
    Ok(plan)
}

impl RequestV1 {
    fn from_parts(
        plan: TrustedRunPlan,
        limits: VerificationLimits,
        bindings: Vec<HeightInputBinding>,
    ) -> Result<Self> {
        Ok(Self {
            version: 1,
            plan: PlanV1 {
                network_id: plan.network_id,
                first_context: plan.first_context,
                first_height: plan.first_height,
                last_height: plan.last_height,
                lane_catalog_hash: plan.lane_catalog_hash,
                active_lanes: plan.active_lanes,
                lane_authorities: plan.lane_authorities,
                scheduled: plan
                    .scheduled
                    .into_iter()
                    .map(|s| ScheduledV1 {
                        logical_id: s.logical_id,
                        phase: s.phase,
                        signed_transaction: s.signed_transaction,
                        route: s.route,
                    })
                    .collect(),
            },
            limits: LimitsV1 {
                admitted_proof_bytes: limits.admitted_proof_bytes,
                input_bytes: limits.input_bytes,
                output_bytes: limits.output_bytes,
                heights: limits.heights,
                requests: u64::try_from(limits.requests)?,
                leaves_per_carrier: u64::try_from(limits.leaves_per_carrier)?,
            },
            bindings: bindings
                .into_iter()
                .map(|b| BindingV1 {
                    height: b.height,
                    finality_hash: b.finality_hash,
                    query_hashes: b.query_hashes,
                })
                .collect(),
        })
    }

    fn into_parts(self) -> Result<(TrustedRunPlan, VerificationLimits, Vec<HeightInputBinding>)> {
        ensure!(self.version == 1, "unsupported launcher request version");
        // Validate hard work bounds before narrowing a wire integer to usize.
        ensure!(
            (1..=MAX_REQUESTS as u64).contains(&self.limits.requests)
                && (1..=MAX_REQUESTS as u64).contains(&self.limits.leaves_per_carrier),
            "invalid launcher work bound"
        );
        let limits = VerificationLimits {
            admitted_proof_bytes: self.limits.admitted_proof_bytes,
            input_bytes: self.limits.input_bytes,
            output_bytes: self.limits.output_bytes,
            heights: self.limits.heights,
            requests: usize::try_from(self.limits.requests)?,
            leaves_per_carrier: usize::try_from(self.limits.leaves_per_carrier)?,
        };
        let plan = TrustedRunPlan {
            network_id: self.plan.network_id,
            first_context: self.plan.first_context,
            first_height: self.plan.first_height,
            last_height: self.plan.last_height,
            lane_catalog_hash: self.plan.lane_catalog_hash,
            active_lanes: self.plan.active_lanes,
            lane_authorities: self.plan.lane_authorities,
            scheduled: self
                .plan
                .scheduled
                .into_iter()
                .map(|s| ScheduledRequest {
                    logical_id: s.logical_id,
                    phase: s.phase,
                    signed_transaction: s.signed_transaction,
                    route: s.route,
                })
                .collect(),
        };
        let bindings = self
            .bindings
            .into_iter()
            .map(|b| HeightInputBinding {
                height: b.height,
                finality_hash: b.finality_hash,
                query_hashes: b.query_hashes,
            })
            .collect();
        Ok((plan, limits, bindings))
    }
}

#[cfg(test)]
mod tests;
