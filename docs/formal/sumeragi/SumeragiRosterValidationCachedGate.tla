---- MODULE SumeragiRosterValidationCachedGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for cached roster-validation wrapper helpers.

This slice captures the wrapper policy in `validate_commit_qc_roster_cached`
and `validate_checkpoint_roster_cached` from `main_loop.rs`. Cryptographic
validation and roster derivation are abstracted behind action labels. The
model pins the observable wrapper contract: subject prefilters run before
memo lookup, optional block views are checked only when present, commit-QC
wrappers enforce epoch/phase/highest-QC/mode-tag gates, empty aggregate
signatures bypass memo lookup and go directly to validation, memo keys retain
the wrapper inputs that distinguish cached results, memo hits return the cached
roster without revalidation, misses validate and insert successful rosters, and
validation calls forward the caller-provided arguments.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

CommitHashPrefilter == 1
CommitHeightPrefilter == 2
CommitOptionalViewPrefilter == 3
CommitEpochPrefilter == 4
CommitPhasePrefilter == 5
CommitHighestQcPrefilter == 6
CommitModeTagPrefilter == 7
CommitEmptyAggregateBypassesMemo == 8
CommitMemoKeyUsesModeInputs == 9
CommitMemoHitReturnsCached == 10
CommitMemoMissValidates == 11
CommitMemoInsertOnSuccess == 12
CommitValidationForwardsArgs == 13
CheckpointHashPrefilter == 14
CheckpointHeightPrefilter == 15
CheckpointOptionalViewPrefilter == 16
CheckpointEmptyAggregateBypassesMemo == 17
CheckpointMemoKeyUsesEpochModeInputs == 18
CheckpointMemoHitReturnsCached == 19
CheckpointMemoMissValidates == 20
CheckpointMemoInsertOnSuccess == 21
CheckpointValidationForwardsArgs == 22
CommitPrefilterBeforeMemo == 23
CheckpointPrefilterBeforeMemo == 24

Candidates == 1..24

CommitPrefilterCases == {
  CommitHashPrefilter,
  CommitHeightPrefilter,
  CommitOptionalViewPrefilter,
  CommitEpochPrefilter,
  CommitPhasePrefilter,
  CommitHighestQcPrefilter,
  CommitModeTagPrefilter
}

CheckpointPrefilterCases == {
  CheckpointHashPrefilter,
  CheckpointHeightPrefilter,
  CheckpointOptionalViewPrefilter
}

EmptyAggregateCases == {
  CommitEmptyAggregateBypassesMemo,
  CheckpointEmptyAggregateBypassesMemo
}

MemoKeyCases == {
  CommitMemoKeyUsesModeInputs,
  CheckpointMemoKeyUsesEpochModeInputs
}

MemoFlowCases == {
  CommitMemoHitReturnsCached,
  CommitMemoMissValidates,
  CommitMemoInsertOnSuccess,
  CheckpointMemoHitReturnsCached,
  CheckpointMemoMissValidates,
  CheckpointMemoInsertOnSuccess
}

ValidationForwardCases == {
  CommitValidationForwardsArgs,
  CheckpointValidationForwardsArgs
}

PrefilterOrderingCases == {
  CommitPrefilterBeforeMemo,
  CheckpointPrefilterBeforeMemo
}

CheckBlockHash == 1
CheckHeight == 2
CheckOptionalView == 3
AllowAbsentView == 4
CheckEpoch == 5
CheckCommitPhase == 6
RejectHighestQc == 7
CheckModeTag == 8
ReturnError == 9
CheckAggregateEmpty == 10
EmptyAggregateDirectValidate == 11
SkipMemo == 12
BuildCommitMemoKey == 13
BuildCheckpointMemoKey == 14
UseCertificateInKey == 15
UseCheckpointInKey == 16
UseConsensusModeInKey == 17
UseInputsInKey == 18
UseEpochInKey == 19
MemoGet == 20
MemoHitReturn == 21
MemoMissValidate == 22
MemoInsert == 23
ReturnValidatedRoster == 24
ReturnCachedRoster == 25
ReturnNone == 26
SkipValidation == 27
ForwardBlockHash == 28
ForwardHeight == 29
ForwardBlockView == 30
ForwardConsensusMode == 31
ForwardExpectedEpoch == 32
ForwardChainId == 33
ForwardModeTag == 34
ForwardGenesisStub == 35
ForwardInputs == 36
ForwardRoots == 37
PrefilterBeforeMemo == 38
MemoBeforePrefilter == 39
ForwardCheckpointEpoch == 40

Actions == 1..40

CommitValidationArgs ==
  {ForwardBlockHash, ForwardHeight, ForwardBlockView, ForwardConsensusMode,
   ForwardExpectedEpoch, ForwardChainId, ForwardModeTag, ForwardGenesisStub,
   ForwardInputs}

CheckpointValidationArgs ==
  {ForwardBlockHash, ForwardHeight, ForwardBlockView, ForwardConsensusMode,
   ForwardChainId, ForwardModeTag, ForwardCheckpointEpoch, ForwardRoots,
   ForwardGenesisStub, ForwardInputs}

CommitPrefilters ==
  {CheckBlockHash, CheckHeight, CheckOptionalView, CheckEpoch,
   CheckCommitPhase, RejectHighestQc, CheckModeTag}

CheckpointPrefilters ==
  {CheckBlockHash, CheckHeight, CheckOptionalView}

SpecActions(candidate) ==
  CASE candidate = CommitHashPrefilter ->
      {CheckBlockHash, ReturnError}
    [] candidate = CommitHeightPrefilter ->
      {CheckHeight, ReturnError}
    [] candidate = CommitOptionalViewPrefilter ->
      {CheckOptionalView, AllowAbsentView, ReturnError}
    [] candidate = CommitEpochPrefilter ->
      {CheckEpoch, ReturnError}
    [] candidate = CommitPhasePrefilter ->
      {CheckCommitPhase, ReturnError}
    [] candidate = CommitHighestQcPrefilter ->
      {RejectHighestQc, ReturnError}
    [] candidate = CommitModeTagPrefilter ->
      {CheckModeTag, ReturnError}
    [] candidate = CommitEmptyAggregateBypassesMemo ->
      {CheckAggregateEmpty, EmptyAggregateDirectValidate, SkipMemo,
       ReturnValidatedRoster} \cup CommitValidationArgs
    [] candidate = CommitMemoKeyUsesModeInputs ->
      {BuildCommitMemoKey, UseCertificateInKey, UseConsensusModeInKey,
       UseInputsInKey}
    [] candidate = CommitMemoHitReturnsCached ->
      {BuildCommitMemoKey, MemoGet, MemoHitReturn, ReturnCachedRoster,
       SkipValidation}
    [] candidate = CommitMemoMissValidates ->
      {BuildCommitMemoKey, MemoGet, MemoMissValidate, ReturnValidatedRoster}
    [] candidate = CommitMemoInsertOnSuccess ->
      {MemoMissValidate, MemoInsert, ReturnValidatedRoster}
    [] candidate = CommitValidationForwardsArgs ->
      CommitValidationArgs
    [] candidate = CheckpointHashPrefilter ->
      {CheckBlockHash, ReturnError}
    [] candidate = CheckpointHeightPrefilter ->
      {CheckHeight, ReturnError}
    [] candidate = CheckpointOptionalViewPrefilter ->
      {CheckOptionalView, AllowAbsentView, ReturnError}
    [] candidate = CheckpointEmptyAggregateBypassesMemo ->
      {CheckAggregateEmpty, EmptyAggregateDirectValidate, SkipMemo,
       ReturnValidatedRoster} \cup CheckpointValidationArgs
    [] candidate = CheckpointMemoKeyUsesEpochModeInputs ->
      {BuildCheckpointMemoKey, UseCheckpointInKey, UseEpochInKey,
       UseConsensusModeInKey, UseInputsInKey}
    [] candidate = CheckpointMemoHitReturnsCached ->
      {BuildCheckpointMemoKey, MemoGet, MemoHitReturn, ReturnCachedRoster,
       SkipValidation}
    [] candidate = CheckpointMemoMissValidates ->
      {BuildCheckpointMemoKey, MemoGet, MemoMissValidate,
       ReturnValidatedRoster}
    [] candidate = CheckpointMemoInsertOnSuccess ->
      {MemoMissValidate, MemoInsert, ReturnValidatedRoster}
    [] candidate = CheckpointValidationForwardsArgs ->
      CheckpointValidationArgs
    [] candidate = CommitPrefilterBeforeMemo ->
      CommitPrefilters \cup {PrefilterBeforeMemo, SkipMemo}
    [] candidate = CheckpointPrefilterBeforeMemo ->
      CheckpointPrefilters \cup {PrefilterBeforeMemo, SkipMemo}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = CommitHashPrefilter /\ Bug = "commit_ignores_hash" ->
      spec \ {CheckBlockHash, ReturnError}
    [] candidate = CommitHeightPrefilter /\ Bug = "commit_ignores_height" ->
      spec \ {CheckHeight, ReturnError}
    [] candidate = CommitOptionalViewPrefilter /\
          Bug = "commit_requires_view_when_absent" ->
      spec \ {AllowAbsentView}
    [] candidate = CommitEpochPrefilter /\ Bug = "commit_ignores_epoch" ->
      spec \ {CheckEpoch, ReturnError}
    [] candidate = CommitPhasePrefilter /\
          Bug = "commit_accepts_prepare_phase" ->
      spec \ {CheckCommitPhase, ReturnError}
    [] candidate = CommitHighestQcPrefilter /\
          Bug = "commit_accepts_highest_qc" ->
      spec \ {RejectHighestQc, ReturnError}
    [] candidate = CommitModeTagPrefilter /\
          Bug = "commit_ignores_mode_tag" ->
      spec \ {CheckModeTag, ReturnError}
    [] candidate = CommitEmptyAggregateBypassesMemo /\
          Bug = "commit_empty_aggregate_uses_memo" ->
      (spec \ {EmptyAggregateDirectValidate, SkipMemo}) \cup
        {BuildCommitMemoKey, MemoGet}
    [] candidate = CommitMemoKeyUsesModeInputs /\
          Bug = "commit_memo_key_drops_inputs" ->
      spec \ {UseInputsInKey}
    [] candidate = CommitMemoHitReturnsCached /\
          Bug = "commit_memo_hit_revalidates" ->
      (spec \ {MemoHitReturn, ReturnCachedRoster, SkipValidation}) \cup
        {MemoMissValidate, ReturnValidatedRoster}
    [] candidate = CommitMemoMissValidates /\
          Bug = "commit_memo_miss_returns_none" ->
      (spec \ {ReturnValidatedRoster}) \cup {ReturnNone}
    [] candidate = CommitMemoInsertOnSuccess /\
          Bug = "commit_success_skips_insert" ->
      spec \ {MemoInsert}
    [] candidate = CommitValidationForwardsArgs /\
          Bug = "commit_validation_drops_chain_id" ->
      spec \ {ForwardChainId}
    [] candidate = CheckpointHashPrefilter /\
          Bug = "checkpoint_ignores_hash" ->
      spec \ {CheckBlockHash, ReturnError}
    [] candidate = CheckpointHeightPrefilter /\
          Bug = "checkpoint_ignores_height" ->
      spec \ {CheckHeight, ReturnError}
    [] candidate = CheckpointOptionalViewPrefilter /\
          Bug = "checkpoint_requires_view_when_absent" ->
      spec \ {AllowAbsentView}
    [] candidate = CheckpointEmptyAggregateBypassesMemo /\
          Bug = "checkpoint_empty_aggregate_uses_memo" ->
      (spec \ {EmptyAggregateDirectValidate, SkipMemo}) \cup
        {BuildCheckpointMemoKey, MemoGet}
    [] candidate = CheckpointMemoKeyUsesEpochModeInputs /\
          Bug = "checkpoint_memo_key_drops_epoch" ->
      spec \ {UseEpochInKey}
    [] candidate = CheckpointMemoHitReturnsCached /\
          Bug = "checkpoint_memo_hit_revalidates" ->
      (spec \ {MemoHitReturn, ReturnCachedRoster, SkipValidation}) \cup
        {MemoMissValidate, ReturnValidatedRoster}
    [] candidate = CheckpointMemoMissValidates /\
          Bug = "checkpoint_memo_miss_returns_none" ->
      (spec \ {ReturnValidatedRoster}) \cup {ReturnNone}
    [] candidate = CheckpointMemoInsertOnSuccess /\
          Bug = "checkpoint_success_skips_insert" ->
      spec \ {MemoInsert}
    [] candidate = CheckpointValidationForwardsArgs /\
          Bug = "checkpoint_validation_drops_roots" ->
      spec \ {ForwardRoots}
    [] candidate = CommitPrefilterBeforeMemo /\
          Bug = "commit_memo_before_prefilter" ->
      (spec \ {PrefilterBeforeMemo, SkipMemo}) \cup
        {MemoBeforePrefilter, MemoGet}
    [] candidate = CheckpointPrefilterBeforeMemo /\
          Bug = "checkpoint_memo_before_prefilter" ->
      (spec \ {PrefilterBeforeMemo, SkipMemo}) \cup
        {MemoBeforePrefilter, MemoGet}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "commit_ignores_hash",
       "commit_ignores_height",
       "commit_requires_view_when_absent",
       "commit_ignores_epoch",
       "commit_accepts_prepare_phase",
       "commit_accepts_highest_qc",
       "commit_ignores_mode_tag",
       "commit_empty_aggregate_uses_memo",
       "commit_memo_key_drops_inputs",
       "commit_memo_hit_revalidates",
       "commit_memo_miss_returns_none",
       "commit_success_skips_insert",
       "commit_validation_drops_chain_id",
       "checkpoint_ignores_hash",
       "checkpoint_ignores_height",
       "checkpoint_requires_view_when_absent",
       "checkpoint_empty_aggregate_uses_memo",
       "checkpoint_memo_key_drops_epoch",
       "checkpoint_memo_hit_revalidates",
       "checkpoint_memo_miss_returns_none",
       "checkpoint_success_skips_insert",
       "checkpoint_validation_drops_roots",
       "commit_memo_before_prefilter",
       "checkpoint_memo_before_prefilter"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCachedCommitPrefilterExact ==
  \A c \in CommitPrefilterCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCachedCheckpointPrefilterExact ==
  \A c \in CheckpointPrefilterCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCachedEmptyAggregateExact ==
  \A c \in EmptyAggregateCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCachedMemoKeyExact ==
  \A c \in MemoKeyCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCachedMemoFlowExact ==
  \A c \in MemoFlowCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCachedForwardingExact ==
  \A c \in ValidationForwardCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCachedPrefilterOrderExact ==
  \A c \in PrefilterOrderingCases:
    ImplementationActions(c) = SpecActions(c)

RosterValidationCachedExactness ==
  /\ RosterValidationCachedCommitPrefilterExact
  /\ RosterValidationCachedCheckpointPrefilterExact
  /\ RosterValidationCachedEmptyAggregateExact
  /\ RosterValidationCachedMemoKeyExact
  /\ RosterValidationCachedMemoFlowExact
  /\ RosterValidationCachedForwardingExact
  /\ RosterValidationCachedPrefilterOrderExact

BugCommitIgnoresHash ==
  ImplementationActions(CommitHashPrefilter) = SpecActions(CommitHashPrefilter)

BugCommitIgnoresHeight ==
  ImplementationActions(CommitHeightPrefilter) =
    SpecActions(CommitHeightPrefilter)

BugCommitRequiresViewWhenAbsent ==
  ImplementationActions(CommitOptionalViewPrefilter) =
    SpecActions(CommitOptionalViewPrefilter)

BugCommitIgnoresEpoch ==
  ImplementationActions(CommitEpochPrefilter) = SpecActions(CommitEpochPrefilter)

BugCommitAcceptsPreparePhase ==
  ImplementationActions(CommitPhasePrefilter) =
    SpecActions(CommitPhasePrefilter)

BugCommitAcceptsHighestQc ==
  ImplementationActions(CommitHighestQcPrefilter) =
    SpecActions(CommitHighestQcPrefilter)

BugCommitIgnoresModeTag ==
  ImplementationActions(CommitModeTagPrefilter) =
    SpecActions(CommitModeTagPrefilter)

BugCommitEmptyAggregateUsesMemo ==
  ImplementationActions(CommitEmptyAggregateBypassesMemo) =
    SpecActions(CommitEmptyAggregateBypassesMemo)

BugCommitMemoKeyDropsInputs ==
  ImplementationActions(CommitMemoKeyUsesModeInputs) =
    SpecActions(CommitMemoKeyUsesModeInputs)

BugCommitMemoHitRevalidates ==
  ImplementationActions(CommitMemoHitReturnsCached) =
    SpecActions(CommitMemoHitReturnsCached)

BugCommitMemoMissReturnsNone ==
  ImplementationActions(CommitMemoMissValidates) =
    SpecActions(CommitMemoMissValidates)

BugCommitSuccessSkipsInsert ==
  ImplementationActions(CommitMemoInsertOnSuccess) =
    SpecActions(CommitMemoInsertOnSuccess)

BugCommitValidationDropsChainId ==
  ImplementationActions(CommitValidationForwardsArgs) =
    SpecActions(CommitValidationForwardsArgs)

BugCheckpointIgnoresHash ==
  ImplementationActions(CheckpointHashPrefilter) =
    SpecActions(CheckpointHashPrefilter)

BugCheckpointIgnoresHeight ==
  ImplementationActions(CheckpointHeightPrefilter) =
    SpecActions(CheckpointHeightPrefilter)

BugCheckpointRequiresViewWhenAbsent ==
  ImplementationActions(CheckpointOptionalViewPrefilter) =
    SpecActions(CheckpointOptionalViewPrefilter)

BugCheckpointEmptyAggregateUsesMemo ==
  ImplementationActions(CheckpointEmptyAggregateBypassesMemo) =
    SpecActions(CheckpointEmptyAggregateBypassesMemo)

BugCheckpointMemoKeyDropsEpoch ==
  ImplementationActions(CheckpointMemoKeyUsesEpochModeInputs) =
    SpecActions(CheckpointMemoKeyUsesEpochModeInputs)

BugCheckpointMemoHitRevalidates ==
  ImplementationActions(CheckpointMemoHitReturnsCached) =
    SpecActions(CheckpointMemoHitReturnsCached)

BugCheckpointMemoMissReturnsNone ==
  ImplementationActions(CheckpointMemoMissValidates) =
    SpecActions(CheckpointMemoMissValidates)

BugCheckpointSuccessSkipsInsert ==
  ImplementationActions(CheckpointMemoInsertOnSuccess) =
    SpecActions(CheckpointMemoInsertOnSuccess)

BugCheckpointValidationDropsRoots ==
  ImplementationActions(CheckpointValidationForwardsArgs) =
    SpecActions(CheckpointValidationForwardsArgs)

BugCommitMemoBeforePrefilter ==
  ImplementationActions(CommitPrefilterBeforeMemo) =
    SpecActions(CommitPrefilterBeforeMemo)

BugCheckpointMemoBeforePrefilter ==
  ImplementationActions(CheckpointPrefilterBeforeMemo) =
    SpecActions(CheckpointPrefilterBeforeMemo)

====
