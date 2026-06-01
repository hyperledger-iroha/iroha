---- MODULE SumeragiEngineCertificatePrefilterStatePreservationGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for certificate prefilter unrelated-state
preservation.

This slice models the state fields that
`ConsensusEngine::on_certificate(...)` must not touch while applying the
shared certificate prefilter. Accepted certificates are dispatched to the
phase-specific handlers with these fields unchanged; rejected certificates
return before any mutation. Companion models cover prefilter dispatch,
prefilter-visible state handoff, and the phase-specific Prepare/Commit/NewView
handler effects.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAcceptedMutatesCommitted,
  \* @type: Bool;
  BugAcceptedMutatesPrepareVote,
  \* @type: Bool;
  BugAcceptedMutatesPendingMap,
  \* @type: Bool;
  BugAcceptedMutatesAvailablePayloads,
  \* @type: Bool;
  BugAcceptedMutatesReconfiguration,
  \* @type: Bool;
  BugRejectedMutatesCommitted,
  \* @type: Bool;
  BugRejectedMutatesPrepareVote,
  \* @type: Bool;
  BugRejectedMutatesPendingMap,
  \* @type: Bool;
  BugRejectedMutatesAvailablePayloads,
  \* @type: Bool;
  BugRejectedMutatesReconfiguration

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "current_prepare_clean",
  "current_prepare_with_all_state",
  "current_commit_with_all_state",
  "new_view_lower_view_with_all_state",
  "new_view_same_view_with_all_state",
  "new_view_future_view_with_all_state",
  "stale_prepare_with_all_state",
  "stale_commit_with_all_state",
  "committed_prepare_with_all_state",
  "committed_new_view_with_all_state",
  "wrong_height_prepare_clean",
  "wrong_height_commit_with_all_state",
  "wrong_epoch_new_view_with_all_state",
  "wrong_validator_set_commit_with_all_state",
  "wrong_quorum_new_view_with_all_state"
}

Accepted(candidate) ==
  candidate \in {
    "current_prepare_clean",
    "current_prepare_with_all_state",
    "current_commit_with_all_state",
    "new_view_lower_view_with_all_state",
    "new_view_same_view_with_all_state",
    "new_view_future_view_with_all_state"
  }

Rejected(candidate) ==
  ~Accepted(candidate)

CommittedValues == {"none", "committed_a", "committed_b"}
PrepareVoteValues == {"none", "prepare_vote_a", "prepare_vote_b"}
PendingMapValues == {"none", "pending_cert_a", "pending_cert_b"}
AvailableStoreValues == {"none", "payload_a", "payload_b", "payloads_ab"}
ReconfigurationValues == {"none", "reconfig_a", "reconfig_b"}

HasAllState(candidate) ==
  candidate \notin {"current_prepare_clean", "wrong_height_prepare_clean"}

InitialCommitted(candidate) ==
  IF HasAllState(candidate) THEN "committed_a" ELSE "none"

InitialPrepareVoteCache(candidate) ==
  IF HasAllState(candidate) THEN "prepare_vote_a" ELSE "none"

InitialPendingMap(candidate) ==
  IF HasAllState(candidate) THEN "pending_cert_a" ELSE "none"

InitialAvailablePayloads(candidate) ==
  IF HasAllState(candidate) THEN "payloads_ab" ELSE "none"

InitialReconfiguration(candidate) ==
  IF HasAllState(candidate) THEN "reconfig_a" ELSE "none"

MutatedCommitted(value) ==
  IF value = "committed_a" THEN "committed_b" ELSE "committed_a"

MutatedPrepareVote(value) ==
  IF value = "prepare_vote_a" THEN "prepare_vote_b" ELSE "prepare_vote_a"

MutatedPendingMap(value) ==
  IF value = "pending_cert_a" THEN "pending_cert_b" ELSE "pending_cert_a"

MutatedAvailablePayloads(value) ==
  IF value = "payloads_ab" THEN "payload_a" ELSE "payloads_ab"

MutatedReconfiguration(value) ==
  IF value = "reconfig_a" THEN "reconfig_b" ELSE "reconfig_a"

ImplementationCommitted(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedMutatesCommitted
  THEN MutatedCommitted(InitialCommitted(candidate))
  ELSE IF Rejected(candidate) /\ BugRejectedMutatesCommitted
  THEN MutatedCommitted(InitialCommitted(candidate))
  ELSE InitialCommitted(candidate)

ImplementationPrepareVoteCache(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedMutatesPrepareVote
  THEN MutatedPrepareVote(InitialPrepareVoteCache(candidate))
  ELSE IF Rejected(candidate) /\ BugRejectedMutatesPrepareVote
  THEN MutatedPrepareVote(InitialPrepareVoteCache(candidate))
  ELSE InitialPrepareVoteCache(candidate)

ImplementationPendingMap(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedMutatesPendingMap
  THEN MutatedPendingMap(InitialPendingMap(candidate))
  ELSE IF Rejected(candidate) /\ BugRejectedMutatesPendingMap
  THEN MutatedPendingMap(InitialPendingMap(candidate))
  ELSE InitialPendingMap(candidate)

ImplementationAvailablePayloads(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedMutatesAvailablePayloads
  THEN MutatedAvailablePayloads(InitialAvailablePayloads(candidate))
  ELSE IF Rejected(candidate) /\ BugRejectedMutatesAvailablePayloads
  THEN MutatedAvailablePayloads(InitialAvailablePayloads(candidate))
  ELSE InitialAvailablePayloads(candidate)

ImplementationReconfiguration(candidate) ==
  IF Accepted(candidate) /\ BugAcceptedMutatesReconfiguration
  THEN MutatedReconfiguration(InitialReconfiguration(candidate))
  ELSE IF Rejected(candidate) /\ BugRejectedMutatesReconfiguration
  THEN MutatedReconfiguration(InitialReconfiguration(candidate))
  ELSE InitialReconfiguration(candidate)

TypeInvariant ==
  /\ BugAcceptedMutatesCommitted \in BOOLEAN
  /\ BugAcceptedMutatesPrepareVote \in BOOLEAN
  /\ BugAcceptedMutatesPendingMap \in BOOLEAN
  /\ BugAcceptedMutatesAvailablePayloads \in BOOLEAN
  /\ BugAcceptedMutatesReconfiguration \in BOOLEAN
  /\ BugRejectedMutatesCommitted \in BOOLEAN
  /\ BugRejectedMutatesPrepareVote \in BOOLEAN
  /\ BugRejectedMutatesPendingMap \in BOOLEAN
  /\ BugRejectedMutatesAvailablePayloads \in BOOLEAN
  /\ BugRejectedMutatesReconfiguration \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InitialCommitted(candidate) \in CommittedValues
    /\ ImplementationCommitted(candidate) \in CommittedValues
    /\ InitialPrepareVoteCache(candidate) \in PrepareVoteValues
    /\ ImplementationPrepareVoteCache(candidate) \in PrepareVoteValues
    /\ InitialPendingMap(candidate) \in PendingMapValues
    /\ ImplementationPendingMap(candidate) \in PendingMapValues
    /\ InitialAvailablePayloads(candidate) \in AvailableStoreValues
    /\ ImplementationAvailablePayloads(candidate) \in AvailableStoreValues
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

AcceptedPreservesCommitted ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationCommitted(candidate) = InitialCommitted(candidate)

AcceptedPreservesPrepareVoteCache ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationPrepareVoteCache(candidate) =
        InitialPrepareVoteCache(candidate)

AcceptedPreservesPendingMap ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationPendingMap(candidate) = InitialPendingMap(candidate)

AcceptedPreservesAvailablePayloads ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationAvailablePayloads(candidate) =
        InitialAvailablePayloads(candidate)

AcceptedPreservesReconfiguration ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationReconfiguration(candidate) =
        InitialReconfiguration(candidate)

RejectedPreservesCommitted ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationCommitted(candidate) = InitialCommitted(candidate)

RejectedPreservesPrepareVoteCache ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationPrepareVoteCache(candidate) =
        InitialPrepareVoteCache(candidate)

RejectedPreservesPendingMap ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationPendingMap(candidate) = InitialPendingMap(candidate)

RejectedPreservesAvailablePayloads ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationAvailablePayloads(candidate) =
        InitialAvailablePayloads(candidate)

RejectedPreservesReconfiguration ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationReconfiguration(candidate) =
        InitialReconfiguration(candidate)

AllModeledStatePreserved ==
  \A candidate \in tried:
    /\ ImplementationCommitted(candidate) = InitialCommitted(candidate)
    /\ ImplementationPrepareVoteCache(candidate) =
      InitialPrepareVoteCache(candidate)
    /\ ImplementationPendingMap(candidate) = InitialPendingMap(candidate)
    /\ ImplementationAvailablePayloads(candidate) =
      InitialAvailablePayloads(candidate)
    /\ ImplementationReconfiguration(candidate) =
      InitialReconfiguration(candidate)

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ InitialCommitted(candidate) \in CommittedValues
    /\ ImplementationCommitted(candidate) \in CommittedValues
    /\ InitialPrepareVoteCache(candidate) \in PrepareVoteValues
    /\ ImplementationPrepareVoteCache(candidate) \in PrepareVoteValues
    /\ InitialPendingMap(candidate) \in PendingMapValues
    /\ ImplementationPendingMap(candidate) \in PendingMapValues
    /\ InitialAvailablePayloads(candidate) \in AvailableStoreValues
    /\ ImplementationAvailablePayloads(candidate) \in AvailableStoreValues
    /\ InitialReconfiguration(candidate) \in ReconfigurationValues
    /\ ImplementationReconfiguration(candidate) \in ReconfigurationValues

Safety ==
  /\ AcceptedPreservesCommitted
  /\ AcceptedPreservesPrepareVoteCache
  /\ AcceptedPreservesPendingMap
  /\ AcceptedPreservesAvailablePayloads
  /\ AcceptedPreservesReconfiguration
  /\ RejectedPreservesCommitted
  /\ RejectedPreservesPrepareVoteCache
  /\ RejectedPreservesPendingMap
  /\ RejectedPreservesAvailablePayloads
  /\ RejectedPreservesReconfiguration
  /\ AllModeledStatePreserved
  /\ ValuesStayInDomain

=============================================================================
====
