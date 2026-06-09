//! Data-availability tracking helpers.
//!
//! When `sumeragi.da_enabled = true`, the node tracks availability and manifest
//! signals for DA commitments and keeps commit/finalize work pending until the
//! required local payload or strict manifest evidence is available.
//!
//! When the local node lacks required payload data it requests the missing data
//! from peers and retries once it arrives. Reliable broadcast remains a transport
//! and recovery mechanism for payload distribution (e.g., when a peer misses
//! `BlockCreated`).

use iroha_data_model::nexus::LaneId;

/// Outcome of the data-availability evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateReason {
    /// Local validation needs data that is missing and must be fetched.
    MissingLocalData,
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
    /// Missing local data was fetched successfully.
    MissingDataRecovered,
    /// Required manifest material became available and passed the guard.
    ManifestGuardRecovered,
}

/// Evaluate whether availability evidence is still missing for a block.
/// A returned reason keeps DA-enabled commit/finalize work pending until cleared.
#[must_use]
pub fn evaluate(da_enabled: bool, missing_local_data: bool) -> Option<GateReason> {
    if !da_enabled {
        return None;
    }

    if missing_local_data {
        return Some(GateReason::MissingLocalData);
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
        (previous @ Some(GateReason::MissingLocalData), current) if current != previous => {
            Some(GateSatisfaction::MissingDataRecovered)
        }
        (previous @ Some(GateReason::ManifestGuard { .. }), current) if current != previous => {
            Some(GateSatisfaction::ManifestGuardRecovered)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{GateReason, GateSatisfaction, ManifestGateKind, evaluate, gate_satisfaction};

    fn manifest_gate(lane: u32, sequence: u64, kind: ManifestGateKind) -> GateReason {
        GateReason::ManifestGuard {
            lane: iroha_data_model::nexus::LaneId::new(lane),
            epoch: 2,
            sequence,
            kind,
        }
    }

    #[test]
    fn da_disabled_skips_gating() {
        assert!(evaluate(false, false).is_none());
    }

    #[test]
    fn missing_data_reports_missing_local_data() {
        assert_eq!(evaluate(true, true), Some(GateReason::MissingLocalData));
    }

    #[test]
    fn available_data_clears_missing_local_data() {
        assert_eq!(evaluate(true, false), None);
    }

    #[test]
    fn gate_satisfaction_tracks_transitions() {
        assert_eq!(
            gate_satisfaction(Some(GateReason::MissingLocalData), None),
            Some(GateSatisfaction::MissingDataRecovered)
        );
        assert_eq!(
            gate_satisfaction(
                Some(GateReason::MissingLocalData),
                Some(manifest_gate(1, 3, ManifestGateKind::Missing))
            ),
            Some(GateSatisfaction::MissingDataRecovered)
        );
        assert_eq!(
            gate_satisfaction(Some(manifest_gate(1, 3, ManifestGateKind::Missing)), None),
            Some(GateSatisfaction::ManifestGuardRecovered)
        );
        assert_eq!(
            gate_satisfaction(
                Some(manifest_gate(1, 3, ManifestGateKind::Missing)),
                Some(GateReason::MissingLocalData)
            ),
            Some(GateSatisfaction::ManifestGuardRecovered)
        );
        assert_eq!(
            gate_satisfaction(
                Some(manifest_gate(1, 3, ManifestGateKind::Missing)),
                Some(manifest_gate(1, 4, ManifestGateKind::Missing))
            ),
            Some(GateSatisfaction::ManifestGuardRecovered),
            "a different exact manifest gate means the previous guard recovered even though another manifest still gates commit"
        );
        assert_eq!(
            gate_satisfaction(
                Some(manifest_gate(1, 3, ManifestGateKind::Missing)),
                Some(manifest_gate(1, 3, ManifestGateKind::HashMismatch))
            ),
            Some(GateSatisfaction::ManifestGuardRecovered),
            "a kind change for the same manifest still means the previous manifest condition changed"
        );
        assert_eq!(gate_satisfaction(None, None), None);
        assert_eq!(
            gate_satisfaction(
                Some(GateReason::MissingLocalData),
                Some(GateReason::MissingLocalData)
            ),
            None
        );
        assert_eq!(
            gate_satisfaction(
                Some(manifest_gate(1, 3, ManifestGateKind::Missing)),
                Some(manifest_gate(1, 3, ManifestGateKind::Missing))
            ),
            None,
            "unchanged manifest gate should not synthesize recovery progress"
        );
    }
}
