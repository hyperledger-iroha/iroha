---- MODULE SumeragiInvalidSignatureLabelsGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for invalid-signature telemetry labels.

This slice pins `InvalidSigKind::as_str(...)`, the telemetry-only
`InvalidSigKind::label(...)` wrapper, `InvalidSigOutcome::label(...)`, and
`RbcMismatchLogOutcome::should_log(...)` from `main_loop.rs`. The throttle gate
proves the state-machine behavior; this companion gate fixes the exported
labels and outcome predicate used by metrics and logs.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Vote == "Vote"
RbcInit == "RbcInit"
RbcReady == "RbcReady"
RbcDeliver == "RbcDeliver"

KindCases == {Vote, RbcInit, RbcReady, RbcDeliver}

Logged == "Logged"
Throttled == "Throttled"

OutcomeCases == {Logged, Throttled}

SpecKindAsStr(kind) ==
  CASE kind = Vote -> "vote"
    [] kind = RbcInit -> "rbc_init"
    [] kind = RbcReady -> "rbc_ready"
    [] kind = RbcDeliver -> "rbc_deliver"
    [] OTHER -> "unknown"

ActualKindAsStr(kind) ==
  CASE Bug = "kind_vote_as_rbc_init"
       /\ kind = Vote -> "rbc_init"
    [] Bug = "kind_rbc_init_as_vote"
       /\ kind = RbcInit -> "vote"
    [] Bug = "kind_rbc_ready_as_deliver"
       /\ kind = RbcReady -> "rbc_deliver"
    [] Bug = "kind_rbc_deliver_as_ready"
       /\ kind = RbcDeliver -> "rbc_ready"
    [] OTHER -> SpecKindAsStr(kind)

SpecKindTelemetryLabel(kind) ==
  SpecKindAsStr(kind)

ActualKindTelemetryLabel(kind) ==
  CASE Bug = "kind_label_wrapper_diverges"
       /\ kind = Vote -> "rbc_deliver"
    [] OTHER -> ActualKindAsStr(kind)

SpecInvalidOutcomeLabel(outcome) ==
  CASE outcome = Logged -> "logged"
    [] outcome = Throttled -> "throttled"
    [] OTHER -> "unknown"

ActualInvalidOutcomeLabel(outcome) ==
  CASE Bug = "outcome_logged_as_throttled"
       /\ outcome = Logged -> "throttled"
    [] Bug = "outcome_throttled_as_logged"
       /\ outcome = Throttled -> "logged"
    [] OTHER -> SpecInvalidOutcomeLabel(outcome)

SpecRbcMismatchShouldLog(outcome) ==
  outcome = Logged

ActualRbcMismatchShouldLog(outcome) ==
  CASE Bug = "rbc_logged_should_log_false"
       /\ outcome = Logged -> FALSE
    [] Bug = "rbc_throttled_should_log_true"
       /\ outcome = Throttled -> TRUE
    [] OTHER -> SpecRbcMismatchShouldLog(outcome)

KindLabels == {SpecKindAsStr(kind): kind \in KindCases}
OutcomeLabels == {SpecInvalidOutcomeLabel(outcome): outcome \in OutcomeCases}

BugSet == {
  "none",
  "kind_vote_as_rbc_init",
  "kind_rbc_init_as_vote",
  "kind_rbc_ready_as_deliver",
  "kind_rbc_deliver_as_ready",
  "kind_label_wrapper_diverges",
  "outcome_logged_as_throttled",
  "outcome_throttled_as_logged",
  "rbc_logged_should_log_false",
  "rbc_throttled_should_log_true"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0
  /\ \A kind \in KindCases: ActualKindAsStr(kind) \in KindLabels
  /\ \A kind \in KindCases: ActualKindTelemetryLabel(kind) \in KindLabels
  /\ \A outcome \in OutcomeCases:
      ActualInvalidOutcomeLabel(outcome) \in OutcomeLabels
  /\ \A outcome \in OutcomeCases:
      ActualRbcMismatchShouldLog(outcome) \in BOOLEAN

KindLabelsExact ==
  \A kind \in KindCases:
    ActualKindAsStr(kind) = SpecKindAsStr(kind)

KindTelemetryLabelsWrapAsStr ==
  \A kind \in KindCases:
    ActualKindTelemetryLabel(kind) = ActualKindAsStr(kind)

InvalidOutcomeLabelsExact ==
  \A outcome \in OutcomeCases:
    ActualInvalidOutcomeLabel(outcome) = SpecInvalidOutcomeLabel(outcome)

RbcMismatchShouldLogExact ==
  \A outcome \in OutcomeCases:
    ActualRbcMismatchShouldLog(outcome) = SpecRbcMismatchShouldLog(outcome)

KindLabelsDistinct ==
  \A a, b \in KindCases:
    a # b => ActualKindAsStr(a) # ActualKindAsStr(b)

OutcomeLabelsDistinct ==
  ActualInvalidOutcomeLabel(Logged) # ActualInvalidOutcomeLabel(Throttled)

SampleMetricTupleStable ==
  /\ ActualKindTelemetryLabel(RbcReady) = "rbc_ready"
  /\ ActualInvalidOutcomeLabel(Throttled) = "throttled"
  /\ ActualRbcMismatchShouldLog(Logged)
  /\ ~ActualRbcMismatchShouldLog(Throttled)

SafetyFast ==
  /\ KindLabelsExact
  /\ KindTelemetryLabelsWrapAsStr
  /\ InvalidOutcomeLabelsExact
  /\ RbcMismatchShouldLogExact
  /\ KindLabelsDistinct
  /\ OutcomeLabelsDistinct
  /\ SampleMetricTupleStable

ConcreteKindLabelAnchors ==
  /\ ActualKindAsStr(Vote) = "vote"
  /\ ActualKindAsStr(RbcInit) = "rbc_init"
  /\ ActualKindAsStr(RbcReady) = "rbc_ready"
  /\ ActualKindAsStr(RbcDeliver) = "rbc_deliver"

ConcreteTelemetryWrapperAnchors ==
  \A kind \in KindCases:
    ActualKindTelemetryLabel(kind) = ActualKindAsStr(kind)

ConcreteOutcomeLabelAnchors ==
  /\ ActualInvalidOutcomeLabel(Logged) = "logged"
  /\ ActualInvalidOutcomeLabel(Throttled) = "throttled"

ConcreteRbcShouldLogAnchors ==
  /\ ActualRbcMismatchShouldLog(Logged)
  /\ ~ActualRbcMismatchShouldLog(Throttled)

ConcreteLabelSetAnchors ==
  /\ KindLabels = {"vote", "rbc_init", "rbc_ready", "rbc_deliver"}
  /\ OutcomeLabels = {"logged", "throttled"}

InvalidSignatureLabelSafetyAnchors ==
  /\ ConcreteKindLabelAnchors
  /\ ConcreteTelemetryWrapperAnchors
  /\ ConcreteOutcomeLabelAnchors
  /\ ConcreteRbcShouldLogAnchors
  /\ ConcreteLabelSetAnchors
  /\ KindLabelsDistinct
  /\ OutcomeLabelsDistinct

InvalidSignatureLabelsExactness ==
  /\ KindLabelsExact
  /\ KindTelemetryLabelsWrapAsStr
  /\ InvalidOutcomeLabelsExact
  /\ RbcMismatchShouldLogExact
  /\ KindLabelsDistinct
  /\ OutcomeLabelsDistinct
  /\ SampleMetricTupleStable
  /\ InvalidSignatureLabelSafetyAnchors

InvalidSignatureLabelsCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ InvalidSignatureLabelsExactness

====
