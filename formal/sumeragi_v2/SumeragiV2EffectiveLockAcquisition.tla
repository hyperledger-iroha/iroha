---- MODULE SumeragiV2EffectiveLockAcquisition ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
Executable state machine for production's height-scoped locked-body owner.

The desired lock identity is `(desiredRound, desiredSubject)`.  The reducer
consumer `(consumerView, consumerGeneration)` may advance independently.  A
physical read has a monotonically allocated ID and immutable subject; a
same-subject consumer rebind never allocates another read.  If a higher lock
changes subject while a read is active, that read is allowed to terminate and
then starts exactly one replacement.  Local absence waits for the exact
certified recovery store before retrying.  Ready bytes may be delivered once
to every latest consumer without another physical read.

This model deliberately stops at the worker/reducer acquisition boundary.
The separate production-refinement obligation must still prove that ordinary
Rust maps, hashes, queues, callbacks, and fair run-loop invocation implement
these transitions.
***************************************************************************)

CONSTANTS AcquisitionSubjects,
          InitialAcquisitionSubject,
          MaxAcquisitionLockRound,
          MaxAcquisitionConsumerView,
          MaxAcquisitionGeneration,
          MaxAcquisitionId

AcquisitionPhases == {"Loading", "Waiting", "Ready"}

AcquisitionConfiguration ==
  /\ IsFiniteSet(AcquisitionSubjects)
  /\ AcquisitionSubjects # {}
  /\ InitialAcquisitionSubject \in AcquisitionSubjects
  /\ MaxAcquisitionLockRound \in Nat \ {0}
  /\ MaxAcquisitionConsumerView \in Nat \ {0}
  /\ MaxAcquisitionGeneration \in Nat \ {0}
  /\ MaxAcquisitionLockRound <= MaxAcquisitionConsumerView
  /\ MaxAcquisitionId \in Nat
  /\ MaxAcquisitionId >= 2 * MaxAcquisitionLockRound + 2

AcquisitionLoad(id, subject) == [id |-> id, subject |-> subject]

AcquisitionLoadSet ==
  [id: 0..MaxAcquisitionId, subject: AcquisitionSubjects]

VARIABLES desiredRound,
          desiredSubject,
          consumerView,
          consumerGeneration,
          physicalId,
          nextPhysicalId,
          physicalSubject,
          acquisitionPhase,
          issuedLoads,
          durableSubjects,
          deliveryValid,
          deliveredRound,
          deliveredSubject,
          deliveredView,
          deliveredGeneration,
          decided

acquisitionVars ==
  <<desiredRound, desiredSubject, consumerView, consumerGeneration,
    physicalId, nextPhysicalId, physicalSubject, acquisitionPhase,
    issuedLoads, durableSubjects, deliveryValid, deliveredRound,
    deliveredSubject, deliveredView, deliveredGeneration, decided>>

DeliveryVars ==
  <<deliveryValid, deliveredRound, deliveredSubject, deliveredView,
    deliveredGeneration>>

DesiredLock(round, subject) ==
  /\ desiredRound = round
  /\ desiredSubject = subject

CurrentConsumerDelivered ==
  /\ deliveryValid
  /\ deliveredRound = desiredRound
  /\ deliveredSubject = desiredSubject
  /\ deliveredView = consumerView
  /\ deliveredGeneration = consumerGeneration

LockReady(round, subject) ==
  /\ DesiredLock(round, subject)
  /\ acquisitionPhase = "Ready"
  /\ physicalSubject = subject

PhysicalLoadIds == {load.id: load \in issuedLoads}

ExactAcquisitionIdentityInvariant ==
  /\ PhysicalLoadIds = 0..(nextPhysicalId - 1)
  /\ \A left, right \in issuedLoads:
       left.id = right.id => left.subject = right.subject
  /\ AcquisitionLoad(physicalId, physicalSubject) \in issuedLoads
  /\ physicalId < nextPhysicalId
  /\ physicalSubject # desiredSubject => acquisitionPhase = "Loading"
  /\ acquisitionPhase \in {"Waiting", "Ready"} =>
       physicalSubject = desiredSubject

(***************************************************************************
The completion classifier mirrors `LockedCandidateAcquisition::physical_owner`.
Older IDs are harmless stale completions.  An unissued future ID, a second
completion of a terminal owner, or an equal-ID wrong subject fails closed.  A
terminating superseded load can only request the latest desired replacement.
***************************************************************************)
PhysicalCompletionDisposition(completionId, completionSubject) ==
  IF completionId < physicalId
  THEN "Stale"
  ELSE IF completionId > physicalId
       THEN "FailClosed"
       ELSE IF acquisitionPhase # "Loading"
            THEN "FailClosed"
            ELSE IF completionSubject # physicalSubject
                 THEN "FailClosed"
                 ELSE IF physicalSubject # desiredSubject
                      THEN "Replace"
                      ELSE "Owned"

CompletionClassificationInvariant ==
  /\ \A completionId \in (physicalId + 1)..MaxAcquisitionId,
         completionSubject \in AcquisitionSubjects:
       PhysicalCompletionDisposition(completionId, completionSubject)
         = "FailClosed"
  /\ \A completionId \in 0..(physicalId - 1),
         completionSubject \in AcquisitionSubjects:
       PhysicalCompletionDisposition(completionId, completionSubject) = "Stale"
  /\ acquisitionPhase # "Loading" =>
       \A completionSubject \in AcquisitionSubjects:
         PhysicalCompletionDisposition(physicalId, completionSubject)
           = "FailClosed"
  /\ acquisitionPhase = "Loading" =>
       /\ \A completionSubject \in AcquisitionSubjects \ {physicalSubject}:
            PhysicalCompletionDisposition(physicalId, completionSubject)
              = "FailClosed"
       /\ PhysicalCompletionDisposition(physicalId, physicalSubject)
            = IF physicalSubject = desiredSubject THEN "Owned" ELSE "Replace"

AcquisitionTypeInvariant ==
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
  /\ ExactAcquisitionIdentityInvariant
  /\ CompletionClassificationInvariant

AcquisitionInit ==
  /\ AcquisitionConfiguration
  /\ desiredRound = 0
  /\ desiredSubject = InitialAcquisitionSubject
  /\ consumerView = 0
  /\ consumerGeneration = 0
  /\ physicalId = 0
  /\ nextPhysicalId = 1
  /\ physicalSubject = InitialAcquisitionSubject
  /\ acquisitionPhase = "Loading"
  /\ issuedLoads = {AcquisitionLoad(0, InitialAcquisitionSubject)}
  /\ durableSubjects = {}
  /\ deliveryValid = FALSE
  /\ deliveredRound = 0
  /\ deliveredSubject = InitialAcquisitionSubject
  /\ deliveredView = 0
  /\ deliveredGeneration = 0
  /\ decided = FALSE

StartPhysicalLoad(subject) ==
  /\ nextPhysicalId <= MaxAcquisitionId
  /\ physicalId' = nextPhysicalId
  /\ nextPhysicalId' = nextPhysicalId + 1
  /\ physicalSubject' = subject
  /\ acquisitionPhase' = "Loading"
  /\ issuedLoads' = issuedLoads \cup {AcquisitionLoad(nextPhysicalId, subject)}

(***************************************************************************
Certified view/generation churn changes only the consumer.  In particular it
does not retag the immutable physical identity or allocate another load.
***************************************************************************)
RebindSameLock(nextView, nextGeneration) ==
  /\ ~decided
  /\ nextView \in (consumerView + 1)..MaxAcquisitionConsumerView
  /\ nextGeneration \in
       (consumerGeneration + 1)..MaxAcquisitionGeneration
  /\ consumerView' = nextView
  /\ consumerGeneration' = nextGeneration
  /\ UNCHANGED <<desiredRound, desiredSubject, physicalId,
                 nextPhysicalId, physicalSubject, acquisitionPhase,
                 issuedLoads, durableSubjects, DeliveryVars, decided>>

InstallHigherLock(nextRound, nextSubject, nextView, nextGeneration) ==
  /\ ~decided
  /\ nextRound \in (desiredRound + 1)..MaxAcquisitionLockRound
  /\ nextSubject \in AcquisitionSubjects
  /\ nextView \in consumerView..MaxAcquisitionConsumerView
  /\ nextGeneration \in consumerGeneration..MaxAcquisitionGeneration
  /\ nextRound <= nextView
  /\ \/ /\ nextView = consumerView
          /\ nextGeneration = consumerGeneration
     \/ /\ nextView > consumerView
          /\ nextGeneration > consumerGeneration
  /\ (nextSubject = desiredSubject \/ acquisitionPhase = "Loading"
        \/ nextPhysicalId <= MaxAcquisitionId)
  /\ desiredRound' = nextRound
  /\ desiredSubject' = nextSubject
  /\ consumerView' = nextView
  /\ consumerGeneration' = nextGeneration
  /\ IF nextSubject = desiredSubject \/ acquisitionPhase = "Loading"
     THEN UNCHANGED <<physicalId, nextPhysicalId, physicalSubject,
                      acquisitionPhase, issuedLoads>>
     ELSE StartPhysicalLoad(nextSubject)
  /\ UNCHANGED <<durableSubjects, DeliveryVars, decided>>

(***************************************************************************
Terminating local work reports either durable bytes or local absence.  A
superseded physical result never becomes Ready: its termination allocates the
one load for the latest desired subject.  An unavailable desired subject waits
without spinning until certified recovery stores that exact subject.
***************************************************************************)
CompleteAvailableLoad ==
  /\ ~decided
  /\ acquisitionPhase = "Loading"
  /\ physicalSubject \in durableSubjects
  /\ (physicalSubject = desiredSubject
        \/ nextPhysicalId <= MaxAcquisitionId)
  /\ IF physicalSubject = desiredSubject
     THEN /\ acquisitionPhase' = "Ready"
          /\ UNCHANGED <<physicalId, nextPhysicalId, physicalSubject,
                         issuedLoads>>
     ELSE StartPhysicalLoad(desiredSubject)
  /\ UNCHANGED <<desiredRound, desiredSubject, consumerView,
                 consumerGeneration, durableSubjects, DeliveryVars, decided>>

CompleteUnavailableLoad ==
  /\ ~decided
  /\ acquisitionPhase = "Loading"
  /\ physicalSubject \notin durableSubjects
  /\ (physicalSubject = desiredSubject
        \/ nextPhysicalId <= MaxAcquisitionId)
  /\ IF physicalSubject = desiredSubject
     THEN /\ acquisitionPhase' = "Waiting"
          /\ UNCHANGED <<physicalId, nextPhysicalId, physicalSubject,
                         issuedLoads>>
     ELSE StartPhysicalLoad(desiredSubject)
  /\ UNCHANGED <<desiredRound, desiredSubject, consumerView,
                 consumerGeneration, durableSubjects, DeliveryVars, decided>>

CompleteOwnedLoad == CompleteAvailableLoad \/ CompleteUnavailableLoad

RecoverDesiredBody ==
  /\ ~decided
  /\ desiredSubject \notin durableSubjects
  /\ durableSubjects' = durableSubjects \cup {desiredSubject}
  /\ UNCHANGED <<desiredRound, desiredSubject, consumerView,
                 consumerGeneration, physicalId, nextPhysicalId,
                 physicalSubject, acquisitionPhase, issuedLoads,
                 DeliveryVars, decided>>

RetryRecoveredBody ==
  /\ ~decided
  /\ acquisitionPhase = "Waiting"
  /\ physicalSubject = desiredSubject
  /\ desiredSubject \in durableSubjects
  /\ StartPhysicalLoad(desiredSubject)
  /\ UNCHANGED <<desiredRound, desiredSubject, consumerView,
                 consumerGeneration, durableSubjects, DeliveryVars, decided>>

DeliverReadyBody ==
  /\ ~decided
  /\ acquisitionPhase = "Ready"
  /\ physicalSubject = desiredSubject
  /\ ~CurrentConsumerDelivered
  /\ deliveryValid' = TRUE
  /\ deliveredRound' = desiredRound
  /\ deliveredSubject' = desiredSubject
  /\ deliveredView' = consumerView
  /\ deliveredGeneration' = consumerGeneration
  /\ UNCHANGED <<desiredRound, desiredSubject, consumerView,
                 consumerGeneration, physicalId, nextPhysicalId,
                 physicalSubject, acquisitionPhase, issuedLoads,
                 durableSubjects, decided>>

RecordDecision ==
  /\ ~decided
  /\ decided' = TRUE
  /\ UNCHANGED <<desiredRound, desiredSubject, consumerView,
                 consumerGeneration, physicalId, nextPhysicalId,
                 physicalSubject, acquisitionPhase, issuedLoads,
                 durableSubjects, DeliveryVars>>

AcquisitionNext ==
  \/ \E nextView \in 0..MaxAcquisitionConsumerView,
         nextGeneration \in 0..MaxAcquisitionGeneration:
       RebindSameLock(nextView, nextGeneration)
  \/ \E nextRound \in 0..MaxAcquisitionLockRound,
         nextSubject \in AcquisitionSubjects,
         nextView \in 0..MaxAcquisitionConsumerView,
         nextGeneration \in 0..MaxAcquisitionGeneration:
       InstallHigherLock(nextRound, nextSubject, nextView, nextGeneration)
  \/ CompleteOwnedLoad
  \/ RecoverDesiredBody
  \/ RetryRecoveredBody
  \/ DeliverReadyBody
  \/ RecordDecision

AcquisitionSpec ==
  AcquisitionInit
    /\ [][AcquisitionNext]_acquisitionVars
    /\ WF_acquisitionVars(CompleteOwnedLoad)
    /\ WF_acquisitionVars(RecoverDesiredBody)
    /\ WF_acquisitionVars(RetryRecoveredBody)
    /\ WF_acquisitionVars(DeliverReadyBody)

(***************************************************************************
Every exact desired lock either becomes Ready, is superseded by a strictly
higher lock, or is overtaken by Decision.  Once a lock remains stable, the
latest consumer is delivered infinitely often: same-lock view changes cannot
turn the immutable bytes into a terminal stale completion.
***************************************************************************)
EffectiveLockAcquisitionProgress ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    DesiredLock(round, subject)
      ~> (decided \/ desiredRound > round \/ LockReady(round, subject))

StableEffectiveLockDelivery ==
  \A round \in 0..MaxAcquisitionLockRound,
     subject \in AcquisitionSubjects:
    (<>[](DesiredLock(round, subject) /\ ~decided))
      => []<>(decided
               \/ /\ DesiredLock(round, subject)
                  /\ CurrentConsumerDelivered)

=============================================================================
