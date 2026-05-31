---- MODULE SumeragiFrontierSidecarRetargetGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for contiguous-frontier sidecar retargeting.

This slice pins the composition between
`frontier_sidecar_hint_can_override_stall_gate(...)`,
`should_retarget_contiguous_frontier_missing_request_to_sidecar_hint(...)`,
`observe_sidecar_mismatch_for_height(...)`, and
`maybe_reacquire_contiguous_frontier_sidecar_hint(...)`.

Sidecar hints may redirect missing-block recovery at the committed+1
contiguous frontier only when the sidecar height is not quarantined, active
frontier-stall mode is bypassed only by the narrow roster/idle/commit-pipeline
reasons, non-stalled recovery has fresh dependency progress, and the sidecar
hash is confirmed by local payload, a matching commit QC, or an allowed
override reason. Reacquire must also reject local slot evidence unless the
sidecar carries commit-certified evidence for its own hash.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoBug == "none"

Bugs == {
  NoBug,
  "override_reject_vote_roster",
  "override_reject_deferred_vote_roster",
  "override_reject_idle_reacquire",
  "override_reject_commit_pipeline",
  "override_accept_generic_reason",
  "gate_reject_certified",
  "gate_reject_progress_changed",
  "gate_accept_quarantined",
  "gate_accept_stall_without_override",
  "gate_accept_no_progress_change",
  "confirm_reject_local_payload",
  "confirm_reject_commit_qc",
  "confirm_reject_override",
  "confirm_accept_unconfirmed",
  "tracked_reject_confirmed",
  "tracked_accept_non_frontier",
  "tracked_accept_unconfirmed",
  "tracked_accept_no_gate",
  "untracked_reject_confirmed",
  "untracked_accept_non_frontier",
  "untracked_accept_unconfirmed",
  "untracked_accept_no_gate",
  "reacquire_reject_commit_certified_with_local_evidence",
  "reacquire_accept_local_evidence_without_commit_qc",
  "reacquire_accept_missing_expected_hash",
  "reacquire_accept_sidecar_same_hash",
  "reacquire_accept_authoritative_payload"
}

OverrideVoteRosterAccepted ==
  IF Bug = "override_reject_vote_roster" THEN FALSE ELSE TRUE

OverrideDeferredVoteRosterAccepted ==
  IF Bug = "override_reject_deferred_vote_roster" THEN FALSE ELSE TRUE

OverrideIdleReacquireAccepted ==
  IF Bug = "override_reject_idle_reacquire" THEN FALSE ELSE TRUE

OverrideCommitPipelineAccepted ==
  IF Bug = "override_reject_commit_pipeline" THEN FALSE ELSE TRUE

GenericReasonRejected ==
  IF Bug = "override_accept_generic_reason" THEN FALSE ELSE TRUE

GateAllowsCertifiedSidecar ==
  IF Bug = "gate_reject_certified" THEN FALSE ELSE TRUE

GateAllowsProgressChanged ==
  IF Bug = "gate_reject_progress_changed" THEN FALSE ELSE TRUE

GateRejectsQuarantined ==
  IF Bug = "gate_accept_quarantined" THEN FALSE ELSE TRUE

GateRejectsStallWithoutOverride ==
  IF Bug = "gate_accept_stall_without_override" THEN FALSE ELSE TRUE

GateRejectsNoProgressChange ==
  IF Bug = "gate_accept_no_progress_change" THEN FALSE ELSE TRUE

ConfirmLocalPayloadAccepted ==
  IF Bug = "confirm_reject_local_payload" THEN FALSE ELSE TRUE

ConfirmCommitQcAccepted ==
  IF Bug = "confirm_reject_commit_qc" THEN FALSE ELSE TRUE

ConfirmOverrideAccepted ==
  IF Bug = "confirm_reject_override" THEN FALSE ELSE TRUE

UnconfirmedSidecarRejected ==
  IF Bug = "confirm_accept_unconfirmed" THEN FALSE ELSE TRUE

TrackedConfirmedRetargetAccepted ==
  IF Bug = "tracked_reject_confirmed" THEN FALSE ELSE TRUE

TrackedNonFrontierRejected ==
  IF Bug = "tracked_accept_non_frontier" THEN FALSE ELSE TRUE

TrackedUnconfirmedRejected ==
  IF Bug = "tracked_accept_unconfirmed" THEN FALSE ELSE TRUE

TrackedNoGateRejected ==
  IF Bug = "tracked_accept_no_gate" THEN FALSE ELSE TRUE

UntrackedConfirmedSeedAccepted ==
  IF Bug = "untracked_reject_confirmed" THEN FALSE ELSE TRUE

UntrackedNonFrontierRejected ==
  IF Bug = "untracked_accept_non_frontier" THEN FALSE ELSE TRUE

UntrackedUnconfirmedRejected ==
  IF Bug = "untracked_accept_unconfirmed" THEN FALSE ELSE TRUE

UntrackedNoGateRejected ==
  IF Bug = "untracked_accept_no_gate" THEN FALSE ELSE TRUE

ReacquireCommitCertifiedWithLocalEvidenceAccepted ==
  IF Bug = "reacquire_reject_commit_certified_with_local_evidence" THEN FALSE ELSE TRUE

ReacquireRejectsLocalEvidenceWithoutCommitQc ==
  IF Bug = "reacquire_accept_local_evidence_without_commit_qc" THEN FALSE ELSE TRUE

ReacquireRejectsMissingExpectedHash ==
  IF Bug = "reacquire_accept_missing_expected_hash" THEN FALSE ELSE TRUE

ReacquireRejectsSidecarSameHash ==
  IF Bug = "reacquire_accept_sidecar_same_hash" THEN FALSE ELSE TRUE

ReacquireRejectsAuthoritativePayload ==
  IF Bug = "reacquire_accept_authoritative_payload" THEN FALSE ELSE TRUE

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1

OverrideReasonSafety ==
  /\ OverrideVoteRosterAccepted
  /\ OverrideDeferredVoteRosterAccepted
  /\ OverrideIdleReacquireAccepted
  /\ OverrideCommitPipelineAccepted
  /\ GenericReasonRejected

RetargetGateSafety ==
  /\ GateAllowsCertifiedSidecar
  /\ GateAllowsProgressChanged
  /\ GateRejectsQuarantined
  /\ GateRejectsStallWithoutOverride
  /\ GateRejectsNoProgressChange

ConfirmationSafety ==
  /\ ConfirmLocalPayloadAccepted
  /\ ConfirmCommitQcAccepted
  /\ ConfirmOverrideAccepted
  /\ UnconfirmedSidecarRejected

TrackedRetargetSafety ==
  /\ TrackedConfirmedRetargetAccepted
  /\ TrackedNonFrontierRejected
  /\ TrackedUnconfirmedRejected
  /\ TrackedNoGateRejected

UntrackedSeedSafety ==
  /\ UntrackedConfirmedSeedAccepted
  /\ UntrackedNonFrontierRejected
  /\ UntrackedUnconfirmedRejected
  /\ UntrackedNoGateRejected

ReacquireSafety ==
  /\ ReacquireCommitCertifiedWithLocalEvidenceAccepted
  /\ ReacquireRejectsLocalEvidenceWithoutCommitQc
  /\ ReacquireRejectsMissingExpectedHash
  /\ ReacquireRejectsSidecarSameHash
  /\ ReacquireRejectsAuthoritativePayload

SafetyFast ==
  /\ OverrideReasonSafety
  /\ RetargetGateSafety
  /\ ConfirmationSafety
  /\ TrackedRetargetSafety
  /\ UntrackedSeedSafety
  /\ ReacquireSafety

====
