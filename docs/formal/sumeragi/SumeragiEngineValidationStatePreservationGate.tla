---- MODULE SumeragiEngineValidationStatePreservationGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for validation-result unrelated-state preservation.

This slice models the state fields that
`ConsensusEngine::on_validation_result(...)` must not touch while accepting
or ignoring validation callbacks. The companion validation-result,
validation-ownership, and invalid-advance models cover acceptance, validation
owner cleanup, view advancement, phase changes, and emitted outputs.

Accepted current validation callbacks may clear the validation owner, and an
invalid callback may advance the round and emit NewView/AdvanceView outputs.
Accepted and ignored callbacks must still preserve the lock/highest-QC state,
the Prepare-QC replay cache, committed-height records, available-payload
state, pending-finality marker, pending-finality certificate-map entries, and
staged reconfiguration exactly.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAcceptedMutatesLock,
  \* @type: Bool;
  BugAcceptedMutatesHighest,
  \* @type: Bool;
  BugAcceptedMutatesCommitVote,
  \* @type: Bool;
  BugAcceptedMutatesCommitted,
  \* @type: Bool;
  BugAcceptedMutatesAvailablePayloads,
  \* @type: Bool;
  BugAcceptedMutatesPendingFinality,
  \* @type: Bool;
  BugAcceptedMutatesPendingMap,
  \* @type: Bool;
  BugAcceptedMutatesReconfiguration,
  \* @type: Bool;
  BugIgnoredMutatesLock,
  \* @type: Bool;
  BugIgnoredMutatesHighest,
  \* @type: Bool;
  BugIgnoredMutatesCommitVote,
  \* @type: Bool;
  BugIgnoredMutatesCommitted,
  \* @type: Bool;
  BugIgnoredMutatesAvailablePayloads,
  \* @type: Bool;
  BugIgnoredMutatesPendingFinality,
  \* @type: Bool;
  BugIgnoredMutatesPendingMap,
  \* @type: Bool;
  BugIgnoredMutatesReconfiguration

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "valid_clean",
  "valid_with_lock",
  "invalid_no_highest",
  "invalid_with_highest",
  "invalid_with_all_state",
  "wrong_round",
  "wrong_block",
  "no_inflight",
  "replay_after_valid",
  "superseded_by_commit",
  "superseded_by_committed",
  "ignored_with_all_state"
}

Accepted(candidate) ==
  candidate \in {
    "valid_clean",
    "valid_with_lock",
    "invalid_no_highest",
    "invalid_with_highest",
    "invalid_with_all_state"
  }

Ignored(candidate) ==
  ~Accepted(candidate)

LockValues == {"none", "lock_a", "lock_b"}
HighestValues == {"none", "highest_a", "highest_b"}
CommitVoteValues == {"none", "prepare_vote_a", "prepare_vote_b"}
CommittedValues == {"none", "committed_a", "committed_b"}
AvailableStoreValues == {"none", "payload_a", "payload_b", "payloads_ab"}
PendingFinalityValues == {"none", "pending_block_a", "pending_block_b"}
PendingMapValues == {"none", "pending_cert_a", "pending_cert_b"}
ReconfigurationValues == {"none", "reconfig_a", "reconfig_b"}

InitialLockedQc(candidate) ==
  IF candidate \in {
    "valid_with_lock",
    "invalid_with_all_state",
    "wrong_block",
    "ignored_with_all_state"
  }
  THEN "lock_a"
  ELSE "none"

InitialHighestQc(candidate) ==
  IF candidate \in {
    "invalid_with_highest",
    "invalid_with_all_state",
    "wrong_round",
    "ignored_with_all_state"
  }
  THEN "highest_a"
  ELSE "none"

InitialCommitVoteCache(candidate) ==
  IF candidate \in {
    "invalid_with_all_state",
    "wrong_round",
    "ignored_with_all_state"
  }
  THEN "prepare_vote_a"
  ELSE "none"

InitialCommitted(candidate) ==
  IF candidate \in {
    "invalid_with_all_state",
    "superseded_by_committed",
    "ignored_with_all_state"
  }
  THEN "committed_a"
  ELSE "none"

InitialAvailablePayloads(candidate) ==
  CASE candidate = "invalid_with_all_state" -> "payloads_ab"
    [] candidate \in {"wrong_block", "ignored_with_all_state"} -> "payload_a"
    [] OTHER -> "none"

InitialPendingFinality(candidate) ==
  IF candidate \in {
    "invalid_with_all_state",
    "superseded_by_commit",
    "ignored_with_all_state"
  }
  THEN "pending_block_a"
  ELSE "none"

InitialPendingMap(candidate) ==
  IF candidate \in {
    "invalid_with_all_state",
    "replay_after_valid",
    "ignored_with_all_state"
  }
  THEN "pending_cert_a"
  ELSE "none"

InitialReconfiguration(candidate) ==
  IF candidate \in {
    "invalid_with_all_state",
    "superseded_by_commit",
    "ignored_with_all_state"
  }
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

MutatedPendingFinality(value) ==
  IF value = "pending_block_a" THEN "pending_block_b" ELSE "pending_block_a"

MutatedPendingMap(value) ==
  IF value = "pending_cert_a" THEN "pending_cert_b" ELSE "pending_cert_a"

MutatedReconfiguration(value) ==
  IF value = "reconfig_a" THEN "reconfig_b" ELSE "reconfig_a"

ImplementationLockedQc(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedMutatesLock
  THEN MutatedLock(InitialLockedQc(candidate))
  ELSE IF Ignored(candidate) /\ BugIgnoredMutatesLock
  THEN MutatedLock(InitialLockedQc(candidate))
  ELSE InitialLockedQc(candidate)

ImplementationHighestQc(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedMutatesHighest
  THEN MutatedHighest(InitialHighestQc(candidate))
  ELSE IF Ignored(candidate) /\ BugIgnoredMutatesHighest
  THEN MutatedHighest(InitialHighestQc(candidate))
  ELSE InitialHighestQc(candidate)

ImplementationCommitVoteCache(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedMutatesCommitVote
  THEN MutatedCommitVote(InitialCommitVoteCache(candidate))
  ELSE IF Ignored(candidate) /\ BugIgnoredMutatesCommitVote
  THEN MutatedCommitVote(InitialCommitVoteCache(candidate))
  ELSE InitialCommitVoteCache(candidate)

ImplementationCommitted(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedMutatesCommitted
  THEN MutatedCommitted(InitialCommitted(candidate))
  ELSE IF Ignored(candidate) /\ BugIgnoredMutatesCommitted
  THEN MutatedCommitted(InitialCommitted(candidate))
  ELSE InitialCommitted(candidate)

ImplementationAvailablePayloads(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedMutatesAvailablePayloads
  THEN MutatedAvailablePayloads(InitialAvailablePayloads(candidate))
  ELSE IF Ignored(candidate) /\ BugIgnoredMutatesAvailablePayloads
  THEN MutatedAvailablePayloads(InitialAvailablePayloads(candidate))
  ELSE InitialAvailablePayloads(candidate)

ImplementationPendingFinality(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedMutatesPendingFinality
  THEN MutatedPendingFinality(InitialPendingFinality(candidate))
  ELSE IF Ignored(candidate) /\ BugIgnoredMutatesPendingFinality
  THEN MutatedPendingFinality(InitialPendingFinality(candidate))
  ELSE InitialPendingFinality(candidate)

ImplementationPendingMap(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedMutatesPendingMap
  THEN MutatedPendingMap(InitialPendingMap(candidate))
  ELSE IF Ignored(candidate) /\ BugIgnoredMutatesPendingMap
  THEN MutatedPendingMap(InitialPendingMap(candidate))
  ELSE InitialPendingMap(candidate)

ImplementationReconfiguration(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedMutatesReconfiguration
  THEN MutatedReconfiguration(InitialReconfiguration(candidate))
  ELSE IF Ignored(candidate) /\ BugIgnoredMutatesReconfiguration
  THEN MutatedReconfiguration(InitialReconfiguration(candidate))
  ELSE InitialReconfiguration(candidate)

TypeInvariant ==
  /\ BugAcceptedMutatesLock \in BOOLEAN
  /\ BugAcceptedMutatesHighest \in BOOLEAN
  /\ BugAcceptedMutatesCommitVote \in BOOLEAN
  /\ BugAcceptedMutatesCommitted \in BOOLEAN
  /\ BugAcceptedMutatesAvailablePayloads \in BOOLEAN
  /\ BugAcceptedMutatesPendingFinality \in BOOLEAN
  /\ BugAcceptedMutatesPendingMap \in BOOLEAN
  /\ BugAcceptedMutatesReconfiguration \in BOOLEAN
  /\ BugIgnoredMutatesLock \in BOOLEAN
  /\ BugIgnoredMutatesHighest \in BOOLEAN
  /\ BugIgnoredMutatesCommitVote \in BOOLEAN
  /\ BugIgnoredMutatesCommitted \in BOOLEAN
  /\ BugIgnoredMutatesAvailablePayloads \in BOOLEAN
  /\ BugIgnoredMutatesPendingFinality \in BOOLEAN
  /\ BugIgnoredMutatesPendingMap \in BOOLEAN
  /\ BugIgnoredMutatesReconfiguration \in BOOLEAN
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
    /\ InitialPendingFinality(candidate) \in PendingFinalityValues
    /\ ImplementationPendingFinality(candidate) \in PendingFinalityValues
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

AcceptedPreservesLockedQc ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationLockedQc(candidate) = InitialLockedQc(candidate)

AcceptedPreservesHighestQc ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationHighestQc(candidate) = InitialHighestQc(candidate)

AcceptedPreservesCommitVoteCache ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationCommitVoteCache(candidate) =
        InitialCommitVoteCache(candidate)

AcceptedPreservesCommittedRecords ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationCommitted(candidate) = InitialCommitted(candidate)

AcceptedPreservesAvailablePayloads ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationAvailablePayloads(candidate) =
        InitialAvailablePayloads(candidate)

AcceptedPreservesPendingFinality ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationPendingFinality(candidate) =
        InitialPendingFinality(candidate)

AcceptedPreservesPendingMap ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationPendingMap(candidate) = InitialPendingMap(candidate)

AcceptedPreservesReconfiguration ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationReconfiguration(candidate) =
        InitialReconfiguration(candidate)

IgnoredPreservesLockedQc ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationLockedQc(candidate) = InitialLockedQc(candidate)

IgnoredPreservesHighestQc ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationHighestQc(candidate) = InitialHighestQc(candidate)

IgnoredPreservesCommitVoteCache ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationCommitVoteCache(candidate) =
        InitialCommitVoteCache(candidate)

IgnoredPreservesCommittedRecords ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationCommitted(candidate) = InitialCommitted(candidate)

IgnoredPreservesAvailablePayloads ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationAvailablePayloads(candidate) =
        InitialAvailablePayloads(candidate)

IgnoredPreservesPendingFinality ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationPendingFinality(candidate) =
        InitialPendingFinality(candidate)

IgnoredPreservesPendingMap ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationPendingMap(candidate) = InitialPendingMap(candidate)

IgnoredPreservesReconfiguration ==
  \A candidate \in tried:
    Ignored(candidate) =>
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
    /\ ImplementationPendingFinality(candidate) =
      InitialPendingFinality(candidate)
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
    /\ InitialPendingFinality(candidate) \in PendingFinalityValues
    /\ ImplementationPendingFinality(candidate) \in PendingFinalityValues
    /\ InitialPendingMap(candidate) \in PendingMapValues
    /\ ImplementationPendingMap(candidate) \in PendingMapValues
    /\ InitialReconfiguration(candidate) \in ReconfigurationValues
    /\ ImplementationReconfiguration(candidate) \in ReconfigurationValues

Safety ==
  /\ AcceptedPreservesLockedQc
  /\ AcceptedPreservesHighestQc
  /\ AcceptedPreservesCommitVoteCache
  /\ AcceptedPreservesCommittedRecords
  /\ AcceptedPreservesAvailablePayloads
  /\ AcceptedPreservesPendingFinality
  /\ AcceptedPreservesPendingMap
  /\ AcceptedPreservesReconfiguration
  /\ IgnoredPreservesLockedQc
  /\ IgnoredPreservesHighestQc
  /\ IgnoredPreservesCommitVoteCache
  /\ IgnoredPreservesCommittedRecords
  /\ IgnoredPreservesAvailablePayloads
  /\ IgnoredPreservesPendingFinality
  /\ IgnoredPreservesPendingMap
  /\ IgnoredPreservesReconfiguration
  /\ AllModeledStatePreserved
  /\ ValuesStayInDomain

=============================================================================
