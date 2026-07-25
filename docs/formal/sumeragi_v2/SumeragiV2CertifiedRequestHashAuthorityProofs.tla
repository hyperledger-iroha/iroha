---- MODULE SumeragiV2CertifiedRequestHashAuthorityProofs ----
EXTENDS SumeragiV2AsyncNetwork, SumeragiV2Proofs, TLAPS

(***************************************************************************
Decision/CommitQC certified-request recovery authority proof.

Scope is deliberately narrow: one durable local Decision carrying a CommitQC,
the exact signed certified-body request derived from that Decision, its
fan-out transport occurrences, and the crash -> authenticated restart ->
replay-reset path which recreates the Decision FetchBody scheduler frontier.
TC-owned acquisition, ordinary proposal acquisition, capacity accounting,
cryptographic collision resistance, and the complete AsyncNext induction are
owned by separate proof obligations.

The base wire occurrence carries requester, the full frozen QC, and a finite
symbolic signature nonce.  Its physical archive recipient remains outside the
exact signed-request projection.  `DecisionRawRequestHash` aliases that shared
collision-free structural surrogate for `HashOf<CertifiedBodyRequest>`.  It is
intentionally distinct from the 14-field `ExactAsyncCandidateIdentity`, whose
consumer generation is volatile.
***************************************************************************)

DecisionCommitAuthority(node, qc) ==
  /\ qc \in QcRecordSet
  /\ qc.context = context
  /\ qc.phase = "Commit"
  /\ [node |-> node, qc |-> qc] \in decisions

DecisionRawRequestPreimage(node, qc) ==
  AsyncCertifiedRequestPreimage(node, qc)

DecisionRawRequestSignature(node, qc) ==
  AsyncCertifiedRequestSignature(node, qc, 0)

DecisionRawSignedRequest(node, qc) ==
  AsyncCertifiedSignedRequest(node, qc, 0)

DecisionRawRequestHash(node, qc) ==
  AsyncCertifiedRequestHashOf(node, qc, 0)

DecisionRequestOccurrences(node, qc) ==
  CertifiedRequestOutbox(node, qc)

DecisionRegisteredOccurrences(node, qc) ==
  DecisionRequestOccurrences(node, qc) \cap asyncActiveRequests

DecisionRawHashRegistered(node, qc) ==
  /\ DecisionCommitAuthority(node, qc)
  /\ DecisionRegisteredOccurrences(node, qc) # {}

(***************************************************************************
The transport-free logical identity mirrors Rust `RequestIdentity`: round,
subject, requester.  The addressed server is transport fan-out and therefore
does not participate in the single outstanding logical registration.
***************************************************************************)

CertifiedRequestLogicalIdentity(request) ==
  [round |-> [context |-> request.envelope.certificate.context,
              height |-> request.envelope.height,
              view |-> request.envelope.view],
   subject |-> request.envelope.subject,
   requester |-> request.source]

DecisionLogicalRequestIdentity(node, qc) ==
  [round |-> [context |-> qc.context,
              height |-> qc.context.height, view |-> qc.view],
   subject |-> qc.subject,
   requester |-> node]

(***************************************************************************
Generation-scoped scheduler identities.  The raw request/hash above has no
consumer generation.  The scheduler candidate below uses the complete
production candidate constructor and exact identity projection.
***************************************************************************)

DecisionRequestCandidateAt(request, consumerView, consumerGeneration) ==
  AsyncCandidateAtConsumer(
    "Completion", "RequestCertifiedBody", request.source,
    request.envelope.height, request.envelope.view,
    request.envelope.subject, request, consumerView, consumerGeneration,
    request, request.envelope.subject, request.envelope.subject,
    request.envelope.subject)

DecisionRequestCandidateIdentityAt(
    request, consumerView, consumerGeneration) ==
  ExactAsyncCandidateIdentity(
    DecisionRequestCandidateAt(request, consumerView, consumerGeneration))

CurrentDecisionRequestCandidateIdentity(request) ==
  DecisionRequestCandidateIdentityAt(
    request, nodeView[request.source], generation[request.source])

CurrentDecisionRequestConsumerGeneration(request) ==
  generation[request.source]

DecisionFetchCandidateAt(node, qc, consumerView, consumerGeneration) ==
  AsyncCandidateAtConsumer(
    "Completion", "FetchBody", node, context.height, qc.view, qc.subject,
    NoAsyncItem, consumerView, consumerGeneration, qc,
    qc.subject, qc.subject, qc.subject)

DecisionFetchCandidateIdentityAt(node, qc, consumerView,
                                 consumerGeneration) ==
  ExactAsyncCandidateIdentity(
    DecisionFetchCandidateAt(
      node, qc, consumerView, consumerGeneration))

(***************************************************************************
Pure split facts.
***************************************************************************)

THEOREM DecisionOutboxHasOneLogicalRegistration ==
  \A node, qc:
    DecisionCommitAuthority(node, qc)
      => \A request \in DecisionRequestOccurrences(node, qc):
           /\ CertifiedRequestLogicalIdentity(request)
                = DecisionLogicalRequestIdentity(node, qc)
           /\ request.envelope.certificate = qc
           /\ AsyncCertifiedRequestHash(request)
                = DecisionRawRequestHash(node, qc)
BY SMT
   DEF DecisionCommitAuthority,
       DecisionRequestOccurrences, CertifiedRequestOutbox,
       CertifiedRequestLogicalIdentity, DecisionLogicalRequestIdentity,
       DecisionRawRequestHash, AsyncCertifiedRequestHash,
       AsyncCertifiedRequestHashOf, AsyncCertifiedSignedRequest,
       AsyncCertifiedRequestSignature, AsyncCertifiedRequestPreimage,
       AsyncNetworkItem, AsyncCertifiedRequestEnvelope

THEOREM DecisionRawHashIsTransportFanoutIndependent ==
  \A node, qc:
    \A left, right \in DecisionRequestOccurrences(node, qc):
      /\ AsyncCertifiedRequestHash(left)
           = DecisionRawRequestHash(node, qc)
      /\ AsyncCertifiedRequestHash(right)
           = DecisionRawRequestHash(node, qc)
      /\ AsyncCertifiedRequestHash(left)
           = AsyncCertifiedRequestHash(right)
BY SMT
   DEF DecisionRequestOccurrences, CertifiedRequestOutbox,
       DecisionRawRequestHash, AsyncCertifiedRequestHash,
       AsyncCertifiedRequestHashOf, AsyncCertifiedSignedRequest,
       AsyncCertifiedRequestSignature, AsyncCertifiedRequestPreimage,
       AsyncNetworkItem, AsyncCertifiedRequestEnvelope

THEOREM DecisionOutboxLogicalIndexIsConsistent ==
  \A node, qc:
    DecisionCommitAuthority(node, qc)
      => AsyncCertifiedRequestLogicalIndexConsistent(
           DecisionRequestOccurrences(node, qc))
BY DecisionOutboxHasOneLogicalRegistration,
   DecisionRawHashIsTransportFanoutIndependent, SMT
   DEF AsyncCertifiedRequestLogicalIndexConsistent,
       AsyncCertifiedRequestsIn,
       AsyncCertifiedRequestAliasesCompatible

THEOREM DecisionOutboxReplySemanticIsRouteFree ==
  \A node, qc:
    \A left, right \in DecisionRequestOccurrences(node, qc):
      AsyncReplySemanticIdentity(left.kind, left.envelope)
        = AsyncReplySemanticIdentity(right.kind, right.envelope)
BY DecisionRawHashIsTransportFanoutIndependent, SMT
   DEF DecisionRequestOccurrences, CertifiedRequestOutbox,
       AsyncReplySemanticIdentity, DecisionRawRequestHash,
       AsyncCertifiedRequestHash, AsyncCertifiedRequestHashOf,
       AsyncCertifiedSignedRequest, AsyncCertifiedRequestSignature,
       AsyncCertifiedRequestPreimage, AsyncNetworkItem,
       AsyncCertifiedRequestEnvelope

THEOREM DecisionRegisteredOccurrenceHasExactSource ==
  \A node, qc:
    \A request \in DecisionRegisteredOccurrences(node, qc):
      request.source = node
BY SMT
   DEF DecisionRegisteredOccurrences, DecisionRequestOccurrences,
       CertifiedRequestOutbox, AsyncNetworkItem,
       AsyncCertifiedRequestEnvelope

THEOREM DecisionRequestCandidateIdentityHasExactProductionShape ==
  \A request, consumerView, consumerGeneration:
    DecisionRequestCandidateIdentityAt(
      request, consumerView, consumerGeneration)
      = [consumer |->
           [context |-> context, height |-> context.height,
            node |-> request.source, view |-> consumerView,
            generation |-> consumerGeneration],
         payload |-> request,
         evidence |-> request,
         work |->
           [class |-> "Completion", kind |-> "RequestCertifiedBody",
            node |-> request.source, height |-> request.envelope.height,
            view |-> request.envelope.view,
            subject |-> request.envelope.subject],
         body |-> request.envelope.subject,
         manifest |-> request.envelope.subject,
         commitment |-> request.envelope.subject]
BY DEF DecisionRequestCandidateIdentityAt, DecisionRequestCandidateAt,
       ExactAsyncCandidateIdentity, AsyncConsumerEventTag,
       AsyncWorkIdentity, AsyncCandidateAtConsumer,
       AsyncCandidateWithIdentity

THEOREM DecisionFetchCandidateIdentityHasExactProductionShape ==
  \A node, qc, consumerView, consumerGeneration:
    DecisionFetchCandidateIdentityAt(
      node, qc, consumerView, consumerGeneration)
      = [consumer |->
           [context |-> context, height |-> context.height,
            node |-> node, view |-> consumerView,
            generation |-> consumerGeneration],
         payload |-> NoAsyncItem,
         evidence |-> qc,
         work |->
           [class |-> "Completion", kind |-> "FetchBody",
            node |-> node, height |-> context.height,
            view |-> qc.view, subject |-> qc.subject],
         body |-> qc.subject,
         manifest |-> qc.subject,
         commitment |-> qc.subject]
BY DEF DecisionFetchCandidateIdentityAt, DecisionFetchCandidateAt,
       ExactAsyncCandidateIdentity, AsyncConsumerEventTag,
       AsyncWorkIdentity, AsyncCandidateAtConsumer,
       AsyncCandidateWithIdentity

THEOREM RawHashStableWhileRequestCandidateRetags ==
  \A node, qc, request, consumerView, oldGeneration, newGeneration:
    /\ request \in DecisionRequestOccurrences(node, qc)
    /\ oldGeneration # newGeneration
    => /\ DecisionRawRequestHash(node, qc)
             = DecisionRawRequestHash(node, qc)
       /\ DecisionRequestCandidateIdentityAt(
            request, consumerView, oldGeneration)
            # DecisionRequestCandidateIdentityAt(
                request, consumerView, newGeneration)
BY DecisionRequestCandidateIdentityHasExactProductionShape, SMT

THEOREM RawHashStableWhileDecisionFetchCandidateRetags ==
  \A node, qc, consumerView, oldGeneration, newGeneration:
    oldGeneration # newGeneration
    => /\ DecisionRawRequestHash(node, qc)
             = DecisionRawRequestHash(node, qc)
       /\ DecisionFetchCandidateIdentityAt(
            node, qc, consumerView, oldGeneration)
            # DecisionFetchCandidateIdentityAt(
                node, qc, consumerView, newGeneration)
BY DecisionFetchCandidateIdentityHasExactProductionShape, SMT

(***************************************************************************
Crash/restart/replay action facts.  Raw registration is represented by exact
active fan-out occurrences plus its durable Decision/CommitQC authorizer.
***************************************************************************)

THEOREM ResponsiveCrashPreservesDecisionRegistration ==
  \A node \in ValidatorIds:
    PreGstResponsiveCrash(node)
      => \A qc \in QcRecordSet:
           /\ DecisionRegisteredOccurrences(node, qc)'
                = DecisionRegisteredOccurrences(node, qc)
           /\ DecisionRawHashRegistered(node, qc)'
                <=> DecisionRawHashRegistered(node, qc)
BY SMT
   DEF PreGstResponsiveCrash, Crash, AsyncSchedulerVars,
       DecisionRegisteredOccurrences, DecisionRequestOccurrences,
       DecisionRawHashRegistered, DecisionCommitAuthority,
       CertifiedRequestOutbox, CertifiedArchiveRoutes,
       AsyncNetworkItem, AsyncCertifiedRequestEnvelope

THEOREM AuthenticatedRestartPreservesRawRegistration ==
  PreGstResponsiveRestart
    => \A qc \in QcRecordSet:
         /\ DecisionRegisteredOccurrences(asyncRecoveryNode, qc)'
              = DecisionRegisteredOccurrences(asyncRecoveryNode, qc)
         /\ DecisionRawHashRegistered(asyncRecoveryNode, qc)'
              <=> DecisionRawHashRegistered(asyncRecoveryNode, qc)
BY SMT
   DEF PreGstResponsiveRestart, Restart, AsyncSchedulerVars,
       DecisionRegisteredOccurrences, DecisionRequestOccurrences,
       DecisionRawHashRegistered, DecisionCommitAuthority,
       CertifiedRequestOutbox, CertifiedArchiveRoutes,
       AsyncNetworkItem, AsyncCertifiedRequestEnvelope

THEOREM AuthenticatedRestartRetagsSourceConsumerGeneration ==
  \A request:
    /\ TypeInvariant
    /\ request.source = asyncRecoveryNode
    /\ generation[request.source] \in Nat
    /\ PreGstResponsiveRestart
    => /\ CurrentDecisionRequestConsumerGeneration(request)'
             = CurrentDecisionRequestConsumerGeneration(request) + 1
       /\ CurrentDecisionRequestConsumerGeneration(request)'
             # CurrentDecisionRequestConsumerGeneration(request)
BY RestartIncrementsSelectedGeneration, SMT
   DEF PreGstResponsiveRestart,
       CurrentDecisionRequestConsumerGeneration

THEOREM ResponsiveReplayClearsRecoveredNodeRegistration ==
  PreGstResponsiveReplay
    => \A qc \in QcRecordSet:
         /\ DecisionRegisteredOccurrences(asyncRecoveryNode, qc)' = {}
         /\ ~DecisionRawHashRegistered(asyncRecoveryNode, qc)'
BY SMT
   DEF PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
       DecisionRegisteredOccurrences, DecisionRequestOccurrences,
       DecisionRawHashRegistered, DecisionCommitAuthority,
       CertifiedRequestOutbox, CertifiedArchiveRoutes,
       AsyncNetworkItem, AsyncCertifiedRequestEnvelope

(***************************************************************************
The replay reset does not synthesize or retain an old-generation request
candidate.  It installs the durable Decision FetchBody frontier with the
post-restart generation; executing that frontier may then atomically publish
and register a fresh raw request for the same CommitQC.
***************************************************************************)

THEOREM RestartDecisionReplayHasCurrentGeneration ==
  \A node \in ValidatorIds:
    /\ ~NodeHasApplication(node)
    /\ RestartDecisions(node) # {}
    => LET qc == RestartDecision(node).qc
       IN /\ RestartReplay(node)
                = <<DecisionFetchCandidateAt(
                      node, qc, nodeView[node], generation[node])>>
          /\ ExactAsyncCandidateIdentity(Head(RestartReplay(node)))
               = DecisionFetchCandidateIdentityAt(
                   node, qc, nodeView[node], generation[node])
BY SMT
   DEF RestartReplay, RestartDecisionReplay, RestartDecision,
       RestartCandidate, DecisionFetchCandidateAt,
       DecisionFetchCandidateIdentityAt

THEOREM ResponsiveReplayQueuesFreshGenerationDecisionFetch ==
  /\ asyncRecoveryNode \in DOMAIN asyncCausalQueues
  /\ PreGstResponsiveReplay
  /\ ~NodeHasApplication(asyncRecoveryNode)
  /\ RestartDecisions(asyncRecoveryNode) # {}
  => LET node == asyncRecoveryNode
         qc == RestartDecision(node).qc
     IN /\ asyncCausalQueues'[node]
              = <<DecisionFetchCandidateAt(
                    node, qc, nodeView[node], generation[node])>>
        /\ ExactAsyncCandidateIdentity(Head(asyncCausalQueues'[node]))
             = DecisionFetchCandidateIdentityAt(
                 node, qc, nodeView[node], generation[node])
BY RestartDecisionReplayHasCurrentGeneration, SMT
   DEF PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
       RestartSignatureReplay, RestartReplay, RestartDecisionReplay,
       RestartDecision, RestartCandidate, DecisionFetchCandidateAt,
       DecisionFetchCandidateIdentityAt

DecisionCertifiedPublish(node, qc) ==
  /\ DecisionCommitAuthority(node, qc)
  /\ UNCHANGED <<context, decisions>>
  /\ PublishCertifiedRequests(DecisionRequestOccurrences(node, qc))

THEOREM DecisionCertifiedPublishUsesCompatibleLogicalIndex ==
  \A node, qc:
    DecisionCertifiedPublish(node, qc)
      => /\ AsyncCertifiedRequestLogicalIndexConsistent(
               DecisionRequestOccurrences(node, qc))
         /\ AsyncCertifiedRequestSetsCompatible(
              asyncActiveRequests,
              DecisionRequestOccurrences(node, qc))
BY SMT
   DEF DecisionCertifiedPublish, PublishCertifiedRequests

THEOREM DecisionCertifiedPublishPreservesLogicalIndex ==
  \A node, qc:
    /\ AsyncActiveRequestLogicalIndexConsistencyInvariant
    /\ DecisionCertifiedPublish(node, qc)
    => AsyncActiveRequestLogicalIndexConsistencyInvariant'
BY SMT
   DEF DecisionCertifiedPublish, PublishCertifiedRequests,
       AsyncActiveRequestLogicalIndexConsistencyInvariant,
       AsyncCertifiedRequestLogicalIndexConsistent,
       AsyncCertifiedRequestSetsCompatible,
       AsyncCertifiedRequestsIn,
       AsyncCertifiedRequestAliasesCompatible

THEOREM DecisionCertifiedPublishRetainsCommitAuthority ==
  \A node, qc:
    DecisionCertifiedPublish(node, qc)
      => DecisionCommitAuthority(node, qc)'
BY SMT
   DEF DecisionCertifiedPublish, DecisionCommitAuthority

THEOREM CertifiedPublishContainsEveryPublishedOccurrence ==
  \A items:
    PublishCertifiedRequests(items)
      => items \subseteq asyncActiveRequests'
BY SMT DEF PublishCertifiedRequests

THEOREM DecisionOccurrencesStableWhenContextIsFramed ==
  \A node, qc:
    UNCHANGED context
      => DecisionRequestOccurrences(node, qc)'
           = DecisionRequestOccurrences(node, qc)
BY SMT
   DEF DecisionRequestOccurrences, CertifiedRequestOutbox,
       CertifiedArchiveRoutes, AsyncNetworkItem,
       AsyncCertifiedRequestEnvelope

THEOREM DecisionCertifiedPublishAddsRegistrationOccurrences ==
  \A node, qc:
    /\ DecisionRequestOccurrences(node, qc) # {}
    /\ DecisionCertifiedPublish(node, qc)
    => DecisionRegisteredOccurrences(node, qc)' # {}
BY CertifiedPublishContainsEveryPublishedOccurrence,
   DecisionOccurrencesStableWhenContextIsFramed, SMT
   DEF DecisionCertifiedPublish, DecisionRegisteredOccurrences

THEOREM DecisionRawRequestHashIsStateIndependent ==
  \A node, qc:
    DecisionRawRequestHash(node, qc)'
      = DecisionRawRequestHash(node, qc)
OBVIOUS

THEOREM DecisionCertifiedPublishRegistersExactRawHash ==
  \A node, qc:
    /\ DecisionRequestOccurrences(node, qc) # {}
    /\ DecisionCertifiedPublish(node, qc)
    => /\ DecisionRawHashRegistered(node, qc)'
       /\ DecisionRawRequestHash(node, qc)'
            = DecisionRawRequestHash(node, qc)
BY DecisionCertifiedPublishRetainsCommitAuthority,
   DecisionCertifiedPublishAddsRegistrationOccurrences,
   DecisionRawRequestHashIsStateIndependent, SMT
   DEF DecisionRawHashRegistered

(***************************************************************************
TODO: the production-refinement ledger must still enumerate the Rust
`ExecuteDecisionFetch` branches and show that its missing-body branch
instantiates `DecisionCertifiedPublish`, while the durable-body branch frames
registration.  This module proves the action-local publisher and exact modeled
recovery authority; the concrete executor/runner trace mapping remains in
`ProgressWitnessProductionRefinementObligation`.
***************************************************************************)

=============================================================================
