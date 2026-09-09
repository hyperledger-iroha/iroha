//! Bounded typed ordinary and AXT bundles on the compact one-delta domain.
//!
//! Every segment binds the original complete public context, ordered root chain
//! and its ordinal before challenges. This module never prepares isolated delta
//! slices or reconstructs private witnesses. All segments must verify before a
//! result is returned. The outer canonical frame is a sole final schema.
//!
//! TODO: Qualify aggregate protocol security, resources and authenticated caller
//! integration before production use. This offline bundle does not change any
//! production default or grant source-state authority or finality.

use iroha_data_model::privacy::GoldilocksDigest384V1;
use norito::{DecodeLimits, NoritoDeserialize, NoritoSerialize};

use super::compact_value_domain::CompactTransferValue;
use super::{
    compact_axt_batch::AxtTransferBatch,
    compact_protocol::shared_openings::SharedVerificationWork,
    compact_public_api::{AxtVerificationContext, SharedVerifier},
    compact_public_batch::{BatchContextLimits, PublicTransferBatch},
};
use crate::{
    Error, ProofSemantics, Result, VerifyLimits,
    gadgets::public_transfer_statement::{PreparedPublicTransfers, PublicTransferLimits},
    proof::PublicIO,
};

const VERSION: u16 = 1;
const MAX_DECODE_DEPTH: usize = 16;

/// Explicit cumulative policy in addition to every unchanged per-segment limit.
#[derive(Debug, Clone, Copy)]
pub(super) struct BundleLimits {
    /// Maximum complete deltas, also bounded by public preparation's defaults.
    pub(super) max_segments: usize,
    /// Maximum canonical outer frame bytes, including all nested raw frames.
    pub(super) max_wire_bytes: usize,
    /// Maximum sum of raw segment frame lengths.
    pub(super) max_total_segment_bytes: usize,
    /// Maximum sum of statement bytes absorbed by all segment transcripts.
    pub(super) max_total_statement_bytes: usize,
    /// Maximum cumulative AIR/query count, checked before any child verification.
    pub(super) max_total_queries: usize,
    /// Cumulative Norito allocation charges across outer and all child decodes.
    pub(super) max_total_decode_allocation_charges: usize,
    /// Exact per-segment policy; the bundle cannot multiply this implicitly.
    pub(super) segment: VerifyLimits,
}

#[cfg(test)]
impl Default for BundleLimits {
    fn default() -> Self {
        let segment = VerifyLimits::default();
        Self {
            max_segments: 1,
            max_wire_bytes: segment.max_proof_bytes,
            max_total_segment_bytes: segment.max_proof_bytes,
            max_total_statement_bytes: segment.max_batch_bytes,
            max_total_queries: segment.max_queries,
            max_total_decode_allocation_charges: 32 * 1024 * 1024,
            segment,
        }
    }
}

/// Canonical carrier only; decoding it does not validate any contained proof.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(
    name = "fastpq_prover::backend::compact_bundle::BundleWire",
    frame = "fastpq_prover::compact_v1::OrdinaryTransferBundleV1"
)]
pub(super) struct BundleWire {
    /// Exact bundle format version.
    pub(super) version: u16,
    /// Exactly one shared endpoint between each pair of consecutive proofs.
    pub(super) intermediate_roots: Vec<[u8; 32]>,
    /// Canonical raw shared proof frames, in original delta occurrence order.
    pub(super) segments: Vec<Vec<u8>>,
}

/// Distinct canonical AXT carrier; no caller context is sourced from this frame.
///
/// Although its structural fields match the ordinary carrier, the nominal
/// schema differs. Re-encoding a carrier cannot retag child segment identities.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(
    name = "fastpq_prover::backend::compact_bundle::AxtBundleWire",
    frame = "fastpq_prover::compact_v1::AxtTransferBundleV1"
)]
pub(super) struct AxtBundleWire {
    /// Exact bundle format version.
    pub(super) version: u16,
    /// Ordered claimed roots between chronological segment proofs.
    pub(super) intermediate_roots: Vec<[u8; 32]>,
    /// Canonical AXT segment proof frames in original occurrence order.
    pub(super) segments: Vec<Vec<u8>>,
}

/// All-or-nothing successful relation result, without an authority grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VerifiedBundle {
    public_io: PublicIO,
    segments: usize,
    wire_bytes: usize,
    statement_bytes: usize,
    work: SharedVerificationWork,
    row_roots: Vec<GoldilocksDigest384V1>,
}

impl VerifiedBundle {
    /// Complete authenticated row roots in original segment occurrence order.
    /// The list is published only after every child has verified successfully.
    pub(super) fn row_roots(&self) -> &[GoldilocksDigest384V1] {
        &self.row_roots
    }

    /// Exact independently expected overall public inputs.
    pub(super) const fn public_io(&self) -> PublicIO {
        self.public_io
    }

    /// Number of complete verified delta occurrences.
    pub(super) const fn segments(&self) -> usize {
        self.segments
    }

    /// Actual outer canonical frame length.
    pub(super) const fn wire_bytes(&self) -> usize {
        self.wire_bytes
    }

    /// Total exact canonical statement lengths absorbed across the segments.
    pub(super) const fn statement_bytes(&self) -> usize {
        self.statement_bytes
    }

    /// Sum of measured child verification work, excluding outer frame overhead.
    pub(super) const fn work(&self) -> SharedVerificationWork {
        self.work
    }
}

/// Verify the complete ordered ordinary bundle using only caller public facts.
#[cfg(test)]
pub(super) fn verify_transfer_bundle<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    expected: &PublicIO,
    bytes: &[u8],
    limits: BundleLimits,
) -> Result<VerifiedBundle> {
    verify_transfer_bundle_with(
        prepared,
        expected,
        bytes,
        limits,
        SharedVerifier {
            max_decode_allocation_charges: 32 * 1024 * 1024,
        },
    )
}

fn verify_transfer_bundle_with<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    expected: &PublicIO,
    bytes: &[u8],
    limits: BundleLimits,
    verifier: SharedVerifier,
) -> Result<VerifiedBundle> {
    check_limit("max_bundle_wire_bytes", bytes.len(), limits.max_wire_bytes)?;
    if prepared.semantics() != ProofSemantics::StateTransition {
        return Err(shape(
            "ordinary compact bundles require ordinary transfer semantics",
        ));
    }
    check_limit(
        "max_transitions",
        prepared.transitions().len(),
        limits.segment.max_transitions,
    )?;
    check_limit(
        "max_batch_bytes",
        prepared.work().public_bytes,
        limits.segment.max_batch_bytes,
    )?;
    let count = prepared.pairs().len();
    preflight_count_for(count, limits, verifier)?;
    // This parent scope survives every nested decode. A child scope cannot
    // reset cumulative charges or raise a stricter caller-supplied budget.
    norito::core::with_decode_limits_scope(
        DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            limits.max_total_decode_allocation_charges,
            MAX_DECODE_DEPTH,
        ),
        || {
            let wire = decode_wire_with_policy(bytes, count, limits, verifier)?;
            let batch = PublicTransferBatch::new(
                prepared,
                expected,
                &wire.intermediate_roots,
                BatchContextLimits {
                    max_segments: limits.max_segments,
                    max_total_statement_bytes: limits.max_total_statement_bytes,
                },
            )?;
            check_limit(
                "max_compact_statement_bytes",
                batch.max_statement_bytes(),
                limits.segment.max_batch_bytes,
            )?;
            // The batch constructor counts every segment's complete statement
            // before constructing an AIR or hashing any child proof.
            let mut work = SharedVerificationWork::default();
            // `count` already passed public and cumulative segment/query bounds.
            let mut row_roots = Vec::with_capacity(count);
            for (ordinal, frame) in wire.segments.iter().enumerate() {
                let relation = batch.segment(ordinal)?;
                let child = verifier.verify_frame_committed(&relation, frame, limits.segment)?;
                add_work(&mut work, child.work())?;
                row_roots.push(child.row_root());
            }
            Ok(VerifiedBundle {
                public_io: *expected,
                segments: count,
                wire_bytes: bytes.len(),
                statement_bytes: batch.total_statement_bytes(),
                work,
                row_roots,
            })
        },
    )
}

/// Verify the complete AXT bundle against the full caller-supplied public context.
///
/// The outer carrier never supplies execution authority, AXT binding/mirrors or
/// source endpoints. Every segment statement binds the original whole AXT facts
/// and remote occurrence list; a successful prefix is never returned.
#[cfg(test)]
pub(super) fn verify_axt_transfer_bundle<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    expected: &PublicIO,
    context: AxtVerificationContext<'_>,
    bytes: &[u8],
    limits: BundleLimits,
) -> Result<VerifiedBundle> {
    verify_axt_transfer_bundle_with(
        prepared,
        expected,
        context,
        bytes,
        limits,
        SharedVerifier {
            max_decode_allocation_charges: 32 * 1024 * 1024,
        },
    )
}

fn verify_axt_transfer_bundle_with<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    expected: &PublicIO,
    context: AxtVerificationContext<'_>,
    bytes: &[u8],
    limits: BundleLimits,
    verifier: SharedVerifier,
) -> Result<VerifiedBundle> {
    check_limit("max_bundle_wire_bytes", bytes.len(), limits.max_wire_bytes)?;
    if prepared.semantics() != ProofSemantics::AxtTransferClaim {
        return Err(shape(
            "AXT compact bundles require AXT transfer-claim semantics",
        ));
    }
    check_limit(
        "max_transitions",
        prepared.transitions().len(),
        limits.segment.max_transitions,
    )?;
    check_limit(
        "max_batch_bytes",
        prepared.work().public_bytes,
        limits.segment.max_batch_bytes,
    )?;
    let count = prepared.pairs().len();
    preflight_count_for(count, limits, verifier)?;
    // This parent scope survives every nested decode. A child scope cannot
    // reset cumulative charges or raise a stricter caller-supplied budget.
    norito::core::with_decode_limits_scope(
        DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            limits.max_total_decode_allocation_charges,
            MAX_DECODE_DEPTH,
        ),
        || {
            let wire = decode_axt_wire_with_policy(bytes, count, limits, verifier)?;
            let batch = AxtTransferBatch::new(
                prepared,
                expected,
                &wire.intermediate_roots,
                context,
                BatchContextLimits {
                    max_segments: limits.max_segments,
                    max_total_statement_bytes: limits.max_total_statement_bytes,
                },
            )?;
            check_limit(
                "max_compact_statement_bytes",
                batch.max_statement_bytes(),
                limits.segment.max_batch_bytes,
            )?;
            // The batch constructor counts every segment's complete statement
            // before constructing an AIR or hashing any child proof.
            let mut work = SharedVerificationWork::default();
            // `count` already passed public and cumulative segment/query bounds.
            let mut row_roots = Vec::with_capacity(count);
            for (ordinal, frame) in wire.segments.iter().enumerate() {
                let relation = batch.segment(ordinal)?;
                let child = verifier.verify_frame_committed(&relation, frame, limits.segment)?;
                add_work(&mut work, child.work())?;
                row_roots.push(child.row_root());
            }
            Ok(VerifiedBundle {
                public_io: *expected,
                segments: count,
                wire_bytes: bytes.len(),
                statement_bytes: batch.total_statement_bytes(),
                work,
                row_roots,
            })
        },
    )
}

/// Verify an ordered candidate ordinary bundle with separate child and cumulative
/// decode charges. Every child must pass before returning a verified result.
pub(super) fn verify_transfer_bundle_with_allocation<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    expected: &PublicIO,
    bytes: &[u8],
    limits: BundleLimits,
    max_segment_decode_allocation_charges: usize,
) -> Result<VerifiedBundle> {
    verify_transfer_bundle_with(
        prepared,
        expected,
        bytes,
        limits,
        SharedVerifier {
            max_decode_allocation_charges: max_segment_decode_allocation_charges,
        },
    )
}

/// Verify an ordered candidate AXT bundle with complete caller AXT context.
/// Neither carrier nor successful proof prefixes grant authority or finality.
pub(super) fn verify_axt_transfer_bundle_with_allocation<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    expected: &PublicIO,
    context: AxtVerificationContext<'_>,
    bytes: &[u8],
    limits: BundleLimits,
    max_segment_decode_allocation_charges: usize,
) -> Result<VerifiedBundle> {
    verify_axt_transfer_bundle_with(
        prepared,
        expected,
        context,
        bytes,
        limits,
        SharedVerifier {
            max_decode_allocation_charges: max_segment_decode_allocation_charges,
        },
    )
}
// Normalize only already bounded nominal carrier fields by moving their owned
// tables. No child is decoded, retagged or accepted by these internal conversions.
fn decode_wire_with_policy(
    bytes: &[u8],
    count: usize,
    limits: BundleLimits,
    verifier: SharedVerifier,
) -> Result<BundleWire> {
    let wire: BundleWire = norito::decode_canonical_with_limits(
        bytes,
        wire_decode_limits_for(bytes, count, limits, verifier)?,
    )?;
    preflight_wire_parts_for(
        wire.version,
        &wire.intermediate_roots,
        &wire.segments,
        count,
        limits,
        verifier,
    )?;
    Ok(BundleWire {
        version: wire.version,
        intermediate_roots: wire.intermediate_roots,
        segments: wire.segments,
    })
}

fn decode_axt_wire_with_policy(
    bytes: &[u8],
    count: usize,
    limits: BundleLimits,
    verifier: SharedVerifier,
) -> Result<AxtBundleWire> {
    let wire: AxtBundleWire = norito::decode_canonical_with_limits(
        bytes,
        wire_decode_limits_for(bytes, count, limits, verifier)?,
    )?;
    preflight_wire_parts_for(
        wire.version,
        &wire.intermediate_roots,
        &wire.segments,
        count,
        limits,
        verifier,
    )?;
    Ok(AxtBundleWire {
        version: wire.version,
        intermediate_roots: wire.intermediate_roots,
        segments: wire.segments,
    })
}

/// Serialize a bounded carrier; this helper does not verify its child frames.
pub(super) fn encode_wire(
    wire: &BundleWire,
    expected_count: usize,
    limits: BundleLimits,
) -> Result<Vec<u8>> {
    preflight_wire(wire, expected_count, limits)?;
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let bytes = norito::core::encoded_frame_len(wire)?;
    check_limit("max_bundle_wire_bytes", bytes, limits.max_wire_bytes)?;
    Ok(norito::encode_canonical(wire)?)
}

/// Encode a bounded nominal AXT carrier without verifying its child frames.
pub(super) fn encode_axt_wire(
    wire: &AxtBundleWire,
    expected_count: usize,
    limits: BundleLimits,
) -> Result<Vec<u8>> {
    preflight_wire_parts(
        wire.version,
        &wire.intermediate_roots,
        &wire.segments,
        expected_count,
        limits,
    )?;
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    check_limit(
        "max_bundle_wire_bytes",
        norito::core::encoded_frame_len(wire)?,
        limits.max_wire_bytes,
    )?;
    Ok(norito::encode_canonical(wire)?)
}

#[cfg(test)]
fn decode_wire(bytes: &[u8], expected_count: usize, limits: BundleLimits) -> Result<BundleWire> {
    let wire = norito::decode_canonical_with_limits(
        bytes,
        wire_decode_limits(bytes, expected_count, limits)?,
    )?;
    preflight_wire(&wire, expected_count, limits)?;
    Ok(wire)
}

#[cfg(test)]
fn decode_axt_wire(
    bytes: &[u8],
    expected_count: usize,
    limits: BundleLimits,
) -> Result<AxtBundleWire> {
    let wire: AxtBundleWire = norito::decode_canonical_with_limits(
        bytes,
        wire_decode_limits(bytes, expected_count, limits)?,
    )?;
    preflight_wire_parts(
        wire.version,
        &wire.intermediate_roots,
        &wire.segments,
        expected_count,
        limits,
    )?;
    Ok(wire)
}

#[cfg(test)]
fn wire_decode_limits(
    bytes: &[u8],
    expected_count: usize,
    limits: BundleLimits,
) -> Result<DecodeLimits> {
    wire_decode_limits_for(
        bytes,
        expected_count,
        limits,
        SharedVerifier {
            max_decode_allocation_charges: 32 * 1024 * 1024,
        },
    )
}

fn wire_decode_limits_for(
    bytes: &[u8],
    expected_count: usize,
    limits: BundleLimits,
    verifier: SharedVerifier,
) -> Result<DecodeLimits> {
    // The raw byte ceiling precedes even public geometry checks and CRC work.
    check_limit("max_bundle_wire_bytes", bytes.len(), limits.max_wire_bytes)?;
    preflight_count_for(expected_count, limits, verifier)?;
    let overhead_elements = expected_count
        .checked_mul(2)
        .and_then(|value| value.checked_sub(1))
        .ok_or_else(|| shape("bundle decode element count overflow"))?;
    let total_elements = limits
        .max_total_segment_bytes
        .checked_add(overhead_elements)
        .ok_or_else(|| shape("bundle decode element budget overflow"))?;
    // Vec<u8> bodies, the segment table and root table share the decoder's
    // generic sequence ceiling. Exact table counts are checked after decoding;
    // hostile count headers are additionally constrained by actual payload and
    // cumulative element/allocation budgets before their allocations.
    let largest_sequence = limits
        .segment
        .max_proof_bytes
        .max(expected_count)
        .min(bytes.len());
    Ok(DecodeLimits::new(
        largest_sequence,
        bytes.len(),
        total_elements,
        limits.max_total_decode_allocation_charges,
        MAX_DECODE_DEPTH,
    ))
}

#[cfg(test)]
fn preflight_count(count: usize, limits: BundleLimits) -> Result<()> {
    preflight_count_for(
        count,
        limits,
        SharedVerifier {
            max_decode_allocation_charges: 32 * 1024 * 1024,
        },
    )
}

fn preflight_count_for(count: usize, limits: BundleLimits, verifier: SharedVerifier) -> Result<()> {
    if count == 0 {
        return Err(shape("compact bundle requires at least one complete delta"));
    }
    check_limit("max_bundle_segments", count, limits.max_segments)?;
    check_limit(
        "max_public_transfer_deltas",
        count,
        PublicTransferLimits::default().max_deltas,
    )?;
    let queries = count
        .checked_mul(verifier.queries())
        .ok_or_else(|| shape("bundle query count overflow"))?;
    check_limit(
        "max_queries",
        verifier.queries(),
        limits.segment.max_queries,
    )?;
    check_limit("max_bundle_queries", queries, limits.max_total_queries)
}

fn preflight_wire(wire: &BundleWire, expected_count: usize, limits: BundleLimits) -> Result<()> {
    preflight_wire_parts(
        wire.version,
        &wire.intermediate_roots,
        &wire.segments,
        expected_count,
        limits,
    )
}

fn preflight_wire_parts(
    version: u16,
    intermediate_roots: &[[u8; 32]],
    segments: &[Vec<u8>],
    expected_count: usize,
    limits: BundleLimits,
) -> Result<()> {
    preflight_wire_parts_for(
        version,
        intermediate_roots,
        segments,
        expected_count,
        limits,
        SharedVerifier {
            max_decode_allocation_charges: 32 * 1024 * 1024,
        },
    )
}

fn preflight_wire_parts_for(
    version: u16,
    intermediate_roots: &[[u8; 32]],
    segments: &[Vec<u8>],
    expected_count: usize,
    limits: BundleLimits,
    verifier: SharedVerifier,
) -> Result<()> {
    preflight_count_for(expected_count, limits, verifier)?;
    if version != VERSION {
        return Err(shape("unsupported compact bundle version"));
    }
    if segments.len() != expected_count || intermediate_roots.len() != expected_count - 1 {
        return Err(shape("compact bundle segment/root count mismatch"));
    }
    for root in intermediate_roots {
        if root[31] & 1 == 0 {
            return Err(shape("compact bundle root requires canonical hash marker"));
        }
    }
    let mut total = 0usize;
    for frame in segments {
        if frame.is_empty() {
            return Err(shape("compact bundle contains an empty segment frame"));
        }
        check_limit(
            "max_proof_bytes",
            frame.len(),
            limits.segment.max_proof_bytes,
        )?;
        total = total
            .checked_add(frame.len())
            .ok_or_else(|| shape("bundle segment byte count overflow"))?;
        check_limit(
            "max_bundle_segment_bytes",
            total,
            limits.max_total_segment_bytes,
        )?;
    }
    Ok(())
}

fn add_work(total: &mut SharedVerificationWork, child: SharedVerificationWork) -> Result<()> {
    // Stage the complete addition so an overflow cannot leave partial counters.
    let mut next = *total;
    for (target, value) in [
        (&mut next.proof_bytes, child.proof_bytes),
        (&mut next.transcripts, child.transcripts),
        (&mut next.row_leaves, child.row_leaves),
        (&mut next.oracle_leaves, child.oracle_leaves),
        (&mut next.fri_leaves, child.fri_leaves),
        (&mut next.parent_hashes, child.parent_hashes),
        (&mut next.air_evaluations, child.air_evaluations),
        (
            &mut next.terminal_degree_checks,
            child.terminal_degree_checks,
        ),
    ] {
        *target = target
            .checked_add(value)
            .ok_or_else(|| shape("bundle verification work overflow"))?;
    }
    *total = next;
    Ok(())
}

fn check_limit(limit: &'static str, actual: usize, max: usize) -> Result<()> {
    if actual > max {
        Err(Error::VerifierLimitExceeded { limit, actual, max })
    } else {
        Ok(())
    }
}

fn shape(details: &str) -> Error {
    Error::TransferInvariant {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    fn test_segment_limits() -> crate::VerifyLimits {
        crate::VerifyLimits {
            max_queries: 375,
            ..crate::VerifyLimits::default()
        }
    }
    use super::*;
    use crate::backend::compact_axt_context::tests::Fixture;

    fn wire(count: usize) -> BundleWire {
        BundleWire {
            version: VERSION,
            intermediate_roots: (1..count)
                .map(|index| {
                    let mut root = [index as u8; 32];
                    root[31] |= 1;
                    root
                })
                .collect(),
            segments: (0..count)
                .map(|index| vec![index as u8 + 1; 37 + index])
                .collect(),
        }
    }

    fn limits(count: usize) -> BundleLimits {
        BundleLimits {
            max_segments: count,
            max_total_queries: count * 375,
            segment: VerifyLimits {
                max_queries: 375,
                ..test_segment_limits()
            },
            ..BundleLimits::default()
        }
    }

    #[test]
    fn default_policy_does_not_multiply_any_single_proof_allowance() {
        let policy = BundleLimits::default();
        let single = VerifyLimits::default();
        assert_eq!(policy.max_segments, 1);
        assert_eq!(policy.max_wire_bytes, single.max_proof_bytes);
        assert_eq!(policy.max_total_segment_bytes, single.max_proof_bytes);
        assert_eq!(policy.max_total_statement_bytes, single.max_batch_bytes);
        assert_eq!(policy.max_total_queries, single.max_queries);
        assert!(preflight_count(2, policy).is_err());
        assert!(preflight_count(0, limits(2)).is_err());
        assert!(preflight_count(129, limits(129)).is_err());
        assert!(matches!(
            preflight_count(
                2,
                BundleLimits {
                    max_total_queries: 271,
                    ..limits(2)
                }
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_bundle_queries",
                actual: 272,
                max: 271
            })
        ));
        preflight_count(2, limits(2)).unwrap();
    }

    #[test]
    fn canonical_outer_roundtrip_preserves_order_roots_and_ambient_flags() {
        for count in [1, 2, 3] {
            let original = wire(count);
            let bytes = encode_wire(&original, count, limits(count)).unwrap();
            let ambient =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _flags = norito::core::DecodeFlagsGuard::enter(ambient);
            assert_eq!(encode_wire(&original, count, limits(count)).unwrap(), bytes);
            assert_eq!(decode_wire(&bytes, count, limits(count)).unwrap(), original);
            assert_eq!(norito::core::effective_decode_flags(), Some(ambient));
            assert!(norito::core::encoded_frame_len(&original).is_ok());
        }
    }

    #[test]
    fn exact_counts_version_marker_and_nonempty_frames_are_required() {
        let original = wire(2);
        for mutation in 0..7 {
            let mut changed = original.clone();
            match mutation {
                0 => changed.version = 2,
                1 => {
                    changed.segments.pop();
                }
                2 => changed.segments.push(vec![3]),
                3 => {
                    changed.intermediate_roots.clear();
                }
                4 => changed.intermediate_roots.push([1; 32]),
                5 => changed.intermediate_roots[0][31] &= !1,
                6 => changed.segments[1].clear(),
                _ => unreachable!(),
            }
            assert!(
                preflight_wire(&changed, 2, limits(2)).is_err(),
                "mutation {mutation}"
            );
            let raw = norito::encode_canonical(&changed).unwrap();
            assert!(
                decode_wire(&raw, 2, limits(2)).is_err(),
                "raw mutation {mutation}"
            );
        }
        // Ordering is preserved by the codec. Cryptographic ordinal/context
        // binding, not structural decoding, rejects a reordered valid bundle.
        let mut swapped = original;
        swapped.segments.swap(0, 1);
        assert_eq!(
            decode_wire(&encode_wire(&swapped, 2, limits(2)).unwrap(), 2, limits(2)).unwrap(),
            swapped
        );
    }

    #[test]
    fn inclusive_wire_child_and_total_byte_limits_are_independent() {
        let original = wire(2);
        let raw = encode_wire(&original, 2, limits(2)).unwrap();
        let exact = BundleLimits {
            max_wire_bytes: raw.len(),
            max_total_segment_bytes: 75,
            segment: VerifyLimits {
                max_proof_bytes: 38,
                ..test_segment_limits()
            },
            ..limits(2)
        };
        assert_eq!(decode_wire(&raw, 2, exact).unwrap(), original);
        assert_eq!(encode_wire(&original, 2, exact).unwrap(), raw);
        for bad in [
            BundleLimits {
                max_wire_bytes: raw.len() - 1,
                ..exact
            },
            BundleLimits {
                max_total_segment_bytes: 74,
                ..exact
            },
            BundleLimits {
                segment: VerifyLimits {
                    max_proof_bytes: 37,
                    ..exact.segment
                },
                ..exact
            },
        ] {
            assert!(decode_wire(&raw, 2, bad).is_err());
            assert!(encode_wire(&original, 2, bad).is_err());
        }
        assert!(matches!(
            decode_wire(
                &[0xff],
                0,
                BundleLimits {
                    max_wire_bytes: 0,
                    ..exact
                }
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_bundle_wire_bytes",
                actual: 1,
                max: 0
            })
        ));
    }

    #[test]
    fn canonical_decoder_rejects_trailing_bytes_wrong_schema_and_resource_bombs() {
        let original = wire(2);
        let raw = encode_wire(&original, 2, limits(2)).unwrap();
        let mut trailing = raw.clone();
        trailing.push(0);
        assert!(decode_wire(&trailing, 2, limits(2)).is_err());
        let wrong = norito::encode_canonical(&original.segments).unwrap();
        assert!(decode_wire(&wrong, 2, limits(2)).is_err());
        assert!(
            decode_wire(
                &raw,
                2,
                BundleLimits {
                    max_total_decode_allocation_charges: 1,
                    ..limits(2)
                }
            )
            .is_err()
        );
        assert!(
            decode_wire(
                &raw,
                2,
                BundleLimits {
                    max_total_segment_bytes: usize::MAX,
                    ..limits(2)
                }
            )
            .is_err()
        );
        let restrictive = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 1);
        norito::core::with_decode_limits_scope(restrictive, || {
            assert!(decode_wire(&raw, 2, limits(2)).is_err());
        });
        assert_eq!(decode_wire(&raw, 2, limits(2)).unwrap(), original);
    }

    #[test]
    fn checksummed_raw_count_and_length_headers_cannot_exceed_decode_budgets() {
        use core::ops::Range;

        fn field(bytes: &[u8], index: usize) -> Range<usize> {
            let mut offset = 0;
            for position in 0..=index {
                let (length, prefix) = norito::core::read_len_from_slice_with_flags(
                    &bytes[offset..],
                    norito::core::default_encode_flags(),
                )
                .unwrap();
                let start = offset + prefix;
                if position == index {
                    return start..start + length;
                }
                offset = start + length;
            }
            unreachable!()
        }

        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let (bare, flags) = norito::codec::encode_with_header_flags(&wire(2));
        assert_eq!(flags, norito::core::default_encode_flags());
        let roots = field(&bare, 1);
        let segments = field(&bare, 2);
        let (_, root_prefix) = norito::core::inspect_seq_len_slice(&bare[roots.clone()]).unwrap();
        let (_, segment_prefix) =
            norito::core::inspect_seq_len_slice(&bare[segments.clone()]).unwrap();
        // The canonical default sequential layout uses fixed eight-byte counts.
        assert_eq!((root_prefix, segment_prefix), (8, 8));
        let first_segment = field(&bare[segments.start + segment_prefix..segments.end], 0);
        let first_start = segments.start + segment_prefix + first_segment.start;
        let (_, byte_prefix) = norito::core::inspect_seq_len_slice(&bare[first_start..]).unwrap();
        assert_eq!(byte_prefix, 8);
        for offset in [roots.start, segments.start, first_start, first_start - 8] {
            let mut malicious = bare.clone();
            malicious[offset..offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
            let frame = norito::core::frame_bare_with_header_flags::<BundleWire>(&malicious, flags)
                .unwrap();
            // The fresh canonical frame has a valid checksum; rejection tests
            // its payload declarations, rather than merely a corrupt CRC.
            assert!(
                matches!(decode_wire(&frame, 2, limits(2)), Err(Error::Encode(_))),
                "offset {offset}"
            );
        }
        let mut too_many = wire(2);
        too_many.intermediate_roots.resize(3, [1; 32]);
        let raw = norito::encode_canonical(&too_many).unwrap();
        assert!(matches!(
            decode_wire(&raw, 2, limits(2)),
            Err(Error::TransferInvariant { .. })
        ));
        too_many = wire(2);
        too_many.segments.push(vec![1]);
        let raw = norito::encode_canonical(&too_many).unwrap();
        assert!(matches!(
            decode_wire(&raw, 2, limits(2)),
            Err(Error::TransferInvariant { .. })
        ));
    }

    #[test]
    fn outer_allocation_scope_cannot_be_reset_by_sequential_child_decodes() {
        let raw = encode_wire(&wire(1), 1, limits(1)).unwrap();
        let unlimited = DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            MAX_DECODE_DEPTH,
        );
        let (decoded, measured) = norito::core::with_decode_limits_measured(unlimited, || {
            decode_wire(&raw, 1, limits(1))
        });
        decoded.unwrap();
        assert!(measured.total_allocated_bytes() > 0);
        let budget = DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            measured.total_allocated_bytes(),
            MAX_DECODE_DEPTH,
        );
        norito::core::with_decode_limits_scope(budget, || {
            decode_wire(&raw, 1, limits(1)).unwrap();
            assert!(decode_wire(&raw, 1, limits(1)).is_err());
        });
        decode_wire(&raw, 1, limits(1)).unwrap();
    }

    #[test]
    fn verification_checks_outer_size_semantics_and_expected_public_io() {
        let fixture = Fixture::new(false);
        let axt = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let expected = fixture.expected(&axt);
        assert!(matches!(
            verify_transfer_bundle(
                &axt,
                &expected,
                &[0xff],
                BundleLimits {
                    max_wire_bytes: 0,
                    ..limits(1)
                }
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_bundle_wire_bytes",
                ..
            })
        ));
        let frame = encode_wire(&wire(1), 1, limits(1)).unwrap();
        assert!(verify_transfer_bundle(&axt, &expected, &frame, limits(1)).is_err());
        let ordinary = fixture.prepare(ProofSemantics::StateTransition);
        let mut wrong = fixture.expected(&ordinary);
        wrong.slot ^= 1;
        assert!(matches!(
            verify_transfer_bundle(&ordinary, &wrong, &frame, limits(1)),
            Err(Error::PublicIoMismatch { .. })
        ));
        // Structurally valid outer bytes and public context do not bypass the
        // canonical shared-proof decoder for an invalid child frame.
        assert!(
            verify_transfer_bundle(&ordinary, &fixture.expected(&ordinary), &frame, limits(1))
                .is_err()
        );
        assert!(matches!(
            verify_transfer_bundle(
                &ordinary,
                &fixture.expected(&ordinary),
                &frame,
                BundleLimits {
                    segment: VerifyLimits {
                        max_queries: 374,
                        ..test_segment_limits()
                    },
                    ..limits(1)
                }
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_queries",
                actual: 375,
                max: 374
            })
        ));
        let prepared_bytes = ordinary.work().public_bytes;
        assert!(matches!(
            verify_transfer_bundle(
                &ordinary,
                &fixture.expected(&ordinary),
                &frame,
                BundleLimits {
                    segment: VerifyLimits {
                        max_batch_bytes: prepared_bytes,
                        ..test_segment_limits()
                    },
                    ..limits(1)
                }
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_statement_bytes",
                ..
            })
        ));
    }

    #[test]
    fn successful_result_accessors_and_work_addition_preserve_every_counter() {
        let child = SharedVerificationWork {
            proof_bytes: 1,
            transcripts: 2,
            row_leaves: 3,
            oracle_leaves: 4,
            fri_leaves: 5,
            parent_hashes: 6,
            air_evaluations: 7,
            terminal_degree_checks: 8,
        };
        let mut work = child;
        add_work(&mut work, child).unwrap();
        assert_eq!(
            work,
            SharedVerificationWork {
                proof_bytes: 2,
                transcripts: 4,
                row_leaves: 6,
                oracle_leaves: 8,
                fri_leaves: 10,
                parent_hashes: 12,
                air_evaluations: 14,
                terminal_degree_checks: 16,
            }
        );
        let before = work;
        assert!(
            add_work(
                &mut work,
                SharedVerificationWork {
                    terminal_degree_checks: usize::MAX,
                    ..SharedVerificationWork::default()
                }
            )
            .is_err()
        );
        assert_eq!(work, before);
        let result = VerifiedBundle {
            public_io: PublicIO::default(),
            segments: 2,
            wire_bytes: 99,
            statement_bytes: 100,
            work,
            row_roots: vec![GoldilocksDigest384V1::new([1, 2, 3, 4, 5, 6]).unwrap(); 2],
        };
        assert_eq!(result.public_io(), PublicIO::default());
        assert_eq!(result.segments(), 2);
        assert_eq!(result.wire_bytes(), 99);
        assert_eq!(result.statement_bytes(), 100);
        assert_eq!(result.work(), work);
        assert_eq!(result.row_roots().len(), 2);
        assert!(
            result
                .row_roots()
                .iter()
                .all(|r| r.words() == [1, 2, 3, 4, 5, 6])
        );
        assert!(check_limit("test", 1, 0).is_err());
        check_limit("test", 1, 1).unwrap();
        assert!(shape("context").to_string().contains("context"));
    }
    fn axt_wire(count: usize) -> AxtBundleWire {
        let ordinary = wire(count);
        AxtBundleWire {
            version: ordinary.version,
            intermediate_roots: ordinary.intermediate_roots,
            segments: ordinary.segments,
        }
    }

    fn axt_context(fixture: &Fixture) -> AxtVerificationContext<'_> {
        AxtVerificationContext {
            binding: &fixture.binding,
            metadata: fixture.metadata(),
            mirrors: fixture.outer,
            remote_spend_claims: fixture.remote.as_deref(),
        }
    }

    #[test]
    fn axt_carrier_has_a_distinct_nominal_schema_and_exact_canonical_roundtrip() {
        for count in [1, 2, 3] {
            let axt = axt_wire(count);
            let raw = encode_axt_wire(&axt, count, limits(count)).unwrap();
            let ordinary = encode_wire(&wire(count), count, limits(count)).unwrap();
            assert_ne!(raw, ordinary);
            assert_eq!(decode_axt_wire(&raw, count, limits(count)).unwrap(), axt);
            assert!(decode_axt_wire(&ordinary, count, limits(count)).is_err());
            assert!(decode_wire(&raw, count, limits(count)).is_err());
            let ambient =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _flags = norito::core::DecodeFlagsGuard::enter(ambient);
            assert_eq!(encode_axt_wire(&axt, count, limits(count)).unwrap(), raw);
            assert_eq!(decode_axt_wire(&raw, count, limits(count)).unwrap(), axt);
            assert_eq!(norito::core::get_decode_flags(), ambient);
            let mut trailing = raw.clone();
            trailing.push(0);
            assert!(decode_axt_wire(&trailing, count, limits(count)).is_err());
            assert!(
                encode_axt_wire(
                    &axt,
                    count,
                    BundleLimits {
                        max_wire_bytes: raw.len() - 1,
                        ..limits(count)
                    }
                )
                .is_err()
            );
            assert_eq!(
                encode_axt_wire(
                    &axt,
                    count,
                    BundleLimits {
                        max_wire_bytes: raw.len(),
                        ..limits(count)
                    }
                )
                .unwrap(),
                raw
            );
        }
        for mutation in 0..7 {
            let mut axt = axt_wire(2);
            match mutation {
                0 => axt.version = 2,
                1 => {
                    axt.segments.pop();
                }
                2 => axt.segments.push(vec![1]),
                3 => axt.intermediate_roots.clear(),
                4 => axt.intermediate_roots.push([1; 32]),
                5 => axt.intermediate_roots[0][31] &= !1,
                6 => axt.segments[1].clear(),
                _ => unreachable!(),
            }
            assert!(encode_axt_wire(&axt, 2, limits(2)).is_err());
            assert!(
                decode_axt_wire(&norito::encode_canonical(&axt).unwrap(), 2, limits(2)).is_err()
            );
        }
    }

    #[test]
    fn axt_bundle_checks_nominal_route_and_whole_context_before_child_decoding() {
        let fixture = Fixture::multiple(2, true);
        let axt = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let expected = fixture.expected(&axt);
        let raw = encode_axt_wire(&axt_wire(2), 2, limits(2)).unwrap();
        let ordinary = fixture.prepare(ProofSemantics::StateTransition);
        assert!(matches!(
            verify_axt_transfer_bundle(
                &ordinary,
                &fixture.expected(&ordinary),
                axt_context(&fixture),
                &[0xff],
                BundleLimits {
                    max_wire_bytes: 0,
                    ..limits(2)
                }
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_bundle_wire_bytes",
                ..
            })
        ));
        assert!(
            verify_axt_transfer_bundle(
                &ordinary,
                &fixture.expected(&ordinary),
                axt_context(&fixture),
                &raw,
                limits(2)
            )
            .is_err()
        );
        assert!(verify_transfer_bundle(&axt, &expected, &raw, limits(2)).is_err());
        let ordinary_carrier = encode_wire(&wire(2), 2, limits(2)).unwrap();
        assert!(
            verify_axt_transfer_bundle(
                &axt,
                &expected,
                axt_context(&fixture),
                &ordinary_carrier,
                limits(2)
            )
            .is_err()
        );
        assert!(
            verify_transfer_bundle(&ordinary, &fixture.expected(&ordinary), &raw, limits(2))
                .is_err()
        );
        let mut wrong = expected;
        wrong.slot ^= 1;
        assert!(matches!(
            verify_axt_transfer_bundle(&axt, &wrong, axt_context(&fixture), &raw, limits(2)),
            Err(Error::PublicIoMismatch { .. })
        ));
        let mut wrong_mirror = axt_context(&fixture);
        wrong_mirror.mirrors.manifest_root[0] ^= 1;
        assert!(matches!(
            verify_axt_transfer_bundle(&axt, &expected, wrong_mirror, &raw, limits(2)),
            Err(Error::InvalidAxtBinding { .. })
        ));
        let missing = AxtVerificationContext {
            remote_spend_claims: None,
            ..axt_context(&fixture)
        };
        assert!(matches!(
            verify_axt_transfer_bundle(&axt, &expected, missing, &raw, limits(2)),
            Err(Error::MissingMetadata { .. })
        ));
        // With all public predicates satisfied, bad child bytes are still
        // rejected; no successful prefix or statement-only result is returned.
        assert!(
            verify_axt_transfer_bundle(&axt, &expected, axt_context(&fixture), &raw, limits(2))
                .is_err()
        );
    }

    #[test]
    fn axt_bundle_preserves_individual_and_cumulative_preflight_limits() {
        let fixture = Fixture::multiple(2, true);
        let axt = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let expected = fixture.expected(&axt);
        let carrier = axt_wire(2);
        let raw = encode_axt_wire(&carrier, 2, limits(2)).unwrap();
        let batch = AxtTransferBatch::new(
            &axt,
            &expected,
            &carrier.intermediate_roots,
            axt_context(&fixture),
            BatchContextLimits::default(),
        )
        .unwrap();
        assert!(batch.max_statement_bytes() > axt.work().public_bytes);
        for (policy, expected_limit) in [
            (
                BundleLimits {
                    segment: VerifyLimits {
                        max_batch_bytes: batch.max_statement_bytes() - 1,
                        ..limits(2).segment
                    },
                    ..limits(2)
                },
                "max_compact_statement_bytes",
            ),
            (
                BundleLimits {
                    max_total_statement_bytes: batch.total_statement_bytes() - 1,
                    ..limits(2)
                },
                "max_compact_bundle_statement_bytes",
            ),
            (
                BundleLimits {
                    segment: VerifyLimits {
                        max_queries: 374,
                        ..limits(2).segment
                    },
                    ..limits(2)
                },
                "max_queries",
            ),
            (
                BundleLimits {
                    max_total_queries: 271,
                    ..limits(2)
                },
                "max_bundle_queries",
            ),
        ] {
            assert!(
                matches!(verify_axt_transfer_bundle(&axt, &expected, axt_context(&fixture), &raw, policy), Err(Error::VerifierLimitExceeded { limit, .. }) if limit == expected_limit),
                "{expected_limit}"
            );
        }
        let segment_bytes = BundleLimits {
            max_total_segment_bytes: carrier.segments.iter().map(Vec::len).sum::<usize>() - 1,
            ..limits(2)
        };
        assert!(matches!(
            encode_axt_wire(&carrier, 2, segment_bytes),
            Err(Error::VerifierLimitExceeded {
                limit: "max_bundle_segment_bytes",
                ..
            })
        ));
        // The raw path derives its cumulative element budget from the byte
        // policy, so it rejects earlier during canonical outer decoding.
        assert!(matches!(
            verify_axt_transfer_bundle(&axt, &expected, axt_context(&fixture), &raw, segment_bytes),
            Err(Error::Encode(norito::Error::TotalElementsExceeded { .. }))
        ));
        assert!(
            verify_axt_transfer_bundle(
                &axt,
                &expected,
                axt_context(&fixture),
                &raw,
                BundleLimits {
                    max_total_decode_allocation_charges: 0,
                    ..limits(2)
                }
            )
            .is_err()
        );
    }

    #[test]
    fn nominal_axt_decoder_retains_parent_allocation_scope_and_restores_it() {
        let raw = encode_axt_wire(&axt_wire(1), 1, limits(1)).unwrap();
        let unlimited = DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            MAX_DECODE_DEPTH,
        );
        let (decoded, measured) = norito::core::with_decode_limits_measured(unlimited, || {
            decode_axt_wire(&raw, 1, limits(1))
        });
        decoded.unwrap();
        assert!(measured.total_allocated_bytes() > 0);
        let budget = DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            measured.total_allocated_bytes(),
            MAX_DECODE_DEPTH,
        );
        norito::core::with_decode_limits_scope(budget, || {
            decode_axt_wire(&raw, 1, limits(1)).unwrap();
            assert!(decode_axt_wire(&raw, 1, limits(1)).is_err());
        });
        decode_axt_wire(&raw, 1, limits(1)).unwrap();
    }

    fn final_limits(count: usize) -> BundleLimits {
        BundleLimits {
            max_segments: count,
            max_wire_bytes: 16 * 1024 * 1024,
            max_total_segment_bytes: 16 * 1024 * 1024,
            max_total_queries: 375 * count,
            max_total_decode_allocation_charges: 128 * 1024 * 1024,
            segment: VerifyLimits {
                max_proof_bytes: 4_326_227,
                max_queries: 375,
                ..test_segment_limits()
            },
            ..BundleLimits::default()
        }
    }

    fn final_verifier() -> SharedVerifier {
        SharedVerifier {
            max_decode_allocation_charges: 64 * 1024 * 1024,
        }
    }

    #[test]
    fn final_bundle_headers_are_nominal_and_reject_every_retired_carrier() {
        macro_rules! retired_carrier {
            ($name:ident,$schema:literal) => {
                #[derive(NoritoSerialize, norito::NoritoSchema)]
                #[norito_schema(name = $schema)]
                struct $name {
                    version: u16,
                    intermediate_roots: Vec<[u8; 32]>,
                    segments: Vec<Vec<u8>>,
                }
            };
        }
        retired_carrier!(
            OldOrdinary,
            "fastpq_prover::compact_prototype::OrdinaryTransferBundleV1"
        );
        retired_carrier!(
            OldAxt,
            "fastpq_prover::compact_prototype::AxtTransferBundleV1"
        );
        retired_carrier!(
            OldShakeOrdinary,
            "fastpq_prover::compact_candidate::ShakeOrdinaryTransferBundleV1"
        );
        retired_carrier!(
            OldShakeAxt,
            "fastpq_prover::compact_candidate::ShakeAxtTransferBundleV1"
        );
        for count in 1..=3 {
            let ordinary = wire(count);
            let axt = AxtBundleWire {
                version: ordinary.version,
                intermediate_roots: ordinary.intermediate_roots.clone(),
                segments: ordinary.segments.clone(),
            };
            let bytes = encode_wire(&ordinary, count, final_limits(count)).unwrap();
            let axt_bytes = encode_axt_wire(&axt, count, final_limits(count)).unwrap();
            assert_eq!(bytes.len(), axt_bytes.len());
            assert_eq!(
                decode_wire_with_policy(&bytes, count, final_limits(count), final_verifier())
                    .unwrap(),
                ordinary
            );
            assert_eq!(
                decode_axt_wire_with_policy(
                    &axt_bytes,
                    count,
                    final_limits(count),
                    final_verifier()
                )
                .unwrap(),
                axt
            );
            assert!(
                decode_wire_with_policy(&axt_bytes, count, final_limits(count), final_verifier())
                    .is_err()
            );
            assert!(
                decode_axt_wire_with_policy(&bytes, count, final_limits(count), final_verifier())
                    .is_err()
            );
            macro_rules! old_bytes {
                ($name:ident) => {
                    norito::encode_canonical(&$name {
                        version: ordinary.version,
                        intermediate_roots: ordinary.intermediate_roots.clone(),
                        segments: ordinary.segments.clone(),
                    })
                    .unwrap()
                };
            }
            for retired in [
                old_bytes!(OldOrdinary),
                old_bytes!(OldAxt),
                old_bytes!(OldShakeOrdinary),
                old_bytes!(OldShakeAxt),
            ] {
                assert_eq!(retired.len(), bytes.len());
                assert!(
                    decode_wire_with_policy(&retired, count, final_limits(count), final_verifier())
                        .is_err()
                );
                assert!(
                    decode_axt_wire_with_policy(
                        &retired,
                        count,
                        final_limits(count),
                        final_verifier()
                    )
                    .is_err()
                );
            }
            assert_eq!(
                decode_wire(&bytes, count, final_limits(count)).unwrap(),
                ordinary
            );
            assert_eq!(
                decode_axt_wire(&axt_bytes, count, final_limits(count)).unwrap(),
                axt
            );
            for flags in [0, norito::core::default_encode_flags()] {
                let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
                assert_eq!(
                    encode_wire(&ordinary, count, final_limits(count)).unwrap(),
                    bytes
                );
                assert_eq!(
                    encode_axt_wire(&axt, count, final_limits(count)).unwrap(),
                    axt_bytes
                );
                assert_eq!(
                    decode_wire_with_policy(&bytes, count, final_limits(count), final_verifier())
                        .unwrap(),
                    ordinary
                );
                assert_eq!(norito::core::get_decode_flags(), flags);
            }
        }
    }

    #[test]
    fn candidate_bundle_cumulative_query_and_raw_caps_precede_child_work() {
        let fixture = Fixture::multiple(2, false);
        for semantics in [
            ProofSemantics::StateTransition,
            ProofSemantics::AxtTransferClaim,
        ] {
            let prepared = fixture.prepare(semantics);
            let expected = fixture.expected(&prepared);
            for (limits, required) in [
                (
                    BundleLimits {
                        max_wire_bytes: 0,
                        ..final_limits(2)
                    },
                    "max_bundle_wire_bytes",
                ),
                (
                    BundleLimits {
                        max_total_queries: 749,
                        ..final_limits(2)
                    },
                    "max_bundle_queries",
                ),
                (
                    BundleLimits {
                        segment: VerifyLimits {
                            max_queries: 374,
                            ..final_limits(2).segment
                        },
                        ..final_limits(2)
                    },
                    "max_queries",
                ),
            ] {
                let result = if semantics == ProofSemantics::StateTransition {
                    verify_transfer_bundle_with_allocation(
                        &prepared,
                        &expected,
                        &[255],
                        limits,
                        usize::MAX,
                    )
                } else {
                    verify_axt_transfer_bundle_with_allocation(
                        &prepared,
                        &expected,
                        AxtVerificationContext {
                            binding: &fixture.binding,
                            metadata: fixture.metadata(),
                            mirrors: fixture.outer,
                            remote_spend_claims: fixture.remote.as_deref(),
                        },
                        &[255],
                        limits,
                        usize::MAX,
                    )
                };
                assert!(
                    matches!(result, Err(Error::VerifierLimitExceeded { limit, .. }) if limit == required),
                    "{semantics:?}/{required}"
                );
            }
        }
        preflight_count_for(2, final_limits(2), final_verifier()).unwrap();
        assert!(preflight_count_for(0, final_limits(2), final_verifier()).is_err());
        assert!(preflight_count_for(129, final_limits(129), final_verifier()).is_err());
        assert_eq!(
            BundleLimits::default().segment.max_proof_bytes,
            VerifyLimits::default().max_proof_bytes
        );
    }

    #[test]
    fn candidate_bundle_geometry_and_nested_decode_budgets_remain_bounded() {
        let original = wire(2);
        let candidate = BundleWire {
            version: original.version,
            intermediate_roots: original.intermediate_roots,
            segments: original.segments,
        };
        for mutation in 0..7 {
            let mut bad = candidate.clone();
            match mutation {
                0 => bad.version = 2,
                1 => {
                    bad.segments.pop();
                }
                2 => bad.segments.push(vec![1]),
                3 => bad.intermediate_roots.clear(),
                4 => bad.intermediate_roots.push([1; 32]),
                5 => bad.intermediate_roots[0][31] &= !1,
                6 => bad.segments[1].clear(),
                _ => unreachable!(),
            }
            assert!(encode_wire(&bad, 2, final_limits(2)).is_err());
            let bytes = norito::encode_canonical(&bad).unwrap();
            assert!(decode_wire_with_policy(&bytes, 2, final_limits(2), final_verifier()).is_err());
        }
        let bytes = encode_wire(&candidate, 2, final_limits(2)).unwrap();
        let zero = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 16);
        let (result, usage) = norito::core::with_decode_limits_measured(zero, || {
            decode_wire_with_policy(&bytes, 2, final_limits(2), final_verifier())
        });
        assert!(matches!(
            result,
            Err(Error::Encode(norito::Error::TotalAllocationExceeded { .. }))
        ));
        assert_eq!(usage.total_allocated_bytes(), 0);
        let decoded =
            decode_wire_with_policy(&bytes, 2, final_limits(2), final_verifier()).unwrap();
        assert_eq!(decoded.segments, candidate.segments);
        assert!(
            encode_wire(
                &candidate,
                2,
                BundleLimits {
                    max_total_segment_bytes: 74,
                    ..final_limits(2)
                }
            )
            .is_err()
        );
    }
}
