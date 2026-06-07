---- MODULE SumeragiCommitPipelineRecoveryGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for the Sumeragi commit-pipeline recovery gate.

This slice models the adapter-side ordering in
`process_commit_candidates_with_trigger_inner(...)` after the local node has a
validated pending block. Cached commit votes must be aggregated into a local
commit QC before the node asks peers for a missing commit QC. Peer recovery is
armed only for a valid, payload-local, locally voted, stale pending block that
still extends the committed tip and still has no commit QC. Cached vote
rebroadcast for near-quorum commit votes must use the quorum missing-signer
target set, not the proposal collector subset.

The model enumerates the finite input shapes that matter for this gate and
keeps the state to the set of cases already exercised.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipLocalQcFormation,
  \* @type: Bool;
  BugRecoverDespiteLocalQuorum,
  \* @type: Bool;
  BugRequestRecoveryBeforeTimeout,
  \* @type: Bool;
  BugRequestRecoveryWithoutLocalVote,
  \* @type: Bool;
  BugRequestRecoveryWithCommitQc,
  \* @type: Bool;
  BugRequestRecoveryWithMissingData,
  \* @type: Bool;
  BugRequestRecoveryInvalidPending,
  \* @type: Bool;
  BugRequestRecoveryOffTip,
  \* @type: Bool;
  BugSkipMissingQcRequest,
  \* @type: Bool;
  BugDropCommitQcMarker,
  \* @type: Bool;
  BugSkipQuorumRetransmit,
  \* @type: Bool;
  BugUseCollectorTargetsForRetransmit,
  \* @type: Bool;
  BugRebroadcastWithoutVotes,
  \* @type: Bool;
  BugRebroadcastAfterQc

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Candidates == {
  "localQuorumStale",
  "localQuorumFresh",
  "stalledLocalVote",
  "freshLocalVote",
  "commitQcAlreadyObserved",
  "missingLocalData",
  "invalidPending",
  "noLocalVote",
  "offTip",
  "nearQuorumRetransmit",
  "collectorDecoyRetransmit",
  "noVotesRetransmit",
  "hasCommitQcRetransmit"
}

LocalVoteEmitted(candidate) ==
  candidate \notin {"noLocalVote", "noVotesRetransmit"}

CommitQcObservedBefore(candidate) ==
  candidate \in {"commitQcAlreadyObserved", "hasCommitQcRetransmit"}

MissingLocalData(candidate) ==
  candidate = "missingLocalData"

ValidPending(candidate) ==
  candidate # "invalidPending"

Stale(candidate) ==
  candidate \notin {"localQuorumFresh", "freshLocalVote"}

ExtendsTip(candidate) ==
  candidate # "offTip"

CachedVotesAtQuorum(candidate) ==
  candidate \in {"localQuorumStale", "localQuorumFresh"}

CachedVotesNearQuorum(candidate) ==
  candidate \in {"nearQuorumRetransmit", "collectorDecoyRetransmit"}

CachedVotesEmpty(candidate) ==
  candidate = "noVotesRetransmit"

HasCommitQcForRebroadcast(candidate) ==
  candidate = "hasCommitQcRetransmit"

SpecFormsLocalQc(candidate) ==
  /\ LocalVoteEmitted(candidate)
  /\ ~CommitQcObservedBefore(candidate)
  /\ ~MissingLocalData(candidate)
  /\ ValidPending(candidate)
  /\ CachedVotesAtQuorum(candidate)

ImplementationFormsLocalQc(candidate) ==
  IF SpecFormsLocalQc(candidate)
  THEN ~BugSkipLocalQcFormation
  ELSE FALSE

CommitQcObservedAfter(candidate) ==
  IF CommitQcObservedBefore(candidate)
  THEN ~BugDropCommitQcMarker
  ELSE
    /\ ImplementationFormsLocalQc(candidate)
    /\ ~BugDropCommitQcMarker

SpecRequestsMissingQc(candidate) ==
  /\ candidate = "stalledLocalVote"
  /\ LocalVoteEmitted(candidate)
  /\ ~CommitQcObservedAfter(candidate)
  /\ ~MissingLocalData(candidate)
  /\ ValidPending(candidate)
  /\ Stale(candidate)
  /\ ExtendsTip(candidate)

ImplementationRequestsMissingQc(candidate) ==
  \/ /\ SpecRequestsMissingQc(candidate)
     /\ ~BugSkipMissingQcRequest
  \/ /\ CachedVotesAtQuorum(candidate)
     /\ BugRecoverDespiteLocalQuorum
  \/ /\ candidate = "freshLocalVote"
     /\ BugRequestRecoveryBeforeTimeout
  \/ /\ candidate = "noLocalVote"
     /\ BugRequestRecoveryWithoutLocalVote
  \/ /\ candidate = "commitQcAlreadyObserved"
     /\ BugRequestRecoveryWithCommitQc
  \/ /\ candidate = "missingLocalData"
     /\ BugRequestRecoveryWithMissingData
  \/ /\ candidate = "invalidPending"
     /\ BugRequestRecoveryInvalidPending
  \/ /\ candidate = "offTip"
     /\ BugRequestRecoveryOffTip

SpecRebroadcastsMissingVotes(candidate) ==
  CachedVotesNearQuorum(candidate)

ImplementationRebroadcastsMissingVotes(candidate) ==
  \/ /\ SpecRebroadcastsMissingVotes(candidate)
     /\ ~BugSkipQuorumRetransmit
  \/ /\ CachedVotesEmpty(candidate)
     /\ BugRebroadcastWithoutVotes
  \/ /\ HasCommitQcForRebroadcast(candidate)
     /\ BugRebroadcastAfterQc

TargetsQuorumMissingSigners(candidate) ==
  /\ ImplementationRebroadcastsMissingVotes(candidate)
  /\ SpecRebroadcastsMissingVotes(candidate)
  /\ ~BugUseCollectorTargetsForRetransmit

TypeInvariant ==
  /\ BugSkipLocalQcFormation \in BOOLEAN
  /\ BugRecoverDespiteLocalQuorum \in BOOLEAN
  /\ BugRequestRecoveryBeforeTimeout \in BOOLEAN
  /\ BugRequestRecoveryWithoutLocalVote \in BOOLEAN
  /\ BugRequestRecoveryWithCommitQc \in BOOLEAN
  /\ BugRequestRecoveryWithMissingData \in BOOLEAN
  /\ BugRequestRecoveryInvalidPending \in BOOLEAN
  /\ BugRequestRecoveryOffTip \in BOOLEAN
  /\ BugSkipMissingQcRequest \in BOOLEAN
  /\ BugDropCommitQcMarker \in BOOLEAN
  /\ BugSkipQuorumRetransmit \in BOOLEAN
  /\ BugUseCollectorTargetsForRetransmit \in BOOLEAN
  /\ BugRebroadcastWithoutVotes \in BOOLEAN
  /\ BugRebroadcastAfterQc \in BOOLEAN
  /\ tried \subseteq Candidates

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

LocalCommitQcFormationMatchesSpec ==
  \A candidate \in tried:
    ImplementationFormsLocalQc(candidate) <=> SpecFormsLocalQc(candidate)

LocalQuorumFormsBeforePeerRecovery ==
  \A candidate \in tried:
    SpecFormsLocalQc(candidate) =>
      /\ ImplementationFormsLocalQc(candidate)
      /\ CommitQcObservedAfter(candidate)
      /\ ~ImplementationRequestsMissingQc(candidate)

CommitQcObservationIsPreserved ==
  \A candidate \in tried:
    CommitQcObservedBefore(candidate) => CommitQcObservedAfter(candidate)

MissingCommitQcRecoveryMatchesSpec ==
  \A candidate \in tried:
    ImplementationRequestsMissingQc(candidate) <=> SpecRequestsMissingQc(candidate)

FreshLocalVoteDoesNotRecover ==
  "freshLocalVote" \in tried =>
    ~ImplementationRequestsMissingQc("freshLocalVote")

RecoveryRequiresLocalVote ==
  \A candidate \in tried:
    ImplementationRequestsMissingQc(candidate) => LocalVoteEmitted(candidate)

RecoveryRequiresCommitQcAbsent ==
  \A candidate \in tried:
    ImplementationRequestsMissingQc(candidate) => ~CommitQcObservedAfter(candidate)

RecoveryRequiresPayloadLocal ==
  \A candidate \in tried:
    ImplementationRequestsMissingQc(candidate) => ~MissingLocalData(candidate)

RecoveryRequiresValidPending ==
  \A candidate \in tried:
    ImplementationRequestsMissingQc(candidate) => ValidPending(candidate)

RecoveryRequiresTipExtension ==
  \A candidate \in tried:
    ImplementationRequestsMissingQc(candidate) => ExtendsTip(candidate)

QuorumRetransmitMatchesSpec ==
  \A candidate \in tried:
    ImplementationRebroadcastsMissingVotes(candidate) <=>
      SpecRebroadcastsMissingVotes(candidate)

QuorumRetransmitUsesMissingSignerTargets ==
  \A candidate \in tried:
    SpecRebroadcastsMissingVotes(candidate) =>
      /\ ImplementationRebroadcastsMissingVotes(candidate)
      /\ TargetsQuorumMissingSigners(candidate)

CollectorSubsetNeverOverridesQuorumTargets ==
  "collectorDecoyRetransmit" \in tried =>
    TargetsQuorumMissingSigners("collectorDecoyRetransmit")

EmptyVoteSetNeverRebroadcasts ==
  "noVotesRetransmit" \in tried =>
    ~ImplementationRebroadcastsMissingVotes("noVotesRetransmit")

CachedCommitQcSkipsRebroadcast ==
  "hasCommitQcRetransmit" \in tried =>
    ~ImplementationRebroadcastsMissingVotes("hasCommitQcRetransmit")

LocalCommitQcOrderingExact ==
  /\ LocalCommitQcFormationMatchesSpec
  /\ LocalQuorumFormsBeforePeerRecovery
  /\ CommitQcObservationIsPreserved

MissingCommitQcRecoveryGateExact ==
  /\ MissingCommitQcRecoveryMatchesSpec
  /\ FreshLocalVoteDoesNotRecover
  /\ RecoveryRequiresLocalVote
  /\ RecoveryRequiresCommitQcAbsent
  /\ RecoveryRequiresPayloadLocal
  /\ RecoveryRequiresValidPending
  /\ RecoveryRequiresTipExtension

QuorumRetransmitGateExact ==
  /\ QuorumRetransmitMatchesSpec
  /\ QuorumRetransmitUsesMissingSignerTargets
  /\ CollectorSubsetNeverOverridesQuorumTargets
  /\ EmptyVoteSetNeverRebroadcasts
  /\ CachedCommitQcSkipsRebroadcast

CommitPipelineRecoveryExactness ==
  /\ LocalCommitQcOrderingExact
  /\ MissingCommitQcRecoveryGateExact
  /\ QuorumRetransmitGateExact

====
