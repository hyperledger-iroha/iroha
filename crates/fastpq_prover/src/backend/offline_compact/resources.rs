//! Witness-free resource planning shared by the offline API and real producers.

use super::{ProvingLimits, VerificationLimits};
use crate::{
    Error, Result,
    backend::{
        compact_prover_resources::{replay_plan, segment_charge},
        compact_public_columns::COMMITTED_COLUMN_COUNT,
        deep_geometry, deep_proof,
    },
    gadgets::compact_smt_air::{COLUMN_COUNT, PHYSICAL_ROW_COUNT},
};

pub(in crate::backend) const QUANTITY_QUERY_COUNT: usize = deep_geometry::QUERY_COUNT;
pub(in crate::backend) const QUANTITY_SHARED_FRAME_BOUND: usize = deep_proof::MAX_FRAME_BYTES;

/// Resource arithmetic for the current masked offline quantity artifact.
///
/// This report neither authenticates a statement nor authorizes a proof. Counts
/// describe the fixed protocol; byte bounds describe different resources and
/// must not be substituted for one another. Producers independently derive this
/// report again and enforce their explicit policies before private-tree work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct QuantityArtifactResources {
    /// Nonzero number of complete delta occurrences, one proof segment each.
    pub segments: usize,
    /// Distinct query positions in each segment.
    pub queries_per_segment: usize,
    /// Query positions summed over all segments.
    pub total_queries: usize,
    /// Base-field trace cells in one segment, before LDE expansion.
    pub trace_cells_per_segment: usize,
    /// Base-field trace cells processed sequentially across all segments.
    pub total_trace_cells: usize,
    /// Fixed replay payload including source, coefficients, entropy and one stripe.
    ///
    /// Digest trees, quotient/FRI workspace, codec and allocator/runtime overhead
    /// are outside this subtotal.
    pub trace_replay_peak_bytes: usize,
    /// Rows per column in each in-place replay transform.
    pub trace_replay_transform_rows: usize,
    /// Stripes visited by each complete replay pass.
    pub trace_replay_stripes: usize,
    /// Maximum column transforms per segment, including initial interpolation.
    ///
    /// Three conservative full passes cover row commitments, the quotient
    /// numerator and selected row openings.
    pub trace_replay_maximum_column_transforms: usize,
    /// Necessary raw row-opening bytes per segment, excluding other proof data.
    ///
    /// This is also a necessary decoded-row allocation charge, not a sufficient
    /// decoder budget. Sharing current/next openings cannot reduce it.
    pub minimum_segment_row_bytes: usize,
    /// Necessary raw row-opening bytes across the complete bundle.
    pub minimum_bundle_row_bytes: usize,
    /// Necessary child payload bytes for rows plus randomized quotient chunks and composition masks.
    ///
    /// Merkle frontiers, FRI openings, indices and framing increase this floor.
    pub minimum_segment_proof_payload_bytes: usize,
    /// Necessary child payload bytes summed across the complete bundle.
    pub minimum_total_segment_payload_bytes: usize,
    /// Canonical valid-shape child frame bound used by the producer preflight.
    pub maximum_segment_frame_bytes: usize,
    /// Sum of the canonical valid-shape child frame bounds.
    pub maximum_total_segment_frame_bytes: usize,
    /// Conservative complete bundle carrier bound, excluding outer transport.
    pub maximum_bundle_frame_bytes: usize,
    /// Necessary structural working-payload floor for one segment.
    ///
    /// This uses the caller's maximum statement length and the fixed child-frame
    /// bound. The exact relation-dependent producer plan must also pass before
    /// trace expansion. This floor is neither an allocation reservation nor an RSS ceiling. Private
    /// SMTs, retained child frames, decoder charges, thread stacks and unrelated
    /// process memory have separate budgets.
    pub segment_charge_bytes: usize,
}

/// Plan fixed-profile quantity resources without a witness or proof/trace buffers.
///
/// `maximum_segment_statement_bytes` bounds the complete statement absorbed by
/// one child transcript, including its ordinary or AXT context. Use zero to
/// inspect the unavoidable working-payload floor; actual proving also checks
/// the complete statement length. For a conservative report before preparing
/// contexts, the bundle's total statement-byte cap bounds every child.
///
/// The masked profile uses the same plan for node and standalone callers.
/// Resource accounting never replaces statement authentication or verification.
///
/// # Errors
/// Rejects an empty bundle or overflow in any work/byte bound.
pub fn quantity_artifact_resources(
    segments: usize,
    maximum_segment_statement_bytes: usize,
) -> Result<QuantityArtifactResources> {
    let roots = segments
        .checked_sub(1)
        .ok_or_else(|| invalid("quantity resource plan requires a nonempty bundle"))?;
    let replay = replay_plan()?;
    let trace_cells_per_segment = mul(COLUMN_COUNT, PHYSICAL_ROW_COUNT)?;
    let minimum_segment_row_bytes = mul(
        mul(QUANTITY_QUERY_COUNT, COMMITTED_COLUMN_COUNT)?,
        size_of::<u64>(),
    )?;
    let minimum_segment_proof_payload_bytes = add(
        minimum_segment_row_bytes,
        mul(mul(QUANTITY_QUERY_COUNT, 3)?, crate::GoldilocksFp4V1::BYTES)?,
    )?;
    // Keep the same conservative framing allowances as actual producer ingress:
    // at most ten prefix bytes per scalar/sequence plus the carrier fields.
    let maximum_bundle_frame_bytes = add(
        1024,
        add(
            mul(roots, 64)?,
            mul(segments, QUANTITY_SHARED_FRAME_BOUND + 32)?,
        )?,
    )?;
    Ok(QuantityArtifactResources {
        segments,
        queries_per_segment: QUANTITY_QUERY_COUNT,
        total_queries: mul(segments, QUANTITY_QUERY_COUNT)?,
        trace_cells_per_segment,
        total_trace_cells: mul(segments, trace_cells_per_segment)?,
        trace_replay_peak_bytes: replay.payload_bytes,
        trace_replay_transform_rows: PHYSICAL_ROW_COUNT,
        trace_replay_stripes: replay.stripes(),
        trace_replay_maximum_column_transforms: replay.maximum_column_transforms,
        minimum_segment_row_bytes,
        minimum_bundle_row_bytes: mul(segments, minimum_segment_row_bytes)?,
        minimum_segment_proof_payload_bytes,
        minimum_total_segment_payload_bytes: mul(segments, minimum_segment_proof_payload_bytes)?,
        maximum_segment_frame_bytes: QUANTITY_SHARED_FRAME_BOUND,
        maximum_total_segment_frame_bytes: mul(segments, QUANTITY_SHARED_FRAME_BOUND)?,
        maximum_bundle_frame_bytes,
        segment_charge_bytes: segment_charge(
            maximum_segment_statement_bytes,
            QUANTITY_SHARED_FRAME_BOUND,
        )?,
    })
}

impl QuantityArtifactResources {
    #[allow(
        clippy::large_types_passed_by_value,
        reason = "keeps the by-value `Copy` limits contract of its sibling-module callers"
    )]
    pub(in crate::backend) fn check_proving_limits(
        self,
        proving: ProvingLimits,
        verification: VerificationLimits,
    ) -> Result<()> {
        let bundle = verification.bundle;
        let child = bundle.segment;
        for (limit, actual, max) in [
            ("max_bundle_segments", self.segments, bundle.max_segments),
            ("max_queries", self.queries_per_segment, child.max_queries),
            (
                "max_bundle_queries",
                self.total_queries,
                bundle.max_total_queries,
            ),
            (
                "max_proof_bytes",
                self.maximum_segment_frame_bytes,
                child.max_proof_bytes,
            ),
            (
                "max_bundle_segment_bytes",
                self.maximum_total_segment_frame_bytes,
                bundle.max_total_segment_bytes,
            ),
            (
                "max_compact_prover_trace_cells",
                self.total_trace_cells,
                proving.max_total_trace_cells,
            ),
            (
                "max_compact_prover_segment_charge_bytes",
                self.segment_charge_bytes,
                proving.max_segment_charge_bytes,
            ),
            (
                "max_compact_prover_segment_work_units",
                replay_plan()?.work_units,
                proving.max_segment_work_units,
            ),
            (
                "max_air_row_values",
                COMMITTED_COLUMN_COUNT,
                child.max_air_row_values,
            ),
            ("max_fri_layers", 6, child.max_fri_layers),
            ("max_query_path_len", 23, child.max_query_path_len),
            ("max_fri_round_values", 16, child.max_fri_round_values),
            (
                "max_bundle_wire_bytes",
                self.maximum_bundle_frame_bytes,
                bundle.max_wire_bytes,
            ),
            (
                "max_compact_producer_bundle_bytes",
                self.maximum_bundle_frame_bytes,
                verification.transport.max_bundle_frame_bytes,
            ),
        ] {
            if actual > max {
                return Err(Error::VerifierLimitExceeded { limit, actual, max });
            }
        }
        Ok(())
    }
}

fn invalid(details: &'static str) -> Error {
    Error::TransferInvariant {
        details: details.to_owned(),
    }
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| invalid("quantity resource byte count overflows"))
}

fn mul(left: usize, right: usize) -> Result<usize> {
    left.checked_mul(right)
        .ok_or_else(|| invalid("quantity resource work count overflows"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_accepts_the_canonical_two_segment_resource_plan() {
        let proving = ProvingLimits::default();
        let verification = VerificationLimits::default();
        let resources =
            quantity_artifact_resources(2, verification.bundle.max_total_statement_bytes).unwrap();
        resources
            .check_proving_limits(proving, verification)
            .unwrap();
        assert_eq!(
            verification.bundle.segment.max_proof_bytes,
            deep_proof::MAX_FRAME_BYTES
        );
        assert_eq!(proving.digest_execution, crate::DigestExecutionV1::Cpu);
        let mut insufficient = verification;
        insufficient.bundle.segment.max_queries -= 1;
        assert!(
            resources
                .check_proving_limits(proving, insufficient)
                .is_err()
        );
    }

    #[test]
    fn planner_reports_unavoidable_wire_bytes_and_sequential_work() {
        let one = quantity_artifact_resources(1, 0).unwrap();
        let two = quantity_artifact_resources(2, 256).unwrap();
        assert_eq!(one.minimum_segment_row_bytes, 154_112);
        assert_eq!(one.minimum_segment_proof_payload_bytes, 160_256);
        assert_eq!(two.minimum_total_segment_payload_bytes, 320_512);
        assert_eq!(two.minimum_bundle_row_bytes, 308_224);
        assert_eq!(two.total_queries, 128);
        assert_eq!(one.trace_cells_per_segment, 342 * 65_536);
        assert_eq!(
            one.trace_replay_peak_bytes,
            replay_plan().unwrap().payload_bytes
        );
        assert_eq!(one.trace_replay_transform_rows, 65_536);
        assert_eq!(one.trace_replay_stripes, 128);
        assert_eq!(one.trace_replay_maximum_column_transforms, 115_885);
        assert_eq!(two.trace_replay_peak_bytes, one.trace_replay_peak_bytes);
        assert_eq!(two.total_trace_cells, 2 * one.trace_cells_per_segment);
        assert_eq!(
            two.maximum_total_segment_frame_bytes,
            2 * QUANTITY_SHARED_FRAME_BOUND
        );
        assert_eq!(
            two.maximum_bundle_frame_bytes,
            1024 + 64 + 2 * (QUANTITY_SHARED_FRAME_BOUND + 32)
        );
        // Sequential children do not multiply the per-segment charge.
        assert_eq!(two.segment_charge_bytes, one.segment_charge_bytes + 8 * 256);
    }

    #[test]
    fn planner_rejects_empty_and_overflowing_shapes_without_materialization() {
        for (segments, bytes) in [(0, 0), (usize::MAX, 0), (1, usize::MAX)] {
            assert!(quantity_artifact_resources(segments, bytes).is_err());
        }
        assert!(add(usize::MAX, 1).is_err());
        assert!(mul(usize::MAX, 2).is_err());
        assert_eq!(add(usize::MAX, 0).unwrap(), usize::MAX);
        assert_eq!(mul(usize::MAX, 0).unwrap(), 0);
    }
}
