---- MODULE SumeragiMissingLockedQcRecoveryGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for missing locked-QC payload recovery.

This slice pins the helper contract around:
- `request_missing_locked_qc_payload(...)`; and
- `drop_missing_lock_if_unknown(...)`.

The model keeps hashes, rosters, and clocks finite while preserving the
observable safety decisions: a missing locked payload is fetched before the
lock-dependent QC path can proceed, no request is emitted without a lock,
known locked payloads are left alone, locked-QC fetches use consensus priority
and the locked hash/height/view, stale same-frontier lock conflicts are cleared
as obsolete, and stale suppression realigns lock/highest-QC state plus a
canonical range-pull reanchor without also fetching the obsolete payload.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoLockRequest == "NoLockRequest"
KnownPayloadRequest == "KnownPayloadRequest"
VoteRosterFetch == "VoteRosterFetch"
CommitFallbackFetch == "CommitFallbackFetch"
NoRosterRequest == "NoRosterRequest"
IngressHoldRequest == "IngressHoldRequest"

RequestCases == {
  NoLockRequest,
  KnownPayloadRequest,
  VoteRosterFetch,
  CommitFallbackFetch,
  NoRosterRequest,
  IngressHoldRequest
}

VoteRoster == "vote_roster"
CommitTopology == "commit_topology"
NoRoster == "none"

LockedConsensusFields == "locked_hash_height_view_consensus"
WrongPriorityFields == "locked_hash_height_view_background"
WrongSlotFields == "incoming_hash_height_view_consensus"
NoFields == "none"

SpecRequestReturn(case) ==
  case \in {VoteRosterFetch, CommitFallbackFetch, IngressHoldRequest}

SpecRequestFetch(case) ==
  case \in {VoteRosterFetch, CommitFallbackFetch}

SpecRequestRoster(case) ==
  CASE case \in {VoteRosterFetch, IngressHoldRequest} -> VoteRoster
    [] case = CommitFallbackFetch -> CommitTopology
    [] OTHER -> NoRoster

SpecRequestViewChangeWindow(case) ==
  case \in {VoteRosterFetch, CommitFallbackFetch, IngressHoldRequest}

SpecRequestFields(case) ==
  CASE SpecRequestFetch(case) -> LockedConsensusFields
    [] OTHER -> NoFields

SpecRequestRosterProof(case) ==
  CASE SpecRequestFetch(case) -> "preserved"
    [] OTHER -> "none"

ActualRequestReturn(case) ==
  CASE Bug = "request_no_lock_fetches" /\ case = NoLockRequest -> TRUE
    [] Bug = "request_known_payload_fetches" /\ case = KnownPayloadRequest -> TRUE
    [] Bug = "request_no_roster_returns_true" /\ case = NoRosterRequest -> TRUE
    [] Bug = "request_hold_returns_false" /\ case = IngressHoldRequest -> FALSE
    [] Bug = "request_missing_returns_false"
       /\ case \in {VoteRosterFetch, CommitFallbackFetch} -> FALSE
    [] OTHER -> SpecRequestReturn(case)

ActualRequestFetch(case) ==
  CASE Bug = "request_no_lock_fetches" /\ case = NoLockRequest -> TRUE
    [] Bug = "request_known_payload_fetches" /\ case = KnownPayloadRequest -> TRUE
    [] Bug = "request_vote_roster_ignored" /\ case = VoteRosterFetch -> FALSE
    [] Bug = "request_no_commit_fallback" /\ case = CommitFallbackFetch -> FALSE
    [] Bug = "request_hold_fetches" /\ case = IngressHoldRequest -> TRUE
    [] OTHER -> SpecRequestFetch(case)

ActualRequestRoster(case) ==
  CASE Bug = "request_vote_roster_ignored" /\ case = VoteRosterFetch -> NoRoster
    [] Bug = "request_no_commit_fallback" /\ case = CommitFallbackFetch -> NoRoster
    [] OTHER -> SpecRequestRoster(case)

ActualRequestViewChangeWindow(case) ==
  CASE Bug = "request_missing_without_view_window"
       /\ case \in {VoteRosterFetch, CommitFallbackFetch, IngressHoldRequest} -> FALSE
    [] OTHER -> SpecRequestViewChangeWindow(case)

ActualRequestFields(case) ==
  CASE Bug = "request_wrong_priority" /\ SpecRequestFetch(case) -> WrongPriorityFields
    [] Bug = "request_wrong_slot" /\ SpecRequestFetch(case) -> WrongSlotFields
    [] OTHER -> SpecRequestFields(case)

ActualRequestRosterProof(case) ==
  CASE Bug = "request_drops_roster_proof" /\ SpecRequestFetch(case) -> "dropped"
    [] OTHER -> SpecRequestRosterProof(case)

DropNoLock == "DropNoLock"
DropKnownPayload == "DropKnownPayload"
DropFreshFetch == "DropFreshFetch"
DropFreshNoRoster == "DropFreshNoRoster"
DropFreshHold == "DropFreshHold"
DropStaleCommittedOldHighest == "DropStaleCommittedOldHighest"
DropStaleCommittedNewerHighest == "DropStaleCommittedNewerHighest"
DropStaleNoCommitted == "DropStaleNoCommitted"
DropSameHashStaleRequest == "DropSameHashStaleRequest"
DropNotHigherStaleRequest == "DropNotHigherStaleRequest"
DropNonFrontierStaleRequest == "DropNonFrontierStaleRequest"
DropFreshConflict == "DropFreshConflict"

DropCases == {
  DropNoLock,
  DropKnownPayload,
  DropFreshFetch,
  DropFreshNoRoster,
  DropFreshHold,
  DropStaleCommittedOldHighest,
  DropStaleCommittedNewerHighest,
  DropStaleNoCommitted,
  DropSameHashStaleRequest,
  DropNotHigherStaleRequest,
  DropNonFrontierStaleRequest,
  DropFreshConflict
}

NoLockState == "no_lock"
OldLock == "old_lock"
CommittedLock == "committed_lock"
CommittedHighest == "committed_highest"
PreserveNewerHighest == "preserve_newer_highest"
PreserveHighest == "preserve_highest"

SpecStaleConflict(case) ==
  case \in {DropStaleCommittedOldHighest,
            DropStaleCommittedNewerHighest,
            DropStaleNoCommitted}

SpecDropRequestAttempt(case) ==
  case \in {DropFreshFetch,
            DropFreshNoRoster,
            DropFreshHold,
            DropSameHashStaleRequest,
            DropNotHigherStaleRequest,
            DropNonFrontierStaleRequest,
            DropFreshConflict}

SpecDropFetch(case) ==
  case \in {DropFreshFetch,
            DropSameHashStaleRequest,
            DropNotHigherStaleRequest,
            DropNonFrontierStaleRequest,
            DropFreshConflict}

SpecDropClearMissing(case) ==
  SpecStaleConflict(case)

SpecDropClearViewChange(case) ==
  SpecStaleConflict(case)

SpecDropLockAfter(case) ==
  CASE case = DropNoLock -> NoLockState
    [] case \in {DropStaleCommittedOldHighest, DropStaleCommittedNewerHighest} -> CommittedLock
    [] case = DropStaleNoCommitted -> NoLockState
    [] OTHER -> OldLock

SpecDropHighestAfter(case) ==
  CASE case = DropStaleCommittedOldHighest -> CommittedHighest
    [] case = DropStaleCommittedNewerHighest -> PreserveNewerHighest
    [] OTHER -> PreserveHighest

SpecDropPruneVotes(case) ==
  case \in {DropStaleCommittedOldHighest, DropStaleCommittedNewerHighest}

SpecDropReanchor(case) ==
  SpecStaleConflict(case)

ActualStaleConflict(case) ==
  CASE Bug = "drop_same_hash_stale_clears" /\ case = DropSameHashStaleRequest -> TRUE
    [] Bug = "drop_not_higher_stale_clears" /\ case = DropNotHigherStaleRequest -> TRUE
    [] Bug = "drop_nonfrontier_stale_clears" /\ case = DropNonFrontierStaleRequest -> TRUE
    [] Bug = "drop_fresh_stale_predicate" /\ case = DropFreshConflict -> TRUE
    [] OTHER -> SpecStaleConflict(case)

ActualDropRequestAttempt(case) ==
  CASE Bug = "drop_stale_fetches_payload" /\ SpecStaleConflict(case) -> TRUE
    [] OTHER -> SpecDropRequestAttempt(case)

ActualDropFetch(case) ==
  CASE Bug = "drop_known_payload_fetches" /\ case = DropKnownPayload -> TRUE
    [] Bug = "drop_fresh_skips_fetch" /\ case = DropFreshFetch -> FALSE
    [] Bug = "drop_stale_fetches_payload" /\ SpecStaleConflict(case) -> TRUE
    [] OTHER -> SpecDropFetch(case)

ActualDropClearMissing(case) ==
  CASE Bug = "drop_no_lock_mutates" /\ case = DropNoLock -> TRUE
    [] Bug = "drop_stale_skips_clear_request" /\ SpecStaleConflict(case) -> FALSE
    [] OTHER -> SpecDropClearMissing(case)

ActualDropClearViewChange(case) ==
  CASE Bug = "drop_stale_skips_view_clear" /\ SpecStaleConflict(case) -> FALSE
    [] OTHER -> SpecDropClearViewChange(case)

ActualDropLockAfter(case) ==
  CASE Bug = "drop_no_lock_mutates" /\ case = DropNoLock -> OldLock
    [] Bug = "drop_fresh_clears_lock" /\ case = DropFreshFetch -> NoLockState
    [] Bug = "drop_stale_keeps_old_lock" /\ SpecStaleConflict(case) -> OldLock
    [] Bug = "drop_stale_clears_lock_with_committed"
       /\ case \in {DropStaleCommittedOldHighest, DropStaleCommittedNewerHighest} -> NoLockState
    [] Bug = "drop_no_committed_keeps_lock" /\ case = DropStaleNoCommitted -> OldLock
    [] OTHER -> SpecDropLockAfter(case)

ActualDropHighestAfter(case) ==
  CASE Bug = "drop_stale_overwrites_newer_highest"
       /\ case = DropStaleCommittedNewerHighest -> CommittedHighest
    [] Bug = "drop_stale_skips_highest_realign"
       /\ case = DropStaleCommittedOldHighest -> PreserveHighest
    [] OTHER -> SpecDropHighestAfter(case)

ActualDropPruneVotes(case) ==
  CASE Bug = "drop_stale_prunes_without_lock_change" /\ case = DropFreshFetch -> TRUE
    [] Bug = "drop_stale_skips_prune_on_change"
       /\ case = DropStaleCommittedOldHighest -> FALSE
    [] OTHER -> SpecDropPruneVotes(case)

ActualDropReanchor(case) ==
  CASE Bug = "drop_stale_skips_reanchor" /\ SpecStaleConflict(case) -> FALSE
    [] OTHER -> SpecDropReanchor(case)

TypeInvariant ==
  checked \in 0..1

RequestReturnMatches ==
  \A case \in RequestCases:
    ActualRequestReturn(case) = SpecRequestReturn(case)

RequestFetchMatches ==
  \A case \in RequestCases:
    ActualRequestFetch(case) = SpecRequestFetch(case)

RequestRosterMatches ==
  \A case \in RequestCases:
    ActualRequestRoster(case) = SpecRequestRoster(case)

RequestViewWindowMatches ==
  \A case \in RequestCases:
    ActualRequestViewChangeWindow(case) = SpecRequestViewChangeWindow(case)

RequestFieldsMatch ==
  \A case \in RequestCases:
    ActualRequestFields(case) = SpecRequestFields(case)

RequestRosterProofMatches ==
  \A case \in RequestCases:
    ActualRequestRosterProof(case) = SpecRequestRosterProof(case)

StaleConflictPredicateMatches ==
  \A case \in DropCases:
    ActualStaleConflict(case) = SpecStaleConflict(case)

DropRequestAttemptMatches ==
  \A case \in DropCases:
    ActualDropRequestAttempt(case) = SpecDropRequestAttempt(case)

DropFetchMatches ==
  \A case \in DropCases:
    ActualDropFetch(case) = SpecDropFetch(case)

DropClearMatches ==
  \A case \in DropCases:
    /\ ActualDropClearMissing(case) = SpecDropClearMissing(case)
    /\ ActualDropClearViewChange(case) = SpecDropClearViewChange(case)

DropLockMatches ==
  \A case \in DropCases:
    ActualDropLockAfter(case) = SpecDropLockAfter(case)

DropHighestMatches ==
  \A case \in DropCases:
    ActualDropHighestAfter(case) = SpecDropHighestAfter(case)

DropPruneMatches ==
  \A case \in DropCases:
    ActualDropPruneVotes(case) = SpecDropPruneVotes(case)

DropReanchorMatches ==
  \A case \in DropCases:
    ActualDropReanchor(case) = SpecDropReanchor(case)

MissingLockedQcRecoveryExactness ==
  /\ RequestReturnMatches
  /\ RequestFetchMatches
  /\ RequestRosterMatches
  /\ RequestViewWindowMatches
  /\ RequestFieldsMatch
  /\ RequestRosterProofMatches
  /\ StaleConflictPredicateMatches
  /\ DropRequestAttemptMatches
  /\ DropFetchMatches
  /\ DropClearMatches
  /\ DropLockMatches
  /\ DropHighestMatches
  /\ DropPruneMatches
  /\ DropReanchorMatches

MissingLockedQcRecoveryCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ MissingLockedQcRecoveryExactness

Init ==
  checked = 0

Next ==
  \/ /\ checked = 0
     /\ checked' = 1
  \/ /\ checked = 1
     /\ UNCHANGED vars

=============================================================================
====
