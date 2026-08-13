//! First-release lifecycle recovery seams retained by production Sumeragi v2.
//!
//! The generic lifecycle-coordinator staging island is not a production
//! scheduler.  This module is the narrow ownership boundary for the recovery
//! and replay values that are already consumed by first-release runtime paths.
//! Phase A keeps the definitions delegated to the reviewed implementation so
//! callers can move without changing bytes or behavior; Phase B moves the
//! closed implementation here before retiring the unwired staging island.

pub(in crate::sumeragi) use super::v2_lifecycle_coordinator::{
    AuthenticatedCompleteTipPredecessorStorageV1, CompleteTipPredecessorStorageErrorV1,
    LifecycleContext, LifecycleDigest, LifecycleReplayAuthorityV1, LocalBodyPreIntentReplaySealV1,
    LocalProposalIntentReplayEvidenceV1, LocalProposalReadyReplayEvidenceV1,
    LocalValidateReplayEvidenceV1, RemoteProposalFetchReplayEvidenceV1,
    RetiredRecoveredCompleteTipActivationAuthorityV1, open_complete_tip_predecessor_storage,
};

#[cfg(all(test, feature = "bls"))]
pub(crate) use super::v2_lifecycle_coordinator::{
    complete_tip_restart_activation_fixture, run_complete_tip_retirement_release_regressions,
};
