---- MODULE SumeragiEvidenceHorizonGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `evidence_within_horizon(...)`.

The helper admits proposal/QC evidence when its subject height is no older than
the configured horizon behind the current committed height. A zero horizon
disables filtering, missing subject height defaults to the current height, and
the lower bound saturates at zero instead of underflowing.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoSubject == -1

Cases == {
  "zero_horizon_stale",
  "zero_horizon_missing",
  "missing_subject_nonzero",
  "exact_lower_bound",
  "below_lower_bound",
  "above_lower_bound",
  "saturating_zero_subject",
  "current_zero_subject_zero",
  "future_subject",
  "stale_when_horizon_one"
}

\* @type: Str => Int;
CurrentHeight(c) ==
  CASE c = "current_zero_subject_zero" -> 0
    [] c \in {"saturating_zero_subject"} -> 5
    [] OTHER -> 10

\* @type: Str => Int;
Horizon(c) ==
  CASE c \in {"zero_horizon_stale", "zero_horizon_missing"} -> 0
    [] c = "saturating_zero_subject" -> 10
    [] c = "current_zero_subject_zero" -> 10
    [] c = "stale_when_horizon_one" -> 1
    [] OTHER -> 3

\* @type: Str => Int;
SubjectHeight(c) ==
  CASE c \in {"zero_horizon_missing", "missing_subject_nonzero"} -> NoSubject
    [] c = "zero_horizon_stale" -> 0
    [] c = "exact_lower_bound" -> 7
    [] c = "below_lower_bound" -> 6
    [] c = "above_lower_bound" -> 8
    [] c = "saturating_zero_subject" -> 0
    [] c = "current_zero_subject_zero" -> 0
    [] c = "future_subject" -> 12
    [] c = "stale_when_horizon_one" -> 8

\* @type: Str => Int;
Reference(c) ==
  IF SubjectHeight(c) = NoSubject THEN CurrentHeight(c) ELSE SubjectHeight(c)

\* @type: Str => Int;
LowerBound(c) ==
  IF CurrentHeight(c) >= Horizon(c) THEN CurrentHeight(c) - Horizon(c) ELSE 0

\* @type: Str => Int;
UnderflowLowerBound(c) ==
  IF CurrentHeight(c) >= Horizon(c) THEN
    CurrentHeight(c) - Horizon(c)
  ELSE
    Horizon(c) - CurrentHeight(c)

\* @type: Str => Int;
MissingSubjectZeroReference(c) ==
  IF SubjectHeight(c) = NoSubject THEN 0 ELSE SubjectHeight(c)

\* @type: Str => Bool;
SpecAllowed(c) ==
  IF Horizon(c) = 0 THEN TRUE ELSE Reference(c) >= LowerBound(c)

\* @type: Str => Bool;
ActualAllowed(c) ==
  CASE Bug = "zero_horizon_rejects_old" /\ Horizon(c) = 0 ->
       Reference(c) >= CurrentHeight(c)
    [] Bug = "missing_subject_uses_zero" ->
       IF Horizon(c) = 0 THEN TRUE ELSE MissingSubjectZeroReference(c) >= LowerBound(c)
    [] Bug = "missing_subject_rejects" /\ SubjectHeight(c) = NoSubject /\ Horizon(c) # 0 ->
       FALSE
    [] Bug = "non_saturating_underflow" ->
       IF Horizon(c) = 0 THEN TRUE ELSE Reference(c) >= UnderflowLowerBound(c)
    [] Bug = "strict_lower_bound" ->
       IF Horizon(c) = 0 THEN TRUE ELSE Reference(c) > LowerBound(c)
    [] Bug = "stale_allowed" /\ c = "below_lower_bound" ->
       TRUE
    [] Bug = "uses_current_as_lower_bound" ->
       IF Horizon(c) = 0 THEN TRUE ELSE Reference(c) >= CurrentHeight(c)
    [] Bug = "future_rejected" /\ Reference(c) > CurrentHeight(c) ->
       FALSE
    [] Bug = "ignores_subject_height" ->
       IF Horizon(c) = 0 THEN TRUE ELSE CurrentHeight(c) >= LowerBound(c)
    [] Bug = "lower_bound_zero_for_all" ->
       IF Horizon(c) = 0 THEN TRUE ELSE Reference(c) >= 0
    [] Bug = "horizon_as_lower_bound" ->
       IF Horizon(c) = 0 THEN TRUE ELSE Reference(c) >= Horizon(c)
    [] OTHER -> SpecAllowed(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "zero_horizon_rejects_old",
       "missing_subject_uses_zero",
       "missing_subject_rejects",
       "non_saturating_underflow",
       "strict_lower_bound",
       "stale_allowed",
       "uses_current_as_lower_bound",
       "future_rejected",
       "ignores_subject_height",
       "lower_bound_zero_for_all",
       "horizon_as_lower_bound"
     }
  /\ checked \in 0..1
  /\ NoSubject = -1
  /\ \A c \in Cases:
       /\ CurrentHeight(c) \in 0..12
       /\ Horizon(c) \in 0..12
       /\ SubjectHeight(c) \in -1..12
       /\ Reference(c) \in 0..12
       /\ LowerBound(c) \in 0..12
       /\ UnderflowLowerBound(c) \in 0..12
       /\ MissingSubjectZeroReference(c) \in 0..12
       /\ SpecAllowed(c) \in BOOLEAN
       /\ ActualAllowed(c) \in BOOLEAN

MatchesSpec ==
  \A c \in Cases:
    ActualAllowed(c) = SpecAllowed(c)

ZeroHorizonDisablesFilter ==
  /\ ActualAllowed("zero_horizon_stale")
  /\ ActualAllowed("zero_horizon_missing")

MissingSubjectDefaultsToCurrent ==
  ActualAllowed("missing_subject_nonzero")

SaturatingLowerBound ==
  /\ ActualAllowed("saturating_zero_subject")
  /\ ActualAllowed("current_zero_subject_zero")

InclusiveLowerBound ==
  ActualAllowed("exact_lower_bound")

StaleEvidenceRejected ==
  /\ ~ActualAllowed("below_lower_bound")
  /\ ~ActualAllowed("stale_when_horizon_one")

RecentAndFutureEvidenceAllowed ==
  /\ ActualAllowed("above_lower_bound")
  /\ ActualAllowed("future_subject")

SafetyFast ==
  /\ MatchesSpec
  /\ ZeroHorizonDisablesFilter
  /\ MissingSubjectDefaultsToCurrent
  /\ SaturatingLowerBound
  /\ InclusiveLowerBound
  /\ StaleEvidenceRejected
  /\ RecentAndFutureEvidenceAllowed

Safety ==
  SafetyFast

====
