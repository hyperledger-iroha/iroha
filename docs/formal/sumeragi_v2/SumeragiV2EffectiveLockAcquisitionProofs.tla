---- MODULE SumeragiV2EffectiveLockAcquisitionProofs ----
EXTENDS SumeragiV2EffectiveLockAcquisition, FiniteSetTheorems,
        SumeragiV2TemporalLemmas, TLAPS

(***************************************************************************
Deductive proof boundary for the executable locked-body owner.

The TLC configuration is a bounded counterexample search only.  The lemmas
below establish the symbolic inductive identity/classification invariant and
then discharge the two weak-fairness properties without fixing a concrete
carrier cardinality.
***************************************************************************)

AcquisitionBaseTypeInvariant ==
  /\ AcquisitionConfiguration
  /\ desiredRound \in 0..MaxAcquisitionLockRound
  /\ desiredSubject \in AcquisitionSubjects
  /\ consumerView \in 0..MaxAcquisitionConsumerView
  /\ consumerGeneration \in 0..MaxAcquisitionGeneration
  /\ desiredRound <= consumerView
  /\ physicalId \in 0..MaxAcquisitionId
  /\ nextPhysicalId \in 1..(MaxAcquisitionId + 1)
  /\ physicalSubject \in AcquisitionSubjects
  /\ acquisitionPhase \in AcquisitionPhases
  /\ issuedLoads \subseteq AcquisitionLoadSet
  /\ IsFiniteSet(issuedLoads)
  /\ durableSubjects \subseteq AcquisitionSubjects
  /\ deliveryValid \in BOOLEAN
  /\ deliveredRound \in 0..MaxAcquisitionLockRound
  /\ deliveredSubject \in AcquisitionSubjects
  /\ deliveredView \in 0..MaxAcquisitionConsumerView
  /\ deliveredGeneration \in 0..MaxAcquisitionGeneration
  /\ decided \in BOOLEAN

AcquisitionIdentityVars ==
  <<desiredSubject, physicalId, nextPhysicalId, physicalSubject,
    acquisitionPhase, issuedLoads>>

THEOREM AcquisitionTypeInvariantDecomposition ==
  AcquisitionTypeInvariant
    <=> /\ AcquisitionBaseTypeInvariant
        /\ ExactAcquisitionIdentityInvariant
        /\ CompletionClassificationInvariant
BY DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant

THEOREM AcquisitionBaseImpliesCompletionClassification ==
  AcquisitionBaseTypeInvariant => CompletionClassificationInvariant
BY SMTT(30)
   DEF AcquisitionBaseTypeInvariant, AcquisitionConfiguration,
       CompletionClassificationInvariant,
       PhysicalCompletionDisposition, AcquisitionPhases

THEOREM PrimedAcquisitionBaseImpliesCompletionClassification ==
  AcquisitionBaseTypeInvariant' => CompletionClassificationInvariant'
BY SMTT(30)
   DEF AcquisitionBaseTypeInvariant, AcquisitionConfiguration,
       CompletionClassificationInvariant,
       PhysicalCompletionDisposition, AcquisitionPhases

THEOREM AcquisitionIdentityStutterPreservesExactIdentity ==
  ExactAcquisitionIdentityInvariant
    /\ UNCHANGED AcquisitionIdentityVars
    => ExactAcquisitionIdentityInvariant'
PROOF
  <1>1. ASSUME ExactAcquisitionIdentityInvariant,
                UNCHANGED AcquisitionIdentityVars
         PROVE ExactAcquisitionIdentityInvariant'
    <2>1. /\ desiredSubject' = desiredSubject
           /\ physicalId' = physicalId
           /\ nextPhysicalId' = nextPhysicalId
           /\ physicalSubject' = physicalSubject
           /\ acquisitionPhase' = acquisitionPhase
           /\ issuedLoads' = issuedLoads
      BY <1>1, SMTT(30) DEF AcquisitionIdentityVars
    <2> QED BY <1>1, <2>1, SMTT(30)
         DEF ExactAcquisitionIdentityInvariant, PhysicalLoadIds
  <1> QED BY <1>1

THEOREM LoadingDesiredRebindPreservesExactIdentity ==
  ExactAcquisitionIdentityInvariant
    /\ acquisitionPhase = "Loading"
    /\ acquisitionPhase' = "Loading"
    /\ UNCHANGED <<physicalId, nextPhysicalId,
                    physicalSubject, issuedLoads>>
    => ExactAcquisitionIdentityInvariant'
PROOF
  <1>1. ASSUME ExactAcquisitionIdentityInvariant,
                acquisitionPhase = "Loading",
                acquisitionPhase' = "Loading",
                UNCHANGED <<physicalId, nextPhysicalId,
                            physicalSubject, issuedLoads>>
         PROVE ExactAcquisitionIdentityInvariant'
    <2>1. /\ physicalId' = physicalId
           /\ nextPhysicalId' = nextPhysicalId
           /\ physicalSubject' = physicalSubject
           /\ issuedLoads' = issuedLoads
      BY <1>1, SMTT(30)
    <2> QED BY <1>1, <2>1, SMTT(30)
         DEF ExactAcquisitionIdentityInvariant, PhysicalLoadIds
  <1> QED BY <1>1

THEOREM StartPhysicalLoadPreservesExactIdentity ==
  \A subject:
    /\ ExactAcquisitionIdentityInvariant
    /\ nextPhysicalId \in Nat
    /\ desiredSubject' = subject
    /\ StartPhysicalLoad(subject)
    => ExactAcquisitionIdentityInvariant'
PROOF
  <1>1. ASSUME NEW subject,
                ExactAcquisitionIdentityInvariant,
                nextPhysicalId \in Nat,
                desiredSubject' = subject,
                StartPhysicalLoad(subject)
         PROVE ExactAcquisitionIdentityInvariant'
    <2>1. \A load \in issuedLoads: load.id < nextPhysicalId
      BY <1>1, SMTT(30)
         DEF ExactAcquisitionIdentityInvariant, PhysicalLoadIds
    <2>2. PhysicalLoadIds' =
             PhysicalLoadIds \cup {nextPhysicalId}
      BY <1>1, SMTT(30)
         DEF StartPhysicalLoad, PhysicalLoadIds, AcquisitionLoad
    <2>3. PhysicalLoadIds' = 0..(nextPhysicalId' - 1)
      BY <1>1, <2>2, FS_Interval, SMTT(30)
         DEF StartPhysicalLoad, ExactAcquisitionIdentityInvariant
    <2>4. \A left, right \in issuedLoads':
             left.id = right.id => left.subject = right.subject
      BY <1>1, <2>1, SMTT(30)
         DEF StartPhysicalLoad, ExactAcquisitionIdentityInvariant,
             AcquisitionLoad
    <2>5. AcquisitionLoad(physicalId', physicalSubject')
             \in issuedLoads'
      BY <1>1, SMTT(30) DEF StartPhysicalLoad
    <2>6. physicalId' < nextPhysicalId'
      BY <1>1, SMTT(30) DEF StartPhysicalLoad
    <2>7. /\ (physicalSubject' # desiredSubject'
                  => acquisitionPhase' = "Loading")
           /\ (acquisitionPhase' \in {"Waiting", "Ready"}
                  => physicalSubject' = desiredSubject')
      BY <1>1, SMTT(30) DEF StartPhysicalLoad
    <2> QED BY <2>3, <2>4, <2>5, <2>6, <2>7, SMTT(30)
         DEF ExactAcquisitionIdentityInvariant
  <1> QED BY <1>1

THEOREM AcquisitionInitEstablishesBaseTypeInvariant ==
  AcquisitionInit => AcquisitionBaseTypeInvariant
BY FS_EmptySet, FS_Singleton, SMTT(30)
   DEF AcquisitionInit, AcquisitionBaseTypeInvariant,
       AcquisitionConfiguration, AcquisitionLoad,
       AcquisitionLoadSet, AcquisitionPhases

THEOREM AcquisitionInitEstablishesExactIdentityInvariant ==
  AcquisitionInit => ExactAcquisitionIdentityInvariant
BY FS_Singleton, FS_Interval, SMTT(30)
   DEF AcquisitionInit, AcquisitionConfiguration,
       ExactAcquisitionIdentityInvariant, PhysicalLoadIds,
       AcquisitionLoad

THEOREM AcquisitionInitEstablishesTypeInvariant ==
  AcquisitionInit => AcquisitionTypeInvariant
BY AcquisitionInitEstablishesBaseTypeInvariant,
   AcquisitionInitEstablishesExactIdentityInvariant,
   AcquisitionBaseImpliesCompletionClassification
   DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant

THEOREM RebindSameLockPreservesTypeInvariant ==
  \A nextView, nextGeneration:
    AcquisitionTypeInvariant
      /\ RebindSameLock(nextView, nextGeneration)
      => AcquisitionTypeInvariant'
PROOF
  <1>1. ASSUME NEW nextView, NEW nextGeneration,
                AcquisitionTypeInvariant,
                RebindSameLock(nextView, nextGeneration)
         PROVE AcquisitionTypeInvariant'
    <2>1. AcquisitionBaseTypeInvariant'
      BY <1>1, SMTT(30)
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant,
             RebindSameLock, AcquisitionConfiguration,
             AcquisitionPhases, DeliveryVars
    <2>2. ExactAcquisitionIdentityInvariant'
      BY <1>1, AcquisitionIdentityStutterPreservesExactIdentity
         DEF AcquisitionTypeInvariant, RebindSameLock,
             AcquisitionIdentityVars, DeliveryVars
    <2>3. CompletionClassificationInvariant'
      BY <2>1, PrimedAcquisitionBaseImpliesCompletionClassification
    <2> QED BY <2>1, <2>2, <2>3
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant
  <1> QED BY <1>1

THEOREM InstallHigherLockPreservesTypeInvariant ==
  \A nextRound, nextSubject, nextView, nextGeneration:
    AcquisitionTypeInvariant
      /\ InstallHigherLock(nextRound, nextSubject, nextView, nextGeneration)
      => AcquisitionTypeInvariant'
PROOF
  <1>1. ASSUME NEW nextRound, NEW nextSubject,
                NEW nextView, NEW nextGeneration,
                AcquisitionTypeInvariant,
                InstallHigherLock(
                  nextRound, nextSubject, nextView, nextGeneration)
         PROVE AcquisitionTypeInvariant'
    <2>1. AcquisitionBaseTypeInvariant'
      BY <1>1, FS_AddElement, SMTT(60)
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant,
             AcquisitionConfiguration, InstallHigherLock,
             StartPhysicalLoad, AcquisitionLoadSet,
             AcquisitionLoad, AcquisitionPhases, DeliveryVars
    <2>2. ExactAcquisitionIdentityInvariant'
      <3>1. CASE nextSubject = desiredSubject
        BY <1>1, <3>1,
           AcquisitionIdentityStutterPreservesExactIdentity
           DEF AcquisitionTypeInvariant, InstallHigherLock,
               AcquisitionIdentityVars, DeliveryVars
      <3>2. CASE nextSubject # desiredSubject
                    /\ acquisitionPhase = "Loading"
        BY <1>1, <3>2, LoadingDesiredRebindPreservesExactIdentity
           DEF AcquisitionTypeInvariant, InstallHigherLock,
               DeliveryVars
      <3>3. CASE nextSubject # desiredSubject
                    /\ acquisitionPhase # "Loading"
        BY <1>1, <3>3, StartPhysicalLoadPreservesExactIdentity
           DEF AcquisitionTypeInvariant, InstallHigherLock,
               AcquisitionBaseTypeInvariant, DeliveryVars
      <3> QED BY <3>1, <3>2, <3>3
    <2>3. CompletionClassificationInvariant'
      BY <2>1, PrimedAcquisitionBaseImpliesCompletionClassification
    <2> QED BY <2>1, <2>2, <2>3
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant
  <1> QED BY <1>1

THEOREM CompleteAvailableLoadPreservesTypeInvariant ==
  AcquisitionTypeInvariant /\ CompleteAvailableLoad
    => AcquisitionTypeInvariant'
PROOF
  <1>1. ASSUME AcquisitionTypeInvariant, CompleteAvailableLoad
         PROVE AcquisitionTypeInvariant'
    <2>1. AcquisitionBaseTypeInvariant'
      BY <1>1, FS_AddElement, SMTT(60)
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant,
             AcquisitionConfiguration, CompleteAvailableLoad,
             StartPhysicalLoad, AcquisitionLoadSet,
             AcquisitionLoad, AcquisitionPhases, DeliveryVars
    <2>2. ExactAcquisitionIdentityInvariant'
      <3>1. CASE physicalSubject = desiredSubject
        BY <1>1, <3>1, SMTT(30)
           DEF AcquisitionTypeInvariant, CompleteAvailableLoad,
               ExactAcquisitionIdentityInvariant, PhysicalLoadIds,
               DeliveryVars
      <3>2. CASE physicalSubject # desiredSubject
        BY <1>1, <3>2, StartPhysicalLoadPreservesExactIdentity
           DEF AcquisitionTypeInvariant, CompleteAvailableLoad,
               AcquisitionBaseTypeInvariant, DeliveryVars
      <3> QED BY <3>1, <3>2
    <2>3. CompletionClassificationInvariant'
      BY <2>1, PrimedAcquisitionBaseImpliesCompletionClassification
    <2> QED BY <2>1, <2>2, <2>3
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant
  <1> QED BY <1>1

THEOREM CompleteUnavailableLoadPreservesTypeInvariant ==
  AcquisitionTypeInvariant /\ CompleteUnavailableLoad
    => AcquisitionTypeInvariant'
PROOF
  <1>1. ASSUME AcquisitionTypeInvariant, CompleteUnavailableLoad
         PROVE AcquisitionTypeInvariant'
    <2>1. AcquisitionBaseTypeInvariant'
      BY <1>1, FS_AddElement, SMTT(60)
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant,
             AcquisitionConfiguration, CompleteUnavailableLoad,
             StartPhysicalLoad, AcquisitionLoadSet,
             AcquisitionLoad, AcquisitionPhases, DeliveryVars
    <2>2. ExactAcquisitionIdentityInvariant'
      <3>1. CASE physicalSubject = desiredSubject
        BY <1>1, <3>1, SMTT(30)
           DEF AcquisitionTypeInvariant, CompleteUnavailableLoad,
               ExactAcquisitionIdentityInvariant, PhysicalLoadIds,
               DeliveryVars
      <3>2. CASE physicalSubject # desiredSubject
        BY <1>1, <3>2, StartPhysicalLoadPreservesExactIdentity
           DEF AcquisitionTypeInvariant, CompleteUnavailableLoad,
               AcquisitionBaseTypeInvariant, DeliveryVars
      <3> QED BY <3>1, <3>2
    <2>3. CompletionClassificationInvariant'
      BY <2>1, PrimedAcquisitionBaseImpliesCompletionClassification
    <2> QED BY <2>1, <2>2, <2>3
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant
  <1> QED BY <1>1

THEOREM RecoverDesiredBodyPreservesTypeInvariant ==
  AcquisitionTypeInvariant /\ RecoverDesiredBody
    => AcquisitionTypeInvariant'
PROOF
  <1>1. ASSUME AcquisitionTypeInvariant, RecoverDesiredBody
         PROVE AcquisitionTypeInvariant'
    <2>1. AcquisitionBaseTypeInvariant'
      BY <1>1, SMTT(30)
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant,
             AcquisitionConfiguration, RecoverDesiredBody,
             AcquisitionPhases, DeliveryVars
    <2>2. ExactAcquisitionIdentityInvariant'
      BY <1>1, AcquisitionIdentityStutterPreservesExactIdentity
         DEF AcquisitionTypeInvariant, RecoverDesiredBody,
             AcquisitionIdentityVars, DeliveryVars
    <2>3. CompletionClassificationInvariant'
      BY <2>1, PrimedAcquisitionBaseImpliesCompletionClassification
    <2> QED BY <2>1, <2>2, <2>3
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant
  <1> QED BY <1>1

THEOREM RetryRecoveredBodyPreservesTypeInvariant ==
  AcquisitionTypeInvariant /\ RetryRecoveredBody
    => AcquisitionTypeInvariant'
PROOF
  <1>1. ASSUME AcquisitionTypeInvariant, RetryRecoveredBody
         PROVE AcquisitionTypeInvariant'
    <2>1. AcquisitionBaseTypeInvariant'
      BY <1>1, FS_AddElement, SMTT(60)
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant,
             AcquisitionConfiguration, RetryRecoveredBody,
             StartPhysicalLoad, AcquisitionLoadSet,
             AcquisitionLoad, AcquisitionPhases, DeliveryVars
    <2>2. ExactAcquisitionIdentityInvariant'
      BY <1>1, StartPhysicalLoadPreservesExactIdentity
         DEF AcquisitionTypeInvariant, RetryRecoveredBody,
             AcquisitionBaseTypeInvariant, DeliveryVars
    <2>3. CompletionClassificationInvariant'
      BY <2>1, PrimedAcquisitionBaseImpliesCompletionClassification
    <2> QED BY <2>1, <2>2, <2>3
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant
  <1> QED BY <1>1

THEOREM DeliverReadyBodyPreservesTypeInvariant ==
  AcquisitionTypeInvariant /\ DeliverReadyBody
    => AcquisitionTypeInvariant'
PROOF
  <1>1. ASSUME AcquisitionTypeInvariant, DeliverReadyBody
         PROVE AcquisitionTypeInvariant'
    <2>1. AcquisitionBaseTypeInvariant'
      BY <1>1, SMTT(30)
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant,
             AcquisitionConfiguration, DeliverReadyBody,
             CurrentConsumerDelivered, AcquisitionPhases, DeliveryVars
    <2>2. ExactAcquisitionIdentityInvariant'
      BY <1>1, AcquisitionIdentityStutterPreservesExactIdentity
         DEF AcquisitionTypeInvariant, DeliverReadyBody,
             AcquisitionIdentityVars, DeliveryVars
    <2>3. CompletionClassificationInvariant'
      BY <2>1, PrimedAcquisitionBaseImpliesCompletionClassification
    <2> QED BY <2>1, <2>2, <2>3
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant
  <1> QED BY <1>1

THEOREM RecordDecisionPreservesTypeInvariant ==
  AcquisitionTypeInvariant /\ RecordDecision
    => AcquisitionTypeInvariant'
PROOF
  <1>1. ASSUME AcquisitionTypeInvariant, RecordDecision
         PROVE AcquisitionTypeInvariant'
    <2>1. AcquisitionBaseTypeInvariant'
      BY <1>1, SMTT(30)
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant,
             AcquisitionConfiguration, RecordDecision,
             AcquisitionPhases, DeliveryVars
    <2>2. ExactAcquisitionIdentityInvariant'
      BY <1>1, AcquisitionIdentityStutterPreservesExactIdentity
         DEF AcquisitionTypeInvariant, RecordDecision,
             AcquisitionIdentityVars, DeliveryVars
    <2>3. CompletionClassificationInvariant'
      BY <2>1, PrimedAcquisitionBaseImpliesCompletionClassification
    <2> QED BY <2>1, <2>2, <2>3
         DEF AcquisitionTypeInvariant, AcquisitionBaseTypeInvariant
  <1> QED BY <1>1

THEOREM AcquisitionNextPreservesTypeInvariant ==
  AcquisitionTypeInvariant /\ AcquisitionNext
    => AcquisitionTypeInvariant'
PROOF
  <1>1. CASE \E nextView \in 0..MaxAcquisitionConsumerView,
                 nextGeneration \in 0..MaxAcquisitionGeneration:
               RebindSameLock(nextView, nextGeneration)
    BY <1>1, RebindSameLockPreservesTypeInvariant
  <1>2. CASE \E nextRound \in 0..MaxAcquisitionLockRound,
                 nextSubject \in AcquisitionSubjects,
                 nextView \in 0..MaxAcquisitionConsumerView,
                 nextGeneration \in 0..MaxAcquisitionGeneration:
               InstallHigherLock(
                 nextRound, nextSubject, nextView, nextGeneration)
    BY <1>2, InstallHigherLockPreservesTypeInvariant
  <1>3. CASE CompleteOwnedLoad
    BY <1>3, CompleteAvailableLoadPreservesTypeInvariant,
       CompleteUnavailableLoadPreservesTypeInvariant
       DEF CompleteOwnedLoad
  <1>4. CASE RecoverDesiredBody
    BY <1>4, RecoverDesiredBodyPreservesTypeInvariant
  <1>5. CASE RetryRecoveredBody
    BY <1>5, RetryRecoveredBodyPreservesTypeInvariant
  <1>6. CASE DeliverReadyBody
    BY <1>6, DeliverReadyBodyPreservesTypeInvariant
  <1>7. CASE RecordDecision
    BY <1>7, RecordDecisionPreservesTypeInvariant
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7
       DEF AcquisitionNext

THEOREM AcquisitionStepPreservesTypeInvariant ==
  AcquisitionTypeInvariant /\ [AcquisitionNext]_acquisitionVars
    => AcquisitionTypeInvariant'
PROOF
  <1>1. CASE AcquisitionNext
    BY <1>1, AcquisitionNextPreservesTypeInvariant
  <1>2. CASE UNCHANGED acquisitionVars
    <2>1. /\ desiredRound' = desiredRound
           /\ desiredSubject' = desiredSubject
           /\ consumerView' = consumerView
           /\ consumerGeneration' = consumerGeneration
           /\ physicalId' = physicalId
           /\ nextPhysicalId' = nextPhysicalId
           /\ physicalSubject' = physicalSubject
           /\ acquisitionPhase' = acquisitionPhase
           /\ issuedLoads' = issuedLoads
           /\ durableSubjects' = durableSubjects
           /\ deliveryValid' = deliveryValid
           /\ deliveredRound' = deliveredRound
           /\ deliveredSubject' = deliveredSubject
           /\ deliveredView' = deliveredView
           /\ deliveredGeneration' = deliveredGeneration
           /\ decided' = decided
      BY <1>2, SMTT(30) DEF acquisitionVars
    <2> QED BY <2>1, SMTT(30)
         DEF AcquisitionTypeInvariant,
             ExactAcquisitionIdentityInvariant,
             CompletionClassificationInvariant,
             PhysicalCompletionDisposition, PhysicalLoadIds
  <1> QED BY <1>1, <1>2

THEOREM AcquisitionSpecAlwaysTypeInvariant ==
  AcquisitionSpec => []AcquisitionTypeInvariant
PROOF
  <1>1. AcquisitionInit => AcquisitionTypeInvariant
    BY AcquisitionInitEstablishesTypeInvariant
  <1>2. AcquisitionTypeInvariant
           /\ [AcquisitionNext]_acquisitionVars
           => AcquisitionTypeInvariant'
    BY AcquisitionStepPreservesTypeInvariant
  <1> QED BY <1>1, <1>2, PTL DEF AcquisitionSpec

(***************************************************************************
The configured physical-ID horizon is justified by a reachable-state budget,
not by the scalar type bound alone.  Each higher lock can consume at most one
replacement read and one recovery retry.  A superseded owner and an
unrecovered exact owner retain the stricter headroom needed by their next
transition.
***************************************************************************)
AcquisitionReadBudgetInvariant ==
  /\ nextPhysicalId <= 2 * desiredRound + 2
  /\ (physicalSubject # desiredSubject
        => nextPhysicalId <= 2 * desiredRound)
  /\ (acquisitionPhase = "Waiting"
        => nextPhysicalId <= 2 * desiredRound + 1)
  /\ (/\ acquisitionPhase = "Loading"
       /\ physicalSubject = desiredSubject
       /\ desiredSubject \notin durableSubjects
      => nextPhysicalId <= 2 * desiredRound + 1)

THEOREM AcquisitionInitEstablishesReadBudgetInvariant ==
  AcquisitionInit => AcquisitionReadBudgetInvariant
BY SMTT(30)
   DEF AcquisitionInit, AcquisitionReadBudgetInvariant,
       AcquisitionConfiguration

THEOREM AcquisitionActionPreservesReadBudgetInvariant ==
  AcquisitionTypeInvariant
    /\ AcquisitionReadBudgetInvariant
    /\ AcquisitionNext
    => AcquisitionReadBudgetInvariant'
BY SMTT(60)
   DEF AcquisitionTypeInvariant, AcquisitionReadBudgetInvariant,
       AcquisitionNext, RebindSameLock, InstallHigherLock,
       CompleteOwnedLoad, CompleteAvailableLoad,
       CompleteUnavailableLoad, RecoverDesiredBody,
       RetryRecoveredBody, DeliverReadyBody, RecordDecision,
       StartPhysicalLoad, DeliveryVars

THEOREM AcquisitionStepPreservesReadBudgetInvariant ==
  AcquisitionTypeInvariant
    /\ AcquisitionReadBudgetInvariant
    /\ [AcquisitionNext]_acquisitionVars
    => AcquisitionReadBudgetInvariant'
PROOF
  <1>1. CASE AcquisitionNext
    BY <1>1, AcquisitionActionPreservesReadBudgetInvariant
  <1>2. CASE UNCHANGED acquisitionVars
    BY <1>2, SMTT(30)
       DEF acquisitionVars, AcquisitionReadBudgetInvariant
  <1> QED BY <1>1, <1>2

THEOREM AcquisitionSpecAlwaysReadBudgetInvariant ==
  AcquisitionSpec => []AcquisitionReadBudgetInvariant
PROOF
  <1>1. AcquisitionSpec => []AcquisitionTypeInvariant
    BY AcquisitionSpecAlwaysTypeInvariant
  <1>2. AcquisitionInit => AcquisitionReadBudgetInvariant
    BY AcquisitionInitEstablishesReadBudgetInvariant
  <1>3. AcquisitionTypeInvariant
           /\ AcquisitionReadBudgetInvariant
           /\ [AcquisitionNext]_acquisitionVars
           => AcquisitionReadBudgetInvariant'
    BY AcquisitionStepPreservesReadBudgetInvariant
  <1> QED BY <1>1, <1>2, <1>3, PTL DEF AcquisitionSpec

(***************************************************************************
An undecided exact lock has five strictly ordered acquisition states:

  5  superseded physical owner still loading,
  4  exact load reports local absence,
  3  exact owner waits for certified recovery,
  2  recovered owner waits to retry,
  1  exact durable load waits to become Ready.

Every other transition either preserves this rank, lowers it, installs a
higher lock, or records Decision.
***************************************************************************)
AcquisitionProgressRankCarrier == 1..5
AcquisitionProgressOrdering == OpToRel(<, Nat)

AcquisitionProgressGoal(round, subject) ==
  \/ decided
  \/ desiredRound > round
  \/ LockReady(round, subject)

AcquisitionProgressRank ==
  CASE acquisitionPhase = "Ready" -> 0
    [] physicalSubject # desiredSubject -> 5
    [] /\ acquisitionPhase = "Loading"
       /\ desiredSubject \notin durableSubjects -> 4
    [] /\ acquisitionPhase = "Waiting"
       /\ desiredSubject \notin durableSubjects -> 3
    [] acquisitionPhase = "Waiting" -> 2
    [] OTHER -> 1

AcquisitionProgressAtRank(round, subject, rank) ==
  /\ AcquisitionTypeInvariant
  /\ AcquisitionReadBudgetInvariant
  /\ DesiredLock(round, subject)
  /\ ~AcquisitionProgressGoal(round, subject)
  /\ AcquisitionProgressRank = rank

AcquisitionProgressExit(round, subject, rank) ==
  \/ AcquisitionProgressGoal(round, subject)
  \/ \E lower \in SetLessThan(
       rank, AcquisitionProgressOrdering,
       AcquisitionProgressRankCarrier):
       AcquisitionProgressAtRank(round, subject, lower)

AcquisitionProgressFairAction(rank) ==
  CASE rank \in {1, 4, 5} -> CompleteOwnedLoad
    [] rank = 3 -> RecoverDesiredBody
    [] OTHER -> RetryRecoveredBody

THEOREM AcquisitionProgressOrderingIsWellFounded ==
  IsWellFoundedOn(
    AcquisitionProgressOrdering, AcquisitionProgressRankCarrier)
PROOF
  <1>1. AcquisitionProgressRankCarrier \subseteq Nat
    BY SMTT(30) DEF AcquisitionProgressRankCarrier
  <1> QED BY <1>1, NatLessThanWellFounded,
       IsWellFoundedOnSubset
       DEF AcquisitionProgressOrdering

THEOREM AcquisitionPendingHasRank ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    /\ AcquisitionTypeInvariant
    /\ AcquisitionReadBudgetInvariant
    /\ DesiredLock(round, subject)
    /\ ~AcquisitionProgressGoal(round, subject)
    => AcquisitionProgressRank \in AcquisitionProgressRankCarrier
BY SMTT(30)
   DEF AcquisitionProgressGoal, AcquisitionProgressRank,
       AcquisitionProgressRankCarrier, AcquisitionTypeInvariant,
       ExactAcquisitionIdentityInvariant, AcquisitionPhases,
       DesiredLock, LockReady

THEOREM AcquisitionRankFairActionRefinesNext ==
  \A rank \in AcquisitionProgressRankCarrier:
    AcquisitionProgressFairAction(rank) => AcquisitionNext
BY SMTT(30)
   DEF AcquisitionProgressRankCarrier,
       AcquisitionProgressFairAction, AcquisitionNext,
       CompleteOwnedLoad

THEOREM AcquisitionRank1Characterization ==
  \A round, subject:
    AcquisitionProgressAtRank(round, subject, 1)
      => /\ ~decided
         /\ acquisitionPhase = "Loading"
         /\ physicalSubject = desiredSubject
         /\ desiredSubject \in durableSubjects
BY SMTT(30)
   DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
       AcquisitionProgressRank, AcquisitionTypeInvariant,
       ExactAcquisitionIdentityInvariant, AcquisitionPhases,
       DesiredLock, LockReady

THEOREM AcquisitionRank2Characterization ==
  \A round, subject:
    AcquisitionProgressAtRank(round, subject, 2)
      => /\ ~decided
         /\ acquisitionPhase = "Waiting"
         /\ physicalSubject = desiredSubject
         /\ desiredSubject \in durableSubjects
         /\ nextPhysicalId <= MaxAcquisitionId
BY SMTT(30)
   DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
       AcquisitionProgressRank, AcquisitionReadBudgetInvariant,
       AcquisitionTypeInvariant, AcquisitionConfiguration,
       ExactAcquisitionIdentityInvariant, AcquisitionPhases,
       DesiredLock, LockReady

THEOREM AcquisitionRank3Characterization ==
  \A round, subject:
    AcquisitionProgressAtRank(round, subject, 3)
      => /\ ~decided
         /\ acquisitionPhase = "Waiting"
         /\ physicalSubject = desiredSubject
         /\ desiredSubject \notin durableSubjects
BY SMTT(30)
   DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
       AcquisitionProgressRank, AcquisitionTypeInvariant,
       ExactAcquisitionIdentityInvariant, AcquisitionPhases,
       DesiredLock, LockReady

THEOREM AcquisitionRank4Characterization ==
  \A round, subject:
    AcquisitionProgressAtRank(round, subject, 4)
      => /\ ~decided
         /\ acquisitionPhase = "Loading"
         /\ physicalSubject = desiredSubject
         /\ desiredSubject \notin durableSubjects
BY SMTT(30)
   DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
       AcquisitionProgressRank, AcquisitionTypeInvariant,
       ExactAcquisitionIdentityInvariant, AcquisitionPhases,
       DesiredLock, LockReady

THEOREM AcquisitionRank5Characterization ==
  \A round, subject:
    AcquisitionProgressAtRank(round, subject, 5)
      => /\ ~decided
         /\ acquisitionPhase = "Loading"
         /\ physicalSubject # desiredSubject
         /\ nextPhysicalId <= MaxAcquisitionId
BY SMTT(30)
   DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
       AcquisitionProgressRank, AcquisitionReadBudgetInvariant,
       AcquisitionTypeInvariant, AcquisitionConfiguration,
       ExactAcquisitionIdentityInvariant, AcquisitionPhases,
       DesiredLock, LockReady

THEOREM AcquisitionRank1ActionIsEnabled ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionProgressAtRank(round, subject, 1)
      => ENABLED <<CompleteOwnedLoad>>_acquisitionVars
BY AcquisitionRank1Characterization, ExpandENABLED, Isa
   DEF CompleteOwnedLoad, CompleteAvailableLoad,
       CompleteUnavailableLoad, StartPhysicalLoad,
       acquisitionVars, DeliveryVars

THEOREM AcquisitionRank2ActionIsEnabled ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionProgressAtRank(round, subject, 2)
      => ENABLED <<RetryRecoveredBody>>_acquisitionVars
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionProgressAtRank(round, subject, 2)
         PROVE ENABLED <<RetryRecoveredBody>>_acquisitionVars
    <2>1. ENABLED RetryRecoveredBody
      BY <1>1, AcquisitionRank2Characterization,
         ExpandENABLED, Isa
         DEF RetryRecoveredBody, StartPhysicalLoad,
             DeliveryVars
    <2>2. RetryRecoveredBody \in BOOLEAN
      BY Isa DEF RetryRecoveredBody
    <2>3. <<RetryRecoveredBody>>_acquisitionVars \in BOOLEAN
      BY Isa
    <2>4. RetryRecoveredBody
             => <<RetryRecoveredBody>>_acquisitionVars
      BY <1>1, AcquisitionRank2Characterization, Isa
         DEF RetryRecoveredBody, StartPhysicalLoad,
             acquisitionVars, DeliveryVars
    <2>5. (ENABLED RetryRecoveredBody)
             => ENABLED <<RetryRecoveredBody>>_acquisitionVars
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

THEOREM AcquisitionRank3ActionIsEnabled ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionProgressAtRank(round, subject, 3)
      => ENABLED <<RecoverDesiredBody>>_acquisitionVars
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionProgressAtRank(round, subject, 3)
         PROVE ENABLED <<RecoverDesiredBody>>_acquisitionVars
    <2>1. ENABLED RecoverDesiredBody
      BY <1>1, AcquisitionRank3Characterization,
         ExpandENABLED, Isa
         DEF RecoverDesiredBody, DeliveryVars
    <2>2. RecoverDesiredBody \in BOOLEAN
      BY Isa DEF RecoverDesiredBody
    <2>3. <<RecoverDesiredBody>>_acquisitionVars \in BOOLEAN
      BY Isa
    <2>4. RecoverDesiredBody
             => <<RecoverDesiredBody>>_acquisitionVars
      BY <1>1, AcquisitionRank3Characterization, SMTT(30)
         DEF RecoverDesiredBody, acquisitionVars, DeliveryVars
    <2>5. (ENABLED RecoverDesiredBody)
             => ENABLED <<RecoverDesiredBody>>_acquisitionVars
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

THEOREM AcquisitionRank4ActionIsEnabled ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionProgressAtRank(round, subject, 4)
      => ENABLED <<CompleteOwnedLoad>>_acquisitionVars
BY AcquisitionRank4Characterization, ExpandENABLED, Isa
   DEF CompleteOwnedLoad, CompleteAvailableLoad,
       CompleteUnavailableLoad, StartPhysicalLoad,
       acquisitionVars, DeliveryVars

THEOREM AcquisitionRank5ActionIsEnabled ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionProgressAtRank(round, subject, 5)
      => ENABLED <<CompleteOwnedLoad>>_acquisitionVars
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionProgressAtRank(round, subject, 5)
         PROVE ENABLED <<CompleteOwnedLoad>>_acquisitionVars
    <2>1. ENABLED CompleteOwnedLoad
      BY <1>1, AcquisitionRank5Characterization,
         ExpandENABLED, Isa
         DEF CompleteOwnedLoad, CompleteAvailableLoad,
             CompleteUnavailableLoad, StartPhysicalLoad,
             DeliveryVars
    <2>2. CompleteOwnedLoad \in BOOLEAN
      BY Isa DEF CompleteOwnedLoad
    <2>3. <<CompleteOwnedLoad>>_acquisitionVars \in BOOLEAN
      BY Isa
    <2>4. CompleteOwnedLoad
             => <<CompleteOwnedLoad>>_acquisitionVars
      BY <1>1, AcquisitionRank5Characterization, Isa
         DEF CompleteOwnedLoad, CompleteAvailableLoad,
             CompleteUnavailableLoad, StartPhysicalLoad,
             acquisitionVars, DeliveryVars
    <2>5. (ENABLED CompleteOwnedLoad)
             => ENABLED <<CompleteOwnedLoad>>_acquisitionVars
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

THEOREM AcquisitionRank1ActionStrictlyProgresses ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    /\ AcquisitionProgressAtRank(round, subject, 1)
    /\ <<CompleteOwnedLoad>>_acquisitionVars
    => AcquisitionProgressExit(round, subject, 1)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionProgressAtRank(round, subject, 1),
                <<CompleteOwnedLoad>>_acquisitionVars
         PROVE AcquisitionProgressExit(round, subject, 1)'
    <2>1. CompleteAvailableLoad
      BY <1>1, AcquisitionRank1Characterization, SMTT(30)
         DEF CompleteOwnedLoad, CompleteAvailableLoad,
             CompleteUnavailableLoad
    <2>2. AcquisitionTypeInvariant'
      BY <1>1, <2>1, CompleteAvailableLoadPreservesTypeInvariant
         DEF AcquisitionProgressAtRank
    <2>3. AcquisitionReadBudgetInvariant'
      BY <1>1, <2>1, AcquisitionActionPreservesReadBudgetInvariant
         DEF AcquisitionProgressAtRank, AcquisitionNext
    <2>4. AcquisitionProgressGoal(round, subject)'
      BY <1>1, <2>1, AcquisitionRank1Characterization, SMTT(30)
         DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
             CompleteAvailableLoad, DesiredLock, LockReady,
             DeliveryVars
    <2> QED BY <2>4
         DEF AcquisitionProgressExit
  <1> QED BY <1>1

THEOREM AcquisitionRank2ActionStrictlyProgresses ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    /\ AcquisitionProgressAtRank(round, subject, 2)
    /\ <<RetryRecoveredBody>>_acquisitionVars
    => AcquisitionProgressExit(round, subject, 2)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionProgressAtRank(round, subject, 2),
                <<RetryRecoveredBody>>_acquisitionVars
         PROVE AcquisitionProgressExit(round, subject, 2)'
    <2>1. RetryRecoveredBody
      BY <1>1
    <2>2. AcquisitionTypeInvariant'
      BY <1>1, <2>1, RetryRecoveredBodyPreservesTypeInvariant
         DEF AcquisitionProgressAtRank
    <2>3. AcquisitionReadBudgetInvariant'
      BY <1>1, <2>1, AcquisitionActionPreservesReadBudgetInvariant
         DEF AcquisitionProgressAtRank, AcquisitionNext
    <2>4. AcquisitionProgressAtRank(round, subject, 1)'
      BY <1>1, <2>1, <2>2, <2>3,
         AcquisitionRank2Characterization, SMTT(30)
         DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
             AcquisitionProgressRank, RetryRecoveredBody,
             StartPhysicalLoad, DesiredLock, LockReady,
             DeliveryVars
    <2>5. 1 \in SetLessThan(
               2, AcquisitionProgressOrdering,
               AcquisitionProgressRankCarrier)
      BY Isa
         DEF AcquisitionProgressOrdering,
             AcquisitionProgressRankCarrier,
             SetLessThan, OpToRel
    <2> QED BY <2>4, <2>5
         DEF AcquisitionProgressExit
  <1> QED BY <1>1

THEOREM AcquisitionRank3ActionStrictlyProgresses ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    /\ AcquisitionProgressAtRank(round, subject, 3)
    /\ <<RecoverDesiredBody>>_acquisitionVars
    => AcquisitionProgressExit(round, subject, 3)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionProgressAtRank(round, subject, 3),
                <<RecoverDesiredBody>>_acquisitionVars
         PROVE AcquisitionProgressExit(round, subject, 3)'
    <2>1. RecoverDesiredBody
      BY <1>1
    <2>2. AcquisitionTypeInvariant'
      BY <1>1, <2>1, RecoverDesiredBodyPreservesTypeInvariant
         DEF AcquisitionProgressAtRank
    <2>3. AcquisitionReadBudgetInvariant'
      BY <1>1, <2>1, AcquisitionActionPreservesReadBudgetInvariant
         DEF AcquisitionProgressAtRank, AcquisitionNext
    <2>4. AcquisitionProgressAtRank(round, subject, 2)'
      BY <1>1, <2>1, <2>2, <2>3,
         AcquisitionRank3Characterization, SMTT(30)
         DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
             AcquisitionProgressRank, RecoverDesiredBody,
             DesiredLock, LockReady, DeliveryVars
    <2>5. 2 \in SetLessThan(
               3, AcquisitionProgressOrdering,
               AcquisitionProgressRankCarrier)
      BY Isa
         DEF AcquisitionProgressOrdering,
             AcquisitionProgressRankCarrier,
             SetLessThan, OpToRel
    <2> QED BY <2>4, <2>5
         DEF AcquisitionProgressExit
  <1> QED BY <1>1

THEOREM AcquisitionRank4ActionStrictlyProgresses ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    /\ AcquisitionProgressAtRank(round, subject, 4)
    /\ <<CompleteOwnedLoad>>_acquisitionVars
    => AcquisitionProgressExit(round, subject, 4)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionProgressAtRank(round, subject, 4),
                <<CompleteOwnedLoad>>_acquisitionVars
         PROVE AcquisitionProgressExit(round, subject, 4)'
    <2>1. CompleteUnavailableLoad
      BY <1>1, AcquisitionRank4Characterization, SMTT(30)
         DEF CompleteOwnedLoad, CompleteAvailableLoad,
             CompleteUnavailableLoad
    <2>2. AcquisitionTypeInvariant'
      BY <1>1, <2>1, CompleteUnavailableLoadPreservesTypeInvariant
         DEF AcquisitionProgressAtRank
    <2>3. AcquisitionReadBudgetInvariant'
      BY <1>1, <2>1, AcquisitionActionPreservesReadBudgetInvariant
         DEF AcquisitionProgressAtRank, AcquisitionNext
    <2>4. AcquisitionProgressAtRank(round, subject, 3)'
      BY <1>1, <2>1, <2>2, <2>3,
         AcquisitionRank4Characterization, SMTT(30)
         DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
             AcquisitionProgressRank, CompleteUnavailableLoad,
             DesiredLock, LockReady, DeliveryVars
    <2>5. 3 \in SetLessThan(
               4, AcquisitionProgressOrdering,
               AcquisitionProgressRankCarrier)
      BY Isa
         DEF AcquisitionProgressOrdering,
             AcquisitionProgressRankCarrier,
             SetLessThan, OpToRel
    <2> QED BY <2>4, <2>5
         DEF AcquisitionProgressExit
  <1> QED BY <1>1

THEOREM AcquisitionRank5ActionStrictlyProgresses ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    /\ AcquisitionProgressAtRank(round, subject, 5)
    /\ <<CompleteOwnedLoad>>_acquisitionVars
    => AcquisitionProgressExit(round, subject, 5)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionProgressAtRank(round, subject, 5),
                <<CompleteOwnedLoad>>_acquisitionVars
         PROVE AcquisitionProgressExit(round, subject, 5)'
    <2>1. CompleteOwnedLoad
      BY <1>1
    <2>2. AcquisitionNext
      BY <2>1 DEF AcquisitionNext
    <2>3. AcquisitionTypeInvariant'
      BY <1>1, <2>2, AcquisitionNextPreservesTypeInvariant
         DEF AcquisitionProgressAtRank
    <2>4. AcquisitionReadBudgetInvariant'
      BY <1>1, <2>2, AcquisitionActionPreservesReadBudgetInvariant
         DEF AcquisitionProgressAtRank
    <2>5. CASE desiredSubject \in durableSubjects
      <3>1. AcquisitionProgressAtRank(round, subject, 1)'
        BY <1>1, <2>1, <2>3, <2>4, <2>5,
           AcquisitionRank5Characterization, SMTT(30)
           DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
               AcquisitionProgressRank, CompleteOwnedLoad,
               CompleteAvailableLoad, CompleteUnavailableLoad,
               StartPhysicalLoad, DesiredLock, LockReady,
               DeliveryVars
      <3>2. 1 \in SetLessThan(
                 5, AcquisitionProgressOrdering,
                 AcquisitionProgressRankCarrier)
        BY Isa
           DEF AcquisitionProgressOrdering,
               AcquisitionProgressRankCarrier,
               SetLessThan, OpToRel
      <3> QED BY <3>1, <3>2
           DEF AcquisitionProgressExit
    <2>6. CASE desiredSubject \notin durableSubjects
      <3>1. AcquisitionProgressAtRank(round, subject, 4)'
        BY <1>1, <2>1, <2>3, <2>4, <2>6,
           AcquisitionRank5Characterization, SMTT(30)
           DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
               AcquisitionProgressRank, CompleteOwnedLoad,
               CompleteAvailableLoad, CompleteUnavailableLoad,
               StartPhysicalLoad, DesiredLock, LockReady,
               DeliveryVars
      <3>2. 4 \in SetLessThan(
                 5, AcquisitionProgressOrdering,
                 AcquisitionProgressRankCarrier)
        BY Isa
           DEF AcquisitionProgressOrdering,
               AcquisitionProgressRankCarrier,
               SetLessThan, OpToRel
      <3> QED BY <3>1, <3>2
           DEF AcquisitionProgressExit
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM AcquisitionProgressRankCarrierCases ==
  \A rank \in AcquisitionProgressRankCarrier:
    \/ rank = 1
    \/ rank = 2
    \/ rank = 3
    \/ rank = 4
    \/ rank = 5
BY Isa DEF AcquisitionProgressRankCarrier

THEOREM AcquisitionRebindPreservesProgressRank ==
  \A round, subject, rank, nextView, nextGeneration:
    /\ AcquisitionProgressAtRank(round, subject, rank)
    /\ RebindSameLock(nextView, nextGeneration)
    => AcquisitionProgressAtRank(round, subject, rank)'
PROOF
  <1>1. ASSUME NEW round, NEW subject, NEW rank,
                NEW nextView, NEW nextGeneration,
                AcquisitionProgressAtRank(round, subject, rank),
                RebindSameLock(nextView, nextGeneration)
         PROVE AcquisitionProgressAtRank(round, subject, rank)'
    <2>1. AcquisitionTypeInvariant'
      BY <1>1, RebindSameLockPreservesTypeInvariant
         DEF AcquisitionProgressAtRank
    <2>2. AcquisitionReadBudgetInvariant'
      BY <1>1, SMTT(30)
         DEF AcquisitionProgressAtRank,
             AcquisitionReadBudgetInvariant,
             RebindSameLock, DeliveryVars
    <2> QED BY <1>1, <2>1, <2>2, SMTT(30)
         DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
             AcquisitionProgressRank, RebindSameLock,
             DesiredLock, LockReady, DeliveryVars
  <1> QED BY <1>1

THEOREM AcquisitionHigherLockExitsProgressRank ==
  \A round, subject, rank,
     nextRound, nextSubject, nextView, nextGeneration:
    /\ AcquisitionProgressAtRank(round, subject, rank)
    /\ InstallHigherLock(
         nextRound, nextSubject, nextView, nextGeneration)
    => AcquisitionProgressExit(round, subject, rank)'
PROOF
  <1>1. ASSUME NEW round, NEW subject, NEW rank,
                NEW nextRound, NEW nextSubject,
                NEW nextView, NEW nextGeneration,
                AcquisitionProgressAtRank(round, subject, rank),
                InstallHigherLock(
                  nextRound, nextSubject, nextView, nextGeneration)
         PROVE AcquisitionProgressExit(round, subject, rank)'
    <2>1. desiredRound = round
      BY <1>1 DEF AcquisitionProgressAtRank, DesiredLock
    <2>2. /\ nextRound \in (desiredRound + 1)..MaxAcquisitionLockRound
           /\ desiredRound' = nextRound
      BY <1>1 DEF InstallHigherLock
    <2>3. /\ desiredRound \in Nat
           /\ round \in Nat
           /\ nextRound \in Int
      BY <1>1, SMTT(30)
         DEF AcquisitionProgressAtRank, AcquisitionTypeInvariant,
             AcquisitionBaseTypeInvariant, AcquisitionConfiguration,
             DesiredLock, InstallHigherLock
    <2>4. desiredRound' > round
      BY <2>1, <2>2, <2>3, SMTT(30)
    <2>5. AcquisitionProgressGoal(round, subject)'
      BY <2>4 DEF AcquisitionProgressGoal
    <2> QED BY <2>5 DEF AcquisitionProgressExit
  <1> QED BY <1>1

THEOREM AcquisitionRank4RecoveryStrictlyProgresses ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    /\ AcquisitionProgressAtRank(round, subject, 4)
    /\ RecoverDesiredBody
    => AcquisitionProgressExit(round, subject, 4)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionProgressAtRank(round, subject, 4),
                RecoverDesiredBody
         PROVE AcquisitionProgressExit(round, subject, 4)'
    <2>1. AcquisitionTypeInvariant'
      BY <1>1, RecoverDesiredBodyPreservesTypeInvariant
         DEF AcquisitionProgressAtRank
    <2>2. AcquisitionReadBudgetInvariant'
      BY <1>1, AcquisitionActionPreservesReadBudgetInvariant
         DEF AcquisitionProgressAtRank, AcquisitionNext
    <2>3. AcquisitionProgressAtRank(round, subject, 1)'
      BY <1>1, <2>1, <2>2,
         AcquisitionRank4Characterization, SMTT(30)
         DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
             AcquisitionProgressRank, RecoverDesiredBody,
             DesiredLock, LockReady, DeliveryVars
    <2>4. 1 \in SetLessThan(
               4, AcquisitionProgressOrdering,
               AcquisitionProgressRankCarrier)
      BY Isa
         DEF AcquisitionProgressOrdering,
             AcquisitionProgressRankCarrier,
             SetLessThan, OpToRel
    <2> QED BY <2>3, <2>4 DEF AcquisitionProgressExit
  <1> QED BY <1>1

THEOREM AcquisitionRank5RecoveryPreservesProgressRank ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    /\ AcquisitionProgressAtRank(round, subject, 5)
    /\ RecoverDesiredBody
    => AcquisitionProgressAtRank(round, subject, 5)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionProgressAtRank(round, subject, 5),
                RecoverDesiredBody
         PROVE AcquisitionProgressAtRank(round, subject, 5)'
    <2>1. AcquisitionTypeInvariant'
      BY <1>1, RecoverDesiredBodyPreservesTypeInvariant
         DEF AcquisitionProgressAtRank
    <2>2. AcquisitionReadBudgetInvariant'
      BY <1>1, AcquisitionActionPreservesReadBudgetInvariant
         DEF AcquisitionProgressAtRank, AcquisitionNext
    <2> QED BY <1>1, <2>1, <2>2,
         AcquisitionRank5Characterization, SMTT(30)
         DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
             AcquisitionProgressRank, RecoverDesiredBody,
             DesiredLock, LockReady, DeliveryVars
  <1> QED BY <1>1

THEOREM AcquisitionCompleteDoesNotOrphanProgressRank ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects,
     rank \in AcquisitionProgressRankCarrier:
    /\ AcquisitionProgressAtRank(round, subject, rank)
    /\ CompleteOwnedLoad
    => AcquisitionProgressExit(round, subject, rank)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                NEW rank \in AcquisitionProgressRankCarrier,
                AcquisitionProgressAtRank(round, subject, rank),
                CompleteOwnedLoad
         PROVE AcquisitionProgressExit(round, subject, rank)'
    <2>1. CASE rank = 1
      <3>1. <<CompleteOwnedLoad>>_acquisitionVars
        BY <1>1, <2>1, AcquisitionRank1Characterization, Isa
           DEF CompleteOwnedLoad, CompleteAvailableLoad,
               CompleteUnavailableLoad, StartPhysicalLoad,
               acquisitionVars, DeliveryVars
      <3> QED BY <1>1, <2>1, <3>1,
           AcquisitionRank1ActionStrictlyProgresses
    <2>2. CASE rank = 2
      BY <1>1, <2>2, AcquisitionRank2Characterization, SMTT(30)
         DEF CompleteOwnedLoad, CompleteAvailableLoad,
             CompleteUnavailableLoad
    <2>3. CASE rank = 3
      BY <1>1, <2>3, AcquisitionRank3Characterization, SMTT(30)
         DEF CompleteOwnedLoad, CompleteAvailableLoad,
             CompleteUnavailableLoad
    <2>4. CASE rank = 4
      <3>1. <<CompleteOwnedLoad>>_acquisitionVars
        BY <1>1, <2>4, AcquisitionRank4Characterization, Isa
           DEF CompleteOwnedLoad, CompleteAvailableLoad,
               CompleteUnavailableLoad, StartPhysicalLoad,
               acquisitionVars, DeliveryVars
      <3> QED BY <1>1, <2>4, <3>1,
           AcquisitionRank4ActionStrictlyProgresses
    <2>5. CASE rank = 5
      <3>1. <<CompleteOwnedLoad>>_acquisitionVars
        BY <1>1, <2>5, AcquisitionRank5Characterization, Isa
           DEF CompleteOwnedLoad, CompleteAvailableLoad,
               CompleteUnavailableLoad, StartPhysicalLoad,
               acquisitionVars, DeliveryVars
      <3> QED BY <1>1, <2>5, <3>1,
           AcquisitionRank5ActionStrictlyProgresses
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5,
         AcquisitionProgressRankCarrierCases
  <1> QED BY <1>1

THEOREM AcquisitionRecoveryDoesNotOrphanProgressRank ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects,
     rank \in AcquisitionProgressRankCarrier:
    /\ AcquisitionProgressAtRank(round, subject, rank)
    /\ RecoverDesiredBody
    => \/ AcquisitionProgressAtRank(round, subject, rank)'
       \/ AcquisitionProgressExit(round, subject, rank)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                NEW rank \in AcquisitionProgressRankCarrier,
                AcquisitionProgressAtRank(round, subject, rank),
                RecoverDesiredBody
         PROVE \/ AcquisitionProgressAtRank(round, subject, rank)'
               \/ AcquisitionProgressExit(round, subject, rank)'
    <2>1. CASE rank = 1
      BY <1>1, <2>1, AcquisitionRank1Characterization, SMTT(30)
         DEF RecoverDesiredBody
    <2>2. CASE rank = 2
      BY <1>1, <2>2, AcquisitionRank2Characterization, SMTT(30)
         DEF RecoverDesiredBody
    <2>3. CASE rank = 3
      <3>1. <<RecoverDesiredBody>>_acquisitionVars
        BY <1>1, <2>3, AcquisitionRank3Characterization, SMTT(30)
           DEF RecoverDesiredBody, acquisitionVars, DeliveryVars
      <3> QED BY <1>1, <2>3, <3>1,
           AcquisitionRank3ActionStrictlyProgresses
    <2>4. CASE rank = 4
      BY <1>1, <2>4, AcquisitionRank4RecoveryStrictlyProgresses
    <2>5. CASE rank = 5
      BY <1>1, <2>5, AcquisitionRank5RecoveryPreservesProgressRank
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5,
         AcquisitionProgressRankCarrierCases
  <1> QED BY <1>1

THEOREM AcquisitionRetryDoesNotOrphanProgressRank ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects,
     rank \in AcquisitionProgressRankCarrier:
    /\ AcquisitionProgressAtRank(round, subject, rank)
    /\ RetryRecoveredBody
    => AcquisitionProgressExit(round, subject, rank)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                NEW rank \in AcquisitionProgressRankCarrier,
                AcquisitionProgressAtRank(round, subject, rank),
                RetryRecoveredBody
         PROVE AcquisitionProgressExit(round, subject, rank)'
    <2>1. CASE rank = 1
      BY <1>1, <2>1, AcquisitionRank1Characterization, SMTT(30)
         DEF RetryRecoveredBody
    <2>2. CASE rank = 2
      <3>1. <<RetryRecoveredBody>>_acquisitionVars
        BY <1>1, <2>2, AcquisitionRank2Characterization, Isa
           DEF RetryRecoveredBody, StartPhysicalLoad,
               acquisitionVars, DeliveryVars
      <3> QED BY <1>1, <2>2, <3>1,
           AcquisitionRank2ActionStrictlyProgresses
    <2>3. CASE rank = 3
      BY <1>1, <2>3, AcquisitionRank3Characterization, SMTT(30)
         DEF RetryRecoveredBody
    <2>4. CASE rank = 4
      BY <1>1, <2>4, AcquisitionRank4Characterization, SMTT(30)
         DEF RetryRecoveredBody
    <2>5. CASE rank = 5
      BY <1>1, <2>5, AcquisitionRank5Characterization, SMTT(30)
         DEF RetryRecoveredBody
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5,
         AcquisitionProgressRankCarrierCases
  <1> QED BY <1>1

THEOREM AcquisitionDeliveryCannotStartAtProgressRank ==
  \A round, subject, rank:
    /\ AcquisitionProgressAtRank(round, subject, rank)
    /\ DeliverReadyBody
    => FALSE
BY SMTT(30)
   DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
       DeliverReadyBody, DesiredLock, LockReady

THEOREM AcquisitionDecisionExitsProgressRank ==
  \A round, subject, rank:
    /\ AcquisitionProgressAtRank(round, subject, rank)
    /\ RecordDecision
    => AcquisitionProgressExit(round, subject, rank)'
PROOF
  <1>1. ASSUME NEW round, NEW subject, NEW rank,
                AcquisitionProgressAtRank(round, subject, rank),
                RecordDecision
         PROVE AcquisitionProgressExit(round, subject, rank)'
    <2>1. AcquisitionProgressGoal(round, subject)'
      BY <1>1, SMTT(30)
         DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
             RecordDecision, DesiredLock, LockReady,
             DeliveryVars
    <2> QED BY <2>1 DEF AcquisitionProgressExit
  <1> QED BY <1>1

THEOREM AcquisitionProgressActionIsNotOrphaned ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects,
     rank \in AcquisitionProgressRankCarrier:
    /\ AcquisitionProgressAtRank(round, subject, rank)
    /\ AcquisitionNext
    => \/ AcquisitionProgressAtRank(round, subject, rank)'
       \/ AcquisitionProgressExit(round, subject, rank)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                NEW rank \in AcquisitionProgressRankCarrier,
                AcquisitionProgressAtRank(round, subject, rank),
                AcquisitionNext
         PROVE \/ AcquisitionProgressAtRank(round, subject, rank)'
               \/ AcquisitionProgressExit(round, subject, rank)'
    <2>1. CASE \E nextView \in 0..MaxAcquisitionConsumerView,
                 nextGeneration \in 0..MaxAcquisitionGeneration:
                 RebindSameLock(nextView, nextGeneration)
      <3>1. PICK nextView \in 0..MaxAcquisitionConsumerView,
                    nextGeneration \in 0..MaxAcquisitionGeneration:
                    RebindSameLock(nextView, nextGeneration)
        BY <2>1
      <3> QED BY <1>1, <3>1,
           AcquisitionRebindPreservesProgressRank
    <2>2. CASE \E nextRound \in 0..MaxAcquisitionLockRound,
                 nextSubject \in AcquisitionSubjects,
                 nextView \in 0..MaxAcquisitionConsumerView,
                 nextGeneration \in 0..MaxAcquisitionGeneration:
                 InstallHigherLock(
                   nextRound, nextSubject, nextView, nextGeneration)
      <3>1. PICK nextRound \in 0..MaxAcquisitionLockRound,
                    nextSubject \in AcquisitionSubjects,
                    nextView \in 0..MaxAcquisitionConsumerView,
                    nextGeneration \in 0..MaxAcquisitionGeneration:
                    InstallHigherLock(
                      nextRound, nextSubject, nextView, nextGeneration)
        BY <2>2
      <3> QED BY <1>1, <3>1,
           AcquisitionHigherLockExitsProgressRank
    <2>3. CASE CompleteOwnedLoad
      BY <1>1, <2>3, AcquisitionCompleteDoesNotOrphanProgressRank
    <2>4. CASE RecoverDesiredBody
      BY <1>1, <2>4, AcquisitionRecoveryDoesNotOrphanProgressRank
    <2>5. CASE RetryRecoveredBody
      BY <1>1, <2>5, AcquisitionRetryDoesNotOrphanProgressRank
    <2>6. CASE DeliverReadyBody
      BY <1>1, <2>6, AcquisitionDeliveryCannotStartAtProgressRank
    <2>7. CASE RecordDecision
      BY <1>1, <2>7, AcquisitionDecisionExitsProgressRank
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4,
         <2>5, <2>6, <2>7 DEF AcquisitionNext
  <1> QED BY <1>1

THEOREM AcquisitionProgressStutterPreservesRank ==
  \A round, subject, rank:
    /\ AcquisitionProgressAtRank(round, subject, rank)
    /\ UNCHANGED acquisitionVars
    => AcquisitionProgressAtRank(round, subject, rank)'
PROOF
  <1>1. ASSUME NEW round, NEW subject, NEW rank,
                AcquisitionProgressAtRank(round, subject, rank),
                UNCHANGED acquisitionVars
         PROVE AcquisitionProgressAtRank(round, subject, rank)'
    <2>1. [AcquisitionNext]_acquisitionVars
      BY <1>1
    <2>2. AcquisitionTypeInvariant'
      BY <1>1, <2>1, AcquisitionStepPreservesTypeInvariant
         DEF AcquisitionProgressAtRank
    <2>3. AcquisitionReadBudgetInvariant'
      BY <1>1, <2>1, AcquisitionStepPreservesReadBudgetInvariant
         DEF AcquisitionProgressAtRank
    <2>4. /\ DesiredLock(round, subject)'
           /\ ~AcquisitionProgressGoal(round, subject)'
           /\ AcquisitionProgressRank' = rank
      BY <1>1, SMTT(30)
         DEF AcquisitionProgressAtRank, AcquisitionProgressGoal,
             AcquisitionProgressRank, DesiredLock, LockReady,
             acquisitionVars
    <2> QED BY <2>2, <2>3, <2>4
         DEF AcquisitionProgressAtRank
  <1> QED BY <1>1

THEOREM AcquisitionProgressRankIsNotOrphaned ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects,
     rank \in AcquisitionProgressRankCarrier:
    /\ AcquisitionProgressAtRank(round, subject, rank)
    /\ [AcquisitionNext]_acquisitionVars
    => \/ AcquisitionProgressAtRank(round, subject, rank)'
       \/ AcquisitionProgressExit(round, subject, rank)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                NEW rank \in AcquisitionProgressRankCarrier,
                AcquisitionProgressAtRank(round, subject, rank),
                [AcquisitionNext]_acquisitionVars
         PROVE \/ AcquisitionProgressAtRank(round, subject, rank)'
               \/ AcquisitionProgressExit(round, subject, rank)'
    <2>1. CASE AcquisitionNext
      BY <1>1, <2>1, AcquisitionProgressActionIsNotOrphaned
    <2>2. CASE UNCHANGED acquisitionVars
      BY <1>1, <2>2, AcquisitionProgressStutterPreservesRank
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
Each concrete rank is discharged by the exact weak-fair action that owns its
next local transition.  The bracketed reducer step may preserve the rank, but
cannot orphan it; continuous enabledness therefore forces a strict exit.
***************************************************************************)
THEOREM AcquisitionRank1LeadsToExit ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (AcquisitionProgressAtRank(round, subject, 1)
            ~> AcquisitionProgressExit(round, subject, 1))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (AcquisitionProgressAtRank(round, subject, 1)
                       ~> AcquisitionProgressExit(round, subject, 1))
    <2>1. /\ AcquisitionProgressAtRank(round, subject, 1)
             /\ [AcquisitionNext]_acquisitionVars
            => \/ AcquisitionProgressAtRank(round, subject, 1)'
               \/ AcquisitionProgressExit(round, subject, 1)'
      BY AcquisitionProgressRankIsNotOrphaned
    <2>2. AcquisitionProgressAtRank(round, subject, 1)
              => ENABLED <<CompleteOwnedLoad>>_acquisitionVars
      BY AcquisitionRank1ActionIsEnabled
    <2>3. /\ AcquisitionProgressAtRank(round, subject, 1)
             /\ <<CompleteOwnedLoad>>_acquisitionVars
            => AcquisitionProgressExit(round, subject, 1)'
      BY AcquisitionRank1ActionStrictlyProgresses
    <2>4. AcquisitionSpec
             => WF_acquisitionVars(CompleteOwnedLoad)
      BY DEF AcquisitionSpec
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF AcquisitionSpec
  <1> QED BY <1>1

THEOREM AcquisitionRank2LeadsToExit ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (AcquisitionProgressAtRank(round, subject, 2)
            ~> AcquisitionProgressExit(round, subject, 2))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (AcquisitionProgressAtRank(round, subject, 2)
                       ~> AcquisitionProgressExit(round, subject, 2))
    <2>1. /\ AcquisitionProgressAtRank(round, subject, 2)
             /\ [AcquisitionNext]_acquisitionVars
            => \/ AcquisitionProgressAtRank(round, subject, 2)'
               \/ AcquisitionProgressExit(round, subject, 2)'
      BY AcquisitionProgressRankIsNotOrphaned
    <2>2. AcquisitionProgressAtRank(round, subject, 2)
              => ENABLED <<RetryRecoveredBody>>_acquisitionVars
      BY AcquisitionRank2ActionIsEnabled
    <2>3. /\ AcquisitionProgressAtRank(round, subject, 2)
             /\ <<RetryRecoveredBody>>_acquisitionVars
            => AcquisitionProgressExit(round, subject, 2)'
      BY AcquisitionRank2ActionStrictlyProgresses
    <2>4. AcquisitionSpec
             => WF_acquisitionVars(RetryRecoveredBody)
      BY DEF AcquisitionSpec
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF AcquisitionSpec
  <1> QED BY <1>1

THEOREM AcquisitionRank3LeadsToExit ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (AcquisitionProgressAtRank(round, subject, 3)
            ~> AcquisitionProgressExit(round, subject, 3))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (AcquisitionProgressAtRank(round, subject, 3)
                       ~> AcquisitionProgressExit(round, subject, 3))
    <2>1. /\ AcquisitionProgressAtRank(round, subject, 3)
             /\ [AcquisitionNext]_acquisitionVars
            => \/ AcquisitionProgressAtRank(round, subject, 3)'
               \/ AcquisitionProgressExit(round, subject, 3)'
      BY AcquisitionProgressRankIsNotOrphaned
    <2>2. AcquisitionProgressAtRank(round, subject, 3)
              => ENABLED <<RecoverDesiredBody>>_acquisitionVars
      BY AcquisitionRank3ActionIsEnabled
    <2>3. /\ AcquisitionProgressAtRank(round, subject, 3)
             /\ <<RecoverDesiredBody>>_acquisitionVars
            => AcquisitionProgressExit(round, subject, 3)'
      BY AcquisitionRank3ActionStrictlyProgresses
    <2>4. AcquisitionSpec
             => WF_acquisitionVars(RecoverDesiredBody)
      BY DEF AcquisitionSpec
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF AcquisitionSpec
  <1> QED BY <1>1

THEOREM AcquisitionRank4LeadsToExit ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (AcquisitionProgressAtRank(round, subject, 4)
            ~> AcquisitionProgressExit(round, subject, 4))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (AcquisitionProgressAtRank(round, subject, 4)
                       ~> AcquisitionProgressExit(round, subject, 4))
    <2>1. /\ AcquisitionProgressAtRank(round, subject, 4)
             /\ [AcquisitionNext]_acquisitionVars
            => \/ AcquisitionProgressAtRank(round, subject, 4)'
               \/ AcquisitionProgressExit(round, subject, 4)'
      BY AcquisitionProgressRankIsNotOrphaned
    <2>2. AcquisitionProgressAtRank(round, subject, 4)
              => ENABLED <<CompleteOwnedLoad>>_acquisitionVars
      BY AcquisitionRank4ActionIsEnabled
    <2>3. /\ AcquisitionProgressAtRank(round, subject, 4)
             /\ <<CompleteOwnedLoad>>_acquisitionVars
            => AcquisitionProgressExit(round, subject, 4)'
      BY AcquisitionRank4ActionStrictlyProgresses
    <2>4. AcquisitionSpec
             => WF_acquisitionVars(CompleteOwnedLoad)
      BY DEF AcquisitionSpec
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF AcquisitionSpec
  <1> QED BY <1>1

THEOREM AcquisitionRank5LeadsToExit ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (AcquisitionProgressAtRank(round, subject, 5)
            ~> AcquisitionProgressExit(round, subject, 5))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (AcquisitionProgressAtRank(round, subject, 5)
                       ~> AcquisitionProgressExit(round, subject, 5))
    <2>1. /\ AcquisitionProgressAtRank(round, subject, 5)
             /\ [AcquisitionNext]_acquisitionVars
            => \/ AcquisitionProgressAtRank(round, subject, 5)'
               \/ AcquisitionProgressExit(round, subject, 5)'
      BY AcquisitionProgressRankIsNotOrphaned
    <2>2. AcquisitionProgressAtRank(round, subject, 5)
              => ENABLED <<CompleteOwnedLoad>>_acquisitionVars
      BY AcquisitionRank5ActionIsEnabled
    <2>3. /\ AcquisitionProgressAtRank(round, subject, 5)
             /\ <<CompleteOwnedLoad>>_acquisitionVars
            => AcquisitionProgressExit(round, subject, 5)'
      BY AcquisitionRank5ActionStrictlyProgresses
    <2>4. AcquisitionSpec
             => WF_acquisitionVars(CompleteOwnedLoad)
      BY DEF AcquisitionSpec
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF AcquisitionSpec
  <1> QED BY <1>1

THEOREM AcquisitionEveryRankLeadsToExit ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects,
     rank \in AcquisitionProgressRankCarrier:
    AcquisitionSpec
      => (AcquisitionProgressAtRank(round, subject, rank)
            ~> AcquisitionProgressExit(round, subject, rank))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                NEW rank \in AcquisitionProgressRankCarrier
         PROVE AcquisitionSpec
                 => (AcquisitionProgressAtRank(round, subject, rank)
                       ~> AcquisitionProgressExit(round, subject, rank))
    <2>1. CASE rank = 1
      BY <2>1, AcquisitionRank1LeadsToExit
    <2>2. CASE rank = 2
      BY <2>2, AcquisitionRank2LeadsToExit
    <2>3. CASE rank = 3
      BY <2>3, AcquisitionRank3LeadsToExit
    <2>4. CASE rank = 4
      BY <2>4, AcquisitionRank4LeadsToExit
    <2>5. CASE rank = 5
      BY <2>5, AcquisitionRank5LeadsToExit
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5,
         AcquisitionProgressRankCarrierCases
  <1> QED BY <1>1

THEOREM AcquisitionRank1ExitClassification ==
  \A round, subject:
    AcquisitionProgressExit(round, subject, 1)
      => AcquisitionProgressGoal(round, subject)
BY Isa
   DEF AcquisitionProgressExit, AcquisitionProgressOrdering,
       AcquisitionProgressRankCarrier, SetLessThan, OpToRel

THEOREM AcquisitionRank2ExitClassification ==
  \A round, subject:
    AcquisitionProgressExit(round, subject, 2)
      => \/ AcquisitionProgressGoal(round, subject)
         \/ AcquisitionProgressAtRank(round, subject, 1)
BY Isa
   DEF AcquisitionProgressExit, AcquisitionProgressOrdering,
       AcquisitionProgressRankCarrier, SetLessThan, OpToRel

THEOREM AcquisitionRank3ExitClassification ==
  \A round, subject:
    AcquisitionProgressExit(round, subject, 3)
      => \/ AcquisitionProgressGoal(round, subject)
         \/ AcquisitionProgressAtRank(round, subject, 1)
         \/ AcquisitionProgressAtRank(round, subject, 2)
BY Isa
   DEF AcquisitionProgressExit, AcquisitionProgressOrdering,
       AcquisitionProgressRankCarrier, SetLessThan, OpToRel

THEOREM AcquisitionRank4ExitClassification ==
  \A round, subject:
    AcquisitionProgressExit(round, subject, 4)
      => \/ AcquisitionProgressGoal(round, subject)
         \/ AcquisitionProgressAtRank(round, subject, 1)
         \/ AcquisitionProgressAtRank(round, subject, 2)
         \/ AcquisitionProgressAtRank(round, subject, 3)
BY Isa
   DEF AcquisitionProgressExit, AcquisitionProgressOrdering,
       AcquisitionProgressRankCarrier, SetLessThan, OpToRel

THEOREM AcquisitionRank5ExitClassification ==
  \A round, subject:
    AcquisitionProgressExit(round, subject, 5)
      => \/ AcquisitionProgressGoal(round, subject)
         \/ AcquisitionProgressAtRank(round, subject, 1)
         \/ AcquisitionProgressAtRank(round, subject, 2)
         \/ AcquisitionProgressAtRank(round, subject, 3)
         \/ AcquisitionProgressAtRank(round, subject, 4)
BY Isa
   DEF AcquisitionProgressExit, AcquisitionProgressOrdering,
       AcquisitionProgressRankCarrier, SetLessThan, OpToRel

THEOREM AcquisitionRank1LeadsToGoal ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (AcquisitionProgressAtRank(round, subject, 1)
            ~> AcquisitionProgressGoal(round, subject))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (AcquisitionProgressAtRank(round, subject, 1)
                       ~> AcquisitionProgressGoal(round, subject))
    <2>1. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 1)
                   ~> AcquisitionProgressExit(round, subject, 1))
      BY AcquisitionRank1LeadsToExit
    <2>2. AcquisitionProgressExit(round, subject, 1)
             => AcquisitionProgressGoal(round, subject)
      BY AcquisitionRank1ExitClassification
    <2>3. AcquisitionProgressExit(round, subject, 1)
             ~> AcquisitionProgressGoal(round, subject)
      BY <2>2, PTL
    <2> QED BY <2>1, <2>3, PTL
  <1> QED BY <1>1

THEOREM AcquisitionRank2LeadsToGoal ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (AcquisitionProgressAtRank(round, subject, 2)
            ~> AcquisitionProgressGoal(round, subject))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (AcquisitionProgressAtRank(round, subject, 2)
                       ~> AcquisitionProgressGoal(round, subject))
    <2>1. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 2)
                   ~> AcquisitionProgressExit(round, subject, 2))
      BY AcquisitionRank2LeadsToExit
    <2>2. AcquisitionProgressExit(round, subject, 2)
             => \/ AcquisitionProgressGoal(round, subject)
                \/ AcquisitionProgressAtRank(round, subject, 1)
      BY AcquisitionRank2ExitClassification
    <2>3. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 1)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank1LeadsToGoal
    <2>4. AcquisitionSpec
             => (AcquisitionProgressExit(round, subject, 2)
                   ~> AcquisitionProgressGoal(round, subject))
      BY <2>2, <2>3, PTL
    <2> QED BY <2>1, <2>4, PTL
  <1> QED BY <1>1

THEOREM AcquisitionRank3LeadsToGoal ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (AcquisitionProgressAtRank(round, subject, 3)
            ~> AcquisitionProgressGoal(round, subject))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (AcquisitionProgressAtRank(round, subject, 3)
                       ~> AcquisitionProgressGoal(round, subject))
    <2>1. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 3)
                   ~> AcquisitionProgressExit(round, subject, 3))
      BY AcquisitionRank3LeadsToExit
    <2>2. AcquisitionProgressExit(round, subject, 3)
             => \/ AcquisitionProgressGoal(round, subject)
                \/ AcquisitionProgressAtRank(round, subject, 1)
                \/ AcquisitionProgressAtRank(round, subject, 2)
      BY AcquisitionRank3ExitClassification
    <2>3. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 1)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank1LeadsToGoal
    <2>4. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 2)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank2LeadsToGoal
    <2>5. AcquisitionSpec
             => (AcquisitionProgressExit(round, subject, 3)
                   ~> AcquisitionProgressGoal(round, subject))
      BY <2>2, <2>3, <2>4, PTL
    <2> QED BY <2>1, <2>5, PTL
  <1> QED BY <1>1

THEOREM AcquisitionRank4LeadsToGoal ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (AcquisitionProgressAtRank(round, subject, 4)
            ~> AcquisitionProgressGoal(round, subject))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (AcquisitionProgressAtRank(round, subject, 4)
                       ~> AcquisitionProgressGoal(round, subject))
    <2>1. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 4)
                   ~> AcquisitionProgressExit(round, subject, 4))
      BY AcquisitionRank4LeadsToExit
    <2>2. AcquisitionProgressExit(round, subject, 4)
             => \/ AcquisitionProgressGoal(round, subject)
                \/ AcquisitionProgressAtRank(round, subject, 1)
                \/ AcquisitionProgressAtRank(round, subject, 2)
                \/ AcquisitionProgressAtRank(round, subject, 3)
      BY AcquisitionRank4ExitClassification
    <2>3. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 1)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank1LeadsToGoal
    <2>4. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 2)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank2LeadsToGoal
    <2>5. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 3)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank3LeadsToGoal
    <2>6. AcquisitionSpec
             => (AcquisitionProgressExit(round, subject, 4)
                   ~> AcquisitionProgressGoal(round, subject))
      BY <2>2, <2>3, <2>4, <2>5, PTL
    <2> QED BY <2>1, <2>6, PTL
  <1> QED BY <1>1

THEOREM AcquisitionRank5LeadsToGoal ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (AcquisitionProgressAtRank(round, subject, 5)
            ~> AcquisitionProgressGoal(round, subject))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (AcquisitionProgressAtRank(round, subject, 5)
                       ~> AcquisitionProgressGoal(round, subject))
    <2>1. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 5)
                   ~> AcquisitionProgressExit(round, subject, 5))
      BY AcquisitionRank5LeadsToExit
    <2>2. AcquisitionProgressExit(round, subject, 5)
             => \/ AcquisitionProgressGoal(round, subject)
                \/ AcquisitionProgressAtRank(round, subject, 1)
                \/ AcquisitionProgressAtRank(round, subject, 2)
                \/ AcquisitionProgressAtRank(round, subject, 3)
                \/ AcquisitionProgressAtRank(round, subject, 4)
      BY AcquisitionRank5ExitClassification
    <2>3. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 1)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank1LeadsToGoal
    <2>4. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 2)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank2LeadsToGoal
    <2>5. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 3)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank3LeadsToGoal
    <2>6. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 4)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank4LeadsToGoal
    <2>7. AcquisitionSpec
             => (AcquisitionProgressExit(round, subject, 5)
                   ~> AcquisitionProgressGoal(round, subject))
      BY <2>2, <2>3, <2>4, <2>5, <2>6, PTL
    <2> QED BY <2>1, <2>7, PTL
  <1> QED BY <1>1

THEOREM AcquisitionEveryRankLeadsToGoal ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => \A rank \in AcquisitionProgressRankCarrier:
           AcquisitionProgressAtRank(round, subject, rank)
             ~> AcquisitionProgressGoal(round, subject)
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => \A rank \in AcquisitionProgressRankCarrier:
                      AcquisitionProgressAtRank(round, subject, rank)
                        ~> AcquisitionProgressGoal(round, subject)
    <2>1. ASSUME NEW rank \in AcquisitionProgressRankCarrier
           PROVE AcquisitionSpec
                   => (AcquisitionProgressAtRank(round, subject, rank)
                         ~> AcquisitionProgressGoal(round, subject))
      <3>1. CASE rank = 1
        BY <3>1, AcquisitionRank1LeadsToGoal
      <3>2. CASE rank = 2
        BY <3>2, AcquisitionRank2LeadsToGoal
      <3>3. CASE rank = 3
        BY <3>3, AcquisitionRank3LeadsToGoal
      <3>4. CASE rank = 4
        BY <3>4, AcquisitionRank4LeadsToGoal
      <3>5. CASE rank = 5
        BY <3>5, AcquisitionRank5LeadsToGoal
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4, <3>5,
           AcquisitionProgressRankCarrierCases
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM AcquisitionPendingHasConcreteProgressRank ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    /\ AcquisitionTypeInvariant
    /\ AcquisitionReadBudgetInvariant
    /\ DesiredLock(round, subject)
    /\ ~AcquisitionProgressGoal(round, subject)
    => \/ AcquisitionProgressAtRank(round, subject, 1)
       \/ AcquisitionProgressAtRank(round, subject, 2)
       \/ AcquisitionProgressAtRank(round, subject, 3)
       \/ AcquisitionProgressAtRank(round, subject, 4)
       \/ AcquisitionProgressAtRank(round, subject, 5)
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionTypeInvariant,
                AcquisitionReadBudgetInvariant,
                DesiredLock(round, subject),
                ~AcquisitionProgressGoal(round, subject)
         PROVE \/ AcquisitionProgressAtRank(round, subject, 1)
               \/ AcquisitionProgressAtRank(round, subject, 2)
               \/ AcquisitionProgressAtRank(round, subject, 3)
               \/ AcquisitionProgressAtRank(round, subject, 4)
               \/ AcquisitionProgressAtRank(round, subject, 5)
    <2>1. AcquisitionProgressRank \in AcquisitionProgressRankCarrier
      BY <1>1, AcquisitionPendingHasRank
    <2>2. AcquisitionProgressAtRank(
             round, subject, AcquisitionProgressRank)
      BY <1>1 DEF AcquisitionProgressAtRank
    <2>3. \/ AcquisitionProgressRank = 1
           \/ AcquisitionProgressRank = 2
           \/ AcquisitionProgressRank = 3
           \/ AcquisitionProgressRank = 4
           \/ AcquisitionProgressRank = 5
      BY <2>1, AcquisitionProgressRankCarrierCases
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM AcquisitionDesiredLockLeadsToGoal ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (DesiredLock(round, subject)
            ~> AcquisitionProgressGoal(round, subject))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (DesiredLock(round, subject)
                       ~> AcquisitionProgressGoal(round, subject))
    <2>1. AcquisitionSpec => []AcquisitionTypeInvariant
      BY AcquisitionSpecAlwaysTypeInvariant
    <2>2. AcquisitionSpec => []AcquisitionReadBudgetInvariant
      BY AcquisitionSpecAlwaysReadBudgetInvariant
    <2>3. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 1)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank1LeadsToGoal
    <2>4. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 2)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank2LeadsToGoal
    <2>5. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 3)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank3LeadsToGoal
    <2>6. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 4)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank4LeadsToGoal
    <2>7. AcquisitionSpec
             => (AcquisitionProgressAtRank(round, subject, 5)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionRank5LeadsToGoal
    <2>8. /\ AcquisitionTypeInvariant
             /\ AcquisitionReadBudgetInvariant
             /\ DesiredLock(round, subject)
             /\ ~AcquisitionProgressGoal(round, subject)
            => \/ AcquisitionProgressAtRank(round, subject, 1)
               \/ AcquisitionProgressAtRank(round, subject, 2)
               \/ AcquisitionProgressAtRank(round, subject, 3)
               \/ AcquisitionProgressAtRank(round, subject, 4)
               \/ AcquisitionProgressAtRank(round, subject, 5)
      BY AcquisitionPendingHasConcreteProgressRank
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5,
         <2>6, <2>7, <2>8, PTL
  <1> QED BY <1>1

THEOREM AcquisitionSpecProvidesEffectiveLockProgress ==
  AcquisitionSpec => EffectiveLockAcquisitionProgress
BY AcquisitionDesiredLockLeadsToGoal
   DEF EffectiveLockAcquisitionProgress,
       AcquisitionProgressGoal

(***************************************************************************
Ready delivery is a second weak-fair obligation.  Same-lock consumer churn
may invalidate the current delivery tuple, but it preserves the exact Ready
body and therefore keeps DeliverReadyBody continuously enabled until the
latest consumer is served.
***************************************************************************)
AcquisitionDeliveryGoal(round, subject) ==
  \/ decided
  \/ desiredRound > round
  \/ /\ DesiredLock(round, subject)
     /\ CurrentConsumerDelivered

AcquisitionDeliveryPending(round, subject) ==
  /\ AcquisitionTypeInvariant
  /\ LockReady(round, subject)
  /\ ~AcquisitionDeliveryGoal(round, subject)

THEOREM AcquisitionRebindPreservesDeliveryPendingOrGoal ==
  \A round, subject, nextView, nextGeneration:
    /\ AcquisitionDeliveryPending(round, subject)
    /\ RebindSameLock(nextView, nextGeneration)
    => \/ AcquisitionDeliveryPending(round, subject)'
       \/ AcquisitionDeliveryGoal(round, subject)'
PROOF
  <1>1. ASSUME NEW round, NEW subject,
                NEW nextView, NEW nextGeneration,
                AcquisitionDeliveryPending(round, subject),
                RebindSameLock(nextView, nextGeneration)
         PROVE \/ AcquisitionDeliveryPending(round, subject)'
               \/ AcquisitionDeliveryGoal(round, subject)'
    <2>1. AcquisitionTypeInvariant'
      BY <1>1, RebindSameLockPreservesTypeInvariant
         DEF AcquisitionDeliveryPending
    <2> QED BY <1>1, <2>1, SMTT(30)
         DEF AcquisitionDeliveryPending, AcquisitionDeliveryGoal,
             RebindSameLock, LockReady, DesiredLock,
             CurrentConsumerDelivered, DeliveryVars
  <1> QED BY <1>1

THEOREM AcquisitionHigherLockReachesDeliveryGoal ==
  \A round, subject, nextRound, nextSubject, nextView, nextGeneration:
    /\ AcquisitionDeliveryPending(round, subject)
    /\ InstallHigherLock(
         nextRound, nextSubject, nextView, nextGeneration)
    => AcquisitionDeliveryGoal(round, subject)'
PROOF
  <1>1. ASSUME NEW round, NEW subject,
                NEW nextRound, NEW nextSubject,
                NEW nextView, NEW nextGeneration,
                AcquisitionDeliveryPending(round, subject),
                InstallHigherLock(
                  nextRound, nextSubject, nextView, nextGeneration)
         PROVE AcquisitionDeliveryGoal(round, subject)'
    <2>1. desiredRound = round
      BY <1>1
         DEF AcquisitionDeliveryPending, LockReady, DesiredLock
    <2>2. /\ nextRound \in (desiredRound + 1)..MaxAcquisitionLockRound
           /\ desiredRound' = nextRound
      BY <1>1 DEF InstallHigherLock
    <2>3. /\ desiredRound \in Nat
           /\ round \in Nat
           /\ nextRound \in Int
      BY <1>1, SMTT(30)
         DEF AcquisitionDeliveryPending, AcquisitionTypeInvariant,
             AcquisitionBaseTypeInvariant, AcquisitionConfiguration,
             LockReady, DesiredLock, InstallHigherLock
    <2>4. desiredRound' > round
      BY <2>1, <2>2, <2>3, SMTT(30)
    <2> QED BY <2>4 DEF AcquisitionDeliveryGoal
  <1> QED BY <1>1

THEOREM AcquisitionCompletionCannotStartWhileDeliveryPending ==
  \A round, subject:
    /\ AcquisitionDeliveryPending(round, subject)
    /\ CompleteOwnedLoad
    => FALSE
BY SMTT(30)
   DEF AcquisitionDeliveryPending, AcquisitionDeliveryGoal,
       CompleteOwnedLoad, CompleteAvailableLoad,
       CompleteUnavailableLoad, LockReady, DesiredLock

THEOREM AcquisitionRecoveryPreservesDeliveryPending ==
  \A round, subject:
    /\ AcquisitionDeliveryPending(round, subject)
    /\ RecoverDesiredBody
    => AcquisitionDeliveryPending(round, subject)'
PROOF
  <1>1. ASSUME NEW round, NEW subject,
                AcquisitionDeliveryPending(round, subject),
                RecoverDesiredBody
         PROVE AcquisitionDeliveryPending(round, subject)'
    <2>1. AcquisitionTypeInvariant'
      BY <1>1, RecoverDesiredBodyPreservesTypeInvariant
         DEF AcquisitionDeliveryPending
    <2> QED BY <1>1, <2>1, SMTT(30)
         DEF AcquisitionDeliveryPending, AcquisitionDeliveryGoal,
             RecoverDesiredBody, LockReady, DesiredLock,
             CurrentConsumerDelivered, DeliveryVars
  <1> QED BY <1>1

THEOREM AcquisitionRetryCannotStartWhileDeliveryPending ==
  \A round, subject:
    /\ AcquisitionDeliveryPending(round, subject)
    /\ RetryRecoveredBody
    => FALSE
BY SMTT(30)
   DEF AcquisitionDeliveryPending, AcquisitionDeliveryGoal,
       RetryRecoveredBody, LockReady, DesiredLock

THEOREM AcquisitionDeliveryActionReachesGoal ==
  \A round, subject:
    /\ AcquisitionDeliveryPending(round, subject)
    /\ DeliverReadyBody
    => AcquisitionDeliveryGoal(round, subject)'
BY SMTT(30)
   DEF AcquisitionDeliveryPending, AcquisitionDeliveryGoal,
       DeliverReadyBody, LockReady, DesiredLock,
       CurrentConsumerDelivered

THEOREM AcquisitionDecisionReachesDeliveryGoal ==
  \A round, subject:
    /\ AcquisitionDeliveryPending(round, subject)
    /\ RecordDecision
    => AcquisitionDeliveryGoal(round, subject)'
BY SMTT(30)
   DEF AcquisitionDeliveryPending, AcquisitionDeliveryGoal,
       RecordDecision, LockReady, DesiredLock, DeliveryVars

THEOREM AcquisitionDeliveryActionIsNotOrphaned ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    /\ AcquisitionDeliveryPending(round, subject)
    /\ AcquisitionNext
    => \/ AcquisitionDeliveryPending(round, subject)'
       \/ AcquisitionDeliveryGoal(round, subject)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionDeliveryPending(round, subject),
                AcquisitionNext
         PROVE \/ AcquisitionDeliveryPending(round, subject)'
               \/ AcquisitionDeliveryGoal(round, subject)'
    <2>1. CASE \E nextView \in 0..MaxAcquisitionConsumerView,
                 nextGeneration \in 0..MaxAcquisitionGeneration:
                 RebindSameLock(nextView, nextGeneration)
      <3>1. PICK nextView \in 0..MaxAcquisitionConsumerView,
                    nextGeneration \in 0..MaxAcquisitionGeneration:
                    RebindSameLock(nextView, nextGeneration)
        BY <2>1
      <3> QED BY <1>1, <3>1,
           AcquisitionRebindPreservesDeliveryPendingOrGoal
    <2>2. CASE \E nextRound \in 0..MaxAcquisitionLockRound,
                 nextSubject \in AcquisitionSubjects,
                 nextView \in 0..MaxAcquisitionConsumerView,
                 nextGeneration \in 0..MaxAcquisitionGeneration:
                 InstallHigherLock(
                   nextRound, nextSubject, nextView, nextGeneration)
      <3>1. PICK nextRound \in 0..MaxAcquisitionLockRound,
                    nextSubject \in AcquisitionSubjects,
                    nextView \in 0..MaxAcquisitionConsumerView,
                    nextGeneration \in 0..MaxAcquisitionGeneration:
                    InstallHigherLock(
                      nextRound, nextSubject, nextView, nextGeneration)
        BY <2>2
      <3> QED BY <1>1, <3>1,
           AcquisitionHigherLockReachesDeliveryGoal
    <2>3. CASE CompleteOwnedLoad
      BY <1>1, <2>3,
         AcquisitionCompletionCannotStartWhileDeliveryPending
    <2>4. CASE RecoverDesiredBody
      BY <1>1, <2>4, AcquisitionRecoveryPreservesDeliveryPending
    <2>5. CASE RetryRecoveredBody
      BY <1>1, <2>5,
         AcquisitionRetryCannotStartWhileDeliveryPending
    <2>6. CASE DeliverReadyBody
      BY <1>1, <2>6, AcquisitionDeliveryActionReachesGoal
    <2>7. CASE RecordDecision
      BY <1>1, <2>7, AcquisitionDecisionReachesDeliveryGoal
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4,
         <2>5, <2>6, <2>7 DEF AcquisitionNext
  <1> QED BY <1>1

THEOREM AcquisitionDeliveryStutterPreservesPending ==
  \A round, subject:
    /\ AcquisitionDeliveryPending(round, subject)
    /\ UNCHANGED acquisitionVars
    => AcquisitionDeliveryPending(round, subject)'
PROOF
  <1>1. ASSUME NEW round, NEW subject,
                AcquisitionDeliveryPending(round, subject),
                UNCHANGED acquisitionVars
         PROVE AcquisitionDeliveryPending(round, subject)'
    <2>1. [AcquisitionNext]_acquisitionVars
      BY <1>1
    <2>2. AcquisitionTypeInvariant'
      BY <1>1, <2>1, AcquisitionStepPreservesTypeInvariant
         DEF AcquisitionDeliveryPending
    <2>3. /\ LockReady(round, subject)'
           /\ ~AcquisitionDeliveryGoal(round, subject)'
      BY <1>1, SMTT(30)
         DEF AcquisitionDeliveryPending, AcquisitionDeliveryGoal,
             LockReady, DesiredLock, CurrentConsumerDelivered,
             acquisitionVars
    <2> QED BY <2>2, <2>3 DEF AcquisitionDeliveryPending
  <1> QED BY <1>1

THEOREM AcquisitionDeliveryPendingIsNotOrphaned ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    /\ AcquisitionDeliveryPending(round, subject)
    /\ [AcquisitionNext]_acquisitionVars
    => \/ AcquisitionDeliveryPending(round, subject)'
       \/ AcquisitionDeliveryGoal(round, subject)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionDeliveryPending(round, subject),
                [AcquisitionNext]_acquisitionVars
         PROVE \/ AcquisitionDeliveryPending(round, subject)'
               \/ AcquisitionDeliveryGoal(round, subject)'
    <2>1. CASE AcquisitionNext
      BY <1>1, <2>1, AcquisitionDeliveryActionIsNotOrphaned
    <2>2. CASE UNCHANGED acquisitionVars
      BY <1>1, <2>2, AcquisitionDeliveryStutterPreservesPending
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM AcquisitionDeliveryActionIsEnabled ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionDeliveryPending(round, subject)
      => ENABLED <<DeliverReadyBody>>_acquisitionVars
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionDeliveryPending(round, subject)
         PROVE ENABLED <<DeliverReadyBody>>_acquisitionVars
    <2>1. ENABLED DeliverReadyBody
      BY <1>1, ExpandENABLED, Isa
         DEF AcquisitionDeliveryPending, AcquisitionDeliveryGoal,
             DeliverReadyBody, LockReady, DesiredLock,
             CurrentConsumerDelivered
    <2>2. DeliverReadyBody \in BOOLEAN
      BY Isa DEF DeliverReadyBody
    <2>3. <<DeliverReadyBody>>_acquisitionVars \in BOOLEAN
      BY Isa
    <2>4. DeliverReadyBody
             => <<DeliverReadyBody>>_acquisitionVars
      BY <1>1, SMTT(30)
         DEF AcquisitionDeliveryPending, AcquisitionDeliveryGoal,
             DeliverReadyBody, LockReady, DesiredLock,
             CurrentConsumerDelivered, acquisitionVars
    <2>5. (ENABLED DeliverReadyBody)
             => ENABLED <<DeliverReadyBody>>_acquisitionVars
      BY <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>1, <2>5
  <1> QED BY <1>1

THEOREM AcquisitionDeliveryActionStrictlyProgresses ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    /\ AcquisitionDeliveryPending(round, subject)
    /\ <<DeliverReadyBody>>_acquisitionVars
    => AcquisitionDeliveryGoal(round, subject)'
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects,
                AcquisitionDeliveryPending(round, subject),
                <<DeliverReadyBody>>_acquisitionVars
         PROVE AcquisitionDeliveryGoal(round, subject)'
    <2>1. DeliverReadyBody
      BY <1>1
    <2> QED BY <1>1, <2>1, AcquisitionDeliveryActionReachesGoal
  <1> QED BY <1>1

THEOREM AcquisitionDeliveryPendingLeadsToGoal ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (AcquisitionDeliveryPending(round, subject)
            ~> AcquisitionDeliveryGoal(round, subject))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (AcquisitionDeliveryPending(round, subject)
                       ~> AcquisitionDeliveryGoal(round, subject))
    <2>1. /\ AcquisitionDeliveryPending(round, subject)
             /\ [AcquisitionNext]_acquisitionVars
            => \/ AcquisitionDeliveryPending(round, subject)'
               \/ AcquisitionDeliveryGoal(round, subject)'
      BY AcquisitionDeliveryPendingIsNotOrphaned
    <2>2. AcquisitionDeliveryPending(round, subject)
              => ENABLED <<DeliverReadyBody>>_acquisitionVars
      BY AcquisitionDeliveryActionIsEnabled
    <2>3. /\ AcquisitionDeliveryPending(round, subject)
             /\ <<DeliverReadyBody>>_acquisitionVars
            => AcquisitionDeliveryGoal(round, subject)'
      BY AcquisitionDeliveryActionStrictlyProgresses
    <2>4. AcquisitionSpec
             => WF_acquisitionVars(DeliverReadyBody)
      BY DEF AcquisitionSpec
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF AcquisitionSpec
  <1> QED BY <1>1

THEOREM AcquisitionReadyLockLeadsToDeliveryGoal ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (LockReady(round, subject)
            ~> AcquisitionDeliveryGoal(round, subject))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (LockReady(round, subject)
                       ~> AcquisitionDeliveryGoal(round, subject))
    <2>1. AcquisitionSpec => []AcquisitionTypeInvariant
      BY AcquisitionSpecAlwaysTypeInvariant
    <2>2. AcquisitionSpec
             => (AcquisitionDeliveryPending(round, subject)
                   ~> AcquisitionDeliveryGoal(round, subject))
      BY AcquisitionDeliveryPendingLeadsToGoal
    <2>3. /\ AcquisitionTypeInvariant
             /\ LockReady(round, subject)
             /\ ~AcquisitionDeliveryGoal(round, subject)
            => AcquisitionDeliveryPending(round, subject)
      BY DEF AcquisitionDeliveryPending
    <2> QED BY <2>1, <2>2, <2>3, PTL
  <1> QED BY <1>1

THEOREM AcquisitionProgressGoalLeadsToDeliveryGoal ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (AcquisitionProgressGoal(round, subject)
            ~> AcquisitionDeliveryGoal(round, subject))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (AcquisitionProgressGoal(round, subject)
                       ~> AcquisitionDeliveryGoal(round, subject))
    <2>1. AcquisitionProgressGoal(round, subject)
             => \/ AcquisitionDeliveryGoal(round, subject)
                \/ LockReady(round, subject)
      BY DEF AcquisitionProgressGoal, AcquisitionDeliveryGoal
    <2>2. AcquisitionSpec
             => (LockReady(round, subject)
                   ~> AcquisitionDeliveryGoal(round, subject))
      BY AcquisitionReadyLockLeadsToDeliveryGoal
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM AcquisitionDesiredLockLeadsToDeliveryGoal ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => (DesiredLock(round, subject)
            ~> AcquisitionDeliveryGoal(round, subject))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => (DesiredLock(round, subject)
                       ~> AcquisitionDeliveryGoal(round, subject))
    <2>1. AcquisitionSpec
             => (DesiredLock(round, subject)
                   ~> AcquisitionProgressGoal(round, subject))
      BY AcquisitionDesiredLockLeadsToGoal
    <2>2. AcquisitionSpec
             => (AcquisitionProgressGoal(round, subject)
                   ~> AcquisitionDeliveryGoal(round, subject))
      BY AcquisitionProgressGoalLeadsToDeliveryGoal
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM AcquisitionStableLockForcesRepeatedDelivery ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    AcquisitionSpec
      => ((<>[](DesiredLock(round, subject) /\ ~decided))
            => []<>(decided
                     \/ /\ DesiredLock(round, subject)
                        /\ CurrentConsumerDelivered))
PROOF
  <1>1. ASSUME NEW round \in 0..MaxAcquisitionLockRound,
                NEW subject \in AcquisitionSubjects
         PROVE AcquisitionSpec
                 => ((<>[](DesiredLock(round, subject) /\ ~decided))
                       => []<>(decided
                                \/ /\ DesiredLock(round, subject)
                                   /\ CurrentConsumerDelivered))
    <2>1. AcquisitionSpec
             => (DesiredLock(round, subject)
                   ~> AcquisitionDeliveryGoal(round, subject))
      BY AcquisitionDesiredLockLeadsToDeliveryGoal
    <2>2. /\ DesiredLock(round, subject)
             /\ ~decided
             /\ AcquisitionDeliveryGoal(round, subject)
            => /\ DesiredLock(round, subject)
               /\ CurrentConsumerDelivered
      BY SMTT(30)
         DEF AcquisitionDeliveryGoal, DesiredLock
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM AcquisitionSpecProvidesStableEffectiveLockDelivery ==
  AcquisitionSpec => StableEffectiveLockDelivery
BY AcquisitionStableLockForcesRepeatedDelivery
   DEF StableEffectiveLockDelivery

THEOREM EffectiveLockAcquisitionModelObligation ==
  AcquisitionSpec
    => /\ []AcquisitionTypeInvariant
       /\ EffectiveLockAcquisitionProgress
       /\ StableEffectiveLockDelivery
PROOF
  <1>1. AcquisitionSpec => []AcquisitionTypeInvariant
    BY AcquisitionSpecAlwaysTypeInvariant
  <1>2. AcquisitionSpec => EffectiveLockAcquisitionProgress
    BY AcquisitionSpecProvidesEffectiveLockProgress
  <1>3. AcquisitionSpec => StableEffectiveLockDelivery
    BY AcquisitionSpecProvidesStableEffectiveLockDelivery
  <1> QED BY <1>1, <1>2, <1>3

=============================================================================
