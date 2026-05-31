---- MODULE SumeragiBlockSyncRecoveryModeGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for stale BlockCreated and block-sync recovery mode
helpers.

This slice captures `stale_height(...)`, `allow_stale_block_created(...)`,
and the accessor methods on `BlockSyncRecoveryMode` from
`main_loop/proposal_handlers.rs`. It abstracts recovery modes and optional
commit-QC epochs as finite integers while preserving the observable contract:
heights at or below the committed height are stale; stale BlockCreated recovery
is admitted by any one recovery signal; only signed-quorum and commit-evidence
repair may supersede an authoritative frontier owner; only requested-payload
and commit-evidence repair may bypass a missing stale request; only
commit-evidence repair with the explicit revival flag may revive an aborted
pending block without a local commit QC; and only commit-evidence repair
returns an observed commit-QC epoch.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

StaleEqual == 1
StaleBelow == 2
StaleAbove == 3
AllowMissingRequest == 4
AllowRetainedMatch == 5
AllowRecoveryEvidence == 6
AllowNoSignal == 7
PayloadOnlySupersede == 8
RequestedPayloadSupersede == 9
SignedQuorumSupersede == 10
CommitEvidenceSupersede == 11
PayloadOnlyStaleWithoutRequest == 12
RequestedPayloadStaleWithoutRequest == 13
SignedQuorumStaleWithoutRequest == 14
CommitEvidenceStaleWithoutRequest == 15
PayloadOnlyAbortedRevival == 16
RequestedPayloadAbortedRevival == 17
SignedQuorumAbortedRevival == 18
CommitEvidenceNoFlagAbortedRevival == 19
CommitEvidenceFlagAbortedRevival == 20
PayloadOnlyEpoch == 21
RequestedPayloadEpoch == 22
SignedQuorumEpoch == 23
CommitEvidenceNoEpoch == 24
CommitEvidenceSomeEpoch == 25

Candidates == 1..25

StaleTrue == 1
StaleFalse == 2
AdmitStaleBlock == 3
RejectStaleBlock == 4
AllowSupersede == 5
RejectSupersede == 6
AllowStaleWithoutRequest == 7
RejectStaleWithoutRequest == 8
AllowAbortedRevival == 9
RejectAbortedRevival == 10
EpochNone == 11
EpochSome == 12

SpecActions(candidate) ==
  CASE candidate \in {StaleEqual, StaleBelow} ->
      {StaleTrue}
    [] candidate = StaleAbove ->
      {StaleFalse}
    [] candidate \in {AllowMissingRequest,
                      AllowRetainedMatch,
                      AllowRecoveryEvidence} ->
      {AdmitStaleBlock}
    [] candidate = AllowNoSignal ->
      {RejectStaleBlock}
    [] candidate \in {SignedQuorumSupersede, CommitEvidenceSupersede} ->
      {AllowSupersede}
    [] candidate \in {PayloadOnlySupersede, RequestedPayloadSupersede} ->
      {RejectSupersede}
    [] candidate \in {RequestedPayloadStaleWithoutRequest,
                      CommitEvidenceStaleWithoutRequest} ->
      {AllowStaleWithoutRequest}
    [] candidate \in {PayloadOnlyStaleWithoutRequest,
                      SignedQuorumStaleWithoutRequest} ->
      {RejectStaleWithoutRequest}
    [] candidate = CommitEvidenceFlagAbortedRevival ->
      {AllowAbortedRevival}
    [] candidate \in {PayloadOnlyAbortedRevival,
                      RequestedPayloadAbortedRevival,
                      SignedQuorumAbortedRevival,
                      CommitEvidenceNoFlagAbortedRevival} ->
      {RejectAbortedRevival}
    [] candidate = CommitEvidenceSomeEpoch ->
      {EpochSome}
    [] candidate \in {PayloadOnlyEpoch,
                      RequestedPayloadEpoch,
                      SignedQuorumEpoch,
                      CommitEvidenceNoEpoch} ->
      {EpochNone}
    [] OTHER ->
      {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = StaleEqual /\ Bug = "stale_height_strict" ->
      {StaleFalse}
    [] candidate = StaleBelow /\ Bug = "stale_height_rejects_below" ->
      {StaleFalse}
    [] candidate = StaleAbove /\ Bug = "stale_height_allows_above" ->
      {StaleTrue}
    [] candidate = AllowMissingRequest /\
          Bug = "allow_stale_ignores_missing_request" ->
      {RejectStaleBlock}
    [] candidate = AllowRetainedMatch /\
          Bug = "allow_stale_ignores_retained_match" ->
      {RejectStaleBlock}
    [] candidate = AllowRecoveryEvidence /\
          Bug = "allow_stale_ignores_recovery_evidence" ->
      {RejectStaleBlock}
    [] candidate = AllowNoSignal /\
          Bug = "allow_stale_accepts_no_signal" ->
      {AdmitStaleBlock}
    [] candidate = PayloadOnlySupersede /\
          Bug = "supersede_allows_payload_only" ->
      {AllowSupersede}
    [] candidate = RequestedPayloadSupersede /\
          Bug = "supersede_allows_requested_payload" ->
      {AllowSupersede}
    [] candidate = SignedQuorumSupersede /\
          Bug = "supersede_rejects_signed_quorum" ->
      {RejectSupersede}
    [] candidate = CommitEvidenceSupersede /\
          Bug = "supersede_rejects_commit_evidence" ->
      {RejectSupersede}
    [] candidate = PayloadOnlyStaleWithoutRequest /\
          Bug = "stale_without_request_allows_payload_only" ->
      {AllowStaleWithoutRequest}
    [] candidate = RequestedPayloadStaleWithoutRequest /\
          Bug = "stale_without_request_rejects_requested_payload" ->
      {RejectStaleWithoutRequest}
    [] candidate = SignedQuorumStaleWithoutRequest /\
          Bug = "stale_without_request_allows_signed_quorum" ->
      {AllowStaleWithoutRequest}
    [] candidate = CommitEvidenceStaleWithoutRequest /\
          Bug = "stale_without_request_rejects_commit_evidence" ->
      {RejectStaleWithoutRequest}
    [] candidate = PayloadOnlyAbortedRevival /\
          Bug = "aborted_revival_allows_payload_only" ->
      {AllowAbortedRevival}
    [] candidate = RequestedPayloadAbortedRevival /\
          Bug = "aborted_revival_allows_requested_payload" ->
      {AllowAbortedRevival}
    [] candidate = SignedQuorumAbortedRevival /\
          Bug = "aborted_revival_allows_signed_quorum" ->
      {AllowAbortedRevival}
    [] candidate = CommitEvidenceNoFlagAbortedRevival /\
          Bug = "aborted_revival_ignores_false_commit_flag" ->
      {AllowAbortedRevival}
    [] candidate = CommitEvidenceFlagAbortedRevival /\
          Bug = "aborted_revival_ignores_true_commit_flag" ->
      {RejectAbortedRevival}
    [] candidate = PayloadOnlyEpoch /\
          Bug = "epoch_payload_only_returns_some" ->
      {EpochSome}
    [] candidate = RequestedPayloadEpoch /\
          Bug = "epoch_requested_payload_returns_some" ->
      {EpochSome}
    [] candidate = SignedQuorumEpoch /\
          Bug = "epoch_signed_quorum_returns_some" ->
      {EpochSome}
    [] candidate = CommitEvidenceNoEpoch /\
          Bug = "epoch_commit_none_returns_some" ->
      {EpochSome}
    [] candidate = CommitEvidenceSomeEpoch /\
          Bug = "epoch_commit_evidence_drops_epoch" ->
      {EpochNone}
    [] OTHER ->
      spec

Init == checked \in Candidates

Next == UNCHANGED vars

TypeInvariant == checked \in Candidates

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugStaleHeightStrict ==
  ImplementationActions(StaleEqual) = SpecActions(StaleEqual)

BugStaleHeightRejectsBelow ==
  ImplementationActions(StaleBelow) = SpecActions(StaleBelow)

BugStaleHeightAllowsAbove ==
  ImplementationActions(StaleAbove) = SpecActions(StaleAbove)

BugAllowStaleIgnoresMissingRequest ==
  ImplementationActions(AllowMissingRequest) = SpecActions(AllowMissingRequest)

BugAllowStaleIgnoresRetainedMatch ==
  ImplementationActions(AllowRetainedMatch) = SpecActions(AllowRetainedMatch)

BugAllowStaleIgnoresRecoveryEvidence ==
  ImplementationActions(AllowRecoveryEvidence) = SpecActions(AllowRecoveryEvidence)

BugAllowStaleAcceptsNoSignal ==
  ImplementationActions(AllowNoSignal) = SpecActions(AllowNoSignal)

BugSupersedeAllowsPayloadOnly ==
  ImplementationActions(PayloadOnlySupersede) = SpecActions(PayloadOnlySupersede)

BugSupersedeAllowsRequestedPayload ==
  ImplementationActions(RequestedPayloadSupersede) =
    SpecActions(RequestedPayloadSupersede)

BugSupersedeRejectsSignedQuorum ==
  ImplementationActions(SignedQuorumSupersede) = SpecActions(SignedQuorumSupersede)

BugSupersedeRejectsCommitEvidence ==
  ImplementationActions(CommitEvidenceSupersede) = SpecActions(CommitEvidenceSupersede)

BugStaleWithoutRequestAllowsPayloadOnly ==
  ImplementationActions(PayloadOnlyStaleWithoutRequest) =
    SpecActions(PayloadOnlyStaleWithoutRequest)

BugStaleWithoutRequestRejectsRequestedPayload ==
  ImplementationActions(RequestedPayloadStaleWithoutRequest) =
    SpecActions(RequestedPayloadStaleWithoutRequest)

BugStaleWithoutRequestAllowsSignedQuorum ==
  ImplementationActions(SignedQuorumStaleWithoutRequest) =
    SpecActions(SignedQuorumStaleWithoutRequest)

BugStaleWithoutRequestRejectsCommitEvidence ==
  ImplementationActions(CommitEvidenceStaleWithoutRequest) =
    SpecActions(CommitEvidenceStaleWithoutRequest)

BugAbortedRevivalAllowsPayloadOnly ==
  ImplementationActions(PayloadOnlyAbortedRevival) =
    SpecActions(PayloadOnlyAbortedRevival)

BugAbortedRevivalAllowsRequestedPayload ==
  ImplementationActions(RequestedPayloadAbortedRevival) =
    SpecActions(RequestedPayloadAbortedRevival)

BugAbortedRevivalAllowsSignedQuorum ==
  ImplementationActions(SignedQuorumAbortedRevival) =
    SpecActions(SignedQuorumAbortedRevival)

BugAbortedRevivalIgnoresFalseCommitFlag ==
  ImplementationActions(CommitEvidenceNoFlagAbortedRevival) =
    SpecActions(CommitEvidenceNoFlagAbortedRevival)

BugAbortedRevivalIgnoresTrueCommitFlag ==
  ImplementationActions(CommitEvidenceFlagAbortedRevival) =
    SpecActions(CommitEvidenceFlagAbortedRevival)

BugEpochPayloadOnlyReturnsSome ==
  ImplementationActions(PayloadOnlyEpoch) = SpecActions(PayloadOnlyEpoch)

BugEpochRequestedPayloadReturnsSome ==
  ImplementationActions(RequestedPayloadEpoch) = SpecActions(RequestedPayloadEpoch)

BugEpochSignedQuorumReturnsSome ==
  ImplementationActions(SignedQuorumEpoch) = SpecActions(SignedQuorumEpoch)

BugEpochCommitNoneReturnsSome ==
  ImplementationActions(CommitEvidenceNoEpoch) = SpecActions(CommitEvidenceNoEpoch)

BugEpochCommitEvidenceDropsEpoch ==
  ImplementationActions(CommitEvidenceSomeEpoch) = SpecActions(CommitEvidenceSomeEpoch)

====
