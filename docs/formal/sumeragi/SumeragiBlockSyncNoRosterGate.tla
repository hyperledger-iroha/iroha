---- MODULE SumeragiBlockSyncNoRosterGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the no-verifiable-roster branch in
`handle_block_sync_update(...)`.

After all roster selection attempts fail, the live path has a special
known-block vote-only branch before the generic missing-roster drop:

* known vote-only updates clear the missing-block request and return `Ok(())`;
* those votes are processed only when there is no local commit-roster snapshot;
* a local commit-roster snapshot suppresses vote processing for the known block;
* known updates that are not vote-only fall through to the missing-roster drop
  and also clear the missing-block request;
* unknown updates may first defer exact-frontier repair using the effective
  commit topology, or the trusted topology when the effective topology is empty;
* otherwise unknown updates may refresh the tracked missing-QC request and force
  deterministic sidecar failover before the missing-roster drop.

Every path in this branch returns `Ok(())`; none continues into validated roster
handling.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "known_vote_no_snapshot",
  "known_vote_with_snapshot",
  "known_vote_with_qc",
  "known_vote_with_checkpoint",
  "known_vote_with_stake",
  "known_no_votes",
  "unknown_defer_effective",
  "unknown_defer_trusted",
  "unknown_request_effective_failover",
  "unknown_request_trusted_no_failover",
  "unknown_initial_requested_no_fallback",
  "unknown_no_fallback",
  "unknown_fallback_no_request",
  "unknown_with_qc_drop",
  "unknown_with_votes_drop"
}

BlockKnown(c) ==
  c \in {
    "known_vote_no_snapshot",
    "known_vote_with_snapshot",
    "known_vote_with_qc",
    "known_vote_with_checkpoint",
    "known_vote_with_stake",
    "known_no_votes"
  }

HasCommitVotes(c) ==
  c \in {
    "known_vote_no_snapshot",
    "known_vote_with_snapshot",
    "known_vote_with_qc",
    "known_vote_with_checkpoint",
    "known_vote_with_stake",
    "unknown_with_votes_drop"
  }

CertHint(c) ==
  c \in {"known_vote_with_qc", "unknown_with_qc_drop"}

CheckpointHint(c) ==
  c = "known_vote_with_checkpoint"

StakeHint(c) ==
  c = "known_vote_with_stake"

RosterSnapshot(c) ==
  c = "known_vote_with_snapshot"

EffectiveFallback(c) ==
  c \in {
    "unknown_defer_effective",
    "unknown_request_effective_failover",
    "unknown_fallback_no_request",
    "unknown_with_qc_drop",
    "unknown_with_votes_drop"
  }

TrustedFallback(c) ==
  c \in {
    "unknown_defer_trusted",
    "unknown_request_trusted_no_failover"
  }

KeepExactFrontierRepair(c) ==
  c \in {"unknown_defer_effective", "unknown_defer_trusted"}

MaybeRequestMissingQc(c) ==
  c \in {
    "unknown_request_effective_failover",
    "unknown_request_trusted_no_failover"
  }

InitialRequested(c) ==
  c = "unknown_initial_requested_no_fallback"

SpecKnownVoteOnly(c) ==
  /\ BlockKnown(c)
  /\ HasCommitVotes(c)
  /\ ~CertHint(c)
  /\ ~CheckpointHint(c)
  /\ ~StakeHint(c)

SpecProcessVotes(c) ==
  SpecKnownVoteOnly(c) /\ ~RosterSnapshot(c)

SpecClearMissing(c) ==
  BlockKnown(c)

SpecClearReason(c) ==
  IF SpecClearMissing(c) THEN "PayloadAvailable" ELSE "none"

SpecFallbackSource(c) ==
  IF BlockKnown(c) THEN "none"
  ELSE IF EffectiveFallback(c) THEN "effective"
  ELSE IF TrustedFallback(c) THEN "trusted"
  ELSE "none"

SpecKeepRepairCalled(c) ==
  SpecFallbackSource(c) # "none"

SpecDeferred(c) ==
  SpecKeepRepairCalled(c) /\ KeepExactFrontierRepair(c)

SpecMaybeRequestCalled(c) ==
  /\ SpecKeepRepairCalled(c)
  /\ ~SpecDeferred(c)

SpecRequestedMissing(c) ==
  InitialRequested(c) \/ (SpecMaybeRequestCalled(c) /\ MaybeRequestMissingQc(c))

SpecFailoverCalled(c) ==
  ~BlockKnown(c) /\ SpecRequestedMissing(c)

SpecOutcome(c) ==
  IF SpecKnownVoteOnly(c) THEN "KnownVoteOnly"
  ELSE IF SpecDeferred(c) THEN "Deferred"
  ELSE "Dropped"

SpecStatusOutcome(c) ==
  CASE SpecOutcome(c) = "Deferred" -> "Deferred"
    [] SpecOutcome(c) = "Dropped" -> "Dropped"
    [] OTHER -> "none"

SpecStatusReason(c) ==
  IF SpecStatusOutcome(c) = "none" THEN "none" ELSE "RosterMissing"

SpecDropMetrics(c) ==
  SpecOutcome(c) = "Dropped"

SpecWarnDrop(c) ==
  SpecOutcome(c) = "Dropped"

SpecReturnKind(c) ==
  "Ok"

SpecContinues(c) ==
  FALSE

ActualKnownVoteOnly(c) ==
  IF Bug = "known_with_qc_treated_vote_only"
     /\ c = "known_vote_with_qc"
  THEN TRUE
  ELSE SpecKnownVoteOnly(c)

ActualProcessVotes(c) ==
  IF Bug = "known_vote_no_snapshot_skips_vote_processing"
     /\ c = "known_vote_no_snapshot"
  THEN FALSE
  ELSE IF Bug = "known_vote_with_snapshot_processes_votes"
          /\ c = "known_vote_with_snapshot" THEN TRUE
  ELSE ActualKnownVoteOnly(c) /\ ~RosterSnapshot(c)

ActualClearMissing(c) ==
  IF Bug = "known_vote_only_no_clear"
     /\ c = "known_vote_no_snapshot"
  THEN FALSE
  ELSE IF Bug = "known_drop_no_clear"
          /\ c = "known_vote_with_qc" THEN FALSE
  ELSE IF Bug = "unknown_drop_clears_missing"
          /\ c = "unknown_no_fallback" THEN TRUE
  ELSE BlockKnown(c)

ActualClearReason(c) ==
  IF ~ActualClearMissing(c) THEN "none"
  ELSE IF Bug = "known_vote_only_wrong_clear_reason"
          /\ c = "known_vote_no_snapshot" THEN "Obsolete"
  ELSE "PayloadAvailable"

ActualFallbackSource(c) ==
  IF Bug = "unknown_effective_fallback_ignored"
     /\ c = "unknown_defer_effective"
  THEN "none"
  ELSE IF Bug = "unknown_trusted_fallback_ignored"
          /\ c = "unknown_defer_trusted" THEN "none"
  ELSE SpecFallbackSource(c)

ActualKeepRepairCalled(c) ==
  ActualFallbackSource(c) # "none"

ActualDeferred(c) ==
  IF Bug = "unknown_defer_skipped"
     /\ c = "unknown_defer_effective"
  THEN FALSE
  ELSE ActualKeepRepairCalled(c) /\ KeepExactFrontierRepair(c)

ActualMaybeRequestCalled(c) ==
  IF Bug = "unknown_no_fallback_requests_qc"
     /\ c = "unknown_no_fallback"
  THEN TRUE
  ELSE ActualKeepRepairCalled(c) /\ ~ActualDeferred(c)

ActualRequestedMissing(c) ==
  IF Bug = "unknown_request_not_recorded"
     /\ c = "unknown_request_effective_failover"
  THEN FALSE
  ELSE IF Bug = "unknown_request_without_maybe"
          /\ c = "unknown_fallback_no_request" THEN TRUE
  ELSE IF Bug = "unknown_no_fallback_requests_qc"
          /\ c = "unknown_no_fallback" THEN TRUE
  ELSE InitialRequested(c) \/ (ActualMaybeRequestCalled(c) /\ MaybeRequestMissingQc(c))

ActualFailoverCalled(c) ==
  IF Bug = "unknown_failover_not_called"
     /\ c = "unknown_request_effective_failover"
  THEN FALSE
  ELSE IF Bug = "unknown_failover_without_request"
          /\ c = "unknown_no_fallback" THEN TRUE
  ELSE ~BlockKnown(c) /\ ActualRequestedMissing(c)

ActualOutcome(c) ==
  IF Bug = "known_vote_only_drops_update"
     /\ c = "known_vote_no_snapshot"
  THEN "Dropped"
  ELSE IF ActualKnownVoteOnly(c) THEN "KnownVoteOnly"
  ELSE IF ActualDeferred(c) THEN "Deferred"
  ELSE "Dropped"

ActualStatusOutcome(c) ==
  IF Bug = "known_vote_only_records_drop"
     /\ c = "known_vote_no_snapshot"
  THEN "Dropped"
  ELSE IF Bug = "unknown_defer_records_drop"
          /\ c = "unknown_defer_effective" THEN "Dropped"
  ELSE IF Bug = "unknown_defer_no_status"
          /\ c = "unknown_defer_effective" THEN "none"
  ELSE IF Bug = "known_drop_no_status"
          /\ c = "known_vote_with_qc" THEN "none"
  ELSE IF Bug = "unknown_drop_no_status"
          /\ c = "unknown_no_fallback" THEN "none"
  ELSE
    CASE ActualOutcome(c) = "Deferred" -> "Deferred"
      [] ActualOutcome(c) = "Dropped" -> "Dropped"
      [] OTHER -> "none"

ActualStatusReason(c) ==
  IF ActualStatusOutcome(c) = "none" THEN "none"
  ELSE IF Bug = "unknown_drop_wrong_reason"
          /\ c = "unknown_no_fallback" THEN "InvalidSignature"
  ELSE "RosterMissing"

ActualDropMetrics(c) ==
  IF Bug = "unknown_drop_no_metrics"
     /\ c = "unknown_no_fallback"
  THEN FALSE
  ELSE ActualOutcome(c) = "Dropped"

ActualWarnDrop(c) ==
  ActualOutcome(c) = "Dropped"

ActualReturnKind(c) ==
  IF Bug = "no_roster_returns_error"
     /\ c = "unknown_no_fallback"
  THEN "Err"
  ELSE "Ok"

ActualContinues(c) ==
  Bug = "no_roster_continues" /\ c = "unknown_no_fallback"

Matches(c) ==
  /\ ActualKnownVoteOnly(c) = SpecKnownVoteOnly(c)
  /\ ActualProcessVotes(c) = SpecProcessVotes(c)
  /\ ActualClearMissing(c) = SpecClearMissing(c)
  /\ ActualClearReason(c) = SpecClearReason(c)
  /\ ActualFallbackSource(c) = SpecFallbackSource(c)
  /\ ActualKeepRepairCalled(c) = SpecKeepRepairCalled(c)
  /\ ActualDeferred(c) = SpecDeferred(c)
  /\ ActualMaybeRequestCalled(c) = SpecMaybeRequestCalled(c)
  /\ ActualRequestedMissing(c) = SpecRequestedMissing(c)
  /\ ActualFailoverCalled(c) = SpecFailoverCalled(c)
  /\ ActualOutcome(c) = SpecOutcome(c)
  /\ ActualStatusOutcome(c) = SpecStatusOutcome(c)
  /\ ActualStatusReason(c) = SpecStatusReason(c)
  /\ ActualDropMetrics(c) = SpecDropMetrics(c)
  /\ ActualWarnDrop(c) = SpecWarnDrop(c)
  /\ ActualReturnKind(c) = SpecReturnKind(c)
  /\ ActualContinues(c) = SpecContinues(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "known_vote_no_snapshot_skips_vote_processing",
       "known_vote_with_snapshot_processes_votes",
       "known_vote_only_no_clear",
       "known_vote_only_wrong_clear_reason",
       "known_vote_only_records_drop",
       "known_vote_only_drops_update",
       "known_with_qc_treated_vote_only",
       "known_drop_no_clear",
       "known_drop_no_status",
       "unknown_defer_skipped",
       "unknown_defer_records_drop",
       "unknown_defer_no_status",
       "unknown_effective_fallback_ignored",
       "unknown_trusted_fallback_ignored",
       "unknown_request_not_recorded",
       "unknown_request_without_maybe",
       "unknown_failover_not_called",
       "unknown_failover_without_request",
       "unknown_drop_clears_missing",
       "unknown_drop_wrong_reason",
       "unknown_drop_no_status",
       "unknown_drop_no_metrics",
       "unknown_no_fallback_requests_qc",
       "no_roster_returns_error",
       "no_roster_continues"
     }
  /\ checked = 0

SafetyFast ==
  \A c \in Cases: Matches(c)

KnownVoteOnlyProcessesWithoutSnapshot ==
  Matches("known_vote_no_snapshot")

KnownVoteOnlySnapshotDropsVotes ==
  Matches("known_vote_with_snapshot")

KnownHintedDropsClearMissing ==
  Matches("known_vote_with_qc")
    /\ Matches("known_vote_with_checkpoint")
    /\ Matches("known_vote_with_stake")
    /\ Matches("known_no_votes")

UnknownEffectiveRepairDefers ==
  Matches("unknown_defer_effective")

UnknownTrustedRepairDefers ==
  Matches("unknown_defer_trusted")

UnknownMissingQcRequestsFailover ==
  Matches("unknown_request_effective_failover")
    /\ Matches("unknown_request_trusted_no_failover")
    /\ Matches("unknown_initial_requested_no_fallback")

UnknownMissingRosterDrops ==
  Matches("unknown_no_fallback")
    /\ Matches("unknown_fallback_no_request")
    /\ Matches("unknown_with_qc_drop")
    /\ Matches("unknown_with_votes_drop")

=============================================================================
