---- MODULE SumeragiV2TraceWitness ----
EXTENDS SumeragiV2

(***************************************************************************
Trace-generation entry point, not a proof module.

TLC is asked to violate NoDecision so tool mode emits one finite WitnessSpec
behavior through the first durable decision.  witnessAction records the exact
bound arguments of each chosen step; the tool-mode trace is then normalized
and replayed against the production Rust reducer.  These operators must never
be listed as safety invariants or proof obligations.

WitnessNext selects one representative production-replay schedule; it does
not constrain SumeragiV2Core or any proof specification.  Selection begins at
GST, avoids refetching an already durable exact body, chooses the view leader
as the single PrepareQC aggregator, and emits one PrepareQC/CommitQC projection
per round and subject.  A designated non-preparing validator is kept below a
local Prepare quorum until it has received the QC, and its validation is
delayed until observation begins, so the replay crosses the ObservePrepare WAL
class.
These Prepare-only scheduling constraints never restrict Commit DeliverVote:
the same Commit envelope may be ignored before a lock and admitted after an
intervening lock transition.  action # witnessAction merely prevents an
immediate marker-only repeat of the same projected action.
****************************************************************************)

VARIABLE witnessAction

WitnessVars == <<vars, witnessAction>>

WitnessNoNumber == -1
WitnessNoText == "-"

WitnessActionRecord(actionName, node, peer, roundView, phase, subject) ==
  [action |-> actionName, node |-> node, peer |-> peer,
   view |-> roundView, phase |-> phase, subject |-> subject]

WitnessInitialAction ==
  WitnessActionRecord("Initial", WitnessNoNumber, WitnessNoNumber,
                      WitnessNoNumber, WitnessNoText, WitnessNoText)

WitnessMark(actionName, node, peer, roundView, phase, subject, step) ==
  LET action ==
        WitnessActionRecord(actionName, node, peer, roundView, phase, subject)
  IN /\ (actionName = "SetGST" \/ gst)
     /\ action # witnessAction
     /\ step
     /\ witnessAction' = action

WitnessInit ==
  /\ Init
  /\ witnessAction = WitnessInitialAction

WitnessHeartbeatSubject == CHOOSE subject \in ValidSubjects: TRUE

WitnessProposalSubject(node) ==
  IF lockRank[node] = NoRank THEN WitnessHeartbeatSubject ELSE lockSubject[node]

WitnessBeginTimeout(node) ==
  /\ Leader(context, nodeView[node]) \notin Responsive
  /\ BeginTimeout(node)

WitnessFetchBody(node, proposal) ==
  /\ ~BodyHeldBy(durableBodies, node, context, proposal.view,
                  proposal.subject)
  /\ FetchBody(node, proposal)

WitnessRebindRetainedBody(node, proposal) ==
  /\ ~BodyHeldBy(durableBodies, node, context, proposal.view,
                  proposal.subject)
  /\ RebindRetainedBody(node, proposal)

WitnessFormPrepareQC(node, roundView, subject) ==
  /\ node = Leader(context, roundView)
  /\ ~\E qc \in prepareQCs:
       /\ qc.context = context
       /\ qc.view = roundView
       /\ qc.phase = "Prepare"
       /\ qc.subject = subject
  /\ FormPrepareQC(node, roundView, subject)

WitnessBeginObservePrepare(node, qc) ==
  /\ ~DualQuorum(CurrentEpoch,
                  VoteSignersAt(node, qc.view, qc.phase, qc.subject))
  /\ BeginObservePrepare(node, qc)

WitnessDeliverVote(envelope) ==
  LET node == envelope.recipient
      vote == envelope.vote
      signers == VoteSignersAt(node, vote.view, vote.phase, vote.subject)
      localPrepared ==
        \E intent \in prepareIntents:
          /\ intent.context = context
          /\ intent.view = vote.view
          /\ intent.phase = "Prepare"
          /\ intent.subject = vote.subject
          /\ intent.signer = node
  IN /\ \/ vote.phase # "Prepare"
         \/ localPrepared
         \/ highestRank[node] >= vote.view
         \/ ~DualQuorum(CurrentEpoch, signers \cup {vote.signer})
     /\ DeliverVote(envelope)

WitnessValidateBody(node, proposal) ==
  LET observing ==
        \E request \in pendingObservePrepare:
          /\ request.node = node
          /\ request.qc.context = context
          /\ request.qc.view = proposal.view
          /\ request.qc.subject = proposal.subject
  IN /\ \/ node # Leader(context, 0)
         \/ observing
         \/ highestRank[node] >= proposal.view
     /\ ValidateBody(node, proposal)

WitnessFormCommitQC(node, roundView, subject) ==
  /\ ~\E qc \in commitQCs:
       /\ qc.context = context
       /\ qc.view = roundView
       /\ qc.phase = "Commit"
       /\ qc.subject = subject
  /\ FormCommitQC(node, roundView, subject)

WitnessNext ==
  \/ WitnessMark("SetGST", WitnessNoNumber, WitnessNoNumber, WitnessNoNumber,
                 WitnessNoText, WitnessNoText, SetGST)
  \/ \E node \in ValidatorIds:
       LET subject == WitnessProposalSubject(node)
       IN WitnessMark("AssembleLocalBody", node, WitnessNoNumber,
                      WitnessNoNumber, WitnessNoText, subject,
                      AssembleLocalBody(node, subject))
  \/ \E node \in ValidatorIds:
       LET subject == WitnessProposalSubject(node)
       IN WitnessMark("BeginLocalProposal", node, WitnessNoNumber,
                      nodeView[node], WitnessNoText, subject,
                      BeginLocalProposal(node, subject))
  \/ \E request \in pendingProposal:
       WitnessMark("PersistProposal", request.node, WitnessNoNumber,
                   request.proposal.view, WitnessNoText,
                   request.proposal.subject, PersistProposal(request))
  \/ \E request \in signProposals:
       WitnessMark("CompleteProposalSignature", request.node, WitnessNoNumber,
                   request.proposal.view, WitnessNoText,
                   request.proposal.subject,
                   CompleteProposalSignature(request))
  \/ \E envelope \in proposalNetwork:
       WitnessMark("DeliverProposal", envelope.recipient,
                   envelope.proposal.proposer, envelope.proposal.view,
                   WitnessNoText, envelope.proposal.subject,
                   DeliverProposal(envelope))
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       \/ WitnessMark("FetchBody", node, WitnessNoNumber, WitnessNoNumber,
                      WitnessNoText, proposal.subject,
                      WitnessFetchBody(node, proposal))
       \/ WitnessMark("FetchBody", node, WitnessNoNumber, WitnessNoNumber,
                      WitnessNoText, proposal.subject,
                      WitnessRebindRetainedBody(node, proposal))
  \/ \E node \in ValidatorIds, roundView \in Views, subject \in Subjects:
       WitnessMark("StoreBody", node, WitnessNoNumber, WitnessNoNumber,
                   WitnessNoText, subject, StoreBody(node, roundView, subject))
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       WitnessMark("ValidateBody", node, WitnessNoNumber, proposal.view,
                   WitnessNoText, proposal.subject,
                   WitnessValidateBody(node, proposal))
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       WitnessMark("BeginPrepare", node, WitnessNoNumber, proposal.view, "Prepare",
                   proposal.subject, BeginPrepare(node, proposal))
  \/ \E request \in pendingPrepare:
       WitnessMark("PersistPrepare", request.node, WitnessNoNumber,
                   request.vote.view, request.vote.phase, request.vote.subject,
                   PersistPrepare(request))
  \/ \E request \in signVotes:
       WitnessMark("CompleteVoteSignature", request.node, WitnessNoNumber,
                   request.vote.view, request.vote.phase, request.vote.subject,
                   CompleteVoteSignature(request))
  \/ \E envelope \in voteNetwork:
       WitnessMark("DeliverVote", envelope.recipient, envelope.vote.signer,
                   envelope.vote.view, envelope.vote.phase,
                   envelope.vote.subject, WitnessDeliverVote(envelope))
  \/ \E node \in ValidatorIds, roundView \in Views,
       subject \in Subjects:
       WitnessMark("FormPrepareQC", node, WitnessNoNumber, roundView, "Prepare",
                   subject, WitnessFormPrepareQC(node, roundView, subject))
  \/ \E envelope \in qcNetwork:
       WitnessMark("DeliverQC", envelope.recipient, WitnessNoNumber,
                   envelope.qc.view, envelope.qc.phase, envelope.qc.subject,
                   DeliverQC(envelope))
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       WitnessMark("BeginObservePrepare", node, WitnessNoNumber, qc.view, qc.phase,
                   qc.subject, WitnessBeginObservePrepare(node, qc))
  \/ \E request \in pendingObservePrepare:
       WitnessMark("PersistObservePrepare", request.node, WitnessNoNumber,
                   request.qc.view, request.qc.phase, request.qc.subject,
                   PersistObservePrepare(request))
  \/ \E node \in ValidatorIds, qc \in LockCommitQcValues:
       WitnessMark("BeginLockCommit", node, WitnessNoNumber, qc.view, qc.phase,
                   qc.subject, BeginLockCommit(node, qc))
  \/ \E request \in pendingLockCommit:
       WitnessMark("PersistLockCommit", request.node, WitnessNoNumber,
                   request.qc.view, request.qc.phase, request.qc.subject,
                   PersistLockCommit(request))
  \/ \E node \in ValidatorIds, roundView \in Views,
       subject \in Subjects:
       WitnessMark("FormCommitQC", node, WitnessNoNumber, roundView, "Commit",
                   subject, WitnessFormCommitQC(node, roundView, subject))
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       WitnessMark("BeginDecision", node, WitnessNoNumber, qc.view, qc.phase,
                   qc.subject, BeginDecision(node, qc))
  \/ \E request \in pendingDecision:
       WitnessMark("PersistDecision", request.node, WitnessNoNumber,
                   request.qc.view, request.qc.phase, request.qc.subject,
                   PersistDecision(request))
  \/ \E node \in ValidatorIds:
       WitnessMark("BeginTimeout", node, WitnessNoNumber, nodeView[node],
                   WitnessNoText, WitnessNoText, WitnessBeginTimeout(node))
  \/ \E request \in pendingTimeout:
       WitnessMark("PersistTimeout", request.node, WitnessNoNumber,
                   request.vote.view, WitnessNoText, WitnessNoText,
                   PersistTimeout(request))
  \/ \E request \in signTimeouts:
       WitnessMark("CompleteTimeoutSignature", request.node, WitnessNoNumber,
                   request.vote.view, WitnessNoText, WitnessNoText,
                   CompleteTimeoutSignature(request))
  \/ \E envelope \in timeoutNetwork:
       WitnessMark("DeliverTimeout", envelope.recipient, envelope.vote.signer,
                   envelope.vote.view, WitnessNoText, WitnessNoText,
                   DeliverTimeout(envelope))
  \/ \E node \in ValidatorIds, roundView \in Views:
       WitnessMark("FormTC", node, WitnessNoNumber, roundView, WitnessNoText,
                   WitnessNoText, FormTC(node, roundView))
  \/ \E envelope \in tcNetwork:
       WitnessMark("DeliverTC", envelope.recipient, WitnessNoNumber,
                   envelope.tc.view, WitnessNoText, WitnessNoText,
                   DeliverTC(envelope))
  \/ \E node \in ValidatorIds, tc \in ReceivedTcValues:
       WitnessMark("BeginInstallTC", node, WitnessNoNumber, tc.view,
                   WitnessNoText, WitnessNoText, BeginInstallTC(node, tc))
  \/ \E request \in pendingInstallTC:
       WitnessMark("PersistInstallTC", request.node, WitnessNoNumber,
                   request.tc.view, WitnessNoText, WitnessNoText,
                   PersistInstallTC(request))
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       WitnessMark("FetchCertifiedBody", node, WitnessNoNumber, qc.view, qc.phase,
                   qc.subject, FetchCertifiedBody(node, qc))
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       WitnessMark("ApplyDecision", node, WitnessNoNumber, qc.view, qc.phase,
                   qc.subject, ApplyDecision(node, qc))

WitnessNextV2 ==
  WitnessNext
  \/ \E subject \in Subjects:
       WitnessMark("AdvanceContext", WitnessNoNumber, WitnessNoNumber,
                   WitnessNoNumber, WitnessNoText, subject,
                   AdvanceContext(subject))

WitnessActionFairness == WF_WitnessVars(WitnessNextV2)

WitnessSpec ==
  WitnessInit /\ [][WitnessNextV2]_WitnessVars /\ WitnessActionFairness

NoDecision == decisions = {}

=============================================================================
