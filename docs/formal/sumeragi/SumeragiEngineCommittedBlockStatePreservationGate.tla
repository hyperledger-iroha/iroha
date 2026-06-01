---- MODULE SumeragiEngineCommittedBlockStatePreservationGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for committed-block unrelated-state preservation.

This slice models the state fields that
`ConsensusEngine::on_committed_block(...)` must not touch. Companion models
cover committed-map recording, current-height validation/pending-finality
cleanup, and reconfiguration staging. This model proves that fresh,
duplicate, and conflicting committed-block notifications preserve the current
round, lock/highest-QC state, the Prepare-QC replay cache, and
available-payload state exactly.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugFreshMutatesRound,
  \* @type: Bool;
  BugFreshMutatesLock,
  \* @type: Bool;
  BugFreshMutatesHighest,
  \* @type: Bool;
  BugFreshMutatesPrepareVote,
  \* @type: Bool;
  BugFreshMutatesAvailablePayloads,
  \* @type: Bool;
  BugNoopMutatesRound,
  \* @type: Bool;
  BugNoopMutatesLock,
  \* @type: Bool;
  BugNoopMutatesHighest,
  \* @type: Bool;
  BugNoopMutatesPrepareVote,
  \* @type: Bool;
  BugNoopMutatesAvailablePayloads

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "fresh_current_clean",
  "fresh_current_with_all_state",
  "fresh_other_with_all_state",
  "fresh_boundary_with_all_state",
  "fresh_non_boundary_with_all_state",
  "duplicate_current_with_all_state",
  "duplicate_other_with_all_state",
  "conflict_current_with_all_state",
  "conflict_other_with_all_state"
}

Fresh(candidate) ==
  candidate \in {
    "fresh_current_clean",
    "fresh_current_with_all_state",
    "fresh_other_with_all_state",
    "fresh_boundary_with_all_state",
    "fresh_non_boundary_with_all_state"
  }

Noop(candidate) ==
  ~Fresh(candidate)

RoundValues == {"round_current", "round_other"}
LockValues == {"none", "lock_a", "lock_b"}
HighestValues == {"none", "highest_a", "highest_b"}
PrepareVoteValues == {"none", "prepare_vote_a", "prepare_vote_b"}
AvailableStoreValues == {"none", "payload_a", "payload_b", "payloads_ab"}

InitialRound(candidate) ==
  "round_current"

InitialLockedQc(candidate) ==
  IF candidate = "fresh_current_clean" THEN "none" ELSE "lock_a"

InitialHighestQc(candidate) ==
  IF candidate = "fresh_current_clean" THEN "none" ELSE "highest_a"

InitialPrepareVoteCache(candidate) ==
  IF candidate = "fresh_current_clean" THEN "none" ELSE "prepare_vote_a"

InitialAvailablePayloads(candidate) ==
  IF candidate = "fresh_current_clean" THEN "none" ELSE "payloads_ab"

MutatedRound(value) ==
  IF value = "round_current" THEN "round_other" ELSE "round_current"

MutatedLock(value) ==
  IF value = "lock_a" THEN "lock_b" ELSE "lock_a"

MutatedHighest(value) ==
  IF value = "highest_a" THEN "highest_b" ELSE "highest_a"

MutatedPrepareVote(value) ==
  IF value = "prepare_vote_a" THEN "prepare_vote_b" ELSE "prepare_vote_a"

MutatedAvailablePayloads(value) ==
  IF value = "payloads_ab" THEN "payload_a" ELSE "payloads_ab"

ImplementationRound(candidate) ==
  IF Fresh(candidate) /\ BugFreshMutatesRound
  THEN MutatedRound(InitialRound(candidate))
  ELSE IF Noop(candidate) /\ BugNoopMutatesRound
  THEN MutatedRound(InitialRound(candidate))
  ELSE InitialRound(candidate)

ImplementationLockedQc(candidate) ==
  IF Fresh(candidate) /\ BugFreshMutatesLock
  THEN MutatedLock(InitialLockedQc(candidate))
  ELSE IF Noop(candidate) /\ BugNoopMutatesLock
  THEN MutatedLock(InitialLockedQc(candidate))
  ELSE InitialLockedQc(candidate)

ImplementationHighestQc(candidate) ==
  IF Fresh(candidate) /\ BugFreshMutatesHighest
  THEN MutatedHighest(InitialHighestQc(candidate))
  ELSE IF Noop(candidate) /\ BugNoopMutatesHighest
  THEN MutatedHighest(InitialHighestQc(candidate))
  ELSE InitialHighestQc(candidate)

ImplementationPrepareVoteCache(candidate) ==
  IF Fresh(candidate) /\ BugFreshMutatesPrepareVote
  THEN MutatedPrepareVote(InitialPrepareVoteCache(candidate))
  ELSE IF Noop(candidate) /\ BugNoopMutatesPrepareVote
  THEN MutatedPrepareVote(InitialPrepareVoteCache(candidate))
  ELSE InitialPrepareVoteCache(candidate)

ImplementationAvailablePayloads(candidate) ==
  IF Fresh(candidate) /\ BugFreshMutatesAvailablePayloads
  THEN MutatedAvailablePayloads(InitialAvailablePayloads(candidate))
  ELSE IF Noop(candidate) /\ BugNoopMutatesAvailablePayloads
  THEN MutatedAvailablePayloads(InitialAvailablePayloads(candidate))
  ELSE InitialAvailablePayloads(candidate)

TypeInvariant ==
  /\ BugFreshMutatesRound \in BOOLEAN
  /\ BugFreshMutatesLock \in BOOLEAN
  /\ BugFreshMutatesHighest \in BOOLEAN
  /\ BugFreshMutatesPrepareVote \in BOOLEAN
  /\ BugFreshMutatesAvailablePayloads \in BOOLEAN
  /\ BugNoopMutatesRound \in BOOLEAN
  /\ BugNoopMutatesLock \in BOOLEAN
  /\ BugNoopMutatesHighest \in BOOLEAN
  /\ BugNoopMutatesPrepareVote \in BOOLEAN
  /\ BugNoopMutatesAvailablePayloads \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InitialRound(candidate) \in RoundValues
    /\ ImplementationRound(candidate) \in RoundValues
    /\ InitialLockedQc(candidate) \in LockValues
    /\ ImplementationLockedQc(candidate) \in LockValues
    /\ InitialHighestQc(candidate) \in HighestValues
    /\ ImplementationHighestQc(candidate) \in HighestValues
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

FreshPreservesRound ==
  \A candidate \in tried:
    Fresh(candidate) =>
      ImplementationRound(candidate) = InitialRound(candidate)

FreshPreservesLockedQc ==
  \A candidate \in tried:
    Fresh(candidate) =>
      ImplementationLockedQc(candidate) = InitialLockedQc(candidate)

FreshPreservesHighestQc ==
  \A candidate \in tried:
    Fresh(candidate) =>
      ImplementationHighestQc(candidate) = InitialHighestQc(candidate)

FreshPreservesPrepareVoteCache ==
  \A candidate \in tried:
    Fresh(candidate) =>
      ImplementationPrepareVoteCache(candidate) =
        InitialPrepareVoteCache(candidate)

FreshPreservesAvailablePayloads ==
  \A candidate \in tried:
    Fresh(candidate) =>
      ImplementationAvailablePayloads(candidate) =
        InitialAvailablePayloads(candidate)

NoopPreservesRound ==
  \A candidate \in tried:
    Noop(candidate) =>
      ImplementationRound(candidate) = InitialRound(candidate)

NoopPreservesLockedQc ==
  \A candidate \in tried:
    Noop(candidate) =>
      ImplementationLockedQc(candidate) = InitialLockedQc(candidate)

NoopPreservesHighestQc ==
  \A candidate \in tried:
    Noop(candidate) =>
      ImplementationHighestQc(candidate) = InitialHighestQc(candidate)

NoopPreservesPrepareVoteCache ==
  \A candidate \in tried:
    Noop(candidate) =>
      ImplementationPrepareVoteCache(candidate) =
        InitialPrepareVoteCache(candidate)

NoopPreservesAvailablePayloads ==
  \A candidate \in tried:
    Noop(candidate) =>
      ImplementationAvailablePayloads(candidate) =
        InitialAvailablePayloads(candidate)

AllModeledStatePreserved ==
  \A candidate \in tried:
    /\ ImplementationRound(candidate) = InitialRound(candidate)
    /\ ImplementationLockedQc(candidate) = InitialLockedQc(candidate)
    /\ ImplementationHighestQc(candidate) = InitialHighestQc(candidate)
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
    /\ InitialHighestQc(candidate) \in HighestValues
    /\ ImplementationHighestQc(candidate) \in HighestValues
    /\ InitialPrepareVoteCache(candidate) \in PrepareVoteValues
    /\ ImplementationPrepareVoteCache(candidate) \in PrepareVoteValues
    /\ InitialAvailablePayloads(candidate) \in AvailableStoreValues
    /\ ImplementationAvailablePayloads(candidate) \in AvailableStoreValues

Safety ==
  /\ FreshPreservesRound
  /\ FreshPreservesLockedQc
  /\ FreshPreservesHighestQc
  /\ FreshPreservesPrepareVoteCache
  /\ FreshPreservesAvailablePayloads
  /\ NoopPreservesRound
  /\ NoopPreservesLockedQc
  /\ NoopPreservesHighestQc
  /\ NoopPreservesPrepareVoteCache
  /\ NoopPreservesAvailablePayloads
  /\ AllModeledStatePreserved
  /\ ValuesStayInDomain

=============================================================================
====
