---- MODULE SumeragiRbcBacklogStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for RBC backlog summaries and published status.

This slice pins the observable helper contracts around:
- `rbc_backlog_summary()` and `proposal_rbc_backlog_summary()`, which count
  only active, non-invalid RBC sessions without complete authoritative progress,
  treating malformed chunk counters as trusted missing pressure even when local
  authoritative payloads exist, plus the appropriate pending RBC stashes;
- `rbc_backlog_exceeds_pacemaker_soft_limits(...)`, which uses strict `>`
  thresholds after requiring a real backlog; and
- `update_rbc_backlog_snapshot(...)`, which publishes undelivered-session
  missing-chunk totals separately from pending-stash caps and contents.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

SummaryDaDisabled == "summary_da_disabled"
SummaryInactive == "summary_inactive"
SummaryInvalid == "summary_invalid"
SummaryAuthoritative == "summary_authoritative"
SummaryOvercountedChunks == "summary_overcounted_chunks"
SummaryZeroTotalChunks == "summary_zero_total_chunks"
SummaryAuthoritativeMalformed == "summary_authoritative_malformed"
SummaryNoPayloadNoMissing == "summary_no_payload_no_missing"
SummaryMissingChunks == "summary_missing_chunks"
SummaryDeliveredBacklog == "summary_delivered_backlog"
SummaryPendingTip == "summary_pending_tip"
SummaryPendingNext == "summary_pending_next"
SummaryPendingFuture == "summary_pending_future"
SummaryMixedSessionPending == "summary_mixed_session_pending"

SummaryCases == {
  SummaryDaDisabled,
  SummaryInactive,
  SummaryInvalid,
  SummaryAuthoritative,
  SummaryOvercountedChunks,
  SummaryZeroTotalChunks,
  SummaryAuthoritativeMalformed,
  SummaryNoPayloadNoMissing,
  SummaryMissingChunks,
  SummaryDeliveredBacklog,
  SummaryPendingTip,
  SummaryPendingNext,
  SummaryPendingFuture,
  SummaryMixedSessionPending
}

ProposalSessionBlocking == "proposal_session_blocking"
ProposalSessionInactive == "proposal_session_inactive"
ProposalOvercountedChunks == "proposal_overcounted_chunks"
ProposalAuthoritativeMalformed == "proposal_authoritative_malformed"
ProposalPendingBlocking == "proposal_pending_blocking"
ProposalPendingTipOnly == "proposal_pending_tip_only"

ProposalCases == {
  ProposalSessionBlocking,
  ProposalSessionInactive,
  ProposalOvercountedChunks,
  ProposalAuthoritativeMalformed,
  ProposalPendingBlocking,
  ProposalPendingTipOnly
}

SoftNoBacklog == "soft_no_backlog"
SoftSessionExact == "soft_session_exact"
SoftSessionOver == "soft_session_over"
SoftMissingExact == "soft_missing_exact"
SoftMissingOver == "soft_missing_over"

SoftLimitCases == {
  SoftNoBacklog,
  SoftSessionExact,
  SoftSessionOver,
  SoftMissingExact,
  SoftMissingOver
}

SnapshotEmpty == "snapshot_empty"
SnapshotUndeliveredMissing == "snapshot_undelivered_missing"
SnapshotOvercountedMissing == "snapshot_overcounted_missing"
SnapshotZeroTotalMissing == "snapshot_zero_total_missing"
SnapshotDeliveredMissing == "snapshot_delivered_missing"
SnapshotTwoUndelivered == "snapshot_two_undelivered"
SnapshotPendingStash == "snapshot_pending_stash"

SnapshotCases == {
  SnapshotEmpty,
  SnapshotUndeliveredMissing,
  SnapshotOvercountedMissing,
  SnapshotZeroTotalMissing,
  SnapshotDeliveredMissing,
  SnapshotTwoUndelivered,
  SnapshotPendingStash
}

SpecSummarySessions(c) ==
  CASE c \in {
         SummaryNoPayloadNoMissing,
         SummaryMissingChunks,
         SummaryOvercountedChunks,
         SummaryZeroTotalChunks,
         SummaryAuthoritativeMalformed,
         SummaryDeliveredBacklog,
         SummaryPendingTip,
         SummaryPendingNext
       } -> 1
    [] c = SummaryMixedSessionPending -> 2
    [] OTHER -> 0

SpecSummaryMissing(c) ==
  CASE c \in {SummaryMissingChunks, SummaryMixedSessionPending} -> 3
    [] c \in {SummaryOvercountedChunks, SummaryAuthoritativeMalformed} -> 4
    [] c = SummaryZeroTotalChunks -> 1
    [] OTHER -> 0

SpecSummaryHasBacklog(c) ==
  SpecSummarySessions(c) > 0 \/ SpecSummaryMissing(c) > 0

ImplementationSummarySessions(c) ==
  CASE Bug = "summary_da_disabled_counts"
       /\ c = SummaryDaDisabled -> 1
    [] Bug = "summary_inactive_counts"
       /\ c = SummaryInactive -> 1
    [] Bug = "summary_invalid_counts"
       /\ c = SummaryInvalid -> 1
    [] Bug = "summary_authoritative_counts"
       /\ c = SummaryAuthoritative -> 1
    [] Bug = "summary_authoritative_malformed_ignored"
       /\ c = SummaryAuthoritativeMalformed -> 0
    [] Bug = "summary_pending_tip_ignored"
       /\ c = SummaryPendingTip -> 0
    [] Bug = "summary_pending_next_ignored"
       /\ c = SummaryPendingNext -> 0
    [] Bug = "summary_pending_future_counts"
       /\ c = SummaryPendingFuture -> 1
    [] Bug = "summary_mixed_drops_pending"
       /\ c = SummaryMixedSessionPending -> 1
    [] Bug = "summary_drops_delivered_backlog"
       /\ c = SummaryDeliveredBacklog -> 0
    [] OTHER -> SpecSummarySessions(c)

ImplementationSummaryMissing(c) ==
  CASE Bug = "summary_missing_chunks_ignored"
       /\ c \in {SummaryMissingChunks, SummaryMixedSessionPending} -> 0
    [] Bug = "summary_missing_uses_received"
       /\ c = SummaryMissingChunks -> 1
    [] Bug = "summary_malformed_uses_saturating_zero"
       /\ c \in {SummaryOvercountedChunks, SummaryZeroTotalChunks} -> 0
    [] Bug = "summary_authoritative_malformed_ignored"
       /\ c = SummaryAuthoritativeMalformed -> 0
    [] OTHER -> SpecSummaryMissing(c)

ImplementationSummaryHasBacklog(c) ==
  ImplementationSummarySessions(c) > 0 \/ ImplementationSummaryMissing(c) > 0

SpecProposalSessions(c) ==
  CASE c \in {
         ProposalSessionBlocking,
         ProposalOvercountedChunks,
         ProposalAuthoritativeMalformed,
         ProposalPendingBlocking
       } -> 1
    [] OTHER -> 0

SpecProposalMissing(c) ==
  CASE c = ProposalSessionBlocking -> 3
    [] c \in {ProposalOvercountedChunks, ProposalAuthoritativeMalformed} -> 4
    [] OTHER -> 0

ImplementationProposalSessions(c) ==
  CASE Bug = "proposal_pending_tip_counts"
       /\ c = ProposalPendingTipOnly -> 1
    [] Bug = "proposal_blocking_pending_ignored"
       /\ c = ProposalPendingBlocking -> 0
    [] Bug = "proposal_inactive_counts"
       /\ c = ProposalSessionInactive -> 1
    [] Bug = "proposal_authoritative_malformed_ignored"
       /\ c = ProposalAuthoritativeMalformed -> 0
    [] OTHER -> SpecProposalSessions(c)

ImplementationProposalMissing(c) ==
  CASE Bug = "proposal_session_ignores_missing"
       /\ c = ProposalSessionBlocking -> 0
    [] Bug = "proposal_malformed_uses_saturating_zero"
       /\ c = ProposalOvercountedChunks -> 0
    [] Bug = "proposal_authoritative_malformed_ignored"
       /\ c = ProposalAuthoritativeMalformed -> 0
    [] OTHER -> SpecProposalMissing(c)

SoftSessions(c) ==
  CASE c = SoftNoBacklog -> 0
    [] c \in {SoftSessionExact, SoftMissingExact, SoftMissingOver} -> 2
    [] c = SoftSessionOver -> 3
    [] OTHER -> 0

SoftMissing(c) ==
  CASE c = SoftNoBacklog -> 0
    [] c = SoftMissingExact -> 5
    [] c = SoftMissingOver -> 6
    [] OTHER -> 0

SoftSessionLimit(c) ==
  CASE c \in {SoftSessionExact, SoftSessionOver} -> 2
    [] OTHER -> 10

SoftChunkLimit(c) ==
  CASE c \in {SoftMissingExact, SoftMissingOver} -> 5
    [] OTHER -> 10

SpecSoftExceeded(c) ==
  (SoftSessions(c) > 0 \/ SoftMissing(c) > 0)
    /\ (SoftSessions(c) > SoftSessionLimit(c)
        \/ SoftMissing(c) > SoftChunkLimit(c))

ImplementationSoftExceeded(c) ==
  CASE Bug = "soft_exact_session_triggers"
       /\ c = SoftSessionExact -> TRUE
    [] Bug = "soft_exact_missing_triggers"
       /\ c = SoftMissingExact -> TRUE
    [] Bug = "soft_ignores_sessions"
       /\ c = SoftSessionOver -> FALSE
    [] Bug = "soft_ignores_missing"
       /\ c = SoftMissingOver -> FALSE
    [] OTHER -> SpecSoftExceeded(c)

SpecSnapshotTotalMissing(c) ==
  CASE c = SnapshotUndeliveredMissing -> 3
    [] c = SnapshotOvercountedMissing -> 4
    [] c = SnapshotZeroTotalMissing -> 1
    [] c = SnapshotTwoUndelivered -> 5
    [] OTHER -> 0

SpecSnapshotMaxMissing(c) ==
  CASE c = SnapshotUndeliveredMissing -> 3
    [] c = SnapshotOvercountedMissing -> 4
    [] c = SnapshotZeroTotalMissing -> 1
    [] c = SnapshotTwoUndelivered -> 3
    [] OTHER -> 0

SpecSnapshotPendingSessions(c) ==
  CASE c \in {
         SnapshotUndeliveredMissing,
         SnapshotOvercountedMissing,
         SnapshotZeroTotalMissing
       } -> 1
    [] c = SnapshotTwoUndelivered -> 2
    [] OTHER -> 0

SpecSnapshotStashSessions(c) ==
  CASE c = SnapshotPendingStash -> 2
    [] OTHER -> 0

SpecSnapshotStashChunks(c) ==
  CASE c = SnapshotPendingStash -> 5
    [] OTHER -> 0

SpecSnapshotStashBytes(c) ==
  CASE c = SnapshotPendingStash -> 13
    [] OTHER -> 0

SpecSnapshotCapChunks(c) ==
  CASE c = SnapshotPendingStash -> 7
    [] OTHER -> 0

SpecSnapshotCapBytes(c) ==
  CASE c = SnapshotPendingStash -> 19
    [] OTHER -> 0

SpecSnapshotTtlMs(c) ==
  CASE c = SnapshotPendingStash -> 11
    [] OTHER -> 0

ImplementationSnapshotTotalMissing(c) ==
  CASE Bug = "snapshot_counts_delivered_as_pending"
       /\ c = SnapshotDeliveredMissing -> 3
    [] Bug = "snapshot_ignores_undelivered"
       /\ c = SnapshotUndeliveredMissing -> 0
    [] Bug = "snapshot_malformed_uses_saturating_zero"
       /\ c \in {SnapshotOvercountedMissing, SnapshotZeroTotalMissing} -> 0
    [] OTHER -> SpecSnapshotTotalMissing(c)

ImplementationSnapshotMaxMissing(c) ==
  CASE Bug = "snapshot_counts_delivered_as_pending"
       /\ c = SnapshotDeliveredMissing -> 3
    [] Bug = "snapshot_ignores_undelivered"
       /\ c = SnapshotUndeliveredMissing -> 0
    [] Bug = "snapshot_max_uses_total"
       /\ c = SnapshotTwoUndelivered -> 5
    [] Bug = "snapshot_malformed_uses_saturating_zero"
       /\ c \in {SnapshotOvercountedMissing, SnapshotZeroTotalMissing} -> 0
    [] OTHER -> SpecSnapshotMaxMissing(c)

ImplementationSnapshotPendingSessions(c) ==
  CASE Bug = "snapshot_counts_delivered_as_pending"
       /\ c = SnapshotDeliveredMissing -> 1
    [] Bug = "snapshot_ignores_undelivered"
       /\ c = SnapshotUndeliveredMissing -> 0
    [] Bug = "snapshot_pending_stash_as_session"
       /\ c = SnapshotPendingStash -> 2
    [] OTHER -> SpecSnapshotPendingSessions(c)

ImplementationSnapshotStashSessions(c) ==
  CASE Bug = "snapshot_pending_stash_ignored"
       /\ c = SnapshotPendingStash -> 0
    [] OTHER -> SpecSnapshotStashSessions(c)

ImplementationSnapshotStashChunks(c) ==
  CASE Bug = "snapshot_pending_stash_ignored"
       /\ c = SnapshotPendingStash -> 0
    [] OTHER -> SpecSnapshotStashChunks(c)

ImplementationSnapshotStashBytes(c) ==
  CASE Bug = "snapshot_pending_stash_ignored"
       /\ c = SnapshotPendingStash -> 0
    [] OTHER -> SpecSnapshotStashBytes(c)

ImplementationSnapshotCapChunks(c) ==
  CASE Bug = "snapshot_caps_zeroed"
       /\ c = SnapshotPendingStash -> 0
    [] OTHER -> SpecSnapshotCapChunks(c)

ImplementationSnapshotCapBytes(c) ==
  CASE Bug = "snapshot_caps_zeroed"
       /\ c = SnapshotPendingStash -> 0
    [] OTHER -> SpecSnapshotCapBytes(c)

ImplementationSnapshotTtlMs(c) ==
  CASE Bug = "snapshot_caps_zeroed"
       /\ c = SnapshotPendingStash -> 0
    [] OTHER -> SpecSnapshotTtlMs(c)

Bugs == {
  "none",
  "summary_da_disabled_counts",
  "summary_inactive_counts",
  "summary_invalid_counts",
  "summary_authoritative_counts",
  "summary_missing_chunks_ignored",
  "summary_missing_uses_received",
  "summary_malformed_uses_saturating_zero",
  "summary_authoritative_malformed_ignored",
  "summary_pending_tip_ignored",
  "summary_pending_next_ignored",
  "summary_pending_future_counts",
  "summary_mixed_drops_pending",
  "summary_drops_delivered_backlog",
  "proposal_pending_tip_counts",
  "proposal_blocking_pending_ignored",
  "proposal_session_ignores_missing",
  "proposal_malformed_uses_saturating_zero",
  "proposal_authoritative_malformed_ignored",
  "proposal_inactive_counts",
  "soft_exact_session_triggers",
  "soft_exact_missing_triggers",
  "soft_ignores_sessions",
  "soft_ignores_missing",
  "snapshot_counts_delivered_as_pending",
  "snapshot_ignores_undelivered",
  "snapshot_max_uses_total",
  "snapshot_malformed_uses_saturating_zero",
  "snapshot_pending_stash_ignored",
  "snapshot_caps_zeroed",
  "snapshot_pending_stash_as_session"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked = 0
  /\ \A c \in SummaryCases:
       /\ SpecSummarySessions(c) \in 0..2
       /\ SpecSummaryMissing(c) \in 0..4
       /\ SpecSummaryHasBacklog(c) \in BOOLEAN
       /\ ImplementationSummarySessions(c) \in 0..2
       /\ ImplementationSummaryMissing(c) \in 0..4
       /\ ImplementationSummaryHasBacklog(c) \in BOOLEAN
  /\ \A c \in ProposalCases:
       /\ SpecProposalSessions(c) \in 0..1
       /\ SpecProposalMissing(c) \in 0..4
       /\ ImplementationProposalSessions(c) \in 0..1
       /\ ImplementationProposalMissing(c) \in 0..4
  /\ \A c \in SoftLimitCases:
       /\ SpecSoftExceeded(c) \in BOOLEAN
       /\ ImplementationSoftExceeded(c) \in BOOLEAN
  /\ \A c \in SnapshotCases:
       /\ SpecSnapshotTotalMissing(c) \in 0..5
       /\ SpecSnapshotMaxMissing(c) \in 0..5
       /\ SpecSnapshotPendingSessions(c) \in 0..2
       /\ SpecSnapshotStashSessions(c) \in 0..2
       /\ SpecSnapshotStashChunks(c) \in 0..5
       /\ SpecSnapshotStashBytes(c) \in 0..13
       /\ SpecSnapshotCapChunks(c) \in 0..7
       /\ SpecSnapshotCapBytes(c) \in 0..19
       /\ SpecSnapshotTtlMs(c) \in 0..11
       /\ ImplementationSnapshotTotalMissing(c) \in 0..5
       /\ ImplementationSnapshotMaxMissing(c) \in 0..5
       /\ ImplementationSnapshotPendingSessions(c) \in 0..2
       /\ ImplementationSnapshotStashSessions(c) \in 0..2
       /\ ImplementationSnapshotStashChunks(c) \in 0..5
       /\ ImplementationSnapshotStashBytes(c) \in 0..13
       /\ ImplementationSnapshotCapChunks(c) \in 0..7
       /\ ImplementationSnapshotCapBytes(c) \in 0..19
       /\ ImplementationSnapshotTtlMs(c) \in 0..11

RbcBacklogStatusMatchesSpec ==
  /\ \A c \in SummaryCases:
       /\ ImplementationSummarySessions(c) = SpecSummarySessions(c)
       /\ ImplementationSummaryMissing(c) = SpecSummaryMissing(c)
       /\ ImplementationSummaryHasBacklog(c) = SpecSummaryHasBacklog(c)
  /\ \A c \in ProposalCases:
       /\ ImplementationProposalSessions(c) = SpecProposalSessions(c)
       /\ ImplementationProposalMissing(c) = SpecProposalMissing(c)
  /\ \A c \in SoftLimitCases:
       ImplementationSoftExceeded(c) = SpecSoftExceeded(c)
  /\ \A c \in SnapshotCases:
       /\ ImplementationSnapshotTotalMissing(c) = SpecSnapshotTotalMissing(c)
       /\ ImplementationSnapshotMaxMissing(c) = SpecSnapshotMaxMissing(c)
       /\ ImplementationSnapshotPendingSessions(c) =
            SpecSnapshotPendingSessions(c)
       /\ ImplementationSnapshotStashSessions(c) =
            SpecSnapshotStashSessions(c)
       /\ ImplementationSnapshotStashChunks(c) = SpecSnapshotStashChunks(c)
       /\ ImplementationSnapshotStashBytes(c) = SpecSnapshotStashBytes(c)
       /\ ImplementationSnapshotCapChunks(c) = SpecSnapshotCapChunks(c)
       /\ ImplementationSnapshotCapBytes(c) = SpecSnapshotCapBytes(c)
       /\ ImplementationSnapshotTtlMs(c) = SpecSnapshotTtlMs(c)

SafetyFast ==
  RbcBacklogStatusMatchesSpec

RbcBacklogStatusExactness ==
  /\ RbcBacklogStatusMatchesSpec
RbcBacklogStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcBacklogStatusExactness

====
