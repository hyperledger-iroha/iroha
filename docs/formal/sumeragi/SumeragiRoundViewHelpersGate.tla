---- MODULE SumeragiRoundViewHelpersGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for Sumeragi round/view helper semantics.

This slice pins:
`active_round_height(...)`,
`new_view_target(...)`,
`bump_view_after_quorum_timeout(...)`, and
`round_phase_after_event(...)`.

Heights and views use a finite `MaxU64` cap to model Rust's saturating
addition at `u64::MAX`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

MaxU64 == 4

Min(a, b) == IF a <= b THEN a ELSE b

Max(a, b) == IF a >= b THEN a ELSE b

SaturatingAddOne(v) == Min(v + 1, MaxU64)

\* @type: (Int, Int) => <<Int, Int>>;
Qc(h, v) == <<h, v>>

\* @type: (<<Int, Int>>, <<Int, Int>>) => Bool;
GeQc(a, b) ==
  \/ a[1] > b[1]
  \/ /\ a[1] = b[1]
     /\ a[2] >= b[2]

ActiveCases == {
  "active_none_mid",
  "active_none_max",
  "active_highest_only",
  "active_committed_only",
  "active_highest_newer_height",
  "active_committed_newer_height",
  "active_qc_height_max"
}

HasHighest(c) ==
  c \in {
    "active_highest_only",
    "active_highest_newer_height",
    "active_committed_newer_height",
    "active_qc_height_max"
  }

HasCommitted(c) ==
  c \in {
    "active_committed_only",
    "active_highest_newer_height",
    "active_committed_newer_height"
  }

CommittedHeight(c) ==
  CASE c = "active_none_max" -> MaxU64
    [] c = "active_highest_only" -> 1
    [] c = "active_committed_only" -> 1
    [] c = "active_qc_height_max" -> 1
    [] OTHER -> 2

Highest(c) ==
  CASE c = "active_highest_only" -> Qc(2, 0)
    [] c = "active_highest_newer_height" -> Qc(3, 0)
    [] c = "active_committed_newer_height" -> Qc(2, 4)
    [] c = "active_qc_height_max" -> Qc(MaxU64, 0)
    [] OTHER -> Qc(0, 0)

Committed(c) ==
  CASE c = "active_committed_only" -> Qc(3, 0)
    [] c = "active_highest_newer_height" -> Qc(2, 4)
    [] c = "active_committed_newer_height" -> Qc(3, 0)
    [] OTHER -> Qc(0, 0)

SpecActiveRoundHeight(c) ==
  IF HasHighest(c) /\ HasCommitted(c) THEN
    IF GeQc(Highest(c), Committed(c))
    THEN SaturatingAddOne(Highest(c)[1])
    ELSE SaturatingAddOne(Committed(c)[1])
  ELSE IF HasHighest(c) THEN
    SaturatingAddOne(Highest(c)[1])
  ELSE IF HasCommitted(c) THEN
    SaturatingAddOne(Committed(c)[1])
  ELSE
    SaturatingAddOne(CommittedHeight(c))

ActualActiveRoundHeight(c) ==
  CASE Bug = "active_none_returns_state"
       /\ c = "active_none_mid" -> CommittedHeight(c)
    [] Bug = "active_state_overflows"
       /\ c = "active_none_max" -> CommittedHeight(c) + 1
    [] Bug = "active_ignores_highest"
       /\ c = "active_highest_only" -> SaturatingAddOne(CommittedHeight(c))
    [] Bug = "active_ignores_committed"
       /\ c = "active_committed_only" -> SaturatingAddOne(CommittedHeight(c))
    [] Bug = "active_uses_lower_height"
       /\ c = "active_committed_newer_height" -> SaturatingAddOne(Highest(c)[1])
    [] Bug = "active_compares_view_before_height"
       /\ c = "active_highest_newer_height" -> SaturatingAddOne(Committed(c)[1])
    [] Bug = "active_qc_height_overflows"
       /\ c = "active_qc_height_max" -> Highest(c)[1] + 1
    [] OTHER -> SpecActiveRoundHeight(c)

TargetCases == {
  "target_default",
  "target_height_new",
  "target_height_same",
  "target_view_override",
  "target_height_max",
  "target_view_max_same_height"
}

TargetHighest(c) ==
  CASE c = "target_height_max" -> Qc(MaxU64, 1)
    [] c = "target_view_max_same_height" -> Qc(2, MaxU64)
    [] c = "target_view_override" -> Qc(2, 1)
    [] OTHER -> Qc(2, 1)

HasTargetHeight(c) ==
  c \in {
    "target_height_new",
    "target_height_same",
    "target_view_override",
    "target_view_max_same_height"
  }

TargetHeight(c) ==
  CASE c = "target_height_new" -> 4
    [] c = "target_height_same" -> 2
    [] c = "target_view_override" -> 4
    [] c = "target_view_max_same_height" -> 2
    [] OTHER -> 0

HasTargetView(c) ==
  c = "target_view_override"

TargetView(c) ==
  IF c = "target_view_override" THEN 3 ELSE 0

SpecNewViewTarget(c) ==
  LET height == IF HasTargetHeight(c)
                THEN TargetHeight(c)
                ELSE SaturatingAddOne(TargetHighest(c)[1]) IN
  LET view == IF HasTargetView(c)
              THEN TargetView(c)
              ELSE IF height > TargetHighest(c)[1]
                   THEN 0
                   ELSE SaturatingAddOne(TargetHighest(c)[2]) IN
    Qc(height, view)

ActualNewViewTarget(c) ==
  CASE Bug = "target_default_keeps_view"
       /\ c = "target_default" ->
       Qc(
         SaturatingAddOne(TargetHighest(c)[1]),
         SaturatingAddOne(TargetHighest(c)[2])
       )
    [] Bug = "target_ignores_height_override"
       /\ c = "target_height_new" ->
       Qc(SaturatingAddOne(TargetHighest(c)[1]), 0)
    [] Bug = "target_ignores_view_override"
       /\ c = "target_view_override" ->
       Qc(TargetHeight(c), 0)
    [] Bug = "target_equal_height_resets_view"
       /\ c = "target_height_same" ->
       Qc(TargetHeight(c), 0)
    [] Bug = "target_height_overflows"
       /\ c = "target_height_max" ->
       Qc(TargetHighest(c)[1] + 1, SaturatingAddOne(TargetHighest(c)[2]))
    [] Bug = "target_view_overflows"
       /\ c = "target_view_max_same_height" ->
       Qc(TargetHeight(c), TargetHighest(c)[2] + 1)
    [] OTHER -> SpecNewViewTarget(c)

BumpCases == {
  "bump_window_zero",
  "bump_window_retains_recent",
  "bump_window_drops_old",
  "bump_saturated"
}

BumpHeight(c) == 2

BumpView(c) ==
  CASE c = "bump_window_zero" -> 1
    [] c = "bump_window_retains_recent" -> 1
    [] c = "bump_window_drops_old" -> 3
    [] c = "bump_saturated" -> MaxU64
    [] OTHER -> 0

BumpWindow(c) ==
  CASE c = "bump_window_zero" -> 0
    [] c = "bump_window_retains_recent" -> 2
    [] c = "bump_window_drops_old" -> 1
    [] c = "bump_saturated" -> 0
    [] OTHER -> 0

\* @type: (Str) => Set(<<Int, Int>>);
BumpEntries(c) ==
  CASE c = "bump_window_zero" ->
       {Qc(2, 0), Qc(2, 1), Qc(2, 2), Qc(3, 0)}
    [] c = "bump_window_retains_recent" ->
       {Qc(2, 0), Qc(2, 1), Qc(2, 2), Qc(3, 0)}
    [] c = "bump_window_drops_old" ->
       {Qc(2, 2), Qc(2, 3), Qc(2, 4), Qc(3, 0)}
    [] c = "bump_saturated" ->
       {Qc(2, 3), Qc(2, 4)}
    [] OTHER -> {}

BumpNextView(c) == SaturatingAddOne(BumpView(c))

BumpMinView(c) ==
  IF BumpWindow(c) = 0 THEN
    BumpNextView(c)
  ELSE
    Max(BumpNextView(c) - BumpWindow(c), 0)

\* @type: (Set(<<Int, Int>>), Int, Int) => Set(<<Int, Int>>);
DropWithMin(entries, height, minView) ==
  {e \in entries: e[1] # height \/ e[2] >= minView}

\* @type: (Set(<<Int, Int>>), Int, Int) => Set(<<Int, Int>>);
RemoveCurrent(entries, height, view) ==
  entries \ {Qc(height, view)}

SpecBumpRetained(c) ==
  DropWithMin(
    RemoveCurrent(BumpEntries(c), BumpHeight(c), BumpView(c)),
    BumpHeight(c),
    BumpMinView(c)
  )

SpecBump(c) ==
  [nextView |-> BumpNextView(c),
   minView |-> BumpMinView(c),
   phaseHeight |-> BumpHeight(c),
   phaseView |-> BumpNextView(c),
   pacemakerReset |-> TRUE,
   retained |-> SpecBumpRetained(c)]

ActualBump(c) ==
  CASE Bug = "bump_next_view_overflows"
       /\ c = "bump_saturated" ->
       [SpecBump(c) EXCEPT !.nextView = BumpView(c) + 1,
                           !.phaseView = BumpView(c) + 1]
    [] Bug = "bump_skips_remove_current"
       /\ c = "bump_window_retains_recent" ->
       [SpecBump(c) EXCEPT !.retained =
         DropWithMin(BumpEntries(c), BumpHeight(c), BumpMinView(c))]
    [] Bug = "bump_window_zero_retains_old"
       /\ c = "bump_window_zero" ->
       [SpecBump(c) EXCEPT !.minView = 0,
                           !.retained =
                             DropWithMin(
                               RemoveCurrent(BumpEntries(c), BumpHeight(c), BumpView(c)),
                               BumpHeight(c),
                               0
                             )]
    [] Bug = "bump_window_uses_next_as_min"
       /\ c = "bump_window_retains_recent" ->
       [SpecBump(c) EXCEPT !.minView = BumpNextView(c),
                           !.retained =
                             DropWithMin(
                               RemoveCurrent(BumpEntries(c), BumpHeight(c), BumpView(c)),
                               BumpHeight(c),
                               BumpNextView(c)
                             )]
    [] Bug = "bump_drops_other_heights"
       /\ c = "bump_window_drops_old" ->
       [SpecBump(c) EXCEPT !.retained =
         {e \in RemoveCurrent(BumpEntries(c), BumpHeight(c), BumpView(c)):
           e[2] >= BumpMinView(c)}]
    [] Bug = "bump_does_not_reset_pacemaker"
       /\ c = "bump_window_retains_recent" ->
       [SpecBump(c) EXCEPT !.pacemakerReset = FALSE]
    [] Bug = "bump_keeps_phase_view"
       /\ c = "bump_window_retains_recent" ->
       [SpecBump(c) EXCEPT !.phaseView = BumpView(c)]
    [] OTHER -> SpecBump(c)

RoundPhaseCases == {
  "phase_advance_view",
  "phase_wait_proposal",
  "phase_wait_block",
  "phase_wait_validation_pending",
  "phase_wait_validation_invalid",
  "phase_wait_da",
  "phase_commit_ready",
  "phase_wait_prepare_qc",
  "phase_wait_commit_qc_local",
  "phase_wait_commit_qc_observed"
}

ProposalObserved(c) ==
  c \notin {"phase_wait_proposal"}

BlockAvailable(c) ==
  c \notin {"phase_wait_proposal", "phase_wait_block"}

ValidationStatus(c) ==
  CASE c = "phase_wait_validation_invalid" -> "invalid"
    [] c = "phase_wait_validation_pending" -> "pending"
    [] OTHER -> "valid"

DaWaiting(c) ==
  c = "phase_wait_da"

CommitReady(c) ==
  c \in {"phase_wait_da", "phase_commit_ready"}

AdvanceView(c) ==
  c = "phase_advance_view"

CommitStage(c) ==
  CASE c = "phase_wait_commit_qc_local" -> "local"
    [] c = "phase_wait_commit_qc_observed" -> "observed"
    [] OTHER -> "awaiting"

SpecRoundPhase(c) ==
  IF AdvanceView(c) THEN
    "AdvanceView"
  ELSE IF ~ProposalObserved(c) THEN
    "WaitProposal"
  ELSE IF ~BlockAvailable(c) THEN
    "WaitBlock"
  ELSE IF ValidationStatus(c) # "valid" THEN
    "WaitValidation"
  ELSE IF DaWaiting(c) THEN
    "WaitDa"
  ELSE IF CommitReady(c) THEN
    "Commit"
  ELSE IF CommitStage(c) = "awaiting" THEN
    "WaitPrepareQc"
  ELSE
    "WaitCommitQc"

ActualRoundPhase(c) ==
  CASE Bug = "phase_advance_not_priority"
       /\ c = "phase_advance_view" -> "WaitProposal"
    [] Bug = "phase_skips_wait_proposal"
       /\ c = "phase_wait_proposal" -> "WaitBlock"
    [] Bug = "phase_skips_wait_block"
       /\ c = "phase_wait_block" -> "WaitValidation"
    [] Bug = "phase_treats_invalid_as_valid"
       /\ c = "phase_wait_validation_invalid" -> "WaitPrepareQc"
    [] Bug = "phase_commit_before_validation"
       /\ c = "phase_wait_validation_pending" -> "Commit"
    [] Bug = "phase_commit_before_da"
       /\ c = "phase_wait_da" -> "Commit"
    [] Bug = "phase_local_vote_waits_prepare"
       /\ c = "phase_wait_commit_qc_local" -> "WaitPrepareQc"
    [] Bug = "phase_observed_qc_commits"
       /\ c = "phase_wait_commit_qc_observed" -> "Commit"
    [] OTHER -> SpecRoundPhase(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "active_none_returns_state",
       "active_state_overflows",
       "active_ignores_highest",
       "active_ignores_committed",
       "active_uses_lower_height",
       "active_compares_view_before_height",
       "active_qc_height_overflows",
       "target_default_keeps_view",
       "target_ignores_height_override",
       "target_ignores_view_override",
       "target_equal_height_resets_view",
       "target_height_overflows",
       "target_view_overflows",
       "bump_next_view_overflows",
       "bump_skips_remove_current",
       "bump_window_zero_retains_old",
       "bump_window_uses_next_as_min",
       "bump_drops_other_heights",
       "bump_does_not_reset_pacemaker",
       "bump_keeps_phase_view",
       "phase_advance_not_priority",
       "phase_skips_wait_proposal",
       "phase_skips_wait_block",
       "phase_treats_invalid_as_valid",
       "phase_commit_before_validation",
       "phase_commit_before_da",
       "phase_local_vote_waits_prepare",
       "phase_observed_qc_commits"
     }
  /\ checked = 0

ActiveRoundHeightMatchesSpec ==
  /\ \A c \in ActiveCases:
       ActualActiveRoundHeight(c) = SpecActiveRoundHeight(c)

NewViewTargetMatchesSpec ==
  /\ \A c \in TargetCases:
       ActualNewViewTarget(c) = SpecNewViewTarget(c)

BumpViewMatchesSpec ==
  /\ \A c \in BumpCases:
       ActualBump(c) = SpecBump(c)

RoundPhaseMatchesSpec ==
  /\ \A c \in RoundPhaseCases:
       ActualRoundPhase(c) = SpecRoundPhase(c)

ActiveRoundHeightBounds ==
  \A c \in ActiveCases:
    /\ ActualActiveRoundHeight(c) >= 0
    /\ ActualActiveRoundHeight(c) <= MaxU64

NewViewTargetBounds ==
  \A c \in TargetCases:
    /\ ActualNewViewTarget(c)[1] >= 0
    /\ ActualNewViewTarget(c)[1] <= MaxU64
    /\ ActualNewViewTarget(c)[2] >= 0
    /\ ActualNewViewTarget(c)[2] <= MaxU64

BumpViewResetsRoundAndPacemaker ==
  \A c \in BumpCases:
    /\ ActualBump(c).phaseHeight = BumpHeight(c)
    /\ ActualBump(c).phaseView = ActualBump(c).nextView
    /\ ActualBump(c).pacemakerReset = TRUE

BumpRetainedWindowMatchesSpec ==
  \A c \in BumpCases:
    /\ Qc(BumpHeight(c), BumpView(c)) \notin ActualBump(c).retained
    /\ \A entry \in ActualBump(c).retained:
         entry[1] # BumpHeight(c) \/ entry[2] >= ActualBump(c).minView

ActiveRoundHeightAnchors ==
  /\ SpecActiveRoundHeight("active_none_mid") = 3
  /\ SpecActiveRoundHeight("active_none_max") = MaxU64
  /\ SpecActiveRoundHeight("active_highest_only") = 3
  /\ SpecActiveRoundHeight("active_committed_only") = 4
  /\ SpecActiveRoundHeight("active_highest_newer_height") = 4
  /\ SpecActiveRoundHeight("active_committed_newer_height") = 4
  /\ SpecActiveRoundHeight("active_qc_height_max") = MaxU64

NewViewTargetAnchors ==
  /\ SpecNewViewTarget("target_default") = Qc(3, 0)
  /\ SpecNewViewTarget("target_height_new") = Qc(4, 0)
  /\ SpecNewViewTarget("target_height_same") = Qc(2, 2)
  /\ SpecNewViewTarget("target_view_override") = Qc(4, 3)
  /\ SpecNewViewTarget("target_height_max") = Qc(MaxU64, 2)
  /\ SpecNewViewTarget("target_view_max_same_height") = Qc(2, MaxU64)

BumpViewAnchors ==
  /\ SpecBump("bump_window_zero") =
       [nextView |-> 2,
        minView |-> 2,
        phaseHeight |-> 2,
        phaseView |-> 2,
        pacemakerReset |-> TRUE,
        retained |-> {Qc(2, 2), Qc(3, 0)}]
  /\ SpecBump("bump_window_retains_recent") =
       [nextView |-> 2,
        minView |-> 0,
        phaseHeight |-> 2,
        phaseView |-> 2,
        pacemakerReset |-> TRUE,
        retained |-> {Qc(2, 0), Qc(2, 2), Qc(3, 0)}]
  /\ SpecBump("bump_window_drops_old") =
       [nextView |-> MaxU64,
        minView |-> 3,
        phaseHeight |-> 2,
        phaseView |-> MaxU64,
        pacemakerReset |-> TRUE,
        retained |-> {Qc(2, MaxU64), Qc(3, 0)}]
  /\ SpecBump("bump_saturated") =
       [nextView |-> MaxU64,
        minView |-> MaxU64,
        phaseHeight |-> 2,
        phaseView |-> MaxU64,
        pacemakerReset |-> TRUE,
        retained |-> {}]

RoundPhaseAnchors ==
  /\ SpecRoundPhase("phase_advance_view") = "AdvanceView"
  /\ SpecRoundPhase("phase_wait_proposal") = "WaitProposal"
  /\ SpecRoundPhase("phase_wait_block") = "WaitBlock"
  /\ SpecRoundPhase("phase_wait_validation_pending") = "WaitValidation"
  /\ SpecRoundPhase("phase_wait_validation_invalid") = "WaitValidation"
  /\ SpecRoundPhase("phase_wait_da") = "WaitDa"
  /\ SpecRoundPhase("phase_commit_ready") = "Commit"
  /\ SpecRoundPhase("phase_wait_prepare_qc") = "WaitPrepareQc"
  /\ SpecRoundPhase("phase_wait_commit_qc_local") = "WaitCommitQc"
  /\ SpecRoundPhase("phase_wait_commit_qc_observed") = "WaitCommitQc"

RoundViewActiveHeightExact ==
  /\ ActiveRoundHeightMatchesSpec
  /\ ActiveRoundHeightBounds
  /\ ActiveRoundHeightAnchors

RoundViewTargetExact ==
  /\ NewViewTargetMatchesSpec
  /\ NewViewTargetBounds
  /\ NewViewTargetAnchors

RoundViewBumpExact ==
  /\ BumpViewMatchesSpec
  /\ BumpViewResetsRoundAndPacemaker
  /\ BumpRetainedWindowMatchesSpec
  /\ BumpViewAnchors

RoundViewPhaseExact ==
  /\ RoundPhaseMatchesSpec
  /\ RoundPhaseAnchors

RoundViewHelpersExactness ==
  /\ RoundViewActiveHeightExact
  /\ RoundViewTargetExact
  /\ RoundViewBumpExact
  /\ RoundViewPhaseExact

SafetyFast ==
  RoundViewHelpersExactness

BugActiveNoneReturnsState ==
  ActualActiveRoundHeight("active_none_mid") =
    SpecActiveRoundHeight("active_none_mid")

BugActiveStateOverflows ==
  ActualActiveRoundHeight("active_none_max") =
    SpecActiveRoundHeight("active_none_max")

BugActiveIgnoresHighest ==
  ActualActiveRoundHeight("active_highest_only") =
    SpecActiveRoundHeight("active_highest_only")

BugActiveIgnoresCommitted ==
  ActualActiveRoundHeight("active_committed_only") =
    SpecActiveRoundHeight("active_committed_only")

BugActiveUsesLowerHeight ==
  ActualActiveRoundHeight("active_committed_newer_height") =
    SpecActiveRoundHeight("active_committed_newer_height")

BugActiveComparesViewBeforeHeight ==
  ActualActiveRoundHeight("active_highest_newer_height") =
    SpecActiveRoundHeight("active_highest_newer_height")

BugActiveQcHeightOverflows ==
  ActualActiveRoundHeight("active_qc_height_max") =
    SpecActiveRoundHeight("active_qc_height_max")

BugTargetDefaultKeepsView ==
  ActualNewViewTarget("target_default") = SpecNewViewTarget("target_default")

BugTargetIgnoresHeightOverride ==
  ActualNewViewTarget("target_height_new") =
    SpecNewViewTarget("target_height_new")

BugTargetIgnoresViewOverride ==
  ActualNewViewTarget("target_view_override") =
    SpecNewViewTarget("target_view_override")

BugTargetEqualHeightResetsView ==
  ActualNewViewTarget("target_height_same") =
    SpecNewViewTarget("target_height_same")

BugTargetHeightOverflows ==
  ActualNewViewTarget("target_height_max") = SpecNewViewTarget("target_height_max")

BugTargetViewOverflows ==
  ActualNewViewTarget("target_view_max_same_height") =
    SpecNewViewTarget("target_view_max_same_height")

BugBumpNextViewOverflows ==
  ActualBump("bump_saturated") = SpecBump("bump_saturated")

BugBumpSkipsRemoveCurrent ==
  ActualBump("bump_window_retains_recent") =
    SpecBump("bump_window_retains_recent")

BugBumpWindowZeroRetainsOld ==
  ActualBump("bump_window_zero") = SpecBump("bump_window_zero")

BugBumpWindowUsesNextAsMin ==
  ActualBump("bump_window_retains_recent") =
    SpecBump("bump_window_retains_recent")

BugBumpDropsOtherHeights ==
  ActualBump("bump_window_drops_old") = SpecBump("bump_window_drops_old")

BugBumpDoesNotResetPacemaker ==
  ActualBump("bump_window_retains_recent") =
    SpecBump("bump_window_retains_recent")

BugBumpKeepsPhaseView ==
  ActualBump("bump_window_retains_recent") =
    SpecBump("bump_window_retains_recent")

BugPhaseAdvanceNotPriority ==
  ActualRoundPhase("phase_advance_view") = SpecRoundPhase("phase_advance_view")

BugPhaseSkipsWaitProposal ==
  ActualRoundPhase("phase_wait_proposal") = SpecRoundPhase("phase_wait_proposal")

BugPhaseSkipsWaitBlock ==
  ActualRoundPhase("phase_wait_block") = SpecRoundPhase("phase_wait_block")

BugPhaseTreatsInvalidAsValid ==
  ActualRoundPhase("phase_wait_validation_invalid") =
    SpecRoundPhase("phase_wait_validation_invalid")

BugPhaseCommitBeforeValidation ==
  ActualRoundPhase("phase_wait_validation_pending") =
    SpecRoundPhase("phase_wait_validation_pending")

BugPhaseCommitBeforeDa ==
  ActualRoundPhase("phase_wait_da") = SpecRoundPhase("phase_wait_da")

BugPhaseLocalVoteWaitsPrepare ==
  ActualRoundPhase("phase_wait_commit_qc_local") =
    SpecRoundPhase("phase_wait_commit_qc_local")

BugPhaseObservedQcCommits ==
  ActualRoundPhase("phase_wait_commit_qc_observed") =
    SpecRoundPhase("phase_wait_commit_qc_observed")

====
