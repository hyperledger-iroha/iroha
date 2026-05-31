---- MODULE SumeragiMissingQcStallRangePullGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the same-height missing-QC stall range-pull
emission branch in `request_range_pull_from_anchor_with_tier(...)`.

The branch is active only for missing-QC stall reanchor reasons and only when
the stall snapshot is active for both the canonical height and active consensus
round. It suppresses repeated emissions in the same stalled window, lets the
recovery FSM veto same-height missing-QC reanchors, deterministically rotates
through sorted/deduplicated two-peer cohorts with every third emitted window
falling back to all peers, applies the stall-window cooldown, and marks the
missing-QC stall range-pull window only after a successful send.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

IdleReason == "idle_reason"
QcFastReason == "qc_fast_reason"
FutureReason == "future_reason"
LockLagReason == "lock_lag_reason"
HardCapReason == "hard_cap_reason"
CommitConflictReason == "commit_conflict_reason"
FrontierGapReason == "frontier_gap_reason"

ReasonCases == {
  IdleReason,
  QcFastReason,
  FutureReason,
  LockLagReason,
  HardCapReason,
  CommitConflictReason,
  FrontierGapReason
}

InactiveStall == "inactive_stall"
CanonicalHeightMismatch == "canonical_height_mismatch"
ActiveRoundMismatch == "active_round_mismatch"
AlreadyEmittedWindow == "already_emitted_window"
RecoveryFsmBlocks == "recovery_fsm_blocks"
EmptyTargets == "empty_targets"
Window0Cohort == "window_0_cohort"
Window1Cohort == "window_1_cohort"
Window2AllPeers == "window_2_all_peers"
SmallPeerAllPeers == "small_peer_all_peers"
DuplicateTargetsDeduped == "duplicate_targets_deduped"
CooldownDuplicate == "cooldown_duplicate"
CooldownBoundary == "cooldown_boundary"
StallCooldown == "stall_cooldown"
SuccessfulMarksWindow == "successful_marks_window"

EmissionCases == {
  InactiveStall,
  CanonicalHeightMismatch,
  ActiveRoundMismatch,
  AlreadyEmittedWindow,
  RecoveryFsmBlocks,
  EmptyTargets,
  Window0Cohort,
  Window1Cohort,
  Window2AllPeers,
  SmallPeerAllPeers,
  DuplicateTargetsDeduped,
  CooldownDuplicate,
  CooldownBoundary,
  StallCooldown,
  SuccessfulMarksWindow
}

Cases == ReasonCases \cup EmissionCases

ReasonAccepted == 1
ReasonIgnored == 2
StallMode == 3
NoStallMode == 4
Send == 5
Suppress == 6
Cohort12 == 7
Cohort23 == 8
AllPeers == 9
SortedDeduped == 10
AlreadyEmittedChecked == 11
RecoveryFsmChecked == 12
RecoveryFsmAllows == 13
RecoveryFsmRejects == 14
MarkWindow == 15
NoMark == 16
StallCooldownApplied == 17
BaseCooldownOnly == 18
DedupChecked == 19
DedupBlocked == 20
DedupAllows == 21
HeightMatched == 22
HeightMismatched == 23
ActiveRoundMatched == 24
ActiveRoundMismatched == 25
TargetsNonEmpty == 26
TargetsEmpty == 27

ActionUniverse == 1..27

AcceptedReasons == {
  IdleReason,
  QcFastReason,
  FutureReason,
  LockLagReason,
  HardCapReason,
  CommitConflictReason
}

BaseSendActions ==
  {StallMode, Send, SortedDeduped, AlreadyEmittedChecked,
   RecoveryFsmChecked, RecoveryFsmAllows, MarkWindow,
   StallCooldownApplied, DedupChecked, DedupAllows, HeightMatched,
   ActiveRoundMatched, TargetsNonEmpty}

SuppressedActions ==
  {StallMode, Suppress, NoMark, StallCooldownApplied}

SpecActions(c) ==
  CASE c \in AcceptedReasons ->
      {ReasonAccepted, StallMode}
    [] c = FrontierGapReason ->
      {ReasonIgnored, NoStallMode, NoMark, BaseCooldownOnly}
    [] c = InactiveStall ->
      {NoStallMode, Send, NoMark, BaseCooldownOnly, HeightMatched,
       ActiveRoundMatched, TargetsNonEmpty}
    [] c = CanonicalHeightMismatch ->
      {NoStallMode, Send, NoMark, BaseCooldownOnly, HeightMismatched,
       ActiveRoundMatched, TargetsNonEmpty}
    [] c = ActiveRoundMismatch ->
      {NoStallMode, Send, NoMark, BaseCooldownOnly, HeightMatched,
       ActiveRoundMismatched, TargetsNonEmpty}
    [] c = AlreadyEmittedWindow ->
      SuppressedActions \cup {AlreadyEmittedChecked, HeightMatched,
        ActiveRoundMatched, TargetsNonEmpty}
    [] c = RecoveryFsmBlocks ->
      SuppressedActions \cup {AlreadyEmittedChecked, RecoveryFsmChecked,
        RecoveryFsmRejects, HeightMatched, ActiveRoundMatched, TargetsNonEmpty}
    [] c = EmptyTargets ->
      SuppressedActions \cup {AlreadyEmittedChecked, RecoveryFsmChecked,
        RecoveryFsmAllows, HeightMatched, ActiveRoundMatched, TargetsEmpty}
    [] c = Window0Cohort ->
      BaseSendActions \cup {Cohort12}
    [] c = Window1Cohort ->
      BaseSendActions \cup {Cohort23}
    [] c = Window2AllPeers ->
      BaseSendActions \cup {AllPeers}
    [] c = SmallPeerAllPeers ->
      BaseSendActions \cup {AllPeers}
    [] c = DuplicateTargetsDeduped ->
      BaseSendActions \cup {Cohort12}
    [] c = CooldownDuplicate ->
      SuppressedActions \cup {AlreadyEmittedChecked, RecoveryFsmChecked,
        RecoveryFsmAllows, DedupChecked, DedupBlocked, HeightMatched,
        ActiveRoundMatched, TargetsNonEmpty}
    [] c = CooldownBoundary ->
      BaseSendActions \cup {Cohort12}
    [] c = StallCooldown ->
      BaseSendActions \cup {Cohort12}
    [] c = SuccessfulMarksWindow ->
      BaseSendActions \cup {Cohort12}

ImplementationActions(c) ==
  CASE Bug = "reject_idle_reason"
       /\ c = IdleReason ->
      {ReasonIgnored, NoStallMode, NoMark, BaseCooldownOnly}
    [] Bug = "reject_qc_fast_reason"
       /\ c = QcFastReason ->
      {ReasonIgnored, NoStallMode, NoMark, BaseCooldownOnly}
    [] Bug = "reject_future_reason"
       /\ c = FutureReason ->
      {ReasonIgnored, NoStallMode, NoMark, BaseCooldownOnly}
    [] Bug = "accept_frontier_gap_reason"
       /\ c = FrontierGapReason ->
      {ReasonAccepted, StallMode}
    [] Bug = "activate_inactive_stall"
       /\ c = InactiveStall ->
      BaseSendActions \cup {Cohort12}
    [] Bug = "activate_canonical_mismatch"
       /\ c = CanonicalHeightMismatch ->
      BaseSendActions \cup {Cohort12, HeightMismatched}
    [] Bug = "activate_active_round_mismatch"
       /\ c = ActiveRoundMismatch ->
      BaseSendActions \cup {Cohort12, ActiveRoundMismatched}
    [] Bug = "ignore_already_emitted_window"
       /\ c = AlreadyEmittedWindow ->
      BaseSendActions \cup {Cohort12}
    [] Bug = "skip_already_emitted_check"
       /\ c = AlreadyEmittedWindow ->
      (SpecActions(c) \ {AlreadyEmittedChecked})
    [] Bug = "ignore_recovery_fsm_block"
       /\ c = RecoveryFsmBlocks ->
      BaseSendActions \cup {Cohort12}
    [] Bug = "skip_recovery_fsm_check"
       /\ c = RecoveryFsmBlocks ->
      (SpecActions(c) \ {RecoveryFsmChecked})
    [] Bug = "empty_targets_sends"
       /\ c = EmptyTargets ->
      BaseSendActions \cup {Cohort12, TargetsEmpty}
    [] Bug = "window0_uses_all_peers"
       /\ c = Window0Cohort ->
      BaseSendActions \cup {AllPeers}
    [] Bug = "window1_wrong_cohort"
       /\ c = Window1Cohort ->
      BaseSendActions \cup {Cohort12}
    [] Bug = "window2_not_all_peers"
       /\ c = Window2AllPeers ->
      BaseSendActions \cup {Cohort12}
    [] Bug = "small_peer_uses_cohort"
       /\ c = SmallPeerAllPeers ->
      BaseSendActions \cup {Cohort12}
    [] Bug = "skip_sort_dedup"
       /\ c = DuplicateTargetsDeduped ->
      (BaseSendActions \ {SortedDeduped}) \cup {Cohort12}
    [] Bug = "dedup_duplicate_sends"
       /\ c = CooldownDuplicate ->
      BaseSendActions \cup {Cohort12, DedupBlocked}
    [] Bug = "dedup_boundary_blocks"
       /\ c = CooldownBoundary ->
      SuppressedActions \cup {AlreadyEmittedChecked, RecoveryFsmChecked,
        RecoveryFsmAllows, DedupChecked, DedupBlocked, HeightMatched,
        ActiveRoundMatched, TargetsNonEmpty}
    [] Bug = "skip_stall_cooldown"
       /\ c = StallCooldown ->
      (BaseSendActions \ {StallCooldownApplied}) \cup {Cohort12, BaseCooldownOnly}
    [] Bug = "skip_mark_on_send"
       /\ c = SuccessfulMarksWindow ->
      (BaseSendActions \ {MarkWindow}) \cup {Cohort12, NoMark}
    [] Bug = "mark_without_send"
       /\ c = CooldownDuplicate ->
      SpecActions(c) \cup {MarkWindow}
    [] Bug = "mark_nonstall"
       /\ c = InactiveStall ->
      (SpecActions(c) \ {NoMark}) \cup {MarkWindow}
    [] OTHER -> SpecActions(c)

Bugs == {
  "none",
  "reject_idle_reason",
  "reject_qc_fast_reason",
  "reject_future_reason",
  "accept_frontier_gap_reason",
  "activate_inactive_stall",
  "activate_canonical_mismatch",
  "activate_active_round_mismatch",
  "ignore_already_emitted_window",
  "skip_already_emitted_check",
  "ignore_recovery_fsm_block",
  "skip_recovery_fsm_check",
  "empty_targets_sends",
  "window0_uses_all_peers",
  "window1_wrong_cohort",
  "window2_not_all_peers",
  "small_peer_uses_cohort",
  "skip_sort_dedup",
  "dedup_duplicate_sends",
  "dedup_boundary_blocks",
  "skip_stall_cooldown",
  "skip_mark_on_send",
  "mark_without_send",
  "mark_nonstall"
}

Init ==
  checked = 0

Next ==
  \/ /\ checked < 23
     /\ checked' = checked + 1
  \/ /\ checked = 23
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..23
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

ReasonClassifierSafety ==
  /\ ReasonAccepted \in ImplementationActions(IdleReason)
  /\ ReasonAccepted \in ImplementationActions(QcFastReason)
  /\ ReasonAccepted \in ImplementationActions(FutureReason)
  /\ ReasonAccepted \in ImplementationActions(LockLagReason)
  /\ ReasonAccepted \in ImplementationActions(HardCapReason)
  /\ ReasonAccepted \in ImplementationActions(CommitConflictReason)
  /\ ReasonIgnored \in ImplementationActions(FrontierGapReason)

ExactStallGateSafety ==
  /\ NoStallMode \in ImplementationActions(InactiveStall)
  /\ NoStallMode \in ImplementationActions(CanonicalHeightMismatch)
  /\ NoStallMode \in ImplementationActions(ActiveRoundMismatch)
  /\ StallMode \in ImplementationActions(Window0Cohort)

SuppressionSafety ==
  /\ Suppress \in ImplementationActions(AlreadyEmittedWindow)
  /\ AlreadyEmittedChecked \in ImplementationActions(AlreadyEmittedWindow)
  /\ Suppress \in ImplementationActions(RecoveryFsmBlocks)
  /\ RecoveryFsmChecked \in ImplementationActions(RecoveryFsmBlocks)
  /\ Suppress \in ImplementationActions(EmptyTargets)
  /\ TargetsEmpty \in ImplementationActions(EmptyTargets)

CohortSafety ==
  /\ Cohort12 \in ImplementationActions(Window0Cohort)
  /\ Cohort23 \in ImplementationActions(Window1Cohort)
  /\ AllPeers \in ImplementationActions(Window2AllPeers)
  /\ AllPeers \in ImplementationActions(SmallPeerAllPeers)
  /\ SortedDeduped \in ImplementationActions(DuplicateTargetsDeduped)

CooldownAndMarkSafety ==
  /\ Suppress \in ImplementationActions(CooldownDuplicate)
  /\ DedupBlocked \in ImplementationActions(CooldownDuplicate)
  /\ Send \in ImplementationActions(CooldownBoundary)
  /\ StallCooldownApplied \in ImplementationActions(StallCooldown)
  /\ MarkWindow \in ImplementationActions(SuccessfulMarksWindow)
  /\ NoMark \in ImplementationActions(CooldownDuplicate)
  /\ NoMark \in ImplementationActions(InactiveStall)

SafetyFast ==
  /\ ActionsMatchSpec
  /\ ReasonClassifierSafety
  /\ ExactStallGateSafety
  /\ SuppressionSafety
  /\ CohortSafety
  /\ CooldownAndMarkSafety

ActionComparisonAnchors ==
  ActionsMatchSpec

ReasonClassifierAnchors ==
  /\ ReasonClassifierSafety
  /\ ReasonAccepted \in ImplementationActions(IdleReason)
  /\ ReasonAccepted \in ImplementationActions(QcFastReason)
  /\ ReasonAccepted \in ImplementationActions(FutureReason)
  /\ ReasonAccepted \in ImplementationActions(LockLagReason)
  /\ ReasonAccepted \in ImplementationActions(HardCapReason)
  /\ ReasonAccepted \in ImplementationActions(CommitConflictReason)
  /\ ReasonIgnored \in ImplementationActions(FrontierGapReason)

ExactStallGateAnchors ==
  /\ ExactStallGateSafety
  /\ NoStallMode \in ImplementationActions(InactiveStall)
  /\ NoStallMode \in ImplementationActions(CanonicalHeightMismatch)
  /\ NoStallMode \in ImplementationActions(ActiveRoundMismatch)
  /\ StallMode \in ImplementationActions(Window0Cohort)

SuppressionAnchors ==
  /\ SuppressionSafety
  /\ Suppress \in ImplementationActions(AlreadyEmittedWindow)
  /\ AlreadyEmittedChecked \in ImplementationActions(AlreadyEmittedWindow)
  /\ Suppress \in ImplementationActions(RecoveryFsmBlocks)
  /\ RecoveryFsmChecked \in ImplementationActions(RecoveryFsmBlocks)
  /\ Suppress \in ImplementationActions(EmptyTargets)
  /\ TargetsEmpty \in ImplementationActions(EmptyTargets)

CohortAnchors ==
  /\ CohortSafety
  /\ Cohort12 \in ImplementationActions(Window0Cohort)
  /\ Cohort23 \in ImplementationActions(Window1Cohort)
  /\ AllPeers \in ImplementationActions(Window2AllPeers)
  /\ AllPeers \in ImplementationActions(SmallPeerAllPeers)
  /\ SortedDeduped \in ImplementationActions(DuplicateTargetsDeduped)

CooldownAndMarkAnchors ==
  /\ CooldownAndMarkSafety
  /\ Suppress \in ImplementationActions(CooldownDuplicate)
  /\ DedupBlocked \in ImplementationActions(CooldownDuplicate)
  /\ Send \in ImplementationActions(CooldownBoundary)
  /\ StallCooldownApplied \in ImplementationActions(StallCooldown)
  /\ MarkWindow \in ImplementationActions(SuccessfulMarksWindow)
  /\ NoMark \in ImplementationActions(CooldownDuplicate)
  /\ NoMark \in ImplementationActions(InactiveStall)

MissingQcStallRangePullSafetyAnchors ==
  /\ ActionComparisonAnchors
  /\ ReasonClassifierAnchors
  /\ ExactStallGateAnchors
  /\ SuppressionAnchors
  /\ CohortAnchors
  /\ CooldownAndMarkAnchors

Safety ==
  MissingQcStallRangePullSafetyAnchors

====
