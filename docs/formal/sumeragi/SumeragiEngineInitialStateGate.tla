---- MODULE SumeragiEngineInitialStateGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for pure-engine constructor initial state.

This slice models the state that `ConsensusEngine::new(round, quorum_policy)`
must install before any input is handled. The constructor must preserve the
caller-supplied round and quorum policy, start in proposal phase, keep every
optional record absent, keep every collection empty, and emit no output.
Companion models cover the later transitions out of this initialized state.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugWrongRound,
  \* @type: Bool;
  BugWrongQuorum,
  \* @type: Bool;
  BugWrongPhase,
  \* @type: Bool;
  BugSetsLock,
  \* @type: Bool;
  BugSetsHighest,
  \* @type: Bool;
  BugSetsPendingFinality,
  \* @type: Bool;
  BugRecordsAvailablePayload,
  \* @type: Bool;
  BugRecordsPendingMap,
  \* @type: Bool;
  BugRecordsCommitted,
  \* @type: Bool;
  BugStagesReconfiguration,
  \* @type: Bool;
  BugSetsValidationOwner,
  \* @type: Bool;
  BugRecordsPrepareVote,
  \* @type: Bool;
  BugEmitsOutput

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {"constructor_count_quorum", "constructor_stake_quorum"}
Rounds == {"round_input", "round_wrong"}
QuorumPolicies == {"count_policy", "stake_policy", "wrong_policy"}
Phases == {"Proposal", "Prepare", "Commit", "PendingFinality"}
OptionValues == {"none", "some"}
StoreValues == {"empty", "nonempty"}
OutputValues == {"none", "some"}

SpecRound(candidate) == "round_input"

SpecQuorumPolicy(candidate) ==
  CASE candidate = "constructor_count_quorum" -> "count_policy"
    [] candidate = "constructor_stake_quorum" -> "stake_policy"

ImplementationRound(candidate) ==
  IF BugWrongRound THEN "round_wrong" ELSE SpecRound(candidate)

ImplementationQuorumPolicy(candidate) ==
  IF BugWrongQuorum THEN "wrong_policy" ELSE SpecQuorumPolicy(candidate)

ImplementationPhase(candidate) ==
  IF BugWrongPhase THEN "Prepare" ELSE "Proposal"

ImplementationLockedQc(candidate) ==
  IF BugSetsLock THEN "some" ELSE "none"

ImplementationHighestQc(candidate) ==
  IF BugSetsHighest THEN "some" ELSE "none"

ImplementationPendingFinalityMarker(candidate) ==
  IF BugSetsPendingFinality THEN "some" ELSE "none"

ImplementationAvailablePayloads(candidate) ==
  IF BugRecordsAvailablePayload THEN "nonempty" ELSE "empty"

ImplementationPendingFinalityMap(candidate) ==
  IF BugRecordsPendingMap THEN "nonempty" ELSE "empty"

ImplementationCommitted(candidate) ==
  IF BugRecordsCommitted THEN "nonempty" ELSE "empty"

ImplementationReconfiguration(candidate) ==
  IF BugStagesReconfiguration THEN "some" ELSE "none"

ImplementationValidationOwner(candidate) ==
  IF BugSetsValidationOwner THEN "some" ELSE "none"

ImplementationPrepareVoteCache(candidate) ==
  IF BugRecordsPrepareVote THEN "nonempty" ELSE "empty"

ImplementationOutput(candidate) ==
  IF BugEmitsOutput THEN "some" ELSE "none"

TypeInvariant ==
  /\ BugWrongRound \in BOOLEAN
  /\ BugWrongQuorum \in BOOLEAN
  /\ BugWrongPhase \in BOOLEAN
  /\ BugSetsLock \in BOOLEAN
  /\ BugSetsHighest \in BOOLEAN
  /\ BugSetsPendingFinality \in BOOLEAN
  /\ BugRecordsAvailablePayload \in BOOLEAN
  /\ BugRecordsPendingMap \in BOOLEAN
  /\ BugRecordsCommitted \in BOOLEAN
  /\ BugStagesReconfiguration \in BOOLEAN
  /\ BugSetsValidationOwner \in BOOLEAN
  /\ BugRecordsPrepareVote \in BOOLEAN
  /\ BugEmitsOutput \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ SpecRound(candidate) \in Rounds
    /\ ImplementationRound(candidate) \in Rounds
    /\ SpecQuorumPolicy(candidate) \in QuorumPolicies
    /\ ImplementationQuorumPolicy(candidate) \in QuorumPolicies
    /\ ImplementationPhase(candidate) \in Phases
    /\ ImplementationLockedQc(candidate) \in OptionValues
    /\ ImplementationHighestQc(candidate) \in OptionValues
    /\ ImplementationPendingFinalityMarker(candidate) \in OptionValues
    /\ ImplementationAvailablePayloads(candidate) \in StoreValues
    /\ ImplementationPendingFinalityMap(candidate) \in StoreValues
    /\ ImplementationCommitted(candidate) \in StoreValues
    /\ ImplementationReconfiguration(candidate) \in OptionValues
    /\ ImplementationValidationOwner(candidate) \in OptionValues
    /\ ImplementationPrepareVoteCache(candidate) \in StoreValues
    /\ ImplementationOutput(candidate) \in OutputValues

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

ConstructorPreservesRound ==
  \A candidate \in tried:
    ImplementationRound(candidate) = SpecRound(candidate)

ConstructorPreservesQuorumPolicy ==
  \A candidate \in tried:
    ImplementationQuorumPolicy(candidate) = SpecQuorumPolicy(candidate)

ConstructorStartsInProposalPhase ==
  \A candidate \in tried:
    ImplementationPhase(candidate) = "Proposal"

ConstructorStartsUnlocked ==
  \A candidate \in tried:
    ImplementationLockedQc(candidate) = "none"

ConstructorStartsWithoutHighestQc ==
  \A candidate \in tried:
    ImplementationHighestQc(candidate) = "none"

ConstructorStartsWithoutPendingFinality ==
  \A candidate \in tried:
    ImplementationPendingFinalityMarker(candidate) = "none"

ConstructorStartsWithEmptyAvailablePayloads ==
  \A candidate \in tried:
    ImplementationAvailablePayloads(candidate) = "empty"

ConstructorStartsWithEmptyPendingFinalityMap ==
  \A candidate \in tried:
    ImplementationPendingFinalityMap(candidate) = "empty"

ConstructorStartsWithEmptyCommittedRecords ==
  \A candidate \in tried:
    ImplementationCommitted(candidate) = "empty"

ConstructorStartsWithNoReconfiguration ==
  \A candidate \in tried:
    ImplementationReconfiguration(candidate) = "none"

ConstructorStartsWithNoValidationOwner ==
  \A candidate \in tried:
    ImplementationValidationOwner(candidate) = "none"

ConstructorStartsWithEmptyPrepareVoteCache ==
  \A candidate \in tried:
    ImplementationPrepareVoteCache(candidate) = "empty"

ConstructorEmitsNoOutput ==
  \A candidate \in tried:
    ImplementationOutput(candidate) = "none"

AllInitialStateMatchesSpec ==
  \A candidate \in tried:
    /\ ImplementationRound(candidate) = SpecRound(candidate)
    /\ ImplementationQuorumPolicy(candidate) = SpecQuorumPolicy(candidate)
    /\ ImplementationPhase(candidate) = "Proposal"
    /\ ImplementationLockedQc(candidate) = "none"
    /\ ImplementationHighestQc(candidate) = "none"
    /\ ImplementationPendingFinalityMarker(candidate) = "none"
    /\ ImplementationAvailablePayloads(candidate) = "empty"
    /\ ImplementationPendingFinalityMap(candidate) = "empty"
    /\ ImplementationCommitted(candidate) = "empty"
    /\ ImplementationReconfiguration(candidate) = "none"
    /\ ImplementationValidationOwner(candidate) = "none"
    /\ ImplementationPrepareVoteCache(candidate) = "empty"
    /\ ImplementationOutput(candidate) = "none"

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ SpecRound(candidate) \in Rounds
    /\ ImplementationRound(candidate) \in Rounds
    /\ SpecQuorumPolicy(candidate) \in QuorumPolicies
    /\ ImplementationQuorumPolicy(candidate) \in QuorumPolicies
    /\ ImplementationPhase(candidate) \in Phases
    /\ ImplementationLockedQc(candidate) \in OptionValues
    /\ ImplementationHighestQc(candidate) \in OptionValues
    /\ ImplementationPendingFinalityMarker(candidate) \in OptionValues
    /\ ImplementationAvailablePayloads(candidate) \in StoreValues
    /\ ImplementationPendingFinalityMap(candidate) \in StoreValues
    /\ ImplementationCommitted(candidate) \in StoreValues
    /\ ImplementationReconfiguration(candidate) \in OptionValues
    /\ ImplementationValidationOwner(candidate) \in OptionValues
    /\ ImplementationPrepareVoteCache(candidate) \in StoreValues
    /\ ImplementationOutput(candidate) \in OutputValues

Safety ==
  /\ ConstructorPreservesRound
  /\ ConstructorPreservesQuorumPolicy
  /\ ConstructorStartsInProposalPhase
  /\ ConstructorStartsUnlocked
  /\ ConstructorStartsWithoutHighestQc
  /\ ConstructorStartsWithoutPendingFinality
  /\ ConstructorStartsWithEmptyAvailablePayloads
  /\ ConstructorStartsWithEmptyPendingFinalityMap
  /\ ConstructorStartsWithEmptyCommittedRecords
  /\ ConstructorStartsWithNoReconfiguration
  /\ ConstructorStartsWithNoValidationOwner
  /\ ConstructorStartsWithEmptyPrepareVoteCache
  /\ ConstructorEmitsNoOutput
  /\ AllInitialStateMatchesSpec
  /\ ValuesStayInDomain

=============================================================================
====
