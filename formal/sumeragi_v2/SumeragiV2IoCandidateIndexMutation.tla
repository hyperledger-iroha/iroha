---- MODULE SumeragiV2IoCandidateIndexMutation ----
EXTENDS Naturals, Sequences, TLC

(***************************************************************************
Minimal mutation for the stage-5 candidate index.  Serve jobs may carry the
same candidate-shaped record as a Consensus job, but they do not own that
candidate.  Choosing over every physical I/O position can therefore select a
Serve occurrence and falsify the ownership characterization at the initial
state.  The repaired index chooses only from Consensus positions.
***************************************************************************)

VARIABLE queue

vars == <<queue>>

Target == [identity |-> 0]

ServeJob == [class |-> "Serve", candidate |-> Target]

ConsensusJob == [class |-> "Consensus", candidate |-> Target]

Init == queue = <<ServeJob, ConsensusJob>>

AllTargetIndices ==
  {index \in 1..Len(queue): queue[index].candidate = Target}

ConsensusTargetIndices ==
  {index \in 1..Len(queue):
     /\ queue[index].class = "Consensus"
     /\ queue[index].candidate = Target}

OldCandidateIndex ==
  CHOOSE index \in AllTargetIndices: TRUE

FixedCandidateIndex ==
  CHOOSE index \in ConsensusTargetIndices: TRUE

OldIndexSelectsConsensus ==
  queue[OldCandidateIndex].class = "Consensus"

FixedIndexSelectsConsensus ==
  queue[FixedCandidateIndex].class = "Consensus"

Next == UNCHANGED vars

Spec == Init /\ [][Next]_vars

=============================================================================
