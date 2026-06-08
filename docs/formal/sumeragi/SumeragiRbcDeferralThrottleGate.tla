---- MODULE SumeragiRbcDeferralThrottleGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for RBC READY/DELIVER deferral throttles.

This slice captures:
- `should_emit_rbc_deliver_deferral(...)`; and
- `should_emit_rbc_ready_deferral(...)`.

The helpers decide whether repeated RBC deferral diagnostics should be emitted
and update their per-session throttle state. The model pins first-observation
logging, progress-triggered logging, exact cooldown-boundary logging,
zero-cooldown logging, saturating elapsed-time behavior for backwards clocks,
READY/received-count progress as strictly increasing, total/required/reason
changes as progress, regression-only observations as non-progress, and state
replacement only when an emission is admitted.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ReasonA == "commit_roster_missing"
ReasonB == "missing_payload"

Reasons == {ReasonA, ReasonB}

DeliverVacant == "deliver_vacant"
DeliverNoProgressBefore == "deliver_no_progress_before"
DeliverReadyProgress == "deliver_ready_progress"
DeliverReadyRegression == "deliver_ready_regression"
DeliverReceivedProgress == "deliver_received_progress"
DeliverReceivedRegression == "deliver_received_regression"
DeliverTotalChanged == "deliver_total_changed"
DeliverBoundary == "deliver_boundary"
DeliverZeroCooldown == "deliver_zero_cooldown"
DeliverFutureClock == "deliver_future_clock"

DeliverCases == {
  DeliverVacant,
  DeliverNoProgressBefore,
  DeliverReadyProgress,
  DeliverReadyRegression,
  DeliverReceivedProgress,
  DeliverReceivedRegression,
  DeliverTotalChanged,
  DeliverBoundary,
  DeliverZeroCooldown,
  DeliverFutureClock
}

DeliverHasOld(c) ==
  c /= DeliverVacant

DeliverOldLast(c) ==
  IF c = DeliverFutureClock THEN 30 ELSE 10

DeliverNow(c) ==
  CASE c = DeliverBoundary -> 20
    [] c = DeliverZeroCooldown -> 10
    [] c = DeliverFutureClock -> 9
    [] OTHER -> 19

DeliverCooldown(c) ==
  IF c = DeliverZeroCooldown THEN 0 ELSE 10

DeliverOldReady(c) == 2
DeliverOldReceived(c) == 3
DeliverOldTotal(c) == 5

DeliverReady(c) ==
  CASE c = DeliverReadyProgress -> 3
    [] c = DeliverReadyRegression -> 1
    [] OTHER -> 2

DeliverReceived(c) ==
  CASE c = DeliverReceivedProgress -> 4
    [] c = DeliverReceivedRegression -> 2
    [] OTHER -> 3

DeliverTotal(c) ==
  IF c = DeliverTotalChanged THEN 6 ELSE 5

Age(now, last) ==
  IF now >= last THEN now - last ELSE 0

DeliverProgress(c) ==
  DeliverReady(c) > DeliverOldReady(c)
  \/ DeliverReceived(c) > DeliverOldReceived(c)
  \/ DeliverTotal(c) # DeliverOldTotal(c)

DeliverUpdate(c, emit) ==
  IF emit THEN
    <<TRUE, DeliverNow(c), DeliverReady(c), DeliverReceived(c), DeliverTotal(c)>>
  ELSE
    <<FALSE, DeliverOldLast(c), DeliverOldReady(c), DeliverOldReceived(c),
      DeliverOldTotal(c)>>

SpecDeliver(c) ==
  IF ~DeliverHasOld(c) THEN
    <<TRUE, DeliverNow(c), DeliverReady(c), DeliverReceived(c), DeliverTotal(c)>>
  ELSE
    DeliverUpdate(
      c,
      DeliverProgress(c)
      \/ Age(DeliverNow(c), DeliverOldLast(c)) >= DeliverCooldown(c)
    )

ActualDeliver(c) ==
  CASE Bug = "deliver_vacant_suppressed"
       /\ c = DeliverVacant ->
         <<FALSE, DeliverOldLast(c), DeliverOldReady(c), DeliverOldReceived(c),
           DeliverOldTotal(c)>>
    [] Bug = "deliver_no_progress_relogs"
       /\ c = DeliverNoProgressBefore ->
         <<TRUE, DeliverNow(c), DeliverReady(c), DeliverReceived(c), DeliverTotal(c)>>
    [] Bug = "deliver_ready_progress_ignored"
       /\ c = DeliverReadyProgress ->
         <<FALSE, DeliverOldLast(c), DeliverOldReady(c), DeliverOldReceived(c),
           DeliverOldTotal(c)>>
    [] Bug = "deliver_ready_regression_counts"
       /\ c = DeliverReadyRegression ->
         <<TRUE, DeliverNow(c), DeliverReady(c), DeliverReceived(c), DeliverTotal(c)>>
    [] Bug = "deliver_received_progress_ignored"
       /\ c = DeliverReceivedProgress ->
         <<FALSE, DeliverOldLast(c), DeliverOldReady(c), DeliverOldReceived(c),
           DeliverOldTotal(c)>>
    [] Bug = "deliver_total_change_ignored"
       /\ c = DeliverTotalChanged ->
         <<FALSE, DeliverOldLast(c), DeliverOldReady(c), DeliverOldReceived(c),
           DeliverOldTotal(c)>>
    [] Bug = "deliver_boundary_suppressed"
       /\ c = DeliverBoundary ->
         <<FALSE, DeliverOldLast(c), DeliverOldReady(c), DeliverOldReceived(c),
           DeliverOldTotal(c)>>
    [] Bug = "deliver_zero_cooldown_suppressed"
       /\ c = DeliverZeroCooldown ->
         <<FALSE, DeliverOldLast(c), DeliverOldReady(c), DeliverOldReceived(c),
           DeliverOldTotal(c)>>
    [] Bug = "deliver_future_elapsed_underflows"
       /\ c = DeliverFutureClock ->
         <<TRUE, DeliverNow(c), DeliverReady(c), DeliverReceived(c), DeliverTotal(c)>>
    [] Bug = "deliver_progress_keeps_old_state"
       /\ c = DeliverReadyProgress ->
         <<TRUE, DeliverOldLast(c), DeliverOldReady(c), DeliverOldReceived(c),
           DeliverOldTotal(c)>>
    [] OTHER -> SpecDeliver(c)

ReadyVacant == "ready_vacant"
ReadyNoProgressBefore == "ready_no_progress_before"
ReadyReasonChanged == "ready_reason_changed"
ReadyReadyProgress == "ready_ready_progress"
ReadyReadyRegression == "ready_ready_regression"
ReadyRequiredChanged == "ready_required_changed"
ReadyReceivedProgress == "ready_received_progress"
ReadyTotalChanged == "ready_total_changed"
ReadyBoundary == "ready_boundary"
ReadyZeroCooldown == "ready_zero_cooldown"
ReadyFutureClock == "ready_future_clock"

ReadyCases == {
  ReadyVacant,
  ReadyNoProgressBefore,
  ReadyReasonChanged,
  ReadyReadyProgress,
  ReadyReadyRegression,
  ReadyRequiredChanged,
  ReadyReceivedProgress,
  ReadyTotalChanged,
  ReadyBoundary,
  ReadyZeroCooldown,
  ReadyFutureClock
}

ReadyHasOld(c) ==
  c /= ReadyVacant

ReadyOldLast(c) ==
  IF c = ReadyFutureClock THEN 30 ELSE 10

ReadyNow(c) ==
  CASE c = ReadyBoundary -> 20
    [] c = ReadyZeroCooldown -> 10
    [] c = ReadyFutureClock -> 9
    [] OTHER -> 19

ReadyCooldown(c) ==
  IF c = ReadyZeroCooldown THEN 0 ELSE 10

ReadyOldReason(c) == ReasonA
ReadyOldReady(c) == 2
ReadyOldRequired(c) == 4
ReadyOldReceived(c) == 3
ReadyOldTotal(c) == 5

ReadyReason(c) ==
  IF c = ReadyReasonChanged THEN ReasonB ELSE ReasonA

ReadyReady(c) ==
  CASE c = ReadyReadyProgress -> 3
    [] c = ReadyReadyRegression -> 1
    [] OTHER -> 2

ReadyRequired(c) ==
  IF c = ReadyRequiredChanged THEN 5 ELSE 4

ReadyReceived(c) ==
  IF c = ReadyReceivedProgress THEN 4 ELSE 3

ReadyTotal(c) ==
  IF c = ReadyTotalChanged THEN 6 ELSE 5

ReadyProgress(c) ==
  ReadyReason(c) # ReadyOldReason(c)
  \/ ReadyReady(c) > ReadyOldReady(c)
  \/ ReadyRequired(c) # ReadyOldRequired(c)
  \/ ReadyReceived(c) > ReadyOldReceived(c)
  \/ ReadyTotal(c) # ReadyOldTotal(c)

ReadyUpdate(c, emit) ==
  IF emit THEN
    <<TRUE, ReadyNow(c), ReadyReason(c), ReadyReady(c), ReadyRequired(c),
      ReadyReceived(c), ReadyTotal(c)>>
  ELSE
    <<FALSE, ReadyOldLast(c), ReadyOldReason(c), ReadyOldReady(c),
      ReadyOldRequired(c), ReadyOldReceived(c), ReadyOldTotal(c)>>

SpecReady(c) ==
  IF ~ReadyHasOld(c) THEN
    <<TRUE, ReadyNow(c), ReadyReason(c), ReadyReady(c), ReadyRequired(c),
      ReadyReceived(c), ReadyTotal(c)>>
  ELSE
    ReadyUpdate(
      c,
      ReadyProgress(c)
      \/ Age(ReadyNow(c), ReadyOldLast(c)) >= ReadyCooldown(c)
    )

ActualReady(c) ==
  CASE Bug = "ready_vacant_suppressed"
       /\ c = ReadyVacant ->
         <<FALSE, ReadyOldLast(c), ReadyOldReason(c), ReadyOldReady(c),
           ReadyOldRequired(c), ReadyOldReceived(c), ReadyOldTotal(c)>>
    [] Bug = "ready_reason_change_ignored"
       /\ c = ReadyReasonChanged ->
         <<FALSE, ReadyOldLast(c), ReadyOldReason(c), ReadyOldReady(c),
           ReadyOldRequired(c), ReadyOldReceived(c), ReadyOldTotal(c)>>
    [] Bug = "ready_ready_progress_ignored"
       /\ c = ReadyReadyProgress ->
         <<FALSE, ReadyOldLast(c), ReadyOldReason(c), ReadyOldReady(c),
           ReadyOldRequired(c), ReadyOldReceived(c), ReadyOldTotal(c)>>
    [] Bug = "ready_required_change_ignored"
       /\ c = ReadyRequiredChanged ->
         <<FALSE, ReadyOldLast(c), ReadyOldReason(c), ReadyOldReady(c),
           ReadyOldRequired(c), ReadyOldReceived(c), ReadyOldTotal(c)>>
    [] Bug = "ready_received_progress_ignored"
       /\ c = ReadyReceivedProgress ->
         <<FALSE, ReadyOldLast(c), ReadyOldReason(c), ReadyOldReady(c),
           ReadyOldRequired(c), ReadyOldReceived(c), ReadyOldTotal(c)>>
    [] Bug = "ready_total_change_ignored"
       /\ c = ReadyTotalChanged ->
         <<FALSE, ReadyOldLast(c), ReadyOldReason(c), ReadyOldReady(c),
           ReadyOldRequired(c), ReadyOldReceived(c), ReadyOldTotal(c)>>
    [] Bug = "ready_ready_regression_counts"
       /\ c = ReadyReadyRegression ->
         <<TRUE, ReadyNow(c), ReadyReason(c), ReadyReady(c), ReadyRequired(c),
           ReadyReceived(c), ReadyTotal(c)>>
    [] Bug = "ready_before_cooldown_relogs"
       /\ c = ReadyNoProgressBefore ->
         <<TRUE, ReadyNow(c), ReadyReason(c), ReadyReady(c), ReadyRequired(c),
           ReadyReceived(c), ReadyTotal(c)>>
    [] Bug = "ready_boundary_suppressed"
       /\ c = ReadyBoundary ->
         <<FALSE, ReadyOldLast(c), ReadyOldReason(c), ReadyOldReady(c),
           ReadyOldRequired(c), ReadyOldReceived(c), ReadyOldTotal(c)>>
    [] Bug = "ready_zero_cooldown_suppressed"
       /\ c = ReadyZeroCooldown ->
         <<FALSE, ReadyOldLast(c), ReadyOldReason(c), ReadyOldReady(c),
           ReadyOldRequired(c), ReadyOldReceived(c), ReadyOldTotal(c)>>
    [] Bug = "ready_future_elapsed_underflows"
       /\ c = ReadyFutureClock ->
         <<TRUE, ReadyNow(c), ReadyReason(c), ReadyReady(c), ReadyRequired(c),
           ReadyReceived(c), ReadyTotal(c)>>
    [] Bug = "ready_progress_keeps_old_state"
       /\ c = ReadyReasonChanged ->
         <<TRUE, ReadyOldLast(c), ReadyOldReason(c), ReadyOldReady(c),
           ReadyOldRequired(c), ReadyOldReceived(c), ReadyOldTotal(c)>>
    [] OTHER -> SpecReady(c)

BugSet == {
  "none",
  "deliver_vacant_suppressed",
  "deliver_no_progress_relogs",
  "deliver_ready_progress_ignored",
  "deliver_ready_regression_counts",
  "deliver_received_progress_ignored",
  "deliver_total_change_ignored",
  "deliver_boundary_suppressed",
  "deliver_zero_cooldown_suppressed",
  "deliver_future_elapsed_underflows",
  "deliver_progress_keeps_old_state",
  "ready_vacant_suppressed",
  "ready_reason_change_ignored",
  "ready_ready_progress_ignored",
  "ready_required_change_ignored",
  "ready_received_progress_ignored",
  "ready_total_change_ignored",
  "ready_ready_regression_counts",
  "ready_before_cooldown_relogs",
  "ready_boundary_suppressed",
  "ready_zero_cooldown_suppressed",
  "ready_future_elapsed_underflows",
  "ready_progress_keeps_old_state"
}

Init ==
  checked = 0

Next ==
  \/ /\ checked < 22
     /\ checked' = checked + 1
  \/ /\ checked = 22
     /\ checked' = checked

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked \in 0..22
  /\ \A c \in DeliverCases:
       /\ ActualDeliver(c)[1] \in BOOLEAN
       /\ ActualDeliver(c)[2] \in Nat
       /\ ActualDeliver(c)[3] \in Nat
       /\ ActualDeliver(c)[4] \in Nat
       /\ ActualDeliver(c)[5] \in Nat
  /\ \A c \in ReadyCases:
       /\ ActualReady(c)[1] \in BOOLEAN
       /\ ActualReady(c)[2] \in Nat
       /\ ActualReady(c)[3] \in Reasons
       /\ ActualReady(c)[4] \in Nat
       /\ ActualReady(c)[5] \in Nat
       /\ ActualReady(c)[6] \in Nat
       /\ ActualReady(c)[7] \in Nat

DeliverExact ==
  \A c \in DeliverCases:
    ActualDeliver(c) = SpecDeliver(c)

ReadyExact ==
  \A c \in ReadyCases:
    ActualReady(c) = SpecReady(c)

DeliverStable ==
  /\ ActualDeliver(DeliverVacant)[1]
  /\ ~ActualDeliver(DeliverNoProgressBefore)[1]
  /\ ActualDeliver(DeliverReadyProgress)[1]
  /\ ~ActualDeliver(DeliverReadyRegression)[1]
  /\ ActualDeliver(DeliverReceivedProgress)[1]
  /\ ~ActualDeliver(DeliverReceivedRegression)[1]
  /\ ActualDeliver(DeliverTotalChanged)[1]
  /\ ActualDeliver(DeliverBoundary)[1]
  /\ ActualDeliver(DeliverZeroCooldown)[1]
  /\ ~ActualDeliver(DeliverFutureClock)[1]
  /\ ActualDeliver(DeliverReadyProgress)[2] = DeliverNow(DeliverReadyProgress)
  /\ ActualDeliver(DeliverReadyProgress)[3] = DeliverReady(DeliverReadyProgress)

ReadyStable ==
  /\ ActualReady(ReadyVacant)[1]
  /\ ~ActualReady(ReadyNoProgressBefore)[1]
  /\ ActualReady(ReadyReasonChanged)[1]
  /\ ActualReady(ReadyReadyProgress)[1]
  /\ ~ActualReady(ReadyReadyRegression)[1]
  /\ ActualReady(ReadyRequiredChanged)[1]
  /\ ActualReady(ReadyReceivedProgress)[1]
  /\ ActualReady(ReadyTotalChanged)[1]
  /\ ActualReady(ReadyBoundary)[1]
  /\ ActualReady(ReadyZeroCooldown)[1]
  /\ ~ActualReady(ReadyFutureClock)[1]
  /\ ActualReady(ReadyReasonChanged)[2] = ReadyNow(ReadyReasonChanged)
  /\ ActualReady(ReadyReasonChanged)[3] = ReasonB

RbcDeferralThrottleCoreSafety ==
  /\ DeliverExact
  /\ ReadyExact
  /\ DeliverStable
  /\ ReadyStable

SafetyFast ==
  RbcDeferralThrottleCoreSafety

AllDeliverCasesMatchSpec ==
  \A c \in DeliverCases:
    ActualDeliver(c) = SpecDeliver(c)

AllReadyCasesMatchSpec ==
  \A c \in ReadyCases:
    ActualReady(c) = SpecReady(c)

DeliverFirstAndProgressAnchors ==
  /\ ActualDeliver(DeliverVacant) = <<TRUE, 19, 2, 3, 5>>
  /\ ActualDeliver(DeliverNoProgressBefore) = <<FALSE, 10, 2, 3, 5>>
  /\ ActualDeliver(DeliverReadyProgress) = <<TRUE, 19, 3, 3, 5>>
  /\ ActualDeliver(DeliverReadyRegression) = <<FALSE, 10, 2, 3, 5>>
  /\ ActualDeliver(DeliverReceivedProgress) = <<TRUE, 19, 2, 4, 5>>
  /\ ActualDeliver(DeliverReceivedRegression) = <<FALSE, 10, 2, 3, 5>>
  /\ ActualDeliver(DeliverTotalChanged) = <<TRUE, 19, 2, 3, 6>>

DeliverCooldownAnchors ==
  /\ ActualDeliver(DeliverBoundary) = <<TRUE, 20, 2, 3, 5>>
  /\ ActualDeliver(DeliverZeroCooldown) = <<TRUE, 10, 2, 3, 5>>
  /\ ActualDeliver(DeliverFutureClock) = <<FALSE, 30, 2, 3, 5>>

DeliverStateReplacementAnchors ==
  /\ ActualDeliver(DeliverReadyProgress)[2] =
       DeliverNow(DeliverReadyProgress)
  /\ ActualDeliver(DeliverReadyProgress)[3] =
       DeliverReady(DeliverReadyProgress)
  /\ ActualDeliver(DeliverNoProgressBefore)[2] =
       DeliverOldLast(DeliverNoProgressBefore)

ReadyFirstAndProgressAnchors ==
  /\ ActualReady(ReadyVacant) = <<TRUE, 19, ReasonA, 2, 4, 3, 5>>
  /\ ActualReady(ReadyNoProgressBefore) =
       <<FALSE, 10, ReasonA, 2, 4, 3, 5>>
  /\ ActualReady(ReadyReasonChanged) =
       <<TRUE, 19, ReasonB, 2, 4, 3, 5>>
  /\ ActualReady(ReadyReadyProgress) =
       <<TRUE, 19, ReasonA, 3, 4, 3, 5>>
  /\ ActualReady(ReadyReadyRegression) =
       <<FALSE, 10, ReasonA, 2, 4, 3, 5>>
  /\ ActualReady(ReadyRequiredChanged) =
       <<TRUE, 19, ReasonA, 2, 5, 3, 5>>
  /\ ActualReady(ReadyReceivedProgress) =
       <<TRUE, 19, ReasonA, 2, 4, 4, 5>>
  /\ ActualReady(ReadyTotalChanged) =
       <<TRUE, 19, ReasonA, 2, 4, 3, 6>>

ReadyCooldownAnchors ==
  /\ ActualReady(ReadyBoundary) = <<TRUE, 20, ReasonA, 2, 4, 3, 5>>
  /\ ActualReady(ReadyZeroCooldown) = <<TRUE, 10, ReasonA, 2, 4, 3, 5>>
  /\ ActualReady(ReadyFutureClock) = <<FALSE, 30, ReasonA, 2, 4, 3, 5>>

ReadyStateReplacementAnchors ==
  /\ ActualReady(ReadyReasonChanged)[2] = ReadyNow(ReadyReasonChanged)
  /\ ActualReady(ReadyReasonChanged)[3] = ReasonB
  /\ ActualReady(ReadyNoProgressBefore)[2] =
       ReadyOldLast(ReadyNoProgressBefore)

SafetyAnchors ==
  /\ AllDeliverCasesMatchSpec
  /\ AllReadyCasesMatchSpec
  /\ DeliverFirstAndProgressAnchors
  /\ DeliverCooldownAnchors
  /\ DeliverStateReplacementAnchors
  /\ ReadyFirstAndProgressAnchors
  /\ ReadyCooldownAnchors
  /\ ReadyStateReplacementAnchors

====
