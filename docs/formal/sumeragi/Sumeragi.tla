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
    /\ ~HonestNewViewVoteEnabled
    /\ ~RbcInitEnabled
    /\ ~RbcChunkGoodEnabled
    /\ ~RbcReadyGoodEnabled
    /\ ~RbcDeliverGoodEnabled
    /\ ~TimeoutTickEnabled
    /\ ~ByzantineFaultEnabled
    /\ ~PostGstProgressEnabled

CommittedPhaseMatchesFinality ==
  (phase = "Committed") <=> committed

CommitViewMatchesFinality ==
  committed => commitView = view

CommitViewDoesNotLeadCurrentView ==
  commitView <= view

GstElapsedGateMatchesPreGst ==
  GstElapsedEnabled <=> ~gst

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

ViewEvidenceMatchesActiveView ==
  /\ (view = 0 => viewEvidenceVotes = 0)
  /\ (phase = "NewView" => viewEvidenceVotes = 0)
  /\ ((view > 0 /\ phase # "NewView") => viewEvidenceVotes >= ViewQuorum)

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

RbcProgressEvidenceMatchesState ==
  /\ (rbcState \in RbcInitializedStates =>
        /\ headerSeen
        /\ digestValid)
  /\ (rbcState \in RbcChunkCoveredStates =>
        chunkCount >= MaxChunks)
  /\ (rbcState \in RbcReadyQuorumStates =>
        readyVotes >= CommitQuorum)

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

RbcDeliverGateMatchesCompleteEvidence ==
  RbcDeliverGoodEnabled <=>
    /\ rbcState = "ReadyQuorum"
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid

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

CommitViewNeverChanges ==
  [] (committed => [] (view = commitView))

CommitViewNeverLeadsCurrentView ==
  [] CommitViewDoesNotLeadCurrentView

GstElapsedGateNeverBypassesPreGst ==
  [] GstElapsedGateMatchesPreGst

GstElapsedStepAlwaysOnlySetsGst ==
  [] [GstElapsedStepOnlySetsGst]_vars

TimeoutTickGateNeverBypassesStalledProgress ==
  [] TimeoutTickGateMatchesStalledProgress

TimeoutTickStepAlwaysStartsFreshNewView ==
  [] [TimeoutTickStepStartsFreshNewView]_vars

ViewQuorumEvidenceNeverDiverges ==
  [] ViewEvidenceMatchesActiveView

NewViewQuorumHandoffNeverStalls ==
  [] NewViewPhaseBelowQuorum

LiveNewViewVotesNeverLeakPastHandoff ==
  [] LiveNewViewVotesStayInHandoff

HonestProposeGateNeverBypassesHandoffEvidence ==
  [] HonestProposeGateMatchesHandoffEvidence

HonestProposeStepAlwaysStartsPrepareAndRbc ==
  [] [HonestProposeStepStartsPrepareAndRbc]_vars

NewViewVoteGateNeverBypassesFreshViewEvidence ==
  [] NewViewVoteGateMatchesFreshViewEvidence

NewViewVoteQuorumGateNeverBypassesNextEvidence ==
  [] NewViewVoteQuorumGateMatchesNextEvidence

NewViewVoteQuorumStepAlwaysInstallsViewEvidence ==
  [] [NewViewVoteQuorumStepInstallsViewEvidence]_vars

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

PrepareVotePendingGateNeverBypassesMissingNextEvidence ==
  [] PrepareVotePendingGateMatchesMissingNextEvidence

PrepareVotePendingStepNeverMutatesCommitArtifacts ==
  [] [PrepareVotePendingStepPreservesPreCommitArtifacts]_vars

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

HonestCommitVotePendingGateNeverBypassesMissingNextEvidence ==
  [] HonestCommitVotePendingGateMatchesMissingNextEvidence

HonestCommitVotePendingStepNeverMutatesCommitArtifacts ==
  [] [HonestCommitVotePendingStepPreservesPreFinalityArtifacts]_vars

ByzantineCommitVoteFinalityGateNeverBypassesNextEvidence ==
  [] ByzantineCommitVoteFinalityGateMatchesNextEvidence

ByzantineCommitVoteFinalityStepAlwaysInstallsCommitArtifacts ==
  [] [ByzantineCommitVoteFinalityStepInstallsCommitArtifacts]_vars

ByzantineCommitVotePendingGateNeverBypassesMissingNextEvidence ==
  [] ByzantineCommitVotePendingGateMatchesMissingNextEvidence

ByzantineCommitVotePendingStepNeverMutatesCommitArtifacts ==
  [] [ByzantineCommitVotePendingStepPreservesPreFinalityArtifacts]_vars

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

RbcProgressEvidenceNeverDiverges ==
  [] RbcProgressEvidenceMatchesState

ByzantineFaultGateNeverBypassesCorruptibleRbc ==
  [] ByzantineFaultGateMatchesCorruptibleRbc

ByzantineFaultStepAlwaysCorruptsOnlyRbcDigest ==
  [] [ByzantineFaultStepCorruptsOnlyRbcDigest]_vars

RbcInitGateNeverBypassesRepairableState ==
  [] RbcInitGateMatchesRepairableState

RbcInitStepAlwaysInstallsHeaderDigestEvidence ==
  [] [RbcInitStepInstallsHeaderDigestEvidence]_vars

RbcChunkGateNeverBypassesHeaderDigestEvidence ==
  [] RbcChunkGateMatchesHeaderDigestEvidence

RbcChunkStepAlwaysAdvancesChunkEvidence ==
  [] [RbcChunkStepAdvancesChunkEvidence]_vars

RbcReadyGateNeverBypassesChunkEvidence ==
  [] RbcReadyGateMatchesChunkEvidence

RbcReadyStepAlwaysAdvancesReadyEvidence ==
  [] [RbcReadyStepAdvancesReadyEvidence]_vars

RbcDeliverGateNeverBypassesCompleteEvidence ==
  [] RbcDeliverGateMatchesCompleteEvidence

RbcDeliverFinalityGateNeverBypassesBufferedCommitEvidence ==
  [] RbcDeliverFinalityGateMatchesBufferedCommitEvidence

RbcDeliverFinalityStepAlwaysInstallsCommitArtifacts ==
  [] [RbcDeliverFinalityStepInstallsCommitArtifacts]_vars

RbcDeliverPendingGateNeverBypassesMissingBufferedCommitEvidence ==
  [] RbcDeliverPendingGateMatchesMissingBufferedCommitEvidence

RbcDeliverPendingStepNeverMutatesCommitArtifacts ==
  [] [RbcDeliverPendingStepPreservesPreFinalityArtifacts]_vars

LiveHeaderDigestEvidenceNeverBypassRbcHandoff ==
  [] LiveHeaderDigestEvidenceStayInRbcHandoff

LiveChunkEvidenceNeverBypassRbcHandoff ==
  [] LiveChunkEvidenceStayInRbcHandoff

LiveReadyVotesNeverBypassRbcHandoff ==
  [] LiveReadyVotesStayInRbcHandoff

====
