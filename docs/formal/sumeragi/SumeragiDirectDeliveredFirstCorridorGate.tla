---- MODULE SumeragiDirectDeliveredFirstCorridorGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the central delivered-first direct commit
corridor.

This slice captures the canonical no-fault path through the top-level Sumeragi
model: proposal seeds RBC, chunks complete, READY quorum delivers the payload,
prepare votes enter commit-vote phase, and honest commit votes install finality.
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
  "chunk_1",
  "chunk_2",
  "ready_1",
  "ready_2",
  "ready_quorum",
  "deliver_pending",
  "prepare_1",
  "prepare_2",
  "prepare_quorum",
  "commit_1",
  "commit_2",
  "commit_final"
}

Phases == {"Propose", "Prepare", "CommitVote", "Committed"}
RbcStates == {"Idle", "Init", "Chunking", "ChunksComplete", "ReadyPartial",
  "ReadyQuorum", "Delivered"}

DeliveredCases == {
  "deliver_pending",
  "prepare_1",
  "prepare_2",
  "prepare_quorum",
  "commit_1",
  "commit_2",
  "commit_final"
}

SpecPhase(c) ==
  CASE c = "init" -> "Propose"
    [] c \in {"prepare_quorum", "commit_1", "commit_2"} -> "CommitVote"
    [] c = "commit_final" -> "Committed"
    [] OTHER -> "Prepare"

SpecRbcState(c) ==
  CASE c = "init" -> "Idle"
    [] c = "propose" -> "Init"
    [] c = "chunk_1" -> "Chunking"
    [] c = "chunk_2" -> "ChunksComplete"
    [] c \in {"ready_1", "ready_2"} -> "ReadyPartial"
    [] c = "ready_quorum" -> "ReadyQuorum"
    [] OTHER -> "Delivered"

SpecChunkCount(c) ==
  CASE c \in {"init", "propose"} -> 0
    [] c = "chunk_1" -> 1
    [] OTHER -> 2

SpecReadyVotes(c) ==
  CASE c \in {"init", "propose", "chunk_1", "chunk_2"} -> 0
    [] c = "ready_1" -> 1
    [] c = "ready_2" -> 2
    [] OTHER -> 3

SpecPrepareVotes(c) ==
  CASE c \in {"prepare_1"} -> 1
    [] c \in {"prepare_2"} -> 2
    [] c \in {"prepare_quorum", "commit_1", "commit_2", "commit_final"} -> 3
    [] OTHER -> 0

SpecCommitVotes(c) ==
  CASE c = "commit_1" -> 1
    [] c = "commit_2" -> 2
    [] c = "commit_final" -> 3
    [] OTHER -> 0

SpecStakeSigned(c) == 3 * SpecCommitVotes(c)

SpecCommitted(c) == c = "commit_final"

SpecCommitEvidenceVotes(c) == IF SpecCommitted(c) THEN 3 ELSE 0

SpecCommitEvidenceStake(c) == IF SpecCommitted(c) THEN 9 ELSE 0

SpecHeaderSeen(c) == c # "init"

SpecDigestValid(c) == c # "init"

ActualPhase(c) ==
  IF Bug = "commit_without_committed_phase" /\ c = "commit_final"
  THEN "CommitVote"
  ELSE SpecPhase(c)

ActualRbcState(c) ==
  IF Bug = "skip_deliver_state" /\ c \in DeliveredCases
  THEN "ReadyQuorum"
  ELSE SpecRbcState(c)

ActualChunkCount(c) ==
  IF Bug = "drop_second_chunk" /\ c \in (Cases \ {"init", "propose", "chunk_1"})
  THEN 1
  ELSE SpecChunkCount(c)

ActualReadyVotes(c) ==
  IF Bug = "ready_quorum_under_counted" /\ c \in (Cases \ {"init", "propose", "chunk_1", "chunk_2", "ready_1", "ready_2"})
  THEN 2
  ELSE SpecReadyVotes(c)

ActualPrepareVotes(c) ==
  IF Bug = "prepare_quorum_under_counted"
     /\ c \in {"prepare_quorum", "commit_1", "commit_2", "commit_final"}
  THEN 2
  ELSE SpecPrepareVotes(c)

ActualCommitVotes(c) ==
  IF Bug = "commit_final_under_counted" /\ c = "commit_final"
  THEN 2
  ELSE SpecCommitVotes(c)

ActualStakeSigned(c) ==
  IF Bug = "stake_not_recorded" /\ c = "commit_final"
  THEN 6
  ELSE SpecStakeSigned(c)

ActualCommitted(c) ==
  IF Bug = "finality_not_latched" /\ c = "commit_final"
  THEN FALSE
  ELSE SpecCommitted(c)

ActualCommitEvidenceVotes(c) ==
  IF Bug = "commit_evidence_votes_missing" /\ c = "commit_final"
  THEN 0
  ELSE SpecCommitEvidenceVotes(c)

ActualCommitEvidenceStake(c) ==
  IF Bug = "commit_evidence_stake_missing" /\ c = "commit_final"
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

FinalityRequiresDeliveredQuorumAndStake ==
  step = "none" \/
    (ActualCommitted(step) =>
      /\ ActualPhase(step) = "Committed"
      /\ ActualRbcState(step) = "Delivered"
      /\ ActualReadyVotes(step) = 3
      /\ ActualPrepareVotes(step) = 3
      /\ ActualCommitVotes(step) = 3
      /\ ActualStakeSigned(step) = 9
      /\ ActualCommitEvidenceVotes(step) = 3
      /\ ActualCommitEvidenceStake(step) = 9)

DirectDeliveredFirstCorridorExactness ==
  /\ PhaseMatchesSpec
  /\ RbcStateMatchesSpec
  /\ RbcEvidenceMatchesSpec
  /\ VoteCountersMatchSpec
  /\ CommitEvidenceMatchesSpec
  /\ FinalityRequiresDeliveredQuorumAndStake

DirectDeliveredFirstCorridorCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ DirectDeliveredFirstCorridorExactness

DirectDeliveredFirstProgressSafetyEnvelope ==
  /\ TypeInvariant
  /\ DirectDeliveredFirstCorridorExactness

SafetyFast == DirectDeliveredFirstCorridorExactness

DeliveredFirstPathNextStep(c) ==
  CASE c = "none" -> "init"
    [] c = "init" -> "propose"
    [] c = "propose" -> "chunk_1"
    [] c = "chunk_1" -> "chunk_2"
    [] c = "chunk_2" -> "ready_1"
    [] c = "ready_1" -> "ready_2"
    [] c = "ready_2" -> "ready_quorum"
    [] c = "ready_quorum" -> "deliver_pending"
    [] c = "deliver_pending" -> "prepare_1"
    [] c = "prepare_1" -> "prepare_2"
    [] c = "prepare_2" -> "prepare_quorum"
    [] c = "prepare_quorum" -> "commit_1"
    [] c = "commit_1" -> "commit_2"
    [] OTHER -> "commit_final"

DeliveredFirstPathAdvance ==
  /\ step # "commit_final"
  /\ step' = DeliveredFirstPathNextStep(step)

DeliveredFirstPathTerminalStutter ==
  /\ step = "commit_final"
  /\ UNCHANGED vars

DeliveredFirstPathNext ==
  \/ DeliveredFirstPathAdvance
  \/ DeliveredFirstPathTerminalStutter

DirectDeliveredFirstCorridorProgressFairness ==
  /\ WF_vars(DeliveredFirstPathAdvance)

DirectDeliveredFirstCorridorProgressSpec ==
  /\ Init
  /\ [][DeliveredFirstPathNext]_vars
  /\ DirectDeliveredFirstCorridorProgressFairness

EventualDirectDeliveredFirstCommit ==
  <>ActualCommitted(step)

DirectDeliveredFirstFinalityStack ==
  /\ step = "commit_final"
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

EventualDirectDeliveredFirstFinalityStack ==
  <>DirectDeliveredFirstFinalityStack

====
