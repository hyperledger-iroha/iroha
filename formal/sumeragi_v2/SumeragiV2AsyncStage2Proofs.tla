---- MODULE SumeragiV2AsyncStage2Proofs ----
EXTENDS SumeragiV2AsyncStage6Proofs

(***************************************************************************
Stage 2: Busy reducer termination and exact deferred handoff.

The local phase kernel, exact handoff token, post-deferred convergence, and
leaf-composition boundaries are kept with the production aggregate so no
scratch-only theorem can stand in for rank progress.
***************************************************************************)

Stage2TwoStepBusyOwners ==
  pendingProposal \cup pendingPrepare \cup pendingLockCommit
    \cup pendingTimeout \cup pendingInstallTC

Stage2OneStepBusyOwners ==
  pendingObservePrepare \cup pendingDecision
    \cup signProposals \cup signVotes \cup signTimeouts

Stage2TwoStepBusyNodes == RequestNodeSet(Stage2TwoStepBusyOwners)

Stage2OneStepBusyNodes == RequestNodeSet(Stage2OneStepBusyOwners)

BusyPhaseCarrier == 0..2

BusyPhaseRank(node) ==
  IF node \in Stage2TwoStepBusyNodes
  THEN 2
  ELSE IF node \in Stage2OneStepBusyNodes THEN 1 ELSE 0

Stage2TwoStepCompletionKinds ==
  {"PersistProposal", "PersistPrepare", "PersistLockCommit",
   "PersistTimeout", "PersistInstallTC"}

Stage2OneStepCompletionKinds ==
  {"PersistObservePrepare", "PersistDecision",
   "SignProposal", "SignVote", "SignTimeout"}

(***************************************************************************
The serialized owner set currently stores the Core request values without a
lane tag.  `ProposalWal`/`ProposalSign` and `TimeoutWal`/`TimeoutSign` are
therefore structurally equal, so set-level node uniqueness alone does not
exclude simultaneous pending and signing ownership.  The production model
also does not yet expose the readiness/provenance guards below as a named
invariant.  Keep both requirements explicit in this production proof: omitting
either admits a concrete counterexample to the Busy rank kernel.
***************************************************************************)

Stage2BusyPhaseSeparated ==
  Stage2TwoStepBusyNodes \cap Stage2OneStepBusyNodes = {}

Stage2BusyCompletionGuards ==
  /\ \A request \in pendingProposal:
       /\ request.proposal.proposer = request.node
       /\ request.proposal \notin proposalIntents
  /\ \A request \in pendingPrepare:
       request.vote \notin prepareIntents
  /\ \A request \in pendingLockCommit:
       request.vote \notin commitIntents
  /\ \A request \in pendingTimeout:
       request.vote \notin timeoutIntents
  /\ \A request \in signProposals:
       request.proposal.proposer = request.node
  /\ \A request \in signVotes:
       /\ request.vote.signer = request.node
       /\ VoteRoundAdmissible(request.node, request.vote)
  /\ \A request \in signTimeouts:
       request.vote.signer = request.node

Stage2BusyKernelInvariant ==
  /\ Stage2BusyPhaseSeparated
  /\ Stage2BusyCompletionGuards

Stage2BusyKernelProperty(specification) ==
  specification => []Stage2BusyKernelInvariant

(***************************************************************************
Exact serialized-owner partition.

`SerializedBusyOwnershipInvariant` excludes distinct same-node owner values;
`Stage2BusyPhaseSeparated` additionally excludes the equal-record pending /
signing alias.  Both are required: without either condition, executing one
completion need not lower the node rank.
***************************************************************************)

THEOREM BusyPhaseOwnerPartitionObligation ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyPhaseSeparated
    => /\ SerializedBusyOwners =
             Stage2TwoStepBusyOwners \cup Stage2OneStepBusyOwners
       /\ BusyPhaseRank(node) \in BusyPhaseCarrier
       /\ (BusyPhaseRank(node) = 0 <=> NodeIdle(node))
       /\ (BusyPhaseRank(node) = 2
             <=> node \in Stage2TwoStepBusyNodes)
       /\ (BusyPhaseRank(node) = 1
             <=> node \in Stage2OneStepBusyNodes)
BY Isa
   DEF Stage2BusyPhaseSeparated,
       AsyncProgressOwnershipInvariant,
       SerializedBusyOwnershipInvariant, SerializedBusyOwners,
       Stage2TwoStepBusyOwners, Stage2OneStepBusyOwners,
       Stage2TwoStepBusyNodes, Stage2OneStepBusyNodes,
       BusyPhaseRank, BusyPhaseCarrier, NodeIdle,
       PendingNodes, SigningNodes, AllPendingRequests,
       RequestNodeSet, RequestsUniqueByNode

THEOREM BusyCompletionKindMatchesPhaseObligation ==
  \A node \in ValidatorIds:
    \A witness \in BusyCompletionCandidates(node):
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ Stage2BusyPhaseSeparated
      => /\ (BusyPhaseRank(node) = 2
               => witness.kind \in Stage2TwoStepCompletionKinds)
         /\ (BusyPhaseRank(node) = 1
               => witness.kind \in Stage2OneStepCompletionKinds)
BY BusyPhaseOwnerPartitionObligation, Isa
   DEF Stage2BusyPhaseSeparated,
       AsyncProgressOwnershipInvariant,
       SerializedBusyOwnershipInvariant,
       BusyCompletionCandidates,
       Stage2TwoStepBusyOwners, Stage2OneStepBusyOwners,
       Stage2TwoStepBusyNodes, Stage2OneStepBusyNodes,
       Stage2TwoStepCompletionKinds, Stage2OneStepCompletionKinds,
       BusyPhaseRank, RequestNodeSet, RequestsUniqueByNode

(***************************************************************************
Concrete Core transition kernel.

The rank-two cases are exactly:

  PersistProposal       pendingProposal   -> signProposals
  PersistPrepare        pendingPrepare    -> signVotes
  PersistLockCommit     pendingLockCommit -> signVotes
  PersistTimeout        pendingTimeout    -> signTimeouts
  PersistInstallTC      pendingInstallTC  -> signVotes or idle

The rank-one cases remove pendingObservePrepare, pendingDecision, or the
matching signature request and install no new Busy owner.  Thus execution of
the authenticated matching completion strictly changes 2 -> {1, 0} or
1 -> 0.  This is the local-work termination fact required by stage 2; merely
removing the scheduler occurrence is not a substitute for it.
***************************************************************************)

BusyCompletionExecution(node, witness) ==
  /\ node \in ValidatorIds
  /\ BusyPhaseRank(node) \in 1..2
  /\ witness \in BusyCompletionCandidates(node)
  /\ ExecuteCommand(witness)

THEOREM BusyCompletionExecutionDropsPhaseObligation ==
  \A node, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyPhaseSeparated
    /\ BusyCompletionExecution(node, witness)
    => /\ BusyPhaseRank(node)' \in 0..1
       /\ BusyPhaseRank(node)' < BusyPhaseRank(node)
       /\ (BusyPhaseRank(node) = 1 => BusyPhaseRank(node)' = 0)
       /\ (BusyPhaseRank(node) = 2
             => BusyPhaseRank(node)' \in 0..1)
PROOF
  <1>1. ASSUME NEW node, NEW witness,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                Stage2BusyPhaseSeparated,
                BusyCompletionExecution(node, witness)
         PROVE /\ BusyPhaseRank(node)' \in 0..1
               /\ BusyPhaseRank(node)' < BusyPhaseRank(node)
               /\ (BusyPhaseRank(node) = 1
                     => BusyPhaseRank(node)' = 0)
               /\ (BusyPhaseRank(node) = 2
                     => BusyPhaseRank(node)' \in 0..1)
    <2>1. witness.kind
             \in Stage2TwoStepCompletionKinds
                  \cup Stage2OneStepCompletionKinds
      BY <1>1, BusyCompletionKindMatchesPhaseObligation, Isa
         DEF BusyCompletionExecution
    <2>2. CASE witness.kind = "PersistProposal"
      BY <1>1, <2>2, BusyPhaseOwnerPartitionObligation,
         AsyncStrongTypeProjectsAsyncType, IsaT(180)
         DEF BusyCompletionExecution, BusyCompletionCandidates,
             ExecuteCommand, ExecuteRegularCommand,
             RegularCoreCommand, PersistProposal, ProposalSign,
             BusyPhaseRank, Stage2TwoStepBusyNodes,
             Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
             Stage2OneStepBusyOwners, RequestNodeSet,
             AsyncProgressOwnershipInvariant,
             SerializedBusyOwnershipInvariant,
             SerializedBusyOwners, RequestsUniqueByNode
    <2>3. CASE witness.kind = "PersistPrepare"
      BY <1>1, <2>3, BusyPhaseOwnerPartitionObligation,
         AsyncStrongTypeProjectsAsyncType, IsaT(180)
         DEF BusyCompletionExecution, BusyCompletionCandidates,
             ExecuteCommand, ExecuteRegularCommand,
             RegularCoreCommand, PersistPrepare, VoteSign,
             BusyPhaseRank, Stage2TwoStepBusyNodes,
             Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
             Stage2OneStepBusyOwners, RequestNodeSet,
             AsyncProgressOwnershipInvariant,
             SerializedBusyOwnershipInvariant,
             SerializedBusyOwners, RequestsUniqueByNode
    <2>4. CASE witness.kind = "PersistLockCommit"
      BY <1>1, <2>4, BusyPhaseOwnerPartitionObligation,
         AsyncStrongTypeProjectsAsyncType, IsaT(180)
         DEF BusyCompletionExecution, BusyCompletionCandidates,
             ExecuteCommand, ExecuteRegularCommand,
             RegularCoreCommand, PersistLockCommit, VoteSign,
             BusyPhaseRank, Stage2TwoStepBusyNodes,
             Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
             Stage2OneStepBusyOwners, RequestNodeSet,
             AsyncProgressOwnershipInvariant,
             SerializedBusyOwnershipInvariant,
             SerializedBusyOwners, RequestsUniqueByNode
    <2>5. CASE witness.kind = "PersistTimeout"
      BY <1>1, <2>5, BusyPhaseOwnerPartitionObligation,
         AsyncStrongTypeProjectsAsyncType, IsaT(180)
         DEF BusyCompletionExecution, BusyCompletionCandidates,
             ExecuteCommand, ExecuteRegularCommand,
             RegularCoreCommand, PersistTimeout, TimeoutSign,
             BusyPhaseRank, Stage2TwoStepBusyNodes,
             Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
             Stage2OneStepBusyOwners, RequestNodeSet,
             AsyncProgressOwnershipInvariant,
             SerializedBusyOwnershipInvariant,
             SerializedBusyOwners, RequestsUniqueByNode
    <2>6. CASE witness.kind = "PersistInstallTC"
      BY <1>1, <2>6, BusyPhaseOwnerPartitionObligation,
         AsyncStrongTypeProjectsAsyncType, IsaT(240)
         DEF BusyCompletionExecution, BusyCompletionCandidates,
             ExecuteCommand, ExecutePersistInstall, PersistInstallTC,
             ActiveLockedCommitSignRequestsAfterInstall,
             ExactLockedCommitIntents, ResultingInstallLockRank,
             ResultingInstallLockSubject, VoteSign,
             BusyPhaseRank, Stage2TwoStepBusyNodes,
             Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
             Stage2OneStepBusyOwners, RequestNodeSet,
             AsyncProgressOwnershipInvariant,
             SerializedBusyOwnershipInvariant,
             SerializedBusyOwners, RequestsUniqueByNode
    <2>7. CASE witness.kind = "PersistObservePrepare"
      BY <1>1, <2>7, BusyPhaseOwnerPartitionObligation,
         AsyncStrongTypeProjectsAsyncType, IsaT(180)
         DEF BusyCompletionExecution, BusyCompletionCandidates,
             ExecuteCommand, ExecuteRegularCommand,
             RegularCoreCommand, PersistObservePrepare,
             BusyPhaseRank, Stage2TwoStepBusyNodes,
             Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
             Stage2OneStepBusyOwners, RequestNodeSet,
             AsyncProgressOwnershipInvariant,
             SerializedBusyOwnershipInvariant,
             SerializedBusyOwners, RequestsUniqueByNode
    <2>8. CASE witness.kind = "PersistDecision"
      BY <1>1, <2>8, BusyPhaseOwnerPartitionObligation,
         AsyncStrongTypeProjectsAsyncType, IsaT(180)
         DEF BusyCompletionExecution, BusyCompletionCandidates,
             ExecuteCommand, ExecutePersistDecision, PersistDecision,
             BusyPhaseRank, Stage2TwoStepBusyNodes,
             Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
             Stage2OneStepBusyOwners, RequestNodeSet,
             AsyncProgressOwnershipInvariant,
             SerializedBusyOwnershipInvariant,
             SerializedBusyOwners, RequestsUniqueByNode
    <2>9. CASE witness.kind = "SignProposal"
      BY <1>1, <2>9, BusyPhaseOwnerPartitionObligation,
         AsyncStrongTypeProjectsAsyncType, IsaT(180)
         DEF BusyCompletionExecution, BusyCompletionCandidates,
             ExecuteCommand, ExecuteSignProposal,
             CompleteProposalSignature,
             BusyPhaseRank, Stage2TwoStepBusyNodes,
             Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
             Stage2OneStepBusyOwners, RequestNodeSet,
             AsyncProgressOwnershipInvariant,
             SerializedBusyOwnershipInvariant,
             SerializedBusyOwners, RequestsUniqueByNode
    <2>10. CASE witness.kind = "SignVote"
      BY <1>1, <2>10, BusyPhaseOwnerPartitionObligation,
         AsyncStrongTypeProjectsAsyncType, IsaT(180)
         DEF BusyCompletionExecution, BusyCompletionCandidates,
             ExecuteCommand, ExecuteSignVote, CompleteVoteSignature,
             BusyPhaseRank, Stage2TwoStepBusyNodes,
             Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
             Stage2OneStepBusyOwners, RequestNodeSet,
             AsyncProgressOwnershipInvariant,
             SerializedBusyOwnershipInvariant,
             SerializedBusyOwners, RequestsUniqueByNode
    <2>11. CASE witness.kind = "SignTimeout"
      BY <1>1, <2>11, BusyPhaseOwnerPartitionObligation,
         AsyncStrongTypeProjectsAsyncType, IsaT(180)
         DEF BusyCompletionExecution, BusyCompletionCandidates,
             ExecuteCommand, ExecuteSignTimeout,
             CompleteTimeoutSignature,
             BusyPhaseRank, Stage2TwoStepBusyNodes,
             Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
             Stage2OneStepBusyOwners, RequestNodeSet,
             AsyncProgressOwnershipInvariant,
             SerializedBusyOwnershipInvariant,
             SerializedBusyOwners, RequestsUniqueByNode
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10, <2>11
         DEF Stage2TwoStepCompletionKinds,
             Stage2OneStepCompletionKinds
  <1> QED BY <1>1

(***************************************************************************
A matching Busy completion is executable while its node is Busy because the
Completion class is the sole `CommandDispatchable` exception to NodeIdle.
This leaf must use the exact pending/signature request guards; proving only
that some Completion command is enabled would not connect the scheduler
owner to the Core rank above.
***************************************************************************)

THEOREM BusyCompletionCandidateDispatchableObligation ==
  \A node \in ValidatorIds:
    \A witness \in BusyCompletionCandidates(node):
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ Stage2BusyKernelInvariant
      /\ BusyPhaseRank(node) \in 1..2
      => CommandDispatchable(witness)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, NEW witness,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                Stage2BusyKernelInvariant,
                BusyPhaseRank(node) \in 1..2,
                witness \in BusyCompletionCandidates(node)
         PROVE CommandDispatchable(witness)
    <2>1. /\ AsyncCandidateTyped(witness)
           /\ CandidateConsumerCurrent(witness)
           /\ witness.class = "Completion"
      BY <1>1, AsyncStrongTypeProjectsAsyncType, Isa
         DEF BusyCompletionCandidates, ActiveBusyCompletionCarrier,
             QueuedCandidates, CausalCandidates,
             TrackedWorkCandidates, SequenceSet,
             AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant,
             AsyncCausalTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoContentTypeInvariant,
             AsyncIoWorkContentTypeInvariant,
             AsyncQueueTyped, AsyncCompletionSequenceTyped,
             AsyncOutstandingCarrierInvariant,
             AsyncProgressOwnershipInvariant
    <2>2. witness.kind
             \in Stage2TwoStepCompletionKinds
                  \cup Stage2OneStepCompletionKinds
      BY <1>1, BusyCompletionKindMatchesPhaseObligation, Isa
         DEF Stage2BusyKernelInvariant
    <2>3. CASE witness.kind = "PersistProposal"
      BY <1>1, <2>3, ExpandENABLED, IsaT(180)
         DEF Stage2BusyKernelInvariant,
             Stage2BusyCompletionGuards,
             BusyCompletionCandidates, CommandExecutionReady,
             ExecuteRegularCommand, RegularCoreCommand,
             PersistProposal, AsyncAuxVars, vars
    <2>4. CASE witness.kind = "PersistPrepare"
      BY <1>1, <2>4, ExpandENABLED, IsaT(180)
         DEF Stage2BusyKernelInvariant,
             Stage2BusyCompletionGuards,
             BusyCompletionCandidates, CommandExecutionReady,
             ExecuteRegularCommand, RegularCoreCommand,
             PersistPrepare, AsyncAuxVars, vars
    <2>5. CASE witness.kind = "PersistLockCommit"
      BY <1>1, <2>5, ExpandENABLED, IsaT(180)
         DEF Stage2BusyKernelInvariant,
             Stage2BusyCompletionGuards,
             BusyCompletionCandidates, CommandExecutionReady,
             ExecuteRegularCommand, RegularCoreCommand,
             PersistLockCommit, AsyncStrongTypeInvariant,
             StrongInductiveInvariant, Safety,
             ReducerProvenanceInvariant,
             PendingVoteWritesAuthorized, AsyncAuxVars, vars
    <2>6. CASE witness.kind = "PersistTimeout"
      BY <1>1, <2>6, ExpandENABLED, IsaT(180)
         DEF Stage2BusyKernelInvariant,
             Stage2BusyCompletionGuards,
             BusyCompletionCandidates, CommandExecutionReady,
             ExecuteRegularCommand, RegularCoreCommand,
             PersistTimeout, AsyncAuxVars, vars
    <2>7. CASE witness.kind = "PersistInstallTC"
      BY <1>1, <2>7, ExpandENABLED, IsaT(240)
         DEF BusyCompletionCandidates, CommandExecutionReady,
             ExecutePersistInstall, PersistInstallTC,
             PersistInstalledControlAfterInstall,
             ActiveLockedCommitSignRequestsAfterInstall,
             ExactLockedCommitIntents, ResultingInstallLockRank,
             ResultingInstallLockSubject, AsyncStrongTypeInvariant,
             StrongInductiveInvariant, Safety,
             ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized,
             AsyncAuxVars, vars
    <2>8. CASE witness.kind = "PersistObservePrepare"
      BY <1>1, <2>8, ExpandENABLED, IsaT(180)
         DEF BusyCompletionCandidates, CommandExecutionReady,
             ExecuteRegularCommand, RegularCoreCommand,
             PersistObservePrepare, AsyncAuxVars, vars
    <2>9. CASE witness.kind = "PersistDecision"
      BY <1>1, <2>9, ExpandENABLED, IsaT(180)
         DEF BusyCompletionCandidates, CommandExecutionReady,
             ExecutePersistDecision, PersistDecision,
             PersistDecisionControl, AsyncAuxVars, vars
    <2>10. CASE witness.kind = "SignProposal"
      BY <1>1, <2>10, AsyncStrongTypeProjectsAsyncType,
         ProposalOutboxIsRetainable, ExpandENABLED, IsaT(240)
         DEF Stage2BusyKernelInvariant,
             Stage2BusyCompletionGuards,
             BusyCompletionCandidates, CommandExecutionReady,
             ExecuteSignProposal, CompleteProposalSignature,
             PublishControlAndEphemeralItems,
             RetainableControlBatch, AsyncStrongTypeInvariant,
             StrongInductiveInvariant, Safety,
             ProposalSigningRequiresIntent, AsyncAuxVars, vars
    <2>11. CASE witness.kind = "SignVote"
      BY <1>1, <2>11, AsyncStrongTypeProjectsAsyncType,
         VoteOutboxIsRetainable, ExpandENABLED, IsaT(240)
         DEF Stage2BusyKernelInvariant,
             Stage2BusyCompletionGuards,
             BusyCompletionCandidates, CommandExecutionReady,
             ExecuteSignVote, CompleteVoteSignature,
             PublishControlItems, RetainableControlBatch,
             AsyncStrongTypeInvariant, StrongInductiveInvariant,
             Safety, PrepareSigningRequiresIntent,
             CommitSigningRequiresIntent, AsyncAuxVars, vars
    <2>12. CASE witness.kind = "SignTimeout"
      BY <1>1, <2>12, AsyncStrongTypeProjectsAsyncType,
         TimeoutOutboxIsRetainable, ExpandENABLED, IsaT(240)
         DEF Stage2BusyKernelInvariant,
             Stage2BusyCompletionGuards,
             BusyCompletionCandidates, CommandExecutionReady,
             ExecuteSignTimeout, CompleteTimeoutSignature,
             PublishControlItems, RetainableControlBatch,
             AsyncStrongTypeInvariant, StrongInductiveInvariant,
             Safety, TimeoutSigningRequiresIntent,
             AsyncAuxVars, vars
    <2>13. CommandExecutionReady(witness)
      BY <2>2, <2>3, <2>4, <2>5, <2>6, <2>7, <2>8,
         <2>9, <2>10, <2>11, <2>12
         DEF Stage2TwoStepCompletionKinds,
             Stage2OneStepCompletionKinds
    <2> QED BY <2>1, <2>13 DEF CommandDispatchable
  <1> QED BY <1>1

(***************************************************************************
Production reachability of the strengthened Busy kernel.

The two missing facts are not consequences of the old named invariants, but
they are established by initialization and preserved by the concrete reducer:
every Begin/Resume edge requires `NodeIdle`, and each Persist edge replaces
its pending owner atomically with the corresponding ready signing owner.
Keep this induction explicit so the temporal stage-2 result does not depend
on an unproved environmental assumption.
***************************************************************************)

THEOREM Stage2BusyKernelInitObligation ==
  \A initialContext:
    AsyncInitAt(initialContext) => Stage2BusyKernelInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
       Stage2BusyKernelInvariant, Stage2BusyPhaseSeparated,
       Stage2BusyCompletionGuards, Stage2TwoStepBusyNodes,
       Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
       Stage2OneStepBusyOwners, RequestNodeSet

THEOREM Stage2BusyKernelNextObligation ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ Stage2BusyKernelInvariant
  /\ [AsyncNext]_AsyncAllVars
  => Stage2BusyKernelInvariant'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   RuntimeSelectedCommandsAreTyped,
   ProgressCoreStutterAndCarrierGrowthRetainsBusyCandidates,
   HeadTailProperties, IsaT(300)
   DEF Stage2BusyKernelInvariant, Stage2BusyPhaseSeparated,
       Stage2BusyCompletionGuards, Stage2TwoStepBusyNodes,
       Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
       Stage2OneStepBusyOwners, RequestNodeSet,
       SerializedBusyOwners, SerializedBusyOwnershipInvariant,
       RequestsUniqueByNode, NodeIdle, PendingNodes, SigningNodes,
       AllPendingRequests, AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ReducerProvenanceInvariant, LineageInvariant,
       PendingVoteWritesAuthorized,
       PendingCertificateWritesAuthorized,
       ProposalSigningRequiresIntent,
       PrepareSigningRequiresIntent, CommitSigningRequiresIntent,
       TimeoutSigningRequiresIntent, VoteRoundAdmissible,
       LockedPrepareRound, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep,
       AdmitProducerCompletion, AdmitCausalHead, IngressDrainStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DirectTimeoutStep, DirectRetransmitStep,
       IdleRuntimeStep, RemoveNextNodeCommand,
       RemoveNextDeferredCommand, DeferCommand, DiscardCommand,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       AssembleLocalBody, BeginLocalProposal, LocalProposalFor,
       LocalProposalJustification, Proposal, PersistProposal,
       FetchBody, RebindRetainedBody, StoreBody, ValidateBody,
       RejectBody, ValidateDecidedBody, ValidateLockedBody,
       BeginPrepare, PersistPrepare, BeginObservePrepare,
       PersistObservePrepare, BeginLockCommit, PersistLockCommit,
       FormCommitQC, BeginDecision, PersistTimeout, FormTC,
       BeginInstallTC, FetchCertifiedBody,
       ExecuteDecisionFetch, ExecuteSignProposal,
       CompleteProposalSignature, ExecuteSignVote,
       CompleteVoteSignature, ExecuteFormPrepareQC,
       ExecuteSignTimeout, CompleteTimeoutSignature,
       ExecutePersistInstall, PersistInstallTC,
       ActiveLockedCommitSignRequestsAfterInstall,
       ExactLockedCommitIntents, ResultingInstallLockRank,
       ResultingInstallLockSubject,
       ExecutePersistDecision, PersistDecision,
       ExecuteRequestCertifiedBody, ExecuteApply, ApplyDecision,
       ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk, ServiceIoWorker,
       ServiceHistoricalRecoveryIoWorker, ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork, OpenHistoricalRecovery,
       CommitCertificateDiscoveryStepWork, AsyncNetworkStep,
       AdmitIngressPacket, AsyncFaultStep, PreGstCrash, Crash,
       PreGstResponsiveCrash, PreGstResponsiveRestart, Restart,
       PreGstResponsiveReplay, ResumeProposal, ResumeVote,
       VoteResumeAuthorized, ResumeTimeout,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       RearmResponsiveRecovery, AsyncTick, AsyncSetGST,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncAllVars

THEOREM AsyncSpecAlwaysStage2BusyKernelObligation ==
  \A initialContext:
    Stage2BusyKernelProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE Stage2BusyKernelProperty(AsyncSpecAt(initialContext))
    <2>1. AsyncInitAt(initialContext) => Stage2BusyKernelInvariant
      BY Stage2BusyKernelInitObligation
    <2>2. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant, PTL
    <2>3. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ Stage2BusyKernelInvariant
           /\ [AsyncNext]_AsyncAllVars
          => Stage2BusyKernelInvariant'
      BY Stage2BusyKernelNextObligation
    <2> QED BY <2>1, <2>2, <2>3, PTL
         DEF Stage2BusyKernelProperty, AsyncSpecAt
  <1> QED BY <1>1

(***************************************************************************
Restricted post-deferred convergence.

Stage 2 cannot assume the full protected-rank theorem which it is needed to
prove.  Busy completion witnesses live only in the active carrier, so their
temporal service composes exactly stages 3 through 6.  Stage-4 and Stage-5
are already proved; Stage-3 and Stage-6 must be supplied by their independent
strict slices before this restricted property is available.
***************************************************************************)

PostDeferredServiceRankCarrier == (3..6) \X Nat

PostDeferredServiceRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), 3..6, Nat)

ProtectedPostDeferredRankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet,
          stage \in 3..6, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<stage, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<stage, position>>))

ProtectedStage2RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<2, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<2, position>>))

THEOREM ProtectedPostDeferredRanksComposeFromLeavesObligation ==
  \A initialContext:
    /\ ProtectedStage3RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedStage4RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedStage5RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedStage6RankProgressProperty(
         AsyncSpecAt(initialContext))
    => ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ProtectedStage3RankProgressProperty(
                  AsyncSpecAt(initialContext)),
                ProtectedStage4RankProgressProperty(
                  AsyncSpecAt(initialContext)),
                ProtectedStage5RankProgressProperty(
                  AsyncSpecAt(initialContext)),
                ProtectedStage6RankProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE ProtectedPostDeferredRankProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME NEW candidate \in AsyncCandidateSet,
                  NEW stage \in 3..6, NEW position \in Nat,
                  AsyncSpecAt(initialContext)
           PROVE (gst
                    /\ ResponsiveProtectedCandidateOwned(candidate)
                    /\ CandidateServiceRank(candidate)
                         = <<stage, position>>)
                   ~> (~ResponsiveProtectedCandidateOwned(candidate)
                        \/ ServiceRankLess(
                             CandidateServiceRank(candidate),
                             <<stage, position>>))
      <3>1. CASE stage = 3
        BY <1>1, <2>1, <3>1
           DEF ProtectedStage3RankProgressProperty,
               Stage3RankProgressExit
      <3>2. CASE stage = 4
        BY <1>1, <2>1, <3>2
           DEF ProtectedStage4RankProgressProperty
      <3>3. CASE stage = 5
        BY <1>1, <2>1, <3>3
           DEF ProtectedStage5RankProgressProperty
      <3>4. CASE stage = 6
        BY <1>1, <2>1, <3>4
           DEF ProtectedStage6RankProgressProperty
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4, Isa
    <2> QED BY <2>1 DEF ProtectedPostDeferredRankProgressProperty
  <1> QED BY <1>1

ProtectedPostDeferredExit(candidate) ==
  \/ ~ResponsiveProtectedCandidateOwned(candidate)
  \/ CandidateServiceRank(candidate)[1] \notin 3..6

ProtectedPostDeferredAtRank(candidate, rank) ==
  /\ gst
  /\ ResponsiveProtectedCandidateOwned(candidate)
  /\ CandidateServiceRank(candidate) = rank

THEOREM PostDeferredServiceRankOrderingWellFoundedObligation ==
  IsWellFoundedOn(
    PostDeferredServiceRankOrdering, PostDeferredServiceRankCarrier)
PROOF
  <1>1. IsWellFoundedOn(OpToRel(<, Nat), 3..6)
    BY NatLessThanWellFounded, IsWellFoundedOnSubset, Isa
  <1> QED BY <1>1, NatLessThanWellFounded, WFLexPairOrdering
       DEF PostDeferredServiceRankOrdering,
           PostDeferredServiceRankCarrier

THEOREM PostDeferredRankProgressConvergesObligation ==
  \A initialContext, candidate:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
    => (gst
          /\ ResponsiveProtectedCandidateOwned(candidate)
          /\ CandidateServiceRank(candidate)[1] \in 3..6)
         ~> ProtectedPostDeferredExit(candidate)
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate,
                AsyncSpecAt(initialContext),
                ProtectedPostDeferredRankProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE (gst
                  /\ ResponsiveProtectedCandidateOwned(candidate)
                  /\ CandidateServiceRank(candidate)[1] \in 3..6)
                 ~> ProtectedPostDeferredExit(candidate)
    <2>1. ASSUME NEW rank \in PostDeferredServiceRankCarrier
           PROVE ProtectedPostDeferredAtRank(candidate, rank)
                   ~> (ProtectedPostDeferredExit(candidate)
                        \/ \E lower \in SetLessThan(
                             rank, PostDeferredServiceRankOrdering,
                             PostDeferredServiceRankCarrier):
                             ProtectedPostDeferredAtRank(
                               candidate, lower))
      <3>1. PICK stage \in 3..6, position \in Nat:
               rank = <<stage, position>>
        BY <2>1 DEF PostDeferredServiceRankCarrier
      <3>2. ProtectedPostDeferredAtRank(candidate, rank)
               ~> (~ResponsiveProtectedCandidateOwned(candidate)
                    \/ ServiceRankLess(
                         CandidateServiceRank(candidate), rank))
        BY <1>1, <3>1
           DEF ProtectedPostDeferredRankProgressProperty,
               ProtectedPostDeferredAtRank
      <3>3. AsyncSpecAt(initialContext) => []AsyncTypeInvariant
        BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
           AsyncStrongTypeProjectsAsyncType, PTL
      <3>4. /\ AsyncTypeInvariant
             /\ gst
             /\ ResponsiveProtectedCandidateOwned(candidate)
             /\ ServiceRankLess(
                  CandidateServiceRank(candidate), rank)
            => \/ ProtectedPostDeferredExit(candidate)
               \/ \E lower \in SetLessThan(
                    rank, PostDeferredServiceRankOrdering,
                    PostDeferredServiceRankCarrier):
                    ProtectedPostDeferredAtRank(candidate, lower)
        BY <2>1, ScheduledCandidateServiceRankInCarrier,
           OwnedServiceRankOrderingMatchesLess, Isa
           DEF ProtectedPostDeferredExit,
               ProtectedPostDeferredAtRank,
               ResponsiveProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateScheduled,
               CandidateServiceRank, ServiceRankLess,
               PostDeferredServiceRankOrdering,
               PostDeferredServiceRankCarrier, SetLessThan
      <3> QED BY <3>2, <3>3, <3>4, PTL
           DEF ProtectedPostDeferredExit
    <2>2. \A rank \in PostDeferredServiceRankCarrier:
             ProtectedPostDeferredAtRank(candidate, rank)
               ~> ProtectedPostDeferredExit(candidate)
      BY <2>1, PostDeferredServiceRankOrderingWellFoundedObligation,
         WellFoundedLeadsTo
    <2>3. (gst
             /\ ResponsiveProtectedCandidateOwned(candidate)
             /\ CandidateServiceRank(candidate)[1] \in 3..6)
             ~> \E rank \in PostDeferredServiceRankCarrier:
                  ProtectedPostDeferredAtRank(candidate, rank)
      BY Isa, PTL
         DEF ProtectedPostDeferredAtRank,
             PostDeferredServiceRankCarrier
    <2> QED BY <2>2, <2>3, PTL
  <1> QED BY <1>1

(***************************************************************************
Fixed-witness bridge.

While the target is still protected and the Core phase has not dropped, the
same exact Busy completion may move 6 -> 5 -> 4 -> 3 but may neither enter the
Busy-deferred stage nor disappear.  An execution/removal changes the matching
pending/signature owner and therefore lowers `BusyPhaseRank`; applying the
height ends protection for both witness and target.  This is the non-vacuous
connection from post-deferred scheduler progress to terminating local work.
***************************************************************************)

ProtectedStage2Owned(candidate) ==
  /\ gst
  /\ ResponsiveProtectedCandidateOwned(candidate)
  /\ candidate \in DeferredCandidates

ProtectedStage2Pending(candidate, position) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ ProtectedOwnedAtServiceRank(candidate, <<2, position>>)

ProtectedBusyCompletionWitness(target, witness) ==
  /\ ProtectedStage2Owned(target)
  /\ BusyPhaseRank(target.node) \in 1..2
  /\ witness \in BusyCompletionCandidates(target.node)

THEOREM ProtectedBusyWitnessHasPostDeferredRankObligation ==
  \A target, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ProtectedBusyCompletionWitness(target, witness)
    => /\ ResponsiveProtectedCandidateOwned(witness)
       /\ CandidateServiceRank(witness)
            \in PostDeferredServiceRankCarrier
BY AsyncStrongTypeProjectsAsyncType,
   ScheduledCandidateServiceRankInCarrier, Isa
   DEF ProtectedBusyCompletionWitness, ProtectedStage2Owned,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       ProtectedServiceCandidate, CandidateScheduled,
       BusyCompletionCandidates, ActiveBusyCompletionCarrier,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, CandidateServiceRank,
       PostDeferredServiceRankCarrier,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant

THEOREM BusyWitnessOwnershipPersistsUntilTargetExitOrPhaseDrop ==
  \A target, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ ProtectedBusyCompletionWitness(target, witness)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~ProtectedServiceOwnershipExit(target)'
    /\ BusyPhaseRank(target.node)'
         >= BusyPhaseRank(target.node)
    => ProtectedBusyCompletionWitness(target, witness)'
BY BusyPhaseOwnerPartitionObligation,
   BusyCompletionExecutionDropsPhaseObligation,
   BusyCompletionCandidateDispatchableObligation,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   RuntimeSelectedCommandsAreTyped,
   ProgressCoreStutterAndCarrierGrowthRetainsBusyCandidates,
   ProgressCoreStutterKeepsBusyWitnessWhenCarried,
   HeadTailProperties, IsaT(240)
   DEF Stage2BusyKernelInvariant,
       ProtectedBusyCompletionWitness, ProtectedStage2Owned,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CandidateScheduled, CandidateServiceRank,
       BusyPhaseRank, Stage2TwoStepBusyNodes,
       Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
       Stage2OneStepBusyOwners, BusyCompletionCandidates,
       ActiveBusyCompletionCarrier, SerializedBusyOwners,
       SerializedBusyOwnershipInvariant, RequestsUniqueByNode,
       RequestNodeSet, NodeIdle, PendingNodes, SigningNodes,
       AllPendingRequests, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       CandidateConsumerCurrent, CommandDispatchable,
       CommandExecutionReady, RunNode, RunNodeWork,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep,
       AdmitProducerCompletion, AdmitCausalHead, IngressDrainStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DirectTimeoutStep, DirectRetransmitStep,
       IdleRuntimeStep, RemoveNextNodeCommand,
       RemoveNextDeferredCommand, DeferCommand, DiscardCommand,
       ExecuteCommand, ExecuteRegularCommand,
       ExecuteDecisionFetch, ExecuteSignProposal, ExecuteSignVote,
       ExecuteFormPrepareQC, ExecuteSignTimeout,
       ExecutePersistInstall, ExecutePersistDecision,
       ExecuteRequestCertifiedBody, ExecuteApply,
       ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk, ServiceIoWorker,
       ServiceHistoricalRecoveryIoWorker, ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork, OpenHistoricalRecovery,
       CommitCertificateDiscoveryStepWork, AsyncNetworkStep,
       AdmitIngressPacket, AsyncFaultStep, PreGstCrash,
       PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, DriveResponsiveReplayHead,
       FinishResponsiveReplay, RearmResponsiveRecovery,
       AsyncTick, AsyncSetGST, AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM BusyWitnessPersistsUntilTargetExitOrPhaseDropObligation ==
  \A target, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ ProtectedBusyCompletionWitness(target, witness)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~ProtectedServiceOwnershipExit(target)'
    /\ BusyPhaseRank(target.node)'
         >= BusyPhaseRank(target.node)
    => /\ ResponsiveProtectedCandidateOwned(witness)'
       /\ CandidateServiceRank(witness)'[1] \in 3..6
PROOF
  <1>1. ASSUME NEW target, NEW witness,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                Stage2BusyKernelInvariant,
                ProtectedBusyCompletionWitness(target, witness),
                [AsyncNext]_AsyncAllVars,
                ~ProtectedServiceOwnershipExit(target)',
                BusyPhaseRank(target.node)'
                  >= BusyPhaseRank(target.node)
         PROVE /\ ResponsiveProtectedCandidateOwned(witness)'
               /\ CandidateServiceRank(witness)'[1] \in 3..6
    <2>1. /\ AsyncStrongTypeInvariant'
           /\ AsyncProgressOwnershipInvariant'
           /\ ProtectedBusyCompletionWitness(target, witness)'
      BY <1>1, AsyncBracketNextPreservesStrongTypeInvariant,
         AsyncBracketNextPreservesProgressOwnership,
         BusyWitnessOwnershipPersistsUntilTargetExitOrPhaseDrop
    <2>2. /\ ResponsiveProtectedCandidateOwned(witness)'
           /\ CandidateServiceRank(witness)'
                \in PostDeferredServiceRankCarrier
      BY <2>1, ProtectedBusyWitnessHasPostDeferredRankObligation
    <2> QED BY <2>2 DEF PostDeferredServiceRankCarrier
  <1> QED BY <1>1

THEOREM BusyPhaseCannotIncreaseWhileProtected ==
  \A target:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ ProtectedStage2Owned(target)
    /\ BusyPhaseRank(target.node) \in 1..2
    /\ [AsyncNext]_AsyncAllVars
    /\ ~ProtectedServiceOwnershipExit(target)'
    => BusyPhaseRank(target.node)' <= BusyPhaseRank(target.node)
BY BusyPhaseOwnerPartitionObligation,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   RuntimeSelectedCommandsAreTyped, IsaT(180)
   DEF Stage2BusyKernelInvariant,
       ProtectedStage2Owned, ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       BusyPhaseRank, Stage2TwoStepBusyNodes,
       Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
       Stage2OneStepBusyOwners, SerializedBusyOwners,
       SerializedBusyOwnershipInvariant, RequestsUniqueByNode,
       RequestNodeSet, NodeIdle, PendingNodes, SigningNodes,
       AllPendingRequests, RunNode, RunNodeWork,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep, FifoRuntimeStep,
       DeferredDrainStep, DeferredTagStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep, ExecuteCommand,
       ExecuteRegularCommand, ExecuteSignProposal, ExecuteSignVote,
       ExecuteFormPrepareQC, ExecuteSignTimeout,
       ExecutePersistInstall, ExecutePersistDecision,
       ExecuteRequestCertifiedBody, ExecuteApply,
       ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk, AsyncNext,
       AsyncNonCrashStep, AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncAllVars

Stage2BusyPhaseGoal(target, phase) ==
  \/ ProtectedServiceOwnershipExit(target)
  \/ BusyPhaseRank(target.node) < phase

Stage2BusyWitnessBlocked(target, witness, phase) ==
  /\ ProtectedBusyCompletionWitness(target, witness)
  /\ BusyPhaseRank(target.node) = phase

THEOREM ProtectedStage2BusyPhaseDescentObligation ==
  \A initialContext, target:
    \A phase \in 1..2:
      /\ AsyncSpecAt(initialContext)
      /\ ProtectedPostDeferredRankProgressProperty(
           AsyncSpecAt(initialContext))
      /\ Stage2BusyKernelProperty(AsyncSpecAt(initialContext))
      => (ProtectedStage2Owned(target)
            /\ BusyPhaseRank(target.node) = phase)
           ~> (ProtectedServiceOwnershipExit(target)
                \/ BusyPhaseRank(target.node) < phase)
PROOF
  <1>1. ASSUME NEW initialContext, NEW target,
                NEW phase \in 1..2,
                AsyncSpecAt(initialContext),
                ProtectedPostDeferredRankProgressProperty(
                  AsyncSpecAt(initialContext)),
                Stage2BusyKernelProperty(
                  AsyncSpecAt(initialContext))
         PROVE (ProtectedStage2Owned(target)
                  /\ BusyPhaseRank(target.node) = phase)
                 ~> Stage2BusyPhaseGoal(target, phase)
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ Stage2BusyKernelInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant, PTL
         DEF Stage2BusyKernelProperty
    <2>2. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ ProtectedStage2Owned(target)
           /\ BusyPhaseRank(target.node) = phase
          => \E witness \in AsyncCandidateSet:
               Stage2BusyWitnessBlocked(target, witness, phase)
      BY <1>1, <2>1, BusyPhaseOwnerPartitionObligation, Isa
         DEF Stage2BusyKernelInvariant, Stage2BusyWitnessBlocked,
             ProtectedBusyCompletionWitness,
             AsyncProgressOwnershipInvariant,
             BusyCompletionWitnessInvariant
    <2>3. ASSUME NEW witness \in AsyncCandidateSet
           PROVE Stage2BusyWitnessBlocked(target, witness, phase)
                   ~> Stage2BusyPhaseGoal(target, phase)
      <3>1. Stage2BusyWitnessBlocked(target, witness, phase)
               => /\ ResponsiveProtectedCandidateOwned(witness)
                  /\ CandidateServiceRank(witness)[1] \in 3..6
        BY <2>1, ProtectedBusyWitnessHasPostDeferredRankObligation
           DEF Stage2BusyWitnessBlocked
      <3>2. (gst
                /\ ResponsiveProtectedCandidateOwned(witness)
                /\ CandidateServiceRank(witness)[1] \in 3..6)
               ~> ProtectedPostDeferredExit(witness)
        BY <1>1, PostDeferredRankProgressConvergesObligation
      <3>3. Stage2BusyWitnessBlocked(target, witness, phase)
               ~> ProtectedPostDeferredExit(witness)
        BY <3>1, <3>2, PTL
           DEF Stage2BusyWitnessBlocked,
               ProtectedBusyCompletionWitness, ProtectedStage2Owned
      <3>4. /\ AsyncStrongTypeInvariant
             /\ AsyncProgressOwnershipInvariant
             /\ Stage2BusyKernelInvariant
             /\ Stage2BusyWitnessBlocked(target, witness, phase)
             /\ [AsyncNext]_AsyncAllVars
            => \/ Stage2BusyPhaseGoal(target, phase)'
               \/ Stage2BusyWitnessBlocked(
                    target, witness, phase)'
        <4>1. ASSUME AsyncStrongTypeInvariant,
                      AsyncProgressOwnershipInvariant,
                      Stage2BusyKernelInvariant,
                      Stage2BusyWitnessBlocked(
                        target, witness, phase),
                      [AsyncNext]_AsyncAllVars
               PROVE \/ Stage2BusyPhaseGoal(target, phase)'
                     \/ Stage2BusyWitnessBlocked(
                          target, witness, phase)'
          <5>1. CASE Stage2BusyPhaseGoal(target, phase)'
            BY <5>1
          <5>2. CASE ~Stage2BusyPhaseGoal(target, phase)'
            <6>1. /\ ~ProtectedServiceOwnershipExit(target)'
                   /\ BusyPhaseRank(target.node)' >= phase
              BY <5>2 DEF Stage2BusyPhaseGoal
            <6>2. BusyPhaseRank(target.node)'
                     <= BusyPhaseRank(target.node)
              BY <4>1, BusyPhaseCannotIncreaseWhileProtected
                 DEF Stage2BusyWitnessBlocked,
                     ProtectedBusyCompletionWitness
            <6>3. BusyPhaseRank(target.node)' = phase
              BY <4>1, <6>1, <6>2
                 DEF Stage2BusyWitnessBlocked
            <6>4. /\ ResponsiveProtectedCandidateOwned(witness)'
                   /\ CandidateServiceRank(witness)'[1] \in 3..6
              BY <4>1, <6>1,
                 BusyWitnessPersistsUntilTargetExitOrPhaseDropObligation
                 DEF Stage2BusyWitnessBlocked
            <6>5. ProtectedBusyCompletionWitness(target, witness)'
              BY <4>1, <6>1,
                 BusyWitnessOwnershipPersistsUntilTargetExitOrPhaseDrop
                 DEF Stage2BusyWitnessBlocked
            <6> QED BY <6>3, <6>4, <6>5
                 DEF Stage2BusyWitnessBlocked
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3>5. [](Stage2BusyWitnessBlocked(target, witness, phase)
                 /\ ProtectedPostDeferredExit(witness)
                => FALSE)
        BY <2>1, ProtectedBusyWitnessHasPostDeferredRankObligation,
           PTL
           DEF Stage2BusyWitnessBlocked,
               ProtectedPostDeferredExit,
               PostDeferredServiceRankCarrier
      <3> QED BY <2>1, <3>3, <3>4, <3>5, PTL
    <2>4. (ProtectedStage2Owned(target)
              /\ BusyPhaseRank(target.node) = phase)
             ~> \E witness \in AsyncCandidateSet:
                  Stage2BusyWitnessBlocked(target, witness, phase)
      BY <2>1, <2>2, PTL
    <2> QED BY <2>3, <2>4, PTL
         DEF Stage2BusyPhaseGoal
  <1> QED BY <1>1

Stage2BusyAtPhase(target, phase) ==
  /\ ProtectedStage2Owned(target)
  /\ BusyPhaseRank(target.node) = phase

Stage2BusyTerminationGoal(target) ==
  \/ ProtectedServiceOwnershipExit(target)
  \/ NodeIdle(target.node)

THEOREM ProtectedStage2BusyTerminatesLocallyObligation ==
  \A initialContext, target:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ Stage2BusyKernelProperty(AsyncSpecAt(initialContext))
    => (ProtectedStage2Owned(target) /\ ~NodeIdle(target.node))
         ~> (ProtectedServiceOwnershipExit(target)
              \/ NodeIdle(target.node))
PROOF
  <1>1. ASSUME NEW initialContext, NEW target,
                AsyncSpecAt(initialContext),
                ProtectedPostDeferredRankProgressProperty(
                  AsyncSpecAt(initialContext)),
                Stage2BusyKernelProperty(
                  AsyncSpecAt(initialContext))
         PROVE (ProtectedStage2Owned(target)
                  /\ ~NodeIdle(target.node))
                 ~> Stage2BusyTerminationGoal(target)
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ Stage2BusyKernelInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant, PTL
         DEF Stage2BusyKernelProperty
    <2>2. Stage2BusyAtPhase(target, 1)
             ~> Stage2BusyTerminationGoal(target)
      <3>1. Stage2BusyAtPhase(target, 1)
               ~> (ProtectedServiceOwnershipExit(target)
                    \/ BusyPhaseRank(target.node) < 1)
        BY <1>1, ProtectedStage2BusyPhaseDescentObligation
           DEF Stage2BusyAtPhase
      <3>2. [](\/ ProtectedServiceOwnershipExit(target)
                 \/ BusyPhaseRank(target.node) < 1
                => Stage2BusyTerminationGoal(target))
        BY <2>1, BusyPhaseOwnerPartitionObligation, Isa, PTL
           DEF Stage2BusyTerminationGoal,
               Stage2BusyKernelInvariant, BusyPhaseCarrier
      <3> QED BY <3>1, <3>2, PTL
    <2>3. Stage2BusyAtPhase(target, 2)
             ~> Stage2BusyTerminationGoal(target)
      <3>1. Stage2BusyAtPhase(target, 2)
               ~> (ProtectedServiceOwnershipExit(target)
                    \/ BusyPhaseRank(target.node) < 2)
        BY <1>1, ProtectedStage2BusyPhaseDescentObligation
           DEF Stage2BusyAtPhase
      <3>2. [](\/ ProtectedServiceOwnershipExit(target)
                 \/ BusyPhaseRank(target.node) < 2
                => \/ Stage2BusyTerminationGoal(target)
                   \/ Stage2BusyAtPhase(target, 1))
        BY <2>1, BusyPhaseOwnerPartitionObligation, Isa, PTL
           DEF Stage2BusyAtPhase, Stage2BusyTerminationGoal,
               Stage2BusyKernelInvariant, BusyPhaseCarrier,
               ProtectedStage2Owned,
               ProtectedServiceOwnershipExit
      <3>3. Stage2BusyAtPhase(target, 2)
               ~> (Stage2BusyTerminationGoal(target)
                    \/ Stage2BusyAtPhase(target, 1))
        BY <3>1, <3>2, PTL
      <3> QED BY <2>2, <3>3, PTL
    <2>4. [](ProtectedStage2Owned(target)
               /\ ~NodeIdle(target.node)
              => \/ Stage2BusyAtPhase(target, 1)
                 \/ Stage2BusyAtPhase(target, 2))
      BY <2>1, BusyPhaseOwnerPartitionObligation, Isa, PTL
         DEF Stage2BusyAtPhase, Stage2BusyKernelInvariant,
             ProtectedStage2Owned
    <2> QED BY <2>2, <2>3, <2>4, PTL
         DEF Stage2BusyTerminationGoal
  <1> QED BY <1>1

(***************************************************************************
Concrete equal-rank rebusy boundary.

The following three ranks are the exact cursor arithmetic for the smallest
problematic cycle with a Normal target and a Progress BeginObservePrepare
blocker:

  idle before blocker:       3 * prefix + 2, Busy 0
  blocker starts Busy:       3 * prefix + 0, Busy 1
  target is retried Busy:    3 * prefix + 2, Busy 1
  PersistObserve completes:  3 * prefix + 2, Busy 0

The final state has the initial target rank and cursor.  A later authenticated
higher-view Observe command can occupy the Progress head and repeat the same
cycle.  Weak fairness of RunNode and termination of the current Busy owner do
not by themselves exclude it.
***************************************************************************)

Stage2EqualRankRebusyRanks(prefix) ==
  <<3 * prefix + 2, 3 * prefix, 3 * prefix + 2, 3 * prefix + 2>>

Stage2ObserveRebusyBoundary(target, blocker) ==
  /\ ProtectedStage2Owned(target)
  /\ NodeIdle(target.node)
  /\ asyncDeferredDrainOwed[target.node]
  /\ target.class = "Normal"
  /\ blocker = NextDeferredCommand(target.node)
  /\ blocker.class = "Progress"
  /\ blocker.kind = "BeginObservePrepare"
  /\ CommandClassDistance(
       asyncNextDeferredClass[target.node], target.class) = 2

(***************************************************************************
Executable exact-identity handoff.

The token contains the authenticated reducer owner plus the full immutable
candidate identity.  It is not a connection generation, cursor position, or
fresh replacement candidate.  `asyncDeferredHandoffs` now records this token
on the concrete Busy deferred-drain edge.  The held candidate remains the
head of its bounded class queue, so capacity ownership does not move to a
second carrier.  A foreign class may advance the cyclic cursor while the node
is idle, but it is skipped rather than executed and therefore cannot install
a fresh Busy owner ahead of the held candidate.
***************************************************************************)

Stage2DeferredHandoffToken(candidate) ==
  [owner |-> candidate.node,
   identity |-> ExactAsyncCandidateIdentity(candidate)]

Stage2NoDeferredHandoff == NoAsyncDeferredHandoff

Stage2ActiveDeferredHandoff(candidate) ==
  AsyncDeferredHandoff(candidate)

Stage2DeferredHandoffValues == AsyncDeferredHandoffSet

Stage2DeferredHandoffTypeInvariant ==
  asyncDeferredHandoffs
    \in [ValidatorIds -> Stage2DeferredHandoffValues]

Stage2DeferredHandoffInit ==
  asyncDeferredHandoffs =
    [node \in ValidatorIds |-> Stage2NoDeferredHandoff]

Stage2DeferredHandoffOwned(candidate) ==
  /\ candidate.node \in ValidatorIds
  /\ asyncDeferredHandoffs[candidate.node]
       = Stage2ActiveDeferredHandoff(candidate)
  /\ candidate \in DeferredCandidates

THEOREM Stage2DeferredHandoffTokenIsInjectiveObligation ==
  \A left, right \in AsyncCandidateSet:
    Stage2DeferredHandoffToken(left)
      = Stage2DeferredHandoffToken(right)
      => left = right
BY ExactCandidateIdentityIffCandidateEquality, SMT
   DEF Stage2DeferredHandoffToken

Stage2BusyRejectedSelected(candidate) ==
  /\ ProtectedStage2Owned(candidate)
  /\ ~NodeIdle(candidate.node)
  /\ asyncDeferredDrainOwed[candidate.node]
  /\ DeferredQueueNonempty(candidate.node)
  /\ NextDeferredCommand(candidate.node) = candidate
  /\ ~CommandDispatchable(candidate)

Stage2BusyRetryClaimsHandoff(candidate) ==
  /\ Stage2BusyRejectedSelected(candidate)
  /\ ~DeferredHandoffActive(candidate.node)
  /\ DeferredDrainStep(candidate.node)
  /\ asyncDeferredHandoffs'[candidate.node]
       = Stage2ActiveDeferredHandoff(candidate)
  /\ \A other \in ValidatorIds \ {candidate.node}:
       asyncDeferredHandoffs'[other] = asyncDeferredHandoffs[other]

Stage2ExactIdleRetryPending(candidate) ==
  /\ Stage2DeferredHandoffOwned(candidate)
  /\ ProtectedStage2Owned(candidate)
  /\ NodeIdle(candidate.node)
  /\ DeferredHandoffQueueHead(candidate.node)

Stage2ExactIdleRetrySelected(candidate) ==
  /\ Stage2ExactIdleRetryPending(candidate)
  /\ asyncDeferredDrainOwed[candidate.node]
  /\ DeferredQueueNonempty(candidate.node)
  /\ Stage2DeferredHandoffToken(NextDeferredCommand(candidate.node))
       = Stage2DeferredHandoffToken(candidate)

Stage2ExactHandoffConsumed(candidate) ==
  /\ Stage2ExactIdleRetrySelected(candidate)
  /\ DeferredDrainStep(candidate.node)
  /\ ~ResponsiveProtectedCandidateOwned(candidate)'

Stage2HandoffRetentionAction(candidate) ==
  /\ Stage2DeferredHandoffOwned(candidate)
  /\ [AsyncNext]_AsyncAllVars
  /\ candidate \in DeferredCandidates'
  => asyncDeferredHandoffs'[candidate.node]
       = asyncDeferredHandoffs[candidate.node]

Stage2HandoffClearOnlyOnExitAction(candidate) ==
  /\ Stage2DeferredHandoffOwned(candidate)
  /\ [AsyncNext]_AsyncAllVars
  /\ asyncDeferredHandoffs'[candidate.node]
       # asyncDeferredHandoffs[candidate.node]
  => candidate \notin DeferredCandidates'

Stage2HandoffCreationOnlyOnBusyRetryAction(node) ==
  /\ node \in ValidatorIds
  /\ [AsyncNext]_AsyncAllVars
  /\ asyncDeferredHandoffs[node] = Stage2NoDeferredHandoff
  /\ asyncDeferredHandoffs'[node] # Stage2NoDeferredHandoff
  => \E candidate \in AsyncCandidateSet:
       /\ candidate.node = node
       /\ Stage2BusyRejectedSelected(candidate)
       /\ DeferredDrainStep(node)
       /\ asyncDeferredHandoffs'[node]
            = Stage2ActiveDeferredHandoff(candidate)

(***************************************************************************
Safety half: a foreign deferred selection cannot turn an idle node Busy while
the exact handoff is outstanding.  An ordinary FIFO selection may still begin
terminating local work; the ReadyRun auxiliary rank and rearm obligations
below account for that finite detour.  This predicate rejects only the foreign
Progress/Normal deferred blocker which closes the equal-rank cycle above.
***************************************************************************)

Stage2NoForeignDeferredRebusyAction(candidate) ==
  /\ Stage2DeferredHandoffOwned(candidate)
  /\ NodeIdle(candidate.node)
  /\ asyncDeferredDrainOwed[candidate.node]
  /\ DeferredQueueNonempty(candidate.node)
  /\ DeferredHandoffBlocksExecution(
       candidate.node, NextDeferredCommand(candidate.node))
  /\ [AsyncNext]_AsyncAllVars
  /\ DeferredDrainStep(candidate.node)
  => /\ NodeIdle(candidate.node)'
     /\ Stage2DeferredHandoffOwned(candidate)'

Stage2HandoffCursorDistance(candidate) ==
  CommandClassDistance(
    asyncNextDeferredClass[candidate.node], candidate.class)

Stage2ForeignIdleSkip(candidate) ==
  /\ Stage2ExactIdleRetryPending(candidate)
  /\ asyncDeferredDrainOwed[candidate.node]
  /\ NextDeferredCommand(candidate.node) # candidate
  /\ DeferredHandoffBlocksExecution(
       candidate.node, NextDeferredCommand(candidate.node))
  /\ ~DeferredHandoffAllowsExecution(
       candidate.node, NextDeferredCommand(candidate.node))
  /\ DeferredDrainStep(candidate.node)

THEOREM Stage2SelectedDifferentDeferredClassDropsDistance ==
  \A node \in ValidatorIds, targetClass \in AsyncCommandClasses:
    /\ asyncNextDeferredClass[node] \in AsyncCommandClasses
    /\ DeferredClassNonempty(node, targetClass)
    /\ SelectedDeferredClass(node) # targetClass
    => CommandClassDistance(
         NextCommandClass(SelectedDeferredClass(node)), targetClass)
         < CommandClassDistance(
             asyncNextDeferredClass[node], targetClass)
BY SMTT(30)
   DEF SelectedDeferredClass, CommandClassDistance,
       NextCommandClass, AsyncCommandClasses

THEOREM Stage2ForeignIdleSkipDropsCursorDistanceObligation ==
  \A candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2ForeignIdleSkip(candidate)
    => /\ Stage2DeferredHandoffOwned(candidate)'
       /\ NodeIdle(candidate.node)'
       /\ ~asyncDeferredDrainOwed'[candidate.node]
       /\ Stage2HandoffCursorDistance(candidate)'
            < Stage2HandoffCursorDistance(candidate)
BY AsyncStrongTypeProjectsAsyncType,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   Stage2DeferredHandoffTokenIsInjectiveObligation,
   Stage2SelectedDifferentDeferredClassDropsDistance,
   RuntimeSelectedCommandsAreTyped, Isa
   DEF Stage2ForeignIdleSkip, Stage2ExactIdleRetryPending,
       Stage2DeferredHandoffOwned, Stage2ActiveDeferredHandoff,
       Stage2DeferredHandoffToken, Stage2HandoffCursorDistance,
       ProtectedStage2Owned, DeferredDrainStep,
       DeferredHandoffAllowsExecution,
       DeferredHandoffBlocksExecution, DeferredHandoffActive,
       DeferredHandoffMatches, DeferredHandoffQueueHead,
       DeferredHandoffCandidate, RetainDeferredHandoffs,
       AdvanceNextDeferredClass, NextDeferredCommand,
       SelectedDeferredClass, DeferredClassQueue,
       DeferredClassNonempty, NodeIdle,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateScheduled,
       DeferredCandidates, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant, AsyncAllVars

(***************************************************************************
Temporal handoff closure.

An idle foreign skip deliberately clears `asyncDeferredDrainOwed`: that is
the same strict ReadyRun auxiliary-rank decrease used by the production
scheduler proof.  The next attempt therefore depends on the concrete
timeout/retransmit rearm path, not on an implicit continuously enabled drain.
The obligations below expose that dependency and then use the finite
three-class cursor to reach the exact held head.  No theorem assumes the
desired exact retry as a fairness premise.
***************************************************************************)

Stage2HandoffProgressExit(candidate) ==
  \/ ~Stage2DeferredHandoffOwned(candidate)
  \/ ProtectedServiceOwnershipExit(candidate)

Stage2IdleHandoffAwaitingRearm(candidate) ==
  /\ Stage2ExactIdleRetryPending(candidate)
  /\ ~asyncDeferredDrainOwed[candidate.node]

Stage2IdleHandoffAtDistance(candidate, distance) ==
  /\ Stage2ExactIdleRetryPending(candidate)
  /\ Stage2HandoffCursorDistance(candidate) = distance

Stage2IdleHandoffCursorProgress(candidate, distance) ==
  \/ Stage2HandoffProgressExit(candidate)
  \/ Stage2ExactIdleRetrySelected(candidate)
  \/ \E lower \in 0..2:
       /\ lower < distance
       /\ Stage2IdleHandoffAtDistance(candidate, lower)

THEOREM Stage2AsyncNextPreservesDeferredHandoffOwnership ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncDeferredHandoffOwnershipInvariant
  /\ [AsyncNext]_AsyncAllVars
  => AsyncDeferredHandoffOwnershipInvariant'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   AsyncDeferredHandoffOwnershipStutter,
   RuntimeSelectedCommandsAreTyped, HeadTailProperties, IsaT(120)
   DEF AsyncDeferredHandoffOwnershipInvariant,
       AsyncDeferredHandoffOwnershipVars,
       DeferredHandoffActive, DeferredHandoffCandidate,
       DeferredHandoffQueueHead, DeferredHandoffMatches,
       DeferredHandoffAllowsExecution,
       DeferredHandoffBlocksExecution, DeferredClassNonempty,
       DeferredClassQueue, SelectedDeferredClass,
       NextDeferredCommand, DeferredDrainStep,
       RemoveNextDeferredCommand, AdvanceNextDeferredClass,
       InstallDeferredHandoff, RetainDeferredHandoffs,
       ClearDeferredHandoff, FifoRuntimeStep, DeferCommand,
       DiscardCommand, RuntimeStep, SerializedRunnerRuntimeStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RunNode, RunNodeWork, AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep, AsyncAllVars

THEOREM AsyncSpecAlwaysDeferredHandoffOwnershipObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []AsyncDeferredHandoffOwnershipInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []AsyncDeferredHandoffOwnershipInvariant
    <2>1. AsyncInitAt(initialContext)
             => AsyncDeferredHandoffOwnershipInvariant
      BY AsyncInitEstablishesDeferredHandoffOwnership
    <2>2. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant, PTL
    <2>3. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ AsyncDeferredHandoffOwnershipInvariant
           /\ [AsyncNext]_AsyncAllVars
          => AsyncDeferredHandoffOwnershipInvariant'
      BY Stage2AsyncNextPreservesDeferredHandoffOwnership
    <2> QED BY <2>1, <2>2, <2>3, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM Stage2HandoffCursorDistanceInCarrierObligation ==
  \A candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncDeferredHandoffOwnershipInvariant
    /\ Stage2ExactIdleRetryPending(candidate)
    => Stage2HandoffCursorDistance(candidate) \in 0..2
BY AsyncStrongTypeProjectsAsyncType, SMTT(30)
   DEF Stage2HandoffCursorDistance, Stage2ExactIdleRetryPending,
       Stage2DeferredHandoffOwned, ProtectedStage2Owned,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncDeferredTypeInvariant,
       AsyncDeferredTopologyTypeInvariant, AsyncCandidateTyped,
       AsyncCandidateSet, AsyncCommandClasses,
       CommandClassDistance, NextCommandClass

THEOREM Stage2IdleHandoffDrainRearmedObligation ==
  \A initialContext, candidate:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ Stage2BusyKernelProperty(AsyncSpecAt(initialContext))
      => Stage2IdleHandoffAwaitingRearm(candidate)
           ~> (Stage2HandoffProgressExit(candidate)
                \/ (Stage2ExactIdleRetryPending(candidate)
                     /\ asyncDeferredDrainOwed[candidate.node]))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate,
                AsyncSpecAt(initialContext),
                ProtectedPostDeferredRankProgressProperty(
                  AsyncSpecAt(initialContext)),
                Stage2BusyKernelProperty(
                  AsyncSpecAt(initialContext))
         PROVE Stage2IdleHandoffAwaitingRearm(candidate)
                 ~> (Stage2HandoffProgressExit(candidate)
                      \/ (Stage2ExactIdleRetryPending(candidate)
                           /\ asyncDeferredDrainOwed[candidate.node]))
    <2> DEFINE Goal ==
           Stage2HandoffProgressExit(candidate)
             \/ (Stage2ExactIdleRetryPending(candidate)
                  /\ asyncDeferredDrainOwed[candidate.node])
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ AsyncDeferredHandoffOwnershipInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysDeferredHandoffOwnershipObligation, PTL
    <2>2. (ProtectedStage2Owned(candidate)
             /\ ~NodeIdle(candidate.node))
             ~> (ProtectedServiceOwnershipExit(candidate)
                  \/ NodeIdle(candidate.node))
      BY <1>1, ProtectedStage2BusyTerminatesLocallyObligation
    <2>3. AsyncSpecAt(initialContext)
             => (Stage2IdleHandoffAwaitingRearm(candidate)
                   ~> (Goal
                        \/ (Stage2DeferredHandoffOwned(candidate)
                             /\ ProtectedStage2Owned(candidate)
                             /\ ~NodeIdle(candidate.node)
                             /\ asyncDeferredDrainOwed[candidate.node])))
      BY <1>1, <2>1, Stage2DeferredHandoffTokenIsInjectiveObligation,
         ReadyRunAuxOrderingIsWellFounded,
         ProtectedOwnedCandidateEnablesFairRunNode,
         LocalAdmissionStrictlyDecreasesRuntimeReach,
         SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
         IngressDrainStrictlyDecreasesRuntimeReach,
         IsaT(180), PTL
         DEF Goal, Stage2IdleHandoffAwaitingRearm,
             Stage2HandoffProgressExit,
             Stage2ExactIdleRetryPending,
             Stage2DeferredHandoffOwned,
             Stage2ActiveDeferredHandoff,
             Stage2DeferredHandoffToken, ProtectedStage2Owned,
             ProtectedServiceOwnershipExit,
             ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned, CandidateScheduled,
             DeferredCandidates, CandidateServiceRank,
             ReadyRunAuxRank, ReadyRunAuxOrdering,
             ReadyRunAuxCarrier, ReadyRunDeferredRank,
             ReadyRunTimeoutRank, ReadyRunInnerRank,
             ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
             ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
             PostGstRunNode, RunNode, RunNodeWork,
             LocalAdmissionStep, SelectedLocalAdmissionAdvance,
             SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
             SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             AsyncServeIngressTargetOnlyTurn,
             RuntimeStep, DirectTimeoutStep,
             DirectRetransmitStep, DeferredTagStep,
             DeferredTimeoutStep, DeferredRetransmitStep,
             FifoRuntimeStep, IdleRuntimeStep, AsyncTick,
             AsyncTickEnabled, TimeoutDue, RetransmitDue,
             DeferredHandoffQueueHead, DeferredHandoffMatches,
             DeferredHandoffAllowsExecution,
             DeferredHandoffBlocksExecution, DeferredDrainStep,
             AsyncSpecAt, AsyncFairnessAt, AsyncFairActionAt,
             AsyncNext, AsyncAllVars
    <2>4. (Stage2DeferredHandoffOwned(candidate)
             /\ ProtectedStage2Owned(candidate)
             /\ ~NodeIdle(candidate.node)
             /\ asyncDeferredDrainOwed[candidate.node])
             ~> Goal
      BY <2>1, <2>2, PTL
         DEF Goal, Stage2HandoffProgressExit,
             Stage2ExactIdleRetryPending,
             Stage2DeferredHandoffOwned,
             ProtectedStage2Owned, ProtectedServiceOwnershipExit,
             AsyncDeferredHandoffOwnershipInvariant,
             DeferredHandoffActive, DeferredHandoffCandidate,
             DeferredHandoffQueueHead
    <2> QED BY <2>3, <2>4, PTL DEF Goal
  <1> QED BY <1>1

THEOREM Stage2IdleHandoffCursorOneStepObligation ==
  \A initialContext, candidate:
    \A distance \in 0..2:
      /\ AsyncSpecAt(initialContext)
      /\ ProtectedPostDeferredRankProgressProperty(
           AsyncSpecAt(initialContext))
      /\ Stage2BusyKernelProperty(AsyncSpecAt(initialContext))
        => Stage2IdleHandoffAtDistance(candidate, distance)
             ~> Stage2IdleHandoffCursorProgress(candidate, distance)
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate,
                NEW distance \in 0..2,
                AsyncSpecAt(initialContext),
                ProtectedPostDeferredRankProgressProperty(
                  AsyncSpecAt(initialContext)),
                Stage2BusyKernelProperty(
                  AsyncSpecAt(initialContext))
         PROVE Stage2IdleHandoffAtDistance(candidate, distance)
                 ~> Stage2IdleHandoffCursorProgress(candidate, distance)
    <2>1. Stage2IdleHandoffAwaitingRearm(candidate)
               ~> (Stage2HandoffProgressExit(candidate)
                    \/ (Stage2ExactIdleRetryPending(candidate)
                         /\ asyncDeferredDrainOwed[candidate.node]))
      BY <1>1, Stage2IdleHandoffDrainRearmedObligation
    <2>2. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ AsyncDeferredHandoffOwnershipInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysDeferredHandoffOwnershipObligation, PTL
    <2>3. (Stage2ExactIdleRetryPending(candidate)
              /\ asyncDeferredDrainOwed[candidate.node]
              /\ Stage2HandoffCursorDistance(candidate) = distance)
             ~> Stage2IdleHandoffCursorProgress(candidate, distance)
      BY <1>1, <2>2,
         Stage2ForeignIdleSkipDropsCursorDistanceObligation,
         Stage2DeferredHandoffTokenIsInjectiveObligation,
         Stage2SelectedDifferentDeferredClassDropsDistance,
         ReadyRunAuxOrderingIsWellFounded,
         ProtectedOwnedCandidateEnablesFairRunNode,
         LocalAdmissionStrictlyDecreasesRuntimeReach,
         SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
         IngressDrainStrictlyDecreasesRuntimeReach,
         HeadTailProperties, IsaT(180), PTL
         DEF Stage2IdleHandoffCursorProgress,
             Stage2IdleHandoffAtDistance,
             Stage2HandoffProgressExit,
             Stage2HandoffCursorDistance,
             Stage2ExactIdleRetrySelected,
             Stage2ExactIdleRetryPending,
             Stage2ForeignIdleSkip, Stage2DeferredHandoffOwned,
             Stage2ActiveDeferredHandoff,
             Stage2DeferredHandoffToken, ProtectedStage2Owned,
             ProtectedServiceOwnershipExit,
             ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned, CandidateScheduled,
             DeferredCandidates, CandidateServiceRank,
             ServiceRankLess, ReadyRunAuxRank,
             ReadyRunAuxOrdering, ReadyRunAuxCarrier,
             ReadyRunDeferredRank, ReadyRunTimeoutRank,
             ReadyRunInnerRank, ReadyFifoDebt,
             ReadyDeferredCount, ReadyTimeoutDebt,
             ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
             PostGstRunNode, RunNode, RunNodeWork,
             LocalAdmissionStep, SelectedLocalAdmissionAdvance,
             SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
             SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             AsyncServeIngressTargetOnlyTurn, RuntimeStep,
             DeferredDrainStep, RemoveNextDeferredCommand,
             AdvanceNextDeferredClass, DeferredHandoffQueueHead,
             DeferredHandoffMatches,
             DeferredHandoffAllowsExecution,
             DeferredHandoffBlocksExecution,
             InstallDeferredHandoff, RetainDeferredHandoffs,
             ClearDeferredHandoff, NextDeferredCommand,
             SelectedDeferredClass, DeferredClassQueue,
             CommandDispatchable, DiscardCommand,
             AsyncSpecAt, AsyncFairnessAt, AsyncFairActionAt,
             AsyncNext, AsyncAllVars
    <2>4. Stage2IdleHandoffAtDistance(candidate, distance)
             ~> (Stage2HandoffProgressExit(candidate)
                  \/ (Stage2ExactIdleRetryPending(candidate)
                       /\ asyncDeferredDrainOwed[candidate.node]
                       /\ Stage2HandoffCursorDistance(candidate)
                            = distance))
      BY <2>1, PTL
         DEF Stage2IdleHandoffAtDistance,
             Stage2IdleHandoffAwaitingRearm,
             Stage2HandoffProgressExit
    <2> QED BY <2>3, <2>4, PTL
         DEF Stage2IdleHandoffCursorProgress
  <1> QED BY <1>1

THEOREM Stage2ExactIdleRetryEventuallySelectedObligation ==
  \A initialContext, candidate:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ Stage2BusyKernelProperty(AsyncSpecAt(initialContext))
      => Stage2ExactIdleRetryPending(candidate)
           ~> (Stage2HandoffProgressExit(candidate)
                \/ Stage2ExactIdleRetrySelected(candidate))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate,
                AsyncSpecAt(initialContext),
                ProtectedPostDeferredRankProgressProperty(
                  AsyncSpecAt(initialContext)),
                Stage2BusyKernelProperty(
                  AsyncSpecAt(initialContext))
         PROVE Stage2ExactIdleRetryPending(candidate)
                 ~> (Stage2HandoffProgressExit(candidate)
                      \/ Stage2ExactIdleRetrySelected(candidate))
    <2> DEFINE Goal ==
           Stage2HandoffProgressExit(candidate)
             \/ Stage2ExactIdleRetrySelected(candidate)
    <2>1. IsWellFoundedOn(OpToRel(<, Nat), 0..2)
      BY NatLessThanWellFounded, IsWellFoundedOnSubset, Isa
    <2>2. ASSUME NEW distance \in 0..2
           PROVE Stage2IdleHandoffAtDistance(candidate, distance)
                   ~> (Goal
                        \/ \E lower \in SetLessThan(
                             distance, OpToRel(<, Nat), 0..2):
                             Stage2IdleHandoffAtDistance(
                               candidate, lower))
      BY <1>1, Stage2IdleHandoffCursorOneStepObligation
         DEF Goal, Stage2IdleHandoffCursorProgress, SetLessThan
    <2>3. \A distance \in 0..2:
             Stage2IdleHandoffAtDistance(candidate, distance)
               ~> Goal
      BY <2>1, <2>2, WellFoundedLeadsTo
    <2>4. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncDeferredHandoffOwnershipInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysDeferredHandoffOwnershipObligation, PTL
    <2>5. Stage2ExactIdleRetryPending(candidate)
             ~> \E distance \in 0..2:
                  Stage2IdleHandoffAtDistance(candidate, distance)
      BY <2>4, Stage2HandoffCursorDistanceInCarrierObligation, PTL
         DEF Stage2IdleHandoffAtDistance
    <2> QED BY <2>3, <2>5, PTL DEF Goal
  <1> QED BY <1>1

THEOREM Stage2ExactIdleRetryDrainConsumesObligation ==
  \A candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncDeferredHandoffOwnershipInvariant
    /\ Stage2ExactIdleRetrySelected(candidate)
    /\ DeferredDrainStep(candidate.node)
    => /\ ~ResponsiveProtectedCandidateOwned(candidate)'
       /\ ~Stage2DeferredHandoffOwned(candidate)'
BY AsyncStrongTypeProjectsAsyncType,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   Stage2DeferredHandoffTokenIsInjectiveObligation,
   RuntimeSelectedCommandsAreTyped, HeadTailProperties,
   IsaT(180)
   DEF Stage2ExactIdleRetrySelected,
       Stage2ExactIdleRetryPending,
       Stage2DeferredHandoffOwned,
       Stage2ActiveDeferredHandoff,
       Stage2DeferredHandoffToken, ProtectedStage2Owned,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CandidateScheduled, DeferredCandidates,
       DeferredDrainStep, DeferredHandoffActive,
       DeferredHandoffMatches, DeferredHandoffQueueHead,
       DeferredHandoffCandidate, DeferredHandoffAllowsExecution,
       DeferredHandoffBlocksExecution, RemoveNextDeferredCommand,
       ClearDeferredHandoff, RetainDeferredHandoffs,
       DiscardCommand, ExecuteCommand, AppendCausalSuccessors,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage2ExactIdleRetryServedObligation ==
  \A initialContext, candidate:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
      => Stage2ExactIdleRetrySelected(candidate)
           ~> Stage2HandoffProgressExit(candidate)
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate,
                AsyncSpecAt(initialContext),
                ProtectedPostDeferredRankProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE Stage2ExactIdleRetrySelected(candidate)
                 ~> Stage2HandoffProgressExit(candidate)
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ AsyncDeferredHandoffOwnershipInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysDeferredHandoffOwnershipObligation, PTL
    <2>2. [] [(\/ ~Stage2ExactIdleRetrySelected(candidate)
                  \/ ~DeferredDrainStep(candidate.node)
                  \/ Stage2HandoffProgressExit(candidate)')]_AsyncAllVars
      BY <2>1, Stage2ExactIdleRetryDrainConsumesObligation, PTL
         DEF Stage2HandoffProgressExit
    <2>3. Stage2ExactIdleRetrySelected(candidate)
             ~> Stage2HandoffProgressExit(candidate)
      BY <1>1, <2>1, <2>2,
         Stage2DeferredHandoffTokenIsInjectiveObligation,
         ReadyRunAuxOrderingIsWellFounded,
         ProtectedOwnedCandidateEnablesFairRunNode,
         LocalAdmissionStrictlyDecreasesRuntimeReach,
         SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
         IngressDrainStrictlyDecreasesRuntimeReach,
         HeadTailProperties, IsaT(180), PTL
         DEF Stage2HandoffProgressExit,
             Stage2ExactIdleRetrySelected,
             Stage2ExactIdleRetryPending,
             Stage2DeferredHandoffOwned,
             Stage2ActiveDeferredHandoff,
             Stage2DeferredHandoffToken, ProtectedStage2Owned,
             ProtectedServiceOwnershipExit,
             ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned, CandidateScheduled,
             DeferredCandidates, CandidateServiceRank,
             ServiceRankLess, ReadyRunAuxRank,
             ReadyRunAuxOrdering, ReadyRunAuxCarrier,
             ReadyRunDeferredRank, ReadyRunTimeoutRank,
             ReadyRunInnerRank, ReadyFifoDebt,
             ReadyDeferredCount, ReadyTimeoutDebt,
             ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
             PostGstRunNode, RunNode, RunNodeWork,
             LocalAdmissionStep, SelectedLocalAdmissionAdvance,
             SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
             SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             AsyncServeIngressTargetOnlyTurn, RuntimeStep,
             DeferredDrainStep, RemoveNextDeferredCommand,
             AdvanceNextDeferredClass, DeferredHandoffQueueHead,
             DeferredHandoffMatches,
             DeferredHandoffAllowsExecution,
             DeferredHandoffBlocksExecution,
             InstallDeferredHandoff, RetainDeferredHandoffs,
             ClearDeferredHandoff, NextDeferredCommand,
             SelectedDeferredClass, DeferredClassQueue,
             CommandDispatchable, DiscardCommand,
             AsyncSpecAt, AsyncFairnessAt, AsyncFairActionAt,
             AsyncNext, AsyncAllVars
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM Stage2IdleHandoffEventuallyExitsObligation ==
  \A initialContext, candidate:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ Stage2BusyKernelProperty(AsyncSpecAt(initialContext))
      => Stage2ExactIdleRetryPending(candidate)
           ~> Stage2HandoffProgressExit(candidate)
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate,
                AsyncSpecAt(initialContext),
                ProtectedPostDeferredRankProgressProperty(
                  AsyncSpecAt(initialContext)),
                Stage2BusyKernelProperty(
                  AsyncSpecAt(initialContext))
         PROVE Stage2ExactIdleRetryPending(candidate)
                 ~> Stage2HandoffProgressExit(candidate)
    <2>1. Stage2ExactIdleRetryPending(candidate)
             ~> (Stage2HandoffProgressExit(candidate)
                  \/ Stage2ExactIdleRetrySelected(candidate))
      BY <1>1, Stage2ExactIdleRetryEventuallySelectedObligation
    <2>2. Stage2ExactIdleRetrySelected(candidate)
             ~> Stage2HandoffProgressExit(candidate)
      BY <1>1, Stage2ExactIdleRetryServedObligation
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

(***************************************************************************
Token-realization contract.

Acquisition is allowed only on the concrete Busy deferred-drain edge;
retention is exact while the candidate remains in its queue; changing the
token removes that exact queue owner; and an idle foreign selection strictly
lowers the three-class cursor distance without creating Busy.  Together with
the independently proved Busy-phase termination and fair RunNode cycle, these
safety facts supply the eventual exact retry without postulating it as an
uncheckable fairness slogan.  Equality is over
`ExactAsyncCandidateIdentity`, so a same-class/same-kind successor, a
reconnect, or a foreign source cannot satisfy the handoff accidentally.
***************************************************************************)

Stage2DeferredHandoffIdleReadyInvariant ==
  \A candidate \in AsyncCandidateSet:
    /\ Stage2DeferredHandoffOwned(candidate)
    => DeferredHandoffQueueHead(candidate.node)

Stage2ExactDeferredHandoffProperty(specification) ==
  specification
    => /\ Stage2DeferredHandoffInit
       /\ []Stage2DeferredHandoffTypeInvariant
       /\ []AsyncDeferredHandoffOwnershipInvariant
       /\ []Stage2DeferredHandoffIdleReadyInvariant
       /\ [] [(\A candidate \in AsyncCandidateSet:
                 /\ Stage2BusyRejectedSelected(candidate)
                 /\ ~DeferredHandoffActive(candidate.node)
                 /\ DeferredDrainStep(candidate.node)
                 => Stage2BusyRetryClaimsHandoff(candidate))]_AsyncAllVars
       /\ [] [(\A candidate \in AsyncCandidateSet:
                 Stage2HandoffRetentionAction(candidate))]_AsyncAllVars
       /\ [] [(\A candidate \in AsyncCandidateSet:
                 Stage2HandoffClearOnlyOnExitAction(candidate))]_AsyncAllVars
       /\ [] [(\A node \in ValidatorIds:
                 Stage2HandoffCreationOnlyOnBusyRetryAction(node))]_AsyncAllVars
       /\ [] [(\A candidate \in AsyncCandidateSet:
                 Stage2NoForeignDeferredRebusyAction(candidate))]_AsyncAllVars

THEOREM AsyncSpecHasExactDeferredHandoffObligation ==
  \A initialContext:
    Stage2ExactDeferredHandoffProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE Stage2ExactDeferredHandoffProperty(
                 AsyncSpecAt(initialContext))
    <2>1. AsyncSpecAt(initialContext) => Stage2DeferredHandoffInit
      BY Isa
         DEF AsyncSpecAt, AsyncInitAt, AsyncBaseInitAt,
             AsyncDeferredInit, Stage2DeferredHandoffInit,
             Stage2NoDeferredHandoff, NoAsyncDeferredHandoff
    <2>2. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ AsyncDeferredHandoffOwnershipInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysDeferredHandoffOwnershipObligation, PTL
    <2>3. AsyncStrongTypeInvariant
             => Stage2DeferredHandoffTypeInvariant
      BY AsyncStrongTypeProjectsAsyncType
         DEF Stage2DeferredHandoffTypeInvariant,
             Stage2DeferredHandoffValues,
             AsyncDeferredHandoffSet, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncDeferredTopologyTypeInvariant
    <2>4. AsyncDeferredHandoffOwnershipInvariant
             => Stage2DeferredHandoffIdleReadyInvariant
      BY Isa
         DEF Stage2DeferredHandoffIdleReadyInvariant,
             Stage2DeferredHandoffOwned,
             Stage2ActiveDeferredHandoff,
             AsyncDeferredHandoffOwnershipInvariant,
             DeferredHandoffActive, DeferredHandoffCandidate,
             DeferredHandoffQueueHead, DeferredCandidates,
             DeferredClassQueue, SequenceSet
    <2>5. \A candidate \in AsyncCandidateSet:
             /\ Stage2BusyRejectedSelected(candidate)
             /\ ~DeferredHandoffActive(candidate.node)
             /\ DeferredDrainStep(candidate.node)
            => Stage2BusyRetryClaimsHandoff(candidate)
      BY IsaT(60)
         DEF Stage2BusyRejectedSelected,
             Stage2BusyRetryClaimsHandoff,
             Stage2ActiveDeferredHandoff,
             ProtectedStage2Owned, DeferredDrainStep,
             DeferredHandoffAllowsExecution,
             DeferredHandoffBlocksExecution,
             DeferredHandoffActive, DeferredHandoffMatches,
             InstallDeferredHandoff, RetainDeferredHandoffs,
             AsyncDeferredHandoff, NoAsyncDeferredHandoff
    <2>6. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ AsyncDeferredHandoffOwnershipInvariant
           /\ [AsyncNext]_AsyncAllVars
          => /\ \A candidate \in AsyncCandidateSet:
                   Stage2HandoffRetentionAction(candidate)
             /\ \A candidate \in AsyncCandidateSet:
                   Stage2HandoffClearOnlyOnExitAction(candidate)
             /\ \A node \in ValidatorIds:
                   Stage2HandoffCreationOnlyOnBusyRetryAction(node)
             /\ \A candidate \in AsyncCandidateSet:
                   Stage2NoForeignDeferredRebusyAction(candidate)
      BY Stage2DeferredHandoffTokenIsInjectiveObligation,
         RuntimeSelectedCommandsAreTyped, HeadTailProperties,
         IsaT(180)
         DEF Stage2HandoffRetentionAction,
             Stage2HandoffClearOnlyOnExitAction,
             Stage2HandoffCreationOnlyOnBusyRetryAction,
             Stage2NoForeignDeferredRebusyAction,
             Stage2BusyRejectedSelected,
             Stage2DeferredHandoffOwned,
             Stage2ActiveDeferredHandoff,
             Stage2DeferredHandoffToken, ProtectedStage2Owned,
             ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned, CandidateScheduled,
             DeferredCandidates, DeferredDrainStep,
             DeferredHandoffAllowsExecution,
             DeferredHandoffBlocksExecution,
             DeferredHandoffActive, DeferredHandoffMatches,
             DeferredHandoffQueueHead, DeferredHandoffCandidate,
             InstallDeferredHandoff, RetainDeferredHandoffs,
             ClearDeferredHandoff, RemoveNextDeferredCommand,
             AdvanceNextDeferredClass, NextDeferredCommand,
             SelectedDeferredClass, DeferredClassQueue,
             FifoRuntimeStep, DeferCommand, DiscardCommand,
             RuntimeStep, SerializedRunnerRuntimeStep,
             SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             LocalAdmissionStep, SelectedLocalAdmissionAdvance,
             SerializedLocalPrecedesServeIngressStep,
             AsyncServeIngressTargetOnlyTurn, RunNode,
             RunNodeWork, AsyncNext, AsyncNonCrashStep,
             AsyncRunnerStep, AsyncNonRunnerStep,
             AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant,
             AsyncOutstandingCarrierInvariant, AsyncAllVars
    <2>7. AsyncSpecAt(initialContext)
             => [] [(/\ (\A candidate \in AsyncCandidateSet:
                              Stage2HandoffRetentionAction(candidate))
                       /\ (\A candidate \in AsyncCandidateSet:
                              Stage2HandoffClearOnlyOnExitAction(candidate))
                       /\ (\A node \in ValidatorIds:
                              Stage2HandoffCreationOnlyOnBusyRetryAction(node))
                       /\ (\A candidate \in AsyncCandidateSet:
                              Stage2NoForeignDeferredRebusyAction(candidate)))]_AsyncAllVars
      BY <2>2, <2>6, PTL DEF AsyncSpecAt
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>7, PTL
         DEF Stage2ExactDeferredHandoffProperty
  <1> QED BY <1>1

(***************************************************************************
Pre-handoff stage-2 closure.

The first temporal leaf follows the exact deferred occurrence until either
its original service rank decreases or that same candidate acquires the
durable handoff.  A foreign selected class removes an earlier occurrence or
strictly lowers the cyclic cursor distance; selecting the target while Busy
installs its exact token.  The second leaf keeps that token and the original
rank bound coupled until Busy terminates and the idle exact retry either
executes or discards the target.  Neither leaf treats token loss alone as
progress.
***************************************************************************)

Stage2RankProgressExit(candidate, position) ==
  \/ ~ResponsiveProtectedCandidateOwned(candidate)
  \/ ServiceRankLess(
       CandidateServiceRank(candidate), <<2, position>>)

Stage2HandoffRankBlocked(candidate, position) ==
  /\ gst
  /\ ResponsiveProtectedCandidateOwned(candidate)
  /\ Stage2DeferredHandoffOwned(candidate)
  /\ ~ServiceRankLess(
       CandidateServiceRank(candidate), <<2, position>>)

Stage2RankOrHandoffProgress(candidate, position) ==
  \/ Stage2RankProgressExit(candidate, position)
  \/ Stage2HandoffRankBlocked(candidate, position)

THEOREM Stage2DeferredRankReachesExitOrExactHandoffObligation ==
  \A initialContext, candidate, position:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ Stage2BusyKernelProperty(AsyncSpecAt(initialContext))
    => ProtectedOwnedAtServiceRank(candidate, <<2, position>>)
         ~> Stage2RankOrHandoffProgress(candidate, position)
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                AsyncSpecAt(initialContext),
                ProtectedPostDeferredRankProgressProperty(
                  AsyncSpecAt(initialContext)),
                Stage2BusyKernelProperty(
                  AsyncSpecAt(initialContext))
         PROVE ProtectedOwnedAtServiceRank(candidate, <<2, position>>)
                 ~> Stage2RankOrHandoffProgress(candidate, position)
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ AsyncDeferredHandoffOwnershipInvariant
                    /\ Stage2BusyKernelInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysDeferredHandoffOwnershipObligation, PTL
         DEF Stage2BusyKernelProperty
    <2>2. (ProtectedStage2Owned(candidate)
             /\ ~NodeIdle(candidate.node))
             ~> (ProtectedServiceOwnershipExit(candidate)
                  \/ NodeIdle(candidate.node))
      BY <1>1, ProtectedStage2BusyTerminatesLocallyObligation
    <2>3. Stage2ExactDeferredHandoffProperty(
             AsyncSpecAt(initialContext))
      BY AsyncSpecHasExactDeferredHandoffObligation
    <2>4. ProtectedOwnedAtServiceRank(candidate, <<2, position>>)
             ~> Stage2RankOrHandoffProgress(candidate, position)
      BY <1>1, <2>1, <2>2, <2>3,
         Stage2SelectedDifferentDeferredClassDropsDistance,
         Stage2ForeignIdleSkipDropsCursorDistanceObligation,
         Stage2IdleHandoffEventuallyExitsObligation,
         Stage2DeferredHandoffTokenIsInjectiveObligation,
         ReadyRunAuxOrderingIsWellFounded,
         ReadyRunAuxRankInCarrier,
         ProtectedOwnedCandidateEnablesFairRunNode,
         LocalAdmissionStrictlyDecreasesRuntimeReach,
         SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
         IngressDrainStrictlyDecreasesRuntimeReach,
         HeadTailProperties, FS_CardinalityType,
         IsaT(300), PTL
         DEF Stage2RankOrHandoffProgress,
             Stage2RankProgressExit, Stage2HandoffRankBlocked,
             Stage2BusyKernelProperty,
             Stage2BusyKernelInvariant,
             Stage2DeferredHandoffOwned,
             Stage2ActiveDeferredHandoff,
             Stage2DeferredHandoffToken,
             Stage2BusyRejectedSelected,
             Stage2BusyRetryClaimsHandoff,
             ProtectedStage2Owned, ProtectedOwnedAtServiceRank,
             ProtectedServiceOwnershipExit,
             ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned, ProtectedServiceCandidate,
             CandidateScheduled, CandidateServiceRank,
             ServiceRankLess, DeferredCandidatePosition,
             DeferredCandidateIndices, DeferredClassPrefixIndices,
             DeferredCandidates, DeferredClassQueue,
             DeferredClassNonempty, DeferredQueueNonempty,
             DeferredHandoffActive, DeferredHandoffMatches,
             DeferredHandoffQueueHead, DeferredHandoffCandidate,
             DeferredHandoffAllowsExecution,
             DeferredHandoffBlocksExecution,
             InstallDeferredHandoff, RetainDeferredHandoffs,
             ClearDeferredHandoff, NextDeferredCommand,
             SelectedDeferredClass, AdvanceNextDeferredClass,
             RemoveNextDeferredCommand, CommandClassDistance,
             NextCommandClass, SequenceSet,
             ReadyRunAuxRank, ReadyRunAuxOrdering,
             ReadyRunAuxCarrier, ReadyRunDeferredRank,
             ReadyRunTimeoutRank, ReadyRunInnerRank,
             ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
             ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
             PostGstRunNode, RunNode, RunNodeWork,
             LocalAdmissionStep, SelectedLocalAdmissionAdvance,
             SerializedLocalPrecedesServeIngressStep,
             AdmitProducerCompletion, AdmitCausalHead, IngressDrainStep,
             SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             AsyncServeIngressTargetOnlyTurn, RuntimeStep,
             DeferredDrainStep, DeferredTagStep,
             DirectTimeoutStep, DirectRetransmitStep,
             FifoRuntimeStep, IdleRuntimeStep,
             RemoveNextNodeCommand, DeferCommand,
             DiscardCommand, CommandDispatchable,
             AsyncSpecAt, AsyncFairnessAt, AsyncFairActionAt,
             AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
             AsyncNonRunnerStep, AsyncAllVars
    <2> QED BY <2>4
  <1> QED BY <1>1

THEOREM Stage2ExactHandoffOwnershipReachesRankExitObligation ==
  \A initialContext, candidate, position:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ Stage2BusyKernelProperty(AsyncSpecAt(initialContext))
    => Stage2HandoffRankBlocked(candidate, position)
         ~> Stage2RankProgressExit(candidate, position)
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                AsyncSpecAt(initialContext),
                ProtectedPostDeferredRankProgressProperty(
                  AsyncSpecAt(initialContext)),
                Stage2BusyKernelProperty(
                  AsyncSpecAt(initialContext))
         PROVE Stage2HandoffRankBlocked(candidate, position)
                 ~> Stage2RankProgressExit(candidate, position)
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ AsyncDeferredHandoffOwnershipInvariant
                    /\ Stage2BusyKernelInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysDeferredHandoffOwnershipObligation, PTL
         DEF Stage2BusyKernelProperty
    <2>2. (ProtectedStage2Owned(candidate)
             /\ ~NodeIdle(candidate.node))
             ~> (ProtectedServiceOwnershipExit(candidate)
                  \/ NodeIdle(candidate.node))
      BY <1>1, ProtectedStage2BusyTerminatesLocallyObligation
    <2>3. Stage2ExactIdleRetryPending(candidate)
             ~> Stage2HandoffProgressExit(candidate)
      BY <1>1, Stage2IdleHandoffEventuallyExitsObligation
    <2>4. Stage2ExactDeferredHandoffProperty(
             AsyncSpecAt(initialContext))
      BY AsyncSpecHasExactDeferredHandoffObligation
    <2>5. Stage2HandoffRankBlocked(candidate, position)
             ~> Stage2RankProgressExit(candidate, position)
      BY <1>1, <2>1, <2>2, <2>3, <2>4,
         Stage2DeferredHandoffTokenIsInjectiveObligation,
         HeadTailProperties, IsaT(300), PTL
         DEF Stage2HandoffRankBlocked, Stage2RankProgressExit,
             Stage2HandoffProgressExit,
             Stage2ExactIdleRetryPending,
             Stage2DeferredHandoffOwned,
             Stage2ActiveDeferredHandoff,
             Stage2DeferredHandoffToken,
             Stage2HandoffRetentionAction,
             Stage2HandoffClearOnlyOnExitAction,
             Stage2DeferredHandoffIdleReadyInvariant,
             Stage2ExactDeferredHandoffProperty,
             ProtectedStage2Owned,
             ProtectedServiceOwnershipExit,
             ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned, CandidateScheduled,
             CandidateServiceRank, ServiceRankLess,
             DeferredCandidates, DeferredClassQueue,
             DeferredHandoffActive, DeferredHandoffCandidate,
             DeferredHandoffQueueHead, DeferredHandoffMatches,
             DeferredHandoffAllowsExecution,
             DeferredHandoffBlocksExecution,
             RemoveNextDeferredCommand, ClearDeferredHandoff,
             RetainDeferredHandoffs, DeferredDrainStep,
             FifoRuntimeStep, DeferCommand, DiscardCommand,
             RuntimeStep, SerializedRunnerRuntimeStep,
             SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             LocalAdmissionStep, SelectedLocalAdmissionAdvance,
             SerializedLocalPrecedesServeIngressStep,
             AsyncServeIngressTargetOnlyTurn, RunNode,
             RunNodeWork, AsyncNext, AsyncNonCrashStep,
             AsyncRunnerStep, AsyncNonRunnerStep, AsyncAllVars
    <2> QED BY <2>5
  <1> QED BY <1>1

THEOREM ProtectedStage2RankProgressWithExactHandoffObligation ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
    => ProtectedStage2RankProgressProperty(
         AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                ProtectedPostDeferredRankProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE ProtectedStage2RankProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. Stage2BusyKernelProperty(AsyncSpecAt(initialContext))
      BY AsyncSpecAlwaysStage2BusyKernelObligation
    <2>2. ASSUME NEW candidate \in AsyncCandidateSet,
                  NEW position \in Nat,
                  AsyncSpecAt(initialContext)
           PROVE ProtectedOwnedAtServiceRank(
                   candidate, <<2, position>>)
                   ~> Stage2RankProgressExit(candidate, position)
      <3>1. ProtectedOwnedAtServiceRank(candidate, <<2, position>>)
               ~> Stage2RankOrHandoffProgress(candidate, position)
        BY <1>1, <2>1,
           Stage2DeferredRankReachesExitOrExactHandoffObligation
      <3>2. Stage2HandoffRankBlocked(candidate, position)
               ~> Stage2RankProgressExit(candidate, position)
        BY <1>1, <2>1,
           Stage2ExactHandoffOwnershipReachesRankExitObligation
      <3> QED BY <3>1, <3>2, PTL
           DEF Stage2RankOrHandoffProgress
    <2> QED BY <2>2
         DEF ProtectedStage2RankProgressProperty,
             ProtectedOwnedAtServiceRank,
             Stage2RankProgressExit
  <1> QED BY <1>1

(***************************************************************************
Entry-38 composition boundary.

This is the only admissible route to the aggregate protected-rank theorem:
the exact stage-2 Busy/handoff leaf, the independently checked stage-3 and
stage-6 leaves, the existing stage-4 and stage-5 leaves, and the separate
fresh-nonce Serve FIFO theorem.  Omitting Serve would prove only the
candidate half of `ProtectedServiceRanksProgressProperty`.
***************************************************************************)

THEOREM ProtectedServiceRanksProgressLeafCompositionObligation ==
  \A initialContext:
    /\ ProtectedStage2RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedStage3RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedStage4RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedStage5RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedStage6RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedServeRankProgressProperty(
         AsyncSpecAt(initialContext))
    => ProtectedServiceRanksProgressProperty(
         AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ProtectedStage2RankProgressProperty(
                  AsyncSpecAt(initialContext)),
                ProtectedStage3RankProgressProperty(
                  AsyncSpecAt(initialContext)),
                ProtectedStage4RankProgressProperty(
                  AsyncSpecAt(initialContext)),
                ProtectedStage5RankProgressProperty(
                  AsyncSpecAt(initialContext)),
                ProtectedStage6RankProgressProperty(
                  AsyncSpecAt(initialContext)),
                ProtectedServeRankProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE ProtectedServiceRanksProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ProtectedServiceRankProgressProperty(
             AsyncSpecAt(initialContext))
      <3>1. ASSUME NEW candidate \in AsyncCandidateSet,
                    NEW stage \in 2..6, NEW position \in Nat,
                    AsyncSpecAt(initialContext)
             PROVE (gst
                      /\ ResponsiveProtectedCandidateOwned(candidate)
                      /\ CandidateServiceRank(candidate)
                           = <<stage, position>>)
                     ~> (~ResponsiveProtectedCandidateOwned(candidate)
                          \/ ServiceRankLess(
                               CandidateServiceRank(candidate),
                               <<stage, position>>))
        <4>1. CASE stage = 2
          BY <1>1, <3>1, <4>1
             DEF ProtectedStage2RankProgressProperty
        <4>2. CASE stage = 3
          BY <1>1, <3>1, <4>2
             DEF ProtectedStage3RankProgressProperty,
                 Stage3RankProgressExit
        <4>3. CASE stage = 4
          BY <1>1, <3>1, <4>3
             DEF ProtectedStage4RankProgressProperty
        <4>4. CASE stage = 5
          BY <1>1, <3>1, <4>4
             DEF ProtectedStage5RankProgressProperty
        <4>5. CASE stage = 6
          BY <1>1, <3>1, <4>5
             DEF ProtectedStage6RankProgressProperty
        <4> QED BY <3>1, <4>1, <4>2, <4>3, <4>4, <4>5, Isa
      <3> QED BY <3>1 DEF ProtectedServiceRankProgressProperty
    <2> QED BY <1>1, <2>1
         DEF ProtectedServiceRanksProgressProperty
  <1> QED BY <1>1


ProtectedServiceFiniteRunnerEpisodeClosureProperty(specification) ==
  /\ Stage3FiniteServeEpisodeResidualProperty(specification)
  /\ Stage6FiniteRunnerEpisodeClosureProperty(specification)

THEOREM ProtectedServiceRankProgressObligation ==
  \A initialContext:
    ProtectedServiceFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
      => ProtectedServiceRanksProgressProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ProtectedServiceFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext))
         PROVE ProtectedServiceRanksProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ProtectedStage3RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedStage3RankProgressFromFairSchedulerObligation
         DEF ProtectedServiceFiniteRunnerEpisodeClosureProperty
    <2>2. ProtectedStage4RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedStage4RankProgressFromFairScheduler
         DEF ProtectedServiceFiniteRunnerEpisodeClosureProperty,
             Stage6FiniteRunnerEpisodeClosureProperty
    <2>3. ProtectedStage5RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedStage5RankProgressFromFairFifo
    <2>4. ProtectedStage6RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedStage6RankProgressFromFairCausalAdmissionObligation
         DEF ProtectedServiceFiniteRunnerEpisodeClosureProperty
    <2>5. ProtectedServeRankProgressProperty(
             AsyncSpecAt(initialContext))
      BY ProtectedServeRankProgressFromFairFifo
    <2>6. ProtectedPostDeferredRankProgressProperty(
             AsyncSpecAt(initialContext))
      BY <2>1, <2>2, <2>3, <2>4,
         ProtectedPostDeferredRanksComposeFromLeavesObligation
    <2>7. ProtectedStage2RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY <2>6, ProtectedStage2RankProgressWithExactHandoffObligation,
         PTL
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>7,
         ProtectedServiceRanksProgressLeafCompositionObligation
  <1> QED BY <1>1

=============================================================================
