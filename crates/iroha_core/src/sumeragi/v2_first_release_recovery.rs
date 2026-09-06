//! Split-module import seam for production Sumeragi v2 lifecycle recovery.
//!
//! [`super::v2_lifecycle_coordinator`] is the sole production lifecycle
//! authority. This module re-exports the recovery/replay types used by split
//! runtime components without introducing a second scheduler or ledger owner.
pub(in crate::sumeragi) use super::v2_lifecycle_coordinator::{
    AuthenticatedCompleteTipPredecessorStorageV1, CompleteTipPayloadStoreOpenTargetV1,
    CompleteTipPredecessorStorageErrorV1, LifecycleContext, LifecycleDigest,
    LifecycleReplayAuthorityV1, LocalBodyPreIntentReplaySealV1,
    LocalProposalIntentReplayEvidenceV1, LocalProposalReadyReplayEvidenceV1,
    LocalValidateReplayEvidenceV1, RemoteProposalFetchReplayEvidenceV1,
    RetiredRecoveredCompleteTipActivationAuthorityV1, open_complete_tip_predecessor_storage,
};
#[cfg(all(test, feature = "bls"))]
pub(crate) use super::v2_lifecycle_coordinator::{
    complete_tip_restart_activation_fixture, run_complete_tip_retirement_release_regressions,
};
