---- MODULE SumeragiV2TimeoutLifecycleStageClassifierMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Finite-state regression for the neutral timeout-lifecycle clock classifier.

PersistInstallTC is live timeout work and must block a fresh service window.
Its ordinary proposal successors intentionally retain the immutable timeout
causal origin across crash/replay, but AssembleBody, FetchBody, and SignVote
are no longer timeout-lifecycle stages.  Classifying those successors by the
root phase creates a replenishment lasso: the new-view proposal itself keeps
the fresh window closed forever.

The fixed classifier uses the current physical candidate kind.  The mutation
uses causalOrigin.phase.  Static invariants also cover each physical carrier,
cross-context and future-view exclusion, and restart retention.  This bounded
matrix is regression evidence, not deductive proof of AsyncNetwork liveness.
***************************************************************************)

CONSTANT ClassifierMode

ClassifierModes == {"CurrentKind", "CausalOriginPhase"}

TimeoutLifecycleKinds ==
  {"BeginTimeout", "PersistTimeout", "SignTimeout",
   "DeliverTimeout", "FormTC", "DeliverTC",
   "BeginInstallTC", "PersistInstallTC"}

ProposalSuccessorKinds == {"AssembleBody", "FetchBody", "SignVote"}

CarrierKinds == {"Queued", "Deferred", "Causal", "Tracked"}

Stages ==
  {"PersistInstallTC", "AssembleBody", "FetchBody", "SignVote", "Decided"}

Candidate(kind, carrier, consumerContext, originContext, originView) ==
  [node |-> "Leader",
   kind |-> kind,
   carrier |-> carrier,
   consumerContext |-> consumerContext,
   causalOrigin |->
     [context |-> originContext,
      view |-> originView,
      phase |-> "PersistInstallTC"]]

OlderOrEqualTimeoutLifecycleCandidateOwned(
    candidate, currentContext, roundView) ==
  /\ candidate.node = "Leader"
  /\ candidate.consumerContext = currentContext
  /\ candidate.causalOrigin.context = currentContext
  /\ candidate.causalOrigin.view \in 0..roundView
  /\ IF ClassifierMode = "CurrentKind"
        THEN candidate.kind \in TimeoutLifecycleKinds
        ELSE candidate.causalOrigin.phase \in TimeoutLifecycleKinds

VARIABLES stage, replayed, freshWindowEntered, decided, lastTransition

vars == <<stage, replayed, freshWindowEntered, decided, lastTransition>>

CurrentCandidate == Candidate(stage, "Causal", "Ctx", "Ctx", 0)

Init ==
  /\ ClassifierMode \in ClassifierModes
  /\ stage = "PersistInstallTC"
  /\ replayed = FALSE
  /\ freshWindowEntered = FALSE
  /\ decided = FALSE
  /\ lastTransition = "Init"

FinishPersistInstallTC ==
  /\ stage = "PersistInstallTC"
  /\ stage' = "AssembleBody"
  /\ lastTransition' = "FinishPersistInstallTC"
  /\ UNCHANGED <<replayed, freshWindowEntered, decided>>

CrashAndReplayProposalSuccessor ==
  /\ stage = "AssembleBody"
  /\ ~replayed
  /\ replayed' = TRUE
  /\ lastTransition' = "CrashAndReplayProposalSuccessor"
  /\ UNCHANGED <<stage, freshWindowEntered, decided>>

EnterFreshSelfLeaderWindow ==
  /\ stage = "AssembleBody"
  /\ replayed
  /\ ~OlderOrEqualTimeoutLifecycleCandidateOwned(
       CurrentCandidate, "Ctx", 0)
  /\ stage' = "FetchBody"
  /\ freshWindowEntered' = TRUE
  /\ lastTransition' = "EnterFreshSelfLeaderWindow"
  /\ UNCHANGED <<replayed, decided>>

FetchProposalBody ==
  /\ stage = "FetchBody"
  /\ stage' = "SignVote"
  /\ lastTransition' = "FetchProposalBody"
  /\ UNCHANGED <<replayed, freshWindowEntered, decided>>

SignProposalVote ==
  /\ stage = "SignVote"
  /\ stage' = "Decided"
  /\ decided' = TRUE
  /\ lastTransition' = "SignProposalVote"
  /\ UNCHANGED <<replayed, freshWindowEntered>>

Next ==
  \/ FinishPersistInstallTC
  \/ CrashAndReplayProposalSuccessor
  \/ EnterFreshSelfLeaderWindow
  \/ FetchProposalBody
  \/ SignProposalVote

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(FinishPersistInstallTC)
  /\ WF_vars(CrashAndReplayProposalSuccessor)
  /\ WF_vars(EnterFreshSelfLeaderWindow)
  /\ WF_vars(FetchProposalBody)
  /\ WF_vars(SignProposalVote)

TypeInvariant ==
  /\ ClassifierMode \in ClassifierModes
  /\ stage \in Stages
  /\ replayed \in BOOLEAN
  /\ freshWindowEntered \in BOOLEAN
  /\ decided \in BOOLEAN
  /\ lastTransition
       \in {"Init", "FinishPersistInstallTC",
            "CrashAndReplayProposalSuccessor",
            "EnterFreshSelfLeaderWindow", "FetchProposalBody",
            "SignProposalVote"}

PersistInstallTcRemainsBlocking ==
  stage = "PersistInstallTC"
    => OlderOrEqualTimeoutLifecycleCandidateOwned(
         CurrentCandidate, "Ctx", 0)

ProposalSuccessorRetainedOriginIsNotTimeoutOwner ==
  stage \in ProposalSuccessorKinds
    => ~OlderOrEqualTimeoutLifecycleCandidateOwned(
          CurrentCandidate, "Ctx", 0)

LiveTimeoutStagesBlockAcrossAllCarriers ==
  \A kind \in TimeoutLifecycleKinds, carrier \in CarrierKinds:
    OlderOrEqualTimeoutLifecycleCandidateOwned(
      Candidate(kind, carrier, "Ctx", "Ctx", 0), "Ctx", 0)

CrossContextAndFutureViewCandidatesDoNotBlock ==
  /\ ~OlderOrEqualTimeoutLifecycleCandidateOwned(
       Candidate("PersistInstallTC", "Queued", "OtherCtx", "Ctx", 0),
       "Ctx", 0)
  /\ ~OlderOrEqualTimeoutLifecycleCandidateOwned(
       Candidate("PersistInstallTC", "Deferred", "Ctx", "OtherCtx", 0),
       "Ctx", 0)
  /\ ~OlderOrEqualTimeoutLifecycleCandidateOwned(
       Candidate("PersistInstallTC", "Tracked", "Ctx", "Ctx", 1),
       "Ctx", 0)

RestartReplayRetainsOriginButFollowsCurrentKind ==
  replayed
    => /\ CurrentCandidate.causalOrigin.phase = "PersistInstallTC"
       /\ (OlderOrEqualTimeoutLifecycleCandidateOwned(
             CurrentCandidate, "Ctx", 0)
              <=> stage \in TimeoutLifecycleKinds)

ProposalPipelineEventuallyDecides ==
  (stage = "PersistInstallTC") ~> decided

====
