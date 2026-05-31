---- MODULE SumeragiBlockingPendingBlocksGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for blocking pending-block counters.

This slice captures `blocking_pending_blocks_len()`,
`blocking_pending_blocks_len_with_progress(...)`, and the boolean
`has_blocking_pending_blocks()` wrapper. The classic counter counts only
active-for-tip pending blocks; observed commit QC always blocks, while
non-QC entries stop blocking after a quorum reschedule or a fast-unblock due
decision. The progress-aware counter falls back to the classic counter when
quorum timeout is zero; otherwise it filters aborted and off-tip blocks, counts
vote/QC-backed blocks immediately, and counts no-evidence blocks only inside
the inclusive-lower/exclusive-upper stall window.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ClassicInactive == "classic_inactive"
ClassicCommitQcRescheduled == "classic_commit_qc_rescheduled"
ClassicNoRescheduleNotFastDue == "classic_no_reschedule_not_fast_due"
ClassicNoRescheduleFastDue == "classic_no_reschedule_fast_due"
ClassicRescheduledNoCommitQc == "classic_rescheduled_no_commit_qc"
ClassicCachedQcNoReschedule == "classic_cached_qc_no_reschedule"

ProgressQuorumZeroClassicBlocking == "progress_quorum_zero_classic_blocking"
ProgressQuorumZeroClassicNonBlocking == "progress_quorum_zero_classic_nonblocking"
ProgressAbortedWithVotes == "progress_aborted_with_votes"
ProgressOffTipWithVotes == "progress_off_tip_with_votes"
ProgressPrecommitVotes == "progress_precommit_votes"
ProgressCommitQc == "progress_commit_qc"
ProgressRescheduledNoEvidenceInWindow == "progress_rescheduled_no_evidence_in_window"
ProgressAgeUnderGrace == "progress_age_under_grace"
ProgressAgeAtGrace == "progress_age_at_grace"
ProgressAgePastGrace == "progress_age_past_grace"
ProgressAgeAtQuorum == "progress_age_at_quorum"
ProgressAgePastQuorum == "progress_age_past_quorum"
ProgressGraceClampedNoWindow == "progress_grace_clamped_no_window"
ProgressAuthoritativeOnly == "progress_authoritative_only"
ProgressConsensusInactiveVoteBacked == "progress_consensus_inactive_vote_backed"

ClassicCases == {
  ClassicInactive,
  ClassicCommitQcRescheduled,
  ClassicNoRescheduleNotFastDue,
  ClassicNoRescheduleFastDue,
  ClassicRescheduledNoCommitQc,
  ClassicCachedQcNoReschedule
}

ProgressCases == {
  ProgressQuorumZeroClassicBlocking,
  ProgressQuorumZeroClassicNonBlocking,
  ProgressAbortedWithVotes,
  ProgressOffTipWithVotes,
  ProgressPrecommitVotes,
  ProgressCommitQc,
  ProgressRescheduledNoEvidenceInWindow,
  ProgressAgeUnderGrace,
  ProgressAgeAtGrace,
  ProgressAgePastGrace,
  ProgressAgeAtQuorum,
  ProgressAgePastQuorum,
  ProgressGraceClampedNoWindow,
  ProgressAuthoritativeOnly,
  ProgressConsensusInactiveVoteBacked
}

Cases == ClassicCases \cup ProgressCases

ClassicActiveCases == {
  ClassicCommitQcRescheduled,
  ClassicNoRescheduleNotFastDue,
  ClassicNoRescheduleFastDue,
  ClassicRescheduledNoCommitQc,
  ClassicCachedQcNoReschedule,
  ProgressQuorumZeroClassicBlocking,
  ProgressQuorumZeroClassicNonBlocking
}

ClassicCommitQcObservedCases == {
  ClassicCommitQcRescheduled
}

ClassicRescheduledCases == {
  ClassicCommitQcRescheduled,
  ClassicRescheduledNoCommitQc
}

ClassicFastUnblockDueCases == {
  ClassicNoRescheduleFastDue,
  ProgressQuorumZeroClassicNonBlocking
}

ProgressQuorumZeroCases == {
  ProgressQuorumZeroClassicBlocking,
  ProgressQuorumZeroClassicNonBlocking
}

ProgressNonzeroCases == ProgressCases \ ProgressQuorumZeroCases

ProgressAbortedCases == {ProgressAbortedWithVotes}

ProgressTipExtendingCases == ProgressCases \ {ProgressOffTipWithVotes}

ProgressPrecommitVoteCases == {
  ProgressAbortedWithVotes,
  ProgressOffTipWithVotes,
  ProgressPrecommitVotes,
  ProgressConsensusInactiveVoteBacked
}

ProgressCommitQcCases == {
  ProgressCommitQc
}

ProgressRescheduledCases == {
  ProgressRescheduledNoEvidenceInWindow
}

ProgressAgeWindowCases == {
  ProgressAgeAtGrace,
  ProgressAgePastGrace
}

ProgressAgeUnderGraceCases == {
  ProgressAgeUnderGrace,
  ProgressAuthoritativeOnly
}

ProgressAgeQuorumReachedCases == {
  ProgressAgeAtQuorum,
  ProgressAgePastQuorum
}

SpecClassicBlocks(c) ==
  c \in ClassicActiveCases
    /\ (c \in ClassicCommitQcObservedCases
        \/ (c \notin ClassicRescheduledCases
            /\ c \notin ClassicFastUnblockDueCases))

SpecProgressBlocks(c) ==
  IF c \in ProgressQuorumZeroCases THEN
    SpecClassicBlocks(c)
  ELSE
    /\ c \in ProgressNonzeroCases
    /\ c \notin ProgressAbortedCases
    /\ c \in ProgressTipExtendingCases
    /\ (c \in ProgressPrecommitVoteCases
        \/ c \in ProgressCommitQcCases
        \/ (c \notin ProgressRescheduledCases
            /\ c \in ProgressAgeWindowCases))

SpecClassicCount(c) ==
  IF SpecClassicBlocks(c) THEN 1 ELSE 0

SpecProgressCount(c) ==
  IF SpecProgressBlocks(c) THEN 1 ELSE 0

ClassicReturnBlocking == 1
ClassicReturnNonBlocking == 2
ClassicCheckActive == 3
ClassicRejectInactive == 4
ClassicCheckCommitQcObserved == 5
ClassicCommitQcBlocks == 6
ClassicCheckReschedule == 7
ClassicRescheduleReject == 8
ClassicCheckFastUnblock == 9
ClassicFastUnblockReject == 10
ClassicNoFastUnblockBlocks == 11

ProgressReturnBlocking == 12
ProgressReturnNonBlocking == 13
ProgressCheckQuorumTimeout == 14
ProgressFallbackClassic == 15
ProgressCheckAborted == 16
ProgressRejectAborted == 17
ProgressCheckTip == 18
ProgressRejectOffTip == 19
ProgressCheckPrecommitVotes == 20
ProgressPrecommitVotesBlock == 21
ProgressCheckCommitQc == 22
ProgressCommitQcBlocks == 23
ProgressCheckReschedule == 24
ProgressRescheduleReject == 25
ProgressCheckAge == 26
ProgressAgeWindowBlocks == 27
ProgressAgeUnderGraceReject == 28
ProgressAgeQuorumReject == 29
ProgressGraceClampedReject == 30

ActionUniverse == 1..30

SpecClassicActions(c) ==
  {ClassicCheckActive}
    \cup (IF SpecClassicBlocks(c)
          THEN {ClassicReturnBlocking}
          ELSE {ClassicReturnNonBlocking})
    \cup (IF c \notin ClassicActiveCases
          THEN {ClassicRejectInactive}
          ELSE {ClassicCheckCommitQcObserved})
    \cup (IF c \in ClassicActiveCases
              /\ c \in ClassicCommitQcObservedCases
          THEN {ClassicCommitQcBlocks}
          ELSE {})
    \cup (IF c \in ClassicActiveCases
              /\ c \notin ClassicCommitQcObservedCases
          THEN {ClassicCheckReschedule}
          ELSE {})
    \cup (IF c \in ClassicActiveCases
              /\ c \notin ClassicCommitQcObservedCases
              /\ c \in ClassicRescheduledCases
          THEN {ClassicRescheduleReject}
          ELSE {})
    \cup (IF c \in ClassicActiveCases
              /\ c \notin ClassicCommitQcObservedCases
              /\ c \notin ClassicRescheduledCases
          THEN {ClassicCheckFastUnblock}
          ELSE {})
    \cup (IF c \in ClassicActiveCases
              /\ c \notin ClassicCommitQcObservedCases
              /\ c \notin ClassicRescheduledCases
              /\ c \in ClassicFastUnblockDueCases
          THEN {ClassicFastUnblockReject}
          ELSE {})
    \cup (IF c \in ClassicActiveCases
              /\ c \notin ClassicCommitQcObservedCases
              /\ c \notin ClassicRescheduledCases
              /\ c \notin ClassicFastUnblockDueCases
          THEN {ClassicNoFastUnblockBlocks}
          ELSE {})

SpecProgressActions(c) ==
  {ProgressCheckQuorumTimeout}
    \cup (IF SpecProgressBlocks(c)
          THEN {ProgressReturnBlocking}
          ELSE {ProgressReturnNonBlocking})
    \cup (IF c \in ProgressQuorumZeroCases
          THEN {ProgressFallbackClassic}
          ELSE {ProgressCheckAborted})
    \cup (IF c \in ProgressNonzeroCases /\ c \in ProgressAbortedCases
          THEN {ProgressRejectAborted}
          ELSE {})
    \cup (IF c \in ProgressNonzeroCases /\ c \notin ProgressAbortedCases
          THEN {ProgressCheckTip}
          ELSE {})
    \cup (IF c \in ProgressNonzeroCases
              /\ c \notin ProgressAbortedCases
              /\ c \notin ProgressTipExtendingCases
          THEN {ProgressRejectOffTip}
          ELSE {})
    \cup (IF c \in ProgressNonzeroCases
              /\ c \notin ProgressAbortedCases
              /\ c \in ProgressTipExtendingCases
          THEN {ProgressCheckPrecommitVotes}
          ELSE {})
    \cup (IF c \in ProgressNonzeroCases
              /\ c \notin ProgressAbortedCases
              /\ c \in ProgressTipExtendingCases
              /\ c \in ProgressPrecommitVoteCases
          THEN {ProgressPrecommitVotesBlock}
          ELSE {})
    \cup (IF c \in ProgressNonzeroCases
              /\ c \notin ProgressAbortedCases
              /\ c \in ProgressTipExtendingCases
              /\ c \notin ProgressPrecommitVoteCases
          THEN {ProgressCheckCommitQc}
          ELSE {})
    \cup (IF c \in ProgressNonzeroCases
              /\ c \notin ProgressAbortedCases
              /\ c \in ProgressTipExtendingCases
              /\ c \notin ProgressPrecommitVoteCases
              /\ c \in ProgressCommitQcCases
          THEN {ProgressCommitQcBlocks}
          ELSE {})
    \cup (IF c \in ProgressNonzeroCases
              /\ c \notin ProgressAbortedCases
              /\ c \in ProgressTipExtendingCases
              /\ c \notin ProgressPrecommitVoteCases
              /\ c \notin ProgressCommitQcCases
          THEN {ProgressCheckReschedule}
          ELSE {})
    \cup (IF c \in ProgressNonzeroCases
              /\ c \notin ProgressAbortedCases
              /\ c \in ProgressTipExtendingCases
              /\ c \notin ProgressPrecommitVoteCases
              /\ c \notin ProgressCommitQcCases
              /\ c \in ProgressRescheduledCases
          THEN {ProgressRescheduleReject}
          ELSE {})
    \cup (IF c \in ProgressNonzeroCases
              /\ c \notin ProgressAbortedCases
              /\ c \in ProgressTipExtendingCases
              /\ c \notin ProgressPrecommitVoteCases
              /\ c \notin ProgressCommitQcCases
              /\ c \notin ProgressRescheduledCases
          THEN {ProgressCheckAge}
          ELSE {})
    \cup (IF c \in ProgressAgeWindowCases
          THEN {ProgressAgeWindowBlocks}
          ELSE {})
    \cup (IF c \in ProgressAgeUnderGraceCases
          THEN {ProgressAgeUnderGraceReject}
          ELSE {})
    \cup (IF c \in ProgressAgeQuorumReachedCases
          THEN {ProgressAgeQuorumReject}
          ELSE {})
    \cup (IF c = ProgressGraceClampedNoWindow
          THEN {ProgressGraceClampedReject}
          ELSE {})

ImplementationClassicBlocks(c) ==
  CASE Bug = "classic_counts_inactive"
       /\ c = ClassicInactive ->
      TRUE
    [] Bug = "classic_reschedule_blocks_commit_qc"
       /\ c = ClassicCommitQcRescheduled ->
      FALSE
    [] Bug = "classic_drops_not_fast_due"
       /\ c = ClassicNoRescheduleNotFastDue ->
      FALSE
    [] Bug = "classic_counts_fast_due"
       /\ c = ClassicNoRescheduleFastDue ->
      TRUE
    [] Bug = "classic_counts_rescheduled_without_qc"
       /\ c = ClassicRescheduledNoCommitQc ->
      TRUE
    [] Bug = "classic_treats_cached_qc_as_fast_due"
       /\ c = ClassicCachedQcNoReschedule ->
      FALSE
    [] OTHER -> SpecClassicBlocks(c)

ImplementationProgressBlocks(c) ==
  CASE Bug = "progress_ignores_quorum_zero_fallback"
       /\ c = ProgressQuorumZeroClassicBlocking ->
      FALSE
    [] Bug = "progress_counts_quorum_zero_nonblocking"
       /\ c = ProgressQuorumZeroClassicNonBlocking ->
      TRUE
    [] Bug = "progress_counts_aborted"
       /\ c = ProgressAbortedWithVotes ->
      TRUE
    [] Bug = "progress_counts_off_tip"
       /\ c = ProgressOffTipWithVotes ->
      TRUE
    [] Bug = "progress_drops_precommit_votes"
       /\ c = ProgressPrecommitVotes ->
      FALSE
    [] Bug = "progress_drops_commit_qc"
       /\ c = ProgressCommitQc ->
      FALSE
    [] Bug = "progress_counts_rescheduled_no_evidence"
       /\ c = ProgressRescheduledNoEvidenceInWindow ->
      TRUE
    [] Bug = "progress_counts_under_grace"
       /\ c = ProgressAgeUnderGrace ->
      TRUE
    [] Bug = "progress_rejects_grace_boundary"
       /\ c = ProgressAgeAtGrace ->
      FALSE
    [] Bug = "progress_counts_at_quorum"
       /\ c = ProgressAgeAtQuorum ->
      TRUE
    [] Bug = "progress_counts_authoritative_only"
       /\ c = ProgressAuthoritativeOnly ->
      TRUE
    [] Bug = "progress_drops_inactive_vote_backed"
       /\ c = ProgressConsensusInactiveVoteBacked ->
      FALSE
    [] OTHER -> SpecProgressBlocks(c)

ImplementationClassicCount(c) ==
  IF ImplementationClassicBlocks(c) THEN 1 ELSE 0

ImplementationProgressCount(c) ==
  IF ImplementationProgressBlocks(c) THEN 1 ELSE 0

ImplementationClassicActions(c) ==
  (SpecClassicActions(c) \ {ClassicReturnBlocking, ClassicReturnNonBlocking})
    \cup (IF ImplementationClassicBlocks(c)
          THEN {ClassicReturnBlocking}
          ELSE {ClassicReturnNonBlocking})

ImplementationProgressActions(c) ==
  (SpecProgressActions(c) \ {ProgressReturnBlocking, ProgressReturnNonBlocking})
    \cup (IF ImplementationProgressBlocks(c)
          THEN {ProgressReturnBlocking}
          ELSE {ProgressReturnNonBlocking})

Bugs == {
  "none",
  "classic_counts_inactive",
  "classic_reschedule_blocks_commit_qc",
  "classic_drops_not_fast_due",
  "classic_counts_fast_due",
  "classic_counts_rescheduled_without_qc",
  "classic_treats_cached_qc_as_fast_due",
  "progress_ignores_quorum_zero_fallback",
  "progress_counts_quorum_zero_nonblocking",
  "progress_counts_aborted",
  "progress_counts_off_tip",
  "progress_drops_precommit_votes",
  "progress_drops_commit_qc",
  "progress_counts_rescheduled_no_evidence",
  "progress_counts_under_grace",
  "progress_rejects_grace_boundary",
  "progress_counts_at_quorum",
  "progress_counts_authoritative_only",
  "progress_drops_inactive_vote_backed"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecClassicBlocks(c) \in BOOLEAN
       /\ ImplementationClassicBlocks(c) \in BOOLEAN
       /\ SpecProgressBlocks(c) \in BOOLEAN
       /\ ImplementationProgressBlocks(c) \in BOOLEAN
       /\ SpecClassicCount(c) \in 0..1
       /\ ImplementationClassicCount(c) \in 0..1
       /\ SpecProgressCount(c) \in 0..1
       /\ ImplementationProgressCount(c) \in 0..1
       /\ SpecClassicActions(c) \subseteq ActionUniverse
       /\ ImplementationClassicActions(c) \subseteq ActionUniverse
       /\ SpecProgressActions(c) \subseteq ActionUniverse
       /\ ImplementationProgressActions(c) \subseteq ActionUniverse

ClassicResultMatchesSpec ==
  \A c \in ClassicCases:
    ImplementationClassicBlocks(c) = SpecClassicBlocks(c)

ProgressResultMatchesSpec ==
  \A c \in ProgressCases:
    ImplementationProgressBlocks(c) = SpecProgressBlocks(c)

ClassicCountsMatchPredicate ==
  \A c \in ClassicCases:
    /\ ImplementationClassicCount(c) = SpecClassicCount(c)
    /\ (ImplementationClassicCount(c) > 0) = ImplementationClassicBlocks(c)

ProgressCountsMatchPredicate ==
  \A c \in ProgressCases:
    /\ ImplementationProgressCount(c) = SpecProgressCount(c)
    /\ (ImplementationProgressCount(c) > 0) = ImplementationProgressBlocks(c)

ClassicCommitQcDominatesReschedule ==
  /\ ImplementationClassicBlocks(ClassicCommitQcRescheduled) = TRUE
  /\ ClassicCommitQcBlocks
       \in ImplementationClassicActions(ClassicCommitQcRescheduled)
  /\ ~(ClassicCheckReschedule
       \in ImplementationClassicActions(ClassicCommitQcRescheduled))
  /\ ~(ClassicCheckFastUnblock
       \in ImplementationClassicActions(ClassicCommitQcRescheduled))

ClassicRescheduleAndFastUnblockRelease ==
  /\ ImplementationClassicBlocks(ClassicNoRescheduleNotFastDue) = TRUE
  /\ ClassicNoFastUnblockBlocks
       \in ImplementationClassicActions(ClassicNoRescheduleNotFastDue)
  /\ ImplementationClassicBlocks(ClassicNoRescheduleFastDue) = FALSE
  /\ ClassicFastUnblockReject
       \in ImplementationClassicActions(ClassicNoRescheduleFastDue)
  /\ ImplementationClassicBlocks(ClassicRescheduledNoCommitQc) = FALSE
  /\ ClassicRescheduleReject
       \in ImplementationClassicActions(ClassicRescheduledNoCommitQc)
  /\ ~(ClassicCheckFastUnblock
       \in ImplementationClassicActions(ClassicRescheduledNoCommitQc))
  /\ ImplementationClassicBlocks(ClassicCachedQcNoReschedule) = TRUE

ClassicRequiresActiveTipPending ==
  /\ ImplementationClassicBlocks(ClassicInactive) = FALSE
  /\ ClassicRejectInactive \in ImplementationClassicActions(ClassicInactive)
  /\ ~(ClassicCheckCommitQcObserved
       \in ImplementationClassicActions(ClassicInactive))

ProgressZeroQuorumFallsBackToClassic ==
  /\ ImplementationProgressBlocks(ProgressQuorumZeroClassicBlocking) =
       SpecClassicBlocks(ProgressQuorumZeroClassicBlocking)
  /\ ImplementationProgressBlocks(ProgressQuorumZeroClassicNonBlocking) =
       SpecClassicBlocks(ProgressQuorumZeroClassicNonBlocking)
  /\ ProgressFallbackClassic
       \in ImplementationProgressActions(ProgressQuorumZeroClassicBlocking)
  /\ ~(ProgressCheckAborted
       \in ImplementationProgressActions(ProgressQuorumZeroClassicBlocking))
  /\ ~(ProgressCheckTip
       \in ImplementationProgressActions(ProgressQuorumZeroClassicBlocking))

ProgressRejectsAbortedAndOffTip ==
  /\ ImplementationProgressBlocks(ProgressAbortedWithVotes) = FALSE
  /\ ProgressRejectAborted
       \in ImplementationProgressActions(ProgressAbortedWithVotes)
  /\ ~(ProgressCheckTip
       \in ImplementationProgressActions(ProgressAbortedWithVotes))
  /\ ImplementationProgressBlocks(ProgressOffTipWithVotes) = FALSE
  /\ ProgressRejectOffTip
       \in ImplementationProgressActions(ProgressOffTipWithVotes)
  /\ ~(ProgressCheckPrecommitVotes
       \in ImplementationProgressActions(ProgressOffTipWithVotes))

ProgressVoteAndQcEvidenceBlocks ==
  /\ ImplementationProgressBlocks(ProgressPrecommitVotes) = TRUE
  /\ ProgressPrecommitVotesBlock
       \in ImplementationProgressActions(ProgressPrecommitVotes)
  /\ ~(ProgressCheckCommitQc
       \in ImplementationProgressActions(ProgressPrecommitVotes))
  /\ ImplementationProgressBlocks(ProgressCommitQc) = TRUE
  /\ ProgressCommitQcBlocks \in ImplementationProgressActions(ProgressCommitQc)
  /\ ~(ProgressCheckReschedule
       \in ImplementationProgressActions(ProgressCommitQc))
  /\ ImplementationProgressBlocks(ProgressConsensusInactiveVoteBacked) = TRUE

ProgressRescheduleAndAgeWindow ==
  /\ ImplementationProgressBlocks(ProgressRescheduledNoEvidenceInWindow) = FALSE
  /\ ProgressRescheduleReject
       \in ImplementationProgressActions(ProgressRescheduledNoEvidenceInWindow)
  /\ ~(ProgressCheckAge
       \in ImplementationProgressActions(ProgressRescheduledNoEvidenceInWindow))
  /\ ImplementationProgressBlocks(ProgressAgeUnderGrace) = FALSE
  /\ ProgressAgeUnderGraceReject
       \in ImplementationProgressActions(ProgressAgeUnderGrace)
  /\ ImplementationProgressBlocks(ProgressAgeAtGrace) = TRUE
  /\ ProgressAgeWindowBlocks
       \in ImplementationProgressActions(ProgressAgeAtGrace)
  /\ ImplementationProgressBlocks(ProgressAgePastGrace) = TRUE
  /\ ImplementationProgressBlocks(ProgressAgeAtQuorum) = FALSE
  /\ ProgressAgeQuorumReject
       \in ImplementationProgressActions(ProgressAgeAtQuorum)
  /\ ImplementationProgressBlocks(ProgressAgePastQuorum) = FALSE
  /\ ImplementationProgressBlocks(ProgressGraceClampedNoWindow) = FALSE
  /\ ProgressGraceClampedReject
       \in ImplementationProgressActions(ProgressGraceClampedNoWindow)
  /\ ImplementationProgressBlocks(ProgressAuthoritativeOnly) = FALSE

NoBugInvariant ==
  /\ ClassicResultMatchesSpec
  /\ ProgressResultMatchesSpec
  /\ ClassicCountsMatchPredicate
  /\ ProgressCountsMatchPredicate
  /\ ClassicCommitQcDominatesReschedule
  /\ ClassicRescheduleAndFastUnblockRelease
  /\ ClassicRequiresActiveTipPending
  /\ ProgressZeroQuorumFallsBackToClassic
  /\ ProgressRejectsAbortedAndOffTip
  /\ ProgressVoteAndQcEvidenceBlocks
  /\ ProgressRescheduleAndAgeWindow

SafetyFast == NoBugInvariant

====
