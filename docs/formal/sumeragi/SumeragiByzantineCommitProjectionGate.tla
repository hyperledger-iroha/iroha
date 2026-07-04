---- MODULE SumeragiByzantineCommitProjectionGate ----
EXTENDS Naturals

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

Interleaving == INSTANCE SumeragiByzantineCommitInterleavingGate

(***************************************************************************
Projection bridge from the finite Byzantine interleaving gate to the
top-level Sumeragi direct-commit corridor.

The imported interleaving gate is the tractable state-space model.  This
module restates the central top-level Byzantine direct-commit obligations over
that compact state, with the view/new-view fields projected to the direct
commit corridor constants:

  view = commitView = newViewVotes = viewEvidenceVotes = 0.
***************************************************************************)

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

Init == Interleaving!Init
Next == Interleaving!Next
TypeInvariant == Interleaving!TypeInvariant
HonestPropose == Interleaving!HonestPropose
PrepareVote == Interleaving!PrepareVote
HonestCommitVote == Interleaving!HonestCommitVote
ByzantineCommitVote == Interleaving!ByzantineCommitVote
RbcChunk == Interleaving!RbcChunk
RbcReady == Interleaving!RbcReady
RbcDeliver == Interleaving!RbcDeliver

CommitQuorum == Interleaving!CommitQuorum
F == Interleaving!F
MaxHonestVotes == Interleaving!MaxHonestVotes
MaxByzVotes == Interleaving!MaxByzVotes
HonestSupportThreshold == Interleaving!HonestSupportThreshold
StakeQuorum == Interleaving!StakeQuorum
MaxChunks == Interleaving!MaxChunks
StakePerHonestVote == Interleaving!StakePerHonestVote
StakePerByzVote == Interleaving!StakePerByzVote
RbcStates == Interleaving!RbcStates

RbcEvidenceShape == Interleaving!RbcEvidenceShape
ProposedRoundInitializesRbc == Interleaving!ProposedRoundInitializesRbc
VoteHandoffShape == Interleaving!VoteHandoffShape
CommitCertificateShape == Interleaving!CommitCertificateShape
BufferedVotesWaitForDelivery == Interleaving!BufferedVotesWaitForDelivery
DeliveredWithBufferedVotesCommits == Interleaving!DeliveredWithBufferedVotesCommits

ProjectedN == MaxHonestVotes + MaxByzVotes
ProjectedCommitView == 0
ProjectedNewViewVotes == 0
ProjectedViewEvidenceVotes == 0

ProjectedCanCommit(h, b, stake, state) ==
  /\ h + b >= CommitQuorum
  /\ stake >= StakeQuorum
  /\ state = "Delivered"

ProjectedByzantineCommitVoteEnabled ==
  /\ phase = "CommitVote"
  /\ commitVotesByz < F

ProjectedRbcInitEnabled ==
  rbcState = "Idle"

ProjectedRbcChunkGoodEnabled ==
  /\ rbcState \in {"Init", "Chunking"}
  /\ headerSeen

ProjectedRbcReadyGoodEnabled ==
  /\ rbcState \in {"ChunksComplete", "ReadyPartial", "ReadyQuorum"}
  /\ headerSeen
  /\ digestValid
  /\ readyVotes < ProjectedN

ProjectedRbcDeliverGoodEnabled ==
  /\ rbcState = "ReadyQuorum"
  /\ readyVotes >= CommitQuorum
  /\ headerSeen
  /\ digestValid

ProjectedLiveCommitGateCanCommitState ==
  /\ commitVotesHonest + commitVotesByz >= CommitQuorum
  /\ stakeSigned >= StakeQuorum
  /\ rbcState = "Delivered"

ProjectedTlcByzantineDirectCommitCorridor ==
  /\ phase \in {"Propose", "Prepare", "CommitVote", "Committed"}
  /\ rbcState \in RbcStates
  /\ ProjectedCommitView = 0
  /\ ProjectedNewViewVotes = 0
  /\ ProjectedViewEvidenceVotes = 0

ProjectedCommitImpliesHonestSupport ==
  committed => commitVotesHonest >= HonestSupportThreshold

ProjectedFinalityCertificateStackPresent ==
  /\ phase = "Committed"
  /\ ProjectedNewViewVotes = 0
  /\ prepareVotes >= CommitQuorum
  /\ commitVotesHonest + commitVotesByz >= CommitQuorum
  /\ commitVotesHonest >= HonestSupportThreshold
  /\ stakeSigned >= StakeQuorum
  /\ commitEvidenceVotes = commitVotesHonest + commitVotesByz
  /\ commitEvidenceStake = stakeSigned
  /\ commitEvidenceVotes >= CommitQuorum
  /\ commitEvidenceStake >= StakeQuorum
  /\ rbcState = "Delivered"
  /\ readyVotes >= CommitQuorum
  /\ chunkCount >= MaxChunks
  /\ headerSeen
  /\ digestValid

ProjectedFinalityCertificateStackComplete ==
  committed => ProjectedFinalityCertificateStackPresent

ProjectedCommitDisablesByzantineCommitVote ==
  committed => ~ProjectedByzantineCommitVoteEnabled

ProjectedByzantineCommitVoteGateMatchesPrepareEvidence ==
  ProjectedByzantineCommitVoteEnabled <=>
    /\ phase = "CommitVote"
    /\ prepareVotes >= CommitQuorum
    /\ commitVotesHonest <= MaxHonestVotes
    /\ commitVotesByz < F
    /\ ProjectedNewViewVotes = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ ProjectedCommitView = 0

ProjectedByzantineCommitVoteFinalityGateMatchesNextEvidence ==
  (ProjectedByzantineCommitVoteEnabled /\
    ProjectedCanCommit(
      commitVotesHonest,
      commitVotesByz + 1,
      stakeSigned + StakePerByzVote,
      rbcState
    )) <=>
    /\ phase = "CommitVote"
    /\ prepareVotes >= CommitQuorum
    /\ commitVotesHonest <= MaxHonestVotes
    /\ commitVotesByz < F
    /\ commitVotesHonest + commitVotesByz + 1 >= CommitQuorum
    /\ commitVotesHonest >= HonestSupportThreshold
    /\ stakeSigned + StakePerByzVote >= StakeQuorum
    /\ rbcState = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid
    /\ ProjectedNewViewVotes = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ ProjectedCommitView = 0

ProjectedByzantineCommitVotePendingGateMatchesMissingNextEvidence ==
  (ProjectedByzantineCommitVoteEnabled /\
    ~ProjectedCanCommit(
      commitVotesHonest,
      commitVotesByz + 1,
      stakeSigned + StakePerByzVote,
      rbcState
    )) <=>
    /\ phase = "CommitVote"
    /\ prepareVotes >= CommitQuorum
    /\ commitVotesHonest <= MaxHonestVotes
    /\ commitVotesByz < F
    /\ ProjectedNewViewVotes = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ ProjectedCommitView = 0
    /\ \/ commitVotesHonest + commitVotesByz + 1 < CommitQuorum
       \/ stakeSigned + StakePerByzVote < StakeQuorum
       \/ rbcState # "Delivered"

ProjectedRbcDeliverFinalityGateMatchesBufferedCommitEvidence ==
  (ProjectedRbcDeliverGoodEnabled /\
    ProjectedCanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) <=>
    /\ rbcState = "ReadyQuorum"
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid
    /\ phase = "CommitVote"
    /\ prepareVotes >= CommitQuorum
    /\ commitVotesHonest + commitVotesByz >= CommitQuorum
    /\ commitVotesHonest >= HonestSupportThreshold
    /\ commitVotesHonest <= MaxHonestVotes
    /\ commitVotesByz <= F
    /\ stakeSigned >= StakeQuorum
    /\ ProjectedNewViewVotes = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ ProjectedCommitView = 0

ProjectedRbcDeliverPendingGateMatchesMissingBufferedCommitEvidence ==
  (ProjectedRbcDeliverGoodEnabled /\
    ~ProjectedCanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) <=>
    /\ rbcState = "ReadyQuorum"
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ ProjectedCommitView = 0
    /\ \/ commitVotesHonest + commitVotesByz < CommitQuorum
       \/ stakeSigned < StakeQuorum

ProjectedCommitEvidenceMatchesVoteCounters ==
  committed =>
    /\ commitEvidenceVotes = commitVotesHonest + commitVotesByz
    /\ commitEvidenceStake = stakeSigned

ProjectedVoteCountersRespectRosterBudgets ==
  /\ prepareVotes \in 0..MaxHonestVotes
  /\ commitVotesHonest \in 0..MaxHonestVotes
  /\ commitVotesByz \in 0..F
  /\ ProjectedNewViewVotes \in 0..MaxHonestVotes
  /\ ProjectedViewEvidenceVotes \in 0..MaxHonestVotes

ProjectedStakeSignedMatchesVoteCounters ==
  stakeSigned =
    (commitVotesHonest * StakePerHonestVote) +
    (commitVotesByz * StakePerByzVote)

ProjectedNoCommitEvidenceBeforeCommit ==
  ~committed =>
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0

ProjectedRbcDeliveredDisablesRbcProgress ==
  rbcState = "Delivered" =>
    /\ ~ProjectedRbcInitEnabled
    /\ ~ProjectedRbcChunkGoodEnabled
    /\ ~ProjectedRbcReadyGoodEnabled
    /\ ~ProjectedRbcDeliverGoodEnabled

ProjectedRbcDeliveredWithoutFinalityHasNoCommitCertificate ==
  (/\ rbcState = "Delivered"
   /\ ~committed) =>
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ ProjectedCommitView = 0
    /\ \/ commitVotesHonest + commitVotesByz < CommitQuorum
       \/ stakeSigned < StakeQuorum

ProjectedRbcDeliveredWithoutFinalityWaitsForCommitEvidence ==
  (/\ rbcState = "Delivered"
   /\ ~committed) =>
    /\ ProjectedRbcDeliveredWithoutFinalityHasNoCommitCertificate
    /\ ProjectedRbcDeliveredDisablesRbcProgress
    /\ ~ProjectedLiveCommitGateCanCommitState
    /\ phase # "Committed"
    /\ (phase = "CommitVote" => prepareVotes >= CommitQuorum)

ProjectedByzantineDeliveredFirstTopExactness ==
  /\ ProjectedTlcByzantineDirectCommitCorridor
  /\ ProjectedCommitImpliesHonestSupport
  /\ ProjectedFinalityCertificateStackComplete
  /\ ProjectedCommitDisablesByzantineCommitVote
  /\ ProjectedByzantineCommitVoteGateMatchesPrepareEvidence
  /\ ProjectedByzantineCommitVoteFinalityGateMatchesNextEvidence
  /\ ProjectedByzantineCommitVotePendingGateMatchesMissingNextEvidence
  /\ ProjectedRbcDeliverFinalityGateMatchesBufferedCommitEvidence
  /\ ProjectedRbcDeliverPendingGateMatchesMissingBufferedCommitEvidence
  /\ ProjectedCommitEvidenceMatchesVoteCounters
  /\ ProjectedVoteCountersRespectRosterBudgets
  /\ ProjectedStakeSignedMatchesVoteCounters
  /\ ProjectedNoCommitEvidenceBeforeCommit

ProjectedByzantineDeliveredFirstTopCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ProjectedByzantineDeliveredFirstTopExactness

ProjectedByzantineVoteFirstTopExactness ==
  /\ ProjectedTlcByzantineDirectCommitCorridor
  /\ ProjectedCommitImpliesHonestSupport
  /\ ProjectedFinalityCertificateStackComplete
  /\ ProjectedCommitDisablesByzantineCommitVote
  /\ ProjectedByzantineCommitVoteGateMatchesPrepareEvidence
  /\ ProjectedByzantineCommitVoteFinalityGateMatchesNextEvidence
  /\ ProjectedByzantineCommitVotePendingGateMatchesMissingNextEvidence
  /\ ProjectedRbcDeliverFinalityGateMatchesBufferedCommitEvidence
  /\ ProjectedRbcDeliverPendingGateMatchesMissingBufferedCommitEvidence
  /\ ProjectedCommitEvidenceMatchesVoteCounters
  /\ ProjectedVoteCountersRespectRosterBudgets
  /\ ProjectedStakeSignedMatchesVoteCounters
  /\ ProjectedNoCommitEvidenceBeforeCommit
  /\ ProjectedRbcDeliveredWithoutFinalityHasNoCommitCertificate
  /\ ProjectedRbcDeliveredWithoutFinalityWaitsForCommitEvidence

ProjectedByzantineVoteFirstTopCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ProjectedByzantineVoteFirstTopExactness

ProjectedByzantineDirectTopExactness ==
  /\ ProjectedTlcByzantineDirectCommitCorridor
  /\ ProjectedCommitImpliesHonestSupport
  /\ ProjectedFinalityCertificateStackComplete
  /\ ProjectedCommitDisablesByzantineCommitVote
  /\ ProjectedByzantineCommitVoteGateMatchesPrepareEvidence
  /\ ProjectedByzantineCommitVoteFinalityGateMatchesNextEvidence
  /\ ProjectedByzantineCommitVotePendingGateMatchesMissingNextEvidence
  /\ ProjectedRbcDeliverFinalityGateMatchesBufferedCommitEvidence
  /\ ProjectedRbcDeliverPendingGateMatchesMissingBufferedCommitEvidence
  /\ ProjectedCommitEvidenceMatchesVoteCounters
  /\ ProjectedVoteCountersRespectRosterBudgets
  /\ ProjectedStakeSignedMatchesVoteCounters
  /\ ProjectedNoCommitEvidenceBeforeCommit
  /\ ProjectedRbcDeliveredWithoutFinalityHasNoCommitCertificate
  /\ ProjectedRbcDeliveredWithoutFinalityWaitsForCommitEvidence

ProjectedByzantineDirectTopCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ProjectedByzantineDirectTopExactness

ProjectionBridgeCoversOrderedTopCorridors ==
  ProjectedByzantineDirectTopExactness =>
    /\ ProjectedByzantineDeliveredFirstTopExactness
    /\ ProjectedByzantineVoteFirstTopExactness

ProjectionBridgeMatchesInterleavingCore ==
  ProjectedByzantineDirectTopExactness =>
    /\ RbcEvidenceShape
    /\ ProposedRoundInitializesRbc
    /\ VoteHandoffShape
    /\ CommitCertificateShape
    /\ BufferedVotesWaitForDelivery
    /\ DeliveredWithBufferedVotesCommits

ProjectionBridgeMatchesInterleavingExactness ==
  /\ ProjectedTlcByzantineDirectCommitCorridor
  /\ ProjectedCommitImpliesHonestSupport
  /\ ProjectedFinalityCertificateStackComplete
  /\ ProjectedCommitDisablesByzantineCommitVote
  /\ ProjectedByzantineCommitVoteGateMatchesPrepareEvidence
  /\ ProjectedByzantineCommitVoteFinalityGateMatchesNextEvidence
  /\ ProjectedByzantineCommitVotePendingGateMatchesMissingNextEvidence
  /\ ProjectedRbcDeliverFinalityGateMatchesBufferedCommitEvidence
  /\ ProjectedRbcDeliverPendingGateMatchesMissingBufferedCommitEvidence
  /\ ProjectedCommitEvidenceMatchesVoteCounters
  /\ ProjectedVoteCountersRespectRosterBudgets
  /\ ProjectedStakeSignedMatchesVoteCounters
  /\ ProjectedNoCommitEvidenceBeforeCommit
  /\ ProjectedRbcDeliveredWithoutFinalityHasNoCommitCertificate
  /\ ProjectedRbcDeliveredWithoutFinalityWaitsForCommitEvidence
  /\ RbcEvidenceShape
  /\ ProposedRoundInitializesRbc
  /\ VoteHandoffShape
  /\ CommitCertificateShape
  /\ BufferedVotesWaitForDelivery
  /\ DeliveredWithBufferedVotesCommits

ProjectionBridgeMatchesInterleavingExactnessCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ProjectionBridgeMatchesInterleavingExactness

ProjectedCommitProgressSafetyEnvelope ==
  /\ ProjectionBridgeCoversOrderedTopCorridors
  /\ ProjectionBridgeMatchesInterleavingExactnessCorrectnessEnvelope

ProjectedCommitProgressFairness ==
  /\ WF_vars(HonestPropose)
  /\ WF_vars(PrepareVote)
  /\ WF_vars(HonestCommitVote)
  /\ WF_vars(ByzantineCommitVote)
  /\ WF_vars(RbcChunk)
  /\ WF_vars(RbcReady)
  /\ WF_vars(RbcDeliver)

ProjectedCommitProgressSpec ==
  /\ Init
  /\ [][Next]_vars
  /\ ProjectedCommitProgressFairness

EventualProjectedCommit ==
  <>committed

ProjectedCommitFinalityStack ==
  /\ committed
  /\ ProjectedFinalityCertificateStackPresent

EventualProjectedCommitFinalityStack ==
  <>ProjectedCommitFinalityStack

====
