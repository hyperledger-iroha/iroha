---- MODULE SumeragiQcRoundCompatibilityGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for pure-engine QC/round compatibility.

This slice models `qc_ref_is_compatible_with_round(...)`, used by
`ConsensusEngine::on_proposal(...)` and
`ConsensusEngine::on_new_view_qc(...)` before carried highest-QC evidence can
unlock proposals or advance NewView context. A QC is compatible exactly when it
belongs to the same epoch and either comes from a lower height, or from the
same height with a view no greater than the candidate round view. Future
heights, same-height future views, and wrong epochs are rejected.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugIgnoreEpoch,
  \* @type: Bool;
  BugRejectLowerHeight,
  \* @type: Bool;
  BugRequireViewForLowerHeight,
  \* @type: Bool;
  BugAcceptSameHeightFutureView,
  \* @type: Bool;
  BugAcceptFutureHeight,
  \* @type: Bool;
  BugRejectSameHeightPastView,
  \* @type: Bool;
  BugRejectSameHeightEqualView,
  \* @type: Bool;
  BugUseViewOnlyComparison

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

RoundEpoch == 1
RoundHeight == 10
RoundView == 5

Cases == {
  "lowerHeightLowerView",
  "lowerHeightEqualView",
  "lowerHeightHigherView",
  "sameHeightPastView",
  "sameHeightEqualView",
  "sameHeightFutureView",
  "futureHeightLowerView",
  "futureHeightEqualView",
  "futureHeightHigherView",
  "wrongEpochLowerHeight",
  "wrongEpochSameHeightPastView",
  "wrongEpochSameHeightEqualView",
  "wrongEpochFutureHeight"
}

QcEpoch(candidate) ==
  CASE candidate \in {
      "wrongEpochLowerHeight",
      "wrongEpochSameHeightPastView",
      "wrongEpochSameHeightEqualView",
      "wrongEpochFutureHeight"
    } -> 2
    [] OTHER -> RoundEpoch

QcHeight(candidate) ==
  CASE candidate \in {
      "lowerHeightLowerView",
      "lowerHeightEqualView",
      "lowerHeightHigherView",
      "wrongEpochLowerHeight"
    } -> 9
    [] candidate \in {
      "sameHeightPastView",
      "sameHeightEqualView",
      "sameHeightFutureView",
      "wrongEpochSameHeightPastView",
      "wrongEpochSameHeightEqualView"
    } -> RoundHeight
    [] OTHER -> 11

QcView(candidate) ==
  CASE candidate \in {
      "lowerHeightLowerView",
      "futureHeightLowerView",
      "sameHeightPastView",
      "wrongEpochSameHeightPastView"
    } -> 4
    [] candidate \in {
      "lowerHeightEqualView",
      "futureHeightEqualView",
      "sameHeightEqualView",
      "wrongEpochSameHeightEqualView"
    } -> RoundView
    [] OTHER -> 6

SpecCompatible(candidate) ==
  /\ QcEpoch(candidate) = RoundEpoch
  /\ \/ QcHeight(candidate) < RoundHeight
     \/ /\ QcHeight(candidate) = RoundHeight
        /\ QcView(candidate) <= RoundView

EpochAccepted(candidate) ==
  \/ QcEpoch(candidate) = RoundEpoch
  \/ BugIgnoreEpoch

HeightViewAccepted(candidate) ==
  IF QcHeight(candidate) < RoundHeight
  THEN
    /\ ~BugRejectLowerHeight
    /\ \/ ~BugRequireViewForLowerHeight
       \/ QcView(candidate) <= RoundView
  ELSE IF QcHeight(candidate) = RoundHeight
  THEN
    IF QcView(candidate) < RoundView
    THEN ~BugRejectSameHeightPastView
    ELSE IF QcView(candidate) = RoundView
    THEN ~BugRejectSameHeightEqualView
    ELSE BugAcceptSameHeightFutureView
  ELSE
    BugAcceptFutureHeight

ImplementationCompatible(candidate) ==
  IF BugUseViewOnlyComparison
  THEN
    /\ EpochAccepted(candidate)
    /\ QcView(candidate) <= RoundView
  ELSE
    /\ EpochAccepted(candidate)
    /\ HeightViewAccepted(candidate)

TypeInvariant ==
  /\ BugIgnoreEpoch \in BOOLEAN
  /\ BugRejectLowerHeight \in BOOLEAN
  /\ BugRequireViewForLowerHeight \in BOOLEAN
  /\ BugAcceptSameHeightFutureView \in BOOLEAN
  /\ BugAcceptFutureHeight \in BOOLEAN
  /\ BugRejectSameHeightPastView \in BOOLEAN
  /\ BugRejectSameHeightEqualView \in BOOLEAN
  /\ BugUseViewOnlyComparison \in BOOLEAN
  /\ tried \subseteq Cases

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Cases \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Cases: TryCandidate(candidate)
  \/ Stable

CompatibilityMatchesSpec ==
  \A candidate \in tried:
    ImplementationCompatible(candidate) <=> SpecCompatible(candidate)

WrongEpochNeverCompatible ==
  \A candidate \in tried:
    QcEpoch(candidate) # RoundEpoch => ~ImplementationCompatible(candidate)

LowerHeightAlwaysCompatible ==
  \A candidate \in tried:
    /\ QcEpoch(candidate) = RoundEpoch
    /\ QcHeight(candidate) < RoundHeight
    => ImplementationCompatible(candidate)

LowerHeightIgnoresView ==
  "lowerHeightHigherView" \in tried =>
    ImplementationCompatible("lowerHeightHigherView")

SameHeightPastViewCompatible ==
  "sameHeightPastView" \in tried =>
    ImplementationCompatible("sameHeightPastView")

SameHeightEqualViewCompatible ==
  "sameHeightEqualView" \in tried =>
    ImplementationCompatible("sameHeightEqualView")

SameHeightFutureViewRejected ==
  "sameHeightFutureView" \in tried =>
    ~ImplementationCompatible("sameHeightFutureView")

FutureHeightNeverCompatible ==
  \A candidate \in tried:
    /\ QcEpoch(candidate) = RoundEpoch
    /\ QcHeight(candidate) > RoundHeight
    => ~ImplementationCompatible(candidate)

ViewOnlyComparisonIsRejected ==
  /\ "lowerHeightHigherView" \in tried
  /\ "futureHeightLowerView" \in tried
  => /\ ImplementationCompatible("lowerHeightHigherView")
     /\ ~ImplementationCompatible("futureHeightLowerView")

Safety ==
  /\ CompatibilityMatchesSpec
  /\ WrongEpochNeverCompatible
  /\ LowerHeightAlwaysCompatible
  /\ LowerHeightIgnoresView
  /\ SameHeightPastViewCompatible
  /\ SameHeightEqualViewCompatible
  /\ SameHeightFutureViewRejected
  /\ FutureHeightNeverCompatible
  /\ ViewOnlyComparisonIsRejected

====
