//! Witness-free resource planning shared by the offline API and real producers.

use super::{ProvingLimits, VerificationLimits};
use crate::{
    Error, Result,
    backend::{
        compact_protocol::replay::TraceReplayPlan, compact_prover_resources::segment_charge,
    },
    gadgets::compact_smt_air::{COLUMN_COUNT, PHYSICAL_ROW_COUNT},
};

pub(in crate::backend) const QUANTITY_QUERY_COUNT: usize = 375;
pub(in crate::backend) const QUANTITY_SHARED_FRAME_BOUND: usize = 4_017_376;

/// Resource arithmetic for the current, unmasked offline quantity artifact.
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
    /// Exact peak owned coefficient-plus-stripe payload for sequential replay.
    ///
    /// Caller inputs, digest trees, scalar/extension oracles, fixed preparation,
    /// row openings and allocator/runtime overhead are outside this subtotal.
    pub trace_replay_peak_bytes: usize,
    /// Rows per column in each in-place replay transform.
    pub trace_replay_transform_rows: usize,
    /// Stripes visited by each complete replay pass.
    pub trace_replay_stripes: usize,
    /// Maximum column transforms per segment, including initial interpolation.
    ///
    /// Three full passes commit rows, mix columns and evaluate quotients. A
    /// fourth visits only stripes needed by final row openings.
    pub trace_replay_maximum_column_transforms: usize,
    /// Necessary raw row-opening bytes per segment, excluding other proof data.
    ///
    /// This is also a necessary decoded-row allocation charge, not a sufficient
    /// decoder budget. Sharing current/next openings cannot reduce it.
    pub minimum_segment_row_bytes: usize,
    /// Necessary raw row-opening bytes across the complete bundle.
    pub minimum_bundle_row_bytes: usize,
    /// Necessary child payload bytes for rows plus mixed/quotient values.
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
    /// Conservative structural working-payload charge for one segment.
    ///
    /// This uses the caller's maximum statement length and the fixed child-frame
    /// bound. It is neither an allocation reservation nor an RSS ceiling. Private
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
/// The current profile is unmasked and offline-only. No plan enables private
/// proofs, production admission, or larger protocol/resource limits.
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
    let replay = TraceReplayPlan::new(PHYSICAL_ROW_COUNT, COLUMN_COUNT)?;
    let trace_cells_per_segment = mul(COLUMN_COUNT, PHYSICAL_ROW_COUNT)?;
    let minimum_segment_row_bytes =
        mul(mul(QUANTITY_QUERY_COUNT, COLUMN_COUNT)?, size_of::<u64>())?;
    let minimum_segment_proof_payload_bytes = add(
        minimum_segment_row_bytes,
        mul(mul(QUANTITY_QUERY_COUNT, 2)?, crate::GoldilocksFp4V1::BYTES)?,
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
        trace_replay_peak_bytes: replay.peak_trace_bytes,
        trace_replay_transform_rows: replay.trace_rows,
        trace_replay_stripes: replay.stripes,
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
            ("max_air_row_values", COLUMN_COUNT, child.max_air_row_values),
            ("max_fri_layers", 18, child.max_fri_layers),
            ("max_query_path_len", 19, child.max_query_path_len),
            ("max_fri_round_values", 4, child.max_fri_round_values),
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
    fn planner_reports_unavoidable_wire_bytes_and_sequential_work() {
        let one = quantity_artifact_resources(1, 0).unwrap();
        let two = quantity_artifact_resources(2, 256).unwrap();
        assert_eq!(one.minimum_segment_row_bytes, 1_026_000);
        assert!(one.minimum_segment_row_bytes > 512 * 1024);
        assert_eq!(one.minimum_segment_proof_payload_bytes, 1_050_000);
        assert!(one.minimum_segment_proof_payload_bytes > 1024 * 1024);
        assert_eq!(two.minimum_total_segment_payload_bytes, 2_100_000);
        assert_eq!(two.minimum_bundle_row_bytes, 2_052_000);
        assert!(two.minimum_bundle_row_bytes > 1024 * 1024);
        assert_eq!(two.total_queries, 750);
        assert_eq!(one.trace_cells_per_segment, 342 * 65_536);
        assert_eq!(one.trace_replay_peak_bytes, 358_612_992);
        assert_eq!(one.trace_replay_transform_rows, 65_536);
        assert_eq!(one.trace_replay_stripes, 8);
        assert_eq!(one.trace_replay_maximum_column_transforms, 11_286);
        assert_eq!(two.trace_replay_peak_bytes, one.trace_replay_peak_bytes);
        assert_eq!(two.total_trace_cells, 2 * one.trace_cells_per_segment);
        assert_eq!(two.maximum_total_segment_frame_bytes, 2 * 4_017_376);
        assert_eq!(
            two.maximum_bundle_frame_bytes,
            1024 + 64 + 2 * (4_017_376 + 32)
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
