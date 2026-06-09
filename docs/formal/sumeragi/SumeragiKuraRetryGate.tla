---- MODULE SumeragiKuraRetryGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for PendingBlock Kura persistence retry state.

This slice captures `PendingBlock::note_kura_failure(...)`,
`kura_retry_due(...)`, `reset_kura_retry(...)`, and
`mark_kura_persisted(...)`.  The model abstracts `Instant` and `Duration` into
finite boundary cases while preserving the observable contract: missing retry
deadlines are due, deadlines are inclusive, aborted retries are never due,
reset clears attempts/deadline/abort state, mark-persisted also records
durability, retry failures increment attempts before scheduling exponential
backoff, max-attempt exhaustion aborts, zero configured attempts aborts
without incrementing, checked-add overflow aborts and clears the retry
deadline, and oversized `next_in_ms` values clamp to the public u32 surface.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

DueNoDeadline == "due_no_deadline"
DueBeforeDeadline == "due_before_deadline"
DueAtDeadline == "due_at_deadline"
DueAfterDeadline == "due_after_deadline"
DueAbortedNoDeadline == "due_aborted_no_deadline"
DueAbortedWithDeadline == "due_aborted_with_deadline"
ResetRetry == "reset_retry"
MarkPersistedRetry == "mark_persisted_retry"
FailureMaxZeroFresh == "failure_max_zero_fresh"
FailureMaxZeroWithDeadline == "failure_max_zero_with_deadline"
FailureFirstRetry == "failure_first_retry"
FailureSecondRetry == "failure_second_retry"
FailureAtMaxAbort == "failure_at_max_abort"
FailureOverflowAbort == "failure_overflow_abort"
FailureNextInClamp == "failure_next_in_clamp"

Cases == {
  DueNoDeadline,
  DueBeforeDeadline,
  DueAtDeadline,
  DueAfterDeadline,
  DueAbortedNoDeadline,
  DueAbortedWithDeadline,
  ResetRetry,
  MarkPersistedRetry,
  FailureMaxZeroFresh,
  FailureMaxZeroWithDeadline,
  FailureFirstRetry,
  FailureSecondRetry,
  FailureAtMaxAbort,
  FailureOverflowAbort,
  FailureNextInClamp
}

DueCases == {
  DueNoDeadline,
  DueBeforeDeadline,
  DueAtDeadline,
  DueAfterDeadline,
  DueAbortedNoDeadline,
  DueAbortedWithDeadline
}

ResetCases == {ResetRetry, MarkPersistedRetry}

MaxAttemptCases == {FailureMaxZeroFresh, FailureMaxZeroWithDeadline}
RetryBackoffCases == {FailureFirstRetry, FailureSecondRetry}
AbortBoundaryCases == {FailureAtMaxAbort, FailureOverflowAbort}
DelayReportingCases == {FailureNextInClamp}

Attempts0 == 1
Attempts1 == 2
Attempts2 == 3
NextNone == 4
NextFuture5 == 5
NextFuture10 == 6
NextFutureHuge == 7
AbortedFalse == 8
AbortedTrue == 9
PersistedFalse == 10
PersistedTrue == 11
ResultNone == 12
ResultRetry1 == 13
ResultRetry2 == 14
ResultAbort0 == 15
ResultAbort1 == 16
ResultAbort2 == 17
NextIn0 == 18
NextIn5 == 19
NextIn10 == 20
NextInMax == 21
DueFalse == 22
DueTrue == 23

Actions == 1..23

DueOnly(due) ==
  IF due THEN {DueTrue} ELSE {DueFalse}

ResetActions ==
  {Attempts0, NextNone, AbortedFalse, PersistedFalse, ResultNone, DueTrue}

PersistedActions ==
  {Attempts0, NextNone, AbortedFalse, PersistedTrue, ResultNone, DueTrue}

MaxZeroFreshActions ==
  {Attempts0, NextNone, AbortedTrue, PersistedFalse, ResultAbort0, NextIn0,
   DueFalse}

MaxZeroWithDeadlineActions ==
  {Attempts2, NextFuture5, AbortedTrue, PersistedFalse, ResultAbort2, NextIn0,
   DueFalse}

FirstRetryActions ==
  {Attempts1, NextFuture5, AbortedFalse, PersistedFalse, ResultRetry1,
   NextIn5, DueFalse}

SecondRetryActions ==
  {Attempts2, NextFuture10, AbortedFalse, PersistedFalse, ResultRetry2,
   NextIn10, DueFalse}

AtMaxAbortActions ==
  {Attempts2, NextFuture10, AbortedTrue, PersistedFalse, ResultAbort2,
   NextIn0, DueFalse}

OverflowAbortActions ==
  {Attempts1, NextNone, AbortedTrue, PersistedFalse, ResultAbort1, NextIn0,
   DueFalse}

NextInClampActions ==
  {Attempts1, NextFutureHuge, AbortedFalse, PersistedFalse, ResultRetry1,
   NextInMax, DueFalse}

SpecActions(c) ==
  CASE c = DueNoDeadline -> DueOnly(TRUE)
    [] c = DueBeforeDeadline -> DueOnly(FALSE)
    [] c = DueAtDeadline -> DueOnly(TRUE)
    [] c = DueAfterDeadline -> DueOnly(TRUE)
    [] c = DueAbortedNoDeadline -> DueOnly(FALSE)
    [] c = DueAbortedWithDeadline -> DueOnly(FALSE)
    [] c = ResetRetry -> ResetActions
    [] c = MarkPersistedRetry -> PersistedActions
    [] c = FailureMaxZeroFresh -> MaxZeroFreshActions
    [] c = FailureMaxZeroWithDeadline -> MaxZeroWithDeadlineActions
    [] c = FailureFirstRetry -> FirstRetryActions
    [] c = FailureSecondRetry -> SecondRetryActions
    [] c = FailureAtMaxAbort -> AtMaxAbortActions
    [] c = FailureOverflowAbort -> OverflowAbortActions
    [] c = FailureNextInClamp -> NextInClampActions
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "due_no_deadline_waits"
       /\ c = DueNoDeadline -> DueOnly(FALSE)
    [] Bug = "due_before_deadline_ready"
       /\ c = DueBeforeDeadline -> DueOnly(TRUE)
    [] Bug = "due_at_deadline_waits"
       /\ c = DueAtDeadline -> DueOnly(FALSE)
    [] Bug = "due_aborted_ready"
       /\ c = DueAbortedWithDeadline -> DueOnly(TRUE)
    [] Bug = "reset_keeps_attempts"
       /\ c = ResetRetry -> (spec \ {Attempts0}) \cup {Attempts2}
    [] Bug = "reset_keeps_deadline"
       /\ c = ResetRetry -> (spec \ {NextNone}) \cup {NextFuture5}
    [] Bug = "reset_keeps_aborted"
       /\ c = ResetRetry -> (spec \ {AbortedFalse, DueTrue}) \cup
         {AbortedTrue, DueFalse}
    [] Bug = "persisted_skips_reset"
       /\ c = MarkPersistedRetry ->
         (spec \ {Attempts0, NextNone}) \cup {Attempts2, NextFuture5}
    [] Bug = "max_zero_increments"
       /\ c = FailureMaxZeroFresh ->
         (spec \ {Attempts0, ResultAbort0}) \cup {Attempts1, ResultAbort1}
    [] Bug = "max_zero_keeps_live"
       /\ c = FailureMaxZeroFresh ->
         (spec \ {AbortedTrue, ResultAbort0, DueFalse}) \cup
         {AbortedFalse, ResultRetry1, DueTrue}
    [] Bug = "max_zero_clears_existing_deadline"
       /\ c = FailureMaxZeroWithDeadline ->
         (spec \ {NextFuture5}) \cup {NextNone}
    [] Bug = "retry_no_increment"
       /\ c = FailureFirstRetry ->
         (spec \ {Attempts1, ResultRetry1}) \cup {Attempts0, ResultNone}
    [] Bug = "retry_no_deadline"
       /\ c = FailureFirstRetry ->
         (spec \ {NextFuture5, DueFalse}) \cup {NextNone, DueTrue}
    [] Bug = "retry_aborts_before_budget"
       /\ c = FailureFirstRetry ->
         (spec \ {AbortedFalse, ResultRetry1}) \cup {AbortedTrue,
           ResultAbort1}
    [] Bug = "retry_wrong_backoff"
       /\ c = FailureFirstRetry ->
         (spec \ {NextFuture5, NextIn5}) \cup {NextFuture10, NextIn10}
    [] Bug = "second_retry_not_exponential"
       /\ c = FailureSecondRetry ->
         (spec \ {NextFuture10, NextIn10}) \cup {NextFuture5, NextIn5}
    [] Bug = "exhausted_retries"
       /\ c = FailureAtMaxAbort ->
         (spec \ {AbortedTrue, ResultAbort2}) \cup {AbortedFalse,
           ResultRetry2}
    [] Bug = "exhausted_clears_deadline"
       /\ c = FailureAtMaxAbort ->
         (spec \ {NextFuture10}) \cup {NextNone}
    [] Bug = "overflow_retries"
       /\ c = FailureOverflowAbort ->
         (spec \ {AbortedTrue, ResultAbort1, NextIn0}) \cup {AbortedFalse,
           ResultRetry1, NextInMax}
    [] Bug = "overflow_keeps_deadline"
       /\ c = FailureOverflowAbort ->
         (spec \ {NextNone}) \cup {NextFutureHuge}
    [] Bug = "next_in_not_clamped"
       /\ c = FailureNextInClamp ->
         (spec \ {NextInMax}) \cup {NextIn10}
    [] OTHER -> spec

Bugs == {
  "none",
  "due_no_deadline_waits",
  "due_before_deadline_ready",
  "due_at_deadline_waits",
  "due_aborted_ready",
  "reset_keeps_attempts",
  "reset_keeps_deadline",
  "reset_keeps_aborted",
  "persisted_skips_reset",
  "max_zero_increments",
  "max_zero_keeps_live",
  "max_zero_clears_existing_deadline",
  "retry_no_increment",
  "retry_no_deadline",
  "retry_aborts_before_budget",
  "retry_wrong_backoff",
  "second_retry_not_exponential",
  "exhausted_retries",
  "exhausted_clears_deadline",
  "overflow_retries",
  "overflow_keeps_deadline",
  "next_in_not_clamped"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked = 0
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

KuraRetryCoreSafety ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

NoBugInvariant == KuraRetryCoreSafety

DueSemantics ==
  /\ ImplementationActions(DueNoDeadline) = DueOnly(TRUE)
  /\ ImplementationActions(DueBeforeDeadline) = DueOnly(FALSE)
  /\ ImplementationActions(DueAtDeadline) = DueOnly(TRUE)
  /\ ImplementationActions(DueAfterDeadline) = DueOnly(TRUE)
  /\ ImplementationActions(DueAbortedNoDeadline) = DueOnly(FALSE)
  /\ ImplementationActions(DueAbortedWithDeadline) = DueOnly(FALSE)

ResetSemantics ==
  /\ ImplementationActions(ResetRetry) = ResetActions
  /\ ImplementationActions(MarkPersistedRetry) = PersistedActions

FailureSemantics ==
  /\ ImplementationActions(FailureMaxZeroFresh) = MaxZeroFreshActions
  /\ ImplementationActions(FailureMaxZeroWithDeadline) =
       MaxZeroWithDeadlineActions
  /\ ImplementationActions(FailureFirstRetry) = FirstRetryActions
  /\ ImplementationActions(FailureSecondRetry) = SecondRetryActions
  /\ ImplementationActions(FailureAtMaxAbort) = AtMaxAbortActions
  /\ ImplementationActions(FailureOverflowAbort) = OverflowAbortActions
  /\ ImplementationActions(FailureNextInClamp) = NextInClampActions

KuraRetryDueExact ==
  \A c \in DueCases:
    ImplementationActions(c) = SpecActions(c)

KuraRetryResetExact ==
  \A c \in ResetCases:
    ImplementationActions(c) = SpecActions(c)

KuraRetryMaxAttemptExact ==
  \A c \in MaxAttemptCases:
    ImplementationActions(c) = SpecActions(c)

KuraRetryBackoffExact ==
  \A c \in RetryBackoffCases:
    ImplementationActions(c) = SpecActions(c)

KuraRetryAbortBoundaryExact ==
  \A c \in AbortBoundaryCases:
    ImplementationActions(c) = SpecActions(c)

KuraRetryDelayReportingExact ==
  \A c \in DelayReportingCases:
    ImplementationActions(c) = SpecActions(c)

KuraRetryStateExactness ==
  /\ KuraRetryDueExact
  /\ KuraRetryResetExact
  /\ KuraRetryMaxAttemptExact
  /\ KuraRetryBackoffExact
  /\ KuraRetryAbortBoundaryExact
  /\ KuraRetryDelayReportingExact

SafetyFast ==
  /\ NoBugInvariant
  /\ DueSemantics
  /\ ResetSemantics
  /\ FailureSemantics
  /\ KuraRetryStateExactness

====
