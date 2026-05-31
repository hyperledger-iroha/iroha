---- MODULE SumeragiEnginePayloadAvailabilityRecordGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for exact payload-availability recording.

This slice models the first side effect in
`ConsensusEngine::on_payload_available(...)`:
`available_payloads.insert((subject.block_hash, subject.payload_hash))`.
The insertion is unconditional. It must record exactly the input subject's
block/payload pair, preserve already recorded availability pairs, and remain
idempotent when the exact pair is already known. Pending-finality matching and
mismatch cases are included only to prove they do not change the availability
recording key.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipRecord,
  \* @type: Bool;
  BugRecordOnlyWhenPending,
  \* @type: Bool;
  BugRecordOnlyOnMatch,
  \* @type: Bool;
  BugRecordWrongBlock,
  \* @type: Bool;
  BugRecordWrongPayload,
  \* @type: Bool;
  BugRecordParentAsBlock,
  \* @type: Bool;
  BugRecordPendingSubjectInstead,
  \* @type: Bool;
  BugClearExistingAvailability,
  \* @type: Bool;
  BugDropUnrelatedAvailability

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "no_pending_empty",
  "no_pending_existing_unrelated",
  "matching_pending",
  "payload_mismatch",
  "parent_mismatch",
  "unknown_block_hash",
  "duplicate_existing",
  "same_payload_wrong_block"
}

PairValues == {
  "pair_subject_a",
  "pair_subject_b",
  "pair_pending",
  "pair_block_wrong_payload",
  "pair_unknown_block_payload",
  "pair_wrong_block_same_payload",
  "pair_unrelated",
  "pair_wrong_block",
  "pair_wrong_payload",
  "pair_parent_payload"
}

HasPending(candidate) ==
  candidate \in {
    "matching_pending",
    "payload_mismatch",
    "parent_mismatch",
    "unknown_block_hash"
  }

MatchesPending(candidate) ==
  candidate = "matching_pending"

MismatchesPending(candidate) ==
  candidate \in {
    "payload_mismatch",
    "parent_mismatch",
    "unknown_block_hash"
  }

ParentMismatch(candidate) ==
  candidate = "parent_mismatch"

NoPending(candidate) ==
  candidate \in {
    "no_pending_empty",
    "no_pending_existing_unrelated",
    "duplicate_existing",
    "same_payload_wrong_block"
  }

InitialAvailable(candidate) ==
  CASE candidate = "no_pending_existing_unrelated" -> {"pair_unrelated"}
    [] candidate = "payload_mismatch" -> {"pair_unrelated"}
    [] candidate = "unknown_block_hash" -> {"pair_unrelated"}
    [] candidate = "duplicate_existing" -> {"pair_subject_a"}
    [] OTHER -> {}

InputAvailablePair(candidate) ==
  CASE candidate = "no_pending_empty" -> "pair_subject_a"
    [] candidate = "no_pending_existing_unrelated" -> "pair_subject_b"
    [] candidate = "matching_pending" -> "pair_pending"
    [] candidate = "payload_mismatch" -> "pair_block_wrong_payload"
    [] candidate = "parent_mismatch" -> "pair_pending"
    [] candidate = "unknown_block_hash" -> "pair_unknown_block_payload"
    [] candidate = "duplicate_existing" -> "pair_subject_a"
    [] candidate = "same_payload_wrong_block" -> "pair_wrong_block_same_payload"
    [] OTHER -> "pair_subject_a"

SpecFinalAvailable(candidate) ==
  InitialAvailable(candidate) \cup {InputAvailablePair(candidate)}

ShouldRecord(candidate) ==
  /\ ~BugSkipRecord
  /\ ~(BugRecordOnlyWhenPending /\ ~HasPending(candidate))
  /\ ~(BugRecordOnlyOnMatch /\ ~MatchesPending(candidate))

ImplementationBaseAvailable(candidate) ==
  IF BugClearExistingAvailability
  THEN {}
  ELSE IF BugDropUnrelatedAvailability
       THEN InitialAvailable(candidate) \ {"pair_unrelated"}
       ELSE InitialAvailable(candidate)

ImplementationRecordedPair(candidate) ==
  CASE BugRecordParentAsBlock /\ ParentMismatch(candidate) -> "pair_parent_payload"
    [] BugRecordPendingSubjectInstead /\ HasPending(candidate) -> "pair_pending"
    [] BugRecordWrongBlock -> "pair_wrong_block"
    [] BugRecordWrongPayload -> "pair_wrong_payload"
    [] OTHER -> InputAvailablePair(candidate)

ImplementationFinalAvailable(candidate) ==
  IF ShouldRecord(candidate)
  THEN ImplementationBaseAvailable(candidate) \cup {ImplementationRecordedPair(candidate)}
  ELSE ImplementationBaseAvailable(candidate)

TypeInvariant ==
  /\ BugSkipRecord \in BOOLEAN
  /\ BugRecordOnlyWhenPending \in BOOLEAN
  /\ BugRecordOnlyOnMatch \in BOOLEAN
  /\ BugRecordWrongBlock \in BOOLEAN
  /\ BugRecordWrongPayload \in BOOLEAN
  /\ BugRecordParentAsBlock \in BOOLEAN
  /\ BugRecordPendingSubjectInstead \in BOOLEAN
  /\ BugClearExistingAvailability \in BOOLEAN
  /\ BugDropUnrelatedAvailability \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InitialAvailable(candidate) \subseteq PairValues
    /\ InputAvailablePair(candidate) \in PairValues
    /\ SpecFinalAvailable(candidate) \subseteq PairValues
    /\ ImplementationBaseAvailable(candidate) \subseteq PairValues
    /\ ImplementationRecordedPair(candidate) \in PairValues
    /\ ImplementationFinalAvailable(candidate) \subseteq PairValues

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

FinalAvailabilityMatchesSpec ==
  \A candidate \in tried:
    ImplementationFinalAvailable(candidate) = SpecFinalAvailable(candidate)

EveryPayloadRecordsExactInputPair ==
  \A candidate \in tried:
    InputAvailablePair(candidate) \in ImplementationFinalAvailable(candidate)

NoWrongAvailabilityPairRecorded ==
  \A candidate \in tried:
    ImplementationFinalAvailable(candidate) \subseteq SpecFinalAvailable(candidate)

ExistingAvailabilityPreserved ==
  \A candidate \in tried:
    InitialAvailable(candidate) \subseteq ImplementationFinalAvailable(candidate)

NoPendingStillRecordsInputPair ==
  \A candidate \in tried:
    NoPending(candidate) =>
      InputAvailablePair(candidate) \in ImplementationFinalAvailable(candidate)

PendingMismatchStillRecordsInputPair ==
  \A candidate \in tried:
    MismatchesPending(candidate) =>
      InputAvailablePair(candidate) \in ImplementationFinalAvailable(candidate)

DuplicateRecordIsIdempotent ==
  "duplicate_existing" \in tried =>
    ImplementationFinalAvailable("duplicate_existing") = {"pair_subject_a"}

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ InitialAvailable(candidate) \subseteq PairValues
    /\ InputAvailablePair(candidate) \in PairValues
    /\ SpecFinalAvailable(candidate) \subseteq PairValues
    /\ ImplementationFinalAvailable(candidate) \subseteq PairValues

Safety ==
  /\ FinalAvailabilityMatchesSpec
  /\ EveryPayloadRecordsExactInputPair
  /\ NoWrongAvailabilityPairRecorded
  /\ ExistingAvailabilityPreserved
  /\ NoPendingStillRecordsInputPair
  /\ PendingMismatchStillRecordsInputPair
  /\ DuplicateRecordIsIdempotent
  /\ ValuesStayInDomain

====
