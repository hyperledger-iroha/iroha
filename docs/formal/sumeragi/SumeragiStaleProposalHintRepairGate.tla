---- MODULE SumeragiStaleProposalHintRepairGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for stale proposal-hint repair admission.

This slice captures `stale_proposal_hint_can_seed_frontier_repair(...)` and
the stale-view branch in `handle_proposal_hint(...)`. It abstracts block hashes,
views, and committed-QC identity to finite cases while preserving the helper
contract: a stale proposal hint may bypass the stale-view drop only when DA is
enabled, the hint targets the active height, the local view is exactly one
ahead, and the hint highest-QC identity exactly matches the latest committed
QC by height, view, subject hash, and epoch. Every other stale hint remains a
normal stale-view drop and must not cache proposal metadata, mark the slot as
observed, or mutate highest-QC state.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ExactCommittedQcRepair == "exact_committed_qc_repair"
DaDisabled == "da_disabled"
NonActiveHeight == "non_active_height"
LocalViewSame == "local_view_same"
LocalViewTooFar == "local_view_too_far"
NoCommittedQc == "no_committed_qc"
QcHeightMismatch == "qc_height_mismatch"
QcViewMismatch == "qc_view_mismatch"
QcHashMismatch == "qc_hash_mismatch"
QcEpochMismatch == "qc_epoch_mismatch"
RejectDoesNotCache == "reject_does_not_cache"
RejectDoesNotObserve == "reject_does_not_observe"
RejectDoesNotMutateHighest == "reject_does_not_mutate_highest"

Cases == {
  ExactCommittedQcRepair,
  DaDisabled,
  NonActiveHeight,
  LocalViewSame,
  LocalViewTooFar,
  NoCommittedQc,
  QcHeightMismatch,
  QcViewMismatch,
  QcHashMismatch,
  QcEpochMismatch,
  RejectDoesNotCache,
  RejectDoesNotObserve,
  RejectDoesNotMutateHighest
}

ReturnAllow == 1
ReturnDeny == 2
RequireDa == 3
RequireActiveHeight == 4
RequireOneViewBehind == 5
RequireCommittedQc == 6
MatchQcHeight == 7
MatchQcView == 8
MatchQcHash == 9
MatchQcEpoch == 10
ContinueAfterStaleView == 11
StaleViewDrop == 12
NoHintCache == 13
NoProposalSeen == 14
NoHighestQcMutation == 15
NoRepairWithoutDa == 16

ActionUniverse == 1..16

AllowActions ==
  {ReturnAllow, RequireDa, RequireActiveHeight, RequireOneViewBehind,
   RequireCommittedQc, MatchQcHeight, MatchQcView, MatchQcHash,
   MatchQcEpoch, ContinueAfterStaleView}

RejectBaseActions ==
  {ReturnDeny, StaleViewDrop, NoHintCache, NoProposalSeen,
   NoHighestQcMutation}

SpecActions(c) ==
  CASE c = ExactCommittedQcRepair ->
      AllowActions
    [] c = DaDisabled ->
      RejectBaseActions \cup {RequireDa, NoRepairWithoutDa}
    [] c = NonActiveHeight ->
      RejectBaseActions \cup {RequireDa, RequireActiveHeight}
    [] c \in {LocalViewSame, LocalViewTooFar} ->
      RejectBaseActions \cup {RequireDa, RequireActiveHeight,
                              RequireOneViewBehind}
    [] c = NoCommittedQc ->
      RejectBaseActions \cup {RequireDa, RequireActiveHeight,
                              RequireOneViewBehind, RequireCommittedQc}
    [] c = QcHeightMismatch ->
      RejectBaseActions \cup {RequireDa, RequireActiveHeight,
                              RequireOneViewBehind, RequireCommittedQc,
                              MatchQcHeight}
    [] c = QcViewMismatch ->
      RejectBaseActions \cup {RequireDa, RequireActiveHeight,
                              RequireOneViewBehind, RequireCommittedQc,
                              MatchQcHeight, MatchQcView}
    [] c = QcHashMismatch ->
      RejectBaseActions \cup {RequireDa, RequireActiveHeight,
                              RequireOneViewBehind, RequireCommittedQc,
                              MatchQcHeight, MatchQcView, MatchQcHash}
    [] c = QcEpochMismatch ->
      RejectBaseActions \cup {RequireDa, RequireActiveHeight,
                              RequireOneViewBehind, RequireCommittedQc,
                              MatchQcHeight, MatchQcView, MatchQcHash,
                              MatchQcEpoch}
    [] c \in {RejectDoesNotCache, RejectDoesNotObserve,
              RejectDoesNotMutateHighest} ->
      RejectBaseActions
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "reject_exact_committed"
       /\ c = ExactCommittedQcRepair ->
      (spec \ {ReturnAllow, ContinueAfterStaleView}) \cup
        {ReturnDeny, StaleViewDrop}
    [] Bug = "allow_da_disabled"
       /\ c = DaDisabled ->
      (spec \ {ReturnDeny, StaleViewDrop, NoRepairWithoutDa}) \cup
        {ReturnAllow, ContinueAfterStaleView}
    [] Bug = "allow_non_active_height"
       /\ c = NonActiveHeight ->
      (spec \ {ReturnDeny, StaleViewDrop}) \cup
        {ReturnAllow, ContinueAfterStaleView}
    [] Bug = "allow_local_view_same"
       /\ c = LocalViewSame ->
      (spec \ {ReturnDeny, StaleViewDrop}) \cup
        {ReturnAllow, ContinueAfterStaleView}
    [] Bug = "allow_local_view_too_far"
       /\ c = LocalViewTooFar ->
      (spec \ {ReturnDeny, StaleViewDrop}) \cup
        {ReturnAllow, ContinueAfterStaleView}
    [] Bug = "allow_without_committed_qc"
       /\ c = NoCommittedQc ->
      (spec \ {ReturnDeny, StaleViewDrop}) \cup
        {ReturnAllow, ContinueAfterStaleView}
    [] Bug = "allow_height_mismatch"
       /\ c = QcHeightMismatch ->
      (spec \ {ReturnDeny, StaleViewDrop}) \cup
        {ReturnAllow, ContinueAfterStaleView}
    [] Bug = "allow_view_mismatch"
       /\ c = QcViewMismatch ->
      (spec \ {ReturnDeny, StaleViewDrop}) \cup
        {ReturnAllow, ContinueAfterStaleView}
    [] Bug = "allow_hash_mismatch"
       /\ c = QcHashMismatch ->
      (spec \ {ReturnDeny, StaleViewDrop}) \cup
        {ReturnAllow, ContinueAfterStaleView}
    [] Bug = "allow_epoch_mismatch"
       /\ c = QcEpochMismatch ->
      (spec \ {ReturnDeny, StaleViewDrop}) \cup
        {ReturnAllow, ContinueAfterStaleView}
    [] Bug = "skip_stale_drop_after_reject"
       /\ c = RejectDoesNotCache ->
      spec \ {StaleViewDrop}
    [] Bug = "cache_rejected_stale"
       /\ c = RejectDoesNotCache ->
      spec \ {NoHintCache}
    [] Bug = "observe_rejected_stale"
       /\ c = RejectDoesNotObserve ->
      spec \ {NoProposalSeen}
    [] Bug = "mutate_highest_on_reject"
       /\ c = RejectDoesNotMutateHighest ->
      spec \ {NoHighestQcMutation}
    [] OTHER -> spec

Bugs == {
  "none",
  "reject_exact_committed",
  "allow_da_disabled",
  "allow_non_active_height",
  "allow_local_view_same",
  "allow_local_view_too_far",
  "allow_without_committed_qc",
  "allow_height_mismatch",
  "allow_view_mismatch",
  "allow_hash_mismatch",
  "allow_epoch_mismatch",
  "skip_stale_drop_after_reject",
  "cache_rejected_stale",
  "observe_rejected_stale",
  "mutate_highest_on_reject"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

ExactCommittedQcStaleHintSeedsRepair ==
  /\ ReturnAllow \in ImplementationActions(ExactCommittedQcRepair)
  /\ ContinueAfterStaleView \in ImplementationActions(ExactCommittedQcRepair)
  /\ ~(StaleViewDrop \in ImplementationActions(ExactCommittedQcRepair))

RepairRequiresDaActiveHeightAndOneLateView ==
  \A c \in {DaDisabled, NonActiveHeight, LocalViewSame, LocalViewTooFar}:
    /\ ReturnDeny \in ImplementationActions(c)
    /\ StaleViewDrop \in ImplementationActions(c)
    /\ ~(ReturnAllow \in ImplementationActions(c))

RepairRequiresExactCommittedQcIdentity ==
  \A c \in {NoCommittedQc, QcHeightMismatch, QcViewMismatch,
            QcHashMismatch, QcEpochMismatch}:
    /\ ReturnDeny \in ImplementationActions(c)
    /\ StaleViewDrop \in ImplementationActions(c)
    /\ ~(ReturnAllow \in ImplementationActions(c))

RejectedStaleHintsHaveNoSideEffects ==
  \A c \in Cases \ {ExactCommittedQcRepair}:
    /\ NoHintCache \in ImplementationActions(c)
    /\ NoProposalSeen \in ImplementationActions(c)
    /\ NoHighestQcMutation \in ImplementationActions(c)

DaDisabledNeverRepairs ==
  /\ NoRepairWithoutDa \in ImplementationActions(DaDisabled)
  /\ ~(ReturnAllow \in ImplementationActions(DaDisabled))

StaleProposalHintRepairCoreSafety ==
  /\ ActionsMatchSpec
  /\ ExactCommittedQcStaleHintSeedsRepair
  /\ RepairRequiresDaActiveHeightAndOneLateView
  /\ RepairRequiresExactCommittedQcIdentity
  /\ RejectedStaleHintsHaveNoSideEffects
  /\ DaDisabledNeverRepairs

StaleProposalHintRepairExactness ==
  /\ ActionsMatchSpec
  /\ ExactCommittedQcStaleHintSeedsRepair
  /\ RepairRequiresDaActiveHeightAndOneLateView
  /\ RepairRequiresExactCommittedQcIdentity
  /\ RejectedStaleHintsHaveNoSideEffects
  /\ DaDisabledNeverRepairs
StaleProposalHintRepairCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ StaleProposalHintRepairExactness

NoBugInvariant == StaleProposalHintRepairExactness

SafetyFast == StaleProposalHintRepairExactness

====
