---- MODULE SumeragiVNextRechainGate ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
A bounded abstract model for the quarantined vNext re-chain helper.

The Rust helper `ChainOrder::rechain_after_suspicions(...)` accepts only
successor-scoped suspicion evidence for the current slot/order/sequence,
canonicalizes evidence, applies it sequentially to the evolving order, moves
the accuser and accused validators to the quarantine tail, and rejects the
re-chain if the remaining critical path no longer satisfies the configured
quorum policy.

This model checks a finite set of representative one- and two-evidence cases:
valid count-quorum re-chain, valid multi-evidence re-chain, empty evidence,
slot/hash/sequence mismatch, non-successor and tail-accuser accusations,
duplicate evidence, no-longer-successor after earlier evidence, insufficient
untainted validators, count-quorum failure, strict stake-quorum failure, taint
placement, sequence increment, and certificate body consistency.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAcceptEmptyEvidence,
  \* @type: Bool;
  BugIgnoreSlotMismatch,
  \* @type: Bool;
  BugIgnoreOrderHashMismatch,
  \* @type: Bool;
  BugIgnoreSequenceMismatch,
  \* @type: Bool;
  BugAcceptNonSuccessor,
  \* @type: Bool;
  BugAllowTailAccuser,
  \* @type: Bool;
  BugSkipSequentialScope,
  \* @type: Bool;
  BugAllowDuplicateEvidence,
  \* @type: Bool;
  BugIgnoreUntaintedLimit,
  \* @type: Bool;
  BugIgnoreCountQuorum,
  \* @type: Bool;
  BugUseNonStrictStake,
  \* @type: Bool;
  BugDropAccuserTaint,
  \* @type: Bool;
  BugDropAccusedTaint,
  \* @type: Bool;
  BugKeepTaintedInCritical,
  \* @type: Bool;
  BugDoNotIncrementSequence,
  \* @type: Bool;
  BugMutateCertificateSlot,
  \* @type: Bool;
  BugReusePreviousHash

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  accepted,
  \* @type: Set(Int);
  tainted,
  \* @type: Set(Int);
  newCritical,
  \* @type: Int;
  newSeq,
  \* @type: Bool;
  certSlotMatches,
  \* @type: Bool;
  newHashChanged

vars == <<candidate, accepted, tainted, newCritical, newSeq, certSlotMatches, newHashChanged>>

Cases == {
  "success_count",
  "multi_success",
  "empty",
  "slot_mismatch",
  "hash_mismatch",
  "seq_mismatch",
  "non_successor",
  "tail_accuser",
  "duplicate",
  "multi_no_longer_successor",
  "insufficient_untainted",
  "count_quorum_fail",
  "stake_boundary"
}

AcceptedCases == {"success_count", "multi_success"}

InvalidEvidenceCases == {
  "empty",
  "slot_mismatch",
  "hash_mismatch",
  "seq_mismatch",
  "non_successor",
  "tail_accuser",
  "duplicate",
  "multi_no_longer_successor"
}

QuorumFailureCases == {"insufficient_untainted", "count_quorum_fail", "stake_boundary"}

Validators == 1..9

EvidencePositions == 1..2

OrderLen(c) ==
  CASE c = "multi_success" -> 9
    [] c = "multi_no_longer_successor" -> 7
    [] c = "insufficient_untainted" -> 4
    [] OTHER -> 5

CriticalLen(c) ==
  CASE c = "multi_success" -> 5
    [] c = "multi_no_longer_successor" -> 4
    [] OTHER -> 3

RequiredCount(c) ==
  CASE c = "count_quorum_fail" -> 4
    [] OTHER -> 3

Policy(c) ==
  IF c = "stake_boundary" THEN "stake" ELSE "count"

RechainSeq(c) == 0

EvidenceCount(c) ==
  CASE c = "empty" -> 0
    [] c \in {"duplicate", "multi_success", "multi_no_longer_successor"} -> 2
    [] OTHER -> 1

Accuser(c, idx) ==
  CASE c = "non_successor" -> 1
    [] c = "tail_accuser" -> 3
    [] c = "multi_success" /\ idx = 1 -> 3
    [] c = "multi_success" /\ idx = 2 -> 1
    [] c = "multi_no_longer_successor" /\ idx = 1 -> 2
    [] c = "multi_no_longer_successor" /\ idx = 2 -> 3
    [] OTHER -> 2

Accused(c, idx) ==
  CASE c = "non_successor" -> 3
    [] c = "tail_accuser" -> 4
    [] c = "multi_success" /\ idx = 1 -> 4
    [] c = "multi_success" /\ idx = 2 -> 2
    [] c = "multi_no_longer_successor" /\ idx = 1 -> 3
    [] c = "multi_no_longer_successor" /\ idx = 2 -> 4
    [] OTHER -> 3

SlotMatches(c) == c # "slot_mismatch"

OrderHashMatches(c) == c # "hash_mismatch"

SequenceMatches(c) == c # "seq_mismatch"

DuplicateEvidence(c) == c = "duplicate"

OriginalCritical(c) ==
  {peer \in Validators : peer <= CriticalLen(c)}

HasCriticalSuccessor(c, peer) ==
  /\ peer \in Validators
  /\ peer < CriticalLen(c)

OriginalSuccessorPair(c, accuser, accused) ==
  /\ accuser \in Validators
  /\ accused \in Validators
  /\ accuser < CriticalLen(c)
  /\ accused = accuser + 1

EvidenceIndices(c) ==
  {idx \in EvidencePositions : idx <= EvidenceCount(c)}

OriginalSuccessorEvidenceOk(c) ==
  \A idx \in EvidenceIndices(c):
    OriginalSuccessorPair(c, Accuser(c, idx), Accused(c, idx))

TailAccuserEvidence(c) ==
  /\ c = "tail_accuser"
  /\ EvidenceCount(c) = 1
  /\ ~HasCriticalSuccessor(c, Accuser(c, 1))

SequentialScopeOk(c) ==
  c # "multi_no_longer_successor"

SpecEvidenceOk(c) ==
  /\ EvidenceCount(c) > 0
  /\ SlotMatches(c)
  /\ OrderHashMatches(c)
  /\ SequenceMatches(c)
  /\ ~DuplicateEvidence(c)
  /\ OriginalSuccessorEvidenceOk(c)
  /\ SequentialScopeOk(c)

Accusers(c) ==
  {Accuser(c, idx) : idx \in EvidenceIndices(c)}

AccusedValidators(c) ==
  {Accused(c, idx) : idx \in EvidenceIndices(c)}

SpecTainted(c) ==
  Accusers(c) \union AccusedValidators(c)

Untainted(c) ==
  {peer \in Validators : peer <= OrderLen(c)} \ SpecTainted(c)

UntaintedEnough(c) ==
  Cardinality(Untainted(c)) >= CriticalLen(c)

SpecNewCriticalIndices(c) ==
  {
    i \in Validators :
      /\ i <= OrderLen(c)
      /\ i \notin SpecTainted(c)
      /\ Cardinality({
           j \in Validators :
             /\ j <= i
             /\ j <= OrderLen(c)
             /\ j \notin SpecTainted(c)
         }) <= CriticalLen(c)
  }

SpecNewCritical(c) ==
  SpecNewCriticalIndices(c)

StrictStakeQuorum(c) ==
  c # "stake_boundary"

NonStrictStakeQuorum(c) ==
  c = "stake_boundary"

SpecQuorumOk(c) ==
  IF Policy(c) = "count"
  THEN CriticalLen(c) >= RequiredCount(c)
  ELSE StrictStakeQuorum(c)

ActualQuorumOk(c) ==
  IF Policy(c) = "count"
  THEN CriticalLen(c) >= RequiredCount(c) \/ BugIgnoreCountQuorum
  ELSE StrictStakeQuorum(c) \/ (BugUseNonStrictStake /\ NonStrictStakeQuorum(c))

SpecAccept(c) ==
  /\ SpecEvidenceOk(c)
  /\ UntaintedEnough(c)
  /\ SpecQuorumOk(c)

ActualEvidenceOk(c) ==
  /\ (EvidenceCount(c) > 0 \/ BugAcceptEmptyEvidence)
  /\ (SlotMatches(c) \/ BugIgnoreSlotMismatch)
  /\ (OrderHashMatches(c) \/ BugIgnoreOrderHashMismatch)
  /\ (SequenceMatches(c) \/ BugIgnoreSequenceMismatch)
  /\ (~DuplicateEvidence(c) \/ BugAllowDuplicateEvidence)
  /\ (OriginalSuccessorEvidenceOk(c)
      \/ BugAcceptNonSuccessor
      \/ (BugAllowTailAccuser /\ TailAccuserEvidence(c)))
  /\ (SequentialScopeOk(c) \/ BugSkipSequentialScope)

ActualAccept(c) ==
  /\ ActualEvidenceOk(c)
  /\ (UntaintedEnough(c) \/ BugIgnoreUntaintedLimit)
  /\ ActualQuorumOk(c)

ActualTainted(c) ==
  (SpecTainted(c) \ (IF BugDropAccuserTaint THEN Accusers(c) ELSE {}))
    \ (IF BugDropAccusedTaint THEN AccusedValidators(c) ELSE {})

ActualNewCritical(c) ==
  IF BugKeepTaintedInCritical
  THEN OriginalCritical(c)
  ELSE SpecNewCritical(c)

ActualNewSeq(c) ==
  IF BugDoNotIncrementSequence
  THEN RechainSeq(c)
  ELSE RechainSeq(c) + EvidenceCount(c)

TypeInvariant ==
  /\ BugAcceptEmptyEvidence \in BOOLEAN
  /\ BugIgnoreSlotMismatch \in BOOLEAN
  /\ BugIgnoreOrderHashMismatch \in BOOLEAN
  /\ BugIgnoreSequenceMismatch \in BOOLEAN
  /\ BugAcceptNonSuccessor \in BOOLEAN
  /\ BugAllowTailAccuser \in BOOLEAN
  /\ BugSkipSequentialScope \in BOOLEAN
  /\ BugAllowDuplicateEvidence \in BOOLEAN
  /\ BugIgnoreUntaintedLimit \in BOOLEAN
  /\ BugIgnoreCountQuorum \in BOOLEAN
  /\ BugUseNonStrictStake \in BOOLEAN
  /\ BugDropAccuserTaint \in BOOLEAN
  /\ BugDropAccusedTaint \in BOOLEAN
  /\ BugKeepTaintedInCritical \in BOOLEAN
  /\ BugDoNotIncrementSequence \in BOOLEAN
  /\ BugMutateCertificateSlot \in BOOLEAN
  /\ BugReusePreviousHash \in BOOLEAN
  /\ candidate \in Cases \union {"none"}
  /\ accepted \in BOOLEAN
  /\ tainted \subseteq Validators
  /\ newCritical \subseteq Validators
  /\ newSeq \in 0..2
  /\ certSlotMatches \in BOOLEAN
  /\ newHashChanged \in BOOLEAN

Init ==
  /\ candidate = "none"
  /\ accepted = FALSE
  /\ tainted = {}
  /\ newCritical = {}
  /\ newSeq = 0
  /\ certSlotMatches = TRUE
  /\ newHashChanged = FALSE

Apply(c) ==
  /\ candidate' = c
  /\ accepted' = ActualAccept(c)
  /\ tainted' = IF ActualAccept(c) THEN ActualTainted(c) ELSE {}
  /\ newCritical' = IF ActualAccept(c) THEN ActualNewCritical(c) ELSE {}
  /\ newSeq' = IF ActualAccept(c) THEN ActualNewSeq(c) ELSE RechainSeq(c)
  /\ certSlotMatches' =
       IF ActualAccept(c) THEN ~BugMutateCertificateSlot ELSE TRUE
  /\ newHashChanged' =
       IF ActualAccept(c) THEN ~BugReusePreviousHash ELSE FALSE

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

AcceptMatchesSpec ==
  candidate = "none" \/ accepted = SpecAccept(candidate)

AcceptedTaintSetMatchesSpec ==
  candidate = "none" \/ (accepted => tainted = SpecTainted(candidate))

AcceptedCriticalPathMatchesSpec ==
  candidate = "none" \/ (accepted => newCritical = SpecNewCritical(candidate))

AcceptedCriticalPathExcludesTainted ==
  accepted => tainted \cap newCritical = {}

AcceptedSequenceIncrements ==
  candidate = "none"
    \/ (accepted => newSeq = RechainSeq(candidate) + EvidenceCount(candidate))

AcceptedCertificateBodyConsistent ==
  accepted => certSlotMatches /\ newHashChanged

RejectedHasNoCertificate ==
  candidate = "none"
    \/ accepted
    \/ /\ tainted = {}
       /\ newCritical = {}
       /\ newSeq = RechainSeq(candidate)
       /\ ~newHashChanged

InvalidEvidenceFailsClosed ==
  candidate \in InvalidEvidenceCases => ~accepted

QuarantineAndQuorumFailClosed ==
  candidate \in QuorumFailureCases => ~accepted

ValidEvidenceCanRechain ==
  candidate \in AcceptedCases => accepted

Safety ==
  /\ AcceptMatchesSpec
  /\ AcceptedTaintSetMatchesSpec
  /\ AcceptedCriticalPathMatchesSpec
  /\ AcceptedCriticalPathExcludesTainted
  /\ AcceptedSequenceIncrements
  /\ AcceptedCertificateBodyConsistent
  /\ RejectedHasNoCertificate
  /\ InvalidEvidenceFailsClosed
  /\ QuarantineAndQuorumFailClosed
  /\ ValidEvidenceCanRechain

====
