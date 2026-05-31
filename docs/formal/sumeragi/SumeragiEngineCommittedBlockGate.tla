---- MODULE SumeragiEngineCommittedBlockGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for the pure Sumeragi engine committed-block gate.

This slice models `ConsensusEngine::on_committed_block(...)`, the boundary
where storage/application finality notifications are reflected back into the
pure consensus engine. A fresh committed-block notification records the height.
A fresh notification carrying a validator-set change may emit activation only
when the change activates at the next height. This slice treats duplicate
reconfiguration notifications as already-scheduled duplicates and keeps them
idempotent; `SumeragiEngineReconfigurationDedupGate.tla` covers the distinct
plain-commit-then-reconfiguration replay case where the same committed hash may
activate later metadata if no change for that activation height is pending.
Conflicting same-height notifications cannot overwrite the recorded commit or
activate a validator set.

The model enumerates the finite input shapes that matter for this boundary.
The implementation transition records whether each notification inserted a
fresh commit, emitted validator-set activation, overwrote a committed height,
or was ignored.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipFreshRecord,
  \* @type: Bool;
  BugRejectBoundaryActivation,
  \* @type: Bool;
  BugActivateWithoutBoundary,
  \* @type: Bool;
  BugActivateNonBoundary,
  \* @type: Bool;
  BugRecordDuplicate,
  \* @type: Bool;
  BugActivateDuplicate,
  \* @type: Bool;
  BugRecordConflict,
  \* @type: Bool;
  BugActivateConflict,
  \* @type: Bool;
  BugOverwriteConflict

VARIABLES
  \* @type: Set(Str);
  tried,
  \* @type: Set(Str);
  recorded,
  \* @type: Set(Str);
  activated,
  \* @type: Set(Str);
  ignored,
  \* @type: Set(Str);
  overwritten

\* @type: <<Set(Str), Set(Str), Set(Str), Set(Str), Set(Str)>>;
vars == <<tried, recorded, activated, ignored, overwritten>>

Candidates == {
  "freshPlain",
  "freshBoundaryReconfiguration",
  "freshNonBoundaryReconfiguration",
  "duplicatePlain",
  "duplicateBoundaryReconfiguration",
  "conflictingPlain",
  "conflictingBoundaryReconfiguration",
  "conflictingNonBoundaryReconfiguration"
}

Fresh(candidate) ==
  candidate \in {
    "freshPlain",
    "freshBoundaryReconfiguration",
    "freshNonBoundaryReconfiguration"
  }

Duplicate(candidate) ==
  candidate \in {
    "duplicatePlain",
    "duplicateBoundaryReconfiguration"
  }

Conflict(candidate) ==
  candidate \in {
    "conflictingPlain",
    "conflictingBoundaryReconfiguration",
    "conflictingNonBoundaryReconfiguration"
  }

BoundaryReconfiguration(candidate) ==
  candidate \in {
    "freshBoundaryReconfiguration",
    "duplicateBoundaryReconfiguration",
    "conflictingBoundaryReconfiguration"
  }

NonBoundaryReconfiguration(candidate) ==
  candidate \in {
    "freshNonBoundaryReconfiguration",
    "conflictingNonBoundaryReconfiguration"
  }

Plain(candidate) ==
  candidate \in {"freshPlain", "duplicatePlain", "conflictingPlain"}

SpecRecords(candidate) ==
  Fresh(candidate)

SpecActivates(candidate) ==
  candidate = "freshBoundaryReconfiguration"

ImplementationRecords(candidate) ==
  IF Fresh(candidate)
  THEN ~BugSkipFreshRecord
  ELSE IF Duplicate(candidate)
       THEN BugRecordDuplicate
       ELSE IF Conflict(candidate)
            THEN BugRecordConflict
            ELSE FALSE

ImplementationActivates(candidate) ==
  IF Fresh(candidate)
  THEN
    \/ /\ candidate = "freshBoundaryReconfiguration"
       /\ ~BugRejectBoundaryActivation
    \/ /\ candidate = "freshNonBoundaryReconfiguration"
       /\ BugActivateNonBoundary
    \/ /\ candidate = "freshPlain"
       /\ BugActivateWithoutBoundary
  ELSE IF Duplicate(candidate)
       THEN /\ BoundaryReconfiguration(candidate)
            /\ BugActivateDuplicate
       ELSE IF Conflict(candidate)
            THEN /\ BoundaryReconfiguration(candidate)
                 /\ BugActivateConflict
            ELSE FALSE

ImplementationOverwrites(candidate) ==
  /\ Conflict(candidate)
  /\ BugOverwriteConflict

TypeInvariant ==
  /\ BugSkipFreshRecord \in BOOLEAN
  /\ BugRejectBoundaryActivation \in BOOLEAN
  /\ BugActivateWithoutBoundary \in BOOLEAN
  /\ BugActivateNonBoundary \in BOOLEAN
  /\ BugRecordDuplicate \in BOOLEAN
  /\ BugActivateDuplicate \in BOOLEAN
  /\ BugRecordConflict \in BOOLEAN
  /\ BugActivateConflict \in BOOLEAN
  /\ BugOverwriteConflict \in BOOLEAN
  /\ tried \subseteq Candidates
  /\ recorded \subseteq Candidates
  /\ activated \subseteq Candidates
  /\ ignored \subseteq Candidates
  /\ overwritten \subseteq Candidates
  /\ ignored = tried \ (recorded \cup activated \cup overwritten)

Init ==
  /\ tried = {}
  /\ recorded = {}
  /\ activated = {}
  /\ ignored = {}
  /\ overwritten = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}
  /\ IF ImplementationRecords(candidate)
     THEN recorded' = recorded \cup {candidate}
     ELSE recorded' = recorded
  /\ IF ImplementationActivates(candidate)
     THEN activated' = activated \cup {candidate}
     ELSE activated' = activated
  /\ IF ImplementationOverwrites(candidate)
     THEN overwritten' = overwritten \cup {candidate}
     ELSE overwritten' = overwritten
  /\ ignored' =
       tried' \ (recorded' \cup activated' \cup overwritten')

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

RecordedMatchesSpec ==
  recorded \subseteq {candidate \in Candidates : SpecRecords(candidate)}

ActivatedMatchesSpec ==
  activated \subseteq {candidate \in Candidates : SpecActivates(candidate)}

IgnoredMatchesSpec ==
  ignored \subseteq {
    candidate \in Candidates :
      ~SpecRecords(candidate) /\ ~SpecActivates(candidate)
  }

FreshCommitNotificationsRecord ==
  \A candidate \in tried:
    Fresh(candidate) => candidate \in recorded

FreshBoundaryReconfigurationActivates ==
  "freshBoundaryReconfiguration" \in tried =>
    "freshBoundaryReconfiguration" \in activated

PlainCommitNotificationsNeverActivate ==
  \A candidate \in Candidates:
    Plain(candidate) => candidate \notin activated

NonBoundaryReconfigurationNeverActivates ==
  \A candidate \in Candidates:
    NonBoundaryReconfiguration(candidate) => candidate \notin activated

DuplicateNotificationsAreIdempotent ==
  \A candidate \in tried:
    Duplicate(candidate) => candidate \in ignored

ConflictingNotificationsAreIgnored ==
  \A candidate \in tried:
    Conflict(candidate) => candidate \in ignored

ConflictsNeverOverwrite ==
  overwritten = {}

ActivationRequiresFreshBoundaryRecord ==
  /\ activated \subseteq {"freshBoundaryReconfiguration"}
  /\ activated \subseteq recorded

NoDuplicateOrConflictRecord ==
  recorded \subseteq {candidate \in Candidates : Fresh(candidate)}

====
