---- MODULE SumeragiEngineCommitQcStatePreservationGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for Commit-QC unrelated-state preservation.

This slice models the state fields that `ConsensusEngine::on_commit_qc(...)`
must not touch. Accepted Commit QCs may record highest-QC state, clear
validation ownership, commit an available payload, or enter pending finality
for a missing payload; those effects are covered by companion models. This
model proves that accepted Commit QCs preserve the current round, locked QC,
Prepare-QC replay cache, and available-payload store exactly. Commit QCs
rejected by the shared certificate prefilter or by pending-finality
replay/conflict guards preserve those same fields exactly.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAcceptedUpdatesRound,
  \* @type: Bool;
  BugAcceptedClearsLock,
  \* @type: Bool;
  BugAcceptedChangesLock,
  \* @type: Bool;
  BugAcceptedClearsPrepareVote,
  \* @type: Bool;
  BugAcceptedChangesPrepareVote,
  \* @type: Bool;
  BugAcceptedDropsAvailablePayload,
  \* @type: Bool;
  BugAcceptedRecordsExtraPayload,
  \* @type: Bool;
  BugRejectedUpdatesRound,
  \* @type: Bool;
  BugRejectedClearsLock,
  \* @type: Bool;
  BugRejectedChangesLock,
  \* @type: Bool;
  BugRejectedClearsPrepareVote,
  \* @type: Bool;
  BugRejectedChangesPrepareVote,
  \* @type: Bool;
  BugRejectedDropsAvailablePayload,
  \* @type: Bool;
  BugRejectedRecordsExtraPayload

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "safe_available_clean",
  "safe_available_with_lock",
  "safe_available_with_prepare_vote",
  "safe_missing_clean",
  "safe_missing_with_lock",
  "safe_missing_with_unrelated_payload",
  "wrong_height",
  "wrong_epoch",
  "wrong_validator_set",
  "wrong_quorum_policy",
  "stale_view",
  "committed_height",
  "pending_replay",
  "pending_conflict"
}

RoundValues == {"round_current", "round_next"}
LockValues == {"none", "lock_prepare_a", "lock_prepare_b"}
PrepareVoteValues == {"none", "prepare_vote_a", "prepare_vote_b"}
AvailableStoreValues == {
  "none",
  "certified_payload",
  "unrelated_payload",
  "certified_and_unrelated",
  "wrong_payload_store"
}

Accepted(candidate) ==
  candidate \in {
    "safe_available_clean",
    "safe_available_with_lock",
    "safe_available_with_prepare_vote",
    "safe_missing_clean",
    "safe_missing_with_lock",
    "safe_missing_with_unrelated_payload"
  }

RejectedPrefilter(candidate) ==
  candidate \in {
    "wrong_height",
    "wrong_epoch",
    "wrong_validator_set",
    "wrong_quorum_policy",
    "stale_view",
    "committed_height"
  }

PendingReplayConflict(candidate) ==
  candidate \in {"pending_replay", "pending_conflict"}

Rejected(candidate) ==
  RejectedPrefilter(candidate) \/ PendingReplayConflict(candidate)

InitialRound(candidate) == "round_current"

InitialLockedQc(candidate) ==
  IF candidate \in {
    "safe_available_with_lock",
    "safe_missing_with_lock",
    "stale_view",
    "pending_replay",
    "pending_conflict"
  }
  THEN "lock_prepare_a"
  ELSE "none"

InitialPrepareVoteCache(candidate) ==
  IF candidate \in {
    "safe_available_with_prepare_vote",
    "pending_replay",
    "pending_conflict"
  }
  THEN "prepare_vote_a"
  ELSE "none"

InitialAvailablePayloads(candidate) ==
  CASE candidate \in {
      "safe_available_clean",
      "safe_available_with_lock",
      "safe_available_with_prepare_vote"
    } -> "certified_payload"
    [] candidate \in {
      "safe_missing_with_unrelated_payload",
      "wrong_height",
      "wrong_epoch",
      "pending_replay"
    } -> "unrelated_payload"
    [] candidate = "pending_conflict" -> "certified_and_unrelated"
    [] OTHER -> "none"

WrongLock(lock) ==
  IF lock = "lock_prepare_a" THEN "lock_prepare_b" ELSE "lock_prepare_a"

WrongPrepareVote(vote) ==
  IF vote = "prepare_vote_a" THEN "prepare_vote_b" ELSE "prepare_vote_a"

ExtraAvailablePayloads(store) ==
  CASE store = "none" -> "certified_payload"
    [] store = "certified_payload" -> "certified_and_unrelated"
    [] store = "unrelated_payload" -> "certified_and_unrelated"
    [] OTHER -> "wrong_payload_store"

ImplementationRound(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedUpdatesRound
    THEN "round_next"
    ELSE InitialRound(candidate)
  ELSE IF BugRejectedUpdatesRound
       THEN "round_next"
       ELSE InitialRound(candidate)

ImplementationLockedQc(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedClearsLock
    THEN "none"
    ELSE IF BugAcceptedChangesLock
         THEN WrongLock(InitialLockedQc(candidate))
         ELSE InitialLockedQc(candidate)
  ELSE IF BugRejectedClearsLock
       THEN "none"
       ELSE IF BugRejectedChangesLock
            THEN WrongLock(InitialLockedQc(candidate))
            ELSE InitialLockedQc(candidate)

ImplementationPrepareVoteCache(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedClearsPrepareVote
    THEN "none"
    ELSE IF BugAcceptedChangesPrepareVote
         THEN WrongPrepareVote(InitialPrepareVoteCache(candidate))
         ELSE InitialPrepareVoteCache(candidate)
  ELSE IF BugRejectedClearsPrepareVote
       THEN "none"
       ELSE IF BugRejectedChangesPrepareVote
            THEN WrongPrepareVote(InitialPrepareVoteCache(candidate))
            ELSE InitialPrepareVoteCache(candidate)

ImplementationAvailablePayloads(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugAcceptedDropsAvailablePayload
    THEN "none"
    ELSE IF BugAcceptedRecordsExtraPayload
         THEN ExtraAvailablePayloads(InitialAvailablePayloads(candidate))
         ELSE InitialAvailablePayloads(candidate)
  ELSE IF BugRejectedDropsAvailablePayload
       THEN "none"
       ELSE IF BugRejectedRecordsExtraPayload
            THEN ExtraAvailablePayloads(InitialAvailablePayloads(candidate))
            ELSE InitialAvailablePayloads(candidate)

TypeInvariant ==
  /\ BugAcceptedUpdatesRound \in BOOLEAN
  /\ BugAcceptedClearsLock \in BOOLEAN
  /\ BugAcceptedChangesLock \in BOOLEAN
  /\ BugAcceptedClearsPrepareVote \in BOOLEAN
  /\ BugAcceptedChangesPrepareVote \in BOOLEAN
  /\ BugAcceptedDropsAvailablePayload \in BOOLEAN
  /\ BugAcceptedRecordsExtraPayload \in BOOLEAN
  /\ BugRejectedUpdatesRound \in BOOLEAN
  /\ BugRejectedClearsLock \in BOOLEAN
  /\ BugRejectedChangesLock \in BOOLEAN
  /\ BugRejectedClearsPrepareVote \in BOOLEAN
  /\ BugRejectedChangesPrepareVote \in BOOLEAN
  /\ BugRejectedDropsAvailablePayload \in BOOLEAN
  /\ BugRejectedRecordsExtraPayload \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InitialRound(candidate) \in RoundValues
    /\ ImplementationRound(candidate) \in RoundValues
    /\ InitialLockedQc(candidate) \in LockValues
    /\ ImplementationLockedQc(candidate) \in LockValues
    /\ InitialPrepareVoteCache(candidate) \in PrepareVoteValues
    /\ ImplementationPrepareVoteCache(candidate) \in PrepareVoteValues
    /\ InitialAvailablePayloads(candidate) \in AvailableStoreValues
    /\ ImplementationAvailablePayloads(candidate) \in AvailableStoreValues

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

AcceptedPreservesRound ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationRound(candidate) = InitialRound(candidate)

AcceptedPreservesLockedQc ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationLockedQc(candidate) = InitialLockedQc(candidate)

AcceptedPreservesPrepareVoteCache ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationPrepareVoteCache(candidate) =
        InitialPrepareVoteCache(candidate)

AcceptedPreservesAvailablePayloads ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationAvailablePayloads(candidate) =
        InitialAvailablePayloads(candidate)

RejectedPreservesRound ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationRound(candidate) = InitialRound(candidate)

RejectedPreservesLockedQc ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationLockedQc(candidate) = InitialLockedQc(candidate)

RejectedPreservesPrepareVoteCache ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationPrepareVoteCache(candidate) =
        InitialPrepareVoteCache(candidate)

RejectedPreservesAvailablePayloads ==
  \A candidate \in tried:
    Rejected(candidate) =>
      ImplementationAvailablePayloads(candidate) =
        InitialAvailablePayloads(candidate)

AllModeledStatePreserved ==
  \A candidate \in tried:
    /\ ImplementationRound(candidate) = InitialRound(candidate)
    /\ ImplementationLockedQc(candidate) = InitialLockedQc(candidate)
    /\ ImplementationPrepareVoteCache(candidate) =
      InitialPrepareVoteCache(candidate)
    /\ ImplementationAvailablePayloads(candidate) =
      InitialAvailablePayloads(candidate)

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ InitialRound(candidate) \in RoundValues
    /\ ImplementationRound(candidate) \in RoundValues
    /\ InitialLockedQc(candidate) \in LockValues
    /\ ImplementationLockedQc(candidate) \in LockValues
    /\ InitialPrepareVoteCache(candidate) \in PrepareVoteValues
    /\ ImplementationPrepareVoteCache(candidate) \in PrepareVoteValues
    /\ InitialAvailablePayloads(candidate) \in AvailableStoreValues
    /\ ImplementationAvailablePayloads(candidate) \in AvailableStoreValues

Safety ==
  /\ AcceptedPreservesRound
  /\ AcceptedPreservesLockedQc
  /\ AcceptedPreservesPrepareVoteCache
  /\ AcceptedPreservesAvailablePayloads
  /\ RejectedPreservesRound
  /\ RejectedPreservesLockedQc
  /\ RejectedPreservesPrepareVoteCache
  /\ RejectedPreservesAvailablePayloads
  /\ AllModeledStatePreserved
  /\ ValuesStayInDomain

====
