---- MODULE SumeragiV2DecisionWitnessPreservationProofs ----
EXTENDS SumeragiV2ProgressWitnessPreservationProofs

(***************************************************************************
Exact durable-Decision witness preservation.

The older `DecisionRecoveryStage` is intentionally permissive: it recognizes
coordinate-compatible FetchBody candidates, includes the dead
RequestCertifiedBody command vocabulary, and attributes a certified response
to its outer transport source.  Those choices are adequate for describing a
pipeline inventory but conflate relay transport with authenticated response
identity and are too weak for an inductive ownership proof.

This module uses the production identities at the points where identity is
semantically authoritative:

  * FetchBody carries the exact durable Decision QC as evidence;
  * a certified request carries the recovering node as source/requester, the
    exact durable Decision QC and signature nonce zero, while its physical
    recipient is one production archive route for that request;
  * a certified response retains the exact signed-request hash, binds its
    authenticated signature owner to the physical archive server, and cites
    one signer of the exact Decision QC.  Its authenticated sent occurrence
    and the matching append-only signed request are retained as the immutable
    delayed-execution capability.  The archive need not equal any original
    request route: routing is liveness-only and reconnect or relay may expose
    the same exact signed request to another archive.  Its outer source remains
    only the transport relay and is deliberately unconstrained;
  * StoreBody, ValidateBody, and Apply stay coordinate-and-state based because
    AssembleBody deliberately emits them with NoAsyncItem; and
  * RequestCertifiedBody is excluded because no AsyncNext constructor emits
    it.  ExecuteDecisionFetch publishes the certified requests directly.

No theorem in this module assumes an action-preservation fact.  Semantic
handoffs name the exact Core/adapter action and exact post-state scheduler
owner which they need.  Scheduler theorems below separately establish that
successful serialized dispatch supplies that owner.
***************************************************************************)

DecisionFetchBodyOwnedExact(node, qc) ==
  \E candidate \in AsyncCandidateSet:
    /\ candidate.kind = "FetchBody"
    /\ candidate.evidence = qc
    /\ DecisionPipelineCandidate(node, qc, candidate)

DecisionCertifiedRequestActiveExact(node, qc) ==
  \E request \in asyncActiveRequests:
    /\ request \in CertifiedRequestOutbox(node, qc)
    /\ AsyncCertifiedRequestHash(request) =
         AsyncCertifiedRequestHashOf(node, qc, 0)

DecisionCertifiedResponseLineageExact(node, qc, item) ==
  /\ item.kind = "CertifiedResponse"
  /\ item.envelope.recipient = node
  /\ item.envelope.height = qc.context.height
  /\ item.envelope.view = qc.view
  /\ item.envelope.subject = qc.subject
  /\ item.envelope.requestHash =
       AsyncCertifiedRequestHashOf(node, qc, 0)
  /\ item.envelope.signatureOwner = item.envelope.archiveServer
  /\ item.envelope.citedResponder \in qc.signers
  /\ CertifiedResponseAuthenticatedOccurrence(item)
  /\ CertifiedResponseCapabilityAuthorized(item)

DecisionCertifiedFetchOwnedExact(node, qc) ==
  \E item \in AsyncNetworkItems:
    /\ DecisionCertifiedResponseLineageExact(node, qc, item)
    /\ CertifiedResponseCandidate(item) \in AsyncCandidateSet
    /\ DecisionPipelineCandidate(
         node, qc, CertifiedResponseCandidate(item))

DecisionStoreBodyOwned(node, qc) ==
  DecisionPipelineKindOwned(node, qc, "StoreBody")

DecisionValidateBodyOwned(node, qc) ==
  DecisionPipelineKindOwned(node, qc, "ValidateBody")

DecisionApplyOwned(node, qc) ==
  DecisionPipelineKindOwned(node, qc, "Apply")

(***************************************************************************
Validation partitions the exact stage before body availability does.
Consequently an old exact FetchBody owner is not a witness after validation
has completed, even if it remains scheduled while its Apply successor moves
through another carrier.  In the no-validation partition, FetchBody may
legitimately observe either missing or already-durable bytes: the latter is
the restart/replay short circuit which schedules ValidateBody.
***************************************************************************)

DecisionRecoveryStageExact(node, qc) ==
  \/ NodeHasApplication(node)
  \/ /\ ~DecisionValidationHeld(node, qc)
     /\ \/ DecisionFetchBodyOwnedExact(node, qc)
        \/ /\ ~BodyHeldBy(durableBodies, node, qc.context,
                           qc.view, qc.subject)
           /\ \/ DecisionCertifiedRequestActiveExact(node, qc)
              \/ DecisionCertifiedFetchOwnedExact(node, qc)
              \/ /\ DecisionBody(node, qc) \in availableBodies
                 /\ DecisionStoreBodyOwned(node, qc)
        \/ /\ BodyHeldBy(durableBodies, node, qc.context,
                          qc.view, qc.subject)
           /\ DecisionValidateBodyOwned(node, qc)
  \/ /\ BodyHeldBy(durableBodies, node, qc.context,
                    qc.view, qc.subject)
     /\ DecisionValidationHeld(node, qc)
     /\ DecisionApplyOwned(node, qc)

AsyncDecisionRecoveryStageExact(node, qc) ==
  \/ DecisionRecoveryStageExact(node, qc)
  \/ DecisionRecoveryAuthority(node, qc)

DecisionExactSourceOwner(node) ==
  \/ node \in AsyncCurrentResponsiveVoters
  \/ HistoricalRecoveryTarget(node)

DecisionExactSourceRetentionInvariant ==
  \A decision \in decisions:
    (DecisionExactSourceOwner(decision.node)
      /\ decision.qc.context = context)
      => AsyncDecisionRecoveryStageExact(decision.node, decision.qc)

DecisionFrontierAndExactStageInvariant ==
  /\ DecisionFrontierUniquenessInvariant
  /\ DecisionExactSourceRetentionInvariant

(***************************************************************************
Projection to the release witness.
***************************************************************************)

THEOREM ExactFetchOwnerProvidesDecisionCompletionWitness ==
  \A node, qc:
    DecisionFetchBodyOwnedExact(node, qc)
      => DecisionCompletionWitness(node, qc)
BY Isa
   DEF DecisionFetchBodyOwnedExact, DecisionCompletionWitness

THEOREM ExactCertifiedRequestProvidesDecisionCompletionWitness ==
  \A node, qc:
    DecisionCertifiedRequestActiveExact(node, qc)
      => DecisionCompletionWitness(node, qc)
BY Isa
   DEF DecisionCertifiedRequestActiveExact,
       DecisionCompletionWitness

THEOREM ExactCertifiedFetchProvidesDecisionCompletionWitness ==
  \A node, qc:
    DecisionCertifiedFetchOwnedExact(node, qc)
      => DecisionCompletionWitness(node, qc)
BY Isa
   DEF DecisionCertifiedFetchOwnedExact,
       DecisionCompletionWitness

THEOREM ExactDecisionStageProvidesCompletionWitness ==
  \A node, qc:
    DecisionRecoveryStageExact(node, qc)
      => DecisionCompletionWitness(node, qc)
BY ExactFetchOwnerProvidesDecisionCompletionWitness,
   ExactCertifiedRequestProvidesDecisionCompletionWitness,
   ExactCertifiedFetchProvidesDecisionCompletionWitness, Isa
   DEF DecisionRecoveryStageExact, DecisionCompletionWitness,
       DecisionStoreBodyOwned, DecisionValidateBodyOwned,
       DecisionApplyOwned, DecisionPipelineKindOwned

THEOREM ExactDecisionSourceRetentionProjectsAsyncWitness ==
  DecisionExactSourceRetentionInvariant
    => AsyncDurableDecisionProgressWitness
PROOF
  <1>1. ASSUME DecisionExactSourceRetentionInvariant
         PROVE AsyncDurableDecisionProgressWitness
    <2>1. ASSUME NEW decision \in decisions,
                  /\ decision.node \in AsyncCurrentResponsiveVoters
                     /\ decision.qc.context = context
           PROVE AsyncDecisionCompletionWitness(
                   decision.node, decision.qc)
      <3>1. AsyncDecisionRecoveryStageExact(
               decision.node, decision.qc)
        BY <1>1, <2>1
           DEF DecisionExactSourceRetentionInvariant,
               DecisionExactSourceOwner
      <3>2. DecisionRecoveryStageExact(
               decision.node, decision.qc)
               => DecisionCompletionWitness(
                    decision.node, decision.qc)
        BY ExactDecisionStageProvidesCompletionWitness
      <3> QED BY <3>1, <3>2
           DEF AsyncDecisionRecoveryStageExact,
               AsyncDecisionCompletionWitness
    <2> QED BY <2>1 DEF AsyncDurableDecisionProgressWitness
  <1> QED BY <1>1

THEOREM DecisionFrontierAndExactStageProjectsAsyncWitness ==
  DecisionFrontierAndExactStageInvariant
    => /\ DecisionsUniqueByNodeContext
       /\ AsyncDurableDecisionProgressWitness
BY ExactDecisionSourceRetentionProjectsAsyncWitness
   DEF DecisionFrontierAndExactStageInvariant,
       DecisionFrontierUniquenessInvariant

(***************************************************************************
The base case is exact.  Genesis has no Decision.  A non-genesis bootstrap
Decision is for BootstrapParentContext, never for the initialized current
context, so the quantified stage is vacuous.
***************************************************************************)

THEOREM AsyncInitEstablishesDecisionExactSourceRetention ==
  \A initialContext:
    AsyncInitAt(initialContext) => DecisionExactSourceRetentionInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE DecisionExactSourceRetentionInvariant
    <2>1. /\ context = initialContext
           /\ initialContext.height \in Nat
           /\ (initialContext.height = 0 => decisions = {})
           /\ (initialContext.height > 0
                 => /\ decisions =
                          {BootstrapParentDecision(initialContext)}
                    /\ BootstrapParentContext(initialContext)
                         # initialContext)
      BY <1>1, FrozenContextFieldsTyped,
         BootstrapParentContextPrecedes, Isa
         DEF AsyncInitAt, AsyncBaseInitAt, InitAt, Heights
    <2>2. ASSUME NEW decision \in decisions,
                  /\ DecisionExactSourceOwner(decision.node)
                     /\ decision.qc.context = context
           PROVE AsyncDecisionRecoveryStageExact(
                   decision.node, decision.qc)
      <3>1. CASE initialContext.height = 0
        BY <2>1, <2>2, <3>1
      <3>2. CASE initialContext.height > 0
        <4>1. decision = BootstrapParentDecision(initialContext)
          BY <2>1, <2>2, <3>2
        <4>2. decision.qc.context =
                 BootstrapParentContext(initialContext)
          BY <4>1
             DEF BootstrapParentDecision, BootstrapParentCommitQC, QC
        <4> QED BY <2>1, <2>2, <4>2
      <3> QED BY <2>1, <3>1, <3>2, SMT
    <2> QED BY <2>2
         DEF DecisionExactSourceRetentionInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesDecisionFrontierAndExactStage ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => DecisionFrontierAndExactStageInvariant
BY AsyncInitEstablishesDecisionFrontierUniqueness,
   AsyncInitEstablishesDecisionExactSourceRetention
   DEF DecisionFrontierAndExactStageInvariant

(***************************************************************************
Static exact-response identity, request-hash, and dispatch facts.
***************************************************************************)

THEOREM ExactDecisionFetchHasCertifiedRecoveryFrontier ==
  \A node, qc, candidate:
    /\ [node |-> node, qc |-> qc] \in decisions
    /\ qc.context = context
    /\ qc.phase = "Commit"
    /\ candidate \in AsyncCandidateSet
    /\ candidate.kind = "FetchBody"
    /\ candidate.node = node
    /\ candidate.height = qc.context.height
    /\ candidate.view = qc.view
    /\ candidate.subject = qc.subject
    /\ candidate.evidence = qc
    => /\ DecisionFetchFrontier(candidate)
       /\ CertifiedRecoveryFetchFrontier(candidate)
BY Isa
   DEF DecisionFetchFrontier, CertifiedRecoveryFetchFrontier,
       DecisionCertifiedBodyRecoveryAuthority

THEOREM DecisionNodeExcludesLockedPrepareRecoveryAuthority ==
  \A node, decisionQc, prepareQc:
    /\ [node |-> node, qc |-> decisionQc] \in decisions
    /\ decisionQc.context = context
    /\ HistoricalLockedPrepareSource(node, prepareQc)
    => FALSE
BY Isa
   DEF HistoricalLockedPrepareSource, NoDecisionForNode

THEOREM ExactDecisionCertifiedRequestBindsHashAndArchiveRoute ==
  \A node, qc, request:
    request \in CertifiedRequestOutbox(node, qc)
      => /\ request.kind = "CertifiedRequest"
         /\ request.source = node
         /\ request.envelope.requester = node
         /\ request.envelope.certificate = qc
         /\ request.envelope.signatureNonce = 0
         /\ request.envelope.recipient
              \in CertifiedArchiveRoutes(node, qc)
         /\ AsyncCertifiedRequestHash(request) =
              AsyncCertifiedRequestHashOf(node, qc, 0)
BY Isa
   DEF CertifiedRequestOutbox, AsyncNetworkItem,
       AsyncCertifiedRequestEnvelope, AsyncCertifiedRequestHash

(***************************************************************************
Decision persistence retires stale node-local request authority, but keeps
the one logical certified request for the Decision target.  The production
logical identity is requester/height/view/subject; the physical archive route
and the Prepare/Commit phase carried by the full certificate are not part of
that retirement key.
***************************************************************************)

THEOREM CertifiedRequestOutboxDecisionSurvivalIsExactTarget ==
  \A requester, requestQc, decisionNode, decisionQc, request:
    request \in CertifiedRequestOutbox(requester, requestQc)
      => (CertifiedRequestSurvivesDecision(
             request, decisionNode, decisionQc)
            <=> \/ requester # decisionNode
                \/ /\ requestQc.context.height =
                         decisionQc.context.height
                   /\ requestQc.view = decisionQc.view
                   /\ requestQc.subject = decisionQc.subject)
BY Isa
   DEF CertifiedRequestSurvivesDecision, CertifiedRequestOutbox,
       AsyncNetworkItem, AsyncCertifiedRequestEnvelope

THEOREM PersistDecisionControlRetainsExactlySurvivingRequests ==
  \A node, qc, items, broadcast, request:
    /\ request \in asyncActiveRequests
    /\ PersistDecisionControl(node, qc, items, broadcast)
    => (request \in asyncActiveRequests'
          <=> CertifiedRequestSurvivesDecision(request, node, qc))
BY Isa
   DEF PersistDecisionControl, FilterCertifiedResponseAuthority

THEOREM ExactCertifiedResponseBindsArchiveAndCitedIdentityRoles ==
  \A node, qc, item:
    DecisionCertifiedResponseLineageExact(node, qc, item)
      => /\ item.envelope.recipient = node
         /\ item.envelope.height = qc.context.height
         /\ item.envelope.view = qc.view
         /\ item.envelope.subject = qc.subject
         /\ item.envelope.requestHash =
              AsyncCertifiedRequestHashOf(node, qc, 0)
         /\ item.envelope.signatureOwner =
              item.envelope.archiveServer
         /\ item.envelope.citedResponder \in qc.signers
         /\ CertifiedResponseAuthenticatedOccurrence(item)
         /\ CertifiedResponseCapabilityAuthorized(item)
BY DEF DecisionCertifiedResponseLineageExact

THEOREM ExactCertifiedResponseMatchesDecisionRequestHash ==
  \A node, qc, item, request:
    /\ DecisionCertifiedResponseLineageExact(node, qc, item)
    /\ request \in asyncActiveRequests
    /\ request \in CertifiedRequestOutbox(node, qc)
    => request \in MatchingCertifiedRequests(item)
BY ExactDecisionCertifiedRequestBindsHashAndArchiveRoute, Isa
   DEF DecisionCertifiedResponseLineageExact, MatchingCertifiedRequests

THEOREM ExactCertifiedResponseCandidateRetainsOuterItem ==
  \A node, qc, item:
    DecisionCertifiedResponseLineageExact(node, qc, item)
      => /\ CertifiedResponseCandidate(item).item = item
         /\ CertifiedResponseCandidate(item).evidence = item
BY DEF CertifiedResponseCandidate, AsyncCandidate,
       AsyncCandidateWithIdentity

DecisionExecutableStageOwner(node, qc, candidate) ==
  /\ [node |-> node, qc |-> qc] \in decisions
  /\ qc.context = context
  /\ qc.phase = "Commit"
  /\ ~NodeHasApplication(node)
  /\ candidate \in AsyncCandidateSet
  /\ DecisionPipelineCandidate(node, qc, candidate)
  /\ CASE candidate.kind = "FetchBody" ->
            /\ candidate.evidence = qc
            /\ ~DecisionValidationHeld(node, qc)
       [] candidate.kind = "FetchCertifiedBody" ->
            /\ ~BodyHeldBy(durableBodies, node, qc.context,
                            qc.view, qc.subject)
            /\ ~DecisionValidationHeld(node, qc)
            /\ DecisionCertifiedResponseLineageExact(
                 node, qc, candidate.item)
            /\ candidate = CertifiedResponseCandidate(candidate.item)
       [] candidate.kind = "StoreBody" ->
            /\ ~BodyHeldBy(durableBodies, node, qc.context,
                            qc.view, qc.subject)
            /\ ~DecisionValidationHeld(node, qc)
            /\ DecisionBody(node, qc) \in availableBodies
       [] candidate.kind = "ValidateBody" ->
            /\ BodyHeldBy(durableBodies, node, qc.context,
                           qc.view, qc.subject)
            /\ ~DecisionValidationHeld(node, qc)
       [] candidate.kind = "Apply" ->
            /\ BodyHeldBy(durableBodies, node, qc.context,
                           qc.view, qc.subject)
            /\ DecisionValidationHeld(node, qc)
       [] OTHER -> FALSE

THEOREM ExactDecisionStageDecomposition ==
  \A node, qc:
    /\ [node |-> node, qc |-> qc] \in decisions
    /\ qc.context = context
    /\ qc.phase = "Commit"
    /\ DecisionRecoveryStageExact(node, qc)
    => \/ NodeHasApplication(node)
       \/ /\ ~BodyHeldBy(durableBodies, node, qc.context,
                          qc.view, qc.subject)
          /\ ~DecisionValidationHeld(node, qc)
          /\ DecisionCertifiedRequestActiveExact(node, qc)
       \/ \E candidate \in AsyncCandidateSet:
            DecisionExecutableStageOwner(node, qc, candidate)
BY Isa
   DEF DecisionRecoveryStageExact,
       DecisionFetchBodyOwnedExact,
       DecisionCertifiedRequestActiveExact,
       DecisionCertifiedFetchOwnedExact,
       DecisionStoreBodyOwned, DecisionValidateBodyOwned,
       DecisionApplyOwned, DecisionPipelineKindOwned,
       DecisionExecutableStageOwner

THEOREM DecisionExecutableStageOwnerEnablesExecution ==
  \A node, qc, candidate:
    /\ AsyncStrongTypeInvariant
    /\ DecisionsUniqueByNodeContext
    /\ DecisionExecutableStageOwner(node, qc, candidate)
    => ENABLED ExecuteCommand(candidate)
BY ExpandENABLED, IsaT(300)
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety,
       TypeInvariant, DecisionAgreement,
       DecisionsUniqueByNodeContext,
       DecisionExecutableStageOwner,
       DecisionPipelineCandidate, CandidateConsumerCurrent,
       DecisionValidationHeld, DecisionBody,
       DecisionCertifiedResponseLineageExact,
       CertifiedResponseCandidate, AsyncCandidate,
       AsyncCandidateWithIdentity,
       CertifiedResponseCapabilityAuthorized,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       ExecuteDecisionFetch, ExecuteApply,
       CertifiedRecoveryFetchFrontier, DecisionFetchFrontier,
       DecisionCertifiedBodyRecoveryAuthority,
       CertifiedBodyRecoveryAuthority, HistoricalLockedPrepareSource,
       FetchCertifiedBody, AcceptCertifiedResponseCapability,
       InstallCertifiedBodyEffect,
       StoreBody, ValidateDecidedBody, ApplyDecision,
       CommandMatches, NodeHasApplication, NoDecisionForNode,
       AsyncAuxVars, vars

THEOREM EnabledDecisionOwnerImpliesCommandExecutionReady ==
  \A node, qc, candidate:
    /\ AsyncStrongTypeInvariant
    /\ DecisionsUniqueByNodeContext
    /\ DecisionExecutableStageOwner(node, qc, candidate)
    => CommandExecutionReady(candidate)
BY DecisionExecutableStageOwnerEnablesExecution, Isa
   DEF CommandExecutionReady, ExecuteCommand

THEOREM DecisionExecutableStageOwnerIsDispatchable ==
  \A node, qc, candidate:
    /\ AsyncStrongTypeInvariant
    /\ DecisionsUniqueByNodeContext
    /\ DecisionExecutableStageOwner(node, qc, candidate)
    => CommandDispatchable(candidate)
PROOF
  <1>1. ASSUME NEW node, NEW qc, NEW candidate,
                AsyncStrongTypeInvariant,
                DecisionsUniqueByNodeContext,
                DecisionExecutableStageOwner(node, qc, candidate)
         PROVE CommandDispatchable(candidate)
    <2>1. AsyncCandidateTyped(candidate)
      BY <1>1, Isa
         DEF DecisionExecutableStageOwner, AsyncCandidateSet,
             AsyncCandidateTyped
    <2>2. /\ CandidateConsumerCurrent(candidate)
           /\ candidate.class = "Completion"
      BY <1>1
         DEF DecisionExecutableStageOwner,
             DecisionPipelineCandidate
    <2>3. CommandExecutionReady(candidate)
      BY <1>1, EnabledDecisionOwnerImpliesCommandExecutionReady
    <2> QED BY <2>1, <2>2, <2>3 DEF CommandDispatchable
  <1> QED BY <1>1

(***************************************************************************
Exact carrier removal and successor scheduling.

The logical-ownership invariant supplies unique sequence occurrences.  A
serialized parent is not one of its successors, so removing that parent
retains every pre-scheduled successor.  Fresh successors are appended to the
causal queue.  These facts are stated for the complete Fifo and deferred
actions, not for the under-framed queue update helpers in isolation.
***************************************************************************)

CommandSuccessorsScheduledAfter(command) ==
  \A successor \in SequenceSet(CommandSuccessors(command)):
    CandidateScheduled(successor)'

THEOREM FifoSuccessfulExecutionSchedulesEverySuccessor ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ NodeQueueNonempty(node)
    /\ CommandDispatchable(NextNodeCommand(node))
    /\ FifoRuntimeStep(node)
    => CommandSuccessorsScheduledAfter(NextNodeCommand(node))
BY FreshCommandSuccessorsAreUnscheduled,
   CommandSuccessorParentDisjoint,
   SequenceWithoutIndexRetainsOtherValue,
   RangeConcatenation, RangeEquality,
   SequenceSetAfterAppend, IsaT(300)
   DEF CommandSuccessorsScheduledAfter,
       FifoRuntimeStep, RemoveNextNodeCommand,
       AppendCausalSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncCausalTypeInvariant,
       AsyncCommandQueueOwnership, AsyncCausalQueueOwnership,
       SequenceHasUniqueValues, SequenceSet

THEOREM DeferredSuccessfulExecutionSchedulesEverySuccessor ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DeferredWorkServiceable(node)
    /\ CommandDispatchable(NextDeferredCommand(node))
    /\ DeferredDrainStep(node)
    => CommandSuccessorsScheduledAfter(NextDeferredCommand(node))
BY FreshCommandSuccessorsAreUnscheduled,
   CommandSuccessorParentDisjoint,
   TailRetainsNonHeadValue,
   RangeConcatenation, RangeEquality,
   SequenceSetAfterAppend, IsaT(300)
   DEF CommandSuccessorsScheduledAfter,
       DeferredDrainStep, RemoveNextDeferredCommand,
       AdvanceNextDeferredClass, DeferredClassQueue,
       AppendCausalSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncCausalTypeInvariant,
       AsyncDeferredTypeInvariant, AsyncDeferredContentTypeInvariant,
       AsyncCommandQueueOwnership, AsyncCausalQueueOwnership,
       SequenceHasUniqueValues, SequenceSet

THEOREM SelectedExactFifoOwnerCannotDeferOrDiscard ==
  \A node \in ValidatorIds:
    \A qc:
      LET command == NextNodeCommand(node)
      IN /\ AsyncStrongTypeInvariant
         /\ DecisionsUniqueByNodeContext
         /\ DecisionExecutableStageOwner(node, qc, command)
         /\ FifoRuntimeStep(node)
         => /\ CommandDispatchable(command)
            /\ ExecuteCommand(command)
            /\ AppendCausalSuccessors(command)
BY DecisionExecutableStageOwnerIsDispatchable, Isa
   DEF FifoRuntimeStep

THEOREM SelectedExactDeferredOwnerCannotDiscard ==
  \A node \in ValidatorIds:
    \A qc:
      LET command == NextDeferredCommand(node)
      IN /\ AsyncStrongTypeInvariant
         /\ DecisionsUniqueByNodeContext
         /\ DecisionExecutableStageOwner(node, qc, command)
         /\ DeferredDrainStep(node)
         => /\ CommandDispatchable(command)
            /\ ExecuteCommand(command)
            /\ AppendCausalSuccessors(command)
BY DecisionExecutableStageOwnerIsDispatchable, Isa
   DEF DeferredDrainStep

(***************************************************************************
Semantic handoffs.

Each successor-producing lemma consumes `CommandSuccessorsScheduledAfter`.
This keeps semantic state changes independent of the particular runtime
carrier which removed the parent.
***************************************************************************)

THEOREM PersistDecisionCreatesExactClassifiedStage ==
  \A command, request:
    /\ AsyncStrongTypeInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ request \in pendingDecision
    /\ command.kind = "PersistDecision"
    /\ CommandMatches(command, request.node, request.qc.view,
                      request.qc.subject)
    /\ CandidateConsumerCurrent(command)
    /\ PersistDecision(request)
    /\ CommandSuccessorsScheduledAfter(command)
    => DecisionRecoveryStageExact(request.node, request.qc)'
BY PersistDecisionRecoveryUsesBodyStateCompletion, IsaT(240)
   DEF CommandSuccessorsScheduledAfter,
       PersistDecision, PersistDecisionRequests,
       PersistDecisionRequest, PersistDecisionBody,
       PersistDecisionValidationHeld,
       PersistDecisionRecoveryKind,
       PersistDecisionRecoverySuccessor,
       CommandSuccessors, DecisionRecoveryStageExact,
       DecisionFetchBodyOwnedExact,
       DecisionCertifiedRequestActiveExact,
       DecisionCertifiedFetchOwnedExact,
       DecisionStoreBodyOwned, DecisionValidateBodyOwned,
       DecisionApplyOwned, DecisionPipelineKindOwned,
       DecisionPipelineCandidate, DecisionValidationHeld, DecisionBody,
       CandidateConsumerCurrent, CandidateScheduled,
       DecisionFrontierUniquenessInvariant,
       PendingDecisionExcludesDurableDecision,
       AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety,
       OnePendingPersistencePerNode, RequestsUniqueByNode,
       AllPendingRequests, RequestNodeSet

THEOREM ExactDecisionFetchMissingBodyOpensCertifiedRequest ==
  \A node, qc, command:
    /\ AsyncStrongTypeInvariant
    /\ DecisionsUniqueByNodeContext
    /\ DecisionExecutableStageOwner(node, qc, command)
    /\ command.kind = "FetchBody"
    /\ ~BodyHeldBy(durableBodies, node, qc.context,
                    qc.view, qc.subject)
    /\ ExecuteDecisionFetch(command)
    => DecisionCertifiedRequestActiveExact(node, qc)'
BY DecisionRecoveryCertificateHasRemoteBodySource,
   IsaT(240)
   DEF DecisionExecutableStageOwner,
       DecisionCertifiedRequestActiveExact,
       DecisionRecoveryCertificate,
       ExecuteDecisionFetch, PublishCertifiedRequests,
       CertifiedRequestOutbox, CertifiedRecoveryFetchFrontier,
       DecisionFetchFrontier,
       DecisionCertifiedBodyRecoveryAuthority,
       CertifiedBodyRecoveryAuthority, HistoricalLockedPrepareSource,
       CommandMatches, AsyncAuxVars, vars

THEOREM ExactDecisionFetchHeldBodySchedulesValidation ==
  \A node, qc, command:
    /\ DecisionExecutableStageOwner(node, qc, command)
    /\ command.kind = "FetchBody"
    /\ BodyHeldBy(durableBodies, node, qc.context,
                   qc.view, qc.subject)
    /\ ExecuteDecisionFetch(command)
    /\ CommandSuccessorsScheduledAfter(command)
    => /\ BodyHeldBy(durableBodies', node, qc.context',
                     qc.view, qc.subject)
       /\ ~DecisionValidationHeld(node, qc)'
       /\ DecisionValidateBodyOwned(node, qc)'
BY IsaT(180)
   DEF DecisionExecutableStageOwner,
       CommandSuccessorsScheduledAfter,
       ExecuteDecisionFetch, CommandSuccessors, CausalCandidate,
       AsyncCandidateFrom,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateSuccessorSemanticPhase,
       AsyncCandidateSuccessorProposalRound,
       AsyncCandidateWithIdentityAndOrigin,
       DecisionValidateBodyOwned, DecisionPipelineKindOwned,
       DecisionPipelineCandidate, DecisionValidationHeld,
       CandidateConsumerCurrent

THEOREM ExactCertifiedFetchStagesBodyAndSchedulesStore ==
  \A node, qc, command:
    /\ AsyncStrongTypeInvariant
    /\ DecisionsUniqueByNodeContext
    /\ DecisionExecutableStageOwner(node, qc, command)
    /\ command.kind = "FetchCertifiedBody"
    /\ ExecuteCommand(command)
    /\ CommandSuccessorsScheduledAfter(command)
    => /\ DecisionBody(node, qc)' \in availableBodies'
       /\ ~BodyHeldBy(durableBodies', node, qc.context',
                      qc.view, qc.subject)
       /\ DecisionStoreBodyOwned(node, qc)'
BY DecisionNodeExcludesLockedPrepareRecoveryAuthority, IsaT(240)
   DEF DecisionExecutableStageOwner,
       DecisionCertifiedResponseLineageExact,
       CommandSuccessorsScheduledAfter,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       FetchCertifiedBody, AcceptCertifiedResponseCapability,
       InstallCertifiedBodyEffect, CertifiedBodyRecoveryAuthority,
       DecisionCertifiedBodyRecoveryAuthority,
       HistoricalLockedPrepareSource, CommandMatches,
       CommandSuccessors, CausalCandidate, AsyncCandidateFrom,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateSuccessorSemanticPhase,
       AsyncCandidateSuccessorProposalRound,
       AsyncCandidateWithIdentityAndOrigin,
       DecisionBody, DecisionStoreBodyOwned,
       DecisionPipelineKindOwned, DecisionPipelineCandidate,
       CandidateConsumerCurrent

THEOREM DecisionStoreSchedulesValidation ==
  \A node, qc, command:
    /\ DecisionExecutableStageOwner(node, qc, command)
    /\ command.kind = "StoreBody"
    /\ ExecuteCommand(command)
    /\ CommandSuccessorsScheduledAfter(command)
    => /\ BodyHeldBy(durableBodies', node, qc.context',
                     qc.view, qc.subject)
       /\ ~DecisionValidationHeld(node, qc)'
       /\ DecisionValidateBodyOwned(node, qc)'
BY IsaT(180)
   DEF DecisionExecutableStageOwner,
       CommandSuccessorsScheduledAfter,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       StoreBody, CommandSuccessors, CausalCandidate,
       AsyncCandidateFrom,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateSuccessorSemanticPhase,
       AsyncCandidateSuccessorProposalRound,
       AsyncCandidateWithIdentityAndOrigin,
       DecisionValidateBodyOwned, DecisionPipelineKindOwned,
       DecisionPipelineCandidate, DecisionValidationHeld,
       CandidateConsumerCurrent

THEOREM DecisionValidationSchedulesApply ==
  \A node, qc, command:
    /\ AsyncStrongTypeInvariant
    /\ DecisionsUniqueByNodeContext
    /\ DecisionExecutableStageOwner(node, qc, command)
    /\ command.kind = "ValidateBody"
    /\ ExecuteCommand(command)
    /\ CommandSuccessorsScheduledAfter(command)
    => /\ BodyHeldBy(durableBodies', node, qc.context',
                     qc.view, qc.subject)
       /\ DecisionValidationHeld(node, qc)'
       /\ DecisionApplyOwned(node, qc)'
BY ValidationCommandSelectsValidationAction,
   DecisionNodeExcludesLockedPrepareRecoveryAuthority,
   IsaT(300)
   DEF DecisionExecutableStageOwner,
       CommandSuccessorsScheduledAfter,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       ValidateBody, ValidateDecidedBody, ValidateLockedBody, RejectBody,
       CommandMatches, CommandSuccessors, CausalCandidate,
       AsyncCandidateFrom,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateSuccessorSemanticPhase,
       AsyncCandidateSuccessorProposalRound,
       AsyncCandidateWithIdentityAndOrigin,
       DecisionApplyOwned, DecisionPipelineKindOwned,
       DecisionPipelineCandidate, DecisionValidationHeld,
       CandidateConsumerCurrent,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, DecisionAgreement

THEOREM DecidedAssembleSchedulesApply ==
  \A node, qc, command:
    /\ AsyncStrongTypeInvariant
    /\ [node |-> node, qc |-> qc] \in decisions
    /\ qc.context = context
    /\ qc.phase = "Commit"
    /\ command.kind = "AssembleBody"
    /\ CommandMatches(command, node, qc.view, qc.subject)
    /\ CandidateConsumerCurrent(command)
    /\ ExecuteCommand(command)
    /\ CommandSuccessorsScheduledAfter(command)
    => /\ BodyHeldBy(durableBodies', node, qc.context',
                     qc.view, qc.subject)
       /\ DecisionValidationHeld(node, qc)'
       /\ DecisionApplyOwned(node, qc)'
BY IsaT(240)
   DEF CommandSuccessorsScheduledAfter,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       AssembleLocalBody, ExactDecidedLocalBody,
       CommandSuccessors, CausalCandidate,
       AsyncCandidateFrom,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateSuccessorSemanticPhase,
       AsyncCandidateSuccessorProposalRound,
       AsyncCandidateWithIdentityAndOrigin,
       DecisionApplyOwned, DecisionPipelineKindOwned,
       DecisionPipelineCandidate, DecisionValidationHeld,
       CandidateConsumerCurrent, CommandMatches

THEOREM DecisionApplyCreatesTerminalStage ==
  \A node, qc, command:
    /\ DecisionsUniqueByNodeContext
    /\ DecisionExecutableStageOwner(node, qc, command)
    /\ command.kind = "Apply"
    /\ ExecuteCommand(command)
    => /\ NodeHasApplication(node)'
       /\ DecisionRecoveryStageExact(node, qc)'
BY IsaT(180)
   DEF DecisionExecutableStageOwner,
       ExecuteCommand, ExecuteApply, ApplyDecision,
       NodeHasApplication, DecisionsUniqueByNodeContext,
       DecisionRecoveryStageExact

(***************************************************************************
PersistInstallTC may clear validation and advance the consumer generation,
but DecisionTimeoutFrontierInvariant proves that its pending request cannot
target a node which already owns a durable Decision.  This is the precise
exclusion needed by the Decision-stage induction.
***************************************************************************)

THEOREM DurableDecisionNodeCannotOwnPendingInstall ==
  \A node, qc, request:
    /\ DecisionTimeoutFrontierInvariant
    /\ [node |-> node, qc |-> qc] \in decisions
    /\ qc.context = context
    /\ request \in pendingInstallTC
    /\ request.node = node
    => FALSE
BY Isa
   DEF DecisionTimeoutFrontierInvariant,
       PendingInstallExcludesDecision, NoDecisionForNode

THEOREM ExecutePersistInstallCannotTargetCurrentDecision ==
  \A node, qc, command:
    /\ DecisionTimeoutFrontierInvariant
    /\ [node |-> node, qc |-> qc] \in decisions
    /\ qc.context = context
    /\ ExecutePersistInstall(command)
    => command.node # node
BY DurableDecisionNodeCannotOwnPendingInstall, Isa
   DEF ExecutePersistInstall

(***************************************************************************
Carrier-neutral exact-stage frame.  It is used for unrelated commands and
non-runner transitions after those transitions have established the exact
retention clauses rather than merely claiming a scheduler stutter.  The
combined ordinary-voter/historical-target owner set may only shrink; opening
a new historical target is handled separately by the final closure module.
The authenticated sent history is append-only: new responses may be
published, but an exact response witness already used by the Decision
pipeline cannot disappear.
***************************************************************************)

DecisionExactCertifiedRequestsRetained ==
  \A node, qc:
    DecisionCertifiedRequestActiveExact(node, qc)
      => DecisionCertifiedRequestActiveExact(node, qc)'

DecisionExactScheduledCandidatesRetained ==
  \A node, qc, candidate:
    DecisionExecutableStageOwner(node, qc, candidate)
      => CandidateScheduled(candidate)'

DecisionExactAuthenticatedHistoryRetained ==
  asyncSentItems \subseteq asyncSentItems'

DecisionExactRetentionFrame ==
  /\ UNCHANGED <<context, nodeView, generation, decisions, applied,
                 availableBodies, durableBodies, validatedBodies,
                 AsyncRecoveryVars>>
  /\ (AsyncCurrentResponsiveVoters'
        \cup asyncHistoricalRecoveryTargets')
       \subseteq
         (AsyncCurrentResponsiveVoters
            \cup asyncHistoricalRecoveryTargets)
  /\ DecisionExactAuthenticatedHistoryRetained
  /\ DecisionExactCertifiedRequestsRetained
  /\ DecisionExactScheduledCandidatesRetained

THEOREM DecisionExactRetentionFramePreservesSource ==
  /\ DecisionExactSourceRetentionInvariant
  /\ DecisionExactRetentionFrame
  => DecisionExactSourceRetentionInvariant'
BY IsaT(180)
   DEF DecisionExactSourceRetentionInvariant,
       DecisionExactSourceOwner, HistoricalRecoveryTarget,
       AsyncDecisionRecoveryStageExact,
       DecisionRecoveryStageExact,
       DecisionFetchBodyOwnedExact,
       DecisionCertifiedRequestActiveExact,
       DecisionCertifiedResponseLineageExact,
       DecisionCertifiedFetchOwnedExact,
       DecisionStoreBodyOwned, DecisionValidateBodyOwned,
       DecisionApplyOwned,
       DecisionExecutableStageOwner,
       DecisionExactRetentionFrame,
       DecisionExactAuthenticatedHistoryRetained,
       DecisionExactCertifiedRequestsRetained,
       DecisionExactScheduledCandidatesRetained,
       CertifiedResponseAuthenticatedOccurrence,
       CertifiedResponseCapabilityAuthorized,
       MatchingSentCertifiedRequests,
       FrozenCertifiedResponseBinding,
       FrozenCertifiedRequestRegistration,
       AsyncCertifiedResponseAuthProjection,
       DecisionPipelineKindOwned, DecisionPipelineCandidate,
       DecisionValidationHeld, DecisionBody,
       DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       CandidateConsumerCurrent

(***************************************************************************
Local admission is an exact owner relocation.  The imported set-equality
theorem covers queued, deferred, causal, and tracked-work carriers together;
Core state, active certified requests, and recovery authority are framed.
***************************************************************************)

THEOREM LocalAdmissionPreservesDecisionExactSourceRetention ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ DecisionExactSourceRetentionInvariant
    /\ LocalAdmissionStep(node)
    => DecisionExactSourceRetentionInvariant'
BY LocalAdmissionPreservesScheduledCandidateSet, IsaT(120)
   DEF DecisionExactSourceRetentionInvariant,
       DecisionExactSourceOwner, HistoricalRecoveryTarget,
       AsyncDecisionRecoveryStageExact,
       DecisionRecoveryStageExact,
       DecisionFetchBodyOwnedExact,
       DecisionCertifiedRequestActiveExact,
       DecisionCertifiedResponseLineageExact,
       DecisionCertifiedFetchOwnedExact,
       DecisionStoreBodyOwned, DecisionValidateBodyOwned,
       DecisionApplyOwned, DecisionPipelineKindOwned,
       DecisionPipelineCandidate, DecisionValidationHeld, DecisionBody,
       DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       CertifiedResponseAuthenticatedOccurrence,
       CertifiedResponseCapabilityAuthorized,
       MatchingSentCertifiedRequests,
       FrozenCertifiedResponseBinding,
       FrozenCertifiedRequestRegistration,
       AsyncCertifiedResponseAuthProjection,
       ScheduledCandidateSet, CandidateScheduled,
       LocalAdmissionStep, AdmitProducerCompletion, AdmitCausalHead,
       AsyncRecoveryVars, vars

THEOREM SerializedLocalPredecessorPreservesDecisionExactSourceRetention ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionExactSourceRetentionInvariant
    /\ SerializedLocalPrecedesServeIngressStep(node)
    => DecisionExactSourceRetentionInvariant'
BY SelectedLocalAdmissionAdvancePreservesScheduledCandidateSet,
   IsaT(120)
   DEF DecisionExactSourceRetentionInvariant,
       DecisionExactSourceOwner, HistoricalRecoveryTarget,
       AsyncDecisionRecoveryStageExact,
       DecisionRecoveryStageExact,
       DecisionFetchBodyOwnedExact,
       DecisionCertifiedRequestActiveExact,
       DecisionCertifiedResponseLineageExact,
       DecisionCertifiedFetchOwnedExact,
       DecisionStoreBodyOwned, DecisionValidateBodyOwned,
       DecisionApplyOwned, DecisionPipelineKindOwned,
       DecisionPipelineCandidate, DecisionValidationHeld, DecisionBody,
       DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       CertifiedResponseAuthenticatedOccurrence,
       CertifiedResponseCapabilityAuthorized,
       MatchingSentCertifiedRequests,
       FrozenCertifiedResponseBinding,
       FrozenCertifiedRequestRegistration,
       AsyncCertifiedResponseAuthProjection,
       ScheduledCandidateSet, CandidateScheduled,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AdmitProducerCompletion, AdmitCausalHead,
       AsyncRecoveryVars, vars

(***************************************************************************
Authenticated ingress has one owner-moving case.  Admitting a valid
CertifiedResponse retires matching logical registrations and installs the
exact FetchCertifiedBody candidate.  Retirement is by the exact signed-request
hash.  The response separately binds its physical archive/signature owner and
its cited Decision-QC signer; the outer source remains a transport relay and
is retained only as part of the complete candidate evidence.  A node with a
Decision cannot use the locked-Prepare authority branch, so a response which
retires an exact Decision request installs an exact Decision response owner.
All other ingress branches retain the existing request or scheduled owner.
***************************************************************************)

THEOREM DrainFairIngressPreservesDecisionExactSourceRetention ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionExactSourceRetentionInvariant
    /\ DrainFairIngressSelected(node)
    => DecisionExactSourceRetentionInvariant'
BY DecisionNodeExcludesLockedPrepareRecoveryAuthority,
   CertifiedResponseClaimAuthorizationSuppliesFrozenCapability,
   ExactCertifiedResponseMatchesDecisionRequestHash,
   ExactCertifiedResponseCandidateRetainsOuterItem,
   SequenceWithoutIndexRetainsOtherValue,
   SequenceSetAfterAppend, IsaT(420)
   DEF DecisionExactSourceRetentionInvariant,
       DecisionExactSourceOwner, HistoricalRecoveryTarget,
       AsyncDecisionRecoveryStageExact,
       DecisionRecoveryStageExact,
       DecisionFetchBodyOwnedExact,
       DecisionCertifiedRequestActiveExact,
       DecisionCertifiedResponseLineageExact,
       DecisionCertifiedFetchOwnedExact,
       DecisionStoreBodyOwned, DecisionValidateBodyOwned,
       DecisionApplyOwned, DecisionPipelineKindOwned,
       DecisionPipelineCandidate, DecisionValidationHeld, DecisionBody,
       DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       DecisionFrontierUniquenessInvariant,
       DecisionsUniqueByNodeContext,
       PendingDecisionExcludesDurableDecision,
       DrainFairIngressSelected, PopSelectedIngress,
       IngressItemCanDrain, DeliveryCandidate,
       CertifiedResponseAuthorized, MatchingCertifiedRequests,
       CertifiedRequestAuthorized,
       CertifiedResponseAuthenticatedOccurrence,
       CertifiedResponseCapabilityAuthorized,
       MatchingSentCertifiedRequests,
       FrozenCertifiedResponseBinding,
       FrozenCertifiedRequestRegistration,
       AsyncCertifiedResponseAuthProjection,
       CertifiedRecoveryFetchFrontier,
       DecisionCertifiedBodyRecoveryAuthority,
       CertifiedBodyRecoveryAuthority, HistoricalLockedPrepareSource,
       CertifiedResponseCandidate, AsyncCandidate,
       AsyncCandidateWithIdentity,
       EnqueueCandidate, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncIoVars, AsyncRecoveryVars, SequenceSet, vars

THEOREM IngressDrainPreservesDecisionExactSourceRetention ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionExactSourceRetentionInvariant
    /\ IngressDrainStep(node)
    => DecisionExactSourceRetentionInvariant'
BY DrainFairIngressPreservesDecisionExactSourceRetention, IsaT(180)
   DEF IngressDrainStep, AsyncRecoveryVars, vars

(***************************************************************************
Serialized runtime closure.

The two selected-owner lemmas exclude Defer/Discard for an exact Completion
owner.  The two successor-scheduling lemmas retain every unselected owner and
install every fresh child.  The semantic handoffs above cover the only Core
state changes which can move a Decision through the body pipeline.
PersistInstallTC is harmless because it cannot target the Decision node.
***************************************************************************)

THEOREM SerializedRuntimePreservesDecisionExactSourceRetention ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ DecisionExactSourceRetentionInvariant
    /\ SerializedRunnerRuntimeStep(node)
    => DecisionExactSourceRetentionInvariant'
BY SelectedExactFifoOwnerCannotDeferOrDiscard,
   SelectedExactDeferredOwnerCannotDiscard,
   FifoSuccessfulExecutionSchedulesEverySuccessor,
   DeferredSuccessfulExecutionSchedulesEverySuccessor,
   PersistDecisionCreatesExactClassifiedStage,
   ExactDecisionFetchMissingBodyOpensCertifiedRequest,
   ExactDecisionFetchHeldBodySchedulesValidation,
   ExactCertifiedFetchStagesBodyAndSchedulesStore,
   DecisionStoreSchedulesValidation,
   DecisionValidationSchedulesApply,
   DecidedAssembleSchedulesApply,
   DecisionApplyCreatesTerminalStage,
   ExecutePersistInstallCannotTargetCurrentDecision,
   CertifiedRequestOutboxDecisionSurvivalIsExactTarget,
   PersistDecisionControlRetainsExactlySurvivingRequests,
   SequenceWithoutIndexRetainsOtherValue,
   TailRetainsNonHeadValue, IsaT(600)
   DEF DecisionExactSourceRetentionInvariant,
       DecisionExactSourceOwner, HistoricalRecoveryTarget,
       AsyncDecisionRecoveryStageExact,
       DecisionRecoveryStageExact,
       DecisionFetchBodyOwnedExact,
       DecisionCertifiedRequestActiveExact,
       DecisionCertifiedResponseLineageExact,
       DecisionCertifiedFetchOwnedExact,
       DecisionStoreBodyOwned, DecisionValidateBodyOwned,
       DecisionApplyOwned, DecisionPipelineKindOwned,
       DecisionPipelineCandidate, DecisionValidationHeld, DecisionBody,
       DecisionExecutableStageOwner,
       DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       DecisionFrontierUniquenessInvariant,
       DecisionsUniqueByNodeContext,
       PendingDecisionExcludesDurableDecision,
       DecisionTimeoutFrontierInvariant,
       PendingInstallExcludesDecision,
       CommandSuccessorsScheduledAfter,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DeferredTimeoutStep,
       DeferredRetransmitStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       DeferCommand, DiscardCommand,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       ExecuteDecisionFetch, ExecutePersistInstall,
       ExecutePersistDecision, PersistDecisionControl,
       CertifiedRequestSurvivesDecision,
       FilterCertifiedResponseAuthority, ExecuteApply,
       AppendCausalSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, CommandSuccessors,
       PersistDecisionRecoverySuccessor,
       PersistDecisionRecoveryKind, PersistDecisionBody,
       PersistDecisionValidationHeld, PersistDecisionRequest,
       PersistDecisionRequests,
       CandidateScheduled, CandidateConsumerCurrent,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, AsyncRecoveryVars, SequenceSet, vars

THEOREM RunNodeWorkPreservesDecisionExactSourceRetention ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ DecisionExactSourceRetentionInvariant
    /\ RunNodeWork(node)
    => DecisionExactSourceRetentionInvariant'
BY LocalAdmissionPreservesDecisionExactSourceRetention,
   SerializedLocalPredecessorPreservesDecisionExactSourceRetention,
   IngressDrainPreservesDecisionExactSourceRetention,
   SerializedRuntimePreservesDecisionExactSourceRetention, Isa
   DEF RunNodeWork, SerializedRunnerRuntimeStep,
       SerializedLocalPrecedesServeIngressStep,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncCandidateProducerContinuationExactReplayIdentity,
       AsyncCandidateProducerContinuationSelectedLocalCandidate,
       AsyncCandidateProducerContinuationSelectedRuntimeCandidate,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationSelectedResolutionRecord,
       AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationConcreteSuccessorOwned,
       AsyncCandidateProducerContinuationHandoffOwned,
       AsyncCandidateProducerContinuationLocalReplayCarrier,
       AsyncSchedulerExceptCausalControlAndNodeService,
       AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
       AsyncSchedulerExceptCausalControlRunnerAndNodeService,
       EnqueueCandidate, CandidateScheduled, ScheduledCandidateSet,
       AsyncRecoveryVars, SequenceSet, vars

THEOREM AsyncRunnerPreservesDecisionExactSourceRetention ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ DecisionExactSourceRetentionInvariant
  /\ AsyncRunnerStep
  => DecisionExactSourceRetentionInvariant'
BY RunNodeWorkPreservesDecisionExactSourceRetention, IsaT(120)
   DEF AsyncRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunHistoricalServer, HistoricalRecoveryTarget,
       DrainHistoricalIngressSelected, HistoricalIdleStep,
       AsyncRecoveryVars, vars

THEOREM CoreBracketedAsyncRunnerPreservesFrontierAndExactStage ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ DecisionFrontierAndExactStageInvariant
  /\ AsyncRunnerStep
  /\ [Next]_vars
  => DecisionFrontierAndExactStageInvariant'
BY AsyncRunnerPreservesDecisionExactSourceRetention,
   CoreBracketPreservesDecisionFrontierUniqueness, Isa
   DEF AsyncStrongTypeInvariant,
       DecisionFrontierAndExactStageInvariant

(***************************************************************************
Exact remaining frontier.

The static vocabulary, projection, initialization, exact enabled/dispatch
facts, carrier removal, successor scheduling, all seven semantic handoffs,
authenticated ingress, SerializedRuntimeStep, and RunNode/runner preservation
are stated above with proof scripts and no assumed action lemma.  The final
runner theorem also conjoins the exact stage with full durable/pending
Decision-frontier uniqueness under AsyncNext's explicit Core bracket.

This module deliberately stops before an AsyncNext/bracketed temporal
obligation.  The remaining proof is the recovery/non-runner aggregation:

  * ordinary non-runner transport and I/O service must be collected under an
    exact request/candidate retention frame;
  * responsive Crash/Restart/Replay must be lifted from the imported exact
    replay Fetch theorem into DecisionExactSourceRetentionInvariant;
  * DriveResponsiveReplayHead, FinishResponsiveReplay, and Rearm must be
    collected without weakening exact Fetch evidence; and
  * those results must be conjoined with the already-proved frontier,
    timeout-frontier, strong-type, and progress-ownership inductions.

No theorem below this comment claims that aggregation.
***************************************************************************)

=============================================================================
