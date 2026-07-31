---- MODULE SumeragiV2TypedRolloverHandoffRepeatedHandoffMutation ----
EXTENDS SumeragiV2TypedRolloverHandoff

(***************************************************************************
Focused negative control for the one-shot predecessor service/transport owner
pair.

The production model binds pair creation and its atomic seal to
`PredecessorTransportOwnershipOpen`.  This mutant follows the real
root-committed crash corridor through validated RestartRestore, then bypasses
that gate twice: it reopens a fresh pair and reseals it while the restored
successor authority already owns the lifecycle.  The reseal creates a Minted
predecessor receipt under RestartRestore and must violate
`UnconsumedPredecessorTransportOwnershipInvariant` (as well as the
RestartRestore receipt-stage arm of the authority invariant).
***************************************************************************)

RepeatedHandoffMutationInit ==
  /\ Init
  /\ state.startupMode = "LiveProcess"
  /\ ChangedRosterReplacementNeeded
  /\ state.serviceGeneration < ServiceGenerationLimit
  /\ state.durableLifecycleRootV3.rootGeneration <
       RootGenerationLimit

ReopenPredecessorTransportAfterRestartRestore ==
  /\ state.lifecycleCommitPhase = "Restored"
  /\ state.transitionAuthority = "RestartRestore"
  /\ state.serviceOwnerNonce = NoIdentity
  /\ state.transportOwnerNonce = NoIdentity
  /\ state.receiptStage \in {"Absent", "Lost"}
  /\ ~state.restartRequired
  /\ ~state.successorActive
  /\ state' =
       [state EXCEPT
          !.serviceOwnerNonce = "OwnerNonce",
          !.transportOwnerNonce = "OwnerNonce",
          !.receiptStage = "Absent",
          !.restartFenceAuthorized = FALSE]

ResealPredecessorTransportAfterRestartRestore ==
  /\ state.lifecycleCommitPhase = "Restored"
  /\ state.transitionAuthority = "RestartRestore"
  /\ state.finalityValidated
  /\ state.workerIngressClosed
  /\ state.workerOutstanding = 0
  /\ ~state.ownerSealed
  /\ state.receiptStage = "Absent"
  /\ ExactServiceTransportOwnerPair
  /\ ~state.successorActive
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.ownerSealed = TRUE,
          !.receiptStage = "Minted",
          !.receiptOwnerNonce = state.serviceOwnerNonce,
          !.receiptContext = ExpectedContext,
          !.receiptArtifact = ExpectedArtifact]

RepeatedHandoffMutationNext ==
  \/ CreateServiceTransportOwnerPair
  \/ ValidateFinality
  \/ CloseWorkerIngress
  \/ ClearOneWorkerExactOutput
  \/ BuildImmediateSuccessor
  \/ SealAppliedHeightOutputHandoff
  \/ RetainExactHandoffReceipt
  \/ PublishDurableExactOutputSuccessorLifecycleStateSlotV3
  \/ SyncSuccessorLifecycleStateDirectoryV3
  \/ ReplaceSuccessorLifecycleRootV3
  \/ CommitSuccessorLifecycleRootV3
  \/ CrashAfterLifecycleRootV3Commit
  \/ ValidateRootSelectedLifecycleV3
  \/ ResyncValidatedLifecycleStateDirectoryV3
  \/ ResyncValidatedLifecycleRootDirectoryV3
  \/ CleanupValidatedLifecycleArtifactsV3
  \/ RestoreSuccessorLifecycleV3AfterCrash
  \/ ReopenPredecessorTransportAfterRestartRestore
  \/ ResealPredecessorTransportAfterRestartRestore

RepeatedHandoffMutationSpec ==
  /\ RepeatedHandoffMutationInit
  /\ [][RepeatedHandoffMutationNext]_typedRolloverVars

=============================================================================
