---- MODULE SumeragiV2TraceWitness ----
EXTENDS SumeragiV2

(***************************************************************************
Trace-generation entry point, not a proof module.

TLC is asked to violate NoDecision so `-dumpTrace json` emits one finite
WitnessSpec behavior through the first durable decision.  The resulting JSON
is normalized and replayed against the production Rust reducer.  These
operators must never be listed as safety invariants or proof obligations.
***************************************************************************)

WitnessHeartbeatSubject == CHOOSE subject \in ValidSubjects: TRUE

WitnessProposalSubject(node) ==
  IF lockRank[node] = NoRank THEN WitnessHeartbeatSubject ELSE lockSubject[node]

WitnessBeginTimeout(node) ==
  /\ Leader(context, nodeView[node]) \notin Responsive
  /\ BeginTimeout(node)

WitnessNext ==
  \/ SetGST
  \/ \E node \in ValidatorIds:
       AssembleLocalBody(node, WitnessProposalSubject(node))
  \/ \E node \in ValidatorIds:
       BeginLocalProposal(node, WitnessProposalSubject(node))
  \/ \E request \in pendingProposal: PersistProposal(request)
  \/ \E request \in signProposals: CompleteProposalSignature(request)
  \/ \E envelope \in proposalNetwork: DeliverProposal(envelope)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       FetchBody(node, proposal)
  \/ \E node \in ValidatorIds, subject \in Subjects: StoreBody(node, subject)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       ValidateBody(node, proposal)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       BeginPrepare(node, proposal)
  \/ \E request \in pendingPrepare: PersistPrepare(request)
  \/ \E request \in signVotes: CompleteVoteSignature(request)
  \/ \E envelope \in voteNetwork: DeliverVote(envelope)
  \/ \E node \in ValidatorIds, roundView \in Views,
       subject \in Subjects: FormPrepareQC(node, roundView, subject)
  \/ \E envelope \in qcNetwork: DeliverQC(envelope)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginObservePrepare(node, qc)
  \/ \E request \in pendingObservePrepare: PersistObservePrepare(request)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginLockCommit(node, qc)
  \/ \E request \in pendingLockCommit: PersistLockCommit(request)
  \/ \E node \in ValidatorIds, roundView \in Views,
       subject \in Subjects: FormCommitQC(node, roundView, subject)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginDecision(node, qc)
  \/ \E request \in pendingDecision: PersistDecision(request)
  \/ \E node \in ValidatorIds: WitnessBeginTimeout(node)
  \/ \E request \in pendingTimeout: PersistTimeout(request)
  \/ \E request \in signTimeouts: CompleteTimeoutSignature(request)
  \/ \E envelope \in timeoutNetwork: DeliverTimeout(envelope)
  \/ \E node \in ValidatorIds, roundView \in Views: FormTC(node, roundView)
  \/ \E envelope \in tcNetwork: DeliverTC(envelope)
  \/ \E node \in ValidatorIds, tc \in ReceivedTcValues:
       BeginInstallTC(node, tc)
  \/ \E request \in pendingInstallTC: PersistInstallTC(request)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       FetchCertifiedBody(node, qc)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       ApplyDecision(node, qc)

WitnessNextV2 ==
  WitnessNext \/ \E subject \in Subjects: AdvanceContext(subject)

WitnessActionFairness == WF_vars(WitnessNextV2)

WitnessSpec == Init /\ [][WitnessNextV2]_vars /\ WitnessActionFairness

NoDecision == decisions = {}

=============================================================================
