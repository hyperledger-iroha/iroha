---- MODULE SumeragiV2AsyncRecoveryVoteEpochBoundaryContinuationProofs ----
EXTENDS SumeragiV2AsyncRecoveryVoteEpochProofs

THEOREM AsyncTimeoutRecoveryEpisodeAfterTransitionPreservesCurrentBoundary ==
  ASSUME NEW preClockState, NEW episode,
         AsyncTimeoutRecoveryMutationFrameShape(episode),
         AsyncTimeoutRecoveryEpisodeBoundaryIn(
           episode, context', nodeView', generation', decisions')
  PROVE /\ AsyncTimeoutRecoveryMutationFrameShape(
             AsyncTimeoutRecoveryEpisodeAfterTransition(
               preClockState, episode))
        /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
             AsyncTimeoutRecoveryEpisodeAfterTransition(
               preClockState, episode),
             context', nodeView', generation', decisions')
PROOF
  <1>1. CASE AsyncTimeoutRecoveryExistingCaptureClearsThisStep(
                preClockState, episode.node)
    <2>1. AsyncTimeoutRecoveryEpisodeAfterTransition(
             preClockState, episode) =
           AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor(
             episode)
      BY <1>1, FunctionalReplacePreservesDomain,
         FunctionalReplaceUpdateAtKey, FunctionalUpdateAwayFromKey,
         Isa
         DEF AsyncTimeoutRecoveryMutationFrameShape,
             AsyncTimeoutRecoveryEpisodeAfterTransition,
             AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor
    <2>2. /\ AsyncTimeoutRecoveryMutationFrameShape(
                  AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor(
                    episode))
           /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor(
                  episode),
                context', nodeView', generation', decisions')
      BY AsyncTimeoutRecoveryClearedFrozenPredecessorPreservesCurrentBoundary
    <2> QED BY <2>1, <2>2
  <1>2. CASE ~AsyncTimeoutRecoveryExistingCaptureClearsThisStep(
                 preClockState, episode.node)
    BY <1>2 DEF AsyncTimeoutRecoveryEpisodeAfterTransition
  <1> QED BY <1>1, <1>2

THEOREM AsyncTimeoutRecoveryRetainedEpisodesHaveCurrentBoundary ==
  \A preClockState, state:
    state.timeoutRecoveryEpisodes \subseteq AsyncTimeoutRecoveryEpisodeSet
      => \A episode \in
           AsyncTimeoutRecoveryRetainedEpisodesAfterTransition(
             preClockState, state):
           /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
           /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                episode, context', nodeView', generation', decisions')
PROOF
  <1>1. ASSUME NEW preClockState, NEW state,
                state.timeoutRecoveryEpisodes
                  \subseteq AsyncTimeoutRecoveryEpisodeSet
         PROVE \A result \in
                   AsyncTimeoutRecoveryRetainedEpisodesAfterTransition(
                     preClockState, state):
                 /\ AsyncTimeoutRecoveryMutationFrameShape(result)
                 /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                      result, context', nodeView', generation', decisions')
    <2>1. ASSUME NEW result \in
                    AsyncTimeoutRecoveryRetainedEpisodesAfterTransition(
                      preClockState, state)
           PROVE /\ AsyncTimeoutRecoveryMutationFrameShape(result)
                 /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                      result, context', nodeView', generation', decisions')
      <3>1. PICK retained \in state.timeoutRecoveryEpisodes:
               /\ ~AsyncTimeoutRecoveryEpisodeRetiresThisStep(retained)
               /\ result =
                    AsyncTimeoutRecoveryEpisodeAfterTransition(
                      preClockState, retained)
        BY <2>1, Zenon
           DEF AsyncTimeoutRecoveryRetainedEpisodesAfterTransition
      <3>2. retained \in AsyncTimeoutRecoveryEpisodeSet
        BY <1>1, <3>1
      <3>3. AsyncTimeoutRecoveryMutationFrameShape(retained)
        BY <3>2, AsyncTimeoutRecoveryEpisodeSetHasMutationFrameShape
      <3>4. AsyncTimeoutRecoveryEpisodeBoundaryIn(
               retained, context', nodeView', generation', decisions')
        BY <3>1, Isa
           DEF AsyncTimeoutRecoveryEpisodeBoundaryIn,
               AsyncTimeoutRecoveryEpisodeRetiresThisStep,
               AsyncNodeHasDecisionIn
      <3> QED BY <3>1, <3>3, <3>4,
           AsyncTimeoutRecoveryEpisodeAfterTransitionPreservesCurrentBoundary
    <2> QED BY <2>1
  <1> QED BY <1>1

AsyncTimeoutRecoveryNewBaseEpisodeIn(
    preClockState, timeoutBaseState, node) ==
  LET timeoutOrdinal ==
        AsyncTimeoutLifecycleOrdinalForStep(timeoutBaseState, node)
      physicalCut ==
        AsyncTimeoutLifecyclePhysicalCutForStep(timeoutBaseState, node)
      preOrdinal == preClockState.retransmitLifecycleOrdinal[node]
      prePhysicalCut ==
        preClockState.retransmitLifecyclePhysicalCut[node]
  IN AsyncTimeoutRecoveryEpisode(
       node, AsyncCurrentTimeoutCausalOrigin(node), generation'[node],
       timeoutOrdinal, physicalCut, preOrdinal, prePhysicalCut, {})

AsyncTimeoutRecoveryNewEpisodeClearsFrozenPredecessorIn(
    preClockState, node) ==
  /\ preClockState.retransmitLifecycleOrdinal[node] # 0
  /\ \/ AsyncRetransmitLifecycleEpisodeCompletesThisStep(node)
     \/ AsyncTimeoutLifecycleTransfersThisStep(node)

THEOREM AsyncTimeoutRecoveryNewEpisodeDecomposition ==
  \A preClockState, timeoutBaseState, node:
    AsyncTimeoutRecoveryNewEpisodeIn(
      preClockState, timeoutBaseState, node)
      = IF AsyncTimeoutRecoveryNewEpisodeClearsFrozenPredecessorIn(
             preClockState, node)
        THEN AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor(
               AsyncTimeoutRecoveryNewBaseEpisodeIn(
                 preClockState, timeoutBaseState, node))
        ELSE AsyncTimeoutRecoveryNewBaseEpisodeIn(
               preClockState, timeoutBaseState, node)
BY Isa
   DEF AsyncTimeoutRecoveryNewEpisodeIn,
       AsyncTimeoutRecoveryNewBaseEpisodeIn,
       AsyncTimeoutRecoveryNewEpisodeClearsFrozenPredecessorIn,
       AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor

THEOREM AsyncTimeoutRecoveryNewBaseEpisodeInHasCurrentBoundary ==
  ASSUME NEW preClockState, NEW timeoutBaseState,
         NEW node \in ValidatorIds,
         AsyncTimeoutRecoveryEpisodeCreationReadyIn(
           preClockState, timeoutBaseState, node)
  PROVE /\ AsyncTimeoutRecoveryMutationFrameShape(
             AsyncTimeoutRecoveryNewBaseEpisodeIn(
               preClockState, timeoutBaseState, node))
        /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
             AsyncTimeoutRecoveryNewBaseEpisodeIn(
               preClockState, timeoutBaseState, node),
             context', nodeView', generation', decisions')
PROOF
  <1> DEFINE BaseEpisode ==
         AsyncTimeoutRecoveryNewBaseEpisodeIn(
           preClockState, timeoutBaseState, node)
  <1> DEFINE Origin == AsyncCurrentTimeoutCausalOrigin(node)
  <1>1. AsyncTimeoutRecoveryMutationFrameShape(BaseEpisode)
    BY Isa
       DEF AsyncTimeoutRecoveryMutationFrameShape,
           AsyncTimeoutRecoveryNewBaseEpisodeIn,
           AsyncTimeoutRecoveryEpisode, BaseEpisode
  <1>2. BaseEpisode.node = node
    BY DEF AsyncTimeoutRecoveryNewBaseEpisodeIn,
           AsyncTimeoutRecoveryEpisode, BaseEpisode
  <1>3. BaseEpisode.key.context = Origin.context
    BY DEF AsyncTimeoutRecoveryNewBaseEpisodeIn,
           AsyncTimeoutRecoveryEpisode,
           AsyncTimeoutRecoveryEpisodeKey, BaseEpisode, Origin
  <1>4. BaseEpisode.timeoutOwnerOrigin.height = Origin.height
    BY DEF AsyncTimeoutRecoveryNewBaseEpisodeIn,
           AsyncTimeoutRecoveryEpisode, BaseEpisode, Origin
  <1>5. BaseEpisode.key.view = Origin.view
    BY DEF AsyncTimeoutRecoveryNewBaseEpisodeIn,
           AsyncTimeoutRecoveryEpisode,
           AsyncTimeoutRecoveryEpisodeKey, BaseEpisode, Origin
  <1>6. BaseEpisode.generation = generation'[node]
    BY DEF AsyncTimeoutRecoveryNewBaseEpisodeIn,
           AsyncTimeoutRecoveryEpisode, BaseEpisode
  <1>7. Origin.context = context'
    BY Isa DEF AsyncTimeoutRecoveryEpisodeCreationReadyIn, Origin
  <1>8. Origin.height = context'.height
    BY Isa DEF AsyncTimeoutRecoveryEpisodeCreationReadyIn, Origin
  <1>9. Origin.view = nodeView'[node]
    BY Isa DEF AsyncTimeoutRecoveryEpisodeCreationReadyIn, Origin
  <1>10. ~AsyncNodeHasDecisionIn(node, context', decisions')
    BY Isa DEF AsyncTimeoutRecoveryEpisodeCreationReadyIn
  <1>11. AsyncTimeoutRecoveryEpisodeBoundaryIn(
            BaseEpisode, context', nodeView', generation', decisions')
    BY <1>2, <1>3, <1>4, <1>5, <1>6,
       <1>7, <1>8, <1>9, <1>10, Isa
       DEF AsyncTimeoutRecoveryEpisodeBoundaryIn
  <1> QED BY <1>1, <1>11 DEF BaseEpisode

THEOREM AsyncTimeoutRecoveryNewEpisodeInHasCurrentBoundary ==
  ASSUME NEW preClockState, NEW timeoutBaseState,
         NEW node \in ValidatorIds,
         AsyncTimeoutRecoveryEpisodeCreationReadyIn(
           preClockState, timeoutBaseState, node)
  PROVE /\ AsyncTimeoutRecoveryMutationFrameShape(
             AsyncTimeoutRecoveryNewEpisodeIn(
               preClockState, timeoutBaseState, node))
        /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
             AsyncTimeoutRecoveryNewEpisodeIn(
               preClockState, timeoutBaseState, node),
             context', nodeView', generation', decisions')
PROOF
  <1> DEFINE BaseEpisode ==
         AsyncTimeoutRecoveryNewBaseEpisodeIn(
           preClockState, timeoutBaseState, node)
  <1>1. /\ AsyncTimeoutRecoveryMutationFrameShape(BaseEpisode)
         /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
              BaseEpisode,
              context', nodeView', generation', decisions')
    BY AsyncTimeoutRecoveryNewBaseEpisodeInHasCurrentBoundary
       DEF BaseEpisode
  <1>2. /\ AsyncTimeoutRecoveryMutationFrameShape(
              AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor(
                BaseEpisode))
         /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
              AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor(
                BaseEpisode),
              context', nodeView', generation', decisions')
    BY <1>1,
       AsyncTimeoutRecoveryClearedFrozenPredecessorPreservesCurrentBoundary
  <1>3. AsyncTimeoutRecoveryNewEpisodeIn(
           preClockState, timeoutBaseState, node)
           = IF AsyncTimeoutRecoveryNewEpisodeClearsFrozenPredecessorIn(
                  preClockState, node)
             THEN AsyncTimeoutRecoveryEpisodeWithClearedFrozenPredecessor(
                    BaseEpisode)
             ELSE BaseEpisode
    BY AsyncTimeoutRecoveryNewEpisodeDecomposition
       DEF BaseEpisode
  <1>4. CASE AsyncTimeoutRecoveryNewEpisodeClearsFrozenPredecessorIn(
                preClockState, node)
    BY <1>2, <1>3, <1>4
  <1>5. CASE ~AsyncTimeoutRecoveryNewEpisodeClearsFrozenPredecessorIn(
                 preClockState, node)
    BY <1>1, <1>3, <1>5
  <1> QED BY <1>4, <1>5

THEOREM AsyncTimeoutRecoveryNewEpisodesHaveCurrentBoundary ==
  \A preClockState, timeoutBaseState:
    AsyncTimeoutRecoveryTransitionGateIn(preClockState, timeoutBaseState)
      => \A episode \in
           AsyncTimeoutRecoveryNewEpisodesAfterTransition(
             preClockState, timeoutBaseState):
           /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
           /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                episode, context', nodeView', generation', decisions')
PROOF
  <1>1. ASSUME NEW preClockState, NEW timeoutBaseState,
                AsyncTimeoutRecoveryTransitionGateIn(
                  preClockState, timeoutBaseState)
         PROVE \A result \in
                   AsyncTimeoutRecoveryNewEpisodesAfterTransition(
                     preClockState, timeoutBaseState):
                 /\ AsyncTimeoutRecoveryMutationFrameShape(result)
                 /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                      result, context', nodeView', generation', decisions')
    <2>1. ASSUME NEW result \in
                    AsyncTimeoutRecoveryNewEpisodesAfterTransition(
                      preClockState, timeoutBaseState)
           PROVE /\ AsyncTimeoutRecoveryMutationFrameShape(result)
                 /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                      result, context', nodeView', generation', decisions')
      <3>1. PICK node \in ValidatorIds:
               /\ AsyncTimeoutRecoveryEpisodeCreationRequiredIn(
                    timeoutBaseState, node)
               /\ result = AsyncTimeoutRecoveryNewEpisodeIn(
                    preClockState, timeoutBaseState, node)
        BY <2>1, Zenon
           DEF AsyncTimeoutRecoveryNewEpisodesAfterTransition
      <3>2. AsyncTimeoutRecoveryEpisodeCreationReadyIn(
               preClockState, timeoutBaseState, node)
        BY <1>1, <3>1, Zenon
           DEF AsyncTimeoutRecoveryTransitionGateIn
      <3>3. /\ AsyncTimeoutRecoveryMutationFrameShape(
                    AsyncTimeoutRecoveryNewEpisodeIn(
                      preClockState, timeoutBaseState, node))
             /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                  AsyncTimeoutRecoveryNewEpisodeIn(
                    preClockState, timeoutBaseState, node),
                  context', nodeView', generation', decisions')
        BY <3>2,
           AsyncTimeoutRecoveryNewEpisodeInHasCurrentBoundary
      <3> QED BY <3>1, <3>3
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM AsyncTimeoutRecoveryEpisodeUnionEstablishesCurrentBoundary ==
  ASSUME NEW preClockState, NEW timeoutBaseState, NEW state,
         NEW episodes,
         state.timeoutRecoveryEpisodes
           \subseteq AsyncTimeoutRecoveryEpisodeSet,
         AsyncTimeoutRecoveryTransitionGateIn(
           preClockState, timeoutBaseState),
         episodes =
           AsyncTimeoutRecoveryRetainedEpisodesAfterTransition(
             preClockState, state)
             \cup
           AsyncTimeoutRecoveryNewEpisodesAfterTransition(
             preClockState, timeoutBaseState)
  PROVE \A episode \in episodes:
          /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
          /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
               episode, context', nodeView', generation', decisions')
PROOF
  <1>1. \A episode \in
             AsyncTimeoutRecoveryRetainedEpisodesAfterTransition(
               preClockState, state):
           /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
           /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                episode, context', nodeView', generation', decisions')
    BY AsyncTimeoutRecoveryRetainedEpisodesHaveCurrentBoundary
  <1>2. \A episode \in
             AsyncTimeoutRecoveryNewEpisodesAfterTransition(
               preClockState, timeoutBaseState):
           /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
           /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                episode, context', nodeView', generation', decisions')
    BY AsyncTimeoutRecoveryNewEpisodesHaveCurrentBoundary
  <1> QED BY <1>1, <1>2, Zenon

THEOREM AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionPreservesCurrentBoundary ==
  ASSUME NEW state, NEW episode,
         AsyncTimeoutRecoveryMutationFrameShape(episode),
         AsyncTimeoutRecoveryEpisodeBoundaryIn(
           episode, context', nodeView', generation', decisions')
  PROVE /\ AsyncTimeoutRecoveryMutationFrameShape(
             AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(
               state, episode))
        /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
             AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(
               state, episode),
             context', nodeView', generation', decisions')
PROOF
  <1> DEFINE MatchingNodes ==
         {candidateNode \in AsyncTimeoutRecoveryVoteAdmissionNodesThisStep:
            LET item == AsyncSelectedFairIngressItem(candidateNode)
                candidate ==
                  AsyncTimeoutRecoveryVoteCandidateOwner(
                    candidateNode, item)
            IN candidate.slot.episode = episode.key}
  <1>1. CASE MatchingNodes = {}
    BY <1>1
       DEF AsyncTimeoutRecoveryEpisodeAfterVoteAdmission,
           MatchingNodes
  <1>2. CASE MatchingNodes # {}
    <2> DEFINE Node ==
           CHOOSE candidateNode \in MatchingNodes: TRUE
    <2> DEFINE Item == AsyncSelectedFairIngressItem(Node)
    <2> DEFINE Candidate ==
           AsyncTimeoutRecoveryVoteCandidateOwner(Node, Item)
    <2>1. CASE "FirstAdmission"
                   \in AsyncTimeoutRecoveryVoteAdmissionPlan(Node, Item)
      <3> DEFINE Updated ==
             [episode EXCEPT
                !.admittedTimeoutVoteOwners = @ \cup {Candidate}]
      <3>1. "admittedTimeoutVoteOwners" \in DOMAIN episode
        BY DEF AsyncTimeoutRecoveryMutationFrameShape
      <3>2. DOMAIN Updated = DOMAIN episode
        BY <3>1, FunctionalReplacePreservesDomain DEF Updated
      <3>3. AsyncTimeoutRecoveryMutationFrameShape(Updated)
        BY <3>2 DEF AsyncTimeoutRecoveryMutationFrameShape
      <3>4. Updated.node = episode.node
        BY FunctionalUpdateAwayFromKey
           DEF AsyncTimeoutRecoveryMutationFrameShape, Updated
      <3>5. Updated.key = episode.key
        BY FunctionalUpdateAwayFromKey
           DEF AsyncTimeoutRecoveryMutationFrameShape, Updated
      <3>6. Updated.generation = episode.generation
        BY FunctionalUpdateAwayFromKey, Isa
           DEF AsyncTimeoutRecoveryMutationFrameShape, Updated
      <3>7. Updated.timeoutOwnerOrigin = episode.timeoutOwnerOrigin
        BY FunctionalUpdateAwayFromKey, Isa
           DEF AsyncTimeoutRecoveryMutationFrameShape, Updated
      <3>8. AsyncTimeoutRecoveryEpisodeBoundaryIn(
               Updated, context', nodeView', generation', decisions')
        BY <3>4, <3>5, <3>6, <3>7, Isa
           DEF AsyncTimeoutRecoveryEpisodeBoundaryIn
      <3>9. AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(
               state, episode) = Updated
        BY <1>2, <2>1, Isa
           DEF AsyncTimeoutRecoveryEpisodeAfterVoteAdmission,
               MatchingNodes, Node, Item, Candidate, Updated
      <3> QED BY <3>3, <3>8, <3>9
    <2>2. CASE "FirstAdmission"
                   \notin AsyncTimeoutRecoveryVoteAdmissionPlan(Node, Item)
      <3>1. AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(
               state, episode) = episode
        BY <1>2, <2>2, Isa
           DEF AsyncTimeoutRecoveryEpisodeAfterVoteAdmission,
               MatchingNodes, Node, Item, Candidate
      <3> QED BY <3>1
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1, <1>2

THEOREM AsyncTimeoutRecoveryVoteOwnerImagePreservesCurrentBoundary ==
  ASSUME NEW state, NEW episodes,
         \A episode \in state.timeoutRecoveryEpisodes:
           /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
           /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                episode, context', nodeView', generation', decisions'),
         episodes =
           {AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(state, episode):
              episode \in state.timeoutRecoveryEpisodes}
  PROVE \A episode \in episodes:
          /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
          /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
               episode, context', nodeView', generation', decisions')
PROOF
  <1>1. ASSUME NEW result \in episodes
         PROVE /\ AsyncTimeoutRecoveryMutationFrameShape(result)
               /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                    result, context', nodeView', generation', decisions')
    <2>1. PICK episode \in state.timeoutRecoveryEpisodes:
             result =
               AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(
                 state, episode)
      BY <1>1, Zenon
    <2>2. /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
           /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                episode, context', nodeView', generation', decisions')
      BY <2>1
    <2> QED BY <2>1, <2>2,
         AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionPreservesCurrentBoundary
  <1> QED BY <1>1

THEOREM AsyncControlServiceTypeProjectsTimeoutRecoveryEpisodeSet ==
  AsyncControlServiceStateTypeInvariant
    => asyncControlServiceState.timeoutRecoveryEpisodes
         \subseteq AsyncTimeoutRecoveryEpisodeSet
BY DEF AsyncControlServiceStateTypeInvariant,
       AsyncTimeoutRecoveryEpisodeTypeInvariantIn,
       AsyncTimeoutRecoveryEpisodesIn

THEOREM AsyncControlServiceResetPreservesTimeoutRecoveryEpisodeSet ==
  \A state, resetNodes:
    state.timeoutRecoveryEpisodes
      \subseteq AsyncTimeoutRecoveryEpisodeSet
      => (AsyncControlServiceStateAfterReset(state, resetNodes))
           .timeoutRecoveryEpisodes
           \subseteq AsyncTimeoutRecoveryEpisodeSet
PROOF
  <1>1. ASSUME NEW state, NEW resetNodes,
                state.timeoutRecoveryEpisodes
                  \subseteq AsyncTimeoutRecoveryEpisodeSet
         PROVE (AsyncControlServiceStateAfterReset(state, resetNodes))
                 .timeoutRecoveryEpisodes
                 \subseteq AsyncTimeoutRecoveryEpisodeSet
    <2>1. (AsyncControlServiceStateAfterReset(state, resetNodes))
             .timeoutRecoveryEpisodes
             = {episode \in state.timeoutRecoveryEpisodes:
                  episode.node \notin resetNodes}
      BY DEF AsyncControlServiceStateAfterReset
    <2>2. {episode \in state.timeoutRecoveryEpisodes:
             episode.node \notin resetNodes}
             \subseteq state.timeoutRecoveryEpisodes
      BY Zenon
    <2> QED BY <1>1, <2>1, <2>2, FS_Subset
  <1> QED BY <1>1

THEOREM AsyncControlServiceSlotTransitionEstablishesTimeoutRecoveryCurrentBoundary ==
  ASSUME AsyncControlServiceStateTypeInvariant,
         AsyncControlServiceSlotTransition
  PROVE AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant'
PROOF
  <1>1. AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant'
    <2> DEFINE ResetState ==
           AsyncControlServiceStateAfterReset(
             asyncControlServiceState,
             AsyncControlServiceResetNodesThisStep)
    <2> DEFINE AdmittedState ==
           IF AsyncControlServiceAdmissionsThisStep = {}
           THEN ResetState
           ELSE AsyncControlServiceStateAfterAdmission(
                  ResetState,
                  CHOOSE item \in
                    AsyncControlServiceAdmissionsThisStep: TRUE)
    <2> DEFINE ServicedState ==
           IF AsyncControlServicesThisStep = {}
           THEN AdmittedState
           ELSE AsyncControlServiceStateAfterService(
                  AdmittedState,
                  CHOOSE item \in AsyncControlServicesThisStep: TRUE)
    <2> DEFINE ResponseRetirementState ==
           AsyncCertifiedResponseClaimStateAfterRetirement(ServicedState)
    <2> DEFINE ResponseState ==
           IF AsyncCertifiedResponseClaimAdmissionsThisStep = {}
           THEN ResponseRetirementState
           ELSE AsyncCertifiedResponseClaimStateAfterAdmission(
                  ResponseRetirementState,
                  CHOOSE item \in
                    AsyncCertifiedResponseClaimAdmissionsThisStep: TRUE)
    <2> DEFINE TimeoutRetirementState ==
           AsyncControlServiceStateAfterTimeoutRetirement(ResponseState)
    <2> DEFINE CandidateReclamationState ==
           AsyncCandidateServiceStateAfterReclamation(
             TimeoutRetirementState)
    <2> DEFINE CandidateMarkedState ==
           IF AsyncCandidateServicesThisStep # {}
           THEN AsyncCandidateServiceStateAfterSuccessfulService(
                  CandidateReclamationState,
                  CHOOSE candidate \in
                    AsyncCandidateServicesThisStep: TRUE)
           ELSE IF AsyncCandidateEligibleTerminalDiscardsThisStep # {}
                THEN AsyncCandidateServiceStateAfterTerminalRetirement(
                       CandidateReclamationState)
                ELSE CandidateReclamationState
    <2> DEFINE CandidateOwnedState ==
           IF AsyncCandidateLifecycleDeparturesThisStep # {}
           THEN AsyncCandidateLifecycleStateAfterServiceSlotTransfer(
                  CandidateMarkedState,
                  CHOOSE candidate \in
                    AsyncCandidateLifecycleDeparturesThisStep: TRUE)
           ELSE CandidateMarkedState
    <2> DEFINE CandidateServiceState ==
           IF AsyncCandidateLifecycleDeparturesThisStep # {}
           THEN AsyncCandidateProducerContinuationStateAfterDeparture(
                  CandidateOwnedState,
                  CHOOSE candidate \in
                    AsyncCandidateLifecycleDeparturesThisStep: TRUE)
           ELSE CandidateOwnedState
    <2> DEFINE OrdinaryCarrierState ==
           AsyncOrdinaryIngressCarrierStateAfterTransition(
             CandidateServiceState)
    <2> DEFINE CarrierState ==
           AsyncCandidateLifecycleStateAfterCarrierUpdate(
             OrdinaryCarrierState)
    <2> DEFINE CompactedState ==
           AsyncCandidateLifecycleStateAfterCompaction(CarrierState)
    <2> DEFINE LeaderWireState ==
           AsyncCandidateLifecycleStateAfterLeaderWireAdmission(
             CompactedState)
    <2> DEFINE ServeIngressState ==
           AsyncCandidateLifecycleStateAfterServeIngressAdmission(
             LeaderWireState)
    <2> DEFINE LifecycleState ==
           AsyncCandidateLifecycleStateAfterAdmission(ServeIngressState)
    <2> DEFINE TimeoutState ==
           AsyncCandidateLifecycleStateAfterTimeoutOwnership(
             ServeIngressState, LifecycleState)
    <2> DEFINE RecoveryState ==
           AsyncTimeoutRecoveryEpisodeStateAfterTransition(
             LeaderWireState, ServeIngressState, TimeoutState)
    <2> DEFINE VoteState ==
           AsyncTimeoutRecoveryVoteOwnerStateAfterAdmission(RecoveryState)
    <2> DEFINE AtomicFacts ==
           /\ AsyncTimeoutRecoveryTransitionGateIn(
                LeaderWireState, ServeIngressState)
           /\ RecoveryState.timeoutRecoveryEpisodes =
                AsyncTimeoutRecoveryRetainedEpisodesAfterTransition(
                  LeaderWireState, TimeoutState)
                  \cup
                AsyncTimeoutRecoveryNewEpisodesAfterTransition(
                  LeaderWireState, ServeIngressState)
           /\ VoteState.timeoutRecoveryEpisodes =
                {AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(
                   RecoveryState, episode):
                   episode \in RecoveryState.timeoutRecoveryEpisodes}
           /\ TimeoutState.timeoutRecoveryEpisodes =
                ResetState.timeoutRecoveryEpisodes
           /\ asyncControlServiceState'.timeoutRecoveryEpisodes =
                VoteState.timeoutRecoveryEpisodes
    <2>1. asyncControlServiceState.timeoutRecoveryEpisodes
             \subseteq AsyncTimeoutRecoveryEpisodeSet
      BY AsyncControlServiceTypeProjectsTimeoutRecoveryEpisodeSet
    <2>2. ResetState.timeoutRecoveryEpisodes
             \subseteq AsyncTimeoutRecoveryEpisodeSet
      BY <2>1,
         AsyncControlServiceResetPreservesTimeoutRecoveryEpisodeSet
         DEF ResetState
    <2>3. AtomicFacts
      <3>1. AsyncTimeoutRecoveryTransitionGateIn(
               LeaderWireState, ServeIngressState)
        BY AsyncControlServiceTransitionRequiresAtomicLifecycleReservation,
           Zenon
           DEF ResetState, AdmittedState, ServicedState,
               ResponseRetirementState, ResponseState,
               TimeoutRetirementState, CandidateReclamationState,
               CandidateMarkedState, CandidateOwnedState,
               CandidateServiceState, OrdinaryCarrierState,
               CarrierState, CompactedState, LeaderWireState,
               ServeIngressState, LifecycleState, TimeoutState,
               RecoveryState, VoteState
      <3>2. RecoveryState.timeoutRecoveryEpisodes =
               AsyncTimeoutRecoveryRetainedEpisodesAfterTransition(
                 LeaderWireState, TimeoutState)
                 \cup
               AsyncTimeoutRecoveryNewEpisodesAfterTransition(
                 LeaderWireState, ServeIngressState)
        BY AsyncControlServiceTransitionRequiresAtomicLifecycleReservation,
           Zenon
           DEF ResetState, AdmittedState, ServicedState,
               ResponseRetirementState, ResponseState,
               TimeoutRetirementState, CandidateReclamationState,
               CandidateMarkedState, CandidateOwnedState,
               CandidateServiceState, OrdinaryCarrierState,
               CarrierState, CompactedState, LeaderWireState,
               ServeIngressState, LifecycleState, TimeoutState,
               RecoveryState, VoteState
      <3>3. VoteState.timeoutRecoveryEpisodes =
               {AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(
                  RecoveryState, episode):
                  episode \in RecoveryState.timeoutRecoveryEpisodes}
        BY AsyncControlServiceTransitionRequiresAtomicLifecycleReservation,
           Zenon
           DEF ResetState, AdmittedState, ServicedState,
               ResponseRetirementState, ResponseState,
               TimeoutRetirementState, CandidateReclamationState,
               CandidateMarkedState, CandidateOwnedState,
               CandidateServiceState, OrdinaryCarrierState,
               CarrierState, CompactedState, LeaderWireState,
               ServeIngressState, LifecycleState, TimeoutState,
               RecoveryState, VoteState
      <3>4. TimeoutState.timeoutRecoveryEpisodes =
               ResetState.timeoutRecoveryEpisodes
        BY AsyncControlServiceTransitionRequiresAtomicLifecycleReservation,
           IsaT(120)
           DEF ResetState, AdmittedState, ServicedState,
               ResponseRetirementState, ResponseState,
               TimeoutRetirementState, CandidateReclamationState,
               CandidateMarkedState, CandidateOwnedState,
               CandidateServiceState, OrdinaryCarrierState,
               CarrierState, CompactedState, LeaderWireState,
               ServeIngressState, LifecycleState, TimeoutState,
               RecoveryState, VoteState
      <3>5. asyncControlServiceState'.timeoutRecoveryEpisodes =
               VoteState.timeoutRecoveryEpisodes
        BY AsyncControlServiceTransitionRequiresAtomicLifecycleReservation,
           Zenon
           DEF ResetState, AdmittedState, ServicedState,
               ResponseRetirementState, ResponseState,
               TimeoutRetirementState, CandidateReclamationState,
               CandidateMarkedState, CandidateOwnedState,
               CandidateServiceState, OrdinaryCarrierState,
               CarrierState, CompactedState, LeaderWireState,
               ServeIngressState, LifecycleState, TimeoutState,
               RecoveryState, VoteState
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5
           DEF AtomicFacts
    <2>4. TimeoutState.timeoutRecoveryEpisodes
             \subseteq AsyncTimeoutRecoveryEpisodeSet
      BY <2>2, <2>3, Zenon
         DEF AtomicFacts
    <2>5. AsyncTimeoutRecoveryTransitionGateIn(
             LeaderWireState, ServeIngressState)
      BY <2>3 DEF AtomicFacts
    <2>6. RecoveryState.timeoutRecoveryEpisodes =
             AsyncTimeoutRecoveryRetainedEpisodesAfterTransition(
               LeaderWireState, TimeoutState)
               \cup
             AsyncTimeoutRecoveryNewEpisodesAfterTransition(
               LeaderWireState, ServeIngressState)
      BY <2>3 DEF AtomicFacts
    <2>7. \A episode \in RecoveryState.timeoutRecoveryEpisodes:
             /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
             /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                  episode, context', nodeView', generation', decisions')
      BY <2>4, <2>5, <2>6,
         AsyncTimeoutRecoveryEpisodeUnionEstablishesCurrentBoundary
    <2>8. VoteState.timeoutRecoveryEpisodes =
             {AsyncTimeoutRecoveryEpisodeAfterVoteAdmission(
                RecoveryState, episode):
                episode \in RecoveryState.timeoutRecoveryEpisodes}
      BY <2>3 DEF AtomicFacts
    <2>9. \A episode \in VoteState.timeoutRecoveryEpisodes:
             /\ AsyncTimeoutRecoveryMutationFrameShape(episode)
             /\ AsyncTimeoutRecoveryEpisodeBoundaryIn(
                  episode, context', nodeView', generation', decisions')
      BY <2>7, <2>8,
         AsyncTimeoutRecoveryVoteOwnerImagePreservesCurrentBoundary
    <2>10. asyncControlServiceState'.timeoutRecoveryEpisodes =
              VoteState.timeoutRecoveryEpisodes
      BY <2>3 DEF AtomicFacts
    <2>11. \A episode \in
                asyncControlServiceState'.timeoutRecoveryEpisodes:
              AsyncTimeoutRecoveryEpisodeBoundaryIn(
                episode, context', nodeView', generation', decisions')
      BY <2>9, <2>10, Zenon
    <2> QED BY <2>11, Isa
         DEF AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant,
             AsyncTimeoutRecoveryEpisodes,
             AsyncTimeoutRecoveryEpisodesIn,
             AsyncTimeoutRecoveryEpisodeBoundaryIn,
             NodeHasDecision, AsyncNodeHasDecisionIn
  <1> QED BY <1>1

THEOREM AsyncNextPreservesTimeoutRecoveryCurrentBoundaryInvariant ==
  ASSUME AsyncControlServiceStateTypeInvariant,
         AsyncNext
  PROVE AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant'
PROOF
  <1>1. AsyncControlServiceSlotTransition
    BY DEF AsyncNext
  <1> QED BY <1>1,
       AsyncControlServiceSlotTransitionEstablishesTimeoutRecoveryCurrentBoundary

THEOREM AsyncNextPreservesStrongTypeInvariant ==
  AsyncStrongTypeInvariant /\ AsyncNext
    => AsyncStrongTypeInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncNext
         PROVE AsyncStrongTypeInvariant'
    <2>1. StrongInductiveInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>2a. AsyncRecoveryTypeInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2b. AsyncRestartAuthorityInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2c. AsyncRecoveryExecutionInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2d. /\ AsyncHistoricalLockRestartAuthorityTypeInvariant
            /\ HistoricalLockRestartAuthoritySourceRetentionInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2e. AsyncGstRecoveryPhaseInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2f. AsyncSerializedBusyKernelInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2g. AsyncCertifiedResponseClaimIngressOwnershipInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2j. AsyncLeaderWireIngressCarrierOwnershipInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2k. AsyncOrdinaryIngressCarrierOwnershipInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2h. AsyncControlServiceStateTypeInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2i. AsyncServiceActivationPairInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2l. AsyncCandidateLifecycleSchedulerCoverageInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2m. /\ AsyncProducerTypeInvariant
            /\ AsyncServeProducerTurnTypeInvariant
            /\ AsyncServeProducerTurnOwnershipInvariant
            /\ AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>3. StrongInductiveInvariant'
      BY <1>1, <2>1, AsyncNextPreservesStrongInductiveInvariant
    <2>4. AsyncSchedulerTypeInvariant'
      BY <1>1, <2>1, <2>2, <2>2a,
         AsyncNextPreservesSchedulerType
    <2>4a. AsyncServiceActivationPairInvariant'
      BY <1>1, <2>2,
         AsyncNextPreservesServiceActivationPairInvariant
    <2>4b. AsyncControlServiceStateTypeInvariant'
      BY <1>1, <2>2, <2>2h,
         AsyncNextPreservesControlServiceStateTypeInvariant
    <2>4c. AsyncCandidateLifecycleSchedulerCoverageInvariant'
      BY <1>1, AsyncNextPreservesCandidateLifecycleSchedulerCoverage
    <2>4d. /\ AsyncServeProducerTurnTypeInvariant'
             /\ AsyncServeProducerTurnOwnershipInvariant'
      BY <1>1, AsyncNextPreservesServeProducerTurnInvariants
    <2>4e. AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant'
      BY <1>1, <2>2h,
         AsyncNextPreservesTimeoutRecoveryCurrentBoundaryInvariant
    <2>4f. AsyncProducerTypeInvariant'
      BY <1>1, <2>2, AsyncProducerProjectionPreservesTypeInvariant
         DEF AsyncNext
    <2>5. ReceivedTimeoutVotePoolInvariant'
      BY <1>1, <2>2, AsyncNextPreservesTimeoutPoolInvariant
    <2>6. /\ AsyncRecoveryTypeInvariant'
           /\ AsyncRestartAuthorityInvariant'
      BY <1>1, <2>1, <2>2, <2>2a, <2>2b, <2>2c,
         AsyncNextPreservesRecoveryInvariants
    <2>7. AsyncRecoveryExecutionInvariant'
      BY <1>1, <2>1, <2>2, <2>2a, <2>2b, <2>2c,
         AsyncNextPreservesRecoveryExecutionInvariant
    <2>8. /\ AsyncHistoricalLockRestartAuthorityTypeInvariant'
           /\ HistoricalLockRestartAuthoritySourceRetentionInvariant'
      BY <1>1, <2>1, <2>2d,
         AsyncNextPreservesHistoricalLockRestartAuthorityInvariants
    <2>9. AsyncSerializedBusyKernelInvariant'
      BY <1>1, <2>1, <2>2f,
         AsyncNextPreservesSerializedBusyKernelInvariant
    <2>10. AsyncGstRecoveryPhaseInvariant'
      BY <1>1, <2>2e,
         AsyncNextPreservesGstRecoveryPhaseInvariant
    <2>11. AsyncCertifiedResponseClaimIngressOwnershipInvariant'
      BY <1>1, <2>2, <2>2g,
         AsyncNextPreservesCertifiedResponseClaimIngressOwnershipInvariant
    <2>12. AsyncLeaderWireIngressCarrierOwnershipInvariant'
      BY <1>1, <2>2j,
         AsyncNextPreservesLeaderWireIngressCarrierOwnership
    <2>13. AsyncOrdinaryIngressCarrierOwnershipInvariant'
      BY <1>1, <2>2k,
         AsyncNextPreservesOrdinaryIngressCarrierOwnership
    <2> QED BY <2>2l, <2>2m, <2>3, <2>4, <2>4a, <2>4b, <2>4c, <2>4d,
                <2>4e, <2>4f, <2>5, <2>6, <2>7, <2>8, <2>9, <2>10,
                <2>11, <2>12, <2>13
         DEF AsyncStrongTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncBracketNextPreservesStrongTypeInvariant ==
  AsyncStrongTypeInvariant /\ [AsyncNext]_AsyncAllVars
    => AsyncStrongTypeInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              [AsyncNext]_AsyncAllVars
         PROVE AsyncStrongTypeInvariant'
    <2>1. CASE AsyncNext
      BY <1>1, <2>1, AsyncNextPreservesStrongTypeInvariant
    <2>2. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>2,
         AsyncAllVarsStutterPreservesStrongTypeInvariant
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
Retained locked-body round rebinding.  View-independent retained authority may
survive a view change, but it is usable only by `RebindRetainedBody`; it is not
durable, validated, or applicable target-round evidence.  Proposal delivery
therefore emits a completion-class rebind candidate that materializes an exact
target-view Available record.  The ordinary StoreBody -> ValidateBody chain
then writes exact-view durable and validation evidence.
***************************************************************************)

RetainedBodyRebindReady(command) ==
  /\ command.kind = "RebindRetainedBody"
  /\ command.class = "Completion"
  /\ CandidateConsumerCurrent(command)
  /\ lockRank[command.node] # NoRank
  /\ lockSubject[command.node] = command.subject
  /\ RetainedLockedBodyHeldBy(
       retainedLockedBodies, command.node, context, command.subject)
  /\ BodyRecord(command.node, context, command.view, command.subject)
       \in BodyRecordSet
  /\ BodyRecord(command.node, context, command.view, command.subject)
       \notin availableBodies
  /\ \E proposal \in SeenProposalValues:
       /\ CommandMatches(command, command.node, proposal.view,
                         proposal.subject)
       /\ ProposalAt(command.node, proposal) \in seenProposals

RetainedBodyRebindAction(command, proposal) ==
  /\ command.kind = "RebindRetainedBody"
  /\ CommandMatches(command, command.node, proposal.view,
                    proposal.subject)
  /\ RebindRetainedBody(command.node, proposal)
  /\ UNCHANGED AsyncAuxVars

THEOREM RetainedBodyRebindCandidateIsTypedAndOwned ==
  \A command:
    (AsyncTypeInvariant /\ AsyncCandidateTyped(command))
      => /\ AsyncCandidateTyped(
               RetainedBodyRebindCandidate(command))
         /\ RetainedBodyRebindCandidate(command)
              \in AsyncCandidateSet
         /\ RetainedBodyRebindCandidate(command).node = command.node
         /\ RetainedBodyRebindCandidate(command).class = "Completion"
         /\ RetainedBodyRebindCandidate(command).kind =
              "RebindRetainedBody"
PROOF
  <1>1. ASSUME NEW command,
                AsyncTypeInvariant,
                AsyncCandidateTyped(command)
         PROVE /\ AsyncCandidateTyped(
                      RetainedBodyRebindCandidate(command))
                /\ RetainedBodyRebindCandidate(command)
                     \in AsyncCandidateSet
                /\ RetainedBodyRebindCandidate(command).node =
                     command.node
                /\ RetainedBodyRebindCandidate(command).class =
                     "Completion"
                /\ RetainedBodyRebindCandidate(command).kind =
                     "RebindRetainedBody"
    <2>1. /\ AsyncCandidateTyped(
                  RetainedBodyRebindCandidate(command))
           /\ RetainedBodyRebindCandidate(command).node = command.node
      BY <1>1, CausalCandidateFromTypedCommand
         DEF RetainedBodyRebindCandidate,
             AsyncCommandClasses, AsyncWorkKinds, AsyncReducerKinds
    <2>2. RetainedBodyRebindCandidate(command) \in AsyncCandidateSet
      BY <2>1, SMT DEF AsyncCandidateTyped, AsyncCandidateSet
    <2> QED BY <2>1, <2>2
       DEF RetainedBodyRebindCandidate, CausalCandidate,
           AsyncCandidateFrom,
           AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
           AsyncCandidateSuccessorSemanticPhase,
           AsyncCandidateSuccessorProposalRound,
           AsyncCandidateWithIdentityAndOrigin
  <1> QED BY <1>1

THEOREM DeliverProposalSchedulesRetainedBodyRebind ==
  \A command:
    command.kind = "DeliverProposal"
      => CommandSuccessors(command) =
           <<RetainedBodyRebindCandidate(command),
             CausalCandidate("Normal", "BeginPrepare", command)>>
BY DEF CommandSuccessors

THEOREM RebindSchedulesCurrentRoundStore ==
  \A command:
    command.kind = "RebindRetainedBody"
      => CommandSuccessors(command) =
           <<CausalCandidate("Completion", "StoreBody", command)>>
BY DEF CommandSuccessors

THEOREM StoreSchedulesCurrentRoundValidation ==
  \A command:
    command.kind = "StoreBody"
      => CommandSuccessors(command) =
           <<CausalCandidate("Completion", "ValidateBody", command)>>
BY DEF CommandSuccessors

THEOREM ValidationSchedulesPrepareAndLockedCommitAttempts ==
  \A command:
    command.kind = "ValidateBody"
      => CommandSuccessors(command) =
           <<CausalCandidate("Normal", "BeginPrepare", command),
             CausalCandidate("Completion", "BeginLockCommit", command),
             CausalCandidate("Completion", "Apply", command)>>
BY DEF CommandSuccessors

(***************************************************************************
The production adapter classifies `ValidationCompleted` as Completion, and
the reducer calls `persist_commit_intent` inside that event.  PrepareQC
processing likewise calls the same persistence routine directly when the
body is already validated.  The split Core commands therefore keep every
internal BeginLockCommit continuation in the Completion lane; treating one
as independent Progress could defer the exact persistence completion behind
an unrelated Progress-capacity fence.
***************************************************************************)
THEOREM PrepareQcDeliverySchedulesCompletionLockedCommitAttempt ==
  \A command:
    /\ command.kind = "DeliverQC"
    /\ command.item.envelope.qc.phase = "Prepare"
    => CommandSuccessors(command) =
         <<CausalCandidate("Progress", "BeginObservePrepare", command),
           CausalCandidate("Completion", "BeginLockCommit", command)>>
BY DEF CommandSuccessors

THEOREM PersistedPrepareObservationSchedulesCompletionLockedCommitAttempt ==
  \A command:
    command.kind = "PersistObservePrepare"
      => CommandSuccessors(command) =
           <<CausalCandidate("Completion", "BeginLockCommit", command)>>
BY DEF CommandSuccessors

THEOREM ReadyRetainedBodyRebindEnablesExecution ==
  \A command:
    RetainedBodyRebindReady(command)
      => ENABLED ExecuteCommand(command)
PROOF
  <1>1. ASSUME NEW command,
                RetainedBodyRebindReady(command)
         PROVE ENABLED ExecuteCommand(command)
    <2>1. PICK proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ ProposalAt(command.node, proposal) \in seenProposals
      BY <1>1 DEF RetainedBodyRebindReady
    <2>2. ENABLED RetainedBodyRebindAction(command, proposal)
      BY <1>1, <2>1, ExpandENABLED, Isa
         DEF RetainedBodyRebindReady, RetainedBodyRebindAction,
             CommandMatches, RebindRetainedBody, AsyncAuxVars
    <2>3. RetainedBodyRebindAction(command, proposal) \in BOOLEAN
      BY Isa DEF RetainedBodyRebindAction
    <2>4. ExecuteCommand(command) \in BOOLEAN
      BY Isa DEF ExecuteCommand
    <2>5. RetainedBodyRebindAction(command, proposal)
             => ExecuteCommand(command)
      BY Isa
         DEF RetainedBodyRebindAction, ExecuteCommand,
             ExecuteRegularCommand, RegularCoreCommand
    <2>6. (ENABLED RetainedBodyRebindAction(command, proposal))
             => ENABLED ExecuteCommand(command)
      BY <2>3, <2>4, <2>5, ENABLEDaxioms
    <2> QED BY <2>2, <2>6
  <1> QED BY <1>1

THEOREM ReadyRetainedBodyRebindIsDispatchable ==
  \A command:
    (RetainedBodyRebindReady(command)
      /\ command \in AsyncCandidateSet)
      => CommandDispatchable(command)
PROOF
  <1>1. ASSUME NEW command,
                RetainedBodyRebindReady(command),
                command \in AsyncCandidateSet
         PROVE \E selectedCommand \in AsyncCandidateSet:
                   /\ selectedCommand = command
                   /\ ENABLED ExecuteCommand(selectedCommand)
                   /\ (NodeIdle(selectedCommand.node)
                         \/ selectedCommand.class = "Completion")
    <2>1. ENABLED ExecuteCommand(command)
      BY <1>1, ReadyRetainedBodyRebindEnablesExecution
    <2>2. command.class = "Completion"
      BY <1>1 DEF RetainedBodyRebindReady
    <2>3. CandidateConsumerCurrent(command)
      BY <1>1 DEF RetainedBodyRebindReady
    <2>4. WITNESS command \in AsyncCandidateSet
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1 DEF CommandDispatchable

THEOREM RebindCommandSelectsRetainedRebind ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "RebindRetainedBody")
      => \E proposal \in SeenProposalValues:
           /\ CommandMatches(command, command.node, proposal.view,
                             proposal.subject)
           /\ RebindRetainedBody(command.node, proposal)
BY IsaT(60) DEF RegularCoreCommand

THEOREM ExecuteRebindStagesCurrentRoundBody ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "RebindRetainedBody")
      => /\ BodyRecord(command.node, context', command.view,
                       command.subject)
                \in availableBodies'
         /\ RetainedLockedBodyHeldBy(
              retainedLockedBodies', command.node, context',
              command.subject)
PROOF
  <1>1. ASSUME NEW command,
                RegularCoreCommand(command),
                command.kind = "RebindRetainedBody"
         PROVE /\ BodyRecord(command.node, context', command.view,
                             command.subject)
                       \in availableBodies'
                /\ RetainedLockedBodyHeldBy(
                     retainedLockedBodies', command.node, context',
                     command.subject)
    <2>1. \E proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ RebindRetainedBody(command.node, proposal)
      BY <1>1, RebindCommandSelectsRetainedRebind
    <2>2. PICK proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ RebindRetainedBody(command.node, proposal)
      BY <2>1
    <2>3. /\ command.view = proposal.view
           /\ command.subject = proposal.subject
           /\ context' = context
           /\ retainedLockedBodies' = retainedLockedBodies
           /\ BodyRecord(command.node, context, proposal.view,
                         proposal.subject)
                \in availableBodies'
           /\ RetainedLockedBodyHeldBy(
                retainedLockedBodies, command.node, context,
                command.subject)
      BY <1>1, <2>2, Isa
         DEF CommandMatches, RebindRetainedBody, RegularCoreCommand
    <2> QED BY <2>3 DEF RetainedLockedBodyHeldBy
  <1> QED BY <1>1

THEOREM ValidationCommandSelectsValidationAction ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "ValidateBody")
      => (\E proposal \in SeenProposalValues:
            /\ CommandMatches(command, command.node, proposal.view,
                              proposal.subject)
            /\ ValidateBody(command.node, proposal))
         \/ (\E proposal \in SeenProposalValues:
               /\ CommandMatches(command, command.node, proposal.view,
                                 proposal.subject)
               /\ RejectBody(command.node, proposal))
         \/ (\E qc \in DecisionQcValues:
               /\ CommandMatches(command, command.node, qc.view, qc.subject)
               /\ ValidateDecidedBody(command.node, qc))
         \/ (\E qc \in prepareQCs:
               /\ CommandMatches(command, command.node, qc.view, qc.subject)
               /\ ValidateLockedBody(command.node, qc))
BY Isa DEF RegularCoreCommand

THEOREM ExecuteValidationBindsCurrentViewAndGeneration ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "ValidateBody")
      => \/ BodyValidatedBy(
               validatedBodies', command.node, context', command.view,
               generation'[command.node], command.subject)
         \/ BodyRecord(command.node, context', command.view,
                       command.subject)
               \in invalidBodies'
PROOF
  <1>1. ASSUME NEW command,
                RegularCoreCommand(command),
                command.kind = "ValidateBody"
         PROVE \/ BodyValidatedBy(
                      validatedBodies', command.node, context', command.view,
                      generation'[command.node], command.subject)
                \/ BodyRecord(command.node, context', command.view,
                              command.subject)
                      \in invalidBodies'
    <2>1. (\E proposal \in SeenProposalValues:
              /\ CommandMatches(command, command.node, proposal.view,
                                proposal.subject)
              /\ ValidateBody(command.node, proposal))
           \/ (\E proposal \in SeenProposalValues:
                 /\ CommandMatches(command, command.node, proposal.view,
                                   proposal.subject)
                 /\ RejectBody(command.node, proposal))
           \/ (\E qc \in DecisionQcValues:
                 /\ CommandMatches(command, command.node, qc.view,
                                   qc.subject)
                 /\ ValidateDecidedBody(command.node, qc))
           \/ (\E qc \in prepareQCs:
                 /\ CommandMatches(command, command.node, qc.view,
                                   qc.subject)
                 /\ ValidateLockedBody(command.node, qc))
      BY <1>1, ValidationCommandSelectsValidationAction
    <2>2. CASE \E proposal \in SeenProposalValues:
                    /\ CommandMatches(
                         command, command.node, proposal.view,
                         proposal.subject)
                    /\ ValidateBody(command.node, proposal)
      <3>1. PICK proposal \in SeenProposalValues:
               /\ CommandMatches(command, command.node, proposal.view,
                                 proposal.subject)
               /\ ValidateBody(command.node, proposal)
        BY <2>2
      <3> QED BY <3>1, Isa
           DEF CommandMatches, ValidateBody, BodyValidatedBy
    <2>3. CASE \E proposal \in SeenProposalValues:
                    /\ CommandMatches(
                         command, command.node, proposal.view,
                         proposal.subject)
                    /\ RejectBody(command.node, proposal)
      <3>1. PICK proposal \in SeenProposalValues:
               /\ CommandMatches(command, command.node, proposal.view,
                                 proposal.subject)
               /\ RejectBody(command.node, proposal)
        BY <2>3
      <3> QED BY <3>1, Isa
           DEF CommandMatches, RejectBody, BodyRecord
    <2>4. CASE \E qc \in DecisionQcValues:
                    /\ CommandMatches(
                         command, command.node, qc.view, qc.subject)
                    /\ ValidateDecidedBody(command.node, qc)
      <3>1. PICK qc \in DecisionQcValues:
               /\ CommandMatches(command, command.node, qc.view, qc.subject)
               /\ ValidateDecidedBody(command.node, qc)
        BY <2>4
      <3> QED BY <3>1, Isa
           DEF CommandMatches, ValidateDecidedBody, BodyValidatedBy
    <2>5. CASE \E qc \in prepareQCs:
                    /\ CommandMatches(
                         command, command.node, qc.view, qc.subject)
                    /\ ValidateLockedBody(command.node, qc)
      <3>1. PICK qc \in prepareQCs:
               /\ CommandMatches(command, command.node, qc.view, qc.subject)
               /\ ValidateLockedBody(command.node, qc)
        BY <2>5
      <3> QED BY <3>1, Isa
           DEF CommandMatches, ValidateLockedBody, BodyValidatedBy
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5
  <1> QED BY <1>1

(***************************************************************************
Locked-round CommitVote recovery after a TC install.  Prepare admission remains
current-view-only.  The install clears only the installing node's volatile
vote receipts.  Retained CommitVote control is still retryable, and every
Commit delivery or locally formed CommitQC requires the exact durable Prepare
lock.  Persisting a replacement lock retires the superseded historical pool
while preserving current-view work and the new exact locked Commit pool.
***************************************************************************)

THEOREM PrepareVoteAdmissionIsCurrentView ==
  \A node, vote:
    (vote.phase = "Prepare" /\ VoteRoundAdmissible(node, vote))
      => vote.view = nodeView[node]
BY DEF VoteRoundAdmissible

THEOREM CommitVoteAdmissionIsExactLockedCommit ==
  \A node, vote:
    (vote.phase = "Commit" /\ VoteRoundAdmissible(node, vote))
      => LockedPrepareRound(node, vote.view, vote.subject)
BY DEF VoteRoundAdmissible

THEOREM CommitFormationIsExactLockedRound ==
  \A node, roundView, subject:
    CommitRoundAdmissible(node, roundView, subject)
      => LockedPrepareRound(node, roundView, subject)
BY DEF CommitRoundAdmissible

=============================================================================
