---- MODULE SumeragiV2TypedRolloverHandoffLivenessMutation ----
EXTENDS SumeragiV2TypedRolloverHandoff

(***************************************************************************
Focused bounded negative controls for the two responsive local corridors.

Each specification preserves the production initial condition, transition
relation, and every other weak-fairness conjunct.  The durable-output mutant
removes only worker-clear fairness, so the closed worker can retain an
outstanding exact output forever.  The restart mutant removes only validated
cleanup fairness, so validated and parent-synchronized crash artifacts can
remain forever.  TLC counterexamples for these finite controls are diagnostic
mutation evidence; they are not deductive proof of the production properties.
***************************************************************************)

MissingWorkerClearFairnessSpec ==
  /\ ResponsiveDurableExactOutputInit
  /\ [][ResponsiveDurableExactOutputNext]_typedRolloverVars
  /\ WF_typedRolloverVars(CreateServiceTransportOwnerPair)
  /\ WF_typedRolloverVars(ValidateFinality)
  /\ WF_typedRolloverVars(CloseWorkerIngress)
  /\ WF_typedRolloverVars(BuildImmediateSuccessor)
  /\ WF_typedRolloverVars(SealAppliedHeightOutputHandoff)
  /\ WF_typedRolloverVars(RetainExactHandoffReceipt)
  /\ WF_typedRolloverVars(
       PublishDurableExactOutputSuccessorLifecycleStateSlotV3)
  /\ WF_typedRolloverVars(
       SyncSuccessorLifecycleStateDirectoryV3)
  /\ WF_typedRolloverVars(ReplaceSuccessorLifecycleRootV3)
  /\ WF_typedRolloverVars(CommitSuccessorLifecycleRootV3)
  /\ WF_typedRolloverVars(PublishCommittedLifecycleV3ToMemory)

MissingWorkerClearFairnessLiveness ==
  state.finalityValidated
    ~> DurableExactOutputSuccessorActiveWithoutRestart

MissingValidatedCleanupFairnessSpec ==
  /\ ResponsiveRestartRestoreInit
  /\ [][ResponsiveRestartRestoreNext]_typedRolloverVars
  /\ WF_typedRolloverVars(ValidateRootSelectedLifecycleV3)
  /\ WF_typedRolloverVars(
       ResyncValidatedLifecycleStateDirectoryV3)
  /\ WF_typedRolloverVars(
       ResyncValidatedLifecycleRootDirectoryV3)
  /\ WF_typedRolloverVars(RecoverPredecessorLifecycleV3)
  /\ WF_typedRolloverVars(
       PublishRestartRestoreSuccessorLifecycleStateSlotV3)
  /\ WF_typedRolloverVars(
       SyncSuccessorLifecycleStateDirectoryV3)
  /\ WF_typedRolloverVars(ReplaceSuccessorLifecycleRootV3)
  /\ WF_typedRolloverVars(CommitSuccessorLifecycleRootV3)
  /\ WF_typedRolloverVars(PublishCommittedLifecycleV3ToMemory)

MissingValidatedCleanupFairnessLiveness ==
  state.restartRequired ~> RestartRestoreSuccessorActiveWithoutRestart

=============================================================================
