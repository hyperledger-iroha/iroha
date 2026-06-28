---- MODULE SumeragiHistoryStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi status/history helper projection.

This slice captures the history-facing helpers in `status.rs` and their Torii
status routes: validator-set checkpoints, commit certificates, NPoS election
outcomes, consensus key lifecycle records, commit-QC status snapshots, and the
bounded route windows that expose those histories.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

CheckpointResetClears == 1
CheckpointRecordAppends == 2
CheckpointNewestFirst == 3
CheckpointCapDropsOldest == 4
CheckpointRouteProjects == 5
CommitQcResetClears == 6
CommitQcReplacesOlderSameBlock == 7
CommitQcKeepsNewerSameBlock == 8
CommitQcPreservesDistinctHash == 9
CommitQcPreservesDistinctHeight == 10
CommitQcSortsHeightViewDesc == 11
CommitQcCapHonorsConfig == 12
CommitQcRouteClampsWindow == 13
CommitQcSnapshotLatest == 14
NposResetClears == 15
NposLatestEmptyNone == 16
NposLatestNewest == 17
NposCapDropsOldest == 18
NposStatusSnapshotProjects == 19
ConsensusKeyResetClears == 20
ConsensusKeyReplacesSameId == 21
ConsensusKeyPreservesDistinct == 22
ConsensusKeyNewestFirst == 23
ConsensusKeyCapDropsOldest == 24
ConsensusKeyRouteProjects == 25

Candidates == 1..25

CheckpointReset == 1
CheckpointSnapshotEmpty == 2
CheckpointAppend == 3
CheckpointHistoryProjects == 4
CheckpointNewerFirst == 5
CheckpointCapEnforced == 6
CheckpointDropsOldest == 7
CheckpointRouteUsesHistory == 8
CommitReset == 9
CommitSnapshotEmpty == 10
CommitInsert == 11
CommitDropOlderSameBlock == 12
CommitKeepNewerSameBlock == 13
CommitKeepDistinctHash == 14
CommitKeepDistinctHeight == 15
CommitSortHeightDesc == 16
CommitSortViewDesc == 17
CommitCapFromConfig == 18
CommitDropsOldest == 19
CommitRouteUsesHistory == 20
CommitRouteWindowFrom == 21
CommitRouteLimitCap == 22
CommitQcSnapshotUsesFirst == 23
CommitQcSnapshotFields == 24
NposReset == 25
NposSnapshotEmpty == 26
NposLatestNone == 27
NposAppend == 28
NposLatestNextBack == 29
NposCapEnforced == 30
NposDropsOldest == 31
NposStatusSnapshotLatest == 32
KeyReset == 33
KeySnapshotEmpty == 34
KeyReplaceSameId == 35
KeyKeepDistinct == 36
KeyNewestFirst == 37
KeyCapEnforced == 38
KeyDropsOldest == 39
KeyRouteUsesHistory == 40

Actions == 1..40

SpecActions(candidate) ==
  CASE candidate = CheckpointResetClears ->
      {CheckpointReset, CheckpointSnapshotEmpty}
    [] candidate = CheckpointRecordAppends ->
      {CheckpointAppend, CheckpointHistoryProjects}
    [] candidate = CheckpointNewestFirst ->
      {CheckpointNewerFirst, CheckpointHistoryProjects}
    [] candidate = CheckpointCapDropsOldest ->
      {CheckpointCapEnforced, CheckpointDropsOldest, CheckpointHistoryProjects}
    [] candidate = CheckpointRouteProjects ->
      {CheckpointRouteUsesHistory, CheckpointHistoryProjects}
    [] candidate = CommitQcResetClears ->
      {CommitReset, CommitSnapshotEmpty}
    [] candidate = CommitQcReplacesOlderSameBlock ->
      {CommitInsert, CommitDropOlderSameBlock}
    [] candidate = CommitQcKeepsNewerSameBlock ->
      {CommitInsert, CommitKeepNewerSameBlock}
    [] candidate = CommitQcPreservesDistinctHash ->
      {CommitInsert, CommitKeepDistinctHash}
    [] candidate = CommitQcPreservesDistinctHeight ->
      {CommitInsert, CommitKeepDistinctHeight}
    [] candidate = CommitQcSortsHeightViewDesc ->
      {CommitSortHeightDesc, CommitSortViewDesc}
    [] candidate = CommitQcCapHonorsConfig ->
      {CommitCapFromConfig, CommitDropsOldest}
    [] candidate = CommitQcRouteClampsWindow ->
      {CommitRouteUsesHistory, CommitRouteWindowFrom, CommitRouteLimitCap}
    [] candidate = CommitQcSnapshotLatest ->
      {CommitSortHeightDesc, CommitSortViewDesc, CommitQcSnapshotUsesFirst,
       CommitQcSnapshotFields}
    [] candidate = NposResetClears ->
      {NposReset, NposSnapshotEmpty}
    [] candidate = NposLatestEmptyNone ->
      {NposLatestNone}
    [] candidate = NposLatestNewest ->
      {NposAppend, NposLatestNextBack}
    [] candidate = NposCapDropsOldest ->
      {NposCapEnforced, NposDropsOldest}
    [] candidate = NposStatusSnapshotProjects ->
      {NposLatestNextBack, NposStatusSnapshotLatest}
    [] candidate = ConsensusKeyResetClears ->
      {KeyReset, KeySnapshotEmpty}
    [] candidate = ConsensusKeyReplacesSameId ->
      {KeyReplaceSameId, KeyNewestFirst}
    [] candidate = ConsensusKeyPreservesDistinct ->
      {KeyKeepDistinct, KeyNewestFirst}
    [] candidate = ConsensusKeyNewestFirst ->
      {KeyNewestFirst}
    [] candidate = ConsensusKeyCapDropsOldest ->
      {KeyCapEnforced, KeyDropsOldest, KeyNewestFirst}
    [] candidate = ConsensusKeyRouteProjects ->
      {KeyRouteUsesHistory, KeyNewestFirst}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = CheckpointResetClears /\
          Bug = "checkpoint_reset_keeps_entries" ->
      spec \ {CheckpointReset, CheckpointSnapshotEmpty}
    [] candidate = CheckpointRecordAppends /\
          Bug = "checkpoint_append_dropped" ->
      spec \ {CheckpointAppend, CheckpointHistoryProjects}
    [] candidate = CheckpointNewestFirst /\
          Bug = "checkpoint_not_newest_first" ->
      spec \ {CheckpointNewerFirst}
    [] candidate = CheckpointCapDropsOldest /\
          Bug = "checkpoint_cap_keeps_oldest" ->
      spec \ {CheckpointDropsOldest, CheckpointHistoryProjects}
    [] candidate = CheckpointRouteProjects /\
          Bug = "checkpoint_route_drops_history" ->
      spec \ {CheckpointRouteUsesHistory}
    [] candidate = CommitQcResetClears /\
          Bug = "commit_reset_keeps_entries" ->
      spec \ {CommitReset, CommitSnapshotEmpty}
    [] candidate = CommitQcReplacesOlderSameBlock /\
          Bug = "commit_same_block_old_view_kept" ->
      spec \ {CommitDropOlderSameBlock}
    [] candidate = CommitQcKeepsNewerSameBlock /\
          Bug = "commit_lower_view_replaces_newer" ->
      spec \ {CommitKeepNewerSameBlock}
    [] candidate = CommitQcPreservesDistinctHash /\
          Bug = "commit_distinct_hash_dropped" ->
      spec \ {CommitKeepDistinctHash}
    [] candidate = CommitQcPreservesDistinctHeight /\
          Bug = "commit_distinct_height_dropped" ->
      spec \ {CommitKeepDistinctHeight}
    [] candidate = CommitQcSortsHeightViewDesc /\
          Bug = "commit_history_not_sorted" ->
      spec \ {CommitSortHeightDesc, CommitSortViewDesc}
    [] candidate = CommitQcCapHonorsConfig /\
          Bug = "commit_cap_ignores_config" ->
      spec \ {CommitCapFromConfig}
    [] candidate = CommitQcCapHonorsConfig /\
          Bug = "commit_cap_keeps_oldest" ->
      spec \ {CommitDropsOldest}
    [] candidate = CommitQcRouteClampsWindow /\
          Bug = "commit_route_ignores_window" ->
      spec \ {CommitRouteWindowFrom}
    [] candidate = CommitQcRouteClampsWindow /\
          Bug = "commit_route_ignores_page_cap" ->
      spec \ {CommitRouteLimitCap}
    [] candidate = CommitQcSnapshotLatest /\
          Bug = "commit_snapshot_uses_tail" ->
      spec \ {CommitQcSnapshotUsesFirst}
    [] candidate = CommitQcSnapshotLatest /\
          Bug = "commit_snapshot_drops_fields" ->
      spec \ {CommitQcSnapshotFields}
    [] candidate = NposResetClears /\
          Bug = "npos_reset_keeps_entry" ->
      spec \ {NposReset, NposSnapshotEmpty}
    [] candidate = NposLatestEmptyNone /\
          Bug = "npos_empty_returns_entry" ->
      spec \ {NposLatestNone}
    [] candidate = NposLatestNewest /\
          Bug = "npos_latest_uses_oldest" ->
      spec \ {NposLatestNextBack}
    [] candidate = NposCapDropsOldest /\
          Bug = "npos_cap_keeps_oldest" ->
      spec \ {NposDropsOldest}
    [] candidate = NposStatusSnapshotProjects /\
          Bug = "npos_snapshot_drops_latest" ->
      spec \ {NposStatusSnapshotLatest}
    [] candidate = ConsensusKeyResetClears /\
          Bug = "key_reset_keeps_entries" ->
      spec \ {KeyReset, KeySnapshotEmpty}
    [] candidate = ConsensusKeyReplacesSameId /\
          Bug = "key_same_id_appends" ->
      spec \ {KeyReplaceSameId}
    [] candidate = ConsensusKeyPreservesDistinct /\
          Bug = "key_distinct_dropped" ->
      spec \ {KeyKeepDistinct}
    [] candidate = ConsensusKeyNewestFirst /\
          Bug = "key_not_newest_first" ->
      spec \ {KeyNewestFirst}
    [] candidate = ConsensusKeyCapDropsOldest /\
          Bug = "key_cap_keeps_oldest" ->
      spec \ {KeyDropsOldest}
    [] candidate = ConsensusKeyRouteProjects /\
          Bug = "key_route_drops_history" ->
      spec \ {KeyRouteUsesHistory}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 25
     /\ checked' = checked + 1
  \/ /\ checked = 25
     /\ checked' = checked

TypeInvariant ==
  checked \in 0..25

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugCheckpointResetKeepsEntries ==
  ImplementationActions(CheckpointResetClears) =
    SpecActions(CheckpointResetClears)

BugCheckpointAppendDropped ==
  ImplementationActions(CheckpointRecordAppends) =
    SpecActions(CheckpointRecordAppends)

BugCheckpointNotNewestFirst ==
  ImplementationActions(CheckpointNewestFirst) =
    SpecActions(CheckpointNewestFirst)

BugCheckpointCapKeepsOldest ==
  ImplementationActions(CheckpointCapDropsOldest) =
    SpecActions(CheckpointCapDropsOldest)

BugCheckpointRouteDropsHistory ==
  ImplementationActions(CheckpointRouteProjects) =
    SpecActions(CheckpointRouteProjects)

BugCommitResetKeepsEntries ==
  ImplementationActions(CommitQcResetClears) =
    SpecActions(CommitQcResetClears)

BugCommitSameBlockOldViewKept ==
  ImplementationActions(CommitQcReplacesOlderSameBlock) =
    SpecActions(CommitQcReplacesOlderSameBlock)

BugCommitLowerViewReplacesNewer ==
  ImplementationActions(CommitQcKeepsNewerSameBlock) =
    SpecActions(CommitQcKeepsNewerSameBlock)

BugCommitDistinctHashDropped ==
  ImplementationActions(CommitQcPreservesDistinctHash) =
    SpecActions(CommitQcPreservesDistinctHash)

BugCommitDistinctHeightDropped ==
  ImplementationActions(CommitQcPreservesDistinctHeight) =
    SpecActions(CommitQcPreservesDistinctHeight)

BugCommitHistoryNotSorted ==
  ImplementationActions(CommitQcSortsHeightViewDesc) =
    SpecActions(CommitQcSortsHeightViewDesc)

BugCommitCapIgnoresConfig ==
  ImplementationActions(CommitQcCapHonorsConfig) =
    SpecActions(CommitQcCapHonorsConfig)

BugCommitCapKeepsOldest ==
  ImplementationActions(CommitQcCapHonorsConfig) =
    SpecActions(CommitQcCapHonorsConfig)

BugCommitRouteIgnoresWindow ==
  ImplementationActions(CommitQcRouteClampsWindow) =
    SpecActions(CommitQcRouteClampsWindow)

BugCommitRouteIgnoresPageCap ==
  ImplementationActions(CommitQcRouteClampsWindow) =
    SpecActions(CommitQcRouteClampsWindow)

BugCommitSnapshotUsesTail ==
  ImplementationActions(CommitQcSnapshotLatest) =
    SpecActions(CommitQcSnapshotLatest)

BugCommitSnapshotDropsFields ==
  ImplementationActions(CommitQcSnapshotLatest) =
    SpecActions(CommitQcSnapshotLatest)

BugNposResetKeepsEntry ==
  ImplementationActions(NposResetClears) =
    SpecActions(NposResetClears)

BugNposEmptyReturnsEntry ==
  ImplementationActions(NposLatestEmptyNone) =
    SpecActions(NposLatestEmptyNone)

BugNposLatestUsesOldest ==
  ImplementationActions(NposLatestNewest) =
    SpecActions(NposLatestNewest)

BugNposCapKeepsOldest ==
  ImplementationActions(NposCapDropsOldest) =
    SpecActions(NposCapDropsOldest)

BugNposSnapshotDropsLatest ==
  ImplementationActions(NposStatusSnapshotProjects) =
    SpecActions(NposStatusSnapshotProjects)

BugKeyResetKeepsEntries ==
  ImplementationActions(ConsensusKeyResetClears) =
    SpecActions(ConsensusKeyResetClears)

BugKeySameIdAppends ==
  ImplementationActions(ConsensusKeyReplacesSameId) =
    SpecActions(ConsensusKeyReplacesSameId)

BugKeyDistinctDropped ==
  ImplementationActions(ConsensusKeyPreservesDistinct) =
    SpecActions(ConsensusKeyPreservesDistinct)

BugKeyNotNewestFirst ==
  ImplementationActions(ConsensusKeyNewestFirst) =
    SpecActions(ConsensusKeyNewestFirst)

BugKeyCapKeepsOldest ==
  ImplementationActions(ConsensusKeyCapDropsOldest) =
    SpecActions(ConsensusKeyCapDropsOldest)

BugKeyRouteDropsHistory ==
  ImplementationActions(ConsensusKeyRouteProjects) =
    SpecActions(ConsensusKeyRouteProjects)

AllHistoryStatusCandidatesMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

CheckpointHistoryAnchors ==
  /\ {CheckpointReset, CheckpointSnapshotEmpty} \subseteq
       ImplementationActions(CheckpointResetClears)
  /\ {CheckpointAppend, CheckpointHistoryProjects} \subseteq
       ImplementationActions(CheckpointRecordAppends)
  /\ CheckpointNewerFirst \in
       ImplementationActions(CheckpointNewestFirst)
  /\ {CheckpointCapEnforced, CheckpointDropsOldest,
      CheckpointHistoryProjects} \subseteq
       ImplementationActions(CheckpointCapDropsOldest)
  /\ {CheckpointRouteUsesHistory, CheckpointHistoryProjects} \subseteq
       ImplementationActions(CheckpointRouteProjects)

CommitQcResetAndReplacementAnchors ==
  /\ {CommitReset, CommitSnapshotEmpty} \subseteq
       ImplementationActions(CommitQcResetClears)
  /\ {CommitInsert, CommitDropOlderSameBlock} \subseteq
       ImplementationActions(CommitQcReplacesOlderSameBlock)
  /\ {CommitInsert, CommitKeepNewerSameBlock} \subseteq
       ImplementationActions(CommitQcKeepsNewerSameBlock)
  /\ {CommitInsert, CommitKeepDistinctHash} \subseteq
       ImplementationActions(CommitQcPreservesDistinctHash)
  /\ {CommitInsert, CommitKeepDistinctHeight} \subseteq
       ImplementationActions(CommitQcPreservesDistinctHeight)

CommitQcOrderingRouteAnchors ==
  /\ {CommitSortHeightDesc, CommitSortViewDesc} \subseteq
       ImplementationActions(CommitQcSortsHeightViewDesc)
  /\ {CommitCapFromConfig, CommitDropsOldest} \subseteq
       ImplementationActions(CommitQcCapHonorsConfig)
  /\ {CommitRouteUsesHistory, CommitRouteWindowFrom,
      CommitRouteLimitCap} \subseteq
       ImplementationActions(CommitQcRouteClampsWindow)

CommitQcSnapshotAnchors ==
  /\ {CommitSortHeightDesc, CommitSortViewDesc,
      CommitQcSnapshotUsesFirst, CommitQcSnapshotFields} \subseteq
       ImplementationActions(CommitQcSnapshotLatest)

NposHistoryAnchors ==
  /\ {NposReset, NposSnapshotEmpty} \subseteq
       ImplementationActions(NposResetClears)
  /\ NposLatestNone \in ImplementationActions(NposLatestEmptyNone)
  /\ {NposAppend, NposLatestNextBack} \subseteq
       ImplementationActions(NposLatestNewest)
  /\ {NposCapEnforced, NposDropsOldest} \subseteq
       ImplementationActions(NposCapDropsOldest)
  /\ {NposLatestNextBack, NposStatusSnapshotLatest} \subseteq
       ImplementationActions(NposStatusSnapshotProjects)

ConsensusKeyHistoryAnchors ==
  /\ {KeyReset, KeySnapshotEmpty} \subseteq
       ImplementationActions(ConsensusKeyResetClears)
  /\ {KeyReplaceSameId, KeyNewestFirst} \subseteq
       ImplementationActions(ConsensusKeyReplacesSameId)
  /\ {KeyKeepDistinct, KeyNewestFirst} \subseteq
       ImplementationActions(ConsensusKeyPreservesDistinct)
  /\ KeyNewestFirst \in ImplementationActions(ConsensusKeyNewestFirst)
  /\ {KeyCapEnforced, KeyDropsOldest, KeyNewestFirst} \subseteq
       ImplementationActions(ConsensusKeyCapDropsOldest)
  /\ {KeyRouteUsesHistory, KeyNewestFirst} \subseteq
       ImplementationActions(ConsensusKeyRouteProjects)

SafetyAnchors ==
  /\ AllHistoryStatusCandidatesMatchSpec
  /\ CheckpointHistoryAnchors
  /\ CommitQcResetAndReplacementAnchors
  /\ CommitQcOrderingRouteAnchors
  /\ CommitQcSnapshotAnchors
  /\ NposHistoryAnchors
  /\ ConsensusKeyHistoryAnchors

HistoryStatusExactness ==
  /\ \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)
  /\ SafetyAnchors

HistoryStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ HistoryStatusExactness

=============================================================================
====
