//! Canonical preparation transport codec for independently supplied launch facts.
//!
//! This owner checks the original fact frame, signed schedule, carrier chain and
//! complete native execution transcript, then encodes the request/bundle transports.
//! The same native owner authenticates the original SignedBlockWire carriers, R context
//! proofs and queried input/output inclusions. This codec does not read Kura, retain
//! filesystem authority or publish anything; disk export rechecks the exact originals.
//! The filesystem prepare-pair owner retains the original facts lease through
//! both output publications and the final reply. These buffers alone are neither
//! a prepare publication receipt nor canonical execution authority.

pub(in crate::kura::scaling_evidence::export) mod assemble;

use super::{LimitsV1, PlanV1, RequestV1};
use crate::kura::scaling_evidence::export::{
    HeightInputBinding, admit,
    filesystem::{SuppliedEvidenceBundleV1, SuppliedEvidenceHeightV1},
};
use crate::kura::scaling_evidence::*;
use color_eyre::eyre::{Result, ensure, eyre};

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_kagami::scaling_evidence::PrepareFactsV1")]
struct PrepareFactsV1 {
    version: u16,
    plan: PlanV1,
    limits: LimitsV1,
    heights: Vec<SuppliedEvidenceHeightV1>,
}

/// Independent preparation allocations, including the entire original fact frame.
#[derive(Clone, Copy)]
pub(crate) struct PrepareOutputCaps {
    /// Maximum canonical launcher-request bytes.
    pub(crate) request_bytes: u64,
    /// Maximum canonical supplied-bundle bytes.
    pub(crate) bundle_bytes: u64,
    /// Maximum sum of facts reservation and both output reservations, at most 256 MiB.
    pub(crate) total_bytes: u64,
}
impl PrepareOutputCaps {
    fn admit(self, facts_max_bytes: u64, actual_bytes: usize) -> Result<()> {
        ensure!(
            [
                facts_max_bytes,
                self.request_bytes,
                self.bundle_bytes,
                self.total_bytes
            ]
            .iter()
            .all(|n| (1..=MAX_PROOF_BYTES).contains(n))
                && actual_bytes > 0
                && u64::try_from(actual_bytes)? <= facts_max_bytes,
            "invalid prepare byte allocations"
        );
        ensure!(
            facts_max_bytes
                .checked_add(self.request_bytes)
                .and_then(|n| n.checked_add(self.bundle_bytes))
                .is_some_and(|n| n <= self.total_bytes),
            "prepare aggregate reservation exceeded"
        );
        Ok(())
    }
}

/// Two complete codec outputs, not a filesystem or completed-proof capability.
///
/// Construction is private. The only handoff consumes both buffers together and
/// is restricted to the export implementation, where the filesystem prepare-pair
/// publisher retains the original facts descriptor. No decoded authority is exposed.
pub(in crate::kura::scaling_evidence::export) struct PreparedTransports {
    request: Vec<u8>,
    bundle: Vec<u8>,
}
impl PreparedTransports {
    /// Consume the pair inside the export boundary; this performs no publication.
    pub(in crate::kura::scaling_evidence::export) fn into_buffers(self) -> (Vec<u8>, Vec<u8>) {
        (self.request, self.bundle)
    }
}

/// Decode one raw-pinned canonical facts frame and count/encode its exact outputs.
///
/// The caller supplies independent facts and caps. A valid frame does not make
/// those inputs trustworthy; there is no proof-derived roster or authority path.
pub(in crate::kura::scaling_evidence::export) fn prepare(
    bytes: &[u8],
    expected_sha256: [u8; 32],
    facts_max_bytes: u64,
    caps: PrepareOutputCaps,
) -> Result<PreparedTransports> {
    caps.admit(facts_max_bytes, bytes.len())?;
    ensure!(
        iroha_crypto::sha256(bytes) == expected_sha256,
        "prepare facts digest mismatch"
    );
    let standard = norito::canonical_decode_limits(bytes.len());
    // Preserve the canonical codec's cumulative allocation accounting, including
    // alignment/sequence plans, under the same independent 512 MiB ceiling as
    // launcher request decoding. File reservations are not a RAM amplification estimate.
    let facts: PrepareFactsV1 = norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            standard.max_sequence_elements(),
            standard.max_field_bytes(),
            standard.max_total_elements(),
            standard
                .max_total_allocated_bytes()
                .min(usize::try_from(MAX_PROOF_BYTES * 2)?),
            128,
        ),
    )?;
    ensure!(facts.version == 1, "unsupported prepare facts version");
    let PrepareFactsV1 {
        plan,
        limits,
        heights,
        ..
    } = facts;
    // Reuse the exact private fixed-width wire conversion. Empty bindings here
    // are temporary codec input; real bindings below derive from original bytes
    // and must pass the existing complete schedule/binding admission.
    let (plan, limits, _) = RequestV1 {
        version: 1,
        plan,
        limits,
        bindings: Vec::new(),
    }
    .into_parts()?;
    let bindings = derive_bindings(&heights, &plan, limits)?;
    // Preparation authenticates the same complete native execution as export.
    // The completed owner returns the original moved launch plan; proof rows
    // never reconstruct its signed requests, routes or trust roots.
    let mut verifier = admit(plan, limits, &bindings)?;
    for height in &heights {
        let queries: Vec<_> = height.queries.iter().map(Vec::as_slice).collect();
        verifier.push_height(&height.carrier, &height.lane_evidence, &queries)?;
    }
    let (completed, plan) = verifier.finish_with_plan()?;
    drop(completed);
    let bundle = SuppliedEvidenceBundleV1 {
        version: 1,
        heights,
    };
    let bundle_count = u64::try_from(norito::canonical_frame_len(&bundle)?)?;
    ensure!(
        bundle_count > 0 && bundle_count <= caps.bundle_bytes,
        "prepare bundle exceeds byte allocation"
    );
    // The existing request encoder runs ordinary admission and counts its real
    // canonical frame before allocating bytes. Neither output reaches a writer.
    let request = super::encode(plan, limits, bindings, caps.request_bytes)?;
    ensure!(
        u64::try_from(bytes.len())?
            .checked_add(u64::try_from(request.len())?)
            .and_then(|n| n.checked_add(bundle_count))
            .is_some_and(|n| n <= caps.total_bytes),
        "prepare actual input/output total exceeded"
    );
    let bundle = norito::encode_canonical(&bundle)?;
    ensure!(
        u64::try_from(bundle.len())? == bundle_count,
        "prepare bundle frame count changed"
    );
    Ok(PreparedTransports { request, bundle })
}

fn derive_bindings(
    heights: &[SuppliedEvidenceHeightV1],
    plan: &TrustedRunPlan,
    limits: VerificationLimits,
) -> Result<Vec<HeightInputBinding>> {
    let count = plan
        .last_height
        .checked_sub(plan.first_height)
        .and_then(|n| n.checked_add(1));
    ensure!(
        (1..=65_536).contains(&limits.heights)
            && plan.first_height > 0
            && plan.last_height < u64::MAX
            && count.is_some_and(
                |n| n <= limits.heights && usize::try_from(n).ok() == Some(heights.len())
            ),
        "prepare height interval is incomplete or unbounded"
    );
    let mut total = 0u64;
    let mut queries = 0usize;
    // Every count and inner byte bound is checked before allocating hash vectors
    // or decoding any carrier/query object.
    for (index, row) in heights.iter().enumerate() {
        ensure!(
            row.height == plan.first_height + u64::try_from(index)?,
            "prepare height order mismatch"
        );
        bounded(&row.carrier, MAX_CARRIER_BYTES)?;
        total = charged(total, row.carrier.len(), limits.input_bytes)?;
        bounded(&row.lane_evidence, MAX_CONTEXT_BYTES)?;
        total = charged(total, row.lane_evidence.len(), limits.input_bytes)?;
        queries = queries
            .checked_add(row.queries.len())
            .ok_or_else(|| eyre!("prepare query count overflow"))?;
        ensure!(
            row.queries.len() <= limits.leaves_per_carrier && queries <= limits.requests,
            "prepare query count exceeds work allocation"
        );
        for query in &row.queries {
            bounded(query, MAX_TRANSACTION_BYTES)?;
            total = charged(total, query.len(), limits.input_bytes)?;
        }
    }
    Ok(heights
        .iter()
        .map(|row| HeightInputBinding {
            height: row.height,
            carrier_hash: Hash::new(&row.carrier),
            lane_evidence_hash: Hash::new(&row.lane_evidence),
            query_hashes: row.queries.iter().map(Hash::new).collect(),
        })
        .collect())
}

#[cfg(test)]
pub(in crate::kura::scaling_evidence::export) mod tests;

// Sole production encoder for the existing facts root. The semantic child owns
// the complete verifier and original authority; no public arbitrary-facts API exists.
fn encode_assembled(
    plan: TrustedRunPlan,
    limits: VerificationLimits,
    heights: Vec<SuppliedEvidenceHeightV1>,
    maximum: u64,
) -> Result<Vec<u8>> {
    ensure!(
        (1..=MAX_PROOF_BYTES).contains(&maximum),
        "invalid facts output cap"
    );
    let RequestV1 { plan, limits, .. } = RequestV1::from_parts(plan, limits, Vec::new())?;
    let facts = PrepareFactsV1 {
        version: 1,
        plan,
        limits,
        heights,
    };
    let count = norito::canonical_frame_len(&facts)?;
    ensure!(
        count > 0 && u64::try_from(count)? <= maximum,
        "facts output cap exceeded"
    );
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let bytes = norito::core::to_bytes_bounded(&facts, usize::try_from(maximum)?)?;
    ensure!(bytes.len() == count, "facts canonical frame count changed");
    Ok(bytes)
}
