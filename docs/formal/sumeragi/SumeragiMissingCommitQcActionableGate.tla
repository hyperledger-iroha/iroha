---- MODULE SumeragiMissingCommitQcActionableGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for known-block missing commit-QC repair predicates.

This slice captures `missing_commit_qc_request_has_actionable_dependency(...)`,
`should_preserve_missing_commit_qc_request_for_local_payload(...)`,
`known_block_commit_qc_request_is_superseded_by_higher_new_view_quorum(...)`,
and the NEW_VIEW subject-height mapping used by
`missing_dependency_subject_height_for_phase(...)`.

A missing commit-QC repair is actionable only when the local node has the
request's payload for the exact height/view, has no cached commit QC, is not
already superseded by a higher-view full NEW_VIEW quorum for committed+1, and
the dependency is not obsolete, superseded by a live same-height owner, or held
only by an active lock-rejected sink. Stale-prune preservation additionally
requires Commit phase, a local payload for the exact slot, no cached commit QC,
and either the authoritative owner or frontier owner slot.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

CommittedHeight == 10
CommittedFrontierHeight == CommittedHeight + 1

PendingPayloadOk == "pending_payload_ok"
LocalPayloadOk == "local_payload_ok"
BothPayloadSourcesOk == "both_payload_sources_ok"
PendingPayloadWrongHeight == "pending_payload_wrong_height"
PendingPayloadWrongView == "pending_payload_wrong_view"
LocalPayloadWrongHeight == "local_payload_wrong_height"
LocalPayloadWrongView == "local_payload_wrong_view"
NoPayload == "no_payload"
CachedCommitQc == "cached_commit_qc"
HigherNewViewQuorum == "higher_new_view_quorum"
HigherNewViewWrongHeight == "higher_new_view_wrong_height"
HigherNewViewEqualView == "higher_new_view_equal_view"
NonActionableObsoleteCommit == "non_actionable_obsolete_commit"
NonActionableSupersededOwner == "non_actionable_superseded_owner"
NonActionableLockRejected == "non_actionable_lock_rejected"
NewViewParentObsolete == "new_view_parent_obsolete"
PrepareSameHeightNotParent == "prepare_same_height_not_parent"

ActionableCases == {
  PendingPayloadOk,
  LocalPayloadOk,
  BothPayloadSourcesOk,
  PendingPayloadWrongHeight,
  PendingPayloadWrongView,
  LocalPayloadWrongHeight,
  LocalPayloadWrongView,
  NoPayload,
  CachedCommitQc,
  HigherNewViewQuorum,
  HigherNewViewWrongHeight,
  HigherNewViewEqualView,
  NonActionableObsoleteCommit,
  NonActionableSupersededOwner,
  NonActionableLockRejected,
  NewViewParentObsolete,
  PrepareSameHeightNotParent
}

NewViewZeroSaturates == "new_view_zero_saturates"

SubjectHeightCases == {
  NewViewParentObsolete,
  PrepareSameHeightNotParent,
  NewViewZeroSaturates
}

PreserveAuthoritativeOwner == "preserve_authoritative_owner"
PreserveFrontierOwner == "preserve_frontier_owner"
PreserveBothOwners == "preserve_both_owners"
PreserveNonCommitPhase == "preserve_non_commit_phase"
PreserveCachedQc == "preserve_cached_qc"
PreserveWrongOwner == "preserve_wrong_owner"
PreserveLocalWrongHeight == "preserve_local_wrong_height"
PreserveLocalWrongView == "preserve_local_wrong_view"
PreserveNoLocalPayload == "preserve_no_local_payload"

PreserveCases == {
  PreserveAuthoritativeOwner,
  PreserveFrontierOwner,
  PreserveBothOwners,
  PreserveNonCommitPhase,
  PreserveCachedQc,
  PreserveWrongOwner,
  PreserveLocalWrongHeight,
  PreserveLocalWrongView,
  PreserveNoLocalPayload
}

Cases == ActionableCases \cup SubjectHeightCases \cup PreserveCases

SpecPendingPayloadMatches(c) ==
  c \in {
    PendingPayloadOk,
    BothPayloadSourcesOk,
    CachedCommitQc,
    HigherNewViewQuorum,
    HigherNewViewWrongHeight,
    HigherNewViewEqualView,
    NonActionableObsoleteCommit,
    NonActionableSupersededOwner,
    NonActionableLockRejected,
    NewViewParentObsolete,
    PrepareSameHeightNotParent
  }

SpecLocalPayloadMatches(c) ==
  c \in {LocalPayloadOk, BothPayloadSourcesOk}

SpecPayloadAvailable(c) ==
  SpecPendingPayloadMatches(c) \/ SpecLocalPayloadMatches(c)

SpecCachedCommitQc(c) ==
  c = CachedCommitQc

SpecSupersededByHigherNewViewQuorum(c) ==
  c = HigherNewViewQuorum

SpecSubjectHeight(c) ==
  CASE c = NewViewParentObsolete -> CommittedHeight
    [] c = PrepareSameHeightNotParent -> CommittedFrontierHeight
    [] c = NewViewZeroSaturates -> 0

DependencyHashObsoleteAtSubjectHeight(c, subject_height) ==
  c \in {NewViewParentObsolete, PrepareSameHeightNotParent}
    /\ subject_height = CommittedHeight

SpecNonActionableDependency(c) ==
  c \in {
    NonActionableObsoleteCommit,
    NonActionableSupersededOwner,
    NonActionableLockRejected
  }
    \/ (c \in SubjectHeightCases
          /\ DependencyHashObsoleteAtSubjectHeight(c, SpecSubjectHeight(c)))

SpecActionable(c) ==
  /\ SpecPayloadAvailable(c)
  /\ ~SpecCachedCommitQc(c)
  /\ ~SpecSupersededByHigherNewViewQuorum(c)
  /\ ~SpecNonActionableDependency(c)

ActualPendingPayloadMatches(c) ==
  CASE Bug = "reject_pending_payload"
       /\ c = PendingPayloadOk -> FALSE
    [] Bug = "accept_wrong_pending_height"
       /\ c = PendingPayloadWrongHeight -> TRUE
    [] Bug = "accept_wrong_pending_view"
       /\ c = PendingPayloadWrongView -> TRUE
    [] Bug = "accept_no_payload"
       /\ c = NoPayload -> TRUE
    [] OTHER -> SpecPendingPayloadMatches(c)

ActualLocalPayloadMatches(c) ==
  CASE Bug = "reject_local_payload"
       /\ c = LocalPayloadOk -> FALSE
    [] Bug = "accept_wrong_local_height"
       /\ c = LocalPayloadWrongHeight -> TRUE
    [] Bug = "accept_wrong_local_view"
       /\ c = LocalPayloadWrongView -> TRUE
    [] OTHER -> SpecLocalPayloadMatches(c)

ActualCachedCommitQc(c) ==
  CASE Bug = "accept_cached_commit_qc"
       /\ c = CachedCommitQc -> FALSE
    [] OTHER -> SpecCachedCommitQc(c)

ActualSupersededByHigherNewViewQuorum(c) ==
  CASE Bug = "accept_higher_new_view_quorum"
       /\ c = HigherNewViewQuorum -> FALSE
    [] Bug = "supersede_wrong_height"
       /\ c = HigherNewViewWrongHeight -> TRUE
    [] Bug = "supersede_equal_view"
       /\ c = HigherNewViewEqualView -> TRUE
    [] OTHER -> SpecSupersededByHigherNewViewQuorum(c)

ActualSubjectHeight(c) ==
  CASE Bug = "new_view_uses_round_height"
       /\ c = NewViewParentObsolete -> CommittedFrontierHeight
    [] Bug = "prepare_subtracts_parent_height"
       /\ c = PrepareSameHeightNotParent -> CommittedHeight
    [] Bug = "new_view_zero_maps_to_one"
       /\ c = NewViewZeroSaturates -> 1
    [] OTHER -> SpecSubjectHeight(c)

ActualNonActionableDependency(c) ==
  CASE Bug = "accept_obsolete_dependency"
       /\ c = NonActionableObsoleteCommit -> FALSE
    [] Bug = "accept_superseded_owner_dependency"
       /\ c = NonActionableSupersededOwner -> FALSE
    [] Bug = "accept_lock_rejected_dependency"
       /\ c = NonActionableLockRejected -> FALSE
    [] c \in SubjectHeightCases ->
      DependencyHashObsoleteAtSubjectHeight(c, ActualSubjectHeight(c))
    [] OTHER -> SpecNonActionableDependency(c)

ActualActionable(c) ==
  /\ (ActualPendingPayloadMatches(c) \/ ActualLocalPayloadMatches(c))
  /\ ~ActualCachedCommitQc(c)
  /\ ~ActualSupersededByHigherNewViewQuorum(c)
  /\ ~ActualNonActionableDependency(c)

SpecPreservePhaseCommit(c) ==
  c \notin {PreserveNonCommitPhase}

SpecPreserveCachedQc(c) ==
  c = PreserveCachedQc

SpecPreserveOwnerMatches(c) ==
  c \in {
    PreserveAuthoritativeOwner,
    PreserveFrontierOwner,
    PreserveBothOwners,
    PreserveNonCommitPhase,
    PreserveCachedQc,
    PreserveLocalWrongHeight,
    PreserveLocalWrongView,
    PreserveNoLocalPayload
  }

SpecPreserveLocalPayloadMatches(c) ==
  c \notin {
    PreserveLocalWrongHeight,
    PreserveLocalWrongView,
    PreserveNoLocalPayload
  }

SpecPreserve(c) ==
  /\ SpecPreservePhaseCommit(c)
  /\ ~SpecPreserveCachedQc(c)
  /\ SpecPreserveOwnerMatches(c)
  /\ SpecPreserveLocalPayloadMatches(c)

ActualPreservePhaseCommit(c) ==
  CASE Bug = "preserve_accept_non_commit_phase"
       /\ c = PreserveNonCommitPhase -> TRUE
    [] OTHER -> SpecPreservePhaseCommit(c)

ActualPreserveCachedQc(c) ==
  CASE Bug = "preserve_accept_cached_qc"
       /\ c = PreserveCachedQc -> FALSE
    [] OTHER -> SpecPreserveCachedQc(c)

ActualPreserveOwnerMatches(c) ==
  CASE Bug = "preserve_reject_authoritative_owner"
       /\ c = PreserveAuthoritativeOwner -> FALSE
    [] Bug = "preserve_reject_frontier_owner"
       /\ c = PreserveFrontierOwner -> FALSE
    [] Bug = "preserve_accept_wrong_owner"
       /\ c = PreserveWrongOwner -> TRUE
    [] OTHER -> SpecPreserveOwnerMatches(c)

ActualPreserveLocalPayloadMatches(c) ==
  CASE Bug = "preserve_accept_local_wrong_height"
       /\ c = PreserveLocalWrongHeight -> TRUE
    [] Bug = "preserve_accept_local_wrong_view"
       /\ c = PreserveLocalWrongView -> TRUE
    [] Bug = "preserve_accept_no_local_payload"
       /\ c = PreserveNoLocalPayload -> TRUE
    [] OTHER -> SpecPreserveLocalPayloadMatches(c)

ActualPreserve(c) ==
  /\ ActualPreservePhaseCommit(c)
  /\ ~ActualPreserveCachedQc(c)
  /\ ActualPreserveOwnerMatches(c)
  /\ ActualPreserveLocalPayloadMatches(c)

Bugs == {
  "none",
  "reject_pending_payload",
  "reject_local_payload",
  "accept_wrong_pending_height",
  "accept_wrong_pending_view",
  "accept_wrong_local_height",
  "accept_wrong_local_view",
  "accept_no_payload",
  "accept_cached_commit_qc",
  "accept_higher_new_view_quorum",
  "supersede_wrong_height",
  "supersede_equal_view",
  "accept_obsolete_dependency",
  "accept_superseded_owner_dependency",
  "accept_lock_rejected_dependency",
  "new_view_uses_round_height",
  "prepare_subtracts_parent_height",
  "new_view_zero_maps_to_one",
  "preserve_reject_authoritative_owner",
  "preserve_reject_frontier_owner",
  "preserve_accept_non_commit_phase",
  "preserve_accept_cached_qc",
  "preserve_accept_wrong_owner",
  "preserve_accept_local_wrong_height",
  "preserve_accept_local_wrong_view",
  "preserve_accept_no_local_payload"
}

Init ==
  checked = 0

Next ==
  \/ /\ checked < 25
     /\ checked' = checked + 1
  \/ /\ checked = 25
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..25
  /\ \A c \in Cases:
       /\ (c \in ActionableCases => SpecActionable(c) \in BOOLEAN)
       /\ (c \in ActionableCases => ActualActionable(c) \in BOOLEAN)
       /\ (c \in PreserveCases => SpecPreserve(c) \in BOOLEAN)
       /\ (c \in PreserveCases => ActualPreserve(c) \in BOOLEAN)
       /\ (c \in SubjectHeightCases => SpecSubjectHeight(c) \in 0..CommittedFrontierHeight)
       /\ (c \in SubjectHeightCases => ActualSubjectHeight(c) \in 0..CommittedFrontierHeight)

ActionableMatchesSpec ==
  \A c \in ActionableCases:
    ActualActionable(c) = SpecActionable(c)

PreserveMatchesSpec ==
  \A c \in PreserveCases:
    ActualPreserve(c) = SpecPreserve(c)

SubjectHeightMatchesSpec ==
  \A c \in SubjectHeightCases:
    ActualSubjectHeight(c) = SpecSubjectHeight(c)

PayloadRequiredForActionableRepair ==
  /\ ActualActionable(PendingPayloadOk)
  /\ ActualActionable(LocalPayloadOk)
  /\ ActualActionable(BothPayloadSourcesOk)
  /\ ~ActualActionable(PendingPayloadWrongHeight)
  /\ ~ActualActionable(PendingPayloadWrongView)
  /\ ~ActualActionable(LocalPayloadWrongHeight)
  /\ ~ActualActionable(LocalPayloadWrongView)
  /\ ~ActualActionable(NoPayload)

CachedQcAndSupersededRepairRejected ==
  /\ ~ActualActionable(CachedCommitQc)
  /\ ~ActualActionable(HigherNewViewQuorum)
  /\ ActualActionable(HigherNewViewWrongHeight)
  /\ ActualActionable(HigherNewViewEqualView)

NonActionableDependenciesRejected ==
  /\ ~ActualActionable(NonActionableObsoleteCommit)
  /\ ~ActualActionable(NonActionableSupersededOwner)
  /\ ~ActualActionable(NonActionableLockRejected)

SubjectHeightMappingPreserved ==
  /\ ActualSubjectHeight(NewViewParentObsolete) = CommittedHeight
  /\ ActualSubjectHeight(PrepareSameHeightNotParent) = CommittedFrontierHeight
  /\ ActualSubjectHeight(NewViewZeroSaturates) = 0
  /\ ~ActualActionable(NewViewParentObsolete)
  /\ ActualActionable(PrepareSameHeightNotParent)

LocalPayloadPreserveRequiresExactCommitOwnerSlot ==
  /\ ActualPreserve(PreserveAuthoritativeOwner)
  /\ ActualPreserve(PreserveFrontierOwner)
  /\ ActualPreserve(PreserveBothOwners)
  /\ ~ActualPreserve(PreserveNonCommitPhase)
  /\ ~ActualPreserve(PreserveCachedQc)
  /\ ~ActualPreserve(PreserveWrongOwner)
  /\ ~ActualPreserve(PreserveLocalWrongHeight)
  /\ ~ActualPreserve(PreserveLocalWrongView)
  /\ ~ActualPreserve(PreserveNoLocalPayload)

MissingCommitQcActionableCoreSafety ==
  /\ ActionableMatchesSpec
  /\ PreserveMatchesSpec
  /\ SubjectHeightMatchesSpec
  /\ PayloadRequiredForActionableRepair
  /\ CachedQcAndSupersededRepairRejected
  /\ NonActionableDependenciesRejected
  /\ SubjectHeightMappingPreserved
  /\ LocalPayloadPreserveRequiresExactCommitOwnerSlot

SafetyFast ==
  MissingCommitQcActionableCoreSafety

SpecComparisonAnchors ==
  /\ ActionableMatchesSpec
  /\ PreserveMatchesSpec
  /\ SubjectHeightMatchesSpec

PayloadActionableAnchors ==
  /\ PayloadRequiredForActionableRepair
  /\ ActualActionable(PendingPayloadOk)
  /\ ActualActionable(LocalPayloadOk)
  /\ ActualActionable(BothPayloadSourcesOk)
  /\ ~ActualActionable(PendingPayloadWrongHeight)
  /\ ~ActualActionable(PendingPayloadWrongView)
  /\ ~ActualActionable(LocalPayloadWrongHeight)
  /\ ~ActualActionable(LocalPayloadWrongView)
  /\ ~ActualActionable(NoPayload)

RejectedRepairAnchors ==
  /\ CachedQcAndSupersededRepairRejected
  /\ NonActionableDependenciesRejected
  /\ ~ActualActionable(CachedCommitQc)
  /\ ~ActualActionable(HigherNewViewQuorum)
  /\ ActualActionable(HigherNewViewWrongHeight)
  /\ ActualActionable(HigherNewViewEqualView)
  /\ ~ActualActionable(NonActionableObsoleteCommit)
  /\ ~ActualActionable(NonActionableSupersededOwner)
  /\ ~ActualActionable(NonActionableLockRejected)

SubjectHeightAnchors ==
  /\ SubjectHeightMappingPreserved
  /\ ActualSubjectHeight(NewViewParentObsolete) = CommittedHeight
  /\ ActualSubjectHeight(PrepareSameHeightNotParent) = CommittedFrontierHeight
  /\ ActualSubjectHeight(NewViewZeroSaturates) = 0
  /\ ~ActualActionable(NewViewParentObsolete)
  /\ ActualActionable(PrepareSameHeightNotParent)

LocalPayloadPreserveAnchors ==
  /\ LocalPayloadPreserveRequiresExactCommitOwnerSlot
  /\ ActualPreserve(PreserveAuthoritativeOwner)
  /\ ActualPreserve(PreserveFrontierOwner)
  /\ ActualPreserve(PreserveBothOwners)
  /\ ~ActualPreserve(PreserveNonCommitPhase)
  /\ ~ActualPreserve(PreserveCachedQc)
  /\ ~ActualPreserve(PreserveWrongOwner)
  /\ ~ActualPreserve(PreserveLocalWrongHeight)
  /\ ~ActualPreserve(PreserveLocalWrongView)
  /\ ~ActualPreserve(PreserveNoLocalPayload)

MissingCommitQcActionableSafetyAnchors ==
  /\ SpecComparisonAnchors
  /\ PayloadActionableAnchors
  /\ RejectedRepairAnchors
  /\ SubjectHeightAnchors
  /\ LocalPayloadPreserveAnchors

Safety ==
  MissingCommitQcActionableSafetyAnchors

====
