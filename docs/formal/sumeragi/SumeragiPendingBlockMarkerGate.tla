---- MODULE SumeragiPendingBlockMarkerGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for PendingBlock progress markers and cooldown gates.

This slice captures `PendingBlock::reset_commit_stage(...)`,
`note_local_commit_vote_emitted(...)`, `note_commit_qc_observed(...)`,
`local_commit_vote_emitted(...)`, `commit_qc_observed(...)`,
`reschedule_due(...)`, `vote_backed_reschedule_due(...)`,
`mark_quorum_reschedule(...)`, `mark_vote_backed_quorum_reschedule(...)`,
`precommit_rebroadcast_due(...)`, `mark_precommit_rebroadcast(...)`,
`validation_redrive_due(...)`, and `mark_validation_redrive(...)`.
The model abstracts timestamps into boundary cases while preserving the
observable contracts: commit-QC observation dominates local-vote state,
progress is touched exactly when a marker advances, cooldown boundaries are
inclusive, elapsed time is saturating, and vote-backed reschedule attempts
require both stale progress and a strictly higher vote count after a previous
vote-backed attempt.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ResetFromQc == "reset_from_qc"
LocalFromAwaiting == "local_from_awaiting"
LocalFromLocal == "local_from_local"
LocalFromQc == "local_from_qc"
QcFromAwaiting == "qc_from_awaiting"
QcFromLocal == "qc_from_local"
QcSameEpoch == "qc_same_epoch"
QcNewEpoch == "qc_new_epoch"

RescheduleNoLast == "reschedule_no_last"
RescheduleBeforeBackoff == "reschedule_before_backoff"
RescheduleAtBackoff == "reschedule_at_backoff"
RescheduleFutureLast == "reschedule_future_last"
RescheduleZeroBackoff == "reschedule_zero_backoff"

VoteNoLastStale == "vote_no_last_stale"
VoteNoLastFresh == "vote_no_last_fresh"
VoteHigherStale == "vote_higher_stale"
VoteEqualStale == "vote_equal_stale"
VoteHigherFresh == "vote_higher_fresh"
VoteZeroBackoffHigher == "vote_zero_backoff_higher"

MarkQuorum == "mark_quorum"
MarkVoteBacked == "mark_vote_backed"

PrecommitNoLast == "precommit_no_last"
PrecommitBeforeCooldown == "precommit_before_cooldown"
PrecommitAtCooldown == "precommit_at_cooldown"
PrecommitZeroCooldown == "precommit_zero_cooldown"
MarkPrecommit == "mark_precommit"

ValidationNoLast == "validation_no_last"
ValidationBeforeCooldown == "validation_before_cooldown"
ValidationAtCooldown == "validation_at_cooldown"
ValidationZeroCooldown == "validation_zero_cooldown"
MarkValidation == "mark_validation"

Cases == {
  ResetFromQc,
  LocalFromAwaiting,
  LocalFromLocal,
  LocalFromQc,
  QcFromAwaiting,
  QcFromLocal,
  QcSameEpoch,
  QcNewEpoch,
  RescheduleNoLast,
  RescheduleBeforeBackoff,
  RescheduleAtBackoff,
  RescheduleFutureLast,
  RescheduleZeroBackoff,
  VoteNoLastStale,
  VoteNoLastFresh,
  VoteHigherStale,
  VoteEqualStale,
  VoteHigherFresh,
  VoteZeroBackoffHigher,
  MarkQuorum,
  MarkVoteBacked,
  PrecommitNoLast,
  PrecommitBeforeCooldown,
  PrecommitAtCooldown,
  PrecommitZeroCooldown,
  MarkPrecommit,
  ValidationNoLast,
  ValidationBeforeCooldown,
  ValidationAtCooldown,
  ValidationZeroCooldown,
  MarkValidation
}

CommitStageMarkerCases == {
  ResetFromQc,
  LocalFromAwaiting,
  LocalFromLocal,
  LocalFromQc,
  QcFromAwaiting,
  QcFromLocal,
  QcSameEpoch,
  QcNewEpoch
}

QuorumRescheduleCooldownCases == {
  RescheduleNoLast,
  RescheduleBeforeBackoff,
  RescheduleAtBackoff,
  RescheduleFutureLast,
  RescheduleZeroBackoff
}

VoteBackedRescheduleCases == {
  VoteNoLastStale,
  VoteNoLastFresh,
  VoteHigherStale,
  VoteEqualStale,
  VoteHigherFresh,
  VoteZeroBackoffHigher
}

RescheduleMarkerCases == {MarkQuorum, MarkVoteBacked}

PrecommitRebroadcastCases == {
  PrecommitNoLast,
  PrecommitBeforeCooldown,
  PrecommitAtCooldown,
  PrecommitZeroCooldown,
  MarkPrecommit
}

ValidationRedriveCases == {
  ValidationNoLast,
  ValidationBeforeCooldown,
  ValidationAtCooldown,
  ValidationZeroCooldown,
  MarkValidation
}

StageAwaiting == 1
StageLocal == 2
StageQc == 3
EpochNone == 4
EpochZero == 5
EpochOne == 6
LocalAccessorFalse == 7
LocalAccessorTrue == 8
QcAccessorFalse == 9
QcAccessorTrue == 10
ProgressTouched == 11
ProgressUntouched == 12
DueTrue == 13
DueFalse == 14
MarkLast == 15
MarkVoteCountZero == 16
MarkVoteCountInput == 17
MarkPrecommitLast == 18
MarkValidationLast == 19

Actions == 1..19

StageResetActions ==
  {StageAwaiting, EpochNone, LocalAccessorFalse, QcAccessorFalse,
   ProgressUntouched}

LocalVoteFirstActions ==
  {StageLocal, EpochNone, LocalAccessorTrue, QcAccessorFalse,
   ProgressTouched}

LocalVoteNoopActions ==
  {StageLocal, EpochNone, LocalAccessorTrue, QcAccessorFalse,
   ProgressUntouched}

LocalVoteAfterQcActions ==
  {StageQc, EpochZero, LocalAccessorTrue, QcAccessorTrue,
   ProgressUntouched}

QcFirstActions ==
  {StageQc, EpochZero, LocalAccessorTrue, QcAccessorTrue, ProgressTouched}

QcSameActions ==
  {StageQc, EpochZero, LocalAccessorTrue, QcAccessorTrue, ProgressUntouched}

QcNewEpochActions ==
  {StageQc, EpochOne, LocalAccessorTrue, QcAccessorTrue, ProgressTouched}

SpecActions(c) ==
  CASE c = ResetFromQc -> StageResetActions
    [] c = LocalFromAwaiting -> LocalVoteFirstActions
    [] c = LocalFromLocal -> LocalVoteNoopActions
    [] c = LocalFromQc -> LocalVoteAfterQcActions
    [] c = QcFromAwaiting -> QcFirstActions
    [] c = QcFromLocal -> QcFirstActions
    [] c = QcSameEpoch -> QcSameActions
    [] c = QcNewEpoch -> QcNewEpochActions
    [] c = RescheduleNoLast -> {DueTrue}
    [] c = RescheduleBeforeBackoff -> {DueFalse}
    [] c = RescheduleAtBackoff -> {DueTrue}
    [] c = RescheduleFutureLast -> {DueFalse}
    [] c = RescheduleZeroBackoff -> {DueTrue}
    [] c = VoteNoLastStale -> {DueTrue}
    [] c = VoteNoLastFresh -> {DueFalse}
    [] c = VoteHigherStale -> {DueTrue}
    [] c = VoteEqualStale -> {DueFalse}
    [] c = VoteHigherFresh -> {DueFalse}
    [] c = VoteZeroBackoffHigher -> {DueTrue}
    [] c = MarkQuorum -> {MarkLast, MarkVoteCountZero}
    [] c = MarkVoteBacked -> {MarkLast, MarkVoteCountInput}
    [] c = PrecommitNoLast -> {DueTrue}
    [] c = PrecommitBeforeCooldown -> {DueFalse}
    [] c = PrecommitAtCooldown -> {DueTrue}
    [] c = PrecommitZeroCooldown -> {DueTrue}
    [] c = MarkPrecommit -> {MarkPrecommitLast}
    [] c = ValidationNoLast -> {DueTrue}
    [] c = ValidationBeforeCooldown -> {DueFalse}
    [] c = ValidationAtCooldown -> {DueTrue}
    [] c = ValidationZeroCooldown -> {DueTrue}
    [] c = MarkValidation -> {MarkValidationLast}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "local_vote_skips_first_progress"
       /\ c = LocalFromAwaiting ->
      (spec \ {ProgressTouched}) \cup {ProgressUntouched}
    [] Bug = "local_vote_advances_from_qc"
       /\ c = LocalFromQc ->
      (spec \ {StageQc, QcAccessorTrue}) \cup {StageLocal, QcAccessorFalse}
    [] Bug = "local_vote_retouches_local"
       /\ c = LocalFromLocal ->
      (spec \ {ProgressUntouched}) \cup {ProgressTouched}
    [] Bug = "qc_does_not_set_stage"
       /\ c \in {QcFromAwaiting, QcFromLocal} ->
      (spec \ {StageQc, QcAccessorTrue}) \cup {StageLocal, QcAccessorFalse}
    [] Bug = "qc_skips_first_progress"
       /\ c \in {QcFromAwaiting, QcFromLocal} ->
      (spec \ {ProgressTouched}) \cup {ProgressUntouched}
    [] Bug = "qc_retouches_same_epoch"
       /\ c = QcSameEpoch ->
      (spec \ {ProgressUntouched}) \cup {ProgressTouched}
    [] Bug = "qc_keeps_old_epoch"
       /\ c = QcNewEpoch ->
      (spec \ {EpochOne}) \cup {EpochZero}
    [] Bug = "reset_keeps_epoch"
       /\ c = ResetFromQc ->
      (spec \ {EpochNone}) \cup {EpochZero}
    [] Bug = "reset_keeps_local_stage"
       /\ c = ResetFromQc ->
      (spec \ {StageAwaiting, LocalAccessorFalse}) \cup {StageLocal,
        LocalAccessorTrue}
    [] Bug = "reschedule_requires_existing_last"
       /\ c = RescheduleNoLast ->
      {DueFalse}
    [] Bug = "reschedule_strict_boundary"
       /\ c = RescheduleAtBackoff ->
      {DueFalse}
    [] Bug = "reschedule_allows_before_backoff"
       /\ c = RescheduleBeforeBackoff ->
      {DueTrue}
    [] Bug = "reschedule_future_underflows"
       /\ c = RescheduleFutureLast ->
      {DueTrue}
    [] Bug = "vote_backed_ignores_progress_age"
       /\ c = VoteHigherFresh ->
      {DueTrue}
    [] Bug = "vote_backed_allows_equal_votes"
       /\ c = VoteEqualStale ->
      {DueTrue}
    [] Bug = "vote_backed_ignores_vote_progress"
       /\ c = VoteHigherStale ->
      {DueFalse}
    [] Bug = "vote_backed_requires_last"
       /\ c = VoteNoLastStale ->
      {DueFalse}
    [] Bug = "mark_quorum_keeps_vote_count"
       /\ c = MarkQuorum ->
      {MarkLast, MarkVoteCountInput}
    [] Bug = "mark_vote_backed_drops_vote_count"
       /\ c = MarkVoteBacked ->
      {MarkLast, MarkVoteCountZero}
    [] Bug = "precommit_requires_existing_last"
       /\ c = PrecommitNoLast ->
      {DueFalse}
    [] Bug = "precommit_strict_boundary"
       /\ c = PrecommitAtCooldown ->
      {DueFalse}
    [] Bug = "precommit_allows_before_cooldown"
       /\ c = PrecommitBeforeCooldown ->
      {DueTrue}
    [] Bug = "validation_requires_existing_last"
       /\ c = ValidationNoLast ->
      {DueFalse}
    [] Bug = "validation_strict_boundary"
       /\ c = ValidationAtCooldown ->
      {DueFalse}
    [] Bug = "validation_allows_before_cooldown"
       /\ c = ValidationBeforeCooldown ->
      {DueTrue}
    [] Bug = "mark_precommit_not_recorded"
       /\ c = MarkPrecommit ->
      {}
    [] Bug = "mark_validation_not_recorded"
       /\ c = MarkValidation ->
      {}
    [] OTHER -> spec

Bugs == {
  "none",
  "local_vote_skips_first_progress",
  "local_vote_advances_from_qc",
  "local_vote_retouches_local",
  "qc_does_not_set_stage",
  "qc_skips_first_progress",
  "qc_retouches_same_epoch",
  "qc_keeps_old_epoch",
  "reset_keeps_epoch",
  "reset_keeps_local_stage",
  "reschedule_requires_existing_last",
  "reschedule_strict_boundary",
  "reschedule_allows_before_backoff",
  "reschedule_future_underflows",
  "vote_backed_ignores_progress_age",
  "vote_backed_allows_equal_votes",
  "vote_backed_ignores_vote_progress",
  "vote_backed_requires_last",
  "mark_quorum_keeps_vote_count",
  "mark_vote_backed_drops_vote_count",
  "precommit_requires_existing_last",
  "precommit_strict_boundary",
  "precommit_allows_before_cooldown",
  "validation_requires_existing_last",
  "validation_strict_boundary",
  "validation_allows_before_cooldown",
  "mark_precommit_not_recorded",
  "mark_validation_not_recorded"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

PendingBlockCommitStageMarkerExact ==
  \A c \in CommitStageMarkerCases:
    ImplementationActions(c) = SpecActions(c)

PendingBlockQuorumRescheduleCooldownExact ==
  \A c \in QuorumRescheduleCooldownCases:
    ImplementationActions(c) = SpecActions(c)

PendingBlockVoteBackedRescheduleExact ==
  \A c \in VoteBackedRescheduleCases:
    ImplementationActions(c) = SpecActions(c)

PendingBlockRescheduleMarkerExact ==
  \A c \in RescheduleMarkerCases:
    ImplementationActions(c) = SpecActions(c)

PendingBlockPrecommitRebroadcastExact ==
  \A c \in PrecommitRebroadcastCases:
    ImplementationActions(c) = SpecActions(c)

PendingBlockValidationRedriveExact ==
  \A c \in ValidationRedriveCases:
    ImplementationActions(c) = SpecActions(c)

PendingBlockMarkerCooldownExactness ==
  /\ PendingBlockCommitStageMarkerExact
  /\ PendingBlockQuorumRescheduleCooldownExact
  /\ PendingBlockVoteBackedRescheduleExact
  /\ PendingBlockRescheduleMarkerExact
  /\ PendingBlockPrecommitRebroadcastExact
  /\ PendingBlockValidationRedriveExact

NoBugInvariant ==
  PendingBlockMarkerCooldownExactness

SafetyFast == NoBugInvariant

BugLocalVoteSkipsFirstProgress == NoBugInvariant
BugLocalVoteAdvancesFromQc == NoBugInvariant
BugLocalVoteRetouchesLocal == NoBugInvariant
BugQcDoesNotSetStage == NoBugInvariant
BugQcSkipsFirstProgress == NoBugInvariant
BugQcRetouchesSameEpoch == NoBugInvariant
BugQcKeepsOldEpoch == NoBugInvariant
BugResetKeepsEpoch == NoBugInvariant
BugResetKeepsLocalStage == NoBugInvariant
BugRescheduleRequiresExistingLast == NoBugInvariant
BugRescheduleStrictBoundary == NoBugInvariant
BugRescheduleAllowsBeforeBackoff == NoBugInvariant
BugRescheduleFutureUnderflows == NoBugInvariant
BugVoteBackedIgnoresProgressAge == NoBugInvariant
BugVoteBackedAllowsEqualVotes == NoBugInvariant
BugVoteBackedIgnoresVoteProgress == NoBugInvariant
BugVoteBackedRequiresLast == NoBugInvariant
BugMarkQuorumKeepsVoteCount == NoBugInvariant
BugMarkVoteBackedDropsVoteCount == NoBugInvariant
BugPrecommitRequiresExistingLast == NoBugInvariant
BugPrecommitStrictBoundary == NoBugInvariant
BugPrecommitAllowsBeforeCooldown == NoBugInvariant
BugValidationRequiresExistingLast == NoBugInvariant
BugValidationStrictBoundary == NoBugInvariant
BugValidationAllowsBeforeCooldown == NoBugInvariant
BugMarkPrecommitNotRecorded == NoBugInvariant
BugMarkValidationNotRecorded == NoBugInvariant

====
