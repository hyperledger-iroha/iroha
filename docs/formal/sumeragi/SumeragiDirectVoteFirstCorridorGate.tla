---- MODULE SumeragiDirectVoteFirstCorridorGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the central vote-first direct commit corridor.

This slice captures the no-fault path where prepare and commit votes reach
quorum before RBC delivery. Live vote/stake quorum must remain buffered without
commit-certificate artifacts until the later RBC DELIVER transition installs
finality from the delivered payload evidence.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  step

\* @type: <<Str>>;
vars == <<step>>

Cases == {
  "init",
  "propose",
  "prepare_1",
  "prepare_2",
  "prepare_quorum",
  "commit_1",
  "commit_2",
  "commit_buffered",
  "chunk_1",
  "chunk_2",
  "ready_1",
  "ready_2",
  "ready_quorum",
  "deliver_final"
}

Phases == {"Propose", "Prepare", "CommitVote", "Committed"}
RbcStates == {"Idle", "Init", "Chunking", "ChunksComplete", "ReadyPartial",
  "ReadyQuorum", "Delivered"}

BufferedCommitCases == {
  "commit_buffered",
  "chunk_1",
  "chunk_2",
  "ready_1",
  "ready_2",
  "ready_quorum"
}

SpecPhase(c) ==
  CASE c = "init" -> "Propose"
    [] c \in {"propose", "prepare_1", "prepare_2"} -> "Prepare"
    [] c = "deliver_final" -> "Committed"
    [] OTHER -> "CommitVote"

SpecRbcState(c) ==
  CASE c = "init" -> "Idle"
    [] c \in {
         "propose",
         "prepare_1",
         "prepare_2",
         "prepare_quorum",
         "commit_1",
         "commit_2",
         "commit_buffered"
       } -> "Init"
    [] c = "chunk_1" -> "Chunking"
    [] c = "chunk_2" -> "ChunksComplete"
    [] c \in {"ready_1", "ready_2"} -> "ReadyPartial"
    [] c = "ready_quorum" -> "ReadyQuorum"
    [] OTHER -> "Delivered"

SpecChunkCount(c) ==
  CASE c \in {
         "init",
         "propose",
         "prepare_1",
         "prepare_2",
         "prepare_quorum",
         "commit_1",
         "commit_2",
         "commit_buffered"
       } -> 0
    [] c = "chunk_1" -> 1
    [] OTHER -> 2

SpecReadyVotes(c) ==
  CASE c \in {
         "init",
         "propose",
         "prepare_1",
         "prepare_2",
         "prepare_quorum",
         "commit_1",
         "commit_2",
         "commit_buffered",
         "chunk_1",
         "chunk_2"
       } -> 0
    [] c = "ready_1" -> 1
    [] c = "ready_2" -> 2
    [] OTHER -> 3

SpecPrepareVotes(c) ==
  CASE c = "prepare_1" -> 1
    [] c = "prepare_2" -> 2
    [] c \in {
         "prepare_quorum",
         "commit_1",
         "commit_2",
         "commit_buffered",
         "chunk_1",
         "chunk_2",
         "ready_1",
         "ready_2",
         "ready_quorum",
         "deliver_final"
       } -> 3
    [] OTHER -> 0

SpecCommitVotes(c) ==
  CASE c = "commit_1" -> 1
    [] c = "commit_2" -> 2
    [] c \in BufferedCommitCases \union {"deliver_final"} -> 3
    [] OTHER -> 0

SpecStakeSigned(c) == 3 * SpecCommitVotes(c)

SpecCommitted(c) == c = "deliver_final"

SpecCommitEvidenceVotes(c) == IF SpecCommitted(c) THEN 3 ELSE 0

SpecCommitEvidenceStake(c) == IF SpecCommitted(c) THEN 9 ELSE 0

SpecHeaderSeen(c) == c # "init"

SpecDigestValid(c) == c # "init"

ActualPhase(c) ==
  IF Bug = "phase_committed_before_delivery" /\ c = "commit_buffered"
  THEN "Committed"
  ELSE SpecPhase(c)

ActualRbcState(c) ==
  IF Bug = "skip_deliver_state" /\ c = "deliver_final"
  THEN "ReadyQuorum"
  ELSE SpecRbcState(c)

ActualChunkCount(c) ==
  IF Bug = "deliver_without_chunks" /\ c = "deliver_final"
  THEN 1
  ELSE SpecChunkCount(c)

ActualReadyVotes(c) ==
  IF Bug = "deliver_without_ready_quorum" /\ c = "deliver_final"
  THEN 2
  ELSE SpecReadyVotes(c)

ActualPrepareVotes(c) ==
  IF Bug = "prepare_quorum_under_counted"
     /\ c \in {
          "prepare_quorum",
          "commit_1",
          "commit_2",
          "commit_buffered",
          "chunk_1",
          "chunk_2",
          "ready_1",
          "ready_2",
          "ready_quorum",
          "deliver_final"
        }
  THEN 2
  ELSE SpecPrepareVotes(c)

ActualCommitVotes(c) ==
  IF Bug = "buffered_commit_under_counted"
     /\ c \in BufferedCommitCases \union {"deliver_final"}
  THEN 2
  ELSE SpecCommitVotes(c)

ActualStakeSigned(c) ==
  IF Bug = "buffered_stake_not_recorded"
     /\ c \in BufferedCommitCases \union {"deliver_final"}
  THEN 6
  ELSE SpecStakeSigned(c)

ActualCommitted(c) ==
  IF Bug = "commit_before_delivery" /\ c = "commit_buffered"
  THEN TRUE
  ELSE IF Bug = "finality_not_latched" /\ c = "deliver_final"
  THEN FALSE
  ELSE SpecCommitted(c)

ActualCommitEvidenceVotes(c) ==
  IF Bug = "commit_evidence_before_delivery" /\ c = "commit_buffered"
  THEN 3
  ELSE IF Bug = "commit_evidence_votes_missing" /\ c = "deliver_final"
  THEN 0
  ELSE SpecCommitEvidenceVotes(c)

ActualCommitEvidenceStake(c) ==
  IF Bug = "commit_evidence_before_delivery" /\ c = "commit_buffered"
  THEN 9
  ELSE IF Bug = "commit_evidence_stake_missing" /\ c = "deliver_final"
  THEN 0
  ELSE SpecCommitEvidenceStake(c)

ActualHeaderSeen(c) ==
  IF Bug = "header_not_seeded" /\ c # "init"
  THEN FALSE
  ELSE SpecHeaderSeen(c)

ActualDigestValid(c) ==
  IF Bug = "digest_not_seeded" /\ c # "init"
  THEN FALSE
  ELSE SpecDigestValid(c)

Init == step = "none"

CheckCase(c) ==
  /\ step = "none"
  /\ step' = c

Next ==
  \/ \E c \in Cases: CheckCase(c)
  \/ /\ step # "none"
     /\ UNCHANGED vars

TypeInvariant ==
  /\ step \in Cases \union {"none"}
  /\ \A c \in Cases:
       /\ SpecPhase(c) \in Phases
       /\ ActualPhase(c) \in Phases
       /\ SpecRbcState(c) \in RbcStates
       /\ ActualRbcState(c) \in RbcStates
       /\ SpecChunkCount(c) \in 0..2
       /\ ActualChunkCount(c) \in 0..2
       /\ SpecReadyVotes(c) \in 0..3
       /\ ActualReadyVotes(c) \in 0..3
       /\ SpecPrepareVotes(c) \in 0..3
       /\ ActualPrepareVotes(c) \in 0..3
       /\ SpecCommitVotes(c) \in 0..3
       /\ ActualCommitVotes(c) \in 0..3
       /\ SpecStakeSigned(c) \in 0..9
       /\ ActualStakeSigned(c) \in 0..9
       /\ SpecCommitted(c) \in BOOLEAN
       /\ ActualCommitted(c) \in BOOLEAN
       /\ SpecHeaderSeen(c) \in BOOLEAN
       /\ ActualHeaderSeen(c) \in BOOLEAN
       /\ SpecDigestValid(c) \in BOOLEAN
       /\ ActualDigestValid(c) \in BOOLEAN

PhaseMatchesSpec ==
  step = "none" \/ ActualPhase(step) = SpecPhase(step)

RbcStateMatchesSpec ==
  step = "none" \/ ActualRbcState(step) = SpecRbcState(step)

RbcEvidenceMatchesSpec ==
  step = "none" \/
    /\ ActualChunkCount(step) = SpecChunkCount(step)
    /\ ActualReadyVotes(step) = SpecReadyVotes(step)
    /\ ActualHeaderSeen(step) = SpecHeaderSeen(step)
    /\ ActualDigestValid(step) = SpecDigestValid(step)

VoteCountersMatchSpec ==
  step = "none" \/
    /\ ActualPrepareVotes(step) = SpecPrepareVotes(step)
    /\ ActualCommitVotes(step) = SpecCommitVotes(step)
    /\ ActualStakeSigned(step) = SpecStakeSigned(step)

CommitEvidenceMatchesSpec ==
  step = "none" \/
    /\ ActualCommitted(step) = SpecCommitted(step)
    /\ ActualCommitEvidenceVotes(step) = SpecCommitEvidenceVotes(step)
    /\ ActualCommitEvidenceStake(step) = SpecCommitEvidenceStake(step)

BufferedCommitWaitHasNoCertificate ==
  step = "none" \/
    (step \in BufferedCommitCases =>
      /\ ActualPhase(step) = "CommitVote"
      /\ ActualCommitted(step) = FALSE
      /\ ActualCommitVotes(step) = 3
      /\ ActualStakeSigned(step) = 9
      /\ ActualRbcState(step) # "Delivered"
      /\ ActualCommitEvidenceVotes(step) = 0
      /\ ActualCommitEvidenceStake(step) = 0)

FinalityRequiresBufferedVotesAndDelivery ==
  step = "none" \/
    (ActualCommitted(step) =>
      /\ step = "deliver_final"
      /\ ActualPhase(step) = "Committed"
      /\ ActualRbcState(step) = "Delivered"
      /\ ActualChunkCount(step) = 2
      /\ ActualReadyVotes(step) = 3
      /\ ActualPrepareVotes(step) = 3
      /\ ActualCommitVotes(step) = 3
      /\ ActualStakeSigned(step) = 9
      /\ ActualCommitEvidenceVotes(step) = 3
      /\ ActualCommitEvidenceStake(step) = 9)

DirectVoteFirstCorridorExactness ==
  /\ PhaseMatchesSpec
  /\ RbcStateMatchesSpec
  /\ RbcEvidenceMatchesSpec
  /\ VoteCountersMatchSpec
  /\ CommitEvidenceMatchesSpec
  /\ BufferedCommitWaitHasNoCertificate
  /\ FinalityRequiresBufferedVotesAndDelivery

DirectVoteFirstCorridorCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ DirectVoteFirstCorridorExactness

DirectVoteFirstProgressSafetyEnvelope ==
  /\ TypeInvariant
  /\ DirectVoteFirstCorridorExactness

SafetyFast == DirectVoteFirstCorridorExactness

VoteFirstPathNextStep(c) ==
  CASE c = "none" -> "init"
    [] c = "init" -> "propose"
    [] c = "propose" -> "prepare_1"
    [] c = "prepare_1" -> "prepare_2"
    [] c = "prepare_2" -> "prepare_quorum"
    [] c = "prepare_quorum" -> "commit_1"
    [] c = "commit_1" -> "commit_2"
    [] c = "commit_2" -> "commit_buffered"
    [] c = "commit_buffered" -> "chunk_1"
    [] c = "chunk_1" -> "chunk_2"
    [] c = "chunk_2" -> "ready_1"
    [] c = "ready_1" -> "ready_2"
    [] c = "ready_2" -> "ready_quorum"
    [] OTHER -> "deliver_final"

VoteFirstPathAdvance ==
  /\ step # "deliver_final"
  /\ step' = VoteFirstPathNextStep(step)

VoteFirstPathTerminalStutter ==
  /\ step = "deliver_final"
  /\ UNCHANGED vars

VoteFirstPathNext ==
  \/ VoteFirstPathAdvance
  \/ VoteFirstPathTerminalStutter

DirectVoteFirstCorridorProgressFairness ==
  /\ WF_vars(VoteFirstPathAdvance)

DirectVoteFirstCorridorProgressSpec ==
  /\ Init
  /\ [][VoteFirstPathNext]_vars
  /\ DirectVoteFirstCorridorProgressFairness

EventualDirectVoteFirstCommit ==
  <>ActualCommitted(step)

DirectVoteFirstFinalityStack ==
  /\ step = "deliver_final"
  /\ ActualCommitted(step)
  /\ ActualPhase(step) = "Committed"
  /\ ActualRbcState(step) = "Delivered"
  /\ ActualChunkCount(step) = 2
  /\ ActualReadyVotes(step) = 3
  /\ ActualHeaderSeen(step)
  /\ ActualDigestValid(step)
  /\ ActualPrepareVotes(step) = 3
  /\ ActualCommitVotes(step) = 3
  /\ ActualStakeSigned(step) = 9
  /\ ActualCommitEvidenceVotes(step) = 3
  /\ ActualCommitEvidenceStake(step) = 9

EventualDirectVoteFirstFinalityStack ==
  <>DirectVoteFirstFinalityStack

====
