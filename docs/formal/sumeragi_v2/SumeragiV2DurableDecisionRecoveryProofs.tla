---- MODULE SumeragiV2DurableDecisionRecoveryProofs ----
EXTENDS SumeragiV2CertifiedRequestHashAuthorityProofs

(***************************************************************************
Exact durable-Decision recovery lifecycle proof.

This module is deliberately narrower than the generic all-stage Decision
progress witness.  Its only authority is an unapplied durable Decision whose
certificate phase is Commit.  PrepareQC/effective-lock acquisition and the
generic `DecisionRecoveryStage` disjunction are excluded.  The proof splits:

  * the reachable durable/pending Decision frontier;
  * generation-free logical certified-request registration;
  * the crash -> authenticated restart -> replay handoff; and
  * the exact current-generation singleton FetchBody replay update.

The certified-request hash-authority module supplies the exact raw request/hash and full
scheduler-candidate identities.  It was independently SANY/TLAPS checked
before being imported here.
***************************************************************************)

DecisionsUniqueByNodeContext ==
  \A left, right \in decisions:
    /\ left.node = right.node
    /\ left.qc.context = right.qc.context
    => left = right

PendingDecisionExcludesDurableDecision ==
  \A request \in pendingDecision:
    ~\E decision \in decisions:
       /\ decision.node = request.node
       /\ decision.qc.context = request.qc.context

DecisionFrontierUniquenessInvariant ==
  /\ DecisionsUniqueByNodeContext
  /\ PendingDecisionExcludesDurableDecision

(***************************************************************************
The logical registration is requester/height/view/subject only.  Recipient
fan-out, transport nonce, and consumer generation are intentionally absent.
***************************************************************************)

DecisionCertifiedRequestIdentity(request) ==
  [source |-> request.source,
   height |-> request.envelope.height,
   view |-> request.envelope.view,
   subject |-> request.envelope.subject]

DecisionCertifiedRequestIdentityFor(node, qc) ==
  [source |-> node,
   height |-> qc.context.height,
   view |-> qc.view,
   subject |-> qc.subject]

DecisionCertifiedRequestRegistered(node, qc) ==
  DecisionCertifiedRequestIdentityFor(node, qc)
    \in {DecisionCertifiedRequestIdentity(request):
          request \in {active \in asyncActiveRequests:
                         active.kind = "CertifiedRequest"}}

(***************************************************************************
Recovery authority is durable and generation-free.  The executor epoch is a
separate process-local fact.  RestartDecisions is production's unapplied,
current-context, Commit-only durable source.
***************************************************************************)

DurableDecisionRecoveryAuthority(node, qc) ==
  /\ asyncRecoveryPhase \in {"RestartRequired", "ReplayRequired"}
  /\ asyncRecoveryNode = node
  /\ [node |-> node, qc |-> qc] \in RestartDecisions(node)

DurableDecisionRecoveryExecutorCurrent(node) ==
  /\ asyncRecoveryNode = node
  /\ generation[node] = asyncRecoveryGeneration

ExactCurrentDecisionFetchUpdate(node, qc) ==
  asyncCausalQueues' =
    [asyncCausalQueues EXCEPT
       ![node] = <<DecisionFetchCandidateAt(
                     node, qc, nodeView[node], generation[node])>>]

DurableDecisionRecoveryLifecycleTransition ==
  /\ \A node, qc:
       /\ [node |-> node, qc |-> qc] \in RestartDecisions(node)
       /\ PreGstResponsiveCrash(node)
       => /\ DurableDecisionRecoveryAuthority(node, qc)'
          /\ DurableDecisionRecoveryExecutorCurrent(node)'
          /\ (DecisionRawHashRegistered(node, qc)
                <=> DecisionRawHashRegistered(node, qc)')
          /\ (DecisionCertifiedRequestRegistered(node, qc)
                <=> DecisionCertifiedRequestRegistered(node, qc)')
  /\ \A node, qc:
       /\ DurableDecisionRecoveryAuthority(node, qc)
       /\ PreGstResponsiveRestart
       => /\ generation'[node] = generation[node] + 1
          /\ DurableDecisionRecoveryAuthority(node, qc)'
          /\ DurableDecisionRecoveryExecutorCurrent(node)'
          /\ (DecisionRawHashRegistered(node, qc)
                <=> DecisionRawHashRegistered(node, qc)')
          /\ (DecisionCertifiedRequestRegistered(node, qc)
                <=> DecisionCertifiedRequestRegistered(node, qc)')
  /\ \A node, qc:
       /\ StrongInductiveInvariant
       /\ DecisionsUniqueByNodeContext
       /\ DurableDecisionRecoveryAuthority(node, qc)
       /\ asyncRecoveryPhase = "ReplayRequired"
       /\ PreGstResponsiveReplay
       => /\ ~DurableDecisionRecoveryAuthority(node, qc)'
          /\ ~DecisionRawHashRegistered(node, qc)'
          /\ ~DecisionCertifiedRequestRegistered(node, qc)'
          /\ ExactCurrentDecisionFetchUpdate(node, qc)

DecisionRecoveryAcrossRestartProperty(specification) ==
  /\ specification => []DecisionFrontierUniquenessInvariant
  /\ specification
       => [][DurableDecisionRecoveryLifecycleTransition]_AsyncAllVars

(***************************************************************************
Pure scope and identity facts.
***************************************************************************)

THEOREM DurableDecisionAuthorityIsCommitOnly ==
  \A node, qc:
    DurableDecisionRecoveryAuthority(node, qc)
      => /\ qc.phase = "Commit"
         /\ [node |-> node, qc |-> qc] \in decisions
BY SMT
   DEF DurableDecisionRecoveryAuthority, RestartDecisions

THEOREM PrepareCertificateCannotAuthorizeDurableDecisionRecovery ==
  \A node, qc:
    qc.phase = "Prepare"
      => ~DurableDecisionRecoveryAuthority(node, qc)
BY DurableDecisionAuthorityIsCommitOnly, SMT

THEOREM DecisionRegistrationIdentityHasExactGenerationFreeShape ==
  \A node, qc:
    DecisionCertifiedRequestIdentityFor(node, qc)
      = [source |-> node,
         height |-> qc.context.height,
         view |-> qc.view,
         subject |-> qc.subject]
BY DEF DecisionCertifiedRequestIdentityFor

THEOREM DecisionOutboxOccurrencesShareOneRegistrationIdentity ==
  \A node, qc:
    DecisionCommitAuthority(node, qc)
      => \A request \in DecisionRequestOccurrences(node, qc):
           DecisionCertifiedRequestIdentity(request)
             = DecisionCertifiedRequestIdentityFor(node, qc)
BY DecisionOutboxHasOneLogicalRegistration, SMT
   DEF DecisionCertifiedRequestIdentity,
       DecisionCertifiedRequestIdentityFor,
       CertifiedRequestLogicalIdentity, DecisionLogicalRequestIdentity

(***************************************************************************
Reachable durable/pending Decision frontier.
***************************************************************************)

THEOREM AsyncInitEstablishesDecisionFrontierUniqueness ==
  \A initialContext:
    AsyncInitAt(initialContext) => DecisionFrontierUniquenessInvariant
BY SMT
   DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
       DecisionFrontierUniquenessInvariant,
       DecisionsUniqueByNodeContext,
       PendingDecisionExcludesDurableDecision, NoDecisionForNode

THEOREM DecisionFrontierStutterPreservesInvariant ==
  /\ DecisionFrontierUniquenessInvariant
  /\ UNCHANGED <<context, decisions, pendingDecision>>
  => DecisionFrontierUniquenessInvariant'
BY SMT
   DEF DecisionFrontierUniquenessInvariant,
       DecisionsUniqueByNodeContext,
       PendingDecisionExcludesDurableDecision, NoDecisionForNode

THEOREM AddCurrentPendingDecisionPreservesFrontierUniqueness ==
  \A request:
    /\ DecisionFrontierUniquenessInvariant
    /\ decisions' = decisions
    /\ pendingDecision' = pendingDecision \cup {request}
    /\ request.qc.context = context
    /\ NoDecisionForNode(request.node)
    => DecisionFrontierUniquenessInvariant'
BY SMT
   DEF DecisionFrontierUniquenessInvariant,
       DecisionsUniqueByNodeContext,
       PendingDecisionExcludesDurableDecision, NoDecisionForNode

THEOREM FormCommitQcPreservesDecisionFrontierUniqueness ==
  \A node, roundView, subject:
    /\ DecisionFrontierUniquenessInvariant
    /\ FormCommitQC(node, roundView, subject)
    => DecisionFrontierUniquenessInvariant'
BY AddCurrentPendingDecisionPreservesFrontierUniqueness, SMT
   DEF FormCommitQC, DecisionWal, QC, NoDecisionForNode

THEOREM BeginDecisionPreservesDecisionFrontierUniqueness ==
  \A node, qc:
    /\ DecisionFrontierUniquenessInvariant
    /\ BeginDecision(node, qc)
    => DecisionFrontierUniquenessInvariant'
BY AddCurrentPendingDecisionPreservesFrontierUniqueness, SMT
   DEF BeginDecision, DecisionWal, NoDecisionForNode

THEOREM StrongInvariantMakesPendingDecisionsNodeUnique ==
  StrongInductiveInvariant => RequestsUniqueByNode(pendingDecision)
BY SMT
   DEF StrongInductiveInvariant, Safety,
       OnePendingPersistencePerNode, RequestsUniqueByNode,
       AllPendingRequests

THEOREM PersistPendingDecisionSetUpdatePreservesFrontierUniqueness ==
  \A request:
    /\ DecisionFrontierUniquenessInvariant
    /\ RequestsUniqueByNode(pendingDecision)
    /\ request \in pendingDecision
    /\ decisions' =
         decisions \cup {[node |-> request.node, qc |-> request.qc]}
    /\ pendingDecision' = pendingDecision \ {request}
    => DecisionFrontierUniquenessInvariant'
BY SMT
   DEF DecisionFrontierUniquenessInvariant,
       DecisionsUniqueByNodeContext,
       PendingDecisionExcludesDurableDecision,
       RequestsUniqueByNode

THEOREM PersistDecisionPreservesDecisionFrontierUniqueness ==
  \A request:
    /\ StrongInductiveInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ PersistDecision(request)
    => DecisionFrontierUniquenessInvariant'
BY StrongInvariantMakesPendingDecisionsNodeUnique,
   PersistPendingDecisionSetUpdatePreservesFrontierUniqueness, SMT
   DEF PersistDecision

THEOREM CrashPreservesDecisionFrontierUniqueness ==
  \A node:
    /\ DecisionFrontierUniquenessInvariant
    /\ Crash(node)
    => DecisionFrontierUniquenessInvariant'
BY SMTT(120), IsaT(120)
   DEF DecisionFrontierUniquenessInvariant,
       DecisionsUniqueByNodeContext,
       PendingDecisionExcludesDurableDecision,
       NoDecisionForNode, Crash

(***************************************************************************
Only FormCommitQC, BeginDecision, PersistDecision, and Crash can change the
durable/pending Decision frontier.  Every other one-height Core action frames
the three variables on which the invariant depends.
***************************************************************************)

DurableDecisionFrontierStutteringStep ==
  \/ SetGST
  \/ \E node \in ValidatorIds, subject \in Subjects:
       AssembleLocalBody(node, subject)
  \/ \E node \in ValidatorIds, subject \in Subjects:
       BeginLocalProposal(node, subject)
  \/ \E request \in pendingProposal: PersistProposal(request)
  \/ \E request \in signProposals: CompleteProposalSignature(request)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       subject \in Subjects, justifyRank \in Ranks,
       justifySubject \in SubjectOrNone:
       ByzantineBroadcastProposal(signer, roundView, subject,
                                  justifyRank, justifySubject)
  \/ \E envelope \in proposalNetwork: DeliverProposal(envelope)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       FetchBody(node, proposal) \/ RebindRetainedBody(node, proposal)
  \/ \E node \in ValidatorIds, roundView \in Views, subject \in Subjects:
       StoreBody(node, roundView, subject)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       ValidateBody(node, proposal) \/ RejectBody(node, proposal)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       ValidateDecidedBody(node, qc)
  \/ \E node \in ValidatorIds, proposal \in SeenProposalValues:
       BeginPrepare(node, proposal)
  \/ \E request \in pendingPrepare: PersistPrepare(request)
  \/ \E request \in signVotes: CompleteVoteSignature(request)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       phase \in Phases, subject \in Subjects:
       ByzantineBroadcastVote(signer, roundView, phase, subject)
  \/ \E envelope \in voteNetwork: DeliverVote(envelope)
  \/ \E node \in ValidatorIds, roundView \in Views,
       subject \in Subjects: FormPrepareQC(node, roundView, subject)
  \/ \E envelope \in QcEnvelopeSet:
       ImportAuthenticatedCommitCertificate(envelope)
  \/ \E envelope \in qcNetwork: DeliverQC(envelope)
  \/ \E node \in ValidatorIds, qc \in ReceivedQcValues:
       BeginObservePrepare(node, qc)
  \/ \E request \in pendingObservePrepare: PersistObservePrepare(request)
  \/ \E node \in ValidatorIds, qc \in LockCommitQcValues:
       BeginLockCommit(node, qc)
  \/ \E request \in pendingLockCommit: PersistLockCommit(request)
  \/ \E node \in ValidatorIds: BeginTimeout(node)
  \/ \E request \in pendingTimeout: PersistTimeout(request)
  \/ \E request \in signTimeouts: CompleteTimeoutSignature(request)
  \/ \E signer \in ValidatorIds, roundView \in Views,
       highRank \in Ranks, highSubject \in SubjectOrNone:
       ByzantineBroadcastTimeout(signer, roundView, highRank, highSubject)
  \/ \E envelope \in timeoutNetwork: DeliverTimeout(envelope)
  \/ \E node \in ValidatorIds, roundView \in Views:
       FormTC(node, roundView)
  \/ \E envelope \in tcNetwork: DeliverTC(envelope)
  \/ \E node \in ValidatorIds, tc \in ReceivedTcValues:
       BeginInstallTC(node, tc)
  \/ \E request \in pendingInstallTC: PersistInstallTC(request)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       FetchCertifiedBody(node, qc)
  \/ \E node \in ValidatorIds, qc \in DecisionQcValues:
       ApplyDecision(node, qc)
  \/ \E node \in ValidatorIds: Restart(node)
  \/ \E node \in ValidatorIds, proposal \in proposalIntents:
       ResumeProposal(node, proposal)
  \/ \E node \in ValidatorIds,
       vote \in prepareIntents \cup commitIntents:
       ResumeVote(node, vote)
  \/ \E node \in ValidatorIds, vote \in timeoutIntents:
       ResumeTimeout(node, vote)
  \/ \E envelope \in proposalNetwork: DropProposal(envelope)

THEOREM DurableDecisionFrontierStutteringStepIsStutter ==
  DurableDecisionFrontierStutteringStep
    => UNCHANGED <<context, decisions, pendingDecision>>
BY IsaT(120)
   DEF DurableDecisionFrontierStutteringStep,
       SetGST, AssembleLocalBody, BeginLocalProposal, PersistProposal,
       CompleteProposalSignature, ByzantineBroadcastProposal,
       DeliverProposal, FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody, ValidateDecidedBody, RejectBody, BeginPrepare,
       PersistPrepare, CompleteVoteSignature, ByzantineBroadcastVote,
       DeliverVote, FormPrepareQC,
       ImportAuthenticatedCommitCertificate, DeliverQC,
       BeginObservePrepare, PersistObservePrepare, BeginLockCommit,
       PersistLockCommit, BeginTimeout, PersistTimeout,
       CompleteTimeoutSignature, ByzantineBroadcastTimeout,
       DeliverTimeout, FormTC, DeliverTC, BeginInstallTC,
       PersistInstallTC, FetchCertifiedBody, ApplyDecision, Restart,
       ResumeProposal, ResumeVote, ResumeTimeout, DropProposal

THEOREM CoreNextPreservesDecisionFrontierUniqueness ==
  /\ StrongInductiveInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ Next
  => DecisionFrontierUniquenessInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              DecisionFrontierUniquenessInvariant,
              Next
         PROVE DecisionFrontierUniquenessInvariant'
    <2>1. CASE DurableDecisionFrontierStutteringStep
      <3>1. UNCHANGED <<context, decisions, pendingDecision>>
        BY <2>1, DurableDecisionFrontierStutteringStepIsStutter
      <3> QED BY <1>1, <3>1,
           DecisionFrontierStutterPreservesInvariant
    <2>2. CASE \E node \in ValidatorIds, roundView \in Views,
                    subject \in Subjects:
                    FormCommitQC(node, roundView, subject)
      BY <1>1, <2>2, FormCommitQcPreservesDecisionFrontierUniqueness
    <2>3. CASE \E node \in ValidatorIds, qc \in ReceivedQcValues:
                    BeginDecision(node, qc)
      BY <1>1, <2>3, BeginDecisionPreservesDecisionFrontierUniqueness
    <2>4. CASE \E request \in pendingDecision: PersistDecision(request)
      BY <1>1, <2>4,
         PersistDecisionPreservesDecisionFrontierUniqueness
    <2>5. CASE \E node \in ValidatorIds: Crash(node)
      BY <1>1, <2>5, CrashPreservesDecisionFrontierUniqueness
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5
         DEF Next, DurableDecisionFrontierStutteringStep
  <1> QED BY <1>1

THEOREM CoreBracketPreservesDecisionFrontierUniqueness ==
  /\ StrongInductiveInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ [Next]_vars
  => DecisionFrontierUniquenessInvariant'
BY CoreNextPreservesDecisionFrontierUniqueness,
   DecisionFrontierStutterPreservesInvariant, Isa
   DEF vars

THEOREM AsyncBracketProjectsCoreBracket ==
  [AsyncNext]_AsyncAllVars => [Next]_vars
BY Isa
   DEF AsyncNext, AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars,
       vars

THEOREM AsyncBracketPreservesStrongDecisionFrontier ==
  /\ StrongInductiveInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ [AsyncNext]_AsyncAllVars
  => /\ StrongInductiveInvariant'
     /\ DecisionFrontierUniquenessInvariant'
BY AsyncBracketProjectsCoreBracket,
   CoreStrongInductiveActionPreservation,
   CoreBracketPreservesDecisionFrontierUniqueness

THEOREM StrongInductiveInvariantProjectsTypeInvariant ==
  StrongInductiveInvariant => TypeInvariant
BY DEF StrongInductiveInvariant, Safety

THEOREM StrongInductiveInvariantFromAsyncSpec ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []StrongInductiveInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext) => []StrongInductiveInvariant
    <2>1. AsyncInitAt(initialContext) => StrongInductiveInvariant
      BY InitAtEstablishesStrongInductiveInvariant
         DEF AsyncInitAt, AsyncBaseInitAt
    <2>2. /\ StrongInductiveInvariant
           /\ [AsyncNext]_AsyncAllVars
          => StrongInductiveInvariant'
      BY AsyncBracketProjectsCoreBracket,
         CoreStrongInductiveActionPreservation
    <2> QED BY <2>1, <2>2, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM DecisionFrontierUniquenessInvariantFromAsyncSpec ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []DecisionFrontierUniquenessInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []DecisionFrontierUniquenessInvariant
    <2>1. AsyncInitAt(initialContext)
            => /\ StrongInductiveInvariant
               /\ DecisionFrontierUniquenessInvariant
      BY InitAtEstablishesStrongInductiveInvariant,
         AsyncInitEstablishesDecisionFrontierUniqueness
         DEF AsyncInitAt, AsyncBaseInitAt
    <2>2. /\ StrongInductiveInvariant
           /\ DecisionFrontierUniquenessInvariant
           /\ [AsyncNext]_AsyncAllVars
          => /\ StrongInductiveInvariant'
             /\ DecisionFrontierUniquenessInvariant'
      BY AsyncBracketPreservesStrongDecisionFrontier
    <2> QED BY <2>1, <2>2, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

(***************************************************************************
Exact durable Decision selection and replay shape.
***************************************************************************)

THEOREM RestartDecisionChoiceIsAvailableLocally ==
  \A node:
    RestartDecisions(node) # {}
      => RestartDecision(node) \in RestartDecisions(node)
BY FS_EmptySet, Zenon DEF RestartDecision

THEOREM UniqueDecisionMembersWithSameNodeContextAreEqual ==
  \A left, right:
    /\ DecisionsUniqueByNodeContext
    /\ left \in decisions
    /\ right \in decisions
    /\ left.node = right.node
    /\ left.qc.context = right.qc.context
    => left = right
BY DEF DecisionsUniqueByNodeContext

THEOREM UniqueDecisionSelectsExactRestartRecordLocally ==
  \A node, qc:
    /\ DecisionsUniqueByNodeContext
    /\ [node |-> node, qc |-> qc] \in RestartDecisions(node)
    => RestartDecision(node) = [node |-> node, qc |-> qc]
PROOF
  <1>1. ASSUME NEW node, NEW qc,
                DecisionsUniqueByNodeContext,
                [node |-> node, qc |-> qc] \in RestartDecisions(node)
         PROVE RestartDecision(node) = [node |-> node, qc |-> qc]
    <2>1. RestartDecisions(node) # {}
      BY <1>1
    <2>2. RestartDecision(node) \in RestartDecisions(node)
      BY <2>1, RestartDecisionChoiceIsAvailableLocally
    <2>3. /\ RestartDecision(node).node = node
          /\ RestartDecision(node).qc.context = context
          /\ [node |-> node, qc |-> qc].node = node
          /\ [node |-> node, qc |-> qc].qc.context = context
      BY <1>1, <2>2 DEF RestartDecisions
    <2>4. /\ RestartDecision(node) \in decisions
          /\ [node |-> node, qc |-> qc] \in decisions
      BY <1>1, <2>2 DEF RestartDecisions
    <2> QED BY <1>1, <2>3, <2>4,
         UniqueDecisionMembersWithSameNodeContextAreEqual
  <1> QED BY <1>1

THEOREM UniqueUnappliedDecisionExcludesNodeApplication ==
  \A node, qc:
    /\ StrongInductiveInvariant
    /\ DecisionsUniqueByNodeContext
    /\ [node |-> node, qc |-> qc] \in RestartDecisions(node)
    => ~NodeHasApplication(node)
BY SMT
   DEF StrongInductiveInvariant, Safety, AppliedRequiresDecision,
       DecisionsUniqueByNodeContext, RestartDecisions,
       NodeHasApplication

THEOREM SelectedRestartDecisionReplayHasExactFetchSequence ==
  \A node, qc:
    RestartDecision(node) = [node |-> node, qc |-> qc]
      => RestartDecisionReplay(node)
           = <<DecisionFetchCandidateAt(
                 node, qc, nodeView[node], generation[node])>>
BY SMT
   DEF RestartDecisionReplay, RestartCandidate,
       DecisionFetchCandidateAt

THEOREM CurrentDecisionFetchCandidateHasExactCurrentFields ==
  \A node, qc:
    LET candidate == DecisionFetchCandidateAt(
                       node, qc, nodeView[node], generation[node])
    IN /\ candidate.kind = "FetchBody"
       /\ candidate.evidence = qc
       /\ candidate.consumerGeneration = generation[node]
       /\ CandidateConsumerCurrent(candidate)
       /\ ExactAsyncCandidateIdentity(candidate)
            = DecisionFetchCandidateIdentityAt(
                node, qc, nodeView[node], generation[node])
BY DEF DecisionFetchCandidateAt, DecisionFetchCandidateIdentityAt,
       CandidateConsumerCurrent, AsyncCandidateAtConsumer,
       AsyncCandidateWithIdentity

THEOREM UniqueDecisionRestartDecisionReplayIsExactCurrentFetch ==
  \A node, qc:
    /\ DecisionsUniqueByNodeContext
    /\ [node |-> node, qc |-> qc] \in RestartDecisions(node)
    => LET candidate == Head(RestartDecisionReplay(node))
       IN /\ RestartDecisionReplay(node)
                = <<DecisionFetchCandidateAt(
                      node, qc, nodeView[node], generation[node])>>
          /\ candidate.kind = "FetchBody"
          /\ candidate.evidence = qc
          /\ candidate.consumerGeneration = generation[node]
          /\ CandidateConsumerCurrent(candidate)
          /\ ExactAsyncCandidateIdentity(candidate)
               = DecisionFetchCandidateIdentityAt(
                   node, qc, nodeView[node], generation[node])
BY UniqueDecisionSelectsExactRestartRecordLocally,
   SelectedRestartDecisionReplayHasExactFetchSequence,
   CurrentDecisionFetchCandidateHasExactCurrentFields, SMT

THEOREM ReachableUniqueDecisionRestartReplayIsExactCurrentFetch ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []\A node, qc:
           [node |-> node, qc |-> qc] \in RestartDecisions(node)
             => LET candidate == Head(RestartDecisionReplay(node))
                IN /\ candidate.kind = "FetchBody"
                   /\ candidate.evidence = qc
                   /\ candidate.consumerGeneration = generation[node]
                   /\ CandidateConsumerCurrent(candidate)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []\A node, qc:
                      [node |-> node, qc |-> qc]
                        \in RestartDecisions(node)
                        => LET candidate ==
                                 Head(RestartDecisionReplay(node))
                           IN /\ candidate.kind = "FetchBody"
                              /\ candidate.evidence = qc
                              /\ candidate.consumerGeneration =
                                   generation[node]
                              /\ CandidateConsumerCurrent(candidate)
    <2>1. AsyncSpecAt(initialContext)
             => []DecisionFrontierUniquenessInvariant
      BY DecisionFrontierUniquenessInvariantFromAsyncSpec
    <2>2. DecisionFrontierUniquenessInvariant
             => DecisionsUniqueByNodeContext
      BY DEF DecisionFrontierUniquenessInvariant
    <2>3. DecisionsUniqueByNodeContext
             => \A node, qc:
                  [node |-> node, qc |-> qc] \in RestartDecisions(node)
                    => LET candidate ==
                             Head(RestartDecisionReplay(node))
                       IN /\ candidate.kind = "FetchBody"
                          /\ candidate.evidence = qc
                          /\ candidate.consumerGeneration = generation[node]
                          /\ CandidateConsumerCurrent(candidate)
      BY UniqueDecisionRestartDecisionReplayIsExactCurrentFetch
    <2> QED BY <2>1, <2>2, <2>3, PTL
  <1> QED BY <1>1

(***************************************************************************
Generation-free registration and exact crash/restart/replay handoff.
***************************************************************************)

THEOREM ResponsiveCrashPreservesGenerationFreeDecisionRegistration ==
  \A node, qc:
    PreGstResponsiveCrash(node)
      => (DecisionCertifiedRequestRegistered(node, qc)
            <=> DecisionCertifiedRequestRegistered(node, qc)')
BY SMT
   DEF PreGstResponsiveCrash, Crash, AsyncSchedulerVars,
       DecisionCertifiedRequestRegistered,
       DecisionCertifiedRequestIdentity,
       DecisionCertifiedRequestIdentityFor

THEOREM ResponsiveRestartPreservesGenerationFreeDecisionRegistration ==
  \A node, qc:
    /\ asyncRecoveryNode = node
    /\ PreGstResponsiveRestart
    => (DecisionCertifiedRequestRegistered(node, qc)
          <=> DecisionCertifiedRequestRegistered(node, qc)')
BY SMT
   DEF PreGstResponsiveRestart, Restart, AsyncSchedulerVars,
       DecisionCertifiedRequestRegistered,
       DecisionCertifiedRequestIdentity,
       DecisionCertifiedRequestIdentityFor

THEOREM ResponsiveReplayClearsGenerationFreeDecisionRegistration ==
  \A qc:
    PreGstResponsiveReplay
      => ~DecisionCertifiedRequestRegistered(asyncRecoveryNode, qc)'
BY SMT
   DEF PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
       DecisionCertifiedRequestRegistered,
       DecisionCertifiedRequestIdentity,
       DecisionCertifiedRequestIdentityFor

THEOREM ResponsiveCrashPreservesExactDecisionRegistrations ==
  \A node, qc:
    /\ StrongInductiveInvariant
    /\ [node |-> node, qc |-> qc] \in RestartDecisions(node)
    /\ PreGstResponsiveCrash(node)
    => /\ (DecisionRawHashRegistered(node, qc)
              <=> DecisionRawHashRegistered(node, qc)')
       /\ (DecisionCertifiedRequestRegistered(node, qc)
              <=> DecisionCertifiedRequestRegistered(node, qc)')
PROOF
  <1>1. ASSUME NEW node, NEW qc,
                StrongInductiveInvariant,
                [node |-> node, qc |-> qc] \in RestartDecisions(node),
                PreGstResponsiveCrash(node)
         PROVE /\ (DecisionRawHashRegistered(node, qc)
                       <=> DecisionRawHashRegistered(node, qc)')
               /\ (DecisionCertifiedRequestRegistered(node, qc)
                       <=> DecisionCertifiedRequestRegistered(node, qc)')
    <2>1. node \in ValidatorIds
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             PreGstResponsiveCrash
    <2>2. qc \in QcRecordSet
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             DecisionAgreement, RestartDecisions
    <2>3. DecisionRawHashRegistered(node, qc)
             <=> DecisionRawHashRegistered(node, qc)'
      BY <1>1, <2>1, <2>2,
         ResponsiveCrashPreservesDecisionRegistration, SMT
    <2>4. DecisionCertifiedRequestRegistered(node, qc)
             <=> DecisionCertifiedRequestRegistered(node, qc)'
      BY <1>1, ResponsiveCrashPreservesGenerationFreeDecisionRegistration
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM ResponsiveRestartPreservesExactDecisionRegistrations ==
  \A node, qc:
    /\ StrongInductiveInvariant
    /\ DurableDecisionRecoveryAuthority(node, qc)
    /\ PreGstResponsiveRestart
    => /\ (DecisionRawHashRegistered(node, qc)
              <=> DecisionRawHashRegistered(node, qc)')
       /\ (DecisionCertifiedRequestRegistered(node, qc)
              <=> DecisionCertifiedRequestRegistered(node, qc)')
PROOF
  <1>1. ASSUME NEW node, NEW qc,
                StrongInductiveInvariant,
                DurableDecisionRecoveryAuthority(node, qc),
                PreGstResponsiveRestart
         PROVE /\ (DecisionRawHashRegistered(node, qc)
                       <=> DecisionRawHashRegistered(node, qc)')
               /\ (DecisionCertifiedRequestRegistered(node, qc)
                       <=> DecisionCertifiedRequestRegistered(node, qc)')
    <2>1. asyncRecoveryNode = node
      BY <1>1 DEF DurableDecisionRecoveryAuthority
    <2>2. asyncRecoveryNode' = node
      BY <1>1, <2>1 DEF PreGstResponsiveRestart
    <2>3. qc \in QcRecordSet
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             DecisionAgreement, DurableDecisionRecoveryAuthority,
             RestartDecisions
    <2>4. DecisionRawHashRegistered(asyncRecoveryNode, qc)'
             <=> DecisionRawHashRegistered(asyncRecoveryNode, qc)
      BY <1>1, <2>3, AuthenticatedRestartPreservesRawRegistration
    <2>5. DecisionRawHashRegistered(node, qc)
             <=> DecisionRawHashRegistered(node, qc)'
      BY <2>1, <2>2, <2>4, SMT
         DEF DecisionRawHashRegistered, DecisionCommitAuthority,
             DecisionRegisteredOccurrences, DecisionRequestOccurrences,
             CertifiedRequestOutbox, AsyncNetworkItem, AsyncBodyEnvelope
    <2>6. DecisionCertifiedRequestRegistered(node, qc)
             <=> DecisionCertifiedRequestRegistered(node, qc)'
      BY <1>1, <2>1,
         ResponsiveRestartPreservesGenerationFreeDecisionRegistration
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM ResponsiveCrashInstallsExactDurableDecisionAuthority ==
  \A node, qc:
    /\ [node |-> node, qc |-> qc] \in RestartDecisions(node)
    /\ PreGstResponsiveCrash(node)
    => /\ DurableDecisionRecoveryAuthority(node, qc)'
       /\ DurableDecisionRecoveryExecutorCurrent(node)'
BY SMT
   DEF DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       RestartDecisions, PreGstResponsiveCrash, Crash

THEOREM ResponsiveRestartAdvancesExactDurableDecisionAuthority ==
  \A node, qc:
    /\ TypeInvariant
    /\ DurableDecisionRecoveryAuthority(node, qc)
    /\ PreGstResponsiveRestart
    => /\ generation'[node] = generation[node] + 1
       /\ DurableDecisionRecoveryAuthority(node, qc)'
       /\ DurableDecisionRecoveryExecutorCurrent(node)'
BY RestartIncrementsSelectedGeneration, SMT
   DEF DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       RestartDecisions, PreGstResponsiveRestart, Restart

THEOREM ResponsiveReplayInstallsExactCurrentDecisionFetchUpdate ==
  \A node, qc:
    /\ StrongInductiveInvariant
    /\ DecisionsUniqueByNodeContext
    /\ DurableDecisionRecoveryAuthority(node, qc)
    /\ asyncRecoveryPhase = "ReplayRequired"
    /\ PreGstResponsiveReplay
    => /\ ~DurableDecisionRecoveryAuthority(node, qc)'
       /\ ~DecisionRawHashRegistered(node, qc)'
       /\ ~DecisionCertifiedRequestRegistered(node, qc)'
       /\ ExactCurrentDecisionFetchUpdate(node, qc)
PROOF
  <1>1. ASSUME NEW node, NEW qc,
                StrongInductiveInvariant,
                DecisionsUniqueByNodeContext,
                DurableDecisionRecoveryAuthority(node, qc),
                asyncRecoveryPhase = "ReplayRequired",
                PreGstResponsiveReplay
         PROVE /\ ~DurableDecisionRecoveryAuthority(node, qc)'
               /\ ~DecisionRawHashRegistered(node, qc)'
               /\ ~DecisionCertifiedRequestRegistered(node, qc)'
               /\ ExactCurrentDecisionFetchUpdate(node, qc)
    <2>1. asyncRecoveryNode = node
      BY <1>1 DEF DurableDecisionRecoveryAuthority
    <2>2. asyncRecoveryNode' = node
      BY <1>1, <2>1 DEF PreGstResponsiveReplay
    <2>3. qc \in QcRecordSet
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             DecisionAgreement, DurableDecisionRecoveryAuthority,
             RestartDecisions
    <2>4. ~DecisionRawHashRegistered(asyncRecoveryNode, qc)'
      BY <1>1, <2>3, ResponsiveReplayClearsRecoveredNodeRegistration
    <2>5. ~DecisionRawHashRegistered(node, qc)'
      BY <2>1, <2>2, <2>4, SMT
         DEF DecisionRawHashRegistered, DecisionCommitAuthority,
             DecisionRegisteredOccurrences, DecisionRequestOccurrences,
             CertifiedRequestOutbox, AsyncNetworkItem, AsyncBodyEnvelope
    <2>6. ~DecisionCertifiedRequestRegistered(asyncRecoveryNode, qc)'
      BY <1>1, ResponsiveReplayClearsGenerationFreeDecisionRegistration
    <2>7. ~DecisionCertifiedRequestRegistered(node, qc)'
      BY <2>1, <2>2, <2>6, SMT
         DEF DecisionCertifiedRequestRegistered,
             DecisionCertifiedRequestIdentity,
             DecisionCertifiedRequestIdentityFor
    <2>8. /\ ~DurableDecisionRecoveryAuthority(node, qc)'
           /\ ExactCurrentDecisionFetchUpdate(node, qc)
      BY <1>1,
         UniqueUnappliedDecisionExcludesNodeApplication,
         UniqueDecisionRestartDecisionReplayIsExactCurrentFetch, SMT
         DEF DurableDecisionRecoveryAuthority, RestartDecisions,
             ExactCurrentDecisionFetchUpdate,
             PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
             RestartSignatureReplay, RestartReplay, RestartDecisionReplay,
             RestartCandidate, DecisionFetchCandidateAt
    <2> QED BY <2>5, <2>7, <2>8
  <1> QED BY <1>1

THEOREM ExactDurableDecisionRecoveryLifecycleTransition ==
  StrongInductiveInvariant => DurableDecisionRecoveryLifecycleTransition
PROOF
  <1>1. ASSUME StrongInductiveInvariant
         PROVE DurableDecisionRecoveryLifecycleTransition
    <2>1. \A node, qc:
            /\ [node |-> node, qc |-> qc] \in RestartDecisions(node)
            /\ PreGstResponsiveCrash(node)
            => /\ DurableDecisionRecoveryAuthority(node, qc)'
               /\ DurableDecisionRecoveryExecutorCurrent(node)'
               /\ (DecisionRawHashRegistered(node, qc)
                     <=> DecisionRawHashRegistered(node, qc)')
               /\ (DecisionCertifiedRequestRegistered(node, qc)
                     <=> DecisionCertifiedRequestRegistered(node, qc)')
      BY ResponsiveCrashInstallsExactDurableDecisionAuthority,
         <1>1, ResponsiveCrashPreservesExactDecisionRegistrations
    <2>2. \A node, qc:
            /\ DurableDecisionRecoveryAuthority(node, qc)
            /\ PreGstResponsiveRestart
            => /\ generation'[node] = generation[node] + 1
               /\ DurableDecisionRecoveryAuthority(node, qc)'
               /\ DurableDecisionRecoveryExecutorCurrent(node)'
               /\ (DecisionRawHashRegistered(node, qc)
                     <=> DecisionRawHashRegistered(node, qc)')
               /\ (DecisionCertifiedRequestRegistered(node, qc)
                     <=> DecisionCertifiedRequestRegistered(node, qc)')
      PROOF
        <3>1. ASSUME NEW node, NEW qc,
                      DurableDecisionRecoveryAuthority(node, qc),
                      PreGstResponsiveRestart
               PROVE /\ generation'[node] = generation[node] + 1
                     /\ DurableDecisionRecoveryAuthority(node, qc)'
                     /\ DurableDecisionRecoveryExecutorCurrent(node)'
                     /\ (DecisionRawHashRegistered(node, qc)
                           <=> DecisionRawHashRegistered(node, qc)')
                     /\ (DecisionCertifiedRequestRegistered(node, qc)
                           <=> DecisionCertifiedRequestRegistered(node, qc)')
          <4>1. /\ generation'[node] = generation[node] + 1
                 /\ DurableDecisionRecoveryAuthority(node, qc)'
                 /\ DurableDecisionRecoveryExecutorCurrent(node)'
            BY <1>1, <3>1,
               ResponsiveRestartAdvancesExactDurableDecisionAuthority
               DEF StrongInductiveInvariant, Safety
          <4>2. /\ (DecisionRawHashRegistered(node, qc)
                         <=> DecisionRawHashRegistered(node, qc)')
                 /\ (DecisionCertifiedRequestRegistered(node, qc)
                         <=> DecisionCertifiedRequestRegistered(node, qc)')
            BY <1>1, <3>1,
               ResponsiveRestartPreservesExactDecisionRegistrations
          <4> QED BY <4>1, <4>2
        <3> QED BY <3>1
    <2>3. \A node, qc:
            /\ StrongInductiveInvariant
            /\ DecisionsUniqueByNodeContext
            /\ DurableDecisionRecoveryAuthority(node, qc)
            /\ asyncRecoveryPhase = "ReplayRequired"
            /\ PreGstResponsiveReplay
            => /\ ~DurableDecisionRecoveryAuthority(node, qc)'
               /\ ~DecisionRawHashRegistered(node, qc)'
               /\ ~DecisionCertifiedRequestRegistered(node, qc)'
               /\ ExactCurrentDecisionFetchUpdate(node, qc)
      BY ResponsiveReplayInstallsExactCurrentDecisionFetchUpdate
    <2> QED BY <2>1, <2>2, <2>3
         DEF DurableDecisionRecoveryLifecycleTransition
  <1> QED BY <1>1

THEOREM DecisionRecoveryAcrossRestartPropertyFromAsyncSpec ==
  \A initialContext:
    DecisionRecoveryAcrossRestartProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE DecisionRecoveryAcrossRestartProperty(
                 AsyncSpecAt(initialContext))
    <2>1. AsyncSpecAt(initialContext)
             => []DecisionFrontierUniquenessInvariant
      BY DecisionFrontierUniquenessInvariantFromAsyncSpec
    <2>2. AsyncSpecAt(initialContext) => []StrongInductiveInvariant
      BY StrongInductiveInvariantFromAsyncSpec
    <2>3. StrongInductiveInvariant
             => DurableDecisionRecoveryLifecycleTransition
      BY ExactDurableDecisionRecoveryLifecycleTransition
    <2> QED BY <2>1, <2>2, <2>3, PTL
         DEF DecisionRecoveryAcrossRestartProperty
  <1> QED BY <1>1

=============================================================================
