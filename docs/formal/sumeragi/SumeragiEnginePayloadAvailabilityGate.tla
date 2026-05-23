---- MODULE SumeragiEnginePayloadAvailabilityGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for the pure Sumeragi engine payload-availability
gate.

This slice models `ConsensusEngine::on_payload_available(...)`, the adapter
boundary where local RBC/payload recovery tells the pure engine that payload
bytes are available. Payload availability alone must never finalize a block.
When a commit QC is pending, only the exact certified subject may commit; a
payload hash mismatch, parent mismatch, or unrelated block hash is ignored
without dropping the pending QC. The exact matching payload commits, clears
pending finality, and returns the engine to proposal phase.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipAvailableRecord,
  \* @type: Bool;
  BugCommitWithoutPendingQc,
  \* @type: Bool;
  BugCommitMismatchedPayload,
  \* @type: Bool;
  BugDropPendingOnMismatch,
  \* @type: Bool;
  BugRejectMatchingPayload,
  \* @type: Bool;
  BugKeepPendingAfterCommit,
  \* @type: Bool;
  BugWrongPhaseAfterCommit

VARIABLES
  \* @type: Set(Str);
  tried,
  \* @type: Set(Str);
  available,
  \* @type: Set(Str);
  committed,
  \* @type: Set(Str);
  ignored,
  \* @type: Set(Str);
  pendingPreserved,
  \* @type: Set(Str);
  pendingCleared,
  \* @type: Set(Str);
  proposalPhase

\* @type: <<Set(Str), Set(Str), Set(Str), Set(Str), Set(Str), Set(Str), Set(Str)>>;
vars == <<
  tried,
  available,
  committed,
  ignored,
  pendingPreserved,
  pendingCleared,
  proposalPhase
>>

Candidates == {
  "noPendingPayload",
  "matchingPendingPayload",
  "payloadHashMismatch",
  "parentMismatch",
  "unknownBlockHash"
}

HasPending(candidate) ==
  candidate # "noPendingPayload"

MatchesPending(candidate) ==
  candidate = "matchingPendingPayload"

MismatchesPending(candidate) ==
  candidate \in {"payloadHashMismatch", "parentMismatch", "unknownBlockHash"}

SpecCommits(candidate) ==
  MatchesPending(candidate)

SpecIgnores(candidate) ==
  ~SpecCommits(candidate)

ImplementationRecordsAvailable ==
  ~BugSkipAvailableRecord

ImplementationCommits(candidate) ==
  IF MatchesPending(candidate)
  THEN ~BugRejectMatchingPayload
  ELSE IF candidate = "noPendingPayload"
       THEN BugCommitWithoutPendingQc
       ELSE BugCommitMismatchedPayload

ImplementationPreservesPending(candidate) ==
  /\ MismatchesPending(candidate)
  /\ ~ImplementationCommits(candidate)
  /\ ~BugDropPendingOnMismatch

ImplementationClearsPending(candidate) ==
  /\ ImplementationCommits(candidate)
  /\ ~BugKeepPendingAfterCommit

ImplementationProposalPhase(candidate) ==
  /\ ImplementationCommits(candidate)
  /\ ~BugWrongPhaseAfterCommit

TypeInvariant ==
  /\ BugSkipAvailableRecord \in BOOLEAN
  /\ BugCommitWithoutPendingQc \in BOOLEAN
  /\ BugCommitMismatchedPayload \in BOOLEAN
  /\ BugDropPendingOnMismatch \in BOOLEAN
  /\ BugRejectMatchingPayload \in BOOLEAN
  /\ BugKeepPendingAfterCommit \in BOOLEAN
  /\ BugWrongPhaseAfterCommit \in BOOLEAN
  /\ tried \subseteq Candidates
  /\ available \subseteq Candidates
  /\ committed \subseteq Candidates
  /\ ignored \subseteq Candidates
  /\ pendingPreserved \subseteq Candidates
  /\ pendingCleared \subseteq Candidates
  /\ proposalPhase \subseteq Candidates
  /\ committed \cap ignored = {}
  /\ committed \cup ignored = tried
  /\ pendingCleared \cap pendingPreserved = {}

Init ==
  /\ tried = {}
  /\ available = {}
  /\ committed = {}
  /\ ignored = {}
  /\ pendingPreserved = {}
  /\ pendingCleared = {}
  /\ proposalPhase = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}
  /\ IF ImplementationRecordsAvailable
     THEN available' = available \cup {candidate}
     ELSE available' = available
  /\ IF ImplementationCommits(candidate)
     THEN
       /\ committed' = committed \cup {candidate}
       /\ ignored' = ignored
     ELSE
       /\ committed' = committed
       /\ ignored' = ignored \cup {candidate}
  /\ IF ImplementationPreservesPending(candidate)
     THEN pendingPreserved' = pendingPreserved \cup {candidate}
     ELSE pendingPreserved' = pendingPreserved
  /\ IF ImplementationClearsPending(candidate)
     THEN pendingCleared' = pendingCleared \cup {candidate}
     ELSE pendingCleared' = pendingCleared
  /\ IF ImplementationProposalPhase(candidate)
     THEN proposalPhase' = proposalPhase \cup {candidate}
     ELSE proposalPhase' = proposalPhase

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

EveryPayloadIsRecordedAvailable ==
  tried \subseteq available

CommittedMatchesSpec ==
  committed \subseteq {candidate \in Candidates : SpecCommits(candidate)}

IgnoredMatchesSpec ==
  ignored \subseteq {candidate \in Candidates : SpecIgnores(candidate)}

PayloadOnlyNeverCommits ==
  "noPendingPayload" \notin committed

MismatchedPayloadsNeverCommit ==
  \A candidate \in Candidates:
    MismatchesPending(candidate) => candidate \notin committed

MatchingPayloadCommits ==
  "matchingPendingPayload" \in tried =>
    "matchingPendingPayload" \in committed

MismatchedPayloadsPreservePending ==
  \A candidate \in tried:
    MismatchesPending(candidate) => candidate \in pendingPreserved

MatchingPayloadClearsPending ==
  "matchingPendingPayload" \in tried =>
    "matchingPendingPayload" \in pendingCleared

CommitClearsPending ==
  committed \subseteq pendingCleared

CommitEntersProposalPhase ==
  committed \subseteq proposalPhase

IgnoredPayloadsDoNotClearPending ==
  ignored \cap pendingCleared = {}

====
