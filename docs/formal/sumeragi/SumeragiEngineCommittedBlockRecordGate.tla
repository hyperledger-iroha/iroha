---- MODULE SumeragiEngineCommittedBlockRecordGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for exact committed-block map writes.

This slice models the `self.committed` mutation in
`ConsensusEngine::on_committed_block(...)`. Fresh committed-block
notifications must insert exactly `committed[round.height] = block_hash` while
preserving unrelated committed heights. Duplicate notifications and
conflicting same-height notifications must preserve the existing committed map;
conflicts return before overwriting or adding any other committed height.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipFreshRecord,
  \* @type: Bool;
  BugRecordWrongHeight,
  \* @type: Bool;
  BugRecordWrongBlock,
  \* @type: Bool;
  BugClearUnrelatedEntry,
  \* @type: Bool;
  BugOverwriteUnrelatedEntry,
  \* @type: Bool;
  BugOverwriteDuplicate,
  \* @type: Bool;
  BugClearDuplicate,
  \* @type: Bool;
  BugDuplicateRecordsWrongHeight,
  \* @type: Bool;
  BugOverwriteConflict,
  \* @type: Bool;
  BugClearExistingOnConflict,
  \* @type: Bool;
  BugConflictRecordsWrongHeight

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "fresh_current_a",
  "fresh_current_b_with_other_a",
  "fresh_other_a",
  "fresh_other_b_with_current_a",
  "duplicate_current_a",
  "duplicate_other_a",
  "conflict_current_b",
  "conflict_other_b"
}

Heights == {"height_current", "height_other", "height_wrong"}

Blocks == {"none", "block_a", "block_b"}

Fresh(candidate) ==
  candidate \in {
    "fresh_current_a",
    "fresh_current_b_with_other_a",
    "fresh_other_a",
    "fresh_other_b_with_current_a"
  }

Duplicate(candidate) ==
  candidate \in {"duplicate_current_a", "duplicate_other_a"}

Conflict(candidate) ==
  candidate \in {"conflict_current_b", "conflict_other_b"}

InputHeight(candidate) ==
  IF candidate \in {
    "fresh_current_a",
    "fresh_current_b_with_other_a",
    "duplicate_current_a",
    "conflict_current_b"
  }
  THEN "height_current"
  ELSE "height_other"

InputBlock(candidate) ==
  IF candidate \in {
    "fresh_current_b_with_other_a",
    "fresh_other_b_with_current_a",
    "conflict_current_b",
    "conflict_other_b"
  }
  THEN "block_b"
  ELSE "block_a"

WrongBlock(block) ==
  IF block = "block_a" THEN "block_b" ELSE "block_a"

WrongHeight(height) ==
  "height_wrong"

InitialCurrentCases == {
  "fresh_other_b_with_current_a",
  "duplicate_current_a",
  "conflict_current_b"
}

InitialOtherCases == {
  "fresh_current_b_with_other_a",
  "duplicate_other_a",
  "conflict_other_b"
}

InitialBlockAt(candidate, height) ==
  CASE
    height = "height_current" /\ candidate \in InitialCurrentCases -> "block_a"
  [] height = "height_other" /\ candidate \in InitialOtherCases -> "block_a"
  [] OTHER -> "none"

SpecBlockAt(candidate, height) ==
  IF Fresh(candidate) /\ height = InputHeight(candidate)
  THEN InputBlock(candidate)
  ELSE InitialBlockAt(candidate, height)

ImplementationFreshBlockAt(candidate, height) ==
  IF BugSkipFreshRecord
  THEN InitialBlockAt(candidate, height)
  ELSE IF BugRecordWrongHeight
       THEN
         IF height = WrongHeight(InputHeight(candidate))
         THEN InputBlock(candidate)
         ELSE InitialBlockAt(candidate, height)
       ELSE IF BugRecordWrongBlock
            THEN
              IF height = InputHeight(candidate)
              THEN WrongBlock(InputBlock(candidate))
              ELSE InitialBlockAt(candidate, height)
            ELSE IF BugClearUnrelatedEntry
                 THEN
                   IF height = InputHeight(candidate)
                   THEN InputBlock(candidate)
                   ELSE "none"
                 ELSE IF BugOverwriteUnrelatedEntry
                      THEN
                        IF height = InputHeight(candidate)
                           \/ InitialBlockAt(candidate, height) /= "none"
                        THEN InputBlock(candidate)
                        ELSE InitialBlockAt(candidate, height)
                      ELSE
                        IF height = InputHeight(candidate)
                        THEN InputBlock(candidate)
                        ELSE InitialBlockAt(candidate, height)

ImplementationDuplicateBlockAt(candidate, height) ==
  IF BugOverwriteDuplicate
  THEN
    IF height = InputHeight(candidate)
    THEN WrongBlock(InputBlock(candidate))
    ELSE InitialBlockAt(candidate, height)
  ELSE IF BugClearDuplicate
       THEN
         IF height = InputHeight(candidate)
         THEN "none"
         ELSE InitialBlockAt(candidate, height)
       ELSE IF BugDuplicateRecordsWrongHeight
            THEN
              IF height = WrongHeight(InputHeight(candidate))
              THEN InputBlock(candidate)
              ELSE InitialBlockAt(candidate, height)
            ELSE InitialBlockAt(candidate, height)

ImplementationConflictBlockAt(candidate, height) ==
  IF BugOverwriteConflict
  THEN
    IF height = InputHeight(candidate)
    THEN InputBlock(candidate)
    ELSE InitialBlockAt(candidate, height)
  ELSE IF BugClearExistingOnConflict
       THEN
         IF height = InputHeight(candidate)
         THEN "none"
         ELSE InitialBlockAt(candidate, height)
       ELSE IF BugConflictRecordsWrongHeight
            THEN
              IF height = WrongHeight(InputHeight(candidate))
              THEN InputBlock(candidate)
              ELSE InitialBlockAt(candidate, height)
            ELSE InitialBlockAt(candidate, height)

ImplementationBlockAt(candidate, height) ==
  IF Fresh(candidate)
  THEN ImplementationFreshBlockAt(candidate, height)
  ELSE IF Duplicate(candidate)
       THEN ImplementationDuplicateBlockAt(candidate, height)
       ELSE ImplementationConflictBlockAt(candidate, height)

TypeInvariant ==
  /\ BugSkipFreshRecord \in BOOLEAN
  /\ BugRecordWrongHeight \in BOOLEAN
  /\ BugRecordWrongBlock \in BOOLEAN
  /\ BugClearUnrelatedEntry \in BOOLEAN
  /\ BugOverwriteUnrelatedEntry \in BOOLEAN
  /\ BugOverwriteDuplicate \in BOOLEAN
  /\ BugClearDuplicate \in BOOLEAN
  /\ BugDuplicateRecordsWrongHeight \in BOOLEAN
  /\ BugOverwriteConflict \in BOOLEAN
  /\ BugClearExistingOnConflict \in BOOLEAN
  /\ BugConflictRecordsWrongHeight \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InputHeight(candidate) \in Heights
    /\ InputBlock(candidate) \in Blocks
    /\ \A height \in Heights:
      /\ InitialBlockAt(candidate, height) \in Blocks
      /\ SpecBlockAt(candidate, height) \in Blocks
      /\ ImplementationBlockAt(candidate, height) \in Blocks

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

CommitMapMatchesSpec ==
  \A candidate \in tried:
    \A height \in Heights:
      ImplementationBlockAt(candidate, height) = SpecBlockAt(candidate, height)

FreshNotificationsRecordExactHeightBlock ==
  \A candidate \in tried:
    Fresh(candidate) =>
      /\ ImplementationBlockAt(candidate, InputHeight(candidate)) =
           InputBlock(candidate)
      /\ \A height \in Heights \ {InputHeight(candidate)}:
           ImplementationBlockAt(candidate, height) =
             InitialBlockAt(candidate, height)

DuplicateNotificationsPreserveMap ==
  \A candidate \in tried:
    Duplicate(candidate) =>
      \A height \in Heights:
        ImplementationBlockAt(candidate, height) =
          InitialBlockAt(candidate, height)

ConflictingNotificationsPreserveMap ==
  \A candidate \in tried:
    Conflict(candidate) =>
      \A height \in Heights:
        ImplementationBlockAt(candidate, height) =
          InitialBlockAt(candidate, height)

NoSpuriousWrongHeightRecords ==
  \A candidate \in tried:
    ImplementationBlockAt(candidate, "height_wrong") = "none"

ExistingInputHeightNeverClearedOnReplay ==
  \A candidate \in tried:
    (Duplicate(candidate) \/ Conflict(candidate)) =>
      ImplementationBlockAt(candidate, InputHeight(candidate)) =
        InitialBlockAt(candidate, InputHeight(candidate))

FreshDoesNotOverwriteUnrelatedHeight ==
  \A candidate \in tried:
    Fresh(candidate) =>
      \A height \in Heights \ {InputHeight(candidate)}:
        ImplementationBlockAt(candidate, height) =
          InitialBlockAt(candidate, height)

ValuesStayInDomain ==
  \A candidate \in tried:
    \A height \in Heights:
      /\ SpecBlockAt(candidate, height) \in Blocks
      /\ ImplementationBlockAt(candidate, height) \in Blocks

EngineCommittedBlockRecordExactness ==
  /\ CommitMapMatchesSpec
  /\ FreshNotificationsRecordExactHeightBlock
  /\ DuplicateNotificationsPreserveMap
  /\ ConflictingNotificationsPreserveMap
  /\ NoSpuriousWrongHeightRecords
  /\ ExistingInputHeightNeverClearedOnReplay
  /\ FreshDoesNotOverwriteUnrelatedHeight
  /\ ValuesStayInDomain

Safety ==
  EngineCommittedBlockRecordExactness

EngineCommittedBlockRecordCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineCommittedBlockRecordExactness

SafetyFast == EngineCommittedBlockRecordExactness

====
