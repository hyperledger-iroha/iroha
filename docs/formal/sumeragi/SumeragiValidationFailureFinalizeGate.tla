---- MODULE SumeragiValidationFailureFinalizeGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `finalize_validation_failure(...)` and its
`validation_reject_reason_label(...)` classification.

The implementation has one deferral escape hatch: a previous-block-height
mismatch whose actual parent height is ahead of the expected local height keeps
the pending block pending. Every other validation error fails closed by
aborting the pending block, attempting transaction requeue, clearing proposal
and RBC/QC state for the rejected block, preserving the correct reason label,
attaching invalid-proposal evidence only when a matching QC is available, and
triggering the previous-roster payload-recovery bundle only for
`PreviousRosterEvidenceInvalid`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "future_prev_height",
  "future_prev_height_with_qc",
  "equal_prev_height",
  "past_prev_height",
  "prev_hash",
  "topology",
  "signature",
  "transaction_accept_with_qc",
  "duplicate_transactions",
  "stateless_genesis",
  "previous_roster_evidence",
  "previous_roster_with_qc"
}

PrevHeightMismatch(c) ==
  c \in {"future_prev_height", "future_prev_height_with_qc",
         "equal_prev_height", "past_prev_height"}

ParentAheadOfLocal(c) ==
  c \in {"future_prev_height", "future_prev_height_with_qc"}

PrevHashMismatch(c) ==
  c = "prev_hash"

TopologyMismatch(c) ==
  c = "topology"

ExecutionFailure(c) ==
  c \in {"signature", "transaction_accept_with_qc", "duplicate_transactions"}

PreviousRosterFailure(c) ==
  c \in {"previous_roster_evidence", "previous_roster_with_qc"}

StatelessFailure(c) ==
  c = "stateless_genesis" \/ PreviousRosterFailure(c)

EvidenceQcAvailable(c) ==
  c \in {
    "future_prev_height_with_qc",
    "transaction_accept_with_qc",
    "previous_roster_with_qc"
  }

ReasonLabel(c) ==
  CASE PrevHashMismatch(c) -> "prev_hash"
    [] PrevHeightMismatch(c) -> "prev_height"
    [] TopologyMismatch(c) -> "topology"
    [] ExecutionFailure(c) -> "execution"
    [] StatelessFailure(c) -> "stateless"
    [] OTHER -> "stateless"

\* @type: Str => <<Str, Bool, Str, Bool, Bool, Bool, Bool, Bool, Str, Bool, Bool>>;
SpecOutput(c) ==
  IF PrevHeightMismatch(c) /\ ParentAheadOfLocal(c) THEN
    <<"deferred", TRUE, "pending", FALSE, FALSE, FALSE, FALSE, FALSE,
      "none", FALSE, FALSE>>
  ELSE
    <<"invalid", FALSE, "invalid", TRUE, TRUE, TRUE, TRUE, TRUE,
      ReasonLabel(c), EvidenceQcAvailable(c), PreviousRosterFailure(c)>>

\* @type: Str => <<Str, Bool, Str, Bool, Bool, Bool, Bool, Bool, Str, Bool, Bool>>;
ActualOutput(c) ==
  CASE Bug = "defer_equal_prev_height"
       /\ c = "equal_prev_height" ->
         <<"deferred", TRUE, "pending", FALSE, FALSE, FALSE, FALSE, FALSE,
           "none", FALSE, FALSE>>
    [] Bug = "reject_future_prev_height"
       /\ c = "future_prev_height" ->
         <<"invalid", FALSE, "invalid", TRUE, TRUE, TRUE, TRUE, TRUE,
           "prev_height", FALSE, FALSE>>
    [] Bug = "store_invalid_pending"
       /\ c = "signature" ->
         <<"invalid", TRUE, "invalid", TRUE, TRUE, TRUE, TRUE, TRUE,
           "execution", FALSE, FALSE>>
    [] Bug = "skip_abort"
       /\ c = "signature" ->
         <<"invalid", FALSE, "invalid", FALSE, TRUE, TRUE, TRUE, TRUE,
           "execution", FALSE, FALSE>>
    [] Bug = "skip_requeue"
       /\ c = "signature" ->
         <<"invalid", FALSE, "invalid", TRUE, FALSE, TRUE, TRUE, TRUE,
           "execution", FALSE, FALSE>>
    [] Bug = "skip_proposal_cleanup"
       /\ c = "signature" ->
         <<"invalid", FALSE, "invalid", TRUE, TRUE, FALSE, TRUE, TRUE,
           "execution", FALSE, FALSE>>
    [] Bug = "skip_rbc_cleanup"
       /\ c = "signature" ->
         <<"invalid", FALSE, "invalid", TRUE, TRUE, TRUE, FALSE, TRUE,
           "execution", FALSE, FALSE>>
    [] Bug = "skip_qc_cache_cleanup"
       /\ c = "signature" ->
         <<"invalid", FALSE, "invalid", TRUE, TRUE, TRUE, TRUE, FALSE,
           "execution", FALSE, FALSE>>
    [] Bug = "wrong_prev_hash_label"
       /\ c = "prev_hash" ->
         <<"invalid", FALSE, "invalid", TRUE, TRUE, TRUE, TRUE, TRUE,
           "stateless", FALSE, FALSE>>
    [] Bug = "wrong_prev_height_label"
       /\ c = "past_prev_height" ->
         <<"invalid", FALSE, "invalid", TRUE, TRUE, TRUE, TRUE, TRUE,
           "stateless", FALSE, FALSE>>
    [] Bug = "wrong_topology_label"
       /\ c = "topology" ->
         <<"invalid", FALSE, "invalid", TRUE, TRUE, TRUE, TRUE, TRUE,
           "execution", FALSE, FALSE>>
    [] Bug = "signature_stateless_label"
       /\ c = "signature" ->
         <<"invalid", FALSE, "invalid", TRUE, TRUE, TRUE, TRUE, TRUE,
           "stateless", FALSE, FALSE>>
    [] Bug = "stateless_execution_label"
       /\ c = "stateless_genesis" ->
         <<"invalid", FALSE, "invalid", TRUE, TRUE, TRUE, TRUE, TRUE,
           "execution", FALSE, FALSE>>
    [] Bug = "attach_evidence_on_deferred"
       /\ c = "future_prev_height_with_qc" ->
         <<"deferred", TRUE, "pending", FALSE, FALSE, FALSE, FALSE, FALSE,
           "none", TRUE, FALSE>>
    [] Bug = "skip_evidence_qc"
       /\ c = "transaction_accept_with_qc" ->
         <<"invalid", FALSE, "invalid", TRUE, TRUE, TRUE, TRUE, TRUE,
           "execution", FALSE, FALSE>>
    [] Bug = "trigger_recovery_for_other_error"
       /\ c = "signature" ->
         <<"invalid", FALSE, "invalid", TRUE, TRUE, TRUE, TRUE, TRUE,
           "execution", FALSE, TRUE>>
    [] Bug = "skip_previous_roster_recovery"
       /\ c = "previous_roster_evidence" ->
         <<"invalid", FALSE, "invalid", TRUE, TRUE, TRUE, TRUE, TRUE,
           "stateless", FALSE, FALSE>>
    [] OTHER -> SpecOutput(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

ValidationFailureFinalizeMatchesSpec ==
  \A c \in Cases: ActualOutput(c) = SpecOutput(c)

SafetyFast ==
  ValidationFailureFinalizeMatchesSpec

BugDeferEqualPrevHeight ==
  ActualOutput("equal_prev_height") = SpecOutput("equal_prev_height")

BugRejectFuturePrevHeight ==
  ActualOutput("future_prev_height") = SpecOutput("future_prev_height")

BugStoreInvalidPending ==
  ActualOutput("signature") = SpecOutput("signature")

BugSkipAbort ==
  ActualOutput("signature") = SpecOutput("signature")

BugSkipRequeue ==
  ActualOutput("signature") = SpecOutput("signature")

BugSkipProposalCleanup ==
  ActualOutput("signature") = SpecOutput("signature")

BugSkipRbcCleanup ==
  ActualOutput("signature") = SpecOutput("signature")

BugSkipQcCacheCleanup ==
  ActualOutput("signature") = SpecOutput("signature")

BugWrongPrevHashLabel ==
  ActualOutput("prev_hash") = SpecOutput("prev_hash")

BugWrongPrevHeightLabel ==
  ActualOutput("past_prev_height") = SpecOutput("past_prev_height")

BugWrongTopologyLabel ==
  ActualOutput("topology") = SpecOutput("topology")

BugSignatureStatelessLabel ==
  ActualOutput("signature") = SpecOutput("signature")

BugStatelessExecutionLabel ==
  ActualOutput("stateless_genesis") = SpecOutput("stateless_genesis")

BugAttachEvidenceOnDeferred ==
  ActualOutput("future_prev_height_with_qc") =
    SpecOutput("future_prev_height_with_qc")

BugSkipEvidenceQc ==
  ActualOutput("transaction_accept_with_qc") =
    SpecOutput("transaction_accept_with_qc")

BugTriggerRecoveryForOtherError ==
  ActualOutput("signature") = SpecOutput("signature")

BugSkipPreviousRosterRecovery ==
  ActualOutput("previous_roster_evidence") =
    SpecOutput("previous_roster_evidence")

ActualKind(c) == ActualOutput(c)[1]

ActualPendingKept(c) == ActualOutput(c)[2]

ActualPendingState(c) == ActualOutput(c)[3]

ActualAbortCalled(c) == ActualOutput(c)[4]

ActualRequeueCalled(c) == ActualOutput(c)[5]

ActualProposalCleared(c) == ActualOutput(c)[6]

ActualRbcCleared(c) == ActualOutput(c)[7]

ActualQcCacheCleared(c) == ActualOutput(c)[8]

ActualReasonLabel(c) == ActualOutput(c)[9]

ActualEvidenceAttached(c) == ActualOutput(c)[10]

ActualPreviousRosterRecovery(c) == ActualOutput(c)[11]

AllFinalizeCasesMatchSpec ==
  ValidationFailureFinalizeMatchesSpec

PrevHeightDeferralBoundaryAnchors ==
  /\ ActualKind("future_prev_height") = "deferred"
  /\ ActualKind("future_prev_height_with_qc") = "deferred"
  /\ ActualKind("equal_prev_height") = "invalid"
  /\ ActualKind("past_prev_height") = "invalid"
  /\ ActualReasonLabel("equal_prev_height") = "prev_height"
  /\ ActualReasonLabel("past_prev_height") = "prev_height"

DeferredCasesSuppressInvalidSideEffects ==
  \A c \in Cases:
    ActualKind(c) = "deferred" =>
      /\ ActualPendingKept(c)
      /\ ActualPendingState(c) = "pending"
      /\ ~ActualAbortCalled(c)
      /\ ~ActualRequeueCalled(c)
      /\ ~ActualProposalCleared(c)
      /\ ~ActualRbcCleared(c)
      /\ ~ActualQcCacheCleared(c)
      /\ ActualReasonLabel(c) = "none"
      /\ ~ActualEvidenceAttached(c)
      /\ ~ActualPreviousRosterRecovery(c)

InvalidCasesRunCleanupAnchors ==
  \A c \in Cases:
    ActualKind(c) = "invalid" =>
      /\ ~ActualPendingKept(c)
      /\ ActualPendingState(c) = "invalid"
      /\ ActualAbortCalled(c)
      /\ ActualRequeueCalled(c)
      /\ ActualProposalCleared(c)
      /\ ActualRbcCleared(c)
      /\ ActualQcCacheCleared(c)

ReasonLabelAnchors ==
  /\ ActualReasonLabel("prev_hash") = "prev_hash"
  /\ ActualReasonLabel("past_prev_height") = "prev_height"
  /\ ActualReasonLabel("topology") = "topology"
  /\ ActualReasonLabel("signature") = "execution"
  /\ ActualReasonLabel("transaction_accept_with_qc") = "execution"
  /\ ActualReasonLabel("duplicate_transactions") = "execution"
  /\ ActualReasonLabel("stateless_genesis") = "stateless"
  /\ ActualReasonLabel("previous_roster_evidence") = "stateless"
  /\ ActualReasonLabel("previous_roster_with_qc") = "stateless"

EvidenceAttachedOnlyForInvalidMatchingQc ==
  \A c \in Cases:
    ActualEvidenceAttached(c) <=>
      /\ ActualKind(c) = "invalid"
      /\ EvidenceQcAvailable(c)

PreviousRosterRecoveryOnlyForPreviousRosterFailures ==
  \A c \in Cases:
    ActualPreviousRosterRecovery(c) <=>
      /\ ActualKind(c) = "invalid"
      /\ PreviousRosterFailure(c)

CleanupAnchorCases ==
  /\ ActualAbortCalled("signature")
  /\ ActualRequeueCalled("signature")
  /\ ActualProposalCleared("signature")
  /\ ActualRbcCleared("signature")
  /\ ActualQcCacheCleared("signature")

SafetyAnchors ==
  /\ AllFinalizeCasesMatchSpec
  /\ PrevHeightDeferralBoundaryAnchors
  /\ DeferredCasesSuppressInvalidSideEffects
  /\ InvalidCasesRunCleanupAnchors
  /\ ReasonLabelAnchors
  /\ EvidenceAttachedOnlyForInvalidMatchingQc
  /\ PreviousRosterRecoveryOnlyForPreviousRosterFailures
  /\ CleanupAnchorCases

ValidationFailureFinalizeCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ SafetyFast
  /\ SafetyAnchors

====
