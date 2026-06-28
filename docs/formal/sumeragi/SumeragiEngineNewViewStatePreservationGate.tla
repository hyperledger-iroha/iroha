---- MODULE SumeragiEngineNewViewStatePreservationGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for NewView-QC unrelated-state preservation.

This slice models the state fields that `ConsensusEngine::on_new_view_qc(...)`
must not touch. Accepted NewView QCs may record carried highest-QC evidence,
advance the stored round, return to proposal phase, clear validation ownership,
and emit `AdvanceView`; those effects are covered by companion models. This
model proves that accepted and rejected NewView QCs preserve the locked QC,
Prepare-QC replay cache, committed-height records, available-payload store,
pending-finality certificate map, and staged reconfiguration exactly.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAcceptedMutatesLock,
  \* @type: Bool;
  BugAcceptedMutatesPrepareVote,
  \* @type: Bool;
  BugAcceptedMutatesCommitted,
  \* @type: Bool;
  BugAcceptedMutatesAvailablePayloads,
  \* @type: Bool;
  BugAcceptedMutatesPendingMap,
  \* @type: Bool;
  BugAcceptedMutatesReconfiguration,
  \* @type: Bool;
  BugRejectedMutatesLock,
  \* @type: Bool;
  BugRejectedMutatesPrepareVote,
  \* @type: Bool;
  BugRejectedMutatesCommitted,
  \* @type: Bool;
  BugRejectedMutatesAvailablePayloads,
  \* @type: Bool;
  BugRejectedMutatesPendingMap,
  \* @type: Bool;
  BugRejectedMutatesReconfiguration

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "safe_no_highest_clean",
  "safe_with_lock",
  "safe_with_prepare_vote",
  "safe_with_committed_record",
  "safe_with_available_payload",
  "safe_with_pending_map",
  "safe_with_reconfiguration",
  "wrong_height",
  "wrong_epoch",
  "wrong_validator_set",
  "wrong_quorum_policy",
  "same_view",
  "lower_view",
  "incompatible_highest",
  "committed_height"
}

LockValues == {"none", "lock_a", "lock_b"}
PrepareVoteValues == {"none", "prepare_vote_a", "prepare_vote_b"}
CommittedValues == {"none", "committed_a", "committed_b"}
AvailableStoreValues == {"none", "payload_a", "payload_b", "payloads_ab"}
PendingMapValues == {"none", "pending_cert_a", "pending_cert_b"}
ReconfigurationValues == {"none", "reconfig_a", "reconfig_b"}

Accepted(candidate) ==
  candidate \in {
    "safe_no_highest_clean",
    "safe_with_lock",
    "safe_with_prepare_vote",
    "safe_with_committed_record",
    "safe_with_available_payload",
    "safe_with_pending_map",
    "safe_with_reconfiguration"
  }

Rejected(candidate) ==
  candidate \in Cases /\ ~Accepted(candidate)

InitialLockedQc(candidate) ==
  IF candidate \in {
    "safe_with_lock",
    "same_view",
    "lower_view",
    "incompatible_highest"
  }
  THEN "lock_a"
  ELSE "none"

InitialPrepareVoteCache(candidate) ==
  IF candidate \in {
    "safe_with_prepare_vote",
    "wrong_height",
    "wrong_epoch",
    "same_view"
  }
  THEN "prepare_vote_a"
  ELSE "none"

InitialCommitted(candidate) ==
  IF candidate \in {
    "safe_with_committed_record",
    "committed_height",
    "wrong_validator_set"
  }
  THEN "committed_a"
  ELSE "none"

InitialAvailablePayloads(candidate) ==
  CASE candidate = "safe_with_available_payload" -> "payload_a"
    [] candidate \in {"wrong_quorum_policy", "same_view"} -> "payload_b"
    [] candidate = "incompatible_highest" -> "payloads_ab"
    [] OTHER -> "none"

InitialPendingMap(candidate) ==
  IF candidate \in {
    "safe_with_pending_map",
    "same_view",
    "committed_height"
  }
  THEN "pending_cert_a"
  ELSE "none"

InitialReconfiguration(candidate) ==
  IF candidate \in {
    "safe_with_reconfiguration",
    "lower_view",
    "wrong_epoch"
  }
  THEN "reconfig_a"
  ELSE "none"

MutatedLock(value) ==
  IF value = "lock_a" THEN "lock_b" ELSE "lock_a"

MutatedPrepareVote(value) ==
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
  IF Accepted(candidate)
  THEN
    IF BugAcceptedMutatesLock
    THEN MutatedLock(InitialLockedQc(candidate))
    ELSE InitialLockedQc(candidate)
  ELSE IF BugRejectedMutatesLock
       THEN MutatedLock(InitialLockedQc(candidate))
       ELSE InitialLockedQc(candidate)

ImplementationPrepareVoteCache(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedMutatesPrepareVote
    THEN MutatedPrepareVote(InitialPrepareVoteCache(candidate))
    ELSE InitialPrepareVoteCache(candidate)
  ELSE IF BugRejectedMutatesPrepareVote
       THEN MutatedPrepareVote(InitialPrepareVoteCache(candidate))
       ELSE InitialPrepareVoteCache(candidate)

ImplementationCommitted(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedMutatesCommitted
    THEN MutatedCommitted(InitialCommitted(candidate))
    ELSE InitialCommitted(candidate)
  ELSE IF BugRejectedMutatesCommitted
       THEN MutatedCommitted(InitialCommitted(candidate))
       ELSE InitialCommitted(candidate)

ImplementationAvailablePayloads(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedMutatesAvailablePayloads
    THEN MutatedAvailablePayloads(InitialAvailablePayloads(candidate))
    ELSE InitialAvailablePayloads(candidate)
  ELSE IF BugRejectedMutatesAvailablePayloads
       THEN MutatedAvailablePayloads(InitialAvailablePayloads(candidate))
       ELSE InitialAvailablePayloads(candidate)

ImplementationPendingMap(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedMutatesPendingMap
    THEN MutatedPendingMap(InitialPendingMap(candidate))
    ELSE InitialPendingMap(candidate)
  ELSE IF BugRejectedMutatesPendingMap
       THEN MutatedPendingMap(InitialPendingMap(candidate))
       ELSE InitialPendingMap(candidate)

ImplementationReconfiguration(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedMutatesReconfiguration
    THEN MutatedReconfiguration(InitialReconfiguration(candidate))
    ELSE InitialReconfiguration(candidate)
  ELSE IF BugRejectedMutatesReconfiguration
       THEN MutatedReconfiguration(InitialReconfiguration(candidate))
       ELSE InitialReconfiguration(candidate)

TypeInvariant ==
  /\ BugAcceptedMutatesLock \in BOOLEAN
  /\ BugAcceptedMutatesPrepareVote \in BOOLEAN
  /\ BugAcceptedMutatesCommitted \in BOOLEAN
  /\ BugAcceptedMutatesAvailablePayloads \in BOOLEAN
  /\ BugAcceptedMutatesPendingMap \in BOOLEAN
  /\ BugAcceptedMutatesReconfiguration \in BOOLEAN
  /\ BugRejectedMutatesLock \in BOOLEAN
  /\ BugRejectedMutatesPrepareVote \in BOOLEAN
  /\ BugRejectedMutatesCommitted \in BOOLEAN
  /\ BugRejectedMutatesAvailablePayloads \in BOOLEAN
  /\ BugRejectedMutatesPendingMap \in BOOLEAN
  /\ BugRejectedMutatesReconfiguration \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InitialLockedQc(candidate) \in LockValues
    /\ ImplementationLockedQc(candidate) \in LockValues
    /\ InitialPrepareVoteCache(candidate) \in PrepareVoteValues
    /\ ImplementationPrepareVoteCache(candidate) \in PrepareVoteValues
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

AcceptedPreservesLockedQc ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationLockedQc(candidate) = InitialLockedQc(candidate)

AcceptedPreservesPrepareVoteCache ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationPrepareVoteCache(candidate) =
        InitialPrepareVoteCache(candidate)

AcceptedPreservesCommittedRecords ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationCommitted(candidate) = InitialCommitted(candidate)

AcceptedPreservesAvailablePayloads ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationAvailablePayloads(candidate) =
        InitialAvailablePayloads(candidate)

AcceptedPreservesPendingMap ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationPendingMap(candidate) = InitialPendingMap(candidate)

AcceptedPreservesReconfiguration ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationReconfiguration(candidate) =
        InitialReconfiguration(candidate)

RejectedPreservesLockedQc ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationLockedQc(candidate) = InitialLockedQc(candidate)

RejectedPreservesPrepareVoteCache ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationPrepareVoteCache(candidate) =
        InitialPrepareVoteCache(candidate)

RejectedPreservesCommittedRecords ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationCommitted(candidate) = InitialCommitted(candidate)

RejectedPreservesAvailablePayloads ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationAvailablePayloads(candidate) =
        InitialAvailablePayloads(candidate)

RejectedPreservesPendingMap ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationPendingMap(candidate) = InitialPendingMap(candidate)

RejectedPreservesReconfiguration ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationReconfiguration(candidate) =
        InitialReconfiguration(candidate)

AllModeledStatePreserved ==
  \A candidate \in tried:
    /\ ImplementationLockedQc(candidate) = InitialLockedQc(candidate)
    /\ ImplementationPrepareVoteCache(candidate) =
      InitialPrepareVoteCache(candidate)
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
    /\ InitialPrepareVoteCache(candidate) \in PrepareVoteValues
    /\ ImplementationPrepareVoteCache(candidate) \in PrepareVoteValues
    /\ InitialCommitted(candidate) \in CommittedValues
    /\ ImplementationCommitted(candidate) \in CommittedValues
    /\ InitialAvailablePayloads(candidate) \in AvailableStoreValues
    /\ ImplementationAvailablePayloads(candidate) \in AvailableStoreValues
    /\ InitialPendingMap(candidate) \in PendingMapValues
    /\ ImplementationPendingMap(candidate) \in PendingMapValues
    /\ InitialReconfiguration(candidate) \in ReconfigurationValues
    /\ ImplementationReconfiguration(candidate) \in ReconfigurationValues

EngineNewViewStatePreservationExactness ==
  /\ AcceptedPreservesLockedQc
  /\ AcceptedPreservesPrepareVoteCache
  /\ AcceptedPreservesCommittedRecords
  /\ AcceptedPreservesAvailablePayloads
  /\ AcceptedPreservesPendingMap
  /\ AcceptedPreservesReconfiguration
  /\ RejectedPreservesLockedQc
  /\ RejectedPreservesPrepareVoteCache
  /\ RejectedPreservesCommittedRecords
  /\ RejectedPreservesAvailablePayloads
  /\ RejectedPreservesPendingMap
  /\ RejectedPreservesReconfiguration
  /\ AllModeledStatePreserved
  /\ ValuesStayInDomain

Safety ==
  EngineNewViewStatePreservationExactness

EngineNewViewStatePreservationCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineNewViewStatePreservationExactness

SafetyFast == EngineNewViewStatePreservationExactness

====
