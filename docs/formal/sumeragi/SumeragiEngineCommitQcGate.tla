---- MODULE SumeragiEngineCommitQcGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for the pure Sumeragi engine commit-QC gate.

This slice models `ConsensusEngine::on_certificate(...)` dispatching a current
view commit certificate into `on_commit_qc(...)`. A commit QC may finalize
immediately only when the certified payload is already locally available. If
the payload is missing, the engine must record pending finality and request
that exact payload instead of committing. Wrong-context, stale, already
committed, replayed, and conflicting pending-finality commit QCs must be
ignored.

Accepted commit QCs, whether they finalize or fetch, must record highest-QC
state. The model enumerates the finite guard cases around this boundary;
`SpecCommits` and `SpecFetches` are the reference contract.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAcceptWrongContext,
  \* @type: Bool;
  BugAcceptWrongQuorumPolicy,
  \* @type: Bool;
  BugAcceptStaleView,
  \* @type: Bool;
  BugAcceptCommittedHeight,
  \* @type: Bool;
  BugAcceptPendingReplay,
  \* @type: Bool;
  BugAcceptPendingConflict,
  \* @type: Bool;
  BugCommitWithoutPayload,
  \* @type: Bool;
  BugFetchDespitePayloadAvailable,
  \* @type: Bool;
  BugRejectAvailableCommitQc,
  \* @type: Bool;
  BugRejectMissingPayloadCommitQc,
  \* @type: Bool;
  BugSkipHighestRecord

VARIABLES
  \* @type: Set(Str);
  tried,
  \* @type: Set(Str);
  committed,
  \* @type: Set(Str);
  fetched,
  \* @type: Set(Str);
  ignored,
  \* @type: Set(Str);
  highest

\* @type: <<Set(Str), Set(Str), Set(Str), Set(Str), Set(Str)>>;
vars == <<tried, committed, fetched, ignored, highest>>

Candidates == {
  "safePayloadAvailable",
  "safePayloadMissing",
  "wrongHeight",
  "wrongEpoch",
  "wrongValidatorSet",
  "wrongQuorumPolicy",
  "staleView",
  "committedHeight",
  "pendingReplaySameCommitQc",
  "pendingConflictingCommitQc"
}

SpecCommits(candidate) ==
  candidate = "safePayloadAvailable"

SpecFetches(candidate) ==
  candidate = "safePayloadMissing"

SpecAccepts(candidate) ==
  SpecCommits(candidate) \/ SpecFetches(candidate)

BugAllowsUnsafe(candidate) ==
  \/ /\ candidate \in {"wrongHeight", "wrongEpoch", "wrongValidatorSet"}
     /\ BugAcceptWrongContext
  \/ /\ candidate = "wrongQuorumPolicy"
     /\ BugAcceptWrongQuorumPolicy
  \/ /\ candidate = "staleView"
     /\ BugAcceptStaleView
  \/ /\ candidate = "committedHeight"
     /\ BugAcceptCommittedHeight
  \/ /\ candidate = "pendingReplaySameCommitQc"
     /\ BugAcceptPendingReplay
  \/ /\ candidate = "pendingConflictingCommitQc"
     /\ BugAcceptPendingConflict

ImplementationCommits(candidate) ==
  IF candidate = "safePayloadAvailable"
  THEN /\ ~BugRejectAvailableCommitQc
       /\ ~BugFetchDespitePayloadAvailable
  ELSE IF candidate = "safePayloadMissing"
       THEN /\ ~BugRejectMissingPayloadCommitQc
            /\ BugCommitWithoutPayload
       ELSE BugAllowsUnsafe(candidate)

ImplementationFetches(candidate) ==
  IF candidate = "safePayloadAvailable"
  THEN /\ ~BugRejectAvailableCommitQc
       /\ BugFetchDespitePayloadAvailable
  ELSE IF candidate = "safePayloadMissing"
       THEN /\ ~BugRejectMissingPayloadCommitQc
            /\ ~BugCommitWithoutPayload
       ELSE FALSE

ImplementationAccepts(candidate) ==
  ImplementationCommits(candidate) \/ ImplementationFetches(candidate)

ImplementationRecordsHighest(candidate) ==
  ImplementationAccepts(candidate) /\ ~BugSkipHighestRecord

TypeInvariant ==
  /\ BugAcceptWrongContext \in BOOLEAN
  /\ BugAcceptWrongQuorumPolicy \in BOOLEAN
  /\ BugAcceptStaleView \in BOOLEAN
  /\ BugAcceptCommittedHeight \in BOOLEAN
  /\ BugAcceptPendingReplay \in BOOLEAN
  /\ BugAcceptPendingConflict \in BOOLEAN
  /\ BugCommitWithoutPayload \in BOOLEAN
  /\ BugFetchDespitePayloadAvailable \in BOOLEAN
  /\ BugRejectAvailableCommitQc \in BOOLEAN
  /\ BugRejectMissingPayloadCommitQc \in BOOLEAN
  /\ BugSkipHighestRecord \in BOOLEAN
  /\ tried \subseteq Candidates
  /\ committed \subseteq Candidates
  /\ fetched \subseteq Candidates
  /\ ignored \subseteq Candidates
  /\ highest \subseteq Candidates
  /\ committed \cap fetched = {}
  /\ committed \cap ignored = {}
  /\ fetched \cap ignored = {}
  /\ committed \cup fetched \cup ignored = tried

Init ==
  /\ tried = {}
  /\ committed = {}
  /\ fetched = {}
  /\ ignored = {}
  /\ highest = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}
  /\ IF ImplementationCommits(candidate)
     THEN
       /\ committed' = committed \cup {candidate}
       /\ fetched' = fetched
       /\ ignored' = ignored
     ELSE IF ImplementationFetches(candidate)
          THEN
            /\ committed' = committed
            /\ fetched' = fetched \cup {candidate}
            /\ ignored' = ignored
          ELSE
            /\ committed' = committed
            /\ fetched' = fetched
            /\ ignored' = ignored \cup {candidate}
  /\ IF ImplementationRecordsHighest(candidate)
     THEN highest' = highest \cup {candidate}
     ELSE highest' = highest

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

CommittedMatchesSpec ==
  committed \subseteq {candidate \in Candidates : SpecCommits(candidate)}

FetchedMatchesSpec ==
  fetched \subseteq {candidate \in Candidates : SpecFetches(candidate)}

IgnoredMatchesSpec ==
  ignored \subseteq {candidate \in Candidates : ~SpecAccepts(candidate)}

SafeAvailableCommitQcsCommit ==
  \A candidate \in tried:
    SpecCommits(candidate) => candidate \in committed

SafeMissingPayloadCommitQcsFetch ==
  \A candidate \in tried:
    SpecFetches(candidate) => candidate \in fetched

UnsafeCommitQcsAreIgnored ==
  \A candidate \in tried:
    ~SpecAccepts(candidate) => candidate \in ignored

WrongContextNeverAccepted ==
  /\ "wrongHeight" \notin committed \cup fetched
  /\ "wrongEpoch" \notin committed \cup fetched
  /\ "wrongValidatorSet" \notin committed \cup fetched

WrongQuorumPolicyNeverAccepted ==
  "wrongQuorumPolicy" \notin committed \cup fetched

StaleViewNeverAccepted ==
  "staleView" \notin committed \cup fetched

CommittedHeightNeverAccepted ==
  "committedHeight" \notin committed \cup fetched

PendingReplayNeverAccepted ==
  "pendingReplaySameCommitQc" \notin committed \cup fetched

PendingConflictNeverAccepted ==
  "pendingConflictingCommitQc" \notin committed \cup fetched

NoCommitWithoutPayload ==
  "safePayloadMissing" \notin committed

NoFetchWhenPayloadAvailable ==
  "safePayloadAvailable" \notin fetched

AcceptedCommitQcsRecordHighest ==
  committed \cup fetched \subseteq highest

IgnoredCommitQcsDoNotRecordHighest ==
  ignored \cap highest = {}

HighestFollowsAcceptedCommitQcs ==
  highest \subseteq committed \cup fetched

EngineCommitQcExactness ==
  /\ CommittedMatchesSpec
  /\ FetchedMatchesSpec
  /\ IgnoredMatchesSpec
  /\ SafeAvailableCommitQcsCommit
  /\ SafeMissingPayloadCommitQcsFetch
  /\ UnsafeCommitQcsAreIgnored
  /\ WrongContextNeverAccepted
  /\ WrongQuorumPolicyNeverAccepted
  /\ StaleViewNeverAccepted
  /\ CommittedHeightNeverAccepted
  /\ PendingReplayNeverAccepted
  /\ PendingConflictNeverAccepted
  /\ NoCommitWithoutPayload
  /\ NoFetchWhenPayloadAvailable
  /\ AcceptedCommitQcsRecordHighest
  /\ IgnoredCommitQcsDoNotRecordHighest
  /\ HighestFollowsAcceptedCommitQcs

EngineCommitQcCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineCommitQcExactness

SafetyFast == EngineCommitQcExactness

Safety ==
  EngineCommitQcCorrectnessEnvelope

====
