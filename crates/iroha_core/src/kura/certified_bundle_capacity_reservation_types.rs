/// Exact active autonomous lane-Commit identity protected by one pre-WSV
/// certified/bundle publication envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct CertifiedBundleCapacityIdentity {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    proposal_height: u64,
    lane_block_height: u64,
    lane_block_view: u64,
    lane_block_descriptor_hash: Hash,
    proposal_hash: Hash,
    autonomous_chain_id_hash: Hash,
    autonomous_epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CertifiedBundleCapacityComponent {
    LatestCertifiedFrontier,
    CertifiedPair,
    AutonomousBundlePair,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CertifiedBundleAppendRecovery {
    intent: BoundProgressAppendIntentV1,
    has_durable_intent: bool,
    physical_temp_bytes: u64,
}

/// Immutable byte plan reconstructed from the exact READY-bearing certified
/// frontier and its independently authenticated autonomous source.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CertifiedBundleCapacityPlan {
    identity: CertifiedBundleCapacityIdentity,
    certified_bytes_hash: Hash,
    frontier_bytes_hash: Hash,
    bundle_bytes_hash: Hash,
    component_bytes: BTreeMap<CertifiedBundleCapacityComponent, u64>,
    component_transient_bytes: BTreeMap<CertifiedBundleCapacityComponent, u64>,
    startup_physical_credit_bytes: u64,
}

/// Process-local capacity authority. Durable frontier bytes are the restart
/// source; the map only records which exact publication components still need
/// configured headroom in this process.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CertifiedBundleCapacityReservation {
    plan: CertifiedBundleCapacityPlan,
    outstanding_components: BTreeSet<CertifiedBundleCapacityComponent>,
}

impl CertifiedBundleCapacityReservation {
    fn reserved_bytes(&self) -> Option<u64> {
        if self.outstanding_components.is_empty() {
            return Some(0);
        }
        let stable = self.outstanding_components.iter().try_fold(
            0_u64,
            |total, component| total.checked_add(*self.plan.component_bytes.get(component)?),
        )?;
        let transient = self
            .outstanding_components
            .iter()
            .try_fold(0_u64, |maximum, component| {
                Some(maximum.max(*self.plan.component_transient_bytes.get(component)?))
            })?;
        stable.checked_add(transient)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingControlSidecarLimits {
    certified_merge_entries: usize,
    queue_plan_admissions: usize,
    aggregate_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingCertifiedMergeVisitOutcome {
    Complete,
    VisitorStopped,
    ScanLimitExceeded,
}

/// Result of a bounded, filtered pending certified-merge evidence scan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingCertifiedMergeEvidenceScan {
    /// The complete bounded inventory was scanned; these hashes matched.
    Complete(Vec<HashOf<MergeLedgerEntry>>),
    /// The stable inventory exceeded the caller's scan limit before decoding.
    LimitExceeded,
}

impl Default for PendingControlSidecarLimits {
    fn default() -> Self {
        Self {
            certified_merge_entries: V2_PENDING_CERTIFIED_MERGE_ENTRY_CAPACITY.get(),
            queue_plan_admissions: V2_PENDING_QUEUE_PLAN_ADMISSION_CAPACITY.get(),
            aggregate_bytes: V2_PENDING_CONTROL_SIDECAR_BYTES.get(),
        }
    }
}

impl PendingControlSidecarLimits {
    fn from_config(config: &SumeragiV2RuntimeLimits, store_root: &Path) -> Result<Self> {
        let limits = Self {
            certified_merge_entries: config.pending_certified_merge_entry_capacity.get(),
            queue_plan_admissions: config.pending_queue_plan_admission_capacity.get(),
            aggregate_bytes: config.pending_control_sidecar_bytes.get(),
        };
        if limits.certified_merge_entries > V2_PENDING_CERTIFIED_MERGE_ENTRY_CAPACITY_MAX {
            return Err(Self::invalid_config(
                store_root,
                "sumeragi.limits.pending_certified_merge_entry_capacity exceeds its hard maximum",
            ));
        }
        if limits.queue_plan_admissions > V2_PENDING_QUEUE_PLAN_ADMISSION_CAPACITY_MAX {
            return Err(Self::invalid_config(
                store_root,
                "sumeragi.limits.pending_queue_plan_admission_capacity exceeds its hard maximum",
            ));
        }
        if limits.aggregate_bytes < V2_PENDING_CONTROL_SIDECAR_BYTES_MIN
            || limits.aggregate_bytes > V2_PENDING_CONTROL_SIDECAR_BYTES_MAX
        {
            return Err(Self::invalid_config(
                store_root,
                "sumeragi.limits.pending_control_sidecar_bytes is outside its hard bounds",
            ));
        }
        Ok(limits)
    }

    fn invalid_config(store_root: &Path, message: &'static str) -> Error {
        Error::IO(
            std::io::Error::new(ErrorKind::InvalidInput, message),
            store_root.to_path_buf(),
        )
    }

    const fn merge_bytes_within_limit(self, bytes: usize) -> bool {
        bytes <= self.aggregate_bytes
    }

    const fn combined_bytes_within_limit(
        self,
        pending_merge_bytes: usize,
        pending_queue_plan_admission_bytes: usize,
    ) -> bool {
        match pending_merge_bytes.checked_add(pending_queue_plan_admission_bytes) {
            Some(total) => total <= self.aggregate_bytes,
            None => false,
        }
    }
}
