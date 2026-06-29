---- MODULE SumeragiEnginePayloadStatePreservationGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for payload-availability unrelated-state
preservation.

This slice models state that `ConsensusEngine::on_payload_available(...)`
must not touch. Companion payload-availability models cover the intentional
available-payload record, exact pending-QC matching, pending cleanup, commit
recording, and output/phase effects. This model proves that every
payload-availability callback preserves current round, lock/highest-QC state,
the Prepare-QC replay cache, and staged reconfiguration. Ignored no-pending
and mismatched callbacks must additionally preserve committed records,
validation ownership, phase, pending-finality marker, and pending-finality
certificate map exactly.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugCommitMutatesRound,
  \* @type: Bool;
  BugCommitMutatesLock,
  \* @type: Bool;
  BugCommitMutatesHighest,
  \* @type: Bool;
  BugCommitMutatesPrepareVote,
  \* @type: Bool;
  BugCommitMutatesReconfiguration,
  \* @type: Bool;
  BugIgnoredMutatesRound,
  \* @type: Bool;
  BugIgnoredMutatesLock,
  \* @type: Bool;
  BugIgnoredMutatesHighest,
  \* @type: Bool;
  BugIgnoredMutatesPrepareVote,
  \* @type: Bool;
  BugIgnoredMutatesReconfiguration,
  \* @type: Bool;
  BugIgnoredMutatesCommitted,
  \* @type: Bool;
  BugIgnoredMutatesValidation,
  \* @type: Bool;
  BugIgnoredMutatesPhase,
  \* @type: Bool;
  BugIgnoredMutatesPendingFinality,
  \* @type: Bool;
  BugIgnoredMutatesPendingMap

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "no_pending_clean",
  "no_pending_with_all_state",
  "matching_pending_clean",
  "matching_pending_with_all_state",
  "payload_hash_mismatch_with_all_state",
  "parent_mismatch_with_all_state",
  "unknown_block_with_all_state"
}

Commits(candidate) ==
  candidate \in {"matching_pending_clean", "matching_pending_with_all_state"}

Ignored(candidate) ==
  ~Commits(candidate)

RoundValues == {"round_current", "round_other"}
LockValues == {"none", "lock_a", "lock_b"}
HighestValues == {"none", "highest_a", "highest_b"}
PrepareVoteValues == {"none", "prepare_vote_a", "prepare_vote_b"}
ReconfigurationValues == {"none", "reconfig_a", "reconfig_b"}
CommittedValues == {"none", "committed_a", "committed_b"}
ValidationValues == {"none", "validating_a", "validating_b"}
PhaseValues == {"Proposal", "Prepare", "PendingFinality"}
PendingFinalityValues == {"none", "pending_block_a", "pending_block_b"}
PendingMapValues == {"none", "pending_cert_a", "pending_cert_b"}

InitialRound(candidate) ==
  "round_current"

HasAllState(candidate) ==
  candidate \in {
    "no_pending_with_all_state",
    "matching_pending_with_all_state",
    "payload_hash_mismatch_with_all_state",
    "parent_mismatch_with_all_state",
    "unknown_block_with_all_state"
  }

HasPending(candidate) ==
  candidate \in {
    "matching_pending_clean",
    "matching_pending_with_all_state",
    "payload_hash_mismatch_with_all_state",
    "parent_mismatch_with_all_state",
    "unknown_block_with_all_state"
  }

InitialLockedQc(candidate) ==
  IF HasAllState(candidate) THEN "lock_a" ELSE "none"

InitialHighestQc(candidate) ==
  IF HasAllState(candidate) THEN "highest_a" ELSE "none"

InitialPrepareVoteCache(candidate) ==
  IF HasAllState(candidate) THEN "prepare_vote_a" ELSE "none"

InitialReconfiguration(candidate) ==
  IF HasAllState(candidate) THEN "reconfig_a" ELSE "none"

InitialCommitted(candidate) ==
  IF HasAllState(candidate) THEN "committed_a" ELSE "none"

InitialValidation(candidate) ==
  IF candidate = "no_pending_clean" THEN "none" ELSE "validating_a"

InitialPhase(candidate) ==
  IF HasPending(candidate)
  THEN "PendingFinality"
  ELSE IF InitialValidation(candidate) # "none"
       THEN "Prepare"
       ELSE "Proposal"

InitialPendingFinality(candidate) ==
  IF HasPending(candidate) THEN "pending_block_a" ELSE "none"

InitialPendingMap(candidate) ==
  IF HasPending(candidate) THEN "pending_cert_a" ELSE "none"

MutatedRound(value) ==
  IF value = "round_current" THEN "round_other" ELSE "round_current"

MutatedLock(value) ==
  IF value = "lock_a" THEN "lock_b" ELSE "lock_a"

MutatedHighest(value) ==
  IF value = "highest_a" THEN "highest_b" ELSE "highest_a"

MutatedPrepareVote(value) ==
  IF value = "prepare_vote_a" THEN "prepare_vote_b" ELSE "prepare_vote_a"

MutatedReconfiguration(value) ==
  IF value = "reconfig_a" THEN "reconfig_b" ELSE "reconfig_a"

MutatedCommitted(value) ==
  IF value = "committed_a" THEN "committed_b" ELSE "committed_a"

MutatedValidation(value) ==
  IF value = "validating_a" THEN "validating_b" ELSE "validating_a"

MutatedPhase(value) ==
  IF value = "PendingFinality" THEN "Prepare" ELSE "PendingFinality"

MutatedPendingFinality(value) ==
  IF value = "pending_block_a" THEN "pending_block_b" ELSE "pending_block_a"

MutatedPendingMap(value) ==
  IF value = "pending_cert_a" THEN "pending_cert_b" ELSE "pending_cert_a"

ImplementationRound(candidate) ==
  IF Commits(candidate) /\ BugCommitMutatesRound
  THEN MutatedRound(InitialRound(candidate))
  ELSE IF Ignored(candidate) /\ BugIgnoredMutatesRound
  THEN MutatedRound(InitialRound(candidate))
  ELSE InitialRound(candidate)

ImplementationLockedQc(candidate) ==
  IF Commits(candidate) /\ BugCommitMutatesLock
  THEN MutatedLock(InitialLockedQc(candidate))
  ELSE IF Ignored(candidate) /\ BugIgnoredMutatesLock
  THEN MutatedLock(InitialLockedQc(candidate))
  ELSE InitialLockedQc(candidate)

ImplementationHighestQc(candidate) ==
  IF Commits(candidate) /\ BugCommitMutatesHighest
  THEN MutatedHighest(InitialHighestQc(candidate))
  ELSE IF Ignored(candidate) /\ BugIgnoredMutatesHighest
  THEN MutatedHighest(InitialHighestQc(candidate))
  ELSE InitialHighestQc(candidate)

ImplementationPrepareVoteCache(candidate) ==
  IF Commits(candidate) /\ BugCommitMutatesPrepareVote
  THEN MutatedPrepareVote(InitialPrepareVoteCache(candidate))
  ELSE IF Ignored(candidate) /\ BugIgnoredMutatesPrepareVote
  THEN MutatedPrepareVote(InitialPrepareVoteCache(candidate))
  ELSE InitialPrepareVoteCache(candidate)

ImplementationReconfiguration(candidate) ==
  IF Commits(candidate) /\ BugCommitMutatesReconfiguration
  THEN MutatedReconfiguration(InitialReconfiguration(candidate))
  ELSE IF Ignored(candidate) /\ BugIgnoredMutatesReconfiguration
  THEN MutatedReconfiguration(InitialReconfiguration(candidate))
  ELSE InitialReconfiguration(candidate)

ImplementationCommitted(candidate) ==
  IF Ignored(candidate) /\ BugIgnoredMutatesCommitted
  THEN MutatedCommitted(InitialCommitted(candidate))
  ELSE InitialCommitted(candidate)

ImplementationValidation(candidate) ==
  IF Ignored(candidate) /\ BugIgnoredMutatesValidation
  THEN MutatedValidation(InitialValidation(candidate))
  ELSE InitialValidation(candidate)

ImplementationPhase(candidate) ==
  IF Ignored(candidate) /\ BugIgnoredMutatesPhase
  THEN MutatedPhase(InitialPhase(candidate))
  ELSE InitialPhase(candidate)

ImplementationPendingFinality(candidate) ==
  IF Ignored(candidate) /\ BugIgnoredMutatesPendingFinality
  THEN MutatedPendingFinality(InitialPendingFinality(candidate))
  ELSE InitialPendingFinality(candidate)

ImplementationPendingMap(candidate) ==
  IF Ignored(candidate) /\ BugIgnoredMutatesPendingMap
  THEN MutatedPendingMap(InitialPendingMap(candidate))
  ELSE InitialPendingMap(candidate)

TypeInvariant ==
  /\ BugCommitMutatesRound \in BOOLEAN
  /\ BugCommitMutatesLock \in BOOLEAN
  /\ BugCommitMutatesHighest \in BOOLEAN
  /\ BugCommitMutatesPrepareVote \in BOOLEAN
  /\ BugCommitMutatesReconfiguration \in BOOLEAN
  /\ BugIgnoredMutatesRound \in BOOLEAN
  /\ BugIgnoredMutatesLock \in BOOLEAN
  /\ BugIgnoredMutatesHighest \in BOOLEAN
  /\ BugIgnoredMutatesPrepareVote \in BOOLEAN
  /\ BugIgnoredMutatesReconfiguration \in BOOLEAN
  /\ BugIgnoredMutatesCommitted \in BOOLEAN
  /\ BugIgnoredMutatesValidation \in BOOLEAN
  /\ BugIgnoredMutatesPhase \in BOOLEAN
  /\ BugIgnoredMutatesPendingFinality \in BOOLEAN
  /\ BugIgnoredMutatesPendingMap \in BOOLEAN
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
    /\ InitialReconfiguration(candidate) \in ReconfigurationValues
    /\ ImplementationReconfiguration(candidate) \in ReconfigurationValues
    /\ InitialCommitted(candidate) \in CommittedValues
    /\ ImplementationCommitted(candidate) \in CommittedValues
    /\ InitialValidation(candidate) \in ValidationValues
    /\ ImplementationValidation(candidate) \in ValidationValues
    /\ InitialPhase(candidate) \in PhaseValues
    /\ ImplementationPhase(candidate) \in PhaseValues
    /\ InitialPendingFinality(candidate) \in PendingFinalityValues
    /\ ImplementationPendingFinality(candidate) \in PendingFinalityValues
    /\ InitialPendingMap(candidate) \in PendingMapValues
    /\ ImplementationPendingMap(candidate) \in PendingMapValues

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

CommitsPreserveRound ==
  \A candidate \in tried:
    Commits(candidate) =>
      ImplementationRound(candidate) = InitialRound(candidate)

CommitsPreserveLockedQc ==
  \A candidate \in tried:
    Commits(candidate) =>
      ImplementationLockedQc(candidate) = InitialLockedQc(candidate)

CommitsPreserveHighestQc ==
  \A candidate \in tried:
    Commits(candidate) =>
      ImplementationHighestQc(candidate) = InitialHighestQc(candidate)

CommitsPreservePrepareVoteCache ==
  \A candidate \in tried:
    Commits(candidate) =>
      ImplementationPrepareVoteCache(candidate) =
        InitialPrepareVoteCache(candidate)

CommitsPreserveReconfiguration ==
  \A candidate \in tried:
    Commits(candidate) =>
      ImplementationReconfiguration(candidate) =
        InitialReconfiguration(candidate)

IgnoredPreserveRound ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationRound(candidate) = InitialRound(candidate)

IgnoredPreserveLockedQc ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationLockedQc(candidate) = InitialLockedQc(candidate)

IgnoredPreserveHighestQc ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationHighestQc(candidate) = InitialHighestQc(candidate)

IgnoredPreservePrepareVoteCache ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationPrepareVoteCache(candidate) =
        InitialPrepareVoteCache(candidate)

IgnoredPreserveReconfiguration ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationReconfiguration(candidate) =
        InitialReconfiguration(candidate)

IgnoredPreserveCommitted ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationCommitted(candidate) = InitialCommitted(candidate)

IgnoredPreserveValidation ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationValidation(candidate) = InitialValidation(candidate)

IgnoredPreservePhase ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationPhase(candidate) = InitialPhase(candidate)

IgnoredPreservePendingFinality ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationPendingFinality(candidate) =
        InitialPendingFinality(candidate)

IgnoredPreservePendingMap ==
  \A candidate \in tried:
    Ignored(candidate) =>
      ImplementationPendingMap(candidate) = InitialPendingMap(candidate)

AllAlwaysPreservedStatePreserved ==
  \A candidate \in tried:
    /\ ImplementationRound(candidate) = InitialRound(candidate)
    /\ ImplementationLockedQc(candidate) = InitialLockedQc(candidate)
    /\ ImplementationHighestQc(candidate) = InitialHighestQc(candidate)
    /\ ImplementationPrepareVoteCache(candidate) =
      InitialPrepareVoteCache(candidate)
    /\ ImplementationReconfiguration(candidate) =
      InitialReconfiguration(candidate)

IgnoredStatePreserved ==
  \A candidate \in tried:
    Ignored(candidate) =>
      /\ ImplementationCommitted(candidate) = InitialCommitted(candidate)
      /\ ImplementationValidation(candidate) = InitialValidation(candidate)
      /\ ImplementationPhase(candidate) = InitialPhase(candidate)
      /\ ImplementationPendingFinality(candidate) =
        InitialPendingFinality(candidate)
      /\ ImplementationPendingMap(candidate) = InitialPendingMap(candidate)

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
    /\ InitialReconfiguration(candidate) \in ReconfigurationValues
    /\ ImplementationReconfiguration(candidate) \in ReconfigurationValues
    /\ InitialCommitted(candidate) \in CommittedValues
    /\ ImplementationCommitted(candidate) \in CommittedValues
    /\ InitialValidation(candidate) \in ValidationValues
    /\ ImplementationValidation(candidate) \in ValidationValues
    /\ InitialPhase(candidate) \in PhaseValues
    /\ ImplementationPhase(candidate) \in PhaseValues
    /\ InitialPendingFinality(candidate) \in PendingFinalityValues
    /\ ImplementationPendingFinality(candidate) \in PendingFinalityValues
    /\ InitialPendingMap(candidate) \in PendingMapValues
    /\ ImplementationPendingMap(candidate) \in PendingMapValues

EnginePayloadStatePreservationExactness ==
  /\ CommitsPreserveRound
  /\ CommitsPreserveLockedQc
  /\ CommitsPreserveHighestQc
  /\ CommitsPreservePrepareVoteCache
  /\ CommitsPreserveReconfiguration
  /\ IgnoredPreserveRound
  /\ IgnoredPreserveLockedQc
  /\ IgnoredPreserveHighestQc
  /\ IgnoredPreservePrepareVoteCache
  /\ IgnoredPreserveReconfiguration
  /\ IgnoredPreserveCommitted
  /\ IgnoredPreserveValidation
  /\ IgnoredPreservePhase
  /\ IgnoredPreservePendingFinality
  /\ IgnoredPreservePendingMap
  /\ AllAlwaysPreservedStatePreserved
  /\ IgnoredStatePreserved
  /\ ValuesStayInDomain

Safety ==
  EnginePayloadStatePreservationExactness

EnginePayloadStatePreservationCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EnginePayloadStatePreservationExactness

SafetyFast == EnginePayloadStatePreservationExactness

=============================================================================
====
