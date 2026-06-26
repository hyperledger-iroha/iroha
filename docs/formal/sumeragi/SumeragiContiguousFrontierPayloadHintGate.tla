---- MODULE SumeragiContiguousFrontierPayloadHintGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for contiguous-frontier payload-hint selection.

This slice pins `deferred_qc_phase_payload_hint_rank(...)` and
`contiguous_frontier_qc_payload_hint_hash(...)` from `main_loop.rs`. Deferred
missing-payload QCs at the frontier are preferred over proposal markers and
are selected by `(phase_rank, view, hash)`. Proposal markers are considered
only when no eligible deferred QC exists, and are selected by `(view, hash)`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Commit == "Commit"
Prepare == "Prepare"
NewView == "NewView"

Phases == {Commit, Prepare, NewView}

NoneSource == "none"
DeferredSource == "deferred_qc"
MarkerSource == "proposal_marker"

Sources == {NoneSource, DeferredSource, MarkerSource}

DeferredCommitOverPrepare == "DeferredCommitOverPrepare"
DeferredPrepareOverNewView == "DeferredPrepareOverNewView"
DeferredViewTieBreak == "DeferredViewTieBreak"
DeferredHashTieBreak == "DeferredHashTieBreak"
WrongHeightDeferredIgnored == "WrongHeightDeferredIgnored"
NonActionableDeferredIgnored == "NonActionableDeferredIgnored"
DeferredPreemptsMarker == "DeferredPreemptsMarker"
MarkerOnly == "MarkerOnly"
MarkerViewTieBreak == "MarkerViewTieBreak"
MarkerHashTieBreak == "MarkerHashTieBreak"
WrongHeightMarkerIgnored == "WrongHeightMarkerIgnored"
NonActionableMarkerIgnored == "NonActionableMarkerIgnored"
NoEligible == "NoEligible"

Cases == {
  DeferredCommitOverPrepare,
  DeferredPrepareOverNewView,
  DeferredViewTieBreak,
  DeferredHashTieBreak,
  WrongHeightDeferredIgnored,
  NonActionableDeferredIgnored,
  DeferredPreemptsMarker,
  MarkerOnly,
  MarkerViewTieBreak,
  MarkerHashTieBreak,
  WrongHeightMarkerIgnored,
  NonActionableMarkerIgnored,
  NoEligible
}

SpecPhaseRank(phase) ==
  CASE phase = Commit -> 3
    [] phase = Prepare -> 2
    [] phase = NewView -> 1

ActualPhaseRank(phase) ==
  CASE Bug = "commit_rank_below_prepare"
       /\ phase = Commit -> 1
    [] Bug = "new_view_rank_above_prepare"
       /\ phase = NewView -> 4
    [] OTHER -> SpecPhaseRank(phase)

SpecSource(c) ==
  CASE c \in {
       DeferredCommitOverPrepare,
       DeferredPrepareOverNewView,
       DeferredViewTieBreak,
       DeferredHashTieBreak,
       DeferredPreemptsMarker
     } -> DeferredSource
    [] c \in {
       WrongHeightDeferredIgnored,
       NonActionableDeferredIgnored,
       MarkerOnly,
       MarkerViewTieBreak,
       MarkerHashTieBreak,
       WrongHeightMarkerIgnored,
       NonActionableMarkerIgnored
     } -> MarkerSource
    [] OTHER -> NoneSource

SpecHash(c) ==
  CASE c = DeferredCommitOverPrepare -> 8
    [] c = DeferredPrepareOverNewView -> 6
    [] c = DeferredViewTieBreak -> 5
    [] c = DeferredHashTieBreak -> 9
    [] c = WrongHeightDeferredIgnored -> 3
    [] c = NonActionableDeferredIgnored -> 4
    [] c = DeferredPreemptsMarker -> 2
    [] c = MarkerOnly -> 5
    [] c = MarkerViewTieBreak -> 4
    [] c = MarkerHashTieBreak -> 8
    [] c = WrongHeightMarkerIgnored -> 2
    [] c = NonActionableMarkerIgnored -> 2
    [] c = NoEligible -> 0

ActualSource(c) ==
  CASE Bug \in {
       "wrong_height_deferred_allowed",
       "non_actionable_deferred_allowed"
     }
       /\ c \in {WrongHeightDeferredIgnored, NonActionableDeferredIgnored} ->
        DeferredSource
    [] Bug = "marker_preempts_deferred"
       /\ c = DeferredPreemptsMarker -> MarkerSource
    [] Bug = "marker_missing_when_no_deferred"
       /\ c = MarkerOnly -> NoneSource
    [] Bug \in {
       "wrong_height_marker_allowed",
       "non_actionable_marker_allowed",
       "no_eligible_returns_marker"
     } -> MarkerSource
    [] OTHER -> SpecSource(c)

ActualHash(c) ==
  CASE Bug = "commit_rank_below_prepare"
       /\ c = DeferredCommitOverPrepare -> 7
    [] Bug = "new_view_rank_above_prepare"
       /\ c = DeferredPrepareOverNewView -> 9
    [] Bug = "deferred_view_ignored"
       /\ c = DeferredViewTieBreak -> 7
    [] Bug = "deferred_hash_ignored"
       /\ c = DeferredHashTieBreak -> 4
    [] Bug = "wrong_height_deferred_allowed"
       /\ c = WrongHeightDeferredIgnored -> 9
    [] Bug = "non_actionable_deferred_allowed"
       /\ c = NonActionableDeferredIgnored -> 9
    [] Bug = "marker_preempts_deferred"
       /\ c = DeferredPreemptsMarker -> 9
    [] Bug = "marker_missing_when_no_deferred"
       /\ c = MarkerOnly -> 0
    [] Bug = "marker_view_ignored"
       /\ c = MarkerViewTieBreak -> 9
    [] Bug = "marker_hash_ignored"
       /\ c = MarkerHashTieBreak -> 3
    [] Bug = "wrong_height_marker_allowed"
       /\ c = WrongHeightMarkerIgnored -> 9
    [] Bug = "non_actionable_marker_allowed"
       /\ c = NonActionableMarkerIgnored -> 9
    [] Bug = "no_eligible_returns_marker"
       /\ c = NoEligible -> 1
    [] OTHER -> SpecHash(c)

BugSet == {
  "none",
  "commit_rank_below_prepare",
  "new_view_rank_above_prepare",
  "deferred_view_ignored",
  "deferred_hash_ignored",
  "wrong_height_deferred_allowed",
  "non_actionable_deferred_allowed",
  "marker_preempts_deferred",
  "marker_missing_when_no_deferred",
  "marker_view_ignored",
  "marker_hash_ignored",
  "wrong_height_marker_allowed",
  "non_actionable_marker_allowed",
  "no_eligible_returns_marker"
}

Init ==
  checked = 0

Next ==
  \/ /\ checked < 13
     /\ checked' = checked + 1
  \/ /\ checked = 13
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked \in 0..13
  /\ \A phase \in Phases: ActualPhaseRank(phase) \in 1..4
  /\ \A c \in Cases: ActualSource(c) \in Sources
  /\ \A c \in Cases: ActualHash(c) \in 0..9

PhaseRanksExact ==
  \A phase \in Phases:
    ActualPhaseRank(phase) = SpecPhaseRank(phase)

SelectionExact ==
  \A c \in Cases:
    /\ ActualSource(c) = SpecSource(c)
    /\ ActualHash(c) = SpecHash(c)

DeferredOrderingStable ==
  /\ ActualHash(DeferredCommitOverPrepare) = 8
  /\ ActualHash(DeferredPrepareOverNewView) = 6
  /\ ActualHash(DeferredViewTieBreak) = 5
  /\ ActualHash(DeferredHashTieBreak) = 9

DeferredEligibilityStable ==
  /\ ActualSource(WrongHeightDeferredIgnored) = MarkerSource
  /\ ActualHash(WrongHeightDeferredIgnored) = 3
  /\ ActualSource(NonActionableDeferredIgnored) = MarkerSource
  /\ ActualHash(NonActionableDeferredIgnored) = 4

DeferredPrecedenceStable ==
  /\ ActualSource(DeferredPreemptsMarker) = DeferredSource
  /\ ActualHash(DeferredPreemptsMarker) = 2

MarkerFallbackStable ==
  /\ ActualSource(MarkerOnly) = MarkerSource
  /\ ActualHash(MarkerOnly) = 5
  /\ ActualHash(MarkerViewTieBreak) = 4
  /\ ActualHash(MarkerHashTieBreak) = 8

MarkerEligibilityStable ==
  /\ ActualSource(WrongHeightMarkerIgnored) = MarkerSource
  /\ ActualHash(WrongHeightMarkerIgnored) = 2
  /\ ActualSource(NonActionableMarkerIgnored) = MarkerSource
  /\ ActualHash(NonActionableMarkerIgnored) = 2

EmptyFallbackStable ==
  /\ ActualSource(NoEligible) = NoneSource
  /\ ActualHash(NoEligible) = 0

PayloadHintHasExactPositiveEvidence ==
  /\ ActualSource(DeferredCommitOverPrepare) = DeferredSource
  /\ ActualSource(DeferredPrepareOverNewView) = DeferredSource
  /\ ActualSource(DeferredViewTieBreak) = DeferredSource
  /\ ActualSource(DeferredHashTieBreak) = DeferredSource
  /\ ActualSource(DeferredPreemptsMarker) = DeferredSource
  /\ ActualSource(MarkerOnly) = MarkerSource
  /\ ActualSource(MarkerViewTieBreak) = MarkerSource
  /\ ActualSource(MarkerHashTieBreak) = MarkerSource

PayloadHintPreservesDeterministicOrdering ==
  /\ ActualPhaseRank(Commit) = 3
  /\ ActualPhaseRank(Prepare) = 2
  /\ ActualPhaseRank(NewView) = 1
  /\ ActualHash(DeferredCommitOverPrepare) = 8
  /\ ActualHash(DeferredPrepareOverNewView) = 6
  /\ ActualHash(DeferredViewTieBreak) = 5
  /\ ActualHash(DeferredHashTieBreak) = 9
  /\ ActualHash(DeferredPreemptsMarker) = 2
  /\ ActualHash(MarkerOnly) = 5
  /\ ActualHash(MarkerViewTieBreak) = 4
  /\ ActualHash(MarkerHashTieBreak) = 8

PayloadHintRejectsIneligibleInputs ==
  /\ ActualSource(WrongHeightDeferredIgnored) = MarkerSource
  /\ ActualHash(WrongHeightDeferredIgnored) = 3
  /\ ActualSource(NonActionableDeferredIgnored) = MarkerSource
  /\ ActualHash(NonActionableDeferredIgnored) = 4
  /\ ActualSource(WrongHeightMarkerIgnored) = MarkerSource
  /\ ActualHash(WrongHeightMarkerIgnored) = 2
  /\ ActualSource(NonActionableMarkerIgnored) = MarkerSource
  /\ ActualHash(NonActionableMarkerIgnored) = 2
  /\ ActualSource(NoEligible) = NoneSource
  /\ ActualHash(NoEligible) = 0

ContiguousFrontierPayloadHintExactness ==
  /\ PayloadHintHasExactPositiveEvidence
  /\ PayloadHintPreservesDeterministicOrdering
  /\ PayloadHintRejectsIneligibleInputs

SafetyFast ==
  /\ PhaseRanksExact
  /\ SelectionExact
  /\ DeferredOrderingStable
  /\ DeferredEligibilityStable
  /\ DeferredPrecedenceStable
  /\ MarkerFallbackStable
  /\ MarkerEligibilityStable
  /\ EmptyFallbackStable
  /\ ContiguousFrontierPayloadHintExactness

PhaseRankAnchors ==
  /\ PhaseRanksExact
  /\ ActualPhaseRank(Commit) = 3
  /\ ActualPhaseRank(Prepare) = 2
  /\ ActualPhaseRank(NewView) = 1

SelectionAnchors ==
  /\ SelectionExact
  /\ \A c \in Cases:
       /\ ActualSource(c) = SpecSource(c)
       /\ ActualHash(c) = SpecHash(c)

DeferredOrderingAnchors ==
  /\ DeferredOrderingStable
  /\ ActualHash(DeferredCommitOverPrepare) = 8
  /\ ActualHash(DeferredPrepareOverNewView) = 6
  /\ ActualHash(DeferredViewTieBreak) = 5
  /\ ActualHash(DeferredHashTieBreak) = 9

DeferredEligibilityAnchors ==
  /\ DeferredEligibilityStable
  /\ ActualSource(WrongHeightDeferredIgnored) = MarkerSource
  /\ ActualHash(WrongHeightDeferredIgnored) = 3
  /\ ActualSource(NonActionableDeferredIgnored) = MarkerSource
  /\ ActualHash(NonActionableDeferredIgnored) = 4

DeferredPrecedenceAnchors ==
  /\ DeferredPrecedenceStable
  /\ ActualSource(DeferredPreemptsMarker) = DeferredSource
  /\ ActualHash(DeferredPreemptsMarker) = 2

MarkerFallbackAnchors ==
  /\ MarkerFallbackStable
  /\ ActualSource(MarkerOnly) = MarkerSource
  /\ ActualHash(MarkerOnly) = 5
  /\ ActualHash(MarkerViewTieBreak) = 4
  /\ ActualHash(MarkerHashTieBreak) = 8

MarkerEligibilityAnchors ==
  /\ MarkerEligibilityStable
  /\ ActualSource(WrongHeightMarkerIgnored) = MarkerSource
  /\ ActualHash(WrongHeightMarkerIgnored) = 2
  /\ ActualSource(NonActionableMarkerIgnored) = MarkerSource
  /\ ActualHash(NonActionableMarkerIgnored) = 2

EmptyFallbackAnchors ==
  /\ EmptyFallbackStable
  /\ ActualSource(NoEligible) = NoneSource
  /\ ActualHash(NoEligible) = 0

ContiguousFrontierPayloadHintSafetyAnchors ==
  /\ PhaseRankAnchors
  /\ SelectionAnchors
  /\ DeferredOrderingAnchors
  /\ DeferredEligibilityAnchors
  /\ DeferredPrecedenceAnchors
  /\ MarkerFallbackAnchors
  /\ MarkerEligibilityAnchors
  /\ EmptyFallbackAnchors

ContiguousFrontierPayloadHintCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ SafetyFast
  /\ ContiguousFrontierPayloadHintSafetyAnchors

Safety == ContiguousFrontierPayloadHintSafetyAnchors

====
