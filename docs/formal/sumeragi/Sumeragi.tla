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
  /\ phase = "CommitVote"
  /\ commitVotesByz < F
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
  /\ ~gst
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

ViewEvidenceMatchesActiveView ==
  /\ (view = 0 => viewEvidenceVotes = 0)
  /\ (phase = "NewView" => viewEvidenceVotes = 0)
  /\ ((view > 0 /\ phase # "NewView") => viewEvidenceVotes >= ViewQuorum)

NewViewPhaseBelowQuorum ==
  phase = "NewView" => newViewVotes < ViewQuorum

ViewEvidenceIsCompleteOrEmpty ==
  viewEvidenceVotes = 0 \/ viewEvidenceVotes >= ViewQuorum

PreCommitPhasesHaveNoCommitVotes ==
  phase \in {"NewView", "Propose", "Prepare"} =>
    /\ commitVotesHonest = 0
    /\ commitVotesByz = 0
    /\ stakeSigned = 0

CommitImpliesViewQuorumEvidence ==
  committed => (commitView = 0 \/ viewEvidenceVotes >= ViewQuorum)

CommitVotePhaseRequiresPrepareQuorum ==
  phase \in {"CommitVote", "Committed"} => prepareVotes >= CommitQuorum

CommitImpliesPrepareQuorum ==
  committed => prepareVotes >= CommitQuorum

CommitEvidenceMatchesVoteCounters ==
  committed =>
    /\ commitEvidenceVotes = commitVotesHonest + commitVotesByz
    /\ commitEvidenceStake = stakeSigned

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

EventuallyCommit ==
  [] (gst => <> committed)

CommitNeverRevoked ==
  [] (committed => [] committed)

CommittedPhaseNeverLeaves ==
  [] (phase = "Committed" => [] (phase = "Committed"))

CommitViewNeverChanges ==
  [] (committed => [] (view = commitView))

CommitViewNeverLeadsCurrentView ==
  [] CommitViewDoesNotLeadCurrentView

ViewQuorumEvidenceNeverDiverges ==
  [] ViewEvidenceMatchesActiveView

NewViewQuorumHandoffNeverStalls ==
  [] NewViewPhaseBelowQuorum

ViewEvidenceNeverPartial ==
  [] ViewEvidenceIsCompleteOrEmpty

PreCommitVotesNeverCarryAcrossViews ==
  [] PreCommitPhasesHaveNoCommitVotes

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

CommitEvidenceNeverLost ==
  [] (committed =>
        [] (/\ commitEvidenceVotes >= CommitQuorum
            /\ commitEvidenceStake >= StakeQuorum))

RbcDeliveryNeverLost ==
  [] (rbcState = "Delivered" => [] (rbcState = "Delivered"))

RbcProgressEvidenceNeverDiverges ==
  [] RbcProgressEvidenceMatchesState

====
