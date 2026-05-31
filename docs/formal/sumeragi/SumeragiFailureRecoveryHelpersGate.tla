---- MODULE SumeragiFailureRecoveryHelpersGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for Sumeragi failed-commit and block-sync helper
semantics.

This slice pins:
`realign_qcs_after_failed_commit(...)`,
`drop_pending_after_requeue(...)`,
`view_change_cause_for_quorum(...)`,
`block_sync_ready_for_qc(...)`, and
`block_sync_apply_qc_after_block(...)`.

QC references are represented by their subject labels. The label `none`
stands for `Option::None`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

None == "none"
Failed == "failed"
Committed == "committed"
Other == "other"

Ok == "ok"
Err == "err"

MissingQc == "missing_qc"
QuorumTimeout == "quorum_timeout"
StakeQuorumTimeout == "stake_quorum_timeout"

\* @type: (Str, Str) => <<Str, Str>>;
PairStr(a, b) == <<a, b>>

\* @type: (Str, Bool) => <<Str, Bool>>;
ApplyResult(result, applied) == <<result, applied>>

RealignCases == {
  "realign_both_failed_with_committed",
  "realign_locked_failed_only",
  "realign_highest_failed_only",
  "realign_unrelated",
  "realign_failed_no_committed",
  "realign_none_inputs"
}

LockedIn(c) ==
  CASE c = "realign_both_failed_with_committed" -> Failed
    [] c = "realign_locked_failed_only" -> Failed
    [] c = "realign_highest_failed_only" -> Other
    [] c = "realign_unrelated" -> Other
    [] c = "realign_failed_no_committed" -> Failed
    [] OTHER -> None

HighestIn(c) ==
  CASE c = "realign_both_failed_with_committed" -> Failed
    [] c = "realign_locked_failed_only" -> Other
    [] c = "realign_highest_failed_only" -> Failed
    [] c = "realign_unrelated" -> Other
    [] c = "realign_failed_no_committed" -> Failed
    [] OTHER -> None

LatestCommitted(c) ==
  IF c = "realign_failed_no_committed" THEN None ELSE Committed

RealignOne(input, latest) ==
  IF input = Failed /\ latest # None THEN latest ELSE input

\* @type: (Str) => <<Str, Str>>;
SpecRealign(c) ==
  PairStr(
    RealignOne(LockedIn(c), LatestCommitted(c)),
    RealignOne(HighestIn(c), LatestCommitted(c))
  )

\* @type: (Str) => <<Str, Str>>;
ActualRealign(c) ==
  CASE Bug = "realign_skip_locked_replacement"
       /\ c = "realign_both_failed_with_committed" ->
       PairStr(LockedIn(c), RealignOne(HighestIn(c), LatestCommitted(c)))
    [] Bug = "realign_skip_highest_replacement"
       /\ c = "realign_both_failed_with_committed" ->
       PairStr(RealignOne(LockedIn(c), LatestCommitted(c)), HighestIn(c))
    [] Bug = "realign_replace_unrelated_locked"
       /\ c = "realign_unrelated" ->
       PairStr(LatestCommitted(c), HighestIn(c))
    [] Bug = "realign_replace_unrelated_highest"
       /\ c = "realign_unrelated" ->
       PairStr(LockedIn(c), LatestCommitted(c))
    [] Bug = "realign_drop_failed_without_committed"
       /\ c = "realign_failed_no_committed" ->
       PairStr(None, None)
    [] Bug = "realign_use_latest_when_input_none"
       /\ c = "realign_none_inputs" ->
       PairStr(LatestCommitted(c), LatestCommitted(c))
    [] OTHER -> SpecRealign(c)

DropCases == {
  "drop_no_failures",
  "drop_failures",
  "drop_duplicates_only"
}

Failures(c) ==
  IF c = "drop_failures" THEN 1 ELSE 0

DuplicateFailures(c) ==
  IF c = "drop_duplicates_only" THEN 2 ELSE 0

SpecDropPending(c) ==
  Failures(c) > 0

ActualDropPending(c) ==
  CASE Bug = "drop_duplicates_trigger"
       /\ c = "drop_duplicates_only" -> DuplicateFailures(c) > 0
    [] Bug = "drop_ignores_failures"
       /\ c = "drop_failures" -> FALSE
    [] OTHER -> SpecDropPending(c)

ViewCauseCases == {
  "view_no_votes",
  "view_no_votes_stake_missing",
  "view_votes",
  "view_votes_stake_missing"
}

VoteCount(c) ==
  IF c \in {"view_no_votes", "view_no_votes_stake_missing"} THEN 0 ELSE 1

StakeMissing(c) ==
  c \in {"view_no_votes_stake_missing", "view_votes_stake_missing"}

SpecViewCause(c) ==
  IF VoteCount(c) = 0 THEN
    MissingQc
  ELSE IF StakeMissing(c) THEN
    StakeQuorumTimeout
  ELSE
    QuorumTimeout

ActualViewCause(c) ==
  CASE Bug = "view_stake_before_missing"
       /\ c = "view_no_votes_stake_missing" -> StakeQuorumTimeout
    [] Bug = "view_zero_as_quorum"
       /\ c = "view_no_votes" -> QuorumTimeout
    [] Bug = "view_ignore_stake"
       /\ c = "view_votes_stake_missing" -> QuorumTimeout
    [] OTHER -> SpecViewCause(c)

ReadyCases == {
  "ready_known_ok",
  "ready_unknown_ok",
  "ready_known_error",
  "ready_unknown_error"
}

ReadyKnown(c) ==
  c \in {"ready_known_ok", "ready_known_error"}

ReadyCreationOk(c) ==
  c \in {"ready_known_ok", "ready_unknown_ok"}

SpecReady(c) ==
  ReadyCreationOk(c) /\ ReadyKnown(c)

ActualReady(c) ==
  CASE Bug = "ready_ignores_known"
       /\ c = "ready_unknown_ok" -> ReadyCreationOk(c)
    [] Bug = "ready_ignores_creation_error"
       /\ c = "ready_known_error" -> ReadyKnown(c)
    [] Bug = "ready_requires_unknown"
       /\ c = "ready_known_ok" -> FALSE
    [] OTHER -> SpecReady(c)

ApplyCases == {
  "apply_known_ok_qc_ok",
  "apply_unknown_ok_qc_ok",
  "apply_ok_no_qc",
  "apply_creation_error",
  "apply_qc_error"
}

ApplyCreationOk(c) ==
  c # "apply_creation_error"

ApplyKnown(c) ==
  c = "apply_known_ok_qc_ok"

ApplyQcPresent(c) ==
  c # "apply_ok_no_qc" /\ c # "apply_creation_error"

ApplyCallbackOk(c) ==
  c # "apply_qc_error"

\* @type: (Str) => <<Str, Bool>>;
SpecApply(c) ==
  IF ~ApplyCreationOk(c) THEN
    ApplyResult(Err, FALSE)
  ELSE IF ApplyQcPresent(c) THEN
    ApplyResult(IF ApplyCallbackOk(c) THEN Ok ELSE Err, TRUE)
  ELSE
    ApplyResult(Ok, FALSE)

\* @type: (Str) => <<Str, Bool>>;
ActualApply(c) ==
  CASE Bug = "apply_runs_on_creation_error"
       /\ c = "apply_creation_error" -> ApplyResult(Err, TRUE)
    [] Bug = "apply_skips_unknown_block"
       /\ c = "apply_unknown_ok_qc_ok" -> ApplyResult(Ok, FALSE)
    [] Bug = "apply_requires_known_block"
       /\ c = "apply_unknown_ok_qc_ok" -> ApplyResult(Err, FALSE)
    [] Bug = "apply_requires_qc"
       /\ c = "apply_ok_no_qc" -> ApplyResult(Err, FALSE)
    [] Bug = "apply_ignores_apply_error"
       /\ c = "apply_qc_error" -> ApplyResult(Ok, TRUE)
    [] Bug = "apply_skips_present_qc"
       /\ c = "apply_known_ok_qc_ok" -> ApplyResult(Ok, FALSE)
    [] OTHER -> SpecApply(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "realign_skip_locked_replacement",
       "realign_skip_highest_replacement",
       "realign_replace_unrelated_locked",
       "realign_replace_unrelated_highest",
       "realign_drop_failed_without_committed",
       "realign_use_latest_when_input_none",
       "drop_duplicates_trigger",
       "drop_ignores_failures",
       "view_stake_before_missing",
       "view_zero_as_quorum",
       "view_ignore_stake",
       "ready_ignores_known",
       "ready_ignores_creation_error",
       "ready_requires_unknown",
       "apply_runs_on_creation_error",
       "apply_skips_unknown_block",
       "apply_requires_known_block",
       "apply_requires_qc",
       "apply_ignores_apply_error",
       "apply_skips_present_qc"
     }
  /\ checked = 0

RealignMatchesSpec ==
  /\ \A c \in RealignCases:
       ActualRealign(c) = SpecRealign(c)

DropPendingMatchesSpec ==
  /\ \A c \in DropCases:
       ActualDropPending(c) = SpecDropPending(c)

ViewCauseMatchesSpec ==
  /\ \A c \in ViewCauseCases:
       ActualViewCause(c) = SpecViewCause(c)

BlockSyncReadyMatchesSpec ==
  /\ \A c \in ReadyCases:
       ActualReady(c) = SpecReady(c)

ApplyAfterBlockMatchesSpec ==
  /\ \A c \in ApplyCases:
       ActualApply(c) = SpecApply(c)

RealignPreservesUnrelatedAndAbsent ==
  /\ ActualRealign("realign_unrelated") = PairStr(Other, Other)
  /\ ActualRealign("realign_none_inputs") = PairStr(None, None)

DropPendingRequiresRealRequeueFailure ==
  \A c \in DropCases:
    ActualDropPending(c) => Failures(c) > 0

ViewCauseMissingVotesPriority ==
  \A c \in ViewCauseCases:
    VoteCount(c) = 0 => ActualViewCause(c) = MissingQc

ReadyRequiresKnownBlockAndSuccessfulCreation ==
  \A c \in ReadyCases:
    ActualReady(c) =>
      /\ ReadyCreationOk(c)
      /\ ReadyKnown(c)

ApplyNeverRunsAfterCreationError ==
  \A c \in ApplyCases:
    ~ApplyCreationOk(c) => ActualApply(c)[2] = FALSE

ApplyPresentQcInvokesCallback ==
  \A c \in ApplyCases:
    /\ ApplyCreationOk(c)
    /\ ApplyQcPresent(c)
    => ActualApply(c)[2] = TRUE

RealignAnchors ==
  /\ SpecRealign("realign_both_failed_with_committed") =
       PairStr(Committed, Committed)
  /\ SpecRealign("realign_locked_failed_only") =
       PairStr(Committed, Other)
  /\ SpecRealign("realign_highest_failed_only") =
       PairStr(Other, Committed)
  /\ SpecRealign("realign_unrelated") = PairStr(Other, Other)
  /\ SpecRealign("realign_failed_no_committed") =
       PairStr(Failed, Failed)
  /\ SpecRealign("realign_none_inputs") = PairStr(None, None)

DropPendingAnchors ==
  /\ SpecDropPending("drop_no_failures") = FALSE
  /\ SpecDropPending("drop_failures") = TRUE
  /\ SpecDropPending("drop_duplicates_only") = FALSE

ViewCauseAnchors ==
  /\ SpecViewCause("view_no_votes") = MissingQc
  /\ SpecViewCause("view_no_votes_stake_missing") = MissingQc
  /\ SpecViewCause("view_votes") = QuorumTimeout
  /\ SpecViewCause("view_votes_stake_missing") = StakeQuorumTimeout

ReadyAnchors ==
  /\ SpecReady("ready_known_ok") = TRUE
  /\ SpecReady("ready_unknown_ok") = FALSE
  /\ SpecReady("ready_known_error") = FALSE
  /\ SpecReady("ready_unknown_error") = FALSE

ApplyAnchors ==
  /\ SpecApply("apply_known_ok_qc_ok") = ApplyResult(Ok, TRUE)
  /\ SpecApply("apply_unknown_ok_qc_ok") = ApplyResult(Ok, TRUE)
  /\ SpecApply("apply_ok_no_qc") = ApplyResult(Ok, FALSE)
  /\ SpecApply("apply_creation_error") = ApplyResult(Err, FALSE)
  /\ SpecApply("apply_qc_error") = ApplyResult(Err, TRUE)

SafetyFast ==
  /\ RealignMatchesSpec
  /\ DropPendingMatchesSpec
  /\ ViewCauseMatchesSpec
  /\ BlockSyncReadyMatchesSpec
  /\ ApplyAfterBlockMatchesSpec
  /\ RealignPreservesUnrelatedAndAbsent
  /\ DropPendingRequiresRealRequeueFailure
  /\ ViewCauseMissingVotesPriority
  /\ ReadyRequiresKnownBlockAndSuccessfulCreation
  /\ ApplyNeverRunsAfterCreationError
  /\ ApplyPresentQcInvokesCallback
  /\ RealignAnchors
  /\ DropPendingAnchors
  /\ ViewCauseAnchors
  /\ ReadyAnchors
  /\ ApplyAnchors

BugRealignSkipLockedReplacement ==
  ActualRealign("realign_both_failed_with_committed") =
    SpecRealign("realign_both_failed_with_committed")

BugRealignSkipHighestReplacement ==
  ActualRealign("realign_both_failed_with_committed") =
    SpecRealign("realign_both_failed_with_committed")

BugRealignReplaceUnrelatedLocked ==
  ActualRealign("realign_unrelated") = SpecRealign("realign_unrelated")

BugRealignReplaceUnrelatedHighest ==
  ActualRealign("realign_unrelated") = SpecRealign("realign_unrelated")

BugRealignDropFailedWithoutCommitted ==
  ActualRealign("realign_failed_no_committed") =
    SpecRealign("realign_failed_no_committed")

BugRealignUseLatestWhenInputNone ==
  ActualRealign("realign_none_inputs") = SpecRealign("realign_none_inputs")

BugDropDuplicatesTrigger ==
  ActualDropPending("drop_duplicates_only") =
    SpecDropPending("drop_duplicates_only")

BugDropIgnoresFailures ==
  ActualDropPending("drop_failures") = SpecDropPending("drop_failures")

BugViewStakeBeforeMissing ==
  ActualViewCause("view_no_votes_stake_missing") =
    SpecViewCause("view_no_votes_stake_missing")

BugViewZeroAsQuorum ==
  ActualViewCause("view_no_votes") = SpecViewCause("view_no_votes")

BugViewIgnoreStake ==
  ActualViewCause("view_votes_stake_missing") =
    SpecViewCause("view_votes_stake_missing")

BugReadyIgnoresKnown ==
  ActualReady("ready_unknown_ok") = SpecReady("ready_unknown_ok")

BugReadyIgnoresCreationError ==
  ActualReady("ready_known_error") = SpecReady("ready_known_error")

BugReadyRequiresUnknown ==
  ActualReady("ready_known_ok") = SpecReady("ready_known_ok")

BugApplyRunsOnCreationError ==
  ActualApply("apply_creation_error") = SpecApply("apply_creation_error")

BugApplySkipsUnknownBlock ==
  ActualApply("apply_unknown_ok_qc_ok") = SpecApply("apply_unknown_ok_qc_ok")

BugApplyRequiresKnownBlock ==
  ActualApply("apply_unknown_ok_qc_ok") = SpecApply("apply_unknown_ok_qc_ok")

BugApplyRequiresQc ==
  ActualApply("apply_ok_no_qc") = SpecApply("apply_ok_no_qc")

BugApplyIgnoresApplyError ==
  ActualApply("apply_qc_error") = SpecApply("apply_qc_error")

BugApplySkipsPresentQc ==
  ActualApply("apply_known_ok_qc_ok") = SpecApply("apply_known_ok_qc_ok")

====
