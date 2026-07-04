---- MODULE SumeragiByzantineCommitInterleavingGate ----
EXTENDS Naturals

(***************************************************************************
A finite Byzantine-tolerant direct commit interleaving model.

This slice extends the no-fault direct commit interleaving gate with one
Byzantine commit voter. It keeps RBC delivery, commit votes, and commit
certificate installation freely interleavable while requiring finality to carry
the same honest-support threshold as the production Sumeragi proof corridor.
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
  commitVotesHonest,
  \* @type: Int;
  commitVotesByz,
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

\* @type: <<Str, Int, Int, Int, Int, Int, Int, Str, Int, Int, Bool, Bool, Bool>>;
vars == <<
  phase,
  prepareVotes,
  commitVotesHonest,
  commitVotesByz,
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
F == 1
MaxHonestVotes == 3
MaxByzVotes == F
HonestSupportThreshold == CommitQuorum - F
StakeQuorum == 9
MaxChunks == 2
StakePerHonestVote == 3
StakePerByzVote == 3
MaxStake == (MaxHonestVotes * StakePerHonestVote) +
  (MaxByzVotes * StakePerByzVote)
MaxEvidenceVotes == MaxHonestVotes + MaxByzVotes

Phases == {"Propose", "Prepare", "CommitVote", "Committed"}
RbcStates == {"Idle", "Init", "Chunking", "ChunksComplete", "ReadyPartial",
  "ReadyQuorum", "Delivered"}

StakeFor(h, b) ==
  (h * StakePerHonestVote) + (b * StakePerByzVote)

BufferedCommitEvidence(h, b, stake) ==
  /\ h + b >= CommitQuorum
  /\ h >= HonestSupportThreshold
  /\ b <= MaxByzVotes
  /\ stake >= StakeQuorum

CanCommitWithHonestSupport(h, b, stake, state) ==
  /\ BufferedCommitEvidence(h, b, stake)
  /\ state = "Delivered"

Init ==
  /\ phase = "Propose"
  /\ prepareVotes = 0
  /\ commitVotesHonest = 0
  /\ commitVotesByz = 0
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
      commitVotesHonest,
      commitVotesByz,
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
      commitVotesHonest,
      commitVotesByz,
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

HonestCommitVote ==
  /\ phase = "CommitVote"
  /\ commitVotesHonest < MaxHonestVotes
  /\ LET nextHonest == commitVotesHonest + 1
         nextStake == stakeSigned + StakePerHonestVote
         actualStake ==
           IF Bug = "honest_stake_not_recorded"
           THEN stakeSigned
           ELSE nextStake
         finalFromVote ==
           \/ CanCommitWithHonestSupport(
                nextHonest,
                commitVotesByz,
                actualStake,
                rbcState
              )
           \/ /\ Bug = "commit_before_delivery"
              /\ BufferedCommitEvidence(
                   nextHonest,
                   commitVotesByz,
                   actualStake
                 )
           \/ /\ Bug = "commit_without_honest_support"
              /\ rbcState = "Delivered"
              /\ nextHonest = 1
              /\ commitVotesByz = MaxByzVotes
         prematureEvidence ==
           /\ Bug = "commit_evidence_before_delivery"
           /\ BufferedCommitEvidence(nextHonest, commitVotesByz, actualStake)
           /\ rbcState # "Delivered"
     IN
       /\ commitVotesHonest' = nextHonest
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
            THEN
              IF Bug = "commit_evidence_votes_missing"
              THEN 0
              ELSE nextHonest + commitVotesByz
            ELSE
              IF prematureEvidence
              THEN nextHonest + commitVotesByz
              ELSE commitEvidenceVotes
       /\ commitEvidenceStake' =
            IF finalFromVote
            THEN
              IF Bug = "commit_evidence_stake_missing"
              THEN 0
              ELSE actualStake
            ELSE
              IF prematureEvidence THEN actualStake ELSE commitEvidenceStake
  /\ UNCHANGED <<
      prepareVotes,
      commitVotesByz,
      rbcState,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid
     >>

ByzantineCommitVote ==
  /\ phase = "CommitVote"
  /\ \/ commitVotesByz < MaxByzVotes
     \/ /\ Bug = "byzantine_vote_over_budget"
        /\ commitVotesByz <= MaxByzVotes
  /\ LET nextByz == commitVotesByz + 1
         nextStake == stakeSigned + StakePerByzVote
         actualStake ==
           IF Bug = "byzantine_stake_over_counted"
           THEN stakeSigned + StakePerByzVote + StakePerByzVote
           ELSE nextStake
         finalFromVote ==
           \/ CanCommitWithHonestSupport(
                commitVotesHonest,
                nextByz,
                actualStake,
                rbcState
              )
           \/ /\ Bug = "commit_before_delivery"
              /\ BufferedCommitEvidence(
                   commitVotesHonest,
                   nextByz,
                   actualStake
                 )
           \/ /\ Bug = "commit_without_honest_support"
              /\ rbcState = "Delivered"
              /\ commitVotesHonest = 1
              /\ nextByz = MaxByzVotes
         prematureEvidence ==
           /\ Bug = "commit_evidence_before_delivery"
           /\ BufferedCommitEvidence(commitVotesHonest, nextByz, actualStake)
           /\ rbcState # "Delivered"
     IN
       /\ commitVotesByz' = nextByz
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
            THEN
              IF Bug = "commit_evidence_votes_missing"
              THEN 0
              ELSE commitVotesHonest + nextByz
            ELSE
              IF prematureEvidence
              THEN commitVotesHonest + nextByz
              ELSE commitEvidenceVotes
       /\ commitEvidenceStake' =
            IF finalFromVote
            THEN
              IF Bug = "commit_evidence_stake_missing"
              THEN 0
              ELSE actualStake
            ELSE
              IF prematureEvidence THEN actualStake ELSE commitEvidenceStake
  /\ UNCHANGED <<
      prepareVotes,
      commitVotesHonest,
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
      commitVotesHonest,
      commitVotesByz,
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
      commitVotesHonest,
      commitVotesByz,
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
  /\ LET finalFromDelivery ==
           \/ CanCommitWithHonestSupport(
                commitVotesHonest,
                commitVotesByz,
                stakeSigned,
                "Delivered"
              )
           \/ /\ Bug = "commit_without_honest_support"
              /\ commitVotesHonest = 1
              /\ commitVotesByz = MaxByzVotes
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
            THEN
              IF Bug = "commit_evidence_votes_missing"
              THEN 0
              ELSE commitVotesHonest + commitVotesByz
            ELSE commitEvidenceVotes
       /\ commitEvidenceStake' =
            IF finalFromDelivery
            THEN
              IF Bug = "commit_evidence_stake_missing"
              THEN 0
              ELSE stakeSigned
            ELSE commitEvidenceStake
  /\ UNCHANGED <<
      prepareVotes,
      commitVotesHonest,
      commitVotesByz,
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
  \/ HonestCommitVote
  \/ ByzantineCommitVote
  \/ RbcChunk
  \/ RbcReady
  \/ RbcDeliver
  \/ TerminalStutter

TypeInvariant ==
  /\ phase \in Phases
  /\ prepareVotes \in 0..CommitQuorum
  /\ commitVotesHonest \in 0..MaxHonestVotes
  /\ commitVotesByz \in 0..MaxByzVotes
  /\ stakeSigned \in 0..MaxStake
  /\ commitEvidenceVotes \in 0..MaxEvidenceVotes
  /\ commitEvidenceStake \in 0..MaxStake
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

ProposedRoundInitializesRbc ==
  phase = "Propose" \/
    /\ rbcState # "Idle"
    /\ headerSeen
    /\ digestValid

VoteHandoffShape ==
  /\ (phase = "Propose" =>
        /\ prepareVotes = 0
        /\ commitVotesHonest = 0
        /\ commitVotesByz = 0
        /\ stakeSigned = 0)
  /\ (phase = "Prepare" =>
        /\ prepareVotes < CommitQuorum
        /\ commitVotesHonest = 0
        /\ commitVotesByz = 0
        /\ stakeSigned = 0)
  /\ (phase \in {"CommitVote", "Committed"} =>
        /\ prepareVotes = CommitQuorum
        /\ commitVotesHonest <= MaxHonestVotes
        /\ commitVotesByz <= MaxByzVotes
        /\ stakeSigned = StakeFor(commitVotesHonest, commitVotesByz))

CommitCertificateShape ==
  /\ (committed <=>
        /\ phase = "Committed"
        /\ prepareVotes = CommitQuorum
        /\ commitVotesHonest + commitVotesByz >= CommitQuorum
        /\ commitVotesHonest >= HonestSupportThreshold
        /\ commitVotesByz <= MaxByzVotes
        /\ stakeSigned = StakeFor(commitVotesHonest, commitVotesByz)
        /\ stakeSigned >= StakeQuorum
        /\ rbcState = "Delivered"
        /\ commitEvidenceVotes = commitVotesHonest + commitVotesByz
        /\ commitEvidenceStake = stakeSigned
        /\ commitEvidenceVotes >= CommitQuorum
        /\ commitEvidenceStake >= StakeQuorum)
  /\ (~committed =>
        /\ commitEvidenceVotes = 0
        /\ commitEvidenceStake = 0)

BufferedVotesWaitForDelivery ==
  /\ (BufferedCommitEvidence(
        commitVotesHonest,
        commitVotesByz,
        stakeSigned
      ) /\ ~committed /\ rbcState # "Delivered") =>
       /\ phase = "CommitVote"
       /\ commitEvidenceVotes = 0
       /\ commitEvidenceStake = 0

DeliveredWithBufferedVotesCommits ==
  (rbcState = "Delivered" /\
    BufferedCommitEvidence(
      commitVotesHonest,
      commitVotesByz,
      stakeSigned
    )) => committed

ByzantineCommitInterleavingExactness ==
  /\ RbcEvidenceShape
  /\ ProposedRoundInitializesRbc
  /\ VoteHandoffShape
  /\ CommitCertificateShape
  /\ BufferedVotesWaitForDelivery
  /\ DeliveredWithBufferedVotesCommits

ByzantineCommitInterleavingCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ByzantineCommitInterleavingExactness

SafetyFast == ByzantineCommitInterleavingExactness

ByzantineCommitProgressSafetyEnvelope ==
  /\ TypeInvariant
  /\ ByzantineCommitInterleavingExactness

ByzantineCommitInterleavingProgressFairness ==
  /\ WF_vars(HonestPropose)
  /\ WF_vars(PrepareVote)
  /\ WF_vars(HonestCommitVote)
  /\ WF_vars(ByzantineCommitVote)
  /\ WF_vars(RbcChunk)
  /\ WF_vars(RbcReady)
  /\ WF_vars(RbcDeliver)

ByzantineCommitInterleavingProgressSpec ==
  /\ Init
  /\ [][Next]_vars
  /\ ByzantineCommitInterleavingProgressFairness

EventualByzantineCommit ==
  <>committed

ByzantineCommitFinalityStack ==
  /\ committed
  /\ phase = "Committed"
  /\ prepareVotes = CommitQuorum
  /\ commitVotesHonest + commitVotesByz >= CommitQuorum
  /\ commitVotesHonest >= HonestSupportThreshold
  /\ commitVotesByz <= MaxByzVotes
  /\ stakeSigned = StakeFor(commitVotesHonest, commitVotesByz)
  /\ stakeSigned >= StakeQuorum
  /\ rbcState = "Delivered"
  /\ commitEvidenceVotes = commitVotesHonest + commitVotesByz
  /\ commitEvidenceStake = stakeSigned
  /\ commitEvidenceVotes >= CommitQuorum
  /\ commitEvidenceStake >= StakeQuorum

EventualByzantineCommitFinalityStack ==
  <>ByzantineCommitFinalityStack

====
