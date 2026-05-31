---- MODULE SumeragiPrecommitVoteGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for Sumeragi local precommit vote emission.

This slice models the safety gate around the live `emit_precommit_vote(...)`
path. A local precommit vote may be emitted only when the candidate pending
block is validated, the local peer belongs to the view-aligned voting topology,
the local validator has not already voted for the same slot, same-height
conflicts are superseded by accepted new-view evidence or complete a newer-view
quorum, and locked-QC checks are satisfied.

The model enumerates the finite guard cases that matter for local signing. The
reference predicate `SpecMayEmit` is the intended implementation contract; each
transition records whether the implementation emitted or rejected that guard
case. The invariants require every emitted vote to match the reference guard
and every guard-permitted candidate to be accepted.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugEmitInvalidValidation,
  \* @type: Bool;
  BugEmitObserver,
  \* @type: Bool;
  BugEmitDuplicate,
  \* @type: Bool;
  BugEmitUnsupersededConflict,
  \* @type: Bool;
  BugEmitOlderQuorumCompletion,
  \* @type: Bool;
  BugEmitLockedConflict,
  \* @type: Bool;
  BugEmitMissingLockedPayload,
  \* @type: Bool;
  BugEmitNonExtendingLock,
  \* @type: Bool;
  BugRejectSafeCandidate

VARIABLES
  \* @type: Set(Str);
  tried,
  \* @type: Set(Str);
  emitted,
  \* @type: Set(Str);
  rejected

\* @type: <<Set(Str), Set(Str), Set(Str)>>;
vars == <<tried, emitted, rejected>>

Candidates == {
  "safe",
  "invalidValidation",
  "observer",
  "notInTopology",
  "duplicateSameSlot",
  "unsupersededConflict",
  "supersededConflict",
  "candidateCompletesNewerQuorum",
  "olderConflictCompletesQuorum",
  "lockedSameHeightConflict",
  "missingLockedPayloadOldView",
  "missingLockedPayloadNewerView",
  "nonExtendingLockedChain",
  "extendsLockedChain"
}

SpecMayEmit(candidate) ==
  candidate \in {
    "safe",
    "supersededConflict",
    "candidateCompletesNewerQuorum",
    "missingLockedPayloadNewerView",
    "extendsLockedChain"
  }

BugAllowsUnsafe(candidate) ==
  \/ /\ candidate = "invalidValidation"
     /\ BugEmitInvalidValidation
  \/ /\ candidate \in {"observer", "notInTopology"}
     /\ BugEmitObserver
  \/ /\ candidate = "duplicateSameSlot"
     /\ BugEmitDuplicate
  \/ /\ candidate = "unsupersededConflict"
     /\ BugEmitUnsupersededConflict
  \/ /\ candidate = "olderConflictCompletesQuorum"
     /\ BugEmitOlderQuorumCompletion
  \/ /\ candidate = "lockedSameHeightConflict"
     /\ BugEmitLockedConflict
  \/ /\ candidate = "missingLockedPayloadOldView"
     /\ BugEmitMissingLockedPayload
  \/ /\ candidate = "nonExtendingLockedChain"
     /\ BugEmitNonExtendingLock

ImplementationEmits(candidate) ==
  IF SpecMayEmit(candidate)
  THEN ~BugRejectSafeCandidate
  ELSE BugAllowsUnsafe(candidate)

TypeInvariant ==
  /\ BugEmitInvalidValidation \in BOOLEAN
  /\ BugEmitObserver \in BOOLEAN
  /\ BugEmitDuplicate \in BOOLEAN
  /\ BugEmitUnsupersededConflict \in BOOLEAN
  /\ BugEmitOlderQuorumCompletion \in BOOLEAN
  /\ BugEmitLockedConflict \in BOOLEAN
  /\ BugEmitMissingLockedPayload \in BOOLEAN
  /\ BugEmitNonExtendingLock \in BOOLEAN
  /\ BugRejectSafeCandidate \in BOOLEAN
  /\ tried \subseteq Candidates
  /\ emitted \subseteq Candidates
  /\ rejected \subseteq Candidates
  /\ emitted \cap rejected = {}
  /\ emitted \cup rejected = tried

Init ==
  /\ tried = {}
  /\ emitted = {}
  /\ rejected = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}
  /\ IF ImplementationEmits(candidate)
     THEN
       /\ emitted' = emitted \cup {candidate}
       /\ rejected' = rejected
     ELSE
       /\ emitted' = emitted
       /\ rejected' = rejected \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

EmittedMatchesSpec ==
  emitted \subseteq {candidate \in Candidates : SpecMayEmit(candidate)}

RejectedMatchesSpec ==
  rejected \subseteq {candidate \in Candidates : ~SpecMayEmit(candidate)}

SafeCandidatesAreAccepted ==
  \A candidate \in tried:
    SpecMayEmit(candidate) => candidate \in emitted

UnsafeCandidatesAreRejected ==
  \A candidate \in tried:
    ~SpecMayEmit(candidate) => candidate \in rejected

InvalidValidationNeverEmits ==
  "invalidValidation" \notin emitted

ObserversNeverEmit ==
  /\ "observer" \notin emitted
  /\ "notInTopology" \notin emitted

DuplicateSameSlotNeverEmits ==
  "duplicateSameSlot" \notin emitted

UnsupersededConflictNeverEmits ==
  "unsupersededConflict" \notin emitted

OlderConflictCannotUseQuorumCompletion ==
  "olderConflictCompletesQuorum" \notin emitted

LockedConflictsNeverEmit ==
  /\ "lockedSameHeightConflict" \notin emitted
  /\ "missingLockedPayloadOldView" \notin emitted
  /\ "nonExtendingLockedChain" \notin emitted

PermittedConflictCasesCanEmit ==
  /\ "supersededConflict" \in tried => "supersededConflict" \in emitted
  /\ "candidateCompletesNewerQuorum" \in tried =>
       "candidateCompletesNewerQuorum" \in emitted

PermittedLockCasesCanEmit ==
  /\ "missingLockedPayloadNewerView" \in tried =>
       "missingLockedPayloadNewerView" \in emitted
  /\ "extendsLockedChain" \in tried => "extendsLockedChain" \in emitted

====
