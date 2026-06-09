---- MODULE SumeragiCommittedEdgeConflictGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for committed-edge highest-QC conflict suppression.

This slice captures `suppress_committed_edge_conflicting_highest_qc(...)`.
It abstracts hashes, storage, pending queues, and recovery windows to finite
cases while preserving the helper contract: only highest QCs at or below the
committed height with a known but different committed hash are suppressed;
suppression records a lock-rejected branch, purges conflicting artifacts,
clears obsolete requests and sidecar mismatch state, preserves canonical
highest/locked QCs, prunes stale frontier recovery only when canonical
frontier evidence is absent, activates the committed-edge owner when unresolved
frontier or NPoS recovery requires it, and emits at most the recovery action
allowed by the shared recovery window.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

FutureHeightNoSuppress == "future_height_no_suppress"
MissingCommittedHashNoSuppress == "missing_committed_hash_no_suppress"
MatchingCommittedHashNoSuppress == "matching_committed_hash_no_suppress"
ConflictBase == "conflict_base"
ConflictDescendantPending == "conflict_descendant_pending"
ConflictHighestReanchor == "conflict_highest_reanchor"
ConflictHigherHighestPreserved == "conflict_higher_highest_preserved"
ConflictLockedRealign == "conflict_locked_realign"
ConflictObsoleteRequest == "conflict_obsolete_request"
CanonicalEvidencePreservesState == "canonical_evidence_preserves_state"
NoCanonicalEvidencePrunesFrontier == "no_canonical_evidence_prunes_frontier"
FutureRoundClamped == "future_round_clamped"
ActiveFrontierViewReset == "active_frontier_view_reset"
ForcedViewReset == "forced_view_reset"
OwnerRequiredForDependency == "owner_required_for_dependency"
OwnerRequiredForNpos == "owner_required_for_npos"
OwnerWindowDenied == "owner_window_denied"
NoOwnerRangePullAllowed == "no_owner_range_pull_allowed"
NoOwnerWindowDenied == "no_owner_window_denied"

Cases == {
  FutureHeightNoSuppress,
  MissingCommittedHashNoSuppress,
  MatchingCommittedHashNoSuppress,
  ConflictBase,
  ConflictDescendantPending,
  ConflictHighestReanchor,
  ConflictHigherHighestPreserved,
  ConflictLockedRealign,
  ConflictObsoleteRequest,
  CanonicalEvidencePreservesState,
  NoCanonicalEvidencePrunesFrontier,
  FutureRoundClamped,
  ActiveFrontierViewReset,
  ForcedViewReset,
  OwnerRequiredForDependency,
  OwnerRequiredForNpos,
  OwnerWindowDenied,
  NoOwnerRangePullAllowed,
  NoOwnerWindowDenied
}

ReturnSuppressed == 1
ReturnNotSuppressed == 2
LockRejectedRecorded == 3
ArtifactsPurged == 4
ObsoleteCounterInc == 5
SidecarCleared == 6
DescendantsDropped == 7
HighestReanchored == 8
HighestPreserved == 9
LockedReanchored == 10
ObsoleteRequestCleared == 11
FrontierPreserved == 12
FrontierPruned == 13
NewViewCleared == 14
PhaseClamped == 15
ForcedViewCleared == 16
OwnerActivated == 17
OwnerCleared == 18
PassiveCatchupHandoff == 19
OwnerReanchor == 20
RangePullReanchor == 21
NoRecoveryReanchor == 22

ActionUniverse == 1..22

BaseSuppressionActions ==
  {ReturnSuppressed, LockRejectedRecorded, ArtifactsPurged,
   ObsoleteCounterInc, SidecarCleared}

SpecActions(c) ==
  CASE c \in {
       FutureHeightNoSuppress,
       MissingCommittedHashNoSuppress,
       MatchingCommittedHashNoSuppress
     } ->
      {ReturnNotSuppressed}
    [] c = ConflictBase ->
      BaseSuppressionActions
    [] c = ConflictDescendantPending ->
      BaseSuppressionActions \cup {DescendantsDropped}
    [] c = ConflictHighestReanchor ->
      BaseSuppressionActions \cup {HighestReanchored}
    [] c = ConflictHigherHighestPreserved ->
      BaseSuppressionActions \cup {HighestPreserved}
    [] c = ConflictLockedRealign ->
      BaseSuppressionActions \cup {LockedReanchored}
    [] c = ConflictObsoleteRequest ->
      BaseSuppressionActions \cup {ObsoleteRequestCleared}
    [] c = CanonicalEvidencePreservesState ->
      BaseSuppressionActions \cup {FrontierPreserved, OwnerCleared,
                                   RangePullReanchor}
    [] c = NoCanonicalEvidencePrunesFrontier ->
      BaseSuppressionActions \cup {FrontierPruned, NewViewCleared}
    [] c = FutureRoundClamped ->
      BaseSuppressionActions \cup {FrontierPruned, PhaseClamped}
    [] c = ActiveFrontierViewReset ->
      BaseSuppressionActions \cup {FrontierPruned, PhaseClamped}
    [] c = ForcedViewReset ->
      BaseSuppressionActions \cup {FrontierPruned, ForcedViewCleared}
    [] c = OwnerRequiredForDependency ->
      BaseSuppressionActions \cup {FrontierPruned, OwnerActivated,
                                   PassiveCatchupHandoff, OwnerReanchor}
    [] c = OwnerRequiredForNpos ->
      BaseSuppressionActions \cup {FrontierPruned, OwnerActivated,
                                   PassiveCatchupHandoff, OwnerReanchor}
    [] c = OwnerWindowDenied ->
      BaseSuppressionActions \cup {FrontierPruned, OwnerActivated,
                                   PassiveCatchupHandoff, NoRecoveryReanchor}
    [] c = NoOwnerRangePullAllowed ->
      BaseSuppressionActions \cup {FrontierPruned, RangePullReanchor}
    [] c = NoOwnerWindowDenied ->
      BaseSuppressionActions \cup {FrontierPruned, NoRecoveryReanchor}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "suppress_future_height"
       /\ c = FutureHeightNoSuppress ->
      BaseSuppressionActions
    [] Bug = "suppress_missing_committed_hash"
       /\ c = MissingCommittedHashNoSuppress ->
      BaseSuppressionActions
    [] Bug = "suppress_matching_hash"
       /\ c = MatchingCommittedHashNoSuppress ->
      BaseSuppressionActions
    [] Bug = "skip_lock_rejected_note"
       /\ c = ConflictBase ->
      spec \ {LockRejectedRecorded}
    [] Bug = "skip_artifact_purge"
       /\ c = ConflictBase ->
      spec \ {ArtifactsPurged}
    [] Bug = "skip_obsolete_counter"
       /\ c = ConflictBase ->
      spec \ {ObsoleteCounterInc}
    [] Bug = "skip_descendant_drop"
       /\ c = ConflictDescendantPending ->
      spec \ {DescendantsDropped}
    [] Bug = "skip_highest_reanchor"
       /\ c = ConflictHighestReanchor ->
      (spec \ {HighestReanchored}) \cup {HighestPreserved}
    [] Bug = "overwrite_higher_highest"
       /\ c = ConflictHigherHighestPreserved ->
      (spec \ {HighestPreserved}) \cup {HighestReanchored}
    [] Bug = "skip_locked_realign"
       /\ c = ConflictLockedRealign ->
      spec \ {LockedReanchored}
    [] Bug = "skip_obsolete_request_clear"
       /\ c = ConflictObsoleteRequest ->
      spec \ {ObsoleteRequestCleared}
    [] Bug = "skip_sidecar_clear"
       /\ c = ConflictBase ->
      spec \ {SidecarCleared}
    [] Bug = "prune_with_canonical_evidence"
       /\ c = CanonicalEvidencePreservesState ->
      (spec \ {FrontierPreserved}) \cup {FrontierPruned}
    [] Bug = "keep_owner_with_canonical_evidence"
       /\ c = CanonicalEvidencePreservesState ->
      (spec \ {OwnerCleared}) \cup {OwnerActivated}
    [] Bug = "skip_frontier_prune"
       /\ c = NoCanonicalEvidencePrunesFrontier ->
      spec \ {FrontierPruned}
    [] Bug = "skip_new_view_clear"
       /\ c = NoCanonicalEvidencePrunesFrontier ->
      spec \ {NewViewCleared}
    [] Bug = "skip_phase_clamp"
       /\ c = FutureRoundClamped ->
      spec \ {PhaseClamped}
    [] Bug = "skip_forced_view_reset"
       /\ c = ForcedViewReset ->
      spec \ {ForcedViewCleared}
    [] Bug = "skip_owner_for_dependency"
       /\ c = OwnerRequiredForDependency ->
      (spec \ {OwnerActivated, PassiveCatchupHandoff, OwnerReanchor}) \cup
        {RangePullReanchor}
    [] Bug = "skip_owner_for_npos"
       /\ c = OwnerRequiredForNpos ->
      (spec \ {OwnerActivated, PassiveCatchupHandoff, OwnerReanchor}) \cup
        {RangePullReanchor}
    [] Bug = "emit_reanchor_when_window_denied"
       /\ c = OwnerWindowDenied ->
      (spec \ {NoRecoveryReanchor}) \cup {OwnerReanchor}
    [] Bug = "skip_range_pull_when_allowed"
       /\ c = NoOwnerRangePullAllowed ->
      (spec \ {RangePullReanchor}) \cup {NoRecoveryReanchor}
    [] Bug = "skip_owner_reanchor_when_allowed"
       /\ c = OwnerRequiredForDependency ->
      (spec \ {OwnerReanchor}) \cup {NoRecoveryReanchor}
    [] OTHER -> spec

Bugs == {
  "none",
  "suppress_future_height",
  "suppress_missing_committed_hash",
  "suppress_matching_hash",
  "skip_lock_rejected_note",
  "skip_artifact_purge",
  "skip_obsolete_counter",
  "skip_descendant_drop",
  "skip_highest_reanchor",
  "overwrite_higher_highest",
  "skip_locked_realign",
  "skip_obsolete_request_clear",
  "skip_sidecar_clear",
  "prune_with_canonical_evidence",
  "keep_owner_with_canonical_evidence",
  "skip_frontier_prune",
  "skip_new_view_clear",
  "skip_phase_clamp",
  "skip_forced_view_reset",
  "skip_owner_for_dependency",
  "skip_owner_for_npos",
  "emit_reanchor_when_window_denied",
  "skip_range_pull_when_allowed",
  "skip_owner_reanchor_when_allowed"
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

SuppressionResultMatchesSpec ==
  \A c \in Cases:
    (ReturnSuppressed \in ImplementationActions(c))
      = (ReturnSuppressed \in SpecActions(c))

NonConflictsDoNotMutate ==
  \A c \in {FutureHeightNoSuppress, MissingCommittedHashNoSuppress,
            MatchingCommittedHashNoSuppress}:
    ImplementationActions(c) = {ReturnNotSuppressed}

CanonicalStateProtected ==
  /\ HighestReanchored \in ImplementationActions(ConflictHighestReanchor)
  /\ HighestPreserved \in ImplementationActions(ConflictHigherHighestPreserved)
  /\ LockedReanchored \in ImplementationActions(ConflictLockedRealign)
  /\ FrontierPreserved \in ImplementationActions(CanonicalEvidencePreservesState)
  /\ OwnerCleared \in ImplementationActions(CanonicalEvidencePreservesState)

RecoveryWindowRespected ==
  /\ NoRecoveryReanchor \in ImplementationActions(OwnerWindowDenied)
  /\ NoRecoveryReanchor \in ImplementationActions(NoOwnerWindowDenied)
  /\ OwnerReanchor \in ImplementationActions(OwnerRequiredForDependency)
  /\ OwnerReanchor \in ImplementationActions(OwnerRequiredForNpos)
  /\ RangePullReanchor \in ImplementationActions(NoOwnerRangePullAllowed)

CommittedEdgeConflictCoreSafety ==
  /\ ActionsMatchSpec
  /\ SuppressionResultMatchesSpec
  /\ NonConflictsDoNotMutate
  /\ CanonicalStateProtected
  /\ RecoveryWindowRespected

NoBugInvariant == CommittedEdgeConflictCoreSafety

SafetyFast == CommittedEdgeConflictCoreSafety

====
