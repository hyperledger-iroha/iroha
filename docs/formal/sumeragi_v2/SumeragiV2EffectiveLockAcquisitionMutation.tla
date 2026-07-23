---- MODULE SumeragiV2EffectiveLockAcquisitionMutation ----
EXTENDS SumeragiV2EffectiveLockAcquisition

(***************************************************************************
Targeted mutations for the two production stall boundaries.

`BuggyRebindSameLock` gives every consumer view a new physical ID, recreating
view-indexed disk work.  `NoRetrySpec` reaches Waiting, records the exact
durable recovery, and then omits the required retry.  The fixed rebind-only
spec and the complete acquisition spec provide the paired positive controls.
***************************************************************************)

BuggyRebindSameLock(nextView, nextGeneration) ==
  /\ ~decided
  /\ nextView \in (consumerView + 1)..MaxAcquisitionConsumerView
  /\ nextGeneration \in
       (consumerGeneration + 1)..MaxAcquisitionGeneration
  /\ nextPhysicalId <= MaxAcquisitionId
  /\ consumerView' = nextView
  /\ consumerGeneration' = nextGeneration
  /\ StartPhysicalLoad(desiredSubject)
  /\ UNCHANGED <<desiredRound, desiredSubject, durableSubjects,
                 DeliveryVars, decided>>

CorrectRebindOnlyNext ==
  \E nextView \in 0..MaxAcquisitionConsumerView,
     nextGeneration \in 0..MaxAcquisitionGeneration:
    RebindSameLock(nextView, nextGeneration)

BuggyRebindOnlyNext ==
  \E nextView \in 0..MaxAcquisitionConsumerView,
     nextGeneration \in 0..MaxAcquisitionGeneration:
    BuggyRebindSameLock(nextView, nextGeneration)

CorrectRebindOnlySpec ==
  AcquisitionInit /\ [][CorrectRebindOnlyNext]_acquisitionVars

BuggyRebindOnlySpec ==
  AcquisitionInit /\ [][BuggyRebindOnlyNext]_acquisitionVars

ViewRebindKeepsOnePhysicalLoad ==
  /\ physicalId = 0
  /\ nextPhysicalId = 1
  /\ physicalSubject = InitialAcquisitionSubject
  /\ issuedLoads = {AcquisitionLoad(0, InitialAcquisitionSubject)}

NoRetryNext == CompleteOwnedLoad \/ RecoverDesiredBody

NoRetrySpec ==
  AcquisitionInit
    /\ [][NoRetryNext]_acquisitionVars
    /\ WF_acquisitionVars(CompleteOwnedLoad)
    /\ WF_acquisitionVars(RecoverDesiredBody)

(***************************************************************************
An adversarial classifier mutation accepts an unissued future completion as
stale.  The invariant below is deliberately phrased against the mutant, so
the negative configuration must emit the shortest counterexample at Init.
***************************************************************************)
BuggyPhysicalCompletionDisposition(completionId, completionSubject) ==
  IF completionId # physicalId
  THEN "Stale"
  ELSE IF acquisitionPhase # "Loading"
       THEN "FailClosed"
       ELSE IF completionSubject # physicalSubject
            THEN "FailClosed"
            ELSE IF physicalSubject # desiredSubject
                 THEN "Replace"
                 ELSE "Owned"

BuggyFutureCompletionFailsClosed ==
  \A completionId \in (physicalId + 1)..MaxAcquisitionId,
     completionSubject \in AcquisitionSubjects:
    BuggyPhysicalCompletionDisposition(completionId, completionSubject)
      = "FailClosed"

=============================================================================
