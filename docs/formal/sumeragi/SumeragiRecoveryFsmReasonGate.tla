---- MODULE SumeragiRecoveryFsmReasonGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi recovery-FSM reason helpers.

This slice pins `RecoveryFsmReason::from_reason(...)`,
`RecoveryFsmReason::rank(...)`, and the stable recovery-event sort key from
`main_loop.rs`. The larger recovery gates prove when events are emitted; this
companion gate fixes the label-to-reason classifier and the deterministic
height/rank/peer ordering used before status transition accounting.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

FrontierGapRealign == "FrontierGapRealign"
FrontierStallReset == "FrontierStallReset"
MissingBlockHeightHardCap == "MissingBlockHeightHardCap"
IdleMissingQcReacquire == "IdleMissingQcReacquire"
LockLagHighestQcDefer == "LockLagHighestQcDefer"
FutureNewViewFrontierReanchor == "FutureNewViewFrontierReanchor"
LockLagFuturePrune == "LockLagFuturePrune"
SidecarMismatch == "SidecarMismatch"
HighestQcCommittedConflict == "HighestQcCommittedConflict"
Other == "Other"

ReasonCases == {
  FrontierGapRealign,
  FrontierStallReset,
  MissingBlockHeightHardCap,
  IdleMissingQcReacquire,
  LockLagHighestQcDefer,
  FutureNewViewFrontierReanchor,
  LockLagFuturePrune,
  SidecarMismatch,
  HighestQcCommittedConflict,
  Other
}

KnownReasonLabels == {
  "frontier_gap_realign",
  "frontier_stall_reset",
  "missing_block_height_hard_cap",
  "idle_missing_qc_reacquire",
  "lock_lag_highest_qc_defer",
  "future_new_view_frontier_reanchor",
  "lock_lag_future_prune",
  "sidecar_mismatch",
  "highest_qc_committed_conflict"
}

UnknownReasonLabels == {"unknown_reason", "range_pull_recovery"}
ReasonInputCases == KnownReasonLabels \cup UnknownReasonLabels

SpecFromReason(label) ==
  CASE label = "frontier_gap_realign" -> FrontierGapRealign
    [] label = "frontier_stall_reset" -> FrontierStallReset
    [] label = "missing_block_height_hard_cap" -> MissingBlockHeightHardCap
    [] label = "idle_missing_qc_reacquire" -> IdleMissingQcReacquire
    [] label = "lock_lag_highest_qc_defer" -> LockLagHighestQcDefer
    [] label = "future_new_view_frontier_reanchor" -> FutureNewViewFrontierReanchor
    [] label = "lock_lag_future_prune" -> LockLagFuturePrune
    [] label = "sidecar_mismatch" -> SidecarMismatch
    [] label = "highest_qc_committed_conflict" -> HighestQcCommittedConflict
    [] OTHER -> Other

ActualFromReason(label) ==
  CASE Bug = "frontier_gap_label_unknown"
       /\ label = "frontier_gap_realign" -> Other
    [] Bug = "frontier_stall_label_unknown"
       /\ label = "frontier_stall_reset" -> Other
    [] Bug = "hard_cap_label_unknown"
       /\ label = "missing_block_height_hard_cap" -> Other
    [] Bug = "idle_reacquire_label_unknown"
       /\ label = "idle_missing_qc_reacquire" -> Other
    [] Bug = "lock_lag_defer_label_unknown"
       /\ label = "lock_lag_highest_qc_defer" -> Other
    [] Bug = "future_reanchor_label_unknown"
       /\ label = "future_new_view_frontier_reanchor" -> Other
    [] Bug = "future_prune_label_unknown"
       /\ label = "lock_lag_future_prune" -> Other
    [] Bug = "sidecar_label_unknown"
       /\ label = "sidecar_mismatch" -> Other
    [] Bug = "committed_conflict_label_unknown"
       /\ label = "highest_qc_committed_conflict" -> Other
    [] Bug = "unknown_maps_to_frontier"
       /\ label \in UnknownReasonLabels -> FrontierGapRealign
    [] OTHER -> SpecFromReason(label)

SpecRank(reason) ==
  CASE reason = FrontierGapRealign -> 0
    [] reason = FrontierStallReset -> 1
    [] reason = MissingBlockHeightHardCap -> 2
    [] reason = IdleMissingQcReacquire -> 3
    [] reason = LockLagHighestQcDefer -> 4
    [] reason = FutureNewViewFrontierReanchor -> 5
    [] reason = LockLagFuturePrune -> 6
    [] reason = SidecarMismatch -> 7
    [] reason = HighestQcCommittedConflict -> 8
    [] reason = Other -> 9

ActualRank(reason) ==
  CASE Bug = "rank_frontier_gap_wrong"
       /\ reason = FrontierGapRealign -> 1
    [] Bug = "rank_future_reanchor_wrong"
       /\ reason = FutureNewViewFrontierReanchor -> 6
    [] Bug = "rank_other_not_last"
       /\ reason = Other -> 0
    [] OTHER -> SpecRank(reason)

HeightDominatesLow == "HeightDominatesLow"
HeightDominatesHigh == "HeightDominatesHigh"
RankDominatesLow == "RankDominatesLow"
RankDominatesHigh == "RankDominatesHigh"
PeerDominatesLow == "PeerDominatesLow"
PeerDominatesHigh == "PeerDominatesHigh"
OtherLastKnown == "OtherLastKnown"
OtherLastOther == "OtherLastOther"

EventCases == {
  HeightDominatesLow,
  HeightDominatesHigh,
  RankDominatesLow,
  RankDominatesHigh,
  PeerDominatesLow,
  PeerDominatesHigh,
  OtherLastKnown,
  OtherLastOther
}

EventHeight(event) ==
  CASE event = HeightDominatesLow -> 1
    [] event = HeightDominatesHigh -> 2
    [] event = RankDominatesLow -> 3
    [] event = RankDominatesHigh -> 3
    [] event = PeerDominatesLow -> 4
    [] event = PeerDominatesHigh -> 4
    [] event = OtherLastKnown -> 5
    [] event = OtherLastOther -> 5

EventReason(event) ==
  CASE event = HeightDominatesLow -> Other
    [] event = HeightDominatesHigh -> FrontierGapRealign
    [] event = RankDominatesLow -> FrontierStallReset
    [] event = RankDominatesHigh -> LockLagFuturePrune
    [] event = PeerDominatesLow -> SidecarMismatch
    [] event = PeerDominatesHigh -> SidecarMismatch
    [] event = OtherLastKnown -> HighestQcCommittedConflict
    [] event = OtherLastOther -> Other

EventPeer(event) ==
  CASE event = HeightDominatesLow -> 3
    [] event = HeightDominatesHigh -> 1
    [] event = RankDominatesLow -> 2
    [] event = RankDominatesHigh -> 1
    [] event = PeerDominatesLow -> 1
    [] event = PeerDominatesHigh -> 2
    [] event = OtherLastKnown -> 2
    [] event = OtherLastOther -> 1

SpecBefore(lhs, rhs) ==
  \/ EventHeight(lhs) < EventHeight(rhs)
  \/ /\ EventHeight(lhs) = EventHeight(rhs)
     /\ SpecRank(EventReason(lhs)) < SpecRank(EventReason(rhs))
  \/ /\ EventHeight(lhs) = EventHeight(rhs)
     /\ SpecRank(EventReason(lhs)) = SpecRank(EventReason(rhs))
     /\ EventPeer(lhs) < EventPeer(rhs)

ActualBeforeByKey(lhs, rhs) ==
  \/ EventHeight(lhs) < EventHeight(rhs)
  \/ /\ EventHeight(lhs) = EventHeight(rhs)
     /\ ActualRank(EventReason(lhs)) < ActualRank(EventReason(rhs))
  \/ /\ EventHeight(lhs) = EventHeight(rhs)
     /\ ActualRank(EventReason(lhs)) = ActualRank(EventReason(rhs))
     /\ EventPeer(lhs) < EventPeer(rhs)

ActualBeforeIgnoringHeight(lhs, rhs) ==
  \/ ActualRank(EventReason(lhs)) < ActualRank(EventReason(rhs))
  \/ /\ ActualRank(EventReason(lhs)) = ActualRank(EventReason(rhs))
     /\ EventPeer(lhs) < EventPeer(rhs)

ActualBeforeIgnoringRank(lhs, rhs) ==
  \/ EventHeight(lhs) < EventHeight(rhs)
  \/ /\ EventHeight(lhs) = EventHeight(rhs)
     /\ EventPeer(lhs) < EventPeer(rhs)

ActualBeforeIgnoringPeer(lhs, rhs) ==
  \/ EventHeight(lhs) < EventHeight(rhs)
  \/ /\ EventHeight(lhs) = EventHeight(rhs)
     /\ ActualRank(EventReason(lhs)) < ActualRank(EventReason(rhs))

ActualBefore(lhs, rhs) ==
  CASE Bug = "sort_ignores_height" -> ActualBeforeIgnoringHeight(lhs, rhs)
    [] Bug = "sort_ignores_rank" -> ActualBeforeIgnoringRank(lhs, rhs)
    [] Bug = "sort_ignores_peer" -> ActualBeforeIgnoringPeer(lhs, rhs)
    [] OTHER -> ActualBeforeByKey(lhs, rhs)

BugSet == {
  "none",
  "frontier_gap_label_unknown",
  "frontier_stall_label_unknown",
  "hard_cap_label_unknown",
  "idle_reacquire_label_unknown",
  "lock_lag_defer_label_unknown",
  "future_reanchor_label_unknown",
  "future_prune_label_unknown",
  "sidecar_label_unknown",
  "committed_conflict_label_unknown",
  "unknown_maps_to_frontier",
  "rank_frontier_gap_wrong",
  "rank_future_reanchor_wrong",
  "rank_other_not_last",
  "sort_ignores_height",
  "sort_ignores_rank",
  "sort_ignores_peer"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0
  /\ \A label \in ReasonInputCases: ActualFromReason(label) \in ReasonCases
  /\ \A reason \in ReasonCases: ActualRank(reason) \in 0..9
  /\ \A event \in EventCases: EventHeight(event) \in 1..5
  /\ \A event \in EventCases: EventReason(event) \in ReasonCases
  /\ \A event \in EventCases: EventPeer(event) \in 1..3
  /\ \A lhs, rhs \in EventCases: ActualBefore(lhs, rhs) \in BOOLEAN

FromReasonExact ==
  \A label \in ReasonInputCases:
    ActualFromReason(label) = SpecFromReason(label)

KnownLabelsNotOther ==
  \A label \in KnownReasonLabels:
    ActualFromReason(label) # Other

UnknownLabelsFallbackOther ==
  \A label \in UnknownReasonLabels:
    ActualFromReason(label) = Other

RankValuesExact ==
  \A reason \in ReasonCases:
    ActualRank(reason) = SpecRank(reason)

RankOrderExact ==
  /\ ActualRank(FrontierGapRealign) = 0
  /\ ActualRank(FrontierStallReset) = 1
  /\ ActualRank(MissingBlockHeightHardCap) = 2
  /\ ActualRank(IdleMissingQcReacquire) = 3
  /\ ActualRank(LockLagHighestQcDefer) = 4
  /\ ActualRank(FutureNewViewFrontierReanchor) = 5
  /\ ActualRank(LockLagFuturePrune) = 6
  /\ ActualRank(SidecarMismatch) = 7
  /\ ActualRank(HighestQcCommittedConflict) = 8
  /\ ActualRank(Other) = 9

RanksDistinct ==
  \A lhs, rhs \in ReasonCases:
    lhs # rhs => ActualRank(lhs) # ActualRank(rhs)

SortMatchesTupleKey ==
  \A lhs, rhs \in EventCases:
    ActualBefore(lhs, rhs) = SpecBefore(lhs, rhs)

RepresentativeSortTieBreaksStable ==
  /\ ActualBefore(HeightDominatesLow, HeightDominatesHigh)
  /\ ~ActualBefore(HeightDominatesHigh, HeightDominatesLow)
  /\ ActualBefore(RankDominatesLow, RankDominatesHigh)
  /\ ~ActualBefore(RankDominatesHigh, RankDominatesLow)
  /\ ActualBefore(PeerDominatesLow, PeerDominatesHigh)
  /\ ~ActualBefore(PeerDominatesHigh, PeerDominatesLow)
  /\ ActualBefore(OtherLastKnown, OtherLastOther)
  /\ ~ActualBefore(OtherLastOther, OtherLastKnown)

RecoveryFsmReasonCoreSafety ==
  /\ FromReasonExact
  /\ KnownLabelsNotOther
  /\ UnknownLabelsFallbackOther
  /\ RankValuesExact
  /\ RankOrderExact
  /\ RanksDistinct
  /\ SortMatchesTupleKey
  /\ RepresentativeSortTieBreaksStable

RecoveryFsmReasonExactness ==
  /\ FromReasonExact
  /\ KnownLabelsNotOther
  /\ UnknownLabelsFallbackOther
  /\ RankValuesExact
  /\ RankOrderExact
  /\ RanksDistinct
  /\ SortMatchesTupleKey
  /\ RepresentativeSortTieBreaksStable
RecoveryFsmReasonCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RecoveryFsmReasonExactness

SafetyFast ==
  RecoveryFsmReasonExactness

====
