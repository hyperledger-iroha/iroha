---- MODULE Sumeragi ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi commit-path safety/liveness checks.

This spec intentionally models only the commit-critical state:
- voting phases and quorum counters,
- latched commit-certificate and commit-view evidence across view-counter resets,
- weighted NPoS stake quorum,
- RBC header/chunk/ready/deliver causality,
- view-change quorum evidence, progression, and GST flip,
- weak fairness assumptions over honest progress actions.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  N,
  \* @type: Int;
  F,
  \* @type: Int;
  CommitQuorum,
  \* @type: Int;
  ViewQuorum,
  \* @type: Int;
  StakeQuorum,
  \* @type: Int;
  StakePerHonestVote,
  \* @type: Int;
  StakePerByzVote,
  \* @type: Int;
  MaxView,
  \* @type: Int;
  MaxChunks

VARIABLES
  \* @type: Str;
  phase,
  \* @type: Int;
  view,
  \* @type: Int;
  commitView,
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
  \* @type: Int;
  newViewVotes,
  \* @type: Int;
  viewEvidenceVotes,
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
  committed,
  \* @type: Bool;
  gst

vars == <<
  phase,
  view,
  commitView,
  prepareVotes,
  commitVotesHonest,
  commitVotesByz,
  stakeSigned,
  commitEvidenceVotes,
  commitEvidenceStake,
  newViewVotes,
  viewEvidenceVotes,
  rbcState,
  chunkCount,
  readyVotes,
  headerSeen,
  digestValid,
  committed,
  gst
>>

Phases == {"Propose", "Prepare", "CommitVote", "NewView", "Committed"}
RbcStates == {
  "Idle",
  "Init",
  "Chunking",
  "ChunksComplete",
  "ReadyPartial",
  "ReadyQuorum",
  "Delivered",
  "Corrupted",
  "Withheld"
}

RbcInitializedStates ==
  {"Init", "Chunking", "ChunksComplete", "ReadyPartial", "ReadyQuorum", "Delivered"}

RbcChunkCoveredStates ==
  {"ChunksComplete", "ReadyPartial", "ReadyQuorum", "Delivered"}

RbcReadyQuorumStates ==
  {"ReadyQuorum", "Delivered"}

CanCommit(vh, vb, stake, rbc) ==
  /\ vh + vb >= CommitQuorum
  /\ stake >= StakeQuorum
  /\ rbc = "Delivered"

MaxCommitEvidenceStake ==
  ((N - F) * StakePerHonestVote) + (F * StakePerByzVote)

HonestCommitSupportThreshold ==
  CommitQuorum - F

TypeInvariant ==
  /\ phase \in Phases
  /\ view \in 0..MaxView
  /\ commitView \in 0..MaxView
  /\ prepareVotes \in 0..N
  /\ commitVotesHonest \in 0..N
  /\ commitVotesByz \in 0..N
  /\ stakeSigned \in Nat
  /\ commitEvidenceVotes \in 0..N
  /\ commitEvidenceStake \in Nat
  /\ newViewVotes \in 0..N
  /\ viewEvidenceVotes \in 0..N
  /\ rbcState \in RbcStates
  /\ chunkCount \in 0..MaxChunks
  /\ readyVotes \in 0..N
  /\ headerSeen \in BOOLEAN
  /\ digestValid \in BOOLEAN
  /\ committed \in BOOLEAN
  /\ gst \in BOOLEAN

Init ==
  /\ phase = "Propose"
  /\ view = 0
  /\ commitView = 0
  /\ prepareVotes = 0
  /\ commitVotesHonest = 0
  /\ commitVotesByz = 0
  /\ stakeSigned = 0
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceStake = 0
  /\ newViewVotes = 0
  /\ viewEvidenceVotes = 0
  /\ rbcState = "Idle"
  /\ chunkCount = 0
  /\ readyVotes = 0
  /\ headerSeen = FALSE
  /\ digestValid = FALSE
  /\ committed = FALSE
  /\ gst = FALSE

HonestProposeEnabled ==
  phase = "Propose"

HonestPrepareVoteEnabled ==
  /\ phase = "Prepare"
  /\ prepareVotes < N - F

HonestCommitVoteEnabled ==
  /\ phase = "CommitVote"
  /\ commitVotesHonest < N - F

ByzantineCommitVoteEnabled ==
  /\ phase = "CommitVote"
  /\ commitVotesByz < F

HonestNewViewVoteEnabled ==
  /\ phase = "NewView"
  /\ newViewVotes < N - F

RbcInitEnabled ==
  rbcState \in {"Idle", "Withheld", "Corrupted"}

RbcChunkGoodEnabled ==
  /\ rbcState \in {"Init", "Chunking", "Withheld"}
  /\ headerSeen
  /\ (rbcState # "Withheld" \/ gst)

RbcReadyGoodEnabled ==
  /\ rbcState \in {"ChunksComplete", "ReadyPartial", "ReadyQuorum"}
  /\ headerSeen
  /\ digestValid
  /\ readyVotes < N

RbcDeliverGoodEnabled ==
  /\ rbcState = "ReadyQuorum"
  /\ readyVotes >= CommitQuorum
  /\ headerSeen
  /\ digestValid

PostGstProgressEnabled ==
  \/ HonestProposeEnabled
  \/ HonestPrepareVoteEnabled
  \/ HonestCommitVoteEnabled
  \/ HonestNewViewVoteEnabled
  \/ RbcInitEnabled
  \/ RbcChunkGoodEnabled
  \/ RbcReadyGoodEnabled
  \/ RbcDeliverGoodEnabled

TimeoutTickEnabled ==
  /\ ~committed
  /\ (~gst \/ ~PostGstProgressEnabled)

ByzantineFaultEnabled ==
  /\ rbcState \in {"Init", "Chunking", "ChunksComplete", "ReadyPartial", "ReadyQuorum"}
  /\ (~gst \/ ~PostGstProgressEnabled)

GstElapsedEnabled ==
  ~gst

HonestPropose ==
  /\ HonestProposeEnabled
  /\ phase' = "Prepare"
  /\ prepareVotes' = 0
  /\ newViewVotes' = 0
  /\ rbcState' = IF rbcState = "Idle" THEN "Init" ELSE rbcState
  /\ headerSeen' = IF rbcState = "Idle" THEN TRUE ELSE headerSeen
  /\ digestValid' = IF rbcState = "Idle" THEN TRUE ELSE digestValid
  /\ chunkCount' = IF rbcState = "Idle" THEN 0 ELSE chunkCount
  /\ readyVotes' = IF rbcState = "Idle" THEN 0 ELSE readyVotes
  /\ UNCHANGED <<
      view,
      commitView,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      commitEvidenceVotes,
      commitEvidenceStake,
      viewEvidenceVotes,
      committed,
      gst
     >>

HonestPrepareVote ==
  /\ HonestPrepareVoteEnabled
  /\ prepareVotes' = prepareVotes + 1
  /\ phase' = IF prepareVotes' >= CommitQuorum THEN "CommitVote" ELSE "Prepare"
  /\ UNCHANGED <<
      view,
      commitView,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      commitEvidenceVotes,
      commitEvidenceStake,
      newViewVotes,
      viewEvidenceVotes,
      rbcState,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid,
      committed,
      gst
     >>

HonestCommitVote ==
  /\ HonestCommitVoteEnabled
  /\ commitVotesHonest' = commitVotesHonest + 1
  /\ stakeSigned' = stakeSigned + StakePerHonestVote
  /\ phase' =
      IF CanCommit(
            commitVotesHonest + 1,
            commitVotesByz,
            stakeSigned + StakePerHonestVote,
            rbcState
         )
      THEN "Committed"
      ELSE "CommitVote"
  /\ committed' =
      (committed \/ CanCommit(
                        commitVotesHonest + 1,
                        commitVotesByz,
                        stakeSigned + StakePerHonestVote,
                        rbcState
                    ))
  /\ commitEvidenceVotes' =
      IF CanCommit(
            commitVotesHonest + 1,
            commitVotesByz,
            stakeSigned + StakePerHonestVote,
            rbcState
         )
      THEN commitVotesHonest + 1 + commitVotesByz
      ELSE commitEvidenceVotes
  /\ commitEvidenceStake' =
      IF CanCommit(
            commitVotesHonest + 1,
            commitVotesByz,
            stakeSigned + StakePerHonestVote,
            rbcState
         )
      THEN stakeSigned + StakePerHonestVote
      ELSE commitEvidenceStake
  /\ commitView' =
      IF CanCommit(
            commitVotesHonest + 1,
            commitVotesByz,
            stakeSigned + StakePerHonestVote,
            rbcState
         )
      THEN view
      ELSE commitView
  /\ UNCHANGED <<
      view,
      prepareVotes,
      commitVotesByz,
      newViewVotes,
      viewEvidenceVotes,
      rbcState,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid,
      gst
     >>

ByzantineEquivocateCommit ==
  /\ ByzantineCommitVoteEnabled
  /\ commitVotesByz' = commitVotesByz + 1
  /\ stakeSigned' = stakeSigned + StakePerByzVote
  /\ phase' =
      IF CanCommit(
            commitVotesHonest,
            commitVotesByz + 1,
            stakeSigned + StakePerByzVote,
            rbcState
         )
      THEN "Committed"
      ELSE "CommitVote"
  /\ committed' =
      (committed \/ CanCommit(
                        commitVotesHonest,
                        commitVotesByz + 1,
                        stakeSigned + StakePerByzVote,
                        rbcState
                    ))
  /\ commitEvidenceVotes' =
      IF CanCommit(
            commitVotesHonest,
            commitVotesByz + 1,
            stakeSigned + StakePerByzVote,
            rbcState
         )
      THEN commitVotesHonest + commitVotesByz + 1
      ELSE commitEvidenceVotes
  /\ commitEvidenceStake' =
      IF CanCommit(
            commitVotesHonest,
            commitVotesByz + 1,
            stakeSigned + StakePerByzVote,
            rbcState
         )
      THEN stakeSigned + StakePerByzVote
      ELSE commitEvidenceStake
  /\ commitView' =
      IF CanCommit(
            commitVotesHonest,
            commitVotesByz + 1,
            stakeSigned + StakePerByzVote,
            rbcState
         )
      THEN view
      ELSE commitView
  /\ UNCHANGED <<
      view,
      prepareVotes,
      commitVotesHonest,
      newViewVotes,
      viewEvidenceVotes,
      rbcState,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid,
      gst
     >>

TimeoutTick ==
  /\ TimeoutTickEnabled
  /\ phase' = "NewView"
  /\ view' = IF view < MaxView THEN view + 1 ELSE MaxView
  /\ newViewVotes' = 0
  /\ prepareVotes' = 0
  /\ commitVotesHonest' = 0
  /\ commitVotesByz' = 0
  /\ stakeSigned' = 0
  /\ viewEvidenceVotes' = 0
  /\ UNCHANGED <<
      rbcState,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid,
      commitView,
      commitEvidenceVotes,
      commitEvidenceStake,
      committed,
      gst
     >>

HonestNewViewVote ==
  /\ HonestNewViewVoteEnabled
  /\ newViewVotes' = newViewVotes + 1
  /\ phase' = IF newViewVotes' >= ViewQuorum THEN "Propose" ELSE "NewView"
  /\ viewEvidenceVotes' =
      IF newViewVotes' >= ViewQuorum
      THEN newViewVotes'
      ELSE viewEvidenceVotes
  /\ UNCHANGED <<
      view,
      commitView,
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
      committed,
      gst
     >>

RbcInit ==
  /\ RbcInitEnabled
  /\ rbcState' = "Init"
  /\ chunkCount' = 0
  /\ readyVotes' = 0
  /\ headerSeen' = TRUE
  /\ digestValid' = TRUE
  /\ UNCHANGED <<
      phase,
      view,
      commitView,
      prepareVotes,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      commitEvidenceVotes,
      commitEvidenceStake,
      newViewVotes,
      viewEvidenceVotes,
      committed,
      gst
     >>

RbcChunkGood ==
  /\ RbcChunkGoodEnabled
  /\ chunkCount' = IF chunkCount < MaxChunks THEN chunkCount + 1 ELSE chunkCount
  /\ rbcState' = IF chunkCount' >= MaxChunks THEN "ChunksComplete" ELSE "Chunking"
  /\ digestValid' = TRUE
  /\ UNCHANGED <<
      phase,
      view,
      commitView,
      prepareVotes,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      commitEvidenceVotes,
      commitEvidenceStake,
      newViewVotes,
      viewEvidenceVotes,
      readyVotes,
      headerSeen,
      committed,
      gst
     >>

RbcReadyGood ==
  /\ RbcReadyGoodEnabled
  /\ readyVotes' = readyVotes + 1
  /\ rbcState' = IF readyVotes' >= CommitQuorum THEN "ReadyQuorum" ELSE "ReadyPartial"
  /\ UNCHANGED <<
      phase,
      view,
      commitView,
      prepareVotes,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      commitEvidenceVotes,
      commitEvidenceStake,
      newViewVotes,
      viewEvidenceVotes,
      chunkCount,
      headerSeen,
      digestValid,
      committed,
      gst
     >>

RbcDeliverGood ==
  /\ RbcDeliverGoodEnabled
  /\ rbcState' = "Delivered"
  /\ phase' =
      IF CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
      THEN "Committed"
      ELSE phase
  /\ committed' =
      (committed \/ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered"))
  /\ commitEvidenceVotes' =
      IF CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
      THEN commitVotesHonest + commitVotesByz
      ELSE commitEvidenceVotes
  /\ commitEvidenceStake' =
      IF CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
      THEN stakeSigned
      ELSE commitEvidenceStake
  /\ commitView' =
      IF CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
      THEN view
      ELSE commitView
  /\ UNCHANGED <<
      view,
      prepareVotes,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      newViewVotes,
      viewEvidenceVotes,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid,
      gst
     >>

ByzantineFault ==
  /\ ByzantineFaultEnabled
  /\ rbcState' = "Corrupted"
  /\ digestValid' = FALSE
  /\ UNCHANGED <<
      phase,
      view,
      commitView,
      prepareVotes,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      commitEvidenceVotes,
      commitEvidenceStake,
      newViewVotes,
      viewEvidenceVotes,
      chunkCount,
      readyVotes,
      headerSeen,
      committed,
      gst
     >>

GstElapsed ==
  /\ GstElapsedEnabled
  /\ gst' = TRUE
  /\ UNCHANGED <<
      phase,
      view,
      commitView,
      prepareVotes,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      newViewVotes,
      viewEvidenceVotes,
      rbcState,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid,
      commitEvidenceVotes,
      commitEvidenceStake,
      committed
     >>

Next ==
  \/ HonestPropose
  \/ HonestPrepareVote
  \/ HonestCommitVote
  \/ ByzantineEquivocateCommit
  \/ TimeoutTick
  \/ HonestNewViewVote
  \/ RbcInit
  \/ RbcChunkGood
  \/ RbcReadyGood
  \/ RbcDeliverGood
  \/ ByzantineFault
  \/ GstElapsed

Fairness ==
  /\ WF_vars(HonestPropose)
  /\ WF_vars(HonestPrepareVote)
  /\ WF_vars(HonestCommitVote)
  /\ WF_vars(HonestNewViewVote)
  /\ WF_vars(RbcInit)
  /\ WF_vars(RbcChunkGood)
  /\ WF_vars(RbcReadyGood)
  /\ WF_vars(RbcDeliverGood)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ Fairness

CommitImpliesQuorum ==
  committed => commitEvidenceVotes >= CommitQuorum

CommitImpliesStakeQuorum ==
  committed => commitEvidenceStake >= StakeQuorum

CommitCertificateMatchesFinality ==
  committed <=> (
    /\ commitEvidenceVotes >= CommitQuorum
    /\ commitEvidenceStake >= StakeQuorum
  )

LiveCommitGateMatchesFinality ==
  committed <=>
    CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, rbcState)

LiveCommitGateRbcEvidenceMatches ==
  CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, rbcState) <=>
    /\ commitVotesHonest + commitVotesByz >= CommitQuorum
    /\ stakeSigned >= StakeQuorum
    /\ rbcState = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid

CommitImpliesLiveVoteQuorum ==
  committed => commitVotesHonest + commitVotesByz >= CommitQuorum

CommitImpliesLiveStakeQuorum ==
  committed => stakeSigned >= StakeQuorum

CommitImpliesHonestSupport ==
  committed => commitVotesHonest >= HonestCommitSupportThreshold

CommitImpliesDelivered ==
  committed => rbcState = "Delivered"

CommitImpliesRbcEvidence ==
  committed =>
    /\ rbcState = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid

FinalityCertificateStackPresent ==
  /\ phase = "Committed"
  /\ newViewVotes = 0
  /\ prepareVotes >= CommitQuorum
  /\ commitVotesHonest + commitVotesByz >= CommitQuorum
  /\ commitVotesHonest >= HonestCommitSupportThreshold
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
  /\ commitView = view
  /\ (commitView = 0 \/ viewEvidenceVotes >= ViewQuorum)

FinalityCertificateStackComplete ==
  committed => FinalityCertificateStackPresent

FinalityCertificateStackMatchesFinality ==
  committed <=> FinalityCertificateStackPresent

FinalityClearsNewViewHandoff ==
  committed => newViewVotes = 0

CommitDisablesProgressActions ==
  committed =>
    /\ ~HonestProposeEnabled
    /\ ~HonestPrepareVoteEnabled
    /\ ~HonestCommitVoteEnabled
    /\ ~ByzantineCommitVoteEnabled
    /\ ~HonestNewViewVoteEnabled
    /\ ~RbcInitEnabled
    /\ ~RbcChunkGoodEnabled
    /\ ~RbcReadyGoodEnabled
    /\ ~RbcDeliverGoodEnabled
    /\ ~TimeoutTickEnabled
    /\ ~ByzantineFaultEnabled
    /\ ~PostGstProgressEnabled

CommitDisablesByzantineCommitVote ==
  committed => ~ByzantineCommitVoteEnabled

CommittedPhaseMatchesFinality ==
  (phase = "Committed") <=> committed

CommitViewMatchesFinality ==
  committed => commitView = view

CommitViewDoesNotLeadCurrentView ==
  commitView <= view

GstElapsedGateMatchesPreGst ==
  GstElapsedEnabled <=> ~gst

CommittedPreGstOnlyEnablesGstElapsed ==
  (committed /\ ~gst) =>
    /\ GstElapsedEnabled
    /\ CommitDisablesProgressActions
    /\ ~HonestProposeEnabled
    /\ ~HonestPrepareVoteEnabled
    /\ ~HonestCommitVoteEnabled
    /\ ~ByzantineCommitVoteEnabled
    /\ ~HonestNewViewVoteEnabled
    /\ ~RbcInitEnabled
    /\ ~RbcChunkGoodEnabled
    /\ ~RbcReadyGoodEnabled
    /\ ~RbcDeliverGoodEnabled
    /\ ~TimeoutTickEnabled
    /\ ~ByzantineFaultEnabled
    /\ ~PostGstProgressEnabled

TimeoutTickGateMatchesStalledProgress ==
  TimeoutTickEnabled <=>
    /\ ~committed
    /\ \/ ~gst
       \/ /\ ~HonestProposeEnabled
          /\ ~HonestPrepareVoteEnabled
          /\ ~HonestCommitVoteEnabled
          /\ ~HonestNewViewVoteEnabled
          /\ ~RbcInitEnabled
          /\ ~RbcChunkGoodEnabled
          /\ ~RbcReadyGoodEnabled
          /\ ~RbcDeliverGoodEnabled

ByzantineCommitVoteDoesNotBlockTimeoutStall ==
  /\ gst
  /\ ~committed
  /\ ByzantineCommitVoteEnabled
  /\ ~PostGstProgressEnabled
  => TimeoutTickEnabled

TimeoutTickStepStartsFreshNewView ==
  TimeoutTick =>
    /\ ~committed
    /\ ~committed'
    /\ phase' = "NewView"
    /\ view' = IF view < MaxView THEN view + 1 ELSE MaxView
    /\ view' >= view
    /\ view' <= MaxView
    /\ prepareVotes' = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = 0
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
       /\ rbcState' = rbcState
       /\ chunkCount' = chunkCount
       /\ readyVotes' = readyVotes
       /\ headerSeen' = headerSeen
       /\ digestValid' = digestValid
       /\ gst' = gst

TimeoutTickStepNeverPreemptsProgressStep ==
  TimeoutTick =>
    /\ TimeoutTickEnabled
    /\ ~committed
    /\ (~gst \/ ~PostGstProgressEnabled)

TimeoutTickStepClearsCommitVoteGates ==
  TimeoutTick =>
    /\ phase' = "NewView"
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')

TimeoutTickStepStartsNewViewVoteHandoff ==
  TimeoutTick =>
    /\ phase' = "NewView"
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = 0
    /\ HonestNewViewVoteEnabled'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'

TimeoutTickStepPreservesRbcEvidence ==
  TimeoutTick =>
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid

ViewAdvanceOnlyComesFromTimeoutStep ==
  (view' # view) =>
    /\ TimeoutTick
    /\ view < MaxView
    /\ view' = view + 1
    /\ ~committed
    /\ ~committed'
    /\ phase' = "NewView"
    /\ prepareVotes' = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = 0
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ gst' = gst

LiveProgressResetOnlyByTimeoutStep ==
  (\/ /\ prepareVotes # 0
      /\ prepareVotes' = 0
   \/ /\ commitVotesHonest # 0
      /\ commitVotesHonest' = 0
   \/ /\ commitVotesByz # 0
      /\ commitVotesByz' = 0
   \/ /\ stakeSigned # 0
      /\ stakeSigned' = 0
   \/ /\ viewEvidenceVotes # 0
      /\ viewEvidenceVotes' = 0
   \/ /\ newViewVotes # 0
      /\ newViewVotes' = 0
      /\ phase' = "NewView") =>
    /\ TimeoutTick
    /\ ~committed
    /\ ~committed'
    /\ phase' = "NewView"
    /\ view' = IF view < MaxView THEN view + 1 ELSE MaxView
    /\ prepareVotes' = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = 0
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ gst' = gst

ViewEvidenceChangesOnlyByQuorumOrTimeoutStep ==
  (viewEvidenceVotes' # viewEvidenceVotes) =>
    \/ /\ HonestNewViewVote
       /\ phase = "NewView"
       /\ phase' = "Propose"
       /\ view > 0
       /\ view' = view
       /\ ~committed
       /\ ~committed'
       /\ newViewVotes < ViewQuorum
       /\ newViewVotes < N - F
       /\ newViewVotes' = newViewVotes + 1
       /\ newViewVotes' >= ViewQuorum
       /\ viewEvidenceVotes = 0
       /\ viewEvidenceVotes' = newViewVotes'
       /\ viewEvidenceVotes' >= ViewQuorum
       /\ prepareVotes = 0
       /\ prepareVotes' = 0
       /\ commitVotesHonest = 0
       /\ commitVotesHonest' = 0
       /\ commitVotesByz = 0
       /\ commitVotesByz' = 0
       /\ stakeSigned = 0
       /\ stakeSigned' = 0
       /\ commitEvidenceVotes = 0
       /\ commitEvidenceVotes' = 0
       /\ commitEvidenceStake = 0
       /\ commitEvidenceStake' = 0
       /\ commitView = 0
       /\ commitView' = 0
       /\ rbcState' = rbcState
       /\ chunkCount' = chunkCount
       /\ readyVotes' = readyVotes
       /\ headerSeen' = headerSeen
       /\ digestValid' = digestValid
    \/ /\ TimeoutTick
       /\ ~committed
       /\ ~committed'
       /\ phase' = "NewView"
       /\ viewEvidenceVotes # 0
       /\ viewEvidenceVotes' = 0
       /\ newViewVotes' = 0
       /\ prepareVotes' = 0
       /\ commitVotesHonest' = 0
       /\ commitVotesByz' = 0
       /\ stakeSigned' = 0
       /\ commitEvidenceVotes = 0
       /\ commitEvidenceVotes' = 0
       /\ commitEvidenceStake = 0
       /\ commitEvidenceStake' = 0
       /\ commitView = 0
       /\ commitView' = 0
       /\ rbcState' = rbcState
       /\ chunkCount' = chunkCount
       /\ readyVotes' = readyVotes
       /\ headerSeen' = headerSeen
       /\ digestValid' = digestValid
       /\ gst' = gst

NewViewVotesIncrementByVoteStep ==
  /\ HonestNewViewVote
  /\ phase = "NewView"
  /\ view > 0
  /\ view' = view
  /\ ~committed
  /\ ~committed'
  /\ viewEvidenceVotes = 0
  /\ newViewVotes < ViewQuorum
  /\ newViewVotes < N - F
  /\ newViewVotes' = newViewVotes + 1
  /\ prepareVotes = 0
  /\ prepareVotes' = 0
  /\ commitVotesHonest = 0
  /\ commitVotesHonest' = 0
  /\ commitVotesByz = 0
  /\ commitVotesByz' = 0
  /\ stakeSigned = 0
  /\ stakeSigned' = 0
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceVotes' = 0
  /\ commitEvidenceStake = 0
  /\ commitEvidenceStake' = 0
  /\ commitView = 0
  /\ commitView' = 0
  /\ rbcState' = rbcState
  /\ chunkCount' = chunkCount
  /\ readyVotes' = readyVotes
  /\ headerSeen' = headerSeen
  /\ digestValid' = digestValid
  /\ (newViewVotes' >= ViewQuorum =>
        /\ phase' = "Propose"
        /\ viewEvidenceVotes' = newViewVotes'
        /\ viewEvidenceVotes' >= ViewQuorum)
  /\ (newViewVotes' < ViewQuorum =>
        /\ phase' = "NewView"
        /\ viewEvidenceVotes' = 0)

NewViewVotesResetByProposalStep ==
  /\ HonestPropose
  /\ phase = "Propose"
  /\ phase' = "Prepare"
  /\ ~committed
  /\ ~committed'
  /\ newViewVotes # 0
  /\ newViewVotes >= ViewQuorum
  /\ newViewVotes' = 0
  /\ view' = view
  /\ viewEvidenceVotes' = viewEvidenceVotes
  /\ prepareVotes = 0
  /\ prepareVotes' = 0
  /\ commitVotesHonest = 0
  /\ commitVotesHonest' = 0
  /\ commitVotesByz = 0
  /\ commitVotesByz' = 0
  /\ stakeSigned = 0
  /\ stakeSigned' = 0
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceVotes' = 0
  /\ commitEvidenceStake = 0
  /\ commitEvidenceStake' = 0
  /\ commitView = 0
  /\ commitView' = 0
  /\ rbcState' = IF rbcState = "Idle" THEN "Init" ELSE rbcState
  /\ headerSeen' = IF rbcState = "Idle" THEN TRUE ELSE headerSeen
  /\ digestValid' = IF rbcState = "Idle" THEN TRUE ELSE digestValid
  /\ chunkCount' = IF rbcState = "Idle" THEN 0 ELSE chunkCount
  /\ readyVotes' = IF rbcState = "Idle" THEN 0 ELSE readyVotes

NewViewVotesResetByTimeoutStep ==
  /\ TimeoutTick
  /\ ~committed
  /\ ~committed'
  /\ phase' = "NewView"
  /\ newViewVotes # 0
  /\ newViewVotes' = 0
  /\ prepareVotes' = 0
  /\ commitVotesHonest' = 0
  /\ commitVotesByz' = 0
  /\ stakeSigned' = 0
  /\ viewEvidenceVotes' = 0
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceVotes' = 0
  /\ commitEvidenceStake = 0
  /\ commitEvidenceStake' = 0
  /\ commitView = 0
  /\ commitView' = 0
  /\ rbcState' = rbcState
  /\ chunkCount' = chunkCount
  /\ readyVotes' = readyVotes
  /\ headerSeen' = headerSeen
  /\ digestValid' = digestValid
  /\ gst' = gst

NewViewVotesChangeOnlyByVoteOrResetStep ==
  (newViewVotes' # newViewVotes) =>
    \/ NewViewVotesIncrementByVoteStep
    \/ NewViewVotesResetByProposalStep
    \/ NewViewVotesResetByTimeoutStep

PrepareVotesIncrementByVoteStep ==
  /\ HonestPrepareVote
  /\ phase = "Prepare"
  /\ view' = view
  /\ ~committed
  /\ ~committed'
  /\ prepareVotes < CommitQuorum
  /\ prepareVotes < N - F
  /\ prepareVotes' = prepareVotes + 1
  /\ newViewVotes = 0
  /\ newViewVotes' = 0
  /\ commitVotesHonest = 0
  /\ commitVotesHonest' = 0
  /\ commitVotesByz = 0
  /\ commitVotesByz' = 0
  /\ stakeSigned = 0
  /\ stakeSigned' = 0
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceVotes' = 0
  /\ commitEvidenceStake = 0
  /\ commitEvidenceStake' = 0
  /\ commitView = 0
  /\ commitView' = 0
  /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
  /\ viewEvidenceVotes' = viewEvidenceVotes
  /\ rbcState' = rbcState
  /\ chunkCount' = chunkCount
  /\ readyVotes' = readyVotes
  /\ headerSeen' = headerSeen
  /\ digestValid' = digestValid
  /\ (prepareVotes' >= CommitQuorum =>
        /\ phase' = "CommitVote"
        /\ prepareVotes' >= CommitQuorum)
  /\ (prepareVotes' < CommitQuorum =>
        /\ phase' = "Prepare"
        /\ prepareVotes' < CommitQuorum)

PrepareVotesResetByTimeoutStep ==
  /\ TimeoutTick
  /\ ~committed
  /\ ~committed'
  /\ phase' = "NewView"
  /\ view' = IF view < MaxView THEN view + 1 ELSE MaxView
  /\ prepareVotes # 0
  /\ prepareVotes' = 0
  /\ newViewVotes' = 0
  /\ commitVotesHonest' = 0
  /\ commitVotesByz' = 0
  /\ stakeSigned' = 0
  /\ viewEvidenceVotes' = 0
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceVotes' = 0
  /\ commitEvidenceStake = 0
  /\ commitEvidenceStake' = 0
  /\ commitView = 0
  /\ commitView' = 0
  /\ rbcState' = rbcState
  /\ chunkCount' = chunkCount
  /\ readyVotes' = readyVotes
  /\ headerSeen' = headerSeen
  /\ digestValid' = digestValid
  /\ gst' = gst

PrepareVotesChangeOnlyByVoteOrTimeoutStep ==
  (prepareVotes' # prepareVotes) =>
    \/ PrepareVotesIncrementByVoteStep
    \/ PrepareVotesResetByTimeoutStep

HonestCommitVoteCountersIncrementStep ==
  /\ HonestCommitVote
  /\ phase = "CommitVote"
  /\ prepareVotes >= CommitQuorum
  /\ prepareVotes' = prepareVotes
  /\ commitVotesHonest < N - F
  /\ commitVotesByz <= F
  /\ commitVotesHonest' = commitVotesHonest + 1
  /\ commitVotesByz' = commitVotesByz
  /\ stakeSigned' = stakeSigned + StakePerHonestVote
  /\ newViewVotes = 0
  /\ newViewVotes' = 0
  /\ ~committed
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceStake = 0
  /\ commitView = 0
  /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
  /\ view' = view
  /\ viewEvidenceVotes' = viewEvidenceVotes
  /\ rbcState' = rbcState
  /\ chunkCount' = chunkCount
  /\ readyVotes' = readyVotes
  /\ headerSeen' = headerSeen
  /\ digestValid' = digestValid
  /\ (CanCommit(
        commitVotesHonest + 1,
        commitVotesByz,
        stakeSigned + StakePerHonestVote,
        rbcState
      ) =>
        /\ phase' = "Committed"
        /\ committed'
        /\ commitEvidenceVotes' = commitVotesHonest + 1 + commitVotesByz
        /\ commitEvidenceStake' = stakeSigned + StakePerHonestVote
        /\ commitView' = view)
  /\ (~CanCommit(
        commitVotesHonest + 1,
        commitVotesByz,
        stakeSigned + StakePerHonestVote,
        rbcState
      ) =>
        /\ phase' = "CommitVote"
        /\ ~committed'
        /\ commitEvidenceVotes' = 0
        /\ commitEvidenceStake' = 0
        /\ commitView' = 0)

ByzantineCommitVoteCountersIncrementStep ==
  /\ ByzantineEquivocateCommit
  /\ phase = "CommitVote"
  /\ prepareVotes >= CommitQuorum
  /\ prepareVotes' = prepareVotes
  /\ commitVotesHonest <= N - F
  /\ commitVotesHonest' = commitVotesHonest
  /\ commitVotesByz < F
  /\ commitVotesByz' = commitVotesByz + 1
  /\ stakeSigned' = stakeSigned + StakePerByzVote
  /\ newViewVotes = 0
  /\ newViewVotes' = 0
  /\ ~committed
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceStake = 0
  /\ commitView = 0
  /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
  /\ view' = view
  /\ viewEvidenceVotes' = viewEvidenceVotes
  /\ rbcState' = rbcState
  /\ chunkCount' = chunkCount
  /\ readyVotes' = readyVotes
  /\ headerSeen' = headerSeen
  /\ digestValid' = digestValid
  /\ (CanCommit(
        commitVotesHonest,
        commitVotesByz + 1,
        stakeSigned + StakePerByzVote,
        rbcState
      ) =>
        /\ phase' = "Committed"
        /\ committed'
        /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz + 1
        /\ commitEvidenceStake' = stakeSigned + StakePerByzVote
        /\ commitView' = view)
  /\ (~CanCommit(
        commitVotesHonest,
        commitVotesByz + 1,
        stakeSigned + StakePerByzVote,
        rbcState
      ) =>
        /\ phase' = "CommitVote"
        /\ ~committed'
        /\ commitEvidenceVotes' = 0
        /\ commitEvidenceStake' = 0
        /\ commitView' = 0)

CommitVoteCountersResetByTimeoutStep ==
  /\ TimeoutTick
  /\ ~committed
  /\ ~committed'
  /\ phase = "CommitVote"
  /\ phase' = "NewView"
  /\ view' = IF view < MaxView THEN view + 1 ELSE MaxView
  /\ \/ commitVotesHonest # 0
     \/ commitVotesByz # 0
     \/ stakeSigned # 0
  /\ prepareVotes' = 0
  /\ commitVotesHonest' = 0
  /\ commitVotesByz' = 0
  /\ stakeSigned' = 0
  /\ newViewVotes' = 0
  /\ viewEvidenceVotes' = 0
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceVotes' = 0
  /\ commitEvidenceStake = 0
  /\ commitEvidenceStake' = 0
  /\ commitView = 0
  /\ commitView' = 0
  /\ rbcState' = rbcState
  /\ chunkCount' = chunkCount
  /\ readyVotes' = readyVotes
  /\ headerSeen' = headerSeen
  /\ digestValid' = digestValid
  /\ gst' = gst

CommitVoteCountersChangeOnlyByVoteOrTimeoutStep ==
  (\/ commitVotesHonest' # commitVotesHonest
   \/ commitVotesByz' # commitVotesByz
   \/ stakeSigned' # stakeSigned) =>
    \/ HonestCommitVoteCountersIncrementStep
    \/ ByzantineCommitVoteCountersIncrementStep
    \/ CommitVoteCountersResetByTimeoutStep

PhaseStartsPrepareByProposalStep ==
  /\ HonestPropose
  /\ phase = "Propose"
  /\ phase' = "Prepare"
  /\ view' = view
  /\ ~committed
  /\ ~committed'
  /\ prepareVotes = 0
  /\ prepareVotes' = 0
  /\ newViewVotes' = 0
  /\ commitVotesHonest = 0
  /\ commitVotesHonest' = 0
  /\ commitVotesByz = 0
  /\ commitVotesByz' = 0
  /\ stakeSigned = 0
  /\ stakeSigned' = 0
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceVotes' = 0
  /\ commitEvidenceStake = 0
  /\ commitEvidenceStake' = 0
  /\ commitView = 0
  /\ commitView' = 0

PhaseStartsCommitVoteByPrepareQuorumStep ==
  /\ HonestPrepareVote
  /\ phase = "Prepare"
  /\ phase' = "CommitVote"
  /\ view' = view
  /\ ~committed
  /\ ~committed'
  /\ prepareVotes < CommitQuorum
  /\ prepareVotes' = prepareVotes + 1
  /\ prepareVotes' >= CommitQuorum
  /\ newViewVotes = 0
  /\ newViewVotes' = 0
  /\ commitVotesHonest = 0
  /\ commitVotesHonest' = 0
  /\ commitVotesByz = 0
  /\ commitVotesByz' = 0
  /\ stakeSigned = 0
  /\ stakeSigned' = 0
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceVotes' = 0
  /\ commitEvidenceStake = 0
  /\ commitEvidenceStake' = 0
  /\ commitView = 0
  /\ commitView' = 0

PhaseStartsProposeByNewViewQuorumStep ==
  /\ HonestNewViewVote
  /\ phase = "NewView"
  /\ phase' = "Propose"
  /\ view > 0
  /\ view' = view
  /\ ~committed
  /\ ~committed'
  /\ newViewVotes < ViewQuorum
  /\ newViewVotes' = newViewVotes + 1
  /\ newViewVotes' >= ViewQuorum
  /\ viewEvidenceVotes = 0
  /\ viewEvidenceVotes' = newViewVotes'
  /\ viewEvidenceVotes' >= ViewQuorum
  /\ prepareVotes = 0
  /\ prepareVotes' = 0
  /\ commitVotesHonest = 0
  /\ commitVotesHonest' = 0
  /\ commitVotesByz = 0
  /\ commitVotesByz' = 0
  /\ stakeSigned = 0
  /\ stakeSigned' = 0
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceVotes' = 0
  /\ commitEvidenceStake = 0
  /\ commitEvidenceStake' = 0
  /\ commitView = 0
  /\ commitView' = 0

PhaseStartsNewViewByTimeoutStep ==
  /\ TimeoutTick
  /\ phase # "NewView"
  /\ phase' = "NewView"
  /\ view' = IF view < MaxView THEN view + 1 ELSE MaxView
  /\ ~committed
  /\ ~committed'
  /\ prepareVotes' = 0
  /\ commitVotesHonest' = 0
  /\ commitVotesByz' = 0
  /\ stakeSigned' = 0
  /\ newViewVotes' = 0
  /\ viewEvidenceVotes' = 0
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceVotes' = 0
  /\ commitEvidenceStake = 0
  /\ commitEvidenceStake' = 0
  /\ commitView = 0
  /\ commitView' = 0

PhaseFinalizesByHonestCommitVoteStep ==
  /\ HonestCommitVote
  /\ phase = "CommitVote"
  /\ phase' = "Committed"
  /\ ~committed
  /\ committed'
  /\ CanCommit(
       commitVotesHonest + 1,
       commitVotesByz,
       stakeSigned + StakePerHonestVote,
       rbcState
     )
  /\ prepareVotes >= CommitQuorum
  /\ prepareVotes' = prepareVotes
  /\ commitVotesHonest' = commitVotesHonest + 1
  /\ commitVotesByz' = commitVotesByz
  /\ stakeSigned' = stakeSigned + StakePerHonestVote
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceVotes' = commitVotesHonest + 1 + commitVotesByz
  /\ commitEvidenceStake = 0
  /\ commitEvidenceStake' = stakeSigned + StakePerHonestVote
  /\ commitView = 0
  /\ commitView' = view
  /\ view' = view
  /\ rbcState = "Delivered"
  /\ rbcState' = "Delivered"

PhaseFinalizesByByzantineCommitVoteStep ==
  /\ ByzantineEquivocateCommit
  /\ phase = "CommitVote"
  /\ phase' = "Committed"
  /\ ~committed
  /\ committed'
  /\ CanCommit(
       commitVotesHonest,
       commitVotesByz + 1,
       stakeSigned + StakePerByzVote,
       rbcState
     )
  /\ prepareVotes >= CommitQuorum
  /\ prepareVotes' = prepareVotes
  /\ commitVotesHonest' = commitVotesHonest
  /\ commitVotesByz' = commitVotesByz + 1
  /\ stakeSigned' = stakeSigned + StakePerByzVote
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz + 1
  /\ commitEvidenceStake = 0
  /\ commitEvidenceStake' = stakeSigned + StakePerByzVote
  /\ commitView = 0
  /\ commitView' = view
  /\ view' = view
  /\ rbcState = "Delivered"
  /\ rbcState' = "Delivered"

PhaseFinalizesByRbcDeliverStep ==
  /\ RbcDeliverGood
  /\ phase = "CommitVote"
  /\ phase' = "Committed"
  /\ ~committed
  /\ committed'
  /\ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
  /\ prepareVotes >= CommitQuorum
  /\ prepareVotes' = prepareVotes
  /\ commitVotesHonest' = commitVotesHonest
  /\ commitVotesByz' = commitVotesByz
  /\ stakeSigned' = stakeSigned
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz
  /\ commitEvidenceStake = 0
  /\ commitEvidenceStake' = stakeSigned
  /\ commitView = 0
  /\ commitView' = view
  /\ view' = view
  /\ rbcState = "ReadyQuorum"
  /\ rbcState' = "Delivered"

PhaseOnlyChangesByProtocolStep ==
  (phase' # phase) =>
    \/ PhaseStartsPrepareByProposalStep
    \/ PhaseStartsCommitVoteByPrepareQuorumStep
    \/ PhaseStartsProposeByNewViewQuorumStep
    \/ PhaseStartsNewViewByTimeoutStep
    \/ PhaseFinalizesByHonestCommitVoteStep
    \/ PhaseFinalizesByByzantineCommitVoteStep
    \/ PhaseFinalizesByRbcDeliverStep

PreparePhaseEntryOnlyByProposalStep ==
  (/\ phase # "Prepare"
   /\ phase' = "Prepare") =>
    /\ PhaseStartsPrepareByProposalStep
    /\ HonestPropose
    /\ phase = "Propose"
    /\ view' = view
    /\ ~committed
    /\ ~committed'
    /\ prepareVotes = 0
    /\ prepareVotes' = 0
    /\ newViewVotes' = 0
    /\ commitVotesHonest = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ (newViewVotes = 0 \/ newViewVotes >= ViewQuorum)
    /\ rbcState' = IF rbcState = "Idle" THEN "Init" ELSE rbcState
    /\ headerSeen' = IF rbcState = "Idle" THEN TRUE ELSE headerSeen
    /\ digestValid' = IF rbcState = "Idle" THEN TRUE ELSE digestValid
    /\ chunkCount' = IF rbcState = "Idle" THEN 0 ELSE chunkCount
    /\ readyVotes' = IF rbcState = "Idle" THEN 0 ELSE readyVotes
    /\ gst' = gst

CommitVotePhaseEntryOnlyByPrepareQuorumStep ==
  (/\ phase # "CommitVote"
   /\ phase' = "CommitVote") =>
    /\ PhaseStartsCommitVoteByPrepareQuorumStep
    /\ HonestPrepareVote
    /\ phase = "Prepare"
    /\ view' = view
    /\ ~committed
    /\ ~committed'
    /\ prepareVotes < CommitQuorum
    /\ prepareVotes < N - F
    /\ prepareVotes' = prepareVotes + 1
    /\ prepareVotes' >= CommitQuorum
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ commitVotesHonest = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ gst' = gst

ProposePhaseEntryOnlyByNewViewQuorumStep ==
  (/\ phase # "Propose"
   /\ phase' = "Propose") =>
    /\ PhaseStartsProposeByNewViewQuorumStep
    /\ HonestNewViewVote
    /\ phase = "NewView"
    /\ view > 0
    /\ view' = view
    /\ ~committed
    /\ ~committed'
    /\ newViewVotes < ViewQuorum
    /\ newViewVotes < N - F
    /\ newViewVotes' = newViewVotes + 1
    /\ newViewVotes' >= ViewQuorum
    /\ viewEvidenceVotes = 0
    /\ viewEvidenceVotes' = newViewVotes'
    /\ viewEvidenceVotes' >= ViewQuorum
    /\ prepareVotes = 0
    /\ prepareVotes' = 0
    /\ commitVotesHonest = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ gst' = gst

NewViewPhaseEntryOnlyByTimeoutStep ==
  (/\ phase # "NewView"
   /\ phase' = "NewView") =>
    /\ PhaseStartsNewViewByTimeoutStep
    /\ TimeoutTick
    /\ TimeoutTickStepStartsFreshNewView
    /\ ~committed
    /\ ~committed'
    /\ view' = IF view < MaxView THEN view + 1 ELSE MaxView
    /\ view' >= view
    /\ view' <= MaxView
    /\ prepareVotes' = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = 0
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ gst' = gst

RbcStateInitByProposalStep ==
  /\ HonestPropose
  /\ rbcState = "Idle"
  /\ rbcState' = "Init"
  /\ phase = "Propose"
  /\ phase' = "Prepare"
  /\ ~committed
  /\ ~committed'
  /\ headerSeen'
  /\ digestValid'
  /\ chunkCount' = 0
  /\ readyVotes' = 0
  /\ newViewVotes' = 0

RbcStateRepairByInitStep ==
  /\ RbcInit
  /\ rbcState \in {"Idle", "Withheld", "Corrupted"}
  /\ rbcState' = "Init"
  /\ chunkCount' = 0
  /\ readyVotes' = 0
  /\ headerSeen'
  /\ digestValid'
  /\ committed' = committed
  /\ gst' = gst

RbcStateAdvanceByChunkStep ==
  /\ RbcChunkGood
  /\ rbcState \in {"Init", "Chunking", "Withheld"}
  /\ rbcState' = IF chunkCount' >= MaxChunks THEN "ChunksComplete" ELSE "Chunking"
  /\ chunkCount' = IF chunkCount < MaxChunks THEN chunkCount + 1 ELSE chunkCount
  /\ chunkCount' >= chunkCount
  /\ chunkCount' <= MaxChunks
  /\ headerSeen' = headerSeen
  /\ digestValid'
  /\ readyVotes' = readyVotes
  /\ committed' = committed
  /\ gst' = gst

RbcStateAdvanceByReadyStep ==
  /\ RbcReadyGood
  /\ rbcState \in {"ChunksComplete", "ReadyPartial", "ReadyQuorum"}
  /\ rbcState' = IF readyVotes' >= CommitQuorum THEN "ReadyQuorum" ELSE "ReadyPartial"
  /\ readyVotes' = readyVotes + 1
  /\ readyVotes' <= N
  /\ chunkCount' = chunkCount
  /\ headerSeen' = headerSeen
  /\ digestValid' = digestValid
  /\ committed' = committed
  /\ gst' = gst

RbcStateDeliverByDeliverStep ==
  /\ RbcDeliverGood
  /\ rbcState = "ReadyQuorum"
  /\ rbcState' = "Delivered"
  /\ readyVotes >= CommitQuorum
  /\ chunkCount >= MaxChunks
  /\ headerSeen
  /\ digestValid
  /\ chunkCount' = chunkCount
  /\ readyVotes' = readyVotes
  /\ headerSeen' = headerSeen
  /\ digestValid' = digestValid
  /\ gst' = gst

RbcStateCorruptedByFaultStep ==
  /\ ByzantineFault
  /\ rbcState \in {"Init", "Chunking", "ChunksComplete", "ReadyPartial", "ReadyQuorum"}
  /\ rbcState' = "Corrupted"
  /\ digestValid' = FALSE
  /\ chunkCount' = chunkCount
  /\ readyVotes' = readyVotes
  /\ headerSeen' = headerSeen
  /\ committed' = committed
  /\ gst' = gst

RbcStateOnlyChangesByProtocolOrFaultStep ==
  (rbcState' # rbcState) =>
    \/ RbcStateInitByProposalStep
    \/ RbcStateRepairByInitStep
    \/ RbcStateAdvanceByChunkStep
    \/ RbcStateAdvanceByReadyStep
    \/ RbcStateDeliverByDeliverStep
    \/ RbcStateCorruptedByFaultStep

RbcEvidenceInitByProposalStep ==
  /\ HonestPropose
  /\ rbcState = "Idle"
  /\ rbcState' = "Init"
  /\ phase = "Propose"
  /\ phase' = "Prepare"
  /\ ~committed
  /\ ~committed'
  /\ headerSeen'
  /\ digestValid'
  /\ chunkCount' = 0
  /\ readyVotes' = 0
  /\ newViewVotes' = 0

RbcEvidenceRepairByInitStep ==
  /\ RbcInit
  /\ rbcState \in {"Idle", "Withheld", "Corrupted"}
  /\ rbcState' = "Init"
  /\ headerSeen'
  /\ digestValid'
  /\ chunkCount' = 0
  /\ readyVotes' = 0
  /\ committed' = committed
  /\ gst' = gst

RbcEvidenceAdvancedByChunkStep ==
  /\ RbcChunkGood
  /\ rbcState \in {"Init", "Chunking", "Withheld"}
  /\ headerSeen
  /\ headerSeen' = headerSeen
  /\ digestValid'
  /\ chunkCount' = IF chunkCount < MaxChunks THEN chunkCount + 1 ELSE chunkCount
  /\ chunkCount' >= chunkCount
  /\ chunkCount' <= MaxChunks
  /\ readyVotes' = readyVotes
  /\ committed' = committed
  /\ gst' = gst

RbcEvidenceAdvancedByReadyStep ==
  /\ RbcReadyGood
  /\ rbcState \in {"ChunksComplete", "ReadyPartial", "ReadyQuorum"}
  /\ headerSeen
  /\ digestValid
  /\ headerSeen' = headerSeen
  /\ digestValid' = digestValid
  /\ chunkCount' = chunkCount
  /\ readyVotes < N
  /\ readyVotes' = readyVotes + 1
  /\ readyVotes' <= N
  /\ committed' = committed
  /\ gst' = gst

RbcEvidenceCorruptedByFaultStep ==
  /\ ByzantineFault
  /\ rbcState \in {"Init", "Chunking", "ChunksComplete", "ReadyPartial", "ReadyQuorum"}
  /\ rbcState' = "Corrupted"
  /\ digestValid' = FALSE
  /\ chunkCount' = chunkCount
  /\ readyVotes' = readyVotes
  /\ headerSeen' = headerSeen
  /\ committed' = committed
  /\ gst' = gst

RbcEvidenceOnlyChangesByProtocolOrFaultStep ==
  (\/ headerSeen' # headerSeen
   \/ digestValid' # digestValid
   \/ chunkCount' # chunkCount
   \/ readyVotes' # readyVotes) =>
    \/ RbcEvidenceInitByProposalStep
    \/ RbcEvidenceRepairByInitStep
    \/ RbcEvidenceAdvancedByChunkStep
    \/ RbcEvidenceAdvancedByReadyStep
    \/ RbcEvidenceCorruptedByFaultStep

RbcHeaderInstallationOnlyByProposalOrInitStep ==
  (/\ ~headerSeen
   /\ headerSeen') =>
    \/ /\ RbcEvidenceInitByProposalStep
       /\ HonestPropose
       /\ rbcState = "Idle"
       /\ rbcState' = "Init"
       /\ phase = "Propose"
       /\ phase' = "Prepare"
       /\ ~committed
       /\ ~committed'
       /\ digestValid'
       /\ chunkCount' = 0
       /\ readyVotes' = 0
    \/ /\ RbcEvidenceRepairByInitStep
       /\ RbcInit
       /\ rbcState \in {"Idle", "Withheld", "Corrupted"}
       /\ rbcState' = "Init"
       /\ headerSeen'
       /\ digestValid'
       /\ chunkCount' = 0
       /\ readyVotes' = 0
       /\ committed' = committed
       /\ gst' = gst

RbcDigestInstallationOnlyByProposalInitOrChunkStep ==
  (/\ ~digestValid
   /\ digestValid') =>
    \/ /\ RbcEvidenceInitByProposalStep
       /\ HonestPropose
       /\ rbcState = "Idle"
       /\ rbcState' = "Init"
       /\ phase = "Propose"
       /\ phase' = "Prepare"
       /\ ~committed
       /\ ~committed'
       /\ headerSeen'
       /\ chunkCount' = 0
       /\ readyVotes' = 0
    \/ /\ RbcEvidenceRepairByInitStep
       /\ RbcInit
       /\ rbcState \in {"Idle", "Withheld", "Corrupted"}
       /\ rbcState' = "Init"
       /\ headerSeen'
       /\ digestValid'
       /\ chunkCount' = 0
       /\ readyVotes' = 0
       /\ committed' = committed
       /\ gst' = gst
    \/ /\ RbcEvidenceAdvancedByChunkStep
       /\ RbcChunkGood
       /\ rbcState \in {"Init", "Chunking", "Withheld"}
       /\ headerSeen
       /\ headerSeen' = headerSeen
       /\ digestValid'
       /\ chunkCount' = IF chunkCount < MaxChunks THEN chunkCount + 1 ELSE chunkCount
       /\ chunkCount' >= chunkCount
       /\ chunkCount' <= MaxChunks
       /\ readyVotes' = readyVotes
       /\ committed' = committed
       /\ gst' = gst

GstElapsedStepOnlySetsGst ==
  GstElapsed =>
    /\ ~gst
    /\ gst' = TRUE
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ committed' = committed

GstOnlyChangesByElapsedStep ==
  (gst' # gst) =>
    /\ GstElapsed
    /\ ~gst
    /\ gst' = TRUE
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ committed' = committed

GstMonotonicStep ==
  gst => gst'

ViewMonotonicStep ==
  view' >= view

CommitViewMonotonicStep ==
  commitView' >= commitView

CommitEvidenceMonotonicStep ==
  /\ commitEvidenceVotes' >= commitEvidenceVotes
  /\ commitEvidenceStake' >= commitEvidenceStake

CommittedConsensusStateStableStep ==
  committed =>
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ committed' = committed

CommittedViewWitnessStaysAtCommittedViewStep ==
  committed =>
    /\ CommittedConsensusStateStableStep
    /\ CommittedPhaseMatchesFinality
    /\ CommitViewMatchesFinality
    /\ CommitViewDoesNotLeadCurrentView
    /\ committed'
    /\ phase = "Committed"
    /\ phase' = "Committed"
    /\ view' = view
    /\ commitView' = commitView
    /\ commitView = view
    /\ commitView' = view'

CommittedOnlyGstObservationCanMoveStep ==
  committed =>
    /\ ~HonestPropose
    /\ ~HonestPrepareVote
    /\ ~HonestCommitVote
    /\ ~ByzantineEquivocateCommit
    /\ ~TimeoutTick
    /\ ~HonestNewViewVote
    /\ ~RbcInit
    /\ ~RbcChunkGood
    /\ ~RbcReadyGood
    /\ ~RbcDeliverGood
    /\ ~ByzantineFault
    /\ (gst' # gst <=> GstElapsed)
    /\ (GstElapsed => /\ ~gst
                      /\ gst'
                      /\ committed' = committed)

CommittedGstStateStableStep ==
  (committed /\ gst) =>
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ committed' = committed
    /\ gst' = gst

CommittedGstDisablesEveryAction ==
  (committed /\ gst) =>
    /\ ~HonestProposeEnabled
    /\ ~HonestPrepareVoteEnabled
    /\ ~HonestCommitVoteEnabled
    /\ ~ByzantineCommitVoteEnabled
    /\ ~HonestNewViewVoteEnabled
    /\ ~RbcInitEnabled
    /\ ~RbcChunkGoodEnabled
    /\ ~RbcReadyGoodEnabled
    /\ ~RbcDeliverGoodEnabled
    /\ ~TimeoutTickEnabled
    /\ ~ByzantineFaultEnabled
    /\ ~GstElapsedEnabled
    /\ ~PostGstProgressEnabled

CommittedPreGstOnlyGstElapsedCanMoveStep ==
  (/\ committed
   /\ ~gst
   /\ vars' # vars) =>
    /\ CommittedConsensusStateStableStep
    /\ CommittedOnlyGstObservationCanMoveStep
    /\ CommittedViewWitnessStaysAtCommittedViewStep
    /\ GstElapsed
    /\ GstElapsedStepOnlySetsGst
    /\ committed'
    /\ gst'
    /\ phase' = "Committed"
    /\ view' = view
    /\ commitView' = commitView
    /\ commitView' = view'
    /\ FinalityCertificateStackPresent
    /\ FinalityCertificateStackPresent'
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ CommitViewMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitImpliesLiveVoteQuorum'
    /\ CommitImpliesLiveStakeQuorum'
    /\ CommitImpliesHonestSupport'
    /\ CommitImpliesDelivered'
    /\ CommitImpliesRbcEvidence'
    /\ FinalityClearsNewViewHandoff'
    /\ CommitDisablesProgressActions'
    /\ CommitDisablesByzantineCommitVote'
    /\ CommitViewDoesNotLeadCurrentView'
    /\ CommittedGstDisablesEveryAction'
    /\ ~GstElapsedEnabled'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~TimeoutTickEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ ~PostGstProgressEnabled'

CommittedPreGstNextOnlyGstElapsedStep ==
  (/\ committed
   /\ ~gst) =>
    /\ CommittedPreGstOnlyEnablesGstElapsed
    /\ (Next <=> GstElapsed)
    /\ (Next =>
          /\ CommittedPreGstOnlyGstElapsedCanMoveStep
          /\ committed'
          /\ gst'
          /\ phase' = "Committed"
          /\ CommittedGstDisablesEveryAction')

CommittedPreGstSpecStepStuttersOrObservesGstStep ==
  (/\ committed
   /\ ~gst
   /\ [Next]_vars) =>
    \/ /\ vars' = vars
       /\ ~Next
       /\ ~GstElapsed
       /\ committed'
       /\ ~gst'
       /\ CommittedPreGstOnlyEnablesGstElapsed
       /\ CommittedPreGstOnlyEnablesGstElapsed'
    \/ /\ Next
       /\ GstElapsed
       /\ vars' # vars
       /\ CommittedPreGstNextOnlyGstElapsedStep
       /\ CommittedPreGstOnlyGstElapsedCanMoveStep
       /\ committed'
       /\ gst'
       /\ CommittedGstDisablesEveryAction'

CommittedGstRejectsNextStep ==
  (committed /\ gst) => ~Next

CommittedGstSpecStepOnlyStuttersStep ==
  (/\ committed
   /\ gst
   /\ [Next]_vars) =>
    /\ vars' = vars
    /\ ~Next
    /\ ~GstElapsed
    /\ CommittedGstRejectsNextStep
    /\ CommittedGstStateStableStep
    /\ CommittedGstDisablesEveryAction
    /\ CommittedGstDisablesEveryAction'
    /\ committed'
    /\ gst'
    /\ FinalityCertificateStackPresent
    /\ FinalityCertificateStackPresent'
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ CommitViewMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitImpliesLiveVoteQuorum'
    /\ CommitImpliesLiveStakeQuorum'
    /\ CommitImpliesHonestSupport'
    /\ CommitImpliesDelivered'
    /\ CommitImpliesRbcEvidence'
    /\ FinalityClearsNewViewHandoff'
    /\ CommitDisablesProgressActions'
    /\ CommitDisablesByzantineCommitVote'
    /\ CommitViewDoesNotLeadCurrentView'

CommittedSpecNonStutteringOnlyObservesGstStep ==
  (/\ committed
   /\ [Next]_vars
   /\ vars' # vars) =>
    /\ ~gst
    /\ Next
    /\ GstElapsed
    /\ CommittedPreGstSpecStepStuttersOrObservesGstStep
    /\ CommittedPreGstNextOnlyGstElapsedStep
    /\ CommittedPreGstOnlyGstElapsedCanMoveStep
    /\ CommittedConsensusStateStableStep
    /\ CommittedOnlyGstObservationCanMoveStep
    /\ CommittedViewWitnessStaysAtCommittedViewStep
    /\ committed'
    /\ gst'
    /\ phase' = "Committed"
    /\ view' = view
    /\ commitView' = commitView
    /\ commitView' = view'
    /\ FinalityCertificateStackPresent
    /\ FinalityCertificateStackPresent'
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ CommitViewMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitImpliesLiveVoteQuorum'
    /\ CommitImpliesLiveStakeQuorum'
    /\ CommitImpliesHonestSupport'
    /\ CommitImpliesDelivered'
    /\ CommitImpliesRbcEvidence'
    /\ FinalityClearsNewViewHandoff'
    /\ CommitDisablesProgressActions'
    /\ CommitDisablesByzantineCommitVote'
    /\ CommitViewDoesNotLeadCurrentView'
    /\ CommittedGstDisablesEveryAction'
    /\ ~GstElapsedEnabled'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~TimeoutTickEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ ~PostGstProgressEnabled'

CommittedSpecStepStuttersOrObservesGstStep ==
  (/\ committed
   /\ [Next]_vars) =>
    \/ /\ vars' = vars
       /\ committed'
       /\ gst' = gst
       /\ phase' = "Committed"
       /\ (gst =>
             /\ CommittedGstSpecStepOnlyStuttersStep
             /\ CommittedGstDisablesEveryAction')
       /\ (~gst =>
             /\ CommittedPreGstSpecStepStuttersOrObservesGstStep
             /\ CommittedPreGstOnlyEnablesGstElapsed')
    \/ /\ vars' # vars
       /\ CommittedSpecNonStutteringOnlyObservesGstStep
       /\ ~gst
       /\ committed'
       /\ gst'
       /\ phase' = "Committed"
       /\ CommittedGstDisablesEveryAction'

CommittedSpecStepPreservesFinalityStackStep ==
  (/\ committed
   /\ [Next]_vars) =>
    /\ CommittedSpecStepStuttersOrObservesGstStep
    /\ committed'
    /\ phase = "Committed"
    /\ phase' = "Committed"
    /\ FinalityCertificateStackPresent
    /\ FinalityCertificateStackPresent'
    /\ FinalityCertificateStackComplete
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommittedPhaseMatchesFinality
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality
    /\ CommitCertificateMatchesFinality'
    /\ CommitViewMatchesFinality
    /\ CommitViewMatchesFinality'
    /\ LiveCommitGateMatchesFinality
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitImpliesLiveVoteQuorum
    /\ CommitImpliesLiveVoteQuorum'
    /\ CommitImpliesLiveStakeQuorum
    /\ CommitImpliesLiveStakeQuorum'
    /\ CommitImpliesHonestSupport
    /\ CommitImpliesHonestSupport'
    /\ CommitImpliesDelivered
    /\ CommitImpliesDelivered'
    /\ CommitImpliesRbcEvidence
    /\ CommitImpliesRbcEvidence'
    /\ FinalityClearsNewViewHandoff
    /\ FinalityClearsNewViewHandoff'
    /\ CommitDisablesProgressActions
    /\ CommitDisablesProgressActions'
    /\ CommitDisablesByzantineCommitVote
    /\ CommitDisablesByzantineCommitVote'
    /\ CommitViewDoesNotLeadCurrentView
    /\ CommitViewDoesNotLeadCurrentView'

CommittedSpecStepOnlyChangesGstFlagStep ==
  (/\ committed
   /\ [Next]_vars) =>
    /\ CommittedSpecStepPreservesFinalityStackStep
    /\ CommittedConsensusStateStableStep
    /\ CommittedOnlyGstObservationCanMoveStep
    /\ CommittedViewWitnessStaysAtCommittedViewStep
    /\ committed'
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ (gst' # gst <=> (/\ ~gst
                         /\ GstElapsed
                         /\ CommittedSpecNonStutteringOnlyObservesGstStep))
    /\ (gst' = gst => vars' = vars)
    /\ (gst' # gst =>
          /\ vars' # vars
          /\ gst'
          /\ CommittedGstDisablesEveryAction')

CommittedSpecStepNeverRunsProtocolActionsStep ==
  (/\ committed
   /\ [Next]_vars) =>
    /\ CommittedSpecStepOnlyChangesGstFlagStep
    /\ CommittedOnlyGstObservationCanMoveStep
    /\ ~HonestPropose
    /\ ~HonestPrepareVote
    /\ ~HonestCommitVote
    /\ ~ByzantineEquivocateCommit
    /\ ~TimeoutTick
    /\ ~HonestNewViewVote
    /\ ~RbcInit
    /\ ~RbcChunkGood
    /\ ~RbcReadyGood
    /\ ~RbcDeliverGood
    /\ ~ByzantineFault
    /\ (Next <=> GstElapsed)
    /\ (Next =>
          /\ GstElapsed
          /\ ~gst
          /\ gst'
          /\ CommittedSpecNonStutteringOnlyObservesGstStep)

CommittedSpecStepKeepsProgressActionsQuiescentStep ==
  (/\ committed
   /\ [Next]_vars) =>
    /\ CommittedSpecStepNeverRunsProtocolActionsStep
    /\ CommitDisablesProgressActions
    /\ CommitDisablesProgressActions'
    /\ ~PostGstProgressEnabled
    /\ ~PostGstProgressEnabled'
    /\ ~HonestProposeEnabled
    /\ ~HonestPrepareVoteEnabled
    /\ ~HonestCommitVoteEnabled
    /\ ~ByzantineCommitVoteEnabled
    /\ ~HonestNewViewVoteEnabled
    /\ ~RbcInitEnabled
    /\ ~RbcChunkGoodEnabled
    /\ ~RbcReadyGoodEnabled
    /\ ~RbcDeliverGoodEnabled
    /\ ~TimeoutTickEnabled
    /\ ~ByzantineFaultEnabled
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~TimeoutTickEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ ~HonestPropose
    /\ ~HonestPrepareVote
    /\ ~HonestCommitVote
    /\ ~ByzantineEquivocateCommit
    /\ ~TimeoutTick
    /\ ~HonestNewViewVote
    /\ ~RbcInit
    /\ ~RbcChunkGood
    /\ ~RbcReadyGood
    /\ ~RbcDeliverGood
    /\ ~ByzantineFault
    /\ (vars' # vars =>
          /\ GstElapsed
          /\ CommittedSpecNonStutteringOnlyObservesGstStep
          /\ CommitDisablesProgressActions'
          /\ ~PostGstProgressEnabled')

CommitArtifactsInstallOnlyAtFinalityStep ==
  (\/ commitView' # commitView
   \/ commitEvidenceVotes' # commitEvidenceVotes
   \/ commitEvidenceStake' # commitEvidenceStake) =>
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ ~committed
    /\ committed'
    /\ view' = view
    /\ commitView = 0
    /\ commitView' = view
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum
    /\ newViewVotes = 0
    /\ newViewVotes' = 0

CommitArtifactsOnlyChangeByFinalitySourceStep ==
  (\/ commitView' # commitView
   \/ commitEvidenceVotes' # commitEvidenceVotes
   \/ commitEvidenceStake' # commitEvidenceStake) =>
    \/ PhaseFinalizesByHonestCommitVoteStep
    \/ PhaseFinalizesByByzantineCommitVoteStep
    \/ PhaseFinalizesByRbcDeliverStep

FinalityLatchSetInstallsCompleteStackStep ==
  (~committed /\ committed') =>
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ view' = view
    /\ newViewVotes' = 0
    /\ prepareVotes' >= CommitQuorum
    /\ commitVotesHonest' + commitVotesByz' >= CommitQuorum
    /\ commitVotesHonest' >= HonestCommitSupportThreshold
    /\ stakeSigned' >= StakeQuorum
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum
    /\ rbcState' = "Delivered"
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ commitView' = view'
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)

FinalityLatchAndArtifactsCoupledStep ==
  (committed' # committed) <=>
    (\/ commitView' # commitView
     \/ commitEvidenceVotes' # commitEvidenceVotes
     \/ commitEvidenceStake' # commitEvidenceStake)

CommittedPhaseEntryInstallsCompleteStackStep ==
  (phase # "Committed" /\ phase' = "Committed") =>
    /\ phase = "CommitVote"
    /\ ~committed
    /\ committed'
    /\ view' = view
    /\ newViewVotes' = 0
    /\ prepareVotes' >= CommitQuorum
    /\ commitVotesHonest' + commitVotesByz' >= CommitQuorum
    /\ commitVotesHonest' >= HonestCommitSupportThreshold
    /\ stakeSigned' >= StakeQuorum
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum
    /\ rbcState' = "Delivered"
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ commitView' = view'
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)

CommittedPhaseEntryMatchesFinalityLatchStep ==
  (phase # "Committed" /\ phase' = "Committed") <=>
    (~committed /\ committed')

FinalityLatchChangeEntersCommittedPhaseStep ==
  (committed' # committed) =>
    /\ ~committed
    /\ committed'
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ view' = view

FinalityLatchChangeMatchesLiveCommitGateCrossingStep ==
  (committed' # committed) <=>
    /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, rbcState)
    /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')

CommitCertificateWitnessesInstallWithFinalityLatchStep ==
  (committed' # committed) <=>
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' # commitEvidenceVotes
    /\ commitEvidenceStake' # commitEvidenceStake
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum

CommitCertificateWitnessComponentsChangeTogetherStep ==
  (commitEvidenceVotes' # commitEvidenceVotes) <=>
    (commitEvidenceStake' # commitEvidenceStake)

CommitViewWitnessChangesOnlyOnNonzeroFinalityStep ==
  (commitView' # commitView) <=>
    /\ ~committed
    /\ committed'
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ view' = view
    /\ view' # 0
    /\ commitView = 0
    /\ commitView' = view'

CommitViewWitnessInstallsWithFinalityLatchStep ==
  (~committed /\ committed') =>
    /\ commitView = 0
    /\ view' = view
    /\ commitView' = view'
    /\ ((view' = 0) <=> (commitView' = commitView))
    /\ ((view' # 0) <=> (commitView' # commitView))
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)

FinalityLatchNeverCarriesNewViewHandoffStep ==
  (~committed /\ committed') =>
    /\ phase # "NewView"
    /\ phase' # "NewView"
    /\ ~HonestNewViewVoteEnabled
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes

FinalityLatchSourceIsCommitOrDeliveryStep ==
  (~committed /\ committed') =>
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ gst' = gst
    /\ (\/ HonestCommitVote
        \/ ByzantineEquivocateCommit
        \/ RbcDeliverGood)
    /\ ~HonestPropose
    /\ ~HonestPrepareVote
    /\ ~TimeoutTick
    /\ ~HonestNewViewVote
    /\ ~RbcInit
    /\ ~RbcChunkGood
    /\ ~RbcReadyGood
    /\ ~ByzantineFault
    /\ ~GstElapsed

FinalityLatchChangePreservesGstStep ==
  (~committed /\ committed') =>
    /\ gst' = gst
    /\ FinalityLatchSourceIsCommitOrDeliveryStep

FinalityLatchSourceEffectsAreExactStep ==
  (~committed /\ committed') =>
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ view' = view
    /\ commitView = 0
    /\ commitView' = view
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ chunkCount' = chunkCount
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ \/ /\ HonestCommitVote
          /\ ~ByzantineEquivocateCommit
          /\ ~RbcDeliverGood
          /\ commitVotesHonest' = commitVotesHonest + 1
          /\ commitVotesByz' = commitVotesByz
          /\ stakeSigned' = stakeSigned + StakePerHonestVote
          /\ commitEvidenceVotes' = commitVotesHonest + 1 + commitVotesByz
          /\ commitEvidenceStake' = stakeSigned + StakePerHonestVote
          /\ rbcState = "Delivered"
       \/ /\ ~HonestCommitVote
          /\ ByzantineEquivocateCommit
          /\ ~RbcDeliverGood
          /\ commitVotesHonest' = commitVotesHonest
          /\ commitVotesByz' = commitVotesByz + 1
          /\ stakeSigned' = stakeSigned + StakePerByzVote
          /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz + 1
          /\ commitEvidenceStake' = stakeSigned + StakePerByzVote
          /\ rbcState = "Delivered"
       \/ /\ ~HonestCommitVote
          /\ ~ByzantineEquivocateCommit
          /\ RbcDeliverGood
          /\ commitVotesHonest' = commitVotesHonest
          /\ commitVotesByz' = commitVotesByz
          /\ stakeSigned' = stakeSigned
          /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz
          /\ commitEvidenceStake' = stakeSigned
          /\ rbcState = "ReadyQuorum"

FinalityLatchChangeLeavesOnlyGstElapsedGateStep ==
  (~committed /\ committed') =>
    /\ CommitDisablesProgressActions'
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (gst' => CommittedGstDisablesEveryAction')
    /\ FinalityLatchChangePreservesGstStep
    /\ FinalityLatchSourceEffectsAreExactStep

FinalityLatchSourceQuorumGatesHoldStep ==
  (~committed /\ committed') =>
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ prepareVotes >= CommitQuorum
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' + commitVotesByz' >= CommitQuorum
    /\ commitVotesHonest' >= HonestCommitSupportThreshold
    /\ stakeSigned' >= StakeQuorum
    /\ rbcState' = "Delivered"
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ (HonestCommitVote =>
          /\ HonestCommitVoteEnabled
          /\ CanCommit(
               commitVotesHonest + 1,
               commitVotesByz,
               stakeSigned + StakePerHonestVote,
               rbcState
             )
          /\ commitVotesHonest + 1 >= HonestCommitSupportThreshold
          /\ rbcState = "Delivered")
    /\ (ByzantineEquivocateCommit =>
          /\ ByzantineCommitVoteEnabled
          /\ CanCommit(
               commitVotesHonest,
               commitVotesByz + 1,
               stakeSigned + StakePerByzVote,
               rbcState
             )
          /\ commitVotesHonest >= HonestCommitSupportThreshold
          /\ rbcState = "Delivered")
    /\ (RbcDeliverGood =>
          /\ RbcDeliverGoodEnabled
          /\ CanCommit(
               commitVotesHonest,
               commitVotesByz,
               stakeSigned,
               "Delivered"
             )
          /\ commitVotesHonest >= HonestCommitSupportThreshold
          /\ rbcState = "ReadyQuorum")

CommittedPhaseEntryOnlyByFinalitySourceStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed") =>
    /\ CommittedPhaseEntryInstallsCompleteStackStep
    /\ FinalityLatchChangeEntersCommittedPhaseStep
    /\ FinalityLatchSourceIsCommitOrDeliveryStep
    /\ FinalityLatchSourceEffectsAreExactStep
    /\ FinalityLatchSourceQuorumGatesHoldStep
    /\ (\/ PhaseFinalizesByHonestCommitVoteStep
        \/ PhaseFinalizesByByzantineCommitVoteStep
        \/ PhaseFinalizesByRbcDeliverStep)

FinalityLatchChangeMatchesCertifiedSourceStackStep ==
  (committed' # committed) =>
    /\ FinalityLatchSetInstallsCompleteStackStep
    /\ FinalityLatchAndArtifactsCoupledStep
    /\ CommitArtifactsInstallOnlyAtFinalityStep
    /\ CommitArtifactsOnlyChangeByFinalitySourceStep
    /\ CommittedPhaseEntryInstallsCompleteStackStep
    /\ CommittedPhaseEntryMatchesFinalityLatchStep
    /\ CommittedPhaseEntryOnlyByFinalitySourceStep
    /\ FinalityLatchChangeEntersCommittedPhaseStep
    /\ FinalityLatchChangeMatchesLiveCommitGateCrossingStep
    /\ CommitCertificateWitnessesInstallWithFinalityLatchStep
    /\ CommitViewWitnessChangesOnlyOnNonzeroFinalityStep
    /\ CommitViewWitnessInstallsWithFinalityLatchStep
    /\ FinalityLatchNeverCarriesNewViewHandoffStep
    /\ FinalityLatchSourceIsCommitOrDeliveryStep
    /\ FinalityLatchSourceEffectsAreExactStep
    /\ FinalityLatchSourceQuorumGatesHoldStep

CommitArtifactsChangeMatchesCertifiedFinalityStackStep ==
  (\/ commitView' # commitView
   \/ commitEvidenceVotes' # commitEvidenceVotes
   \/ commitEvidenceStake' # commitEvidenceStake) =>
    /\ ~committed
    /\ committed'
    /\ committed' # committed
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ CommitArtifactsInstallOnlyAtFinalityStep
    /\ CommitArtifactsOnlyChangeByFinalitySourceStep
    /\ FinalityLatchAndArtifactsCoupledStep
    /\ FinalityLatchChangeMatchesCertifiedSourceStackStep

CommitArtifactsChangeCommitsCurrentViewStep ==
  (\/ commitView' # commitView
   \/ commitEvidenceVotes' # commitEvidenceVotes
   \/ commitEvidenceStake' # commitEvidenceStake) =>
    /\ ~committed
    /\ committed'
    /\ committed' # committed
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ view' = view
    /\ commitView = 0
    /\ commitView' = view
    /\ commitView' = view'
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ CommitArtifactsChangeMatchesCertifiedFinalityStackStep
    /\ CommitViewWitnessInstallsWithFinalityLatchStep
    /\ FinalityLatchNeverCarriesNewViewHandoffStep

CommitArtifactsChangePreservesGstStep ==
  (\/ commitView' # commitView
   \/ commitEvidenceVotes' # commitEvidenceVotes
   \/ commitEvidenceStake' # commitEvidenceStake) =>
    /\ gst' = gst
    /\ CommitArtifactsOnlyChangeByFinalitySourceStep

CommitCertificateWitnessChangeMatchesCertifiedFinalityStackStep ==
  (\/ commitEvidenceVotes' # commitEvidenceVotes
   \/ commitEvidenceStake' # commitEvidenceStake) =>
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' # commitEvidenceVotes
    /\ commitEvidenceStake' # commitEvidenceStake
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum
    /\ CommitCertificateWitnessesInstallWithFinalityLatchStep
    /\ CommitArtifactsChangeMatchesCertifiedFinalityStackStep

CommitCertificateWitnessChangeInstallsCommitViewWitnessStep ==
  (\/ commitEvidenceVotes' # commitEvidenceVotes
   \/ commitEvidenceStake' # commitEvidenceStake) =>
    /\ ~committed
    /\ committed'
    /\ committed' # committed
    /\ commitView = 0
    /\ view' = view
    /\ commitView' = view'
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ CommitViewWitnessInstallsWithFinalityLatchStep
    /\ CommitCertificateWitnessChangeMatchesCertifiedFinalityStackStep

CommitCertificateWitnessChangePreservesGstStep ==
  (\/ commitEvidenceVotes' # commitEvidenceVotes
   \/ commitEvidenceStake' # commitEvidenceStake) =>
    /\ gst' = gst
    /\ CommitCertificateWitnessChangeMatchesCertifiedFinalityStackStep
    /\ CommitArtifactsChangePreservesGstStep

CommitViewWitnessChangeMatchesCertifiedFinalityStackStep ==
  (commitView' # commitView) =>
    /\ view' # 0
    /\ commitView = 0
    /\ commitView' = view'
    /\ viewEvidenceVotes' >= ViewQuorum
    /\ CommitViewWitnessChangesOnlyOnNonzeroFinalityStep
    /\ CommitViewWitnessInstallsWithFinalityLatchStep
    /\ CommitArtifactsChangeMatchesCertifiedFinalityStackStep

CommitViewWitnessChangeInstallsCommitCertificateWitnessesStep ==
  (commitView' # commitView) =>
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' # commitEvidenceVotes
    /\ commitEvidenceStake' # commitEvidenceStake
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum
    /\ CommitCertificateWitnessesInstallWithFinalityLatchStep
    /\ CommitViewWitnessChangeMatchesCertifiedFinalityStackStep

CommitViewWitnessChangePreservesGstStep ==
  (commitView' # commitView) =>
    /\ gst' = gst
    /\ CommitViewWitnessChangeMatchesCertifiedFinalityStackStep
    /\ CommitCertificateWitnessChangePreservesGstStep

CommittedPhaseEntryMatchesCertifiedFinalityStackStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed") =>
    /\ ~committed
    /\ committed'
    /\ committed' # committed
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ CommittedPhaseEntryInstallsCompleteStackStep
    /\ CommittedPhaseEntryMatchesFinalityLatchStep
    /\ CommittedPhaseEntryOnlyByFinalitySourceStep
    /\ FinalityLatchChangeMatchesCertifiedSourceStackStep

CommittedPhaseEntryInstallsCommitCertificateWitnessesStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed") =>
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' # commitEvidenceVotes
    /\ commitEvidenceStake' # commitEvidenceStake
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum
    /\ CommitCertificateWitnessesInstallWithFinalityLatchStep
    /\ CommittedPhaseEntryMatchesCertifiedFinalityStackStep

CommittedPhaseEntryMatchesCommitCertificateWitnessChangeStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed") <=>
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' # commitEvidenceVotes
    /\ commitEvidenceStake' # commitEvidenceStake
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum

CommittedPhaseEntryInstallsCommitViewWitnessStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed") =>
    /\ commitView = 0
    /\ view' = view
    /\ commitView' = view'
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ CommitViewWitnessInstallsWithFinalityLatchStep
    /\ CommittedPhaseEntryMatchesCertifiedFinalityStackStep

CommittedPhaseEntryMatchesLiveCommitGateCrossingStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed") <=>
    /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, rbcState)
    /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')

CommittedPhaseEntryMatchesCommitArtifactsChangeStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed") <=>
    (\/ commitView' # commitView
     \/ commitEvidenceVotes' # commitEvidenceVotes
     \/ commitEvidenceStake' # commitEvidenceStake)

CommittedPhaseEntryMatchesExactFinalitySourceEffectsStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed") =>
    /\ CommittedPhaseEntryMatchesFinalityLatchStep
    /\ FinalityLatchSourceIsCommitOrDeliveryStep
    /\ FinalityLatchSourceEffectsAreExactStep
    /\ FinalityLatchSourceQuorumGatesHoldStep

CommittedPhaseEntryNeverCarriesNewViewHandoffStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed") =>
    /\ CommittedPhaseEntryMatchesFinalityLatchStep
    /\ FinalityLatchNeverCarriesNewViewHandoffStep
    /\ phase # "NewView"
    /\ phase' # "NewView"
    /\ ~HonestNewViewVoteEnabled
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes

CommittedPhaseEntryCommitsCurrentViewStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed") =>
    /\ phase = "CommitVote"
    /\ ~committed
    /\ committed'
    /\ view' = view
    /\ commitView = 0
    /\ commitView' = view
    /\ commitView' = view'
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ CommittedPhaseEntryInstallsCommitViewWitnessStep
    /\ CommittedPhaseEntryNeverCarriesNewViewHandoffStep

CommittedPhaseEntryPreservesGstStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed") =>
    /\ gst' = gst
    /\ CommittedPhaseEntryOnlyByFinalitySourceStep

CommittedPhaseEntryDisablesProgressActionsStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed") =>
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~TimeoutTickEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ ~PostGstProgressEnabled'

CommittedPhaseEntryLeavesOnlyGstElapsedGateStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed") =>
    /\ CommittedPhaseEntryDisablesProgressActionsStep
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (gst' => CommittedGstDisablesEveryAction')

ViewEvidenceMatchesActiveView ==
  /\ (view = 0 => viewEvidenceVotes = 0)
  /\ (phase = "NewView" => viewEvidenceVotes = 0)
  /\ ((view > 0 /\ phase # "NewView") => viewEvidenceVotes >= ViewQuorum)

ViewEvidenceWitnessRequiresNonzeroActiveView ==
  viewEvidenceVotes > 0 =>
    /\ view > 0
    /\ phase # "NewView"
    /\ viewEvidenceVotes >= ViewQuorum

NewViewPhaseBelowQuorum ==
  phase = "NewView" => newViewVotes < ViewQuorum

LiveNewViewVotesStayInHandoff ==
  newViewVotes > 0 =>
    /\ phase \in {"NewView", "Propose"}
    /\ ~committed
    /\ (phase = "Propose" => viewEvidenceVotes >= ViewQuorum)

HonestProposeGateMatchesHandoffEvidence ==
  HonestProposeEnabled <=>
    /\ phase = "Propose"
    /\ prepareVotes = 0
    /\ commitVotesHonest = 0
    /\ commitVotesByz = 0
    /\ stakeSigned = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ (newViewVotes = 0 \/ newViewVotes >= ViewQuorum)

HonestProposeStepStartsPrepareAndRbc ==
  HonestPropose =>
    /\ phase = "Propose"
    /\ phase' = "Prepare"
    /\ view' = view
    /\ ~committed
    /\ ~committed'
    /\ prepareVotes = 0
    /\ prepareVotes' = 0
    /\ newViewVotes' = 0
    /\ commitVotesHonest = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ (newViewVotes = 0 \/ newViewVotes >= ViewQuorum)
    /\ rbcState' = IF rbcState = "Idle" THEN "Init" ELSE rbcState
    /\ headerSeen' = IF rbcState = "Idle" THEN TRUE ELSE headerSeen
    /\ digestValid' = IF rbcState = "Idle" THEN TRUE ELSE digestValid
    /\ chunkCount' = IF rbcState = "Idle" THEN 0 ELSE chunkCount
    /\ readyVotes' = IF rbcState = "Idle" THEN 0 ELSE readyVotes

HonestProposeStepStartsPrepareVoteHandoff ==
  HonestPropose =>
    /\ phase' = "Prepare"
    /\ view' = view
    /\ prepareVotes' = 0
    /\ newViewVotes' = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~committed'
    /\ (view' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ HonestPrepareVoteEnabled'
    /\ ~HonestProposeEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'

NewViewVoteGateMatchesFreshViewEvidence ==
  HonestNewViewVoteEnabled <=>
    /\ phase = "NewView"
    /\ view > 0
    /\ viewEvidenceVotes = 0
    /\ newViewVotes < ViewQuorum
    /\ newViewVotes < N - F
    /\ prepareVotes = 0
    /\ commitVotesHonest = 0
    /\ commitVotesByz = 0
    /\ stakeSigned = 0

NewViewVoteQuorumGateMatchesNextEvidence ==
  (HonestNewViewVoteEnabled /\ newViewVotes + 1 >= ViewQuorum) <=>
    /\ phase = "NewView"
    /\ view > 0
    /\ viewEvidenceVotes = 0
    /\ newViewVotes < ViewQuorum
    /\ newViewVotes + 1 >= ViewQuorum
    /\ newViewVotes < N - F
    /\ prepareVotes = 0
    /\ commitVotesHonest = 0
    /\ commitVotesByz = 0
    /\ stakeSigned = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0

NewViewVoteQuorumStepInstallsViewEvidence ==
  (HonestNewViewVote /\ newViewVotes + 1 >= ViewQuorum) =>
    /\ phase = "NewView"
    /\ phase' = "Propose"
    /\ view > 0
    /\ view' = view
    /\ ~committed
    /\ ~committed'
    /\ newViewVotes < ViewQuorum
    /\ newViewVotes < N - F
    /\ newViewVotes' = newViewVotes + 1
    /\ newViewVotes' >= ViewQuorum
    /\ viewEvidenceVotes = 0
    /\ viewEvidenceVotes' = newViewVotes + 1
    /\ viewEvidenceVotes' >= ViewQuorum
    /\ prepareVotes = 0
    /\ prepareVotes' = 0
    /\ commitVotesHonest = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid

NewViewVoteQuorumStepStartsProposalHandoff ==
  (HonestNewViewVote /\ newViewVotes + 1 >= ViewQuorum) =>
    /\ phase' = "Propose"
    /\ view' = view
    /\ view' > 0
    /\ viewEvidenceVotes' = newViewVotes'
    /\ viewEvidenceVotes' >= ViewQuorum
    /\ HonestProposeEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'

NewViewVotePendingGateMatchesMissingNextEvidence ==
  (HonestNewViewVoteEnabled /\ newViewVotes + 1 < ViewQuorum) <=>
    /\ phase = "NewView"
    /\ view > 0
    /\ viewEvidenceVotes = 0
    /\ newViewVotes < ViewQuorum
    /\ newViewVotes + 1 < ViewQuorum
    /\ newViewVotes < N - F
    /\ prepareVotes = 0
    /\ commitVotesHonest = 0
    /\ commitVotesByz = 0
    /\ stakeSigned = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0

NewViewVotePendingStepPreservesPreProposalArtifacts ==
  (HonestNewViewVote /\ newViewVotes + 1 < ViewQuorum) =>
    /\ phase = "NewView"
    /\ phase' = "NewView"
    /\ view > 0
    /\ view' = view
    /\ ~committed
    /\ ~committed'
    /\ newViewVotes < ViewQuorum
    /\ newViewVotes < N - F
    /\ newViewVotes' = newViewVotes + 1
    /\ newViewVotes' < ViewQuorum
    /\ viewEvidenceVotes = 0
    /\ viewEvidenceVotes' = 0
    /\ prepareVotes = 0
    /\ prepareVotes' = 0
    /\ commitVotesHonest = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid

ViewEvidenceIsCompleteOrEmpty ==
  viewEvidenceVotes = 0 \/ viewEvidenceVotes >= ViewQuorum

PreCommitPhasesHaveNoCommitVotes ==
  phase \in {"NewView", "Propose", "Prepare"} =>
    /\ commitVotesHonest = 0
    /\ commitVotesByz = 0
    /\ stakeSigned = 0

PrePreparePhasesHaveNoPrepareVotes ==
  phase \in {"NewView", "Propose"} => prepareVotes = 0

LivePrepareVotesStayInHandoff ==
  prepareVotes > 0 =>
    /\ phase \in {"Prepare", "CommitVote", "Committed"}
    /\ (phase # "Prepare" => prepareVotes >= CommitQuorum)

PrepareVoteGateMatchesProposalEvidence ==
  HonestPrepareVoteEnabled <=>
    /\ phase = "Prepare"
    /\ prepareVotes < CommitQuorum
    /\ prepareVotes < N - F
    /\ newViewVotes = 0
    /\ commitVotesHonest = 0
    /\ commitVotesByz = 0
    /\ stakeSigned = 0
    /\ ~committed
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)

PrepareVoteQuorumGateMatchesNextEvidence ==
  (HonestPrepareVoteEnabled /\ prepareVotes + 1 >= CommitQuorum) <=>
    /\ phase = "Prepare"
    /\ prepareVotes < CommitQuorum
    /\ prepareVotes + 1 >= CommitQuorum
    /\ prepareVotes < N - F
    /\ newViewVotes = 0
    /\ commitVotesHonest = 0
    /\ commitVotesByz = 0
    /\ stakeSigned = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)

PrepareVoteQuorumStepEntersCommitVote ==
  (HonestPrepareVote /\ prepareVotes + 1 >= CommitQuorum) =>
    /\ phase = "Prepare"
    /\ phase' = "CommitVote"
    /\ view' = view
    /\ ~committed
    /\ ~committed'
    /\ prepareVotes < CommitQuorum
    /\ prepareVotes < N - F
    /\ prepareVotes' = prepareVotes + 1
    /\ prepareVotes' >= CommitQuorum
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ commitVotesHonest = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid

PrepareVoteQuorumStepStartsCommitVoteHandoff ==
  (HonestPrepareVote /\ prepareVotes + 1 >= CommitQuorum) =>
    /\ phase' = "CommitVote"
    /\ view' = view
    /\ prepareVotes' >= CommitQuorum
    /\ newViewVotes' = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~committed'
    /\ (view' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ HonestCommitVoteEnabled'
    /\ (F > 0 => ByzantineCommitVoteEnabled')
    /\ (F = 0 => ~ByzantineCommitVoteEnabled')
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestProposeEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')

PrepareVotePendingGateMatchesMissingNextEvidence ==
  (HonestPrepareVoteEnabled /\ prepareVotes + 1 < CommitQuorum) <=>
    /\ phase = "Prepare"
    /\ prepareVotes < CommitQuorum
    /\ prepareVotes + 1 < CommitQuorum
    /\ prepareVotes < N - F
    /\ newViewVotes = 0
    /\ commitVotesHonest = 0
    /\ commitVotesByz = 0
    /\ stakeSigned = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)

PrepareVotePendingStepPreservesPreCommitArtifacts ==
  (HonestPrepareVote /\ prepareVotes + 1 < CommitQuorum) =>
    /\ phase = "Prepare"
    /\ phase' = "Prepare"
    /\ view' = view
    /\ ~committed
    /\ ~committed'
    /\ prepareVotes < CommitQuorum
    /\ prepareVotes < N - F
    /\ prepareVotes' = prepareVotes + 1
    /\ prepareVotes' < CommitQuorum
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ commitVotesHonest = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid

PrepareVotePendingStepKeepsPrepareVoteHandoff ==
  (HonestPrepareVote /\ prepareVotes + 1 < CommitQuorum) =>
    /\ phase' = "Prepare"
    /\ view' = view
    /\ prepareVotes' = prepareVotes + 1
    /\ prepareVotes' < CommitQuorum
    /\ newViewVotes' = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~committed'
    /\ (view' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ HonestPrepareVoteEnabled'
    /\ ~HonestProposeEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid

CommitImpliesViewQuorumEvidence ==
  committed => (commitView = 0 \/ viewEvidenceVotes >= ViewQuorum)

CommitVotePhaseRequiresPrepareQuorum ==
  phase \in {"CommitVote", "Committed"} => prepareVotes >= CommitQuorum

LiveCommitVotesRequirePrepareQuorum ==
  (commitVotesHonest + commitVotesByz > 0 \/ stakeSigned > 0) =>
    /\ phase \in {"CommitVote", "Committed"}
    /\ prepareVotes >= CommitQuorum

CommitVoteGateMatchesPrepareEvidence ==
  HonestCommitVoteEnabled <=>
    /\ phase = "CommitVote"
    /\ prepareVotes >= CommitQuorum
    /\ commitVotesHonest < N - F
    /\ commitVotesByz <= F
    /\ newViewVotes = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)

ByzantineCommitVoteGateMatchesPrepareEvidence ==
  ByzantineCommitVoteEnabled <=>
    /\ phase = "CommitVote"
    /\ prepareVotes >= CommitQuorum
    /\ commitVotesHonest <= N - F
    /\ commitVotesByz < F
    /\ newViewVotes = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)

HonestCommitVoteFinalityGateMatchesNextEvidence ==
  (HonestCommitVoteEnabled /\
    CanCommit(
      commitVotesHonest + 1,
      commitVotesByz,
      stakeSigned + StakePerHonestVote,
      rbcState
    )) <=>
    /\ phase = "CommitVote"
    /\ prepareVotes >= CommitQuorum
    /\ commitVotesHonest < N - F
    /\ commitVotesByz <= F
    /\ commitVotesHonest + 1 + commitVotesByz >= CommitQuorum
    /\ commitVotesHonest + 1 >= HonestCommitSupportThreshold
    /\ stakeSigned + StakePerHonestVote >= StakeQuorum
    /\ rbcState = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid
    /\ newViewVotes = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)

HonestCommitVoteFinalityStepInstallsCommitArtifacts ==
  (HonestCommitVote /\
    CanCommit(
      commitVotesHonest + 1,
      commitVotesByz,
      stakeSigned + StakePerHonestVote,
      rbcState
    )) =>
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ ~committed
    /\ committed'
    /\ prepareVotes >= CommitQuorum
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest < N - F
    /\ commitVotesHonest' = commitVotesHonest + 1
    /\ commitVotesByz' = commitVotesByz
    /\ commitVotesHonest + 1 + commitVotesByz >= CommitQuorum
    /\ commitVotesHonest + 1 >= HonestCommitSupportThreshold
    /\ stakeSigned + StakePerHonestVote >= StakeQuorum
    /\ stakeSigned' = stakeSigned + StakePerHonestVote
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = commitVotesHonest + 1 + commitVotesByz
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = stakeSigned + StakePerHonestVote
    /\ commitView = 0
    /\ commitView' = view
    /\ view' = view
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ rbcState = "Delivered"
    /\ rbcState' = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ readyVotes' = readyVotes
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid

HonestCommitVoteFinalityStepCompletesCommittedDelivery ==
  (HonestCommitVote /\
    CanCommit(
      commitVotesHonest + 1,
      commitVotesByz,
      stakeSigned + StakePerHonestVote,
      rbcState
    )) =>
    /\ HonestCommitVoteEnabled
    /\ PhaseFinalizesByHonestCommitVoteStep
    /\ HonestCommitVoteFinalityStepInstallsCommitArtifacts
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ ~committed
    /\ committed'
    /\ committed' # committed
    /\ prepareVotes >= CommitQuorum
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest < N - F
    /\ commitVotesHonest' = commitVotesHonest + 1
    /\ commitVotesHonest' <= N - F
    /\ commitVotesByz' = commitVotesByz
    /\ commitVotesByz' <= F
    /\ commitVotesHonest' + commitVotesByz' >= CommitQuorum
    /\ commitVotesHonest' >= HonestCommitSupportThreshold
    /\ stakeSigned' = stakeSigned + StakePerHonestVote
    /\ stakeSigned' >= StakeQuorum
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceStake' >= StakeQuorum
    /\ commitView = 0
    /\ commitView' = view
    /\ commitView' = view'
    /\ view' = view
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ rbcState = "Delivered"
    /\ rbcState' = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid
    /\ gst' = gst
    /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ FinalityCertificateStackPresent'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitDisablesProgressActions'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~TimeoutTickEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ ~PostGstProgressEnabled'

HonestCommitVotePendingGateMatchesMissingNextEvidence ==
  (HonestCommitVoteEnabled /\
    ~CanCommit(
      commitVotesHonest + 1,
      commitVotesByz,
      stakeSigned + StakePerHonestVote,
      rbcState
    )) <=>
    /\ phase = "CommitVote"
    /\ prepareVotes >= CommitQuorum
    /\ commitVotesHonest < N - F
    /\ commitVotesByz <= F
    /\ newViewVotes = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ \/ commitVotesHonest + 1 + commitVotesByz < CommitQuorum
       \/ stakeSigned + StakePerHonestVote < StakeQuorum
       \/ rbcState # "Delivered"

HonestCommitVotePendingStepPreservesPreFinalityArtifacts ==
  (HonestCommitVote /\
    ~CanCommit(
      commitVotesHonest + 1,
      commitVotesByz,
      stakeSigned + StakePerHonestVote,
      rbcState
    )) =>
    /\ phase = "CommitVote"
    /\ phase' = "CommitVote"
    /\ ~committed
    /\ ~committed'
    /\ prepareVotes >= CommitQuorum
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest < N - F
    /\ commitVotesHonest' = commitVotesHonest + 1
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned + StakePerHonestVote
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ view' = view
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ \/ commitVotesHonest + 1 + commitVotesByz < CommitQuorum
       \/ stakeSigned + StakePerHonestVote < StakeQuorum
       \/ rbcState # "Delivered"

HonestCommitVotePendingStepKeepsCommitVoteHandoff ==
  (HonestCommitVote /\
    ~CanCommit(
      commitVotesHonest + 1,
      commitVotesByz,
      stakeSigned + StakePerHonestVote,
      rbcState
    )) =>
    /\ phase' = "CommitVote"
    /\ view' = view
    /\ prepareVotes' = prepareVotes
    /\ prepareVotes' >= CommitQuorum
    /\ commitVotesHonest' = commitVotesHonest + 1
    /\ commitVotesHonest' <= N - F
    /\ commitVotesByz' = commitVotesByz
    /\ commitVotesByz' <= F
    /\ stakeSigned' = stakeSigned + StakePerHonestVote
    /\ newViewVotes' = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~committed'
    /\ (view' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ (HonestCommitVoteEnabled' <=> commitVotesHonest' < N - F)
    /\ (ByzantineCommitVoteEnabled' <=> commitVotesByz' < F)
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ \/ commitVotesHonest' + commitVotesByz' < CommitQuorum
       \/ stakeSigned' < StakeQuorum
       \/ rbcState' # "Delivered"
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid

ByzantineCommitVoteFinalityGateMatchesNextEvidence ==
  (ByzantineCommitVoteEnabled /\
    CanCommit(
      commitVotesHonest,
      commitVotesByz + 1,
      stakeSigned + StakePerByzVote,
      rbcState
    )) <=>
    /\ phase = "CommitVote"
    /\ prepareVotes >= CommitQuorum
    /\ commitVotesHonest <= N - F
    /\ commitVotesByz < F
    /\ commitVotesHonest + commitVotesByz + 1 >= CommitQuorum
    /\ commitVotesHonest >= HonestCommitSupportThreshold
    /\ stakeSigned + StakePerByzVote >= StakeQuorum
    /\ rbcState = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid
    /\ newViewVotes = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)

ByzantineCommitVoteFinalityStepInstallsCommitArtifacts ==
  (ByzantineEquivocateCommit /\
    CanCommit(
      commitVotesHonest,
      commitVotesByz + 1,
      stakeSigned + StakePerByzVote,
      rbcState
    )) =>
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ ~committed
    /\ committed'
    /\ prepareVotes >= CommitQuorum
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest <= N - F
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz < F
    /\ commitVotesByz' = commitVotesByz + 1
    /\ commitVotesHonest + commitVotesByz + 1 >= CommitQuorum
    /\ commitVotesHonest >= HonestCommitSupportThreshold
    /\ stakeSigned + StakePerByzVote >= StakeQuorum
    /\ stakeSigned' = stakeSigned + StakePerByzVote
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz + 1
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = stakeSigned + StakePerByzVote
    /\ commitView = 0
    /\ commitView' = view
    /\ view' = view
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ rbcState = "Delivered"
    /\ rbcState' = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ readyVotes' = readyVotes
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid

ByzantineCommitVoteFinalityStepCompletesCommittedDelivery ==
  (ByzantineEquivocateCommit /\
    CanCommit(
      commitVotesHonest,
      commitVotesByz + 1,
      stakeSigned + StakePerByzVote,
      rbcState
    )) =>
    /\ ByzantineCommitVoteEnabled
    /\ PhaseFinalizesByByzantineCommitVoteStep
    /\ ByzantineCommitVoteFinalityStepInstallsCommitArtifacts
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ ~committed
    /\ committed'
    /\ committed' # committed
    /\ prepareVotes >= CommitQuorum
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesHonest' <= N - F
    /\ commitVotesHonest' >= HonestCommitSupportThreshold
    /\ commitVotesByz < F
    /\ commitVotesByz' = commitVotesByz + 1
    /\ commitVotesByz' <= F
    /\ commitVotesHonest' + commitVotesByz' >= CommitQuorum
    /\ stakeSigned' = stakeSigned + StakePerByzVote
    /\ stakeSigned' >= StakeQuorum
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceStake' >= StakeQuorum
    /\ commitView = 0
    /\ commitView' = view
    /\ commitView' = view'
    /\ view' = view
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ rbcState = "Delivered"
    /\ rbcState' = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid
    /\ gst' = gst
    /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ FinalityCertificateStackPresent'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitDisablesProgressActions'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~TimeoutTickEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ ~PostGstProgressEnabled'

ByzantineCommitVotePendingGateMatchesMissingNextEvidence ==
  (ByzantineCommitVoteEnabled /\
    ~CanCommit(
      commitVotesHonest,
      commitVotesByz + 1,
      stakeSigned + StakePerByzVote,
      rbcState
    )) <=>
    /\ phase = "CommitVote"
    /\ prepareVotes >= CommitQuorum
    /\ commitVotesHonest <= N - F
    /\ commitVotesByz < F
    /\ newViewVotes = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ \/ commitVotesHonest + commitVotesByz + 1 < CommitQuorum
       \/ stakeSigned + StakePerByzVote < StakeQuorum
       \/ rbcState # "Delivered"

ByzantineCommitVotePendingStepPreservesPreFinalityArtifacts ==
  (ByzantineEquivocateCommit /\
    ~CanCommit(
      commitVotesHonest,
      commitVotesByz + 1,
      stakeSigned + StakePerByzVote,
      rbcState
    )) =>
    /\ phase = "CommitVote"
    /\ phase' = "CommitVote"
    /\ ~committed
    /\ ~committed'
    /\ prepareVotes >= CommitQuorum
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest <= N - F
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz < F
    /\ commitVotesByz' = commitVotesByz + 1
    /\ stakeSigned' = stakeSigned + StakePerByzVote
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ view' = view
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ \/ commitVotesHonest + commitVotesByz + 1 < CommitQuorum
       \/ stakeSigned + StakePerByzVote < StakeQuorum
       \/ rbcState # "Delivered"

ByzantineCommitVotePendingStepKeepsCommitVoteHandoff ==
  (ByzantineEquivocateCommit /\
    ~CanCommit(
      commitVotesHonest,
      commitVotesByz + 1,
      stakeSigned + StakePerByzVote,
      rbcState
    )) =>
    /\ phase' = "CommitVote"
    /\ view' = view
    /\ prepareVotes' = prepareVotes
    /\ prepareVotes' >= CommitQuorum
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesHonest' <= N - F
    /\ commitVotesByz' = commitVotesByz + 1
    /\ commitVotesByz' <= F
    /\ stakeSigned' = stakeSigned + StakePerByzVote
    /\ newViewVotes' = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~committed'
    /\ (view' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ (HonestCommitVoteEnabled' <=> commitVotesHonest' < N - F)
    /\ (ByzantineCommitVoteEnabled' <=> commitVotesByz' < F)
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ \/ commitVotesHonest' + commitVotesByz' < CommitQuorum
       \/ stakeSigned' < StakeQuorum
       \/ rbcState' # "Delivered"
    /\ rbcState' = rbcState
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid

LiveCommitVotesStayInCommitHandoff ==
  (commitVotesHonest + commitVotesByz > 0 \/ stakeSigned > 0) =>
    /\ phase \in {"CommitVote", "Committed"}
    /\ (phase = "Committed" =>
          /\ commitVotesHonest + commitVotesByz >= CommitQuorum
          /\ stakeSigned >= StakeQuorum)

CommitImpliesPrepareQuorum ==
  committed => prepareVotes >= CommitQuorum

CommitEvidenceMatchesVoteCounters ==
  committed =>
    /\ commitEvidenceVotes = commitVotesHonest + commitVotesByz
    /\ commitEvidenceStake = stakeSigned

CommitEvidenceIsCompleteOrEmpty ==
  \/ /\ commitEvidenceVotes = 0
     /\ commitEvidenceStake = 0
  \/ /\ commitEvidenceVotes >= CommitQuorum
     /\ commitEvidenceStake >= StakeQuorum

CommitEvidenceIsBounded ==
  /\ commitEvidenceVotes \in 0..N
  /\ commitEvidenceStake \in 0..MaxCommitEvidenceStake

VoteCountersRespectRosterBudgets ==
  /\ prepareVotes \in 0..(N - F)
  /\ commitVotesHonest \in 0..(N - F)
  /\ commitVotesByz \in 0..F
  /\ newViewVotes \in 0..(N - F)
  /\ viewEvidenceVotes \in 0..(N - F)

StakeSignedMatchesVoteCounters ==
  stakeSigned =
    (commitVotesHonest * StakePerHonestVote) +
    (commitVotesByz * StakePerByzVote)

LiveStakeSignedIsBounded ==
  stakeSigned \in 0..MaxCommitEvidenceStake

CommittedSpecStepPreservesBudgetedRbcEvidenceStep ==
  (/\ committed
   /\ [Next]_vars) =>
    /\ CommittedSpecStepKeepsProgressActionsQuiescentStep
    /\ VoteCountersRespectRosterBudgets
    /\ VoteCountersRespectRosterBudgets'
    /\ StakeSignedMatchesVoteCounters
    /\ StakeSignedMatchesVoteCounters'
    /\ LiveStakeSignedIsBounded
    /\ LiveStakeSignedIsBounded'
    /\ CommitEvidenceIsBounded
    /\ CommitEvidenceIsBounded'
    /\ CommitEvidenceMatchesVoteCounters
    /\ CommitEvidenceMatchesVoteCounters'
    /\ CommitImpliesLiveVoteQuorum
    /\ CommitImpliesLiveVoteQuorum'
    /\ CommitImpliesLiveStakeQuorum
    /\ CommitImpliesLiveStakeQuorum'
    /\ CommitImpliesHonestSupport
    /\ CommitImpliesHonestSupport'
    /\ CommitImpliesRbcEvidence
    /\ CommitImpliesRbcEvidence'
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ rbcState = "Delivered"
    /\ rbcState' = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ readyVotes' = readyVotes
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid

NoCommitEvidenceBeforeCommit ==
  ~committed =>
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0

NoCommitViewBeforeCommit ==
  ~committed => commitView = 0

DeliverImpliesEvidence ==
  rbcState = "Delivered" =>
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid

RbcDeliveredWithoutFinalityHasNoCommitCertificate ==
  (/\ rbcState = "Delivered"
   /\ ~committed) =>
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ \/ commitVotesHonest + commitVotesByz < CommitQuorum
       \/ stakeSigned < StakeQuorum

RbcProgressEvidenceMatchesState ==
  /\ (rbcState \in RbcInitializedStates =>
        /\ headerSeen
        /\ digestValid)
  /\ (rbcState \in RbcChunkCoveredStates =>
        chunkCount >= MaxChunks)
  /\ (rbcState \in RbcReadyQuorumStates =>
        readyVotes >= CommitQuorum)

RbcPartialProgressEvidenceMatchesState ==
  /\ (rbcState = "Idle" =>
        /\ ~headerSeen
        /\ ~digestValid
        /\ chunkCount = 0
        /\ readyVotes = 0)
  /\ (rbcState = "Init" =>
        /\ headerSeen
        /\ digestValid
        /\ chunkCount = 0
        /\ readyVotes = 0)
  /\ (rbcState = "Chunking" =>
        /\ headerSeen
        /\ digestValid
        /\ chunkCount > 0
        /\ chunkCount < MaxChunks
        /\ readyVotes = 0)
  /\ (rbcState = "ChunksComplete" =>
        /\ headerSeen
        /\ digestValid
        /\ chunkCount >= MaxChunks
        /\ readyVotes = 0)
  /\ (rbcState = "ReadyPartial" =>
        /\ headerSeen
        /\ digestValid
        /\ chunkCount >= MaxChunks
        /\ readyVotes > 0
        /\ readyVotes < CommitQuorum)

RbcCorruptedNeverHasValidDigest ==
  rbcState = "Corrupted" => ~digestValid

RbcCorruptedRetainsHeaderEvidence ==
  rbcState = "Corrupted" => headerSeen

RbcCorruptedHasNoFinalityArtifacts ==
  rbcState = "Corrupted" =>
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0

RbcCorruptedOnlyEnablesInitRepairProgress ==
  rbcState = "Corrupted" =>
    /\ RbcInitEnabled
    /\ ~RbcChunkGoodEnabled
    /\ ~RbcReadyGoodEnabled
    /\ ~RbcDeliverGoodEnabled
    /\ ~ByzantineFaultEnabled

RbcMissingHeaderRequiresIdle ==
  ~headerSeen => rbcState = "Idle"

RbcHeaderEvidenceRequiresNonIdle ==
  headerSeen => rbcState # "Idle"

RbcValidDigestRequiresHeader ==
  digestValid => headerSeen

RbcValidDigestRequiresActiveState ==
  digestValid => rbcState \in RbcInitializedStates

RbcChunkEvidenceRequiresHeader ==
  chunkCount > 0 => headerSeen

RbcChunkEvidenceRequiresChunkOrCorruptedState ==
  chunkCount > 0 =>
    rbcState \in {
      "Chunking",
      "ChunksComplete",
      "ReadyPartial",
      "ReadyQuorum",
      "Delivered",
      "Corrupted"
    }

RbcPartialChunkEvidenceRequiresChunkingOrCorruption ==
  (/\ chunkCount > 0
   /\ chunkCount < MaxChunks) =>
    rbcState \in {"Chunking", "Corrupted"}

RbcFullChunkCoverageRequiresCoveredOrCorruptedState ==
  chunkCount >= MaxChunks =>
    rbcState \in RbcChunkCoveredStates \cup {"Corrupted"}

RbcZeroChunkEvidenceRequiresPreChunkOrCorruption ==
  chunkCount = 0 => rbcState \in {"Idle", "Init", "Corrupted"}

RbcReadyVotesRequireChunkHeaderEvidence ==
  readyVotes > 0 =>
    /\ headerSeen
    /\ chunkCount >= MaxChunks

RbcReadyVotesRequireReadyOrCorruptedState ==
  readyVotes > 0 =>
    rbcState \in {"ReadyPartial", "ReadyQuorum", "Delivered", "Corrupted"}

RbcPartialReadyEvidenceRequiresReadyPartialOrCorruption ==
  (/\ readyVotes > 0
   /\ readyVotes < CommitQuorum) =>
    rbcState \in {"ReadyPartial", "Corrupted"}

RbcReadyQuorumEvidenceRequiresQuorumOrCorruptedState ==
  readyVotes >= CommitQuorum =>
    rbcState \in RbcReadyQuorumStates \cup {"Corrupted"}

RbcZeroReadyEvidenceRequiresPreReadyOrCorruption ==
  readyVotes = 0 =>
    rbcState \in {"Idle", "Init", "Chunking", "ChunksComplete", "Corrupted"}

RbcCounterEvidenceRequiresValidDigestOrCorruption ==
  (chunkCount > 0 \/ readyVotes > 0) =>
    \/ digestValid
    \/ rbcState = "Corrupted"

RbcInvalidDigestRequiresIdleOrCorruption ==
  ~digestValid => rbcState \in {"Idle", "Corrupted"}

ByzantineFaultGateMatchesCorruptibleRbc ==
  ByzantineFaultEnabled <=>
    /\ rbcState \in {"Init", "Chunking", "ChunksComplete", "ReadyPartial", "ReadyQuorum"}
    /\ ~committed
    /\ headerSeen
    /\ digestValid
    /\ (rbcState \in RbcChunkCoveredStates => chunkCount >= MaxChunks)
    /\ (rbcState \in RbcReadyQuorumStates => readyVotes >= CommitQuorum)
    /\ \/ ~gst
       \/ /\ ~HonestProposeEnabled
          /\ ~HonestPrepareVoteEnabled
          /\ ~HonestCommitVoteEnabled
          /\ ~HonestNewViewVoteEnabled
          /\ ~RbcInitEnabled
          /\ ~RbcChunkGoodEnabled
          /\ ~RbcReadyGoodEnabled
          /\ ~RbcDeliverGoodEnabled

ByzantineFaultStepCorruptsOnlyRbcDigest ==
  ByzantineFault =>
    /\ rbcState \in {"Init", "Chunking", "ChunksComplete", "ReadyPartial", "ReadyQuorum"}
    /\ ~committed
    /\ ~committed'
    /\ rbcState' = "Corrupted"
    /\ digestValid' = FALSE
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ gst' = gst

RbcDigestInvalidationOnlyByFaultStep ==
  (/\ digestValid
   /\ ~digestValid') =>
    /\ ByzantineFault
    /\ RbcStateCorruptedByFaultStep
    /\ RbcEvidenceCorruptedByFaultStep
    /\ ByzantineFaultStepCorruptsOnlyRbcDigest
    /\ rbcState \in {"Init", "Chunking", "ChunksComplete", "ReadyPartial", "ReadyQuorum"}
    /\ ~committed
    /\ ~committed'
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid' = FALSE
    /\ (rbcState \in RbcChunkCoveredStates => chunkCount >= MaxChunks)
    /\ (rbcState \in RbcReadyQuorumStates => readyVotes >= CommitQuorum)
    /\ rbcState' = "Corrupted"
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ gst' = gst

RbcCorruptionEntryOnlyByFaultStep ==
  (/\ rbcState # "Corrupted"
   /\ rbcState' = "Corrupted") =>
    /\ ByzantineFault
    /\ RbcStateCorruptedByFaultStep
    /\ RbcEvidenceCorruptedByFaultStep
    /\ ByzantineFaultStepCorruptsOnlyRbcDigest
    /\ rbcState \in {"Init", "Chunking", "ChunksComplete", "ReadyPartial", "ReadyQuorum"}
    /\ ~committed
    /\ ~committed'
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = FALSE
    /\ (rbcState \in RbcChunkCoveredStates => chunkCount >= MaxChunks)
    /\ (rbcState \in RbcReadyQuorumStates => readyVotes >= CommitQuorum)
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ gst' = gst

RbcInitGateMatchesRepairableState ==
  RbcInitEnabled <=>
    rbcState \in {"Idle", "Withheld", "Corrupted"}

RbcInitStepInstallsHeaderDigestEvidence ==
  RbcInit =>
    /\ rbcState \in {"Idle", "Withheld", "Corrupted"}
    /\ rbcState' = "Init"
    /\ chunkCount' = 0
    /\ readyVotes' = 0
    /\ headerSeen'
    /\ digestValid'
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ committed' = committed
    /\ gst' = gst

RbcInitStepStartsChunkOnlyHandoffStep ==
  RbcInit =>
    /\ RbcInitStepInstallsHeaderDigestEvidence
    /\ RbcChunkGoodEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'

RbcIdleExitOnlyByProposalOrInitStep ==
  (/\ rbcState = "Idle"
   /\ rbcState' # "Idle") =>
    \/ /\ RbcStateInitByProposalStep
       /\ RbcEvidenceInitByProposalStep
       /\ HonestPropose
       /\ rbcState' = "Init"
       /\ phase = "Propose"
       /\ phase' = "Prepare"
       /\ ~committed
       /\ ~committed'
       /\ headerSeen'
       /\ digestValid'
       /\ chunkCount' = 0
       /\ readyVotes' = 0
       /\ newViewVotes' = 0
       /\ view' = view
       /\ commitView' = commitView
       /\ commitVotesHonest' = commitVotesHonest
       /\ commitVotesByz' = commitVotesByz
       /\ stakeSigned' = stakeSigned
       /\ commitEvidenceVotes' = commitEvidenceVotes
       /\ commitEvidenceStake' = commitEvidenceStake
       /\ viewEvidenceVotes' = viewEvidenceVotes
       /\ gst' = gst
    \/ /\ RbcStateRepairByInitStep
       /\ RbcEvidenceRepairByInitStep
       /\ RbcInitStepInstallsHeaderDigestEvidence
       /\ RbcInit
       /\ rbcState' = "Init"
       /\ phase' = phase
       /\ view' = view
       /\ commitView' = commitView
       /\ prepareVotes' = prepareVotes
       /\ commitVotesHonest' = commitVotesHonest
       /\ commitVotesByz' = commitVotesByz
       /\ stakeSigned' = stakeSigned
       /\ commitEvidenceVotes' = commitEvidenceVotes
       /\ commitEvidenceStake' = commitEvidenceStake
       /\ newViewVotes' = newViewVotes
       /\ viewEvidenceVotes' = viewEvidenceVotes
       /\ headerSeen'
       /\ digestValid'
       /\ chunkCount' = 0
       /\ readyVotes' = 0
       /\ committed' = committed
       /\ gst' = gst

RbcInitEntryOnlyByProposalOrInitStep ==
  (/\ rbcState # "Init"
   /\ rbcState' = "Init") =>
    \/ /\ RbcStateInitByProposalStep
       /\ HonestPropose
       /\ rbcState = "Idle"
       /\ phase = "Propose"
       /\ phase' = "Prepare"
       /\ headerSeen'
       /\ digestValid'
       /\ chunkCount' = 0
       /\ readyVotes' = 0
       /\ committed' = committed
       /\ gst' = gst
    \/ /\ RbcStateRepairByInitStep
       /\ RbcInitStepInstallsHeaderDigestEvidence
       /\ rbcState \in {"Idle", "Withheld", "Corrupted"}
       /\ phase' = phase
       /\ view' = view
       /\ headerSeen'
       /\ digestValid'
       /\ chunkCount' = 0
       /\ readyVotes' = 0
       /\ committed' = committed
       /\ gst' = gst

RbcCorruptionExitOnlyByInitStep ==
  (/\ rbcState = "Corrupted"
   /\ rbcState' # "Corrupted") =>
    /\ RbcStateRepairByInitStep
    /\ RbcEvidenceRepairByInitStep
    /\ RbcInitStepInstallsHeaderDigestEvidence
    /\ RbcInit
    /\ rbcState' = "Init"
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ headerSeen'
    /\ digestValid'
    /\ chunkCount' = 0
    /\ readyVotes' = 0
    /\ committed' = committed
    /\ gst' = gst

RbcCorruptedInitRepairResetsEvidenceStep ==
  (/\ rbcState = "Corrupted"
   /\ RbcInit) =>
    /\ RbcCorruptionExitOnlyByInitStep
    /\ rbcState' = "Init"
    /\ headerSeen'
    /\ digestValid'
    /\ chunkCount' = 0
    /\ readyVotes' = 0
    /\ ~committed'
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ phase' = phase
    /\ view' = view
    /\ gst' = gst

RbcChunkGateMatchesHeaderDigestEvidence ==
  RbcChunkGoodEnabled <=>
    /\ rbcState \in {"Init", "Chunking", "Withheld"}
    /\ headerSeen
    /\ (rbcState # "Withheld" => digestValid)
    /\ (rbcState = "Chunking" => chunkCount < MaxChunks)
    /\ (rbcState = "Withheld" => gst)

RbcChunkStepAdvancesChunkEvidence ==
  RbcChunkGood =>
    /\ rbcState \in {"Init", "Chunking", "Withheld"}
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid'
    /\ chunkCount' = IF chunkCount < MaxChunks THEN chunkCount + 1 ELSE chunkCount
    /\ chunkCount' >= chunkCount
    /\ chunkCount' <= MaxChunks
    /\ rbcState' = IF chunkCount' >= MaxChunks THEN "ChunksComplete" ELSE "Chunking"
    /\ (rbcState' = "ChunksComplete" => chunkCount' >= MaxChunks)
    /\ (rbcState' = "Chunking" => chunkCount' < MaxChunks)
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ readyVotes' = readyVotes
    /\ committed' = committed
    /\ gst' = gst

RbcChunkStepHandoffMatchesCoverageStep ==
  RbcChunkGood =>
    /\ RbcChunkStepAdvancesChunkEvidence
    /\ ~RbcInitEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ (chunkCount' < MaxChunks =>
          /\ rbcState' = "Chunking"
          /\ RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled')
    /\ (chunkCount' >= MaxChunks =>
          /\ rbcState' = "ChunksComplete"
          /\ ~RbcChunkGoodEnabled'
          /\ RbcReadyGoodEnabled')

RbcInitExitOnlyByChunkOrFaultStep ==
  (/\ rbcState = "Init"
   /\ rbcState' # "Init") =>
    \/ /\ RbcChunkGood
       /\ RbcStateAdvanceByChunkStep
       /\ RbcEvidenceAdvancedByChunkStep
       /\ RbcChunkStepAdvancesChunkEvidence
       /\ rbcState' \in {"Chunking", "ChunksComplete"}
       /\ headerSeen
       /\ headerSeen' = headerSeen
       /\ digestValid
       /\ digestValid'
       /\ chunkCount' = IF chunkCount < MaxChunks THEN chunkCount + 1 ELSE chunkCount
       /\ chunkCount' >= chunkCount
       /\ chunkCount' <= MaxChunks
       /\ (rbcState' = "Chunking" => chunkCount' < MaxChunks)
       /\ (rbcState' = "ChunksComplete" => chunkCount' >= MaxChunks)
       /\ readyVotes' = readyVotes
       /\ phase' = phase
       /\ view' = view
       /\ commitView' = commitView
       /\ prepareVotes' = prepareVotes
       /\ commitVotesHonest' = commitVotesHonest
       /\ commitVotesByz' = commitVotesByz
       /\ stakeSigned' = stakeSigned
       /\ commitEvidenceVotes' = commitEvidenceVotes
       /\ commitEvidenceStake' = commitEvidenceStake
       /\ newViewVotes' = newViewVotes
       /\ viewEvidenceVotes' = viewEvidenceVotes
       /\ committed' = committed
       /\ gst' = gst
    \/ /\ ByzantineFault
       /\ RbcStateCorruptedByFaultStep
       /\ RbcEvidenceCorruptedByFaultStep
       /\ ByzantineFaultStepCorruptsOnlyRbcDigest
       /\ rbcState' = "Corrupted"
       /\ headerSeen
       /\ headerSeen' = headerSeen
       /\ digestValid
       /\ ~digestValid'
       /\ chunkCount' = chunkCount
       /\ readyVotes' = readyVotes
       /\ phase' = phase
       /\ view' = view
       /\ commitView' = commitView
       /\ prepareVotes' = prepareVotes
       /\ commitVotesHonest' = commitVotesHonest
       /\ commitVotesByz' = commitVotesByz
       /\ stakeSigned' = stakeSigned
       /\ commitEvidenceVotes' = commitEvidenceVotes
       /\ commitEvidenceStake' = commitEvidenceStake
       /\ newViewVotes' = newViewVotes
       /\ viewEvidenceVotes' = viewEvidenceVotes
       /\ committed' = committed
       /\ gst' = gst

RbcChunkCountIncreaseOnlyByChunkStep ==
  (chunkCount' > chunkCount) =>
    /\ RbcChunkGood
    /\ RbcStateAdvanceByChunkStep
    /\ RbcEvidenceAdvancedByChunkStep
    /\ RbcChunkStepAdvancesChunkEvidence
    /\ rbcState \in {"Init", "Chunking", "Withheld"}
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid'
    /\ chunkCount < MaxChunks
    /\ chunkCount' = chunkCount + 1
    /\ chunkCount' <= MaxChunks
    /\ rbcState' = IF chunkCount' >= MaxChunks THEN "ChunksComplete" ELSE "Chunking"
    /\ (rbcState' = "ChunksComplete" => chunkCount' >= MaxChunks)
    /\ (rbcState' = "Chunking" => chunkCount' < MaxChunks)
    /\ readyVotes' = readyVotes
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ committed' = committed
    /\ gst' = gst

RbcChunkCountDecreaseOnlyByProposalOrInitStep ==
  (chunkCount' < chunkCount) =>
    \/ /\ RbcStateInitByProposalStep
       /\ RbcEvidenceInitByProposalStep
       /\ HonestPropose
       /\ rbcState = "Idle"
       /\ rbcState' = "Init"
       /\ phase = "Propose"
       /\ phase' = "Prepare"
       /\ ~committed
       /\ ~committed'
       /\ headerSeen'
       /\ digestValid'
       /\ chunkCount' = 0
       /\ readyVotes' = 0
       /\ newViewVotes' = 0
       /\ committed' = committed
       /\ gst' = gst
    \/ /\ RbcStateRepairByInitStep
       /\ RbcEvidenceRepairByInitStep
       /\ RbcInitStepInstallsHeaderDigestEvidence
       /\ RbcInit
       /\ rbcState \in {"Idle", "Withheld", "Corrupted"}
       /\ rbcState' = "Init"
       /\ phase' = phase
       /\ view' = view
       /\ commitView' = commitView
       /\ prepareVotes' = prepareVotes
       /\ commitVotesHonest' = commitVotesHonest
       /\ commitVotesByz' = commitVotesByz
       /\ stakeSigned' = stakeSigned
       /\ commitEvidenceVotes' = commitEvidenceVotes
       /\ commitEvidenceStake' = commitEvidenceStake
       /\ newViewVotes' = newViewVotes
       /\ viewEvidenceVotes' = viewEvidenceVotes
       /\ headerSeen'
       /\ digestValid'
       /\ chunkCount' = 0
       /\ readyVotes' = 0
       /\ committed' = committed
       /\ gst' = gst

RbcChunkingEntryOnlyByChunkStep ==
  (/\ rbcState # "Chunking"
   /\ rbcState' = "Chunking") =>
    /\ RbcChunkGood
    /\ RbcStateAdvanceByChunkStep
    /\ RbcEvidenceAdvancedByChunkStep
    /\ RbcChunkStepAdvancesChunkEvidence
    /\ rbcState \in {"Init", "Withheld"}
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid'
    /\ chunkCount' = IF chunkCount < MaxChunks THEN chunkCount + 1 ELSE chunkCount
    /\ chunkCount' > chunkCount
    /\ chunkCount' < MaxChunks
    /\ readyVotes' = readyVotes
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ committed' = committed
    /\ gst' = gst

RbcChunkingExitOnlyByChunkOrFaultStep ==
  (/\ rbcState = "Chunking"
   /\ rbcState' # "Chunking") =>
    \/ /\ RbcChunkGood
       /\ RbcStateAdvanceByChunkStep
       /\ RbcEvidenceAdvancedByChunkStep
       /\ RbcChunkStepAdvancesChunkEvidence
       /\ rbcState' = "ChunksComplete"
       /\ headerSeen
       /\ headerSeen' = headerSeen
       /\ digestValid
       /\ digestValid'
       /\ chunkCount < MaxChunks
       /\ chunkCount' = chunkCount + 1
       /\ chunkCount' >= MaxChunks
       /\ chunkCount' <= MaxChunks
       /\ readyVotes' = readyVotes
       /\ phase' = phase
       /\ view' = view
       /\ commitView' = commitView
       /\ prepareVotes' = prepareVotes
       /\ commitVotesHonest' = commitVotesHonest
       /\ commitVotesByz' = commitVotesByz
       /\ stakeSigned' = stakeSigned
       /\ commitEvidenceVotes' = commitEvidenceVotes
       /\ commitEvidenceStake' = commitEvidenceStake
       /\ newViewVotes' = newViewVotes
       /\ viewEvidenceVotes' = viewEvidenceVotes
       /\ committed' = committed
       /\ gst' = gst
    \/ /\ ByzantineFault
       /\ RbcStateCorruptedByFaultStep
       /\ RbcEvidenceCorruptedByFaultStep
       /\ ByzantineFaultStepCorruptsOnlyRbcDigest
       /\ rbcState' = "Corrupted"
       /\ headerSeen
       /\ headerSeen' = headerSeen
       /\ digestValid
       /\ ~digestValid'
       /\ chunkCount < MaxChunks
       /\ chunkCount' = chunkCount
       /\ readyVotes' = readyVotes
       /\ phase' = phase
       /\ view' = view
       /\ commitView' = commitView
       /\ prepareVotes' = prepareVotes
       /\ commitVotesHonest' = commitVotesHonest
       /\ commitVotesByz' = commitVotesByz
       /\ stakeSigned' = stakeSigned
       /\ commitEvidenceVotes' = commitEvidenceVotes
       /\ commitEvidenceStake' = commitEvidenceStake
       /\ newViewVotes' = newViewVotes
       /\ viewEvidenceVotes' = viewEvidenceVotes
       /\ committed' = committed
       /\ gst' = gst

RbcChunkCompletionEntryOnlyByChunkStep ==
  (/\ rbcState # "ChunksComplete"
   /\ rbcState' = "ChunksComplete") =>
    /\ RbcChunkGood
    /\ RbcStateAdvanceByChunkStep
    /\ RbcChunkStepAdvancesChunkEvidence
    /\ rbcState \in {"Init", "Chunking", "Withheld"}
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid'
    /\ chunkCount' = IF chunkCount < MaxChunks THEN chunkCount + 1 ELSE chunkCount
    /\ chunkCount' >= MaxChunks
    /\ readyVotes' = readyVotes
    /\ committed' = committed
    /\ gst' = gst

RbcChunksCompleteExitOnlyByReadyOrFaultStep ==
  (/\ rbcState = "ChunksComplete"
   /\ rbcState' # "ChunksComplete") =>
    \/ /\ RbcReadyGood
       /\ RbcStateAdvanceByReadyStep
       /\ RbcEvidenceAdvancedByReadyStep
       /\ rbcState' \in {"ReadyPartial", "ReadyQuorum"}
       /\ chunkCount >= MaxChunks
       /\ chunkCount' = chunkCount
       /\ headerSeen
       /\ headerSeen' = headerSeen
       /\ digestValid
       /\ digestValid' = digestValid
       /\ readyVotes < N
       /\ readyVotes' = readyVotes + 1
       /\ readyVotes' <= N
       /\ (rbcState' = "ReadyPartial" => readyVotes' < CommitQuorum)
       /\ (rbcState' = "ReadyQuorum" => readyVotes' >= CommitQuorum)
       /\ phase' = phase
       /\ view' = view
       /\ commitView' = commitView
       /\ prepareVotes' = prepareVotes
       /\ commitVotesHonest' = commitVotesHonest
       /\ commitVotesByz' = commitVotesByz
       /\ stakeSigned' = stakeSigned
       /\ commitEvidenceVotes' = commitEvidenceVotes
       /\ commitEvidenceStake' = commitEvidenceStake
       /\ newViewVotes' = newViewVotes
       /\ viewEvidenceVotes' = viewEvidenceVotes
       /\ committed' = committed
       /\ gst' = gst
    \/ /\ ByzantineFault
       /\ RbcStateCorruptedByFaultStep
       /\ RbcEvidenceCorruptedByFaultStep
       /\ ByzantineFaultStepCorruptsOnlyRbcDigest
       /\ rbcState' = "Corrupted"
       /\ chunkCount >= MaxChunks
       /\ chunkCount' = chunkCount
       /\ headerSeen
       /\ headerSeen' = headerSeen
       /\ digestValid
       /\ ~digestValid'
       /\ readyVotes' = readyVotes
       /\ phase' = phase
       /\ view' = view
       /\ commitView' = commitView
       /\ prepareVotes' = prepareVotes
       /\ commitVotesHonest' = commitVotesHonest
       /\ commitVotesByz' = commitVotesByz
       /\ stakeSigned' = stakeSigned
       /\ commitEvidenceVotes' = commitEvidenceVotes
       /\ commitEvidenceStake' = commitEvidenceStake
       /\ newViewVotes' = newViewVotes
       /\ viewEvidenceVotes' = viewEvidenceVotes
       /\ committed' = committed
       /\ gst' = gst

RbcReadyGateMatchesChunkEvidence ==
  RbcReadyGoodEnabled <=>
    /\ rbcState \in {"ChunksComplete", "ReadyPartial", "ReadyQuorum"}
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid
    /\ readyVotes < N

RbcReadyStepAdvancesReadyEvidence ==
  RbcReadyGood =>
    /\ rbcState \in {"ChunksComplete", "ReadyPartial", "ReadyQuorum"}
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid
    /\ readyVotes < N
    /\ readyVotes' = readyVotes + 1
    /\ readyVotes' <= N
    /\ rbcState' = IF readyVotes' >= CommitQuorum THEN "ReadyQuorum" ELSE "ReadyPartial"
    /\ (rbcState' = "ReadyQuorum" => readyVotes' >= CommitQuorum)
    /\ (rbcState' = "ReadyPartial" => readyVotes' < CommitQuorum)
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ committed' = committed
    /\ gst' = gst

RbcReadyStepHandoffMatchesQuorumStep ==
  RbcReadyGood =>
    /\ RbcReadyStepAdvancesReadyEvidence
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ (readyVotes' < CommitQuorum =>
          /\ rbcState' = "ReadyPartial"
          /\ RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled')
    /\ (readyVotes' >= CommitQuorum =>
          /\ rbcState' = "ReadyQuorum"
          /\ RbcDeliverGoodEnabled'
          /\ (RbcReadyGoodEnabled' <=> readyVotes' < N))

RbcReadyQuorumStepEnablesDeliverHandoff ==
  (RbcReadyGood /\ readyVotes + 1 >= CommitQuorum) =>
    /\ rbcState \in {"ChunksComplete", "ReadyPartial", "ReadyQuorum"}
    /\ rbcState' = "ReadyQuorum"
    /\ readyVotes < N
    /\ readyVotes' = readyVotes + 1
    /\ readyVotes' >= CommitQuorum
    /\ readyVotes' <= N
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid
    /\ RbcDeliverGoodEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ (RbcReadyGoodEnabled' <=> readyVotes' < N)
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ committed' = committed
    /\ gst' = gst

RbcReadyVotesIncreaseOnlyByReadyStep ==
  (readyVotes' > readyVotes) =>
    /\ RbcReadyGood
    /\ RbcStateAdvanceByReadyStep
    /\ RbcEvidenceAdvancedByReadyStep
    /\ RbcReadyStepAdvancesReadyEvidence
    /\ rbcState \in {"ChunksComplete", "ReadyPartial", "ReadyQuorum"}
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid
    /\ readyVotes < N
    /\ readyVotes' = readyVotes + 1
    /\ readyVotes' <= N
    /\ rbcState' = IF readyVotes' >= CommitQuorum THEN "ReadyQuorum" ELSE "ReadyPartial"
    /\ (rbcState' = "ReadyQuorum" => readyVotes' >= CommitQuorum)
    /\ (rbcState' = "ReadyPartial" => readyVotes' < CommitQuorum)
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ committed' = committed
    /\ gst' = gst

RbcReadyVotesDecreaseOnlyByProposalOrInitStep ==
  (readyVotes' < readyVotes) =>
    \/ /\ RbcStateInitByProposalStep
       /\ RbcEvidenceInitByProposalStep
       /\ HonestPropose
       /\ rbcState = "Idle"
       /\ rbcState' = "Init"
       /\ phase = "Propose"
       /\ phase' = "Prepare"
       /\ ~committed
       /\ ~committed'
       /\ headerSeen'
       /\ digestValid'
       /\ chunkCount' = 0
       /\ readyVotes' = 0
       /\ newViewVotes' = 0
       /\ committed' = committed
       /\ gst' = gst
    \/ /\ RbcStateRepairByInitStep
       /\ RbcEvidenceRepairByInitStep
       /\ RbcInitStepInstallsHeaderDigestEvidence
       /\ RbcInit
       /\ rbcState \in {"Idle", "Withheld", "Corrupted"}
       /\ rbcState' = "Init"
       /\ phase' = phase
       /\ view' = view
       /\ commitView' = commitView
       /\ prepareVotes' = prepareVotes
       /\ commitVotesHonest' = commitVotesHonest
       /\ commitVotesByz' = commitVotesByz
       /\ stakeSigned' = stakeSigned
       /\ commitEvidenceVotes' = commitEvidenceVotes
       /\ commitEvidenceStake' = commitEvidenceStake
       /\ newViewVotes' = newViewVotes
       /\ viewEvidenceVotes' = viewEvidenceVotes
       /\ headerSeen'
       /\ digestValid'
       /\ chunkCount' = 0
       /\ readyVotes' = 0
       /\ committed' = committed
       /\ gst' = gst

RbcReadyPartialEntryOnlyByReadyStep ==
  (/\ rbcState # "ReadyPartial"
   /\ rbcState' = "ReadyPartial") =>
    /\ RbcReadyGood
    /\ RbcStateAdvanceByReadyStep
    /\ RbcEvidenceAdvancedByReadyStep
    /\ RbcReadyStepAdvancesReadyEvidence
    /\ rbcState = "ChunksComplete"
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid
    /\ readyVotes < N
    /\ readyVotes' = readyVotes + 1
    /\ readyVotes' > readyVotes
    /\ readyVotes' < CommitQuorum
    /\ phase' = phase
    /\ view' = view
    /\ commitView' = commitView
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes' = commitEvidenceVotes
    /\ commitEvidenceStake' = commitEvidenceStake
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ committed' = committed
    /\ gst' = gst

RbcReadyPartialExitOnlyByReadyOrFaultStep ==
  (/\ rbcState = "ReadyPartial"
   /\ rbcState' # "ReadyPartial") =>
    \/ /\ RbcReadyGood
       /\ RbcStateAdvanceByReadyStep
       /\ RbcEvidenceAdvancedByReadyStep
       /\ RbcReadyStepAdvancesReadyEvidence
       /\ rbcState' = "ReadyQuorum"
       /\ readyVotes < N
       /\ readyVotes' = readyVotes + 1
       /\ readyVotes' >= CommitQuorum
       /\ chunkCount >= MaxChunks
       /\ chunkCount' = chunkCount
       /\ headerSeen
       /\ headerSeen' = headerSeen
       /\ digestValid
       /\ digestValid' = digestValid
       /\ phase' = phase
       /\ view' = view
       /\ commitView' = commitView
       /\ prepareVotes' = prepareVotes
       /\ commitVotesHonest' = commitVotesHonest
       /\ commitVotesByz' = commitVotesByz
       /\ stakeSigned' = stakeSigned
       /\ commitEvidenceVotes' = commitEvidenceVotes
       /\ commitEvidenceStake' = commitEvidenceStake
       /\ newViewVotes' = newViewVotes
       /\ viewEvidenceVotes' = viewEvidenceVotes
       /\ committed' = committed
       /\ gst' = gst
    \/ /\ ByzantineFault
       /\ RbcStateCorruptedByFaultStep
       /\ ByzantineFaultStepCorruptsOnlyRbcDigest
       /\ rbcState' = "Corrupted"
       /\ ~digestValid'
       /\ readyVotes' = readyVotes
       /\ chunkCount' = chunkCount
       /\ headerSeen' = headerSeen
       /\ committed' = committed
       /\ gst' = gst

RbcReadyQuorumEntryOnlyByReadyStep ==
  (/\ rbcState # "ReadyQuorum"
   /\ rbcState' = "ReadyQuorum") =>
    /\ RbcReadyGood
    /\ RbcStateAdvanceByReadyStep
    /\ RbcReadyStepAdvancesReadyEvidence
    /\ rbcState \in {"ChunksComplete", "ReadyPartial"}
    /\ readyVotes' = readyVotes + 1
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid

RbcDeliverGateMatchesCompleteEvidence ==
  RbcDeliverGoodEnabled <=>
    /\ rbcState = "ReadyQuorum"
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid

RbcReadyQuorumEnablesDeliverGate ==
  rbcState = "ReadyQuorum" => RbcDeliverGoodEnabled

RbcDeliverFinalityGateMatchesBufferedCommitEvidence ==
  (RbcDeliverGoodEnabled /\ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) <=>
    /\ rbcState = "ReadyQuorum"
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid
    /\ phase = "CommitVote"
    /\ prepareVotes >= CommitQuorum
    /\ commitVotesHonest + commitVotesByz >= CommitQuorum
    /\ commitVotesHonest >= HonestCommitSupportThreshold
    /\ commitVotesHonest <= N - F
    /\ commitVotesByz <= F
    /\ stakeSigned >= StakeQuorum
    /\ newViewVotes = 0
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)

RbcDeliverFinalityStepInstallsCommitArtifacts ==
  (RbcDeliverGood /\ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) =>
    /\ rbcState = "ReadyQuorum"
    /\ rbcState' = "Delivered"
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ ~committed
    /\ committed'
    /\ prepareVotes >= CommitQuorum
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest + commitVotesByz >= CommitQuorum
    /\ commitVotesHonest >= HonestCommitSupportThreshold
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned >= StakeQuorum
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = stakeSigned
    /\ commitView = 0
    /\ commitView' = view
    /\ view' = view
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid

RbcDeliverFinalityStepCompletesCommittedDelivery ==
  (RbcDeliverGood /\ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) =>
    /\ RbcDeliverGoodEnabled
    /\ RbcStateDeliverByDeliverStep
    /\ PhaseFinalizesByRbcDeliverStep
    /\ RbcDeliverFinalityStepInstallsCommitArtifacts
    /\ rbcState = "ReadyQuorum"
    /\ rbcState' = "Delivered"
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ ~committed
    /\ committed'
    /\ committed' # committed
    /\ prepareVotes >= CommitQuorum
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest + commitVotesByz >= CommitQuorum
    /\ commitVotesHonest >= HonestCommitSupportThreshold
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned >= StakeQuorum
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = stakeSigned
    /\ commitEvidenceStake' >= StakeQuorum
    /\ commitView = 0
    /\ commitView' = view
    /\ commitView' = view'
    /\ view' = view
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ readyVotes >= CommitQuorum
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid
    /\ gst' = gst
    /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ FinalityCertificateStackPresent'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitDisablesProgressActions'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~TimeoutTickEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ ~PostGstProgressEnabled'

FinalityLatchChangeCompletesCommittedDeliveryFromExactSourceStep ==
  (~committed /\ committed') =>
    /\ FinalityLatchChangeMatchesCertifiedSourceStackStep
    /\ CommittedPhaseEntryDisablesProgressActionsStep
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ committed' # committed
    /\ view' = view
    /\ commitView = 0
    /\ commitView' = view'
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum
    /\ prepareVotes >= CommitQuorum
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' + commitVotesByz' >= CommitQuorum
    /\ commitVotesHonest' >= HonestCommitSupportThreshold
    /\ stakeSigned' >= StakeQuorum
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ rbcState' = "Delivered"
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ gst' = gst
    /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ FinalityCertificateStackPresent'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitDisablesProgressActions'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~TimeoutTickEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ ~PostGstProgressEnabled'
    /\ \/ /\ HonestCommitVote
          /\ ~ByzantineEquivocateCommit
          /\ ~RbcDeliverGood
          /\ HonestCommitVoteFinalityStepCompletesCommittedDelivery
       \/ /\ ~HonestCommitVote
          /\ ByzantineEquivocateCommit
          /\ ~RbcDeliverGood
          /\ ByzantineCommitVoteFinalityStepCompletesCommittedDelivery
       \/ /\ ~HonestCommitVote
          /\ ~ByzantineEquivocateCommit
          /\ RbcDeliverGood
          /\ RbcDeliverFinalityStepCompletesCommittedDelivery

CommittedPhaseEntryCompletesCommittedDeliveryFromExactSourceStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed") =>
    /\ CommittedPhaseEntryMatchesCertifiedFinalityStackStep
    /\ CommittedPhaseEntryInstallsCommitCertificateWitnessesStep
    /\ CommittedPhaseEntryMatchesCommitCertificateWitnessChangeStep
    /\ CommittedPhaseEntryInstallsCommitViewWitnessStep
    /\ CommittedPhaseEntryMatchesLiveCommitGateCrossingStep
    /\ CommittedPhaseEntryMatchesCommitArtifactsChangeStep
    /\ CommittedPhaseEntryMatchesExactFinalitySourceEffectsStep
    /\ CommittedPhaseEntryNeverCarriesNewViewHandoffStep
    /\ CommittedPhaseEntryDisablesProgressActionsStep
    /\ FinalityLatchChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ phase = "CommitVote"
    /\ ~committed
    /\ committed'
    /\ committed' # committed
    /\ view' = view
    /\ commitView = 0
    /\ commitView' = view'
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum
    /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, rbcState)
    /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ rbcState' = "Delivered"
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ FinalityCertificateStackPresent'
    /\ CommitDisablesProgressActions'

CommitArtifactsChangeCompletesCommittedDeliveryFromExactSourceStep ==
  (\/ commitView' # commitView
   \/ commitEvidenceVotes' # commitEvidenceVotes
   \/ commitEvidenceStake' # commitEvidenceStake) =>
    /\ CommitArtifactsChangeMatchesCertifiedFinalityStackStep
    /\ FinalityLatchChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ CommittedPhaseEntryCompletesCommittedDeliveryFromExactSourceStep
    /\ ~committed
    /\ committed'
    /\ committed' # committed
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ view' = view
    /\ commitView = 0
    /\ commitView' = view'
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum
    /\ rbcState' = "Delivered"
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ FinalityCertificateStackPresent'
    /\ CommitDisablesProgressActions'

CommitArtifactsChangeLeavesOnlyGstElapsedGateStep ==
  (\/ commitView' # commitView
   \/ commitEvidenceVotes' # commitEvidenceVotes
   \/ commitEvidenceStake' # commitEvidenceStake) =>
    /\ CommitArtifactsChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ CommitArtifactsChangePreservesGstStep
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (gst' => CommittedGstDisablesEveryAction')

CommitCertificateWitnessChangeCompletesCommittedDeliveryFromExactSourceStep ==
  (\/ commitEvidenceVotes' # commitEvidenceVotes
   \/ commitEvidenceStake' # commitEvidenceStake) =>
    /\ CommitCertificateWitnessChangeMatchesCertifiedFinalityStackStep
    /\ CommitCertificateWitnessChangeInstallsCommitViewWitnessStep
    /\ CommitArtifactsChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ FinalityLatchChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ CommittedPhaseEntryCompletesCommittedDeliveryFromExactSourceStep
    /\ ~committed
    /\ committed'
    /\ committed' # committed
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ view' = view
    /\ commitView = 0
    /\ commitView' = view'
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' # commitEvidenceVotes
    /\ commitEvidenceStake' # commitEvidenceStake
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum
    /\ rbcState' = "Delivered"
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ FinalityCertificateStackPresent'
    /\ CommitDisablesProgressActions'

CommitCertificateWitnessChangeLeavesOnlyGstElapsedGateStep ==
  (\/ commitEvidenceVotes' # commitEvidenceVotes
   \/ commitEvidenceStake' # commitEvidenceStake) =>
    /\ CommitCertificateWitnessChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ CommitCertificateWitnessChangePreservesGstStep
    /\ CommitArtifactsChangeLeavesOnlyGstElapsedGateStep
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (gst' => CommittedGstDisablesEveryAction')

CommitViewWitnessChangeCompletesCommittedDeliveryFromExactSourceStep ==
  (commitView' # commitView) =>
    /\ CommitViewWitnessChangeMatchesCertifiedFinalityStackStep
    /\ CommitViewWitnessChangeInstallsCommitCertificateWitnessesStep
    /\ CommitCertificateWitnessChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ CommitArtifactsChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ FinalityLatchChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ CommittedPhaseEntryCompletesCommittedDeliveryFromExactSourceStep
    /\ ~committed
    /\ committed'
    /\ committed' # committed
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ view' = view
    /\ view' # 0
    /\ viewEvidenceVotes' >= ViewQuorum
    /\ commitView = 0
    /\ commitView' = view'
    /\ commitView' # commitView
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' # commitEvidenceVotes
    /\ commitEvidenceStake' # commitEvidenceStake
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum
    /\ rbcState' = "Delivered"
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ FinalityCertificateStackPresent'
    /\ CommitDisablesProgressActions'

CommitViewWitnessChangeLeavesOnlyGstElapsedGateStep ==
  (commitView' # commitView) =>
    /\ CommitViewWitnessChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ CommitViewWitnessChangePreservesGstStep
    /\ CommitCertificateWitnessChangeLeavesOnlyGstElapsedGateStep
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (gst' => CommittedGstDisablesEveryAction')

CommittedPhaseEntryMatchesCommitViewWitnessChangeStep ==
  (/\ phase # "Committed"
   /\ phase' = "Committed"
   /\ commitView' # commitView) =>
    /\ CommitViewWitnessChangeMatchesCertifiedFinalityStackStep
    /\ CommitViewWitnessChangeInstallsCommitCertificateWitnessesStep

FinalitySourceActionCompletesCommittedDeliveryFromExactSourceStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    /\ FinalityLatchSourceIsCommitOrDeliveryStep
    /\ FinalityLatchSourceEffectsAreExactStep
    /\ FinalityLatchSourceQuorumGatesHoldStep
    /\ FinalityLatchChangeMatchesCertifiedSourceStackStep
    /\ FinalityLatchChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ CommittedPhaseEntryCompletesCommittedDeliveryFromExactSourceStep
    /\ CommitArtifactsChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ CommitCertificateWitnessChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ CommitViewWitnessChangeCompletesCommittedDeliveryFromExactSourceStep

FinalitySourceActionMatchesCertifiedSourceStackStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    FinalityLatchChangeMatchesCertifiedSourceStackStep

FinalitySourceActionMatchesFinalityLatchChangeStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    /\ FinalityLatchSetInstallsCompleteStackStep
    /\ FinalityLatchAndArtifactsCoupledStep
    /\ FinalityLatchChangeEntersCommittedPhaseStep
    /\ FinalityLatchChangeMatchesLiveCommitGateCrossingStep
    /\ FinalityLatchChangeMatchesCertifiedSourceStackStep

FinalitySourceActionMatchesCommittedPhaseEntryStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    /\ FinalityLatchChangeEntersCommittedPhaseStep
    /\ CommittedPhaseEntryMatchesFinalityLatchStep
    /\ CommittedPhaseEntryInstallsCompleteStackStep
    /\ CommittedPhaseEntryOnlyByFinalitySourceStep
    /\ CommittedPhaseEntryMatchesCertifiedFinalityStackStep

FinalitySourceActionInstallsFinalityCertificateStackStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    /\ FinalityCertificateStackPresent'
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ CommitViewMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'

FinalitySourceActionSourceIsCommitOrDeliveryStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    FinalityLatchSourceIsCommitOrDeliveryStep

FinalitySourceActionSourceEffectsAreExactStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    FinalityLatchSourceEffectsAreExactStep

FinalitySourceActionQuorumGatesHoldStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    FinalityLatchSourceQuorumGatesHoldStep

FinalitySourceActionMatchesCommitArtifactsChangeStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    /\ CommittedPhaseEntryMatchesCommitArtifactsChangeStep
    /\ CommitArtifactsChangeMatchesCertifiedFinalityStackStep
    /\ CommitArtifactsChangeCompletesCommittedDeliveryFromExactSourceStep

FinalitySourceActionMatchesLiveCommitGateCrossingStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    /\ FinalityLatchChangeMatchesLiveCommitGateCrossingStep
    /\ CommittedPhaseEntryMatchesLiveCommitGateCrossingStep
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'

FinalitySourceActionDisablesProgressAfterCommittedDeliveryStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    CommitDisablesProgressActions'

FinalitySourceActionPreservesGstStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    gst' = gst

FinalitySourceActionLeavesOnlyGstElapsedGateStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    /\ CommitDisablesProgressActions'
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (gst' => CommittedGstDisablesEveryAction')

FinalitySourceActionInstallsCommitCertificateWitnessesStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    CommitCertificateWitnessesInstallWithFinalityLatchStep

FinalitySourceActionMatchesCommitCertificateWitnessChangeStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    /\ CommittedPhaseEntryMatchesCommitCertificateWitnessChangeStep
    /\ CommitCertificateWitnessChangeMatchesCertifiedFinalityStackStep
    /\ CommitCertificateWitnessChangeInstallsCommitViewWitnessStep
    /\ CommitCertificateWitnessChangeCompletesCommittedDeliveryFromExactSourceStep

FinalitySourceActionMatchesCommitViewWitnessChangeStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed' /\ commitView' # commitView) =>
    CommitViewWitnessChangeCompletesCommittedDeliveryFromExactSourceStep

FinalitySourceActionInstallsCommitViewWitnessStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    CommitViewWitnessInstallsWithFinalityLatchStep

FinalitySourceActionNeverCarriesNewViewHandoffStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    FinalityLatchNeverCarriesNewViewHandoffStep

FinalitySourceActionCommitsCurrentViewStep ==
  ((HonestCommitVote \/ ByzantineEquivocateCommit \/ RbcDeliverGood) /\
   ~committed /\ committed') =>
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ view' = view
    /\ commitView = 0
    /\ commitView' = view
    /\ commitView' = view'
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ FinalitySourceActionNeverCarriesNewViewHandoffStep

RbcDeliverPendingGateMatchesMissingBufferedCommitEvidence ==
  (RbcDeliverGoodEnabled /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) <=>
    /\ rbcState = "ReadyQuorum"
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ \/ commitVotesHonest + commitVotesByz < CommitQuorum
       \/ stakeSigned < StakeQuorum

RbcDeliverPendingStepPreservesPreFinalityArtifacts ==
  (RbcDeliverGood /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) =>
    /\ rbcState = "ReadyQuorum"
    /\ rbcState' = "Delivered"
    /\ phase' = phase
    /\ ~committed
    /\ ~committed'
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ chunkCount' = chunkCount
    /\ readyVotes' = readyVotes
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid

RbcDeliverPendingStepKeepsDeliveredEvidenceWithoutFinality ==
  (RbcDeliverGood /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) =>
    /\ rbcState = "ReadyQuorum"
    /\ rbcState' = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid
    /\ phase' = phase
    /\ view' = view
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ ~committed'
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ \/ commitVotesHonest' + commitVotesByz' < CommitQuorum
       \/ stakeSigned' < StakeQuorum
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

PendingProtocolStepsPreserveGst ==
  /\ ((HonestNewViewVote /\ newViewVotes + 1 < ViewQuorum) => gst' = gst)
  /\ ((HonestPrepareVote /\ prepareVotes + 1 < CommitQuorum) => gst' = gst)
  /\ ((HonestCommitVote /\
        ~CanCommit(
          commitVotesHonest + 1,
          commitVotesByz,
          stakeSigned + StakePerHonestVote,
          rbcState
        )) => gst' = gst)
  /\ ((ByzantineEquivocateCommit /\
        ~CanCommit(
          commitVotesHonest,
          commitVotesByz + 1,
          stakeSigned + StakePerByzVote,
          rbcState
        )) => gst' = gst)
  /\ ((RbcDeliverGood /\
        ~CanCommit(
          commitVotesHonest,
          commitVotesByz,
          stakeSigned,
          "Delivered"
        )) => gst' = gst)

RbcDeliverStepPreservesCompleteEvidence ==
  RbcDeliverGood =>
    /\ RbcDeliverGoodEnabled
    /\ RbcStateDeliverByDeliverStep
    /\ rbcState = "ReadyQuorum"
    /\ rbcState' = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ readyVotes' = readyVotes
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid
    /\ view' = view
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ gst' = gst

RbcDeliverStepHandoffMatchesCommitEvidenceStep ==
  RbcDeliverGood =>
    /\ RbcDeliverStepPreservesCompleteEvidence
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ (CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
          /\ RbcDeliverFinalityStepCompletesCommittedDelivery
          /\ committed'
          /\ phase' = "Committed"
          /\ CommitDisablesProgressActions')
    /\ (~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
          /\ RbcDeliverPendingStepKeepsDeliveredEvidenceWithoutFinality
          /\ ~committed'
          /\ phase' = phase
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0)

RbcReadyQuorumExitOnlyByDeliverOrFaultStep ==
  (/\ rbcState = "ReadyQuorum"
   /\ rbcState' # "ReadyQuorum") =>
    \/ /\ RbcDeliverGood
       /\ RbcStateDeliverByDeliverStep
       /\ RbcDeliverStepPreservesCompleteEvidence
       /\ rbcState' = "Delivered"
       /\ readyVotes >= CommitQuorum
       /\ readyVotes' = readyVotes
       /\ chunkCount >= MaxChunks
       /\ chunkCount' = chunkCount
       /\ headerSeen
       /\ headerSeen' = headerSeen
       /\ digestValid
       /\ digestValid' = digestValid
       /\ gst' = gst
       /\ (CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
             RbcDeliverFinalityStepInstallsCommitArtifacts)
       /\ (~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
             RbcDeliverPendingStepPreservesPreFinalityArtifacts)
    \/ /\ ByzantineFault
       /\ RbcStateCorruptedByFaultStep
       /\ ByzantineFaultStepCorruptsOnlyRbcDigest
       /\ rbcState' = "Corrupted"
       /\ ~digestValid'
       /\ readyVotes' = readyVotes
       /\ chunkCount' = chunkCount
       /\ headerSeen' = headerSeen
       /\ committed' = committed
       /\ gst' = gst

RbcDeliveryEntryOnlyByDeliverStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliverGood
    /\ RbcStateDeliverByDeliverStep
    /\ (CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
          /\ PhaseFinalizesByRbcDeliverStep
          /\ RbcDeliverFinalityStepInstallsCommitArtifacts)
    /\ (~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
          /\ ~committed'
          /\ RbcDeliverPendingStepPreservesPreFinalityArtifacts)

RbcDeliveryEntryMatchesReadyQuorumExitAndCommitBranchStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryOnlyByDeliverStep
    /\ RbcReadyQuorumExitOnlyByDeliverOrFaultStep
    /\ RbcDeliverStepPreservesCompleteEvidence
    /\ rbcState = "ReadyQuorum"
    /\ rbcState' = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ readyVotes' = readyVotes
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid
    /\ (CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
          /\ PhaseFinalizesByRbcDeliverStep
          /\ RbcDeliverFinalityStepInstallsCommitArtifacts
          /\ phase = "CommitVote"
          /\ phase' = "Committed"
          /\ ~committed
          /\ committed'
          /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz
          /\ commitEvidenceStake' = stakeSigned)
    /\ (~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
          /\ RbcDeliverPendingStepPreservesPreFinalityArtifacts
          /\ phase' = phase
          /\ ~committed
          /\ ~committed'
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0)

RbcDeliveryEntryFinalityCompletesCommittedDeliveryStep ==
  (RbcDeliverGood /\ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) =>
    /\ RbcDeliverFinalityGateMatchesBufferedCommitEvidence
    /\ RbcDeliveryEntryMatchesReadyQuorumExitAndCommitBranchStep
    /\ RbcDeliverStepHandoffMatchesCommitEvidenceStep
    /\ RbcDeliverFinalityStepCompletesCommittedDelivery
    /\ FinalitySourceActionCompletesCommittedDeliveryFromExactSourceStep
    /\ FinalitySourceActionCommitsCurrentViewStep
    /\ FinalitySourceActionLeavesOnlyGstElapsedGateStep
    /\ rbcState = "ReadyQuorum"
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ view' = view
    /\ gst' = gst
    /\ ~committed
    /\ committed'
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = stakeSigned
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceStake' >= StakeQuorum
    /\ commitView = 0
    /\ commitView' = view
    /\ commitView' = view'
    /\ prepareVotes' = prepareVotes
    /\ prepareVotes' >= CommitQuorum
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ FinalityCertificateStackPresent'
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ CommitViewMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ FinalityClearsNewViewHandoff'
    /\ CommitDisablesProgressActions'
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (gst' => CommittedGstDisablesEveryAction')

RbcDeliveryEntryPendingInstallsCompleteWaitStateStep ==
  (RbcDeliverGood /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) =>
    /\ RbcDeliverPendingGateMatchesMissingBufferedCommitEvidence
    /\ RbcDeliveryEntryMatchesReadyQuorumExitAndCommitBranchStep
    /\ RbcDeliverPendingStepKeepsDeliveredEvidenceWithoutFinality
    /\ rbcState = "ReadyQuorum"
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ phase' = phase
    /\ view' = view
    /\ gst' = gst
    /\ ~committed
    /\ ~committed'
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ ~FinalityCertificateStackPresent'
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitDisablesProgressActions'
    /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
    /\ DeliverImpliesEvidence'
    /\ NoCommitEvidenceBeforeCommit'
    /\ NoCommitViewBeforeCommit'
    /\ ViewEvidenceIsCompleteOrEmpty'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ \/ commitVotesHonest' + commitVotesByz' < CommitQuorum
       \/ stakeSigned' < StakeQuorum
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ (PostGstProgressEnabled' <=>
          \/ HonestProposeEnabled'
          \/ HonestPrepareVoteEnabled'
          \/ HonestCommitVoteEnabled'
          \/ HonestNewViewVoteEnabled')
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))

RbcDeliveryEntryCompletesFinalityOrWaitStateStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryOnlyByDeliverStep
    /\ RbcDeliveryEntryMatchesReadyQuorumExitAndCommitBranchStep
    /\ RbcDeliverStepHandoffMatchesCommitEvidenceStep
    /\ rbcState = "ReadyQuorum"
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ view' = view
    /\ gst' = gst
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ \/ /\ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
          /\ RbcDeliveryEntryFinalityCompletesCommittedDeliveryStep
          /\ phase = "CommitVote"
          /\ phase' = "Committed"
          /\ ~committed
          /\ committed'
          /\ commitEvidenceVotes = 0
          /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz
          /\ commitEvidenceStake = 0
          /\ commitEvidenceStake' = stakeSigned
          /\ commitView = 0
          /\ commitView' = view
          /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ FinalityCertificateStackPresent'
          /\ CommitDisablesProgressActions'
          /\ (GstElapsedEnabled' <=> ~gst')
       \/ /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
          /\ RbcDeliveryEntryPendingInstallsCompleteWaitStateStep
          /\ phase' = phase
          /\ ~committed
          /\ ~committed'
          /\ commitEvidenceVotes = 0
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake = 0
          /\ commitEvidenceStake' = 0
          /\ commitView = 0
          /\ commitView' = 0
          /\ ~FinalityCertificateStackPresent'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ (GstElapsedEnabled' <=> ~gst')
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))

RbcDeliveryEntryCommitArtifactsMatchOutcomeStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCompletesFinalityOrWaitStateStep
    /\ (committed' <=> CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered"))
    /\ (FinalityCertificateStackPresent' <=>
          CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered"))
    /\ (CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState') <=>
          CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered"))
    /\ commitEvidenceVotes' =
          IF CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
          THEN commitVotesHonest + commitVotesByz
          ELSE 0
    /\ commitEvidenceStake' =
          IF CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
          THEN stakeSigned
          ELSE 0
    /\ commitView' =
          IF CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
          THEN view
          ELSE 0
    /\ (CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
          /\ phase = "CommitVote"
          /\ phase' = "Committed"
          /\ ~committed
          /\ committed'
          /\ commitEvidenceVotes = 0
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake = 0
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitEvidenceStake' >= StakeQuorum
          /\ commitView = 0
          /\ commitView' = view'
          /\ FinalityCertificateStackPresent'
          /\ FinalityCertificateStackComplete'
          /\ FinalityCertificateStackMatchesFinality'
          /\ CommitCertificateMatchesFinality'
          /\ CommitViewMatchesFinality'
          /\ LiveCommitGateMatchesFinality')
    /\ (~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
          /\ phase' = phase
          /\ ~committed
          /\ ~committed'
          /\ commitEvidenceVotes = 0
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake = 0
          /\ commitEvidenceStake' = 0
          /\ commitView = 0
          /\ commitView' = 0
          /\ ~FinalityCertificateStackPresent'
          /\ FinalityCertificateStackComplete'
          /\ FinalityCertificateStackMatchesFinality'
          /\ CommitCertificateMatchesFinality'
          /\ LiveCommitGateMatchesFinality')

RbcDeliveryEntryPostGateSurfaceMatchesOutcomeStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitArtifactsMatchOutcomeStep
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
          /\ committed'
          /\ phase' = "Committed"
          /\ CommitDisablesProgressActions'
          /\ ~PostGstProgressEnabled'
          /\ ~TimeoutTickEnabled'
          /\ (gst' => CommittedGstDisablesEveryAction'))
    /\ (~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
          /\ ~committed'
          /\ phase' = phase
          /\ (PostGstProgressEnabled' <=>
                \/ HonestProposeEnabled'
                \/ HonestPrepareVoteEnabled'
                \/ HonestCommitVoteEnabled'
                \/ HonestNewViewVoteEnabled')
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled')))

RbcDeliveryEntryConsensusFrameMatchesOutcomeStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryPostGateSurfaceMatchesOutcomeStep
    /\ view' = view
    /\ gst' = gst
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ phase' =
          IF CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
          THEN "Committed"
          ELSE phase
    /\ newViewVotes' =
          IF CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
          THEN 0
          ELSE newViewVotes
    /\ (CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
          /\ phase = "CommitVote"
          /\ phase' = "Committed"
          /\ newViewVotes = 0
          /\ newViewVotes' = 0
          /\ prepareVotes >= CommitQuorum
          /\ commitVotesHonest + commitVotesByz >= CommitQuorum
          /\ commitVotesHonest >= HonestCommitSupportThreshold
          /\ stakeSigned >= StakeQuorum
          /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum))
    /\ (~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
          /\ phase' = phase
          /\ newViewVotes' = newViewVotes
          /\ \/ commitVotesHonest + commitVotesByz < CommitQuorum
             \/ stakeSigned < StakeQuorum)

RbcDeliveryEntryFinalityMatchesCertifiedSourceStackStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered"
   /\ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) =>
    /\ RbcDeliveryEntryConsensusFrameMatchesOutcomeStep
    /\ RbcDeliveryEntryFinalityCompletesCommittedDeliveryStep
    /\ RbcDeliverFinalityStepCompletesCommittedDelivery
    /\ FinalitySourceActionCompletesCommittedDeliveryFromExactSourceStep
    /\ FinalitySourceActionMatchesCertifiedSourceStackStep
    /\ FinalitySourceActionMatchesFinalityLatchChangeStep
    /\ FinalitySourceActionInstallsFinalityCertificateStackStep
    /\ FinalitySourceActionSourceIsCommitOrDeliveryStep
    /\ FinalitySourceActionSourceEffectsAreExactStep
    /\ FinalitySourceActionQuorumGatesHoldStep
    /\ FinalityLatchChangeMatchesCertifiedSourceStackStep
    /\ FinalityLatchSourceIsCommitOrDeliveryStep
    /\ FinalityLatchSourceEffectsAreExactStep
    /\ FinalityLatchSourceQuorumGatesHoldStep
    /\ PhaseFinalizesByRbcDeliverStep
    /\ ~PhaseFinalizesByHonestCommitVoteStep
    /\ ~PhaseFinalizesByByzantineCommitVoteStep
    /\ RbcDeliverGood
    /\ ~HonestCommitVote
    /\ ~ByzantineEquivocateCommit
    /\ rbcState = "ReadyQuorum"
    /\ rbcState' = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ readyVotes' = readyVotes
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ headerSeen
    /\ headerSeen'
    /\ digestValid
    /\ digestValid'
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ ~committed
    /\ committed'
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitView = 0
    /\ commitView' = view'
    /\ FinalityCertificateStackPresent'
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ CommitViewMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'

RbcDeliveryEntryFinalityInstallsCommittedPostStateInvariantsStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered"
   /\ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) =>
    /\ RbcDeliveryEntryFinalityMatchesCertifiedSourceStackStep
    /\ committed'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitImpliesLiveVoteQuorum'
    /\ CommitImpliesLiveStakeQuorum'
    /\ CommitImpliesHonestSupport'
    /\ CommitImpliesDelivered'
    /\ CommitImpliesRbcEvidence'
    /\ FinalityCertificateStackPresent'
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ FinalityClearsNewViewHandoff'
    /\ CommitDisablesProgressActions'
    /\ CommitDisablesByzantineCommitVote'
    /\ CommitViewMatchesFinality'
    /\ CommitViewDoesNotLeadCurrentView'

RbcDeliveryEntryFinalityPostStateGateSplitStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered"
   /\ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) =>
    /\ RbcDeliveryEntryFinalityInstallsCommittedPostStateInvariantsStep
    /\ RbcDeliveryEntryPostGateSurfaceMatchesOutcomeStep
    /\ gst' = gst
    /\ CommitDisablesProgressActions'
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ ~TimeoutTickEnabled'
    /\ ~PostGstProgressEnabled'
    /\ (~gst =>
          /\ ~gst'
          /\ GstElapsedEnabled'
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~TimeoutTickEnabled'
          /\ ~ByzantineFaultEnabled'
          /\ ~PostGstProgressEnabled')
    /\ (gst =>
          /\ gst'
          /\ ~GstElapsedEnabled'
          /\ CommittedGstDisablesEveryAction'
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~TimeoutTickEnabled'
          /\ ~ByzantineFaultEnabled'
          /\ ~PostGstProgressEnabled')

RbcDeliveryEntryFinalityPreGstPostStateLeavesOnlyGstElapsedStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered"
   /\ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
   /\ ~gst) =>
    /\ RbcDeliveryEntryFinalityPostStateGateSplitStep
    /\ committed'
    /\ ~gst'
    /\ GstElapsedEnabled'
    /\ CommitDisablesProgressActions'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~TimeoutTickEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ ~PostGstProgressEnabled'

RbcDeliveryEntryFinalityPostGstPostStateIsTerminalStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered"
   /\ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
   /\ gst) =>
    /\ RbcDeliveryEntryFinalityPostStateGateSplitStep
    /\ committed'
    /\ gst'
    /\ ~GstElapsedEnabled'
    /\ CommittedGstDisablesEveryAction'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~TimeoutTickEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ ~PostGstProgressEnabled'

RbcDeliveryEntryPendingMatchesNonFinalWaitSurfaceStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered"
   /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) =>
    /\ RbcDeliveryEntryConsensusFrameMatchesOutcomeStep
    /\ RbcDeliveryEntryPendingInstallsCompleteWaitStateStep
    /\ rbcState = "ReadyQuorum"
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ phase' = phase
    /\ view' = view
    /\ gst' = gst
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ newViewVotes' = newViewVotes
    /\ ~committed
    /\ ~committed'
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceStake' = 0
    /\ commitView = 0
    /\ commitView' = 0
    /\ ~FinalityCertificateStackPresent'
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
    /\ DeliverImpliesEvidence'
    /\ NoCommitEvidenceBeforeCommit'
    /\ NoCommitViewBeforeCommit'
    /\ ViewEvidenceIsCompleteOrEmpty'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ \/ commitVotesHonest' + commitVotesByz' < CommitQuorum
       \/ stakeSigned' < StakeQuorum
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ (PostGstProgressEnabled' <=>
          \/ HonestProposeEnabled'
          \/ HonestPrepareVoteEnabled'
          \/ HonestCommitVoteEnabled'
          \/ HonestNewViewVoteEnabled')
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
    /\ (~gst =>
          /\ ~gst'
          /\ GstElapsedEnabled'
          /\ TimeoutTickEnabled')
    /\ (gst =>
          /\ gst'
          /\ ~GstElapsedEnabled'
          /\ (TimeoutTickEnabled' <=> ~PostGstProgressEnabled'))

RbcDeliveryEntryPendingPostStateTimerGateSplitStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered"
   /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) =>
    /\ RbcDeliveryEntryPendingMatchesNonFinalWaitSurfaceStep
    /\ ~committed'
    /\ gst' = gst
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ (PostGstProgressEnabled' <=>
          \/ HonestProposeEnabled'
          \/ HonestPrepareVoteEnabled'
          \/ HonestCommitVoteEnabled'
          \/ HonestNewViewVoteEnabled')
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
    /\ (~gst =>
          /\ ~gst'
          /\ GstElapsedEnabled'
          /\ TimeoutTickEnabled')
    /\ (gst =>
          /\ gst'
          /\ ~GstElapsedEnabled'
          /\ (TimeoutTickEnabled' <=> ~PostGstProgressEnabled')
          /\ (PostGstProgressEnabled' => ~TimeoutTickEnabled')
          /\ (~PostGstProgressEnabled' => TimeoutTickEnabled')
          /\ ((HonestProposeEnabled' \/ HonestPrepareVoteEnabled' \/
               HonestCommitVoteEnabled' \/ HonestNewViewVoteEnabled') =>
                ~TimeoutTickEnabled')
          /\ ((/\ ~HonestProposeEnabled'
                /\ ~HonestPrepareVoteEnabled'
                /\ ~HonestCommitVoteEnabled'
                /\ ~HonestNewViewVoteEnabled') =>
                TimeoutTickEnabled'))

RbcDeliveryEntryPendingPreGstPostStateKeepsWaitTimersStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered"
   /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
   /\ ~gst) =>
    /\ RbcDeliveryEntryPendingPostStateTimerGateSplitStep
    /\ ~committed'
    /\ ~gst'
    /\ GstElapsedEnabled'
    /\ TimeoutTickEnabled'
    /\ ~FinalityCertificateStackPresent'
    /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ (PostGstProgressEnabled' <=>
          \/ HonestProposeEnabled'
          \/ HonestPrepareVoteEnabled'
          \/ HonestCommitVoteEnabled'
          \/ HonestNewViewVoteEnabled')
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))

RbcDeliveryEntryPendingPostGstPostStateTimeoutTracksProgressStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered"
   /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
   /\ gst) =>
    /\ RbcDeliveryEntryPendingPostStateTimerGateSplitStep
    /\ ~committed'
    /\ gst'
    /\ ~GstElapsedEnabled'
    /\ ~FinalityCertificateStackPresent'
    /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ (PostGstProgressEnabled' <=>
          \/ HonestProposeEnabled'
          \/ HonestPrepareVoteEnabled'
          \/ HonestCommitVoteEnabled'
          \/ HonestNewViewVoteEnabled')
    /\ (TimeoutTickEnabled' <=> ~PostGstProgressEnabled')
    /\ (PostGstProgressEnabled' => ~TimeoutTickEnabled')
    /\ (~PostGstProgressEnabled' => TimeoutTickEnabled')
    /\ ((HonestProposeEnabled' \/ HonestPrepareVoteEnabled' \/
         HonestCommitVoteEnabled' \/ HonestNewViewVoteEnabled') =>
          ~TimeoutTickEnabled')
    /\ ((/\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~HonestNewViewVoteEnabled') =>
          TimeoutTickEnabled')

RbcDeliveredEvidenceStableStep ==
  (rbcState = "Delivered") =>
    /\ rbcState' = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ readyVotes' = readyVotes
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ digestValid
    /\ digestValid' = digestValid

RbcDeliveredFinalityOnlyByCommitVoteStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate
    /\ FinalityLatchSourceIsCommitOrDeliveryStep
    /\ RbcDeliveredEvidenceStableStep
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ ~RbcDeliverGood
    /\ \/ /\ HonestCommitVote
          /\ ~ByzantineEquivocateCommit
       \/ /\ ~HonestCommitVote
          /\ ByzantineEquivocateCommit

RbcDeliveredFinalityStepCompletesCommittedDelivery ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityOnlyByCommitVoteStep
    /\ FinalitySourceActionCompletesCommittedDeliveryFromExactSourceStep
    /\ FinalityLatchChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ CommittedPhaseEntryCompletesCommittedDeliveryFromExactSourceStep
    /\ FinalityCertificateStackPresent'
    /\ CommitDisablesProgressActions'
    /\ rbcState' = "Delivered"
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'

RbcDeliveredFinalityCommitsCurrentViewStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityStepCompletesCommittedDelivery
    /\ FinalitySourceActionCommitsCurrentViewStep
    /\ FinalitySourceActionNeverCarriesNewViewHandoffStep
    /\ view' = view
    /\ commitView = 0
    /\ commitView' = view
    /\ commitView' = view'
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)

RbcDeliveredFinalityLeavesOnlyGstElapsedGateStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityStepCompletesCommittedDelivery
    /\ RbcDeliveredFinalityCommitsCurrentViewStep
    /\ FinalitySourceActionPreservesGstStep
    /\ FinalitySourceActionLeavesOnlyGstElapsedGateStep
    /\ gst' = gst
    /\ CommitDisablesProgressActions'
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (gst' => CommittedGstDisablesEveryAction')

RbcDeliveredFinalityInstallsCommitCertificateWitnessesStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityStepCompletesCommittedDelivery
    /\ RbcDeliveredFinalityOnlyByCommitVoteStep
    /\ FinalitySourceActionInstallsCommitCertificateWitnessesStep
    /\ CommitCertificateWitnessesInstallWithFinalityLatchStep
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum

RbcDeliveredFinalityMatchesCommitCertificateWitnessChangeStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityInstallsCommitCertificateWitnessesStep
    /\ RbcDeliveredFinalityCommitsCurrentViewStep
    /\ FinalitySourceActionMatchesCommitCertificateWitnessChangeStep
    /\ CommittedPhaseEntryMatchesCommitCertificateWitnessChangeStep
    /\ CommitCertificateWitnessChangeMatchesCertifiedFinalityStackStep
    /\ CommitCertificateWitnessChangeInstallsCommitViewWitnessStep
    /\ CommitCertificateWitnessChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ commitEvidenceVotes' # commitEvidenceVotes
    /\ commitEvidenceStake' # commitEvidenceStake
    /\ commitView' = view'
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)

RbcDeliveredFinalityMatchesCommitViewWitnessChangeStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityMatchesCommitCertificateWitnessChangeStep
    /\ RbcDeliveredFinalityCommitsCurrentViewStep
    /\ FinalitySourceActionInstallsCommitViewWitnessStep
    /\ CommitViewWitnessInstallsWithFinalityLatchStep
    /\ commitView = 0
    /\ commitView' = view'
    /\ ((commitView' # commitView) <=> (view' # 0))
    /\ ((commitView' # commitView) =>
          /\ FinalitySourceActionMatchesCommitViewWitnessChangeStep
          /\ CommitViewWitnessChangeMatchesCertifiedFinalityStackStep
          /\ CommitViewWitnessChangeInstallsCommitCertificateWitnessesStep
          /\ CommitViewWitnessChangeCompletesCommittedDeliveryFromExactSourceStep
          /\ CommitViewWitnessChangePreservesGstStep
          /\ CommitViewWitnessChangeLeavesOnlyGstElapsedGateStep)
    /\ (commitView' = commitView =>
          /\ view' = 0
          /\ commitView' = 0)

RbcDeliveredFinalityMatchesLiveCommitGateCrossingStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityMatchesCommitViewWitnessChangeStep
    /\ FinalitySourceActionMatchesLiveCommitGateCrossingStep
    /\ FinalityLatchChangeMatchesLiveCommitGateCrossingStep
    /\ CommittedPhaseEntryMatchesLiveCommitGateCrossingStep
    /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, rbcState)
    /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ rbcState' = "Delivered"
    /\ commitVotesHonest' + commitVotesByz' >= CommitQuorum
    /\ stakeSigned' >= StakeQuorum
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'

RbcDeliveredFinalityDisablesProgressAfterCommittedDeliveryStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityMatchesLiveCommitGateCrossingStep
    /\ FinalitySourceActionDisablesProgressAfterCommittedDeliveryStep
    /\ CommittedPhaseEntryDisablesProgressActionsStep
    /\ CommitDisablesProgressActions'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~TimeoutTickEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ ~PostGstProgressEnabled'

RbcDeliveredFinalityMatchesCertifiedSourceStackStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityDisablesProgressAfterCommittedDeliveryStep
    /\ FinalitySourceActionMatchesCertifiedSourceStackStep
    /\ FinalitySourceActionMatchesFinalityLatchChangeStep
    /\ FinalitySourceActionSourceIsCommitOrDeliveryStep
    /\ FinalitySourceActionSourceEffectsAreExactStep
    /\ FinalitySourceActionQuorumGatesHoldStep
    /\ FinalityLatchChangeMatchesCertifiedSourceStackStep
    /\ FinalityLatchSourceIsCommitOrDeliveryStep
    /\ FinalityLatchSourceEffectsAreExactStep
    /\ FinalityLatchSourceQuorumGatesHoldStep
    /\ ~RbcDeliverGood
    /\ \/ /\ HonestCommitVote
          /\ ~ByzantineEquivocateCommit
       \/ /\ ~HonestCommitVote
          /\ ByzantineEquivocateCommit
    /\ rbcState = "Delivered"
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ chunkCount' = chunkCount
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ prepareVotes >= CommitQuorum
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' + commitVotesByz' >= CommitQuorum
    /\ commitVotesHonest' >= HonestCommitSupportThreshold
    /\ stakeSigned' >= StakeQuorum

RbcDeliveredFinalityInstallsFinalityCertificateStackStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityMatchesCertifiedSourceStackStep
    /\ FinalitySourceActionInstallsFinalityCertificateStackStep
    /\ FinalityCertificateStackPresent'
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ CommitViewMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ phase' = "Committed"
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum
    /\ commitView' = view'

RbcDeliveredFinalityMatchesCommittedPhaseEntryStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityInstallsFinalityCertificateStackStep
    /\ FinalitySourceActionMatchesCommittedPhaseEntryStep
    /\ FinalityLatchChangeEntersCommittedPhaseStep
    /\ CommittedPhaseEntryMatchesFinalityLatchStep
    /\ CommittedPhaseEntryInstallsCompleteStackStep
    /\ CommittedPhaseEntryOnlyByFinalitySourceStep
    /\ CommittedPhaseEntryMatchesCertifiedFinalityStackStep
    /\ CommittedPhaseEntryMatchesExactFinalitySourceEffectsStep
    /\ CommittedPhaseEntryNeverCarriesNewViewHandoffStep
    /\ CommittedPhaseEntryCommitsCurrentViewStep
    /\ CommittedPhaseEntryPreservesGstStep
    /\ CommittedPhaseEntryLeavesOnlyGstElapsedGateStep
    /\ CommittedPhaseEntryCompletesCommittedDeliveryFromExactSourceStep
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ committed' # committed
    /\ view' = view
    /\ commitView = 0
    /\ commitView' = view'

RbcDeliveredFinalityMatchesCommitArtifactsChangeStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityMatchesCommittedPhaseEntryStep
    /\ FinalitySourceActionMatchesCommitArtifactsChangeStep
    /\ CommittedPhaseEntryMatchesCommitArtifactsChangeStep
    /\ CommitArtifactsChangeMatchesCertifiedFinalityStackStep
    /\ CommitArtifactsChangeCompletesCommittedDeliveryFromExactSourceStep
    /\ CommitArtifactsChangeCommitsCurrentViewStep
    /\ CommitArtifactsChangePreservesGstStep
    /\ CommitArtifactsChangeLeavesOnlyGstElapsedGateStep
    /\ (\/ commitView' # commitView
        \/ commitEvidenceVotes' # commitEvidenceVotes
        \/ commitEvidenceStake' # commitEvidenceStake)
    /\ commitEvidenceVotes' # commitEvidenceVotes
    /\ commitEvidenceStake' # commitEvidenceStake
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum
    /\ commitView' = view'

RbcDeliveredFinalityCouplesLatchAndCommitArtifactsStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityMatchesCommitArtifactsChangeStep
    /\ FinalityLatchSetInstallsCompleteStackStep
    /\ FinalityLatchAndArtifactsCoupledStep
    /\ CommitArtifactsInstallOnlyAtFinalityStep
    /\ CommitArtifactsOnlyChangeByFinalitySourceStep
    /\ committed' # committed
    /\ (\/ commitView' # commitView
        \/ commitEvidenceVotes' # commitEvidenceVotes
        \/ commitEvidenceStake' # commitEvidenceStake)
    /\ commitEvidenceVotes' # commitEvidenceVotes
    /\ commitEvidenceStake' # commitEvidenceStake
    /\ (\/ PhaseFinalizesByHonestCommitVoteStep
        \/ PhaseFinalizesByByzantineCommitVoteStep)
    /\ ~PhaseFinalizesByRbcDeliverStep

RbcDeliveredFinalityRecordsExactCommitVoteWitnessesStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityCouplesLatchAndCommitArtifactsStep
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ commitEvidenceVotes' # commitEvidenceVotes
    /\ commitEvidenceStake' # commitEvidenceStake
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' >= StakeQuorum
    /\ commitView' = view
    /\ commitView' = view'
    /\ view' = view
    /\ prepareVotes >= CommitQuorum
    /\ prepareVotes' = prepareVotes
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ (\/ /\ HonestCommitVote
          /\ ~ByzantineEquivocateCommit
          /\ PhaseFinalizesByHonestCommitVoteStep
          /\ commitVotesHonest' = commitVotesHonest + 1
          /\ commitVotesByz' = commitVotesByz
          /\ stakeSigned' = stakeSigned + StakePerHonestVote
          /\ commitEvidenceVotes' = commitVotesHonest + 1 + commitVotesByz
          /\ commitEvidenceStake' = stakeSigned + StakePerHonestVote
        \/ /\ ~HonestCommitVote
          /\ ByzantineEquivocateCommit
          /\ PhaseFinalizesByByzantineCommitVoteStep
          /\ commitVotesHonest' = commitVotesHonest
          /\ commitVotesByz' = commitVotesByz + 1
          /\ stakeSigned' = stakeSigned + StakePerByzVote
          /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz + 1
          /\ commitEvidenceStake' = stakeSigned + StakePerByzVote)
    /\ ~PhaseFinalizesByRbcDeliverStep

RbcDeliveredFinalityPreservesDeliveredRbcEvidenceStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityRecordsExactCommitVoteWitnessesStep
    /\ RbcDeliveredEvidenceStableStep
    /\ LiveCommitGateRbcEvidenceMatches
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, rbcState)
    /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ rbcState = "Delivered"
    /\ rbcState' = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ ~RbcDeliverGood

RbcDeliveredFinalityPreservesViewPrepareHandoffEvidenceStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityPreservesDeliveredRbcEvidenceStep
    /\ RbcDeliveredFinalityMatchesCommittedPhaseEntryStep
    /\ CommittedPhaseEntryNeverCarriesNewViewHandoffStep
    /\ CommittedPhaseEntryCommitsCurrentViewStep
    /\ FinalityLatchNeverCarriesNewViewHandoffStep
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ phase # "NewView"
    /\ phase' # "NewView"
    /\ ~HonestNewViewVoteEnabled
    /\ ~HonestNewViewVoteEnabled'
    /\ view' = view
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
    /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
    /\ prepareVotes >= CommitQuorum
    /\ prepareVotes' = prepareVotes
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ commitView = 0
    /\ commitView' = view'

RbcDeliveredFinalityHasExactProtocolFrameStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityPreservesViewPrepareHandoffEvidenceStep
    /\ RbcDeliveredFinalityLeavesOnlyGstElapsedGateStep
    /\ RbcDeliveredFinalityDisablesProgressAfterCommittedDeliveryStep
    /\ RbcDeliveredFinalityRecordsExactCommitVoteWitnessesStep
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ committed' # committed
    /\ gst' = gst
    /\ view' = view
    /\ prepareVotes' = prepareVotes
    /\ newViewVotes = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ rbcState' = rbcState
    /\ readyVotes' = readyVotes
    /\ chunkCount' = chunkCount
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ commitView = 0
    /\ commitView' = view'
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceStake' = stakeSigned'
    /\ CommitDisablesProgressActions'
    /\ (GstElapsedEnabled' <=> ~gst')

RbcDeliveredFinalityHasExactCommitVoteActionFrameStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityHasExactProtocolFrameStep
    /\ RbcDeliveredFinalityOnlyByCommitVoteStep
    /\ (\/ /\ HonestCommitVote
          /\ ~ByzantineEquivocateCommit
        \/ /\ ~HonestCommitVote
          /\ ByzantineEquivocateCommit)
    /\ ~HonestPropose
    /\ ~HonestPrepareVote
    /\ ~TimeoutTick
    /\ ~HonestNewViewVote
    /\ ~RbcInit
    /\ ~RbcChunkGood
    /\ ~RbcReadyGood
    /\ ~RbcDeliverGood
    /\ ~ByzantineFault
    /\ ~GstElapsed

RbcDeliveredFinalityInstallsCommittedPostStateInvariantsStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityHasExactCommitVoteActionFrameStep
    /\ RbcDeliveredFinalityInstallsFinalityCertificateStackStep
    /\ committed'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitImpliesLiveVoteQuorum'
    /\ CommitImpliesLiveStakeQuorum'
    /\ CommitImpliesHonestSupport'
    /\ CommitImpliesDelivered'
    /\ CommitImpliesRbcEvidence'
    /\ FinalityCertificateStackPresent'
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ FinalityClearsNewViewHandoff'
    /\ CommitDisablesProgressActions'
    /\ CommitDisablesByzantineCommitVote'
    /\ CommitViewMatchesFinality'
    /\ CommitViewDoesNotLeadCurrentView'

RbcDeliveredFinalityPostStateGateSplitStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed') =>
    /\ RbcDeliveredFinalityInstallsCommittedPostStateInvariantsStep
    /\ RbcDeliveredFinalityLeavesOnlyGstElapsedGateStep
    /\ gst' = gst
    /\ CommitDisablesProgressActions'
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (~gst =>
          /\ ~gst'
          /\ GstElapsedEnabled'
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~TimeoutTickEnabled'
          /\ ~ByzantineFaultEnabled'
          /\ ~PostGstProgressEnabled')
    /\ (gst =>
          /\ gst'
          /\ ~GstElapsedEnabled'
          /\ CommittedGstDisablesEveryAction')

RbcDeliveredFinalityPreGstPostStateLeavesOnlyGstElapsedStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed'
   /\ ~gst) =>
    /\ RbcDeliveredFinalityPostStateGateSplitStep
    /\ committed'
    /\ ~gst'
    /\ GstElapsedEnabled'
    /\ CommitDisablesProgressActions'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~TimeoutTickEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ ~PostGstProgressEnabled'

RbcDeliveredFinalityPostGstPostStateIsTerminalStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ committed'
   /\ gst) =>
    /\ RbcDeliveredFinalityPostStateGateSplitStep
    /\ committed'
    /\ gst'
    /\ ~GstElapsedEnabled'
    /\ CommittedGstDisablesEveryAction'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~TimeoutTickEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ ~PostGstProgressEnabled'

RbcDeliveredDisablesRbcProgress ==
  rbcState = "Delivered" =>
    /\ ~RbcInitEnabled
    /\ ~RbcChunkGoodEnabled
    /\ ~RbcReadyGoodEnabled
    /\ ~RbcDeliverGoodEnabled
    /\ ~ByzantineFaultEnabled

RbcDeliveredWithoutFinalityWaitsForCommitEvidence ==
  (/\ rbcState = "Delivered"
   /\ ~committed) =>
    /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate
    /\ RbcDeliveredDisablesRbcProgress
    /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, rbcState)
    /\ phase # "Committed"
    /\ (phase = "CommitVote" => prepareVotes >= CommitQuorum)

DeliveredPendingCompleteWaitState ==
  /\ rbcState = "Delivered"
  /\ ~committed
  /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
  /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate
  /\ RbcDeliveredDisablesRbcProgress
  /\ DeliverImpliesEvidence
  /\ FinalityCertificateStackComplete
  /\ FinalityCertificateStackMatchesFinality
  /\ CommittedPhaseMatchesFinality
  /\ CommitCertificateMatchesFinality
  /\ LiveCommitGateMatchesFinality
  /\ LiveCommitGateRbcEvidenceMatches
  /\ ViewEvidenceIsCompleteOrEmpty
  /\ VoteCountersRespectRosterBudgets
  /\ StakeSignedMatchesVoteCounters
  /\ LiveStakeSignedIsBounded
  /\ CommitEvidenceIsBounded
  /\ CommitEvidenceIsCompleteOrEmpty
  /\ phase \in {"Propose", "Prepare", "CommitVote", "NewView"}
  /\ phase # "Committed"
  /\ (phase = "CommitVote" => prepareVotes >= CommitQuorum)
  /\ readyVotes >= CommitQuorum
  /\ chunkCount >= MaxChunks
  /\ headerSeen
  /\ digestValid
  /\ commitEvidenceVotes = 0
  /\ commitEvidenceStake = 0
  /\ commitView = 0
  /\ ~FinalityCertificateStackPresent
  /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, rbcState)
  /\ ~RbcInitEnabled
  /\ ~RbcChunkGoodEnabled
  /\ ~RbcReadyGoodEnabled
  /\ ~RbcDeliverGoodEnabled
  /\ ~ByzantineFaultEnabled
  /\ (PostGstProgressEnabled <=>
        \/ HonestProposeEnabled
        \/ HonestPrepareVoteEnabled
        \/ HonestCommitVoteEnabled
        \/ HonestNewViewVoteEnabled)
  /\ (GstElapsedEnabled <=> ~gst)
  /\ (TimeoutTickEnabled <=> (~gst \/ ~PostGstProgressEnabled))
  /\ (\/ GstElapsedEnabled
      \/ PostGstProgressEnabled
      \/ TimeoutTickEnabled)

RbcDeliveryEntryPendingInstallsDeliveredWaitPredicateStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered"
   /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) =>
    /\ RbcDeliveryEntryPendingMatchesNonFinalWaitSurfaceStep
    /\ RbcDeliveryEntryPendingPostStateTimerGateSplitStep
    /\ (~gst => RbcDeliveryEntryPendingPreGstPostStateKeepsWaitTimersStep)
    /\ (gst => RbcDeliveryEntryPendingPostGstPostStateTimeoutTracksProgressStep)
    /\ ~committed'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitVotePhaseRequiresPrepareQuorum'
    /\ phase' # "Committed"
    /\ (phase' = "CommitVote" => prepareVotes' >= CommitQuorum)
    /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
    /\ RbcDeliveredDisablesRbcProgress'
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ ~FinalityCertificateStackPresent'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'

RbcDeliveryEntryPendingOpensDeliveredPendingContinuationSurfaceStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered"
   /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")) =>
    /\ RbcDeliveryEntryPendingInstallsDeliveredWaitPredicateStep
    /\ rbcState' = "Delivered"
    /\ ~committed'
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
    /\ RbcDeliveredDisablesRbcProgress'
    /\ DeliverImpliesEvidence'
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ phase' # "Committed"
    /\ view' = view
    /\ gst' = gst
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~FinalityCertificateStackPresent'
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ (PostGstProgressEnabled' <=>
          \/ HonestProposeEnabled'
          \/ HonestPrepareVoteEnabled'
          \/ HonestCommitVoteEnabled'
          \/ HonestNewViewVoteEnabled')
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
    /\ (~gst => RbcDeliveryEntryPendingPreGstPostStateKeepsWaitTimersStep)
    /\ (gst => RbcDeliveryEntryPendingPostGstPostStateTimeoutTracksProgressStep)

RbcDeliveryEntryCommitEvidenceBranchOpensExactContinuationStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitArtifactsMatchOutcomeStep
    /\ RbcDeliveryEntryConsensusFrameMatchesOutcomeStep
    /\ RbcDeliverStepHandoffMatchesCommitEvidenceStep
    /\ (committed' <=> CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered"))
    /\ (FinalityCertificateStackPresent' <=>
          CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered"))
    /\ (CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState') <=>
          CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered"))
    /\ (CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
          /\ RbcDeliveryEntryFinalityMatchesCertifiedSourceStackStep
          /\ RbcDeliveryEntryFinalityInstallsCommittedPostStateInvariantsStep
          /\ RbcDeliveryEntryFinalityPostStateGateSplitStep
          /\ committed'
          /\ phase' = "Committed"
          /\ FinalityCertificateStackPresent'
          /\ CommitDisablesProgressActions'
          /\ ~PostGstProgressEnabled'
          /\ ~TimeoutTickEnabled')
    /\ (~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") =>
          /\ RbcDeliveryEntryPendingOpensDeliveredPendingContinuationSurfaceStep
          /\ ~committed'
          /\ phase' # "Committed"
          /\ ~FinalityCertificateStackPresent'
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))

RbcDeliveryEntryCommitEvidenceBranchMatchesExclusiveOutcomeStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchOpensExactContinuationStep
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitViewMatchesFinality'
    /\ (committed' <=> CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered"))
    /\ (committed' <=> FinalityCertificateStackPresent')
    /\ (committed' <=> (phase' = "Committed"))
    /\ (committed' <=>
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake' >= StakeQuorum)
    /\ (committed' <=>
          CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))
    /\ (committed' =>
          /\ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
          /\ FinalityCertificateStackPresent'
          /\ phase' = "Committed"
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake' >= StakeQuorum
          /\ commitView' = view'
          /\ rbcState' = "Delivered"
          /\ readyVotes' >= CommitQuorum
          /\ chunkCount' >= MaxChunks
          /\ headerSeen'
          /\ digestValid')
    /\ (~committed' =>
          /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ ~FinalityCertificateStackPresent'
          /\ phase' # "Committed"
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))

RbcDeliveryEntryCommitEvidenceBranchMatchesExclusiveGateOutcomeStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesExclusiveOutcomeStep
    /\ RbcDeliveryEntryPostGateSurfaceMatchesOutcomeStep
    /\ gst' = gst
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (committed' =>
          /\ RbcDeliveryEntryFinalityPostStateGateSplitStep
          /\ CommitDisablesProgressActions'
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~TimeoutTickEnabled'
          /\ ~ByzantineFaultEnabled'
          /\ ~PostGstProgressEnabled'
          /\ (~gst' => GstElapsedEnabled')
          /\ (gst' =>
                /\ ~GstElapsedEnabled'
                /\ CommittedGstDisablesEveryAction'))
    /\ (~committed' =>
          /\ RbcDeliveryEntryPendingPostStateTimerGateSplitStep
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ phase' # "Committed"
          /\ ~FinalityCertificateStackPresent'
          /\ (PostGstProgressEnabled' <=>
                \/ HonestProposeEnabled'
                \/ HonestPrepareVoteEnabled'
                \/ HonestCommitVoteEnabled'
                \/ HonestNewViewVoteEnabled')
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
          /\ (~gst' =>
                /\ GstElapsedEnabled'
                /\ TimeoutTickEnabled')
          /\ (gst' =>
                /\ ~GstElapsedEnabled'
                /\ (TimeoutTickEnabled' <=> ~PostGstProgressEnabled')
                /\ (PostGstProgressEnabled' => ~TimeoutTickEnabled')
                /\ (~PostGstProgressEnabled' => TimeoutTickEnabled')))

RbcDeliveryEntryCommitEvidenceBranchMatchesExactConsensusFrameStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesExclusiveGateOutcomeStep
    /\ RbcDeliveryEntryConsensusFrameMatchesOutcomeStep
    /\ RbcDeliveryEntryMatchesReadyQuorumExitAndCommitBranchStep
    /\ RbcDeliverGood
    /\ rbcState = "ReadyQuorum"
    /\ rbcState' = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ readyVotes' = readyVotes
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ view' = view
    /\ gst' = gst
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ ~committed
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ phase' = IF committed' THEN "Committed" ELSE phase
    /\ newViewVotes' = IF committed' THEN 0 ELSE newViewVotes
    /\ (committed' =>
          /\ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
          /\ phase = "CommitVote"
          /\ phase' = "Committed"
          /\ newViewVotes = 0
          /\ newViewVotes' = 0
          /\ prepareVotes >= CommitQuorum
          /\ commitVotesHonest + commitVotesByz >= CommitQuorum
          /\ commitVotesHonest >= HonestCommitSupportThreshold
          /\ stakeSigned >= StakeQuorum
          /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
          /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz
          /\ commitEvidenceStake' = stakeSigned
          /\ commitView' = view)
    /\ (~committed' =>
          /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
          /\ phase' = phase
          /\ phase # "Committed"
          /\ newViewVotes' = newViewVotes
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ \/ commitVotesHonest + commitVotesByz < CommitQuorum
             \/ stakeSigned < StakeQuorum)

RbcDeliveryEntryCommitEvidenceBranchMatchesExactActionSourceStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesExactConsensusFrameStep
    /\ RbcDeliveryEntryOnlyByDeliverStep
    /\ RbcDeliverStepPreservesCompleteEvidence
    /\ Next
    /\ RbcDeliverGood
    /\ RbcDeliverGoodEnabled
    /\ RbcStateDeliverByDeliverStep
    /\ (Next <=> RbcDeliverGood)
    /\ ~HonestPropose
    /\ ~HonestPrepareVote
    /\ ~HonestCommitVote
    /\ ~ByzantineEquivocateCommit
    /\ ~TimeoutTick
    /\ ~HonestNewViewVote
    /\ ~RbcInit
    /\ ~RbcChunkGood
    /\ ~RbcReadyGood
    /\ ~ByzantineFault
    /\ ~GstElapsed
    /\ (committed' =>
          /\ PhaseFinalizesByRbcDeliverStep
          /\ ~PhaseFinalizesByHonestCommitVoteStep
          /\ ~PhaseFinalizesByByzantineCommitVoteStep
          /\ RbcDeliverFinalityStepInstallsCommitArtifacts)
    /\ (~committed' =>
          /\ ~PhaseFinalizesByRbcDeliverStep
          /\ ~PhaseFinalizesByHonestCommitVoteStep
          /\ ~PhaseFinalizesByByzantineCommitVoteStep
          /\ RbcDeliverPendingStepPreservesPreFinalityArtifacts)

RbcDeliveryEntryCommitEvidenceBranchMatchesCertifiedOrPendingStackStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesExactActionSourceStep
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitViewMatchesFinality'
    /\ (committed' =>
          /\ RbcDeliveryEntryFinalityMatchesCertifiedSourceStackStep
          /\ RbcDeliveryEntryFinalityInstallsCommittedPostStateInvariantsStep
          /\ FinalitySourceActionCompletesCommittedDeliveryFromExactSourceStep
          /\ FinalitySourceActionMatchesCertifiedSourceStackStep
          /\ FinalitySourceActionMatchesFinalityLatchChangeStep
          /\ FinalitySourceActionInstallsFinalityCertificateStackStep
          /\ FinalitySourceActionSourceIsCommitOrDeliveryStep
          /\ FinalitySourceActionSourceEffectsAreExactStep
          /\ FinalitySourceActionQuorumGatesHoldStep
          /\ FinalityLatchChangeMatchesCertifiedSourceStackStep
          /\ FinalityLatchSourceIsCommitOrDeliveryStep
          /\ FinalityLatchSourceEffectsAreExactStep
          /\ FinalityLatchSourceQuorumGatesHoldStep
          /\ FinalityCertificateStackPresent'
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitView' = view')
    /\ (~committed' =>
          /\ RbcDeliveryEntryPendingOpensDeliveredPendingContinuationSurfaceStep
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
          /\ ~FinalityCertificateStackPresent'
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))

RbcDeliveryEntryCommitEvidenceBranchMatchesExactWitnessSurfaceStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesCertifiedOrPendingStackStep
    /\ CommitCertificateWitnessesInstallWithFinalityLatchStep
    /\ CommitCertificateWitnessComponentsChangeTogetherStep
    /\ CommitViewWitnessChangesOnlyOnNonzeroFinalityStep
    /\ CommitViewWitnessInstallsWithFinalityLatchStep
    /\ commitEvidenceVotes' =
          IF committed' THEN commitVotesHonest' + commitVotesByz' ELSE 0
    /\ commitEvidenceStake' =
          IF committed' THEN stakeSigned' ELSE 0
    /\ commitView' =
          IF committed' THEN view' ELSE 0
    /\ ((commitEvidenceVotes' # commitEvidenceVotes) <=> committed')
    /\ ((commitEvidenceStake' # commitEvidenceStake) <=> committed')
    /\ ((commitView' # commitView) <=> (committed' /\ view' # 0))
    /\ (committed' =>
          /\ FinalitySourceActionInstallsCommitCertificateWitnessesStep
          /\ FinalitySourceActionMatchesCommitCertificateWitnessChangeStep
          /\ FinalitySourceActionInstallsCommitViewWitnessStep
          /\ CommitCertificateWitnessChangeMatchesCertifiedFinalityStackStep
          /\ CommitCertificateWitnessChangeInstallsCommitViewWitnessStep
          /\ commitEvidenceVotes = 0
          /\ commitEvidenceStake = 0
          /\ commitView = 0
          /\ commitEvidenceVotes' # commitEvidenceVotes
          /\ commitEvidenceStake' # commitEvidenceStake
          /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz
          /\ commitEvidenceStake' = stakeSigned
          /\ commitView' = view'
          /\ ((commitView' # commitView) =>
                /\ FinalitySourceActionMatchesCommitViewWitnessChangeStep
                /\ CommitViewWitnessChangeMatchesCertifiedFinalityStackStep
                /\ CommitViewWitnessChangeInstallsCommitCertificateWitnessesStep))
    /\ (~committed' =>
          /\ commitEvidenceVotes = 0
          /\ commitEvidenceStake = 0
          /\ commitView = 0
          /\ commitEvidenceVotes' = commitEvidenceVotes
          /\ commitEvidenceStake' = commitEvidenceStake
          /\ commitView' = commitView
          /\ ~(commitEvidenceVotes' # commitEvidenceVotes)
          /\ ~(commitEvidenceStake' # commitEvidenceStake)
          /\ ~(commitView' # commitView))

RbcDeliveryEntryCommitEvidenceBranchMatchesLiveCommitGateCrossingStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesExactWitnessSurfaceStep
    /\ RbcDeliverGood
    /\ rbcState = "ReadyQuorum"
    /\ ~committed
    /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, rbcState)
    /\ (CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered") <=>
          CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))
    /\ (committed' <=>
          /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, rbcState)
          /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))
    /\ ((committed' # committed) <=> committed')
    /\ (committed' =>
          /\ FinalitySourceActionMatchesLiveCommitGateCrossingStep
          /\ FinalityLatchChangeMatchesLiveCommitGateCrossingStep
          /\ CommittedPhaseEntryMatchesLiveCommitGateCrossingStep
          /\ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
          /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ LiveCommitGateMatchesFinality'
          /\ LiveCommitGateRbcEvidenceMatches'
          /\ commitVotesHonest' + commitVotesByz' >= CommitQuorum
          /\ stakeSigned' >= StakeQuorum)
    /\ (~committed' =>
          /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ ~( /\ ~CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, rbcState)
                /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))
          /\ phase' # "Committed"
          /\ ~FinalityCertificateStackPresent'
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0)

RbcDeliveryEntryCommitEvidenceBranchMatchesContinuationModeStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesLiveCommitGateCrossingStep
    /\ (committed' <=> (phase' = "Committed"))
    /\ (committed' <=> FinalityCertificateStackPresent')
    /\ (committed' <=>
          CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))
    /\ (committed' =>
          /\ RbcDeliveryEntryFinalityPostStateGateSplitStep
          /\ CommitDisablesProgressActions'
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~TimeoutTickEnabled'
          /\ ~ByzantineFaultEnabled'
          /\ ~PostGstProgressEnabled'
          /\ (GstElapsedEnabled' <=> ~gst')
          /\ (gst' => CommittedGstDisablesEveryAction'))
    /\ (~committed' =>
          /\ RbcDeliveryEntryPendingPostStateTimerGateSplitStep
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
          /\ RbcDeliveredDisablesRbcProgress'
          /\ phase' # "Committed"
          /\ ~FinalityCertificateStackPresent'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ (phase' = "CommitVote" => prepareVotes' >= CommitQuorum)
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~ByzantineFaultEnabled'
          /\ (PostGstProgressEnabled' <=>
                \/ HonestProposeEnabled'
                \/ HonestPrepareVoteEnabled'
                \/ HonestCommitVoteEnabled'
                \/ HonestNewViewVoteEnabled')
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
          /\ (~gst' =>
                /\ GstElapsedEnabled'
                /\ TimeoutTickEnabled')
          /\ (gst' =>
                /\ ~GstElapsedEnabled'
                /\ (TimeoutTickEnabled' <=> ~PostGstProgressEnabled')))

RbcDeliveryEntryCommitEvidenceBranchMatchesViewHandoffSurfaceStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesContinuationModeStep
    /\ view' = view
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ newViewVotes' = IF committed' THEN 0 ELSE newViewVotes
    /\ FinalityClearsNewViewHandoff'
    /\ (committed' =>
          /\ FinalitySourceActionCommitsCurrentViewStep
          /\ FinalitySourceActionNeverCarriesNewViewHandoffStep
          /\ FinalityLatchNeverCarriesNewViewHandoffStep
          /\ CommittedPhaseEntryNeverCarriesNewViewHandoffStep
          /\ phase = "CommitVote"
          /\ phase' = "Committed"
          /\ phase # "NewView"
          /\ phase' # "NewView"
          /\ ~HonestNewViewVoteEnabled
          /\ ~HonestNewViewVoteEnabled'
          /\ newViewVotes = 0
          /\ newViewVotes' = 0
          /\ viewEvidenceVotes' = viewEvidenceVotes
          /\ (view = 0 \/ viewEvidenceVotes >= ViewQuorum)
          /\ (commitView' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
          /\ commitView' = view')
    /\ (~committed' =>
          /\ phase' = phase
          /\ newViewVotes' = newViewVotes
          /\ viewEvidenceVotes' = viewEvidenceVotes
          /\ commitView' = 0
          /\ ~FinalityCertificateStackPresent'
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ (phase' = "CommitVote" => prepareVotes' >= CommitQuorum))

RbcDeliveryEntryCommitEvidenceBranchMatchesDeliveredEvidenceSurfaceStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesViewHandoffSurfaceStep
    /\ RbcDeliverStepPreservesCompleteEvidence
    /\ DeliverImpliesEvidence'
    /\ CommitImpliesRbcEvidence'
    /\ RbcDeliveredDisablesRbcProgress'
    /\ rbcState = "ReadyQuorum"
    /\ rbcState' = "Delivered"
    /\ readyVotes >= CommitQuorum
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ (committed' =>
          /\ CommitImpliesRbcEvidence'
          /\ FinalityCertificateStackPresent'
          /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake' >= StakeQuorum)
    /\ (~committed' =>
          /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ ~FinalityCertificateStackPresent'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0)

RbcDeliveryEntryCommitEvidenceBranchMatchesGstTimerSurfaceStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesDeliveredEvidenceSurfaceStep
    /\ gst' = gst
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (committed' =>
          /\ RbcDeliveryEntryFinalityPostStateGateSplitStep
          /\ ~TimeoutTickEnabled'
          /\ ~PostGstProgressEnabled'
          /\ (~gst' =>
                /\ RbcDeliveryEntryFinalityPreGstPostStateLeavesOnlyGstElapsedStep
                /\ GstElapsedEnabled'
                /\ ~TimeoutTickEnabled'
                /\ ~PostGstProgressEnabled')
          /\ (gst' =>
                /\ RbcDeliveryEntryFinalityPostGstPostStateIsTerminalStep
                /\ ~GstElapsedEnabled'
                /\ CommittedGstDisablesEveryAction'
                /\ ~TimeoutTickEnabled'
                /\ ~PostGstProgressEnabled'))
    /\ (~committed' =>
          /\ RbcDeliveryEntryPendingPostStateTimerGateSplitStep
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
          /\ (~gst' =>
                /\ RbcDeliveryEntryPendingPreGstPostStateKeepsWaitTimersStep
                /\ GstElapsedEnabled'
                /\ TimeoutTickEnabled')
          /\ (gst' =>
                /\ RbcDeliveryEntryPendingPostGstPostStateTimeoutTracksProgressStep
                /\ ~GstElapsedEnabled'
                /\ (TimeoutTickEnabled' <=> ~PostGstProgressEnabled')
                /\ (PostGstProgressEnabled' => ~TimeoutTickEnabled')
                /\ (~PostGstProgressEnabled' => TimeoutTickEnabled')))

RbcDeliveryEntryCommitEvidenceBranchMatchesProgressActionSurfaceStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesGstTimerSurfaceStep
    /\ (committed' =>
          /\ CommitDisablesProgressActions'
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ ~PostGstProgressEnabled')
    /\ (~committed' =>
          /\ phase' # "Committed"
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ (HonestProposeEnabled' <=> phase' = "Propose")
          /\ (HonestPrepareVoteEnabled' <=>
                /\ phase' = "Prepare"
                /\ prepareVotes' < N - F)
          /\ (HonestCommitVoteEnabled' <=>
                /\ phase' = "CommitVote"
                /\ commitVotesHonest' < N - F)
          /\ (ByzantineCommitVoteEnabled' <=>
                /\ phase' = "CommitVote"
                /\ commitVotesByz' < F)
          /\ (HonestNewViewVoteEnabled' <=>
                /\ phase' = "NewView"
                /\ newViewVotes' < N - F)
          /\ (PostGstProgressEnabled' <=>
                \/ HonestProposeEnabled'
                \/ HonestPrepareVoteEnabled'
                \/ HonestCommitVoteEnabled'
                \/ HonestNewViewVoteEnabled')
          /\ ((phase' = "CommitVote") =>
                /\ prepareVotes' >= CommitQuorum
                /\ ~HonestProposeEnabled'
                /\ ~HonestPrepareVoteEnabled'
                /\ ~HonestNewViewVoteEnabled')
          /\ ((phase' # "CommitVote") =>
                /\ ~HonestCommitVoteEnabled'
                /\ ~ByzantineCommitVoteEnabled'))

RbcDeliveryEntryCommitEvidenceBranchMatchesVoteBudgetSurfaceStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesProgressActionSurfaceStep
    /\ VoteCountersRespectRosterBudgets
    /\ VoteCountersRespectRosterBudgets'
    /\ StakeSignedMatchesVoteCounters
    /\ StakeSignedMatchesVoteCounters'
    /\ LiveStakeSignedIsBounded
    /\ LiveStakeSignedIsBounded'
    /\ CommitEvidenceIsBounded
    /\ CommitEvidenceIsBounded'
    /\ CommitEvidenceIsCompleteOrEmpty'
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitEvidenceVotes' =
          IF committed' THEN commitVotesHonest' + commitVotesByz' ELSE 0
    /\ commitEvidenceStake' =
          IF committed' THEN stakeSigned' ELSE 0
    /\ (committed' =>
          /\ CommitEvidenceMatchesVoteCounters'
          /\ CommitImpliesPrepareQuorum'
          /\ CommitImpliesLiveVoteQuorum'
          /\ CommitImpliesLiveStakeQuorum'
          /\ CommitImpliesHonestSupport'
          /\ prepareVotes' >= CommitQuorum
          /\ commitVotesHonest' + commitVotesByz' >= CommitQuorum
          /\ commitVotesHonest' >= HonestCommitSupportThreshold
          /\ stakeSigned' >= StakeQuorum
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake' >= StakeQuorum)
    /\ (~committed' =>
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ \/ commitVotesHonest' + commitVotesByz' < CommitQuorum
             \/ stakeSigned' < StakeQuorum
          /\ (phase' = "CommitVote" => prepareVotes' >= CommitQuorum))

RbcDeliveryEntryCommitEvidenceBranchMatchesThresholdClassifierStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesVoteBudgetSurfaceStep
    /\ (committed' <=>
          /\ commitVotesHonest' + commitVotesByz' >= CommitQuorum
          /\ stakeSigned' >= StakeQuorum)
    /\ (~committed' <=>
          \/ commitVotesHonest' + commitVotesByz' < CommitQuorum
          \/ stakeSigned' < StakeQuorum)
    /\ ((commitEvidenceVotes' >= CommitQuorum) <=> committed')
    /\ ((commitEvidenceStake' >= StakeQuorum) <=> committed')
    /\ ((commitEvidenceVotes' # 0) <=> committed')
    /\ ((commitEvidenceStake' # 0) <=> committed')
    /\ (committed' =>
          /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake' >= StakeQuorum
          /\ commitVotesHonest' >= HonestCommitSupportThreshold)
    /\ (~committed' =>
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~FinalityCertificateStackPresent')

RbcDeliveryEntryCommitEvidenceBranchMatchesPendingCommitVoteProgressSplitStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesThresholdClassifierStep
    /\ (committed' =>
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled'
          /\ ~TimeoutTickEnabled'
          /\ ~PostGstProgressEnabled')
    /\ ((/\ ~committed'
         /\ phase' = "CommitVote") =>
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ prepareVotes' >= CommitQuorum
          /\ \/ commitVotesHonest' + commitVotesByz' < CommitQuorum
             \/ stakeSigned' < StakeQuorum
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ (HonestCommitVoteEnabled' <=> commitVotesHonest' < N - F)
          /\ (ByzantineCommitVoteEnabled' <=> commitVotesByz' < F)
          /\ (PostGstProgressEnabled' <=> HonestCommitVoteEnabled')
          /\ (TimeoutTickEnabled' <=>
                \/ ~gst'
                \/ ~HonestCommitVoteEnabled')
          /\ (gst' => (TimeoutTickEnabled' <=> ~HonestCommitVoteEnabled')))
    /\ ((/\ ~committed'
         /\ phase' # "CommitVote") =>
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled')

RbcDeliveryEntryCommitEvidenceBranchMatchesPendingNonCommitVoteProgressSplitStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesPendingCommitVoteProgressSplitStep
    /\ (committed' =>
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ ~TimeoutTickEnabled'
          /\ ~PostGstProgressEnabled')
    /\ ((/\ ~committed'
         /\ phase' # "CommitVote") =>
          phase' \in {"Propose", "Prepare", "NewView"})
    /\ ((/\ ~committed'
         /\ phase' = "Propose") =>
          /\ HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ PostGstProgressEnabled'
          /\ (TimeoutTickEnabled' <=> ~gst'))
    /\ ((/\ ~committed'
         /\ phase' = "Prepare") =>
          /\ ~HonestProposeEnabled'
          /\ (HonestPrepareVoteEnabled' <=> prepareVotes' < N - F)
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ (PostGstProgressEnabled' <=> HonestPrepareVoteEnabled')
          /\ (TimeoutTickEnabled' <=>
                \/ ~gst'
                \/ ~HonestPrepareVoteEnabled')
          /\ (gst' => (TimeoutTickEnabled' <=> ~HonestPrepareVoteEnabled')))
    /\ ((/\ ~committed'
         /\ phase' = "NewView") =>
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled'
          /\ (HonestNewViewVoteEnabled' <=> newViewVotes' < N - F)
          /\ (PostGstProgressEnabled' <=> HonestNewViewVoteEnabled')
          /\ (TimeoutTickEnabled' <=>
                \/ ~gst'
                \/ ~HonestNewViewVoteEnabled')
          /\ (gst' => (TimeoutTickEnabled' <=> ~HonestNewViewVoteEnabled')))

RbcDeliveryEntryCommitEvidenceBranchMatchesPendingProgressPartitionStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesPendingNonCommitVoteProgressSplitStep
    /\ (committed' =>
          /\ phase' = "Committed"
          /\ ~PostGstProgressEnabled'
          /\ ~TimeoutTickEnabled')
    /\ (~committed' =>
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ (PostGstProgressEnabled' <=>
                \/ phase' = "Propose"
                \/ /\ phase' = "Prepare"
                   /\ prepareVotes' < N - F
                \/ /\ phase' = "CommitVote"
                   /\ commitVotesHonest' < N - F
                \/ /\ phase' = "NewView"
                   /\ newViewVotes' < N - F)
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
          /\ (~gst' => TimeoutTickEnabled')
          /\ (gst' => (TimeoutTickEnabled' <=> ~PostGstProgressEnabled'))
          /\ ((/\ gst'
               /\ PostGstProgressEnabled') => ~TimeoutTickEnabled')
          /\ ((/\ gst'
               /\ ~PostGstProgressEnabled') => TimeoutTickEnabled'))

RbcDeliveryEntryCommitEvidenceBranchMatchesPostStateClassifierStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesPendingProgressPartitionStep
    /\ (committed' <=> phase' = "Committed")
    /\ (committed' <=> FinalityCertificateStackPresent')
    /\ (committed' <=>
          CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))
    /\ (committed' =>
          /\ phase' = "Committed"
          /\ FinalityCertificateStackPresent'
          /\ CommitDisablesProgressActions'
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake' >= StakeQuorum
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitView' = view'
          /\ newViewVotes' = 0
          /\ ~PostGstProgressEnabled'
          /\ ~TimeoutTickEnabled')
    /\ (~committed' =>
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ ~FinalityCertificateStackPresent'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ (PostGstProgressEnabled' <=>
                \/ phase' = "Propose"
                \/ /\ phase' = "Prepare"
                   /\ prepareVotes' < N - F
                \/ /\ phase' = "CommitVote"
                   /\ commitVotesHonest' < N - F
                \/ /\ phase' = "NewView"
                   /\ newViewVotes' < N - F)
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled')))

RbcDeliveryEntryCommitEvidenceBranchMatchesCertificateProgressDisjointnessStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesPostStateClassifierStep
    /\ (FinalityCertificateStackPresent' <=>
          /\ phase' = "Committed"
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake' >= StakeQuorum
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitView' = view'
          /\ ~PostGstProgressEnabled'
          /\ ~TimeoutTickEnabled')
    /\ (~FinalityCertificateStackPresent' <=>
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled')))
    /\ ~(FinalityCertificateStackPresent' /\ PostGstProgressEnabled')
    /\ ~(FinalityCertificateStackPresent' /\ TimeoutTickEnabled')
    /\ (PostGstProgressEnabled' => ~FinalityCertificateStackPresent')
    /\ (TimeoutTickEnabled' => ~FinalityCertificateStackPresent')
    /\ ((commitEvidenceVotes' # 0) => FinalityCertificateStackPresent')
    /\ ((commitEvidenceStake' # 0) => FinalityCertificateStackPresent')
    /\ ((commitView' # 0) => FinalityCertificateStackPresent')

RbcDeliveryEntryCommitEvidenceBranchMatchesActionFamilyClassifierStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesCertificateProgressDisjointnessStep
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (committed' =>
          /\ CommitDisablesProgressActions'
          /\ ~PostGstProgressEnabled'
          /\ ~TimeoutTickEnabled'
          /\ (~gst' =>
                /\ GstElapsedEnabled'
                /\ ~TimeoutTickEnabled'
                /\ ~PostGstProgressEnabled')
          /\ (gst' =>
                /\ ~GstElapsedEnabled'
                /\ CommittedGstDisablesEveryAction'))
    /\ (~committed' =>
          /\ ~FinalityCertificateStackPresent'
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ (PostGstProgressEnabled' <=>
                \/ HonestProposeEnabled'
                \/ HonestPrepareVoteEnabled'
                \/ HonestCommitVoteEnabled'
                \/ HonestNewViewVoteEnabled')
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
          /\ ((PostGstProgressEnabled' \/ TimeoutTickEnabled') =>
                ~FinalityCertificateStackPresent'))

RbcDeliveryEntryCommitEvidenceBranchMatchesByzantineCommitVoteBoundaryStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesActionFamilyClassifierStep
    /\ Next
    /\ RbcDeliverGood
    /\ (Next <=> RbcDeliverGood)
    /\ (committed' =>
          /\ CommitDisablesProgressActions'
          /\ ~ByzantineCommitVoteEnabled'
          /\ ~PostGstProgressEnabled'
          /\ ~TimeoutTickEnabled')
    /\ (~committed' =>
          /\ (ByzantineCommitVoteEnabled' <=>
                /\ phase' = "CommitVote"
                /\ commitVotesByz' < F)
          /\ ((phase' # "CommitVote") => ~ByzantineCommitVoteEnabled')
          /\ ((/\ phase' = "CommitVote"
               /\ commitVotesByz' >= F) => ~ByzantineCommitVoteEnabled')
          /\ (ByzantineCommitVoteEnabled' =>
                /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
                /\ prepareVotes' >= CommitQuorum
                /\ ~FinalityCertificateStackPresent'
                /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
                /\ ~HonestProposeEnabled'
                /\ ~HonestPrepareVoteEnabled'
                /\ ~HonestNewViewVoteEnabled'
                /\ (PostGstProgressEnabled' <=> HonestCommitVoteEnabled')
                /\ (TimeoutTickEnabled' <=> (~gst' \/ ~HonestCommitVoteEnabled')))
          /\ ((/\ ByzantineCommitVoteEnabled'
               /\ gst'
               /\ HonestCommitVoteEnabled') =>
                /\ PostGstProgressEnabled'
                /\ ~TimeoutTickEnabled')
          /\ ((/\ ByzantineCommitVoteEnabled'
               /\ gst'
               /\ ~HonestCommitVoteEnabled') =>
                /\ ~PostGstProgressEnabled'
                /\ TimeoutTickEnabled'))

RbcDeliveryEntryCommitEvidenceBranchMatchesResidualGatePartitionStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesByzantineCommitVoteBoundaryStep
    /\ (committed' =>
          /\ ((\/ GstElapsedEnabled'
               \/ HonestProposeEnabled'
               \/ HonestPrepareVoteEnabled'
               \/ HonestCommitVoteEnabled'
               \/ ByzantineCommitVoteEnabled'
               \/ HonestNewViewVoteEnabled'
               \/ RbcInitEnabled'
               \/ RbcChunkGoodEnabled'
               \/ RbcReadyGoodEnabled'
               \/ RbcDeliverGoodEnabled'
               \/ TimeoutTickEnabled'
               \/ ByzantineFaultEnabled'
               \/ PostGstProgressEnabled') <=> ~gst')
          /\ (~gst' =>
                /\ GstElapsedEnabled'
                /\ ~TimeoutTickEnabled'
                /\ ~PostGstProgressEnabled')
          /\ (gst' =>
                /\ ~GstElapsedEnabled'
                /\ CommittedGstDisablesEveryAction'))
    /\ (~committed' =>
          /\ (\/ GstElapsedEnabled'
              \/ PostGstProgressEnabled'
              \/ TimeoutTickEnabled')
          /\ ((\/ HonestProposeEnabled'
               \/ HonestPrepareVoteEnabled'
               \/ HonestCommitVoteEnabled'
               \/ HonestNewViewVoteEnabled') <=> PostGstProgressEnabled')
          /\ (ByzantineCommitVoteEnabled' =>
                /\ phase' = "CommitVote"
                /\ (PostGstProgressEnabled' <=> HonestCommitVoteEnabled'))
          /\ (~gst' =>
                /\ GstElapsedEnabled'
                /\ TimeoutTickEnabled')
          /\ (gst' =>
                /\ ~GstElapsedEnabled'
                /\ (TimeoutTickEnabled' <=> ~PostGstProgressEnabled'))
          /\ ((PostGstProgressEnabled' \/ TimeoutTickEnabled') =>
                ~FinalityCertificateStackPresent'))

RbcDeliveryEntryCommitEvidenceBranchMatchesCompleteHandoffStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesResidualGatePartitionStep
    /\ RbcDeliveryEntryCommitEvidenceBranchOpensExactContinuationStep
    /\ Next
    /\ RbcDeliverGood
    /\ (Next <=> RbcDeliverGood)
    /\ rbcState = "ReadyQuorum"
    /\ rbcState' = "Delivered"
    /\ (committed' <=>
          CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered"))
    /\ (committed' <=> FinalityCertificateStackPresent')
    /\ (committed' <=> phase' = "Committed")
    /\ (committed' =>
          /\ phase' = "Committed"
          /\ FinalityCertificateStackPresent'
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitView' = view'
          /\ CommitDisablesProgressActions'
          /\ ~PostGstProgressEnabled'
          /\ ~TimeoutTickEnabled'
          /\ ((\/ GstElapsedEnabled'
               \/ HonestProposeEnabled'
               \/ HonestPrepareVoteEnabled'
               \/ HonestCommitVoteEnabled'
               \/ ByzantineCommitVoteEnabled'
               \/ HonestNewViewVoteEnabled'
               \/ RbcInitEnabled'
               \/ RbcChunkGoodEnabled'
               \/ RbcReadyGoodEnabled'
               \/ RbcDeliverGoodEnabled'
               \/ TimeoutTickEnabled'
               \/ ByzantineFaultEnabled'
               \/ PostGstProgressEnabled') <=> ~gst'))
    /\ (~committed' =>
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ ~FinalityCertificateStackPresent'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ (PostGstProgressEnabled' <=>
                \/ HonestProposeEnabled'
                \/ HonestPrepareVoteEnabled'
                \/ HonestCommitVoteEnabled'
                \/ HonestNewViewVoteEnabled')
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
          /\ (\/ GstElapsedEnabled'
              \/ PostGstProgressEnabled'
              \/ TimeoutTickEnabled')
          /\ ((PostGstProgressEnabled' \/ TimeoutTickEnabled') =>
                ~FinalityCertificateStackPresent'))

RbcDeliveryEntryCommitEvidenceBranchSeedsContinuationStateStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchMatchesCompleteHandoffStep
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ DeliverImpliesEvidence'
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ gst' = gst
    /\ (committed' =>
          /\ FinalityCertificateStackPresent'
          /\ CommitImpliesRbcEvidence'
          /\ CommitDisablesProgressActions'
          /\ CommitViewMatchesFinality'
          /\ FinalityClearsNewViewHandoff'
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitView' = view'
          /\ newViewVotes' = 0
          /\ ~PostGstProgressEnabled'
          /\ ~TimeoutTickEnabled')
    /\ (~committed' =>
          /\ RbcDeliveryEntryPendingOpensDeliveredPendingContinuationSurfaceStep
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
          /\ RbcDeliveredDisablesRbcProgress'
          /\ CommitVotePhaseRequiresPrepareQuorum'
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ (phase' = "CommitVote" => prepareVotes' >= CommitQuorum)
          /\ view' = view
          /\ prepareVotes' = prepareVotes
          /\ commitVotesHonest' = commitVotesHonest
          /\ commitVotesByz' = commitVotesByz
          /\ stakeSigned' = stakeSigned
          /\ newViewVotes' = newViewVotes
          /\ viewEvidenceVotes' = viewEvidenceVotes
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~FinalityCertificateStackPresent'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~ByzantineFaultEnabled'
          /\ (PostGstProgressEnabled' <=>
                \/ HonestProposeEnabled'
                \/ HonestPrepareVoteEnabled'
                \/ HonestCommitVoteEnabled'
                \/ HonestNewViewVoteEnabled')
          /\ (GstElapsedEnabled' <=> ~gst')
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled')))

RbcDeliveryEntryCommitEvidenceBranchSeedsPendingActionSurfaceStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchSeedsContinuationStateStep
    /\ (~committed' =>
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ ((phase' = "Propose") =>
                /\ HonestProposeEnabled'
                /\ ~HonestPrepareVoteEnabled'
                /\ ~HonestCommitVoteEnabled'
                /\ ~ByzantineCommitVoteEnabled'
                /\ ~HonestNewViewVoteEnabled')
          /\ ((phase' = "Prepare") =>
                /\ ~HonestProposeEnabled'
                /\ (HonestPrepareVoteEnabled' <=> prepareVotes' < N - F)
                /\ ~HonestCommitVoteEnabled'
                /\ ~ByzantineCommitVoteEnabled'
                /\ ~HonestNewViewVoteEnabled')
          /\ ((phase' = "CommitVote") =>
                /\ prepareVotes' >= CommitQuorum
                /\ ~HonestProposeEnabled'
                /\ ~HonestPrepareVoteEnabled'
                /\ (HonestCommitVoteEnabled' <=> commitVotesHonest' < N - F)
                /\ (ByzantineCommitVoteEnabled' <=> commitVotesByz' < F)
                /\ ~HonestNewViewVoteEnabled')
          /\ ((phase' = "NewView") =>
                /\ ~HonestProposeEnabled'
                /\ ~HonestPrepareVoteEnabled'
                /\ ~HonestCommitVoteEnabled'
                /\ ~ByzantineCommitVoteEnabled'
                /\ (HonestNewViewVoteEnabled' <=> newViewVotes' < N - F))
          /\ (PostGstProgressEnabled' <=>
                \/ HonestProposeEnabled'
                \/ HonestPrepareVoteEnabled'
                \/ HonestCommitVoteEnabled'
                \/ HonestNewViewVoteEnabled')
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
          /\ (GstElapsedEnabled' <=> ~gst')
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~ByzantineFaultEnabled')
    /\ (committed' =>
          /\ FinalityCertificateStackPresent'
          /\ CommitDisablesProgressActions'
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ ~PostGstProgressEnabled'
          /\ ~TimeoutTickEnabled')

RbcDeliveryEntryCommitEvidenceBranchSeedsPendingTimerSurfaceStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchSeedsPendingActionSurfaceStep
    /\ gst' = gst
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (committed' =>
          /\ ~PostGstProgressEnabled'
          /\ ~TimeoutTickEnabled'
          /\ (~gst' =>
                /\ GstElapsedEnabled'
                /\ ~TimeoutTickEnabled')
          /\ (gst' =>
                /\ ~GstElapsedEnabled'
                /\ CommittedGstDisablesEveryAction'))
    /\ (~committed' =>
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
          /\ (~gst' =>
                /\ GstElapsedEnabled'
                /\ TimeoutTickEnabled')
          /\ (gst' =>
                /\ ~GstElapsedEnabled'
                /\ (TimeoutTickEnabled' <=> ~PostGstProgressEnabled')
                /\ (PostGstProgressEnabled' => ~TimeoutTickEnabled')
                /\ (~PostGstProgressEnabled' => TimeoutTickEnabled')
                /\ ((HonestProposeEnabled' \/ HonestPrepareVoteEnabled' \/
                     HonestCommitVoteEnabled' \/ HonestNewViewVoteEnabled') =>
                      ~TimeoutTickEnabled')
                /\ ((/\ ~HonestProposeEnabled'
                      /\ ~HonestPrepareVoteEnabled'
                      /\ ~HonestCommitVoteEnabled'
                      /\ ~HonestNewViewVoteEnabled') =>
                      TimeoutTickEnabled')))

RbcDeliveryEntryCommitEvidenceBranchSeedsPendingCounterFrameStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchSeedsPendingTimerSurfaceStep
    /\ VoteCountersRespectRosterBudgets'
    /\ StakeSignedMatchesVoteCounters'
    /\ LiveStakeSignedIsBounded'
    /\ CommitEvidenceIsBounded'
    /\ CommitEvidenceIsCompleteOrEmpty'
    /\ view' = view
    /\ gst' = gst
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ (committed' =>
          /\ phase' = "Committed"
          /\ FinalityCertificateStackPresent'
          /\ CommitEvidenceMatchesVoteCounters'
          /\ CommitImpliesPrepareQuorum'
          /\ CommitImpliesLiveVoteQuorum'
          /\ CommitImpliesLiveStakeQuorum'
          /\ CommitImpliesHonestSupport'
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake' >= StakeQuorum
          /\ commitView' = view'
          /\ newViewVotes' = 0)
    /\ (~committed' =>
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ phase' = phase
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ newViewVotes' = newViewVotes
          /\ commitEvidenceVotes = 0
          /\ commitEvidenceStake = 0
          /\ commitView = 0
          /\ commitEvidenceVotes' = commitEvidenceVotes
          /\ commitEvidenceStake' = commitEvidenceStake
          /\ commitView' = commitView
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~FinalityCertificateStackPresent'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ \/ commitVotesHonest' + commitVotesByz' < CommitQuorum
             \/ stakeSigned' < StakeQuorum
          /\ (phase' = "CommitVote" => prepareVotes' >= CommitQuorum))

RbcDeliveryEntryCommitEvidenceBranchSeedsPendingCompleteWaitStateStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchSeedsPendingCounterFrameStep
    /\ (committed' =>
          /\ FinalityCertificateStackPresent'
          /\ CommitImpliesRbcEvidence'
          /\ CommitDisablesProgressActions'
          /\ CommitViewMatchesFinality'
          /\ FinalityClearsNewViewHandoff'
          /\ CommitEvidenceMatchesVoteCounters'
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitView' = view'
          /\ newViewVotes' = 0
          /\ ~PostGstProgressEnabled'
          /\ ~TimeoutTickEnabled')
    /\ (~committed' =>
          /\ RbcDeliveryEntryPendingOpensDeliveredPendingContinuationSurfaceStep
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
          /\ RbcDeliveredDisablesRbcProgress'
          /\ DeliverImpliesEvidence'
          /\ FinalityCertificateStackComplete'
          /\ FinalityCertificateStackMatchesFinality'
          /\ CommittedPhaseMatchesFinality'
          /\ CommitCertificateMatchesFinality'
          /\ LiveCommitGateMatchesFinality'
          /\ LiveCommitGateRbcEvidenceMatches'
          /\ rbcState' = "Delivered"
          /\ readyVotes' = readyVotes
          /\ readyVotes' >= CommitQuorum
          /\ chunkCount' = chunkCount
          /\ chunkCount' >= MaxChunks
          /\ headerSeen' = headerSeen
          /\ headerSeen'
          /\ digestValid' = digestValid
          /\ digestValid'
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ phase' # "Committed"
          /\ (phase' = "CommitVote" => prepareVotes' >= CommitQuorum)
          /\ view' = view
          /\ gst' = gst
          /\ prepareVotes' = prepareVotes
          /\ commitVotesHonest' = commitVotesHonest
          /\ commitVotesByz' = commitVotesByz
          /\ stakeSigned' = stakeSigned
          /\ newViewVotes' = newViewVotes
          /\ viewEvidenceVotes' = viewEvidenceVotes
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~FinalityCertificateStackPresent'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~ByzantineFaultEnabled'
          /\ (PostGstProgressEnabled' <=>
                \/ HonestProposeEnabled'
                \/ HonestPrepareVoteEnabled'
                \/ HonestCommitVoteEnabled'
                \/ HonestNewViewVoteEnabled')
          /\ (GstElapsedEnabled' <=> ~gst')
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
          /\ (\/ GstElapsedEnabled'
              \/ PostGstProgressEnabled'
              \/ TimeoutTickEnabled'))

RbcDeliveryEntryCommitEvidenceBranchHandsOffToDeliveredPendingWaitStateStep ==
  (/\ rbcState # "Delivered"
   /\ rbcState' = "Delivered") =>
    /\ RbcDeliveryEntryCommitEvidenceBranchSeedsPendingCompleteWaitStateStep
    /\ (committed' =>
          /\ ~DeliveredPendingCompleteWaitState'
          /\ FinalityCertificateStackPresent'
          /\ phase' = "Committed"
          /\ CommitDisablesProgressActions'
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitView' = view')
    /\ (~committed' =>
          /\ DeliveredPendingCompleteWaitState'
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
          /\ DeliverImpliesEvidence'
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~FinalityCertificateStackPresent'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ (\/ GstElapsedEnabled'
              \/ PostGstProgressEnabled'
              \/ TimeoutTickEnabled')
          /\ ((PostGstProgressEnabled' \/ TimeoutTickEnabled') =>
                /\ ~FinalityCertificateStackPresent'
                /\ phase' # "Committed"))

RbcDeliveredPendingHonestCommitVoteStepKeepsWaitState ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ HonestCommitVote
   /\ ~CanCommit(
        commitVotesHonest + 1,
        commitVotesByz,
        stakeSigned + StakePerHonestVote,
        rbcState
      )) =>
    /\ HonestCommitVotePendingStepPreservesPreFinalityArtifacts
    /\ HonestCommitVotePendingStepKeepsCommitVoteHandoff
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ ~committed'
    /\ phase' = "CommitVote"
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

RbcDeliveredPendingByzantineCommitVoteStepKeepsWaitState ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ ByzantineEquivocateCommit
   /\ ~CanCommit(
        commitVotesHonest,
        commitVotesByz + 1,
        stakeSigned + StakePerByzVote,
        rbcState
      )) =>
    /\ ByzantineCommitVotePendingStepPreservesPreFinalityArtifacts
    /\ ByzantineCommitVotePendingStepKeepsCommitVoteHandoff
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ ~committed'
    /\ phase' = "CommitVote"
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

RbcDeliveredPendingHonestCommitVoteStepCompletesFinality ==
  (/\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ HonestCommitVote
   /\ CanCommit(
        commitVotesHonest + 1,
        commitVotesByz,
        stakeSigned + StakePerHonestVote,
        rbcState
      )) =>
    /\ HonestCommitVoteFinalityStepCompletesCommittedDelivery
    /\ RbcDeliveredFinalityStepCompletesCommittedDelivery
    /\ RbcDeliveredFinalityPreservesDeliveredRbcEvidenceStep
    /\ rbcState = "Delivered"
    /\ rbcState' = "Delivered"
    /\ committed'
    /\ phase' = "Committed"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceStake' >= StakeQuorum
    /\ commitView' = view'
    /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ FinalityCertificateStackPresent'
    /\ CommitDisablesProgressActions'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

RbcDeliveredPendingByzantineCommitVoteStepCompletesFinality ==
  (/\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ ByzantineEquivocateCommit
   /\ CanCommit(
        commitVotesHonest,
        commitVotesByz + 1,
        stakeSigned + StakePerByzVote,
        rbcState
      )) =>
    /\ ByzantineCommitVoteFinalityStepCompletesCommittedDelivery
    /\ RbcDeliveredFinalityStepCompletesCommittedDelivery
    /\ RbcDeliveredFinalityPreservesDeliveredRbcEvidenceStep
    /\ rbcState = "Delivered"
    /\ rbcState' = "Delivered"
    /\ committed'
    /\ phase' = "Committed"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceStake' >= StakeQuorum
    /\ commitView' = view'
    /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ FinalityCertificateStackPresent'
    /\ CommitDisablesProgressActions'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

RbcDeliveredPendingPrepareVoteStepKeepsWaitState ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ HonestPrepareVote
   /\ prepareVotes + 1 < CommitQuorum) =>
    /\ PrepareVotePendingStepPreservesPreCommitArtifacts
    /\ PrepareVotePendingStepKeepsPrepareVoteHandoff
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ rbcState = "Delivered"
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ ~committed'
    /\ phase' = "Prepare"
    /\ prepareVotes' = prepareVotes + 1
    /\ prepareVotes' < CommitQuorum
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

RbcDeliveredPendingPrepareVoteStepStartsCommitVoteWaitState ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ HonestPrepareVote
   /\ prepareVotes + 1 >= CommitQuorum) =>
    /\ PrepareVoteQuorumStepEntersCommitVote
    /\ PrepareVoteQuorumStepStartsCommitVoteHandoff
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ rbcState = "Delivered"
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ ~committed'
    /\ phase' = "CommitVote"
    /\ prepareVotes' = prepareVotes + 1
    /\ prepareVotes' >= CommitQuorum
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ HonestCommitVoteEnabled'
    /\ (F > 0 => ByzantineCommitVoteEnabled')
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

RbcDeliveredPendingTimeoutStepStartsNewViewWaitState ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ TimeoutTick) =>
    /\ TimeoutTickStepStartsFreshNewView
    /\ TimeoutTickStepStartsNewViewVoteHandoff
    /\ TimeoutTickStepPreservesRbcEvidence
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ ~committed'
    /\ phase' = "NewView"
    /\ prepareVotes' = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ HonestNewViewVoteEnabled'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

RbcDeliveredPendingNewViewVoteStepKeepsWaitState ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ HonestNewViewVote
   /\ newViewVotes + 1 < ViewQuorum) =>
    /\ NewViewVotePendingStepPreservesPreProposalArtifacts
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ ~committed'
    /\ phase' = "NewView"
    /\ newViewVotes' = newViewVotes + 1
    /\ newViewVotes' < ViewQuorum
    /\ viewEvidenceVotes' = 0
    /\ prepareVotes' = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

RbcDeliveredPendingNewViewVoteStepStartsProposalWaitState ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ HonestNewViewVote
   /\ newViewVotes + 1 >= ViewQuorum) =>
    /\ NewViewVoteQuorumStepInstallsViewEvidence
    /\ NewViewVoteQuorumStepStartsProposalHandoff
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ ~committed'
    /\ phase' = "Propose"
    /\ newViewVotes' = newViewVotes + 1
    /\ newViewVotes' >= ViewQuorum
    /\ viewEvidenceVotes' = newViewVotes'
    /\ viewEvidenceVotes' >= ViewQuorum
    /\ prepareVotes' = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ HonestProposeEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

RbcDeliveredPendingHonestProposeStepStartsPrepareWaitState ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ HonestPropose) =>
    /\ HonestProposeStepStartsPrepareAndRbc
    /\ HonestProposeStepStartsPrepareVoteHandoff
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ ~committed'
    /\ phase' = "Prepare"
    /\ prepareVotes' = 0
    /\ newViewVotes' = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ HonestPrepareVoteEnabled'
    /\ ~HonestProposeEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

RbcDeliveredPendingGstElapsedStepKeepsWaitState ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ GstElapsed) =>
    /\ gst'
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ ~committed'
    /\ phase' = phase
    /\ view' = view
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

RbcDeliveredPendingNextStepCoveredByHandoffs ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ Next) =>
    \/ /\ HonestCommitVote
       /\ ~CanCommit(
            commitVotesHonest + 1,
            commitVotesByz,
            stakeSigned + StakePerHonestVote,
            rbcState
          )
       /\ RbcDeliveredPendingHonestCommitVoteStepKeepsWaitState
    \/ /\ HonestCommitVote
       /\ CanCommit(
            commitVotesHonest + 1,
            commitVotesByz,
            stakeSigned + StakePerHonestVote,
            rbcState
          )
       /\ RbcDeliveredPendingHonestCommitVoteStepCompletesFinality
    \/ /\ ByzantineEquivocateCommit
       /\ ~CanCommit(
            commitVotesHonest,
            commitVotesByz + 1,
            stakeSigned + StakePerByzVote,
            rbcState
          )
       /\ RbcDeliveredPendingByzantineCommitVoteStepKeepsWaitState
    \/ /\ ByzantineEquivocateCommit
       /\ CanCommit(
            commitVotesHonest,
            commitVotesByz + 1,
            stakeSigned + StakePerByzVote,
            rbcState
          )
       /\ RbcDeliveredPendingByzantineCommitVoteStepCompletesFinality
    \/ /\ HonestPrepareVote
       /\ prepareVotes + 1 < CommitQuorum
       /\ RbcDeliveredPendingPrepareVoteStepKeepsWaitState
    \/ /\ HonestPrepareVote
       /\ prepareVotes + 1 >= CommitQuorum
       /\ RbcDeliveredPendingPrepareVoteStepStartsCommitVoteWaitState
    \/ /\ TimeoutTick
       /\ RbcDeliveredPendingTimeoutStepStartsNewViewWaitState
    \/ /\ HonestNewViewVote
       /\ newViewVotes + 1 < ViewQuorum
       /\ RbcDeliveredPendingNewViewVoteStepKeepsWaitState
    \/ /\ HonestNewViewVote
       /\ newViewVotes + 1 >= ViewQuorum
       /\ RbcDeliveredPendingNewViewVoteStepStartsProposalWaitState
    \/ /\ HonestPropose
       /\ RbcDeliveredPendingHonestProposeStepStartsPrepareWaitState
    \/ /\ GstElapsed
       /\ RbcDeliveredPendingGstElapsedStepKeepsWaitState

RbcDeliveredPendingSpecStepStuttersOrTakesCoveredHandoffStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    \/ /\ Next
       /\ RbcDeliveredPendingNextStepCoveredByHandoffs
    \/ /\ ~Next
       /\ vars' = vars
       /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
       /\ rbcState' = "Delivered"
       /\ readyVotes' = readyVotes
       /\ readyVotes' >= CommitQuorum
       /\ chunkCount' = chunkCount
       /\ chunkCount' >= MaxChunks
       /\ headerSeen'
       /\ digestValid'
       /\ ~committed'
       /\ phase' = phase
       /\ view' = view
       /\ prepareVotes' = prepareVotes
       /\ commitVotesHonest' = commitVotesHonest
       /\ commitVotesByz' = commitVotesByz
       /\ stakeSigned' = stakeSigned
       /\ newViewVotes' = newViewVotes
       /\ viewEvidenceVotes' = viewEvidenceVotes
       /\ commitEvidenceVotes' = 0
       /\ commitEvidenceStake' = 0
       /\ commitView' = 0
       /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
       /\ ~RbcInitEnabled'
       /\ ~RbcChunkGoodEnabled'
       /\ ~RbcReadyGoodEnabled'
       /\ ~RbcDeliverGoodEnabled'
       /\ ~ByzantineFaultEnabled'

RbcDeliveredPendingSpecStepEndsInFinalityOrWaitStateStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepStuttersOrTakesCoveredHandoffStep
    /\ rbcState' = "Delivered"
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ \/ /\ committed'
          /\ phase' = "Committed"
          /\ FinalityCertificateStackPresent'
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitEvidenceStake' >= StakeQuorum
          /\ commitView' = view'
          /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ CommitDisablesProgressActions'
       \/ /\ ~committed'
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~ByzantineFaultEnabled'

RbcDeliveredPendingSpecStepPreservesDeliveredRbcEvidenceStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepEndsInFinalityOrWaitStateStep
    /\ rbcState' = rbcState
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid' = digestValid
    /\ digestValid'

RbcDeliveredPendingSpecStepCommitArtifactsMatchOutcomeStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepEndsInFinalityOrWaitStateStep
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ ((\/ commitEvidenceVotes' # commitEvidenceVotes
          \/ commitEvidenceStake' # commitEvidenceStake
          \/ commitView' # commitView) <=> committed')
    /\ ((commitEvidenceVotes' # commitEvidenceVotes) <=>
          (commitEvidenceStake' # commitEvidenceStake))
    /\ (commitView' # commitView => committed')
    /\ (committed' =>
          /\ phase' = "Committed"
          /\ FinalityCertificateStackPresent'
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitEvidenceStake' >= StakeQuorum
          /\ commitView' = view'
          /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))
    /\ (~committed' =>
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))

RbcDeliveredPendingSpecStepGstChangesOnlyByElapsedStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepStuttersOrTakesCoveredHandoffStep
    /\ (gst' # gst <=> GstElapsed)
    /\ (GstElapsed =>
          /\ RbcDeliveredPendingGstElapsedStepKeepsWaitState
          /\ GstElapsedStepOnlySetsGst
          /\ ~gst
          /\ gst'
          /\ ~committed'
          /\ phase' = phase
          /\ view' = view
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0)
    /\ (~GstElapsed =>
          /\ gst' = gst
          /\ (gst => gst'))

RbcDeliveredPendingSpecStepViewChangesOnlyByTimeoutStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepPreservesDeliveredRbcEvidenceStep
    /\ view' >= view
    /\ view' <= MaxView
    /\ (view' # view =>
          /\ TimeoutTick
          /\ RbcDeliveredPendingTimeoutStepStartsNewViewWaitState
          /\ view < MaxView
          /\ view' = view + 1
          /\ phase' = "NewView"
          /\ ~committed'
          /\ prepareVotes' = 0
          /\ commitVotesHonest' = 0
          /\ commitVotesByz' = 0
          /\ stakeSigned' = 0
          /\ newViewVotes' = 0
          /\ viewEvidenceVotes' = 0
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0)
    /\ (~TimeoutTick => view' = view)
    /\ (TimeoutTick =>
          /\ RbcDeliveredPendingTimeoutStepStartsNewViewWaitState
          /\ view' = IF view < MaxView THEN view + 1 ELSE MaxView
          /\ view' >= view
          /\ phase' = "NewView"
          /\ ~committed'
          /\ prepareVotes' = 0
          /\ commitVotesHonest' = 0
          /\ commitVotesByz' = 0
          /\ stakeSigned' = 0
          /\ newViewVotes' = 0
          /\ viewEvidenceVotes' = 0
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ HonestNewViewVoteEnabled')

RbcDeliveredPendingSpecStepViewEvidenceChangesOnlyByNewViewOrTimeoutStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepViewChangesOnlyByTimeoutStep
    /\ ViewEvidenceIsCompleteOrEmpty'
    /\ (viewEvidenceVotes' # viewEvidenceVotes =>
          /\ ViewEvidenceChangesOnlyByQuorumOrTimeoutStep
          /\ \/ /\ TimeoutTick
                /\ RbcDeliveredPendingTimeoutStepStartsNewViewWaitState
                /\ viewEvidenceVotes # 0
                /\ viewEvidenceVotes' = 0
             \/ /\ HonestNewViewVote
                /\ newViewVotes + 1 >= ViewQuorum
                /\ RbcDeliveredPendingNewViewVoteStepStartsProposalWaitState
                /\ viewEvidenceVotes = 0
                /\ viewEvidenceVotes' = newViewVotes'
                /\ viewEvidenceVotes' >= ViewQuorum)
    /\ (TimeoutTick =>
          /\ RbcDeliveredPendingTimeoutStepStartsNewViewWaitState
          /\ viewEvidenceVotes' = 0)
    /\ (HonestNewViewVote =>
          \/ /\ newViewVotes + 1 < ViewQuorum
             /\ RbcDeliveredPendingNewViewVoteStepKeepsWaitState
             /\ viewEvidenceVotes = 0
             /\ viewEvidenceVotes' = 0
          \/ /\ newViewVotes + 1 >= ViewQuorum
             /\ RbcDeliveredPendingNewViewVoteStepStartsProposalWaitState
             /\ viewEvidenceVotes = 0
             /\ viewEvidenceVotes' = newViewVotes'
             /\ viewEvidenceVotes' >= ViewQuorum)
    /\ (~(TimeoutTick \/ HonestNewViewVote) =>
          viewEvidenceVotes' = viewEvidenceVotes)

RbcDeliveredPendingSpecStepVoteCountersMatchHandoffStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepEndsInFinalityOrWaitStateStep
    /\ ((HonestPrepareVote /\ prepareVotes + 1 < CommitQuorum) =>
          /\ RbcDeliveredPendingPrepareVoteStepKeepsWaitState
          /\ phase' = "Prepare"
          /\ prepareVotes' = prepareVotes + 1
          /\ commitVotesHonest' = 0
          /\ commitVotesByz' = 0
          /\ stakeSigned' = 0
          /\ newViewVotes' = newViewVotes)
    /\ ((HonestPrepareVote /\ prepareVotes + 1 >= CommitQuorum) =>
          /\ RbcDeliveredPendingPrepareVoteStepStartsCommitVoteWaitState
          /\ phase' = "CommitVote"
          /\ prepareVotes' = prepareVotes + 1
          /\ commitVotesHonest' = 0
          /\ commitVotesByz' = 0
          /\ stakeSigned' = 0
          /\ newViewVotes' = newViewVotes)
    /\ ((HonestCommitVote /\
          ~CanCommit(
            commitVotesHonest + 1,
            commitVotesByz,
            stakeSigned + StakePerHonestVote,
            rbcState
          )) =>
          /\ RbcDeliveredPendingHonestCommitVoteStepKeepsWaitState
          /\ phase' = "CommitVote"
          /\ prepareVotes' = prepareVotes
          /\ commitVotesHonest' = commitVotesHonest + 1
          /\ commitVotesByz' = commitVotesByz
          /\ stakeSigned' = stakeSigned + StakePerHonestVote
          /\ newViewVotes' = newViewVotes)
    /\ ((HonestCommitVote /\
          CanCommit(
            commitVotesHonest + 1,
            commitVotesByz,
            stakeSigned + StakePerHonestVote,
            rbcState
          )) =>
          /\ RbcDeliveredPendingHonestCommitVoteStepCompletesFinality
          /\ phase' = "Committed"
          /\ prepareVotes' = prepareVotes
          /\ commitVotesHonest' = commitVotesHonest + 1
          /\ commitVotesByz' = commitVotesByz
          /\ stakeSigned' = stakeSigned + StakePerHonestVote
          /\ newViewVotes' = 0)
    /\ ((ByzantineEquivocateCommit /\
          ~CanCommit(
            commitVotesHonest,
            commitVotesByz + 1,
            stakeSigned + StakePerByzVote,
            rbcState
          )) =>
          /\ RbcDeliveredPendingByzantineCommitVoteStepKeepsWaitState
          /\ phase' = "CommitVote"
          /\ prepareVotes' = prepareVotes
          /\ commitVotesHonest' = commitVotesHonest
          /\ commitVotesByz' = commitVotesByz + 1
          /\ stakeSigned' = stakeSigned + StakePerByzVote
          /\ newViewVotes' = newViewVotes)
    /\ ((ByzantineEquivocateCommit /\
          CanCommit(
            commitVotesHonest,
            commitVotesByz + 1,
            stakeSigned + StakePerByzVote,
            rbcState
          )) =>
          /\ RbcDeliveredPendingByzantineCommitVoteStepCompletesFinality
          /\ phase' = "Committed"
          /\ prepareVotes' = prepareVotes
          /\ commitVotesHonest' = commitVotesHonest
          /\ commitVotesByz' = commitVotesByz + 1
          /\ stakeSigned' = stakeSigned + StakePerByzVote
          /\ newViewVotes' = 0)
    /\ (TimeoutTick =>
          /\ RbcDeliveredPendingTimeoutStepStartsNewViewWaitState
          /\ phase' = "NewView"
          /\ prepareVotes' = 0
          /\ commitVotesHonest' = 0
          /\ commitVotesByz' = 0
          /\ stakeSigned' = 0
          /\ newViewVotes' = 0)
    /\ ((HonestNewViewVote /\ newViewVotes + 1 < ViewQuorum) =>
          /\ RbcDeliveredPendingNewViewVoteStepKeepsWaitState
          /\ phase' = "NewView"
          /\ prepareVotes' = 0
          /\ commitVotesHonest' = 0
          /\ commitVotesByz' = 0
          /\ stakeSigned' = 0
          /\ newViewVotes' = newViewVotes + 1)
    /\ ((HonestNewViewVote /\ newViewVotes + 1 >= ViewQuorum) =>
          /\ RbcDeliveredPendingNewViewVoteStepStartsProposalWaitState
          /\ phase' = "Propose"
          /\ prepareVotes' = 0
          /\ commitVotesHonest' = 0
          /\ commitVotesByz' = 0
          /\ stakeSigned' = 0
          /\ newViewVotes' = newViewVotes + 1)
    /\ (HonestPropose =>
          /\ RbcDeliveredPendingHonestProposeStepStartsPrepareWaitState
          /\ phase' = "Prepare"
          /\ prepareVotes' = 0
          /\ commitVotesHonest' = 0
          /\ commitVotesByz' = 0
          /\ stakeSigned' = 0
          /\ newViewVotes' = 0)
    /\ (GstElapsed =>
          /\ RbcDeliveredPendingGstElapsedStepKeepsWaitState
          /\ phase' = phase
          /\ prepareVotes' = prepareVotes
          /\ commitVotesHonest' = commitVotesHonest
          /\ commitVotesByz' = commitVotesByz
          /\ stakeSigned' = stakeSigned
          /\ newViewVotes' = newViewVotes)
    /\ (~(HonestPrepareVote \/ HonestCommitVote \/ ByzantineEquivocateCommit \/
          TimeoutTick \/ HonestNewViewVote \/ HonestPropose \/ GstElapsed) =>
          /\ phase' = phase
          /\ prepareVotes' = prepareVotes
          /\ commitVotesHonest' = commitVotesHonest
          /\ commitVotesByz' = commitVotesByz
          /\ stakeSigned' = stakeSigned
          /\ newViewVotes' = newViewVotes)

RbcDeliveredPendingSpecStepPostGatesMatchHandoffStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepVoteCountersMatchHandoffStep
    /\ rbcState' = "Delivered"
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ (committed' =>
          /\ CommitDisablesProgressActions'
          /\ ~PostGstProgressEnabled')
    /\ (~committed' =>
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ (HonestProposeEnabled' <=> phase' = "Propose")
          /\ (HonestPrepareVoteEnabled' <=>
                /\ phase' = "Prepare"
                /\ prepareVotes' < N - F)
          /\ (HonestCommitVoteEnabled' <=>
                /\ phase' = "CommitVote"
                /\ commitVotesHonest' < N - F)
          /\ (ByzantineCommitVoteEnabled' <=>
                /\ phase' = "CommitVote"
                /\ commitVotesByz' < F)
          /\ (HonestNewViewVoteEnabled' <=>
                /\ phase' = "NewView"
                /\ newViewVotes' < N - F)
          /\ (PostGstProgressEnabled' <=>
                \/ HonestProposeEnabled'
                \/ HonestPrepareVoteEnabled'
                \/ HonestCommitVoteEnabled'
                \/ HonestNewViewVoteEnabled'))
    /\ ((HonestPrepareVote /\ prepareVotes + 1 < CommitQuorum) =>
          /\ RbcDeliveredPendingPrepareVoteStepKeepsWaitState
          /\ HonestPrepareVoteEnabled'
          /\ ~HonestProposeEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled')
    /\ ((HonestPrepareVote /\ prepareVotes + 1 >= CommitQuorum) =>
          /\ RbcDeliveredPendingPrepareVoteStepStartsCommitVoteWaitState
          /\ HonestCommitVoteEnabled'
          /\ (F > 0 => ByzantineCommitVoteEnabled')
          /\ (F = 0 => ~ByzantineCommitVoteEnabled')
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestProposeEnabled'
          /\ ~HonestNewViewVoteEnabled')
    /\ ((HonestCommitVote /\
          ~CanCommit(
            commitVotesHonest + 1,
            commitVotesByz,
            stakeSigned + StakePerHonestVote,
            rbcState
          )) =>
          /\ RbcDeliveredPendingHonestCommitVoteStepKeepsWaitState
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ (HonestCommitVoteEnabled' <=> commitVotesHonest' < N - F)
          /\ (ByzantineCommitVoteEnabled' <=> commitVotesByz' < F))
    /\ ((HonestCommitVote /\
          CanCommit(
            commitVotesHonest + 1,
            commitVotesByz,
            stakeSigned + StakePerHonestVote,
            rbcState
          )) =>
          /\ RbcDeliveredPendingHonestCommitVoteStepCompletesFinality
          /\ CommitDisablesProgressActions'
          /\ ~PostGstProgressEnabled')
    /\ ((ByzantineEquivocateCommit /\
          ~CanCommit(
            commitVotesHonest,
            commitVotesByz + 1,
            stakeSigned + StakePerByzVote,
            rbcState
          )) =>
          /\ RbcDeliveredPendingByzantineCommitVoteStepKeepsWaitState
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ (HonestCommitVoteEnabled' <=> commitVotesHonest' < N - F)
          /\ (ByzantineCommitVoteEnabled' <=> commitVotesByz' < F))
    /\ ((ByzantineEquivocateCommit /\
          CanCommit(
            commitVotesHonest,
            commitVotesByz + 1,
            stakeSigned + StakePerByzVote,
            rbcState
          )) =>
          /\ RbcDeliveredPendingByzantineCommitVoteStepCompletesFinality
          /\ CommitDisablesProgressActions'
          /\ ~PostGstProgressEnabled')
    /\ (TimeoutTick =>
          /\ RbcDeliveredPendingTimeoutStepStartsNewViewWaitState
          /\ HonestNewViewVoteEnabled'
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled')
    /\ ((HonestNewViewVote /\ newViewVotes + 1 < ViewQuorum) =>
          /\ RbcDeliveredPendingNewViewVoteStepKeepsWaitState
          /\ HonestNewViewVoteEnabled'
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled')
    /\ ((HonestNewViewVote /\ newViewVotes + 1 >= ViewQuorum) =>
          /\ RbcDeliveredPendingNewViewVoteStepStartsProposalWaitState
          /\ HonestProposeEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled')
    /\ (HonestPropose =>
          /\ RbcDeliveredPendingHonestProposeStepStartsPrepareWaitState
          /\ HonestPrepareVoteEnabled'
          /\ ~HonestProposeEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled')
    /\ (GstElapsed =>
          /\ RbcDeliveredPendingGstElapsedStepKeepsWaitState
          /\ (HonestProposeEnabled' <=> HonestProposeEnabled)
          /\ (HonestPrepareVoteEnabled' <=> HonestPrepareVoteEnabled)
          /\ (HonestCommitVoteEnabled' <=> HonestCommitVoteEnabled)
          /\ (ByzantineCommitVoteEnabled' <=> ByzantineCommitVoteEnabled)
          /\ (HonestNewViewVoteEnabled' <=> HonestNewViewVoteEnabled)
          /\ (PostGstProgressEnabled' <=> PostGstProgressEnabled))
    /\ (~(HonestPrepareVote \/ HonestCommitVote \/ ByzantineEquivocateCommit \/
          TimeoutTick \/ HonestNewViewVote \/ HonestPropose \/ GstElapsed) =>
          /\ (HonestProposeEnabled' <=> HonestProposeEnabled)
          /\ (HonestPrepareVoteEnabled' <=> HonestPrepareVoteEnabled)
          /\ (HonestCommitVoteEnabled' <=> HonestCommitVoteEnabled)
          /\ (ByzantineCommitVoteEnabled' <=> ByzantineCommitVoteEnabled)
          /\ (HonestNewViewVoteEnabled' <=> HonestNewViewVoteEnabled)
          /\ (PostGstProgressEnabled' <=> PostGstProgressEnabled))

RbcDeliveredPendingSpecStepTimerGatesMatchHandoffStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepPostGatesMatchHandoffStep
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (committed' =>
          /\ CommitDisablesProgressActions'
          /\ ~TimeoutTickEnabled'
          /\ ~PostGstProgressEnabled'
          /\ (gst' => ~GstElapsedEnabled')
          /\ (~gst' => GstElapsedEnabled'))
    /\ (~committed' =>
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
          /\ (gst' /\ PostGstProgressEnabled' => ~TimeoutTickEnabled')
          /\ (gst' /\ ~PostGstProgressEnabled' => TimeoutTickEnabled')
          /\ (~gst' => TimeoutTickEnabled'))
    /\ (GstElapsed =>
          /\ RbcDeliveredPendingGstElapsedStepKeepsWaitState
          /\ gst'
          /\ ~GstElapsedEnabled'
          /\ (TimeoutTickEnabled' <=> ~PostGstProgressEnabled'))
    /\ (~GstElapsed =>
          /\ gst' = gst
          /\ (GstElapsedEnabled' <=> GstElapsedEnabled))

RbcDeliveredPendingSpecStepFinalitySourceMatchesCommitVoteStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepTimerGatesMatchHandoffStep
    /\ (committed' =>
          /\ Next
          /\ RbcDeliveredFinalityOnlyByCommitVoteStep
          /\ RbcDeliveredFinalityStepCompletesCommittedDelivery
          /\ RbcDeliveredPendingSpecStepCommitArtifactsMatchOutcomeStep
          /\ phase = "CommitVote"
          /\ phase' = "Committed"
          /\ view' = view
          /\ gst' = gst
          /\ ~HonestPropose
          /\ ~HonestPrepareVote
          /\ ~TimeoutTick
          /\ ~HonestNewViewVote
          /\ ~RbcInit
          /\ ~RbcChunkGood
          /\ ~RbcReadyGood
          /\ ~RbcDeliverGood
          /\ ~ByzantineFault
          /\ ~GstElapsed
          /\ \/ /\ HonestCommitVote
                /\ ~ByzantineEquivocateCommit
                /\ CanCommit(
                     commitVotesHonest + 1,
                     commitVotesByz,
                     stakeSigned + StakePerHonestVote,
                     rbcState
                   )
                /\ RbcDeliveredPendingHonestCommitVoteStepCompletesFinality
                /\ commitVotesHonest' = commitVotesHonest + 1
                /\ commitVotesByz' = commitVotesByz
                /\ stakeSigned' = stakeSigned + StakePerHonestVote
             \/ /\ ~HonestCommitVote
                /\ ByzantineEquivocateCommit
                /\ CanCommit(
                     commitVotesHonest,
                     commitVotesByz + 1,
                     stakeSigned + StakePerByzVote,
                     rbcState
                   )
                /\ RbcDeliveredPendingByzantineCommitVoteStepCompletesFinality
                /\ commitVotesHonest' = commitVotesHonest
                /\ commitVotesByz' = commitVotesByz + 1
                /\ stakeSigned' = stakeSigned + StakePerByzVote
          /\ FinalityCertificateStackPresent'
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitView' = view'
          /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))
    /\ (~committed' =>
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ phase' # "Committed"
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ (HonestCommitVote =>
                ~CanCommit(
                  commitVotesHonest + 1,
                  commitVotesByz,
                  stakeSigned + StakePerHonestVote,
                  rbcState
                ))
          /\ (ByzantineEquivocateCommit =>
                ~CanCommit(
                  commitVotesHonest,
                  commitVotesByz + 1,
                  stakeSigned + StakePerByzVote,
                  rbcState
                )))

RbcDeliveredPendingSpecStepFinalityWitnessFrameStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepFinalitySourceMatchesCommitVoteStep
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ (committed' =>
          /\ RbcDeliveredFinalityCommitsCurrentViewStep
          /\ RbcDeliveredFinalityRecordsExactCommitVoteWitnessesStep
          /\ RbcDeliveredFinalityPreservesViewPrepareHandoffEvidenceStep
          /\ RbcDeliveredPendingSpecStepCommitArtifactsMatchOutcomeStep
          /\ phase = "CommitVote"
          /\ phase' = "Committed"
          /\ view' = view
          /\ prepareVotes' = prepareVotes
          /\ commitView' = view'
          /\ ((commitView' # commitView) <=> (view' # 0))
          /\ commitEvidenceVotes' # commitEvidenceVotes
          /\ commitEvidenceStake' # commitEvidenceStake
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ newViewVotes = 0
          /\ newViewVotes' = 0
          /\ viewEvidenceVotes' = viewEvidenceVotes
          /\ gst' = gst
          /\ FinalityCertificateStackPresent'
          /\ CommitViewMatchesFinality')
    /\ (~committed' =>
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ phase' # "Committed"
          /\ commitEvidenceVotes' = commitEvidenceVotes
          /\ commitEvidenceStake' = commitEvidenceStake
          /\ commitView' = commitView
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~FinalityCertificateStackPresent'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))

RbcDeliveredPendingSpecStepFinalityStackMatchesOutcomeStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepFinalityWitnessFrameStep
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitViewMatchesFinality'
    /\ (committed' <=> FinalityCertificateStackPresent')
    /\ (committed' <=> (phase' = "Committed"))
    /\ (committed' <=>
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake' >= StakeQuorum)
    /\ (committed' <=>
          CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))
    /\ (committed' =>
          /\ FinalityCertificateStackPresent'
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitView' = view'
          /\ rbcState' = "Delivered"
          /\ readyVotes' >= CommitQuorum
          /\ chunkCount' >= MaxChunks
          /\ headerSeen'
          /\ digestValid')
    /\ (~committed' =>
          /\ ~FinalityCertificateStackPresent'
          /\ phase' # "Committed"
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))

RbcDeliveredPendingSpecStepFinalityGateOutcomeStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepFinalityStackMatchesOutcomeStep
    /\ RbcDeliveredPendingSpecStepTimerGatesMatchHandoffStep
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (committed' =>
          /\ RbcDeliveredFinalityPostStateGateSplitStep
          /\ RbcDeliveredFinalityDisablesProgressAfterCommittedDeliveryStep
          /\ RbcDeliveredFinalityLeavesOnlyGstElapsedGateStep
          /\ CommitDisablesProgressActions'
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~TimeoutTickEnabled'
          /\ ~ByzantineFaultEnabled'
          /\ ~PostGstProgressEnabled'
          /\ (~gst' =>
                /\ GstElapsedEnabled'
                /\ CommittedPreGstOnlyEnablesGstElapsed')
          /\ (gst' =>
                /\ ~GstElapsedEnabled'
                /\ CommittedGstDisablesEveryAction'))
    /\ (~committed' =>
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ ~FinalityCertificateStackPresent'
          /\ phase' # "Committed"
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~ByzantineFaultEnabled'
          /\ (PostGstProgressEnabled' <=>
                \/ HonestProposeEnabled'
                \/ HonestPrepareVoteEnabled'
                \/ HonestCommitVoteEnabled'
                \/ HonestNewViewVoteEnabled')
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
          /\ (gst' /\ PostGstProgressEnabled' => ~TimeoutTickEnabled')
          /\ (gst' /\ ~PostGstProgressEnabled' => TimeoutTickEnabled')
          /\ (~gst' => TimeoutTickEnabled'))

RbcDeliveredPendingSpecStepFinalityQuorumOutcomeStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepFinalityGateOutcomeStep
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ (committed' <=>
          /\ commitVotesHonest' + commitVotesByz' >= CommitQuorum
          /\ stakeSigned' >= StakeQuorum
          /\ rbcState' = "Delivered")
    /\ (committed' =>
          /\ RbcDeliveredFinalityRecordsExactCommitVoteWitnessesStep
          /\ prepareVotes >= CommitQuorum
          /\ prepareVotes' = prepareVotes
          /\ commitVotesHonest' >= HonestCommitSupportThreshold
          /\ commitVotesHonest' + commitVotesByz' >= CommitQuorum
          /\ stakeSigned' >= StakeQuorum
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake' >= StakeQuorum
          /\ commitView' = view'
          /\ view' = view
          /\ readyVotes' >= CommitQuorum
          /\ chunkCount' >= MaxChunks
          /\ newViewVotes' = 0
          /\ (view' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
          /\ \/ /\ HonestCommitVote
                /\ ~ByzantineEquivocateCommit
                /\ commitVotesHonest' = commitVotesHonest + 1
                /\ commitVotesByz' = commitVotesByz
                /\ stakeSigned' = stakeSigned + StakePerHonestVote
                /\ commitEvidenceVotes' = commitVotesHonest + 1 + commitVotesByz
                /\ commitEvidenceStake' = stakeSigned + StakePerHonestVote
             \/ /\ ~HonestCommitVote
                /\ ByzantineEquivocateCommit
                /\ commitVotesHonest' = commitVotesHonest
                /\ commitVotesByz' = commitVotesByz + 1
                /\ stakeSigned' = stakeSigned + StakePerByzVote
                /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz + 1
                /\ commitEvidenceStake' = stakeSigned + StakePerByzVote)
    /\ (~committed' =>
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ \/ commitVotesHonest' + commitVotesByz' < CommitQuorum
             \/ stakeSigned' < StakeQuorum)

RbcDeliveredPendingSpecStepNonFinalHandoffPhaseShapeStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepFinalityQuorumOutcomeStep
    /\ RbcDeliveredPendingSpecStepViewEvidenceChangesOnlyByNewViewOrTimeoutStep
    /\ (~committed' =>
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ rbcState' = "Delivered"
          /\ readyVotes' >= CommitQuorum
          /\ chunkCount' >= MaxChunks
          /\ headerSeen'
          /\ digestValid'
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ViewEvidenceIsCompleteOrEmpty'
          /\ ~FinalityCertificateStackPresent'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ (phase' = "Propose" =>
                /\ HonestProposeEnabled'
                /\ prepareVotes' = 0
                /\ commitVotesHonest' = 0
                /\ commitVotesByz' = 0
                /\ stakeSigned' = 0
                /\ (view' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
                /\ (newViewVotes' = 0 \/ newViewVotes' >= ViewQuorum)
                /\ ~HonestPrepareVoteEnabled'
                /\ ~HonestCommitVoteEnabled'
                /\ ~ByzantineCommitVoteEnabled'
                /\ ~HonestNewViewVoteEnabled')
          /\ (phase' = "Prepare" =>
                /\ HonestPrepareVoteEnabled'
                /\ prepareVotes' < CommitQuorum
                /\ prepareVotes' < N - F
                /\ newViewVotes' = 0
                /\ commitVotesHonest' = 0
                /\ commitVotesByz' = 0
                /\ stakeSigned' = 0
                /\ (view' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
                /\ ~HonestProposeEnabled'
                /\ ~HonestCommitVoteEnabled'
                /\ ~ByzantineCommitVoteEnabled'
                /\ ~HonestNewViewVoteEnabled')
          /\ (phase' = "CommitVote" =>
                /\ prepareVotes' >= CommitQuorum
                /\ newViewVotes' = 0
                /\ (view' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
                /\ ~HonestProposeEnabled'
                /\ ~HonestPrepareVoteEnabled'
                /\ ~HonestNewViewVoteEnabled'
                /\ (HonestCommitVoteEnabled' <=> commitVotesHonest' < N - F)
                /\ (ByzantineCommitVoteEnabled' <=> commitVotesByz' < F))
          /\ (phase' = "NewView" =>
                /\ HonestNewViewVoteEnabled'
                /\ NewViewPhaseBelowQuorum'
                /\ prepareVotes' = 0
                /\ commitVotesHonest' = 0
                /\ commitVotesByz' = 0
                /\ stakeSigned' = 0
                /\ viewEvidenceVotes' = 0
                /\ newViewVotes' < ViewQuorum
                /\ ~HonestProposeEnabled'
                /\ ~HonestPrepareVoteEnabled'
                /\ ~HonestCommitVoteEnabled'
                /\ ~ByzantineCommitVoteEnabled'))

RbcDeliveredPendingSpecStepActionSurfaceClosedStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepNonFinalHandoffPhaseShapeStep
    /\ (Next =>
          /\ RbcDeliveredPendingNextStepCoveredByHandoffs
          /\ ~RbcInit
          /\ ~RbcChunkGood
          /\ ~RbcReadyGood
          /\ ~RbcDeliverGood
          /\ ~ByzantineFault
          /\ \/ HonestPropose
             \/ HonestPrepareVote
             \/ HonestCommitVote
             \/ ByzantineEquivocateCommit
             \/ TimeoutTick
             \/ HonestNewViewVote
             \/ GstElapsed)
    /\ (~Next =>
          /\ vars' = vars
          /\ ~committed'
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ phase' = phase
          /\ view' = view
          /\ gst' = gst
          /\ rbcState' = "Delivered"
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0)
    /\ (committed' =>
          /\ Next
          /\ RbcDeliveredPendingSpecStepFinalitySourceMatchesCommitVoteStep
          /\ ~HonestPropose
          /\ ~HonestPrepareVote
          /\ ~TimeoutTick
          /\ ~HonestNewViewVote
          /\ ~RbcInit
          /\ ~RbcChunkGood
          /\ ~RbcReadyGood
          /\ ~RbcDeliverGood
          /\ ~ByzantineFault
          /\ ~GstElapsed
          /\ \/ /\ HonestCommitVote
                /\ ~ByzantineEquivocateCommit
                /\ CanCommit(
                     commitVotesHonest + 1,
                     commitVotesByz,
                     stakeSigned + StakePerHonestVote,
                     rbcState
                   )
             \/ /\ ~HonestCommitVote
                /\ ByzantineEquivocateCommit
                /\ CanCommit(
                     commitVotesHonest,
                     commitVotesByz + 1,
                     stakeSigned + StakePerByzVote,
                     rbcState
                   ))
    /\ (~committed' =>
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ (Next =>
                \/ HonestPropose
                \/ HonestPrepareVote
                \/ TimeoutTick
                \/ HonestNewViewVote
                \/ GstElapsed
                \/ /\ HonestCommitVote
                   /\ ~CanCommit(
                        commitVotesHonest + 1,
                        commitVotesByz,
                        stakeSigned + StakePerHonestVote,
                        rbcState
                      )
                \/ /\ ByzantineEquivocateCommit
                   /\ ~CanCommit(
                        commitVotesHonest,
                        commitVotesByz + 1,
                        stakeSigned + StakePerByzVote,
                        rbcState
                      )))

RbcDeliveredPendingSpecStepPhaseChangeMatchesActionStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepActionSurfaceClosedStep
    /\ (phase' # phase =>
          /\ Next
          /\ ~GstElapsed
          /\ \/ /\ committed'
                /\ RbcDeliveredPendingSpecStepFinalitySourceMatchesCommitVoteStep
                /\ phase = "CommitVote"
                /\ phase' = "Committed"
             \/ /\ ~committed'
                /\ HonestPropose
                /\ RbcDeliveredPendingHonestProposeStepStartsPrepareWaitState
                /\ phase = "Propose"
                /\ phase' = "Prepare"
             \/ /\ ~committed'
                /\ HonestPrepareVote
                /\ prepareVotes + 1 >= CommitQuorum
                /\ RbcDeliveredPendingPrepareVoteStepStartsCommitVoteWaitState
                /\ phase = "Prepare"
                /\ phase' = "CommitVote"
             \/ /\ ~committed'
                /\ TimeoutTick
                /\ RbcDeliveredPendingTimeoutStepStartsNewViewWaitState
                /\ phase # "NewView"
                /\ phase' = "NewView"
             \/ /\ ~committed'
                /\ HonestNewViewVote
                /\ newViewVotes + 1 >= ViewQuorum
                /\ RbcDeliveredPendingNewViewVoteStepStartsProposalWaitState
                /\ phase = "NewView"
                /\ phase' = "Propose")
    /\ (phase' = phase =>
          \/ /\ ~Next
             /\ vars' = vars
          \/ /\ GstElapsed
             /\ RbcDeliveredPendingGstElapsedStepKeepsWaitState
          \/ /\ HonestPrepareVote
             /\ prepareVotes + 1 < CommitQuorum
             /\ RbcDeliveredPendingPrepareVoteStepKeepsWaitState
          \/ /\ HonestCommitVote
             /\ ~CanCommit(
                  commitVotesHonest + 1,
                  commitVotesByz,
                  stakeSigned + StakePerHonestVote,
                  rbcState
                )
             /\ RbcDeliveredPendingHonestCommitVoteStepKeepsWaitState
          \/ /\ ByzantineEquivocateCommit
             /\ ~CanCommit(
                  commitVotesHonest,
                  commitVotesByz + 1,
                  stakeSigned + StakePerByzVote,
                  rbcState
                )
             /\ RbcDeliveredPendingByzantineCommitVoteStepKeepsWaitState
          \/ /\ TimeoutTick
             /\ phase = "NewView"
             /\ RbcDeliveredPendingTimeoutStepStartsNewViewWaitState
          \/ /\ HonestNewViewVote
             /\ newViewVotes + 1 < ViewQuorum
             /\ RbcDeliveredPendingNewViewVoteStepKeepsWaitState)

RbcDeliveredPendingSpecStepCounterChangesMatchActionStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepPhaseChangeMatchesActionStep
    /\ ((prepareVotes' # prepareVotes) =>
          \/ /\ HonestPrepareVote
             /\ prepareVotes' = prepareVotes + 1
          \/ /\ TimeoutTick
             /\ prepareVotes' = 0)
    /\ ((commitVotesHonest' # commitVotesHonest) =>
          \/ /\ HonestCommitVote
             /\ commitVotesHonest' = commitVotesHonest + 1
          \/ /\ TimeoutTick
             /\ commitVotesHonest' = 0)
    /\ ((commitVotesByz' # commitVotesByz) =>
          \/ /\ ByzantineEquivocateCommit
             /\ commitVotesByz' = commitVotesByz + 1
          \/ /\ TimeoutTick
             /\ commitVotesByz' = 0)
    /\ ((stakeSigned' # stakeSigned) =>
          \/ /\ HonestCommitVote
             /\ stakeSigned' = stakeSigned + StakePerHonestVote
          \/ /\ ByzantineEquivocateCommit
             /\ stakeSigned' = stakeSigned + StakePerByzVote
          \/ /\ TimeoutTick
             /\ stakeSigned' = 0)
    /\ ((newViewVotes' # newViewVotes) =>
          \/ /\ HonestNewViewVote
             /\ newViewVotes' = newViewVotes + 1
          \/ /\ TimeoutTick
             /\ newViewVotes' = 0
          \/ /\ HonestPropose
             /\ newViewVotes' = 0)
    /\ ((viewEvidenceVotes' # viewEvidenceVotes) =>
          \/ /\ TimeoutTick
             /\ viewEvidenceVotes' = 0
          \/ /\ HonestNewViewVote
             /\ newViewVotes + 1 >= ViewQuorum
             /\ viewEvidenceVotes' = newViewVotes')

RbcDeliveredPendingSpecStepActionSourcesExclusiveStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepCounterChangesMatchActionStep
    /\ (Next <=>
          \/ HonestPropose
          \/ HonestPrepareVote
          \/ HonestCommitVote
          \/ ByzantineEquivocateCommit
          \/ TimeoutTick
          \/ HonestNewViewVote
          \/ GstElapsed)
    /\ (HonestPropose =>
          /\ ~HonestPrepareVote
          /\ ~HonestCommitVote
          /\ ~ByzantineEquivocateCommit
          /\ ~TimeoutTick
          /\ ~HonestNewViewVote
          /\ ~GstElapsed)
    /\ (HonestPrepareVote =>
          /\ ~HonestPropose
          /\ ~HonestCommitVote
          /\ ~ByzantineEquivocateCommit
          /\ ~TimeoutTick
          /\ ~HonestNewViewVote
          /\ ~GstElapsed)
    /\ (HonestCommitVote =>
          /\ ~HonestPropose
          /\ ~HonestPrepareVote
          /\ ~ByzantineEquivocateCommit
          /\ ~TimeoutTick
          /\ ~HonestNewViewVote
          /\ ~GstElapsed)
    /\ (ByzantineEquivocateCommit =>
          /\ ~HonestPropose
          /\ ~HonestPrepareVote
          /\ ~HonestCommitVote
          /\ ~TimeoutTick
          /\ ~HonestNewViewVote
          /\ ~GstElapsed)
    /\ (TimeoutTick =>
          /\ ~HonestPropose
          /\ ~HonestPrepareVote
          /\ ~HonestCommitVote
          /\ ~ByzantineEquivocateCommit
          /\ ~HonestNewViewVote
          /\ ~GstElapsed)
    /\ (HonestNewViewVote =>
          /\ ~HonestPropose
          /\ ~HonestPrepareVote
          /\ ~HonestCommitVote
          /\ ~ByzantineEquivocateCommit
          /\ ~TimeoutTick
          /\ ~GstElapsed)
    /\ (GstElapsed =>
          /\ ~HonestPropose
          /\ ~HonestPrepareVote
          /\ ~HonestCommitVote
          /\ ~ByzantineEquivocateCommit
          /\ ~TimeoutTick
          /\ ~HonestNewViewVote)

RbcDeliveredPendingSpecStepStutterPreservesActionSurfaceStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepActionSourcesExclusiveStep
    /\ (~Next =>
          /\ vars' = vars
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ ~HonestPropose
          /\ ~HonestPrepareVote
          /\ ~HonestCommitVote
          /\ ~ByzantineEquivocateCommit
          /\ ~TimeoutTick
          /\ ~HonestNewViewVote
          /\ ~RbcInit
          /\ ~RbcChunkGood
          /\ ~RbcReadyGood
          /\ ~RbcDeliverGood
          /\ ~ByzantineFault
          /\ ~GstElapsed
          /\ (HonestProposeEnabled' <=> HonestProposeEnabled)
          /\ (HonestPrepareVoteEnabled' <=> HonestPrepareVoteEnabled)
          /\ (HonestCommitVoteEnabled' <=> HonestCommitVoteEnabled)
          /\ (ByzantineCommitVoteEnabled' <=> ByzantineCommitVoteEnabled)
          /\ (HonestNewViewVoteEnabled' <=> HonestNewViewVoteEnabled)
          /\ (RbcInitEnabled' <=> RbcInitEnabled)
          /\ (RbcChunkGoodEnabled' <=> RbcChunkGoodEnabled)
          /\ (RbcReadyGoodEnabled' <=> RbcReadyGoodEnabled)
          /\ (RbcDeliverGoodEnabled' <=> RbcDeliverGoodEnabled)
          /\ (ByzantineFaultEnabled' <=> ByzantineFaultEnabled)
          /\ (PostGstProgressEnabled' <=> PostGstProgressEnabled)
          /\ (TimeoutTickEnabled' <=> TimeoutTickEnabled)
          /\ (GstElapsedEnabled' <=> GstElapsedEnabled))

RbcDeliveredPendingSpecStepCommitArtifactChangeMatchesSourceStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepStutterPreservesActionSurfaceStep
    /\ RbcDeliveredPendingSpecStepCommitArtifactsMatchOutcomeStep
    /\ commitEvidenceVotes = 0
    /\ commitEvidenceStake = 0
    /\ commitView = 0
    /\ ((commitEvidenceVotes' # commitEvidenceVotes) <=> committed')
    /\ ((commitEvidenceStake' # commitEvidenceStake) <=> committed')
    /\ ((commitView' # commitView) <=> (committed' /\ view' # 0))
    /\ (committed' =>
          /\ Next
          /\ RbcDeliveredPendingSpecStepFinalitySourceMatchesCommitVoteStep
          /\ RbcDeliveredPendingSpecStepFinalityWitnessFrameStep
          /\ phase = "CommitVote"
          /\ phase' = "Committed"
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitView' = view'
          /\ \/ /\ HonestCommitVote
                /\ ~ByzantineEquivocateCommit
                /\ commitVotesHonest' = commitVotesHonest + 1
                /\ commitVotesByz' = commitVotesByz
                /\ stakeSigned' = stakeSigned + StakePerHonestVote
                /\ commitEvidenceVotes' = commitVotesHonest + 1 + commitVotesByz
                /\ commitEvidenceStake' = stakeSigned + StakePerHonestVote
             \/ /\ ~HonestCommitVote
                /\ ByzantineEquivocateCommit
                /\ commitVotesHonest' = commitVotesHonest
                /\ commitVotesByz' = commitVotesByz + 1
                /\ stakeSigned' = stakeSigned + StakePerByzVote
                /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz + 1
                /\ commitEvidenceStake' = stakeSigned + StakePerByzVote)
    /\ (~committed' =>
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ commitEvidenceVotes' = commitEvidenceVotes
          /\ commitEvidenceStake' = commitEvidenceStake
          /\ commitView' = commitView
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0)

RbcDeliveredPendingSpecStepCommitArtifactChangeInstallsCertifiedDeliveryStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepCommitArtifactChangeMatchesSourceStep
    /\ RbcDeliveredPendingSpecStepPreservesDeliveredRbcEvidenceStep
    /\ RbcDeliveredPendingSpecStepFinalityStackMatchesOutcomeStep
    /\ RbcDeliveredPendingSpecStepFinalityGateOutcomeStep
    /\ RbcDeliveredPendingSpecStepFinalityQuorumOutcomeStep
    /\ ((\/ commitEvidenceVotes' # commitEvidenceVotes
         \/ commitEvidenceStake' # commitEvidenceStake
         \/ commitView' # commitView) <=> committed')
    /\ ((\/ commitEvidenceVotes' # commitEvidenceVotes
         \/ commitEvidenceStake' # commitEvidenceStake
         \/ commitView' # commitView) =>
          /\ Next
          /\ committed'
          /\ phase = "CommitVote"
          /\ phase' = "Committed"
          /\ rbcState' = "Delivered"
          /\ headerSeen'
          /\ digestValid'
          /\ chunkCount' >= MaxChunks
          /\ readyVotes' >= CommitQuorum
          /\ FinalityCertificateStackPresent'
          /\ FinalityCertificateStackComplete'
          /\ FinalityCertificateStackMatchesFinality'
          /\ CommittedPhaseMatchesFinality'
          /\ CommitCertificateMatchesFinality'
          /\ LiveCommitGateMatchesFinality'
          /\ LiveCommitGateRbcEvidenceMatches'
          /\ CommitViewMatchesFinality'
          /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ prepareVotes' = prepareVotes
          /\ newViewVotes' = 0
          /\ view' = view
          /\ commitView' = view'
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake' >= StakeQuorum
          /\ (view' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
          /\ CommitDisablesProgressActions'
          /\ ~HonestProposeEnabled'
          /\ ~HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~TimeoutTickEnabled'
          /\ ~ByzantineFaultEnabled'
          /\ ~PostGstProgressEnabled'
          /\ (GstElapsedEnabled' <=> ~gst'))
    /\ (~(\/ commitEvidenceVotes' # commitEvidenceVotes
          \/ commitEvidenceStake' # commitEvidenceStake
          \/ commitView' # commitView) =>
          /\ ~committed'
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ ~FinalityCertificateStackPresent'
          /\ phase' # "Committed"
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0)

RbcDeliveredPendingSpecStepCommitArtifactChangeExactSourceCertifiedDeliveryStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepCommitArtifactChangeInstallsCertifiedDeliveryStep
    /\ ((\/ commitEvidenceVotes' # commitEvidenceVotes
         \/ commitEvidenceStake' # commitEvidenceStake
         \/ commitView' # commitView) =>
          /\ RbcDeliveredPendingSpecStepFinalitySourceMatchesCommitVoteStep
          /\ RbcDeliveredPendingSpecStepFinalityWitnessFrameStep
          /\ RbcDeliveredPendingSpecStepFinalityQuorumOutcomeStep
          /\ Next
          /\ committed'
          /\ phase = "CommitVote"
          /\ phase' = "Committed"
          /\ view' = view
          /\ gst' = gst
          /\ commitView' = view
          /\ FinalityCertificateStackPresent'
          /\ FinalityCertificateStackComplete'
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ CommitDisablesProgressActions'
          /\ ~HonestPropose
          /\ ~HonestPrepareVote
          /\ ~TimeoutTick
          /\ ~HonestNewViewVote
          /\ ~RbcInit
          /\ ~RbcChunkGood
          /\ ~RbcReadyGood
          /\ ~RbcDeliverGood
          /\ ~ByzantineFault
          /\ ~GstElapsed
          /\ (GstElapsedEnabled' <=> ~gst)
          /\ \/ /\ HonestCommitVote
                /\ ~ByzantineEquivocateCommit
                /\ CanCommit(
                     commitVotesHonest + 1,
                     commitVotesByz,
                     stakeSigned + StakePerHonestVote,
                     rbcState
                   )
                /\ RbcDeliveredPendingHonestCommitVoteStepCompletesFinality
                /\ commitVotesHonest' = commitVotesHonest + 1
                /\ commitVotesByz' = commitVotesByz
                /\ stakeSigned' = stakeSigned + StakePerHonestVote
                /\ commitEvidenceVotes' = commitVotesHonest + 1 + commitVotesByz
                /\ commitEvidenceStake' = stakeSigned + StakePerHonestVote
             \/ /\ ~HonestCommitVote
                /\ ByzantineEquivocateCommit
                /\ CanCommit(
                     commitVotesHonest,
                     commitVotesByz + 1,
                     stakeSigned + StakePerByzVote,
                     rbcState
                   )
                /\ RbcDeliveredPendingByzantineCommitVoteStepCompletesFinality
                /\ commitVotesHonest' = commitVotesHonest
                /\ commitVotesByz' = commitVotesByz + 1
                /\ stakeSigned' = stakeSigned + StakePerByzVote
                /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz + 1
                /\ commitEvidenceStake' = stakeSigned + StakePerByzVote)
    /\ (~(\/ commitEvidenceVotes' # commitEvidenceVotes
          \/ commitEvidenceStake' # commitEvidenceStake
          \/ commitView' # commitView) =>
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ ~committed'
          /\ ~FinalityCertificateStackPresent'
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0)

RbcDeliveredPendingSpecStepStableCommitArtifactsStayNonFinalHandoffStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepCommitArtifactChangeExactSourceCertifiedDeliveryStep
    /\ RbcDeliveredPendingSpecStepNonFinalHandoffPhaseShapeStep
    /\ RbcDeliveredPendingSpecStepActionSurfaceClosedStep
    /\ (~(\/ commitEvidenceVotes' # commitEvidenceVotes
          \/ commitEvidenceStake' # commitEvidenceStake
          \/ commitView' # commitView) <=>
          ~committed')
    /\ (~(\/ commitEvidenceVotes' # commitEvidenceVotes
          \/ commitEvidenceStake' # commitEvidenceStake
          \/ commitView' # commitView) =>
          /\ ~committed'
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ rbcState' = "Delivered"
          /\ readyVotes' = readyVotes
          /\ readyVotes' >= CommitQuorum
          /\ chunkCount' = chunkCount
          /\ chunkCount' >= MaxChunks
          /\ headerSeen' = headerSeen
          /\ headerSeen'
          /\ digestValid' = digestValid
          /\ digestValid'
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ViewEvidenceIsCompleteOrEmpty'
          /\ ~FinalityCertificateStackPresent'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~ByzantineFaultEnabled'
          /\ (PostGstProgressEnabled' <=>
                \/ HonestProposeEnabled'
                \/ HonestPrepareVoteEnabled'
                \/ HonestCommitVoteEnabled'
                \/ HonestNewViewVoteEnabled')
          /\ (GstElapsedEnabled' <=> ~gst')
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
          /\ (phase' = "Propose" =>
                /\ HonestProposeEnabled'
                /\ prepareVotes' = 0
                /\ commitVotesHonest' = 0
                /\ commitVotesByz' = 0
                /\ stakeSigned' = 0
                /\ (view' = 0 \/ viewEvidenceVotes' >= ViewQuorum)
                /\ ~HonestPrepareVoteEnabled'
                /\ ~HonestCommitVoteEnabled'
                /\ ~ByzantineCommitVoteEnabled'
                /\ ~HonestNewViewVoteEnabled')
          /\ (phase' = "Prepare" =>
                /\ HonestPrepareVoteEnabled'
                /\ prepareVotes' < CommitQuorum
                /\ commitVotesHonest' = 0
                /\ commitVotesByz' = 0
                /\ stakeSigned' = 0
                /\ ~HonestProposeEnabled'
                /\ ~HonestCommitVoteEnabled'
                /\ ~ByzantineCommitVoteEnabled'
                /\ ~HonestNewViewVoteEnabled')
          /\ (phase' = "CommitVote" =>
                /\ prepareVotes' >= CommitQuorum
                /\ newViewVotes' = 0
                /\ ~HonestProposeEnabled'
                /\ ~HonestPrepareVoteEnabled'
                /\ ~HonestNewViewVoteEnabled'
                /\ (HonestCommitVoteEnabled' <=> commitVotesHonest' < N - F)
                /\ (ByzantineCommitVoteEnabled' <=> commitVotesByz' < F))
          /\ (phase' = "NewView" =>
                /\ HonestNewViewVoteEnabled'
                /\ NewViewPhaseBelowQuorum'
                /\ prepareVotes' = 0
                /\ commitVotesHonest' = 0
                /\ commitVotesByz' = 0
                /\ stakeSigned' = 0
                /\ viewEvidenceVotes' = 0
                /\ newViewVotes' < ViewQuorum
                /\ ~HonestProposeEnabled'
                /\ ~HonestPrepareVoteEnabled'
                /\ ~HonestCommitVoteEnabled'
                /\ ~ByzantineCommitVoteEnabled'))

RbcDeliveredPendingSpecStepStableCommitArtifactsMatchNonFinalSourceStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepStableCommitArtifactsStayNonFinalHandoffStep
    /\ RbcDeliveredPendingSpecStepActionSourcesExclusiveStep
    /\ (~(\/ commitEvidenceVotes' # commitEvidenceVotes
          \/ commitEvidenceStake' # commitEvidenceStake
          \/ commitView' # commitView) =>
          /\ ~committed'
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ (Next <=>
                \/ HonestPropose
                \/ HonestPrepareVote
                \/ HonestCommitVote
                \/ ByzantineEquivocateCommit
                \/ TimeoutTick
                \/ HonestNewViewVote
                \/ GstElapsed)
          /\ ~(HonestCommitVote /\
                CanCommit(
                  commitVotesHonest + 1,
                  commitVotesByz,
                  stakeSigned + StakePerHonestVote,
                  rbcState
                ))
          /\ ~(ByzantineEquivocateCommit /\
                CanCommit(
                  commitVotesHonest,
                  commitVotesByz + 1,
                  stakeSigned + StakePerByzVote,
                  rbcState
                ))
          /\ (~Next =>
                /\ vars' = vars
                /\ ~HonestPropose
                /\ ~HonestPrepareVote
                /\ ~HonestCommitVote
                /\ ~ByzantineEquivocateCommit
                /\ ~TimeoutTick
                /\ ~HonestNewViewVote
                /\ ~GstElapsed)
          /\ (Next =>
                \/ /\ HonestPropose
                   /\ RbcDeliveredPendingHonestProposeStepStartsPrepareWaitState
                \/ /\ HonestPrepareVote
                   /\ prepareVotes + 1 < CommitQuorum
                   /\ RbcDeliveredPendingPrepareVoteStepKeepsWaitState
                \/ /\ HonestPrepareVote
                   /\ prepareVotes + 1 >= CommitQuorum
                   /\ RbcDeliveredPendingPrepareVoteStepStartsCommitVoteWaitState
                \/ /\ HonestCommitVote
                   /\ ~CanCommit(
                        commitVotesHonest + 1,
                        commitVotesByz,
                        stakeSigned + StakePerHonestVote,
                        rbcState
                      )
                   /\ RbcDeliveredPendingHonestCommitVoteStepKeepsWaitState
                \/ /\ ByzantineEquivocateCommit
                   /\ ~CanCommit(
                        commitVotesHonest,
                        commitVotesByz + 1,
                        stakeSigned + StakePerByzVote,
                        rbcState
                      )
                   /\ RbcDeliveredPendingByzantineCommitVoteStepKeepsWaitState
                \/ /\ TimeoutTick
                   /\ RbcDeliveredPendingTimeoutStepStartsNewViewWaitState
                \/ /\ HonestNewViewVote
                   /\ newViewVotes + 1 < ViewQuorum
                   /\ RbcDeliveredPendingNewViewVoteStepKeepsWaitState
                \/ /\ HonestNewViewVote
                   /\ newViewVotes + 1 >= ViewQuorum
                   /\ RbcDeliveredPendingNewViewVoteStepStartsProposalWaitState
                \/ /\ GstElapsed
                   /\ RbcDeliveredPendingGstElapsedStepKeepsWaitState))

RbcDeliveredPendingSpecStepStableCommitArtifactsCounterFootprintStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepStableCommitArtifactsMatchNonFinalSourceStep
    /\ RbcDeliveredPendingSpecStepCounterChangesMatchActionStep
    /\ (~(\/ commitEvidenceVotes' # commitEvidenceVotes
          \/ commitEvidenceStake' # commitEvidenceStake
          \/ commitView' # commitView) =>
          /\ ~committed'
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ (HonestCommitVote =>
                /\ ~ByzantineEquivocateCommit
                /\ phase = "CommitVote"
                /\ phase' = "CommitVote"
                /\ ~CanCommit(
                     commitVotesHonest + 1,
                     commitVotesByz,
                     stakeSigned + StakePerHonestVote,
                     rbcState
                   )
                /\ RbcDeliveredPendingHonestCommitVoteStepKeepsWaitState
                /\ prepareVotes' = prepareVotes
                /\ commitVotesHonest' = commitVotesHonest + 1
                /\ commitVotesByz' = commitVotesByz
                /\ stakeSigned' = stakeSigned + StakePerHonestVote
                /\ newViewVotes' = 0
                /\ viewEvidenceVotes' = viewEvidenceVotes)
          /\ (ByzantineEquivocateCommit =>
                /\ ~HonestCommitVote
                /\ phase = "CommitVote"
                /\ phase' = "CommitVote"
                /\ ~CanCommit(
                     commitVotesHonest,
                     commitVotesByz + 1,
                     stakeSigned + StakePerByzVote,
                     rbcState
                   )
                /\ RbcDeliveredPendingByzantineCommitVoteStepKeepsWaitState
                /\ prepareVotes' = prepareVotes
                /\ commitVotesHonest' = commitVotesHonest
                /\ commitVotesByz' = commitVotesByz + 1
                /\ stakeSigned' = stakeSigned + StakePerByzVote
                /\ newViewVotes' = 0
                /\ viewEvidenceVotes' = viewEvidenceVotes)
          /\ ((HonestPropose \/ HonestPrepareVote \/ TimeoutTick \/ HonestNewViewVote) =>
                /\ commitVotesHonest' = 0
                /\ commitVotesByz' = 0
                /\ stakeSigned' = 0)
          /\ ((~Next \/ GstElapsed) =>
                /\ prepareVotes' = prepareVotes
                /\ commitVotesHonest' = commitVotesHonest
                /\ commitVotesByz' = commitVotesByz
                /\ stakeSigned' = stakeSigned
                /\ newViewVotes' = newViewVotes
                /\ viewEvidenceVotes' = viewEvidenceVotes)
          /\ (HonestPrepareVote => prepareVotes' = prepareVotes + 1)
          /\ (HonestPropose =>
                /\ prepareVotes' = 0
                /\ newViewVotes' = 0)
          /\ (TimeoutTick =>
                /\ prepareVotes' = 0
                /\ newViewVotes' = 0
                /\ viewEvidenceVotes' = 0)
          /\ (HonestNewViewVote =>
                /\ prepareVotes' = 0
                /\ newViewVotes' = newViewVotes + 1))

RbcDeliveredPendingSpecStepStableCommitArtifactsPhaseGateFootprintStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepStableCommitArtifactsCounterFootprintStep
    /\ RbcDeliveredPendingSpecStepPhaseChangeMatchesActionStep
    /\ RbcDeliveredPendingSpecStepPostGatesMatchHandoffStep
    /\ (~(\/ commitEvidenceVotes' # commitEvidenceVotes
          \/ commitEvidenceStake' # commitEvidenceStake
          \/ commitView' # commitView) =>
          /\ ~committed'
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ (HonestPropose =>
                /\ RbcDeliveredPendingHonestProposeStepStartsPrepareWaitState
                /\ phase' = "Prepare"
                /\ HonestPrepareVoteEnabled'
                /\ ~HonestProposeEnabled'
                /\ ~HonestNewViewVoteEnabled'
                /\ ~HonestCommitVoteEnabled'
                /\ ~ByzantineCommitVoteEnabled')
          /\ (HonestPrepareVote =>
                \/ /\ prepareVotes + 1 < CommitQuorum
                   /\ RbcDeliveredPendingPrepareVoteStepKeepsWaitState
                   /\ phase' = "Prepare"
                   /\ HonestPrepareVoteEnabled'
                   /\ ~HonestProposeEnabled'
                   /\ ~HonestCommitVoteEnabled'
                   /\ ~ByzantineCommitVoteEnabled'
                   /\ ~HonestNewViewVoteEnabled'
                \/ /\ prepareVotes + 1 >= CommitQuorum
                   /\ RbcDeliveredPendingPrepareVoteStepStartsCommitVoteWaitState
                   /\ phase' = "CommitVote"
                   /\ ~HonestProposeEnabled'
                   /\ ~HonestPrepareVoteEnabled'
                   /\ ~HonestNewViewVoteEnabled'
                   /\ (HonestCommitVoteEnabled' <=> commitVotesHonest' < N - F)
                   /\ (ByzantineCommitVoteEnabled' <=> commitVotesByz' < F))
          /\ (HonestCommitVote =>
                /\ RbcDeliveredPendingHonestCommitVoteStepKeepsWaitState
                /\ phase' = "CommitVote"
                /\ ~HonestProposeEnabled'
                /\ ~HonestPrepareVoteEnabled'
                /\ ~HonestNewViewVoteEnabled'
                /\ (HonestCommitVoteEnabled' <=> commitVotesHonest' < N - F)
                /\ (ByzantineCommitVoteEnabled' <=> commitVotesByz' < F))
          /\ (ByzantineEquivocateCommit =>
                /\ RbcDeliveredPendingByzantineCommitVoteStepKeepsWaitState
                /\ phase' = "CommitVote"
                /\ ~HonestProposeEnabled'
                /\ ~HonestPrepareVoteEnabled'
                /\ ~HonestNewViewVoteEnabled'
                /\ (HonestCommitVoteEnabled' <=> commitVotesHonest' < N - F)
                /\ (ByzantineCommitVoteEnabled' <=> commitVotesByz' < F))
          /\ (TimeoutTick =>
                /\ RbcDeliveredPendingTimeoutStepStartsNewViewWaitState
                /\ phase' = "NewView"
                /\ HonestNewViewVoteEnabled'
                /\ ~HonestProposeEnabled'
                /\ ~HonestPrepareVoteEnabled'
                /\ ~HonestCommitVoteEnabled'
                /\ ~ByzantineCommitVoteEnabled')
          /\ (HonestNewViewVote =>
                \/ /\ newViewVotes + 1 < ViewQuorum
                   /\ RbcDeliveredPendingNewViewVoteStepKeepsWaitState
                   /\ phase' = "NewView"
                   /\ HonestNewViewVoteEnabled'
                   /\ ~HonestProposeEnabled'
                   /\ ~HonestPrepareVoteEnabled'
                   /\ ~HonestCommitVoteEnabled'
                   /\ ~ByzantineCommitVoteEnabled'
                \/ /\ newViewVotes + 1 >= ViewQuorum
                   /\ RbcDeliveredPendingNewViewVoteStepStartsProposalWaitState
                   /\ phase' = "Propose"
                   /\ HonestProposeEnabled'
                   /\ ~HonestNewViewVoteEnabled'
                   /\ ~HonestPrepareVoteEnabled'
                   /\ ~HonestCommitVoteEnabled'
                   /\ ~ByzantineCommitVoteEnabled')
          /\ (GstElapsed =>
                /\ RbcDeliveredPendingGstElapsedStepKeepsWaitState
                /\ phase' = phase
                /\ gst'
                /\ (HonestProposeEnabled' <=> HonestProposeEnabled)
                /\ (HonestPrepareVoteEnabled' <=> HonestPrepareVoteEnabled)
                /\ (HonestCommitVoteEnabled' <=> HonestCommitVoteEnabled)
                /\ (ByzantineCommitVoteEnabled' <=> ByzantineCommitVoteEnabled)
                /\ (HonestNewViewVoteEnabled' <=> HonestNewViewVoteEnabled))
          /\ (~Next =>
                /\ vars' = vars
                /\ phase' = phase
                /\ (HonestProposeEnabled' <=> HonestProposeEnabled)
                /\ (HonestPrepareVoteEnabled' <=> HonestPrepareVoteEnabled)
                /\ (HonestCommitVoteEnabled' <=> HonestCommitVoteEnabled)
                /\ (ByzantineCommitVoteEnabled' <=> ByzantineCommitVoteEnabled)
                /\ (HonestNewViewVoteEnabled' <=> HonestNewViewVoteEnabled)))

RbcDeliveredPendingSpecStepStableCommitArtifactsTimerFootprintStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepStableCommitArtifactsPhaseGateFootprintStep
    /\ RbcDeliveredPendingSpecStepTimerGatesMatchHandoffStep
    /\ (~(\/ commitEvidenceVotes' # commitEvidenceVotes
          \/ commitEvidenceStake' # commitEvidenceStake
          \/ commitView' # commitView) =>
          /\ ~committed'
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ (GstElapsedEnabled' <=> ~gst')
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
          /\ (~gst' =>
                /\ GstElapsedEnabled'
                /\ TimeoutTickEnabled')
          /\ (gst' /\ PostGstProgressEnabled' =>
                /\ ~GstElapsedEnabled'
                /\ ~TimeoutTickEnabled')
          /\ (gst' /\ ~PostGstProgressEnabled' =>
                /\ ~GstElapsedEnabled'
                /\ TimeoutTickEnabled')
          /\ (GstElapsed =>
                /\ RbcDeliveredPendingGstElapsedStepKeepsWaitState
                /\ gst'
                /\ ~GstElapsedEnabled'
                /\ (TimeoutTickEnabled' <=> ~PostGstProgressEnabled'))
          /\ (~GstElapsed =>
                /\ gst' = gst
                /\ (GstElapsedEnabled' <=> GstElapsedEnabled))
          /\ (~Next =>
                /\ vars' = vars
                /\ (GstElapsedEnabled' <=> GstElapsedEnabled)
                /\ (TimeoutTickEnabled' <=> TimeoutTickEnabled)))

RbcDeliveredPendingSpecStepStableCommitArtifactsViewFootprintStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepStableCommitArtifactsTimerFootprintStep
    /\ RbcDeliveredPendingSpecStepViewEvidenceChangesOnlyByNewViewOrTimeoutStep
    /\ (~(\/ commitEvidenceVotes' # commitEvidenceVotes
          \/ commitEvidenceStake' # commitEvidenceStake
          \/ commitView' # commitView) =>
          /\ ~committed'
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ ViewEvidenceIsCompleteOrEmpty'
          /\ view' >= view
          /\ view' <= MaxView
          /\ (TimeoutTick =>
                /\ RbcDeliveredPendingTimeoutStepStartsNewViewWaitState
                /\ phase' = "NewView"
                /\ view' = IF view < MaxView THEN view + 1 ELSE MaxView
                /\ newViewVotes' = 0
                /\ viewEvidenceVotes' = 0
                /\ prepareVotes' = 0
                /\ commitVotesHonest' = 0
                /\ commitVotesByz' = 0
                /\ stakeSigned' = 0)
          /\ (~TimeoutTick => view' = view)
          /\ (HonestNewViewVote =>
                /\ view' = view
                /\ viewEvidenceVotes = 0
                /\ prepareVotes' = 0
                /\ commitVotesHonest' = 0
                /\ commitVotesByz' = 0
                /\ stakeSigned' = 0
                /\ newViewVotes' = newViewVotes + 1
                /\ \/ /\ newViewVotes + 1 < ViewQuorum
                      /\ RbcDeliveredPendingNewViewVoteStepKeepsWaitState
                      /\ phase' = "NewView"
                      /\ viewEvidenceVotes' = 0
                   \/ /\ newViewVotes + 1 >= ViewQuorum
                      /\ RbcDeliveredPendingNewViewVoteStepStartsProposalWaitState
                      /\ phase' = "Propose"
                      /\ viewEvidenceVotes' = newViewVotes'
                      /\ viewEvidenceVotes' >= ViewQuorum
                      /\ (view' = 0 \/ viewEvidenceVotes' >= ViewQuorum))
          /\ (~(TimeoutTick \/ HonestNewViewVote) =>
                viewEvidenceVotes' = viewEvidenceVotes)
          /\ ((HonestPropose \/ HonestPrepareVote \/ HonestCommitVote \/
               ByzantineEquivocateCommit \/ GstElapsed \/ ~Next) =>
                /\ view' = view
                /\ viewEvidenceVotes' = viewEvidenceVotes))

RbcDeliveredPendingSpecStepStableCommitArtifactsFinalityFootprintStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepStableCommitArtifactsViewFootprintStep
    /\ RbcDeliveredPendingSpecStepFinalityStackMatchesOutcomeStep
    /\ RbcDeliveredPendingSpecStepFinalityGateOutcomeStep
    /\ RbcDeliveredPendingSpecStepFinalityQuorumOutcomeStep
    /\ (~(\/ commitEvidenceVotes' # commitEvidenceVotes
          \/ commitEvidenceStake' # commitEvidenceStake
          \/ commitView' # commitView) =>
          /\ ~committed'
          /\ phase' # "Committed"
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~FinalityCertificateStackPresent'
          /\ FinalityCertificateStackComplete'
          /\ FinalityCertificateStackMatchesFinality'
          /\ CommitCertificateMatchesFinality'
          /\ LiveCommitGateMatchesFinality'
          /\ LiveCommitGateRbcEvidenceMatches'
          /\ CommitDisablesProgressActions'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ \/ commitVotesHonest' + commitVotesByz' < CommitQuorum
             \/ stakeSigned' < StakeQuorum
          /\ (HonestCommitVote =>
                /\ ~CanCommit(
                     commitVotesHonest + 1,
                     commitVotesByz,
                     stakeSigned + StakePerHonestVote,
                     rbcState
                   )
                /\ commitVotesHonest' = commitVotesHonest + 1
                /\ commitVotesByz' = commitVotesByz
                /\ stakeSigned' = stakeSigned + StakePerHonestVote)
          /\ (ByzantineEquivocateCommit =>
                /\ ~CanCommit(
                     commitVotesHonest,
                     commitVotesByz + 1,
                     stakeSigned + StakePerByzVote,
                     rbcState
                   )
                /\ commitVotesHonest' = commitVotesHonest
                /\ commitVotesByz' = commitVotesByz + 1
                /\ stakeSigned' = stakeSigned + StakePerByzVote)
          /\ ((HonestPropose \/ HonestPrepareVote \/ TimeoutTick \/
               HonestNewViewVote) =>
                /\ commitVotesHonest' = 0
                /\ commitVotesByz' = 0
                /\ stakeSigned' = 0)
          /\ ((GstElapsed \/ ~Next) =>
                /\ commitVotesHonest' = commitVotesHonest
                /\ commitVotesByz' = commitVotesByz
                /\ stakeSigned' = stakeSigned))

RbcDeliveredPendingSpecStepStableCommitArtifactsRbcSurfaceStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepStableCommitArtifactsFinalityFootprintStep
    /\ RbcDeliveredPendingSpecStepPreservesDeliveredRbcEvidenceStep
    /\ ~RbcInitEnabled
    /\ ~RbcChunkGoodEnabled
    /\ ~RbcReadyGoodEnabled
    /\ ~RbcDeliverGoodEnabled
    /\ ~ByzantineFaultEnabled
    /\ ~RbcInit
    /\ ~RbcChunkGood
    /\ ~RbcReadyGood
    /\ ~RbcDeliverGood
    /\ ~ByzantineFault
    /\ (~(\/ commitEvidenceVotes' # commitEvidenceVotes
          \/ commitEvidenceStake' # commitEvidenceStake
          \/ commitView' # commitView) =>
          /\ rbcState' = "Delivered"
          /\ readyVotes' = readyVotes
          /\ readyVotes' >= CommitQuorum
          /\ chunkCount' = chunkCount
          /\ chunkCount' >= MaxChunks
          /\ headerSeen' = headerSeen
          /\ headerSeen'
          /\ digestValid' = digestValid
          /\ digestValid'
          /\ DeliverImpliesEvidence'
          /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~ByzantineFaultEnabled'
          /\ (PostGstProgressEnabled' <=>
                \/ HonestProposeEnabled'
                \/ HonestPrepareVoteEnabled'
                \/ HonestCommitVoteEnabled'
                \/ HonestNewViewVoteEnabled'))

RbcDeliveredPendingSpecStepStableCommitArtifactsCompleteWaitStateStep ==
  (/\ rbcState = "Delivered"
   /\ ~committed
   /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepStableCommitArtifactsRbcSurfaceStep
    /\ (~(\/ commitEvidenceVotes' # commitEvidenceVotes
          \/ commitEvidenceStake' # commitEvidenceStake
          \/ commitView' # commitView) =>
          /\ RbcDeliveredPendingSpecStepStableCommitArtifactsMatchNonFinalSourceStep
          /\ RbcDeliveredPendingSpecStepStableCommitArtifactsCounterFootprintStep
          /\ RbcDeliveredPendingSpecStepStableCommitArtifactsPhaseGateFootprintStep
          /\ RbcDeliveredPendingSpecStepStableCommitArtifactsTimerFootprintStep
          /\ RbcDeliveredPendingSpecStepStableCommitArtifactsViewFootprintStep
          /\ RbcDeliveredPendingSpecStepStableCommitArtifactsFinalityFootprintStep
          /\ RbcDeliveredPendingSpecStepStableCommitArtifactsRbcSurfaceStep
          /\ ~committed'
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
          /\ DeliverImpliesEvidence'
          /\ ViewEvidenceIsCompleteOrEmpty'
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ rbcState' = "Delivered"
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~FinalityCertificateStackPresent'
          /\ FinalityCertificateStackComplete'
          /\ FinalityCertificateStackMatchesFinality'
          /\ CommitCertificateMatchesFinality'
          /\ LiveCommitGateMatchesFinality'
          /\ LiveCommitGateRbcEvidenceMatches'
          /\ CommitDisablesProgressActions'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ (Next <=>
                \/ HonestPropose
                \/ HonestPrepareVote
                \/ HonestCommitVote
                \/ ByzantineEquivocateCommit
                \/ TimeoutTick
                \/ HonestNewViewVote
                \/ GstElapsed)
          /\ (PostGstProgressEnabled' <=>
                \/ HonestProposeEnabled'
                \/ HonestPrepareVoteEnabled'
                \/ HonestCommitVoteEnabled'
                \/ HonestNewViewVoteEnabled')
          /\ (GstElapsedEnabled' <=> ~gst')
          /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
          /\ ~RbcInitEnabled'
          /\ ~RbcChunkGoodEnabled'
          /\ ~RbcReadyGoodEnabled'
          /\ ~RbcDeliverGoodEnabled'
          /\ ~ByzantineFaultEnabled')

DeliveredPendingCompleteWaitStateSpecStepClosesStep ==
  (/\ DeliveredPendingCompleteWaitState
   /\ [Next]_vars) =>
    /\ RbcDeliveredPendingSpecStepStableCommitArtifactsCompleteWaitStateStep
    /\ RbcDeliveredPendingSpecStepCommitArtifactsMatchOutcomeStep
    /\ RbcDeliveredPendingSpecStepEndsInFinalityOrWaitStateStep
    /\ rbcState' = "Delivered"
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' >= MaxChunks
    /\ headerSeen'
    /\ digestValid'
    /\ ((\/ commitEvidenceVotes' # commitEvidenceVotes
          \/ commitEvidenceStake' # commitEvidenceStake
          \/ commitView' # commitView) <=> committed')
    /\ (committed' =>
          /\ ~DeliveredPendingCompleteWaitState'
          /\ phase' = "Committed"
          /\ FinalityCertificateStackPresent'
          /\ CommitDisablesProgressActions'
          /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
          /\ commitEvidenceStake' = stakeSigned'
          /\ commitEvidenceVotes' >= CommitQuorum
          /\ commitEvidenceStake' >= StakeQuorum
          /\ commitView' = view'
          /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ ~PostGstProgressEnabled'
          /\ ~TimeoutTickEnabled')
    /\ (~committed' =>
          /\ DeliveredPendingCompleteWaitState'
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ commitEvidenceVotes' = 0
          /\ commitEvidenceStake' = 0
          /\ commitView' = 0
          /\ ~FinalityCertificateStackPresent'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
          /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
          /\ (\/ GstElapsedEnabled'
              \/ PostGstProgressEnabled'
              \/ TimeoutTickEnabled'))

DeliveredPendingCompleteWaitStateCommitVoteStepSplitsStep ==
  (/\ DeliveredPendingCompleteWaitState
   /\ [Next]_vars
   /\ (HonestCommitVote \/ ByzantineEquivocateCommit)) =>
    /\ DeliveredPendingCompleteWaitStateSpecStepClosesStep
    /\ RbcDeliveredPendingSpecStepVoteCountersMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepFinalitySourceMatchesCommitVoteStep
    /\ RbcDeliveredPendingSpecStepActionSourcesExclusiveStep
    /\ phase = "CommitVote"
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ chunkCount' = chunkCount
    /\ headerSeen' = headerSeen
    /\ digestValid' = digestValid
    /\ view' = view
    /\ gst' = gst
    /\ prepareVotes' = prepareVotes
    /\ (HonestCommitVote =>
          /\ ~ByzantineEquivocateCommit
          /\ commitVotesHonest' = commitVotesHonest + 1
          /\ commitVotesByz' = commitVotesByz
          /\ stakeSigned' = stakeSigned + StakePerHonestVote
          /\ (committed' <=>
                CanCommit(
                  commitVotesHonest + 1,
                  commitVotesByz,
                  stakeSigned + StakePerHonestVote,
                  rbcState
                ))
          /\ (committed' =>
                /\ RbcDeliveredPendingHonestCommitVoteStepCompletesFinality
                /\ ~DeliveredPendingCompleteWaitState'
                /\ phase' = "Committed"
                /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
                /\ commitEvidenceStake' = stakeSigned'
                /\ commitView' = view')
          /\ (~committed' =>
                /\ RbcDeliveredPendingHonestCommitVoteStepKeepsWaitState
                /\ DeliveredPendingCompleteWaitState'
                /\ phase' = "CommitVote"
                /\ commitEvidenceVotes' = 0
                /\ commitEvidenceStake' = 0
                /\ commitView' = 0))
    /\ (ByzantineEquivocateCommit =>
          /\ ~HonestCommitVote
          /\ commitVotesHonest' = commitVotesHonest
          /\ commitVotesByz' = commitVotesByz + 1
          /\ stakeSigned' = stakeSigned + StakePerByzVote
          /\ (committed' <=>
                CanCommit(
                  commitVotesHonest,
                  commitVotesByz + 1,
                  stakeSigned + StakePerByzVote,
                  rbcState
                ))
          /\ (committed' =>
                /\ RbcDeliveredPendingByzantineCommitVoteStepCompletesFinality
                /\ ~DeliveredPendingCompleteWaitState'
                /\ phase' = "Committed"
                /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
                /\ commitEvidenceStake' = stakeSigned'
                /\ commitView' = view')
          /\ (~committed' =>
                /\ RbcDeliveredPendingByzantineCommitVoteStepKeepsWaitState
                /\ DeliveredPendingCompleteWaitState'
                /\ phase' = "CommitVote"
                /\ commitEvidenceVotes' = 0
                /\ commitEvidenceStake' = 0
                /\ commitView' = 0))
    /\ (committed' =>
          /\ phase' = "Committed"
          /\ FinalityCertificateStackPresent'
          /\ CommitDisablesProgressActions'
          /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))
    /\ (~committed' =>
          /\ DeliveredPendingCompleteWaitState'
          /\ phase' = "CommitVote"
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
          /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState'))

DeliveredPendingCompleteWaitStateCommitVoteStepPreservesWaitStateStep ==
  (/\ DeliveredPendingCompleteWaitState
   /\ [Next]_vars
   /\ (\/ /\ HonestCommitVote
          /\ ~CanCommit(
               commitVotesHonest + 1,
               commitVotesByz,
               stakeSigned + StakePerHonestVote,
               rbcState
             )
       \/ /\ ByzantineEquivocateCommit
          /\ ~CanCommit(
               commitVotesHonest,
               commitVotesByz + 1,
               stakeSigned + StakePerByzVote,
               rbcState
             ))) =>
    /\ DeliveredPendingCompleteWaitStateCommitVoteStepSplitsStep
    /\ DeliveredPendingCompleteWaitState'
    /\ ~committed'
    /\ phase = "CommitVote"
    /\ phase' = "CommitVote"
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ view' = view
    /\ gst' = gst
    /\ prepareVotes' = prepareVotes
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~FinalityCertificateStackPresent'
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ VoteCountersRespectRosterBudgets'
    /\ StakeSignedMatchesVoteCounters'
    /\ LiveStakeSignedIsBounded'
    /\ CommitEvidenceIsBounded'
    /\ CommitEvidenceIsCompleteOrEmpty'
    /\ \/ commitVotesHonest' + commitVotesByz' < CommitQuorum
       \/ stakeSigned' < StakeQuorum
    /\ (HonestCommitVote =>
          /\ ~ByzantineEquivocateCommit
          /\ RbcDeliveredPendingHonestCommitVoteStepKeepsWaitState
          /\ commitVotesHonest' = commitVotesHonest + 1
          /\ commitVotesByz' = commitVotesByz
          /\ stakeSigned' = stakeSigned + StakePerHonestVote)
    /\ (ByzantineEquivocateCommit =>
          /\ ~HonestCommitVote
          /\ RbcDeliveredPendingByzantineCommitVoteStepKeepsWaitState
          /\ commitVotesHonest' = commitVotesHonest
          /\ commitVotesByz' = commitVotesByz + 1
          /\ stakeSigned' = stakeSigned + StakePerByzVote)
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ (HonestCommitVoteEnabled' <=> commitVotesHonest' < N - F)
    /\ (ByzantineCommitVoteEnabled' <=> commitVotesByz' < F)
    /\ ~HonestNewViewVoteEnabled'
    /\ (PostGstProgressEnabled' <=> HonestCommitVoteEnabled')
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))

DeliveredPendingCompleteWaitStateCommitVoteStepCompletesFinalityStep ==
  (/\ DeliveredPendingCompleteWaitState
   /\ [Next]_vars
   /\ (\/ /\ HonestCommitVote
          /\ CanCommit(
               commitVotesHonest + 1,
               commitVotesByz,
               stakeSigned + StakePerHonestVote,
               rbcState
             )
       \/ /\ ByzantineEquivocateCommit
          /\ CanCommit(
               commitVotesHonest,
               commitVotesByz + 1,
               stakeSigned + StakePerByzVote,
               rbcState
             ))) =>
    /\ DeliveredPendingCompleteWaitStateCommitVoteStepSplitsStep
    /\ RbcDeliveredFinalityStepCompletesCommittedDelivery
    /\ RbcDeliveredFinalityInstallsFinalityCertificateStackStep
    /\ RbcDeliveredFinalityHasExactCommitVoteActionFrameStep
    /\ RbcDeliveredFinalityInstallsCommittedPostStateInvariantsStep
    /\ RbcDeliveredFinalityPostStateGateSplitStep
    /\ ~DeliveredPendingCompleteWaitState'
    /\ committed'
    /\ phase = "CommitVote"
    /\ phase' = "Committed"
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ view' = view
    /\ gst' = gst
    /\ prepareVotes' = prepareVotes
    /\ commitEvidenceVotes' = commitVotesHonest' + commitVotesByz'
    /\ commitEvidenceVotes' >= CommitQuorum
    /\ commitEvidenceStake' = stakeSigned'
    /\ commitEvidenceStake' >= StakeQuorum
    /\ commitView' = view'
    /\ commitView' = view
    /\ CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ FinalityCertificateStackPresent'
    /\ FinalityCertificateStackComplete'
    /\ FinalityCertificateStackMatchesFinality'
    /\ CommittedPhaseMatchesFinality'
    /\ CommitCertificateMatchesFinality'
    /\ LiveCommitGateMatchesFinality'
    /\ LiveCommitGateRbcEvidenceMatches'
    /\ CommitViewMatchesFinality'
    /\ FinalityClearsNewViewHandoff'
    /\ CommitEvidenceMatchesVoteCounters'
    /\ CommitImpliesPrepareQuorum'
    /\ CommitImpliesLiveVoteQuorum'
    /\ CommitImpliesLiveStakeQuorum'
    /\ CommitImpliesHonestSupport'
    /\ CommitDisablesProgressActions'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~PostGstProgressEnabled'
    /\ ~TimeoutTickEnabled'
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ (HonestCommitVote =>
          /\ ~ByzantineEquivocateCommit
          /\ RbcDeliveredPendingHonestCommitVoteStepCompletesFinality
          /\ commitVotesHonest' = commitVotesHonest + 1
          /\ commitVotesByz' = commitVotesByz
          /\ stakeSigned' = stakeSigned + StakePerHonestVote
          /\ commitEvidenceVotes' = commitVotesHonest + 1 + commitVotesByz
          /\ commitEvidenceStake' = stakeSigned + StakePerHonestVote)
    /\ (ByzantineEquivocateCommit =>
          /\ ~HonestCommitVote
          /\ RbcDeliveredPendingByzantineCommitVoteStepCompletesFinality
          /\ commitVotesHonest' = commitVotesHonest
          /\ commitVotesByz' = commitVotesByz + 1
          /\ stakeSigned' = stakeSigned + StakePerByzVote
          /\ commitEvidenceVotes' = commitVotesHonest + commitVotesByz + 1
          /\ commitEvidenceStake' = stakeSigned + StakePerByzVote)

DeliveredPendingCompleteWaitStatePrepareVoteStepSplitsStep ==
  (/\ DeliveredPendingCompleteWaitState
   /\ [Next]_vars
   /\ HonestPrepareVote) =>
    /\ DeliveredPendingCompleteWaitStateSpecStepClosesStep
    /\ RbcDeliveredPendingSpecStepVoteCountersMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepPostGatesMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepTimerGatesMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepActionSourcesExclusiveStep
    /\ DeliveredPendingCompleteWaitState'
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
    /\ ~committed'
    /\ phase = "Prepare"
    /\ phase' \in {"Prepare", "CommitVote"}
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ view' = view
    /\ gst' = gst
    /\ prepareVotes' = prepareVotes + 1
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~FinalityCertificateStackPresent'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ VoteCountersRespectRosterBudgets'
    /\ StakeSignedMatchesVoteCounters'
    /\ LiveStakeSignedIsBounded'
    /\ CommitEvidenceIsBounded'
    /\ CommitEvidenceIsCompleteOrEmpty'
    /\ ViewEvidenceIsCompleteOrEmpty'
    /\ ~HonestProposeEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
    /\ (prepareVotes + 1 < CommitQuorum =>
          /\ RbcDeliveredPendingPrepareVoteStepKeepsWaitState
          /\ phase' = "Prepare"
          /\ prepareVotes' < CommitQuorum
          /\ HonestPrepareVoteEnabled'
          /\ ~HonestCommitVoteEnabled'
          /\ ~ByzantineCommitVoteEnabled'
          /\ (PostGstProgressEnabled' <=> HonestPrepareVoteEnabled'))
    /\ (prepareVotes + 1 >= CommitQuorum =>
          /\ RbcDeliveredPendingPrepareVoteStepStartsCommitVoteWaitState
          /\ phase' = "CommitVote"
          /\ prepareVotes' >= CommitQuorum
          /\ ~HonestPrepareVoteEnabled'
          /\ (HonestCommitVoteEnabled' <=> commitVotesHonest' < N - F)
          /\ (ByzantineCommitVoteEnabled' <=> commitVotesByz' < F)
          /\ (PostGstProgressEnabled' <=> HonestCommitVoteEnabled'))

DeliveredPendingCompleteWaitStateTimeoutStepStartsNewViewStep ==
  (/\ DeliveredPendingCompleteWaitState
   /\ [Next]_vars
   /\ TimeoutTick) =>
    /\ DeliveredPendingCompleteWaitStateSpecStepClosesStep
    /\ RbcDeliveredPendingSpecStepViewChangesOnlyByTimeoutStep
    /\ RbcDeliveredPendingSpecStepViewEvidenceChangesOnlyByNewViewOrTimeoutStep
    /\ RbcDeliveredPendingSpecStepVoteCountersMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepPostGatesMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepTimerGatesMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepActionSourcesExclusiveStep
    /\ RbcDeliveredPendingTimeoutStepStartsNewViewWaitState
    /\ TimeoutTickStepStartsFreshNewView
    /\ TimeoutTickStepStartsNewViewVoteHandoff
    /\ TimeoutTickStepPreservesRbcEvidence
    /\ DeliveredPendingCompleteWaitState'
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
    /\ ~committed'
    /\ phase' = "NewView"
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ view' = IF view < MaxView THEN view + 1 ELSE MaxView
    /\ view' >= view
    /\ view' <= MaxView
    /\ gst' = gst
    /\ prepareVotes' = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~FinalityCertificateStackPresent'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ ViewEvidenceIsCompleteOrEmpty'
    /\ VoteCountersRespectRosterBudgets'
    /\ StakeSignedMatchesVoteCounters'
    /\ LiveStakeSignedIsBounded'
    /\ CommitEvidenceIsBounded'
    /\ CommitEvidenceIsCompleteOrEmpty'
    /\ HonestNewViewVoteEnabled'
    /\ ~HonestProposeEnabled'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ (PostGstProgressEnabled' <=> HonestNewViewVoteEnabled')
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

DeliveredPendingCompleteWaitStateNewViewVoteStepSplitsStep ==
  (/\ DeliveredPendingCompleteWaitState
   /\ [Next]_vars
   /\ HonestNewViewVote) =>
    /\ DeliveredPendingCompleteWaitStateSpecStepClosesStep
    /\ RbcDeliveredPendingSpecStepViewChangesOnlyByTimeoutStep
    /\ RbcDeliveredPendingSpecStepViewEvidenceChangesOnlyByNewViewOrTimeoutStep
    /\ RbcDeliveredPendingSpecStepVoteCountersMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepPostGatesMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepTimerGatesMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepActionSourcesExclusiveStep
    /\ DeliveredPendingCompleteWaitState'
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
    /\ ~committed'
    /\ phase = "NewView"
    /\ phase' \in {"NewView", "Propose"}
    /\ view > 0
    /\ view' = view
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ gst' = gst
    /\ newViewVotes' = newViewVotes + 1
    /\ prepareVotes' = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~FinalityCertificateStackPresent'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ ViewEvidenceIsCompleteOrEmpty'
    /\ VoteCountersRespectRosterBudgets'
    /\ StakeSignedMatchesVoteCounters'
    /\ LiveStakeSignedIsBounded'
    /\ CommitEvidenceIsBounded'
    /\ CommitEvidenceIsCompleteOrEmpty'
    /\ ~HonestPrepareVoteEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'
    /\ (newViewVotes + 1 < ViewQuorum =>
          /\ RbcDeliveredPendingNewViewVoteStepKeepsWaitState
          /\ NewViewVotePendingStepPreservesPreProposalArtifacts
          /\ phase' = "NewView"
          /\ newViewVotes' < ViewQuorum
          /\ viewEvidenceVotes' = 0
          /\ HonestNewViewVoteEnabled'
          /\ ~HonestProposeEnabled'
          /\ (PostGstProgressEnabled' <=> HonestNewViewVoteEnabled'))
    /\ (newViewVotes + 1 >= ViewQuorum =>
          /\ RbcDeliveredPendingNewViewVoteStepStartsProposalWaitState
          /\ NewViewVoteQuorumStepInstallsViewEvidence
          /\ NewViewVoteQuorumStepStartsProposalHandoff
          /\ phase' = "Propose"
          /\ newViewVotes' >= ViewQuorum
          /\ viewEvidenceVotes' = newViewVotes'
          /\ viewEvidenceVotes' >= ViewQuorum
          /\ HonestProposeEnabled'
          /\ ~HonestNewViewVoteEnabled'
          /\ (PostGstProgressEnabled' <=> HonestProposeEnabled'))

DeliveredPendingCompleteWaitStateHonestProposeStepStartsPrepareStep ==
  (/\ DeliveredPendingCompleteWaitState
   /\ [Next]_vars
   /\ HonestPropose) =>
    /\ DeliveredPendingCompleteWaitStateSpecStepClosesStep
    /\ RbcDeliveredPendingSpecStepViewEvidenceChangesOnlyByNewViewOrTimeoutStep
    /\ RbcDeliveredPendingSpecStepVoteCountersMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepPostGatesMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepTimerGatesMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepActionSourcesExclusiveStep
    /\ RbcDeliveredPendingHonestProposeStepStartsPrepareWaitState
    /\ HonestProposeStepStartsPrepareAndRbc
    /\ HonestProposeStepStartsPrepareVoteHandoff
    /\ DeliveredPendingCompleteWaitState'
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
    /\ ~committed'
    /\ phase = "Propose"
    /\ phase' = "Prepare"
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ view' = view
    /\ gst' = gst
    /\ prepareVotes' = 0
    /\ commitVotesHonest' = 0
    /\ commitVotesByz' = 0
    /\ stakeSigned' = 0
    /\ newViewVotes' = 0
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~FinalityCertificateStackPresent'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ ViewEvidenceIsCompleteOrEmpty'
    /\ VoteCountersRespectRosterBudgets'
    /\ StakeSignedMatchesVoteCounters'
    /\ LiveStakeSignedIsBounded'
    /\ CommitEvidenceIsBounded'
    /\ CommitEvidenceIsCompleteOrEmpty'
    /\ HonestPrepareVoteEnabled'
    /\ ~HonestProposeEnabled'
    /\ ~HonestCommitVoteEnabled'
    /\ ~ByzantineCommitVoteEnabled'
    /\ ~HonestNewViewVoteEnabled'
    /\ (PostGstProgressEnabled' <=> HonestPrepareVoteEnabled')
    /\ (GstElapsedEnabled' <=> ~gst')
    /\ (TimeoutTickEnabled' <=> (~gst' \/ ~PostGstProgressEnabled'))
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

DeliveredPendingCompleteWaitStateGstElapsedStepKeepsWaitStateStep ==
  (/\ DeliveredPendingCompleteWaitState
   /\ [Next]_vars
   /\ GstElapsed) =>
    /\ DeliveredPendingCompleteWaitStateSpecStepClosesStep
    /\ RbcDeliveredPendingSpecStepGstChangesOnlyByElapsedStep
    /\ RbcDeliveredPendingSpecStepPostGatesMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepTimerGatesMatchHandoffStep
    /\ RbcDeliveredPendingSpecStepActionSourcesExclusiveStep
    /\ RbcDeliveredPendingGstElapsedStepKeepsWaitState
    /\ GstElapsedStepOnlySetsGst
    /\ DeliveredPendingCompleteWaitState'
    /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence'
    /\ RbcDeliveredWithoutFinalityHasNoCommitCertificate'
    /\ ~committed'
    /\ ~gst
    /\ gst'
    /\ phase' = phase
    /\ phase' \in {"Propose", "Prepare", "CommitVote", "NewView"}
    /\ view' = view
    /\ rbcState' = "Delivered"
    /\ readyVotes' = readyVotes
    /\ readyVotes' >= CommitQuorum
    /\ chunkCount' = chunkCount
    /\ chunkCount' >= MaxChunks
    /\ headerSeen' = headerSeen
    /\ headerSeen'
    /\ digestValid' = digestValid
    /\ digestValid'
    /\ prepareVotes' = prepareVotes
    /\ commitVotesHonest' = commitVotesHonest
    /\ commitVotesByz' = commitVotesByz
    /\ stakeSigned' = stakeSigned
    /\ newViewVotes' = newViewVotes
    /\ viewEvidenceVotes' = viewEvidenceVotes
    /\ commitEvidenceVotes' = 0
    /\ commitEvidenceStake' = 0
    /\ commitView' = 0
    /\ ~FinalityCertificateStackPresent'
    /\ ~CanCommit(commitVotesHonest', commitVotesByz', stakeSigned', rbcState')
    /\ ViewEvidenceIsCompleteOrEmpty'
    /\ VoteCountersRespectRosterBudgets'
    /\ StakeSignedMatchesVoteCounters'
    /\ LiveStakeSignedIsBounded'
    /\ CommitEvidenceIsBounded'
    /\ CommitEvidenceIsCompleteOrEmpty'
    /\ (HonestProposeEnabled' <=> HonestProposeEnabled)
    /\ (HonestPrepareVoteEnabled' <=> HonestPrepareVoteEnabled)
    /\ (HonestCommitVoteEnabled' <=> HonestCommitVoteEnabled)
    /\ (ByzantineCommitVoteEnabled' <=> ByzantineCommitVoteEnabled)
    /\ (HonestNewViewVoteEnabled' <=> HonestNewViewVoteEnabled)
    /\ (PostGstProgressEnabled' <=> PostGstProgressEnabled)
    /\ ~GstElapsedEnabled'
    /\ (TimeoutTickEnabled' <=> ~PostGstProgressEnabled')
    /\ ~RbcInitEnabled'
    /\ ~RbcChunkGoodEnabled'
    /\ ~RbcReadyGoodEnabled'
    /\ ~RbcDeliverGoodEnabled'
    /\ ~ByzantineFaultEnabled'

DeliveredPendingCompleteWaitStateNextStepMatchesNamedActionBranchStep ==
  (/\ DeliveredPendingCompleteWaitState
   /\ [Next]_vars
   /\ Next) =>
    /\ DeliveredPendingCompleteWaitStateSpecStepClosesStep
    /\ RbcDeliveredPendingNextStepCoveredByHandoffs
    /\ RbcDeliveredPendingSpecStepStuttersOrTakesCoveredHandoffStep
    /\ RbcDeliveredPendingSpecStepActionSourcesExclusiveStep
    /\ \/ HonestCommitVote
       \/ ByzantineEquivocateCommit
       \/ HonestPrepareVote
       \/ TimeoutTick
       \/ HonestNewViewVote
       \/ HonestPropose
       \/ GstElapsed
    /\ ~RbcInit
    /\ ~RbcChunkGood
    /\ ~RbcReadyGood
    /\ ~RbcDeliverGood
    /\ ~ByzantineFault
    /\ ((HonestCommitVote \/ ByzantineEquivocateCommit) =>
          DeliveredPendingCompleteWaitStateCommitVoteStepSplitsStep)
    /\ (HonestPrepareVote =>
          DeliveredPendingCompleteWaitStatePrepareVoteStepSplitsStep)
    /\ (TimeoutTick =>
          DeliveredPendingCompleteWaitStateTimeoutStepStartsNewViewStep)
    /\ (HonestNewViewVote =>
          DeliveredPendingCompleteWaitStateNewViewVoteStepSplitsStep)
    /\ (HonestPropose =>
          DeliveredPendingCompleteWaitStateHonestProposeStepStartsPrepareStep)
    /\ (GstElapsed =>
          DeliveredPendingCompleteWaitStateGstElapsedStepKeepsWaitStateStep)
    /\ (committed' =>
          /\ phase' = "Committed"
          /\ ~DeliveredPendingCompleteWaitState'
          /\ FinalityCertificateStackPresent')
    /\ (~committed' =>
          /\ DeliveredPendingCompleteWaitState'
          /\ RbcDeliveredWithoutFinalityWaitsForCommitEvidence')

RbcStateChangeMatchesLocalExitClassificationStep ==
  (rbcState' # rbcState) =>
    /\ RbcStateOnlyChangesByProtocolOrFaultStep
    /\ RbcEvidenceOnlyChangesByProtocolOrFaultStep
    /\ rbcState \in {"Idle", "Init", "Chunking", "ChunksComplete", "ReadyPartial", "ReadyQuorum", "Corrupted"}
    /\ \/ /\ rbcState = "Idle"
          /\ RbcIdleExitOnlyByProposalOrInitStep
       \/ /\ rbcState = "Init"
          /\ RbcInitExitOnlyByChunkOrFaultStep
       \/ /\ rbcState = "Chunking"
          /\ RbcChunkingExitOnlyByChunkOrFaultStep
       \/ /\ rbcState = "ChunksComplete"
          /\ RbcChunksCompleteExitOnlyByReadyOrFaultStep
       \/ /\ rbcState = "ReadyPartial"
          /\ RbcReadyPartialExitOnlyByReadyOrFaultStep
       \/ /\ rbcState = "ReadyQuorum"
          /\ RbcReadyQuorumExitOnlyByDeliverOrFaultStep
       \/ /\ rbcState = "Corrupted"
          /\ RbcCorruptionExitOnlyByInitStep

RbcEvidenceChangeMatchesLocalEffectClassificationStep ==
  (\/ headerSeen' # headerSeen
   \/ digestValid' # digestValid
   \/ chunkCount' # chunkCount
   \/ readyVotes' # readyVotes) =>
    /\ RbcEvidenceOnlyChangesByProtocolOrFaultStep
    /\ ((headerSeen' # headerSeen) =>
          /\ ~headerSeen
          /\ headerSeen'
          /\ RbcHeaderInstallationOnlyByProposalOrInitStep)
    /\ ((digestValid' # digestValid) =>
          \/ /\ ~digestValid
             /\ digestValid'
             /\ RbcDigestInstallationOnlyByProposalInitOrChunkStep
          \/ /\ digestValid
             /\ ~digestValid'
             /\ RbcDigestInvalidationOnlyByFaultStep)
    /\ ((chunkCount' # chunkCount) =>
          \/ /\ chunkCount' > chunkCount
             /\ RbcChunkCountIncreaseOnlyByChunkStep
          \/ /\ chunkCount' < chunkCount
             /\ RbcChunkCountDecreaseOnlyByProposalOrInitStep)
    /\ ((readyVotes' # readyVotes) =>
          \/ /\ readyVotes' > readyVotes
             /\ RbcReadyVotesIncreaseOnlyByReadyStep
          \/ /\ readyVotes' < readyVotes
             /\ RbcReadyVotesDecreaseOnlyByProposalOrInitStep)

RbcWithheldEntryOnlyByStutteringFromWithheldStep ==
  (rbcState' = "Withheld") =>
    /\ rbcState = "Withheld"
    /\ vars' = vars

LiveHeaderDigestEvidenceStayInRbcHandoff ==
  /\ (digestValid =>
        /\ headerSeen
        /\ rbcState \in RbcInitializedStates)
  /\ (headerSeen =>
        \/ /\ rbcState \in RbcInitializedStates
           /\ digestValid
        \/ /\ rbcState \in {"Corrupted", "Withheld"}
           /\ ~digestValid)
  /\ (rbcState = "Corrupted" => ~digestValid)

LiveChunkEvidenceStayInRbcHandoff ==
  chunkCount > 0 =>
    \/ /\ rbcState = "Corrupted"
       /\ ~digestValid
    \/ /\ rbcState \in {"Chunking", "ChunksComplete", "ReadyPartial", "ReadyQuorum", "Delivered"}
       /\ headerSeen
       /\ digestValid
       /\ (rbcState = "Chunking" => chunkCount < MaxChunks)
       /\ (rbcState \in RbcChunkCoveredStates => chunkCount >= MaxChunks)

LiveReadyVotesStayInRbcHandoff ==
  readyVotes > 0 =>
    \/ /\ rbcState = "Corrupted"
       /\ ~digestValid
    \/ /\ rbcState \in {"ReadyPartial", "ReadyQuorum", "Delivered"}
       /\ headerSeen
       /\ digestValid
       /\ chunkCount >= MaxChunks
       /\ (rbcState = "ReadyPartial" => readyVotes < CommitQuorum)
       /\ (rbcState \in RbcReadyQuorumStates => readyVotes >= CommitQuorum)

EventuallyCommit ==
  [] (gst => <> committed)

CommitNeverRevoked ==
  [] (committed => [] committed)

CommittedPhaseAlwaysMatchesFinality ==
  [] CommittedPhaseMatchesFinality

CommitCertificateAlwaysMatchesFinality ==
  [] CommitCertificateMatchesFinality

LiveCommitGateAlwaysMatchesFinality ==
  [] LiveCommitGateMatchesFinality

LiveCommitGateRbcEvidenceAlwaysMatches ==
  [] LiveCommitGateRbcEvidenceMatches

CommittedPhaseNeverLeaves ==
  [] (phase = "Committed" => [] (phase = "Committed"))

CommittedConsensusStateNeverChanges ==
  [] [CommittedConsensusStateStableStep]_vars

CommittedOnlyGstObservationCanChange ==
  [] [CommittedOnlyGstObservationCanMoveStep]_vars

CommittedPreGstOnlyGstElapsedCanMove ==
  [] [CommittedPreGstOnlyGstElapsedCanMoveStep]_vars

CommittedPreGstNextOnlyGstElapsed ==
  [] [CommittedPreGstNextOnlyGstElapsedStep]_vars

CommittedPreGstSpecStepStuttersOrObservesGst ==
  [] [CommittedPreGstSpecStepStuttersOrObservesGstStep]_vars

CommittedGstStateNeverChanges ==
  [] [CommittedGstStateStableStep]_vars

CommittedGstNeverEnablesActions ==
  [] CommittedGstDisablesEveryAction

CommittedGstOnlyAllowsStuttering ==
  [] [CommittedGstRejectsNextStep]_vars

CommittedGstSpecStepOnlyStutters ==
  [] [CommittedGstSpecStepOnlyStuttersStep]_vars

CommittedSpecNonStutteringOnlyObservesGst ==
  [] [CommittedSpecNonStutteringOnlyObservesGstStep]_vars

CommittedSpecStepStuttersOrObservesGst ==
  [] [CommittedSpecStepStuttersOrObservesGstStep]_vars

CommittedSpecStepPreservesFinalityStack ==
  [] [CommittedSpecStepPreservesFinalityStackStep]_vars

CommittedSpecStepOnlyChangesGstFlag ==
  [] [CommittedSpecStepOnlyChangesGstFlagStep]_vars

CommittedSpecStepNeverRunsProtocolActions ==
  [] [CommittedSpecStepNeverRunsProtocolActionsStep]_vars

CommittedSpecStepKeepsProgressActionsQuiescent ==
  [] [CommittedSpecStepKeepsProgressActionsQuiescentStep]_vars

CommittedSpecStepPreservesBudgetedRbcEvidence ==
  [] [CommittedSpecStepPreservesBudgetedRbcEvidenceStep]_vars

CommitArtifactsOnlyInstallAtFinality ==
  [] [CommitArtifactsInstallOnlyAtFinalityStep]_vars

CommitArtifactsOnlyChangeByFinalitySource ==
  [] [CommitArtifactsOnlyChangeByFinalitySourceStep]_vars

CommitArtifactsChangeAlwaysMatchesCertifiedFinalityStack ==
  [] [CommitArtifactsChangeMatchesCertifiedFinalityStackStep]_vars

CommitArtifactsChangeAlwaysCompletesCommittedDeliveryFromExactSource ==
  [] [CommitArtifactsChangeCompletesCommittedDeliveryFromExactSourceStep]_vars

CommitArtifactsChangeAlwaysCommitsCurrentView ==
  [] [CommitArtifactsChangeCommitsCurrentViewStep]_vars

CommitArtifactsChangeNeverChangesGst ==
  [] [CommitArtifactsChangePreservesGstStep]_vars

CommitArtifactsChangeOnlyLeavesGstElapsedGate ==
  [] [CommitArtifactsChangeLeavesOnlyGstElapsedGateStep]_vars

FinalityLatchOnlySetsCompleteStack ==
  [] [FinalityLatchSetInstallsCompleteStackStep]_vars

FinalityLatchAndArtifactsAlwaysChangeTogether ==
  [] [FinalityLatchAndArtifactsCoupledStep]_vars

CommittedPhaseOnlyEntersWithCompleteStack ==
  [] [CommittedPhaseEntryInstallsCompleteStackStep]_vars

CommittedPhaseEntryAlwaysMatchesFinalityLatch ==
  [] [CommittedPhaseEntryMatchesFinalityLatchStep]_vars

FinalityLatchChangeOnlyEntersCommittedPhase ==
  [] [FinalityLatchChangeEntersCommittedPhaseStep]_vars

FinalityLatchChangeAlwaysMatchesLiveCommitGateCrossing ==
  [] [FinalityLatchChangeMatchesLiveCommitGateCrossingStep]_vars

CommitCertificateWitnessesAlwaysInstallWithFinalityLatch ==
  [] [CommitCertificateWitnessesInstallWithFinalityLatchStep]_vars

CommitCertificateWitnessComponentsAlwaysChangeTogether ==
  [] [CommitCertificateWitnessComponentsChangeTogetherStep]_vars

CommitCertificateWitnessChangeAlwaysMatchesCertifiedFinalityStack ==
  [] [CommitCertificateWitnessChangeMatchesCertifiedFinalityStackStep]_vars

CommitCertificateWitnessChangeAlwaysInstallsCommitViewWitness ==
  [] [CommitCertificateWitnessChangeInstallsCommitViewWitnessStep]_vars

CommitCertificateWitnessChangeAlwaysCompletesCommittedDeliveryFromExactSource ==
  [] [CommitCertificateWitnessChangeCompletesCommittedDeliveryFromExactSourceStep]_vars

CommitCertificateWitnessChangeNeverChangesGst ==
  [] [CommitCertificateWitnessChangePreservesGstStep]_vars

CommitCertificateWitnessChangeOnlyLeavesGstElapsedGate ==
  [] [CommitCertificateWitnessChangeLeavesOnlyGstElapsedGateStep]_vars

CommitViewWitnessOnlyChangesOnNonzeroFinality ==
  [] [CommitViewWitnessChangesOnlyOnNonzeroFinalityStep]_vars

CommitViewWitnessAlwaysInstallsWithFinalityLatch ==
  [] [CommitViewWitnessInstallsWithFinalityLatchStep]_vars

CommitViewWitnessChangeAlwaysMatchesCertifiedFinalityStack ==
  [] [CommitViewWitnessChangeMatchesCertifiedFinalityStackStep]_vars

CommitViewWitnessChangeAlwaysInstallsCommitCertificateWitnesses ==
  [] [CommitViewWitnessChangeInstallsCommitCertificateWitnessesStep]_vars

CommitViewWitnessChangeAlwaysCompletesCommittedDeliveryFromExactSource ==
  [] [CommitViewWitnessChangeCompletesCommittedDeliveryFromExactSourceStep]_vars

CommitViewWitnessChangeNeverChangesGst ==
  [] [CommitViewWitnessChangePreservesGstStep]_vars

CommitViewWitnessChangeOnlyLeavesGstElapsedGate ==
  [] [CommitViewWitnessChangeLeavesOnlyGstElapsedGateStep]_vars

FinalityLatchNeverCarriesNewViewHandoff ==
  [] [FinalityLatchNeverCarriesNewViewHandoffStep]_vars

FinalityLatchOnlyComesFromCommitOrDelivery ==
  [] [FinalityLatchSourceIsCommitOrDeliveryStep]_vars

FinalityLatchChangeNeverChangesGst ==
  [] [FinalityLatchChangePreservesGstStep]_vars

FinalityLatchChangeOnlyLeavesGstElapsedGate ==
  [] [FinalityLatchChangeLeavesOnlyGstElapsedGateStep]_vars

FinalityLatchSourceEffectsAlwaysExact ==
  [] [FinalityLatchSourceEffectsAreExactStep]_vars

FinalityLatchSourceQuorumGatesAlwaysHold ==
  [] [FinalityLatchSourceQuorumGatesHoldStep]_vars

FinalitySourceActionAlwaysCompletesCommittedDeliveryFromExactSource ==
  [] [FinalitySourceActionCompletesCommittedDeliveryFromExactSourceStep]_vars

FinalitySourceActionAlwaysMatchesCertifiedSourceStack ==
  [] [FinalitySourceActionMatchesCertifiedSourceStackStep]_vars

FinalitySourceActionAlwaysMatchesFinalityLatchChange ==
  [] [FinalitySourceActionMatchesFinalityLatchChangeStep]_vars

FinalitySourceActionAlwaysMatchesCommittedPhaseEntry ==
  [] [FinalitySourceActionMatchesCommittedPhaseEntryStep]_vars

FinalitySourceActionAlwaysInstallsFinalityCertificateStack ==
  [] [FinalitySourceActionInstallsFinalityCertificateStackStep]_vars

FinalitySourceActionSourceAlwaysIsCommitOrDelivery ==
  [] [FinalitySourceActionSourceIsCommitOrDeliveryStep]_vars

FinalitySourceActionSourceEffectsAlwaysExact ==
  [] [FinalitySourceActionSourceEffectsAreExactStep]_vars

FinalitySourceActionQuorumGatesAlwaysHold ==
  [] [FinalitySourceActionQuorumGatesHoldStep]_vars

FinalitySourceActionAlwaysMatchesCommitArtifactsChange ==
  [] [FinalitySourceActionMatchesCommitArtifactsChangeStep]_vars

FinalitySourceActionAlwaysMatchesLiveCommitGateCrossing ==
  [] [FinalitySourceActionMatchesLiveCommitGateCrossingStep]_vars

FinalitySourceActionAlwaysDisablesProgressAfterCommittedDelivery ==
  [] [FinalitySourceActionDisablesProgressAfterCommittedDeliveryStep]_vars

FinalitySourceActionNeverChangesGst ==
  [] [FinalitySourceActionPreservesGstStep]_vars

FinalitySourceActionOnlyLeavesGstElapsedGate ==
  [] [FinalitySourceActionLeavesOnlyGstElapsedGateStep]_vars

FinalitySourceActionAlwaysInstallsCommitCertificateWitnesses ==
  [] [FinalitySourceActionInstallsCommitCertificateWitnessesStep]_vars

FinalitySourceActionAlwaysMatchesCommitCertificateWitnessChange ==
  [] [FinalitySourceActionMatchesCommitCertificateWitnessChangeStep]_vars

FinalitySourceActionAlwaysMatchesCommitViewWitnessChange ==
  [] [FinalitySourceActionMatchesCommitViewWitnessChangeStep]_vars

FinalitySourceActionAlwaysInstallsCommitViewWitness ==
  [] [FinalitySourceActionInstallsCommitViewWitnessStep]_vars

FinalitySourceActionNeverCarriesNewViewHandoff ==
  [] [FinalitySourceActionNeverCarriesNewViewHandoffStep]_vars

FinalitySourceActionAlwaysCommitsCurrentView ==
  [] [FinalitySourceActionCommitsCurrentViewStep]_vars

FinalityLatchChangeAlwaysMatchesCertifiedSourceStack ==
  [] [FinalityLatchChangeMatchesCertifiedSourceStackStep]_vars

FinalityLatchChangeAlwaysCompletesCommittedDeliveryFromExactSource ==
  [] [FinalityLatchChangeCompletesCommittedDeliveryFromExactSourceStep]_vars

CommitViewNeverChanges ==
  [] (committed => [] (view = commitView))

CommittedViewWitnessAlwaysStaysAtCommittedView ==
  [] [CommittedViewWitnessStaysAtCommittedViewStep]_vars

CommitViewNeverLeadsCurrentView ==
  [] CommitViewDoesNotLeadCurrentView

GstElapsedGateNeverBypassesPreGst ==
  [] GstElapsedGateMatchesPreGst

GstElapsedStepAlwaysOnlySetsGst ==
  [] [GstElapsedStepOnlySetsGst]_vars

GstOnlyChangesByElapsed ==
  [] [GstOnlyChangesByElapsedStep]_vars

GstNeverRegresses ==
  [] [GstMonotonicStep]_vars

ViewNeverRegresses ==
  [] [ViewMonotonicStep]_vars

CommitViewNeverRegresses ==
  [] [CommitViewMonotonicStep]_vars

CommitEvidenceNeverRegresses ==
  [] [CommitEvidenceMonotonicStep]_vars

TimeoutTickGateNeverBypassesStalledProgress ==
  [] TimeoutTickGateMatchesStalledProgress

TimeoutTickStepAlwaysStartsFreshNewView ==
  [] [TimeoutTickStepStartsFreshNewView]_vars

TimeoutTickStepNeverPreemptsProgress ==
  [] [TimeoutTickStepNeverPreemptsProgressStep]_vars

TimeoutTickStepAlwaysClearsCommitVoteGates ==
  [] [TimeoutTickStepClearsCommitVoteGates]_vars

TimeoutTickStepAlwaysStartsNewViewVoteHandoff ==
  [] [TimeoutTickStepStartsNewViewVoteHandoff]_vars

TimeoutTickStepAlwaysPreservesRbcEvidence ==
  [] [TimeoutTickStepPreservesRbcEvidence]_vars

ViewAdvanceOnlyComesFromTimeout ==
  [] [ViewAdvanceOnlyComesFromTimeoutStep]_vars

LiveProgressResetOnlyByTimeout ==
  [] [LiveProgressResetOnlyByTimeoutStep]_vars

ViewEvidenceOnlyChangesByQuorumOrTimeout ==
  [] [ViewEvidenceChangesOnlyByQuorumOrTimeoutStep]_vars

NewViewVotesOnlyChangeByVoteOrReset ==
  [] [NewViewVotesChangeOnlyByVoteOrResetStep]_vars

PrepareVotesOnlyChangeByVoteOrTimeout ==
  [] [PrepareVotesChangeOnlyByVoteOrTimeoutStep]_vars

CommitVoteCountersOnlyChangeByVoteOrTimeout ==
  [] [CommitVoteCountersChangeOnlyByVoteOrTimeoutStep]_vars

PhaseOnlyChangesByProtocol ==
  [] [PhaseOnlyChangesByProtocolStep]_vars

PreparePhaseEntryOnlyByProposal ==
  [] [PreparePhaseEntryOnlyByProposalStep]_vars

CommitVotePhaseEntryOnlyByPrepareQuorum ==
  [] [CommitVotePhaseEntryOnlyByPrepareQuorumStep]_vars

ProposePhaseEntryOnlyByNewViewQuorum ==
  [] [ProposePhaseEntryOnlyByNewViewQuorumStep]_vars

NewViewPhaseEntryOnlyByTimeout ==
  [] [NewViewPhaseEntryOnlyByTimeoutStep]_vars

CommittedPhaseEntryOnlyByFinalitySource ==
  [] [CommittedPhaseEntryOnlyByFinalitySourceStep]_vars

CommittedPhaseEntryAlwaysMatchesCertifiedFinalityStack ==
  [] [CommittedPhaseEntryMatchesCertifiedFinalityStackStep]_vars

CommittedPhaseEntryAlwaysInstallsCommitCertificateWitnesses ==
  [] [CommittedPhaseEntryInstallsCommitCertificateWitnessesStep]_vars

CommittedPhaseEntryAlwaysMatchesCommitCertificateWitnessChange ==
  [] [CommittedPhaseEntryMatchesCommitCertificateWitnessChangeStep]_vars

CommittedPhaseEntryAlwaysMatchesCommitViewWitnessChange ==
  [] [CommittedPhaseEntryMatchesCommitViewWitnessChangeStep]_vars

CommittedPhaseEntryAlwaysInstallsCommitViewWitness ==
  [] [CommittedPhaseEntryInstallsCommitViewWitnessStep]_vars

CommittedPhaseEntryAlwaysMatchesLiveCommitGateCrossing ==
  [] [CommittedPhaseEntryMatchesLiveCommitGateCrossingStep]_vars

CommittedPhaseEntryAlwaysMatchesCommitArtifactsChange ==
  [] [CommittedPhaseEntryMatchesCommitArtifactsChangeStep]_vars

CommittedPhaseEntryAlwaysMatchesExactFinalitySourceEffects ==
  [] [CommittedPhaseEntryMatchesExactFinalitySourceEffectsStep]_vars

CommittedPhaseEntryNeverCarriesNewViewHandoff ==
  [] [CommittedPhaseEntryNeverCarriesNewViewHandoffStep]_vars

CommittedPhaseEntryAlwaysCommitsCurrentView ==
  [] [CommittedPhaseEntryCommitsCurrentViewStep]_vars

CommittedPhaseEntryNeverChangesGst ==
  [] [CommittedPhaseEntryPreservesGstStep]_vars

CommittedPhaseEntryOnlyLeavesGstElapsedGate ==
  [] [CommittedPhaseEntryLeavesOnlyGstElapsedGateStep]_vars

CommittedPhaseEntryAlwaysDisablesProgressActions ==
  [] [CommittedPhaseEntryDisablesProgressActionsStep]_vars

CommittedPhaseEntryAlwaysCompletesCommittedDeliveryFromExactSource ==
  [] [CommittedPhaseEntryCompletesCommittedDeliveryFromExactSourceStep]_vars

ViewQuorumEvidenceNeverDiverges ==
  [] ViewEvidenceMatchesActiveView

ViewEvidenceWitnessNeverTargetsZeroOrNewView ==
  [] ViewEvidenceWitnessRequiresNonzeroActiveView

NewViewQuorumHandoffNeverStalls ==
  [] NewViewPhaseBelowQuorum

LiveNewViewVotesNeverLeakPastHandoff ==
  [] LiveNewViewVotesStayInHandoff

HonestProposeGateNeverBypassesHandoffEvidence ==
  [] HonestProposeGateMatchesHandoffEvidence

HonestProposeStepAlwaysStartsPrepareAndRbc ==
  [] [HonestProposeStepStartsPrepareAndRbc]_vars

HonestProposeStepAlwaysStartsPrepareVoteHandoff ==
  [] [HonestProposeStepStartsPrepareVoteHandoff]_vars

NewViewVoteGateNeverBypassesFreshViewEvidence ==
  [] NewViewVoteGateMatchesFreshViewEvidence

NewViewVoteQuorumGateNeverBypassesNextEvidence ==
  [] NewViewVoteQuorumGateMatchesNextEvidence

NewViewVoteQuorumStepAlwaysInstallsViewEvidence ==
  [] [NewViewVoteQuorumStepInstallsViewEvidence]_vars

NewViewVoteQuorumStepAlwaysStartsProposalHandoff ==
  [] [NewViewVoteQuorumStepStartsProposalHandoff]_vars

NewViewVotePendingGateNeverBypassesMissingNextEvidence ==
  [] NewViewVotePendingGateMatchesMissingNextEvidence

NewViewVotePendingStepNeverInstallsViewEvidence ==
  [] [NewViewVotePendingStepPreservesPreProposalArtifacts]_vars

ViewEvidenceNeverPartial ==
  [] ViewEvidenceIsCompleteOrEmpty

PreCommitVotesNeverCarryAcrossViews ==
  [] PreCommitPhasesHaveNoCommitVotes

PrePrepareVotesNeverCarryAcrossViews ==
  [] PrePreparePhasesHaveNoPrepareVotes

LivePrepareVotesNeverBypassPrepareHandoff ==
  [] LivePrepareVotesStayInHandoff

PrepareVoteGateNeverBypassesProposalEvidence ==
  [] PrepareVoteGateMatchesProposalEvidence

PrepareVoteQuorumGateNeverBypassesNextEvidence ==
  [] PrepareVoteQuorumGateMatchesNextEvidence

PrepareVoteQuorumStepAlwaysEntersCommitVote ==
  [] [PrepareVoteQuorumStepEntersCommitVote]_vars

PrepareVoteQuorumStepAlwaysStartsCommitVoteHandoff ==
  [] [PrepareVoteQuorumStepStartsCommitVoteHandoff]_vars

PrepareVotePendingGateNeverBypassesMissingNextEvidence ==
  [] PrepareVotePendingGateMatchesMissingNextEvidence

PrepareVotePendingStepNeverMutatesCommitArtifacts ==
  [] [PrepareVotePendingStepPreservesPreCommitArtifacts]_vars

PrepareVotePendingStepAlwaysKeepsPrepareVoteHandoff ==
  [] [PrepareVotePendingStepKeepsPrepareVoteHandoff]_vars

CommitEvidenceNeverPartial ==
  [] CommitEvidenceIsCompleteOrEmpty

CommitPhasesNeverBypassPrepareQuorum ==
  [] CommitVotePhaseRequiresPrepareQuorum

LiveCommitVotesNeverBypassPrepareQuorum ==
  [] LiveCommitVotesRequirePrepareQuorum

CommitVoteGateNeverBypassesPrepareEvidence ==
  [] CommitVoteGateMatchesPrepareEvidence

ByzantineCommitVoteGateNeverBypassesPrepareEvidence ==
  [] ByzantineCommitVoteGateMatchesPrepareEvidence

HonestCommitVoteFinalityGateNeverBypassesNextEvidence ==
  [] HonestCommitVoteFinalityGateMatchesNextEvidence

HonestCommitVoteFinalityStepAlwaysInstallsCommitArtifacts ==
  [] [HonestCommitVoteFinalityStepInstallsCommitArtifacts]_vars

HonestCommitVoteFinalityStepAlwaysCompletesCommittedDelivery ==
  [] [HonestCommitVoteFinalityStepCompletesCommittedDelivery]_vars

HonestCommitVotePendingGateNeverBypassesMissingNextEvidence ==
  [] HonestCommitVotePendingGateMatchesMissingNextEvidence

HonestCommitVotePendingStepNeverMutatesCommitArtifacts ==
  [] [HonestCommitVotePendingStepPreservesPreFinalityArtifacts]_vars

HonestCommitVotePendingStepAlwaysKeepsCommitVoteHandoff ==
  [] [HonestCommitVotePendingStepKeepsCommitVoteHandoff]_vars

ByzantineCommitVoteFinalityGateNeverBypassesNextEvidence ==
  [] ByzantineCommitVoteFinalityGateMatchesNextEvidence

ByzantineCommitVoteFinalityStepAlwaysInstallsCommitArtifacts ==
  [] [ByzantineCommitVoteFinalityStepInstallsCommitArtifacts]_vars

ByzantineCommitVoteFinalityStepAlwaysCompletesCommittedDelivery ==
  [] [ByzantineCommitVoteFinalityStepCompletesCommittedDelivery]_vars

ByzantineCommitVotePendingGateNeverBypassesMissingNextEvidence ==
  [] ByzantineCommitVotePendingGateMatchesMissingNextEvidence

ByzantineCommitVotePendingStepNeverMutatesCommitArtifacts ==
  [] [ByzantineCommitVotePendingStepPreservesPreFinalityArtifacts]_vars

ByzantineCommitVotePendingStepAlwaysKeepsCommitVoteHandoff ==
  [] [ByzantineCommitVotePendingStepKeepsCommitVoteHandoff]_vars

LiveCommitVotesNeverBypassCommitHandoff ==
  [] LiveCommitVotesStayInCommitHandoff

PreFinalityCommitArtifactsNeverAppear ==
  [] (NoCommitEvidenceBeforeCommit /\ NoCommitViewBeforeCommit)

FinalityCertificateStackNeverIncomplete ==
  [] FinalityCertificateStackComplete

FinalityCertificateStackAlwaysMatchesFinality ==
  [] FinalityCertificateStackMatchesFinality

FinalityNeverRetainsNewViewHandoff ==
  [] (committed => [] (newViewVotes = 0))

CommitViewQuorumEvidenceNeverLost ==
  [] (committed =>
        [] (commitView = 0 \/ viewEvidenceVotes >= ViewQuorum))

PrepareQuorumNeverLostAfterCommit ==
  [] (committed => [] (prepareVotes >= CommitQuorum))

LiveCommitQuorumNeverLost ==
  [] (committed =>
        [] (/\ commitVotesHonest + commitVotesByz >= CommitQuorum
            /\ stakeSigned >= StakeQuorum))

CommitHonestSupportNeverLost ==
  [] (committed =>
        [] (commitVotesHonest >= HonestCommitSupportThreshold))

CommitRbcEvidenceNeverLost ==
  [] (committed =>
        [] (/\ rbcState = "Delivered"
            /\ readyVotes >= CommitQuorum
            /\ chunkCount >= MaxChunks
            /\ headerSeen
            /\ digestValid))

CommitProgressActionsNeverReenabled ==
  [] (committed => [] CommitDisablesProgressActions)

CommitEvidenceNeverDivergesFromVoteCounters ==
  [] (committed =>
        [] (/\ commitEvidenceVotes = commitVotesHonest + commitVotesByz
            /\ commitEvidenceStake = stakeSigned))

StakeAccountingNeverDiverges ==
  [] StakeSignedMatchesVoteCounters

LiveStakeNeverExceedsRosterBudget ==
  [] LiveStakeSignedIsBounded

CommitEvidenceNeverExceedsRosterBudget ==
  [] CommitEvidenceIsBounded

VoteCountersNeverExceedRosterBudgets ==
  [] VoteCountersRespectRosterBudgets

CommitEvidenceNeverLost ==
  [] (committed =>
        [] (/\ commitEvidenceVotes >= CommitQuorum
            /\ commitEvidenceStake >= StakeQuorum))

RbcDeliveryNeverLost ==
  [] (rbcState = "Delivered" => [] (rbcState = "Delivered"))

RbcDeliveredEvidenceNeverRegresses ==
  [] [RbcDeliveredEvidenceStableStep]_vars

RbcDeliveredWithoutFinalityNeverCarriesCommitCertificate ==
  [] RbcDeliveredWithoutFinalityHasNoCommitCertificate

RbcDeliveredFinalityOnlyComesFromCommitVote ==
  [] [RbcDeliveredFinalityOnlyByCommitVoteStep]_vars

RbcDeliveredFinalityAlwaysCompletesCommittedDelivery ==
  [] [RbcDeliveredFinalityStepCompletesCommittedDelivery]_vars

RbcDeliveredFinalityAlwaysCommitsCurrentView ==
  [] [RbcDeliveredFinalityCommitsCurrentViewStep]_vars

RbcDeliveredFinalityOnlyLeavesGstElapsedGate ==
  [] [RbcDeliveredFinalityLeavesOnlyGstElapsedGateStep]_vars

RbcDeliveredFinalityAlwaysInstallsCommitCertificateWitnesses ==
  [] [RbcDeliveredFinalityInstallsCommitCertificateWitnessesStep]_vars

RbcDeliveredFinalityAlwaysMatchesCommitCertificateWitnessChange ==
  [] [RbcDeliveredFinalityMatchesCommitCertificateWitnessChangeStep]_vars

RbcDeliveredFinalityAlwaysMatchesCommitViewWitnessChange ==
  [] [RbcDeliveredFinalityMatchesCommitViewWitnessChangeStep]_vars

RbcDeliveredFinalityAlwaysMatchesLiveCommitGateCrossing ==
  [] [RbcDeliveredFinalityMatchesLiveCommitGateCrossingStep]_vars

RbcDeliveredFinalityAlwaysDisablesProgressAfterCommittedDelivery ==
  [] [RbcDeliveredFinalityDisablesProgressAfterCommittedDeliveryStep]_vars

RbcDeliveredFinalityAlwaysMatchesCertifiedSourceStack ==
  [] [RbcDeliveredFinalityMatchesCertifiedSourceStackStep]_vars

RbcDeliveredFinalityAlwaysInstallsFinalityCertificateStack ==
  [] [RbcDeliveredFinalityInstallsFinalityCertificateStackStep]_vars

RbcDeliveredFinalityAlwaysMatchesCommittedPhaseEntry ==
  [] [RbcDeliveredFinalityMatchesCommittedPhaseEntryStep]_vars

RbcDeliveredFinalityAlwaysMatchesCommitArtifactsChange ==
  [] [RbcDeliveredFinalityMatchesCommitArtifactsChangeStep]_vars

RbcDeliveredFinalityAlwaysCouplesLatchAndCommitArtifacts ==
  [] [RbcDeliveredFinalityCouplesLatchAndCommitArtifactsStep]_vars

RbcDeliveredFinalityAlwaysRecordsExactCommitVoteWitnesses ==
  [] [RbcDeliveredFinalityRecordsExactCommitVoteWitnessesStep]_vars

RbcDeliveredFinalityAlwaysPreservesDeliveredRbcEvidence ==
  [] [RbcDeliveredFinalityPreservesDeliveredRbcEvidenceStep]_vars

RbcDeliveredFinalityAlwaysPreservesViewPrepareHandoffEvidence ==
  [] [RbcDeliveredFinalityPreservesViewPrepareHandoffEvidenceStep]_vars

RbcDeliveredFinalityAlwaysHasExactProtocolFrame ==
  [] [RbcDeliveredFinalityHasExactProtocolFrameStep]_vars

RbcDeliveredFinalityAlwaysHasExactCommitVoteActionFrame ==
  [] [RbcDeliveredFinalityHasExactCommitVoteActionFrameStep]_vars

RbcDeliveredFinalityAlwaysInstallsCommittedPostStateInvariants ==
  [] [RbcDeliveredFinalityInstallsCommittedPostStateInvariantsStep]_vars

RbcDeliveredFinalityAlwaysSplitsPostStateGate ==
  [] [RbcDeliveredFinalityPostStateGateSplitStep]_vars

RbcDeliveredFinalityPreGstPostStateOnlyLeavesGstElapsed ==
  [] [RbcDeliveredFinalityPreGstPostStateLeavesOnlyGstElapsedStep]_vars

RbcDeliveredFinalityPostGstPostStateIsTerminal ==
  [] [RbcDeliveredFinalityPostGstPostStateIsTerminalStep]_vars

RbcDeliveredNeverEnablesRbcProgress ==
  [] RbcDeliveredDisablesRbcProgress

RbcDeliveredWithoutFinalityAlwaysWaitsForCommitEvidence ==
  [] RbcDeliveredWithoutFinalityWaitsForCommitEvidence

RbcDeliveredPendingHonestCommitVoteAlwaysKeepsWaitState ==
  [] [RbcDeliveredPendingHonestCommitVoteStepKeepsWaitState]_vars

RbcDeliveredPendingByzantineCommitVoteAlwaysKeepsWaitState ==
  [] [RbcDeliveredPendingByzantineCommitVoteStepKeepsWaitState]_vars

RbcDeliveredPendingHonestCommitVoteAlwaysCompletesFinality ==
  [] [RbcDeliveredPendingHonestCommitVoteStepCompletesFinality]_vars

RbcDeliveredPendingByzantineCommitVoteAlwaysCompletesFinality ==
  [] [RbcDeliveredPendingByzantineCommitVoteStepCompletesFinality]_vars

RbcDeliveredPendingPrepareVoteAlwaysKeepsWaitState ==
  [] [RbcDeliveredPendingPrepareVoteStepKeepsWaitState]_vars

RbcDeliveredPendingPrepareVoteAlwaysStartsCommitVoteWaitState ==
  [] [RbcDeliveredPendingPrepareVoteStepStartsCommitVoteWaitState]_vars

RbcDeliveredPendingTimeoutAlwaysStartsNewViewWaitState ==
  [] [RbcDeliveredPendingTimeoutStepStartsNewViewWaitState]_vars

RbcDeliveredPendingNewViewVoteAlwaysKeepsWaitState ==
  [] [RbcDeliveredPendingNewViewVoteStepKeepsWaitState]_vars

RbcDeliveredPendingNewViewVoteAlwaysStartsProposalWaitState ==
  [] [RbcDeliveredPendingNewViewVoteStepStartsProposalWaitState]_vars

RbcDeliveredPendingHonestProposeAlwaysStartsPrepareWaitState ==
  [] [RbcDeliveredPendingHonestProposeStepStartsPrepareWaitState]_vars

RbcDeliveredPendingGstElapsedAlwaysKeepsWaitState ==
  [] [RbcDeliveredPendingGstElapsedStepKeepsWaitState]_vars

RbcDeliveredPendingNextAlwaysCoveredByHandoffs ==
  [] [RbcDeliveredPendingNextStepCoveredByHandoffs]_vars

RbcDeliveredPendingSpecStepAlwaysStuttersOrTakesCoveredHandoff ==
  [] [RbcDeliveredPendingSpecStepStuttersOrTakesCoveredHandoffStep]_vars

RbcDeliveredPendingSpecStepAlwaysEndsInFinalityOrWaitState ==
  [] [RbcDeliveredPendingSpecStepEndsInFinalityOrWaitStateStep]_vars

RbcDeliveredPendingSpecStepAlwaysPreservesDeliveredRbcEvidence ==
  [] [RbcDeliveredPendingSpecStepPreservesDeliveredRbcEvidenceStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesCommitArtifactsOutcome ==
  [] [RbcDeliveredPendingSpecStepCommitArtifactsMatchOutcomeStep]_vars

RbcDeliveredPendingSpecStepAlwaysChangesGstOnlyByElapsed ==
  [] [RbcDeliveredPendingSpecStepGstChangesOnlyByElapsedStep]_vars

RbcDeliveredPendingSpecStepAlwaysChangesViewOnlyByTimeout ==
  [] [RbcDeliveredPendingSpecStepViewChangesOnlyByTimeoutStep]_vars

RbcDeliveredPendingSpecStepAlwaysChangesViewEvidenceOnlyByNewViewOrTimeout ==
  [] [RbcDeliveredPendingSpecStepViewEvidenceChangesOnlyByNewViewOrTimeoutStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesVoteCounterHandoff ==
  [] [RbcDeliveredPendingSpecStepVoteCountersMatchHandoffStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesPostGateHandoff ==
  [] [RbcDeliveredPendingSpecStepPostGatesMatchHandoffStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesTimerGateHandoff ==
  [] [RbcDeliveredPendingSpecStepTimerGatesMatchHandoffStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesFinalitySource ==
  [] [RbcDeliveredPendingSpecStepFinalitySourceMatchesCommitVoteStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesFinalityWitnessFrame ==
  [] [RbcDeliveredPendingSpecStepFinalityWitnessFrameStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesFinalityStackOutcome ==
  [] [RbcDeliveredPendingSpecStepFinalityStackMatchesOutcomeStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesFinalityGateOutcome ==
  [] [RbcDeliveredPendingSpecStepFinalityGateOutcomeStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesFinalityQuorumOutcome ==
  [] [RbcDeliveredPendingSpecStepFinalityQuorumOutcomeStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesNonFinalHandoffPhaseShape ==
  [] [RbcDeliveredPendingSpecStepNonFinalHandoffPhaseShapeStep]_vars

RbcDeliveredPendingSpecStepAlwaysClosesActionSurface ==
  [] [RbcDeliveredPendingSpecStepActionSurfaceClosedStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesPhaseChangeAction ==
  [] [RbcDeliveredPendingSpecStepPhaseChangeMatchesActionStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesCounterChangeAction ==
  [] [RbcDeliveredPendingSpecStepCounterChangesMatchActionStep]_vars

RbcDeliveredPendingSpecStepAlwaysHasExclusiveActionSource ==
  [] [RbcDeliveredPendingSpecStepActionSourcesExclusiveStep]_vars

RbcDeliveredPendingSpecStepAlwaysPreservesActionSurfaceOnStutter ==
  [] [RbcDeliveredPendingSpecStepStutterPreservesActionSurfaceStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesCommitArtifactChangeSource ==
  [] [RbcDeliveredPendingSpecStepCommitArtifactChangeMatchesSourceStep]_vars

RbcDeliveredPendingSpecStepAlwaysInstallsCertifiedDeliveryOnCommitArtifactChange ==
  [] [RbcDeliveredPendingSpecStepCommitArtifactChangeInstallsCertifiedDeliveryStep]_vars

RbcDeliveredPendingSpecStepAlwaysInstallsExactSourceCertifiedDeliveryOnCommitArtifactChange ==
  [] [RbcDeliveredPendingSpecStepCommitArtifactChangeExactSourceCertifiedDeliveryStep]_vars

RbcDeliveredPendingSpecStepAlwaysKeepsNonFinalHandoffOnStableCommitArtifacts ==
  [] [RbcDeliveredPendingSpecStepStableCommitArtifactsStayNonFinalHandoffStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesNonFinalSourceOnStableCommitArtifacts ==
  [] [RbcDeliveredPendingSpecStepStableCommitArtifactsMatchNonFinalSourceStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesCounterFootprintOnStableCommitArtifacts ==
  [] [RbcDeliveredPendingSpecStepStableCommitArtifactsCounterFootprintStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesPhaseGateFootprintOnStableCommitArtifacts ==
  [] [RbcDeliveredPendingSpecStepStableCommitArtifactsPhaseGateFootprintStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesTimerFootprintOnStableCommitArtifacts ==
  [] [RbcDeliveredPendingSpecStepStableCommitArtifactsTimerFootprintStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesViewFootprintOnStableCommitArtifacts ==
  [] [RbcDeliveredPendingSpecStepStableCommitArtifactsViewFootprintStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesFinalityFootprintOnStableCommitArtifacts ==
  [] [RbcDeliveredPendingSpecStepStableCommitArtifactsFinalityFootprintStep]_vars

RbcDeliveredPendingSpecStepAlwaysMatchesRbcSurfaceOnStableCommitArtifacts ==
  [] [RbcDeliveredPendingSpecStepStableCommitArtifactsRbcSurfaceStep]_vars

RbcDeliveredPendingSpecStepAlwaysClosesCompleteWaitStateOnStableCommitArtifacts ==
  [] [RbcDeliveredPendingSpecStepStableCommitArtifactsCompleteWaitStateStep]_vars

DeliveredPendingCompleteWaitStateSpecStepAlwaysCloses ==
  [] [DeliveredPendingCompleteWaitStateSpecStepClosesStep]_vars

DeliveredPendingCompleteWaitStateCommitVoteStepAlwaysSplits ==
  [] [DeliveredPendingCompleteWaitStateCommitVoteStepSplitsStep]_vars

DeliveredPendingCompleteWaitStateCommitVoteStepAlwaysPreservesWaitState ==
  [] [DeliveredPendingCompleteWaitStateCommitVoteStepPreservesWaitStateStep]_vars

DeliveredPendingCompleteWaitStateCommitVoteStepAlwaysCompletesFinality ==
  [] [DeliveredPendingCompleteWaitStateCommitVoteStepCompletesFinalityStep]_vars

DeliveredPendingCompleteWaitStatePrepareVoteStepAlwaysSplits ==
  [] [DeliveredPendingCompleteWaitStatePrepareVoteStepSplitsStep]_vars

DeliveredPendingCompleteWaitStateTimeoutStepAlwaysStartsNewView ==
  [] [DeliveredPendingCompleteWaitStateTimeoutStepStartsNewViewStep]_vars

DeliveredPendingCompleteWaitStateNewViewVoteStepAlwaysSplits ==
  [] [DeliveredPendingCompleteWaitStateNewViewVoteStepSplitsStep]_vars

DeliveredPendingCompleteWaitStateHonestProposeStepAlwaysStartsPrepare ==
  [] [DeliveredPendingCompleteWaitStateHonestProposeStepStartsPrepareStep]_vars

DeliveredPendingCompleteWaitStateGstElapsedStepAlwaysKeepsWaitState ==
  [] [DeliveredPendingCompleteWaitStateGstElapsedStepKeepsWaitStateStep]_vars

DeliveredPendingCompleteWaitStateNextStepAlwaysMatchesNamedActionBranch ==
  [] [DeliveredPendingCompleteWaitStateNextStepMatchesNamedActionBranchStep]_vars

RbcDeliveryEntryOnlyByDeliver ==
  [] [RbcDeliveryEntryOnlyByDeliverStep]_vars

RbcDeliveryEntryAlwaysMatchesReadyQuorumExitAndCommitBranch ==
  [] [RbcDeliveryEntryMatchesReadyQuorumExitAndCommitBranchStep]_vars

RbcDeliveryEntryFinalityAlwaysCompletesCommittedDelivery ==
  [] [RbcDeliveryEntryFinalityCompletesCommittedDeliveryStep]_vars

RbcDeliveryEntryPendingAlwaysInstallsCompleteWaitState ==
  [] [RbcDeliveryEntryPendingInstallsCompleteWaitStateStep]_vars

RbcDeliveryEntryAlwaysCompletesFinalityOrWaitState ==
  [] [RbcDeliveryEntryCompletesFinalityOrWaitStateStep]_vars

RbcDeliveryEntryAlwaysMatchesCommitArtifactOutcome ==
  [] [RbcDeliveryEntryCommitArtifactsMatchOutcomeStep]_vars

RbcDeliveryEntryAlwaysMatchesPostGateSurfaceOutcome ==
  [] [RbcDeliveryEntryPostGateSurfaceMatchesOutcomeStep]_vars

RbcDeliveryEntryAlwaysMatchesConsensusFrameOutcome ==
  [] [RbcDeliveryEntryConsensusFrameMatchesOutcomeStep]_vars

RbcDeliveryEntryFinalityAlwaysMatchesCertifiedSourceStack ==
  [] [RbcDeliveryEntryFinalityMatchesCertifiedSourceStackStep]_vars

RbcDeliveryEntryFinalityAlwaysInstallsCommittedPostStateInvariants ==
  [] [RbcDeliveryEntryFinalityInstallsCommittedPostStateInvariantsStep]_vars

RbcDeliveryEntryFinalityAlwaysSplitsPostStateGate ==
  [] [RbcDeliveryEntryFinalityPostStateGateSplitStep]_vars

RbcDeliveryEntryFinalityPreGstPostStateOnlyLeavesGstElapsed ==
  [] [RbcDeliveryEntryFinalityPreGstPostStateLeavesOnlyGstElapsedStep]_vars

RbcDeliveryEntryFinalityPostGstPostStateIsTerminal ==
  [] [RbcDeliveryEntryFinalityPostGstPostStateIsTerminalStep]_vars

RbcDeliveryEntryPendingAlwaysMatchesNonFinalWaitSurface ==
  [] [RbcDeliveryEntryPendingMatchesNonFinalWaitSurfaceStep]_vars

RbcDeliveryEntryPendingAlwaysSplitsPostStateTimerGate ==
  [] [RbcDeliveryEntryPendingPostStateTimerGateSplitStep]_vars

RbcDeliveryEntryPendingPreGstPostStateAlwaysKeepsWaitTimers ==
  [] [RbcDeliveryEntryPendingPreGstPostStateKeepsWaitTimersStep]_vars

RbcDeliveryEntryPendingPostGstPostStateAlwaysTracksProgressTimeout ==
  [] [RbcDeliveryEntryPendingPostGstPostStateTimeoutTracksProgressStep]_vars

RbcDeliveryEntryPendingAlwaysInstallsDeliveredWaitPredicate ==
  [] [RbcDeliveryEntryPendingInstallsDeliveredWaitPredicateStep]_vars

RbcDeliveryEntryPendingAlwaysOpensDeliveredPendingContinuationSurface ==
  [] [RbcDeliveryEntryPendingOpensDeliveredPendingContinuationSurfaceStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysOpensExactContinuation ==
  [] [RbcDeliveryEntryCommitEvidenceBranchOpensExactContinuationStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExclusiveOutcome ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesExclusiveOutcomeStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExclusiveGateOutcome ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesExclusiveGateOutcomeStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExactConsensusFrame ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesExactConsensusFrameStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExactActionSource ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesExactActionSourceStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesCertifiedOrPendingStack ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesCertifiedOrPendingStackStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExactWitnessSurface ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesExactWitnessSurfaceStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesLiveCommitGateCrossing ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesLiveCommitGateCrossingStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesContinuationMode ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesContinuationModeStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesViewHandoffSurface ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesViewHandoffSurfaceStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesDeliveredEvidenceSurface ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesDeliveredEvidenceSurfaceStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesGstTimerSurface ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesGstTimerSurfaceStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesProgressActionSurface ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesProgressActionSurfaceStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesVoteBudgetSurface ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesVoteBudgetSurfaceStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesThresholdClassifier ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesThresholdClassifierStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesPendingCommitVoteProgressSplit ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesPendingCommitVoteProgressSplitStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesPendingNonCommitVoteProgressSplit ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesPendingNonCommitVoteProgressSplitStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesPendingProgressPartition ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesPendingProgressPartitionStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesPostStateClassifier ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesPostStateClassifierStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesCertificateProgressDisjointness ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesCertificateProgressDisjointnessStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesActionFamilyClassifier ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesActionFamilyClassifierStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesByzantineCommitVoteBoundary ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesByzantineCommitVoteBoundaryStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesResidualGatePartition ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesResidualGatePartitionStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesCompleteHandoff ==
  [] [RbcDeliveryEntryCommitEvidenceBranchMatchesCompleteHandoffStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsContinuationState ==
  [] [RbcDeliveryEntryCommitEvidenceBranchSeedsContinuationStateStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsPendingActionSurface ==
  [] [RbcDeliveryEntryCommitEvidenceBranchSeedsPendingActionSurfaceStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsPendingTimerSurface ==
  [] [RbcDeliveryEntryCommitEvidenceBranchSeedsPendingTimerSurfaceStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsPendingCounterFrame ==
  [] [RbcDeliveryEntryCommitEvidenceBranchSeedsPendingCounterFrameStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsPendingCompleteWaitState ==
  [] [RbcDeliveryEntryCommitEvidenceBranchSeedsPendingCompleteWaitStateStep]_vars

RbcDeliveryEntryCommitEvidenceBranchAlwaysHandsOffToDeliveredPendingWaitState ==
  [] [RbcDeliveryEntryCommitEvidenceBranchHandsOffToDeliveredPendingWaitStateStep]_vars

RbcProgressEvidenceNeverDiverges ==
  [] RbcProgressEvidenceMatchesState

RbcPartialProgressEvidenceNeverDiverges ==
  [] RbcPartialProgressEvidenceMatchesState

RbcCorruptedDigestNeverValid ==
  [] RbcCorruptedNeverHasValidDigest

RbcCorruptedAlwaysRetainsHeaderEvidence ==
  [] RbcCorruptedRetainsHeaderEvidence

RbcCorruptedNeverCarriesFinalityArtifacts ==
  [] RbcCorruptedHasNoFinalityArtifacts

RbcCorruptedNeverBypassesInitRepairProgress ==
  [] RbcCorruptedOnlyEnablesInitRepairProgress

RbcStateOnlyChangesByProtocolOrFault ==
  [] [RbcStateOnlyChangesByProtocolOrFaultStep]_vars

RbcStateChangeAlwaysMatchesLocalExitClassification ==
  [] [RbcStateChangeMatchesLocalExitClassificationStep]_vars

RbcEvidenceOnlyChangesByProtocolOrFault ==
  [] [RbcEvidenceOnlyChangesByProtocolOrFaultStep]_vars

RbcEvidenceChangeAlwaysMatchesLocalEffectClassification ==
  [] [RbcEvidenceChangeMatchesLocalEffectClassificationStep]_vars

RbcHeaderInstallationOnlyByProposalOrInit ==
  [] [RbcHeaderInstallationOnlyByProposalOrInitStep]_vars

RbcHeaderEvidenceNeverLost ==
  [] (headerSeen => [] headerSeen)

RbcMissingHeaderNeverLeavesIdle ==
  [] RbcMissingHeaderRequiresIdle

RbcHeaderEvidenceNeverReturnsToIdle ==
  [] RbcHeaderEvidenceRequiresNonIdle

RbcDigestInstallationOnlyByProposalInitOrChunk ==
  [] [RbcDigestInstallationOnlyByProposalInitOrChunkStep]_vars

RbcValidDigestNeverOutrunsHeader ==
  [] RbcValidDigestRequiresHeader

RbcValidDigestNeverLeavesActiveStates ==
  [] RbcValidDigestRequiresActiveState

RbcChunkEvidenceNeverOutrunsHeader ==
  [] RbcChunkEvidenceRequiresHeader

RbcChunkEvidenceNeverLeavesChunkOrCorruptedHandoff ==
  [] RbcChunkEvidenceRequiresChunkOrCorruptedState

RbcPartialChunkEvidenceNeverLeavesChunkingOrCorruptedHandoff ==
  [] RbcPartialChunkEvidenceRequiresChunkingOrCorruption

RbcFullChunkCoverageNeverLeavesCoveredOrCorruptedHandoff ==
  [] RbcFullChunkCoverageRequiresCoveredOrCorruptedState

RbcZeroChunkEvidenceNeverLeavesPreChunkOrCorruptedHandoff ==
  [] RbcZeroChunkEvidenceRequiresPreChunkOrCorruption

RbcReadyVotesNeverOutrunChunkHeaderEvidence ==
  [] RbcReadyVotesRequireChunkHeaderEvidence

RbcReadyVotesNeverLeaveReadyOrCorruptedHandoff ==
  [] RbcReadyVotesRequireReadyOrCorruptedState

RbcPartialReadyEvidenceNeverLeavesPartialOrCorruptedHandoff ==
  [] RbcPartialReadyEvidenceRequiresReadyPartialOrCorruption

RbcReadyQuorumEvidenceNeverLeavesQuorumOrCorruptedHandoff ==
  [] RbcReadyQuorumEvidenceRequiresQuorumOrCorruptedState

RbcZeroReadyEvidenceNeverLeavesPreReadyOrCorruptedHandoff ==
  [] RbcZeroReadyEvidenceRequiresPreReadyOrCorruption

RbcCounterEvidenceNeverOutrunsValidDigestOrCorruption ==
  [] RbcCounterEvidenceRequiresValidDigestOrCorruption

RbcInvalidDigestNeverLeavesIdleOrCorruption ==
  [] RbcInvalidDigestRequiresIdleOrCorruption

RbcWithheldNeverReached ==
  [] (rbcState # "Withheld")

RbcWithheldEntryOnlyByStutteringFromWithheld ==
  [] [RbcWithheldEntryOnlyByStutteringFromWithheldStep]_vars

ByzantineFaultGateNeverBypassesCorruptibleRbc ==
  [] ByzantineFaultGateMatchesCorruptibleRbc

ByzantineFaultStepAlwaysCorruptsOnlyRbcDigest ==
  [] [ByzantineFaultStepCorruptsOnlyRbcDigest]_vars

RbcDigestInvalidationOnlyByFault ==
  [] [RbcDigestInvalidationOnlyByFaultStep]_vars

RbcCorruptionEntryOnlyByFault ==
  [] [RbcCorruptionEntryOnlyByFaultStep]_vars

RbcInitGateNeverBypassesRepairableState ==
  [] RbcInitGateMatchesRepairableState

RbcInitStepAlwaysInstallsHeaderDigestEvidence ==
  [] [RbcInitStepInstallsHeaderDigestEvidence]_vars

RbcInitStepAlwaysStartsChunkOnlyHandoff ==
  [] [RbcInitStepStartsChunkOnlyHandoffStep]_vars

RbcIdleExitOnlyByProposalOrInit ==
  [] [RbcIdleExitOnlyByProposalOrInitStep]_vars

RbcInitEntryOnlyByProposalOrInit ==
  [] [RbcInitEntryOnlyByProposalOrInitStep]_vars

RbcCorruptionExitOnlyByInit ==
  [] [RbcCorruptionExitOnlyByInitStep]_vars

RbcCorruptedInitRepairAlwaysResetsEvidence ==
  [] [RbcCorruptedInitRepairResetsEvidenceStep]_vars

RbcChunkGateNeverBypassesHeaderDigestEvidence ==
  [] RbcChunkGateMatchesHeaderDigestEvidence

RbcChunkStepAlwaysAdvancesChunkEvidence ==
  [] [RbcChunkStepAdvancesChunkEvidence]_vars

RbcChunkStepAlwaysHandsOffByCoverage ==
  [] [RbcChunkStepHandoffMatchesCoverageStep]_vars

RbcInitExitOnlyByChunkOrFault ==
  [] [RbcInitExitOnlyByChunkOrFaultStep]_vars

RbcChunkCountIncreaseOnlyByChunk ==
  [] [RbcChunkCountIncreaseOnlyByChunkStep]_vars

RbcChunkCountDecreaseOnlyByProposalOrInit ==
  [] [RbcChunkCountDecreaseOnlyByProposalOrInitStep]_vars

RbcChunkingEntryOnlyByChunk ==
  [] [RbcChunkingEntryOnlyByChunkStep]_vars

RbcChunkingExitOnlyByChunkOrFault ==
  [] [RbcChunkingExitOnlyByChunkOrFaultStep]_vars

RbcChunkCompletionEntryOnlyByChunk ==
  [] [RbcChunkCompletionEntryOnlyByChunkStep]_vars

RbcChunksCompleteExitOnlyByReadyOrFault ==
  [] [RbcChunksCompleteExitOnlyByReadyOrFaultStep]_vars

RbcReadyGateNeverBypassesChunkEvidence ==
  [] RbcReadyGateMatchesChunkEvidence

RbcReadyStepAlwaysAdvancesReadyEvidence ==
  [] [RbcReadyStepAdvancesReadyEvidence]_vars

RbcReadyStepAlwaysHandsOffByQuorum ==
  [] [RbcReadyStepHandoffMatchesQuorumStep]_vars

RbcReadyQuorumStepAlwaysEnablesDeliverHandoff ==
  [] [RbcReadyQuorumStepEnablesDeliverHandoff]_vars

RbcReadyVotesIncreaseOnlyByReady ==
  [] [RbcReadyVotesIncreaseOnlyByReadyStep]_vars

RbcReadyVotesDecreaseOnlyByProposalOrInit ==
  [] [RbcReadyVotesDecreaseOnlyByProposalOrInitStep]_vars

RbcReadyPartialEntryOnlyByReady ==
  [] [RbcReadyPartialEntryOnlyByReadyStep]_vars

RbcReadyPartialExitOnlyByReadyOrFault ==
  [] [RbcReadyPartialExitOnlyByReadyOrFaultStep]_vars

RbcReadyQuorumEntryOnlyByReady ==
  [] [RbcReadyQuorumEntryOnlyByReadyStep]_vars

RbcReadyQuorumExitOnlyByDeliverOrFault ==
  [] [RbcReadyQuorumExitOnlyByDeliverOrFaultStep]_vars

RbcDeliverGateNeverBypassesCompleteEvidence ==
  [] RbcDeliverGateMatchesCompleteEvidence

RbcReadyQuorumNeverLacksDeliverGate ==
  [] RbcReadyQuorumEnablesDeliverGate

RbcDeliverStepAlwaysPreservesCompleteEvidence ==
  [] [RbcDeliverStepPreservesCompleteEvidence]_vars

RbcDeliverStepAlwaysHandsOffByCommitEvidence ==
  [] [RbcDeliverStepHandoffMatchesCommitEvidenceStep]_vars

RbcDeliverFinalityGateNeverBypassesBufferedCommitEvidence ==
  [] RbcDeliverFinalityGateMatchesBufferedCommitEvidence

RbcDeliverFinalityStepAlwaysInstallsCommitArtifacts ==
  [] [RbcDeliverFinalityStepInstallsCommitArtifacts]_vars

RbcDeliverFinalityStepAlwaysCompletesCommittedDelivery ==
  [] [RbcDeliverFinalityStepCompletesCommittedDelivery]_vars

RbcDeliverPendingGateNeverBypassesMissingBufferedCommitEvidence ==
  [] RbcDeliverPendingGateMatchesMissingBufferedCommitEvidence

RbcDeliverPendingStepNeverMutatesCommitArtifacts ==
  [] [RbcDeliverPendingStepPreservesPreFinalityArtifacts]_vars

RbcDeliverPendingStepAlwaysKeepsDeliveredEvidenceWithoutFinality ==
  [] [RbcDeliverPendingStepKeepsDeliveredEvidenceWithoutFinality]_vars

PendingProtocolStepsNeverChangeGst ==
  [] [PendingProtocolStepsPreserveGst]_vars

LiveHeaderDigestEvidenceNeverBypassRbcHandoff ==
  [] LiveHeaderDigestEvidenceStayInRbcHandoff

LiveChunkEvidenceNeverBypassRbcHandoff ==
  [] LiveChunkEvidenceStayInRbcHandoff

LiveReadyVotesNeverBypassRbcHandoff ==
  [] LiveReadyVotesStayInRbcHandoff

====
