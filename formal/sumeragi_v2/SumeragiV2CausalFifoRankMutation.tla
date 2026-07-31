---- MODULE SumeragiV2CausalFifoRankMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
A compact regression for the Stage-6 causal FIFO rank.  The target starts
behind one earlier causal candidate while Causal is the preferred local
source.  Removing that earlier head also resets the local-source cursor to
Producer, so the source-distance bit changes from zero to one.  Multiplying
the FIFO index by two makes the target rank fall from four to three.  The
historical multiplier-one mutation leaves it at two and violates the strict
descent invariant.

This model checks only the FIFO/cursor arithmetic.  It does not model or close
the separate Completion causal-capacity liveness obligation.
***************************************************************************)

CONSTANT RankMultiplier

ASSUME RankMultiplier \in {1, 2}

VARIABLES causalQueue, preferredLocalSource, earlierHeadRemoved

EarlierCandidate ==
  [node |-> "validator", class |-> "Progress", kind |-> "Earlier"]

TargetCandidate ==
  [node |-> "validator", class |-> "Progress", kind |-> "Target"]

vars == <<causalQueue, preferredLocalSource, earlierHeadRemoved>>

CandidateSequenceIndex(candidate, queue) ==
  CHOOSE index \in 1..Len(queue): queue[index] = candidate

LocalSourceDistance(source) ==
  IF preferredLocalSource = source THEN 0 ELSE 1

CausalCandidatePosition(candidate) ==
  RankMultiplier * CandidateSequenceIndex(candidate, causalQueue)
    + LocalSourceDistance("Causal")

TargetRank == CausalCandidatePosition(TargetCandidate)

InitialTargetRank == RankMultiplier * 2

TypeInvariant ==
  /\ causalQueue \in {<<EarlierCandidate, TargetCandidate>>,
                       <<TargetCandidate>>}
  /\ preferredLocalSource \in {"Causal", "Producer"}
  /\ earlierHeadRemoved \in BOOLEAN
  /\ TargetCandidate \in {causalQueue[index]: index \in 1..Len(causalQueue)}

TargetIndexIsNatural ==
  CandidateSequenceIndex(TargetCandidate, causalQueue) \in Nat

Init ==
  /\ causalQueue = <<EarlierCandidate, TargetCandidate>>
  /\ preferredLocalSource = "Causal"
  /\ earlierHeadRemoved = FALSE

RemoveEarlierHead ==
  /\ ~earlierHeadRemoved
  /\ causalQueue = <<EarlierCandidate, TargetCandidate>>
  /\ causalQueue' = Tail(causalQueue)
  /\ preferredLocalSource' = "Producer"
  /\ earlierHeadRemoved' = TRUE

Next == RemoveEarlierHead

Spec == Init /\ [][Next]_vars

EarlierHeadRemovalStrictlyDropsTargetRank ==
  earlierHeadRemoved => TargetRank < InitialTargetRank

=============================================================================
