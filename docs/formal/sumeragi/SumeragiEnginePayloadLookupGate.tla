---- MODULE SumeragiEnginePayloadLookupGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for the pure-engine payload lookup helper.

This slice models `ConsensusEngine::has_payload(...)`. The helper is the
exact local-availability guard used by commit-QC handling before immediate
finality. Availability is keyed by the pair `(block_hash, payload_hash)`, so a
matching block hash with a different payload hash, a matching payload hash for
another block, any unrelated recorded payload, or an empty store must all
return false.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugIgnoreBlockHash,
  \* @type: Bool;
  BugIgnorePayloadHash,
  \* @type: Bool;
  BugAcceptAnyRecordedPayload,
  \* @type: Bool;
  BugAcceptEmptyStore,
  \* @type: Bool;
  BugRejectExactPair,
  \* @type: Bool;
  BugInvertLookup

VARIABLES
  \* @type: Set(Str);
  tried,
  \* @type: Set(Str);
  accepted,
  \* @type: Set(Str);
  rejected

\* @type: <<Set(Str), Set(Str), Set(Str)>>;
vars == <<tried, accepted, rejected>>

Candidates == {
  "exact_pair",
  "same_block_wrong_payload",
  "wrong_block_same_payload",
  "wrong_block_wrong_payload",
  "empty_store"
}

SpecAccepts(candidate) ==
  candidate = "exact_pair"

HasRecordedPayload(candidate) ==
  candidate # "empty_store"

ImplementationAcceptsWithoutInvert(candidate) ==
  \/ /\ candidate = "exact_pair"
     /\ ~BugRejectExactPair
  \/ /\ candidate = "same_block_wrong_payload"
     /\ (BugIgnorePayloadHash \/ BugAcceptAnyRecordedPayload)
  \/ /\ candidate = "wrong_block_same_payload"
     /\ (BugIgnoreBlockHash \/ BugAcceptAnyRecordedPayload)
  \/ /\ candidate = "wrong_block_wrong_payload"
     /\ BugAcceptAnyRecordedPayload
  \/ /\ candidate = "empty_store"
     /\ BugAcceptEmptyStore

ImplementationAccepts(candidate) ==
  IF BugInvertLookup
  THEN ~SpecAccepts(candidate)
  ELSE ImplementationAcceptsWithoutInvert(candidate)

TypeInvariant ==
  /\ BugIgnoreBlockHash \in BOOLEAN
  /\ BugIgnorePayloadHash \in BOOLEAN
  /\ BugAcceptAnyRecordedPayload \in BOOLEAN
  /\ BugAcceptEmptyStore \in BOOLEAN
  /\ BugRejectExactPair \in BOOLEAN
  /\ BugInvertLookup \in BOOLEAN
  /\ tried \subseteq Candidates
  /\ accepted \subseteq Candidates
  /\ rejected \subseteq Candidates
  /\ accepted \cap rejected = {}
  /\ accepted \cup rejected = tried

Init ==
  /\ tried = {}
  /\ accepted = {}
  /\ rejected = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}
  /\ IF ImplementationAccepts(candidate)
     THEN
       /\ accepted' = accepted \cup {candidate}
       /\ rejected' = rejected
     ELSE
       /\ accepted' = accepted
       /\ rejected' = rejected \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

AcceptedMatchesSpec ==
  accepted \subseteq {candidate \in Candidates : SpecAccepts(candidate)}

RejectedMatchesSpec ==
  rejected \subseteq {candidate \in Candidates : ~SpecAccepts(candidate)}

ExactPairAccepted ==
  "exact_pair" \in tried =>
    "exact_pair" \in accepted

SameBlockWrongPayloadRejected ==
  "same_block_wrong_payload" \in tried =>
    "same_block_wrong_payload" \in rejected

WrongBlockSamePayloadRejected ==
  "wrong_block_same_payload" \in tried =>
    "wrong_block_same_payload" \in rejected

WrongBlockWrongPayloadRejected ==
  "wrong_block_wrong_payload" \in tried =>
    "wrong_block_wrong_payload" \in rejected

EmptyStoreRejected ==
  "empty_store" \in tried =>
    "empty_store" \in rejected

RecordedPayloadAloneIsInsufficient ==
  \A candidate \in tried:
    /\ HasRecordedPayload(candidate)
    /\ candidate # "exact_pair"
    =>
      candidate \in rejected

EnginePayloadLookupExactness ==
  /\ AcceptedMatchesSpec
  /\ RejectedMatchesSpec
  /\ ExactPairAccepted
  /\ SameBlockWrongPayloadRejected
  /\ WrongBlockSamePayloadRejected
  /\ WrongBlockWrongPayloadRejected
  /\ EmptyStoreRejected
  /\ RecordedPayloadAloneIsInsufficient

Safety ==
  EnginePayloadLookupExactness

EnginePayloadLookupCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EnginePayloadLookupExactness

SafetyFast == EnginePayloadLookupExactness

====
