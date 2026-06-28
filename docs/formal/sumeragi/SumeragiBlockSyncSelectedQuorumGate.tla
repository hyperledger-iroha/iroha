---- MODULE SumeragiBlockSyncSelectedQuorumGate ----
EXTENDS Integers

(***************************************************************************
A bounded boolean model for the selected-roster BlockSyncUpdate quorum and
missing-QC repair gate in `handle_block_sync_update(...)`.

After selected-roster signatures and QC candidate validation, the live path:

  * drops an invalid incoming QC only when neither block-signature quorum nor
    commit-cert/checkpoint evidence can justify keeping the update;
  * admits updates that already have QC evidence, commit-cert evidence, block
    signature quorum, checkpoint evidence, or a sparse exact-frontier missing
    block request with at least one validated block signer;
  * requests the pending block when a sparse update is missing QC evidence,
    then either defers vote-only NPoS frontier payloads or recomputes quorum
    with the newly tracked missing-block request; and
  * if quorum is still unavailable, keeps exact frontier body repair in-slot or
    records a quorum-missing drop with the invalid-signature drop metric.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "qc_evidence_quorum",
  "commit_cert_quorum",
  "signature_quorum",
  "checkpoint_quorum",
  "explicit_frontier_sparse",
  "tracked_frontier_sparse",
  "frontier_sparse_with_commit_votes",
  "sparse_no_signers",
  "requested_nonfrontier_no_quorum",
  "unrequested_sparse_no_quorum",
  "missing_qc_request_npos_vote_only",
  "missing_qc_request_classic_continue",
  "missing_qc_request_npos_non_vote_continue",
  "missing_qc_request_nonfrontier_drop",
  "missing_qc_request_zero_signer_drop",
  "missing_qc_request_backoff_repair",
  "repair_deferred",
  "quorum_drop",
  "invalid_qc_drop",
  "invalid_qc_block_quorum",
  "invalid_qc_checkpoint"
}

QcEvidence(c) ==
  c = "qc_evidence_quorum"

CommitCert(c) ==
  c = "commit_cert_quorum"

SignatureQuorum(c) ==
  c \in {"signature_quorum", "invalid_qc_block_quorum"}

Checkpoint(c) ==
  c \in {"checkpoint_quorum", "invalid_qc_checkpoint"}

InvalidQc(c) ==
  c \in {
    "invalid_qc_drop",
    "invalid_qc_block_quorum",
    "invalid_qc_checkpoint"
  }

BlockQuorumMet(c) ==
  SignatureQuorum(c)

ExplicitRequested(c) ==
  c = "explicit_frontier_sparse"

RequestedAtEntry(c) ==
  c \in {
    "tracked_frontier_sparse",
    "frontier_sparse_with_commit_votes",
    "sparse_no_signers",
    "requested_nonfrontier_no_quorum",
    "repair_deferred",
    "quorum_drop"
  }

ExactContiguousFrontier(c) ==
  c \in {
    "tracked_frontier_sparse",
    "frontier_sparse_with_commit_votes",
    "sparse_no_signers"
  }

FrontierNextHeight(c) ==
  c \in {
    "explicit_frontier_sparse",
    "tracked_frontier_sparse",
    "frontier_sparse_with_commit_votes",
    "sparse_no_signers",
    "unrequested_sparse_no_quorum",
    "missing_qc_request_npos_vote_only",
    "missing_qc_request_classic_continue",
    "missing_qc_request_npos_non_vote_continue",
    "missing_qc_request_zero_signer_drop",
    "missing_qc_request_backoff_repair"
  }

HasBlockSigner(c) ==
  ~(c \in {
      "sparse_no_signers",
      "missing_qc_request_zero_signer_drop"
    })

HasCommitVotes(c) ==
  c = "frontier_sparse_with_commit_votes"

MaybeRequestWouldTrack(c) ==
  c \in {
    "missing_qc_request_npos_vote_only",
    "missing_qc_request_classic_continue",
    "missing_qc_request_npos_non_vote_continue",
    "missing_qc_request_nonfrontier_drop",
    "missing_qc_request_zero_signer_drop"
  }

NposMode(c) ==
  c \in {
    "missing_qc_request_npos_vote_only",
    "missing_qc_request_npos_non_vote_continue"
  }

VoteOnlyFrontier(c) ==
  c = "missing_qc_request_npos_vote_only"

RepairKept(c) ==
  c \in {
    "missing_qc_request_backoff_repair",
    "repair_deferred"
  }

SpecInvalidQcDrop(c) ==
  /\ InvalidQc(c)
  /\ ~BlockQuorumMet(c)
  /\ ~CommitCert(c)
  /\ ~Checkpoint(c)

SpecCanCheckQuorum(c) ==
  ~SpecInvalidQcDrop(c)

SpecSparseExactFrontierRequest(c) ==
  /\ RequestedAtEntry(c)
  /\ ExactContiguousFrontier(c)
  /\ ~QcEvidence(c)
  /\ ~Checkpoint(c)
  /\ ~HasCommitVotes(c)

SpecInitialMissingRequestArg(c) ==
  ExplicitRequested(c) \/ SpecSparseExactFrontierRequest(c)

SpecInitialSparseQuorum(c) ==
  /\ SpecInitialMissingRequestArg(c)
  /\ FrontierNextHeight(c)
  /\ HasBlockSigner(c)

SpecQuorumInitial(c) ==
  \/ QcEvidence(c)
  \/ CommitCert(c)
  \/ SignatureQuorum(c)
  \/ Checkpoint(c)
  \/ SpecInitialSparseQuorum(c)

SpecMaybeRequestCalled(c) ==
  /\ SpecCanCheckQuorum(c)
  /\ ~SpecQuorumInitial(c)
  /\ ~QcEvidence(c)
  /\ ~CommitCert(c)
  /\ ~Checkpoint(c)
  /\ ~BlockQuorumMet(c)
  /\ ~RequestedAtEntry(c)

SpecMaybeRequestTracked(c) ==
  SpecMaybeRequestCalled(c) /\ MaybeRequestWouldTrack(c)

SpecNposVoteOnlyDeferred(c) ==
  /\ SpecMaybeRequestTracked(c)
  /\ NposMode(c)
  /\ VoteOnlyFrontier(c)
  /\ ~ExplicitRequested(c)

SpecRequestedAfterMaybe(c) ==
  SpecMaybeRequestTracked(c) /\ ~SpecNposVoteOnlyDeferred(c)

SpecQuorumAfterMaybe(c) ==
  \/ SpecQuorumInitial(c)
  \/ /\ SpecRequestedAfterMaybe(c)
     /\ FrontierNextHeight(c)
     /\ HasBlockSigner(c)

SpecRepairCalled(c) ==
  /\ SpecCanCheckQuorum(c)
  /\ ~SpecNposVoteOnlyDeferred(c)
  /\ ~SpecQuorumAfterMaybe(c)

SpecRepairDeferred(c) ==
  SpecRepairCalled(c) /\ RepairKept(c)

SpecDropQuorumMissing(c) ==
  SpecRepairCalled(c) /\ ~RepairKept(c)

SpecRecordDeferredQuorum(c) ==
  SpecNposVoteOnlyDeferred(c) \/ SpecRepairDeferred(c)

SpecRecordDroppedInvalid(c) ==
  SpecInvalidQcDrop(c)

SpecRecordDroppedQuorum(c) ==
  SpecDropQuorumMissing(c)

SpecDropInvalidSignatureMetric(c) ==
  SpecDropQuorumMissing(c)

SpecReturnsOk(c) ==
  \/ SpecInvalidQcDrop(c)
  \/ SpecNposVoteOnlyDeferred(c)
  \/ SpecRepairDeferred(c)
  \/ SpecDropQuorumMissing(c)

SpecContinuesToApply(c) ==
  /\ SpecCanCheckQuorum(c)
  /\ ~SpecNposVoteOnlyDeferred(c)
  /\ SpecQuorumAfterMaybe(c)

SpecClearsMissing(c) ==
  FALSE

ActualQuorumInitial(c) ==
  IF Bug = "qc_evidence_ignored"
     /\ c = "qc_evidence_quorum"
  THEN FALSE
  ELSE IF Bug = "commit_cert_ignored"
          /\ c = "commit_cert_quorum" THEN FALSE
  ELSE IF Bug = "signature_quorum_ignored"
          /\ c = "signature_quorum" THEN FALSE
  ELSE IF Bug = "checkpoint_ignored"
          /\ c = "checkpoint_quorum" THEN FALSE
  ELSE IF Bug = "frontier_sparse_not_allowed"
          /\ c = "explicit_frontier_sparse" THEN FALSE
  ELSE IF Bug = "tracked_frontier_sparse_not_allowed"
          /\ c = "tracked_frontier_sparse" THEN FALSE
  ELSE IF Bug = "frontier_sparse_allows_zero_signers"
          /\ c = "sparse_no_signers" THEN TRUE
  ELSE IF Bug = "nonfrontier_requested_allowed"
          /\ c = "requested_nonfrontier_no_quorum" THEN TRUE
  ELSE IF Bug = "sparse_ignores_commit_votes"
          /\ c = "frontier_sparse_with_commit_votes" THEN TRUE
  ELSE IF Bug = "unrequested_sparse_allowed"
          /\ c = "unrequested_sparse_no_quorum" THEN TRUE
  ELSE SpecQuorumInitial(c)

ActualMaybeRequestCalled(c) ==
  IF Bug = "maybe_request_skipped"
     /\ c = "unrequested_sparse_no_quorum"
  THEN FALSE
  ELSE IF Bug = "maybe_request_with_evidence"
          /\ c = "qc_evidence_quorum" THEN TRUE
  ELSE IF Bug = "maybe_request_with_requested"
          /\ c = "tracked_frontier_sparse" THEN TRUE
  ELSE SpecMaybeRequestCalled(c)

ActualRequestedAfterMaybe(c) ==
  IF Bug = "request_not_marked"
     /\ c = "missing_qc_request_classic_continue"
  THEN FALSE
  ELSE SpecRequestedAfterMaybe(c)

ActualQuorumAfterMaybe(c) ==
  IF Bug = "maybe_request_not_recomputing_quorum"
     /\ c = "missing_qc_request_classic_continue"
  THEN FALSE
  ELSE SpecQuorumAfterMaybe(c)

ActualNposVoteOnlyDeferred(c) ==
  IF Bug = "npos_vote_only_not_deferred"
     /\ c = "missing_qc_request_npos_vote_only"
  THEN FALSE
  ELSE SpecNposVoteOnlyDeferred(c)

ActualRepairCalled(c) ==
  IF Bug = "repair_not_called"
     /\ c = "repair_deferred"
  THEN FALSE
  ELSE SpecRepairCalled(c)

ActualRepairDeferred(c) ==
  SpecRepairDeferred(c)

ActualDropQuorumMissing(c) ==
  SpecDropQuorumMissing(c)

ActualInvalidQcDrop(c) ==
  IF Bug = "invalid_qc_not_dropped"
     /\ c = "invalid_qc_drop"
  THEN FALSE
  ELSE IF Bug = "invalid_qc_drop_with_block_quorum"
          /\ c = "invalid_qc_block_quorum" THEN TRUE
  ELSE IF Bug = "invalid_qc_drop_with_checkpoint"
          /\ c = "invalid_qc_checkpoint" THEN TRUE
  ELSE SpecInvalidQcDrop(c)

ActualRecordDeferredQuorum(c) ==
  IF Bug = "repair_deferred_no_status"
     /\ c = "repair_deferred"
  THEN FALSE
  ELSE SpecRecordDeferredQuorum(c)

ActualRecordDroppedInvalid(c) ==
  IF Bug = "repair_deferred_wrong_reason"
     /\ c = "repair_deferred"
  THEN TRUE
  ELSE SpecRecordDroppedInvalid(c)

ActualRecordDroppedQuorum(c) ==
  IF Bug = "drop_not_recorded"
     /\ c = "quorum_drop"
  THEN FALSE
  ELSE SpecRecordDroppedQuorum(c)

ActualDropInvalidSignatureMetric(c) ==
  IF Bug = "drop_no_metric"
     /\ c = "quorum_drop"
  THEN FALSE
  ELSE SpecDropInvalidSignatureMetric(c)

ActualReturnsOk(c) ==
  IF Bug = "classic_request_returns_early"
     /\ c = "missing_qc_request_classic_continue"
  THEN TRUE
  ELSE IF Bug = "drop_returns_error"
          /\ c = "quorum_drop" THEN FALSE
  ELSE IF Bug = "invalid_qc_returns_error"
          /\ c = "invalid_qc_drop" THEN FALSE
  ELSE SpecReturnsOk(c)

ActualContinuesToApply(c) ==
  IF Bug = "classic_request_returns_early"
     /\ c = "missing_qc_request_classic_continue"
  THEN FALSE
  ELSE IF Bug = "drop_continues"
          /\ c = "quorum_drop" THEN TRUE
  ELSE SpecContinuesToApply(c)

ActualClearsMissing(c) ==
  IF Bug = "invalid_qc_clears_missing"
     /\ c = "invalid_qc_drop"
  THEN TRUE
  ELSE SpecClearsMissing(c)

ActualRecordReasonInvalidPayload(c) ==
  IF Bug = "drop_wrong_reason"
     /\ c = "quorum_drop"
  THEN TRUE
  ELSE SpecRecordDroppedInvalid(c)

ActualRecordReasonQuorumMissing(c) ==
  IF Bug = "drop_wrong_reason"
     /\ c = "quorum_drop"
  THEN FALSE
  ELSE SpecRecordDeferredQuorum(c) \/ SpecRecordDroppedQuorum(c)

SpecTrace(c) ==
  [
    quorum_initial |-> SpecQuorumInitial(c),
    maybe_request_called |-> SpecMaybeRequestCalled(c),
    requested_after_maybe |-> SpecRequestedAfterMaybe(c),
    quorum_after_maybe |-> SpecQuorumAfterMaybe(c),
    npos_vote_only_deferred |-> SpecNposVoteOnlyDeferred(c),
    repair_called |-> SpecRepairCalled(c),
    repair_deferred |-> SpecRepairDeferred(c),
    drop_quorum_missing |-> SpecDropQuorumMissing(c),
    invalid_qc_drop |-> SpecInvalidQcDrop(c),
    record_deferred_quorum |-> SpecRecordDeferredQuorum(c),
    record_dropped_invalid |-> SpecRecordDroppedInvalid(c),
    record_dropped_quorum |-> SpecRecordDroppedQuorum(c),
    record_reason_invalid_payload |-> SpecRecordDroppedInvalid(c),
    record_reason_quorum_missing |-> SpecRecordDeferredQuorum(c) \/ SpecRecordDroppedQuorum(c),
    drop_invalid_signature_metric |-> SpecDropInvalidSignatureMetric(c),
    returns_ok |-> SpecReturnsOk(c),
    continues_to_apply |-> SpecContinuesToApply(c),
    clears_missing |-> SpecClearsMissing(c)
  ]

ActualTrace(c) ==
  [
    quorum_initial |-> ActualQuorumInitial(c),
    maybe_request_called |-> ActualMaybeRequestCalled(c),
    requested_after_maybe |-> ActualRequestedAfterMaybe(c),
    quorum_after_maybe |-> ActualQuorumAfterMaybe(c),
    npos_vote_only_deferred |-> ActualNposVoteOnlyDeferred(c),
    repair_called |-> ActualRepairCalled(c),
    repair_deferred |-> ActualRepairDeferred(c),
    drop_quorum_missing |-> ActualDropQuorumMissing(c),
    invalid_qc_drop |-> ActualInvalidQcDrop(c),
    record_deferred_quorum |-> ActualRecordDeferredQuorum(c),
    record_dropped_invalid |-> ActualRecordDroppedInvalid(c),
    record_dropped_quorum |-> ActualRecordDroppedQuorum(c),
    record_reason_invalid_payload |-> ActualRecordReasonInvalidPayload(c),
    record_reason_quorum_missing |-> ActualRecordReasonQuorumMissing(c),
    drop_invalid_signature_metric |-> ActualDropInvalidSignatureMetric(c),
    returns_ok |-> ActualReturnsOk(c),
    continues_to_apply |-> ActualContinuesToApply(c),
    clears_missing |-> ActualClearsMissing(c)
  ]

Matches(c) ==
  /\ ActualQuorumInitial(c) = SpecQuorumInitial(c)
  /\ ActualMaybeRequestCalled(c) = SpecMaybeRequestCalled(c)
  /\ ActualRequestedAfterMaybe(c) = SpecRequestedAfterMaybe(c)
  /\ ActualQuorumAfterMaybe(c) = SpecQuorumAfterMaybe(c)
  /\ ActualNposVoteOnlyDeferred(c) = SpecNposVoteOnlyDeferred(c)
  /\ ActualRepairCalled(c) = SpecRepairCalled(c)
  /\ ActualRepairDeferred(c) = SpecRepairDeferred(c)
  /\ ActualDropQuorumMissing(c) = SpecDropQuorumMissing(c)
  /\ ActualInvalidQcDrop(c) = SpecInvalidQcDrop(c)
  /\ ActualRecordDeferredQuorum(c) = SpecRecordDeferredQuorum(c)
  /\ ActualRecordDroppedInvalid(c) = SpecRecordDroppedInvalid(c)
  /\ ActualRecordDroppedQuorum(c) = SpecRecordDroppedQuorum(c)
  /\ ActualRecordReasonInvalidPayload(c) = SpecRecordDroppedInvalid(c)
  /\ ActualRecordReasonQuorumMissing(c)
       = (SpecRecordDeferredQuorum(c) \/ SpecRecordDroppedQuorum(c))
  /\ ActualDropInvalidSignatureMetric(c) = SpecDropInvalidSignatureMetric(c)
  /\ ActualReturnsOk(c) = SpecReturnsOk(c)
  /\ ActualContinuesToApply(c) = SpecContinuesToApply(c)
  /\ ActualClearsMissing(c) = SpecClearsMissing(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "qc_evidence_ignored",
       "commit_cert_ignored",
       "signature_quorum_ignored",
       "checkpoint_ignored",
       "frontier_sparse_not_allowed",
       "tracked_frontier_sparse_not_allowed",
       "frontier_sparse_allows_zero_signers",
       "nonfrontier_requested_allowed",
       "sparse_ignores_commit_votes",
       "unrequested_sparse_allowed",
       "maybe_request_skipped",
       "maybe_request_with_evidence",
       "maybe_request_with_requested",
       "request_not_marked",
       "maybe_request_not_recomputing_quorum",
       "npos_vote_only_not_deferred",
       "classic_request_returns_early",
       "repair_not_called",
       "repair_deferred_no_status",
       "repair_deferred_wrong_reason",
       "drop_not_recorded",
       "drop_wrong_reason",
       "drop_no_metric",
       "drop_returns_error",
       "drop_continues",
       "invalid_qc_not_dropped",
       "invalid_qc_drop_with_block_quorum",
       "invalid_qc_drop_with_checkpoint",
       "invalid_qc_returns_error",
       "invalid_qc_clears_missing"
     }
  /\ checked = 0

SelectedQuorumMatchesSpec ==
  /\ Matches("qc_evidence_quorum")
  /\ Matches("commit_cert_quorum")
  /\ Matches("signature_quorum")
  /\ Matches("checkpoint_quorum")
  /\ Matches("explicit_frontier_sparse")
  /\ Matches("tracked_frontier_sparse")
  /\ Matches("frontier_sparse_with_commit_votes")
  /\ Matches("sparse_no_signers")
  /\ Matches("requested_nonfrontier_no_quorum")
  /\ Matches("unrequested_sparse_no_quorum")
  /\ Matches("missing_qc_request_npos_vote_only")
  /\ Matches("missing_qc_request_classic_continue")
  /\ Matches("missing_qc_request_npos_non_vote_continue")
  /\ Matches("missing_qc_request_nonfrontier_drop")
  /\ Matches("missing_qc_request_zero_signer_drop")
  /\ Matches("missing_qc_request_backoff_repair")
  /\ Matches("repair_deferred")
  /\ Matches("quorum_drop")
  /\ Matches("invalid_qc_drop")
  /\ Matches("invalid_qc_block_quorum")
  /\ Matches("invalid_qc_checkpoint")

BlockSyncSelectedQuorumExactness ==
  SelectedQuorumMatchesSpec

BlockSyncSelectedQuorumCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncSelectedQuorumExactness

SafetyFast ==
  BlockSyncSelectedQuorumExactness

EvidenceQuorum ==
  /\ ActualQuorumInitial("qc_evidence_quorum") = SpecQuorumInitial("qc_evidence_quorum")
  /\ ActualQuorumInitial("commit_cert_quorum") = SpecQuorumInitial("commit_cert_quorum")
  /\ ActualQuorumInitial("signature_quorum") = SpecQuorumInitial("signature_quorum")
  /\ ActualQuorumInitial("checkpoint_quorum") = SpecQuorumInitial("checkpoint_quorum")
  /\ ActualMaybeRequestCalled("qc_evidence_quorum") = SpecMaybeRequestCalled("qc_evidence_quorum")

SparseFrontierQuorum ==
  /\ ActualQuorumInitial("explicit_frontier_sparse") = SpecQuorumInitial("explicit_frontier_sparse")
  /\ ActualQuorumInitial("tracked_frontier_sparse") = SpecQuorumInitial("tracked_frontier_sparse")
  /\ ActualQuorumInitial("frontier_sparse_with_commit_votes") = SpecQuorumInitial("frontier_sparse_with_commit_votes")
  /\ ActualQuorumInitial("sparse_no_signers") = SpecQuorumInitial("sparse_no_signers")
  /\ ActualQuorumInitial("requested_nonfrontier_no_quorum") = SpecQuorumInitial("requested_nonfrontier_no_quorum")
  /\ ActualQuorumInitial("unrequested_sparse_no_quorum") = SpecQuorumInitial("unrequested_sparse_no_quorum")
  /\ ActualMaybeRequestCalled("unrequested_sparse_no_quorum") = SpecMaybeRequestCalled("unrequested_sparse_no_quorum")
  /\ ActualMaybeRequestCalled("tracked_frontier_sparse") = SpecMaybeRequestCalled("tracked_frontier_sparse")

MissingQcRepair ==
  /\ ActualNposVoteOnlyDeferred("missing_qc_request_npos_vote_only")
       = SpecNposVoteOnlyDeferred("missing_qc_request_npos_vote_only")
  /\ ActualRequestedAfterMaybe("missing_qc_request_classic_continue")
       = SpecRequestedAfterMaybe("missing_qc_request_classic_continue")
  /\ ActualQuorumAfterMaybe("missing_qc_request_classic_continue")
       = SpecQuorumAfterMaybe("missing_qc_request_classic_continue")
  /\ ActualReturnsOk("missing_qc_request_classic_continue")
       = SpecReturnsOk("missing_qc_request_classic_continue")
  /\ ActualContinuesToApply("missing_qc_request_classic_continue")
       = SpecContinuesToApply("missing_qc_request_classic_continue")

RepairAndDrops ==
  /\ ActualRepairCalled("repair_deferred") = SpecRepairCalled("repair_deferred")
  /\ ActualRecordDeferredQuorum("repair_deferred") = SpecRecordDeferredQuorum("repair_deferred")
  /\ ActualRecordDroppedInvalid("repair_deferred") = SpecRecordDroppedInvalid("repair_deferred")
  /\ ActualRecordDroppedQuorum("quorum_drop") = SpecRecordDroppedQuorum("quorum_drop")
  /\ ActualRecordReasonInvalidPayload("quorum_drop") = SpecRecordDroppedInvalid("quorum_drop")
  /\ ActualRecordReasonQuorumMissing("quorum_drop")
       = (SpecRecordDeferredQuorum("quorum_drop") \/ SpecRecordDroppedQuorum("quorum_drop"))
  /\ ActualDropInvalidSignatureMetric("quorum_drop") = SpecDropInvalidSignatureMetric("quorum_drop")
  /\ ActualReturnsOk("quorum_drop") = SpecReturnsOk("quorum_drop")
  /\ ActualContinuesToApply("quorum_drop") = SpecContinuesToApply("quorum_drop")
  /\ ActualInvalidQcDrop("invalid_qc_drop") = SpecInvalidQcDrop("invalid_qc_drop")
  /\ ActualInvalidQcDrop("invalid_qc_block_quorum") = SpecInvalidQcDrop("invalid_qc_block_quorum")
  /\ ActualInvalidQcDrop("invalid_qc_checkpoint") = SpecInvalidQcDrop("invalid_qc_checkpoint")
  /\ ActualReturnsOk("invalid_qc_drop") = SpecReturnsOk("invalid_qc_drop")
  /\ ActualClearsMissing("invalid_qc_drop") = SpecClearsMissing("invalid_qc_drop")

=============================================================================
====
