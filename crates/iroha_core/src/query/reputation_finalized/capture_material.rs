// Canonical successor material captured from one immutable committed State view.

struct CapturedReputationCandidate {
    next_state: ReputationReconstructionStateV1,
    authority_policy_history: Vec<ReputationJournalAuthorityPolicyRecordV1>,
}

// All fallible preparation borrows the original successor. Only successful
// material consumes it, so a local refusal cannot discard or recapture execution.
struct PreparedReputationMaterial {
    policies: Vec<PreparedReputationPolicy>,
    anchor: PersistedReputationFinalizedAnchorV1,
    anchor_digest: [u8; 32],
    anchor_bytes: Vec<u8>,
    anchor_path: PathBuf,
    full_projection: Option<ReputationFinalizedProjectionV1>,
    total_bytes: u64,
    anchor_count: usize,
    generation: u64,
}

impl PreparedReputationMaterial {
    fn finish(self, next_state: ReputationReconstructionStateV1) -> PreparedReputationState {
        let Self {
            policies,
            anchor,
            anchor_digest,
            anchor_bytes,
            anchor_path,
            full_projection,
            total_bytes,
            anchor_count,
            generation,
        } = self;
        PreparedReputationState {
            policies,
            anchor,
            anchor_digest,
            anchor_bytes,
            anchor_path,
            next_state,
            full_projection,
            total_bytes,
            anchor_count,
            generation,
        }
    }
}
