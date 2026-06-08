---- MODULE SumeragiPersistedRosterSelectionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for persisted block-sync roster selection.

This slice captures the non-cryptographic control policy in
`persisted_roster_for_block(...)`. It preserves the observable selector
behavior: consensus mode chooses the matching mode tag; commit-roster journal
evidence is tried before sidecars and successor evidence; cache hits return
before revalidation; successful selections use the source tied to the evidence
lane; cache insertion is guarded by returned roster evidence; commit-QC and
checkpoint histories are recorded only for artifacts returned by the selection;
failed journal/sidecar validation falls through to the next persisted source;
sidecars are gated by `allow_sidecar` and target block hash; successor previous
roster evidence is accepted only when the successor points at the requested
block and the evidence target matches; previous-roster stake snapshots are
converted into the runtime cache representation; previous-roster selection is
checkpoint-only; and missing or mismatched persisted sources fail closed.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ModeTagPermissioned == 1
ModeTagNpos == 2
CommitJournalFirst == 3
CommitJournalSelectionView == 4
CommitJournalKeyView == 5
CommitJournalCacheHitReturns == 6
CommitJournalSource == 7
CommitJournalCacheInsertGuard == 8
CommitJournalRecordsArtifacts == 9
CommitJournalFailureFallsThrough == 10
SidecarAllowGate == 11
SidecarHashGate == 12
SidecarSelectionView == 13
SidecarKeyView == 14
SidecarCacheHitReturns == 15
SidecarSource == 16
SidecarCacheInsertGuard == 17
SidecarRecordsArtifacts == 18
SidecarFailureFallsThrough == 19
SuccessorHeightChecked == 20
SuccessorPrevHashGate == 21
PreviousEvidenceTargetGate == 22
PreviousEvidenceMismatchReturnsNone == 23
PreviousEvidenceStakeSnapshotConverted == 24
PreviousEvidenceCheckpointOnly == 25
PreviousEvidenceCacheHitReturns == 26
PreviousEvidenceSource == 27
PreviousEvidenceCacheInsertGuard == 28
PreviousEvidenceRecordsCheckpointOnly == 29
NoSourcesReturnsNone == 30

Candidates == 1..30

ModeTagCases == {
  ModeTagPermissioned,
  ModeTagNpos
}

CommitJournalSourceCases == {
  CommitJournalFirst,
  CommitJournalFailureFallsThrough
}

CommitJournalSelectionCases == {
  CommitJournalSelectionView,
  CommitJournalKeyView,
  CommitJournalCacheHitReturns,
  CommitJournalSource,
  CommitJournalCacheInsertGuard,
  CommitJournalRecordsArtifacts
}

SidecarGateCases == {
  SidecarAllowGate,
  SidecarHashGate,
  SidecarFailureFallsThrough
}

SidecarSelectionCases == {
  SidecarSelectionView,
  SidecarKeyView,
  SidecarCacheHitReturns,
  SidecarSource,
  SidecarCacheInsertGuard,
  SidecarRecordsArtifacts
}

SuccessorPreviousGateCases == {
  SuccessorHeightChecked,
  SuccessorPrevHashGate,
  PreviousEvidenceTargetGate,
  PreviousEvidenceMismatchReturnsNone
}

PreviousEvidenceSelectionCases == {
  PreviousEvidenceStakeSnapshotConverted,
  PreviousEvidenceCheckpointOnly,
  PreviousEvidenceCacheHitReturns,
  PreviousEvidenceSource,
  PreviousEvidenceCacheInsertGuard,
  PreviousEvidenceRecordsCheckpointOnly
}

NoSourceCases == {
  NoSourcesReturnsNone
}

ModeTagPermissionedAction == 1
ModeTagNposAction == 2
JournalLookup == 3
SidecarLookup == 4
SuccessorLookup == 5
SourceCommitRosterJournal == 6
SourceRosterSidecar == 7
SourcePreviousBlockEvidence == 8
SelectionViewUsesCommitAndCheckpoint == 9
SelectionViewUsesSidecarArtifacts == 10
SelectionViewUsesCheckpointOnly == 11
KeyViewFromSelection == 12
KeyViewFallbackCommitView == 13
KeyViewFallbackCheckpointView == 14
KeyViewFallbackZero == 15
CacheKeyFromHints == 16
CacheLookup == 17
CacheHitReturns == 18
CacheInsert == 19
CacheInsertEvidenceGuard == 20
RecordCommitQc == 21
RecordCheckpoint == 22
FailureFallsThrough == 23
AllowSidecarGate == 24
BlockHashMatchGate == 25
MismatchedSidecarFallsThrough == 26
SuccessorHeightCheckedAction == 27
SuccessorPrevHashGateAction == 28
PreviousEvidenceTargetGateAction == 29
PreviousMismatchReturnsNoneAction == 30
ConvertPreviousStakeSnapshot == 31
NoCommitQcForPreviousEvidence == 32
ReturnNone == 33
SelectionCall == 34

Actions == 1..34

SpecActions(candidate) ==
  CASE candidate = ModeTagPermissioned ->
      {ModeTagPermissionedAction}
    [] candidate = ModeTagNpos ->
      {ModeTagNposAction}
    [] candidate = CommitJournalFirst ->
      {JournalLookup, SourceCommitRosterJournal}
    [] candidate = CommitJournalSelectionView ->
      {SelectionViewUsesCommitAndCheckpoint}
    [] candidate = CommitJournalKeyView ->
      {KeyViewFromSelection, KeyViewFallbackCommitView, CacheKeyFromHints}
    [] candidate = CommitJournalCacheHitReturns ->
      {CacheLookup, CacheHitReturns}
    [] candidate = CommitJournalSource ->
      {SelectionCall, SourceCommitRosterJournal}
    [] candidate = CommitJournalCacheInsertGuard ->
      {CacheInsertEvidenceGuard, CacheInsert}
    [] candidate = CommitJournalRecordsArtifacts ->
      {RecordCommitQc, RecordCheckpoint}
    [] candidate = CommitJournalFailureFallsThrough ->
      {FailureFallsThrough, SidecarLookup}
    [] candidate = SidecarAllowGate ->
      {AllowSidecarGate, SidecarLookup}
    [] candidate = SidecarHashGate ->
      {BlockHashMatchGate, MismatchedSidecarFallsThrough, SuccessorLookup}
    [] candidate = SidecarSelectionView ->
      {SelectionViewUsesSidecarArtifacts}
    [] candidate = SidecarKeyView ->
      {KeyViewFromSelection, KeyViewFallbackCheckpointView, KeyViewFallbackZero,
       CacheKeyFromHints}
    [] candidate = SidecarCacheHitReturns ->
      {CacheLookup, CacheHitReturns}
    [] candidate = SidecarSource ->
      {SelectionCall, SourceRosterSidecar}
    [] candidate = SidecarCacheInsertGuard ->
      {CacheInsertEvidenceGuard, CacheInsert}
    [] candidate = SidecarRecordsArtifacts ->
      {RecordCommitQc, RecordCheckpoint}
    [] candidate = SidecarFailureFallsThrough ->
      {FailureFallsThrough, SuccessorLookup}
    [] candidate = SuccessorHeightChecked ->
      {SuccessorHeightCheckedAction}
    [] candidate = SuccessorPrevHashGate ->
      {SuccessorLookup, SuccessorPrevHashGateAction}
    [] candidate = PreviousEvidenceTargetGate ->
      {PreviousEvidenceTargetGateAction}
    [] candidate = PreviousEvidenceMismatchReturnsNone ->
      {PreviousEvidenceTargetGateAction, PreviousMismatchReturnsNoneAction,
       ReturnNone}
    [] candidate = PreviousEvidenceStakeSnapshotConverted ->
      {ConvertPreviousStakeSnapshot}
    [] candidate = PreviousEvidenceCheckpointOnly ->
      {SelectionViewUsesCheckpointOnly, NoCommitQcForPreviousEvidence}
    [] candidate = PreviousEvidenceCacheHitReturns ->
      {CacheLookup, CacheHitReturns}
    [] candidate = PreviousEvidenceSource ->
      {SelectionCall, SourcePreviousBlockEvidence}
    [] candidate = PreviousEvidenceCacheInsertGuard ->
      {CacheInsertEvidenceGuard, CacheInsert}
    [] candidate = PreviousEvidenceRecordsCheckpointOnly ->
      {RecordCheckpoint}
    [] candidate = NoSourcesReturnsNone ->
      {ReturnNone}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ModeTagPermissioned /\
          Bug = "permissioned_uses_npos_tag" ->
      (spec \ {ModeTagPermissionedAction}) \cup {ModeTagNposAction}
    [] candidate = ModeTagNpos /\ Bug = "npos_uses_permissioned_tag" ->
      (spec \ {ModeTagNposAction}) \cup {ModeTagPermissionedAction}
    [] candidate = CommitJournalFirst /\
          Bug = "sidecar_before_commit_journal" ->
      (spec \ {JournalLookup, SourceCommitRosterJournal}) \cup
        {SidecarLookup, SourceRosterSidecar}
    [] candidate = CommitJournalSelectionView /\
          Bug = "journal_view_uses_checkpoint_only" ->
      (spec \ {SelectionViewUsesCommitAndCheckpoint}) \cup
        {SelectionViewUsesCheckpointOnly}
    [] candidate = CommitJournalKeyView /\
          Bug = "journal_key_fallback_checkpoint" ->
      (spec \ {KeyViewFallbackCommitView}) \cup {KeyViewFallbackCheckpointView}
    [] candidate = CommitJournalCacheHitReturns /\
          Bug = "journal_cache_hit_ignored" ->
      (spec \ {CacheHitReturns}) \cup {SelectionCall}
    [] candidate = CommitJournalSource /\
          Bug = "journal_source_sidecar" ->
      (spec \ {SourceCommitRosterJournal}) \cup {SourceRosterSidecar}
    [] candidate = CommitJournalCacheInsertGuard /\
          Bug = "journal_cache_insert_without_evidence" ->
      spec \ {CacheInsertEvidenceGuard}
    [] candidate = CommitJournalRecordsArtifacts /\
          Bug = "journal_records_checkpoint_only" ->
      spec \ {RecordCommitQc}
    [] candidate = CommitJournalFailureFallsThrough /\
          Bug = "journal_failure_returns_none" ->
      (spec \ {FailureFallsThrough, SidecarLookup}) \cup {ReturnNone}
    [] candidate = SidecarAllowGate /\ Bug = "sidecar_ignores_allow_gate" ->
      spec \ {AllowSidecarGate}
    [] candidate = SidecarHashGate /\ Bug = "sidecar_mismatch_selected" ->
      (spec \ {MismatchedSidecarFallsThrough, SuccessorLookup}) \cup
        {SelectionCall, SourceRosterSidecar}
    [] candidate = SidecarKeyView /\ Bug = "sidecar_key_fallback_commit" ->
      (spec \ {KeyViewFallbackCheckpointView, KeyViewFallbackZero}) \cup
        {KeyViewFallbackCommitView}
    [] candidate = SidecarSource /\ Bug = "sidecar_source_commit_journal" ->
      (spec \ {SourceRosterSidecar}) \cup {SourceCommitRosterJournal}
    [] candidate = SidecarFailureFallsThrough /\
          Bug = "sidecar_failure_returns_none" ->
      (spec \ {FailureFallsThrough, SuccessorLookup}) \cup {ReturnNone}
    [] candidate = SuccessorHeightChecked /\
          Bug = "successor_height_unchecked" ->
      spec \ {SuccessorHeightCheckedAction}
    [] candidate = SuccessorPrevHashGate /\
          Bug = "successor_prev_hash_ignored" ->
      spec \ {SuccessorPrevHashGateAction}
    [] candidate = PreviousEvidenceTargetGate /\
          Bug = "previous_evidence_target_ignored" ->
      spec \ {PreviousEvidenceTargetGateAction}
    [] candidate = PreviousEvidenceMismatchReturnsNone /\
          Bug = "previous_mismatch_falls_through" ->
      (spec \ {PreviousMismatchReturnsNoneAction, ReturnNone}) \cup
        {FailureFallsThrough}
    [] candidate = PreviousEvidenceStakeSnapshotConverted /\
          Bug = "previous_stake_snapshot_not_converted" ->
      spec \ {ConvertPreviousStakeSnapshot}
    [] candidate = PreviousEvidenceCheckpointOnly /\
          Bug = "previous_uses_commit_qc" ->
      (spec \ {NoCommitQcForPreviousEvidence}) \cup {RecordCommitQc}
    [] candidate = PreviousEvidenceSource /\
          Bug = "previous_source_sidecar" ->
      (spec \ {SourcePreviousBlockEvidence}) \cup {SourceRosterSidecar}
    [] candidate = PreviousEvidenceCacheInsertGuard /\
          Bug = "previous_cache_insert_without_evidence" ->
      spec \ {CacheInsertEvidenceGuard}
    [] candidate = PreviousEvidenceRecordsCheckpointOnly /\
          Bug = "previous_records_commit_qc" ->
      spec \cup {RecordCommitQc}
    [] candidate = NoSourcesReturnsNone /\
          Bug = "no_sources_returns_selection" ->
      (spec \ {ReturnNone}) \cup {SelectionCall}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "permissioned_uses_npos_tag",
       "npos_uses_permissioned_tag",
       "sidecar_before_commit_journal",
       "journal_view_uses_checkpoint_only",
       "journal_key_fallback_checkpoint",
       "journal_cache_hit_ignored",
       "journal_source_sidecar",
       "journal_cache_insert_without_evidence",
       "journal_records_checkpoint_only",
       "journal_failure_returns_none",
       "sidecar_ignores_allow_gate",
       "sidecar_mismatch_selected",
       "sidecar_key_fallback_commit",
       "sidecar_source_commit_journal",
       "sidecar_failure_returns_none",
       "successor_height_unchecked",
       "successor_prev_hash_ignored",
       "previous_evidence_target_ignored",
       "previous_mismatch_falls_through",
       "previous_stake_snapshot_not_converted",
       "previous_uses_commit_qc",
       "previous_source_sidecar",
       "previous_cache_insert_without_evidence",
       "previous_records_commit_qc",
       "no_sources_returns_selection"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

PersistedRosterModeTagExact ==
  \A c \in ModeTagCases:
    ImplementationActions(c) = SpecActions(c)

PersistedRosterCommitJournalSourceExact ==
  \A c \in CommitJournalSourceCases:
    ImplementationActions(c) = SpecActions(c)

PersistedRosterCommitJournalSelectionExact ==
  \A c \in CommitJournalSelectionCases:
    ImplementationActions(c) = SpecActions(c)

PersistedRosterSidecarGateExact ==
  \A c \in SidecarGateCases:
    ImplementationActions(c) = SpecActions(c)

PersistedRosterSidecarSelectionExact ==
  \A c \in SidecarSelectionCases:
    ImplementationActions(c) = SpecActions(c)

PersistedRosterSuccessorPreviousGateExact ==
  \A c \in SuccessorPreviousGateCases:
    ImplementationActions(c) = SpecActions(c)

PersistedRosterPreviousEvidenceSelectionExact ==
  \A c \in PreviousEvidenceSelectionCases:
    ImplementationActions(c) = SpecActions(c)

PersistedRosterNoSourceExact ==
  \A c \in NoSourceCases:
    ImplementationActions(c) = SpecActions(c)

PersistedRosterSelectionExactness ==
  /\ PersistedRosterModeTagExact
  /\ PersistedRosterCommitJournalSourceExact
  /\ PersistedRosterCommitJournalSelectionExact
  /\ PersistedRosterSidecarGateExact
  /\ PersistedRosterSidecarSelectionExact
  /\ PersistedRosterSuccessorPreviousGateExact
  /\ PersistedRosterPreviousEvidenceSelectionExact
  /\ PersistedRosterNoSourceExact

BugPermissionedUsesNposTag ==
  ImplementationActions(ModeTagPermissioned) = SpecActions(ModeTagPermissioned)

BugNposUsesPermissionedTag ==
  ImplementationActions(ModeTagNpos) = SpecActions(ModeTagNpos)

BugSidecarBeforeCommitJournal ==
  ImplementationActions(CommitJournalFirst) = SpecActions(CommitJournalFirst)

BugJournalViewUsesCheckpointOnly ==
  ImplementationActions(CommitJournalSelectionView) =
    SpecActions(CommitJournalSelectionView)

BugJournalKeyFallbackCheckpoint ==
  ImplementationActions(CommitJournalKeyView) =
    SpecActions(CommitJournalKeyView)

BugJournalCacheHitIgnored ==
  ImplementationActions(CommitJournalCacheHitReturns) =
    SpecActions(CommitJournalCacheHitReturns)

BugJournalSourceSidecar ==
  ImplementationActions(CommitJournalSource) =
    SpecActions(CommitJournalSource)

BugJournalCacheInsertWithoutEvidence ==
  ImplementationActions(CommitJournalCacheInsertGuard) =
    SpecActions(CommitJournalCacheInsertGuard)

BugJournalRecordsCheckpointOnly ==
  ImplementationActions(CommitJournalRecordsArtifacts) =
    SpecActions(CommitJournalRecordsArtifacts)

BugJournalFailureReturnsNone ==
  ImplementationActions(CommitJournalFailureFallsThrough) =
    SpecActions(CommitJournalFailureFallsThrough)

BugSidecarIgnoresAllowGate ==
  ImplementationActions(SidecarAllowGate) = SpecActions(SidecarAllowGate)

BugSidecarMismatchSelected ==
  ImplementationActions(SidecarHashGate) = SpecActions(SidecarHashGate)

BugSidecarKeyFallbackCommit ==
  ImplementationActions(SidecarKeyView) = SpecActions(SidecarKeyView)

BugSidecarSourceCommitJournal ==
  ImplementationActions(SidecarSource) = SpecActions(SidecarSource)

BugSidecarFailureReturnsNone ==
  ImplementationActions(SidecarFailureFallsThrough) =
    SpecActions(SidecarFailureFallsThrough)

BugSuccessorHeightUnchecked ==
  ImplementationActions(SuccessorHeightChecked) =
    SpecActions(SuccessorHeightChecked)

BugSuccessorPrevHashIgnored ==
  ImplementationActions(SuccessorPrevHashGate) =
    SpecActions(SuccessorPrevHashGate)

BugPreviousEvidenceTargetIgnored ==
  ImplementationActions(PreviousEvidenceTargetGate) =
    SpecActions(PreviousEvidenceTargetGate)

BugPreviousMismatchFallsThrough ==
  ImplementationActions(PreviousEvidenceMismatchReturnsNone) =
    SpecActions(PreviousEvidenceMismatchReturnsNone)

BugPreviousStakeSnapshotNotConverted ==
  ImplementationActions(PreviousEvidenceStakeSnapshotConverted) =
    SpecActions(PreviousEvidenceStakeSnapshotConverted)

BugPreviousUsesCommitQc ==
  ImplementationActions(PreviousEvidenceCheckpointOnly) =
    SpecActions(PreviousEvidenceCheckpointOnly)

BugPreviousSourceSidecar ==
  ImplementationActions(PreviousEvidenceSource) =
    SpecActions(PreviousEvidenceSource)

BugPreviousCacheInsertWithoutEvidence ==
  ImplementationActions(PreviousEvidenceCacheInsertGuard) =
    SpecActions(PreviousEvidenceCacheInsertGuard)

BugPreviousRecordsCommitQc ==
  ImplementationActions(PreviousEvidenceRecordsCheckpointOnly) =
    SpecActions(PreviousEvidenceRecordsCheckpointOnly)

BugNoSourcesReturnsSelection ==
  ImplementationActions(NoSourcesReturnsNone) =
    SpecActions(NoSourcesReturnsNone)

====
