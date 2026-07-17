---- MODULE SumeragiV2EffectiveLockAcquisitionProofs ----
EXTENDS SumeragiV2EffectiveLockAcquisition, FiniteSetTheorems, TLAPS

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
    <2>7. /\ physicalSubject' # desiredSubject'
                 => acquisitionPhase' = "Loading"
           /\ acquisitionPhase' \in {"Waiting", "Ready"}
                 => physicalSubject' = desiredSubject'
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

THEOREM EffectiveLockAcquisitionModelObligation ==
  AcquisitionSpec
    => /\ []AcquisitionTypeInvariant
       /\ EffectiveLockAcquisitionProgress
       /\ StableEffectiveLockDelivery

=============================================================================
