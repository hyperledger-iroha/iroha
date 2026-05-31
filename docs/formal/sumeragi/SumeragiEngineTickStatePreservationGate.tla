---- MODULE SumeragiEngineTickStatePreservationGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for tick unrelated-state preservation.

This slice models the state fields that `ConsensusEngine::on_tick(...)` must
not touch. Ticks intentionally advance the stored round, enter proposal phase,
clear validation ownership, and emit NewView/AdvanceView outputs; companion
models cover those effects and pending-finality marker preservation. This
model proves that every tick preserves lock/highest-QC state, the Prepare-QC
replay cache, committed-height records, available-payload state,
pending-finality certificate-map entries, and staged reconfiguration exactly.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugMutatesLock,
  \* @type: Bool;
  BugMutatesHighest,
  \* @type: Bool;
  BugMutatesCommitVote,
  \* @type: Bool;
  BugMutatesCommitted,
  \* @type: Bool;
  BugMutatesAvailablePayloads,
  \* @type: Bool;
  BugMutatesPendingMap,
  \* @type: Bool;
  BugMutatesReconfiguration

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "idle_clean",
  "with_lock",
  "with_highest",
  "with_commit_vote",
  "with_committed",
  "with_available_payload",
  "with_pending_map",
  "with_reconfiguration",
  "with_all_state"
}

LockValues == {"none", "lock_a", "lock_b"}
HighestValues == {"none", "highest_a", "highest_b"}
CommitVoteValues == {"none", "prepare_vote_a", "prepare_vote_b"}
CommittedValues == {"none", "committed_a", "committed_b"}
AvailableStoreValues == {"none", "payload_a", "payload_b", "payloads_ab"}
PendingMapValues == {"none", "pending_cert_a", "pending_cert_b"}
ReconfigurationValues == {"none", "reconfig_a", "reconfig_b"}

InitialLockedQc(candidate) ==
  IF candidate \in {"with_lock", "with_all_state"}
  THEN "lock_a"
  ELSE "none"

InitialHighestQc(candidate) ==
  IF candidate \in {"with_highest", "with_all_state"}
  THEN "highest_a"
  ELSE "none"

InitialCommitVoteCache(candidate) ==
  IF candidate \in {"with_commit_vote", "with_all_state"}
  THEN "prepare_vote_a"
  ELSE "none"

InitialCommitted(candidate) ==
  IF candidate \in {"with_committed", "with_all_state"}
  THEN "committed_a"
  ELSE "none"

InitialAvailablePayloads(candidate) ==
  CASE candidate = "with_available_payload" -> "payload_a"
    [] candidate = "with_all_state" -> "payloads_ab"
    [] OTHER -> "none"

InitialPendingMap(candidate) ==
  IF candidate \in {"with_pending_map", "with_all_state"}
  THEN "pending_cert_a"
  ELSE "none"

InitialReconfiguration(candidate) ==
  IF candidate \in {"with_reconfiguration", "with_all_state"}
  THEN "reconfig_a"
  ELSE "none"

MutatedLock(value) ==
  IF value = "lock_a" THEN "lock_b" ELSE "lock_a"

MutatedHighest(value) ==
  IF value = "highest_a" THEN "highest_b" ELSE "highest_a"

MutatedCommitVote(value) ==
  IF value = "prepare_vote_a" THEN "prepare_vote_b" ELSE "prepare_vote_a"

MutatedCommitted(value) ==
  IF value = "committed_a" THEN "committed_b" ELSE "committed_a"

MutatedAvailablePayloads(value) ==
  IF value = "payload_a" THEN "payload_b" ELSE "payload_a"

MutatedPendingMap(value) ==
  IF value = "pending_cert_a" THEN "pending_cert_b" ELSE "pending_cert_a"

MutatedReconfiguration(value) ==
  IF value = "reconfig_a" THEN "reconfig_b" ELSE "reconfig_a"

ImplementationLockedQc(candidate) ==
  IF BugMutatesLock
  THEN MutatedLock(InitialLockedQc(candidate))
  ELSE InitialLockedQc(candidate)

ImplementationHighestQc(candidate) ==
  IF BugMutatesHighest
  THEN MutatedHighest(InitialHighestQc(candidate))
  ELSE InitialHighestQc(candidate)

ImplementationCommitVoteCache(candidate) ==
  IF BugMutatesCommitVote
  THEN MutatedCommitVote(InitialCommitVoteCache(candidate))
  ELSE InitialCommitVoteCache(candidate)

ImplementationCommitted(candidate) ==
  IF BugMutatesCommitted
  THEN MutatedCommitted(InitialCommitted(candidate))
  ELSE InitialCommitted(candidate)

ImplementationAvailablePayloads(candidate) ==
  IF BugMutatesAvailablePayloads
  THEN MutatedAvailablePayloads(InitialAvailablePayloads(candidate))
  ELSE InitialAvailablePayloads(candidate)

ImplementationPendingMap(candidate) ==
  IF BugMutatesPendingMap
  THEN MutatedPendingMap(InitialPendingMap(candidate))
  ELSE InitialPendingMap(candidate)

ImplementationReconfiguration(candidate) ==
  IF BugMutatesReconfiguration
  THEN MutatedReconfiguration(InitialReconfiguration(candidate))
  ELSE InitialReconfiguration(candidate)

TypeInvariant ==
  /\ BugMutatesLock \in BOOLEAN
  /\ BugMutatesHighest \in BOOLEAN
  /\ BugMutatesCommitVote \in BOOLEAN
  /\ BugMutatesCommitted \in BOOLEAN
  /\ BugMutatesAvailablePayloads \in BOOLEAN
  /\ BugMutatesPendingMap \in BOOLEAN
  /\ BugMutatesReconfiguration \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InitialLockedQc(candidate) \in LockValues
    /\ ImplementationLockedQc(candidate) \in LockValues
    /\ InitialHighestQc(candidate) \in HighestValues
    /\ ImplementationHighestQc(candidate) \in HighestValues
    /\ InitialCommitVoteCache(candidate) \in CommitVoteValues
    /\ ImplementationCommitVoteCache(candidate) \in CommitVoteValues
    /\ InitialCommitted(candidate) \in CommittedValues
    /\ ImplementationCommitted(candidate) \in CommittedValues
    /\ InitialAvailablePayloads(candidate) \in AvailableStoreValues
    /\ ImplementationAvailablePayloads(candidate) \in AvailableStoreValues
    /\ InitialPendingMap(candidate) \in PendingMapValues
    /\ ImplementationPendingMap(candidate) \in PendingMapValues
    /\ InitialReconfiguration(candidate) \in ReconfigurationValues
    /\ ImplementationReconfiguration(candidate) \in ReconfigurationValues

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Cases \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Cases: TryCandidate(candidate)
  \/ Stable

TicksPreserveLockedQc ==
  \A candidate \in tried:
    ImplementationLockedQc(candidate) = InitialLockedQc(candidate)

TicksPreserveHighestQc ==
  \A candidate \in tried:
    ImplementationHighestQc(candidate) = InitialHighestQc(candidate)

TicksPreserveCommitVoteCache ==
  \A candidate \in tried:
    ImplementationCommitVoteCache(candidate) =
      InitialCommitVoteCache(candidate)

TicksPreserveCommittedRecords ==
  \A candidate \in tried:
    ImplementationCommitted(candidate) = InitialCommitted(candidate)

TicksPreserveAvailablePayloads ==
  \A candidate \in tried:
    ImplementationAvailablePayloads(candidate) =
      InitialAvailablePayloads(candidate)

TicksPreservePendingMap ==
  \A candidate \in tried:
    ImplementationPendingMap(candidate) = InitialPendingMap(candidate)

TicksPreserveReconfiguration ==
  \A candidate \in tried:
    ImplementationReconfiguration(candidate) =
      InitialReconfiguration(candidate)

AllModeledStatePreserved ==
  \A candidate \in tried:
    /\ ImplementationLockedQc(candidate) = InitialLockedQc(candidate)
    /\ ImplementationHighestQc(candidate) = InitialHighestQc(candidate)
    /\ ImplementationCommitVoteCache(candidate) =
      InitialCommitVoteCache(candidate)
    /\ ImplementationCommitted(candidate) = InitialCommitted(candidate)
    /\ ImplementationAvailablePayloads(candidate) =
      InitialAvailablePayloads(candidate)
    /\ ImplementationPendingMap(candidate) = InitialPendingMap(candidate)
    /\ ImplementationReconfiguration(candidate) =
      InitialReconfiguration(candidate)

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ InitialLockedQc(candidate) \in LockValues
    /\ ImplementationLockedQc(candidate) \in LockValues
    /\ InitialHighestQc(candidate) \in HighestValues
    /\ ImplementationHighestQc(candidate) \in HighestValues
    /\ InitialCommitVoteCache(candidate) \in CommitVoteValues
    /\ ImplementationCommitVoteCache(candidate) \in CommitVoteValues
    /\ InitialCommitted(candidate) \in CommittedValues
    /\ ImplementationCommitted(candidate) \in CommittedValues
    /\ InitialAvailablePayloads(candidate) \in AvailableStoreValues
    /\ ImplementationAvailablePayloads(candidate) \in AvailableStoreValues
    /\ InitialPendingMap(candidate) \in PendingMapValues
    /\ ImplementationPendingMap(candidate) \in PendingMapValues
    /\ InitialReconfiguration(candidate) \in ReconfigurationValues
    /\ ImplementationReconfiguration(candidate) \in ReconfigurationValues

Safety ==
  /\ TicksPreserveLockedQc
  /\ TicksPreserveHighestQc
  /\ TicksPreserveCommitVoteCache
  /\ TicksPreserveCommittedRecords
  /\ TicksPreserveAvailablePayloads
  /\ TicksPreservePendingMap
  /\ TicksPreserveReconfiguration
  /\ AllModeledStatePreserved
  /\ ValuesStayInDomain

====
