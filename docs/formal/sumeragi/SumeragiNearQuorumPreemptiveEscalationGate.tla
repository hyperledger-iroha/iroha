---- MODULE SumeragiNearQuorumPreemptiveEscalationGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the pre-timeout near-quorum missing-payload
escalation coordinator in `reschedule_stale_pending_blocks(...)`.

The hard-cap and range-pull escalation internals are covered by
`SumeragiMissingBlockHardCapGate.tla`. This slice pins the coordinator gate
around that delegate:
- each tick handles at most one near-quorum recovery candidate,
- an exhausted tick budget marks `budget_exhausted` and fails closed,
- candidates without a pending block cannot escalate,
- matching fresh missing-block requests suppress duplicate fetch escalation,
- matching fresh in-flight recovery range pulls suppress duplicate escalation,
- stale, mismatched, or non-actionable duplicate records do not suppress,
- the delegate return value is the only source of escalation counts/progress,
  and
- a second candidate in the same tick is ignored by the per-tick cap.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoCandidates == "NoCandidates"
BudgetExhausted == "BudgetExhausted"
MissingPending == "MissingPending"
FreshRequestSuppresses == "FreshRequestSuppresses"
RequestHeightMismatch == "RequestHeightMismatch"
RequestViewMismatch == "RequestViewMismatch"
RequestNotActionable == "RequestNotActionable"
RequestBoundaryStale == "RequestBoundaryStale"
RequestStale == "RequestStale"
RequestWindowZeroFresh == "RequestWindowZeroFresh"
RequestCapBoundaryStale == "RequestCapBoundaryStale"
InflightSuppresses == "InflightSuppresses"
InflightHashMismatch == "InflightHashMismatch"
InflightViewMismatch == "InflightViewMismatch"
InflightNotInflight == "InflightNotInflight"
InflightBoundaryStale == "InflightBoundaryStale"
InflightStale == "InflightStale"
InflightTtlZeroFresh == "InflightTtlZeroFresh"
DelegateFalse == "DelegateFalse"
DelegateTrue == "DelegateTrue"
SecondCandidateIgnored == "SecondCandidateIgnored"

Cases == {
  NoCandidates,
  BudgetExhausted,
  MissingPending,
  FreshRequestSuppresses,
  RequestHeightMismatch,
  RequestViewMismatch,
  RequestNotActionable,
  RequestBoundaryStale,
  RequestStale,
  RequestWindowZeroFresh,
  RequestCapBoundaryStale,
  InflightSuppresses,
  InflightHashMismatch,
  InflightViewMismatch,
  InflightNotInflight,
  InflightBoundaryStale,
  InflightStale,
  InflightTtlZeroFresh,
  DelegateFalse,
  DelegateTrue,
  SecondCandidateIgnored
}

BudgetGateCases == {
  NoCandidates,
  BudgetExhausted,
  MissingPending
}

FreshRequestSuppressionCases == {
  FreshRequestSuppresses,
  RequestWindowZeroFresh
}

FreshRequestNoSuppressionCases == {
  RequestHeightMismatch,
  RequestViewMismatch,
  RequestNotActionable,
  RequestBoundaryStale,
  RequestStale,
  RequestCapBoundaryStale
}

InflightSuppressionCases == {
  InflightSuppresses,
  InflightTtlZeroFresh
}

InflightNoSuppressionCases == {
  InflightHashMismatch,
  InflightViewMismatch,
  InflightNotInflight,
  InflightBoundaryStale,
  InflightStale
}

DelegateCases == {
  DelegateFalse,
  DelegateTrue
}

Bugs == {
  "none",
  "budget_exhausted_escalates",
  "budget_exhausted_does_not_mark",
  "missing_pending_escalates",
  "fresh_request_suppression_dropped",
  "fresh_request_height_ignored",
  "fresh_request_view_ignored",
  "fresh_request_actionable_ignored",
  "fresh_request_age_boundary_blocked",
  "fresh_request_age_stale_blocks",
  "fresh_request_window_not_floored",
  "fresh_request_fetch_cap_ignored",
  "inflight_suppression_dropped",
  "inflight_hash_ignored",
  "inflight_view_ignored",
  "inflight_flag_ignored",
  "inflight_ttl_boundary_blocked",
  "inflight_stale_blocks",
  "inflight_ttl_not_floored",
  "delegate_false_counted",
  "delegate_true_not_counted",
  "second_candidate_processed",
  "progress_set_without_escalation"
}

Max(a, b) == IF a >= b THEN a ELSE b
Min(a, b) == IF a <= b THEN a ELSE b

HasCandidate(c) == c # NoCandidates
PendingBlockExists(c) == c # MissingPending
TickBudgetExhausted(c) == c = BudgetExhausted

RequestCases == {
  FreshRequestSuppresses,
  RequestHeightMismatch,
  RequestViewMismatch,
  RequestNotActionable,
  RequestBoundaryStale,
  RequestStale,
  RequestWindowZeroFresh,
  RequestCapBoundaryStale
}

HasRequest(c) == c \in RequestCases
RequestHeightMatches(c) == c # RequestHeightMismatch
RequestViewMatches(c) == c # RequestViewMismatch
RequestActionable(c) == c # RequestNotActionable

RebroadcastCooldown(c) ==
  CASE c = RequestCapBoundaryStale -> 2
    [] OTHER -> 3

FetchFreshnessCap(c) == Max(2 * RebroadcastCooldown(c), 1)

RequestRetryWindow(c) ==
  CASE c = RequestWindowZeroFresh -> 0
    [] c = RequestCapBoundaryStale -> 10
    [] OTHER -> 5

FreshRequestBound(c) ==
  Min(Max(RequestRetryWindow(c), 1), FetchFreshnessCap(c))

RequestAge(c) ==
  CASE c = RequestBoundaryStale -> FreshRequestBound(c)
    [] c = RequestStale -> FreshRequestBound(c) + 2
    [] c = RequestWindowZeroFresh -> 0
    [] c = RequestCapBoundaryStale -> FetchFreshnessCap(c)
    [] OTHER -> 2

SpecFreshRequestSuppresses(c) ==
  /\ HasRequest(c)
  /\ RequestHeightMatches(c)
  /\ RequestViewMatches(c)
  /\ RequestActionable(c)
  /\ RequestAge(c) < FreshRequestBound(c)

InflightCases == {
  InflightSuppresses,
  InflightHashMismatch,
  InflightViewMismatch,
  InflightNotInflight,
  InflightBoundaryStale,
  InflightStale,
  InflightTtlZeroFresh
}

HasRecoveryBudget(c) == c \in InflightCases
RecoveryHashMatches(c) == c # InflightHashMismatch
RecoveryViewMatches(c) == c # InflightViewMismatch
RangePullInflight(c) == c # InflightNotInflight

RecoveryTtl(c) ==
  CASE c = InflightTtlZeroFresh -> 0
    [] OTHER -> 5

FreshInflightBound(c) == Max(RecoveryTtl(c), 1)

InflightAge(c) ==
  CASE c = InflightBoundaryStale -> FreshInflightBound(c)
    [] c = InflightStale -> FreshInflightBound(c) + 2
    [] c = InflightTtlZeroFresh -> 0
    [] OTHER -> 2

SpecInflightSuppresses(c) ==
  /\ HasRecoveryBudget(c)
  /\ RecoveryHashMatches(c)
  /\ RecoveryViewMatches(c)
  /\ RangePullInflight(c)
  /\ InflightAge(c) < FreshInflightBound(c)

DelegateReturns(c) ==
  c \notin {DelegateFalse, SecondCandidateIgnored}

SpecBudgetExhausted(c) ==
  HasCandidate(c) /\ TickBudgetExhausted(c)

SpecProcessed(c) ==
  /\ HasCandidate(c)
  /\ ~SpecBudgetExhausted(c)
  /\ PendingBlockExists(c)

SpecEscalated(c) ==
  /\ SpecProcessed(c)
  /\ ~SpecFreshRequestSuppresses(c)
  /\ ~SpecInflightSuppresses(c)
  /\ DelegateReturns(c)

SpecCounter(c) == IF SpecEscalated(c) THEN 1 ELSE 0
SpecProgress(c) == SpecEscalated(c)
SpecSecondCandidateProcessed(c) == FALSE

ActualBudgetExhausted(c) ==
  IF Bug = "budget_exhausted_does_not_mark" /\ c = BudgetExhausted THEN
    FALSE
  ELSE
    SpecBudgetExhausted(c)

ActualProcessed(c) ==
  IF Bug = "budget_exhausted_escalates" /\ c = BudgetExhausted THEN
    TRUE
  ELSE IF Bug = "missing_pending_escalates" /\ c = MissingPending THEN
    TRUE
  ELSE
    SpecProcessed(c)

ActualFreshRequestSuppresses(c) ==
  IF Bug = "fresh_request_suppression_dropped" /\ c = FreshRequestSuppresses THEN
    FALSE
  ELSE IF Bug = "fresh_request_height_ignored" /\ c = RequestHeightMismatch THEN
    /\ HasRequest(c)
    /\ RequestViewMatches(c)
    /\ RequestActionable(c)
    /\ RequestAge(c) < FreshRequestBound(c)
  ELSE IF Bug = "fresh_request_view_ignored" /\ c = RequestViewMismatch THEN
    /\ HasRequest(c)
    /\ RequestHeightMatches(c)
    /\ RequestActionable(c)
    /\ RequestAge(c) < FreshRequestBound(c)
  ELSE IF Bug = "fresh_request_actionable_ignored" /\ c = RequestNotActionable THEN
    /\ HasRequest(c)
    /\ RequestHeightMatches(c)
    /\ RequestViewMatches(c)
    /\ RequestAge(c) < FreshRequestBound(c)
  ELSE IF Bug = "fresh_request_age_boundary_blocked" /\ c = RequestBoundaryStale THEN
    /\ HasRequest(c)
    /\ RequestHeightMatches(c)
    /\ RequestViewMatches(c)
    /\ RequestActionable(c)
    /\ RequestAge(c) <= FreshRequestBound(c)
  ELSE IF Bug = "fresh_request_age_stale_blocks" /\ c = RequestStale THEN
    TRUE
  ELSE IF Bug = "fresh_request_window_not_floored" /\ c = RequestWindowZeroFresh THEN
    /\ HasRequest(c)
    /\ RequestHeightMatches(c)
    /\ RequestViewMatches(c)
    /\ RequestActionable(c)
    /\ RequestAge(c) < Min(RequestRetryWindow(c), FetchFreshnessCap(c))
  ELSE IF Bug = "fresh_request_fetch_cap_ignored" /\ c = RequestCapBoundaryStale THEN
    /\ HasRequest(c)
    /\ RequestHeightMatches(c)
    /\ RequestViewMatches(c)
    /\ RequestActionable(c)
    /\ RequestAge(c) < Max(RequestRetryWindow(c), 1)
  ELSE
    SpecFreshRequestSuppresses(c)

ActualInflightSuppresses(c) ==
  IF Bug = "inflight_suppression_dropped" /\ c = InflightSuppresses THEN
    FALSE
  ELSE IF Bug = "inflight_hash_ignored" /\ c = InflightHashMismatch THEN
    /\ HasRecoveryBudget(c)
    /\ RecoveryViewMatches(c)
    /\ RangePullInflight(c)
    /\ InflightAge(c) < FreshInflightBound(c)
  ELSE IF Bug = "inflight_view_ignored" /\ c = InflightViewMismatch THEN
    /\ HasRecoveryBudget(c)
    /\ RecoveryHashMatches(c)
    /\ RangePullInflight(c)
    /\ InflightAge(c) < FreshInflightBound(c)
  ELSE IF Bug = "inflight_flag_ignored" /\ c = InflightNotInflight THEN
    /\ HasRecoveryBudget(c)
    /\ RecoveryHashMatches(c)
    /\ RecoveryViewMatches(c)
    /\ InflightAge(c) < FreshInflightBound(c)
  ELSE IF Bug = "inflight_ttl_boundary_blocked" /\ c = InflightBoundaryStale THEN
    /\ HasRecoveryBudget(c)
    /\ RecoveryHashMatches(c)
    /\ RecoveryViewMatches(c)
    /\ RangePullInflight(c)
    /\ InflightAge(c) <= FreshInflightBound(c)
  ELSE IF Bug = "inflight_stale_blocks" /\ c = InflightStale THEN
    TRUE
  ELSE IF Bug = "inflight_ttl_not_floored" /\ c = InflightTtlZeroFresh THEN
    /\ HasRecoveryBudget(c)
    /\ RecoveryHashMatches(c)
    /\ RecoveryViewMatches(c)
    /\ RangePullInflight(c)
    /\ InflightAge(c) < RecoveryTtl(c)
  ELSE
    SpecInflightSuppresses(c)

ActualEscalated(c) ==
  IF Bug = "delegate_false_counted" /\ c = DelegateFalse THEN
    TRUE
  ELSE IF Bug = "delegate_true_not_counted" /\ c = DelegateTrue THEN
    FALSE
  ELSE IF Bug = "second_candidate_processed" /\ c = SecondCandidateIgnored THEN
    TRUE
  ELSE
    /\ ActualProcessed(c)
    /\ ~ActualFreshRequestSuppresses(c)
    /\ ~ActualInflightSuppresses(c)
    /\ DelegateReturns(c)

ActualCounter(c) == IF ActualEscalated(c) THEN 1 ELSE 0

ActualProgress(c) ==
  IF Bug = "progress_set_without_escalation" /\ c = FreshRequestSuppresses THEN
    TRUE
  ELSE
    ActualEscalated(c)

ActualSecondCandidateProcessed(c) ==
  Bug = "second_candidate_processed" /\ c = SecondCandidateIgnored

SpecOutput(c) ==
  <<
    SpecProcessed(c),
    SpecFreshRequestSuppresses(c),
    SpecInflightSuppresses(c),
    SpecEscalated(c),
    SpecCounter(c),
    SpecProgress(c),
    SpecBudgetExhausted(c),
    SpecSecondCandidateProcessed(c)
  >>

ActualOutput(c) ==
  <<
    ActualProcessed(c),
    ActualFreshRequestSuppresses(c),
    ActualInflightSuppresses(c),
    ActualEscalated(c),
    ActualCounter(c),
    ActualProgress(c),
    ActualBudgetExhausted(c),
    ActualSecondCandidateProcessed(c)
  >>

Init == checked = 0

Next == UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in Bugs

NearQuorumPreemptiveEscalationCoreSafety ==
  \A c \in Cases: ActualOutput(c) = SpecOutput(c)

SafetyFast == NearQuorumPreemptiveEscalationCoreSafety

NearQuorumBudgetGateExact ==
  /\ \A c \in BudgetGateCases:
       /\ ActualProcessed(c) = FALSE
       /\ ActualEscalated(c) = FALSE
       /\ ActualCounter(c) = 0
       /\ ActualProgress(c) = FALSE
       /\ ActualOutput(c) = SpecOutput(c)
  /\ ActualBudgetExhausted(BudgetExhausted) = TRUE
  /\ ActualBudgetExhausted(NoCandidates) = FALSE
  /\ ActualBudgetExhausted(MissingPending) = FALSE

NearQuorumFreshRequestSuppressionExact ==
  /\ \A c \in FreshRequestSuppressionCases:
       /\ ActualProcessed(c) = TRUE
       /\ ActualFreshRequestSuppresses(c) = TRUE
       /\ ActualEscalated(c) = FALSE
       /\ ActualCounter(c) = 0
       /\ ActualProgress(c) = FALSE
       /\ ActualOutput(c) = SpecOutput(c)
  /\ \A c \in FreshRequestNoSuppressionCases:
       /\ ActualProcessed(c) = TRUE
       /\ ActualFreshRequestSuppresses(c) = FALSE
       /\ ActualEscalated(c) = TRUE
       /\ ActualCounter(c) = 1
       /\ ActualProgress(c) = TRUE
       /\ ActualOutput(c) = SpecOutput(c)

NearQuorumInflightSuppressionExact ==
  /\ \A c \in InflightSuppressionCases:
       /\ ActualProcessed(c) = TRUE
       /\ ActualInflightSuppresses(c) = TRUE
       /\ ActualEscalated(c) = FALSE
       /\ ActualCounter(c) = 0
       /\ ActualProgress(c) = FALSE
       /\ ActualOutput(c) = SpecOutput(c)
  /\ \A c \in InflightNoSuppressionCases:
       /\ ActualProcessed(c) = TRUE
       /\ ActualInflightSuppresses(c) = FALSE
       /\ ActualEscalated(c) = TRUE
       /\ ActualCounter(c) = 1
       /\ ActualProgress(c) = TRUE
       /\ ActualOutput(c) = SpecOutput(c)

NearQuorumDelegateResultExact ==
  /\ ActualProcessed(DelegateFalse) = TRUE
  /\ ActualEscalated(DelegateFalse) = FALSE
  /\ ActualCounter(DelegateFalse) = 0
  /\ ActualProgress(DelegateFalse) = FALSE
  /\ ActualOutput(DelegateFalse) = SpecOutput(DelegateFalse)
  /\ ActualProcessed(DelegateTrue) = TRUE
  /\ ActualEscalated(DelegateTrue) = TRUE
  /\ ActualCounter(DelegateTrue) = 1
  /\ ActualProgress(DelegateTrue) = TRUE
  /\ ActualOutput(DelegateTrue) = SpecOutput(DelegateTrue)

NearQuorumPerTickCapExact ==
  /\ ActualProcessed(SecondCandidateIgnored) = TRUE
  /\ ActualEscalated(SecondCandidateIgnored) = FALSE
  /\ ActualCounter(SecondCandidateIgnored) = 0
  /\ ActualProgress(SecondCandidateIgnored) = FALSE
  /\ ActualSecondCandidateProcessed(SecondCandidateIgnored) = FALSE
  /\ ActualOutput(SecondCandidateIgnored) = SpecOutput(SecondCandidateIgnored)

NearQuorumProgressCounterExact ==
  \A c \in Cases:
    /\ ActualCounter(c) = IF ActualEscalated(c) THEN 1 ELSE 0
    /\ ActualProgress(c) = ActualEscalated(c)
    /\ ActualCounter(c) = SpecCounter(c)
    /\ ActualProgress(c) = SpecProgress(c)
    /\ ActualOutput(c) = SpecOutput(c)

NearQuorumPreemptiveEscalationExactness ==
  /\ NearQuorumPreemptiveEscalationCoreSafety
  /\ NearQuorumBudgetGateExact
  /\ NearQuorumFreshRequestSuppressionExact
  /\ NearQuorumInflightSuppressionExact
  /\ NearQuorumDelegateResultExact
  /\ NearQuorumPerTickCapExact
  /\ NearQuorumProgressCounterExact

NearQuorumPreemptiveEscalationCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ NearQuorumPreemptiveEscalationExactness

====
