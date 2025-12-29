//! Data-availability tracking helpers.
//!
//! When `sumeragi.da_enabled = true`, the node tracks availability evidence
//! (`AvailabilityQC` or an RBC `READY` quorum) for DA commitments. The evidence
//! is recorded and surfaced via telemetry to support long-term storage guarantees,
//! but it does not gate consensus in v1.
//!
//! Reliable broadcast remains a transport and recovery mechanism for payload
//! distribution (e.g., when a peer misses `BlockCreated`), but it must not
//! block commit or finalize paths.

use iroha_data_model::nexus::LaneId;

/// Outcome of the data-availability evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateReason {
    /// The node has not yet observed an `AvailabilityQC` for the block.
    MissingAvailabilityQc,
    /// Required manifest is missing or invalid on the local node.
    ManifestGuard {
        /// Lane the guarded blob belongs to.
        lane: LaneId,
        /// Epoch assigned to the blob.
        epoch: u64,
        /// Sequence number within the epoch.
        sequence: u64,
        /// Specific manifest guard failure.
        kind: ManifestGateKind,
    },
}

/// Classification of manifest guard failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestGateKind {
    /// No manifest file present for the commitment.
    Missing,
    /// Manifest hash does not match the commitment.
    HashMismatch,
    /// Manifest could not be read from disk.
    ReadFailed,
    /// Spool scan failed while searching for manifests.
    SpoolScan,
}

impl ManifestGateKind {
    /// String label used for telemetry and status snapshots.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "manifest_missing",
            Self::HashMismatch => "manifest_hash_mismatch",
            Self::ReadFailed => "manifest_read_failed",
            Self::SpoolScan => "manifest_spool_scan",
        }
    }
}

/// Which gate condition was satisfied most recently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateSatisfaction {
    /// An `AvailabilityQC` was observed after previously gating on it.
    AvailabilityQc,
}

/// Evaluate whether availability evidence is still missing for a block.
/// This result is advisory and must not block consensus.
#[must_use]
pub fn evaluate(da_enabled: bool, has_availability_qc: bool) -> Option<GateReason> {
    if !da_enabled {
        return None;
    }

    if !has_availability_qc {
        return Some(GateReason::MissingAvailabilityQc);
    }

    None
}

/// Determine which gate transitioned to satisfied between `previous` and `current`.
#[must_use]
pub fn gate_satisfaction(
    previous: Option<GateReason>,
    current: Option<GateReason>,
) -> Option<GateSatisfaction> {
    match (previous, current) {
        (Some(GateReason::MissingAvailabilityQc), None) => Some(GateSatisfaction::AvailabilityQc),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{GateReason, GateSatisfaction, evaluate, gate_satisfaction};

    #[test]
    fn da_disabled_skips_gating() {
        assert!(evaluate(false, false).is_none());
    }

    #[test]
    fn missing_qc_reports_missing_availability() {
        assert_eq!(
            evaluate(true, false),
            Some(GateReason::MissingAvailabilityQc)
        );
    }

    #[test]
    fn availability_qc_clears_missing_availability() {
        assert_eq!(evaluate(true, true), None);
    }

    #[test]
    fn gate_satisfaction_tracks_transitions() {
        assert_eq!(
            gate_satisfaction(Some(GateReason::MissingAvailabilityQc), None),
            Some(GateSatisfaction::AvailabilityQc)
        );
        assert_eq!(gate_satisfaction(None, None), None);
        assert_eq!(
            gate_satisfaction(
                Some(GateReason::MissingAvailabilityQc),
                Some(GateReason::MissingAvailabilityQc)
            ),
            None
        );
    }
}
