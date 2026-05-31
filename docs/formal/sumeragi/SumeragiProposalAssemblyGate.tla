---- MODULE SumeragiProposalAssemblyGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for Sumeragi local proposal assembly gating.

This slice models the safety gate before a local leader assembles and
broadcasts a fresh proposal. A proposal may be assembled only when the local
node is an eligible proposer, same-height local vote history does not anchor a
different live branch, pending vote verification cannot still reveal a
same-height conflict, the selected highest QC is available, and the chosen
proposal parent is compatible with the locked chain.

The model enumerates the finite guard cases that matter before proposal cache
or slot-observed state may be mutated. `SpecMayAssemble` is the reference
contract; the implementation transition records whether each candidate guard
case assembled or deferred. The invariants require all assembled proposals to
match the reference guard and all guard-permitted candidates to assemble.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAssembleObserver,
  \* @type: Bool;
  BugAssembleActiveVoteConflict,
  \* @type: Bool;
  BugAssemblePendingVoteVerification,
  \* @type: Bool;
  BugAssembleMissingHighestQc,
  \* @type: Bool;
  BugAssembleNonExtendingHighestQc,
  \* @type: Bool;
  BugAssembleSplitVoteLock,
  \* @type: Bool;
  BugAssembleCommittedEdgeConflict,
  \* @type: Bool;
  BugRejectSafeCandidate,
  \* @type: Bool;
  BugRejectStaleRetiredVote,
  \* @type: Bool;
  BugRejectLockedFallback

VARIABLES
  \* @type: Set(Str);
  tried,
  \* @type: Set(Str);
  assembled,
  \* @type: Set(Str);
  deferred

\* @type: <<Set(Str), Set(Str), Set(Str)>>;
vars == <<tried, assembled, deferred>>

Candidates == {
  "safe",
  "observer",
  "notLeader",
  "activeLocalVoteConflict",
  "staleRetiredPriorVote",
  "newViewSupersedesLocalVote",
  "pendingVoteVerification",
  "missingHighestQc",
  "regressedHighestReplacedByLock",
  "nonExtendingHighestQc",
  "splitSameHeightVotesNonViable",
  "committedEdgeHighestConflict",
  "lockedChainExtends"
}

SpecMayAssemble(candidate) ==
  candidate \in {
    "safe",
    "staleRetiredPriorVote",
    "newViewSupersedesLocalVote",
    "regressedHighestReplacedByLock",
    "lockedChainExtends"
  }

BugAllowsUnsafe(candidate) ==
  \/ /\ candidate \in {"observer", "notLeader"}
     /\ BugAssembleObserver
  \/ /\ candidate = "activeLocalVoteConflict"
     /\ BugAssembleActiveVoteConflict
  \/ /\ candidate = "pendingVoteVerification"
     /\ BugAssemblePendingVoteVerification
  \/ /\ candidate = "missingHighestQc"
     /\ BugAssembleMissingHighestQc
  \/ /\ candidate = "nonExtendingHighestQc"
     /\ BugAssembleNonExtendingHighestQc
  \/ /\ candidate = "splitSameHeightVotesNonViable"
     /\ BugAssembleSplitVoteLock
  \/ /\ candidate = "committedEdgeHighestConflict"
     /\ BugAssembleCommittedEdgeConflict

BugRejectsSafe(candidate) ==
  \/ /\ candidate = "safe"
     /\ BugRejectSafeCandidate
  \/ /\ candidate = "staleRetiredPriorVote"
     /\ BugRejectStaleRetiredVote
  \/ /\ candidate = "newViewSupersedesLocalVote"
     /\ BugRejectStaleRetiredVote
  \/ /\ candidate = "regressedHighestReplacedByLock"
     /\ BugRejectLockedFallback
  \/ /\ candidate = "lockedChainExtends"
     /\ BugRejectLockedFallback

ImplementationAssembles(candidate) ==
  IF SpecMayAssemble(candidate)
  THEN ~BugRejectsSafe(candidate)
  ELSE BugAllowsUnsafe(candidate)

TypeInvariant ==
  /\ BugAssembleObserver \in BOOLEAN
  /\ BugAssembleActiveVoteConflict \in BOOLEAN
  /\ BugAssemblePendingVoteVerification \in BOOLEAN
  /\ BugAssembleMissingHighestQc \in BOOLEAN
  /\ BugAssembleNonExtendingHighestQc \in BOOLEAN
  /\ BugAssembleSplitVoteLock \in BOOLEAN
  /\ BugAssembleCommittedEdgeConflict \in BOOLEAN
  /\ BugRejectSafeCandidate \in BOOLEAN
  /\ BugRejectStaleRetiredVote \in BOOLEAN
  /\ BugRejectLockedFallback \in BOOLEAN
  /\ tried \subseteq Candidates
  /\ assembled \subseteq Candidates
  /\ deferred \subseteq Candidates
  /\ assembled \cap deferred = {}
  /\ assembled \cup deferred = tried

Init ==
  /\ tried = {}
  /\ assembled = {}
  /\ deferred = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}
  /\ IF ImplementationAssembles(candidate)
     THEN
       /\ assembled' = assembled \cup {candidate}
       /\ deferred' = deferred
     ELSE
       /\ assembled' = assembled
       /\ deferred' = deferred \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

AssembledMatchesSpec ==
  assembled \subseteq {candidate \in Candidates : SpecMayAssemble(candidate)}

DeferredMatchesSpec ==
  deferred \subseteq {candidate \in Candidates : ~SpecMayAssemble(candidate)}

SafeCandidatesAreAssembled ==
  \A candidate \in tried:
    SpecMayAssemble(candidate) => candidate \in assembled

UnsafeCandidatesAreDeferred ==
  \A candidate \in tried:
    ~SpecMayAssemble(candidate) => candidate \in deferred

ObserversNeverAssemble ==
  /\ "observer" \notin assembled
  /\ "notLeader" \notin assembled

ActiveLocalVoteConflictNeverAssembles ==
  "activeLocalVoteConflict" \notin assembled

PendingVoteVerificationNeverAssembles ==
  "pendingVoteVerification" \notin assembled

MissingHighestQcNeverAssembles ==
  "missingHighestQc" \notin assembled

NonExtendingHighestQcNeverAssembles ==
  "nonExtendingHighestQc" \notin assembled

SplitVoteLockNeverAssembles ==
  "splitSameHeightVotesNonViable" \notin assembled

CommittedEdgeConflictNeverAssembles ==
  "committedEdgeHighestConflict" \notin assembled

PermittedVoteHistoryCasesAssemble ==
  /\ "staleRetiredPriorVote" \in tried => "staleRetiredPriorVote" \in assembled
  /\ "newViewSupersedesLocalVote" \in tried =>
       "newViewSupersedesLocalVote" \in assembled

PermittedLockedParentCasesAssemble ==
  /\ "regressedHighestReplacedByLock" \in tried =>
       "regressedHighestReplacedByLock" \in assembled
  /\ "lockedChainExtends" \in tried => "lockedChainExtends" \in assembled

====
