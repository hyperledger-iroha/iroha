---- MODULE SumeragiDirectCommitInterleavingGate ----
EXTENDS Naturals

(***************************************************************************
A finite no-fault direct commit interleaving model.

This slice strengthens the separate delivered-first and vote-first corridor
models by allowing prepare votes, commit votes, RBC chunks, READY votes, and RBC
delivery to interleave freely. Finality may be installed by the last commit
vote after prior delivery, or by RBC delivery after buffered vote/stake quorum.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  phase,
  \* @type: Int;
  prepareVotes,
  \* @type: Int;
  commitVotes,
  \* @type: Int;
  stakeSigned,
  \* @type: Int;
  commitEvidenceVotes,
  \* @type: Int;
  commitEvidenceStake,
  \* @type: Str;
  rbcState,
  \* @type: Int;
  chunkCount,
  \* @type: Int;
  readyVotes,
  \* @type: Bool;
  headerSeen,
  \* @type: Bool;
  digestValid,
  \* @type: Bool;
  committed

\* @type: <<Str, Int, Int, Int, Int, Int, Str, Int, Int, Bool, Bool, Bool>>;
vars == <<
  phase,
  prepareVotes,
  commitVotes,
  stakeSigned,
  commitEvidenceVotes,
  commitEvidenceStake,
  rbcState,
  chunkCount,
  readyVotes,
  headerSeen,
  digestValid,
  committed
>>

CommitQuorum == 3
StakeQuorum == 9
MaxChunks == 2
StakePerVote == 3

Phases == {"Propose", "Prepare", "CommitVote", "Committed"}
RbcStates == {"Idle", "Init", "Chunking", "ChunksComplete", "ReadyPartial",
  "ReadyQuorum", "Delivered"}

CanCommit(votes, stake, state) ==
  /\ votes >= CommitQuorum
  /\ stake >= StakeQuorum
  /\ state = "Delivered"

Init ==
  /\ phase = "Propose"
  /\ prepareVotes = 0
  /\ commitVotes = 0
  /\ stakeSigned = 0
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceStake = 0
  /\ rbcState = "Idle"
  /\ chunkCount = 0
  /\ readyVotes = 0
  /\ headerSeen = FALSE
  /\ digestValid = FALSE
  /\ committed = FALSE

HonestPropose ==
  /\ phase = "Propose"
  /\ phase' = "Prepare"
  /\ prepareVotes' = 0
  /\ rbcState' = IF Bug = "propose_skips_rbc" THEN "Idle" ELSE "Init"
  /\ headerSeen' = IF Bug = "header_not_seeded" THEN FALSE ELSE TRUE
  /\ digestValid' = IF Bug = "digest_not_seeded" THEN FALSE ELSE TRUE
  /\ chunkCount' = 0
  /\ readyVotes' = 0
  /\ UNCHANGED <<
      commitVotes,
      stakeSigned,
      commitEvidenceVotes,
      commitEvidenceStake,
      committed
     >>

PrepareVote ==
  /\ phase = "Prepare"
  /\ prepareVotes < CommitQuorum
  /\ LET nextPrepare == prepareVotes + 1
         actualPrepare ==
           IF Bug = "prepare_quorum_under_counted"
              /\ nextPrepare >= CommitQuorum
           THEN prepareVotes
           ELSE nextPrepare
     IN
       /\ prepareVotes' = actualPrepare
       /\ phase' =
            IF nextPrepare >= CommitQuorum THEN "CommitVote" ELSE "Prepare"
  /\ UNCHANGED <<
      commitVotes,
      stakeSigned,
      commitEvidenceVotes,
      commitEvidenceStake,
      rbcState,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid,
      committed
     >>

CommitVote ==
  /\ phase = "CommitVote"
  /\ commitVotes < CommitQuorum
  /\ LET nextVotes == commitVotes + 1
         actualVotes ==
           IF Bug = "commit_quorum_under_counted"
              /\ nextVotes >= CommitQuorum
           THEN commitVotes
           ELSE nextVotes
         nextStake == stakeSigned + StakePerVote
         actualStake ==
           IF Bug = "stake_not_recorded" /\ nextVotes >= CommitQuorum
           THEN stakeSigned
           ELSE nextStake
         finalFromVote ==
           \/ CanCommit(actualVotes, actualStake, rbcState)
           \/ /\ Bug = "commit_before_delivery"
              /\ actualVotes >= CommitQuorum
              /\ actualStake >= StakeQuorum
         prematureEvidence ==
           /\ Bug = "commit_evidence_before_delivery"
           /\ actualVotes >= CommitQuorum
           /\ actualStake >= StakeQuorum
           /\ rbcState # "Delivered"
     IN
       /\ commitVotes' = actualVotes
       /\ stakeSigned' = actualStake
       /\ phase' =
            IF finalFromVote
            THEN IF Bug = "phase_not_committed" THEN "CommitVote" ELSE "Committed"
            ELSE "CommitVote"
       /\ committed' =
            IF finalFromVote
            THEN IF Bug = "finality_not_latched" THEN FALSE ELSE TRUE
            ELSE FALSE
       /\ commitEvidenceVotes' =
            IF finalFromVote
            THEN IF Bug = "commit_evidence_votes_missing" THEN 0 ELSE actualVotes
            ELSE IF prematureEvidence THEN actualVotes ELSE commitEvidenceVotes
       /\ commitEvidenceStake' =
            IF finalFromVote
            THEN IF Bug = "commit_evidence_stake_missing" THEN 0 ELSE actualStake
            ELSE IF prematureEvidence THEN actualStake ELSE commitEvidenceStake
  /\ UNCHANGED <<
      prepareVotes,
      rbcState,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid
     >>

RbcChunk ==
  /\ rbcState \in {"Init", "Chunking"}
  /\ headerSeen
  /\ digestValid
  /\ chunkCount < MaxChunks
  /\ LET nextChunk == chunkCount + 1
         actualChunk ==
           IF Bug = "drop_second_chunk" /\ nextChunk >= MaxChunks
           THEN chunkCount
           ELSE nextChunk
     IN
       /\ chunkCount' = actualChunk
       /\ rbcState' =
            IF nextChunk >= MaxChunks THEN "ChunksComplete" ELSE "Chunking"
  /\ UNCHANGED <<
      phase,
      prepareVotes,
      commitVotes,
      stakeSigned,
      commitEvidenceVotes,
      commitEvidenceStake,
      readyVotes,
      headerSeen,
      digestValid,
      committed
     >>

RbcReady ==
  /\ rbcState \in {"ChunksComplete", "ReadyPartial"}
  /\ headerSeen
  /\ digestValid
  /\ chunkCount >= MaxChunks
  /\ readyVotes < CommitQuorum
  /\ LET nextReady == readyVotes + 1
         actualReady ==
           IF Bug = "ready_quorum_under_counted"
              /\ nextReady >= CommitQuorum
           THEN readyVotes
           ELSE nextReady
     IN
       /\ readyVotes' = actualReady
       /\ rbcState' =
            IF nextReady >= CommitQuorum THEN "ReadyQuorum" ELSE "ReadyPartial"
  /\ UNCHANGED <<
      phase,
      prepareVotes,
      commitVotes,
      stakeSigned,
      commitEvidenceVotes,
      commitEvidenceStake,
      chunkCount,
      headerSeen,
      digestValid,
      committed
     >>

RbcDeliver ==
  /\ rbcState = "ReadyQuorum"
  /\ headerSeen
  /\ digestValid
  /\ chunkCount >= MaxChunks
  /\ readyVotes >= CommitQuorum
  /\ LET finalFromDelivery == CanCommit(commitVotes, stakeSigned, "Delivered")
     IN
       /\ rbcState' =
            IF Bug = "skip_deliver_state" THEN "ReadyQuorum" ELSE "Delivered"
       /\ phase' =
            IF finalFromDelivery
            THEN IF Bug = "phase_not_committed" THEN "CommitVote" ELSE "Committed"
            ELSE phase
       /\ committed' =
            IF finalFromDelivery
            THEN IF Bug = "finality_not_latched" THEN FALSE ELSE TRUE
            ELSE FALSE
       /\ commitEvidenceVotes' =
            IF finalFromDelivery
            THEN IF Bug = "commit_evidence_votes_missing" THEN 0 ELSE commitVotes
            ELSE commitEvidenceVotes
       /\ commitEvidenceStake' =
            IF finalFromDelivery
            THEN IF Bug = "commit_evidence_stake_missing" THEN 0 ELSE stakeSigned
            ELSE commitEvidenceStake
  /\ UNCHANGED <<
      prepareVotes,
      commitVotes,
      stakeSigned,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid
     >>

TerminalStutter ==
  /\ committed
  /\ UNCHANGED vars

Next ==
  \/ HonestPropose
  \/ PrepareVote
  \/ CommitVote
  \/ RbcChunk
  \/ RbcReady
  \/ RbcDeliver
  \/ TerminalStutter

TypeInvariant ==
  /\ phase \in Phases
  /\ prepareVotes \in 0..CommitQuorum
  /\ commitVotes \in 0..CommitQuorum
  /\ stakeSigned \in 0..StakeQuorum
  /\ commitEvidenceVotes \in 0..CommitQuorum
  /\ commitEvidenceStake \in 0..StakeQuorum
  /\ rbcState \in RbcStates
  /\ chunkCount \in 0..MaxChunks
  /\ readyVotes \in 0..CommitQuorum
  /\ headerSeen \in BOOLEAN
  /\ digestValid \in BOOLEAN
  /\ committed \in BOOLEAN

RbcEvidenceShape ==
  /\ (rbcState = "Idle" =>
        /\ chunkCount = 0
        /\ readyVotes = 0
        /\ ~headerSeen
        /\ ~digestValid)
  /\ (rbcState = "Init" =>
        /\ chunkCount = 0
        /\ readyVotes = 0
        /\ headerSeen
        /\ digestValid)
  /\ (rbcState = "Chunking" =>
        /\ chunkCount = 1
        /\ readyVotes = 0
        /\ headerSeen
        /\ digestValid)
  /\ (rbcState = "ChunksComplete" =>
        /\ chunkCount = MaxChunks
        /\ readyVotes = 0
        /\ headerSeen
        /\ digestValid)
  /\ (rbcState = "ReadyPartial" =>
        /\ chunkCount = MaxChunks
        /\ readyVotes \in 1..(CommitQuorum - 1)
        /\ headerSeen
        /\ digestValid)
  /\ (rbcState = "ReadyQuorum" =>
        /\ chunkCount = MaxChunks
        /\ readyVotes = CommitQuorum
        /\ headerSeen
        /\ digestValid)
  /\ (rbcState = "Delivered" =>
        /\ chunkCount = MaxChunks
        /\ readyVotes = CommitQuorum
        /\ headerSeen
        /\ digestValid)

VoteHandoffShape ==
  /\ (phase = "Propose" =>
        /\ prepareVotes = 0
        /\ commitVotes = 0
        /\ stakeSigned = 0)
  /\ (phase = "Prepare" =>
        /\ prepareVotes < CommitQuorum
        /\ commitVotes = 0
        /\ stakeSigned = 0)
  /\ (phase \in {"CommitVote", "Committed"} =>
        /\ prepareVotes = CommitQuorum
        /\ stakeSigned = StakePerVote * commitVotes)

CommitCertificateShape ==
  /\ (committed <=>
        /\ phase = "Committed"
        /\ prepareVotes = CommitQuorum
        /\ commitVotes = CommitQuorum
        /\ stakeSigned = StakeQuorum
        /\ rbcState = "Delivered"
        /\ commitEvidenceVotes = CommitQuorum
        /\ commitEvidenceStake = StakeQuorum)
  /\ (~committed =>
        /\ commitEvidenceVotes = 0
        /\ commitEvidenceStake = 0)

BufferedVotesWaitForDelivery ==
  /\ (commitVotes = CommitQuorum /\ stakeSigned = StakeQuorum /\ ~committed) =>
       /\ phase = "CommitVote"
       /\ rbcState # "Delivered"
       /\ commitEvidenceVotes = 0
       /\ commitEvidenceStake = 0

DeliveredWithBufferedVotesCommits ==
  (rbcState = "Delivered" /\ commitVotes = CommitQuorum /\ stakeSigned = StakeQuorum) =>
    committed

DirectCommitInterleavingExactness ==
  /\ RbcEvidenceShape
  /\ VoteHandoffShape
  /\ CommitCertificateShape
  /\ BufferedVotesWaitForDelivery
  /\ DeliveredWithBufferedVotesCommits

DirectCommitInterleavingCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ DirectCommitInterleavingExactness

SafetyFast == DirectCommitInterleavingExactness

DirectCommitProgressSafetyEnvelope ==
  /\ TypeInvariant
  /\ DirectCommitInterleavingExactness

DirectCommitInterleavingProgressFairness ==
  /\ WF_vars(HonestPropose)
  /\ WF_vars(PrepareVote)
  /\ WF_vars(CommitVote)
  /\ WF_vars(RbcChunk)
  /\ WF_vars(RbcReady)
  /\ WF_vars(RbcDeliver)

DirectCommitInterleavingProgressSpec ==
  /\ Init
  /\ [][Next]_vars
  /\ DirectCommitInterleavingProgressFairness

EventualDirectCommit ==
  <>committed

DirectCommitFinalityStack ==
  /\ committed
  /\ phase = "Committed"
  /\ prepareVotes = CommitQuorum
  /\ commitVotes = CommitQuorum
  /\ stakeSigned = StakeQuorum
  /\ rbcState = "Delivered"
  /\ readyVotes = CommitQuorum
  /\ chunkCount = MaxChunks
  /\ headerSeen
  /\ digestValid
  /\ commitEvidenceVotes = CommitQuorum
  /\ commitEvidenceStake = StakeQuorum

EventualDirectCommitFinalityStack ==
  <>DirectCommitFinalityStack

====
