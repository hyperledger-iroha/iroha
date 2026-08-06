---- MODULE SumeragiV2AsyncSchedulerCompositionProofs ----
EXTENDS SumeragiV2AsyncProtectedSlotProofs

(***************************************************************************
Exact scheduler-action composition.  The private source histories are carried
through every executable queue carrier and then projected back to the public
protected deferred-slot invariant.
***************************************************************************)
ProgressCommitSlotInvariant ==
  /\ LockWithinNodeViewInvariant
  /\ QueuedProgressCommitHistoryInvariant
  /\ DeferredProgressCommitHistoryInvariant
  /\ CausalProgressCommitHistoryInvariant
  /\ ProtectedDeferredProgressInvariant

THEOREM UnchangedSchedulerCarriersPreserveProgressCommitSlots ==
  /\ AsyncTypeInvariant
  /\ TypeInvariant'
  /\ LockWithinNodeViewInvariant
  /\ QueuedProgressCommitHistoryInvariant
  /\ DeferredProgressCommitHistoryInvariant
  /\ CausalProgressCommitHistoryInvariant
  /\ ProtectedDeferredProgressInvariant
  /\ ProgressFrontierAction
  /\ UNCHANGED <<asyncCommandQueues,
                  asyncDeferredProgressQueues,
                  asyncCausalQueues>>
  => /\ QueuedProgressCommitHistoryInvariant'
     /\ DeferredProgressCommitHistoryInvariant'
     /\ CausalProgressCommitHistoryInvariant'
     /\ ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME /\ AsyncTypeInvariant
                /\ TypeInvariant'
                /\ LockWithinNodeViewInvariant
                /\ QueuedProgressCommitHistoryInvariant
                /\ DeferredProgressCommitHistoryInvariant
                /\ CausalProgressCommitHistoryInvariant
                /\ ProtectedDeferredProgressInvariant
                /\ ProgressFrontierAction
                /\ UNCHANGED <<asyncCommandQueues,
                                asyncDeferredProgressQueues,
                                asyncCausalQueues>>
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
               /\ ProtectedDeferredProgressInvariant'
    <2>1. /\ TypeInvariant
           /\ N \in Nat \ {0}
           /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncDeferredTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncDeferredTypeInvariant,
             TypeInvariant, ModelConfiguration, QuorumConfiguration
    <2>2. QueuedProgressCommitHistoryInvariant'
      BY <1>1, <2>1,
         UnchangedCommandQueuesPreserveProgressCommitHistory
    <2>3. DeferredProgressCommitHistoryInvariant'
      BY <1>1, <2>1,
         UnchangedDeferredProgressQueuesPreserveCommitHistory
    <2>4. CausalProgressCommitHistoryInvariant'
      BY <1>1, <2>1,
         UnchangedCausalQueuesPreserveProgressCommitHistory
    <2>5. ProtectedDeferredProgressInvariant'
      BY <1>1, <2>1,
         LockAdvancePreservesProtectedDeferredProgressInvariant
         DEF ProgressFrontierAction
    <2> QED BY <2>2, <2>3, <2>4, <2>5
  <1> QED BY <1>1

THEOREM FifoRuntimePreservesProgressCommitSlotInvariantExact ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ TypeInvariant'

    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ ProgressFrontierAction
    /\ FifoRuntimeStep(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
       /\ ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                AsyncControlServiceStateTypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                QueuedProgressCommitHistoryInvariant,
                DeferredProgressCommitHistoryInvariant,
                CausalProgressCommitHistoryInvariant,
                ProtectedDeferredProgressInvariant,
                ProgressFrontierAction,
                FifoRuntimeStep(node)
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
               /\ ProtectedDeferredProgressInvariant'
    <2> DEFINE Command == NextNodeCommand(node)
    <2>1. /\ TypeInvariant
           /\ N \in Nat \ {0}
           /\ AsyncConfiguration
           /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncDeferredTypeInvariant
           /\ NodeQueueNonempty(node)
           /\ RemoveNextNodeCommand(node)
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ModelConfiguration, QuorumConfiguration,
             AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, FifoRuntimeStep
    <2>2. /\ AsyncCandidateTyped(Command)
           /\ Command.node = node
           /\ ProgressCommitSource(Command)
      <3>1. /\ NextNodeCommandIndex(node)
                       \in 1..Len(asyncCommandQueues[node])
             /\ NextNodeCommand(node) =
                  asyncCommandQueues[node][NextNodeCommandIndex(node)]
             /\ AsyncCandidateTyped(NextNodeCommand(node))
             /\ NextNodeCommand(node).node = node
        BY <2>1, NextNodeCommandIndexFacts
      <3>2. NextNodeCommand(node) \in
               SequenceSet(asyncCommandQueues[node])
        BY <3>1 DEF SequenceSet
      <3> QED BY <1>1, <3>1, <3>2
           DEF QueuedProgressCommitHistoryInvariant,
               ProgressCommitSourcesIn, Command
    <2>3. QueuedProgressCommitHistoryInvariant'
      BY <1>1, <2>1,
         RemoveNextNodeCommandPreservesQueuedProgressCommitHistory
    <2>4. CASE CommandDispatchable(Command)
      <3>1. /\ ExecuteCommand(Command)
             /\ AppendCausalSuccessors(Command)
             /\ UNCHANGED asyncDeferredProgressQueues
        BY <1>1, <2>4, Isa DEF FifoRuntimeStep, Command
      <3>2. DeferredProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <3>1,
           UnchangedDeferredProgressQueuesPreserveCommitHistory
      <3>3. CausalProgressCommitHistoryInvariant'
        BY <1>1, <2>2, <3>1,
           AppendCausalSuccessorsPreservesProgressCommitHistory
      <3>4. ProtectedDeferredProgressInvariant'
        BY <1>1, <2>1, <3>1,
           LockAdvancePreservesProtectedDeferredProgressInvariant
           DEF ProgressFrontierAction
      <3> QED BY <2>3, <3>2, <3>3, <3>4
    <2>5. CASE /\ ~CommandDispatchable(Command)
                 /\ ~NodeIdle(node)
      <3>1. /\ DeferCommand(Command)
             /\ LeaveCausalQueues
        BY <1>1, <2>5, Isa DEF FifoRuntimeStep, Command
      <3>2. DeferredProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <2>2, <3>1,
           DeferCommandPreservesDeferredProgressCommitHistory
      <3>3. CausalProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <3>1,
           UnchangedCausalQueuesPreserveProgressCommitHistory
           DEF LeaveCausalQueues
      <3>4. ProtectedDeferredProgressInvariant'
        BY <1>1, <2>1, <2>2, <3>1,
           DeferCommandPreservesProtectedDeferredProgressInvariant
      <3> QED BY <2>3, <3>2, <3>3, <3>4
    <2>6. CASE /\ ~CommandDispatchable(Command)
                 /\ NodeIdle(node)
      <3>1. /\ UNCHANGED asyncDeferredProgressQueues
             /\ LeaveCausalQueues
        BY <1>1, <2>6, Isa DEF FifoRuntimeStep, Command
      <3>2. DeferredProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <3>1,
           UnchangedDeferredProgressQueuesPreserveCommitHistory
      <3>3. CausalProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <3>1,
           UnchangedCausalQueuesPreserveProgressCommitHistory
           DEF LeaveCausalQueues
      <3>4. ProtectedDeferredProgressInvariant'
        BY <1>1, <2>1, <3>1,
           LockAdvancePreservesProtectedDeferredProgressInvariant
           DEF ProgressFrontierAction
      <3> QED BY <2>3, <3>2, <3>3, <3>4
    <2>7. \/ CommandDispatchable(Command)
           \/ /\ ~CommandDispatchable(Command) /\ ~NodeIdle(node)
           \/ /\ ~CommandDispatchable(Command) /\ NodeIdle(node)
      BY SMT
    <2> QED BY <2>4, <2>5, <2>6, <2>7
  <1> QED BY <1>1

THEOREM DeferredDrainPreservesProgressCommitSlotInvariant ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ ProgressFrontierAction
    /\ DeferredDrainStep(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
       /\ ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                AsyncControlServiceStateTypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                QueuedProgressCommitHistoryInvariant,
                DeferredProgressCommitHistoryInvariant,
                CausalProgressCommitHistoryInvariant,
                ProtectedDeferredProgressInvariant,
                ProgressFrontierAction,
                DeferredDrainStep(node)
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
               /\ ProtectedDeferredProgressInvariant'
    <2>1. /\ TypeInvariant
           /\ N \in Nat \ {0}
           /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncDeferredTypeInvariant
      BY <1>1
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ModelConfiguration, QuorumConfiguration,
             AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant
    <2>2. QueuedProgressCommitHistoryInvariant'
      BY <1>1, <2>1,
         UnchangedCommandQueuesPreserveProgressCommitHistory
         DEF DeferredDrainStep
    <2>3. CASE ~DeferredQueueNonempty(node)
      <3>1. UNCHANGED <<asyncDeferredProgressQueues,
                        asyncCausalQueues>>
        BY <1>1, <2>3, Isa
           DEF DeferredDrainStep, DeferredWorkServiceable,
               LeaveCausalQueues, vars
      <3>2. DeferredProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <3>1,
           UnchangedDeferredProgressQueuesPreserveCommitHistory
      <3>3. CausalProgressCommitHistoryInvariant'
        BY <1>1, <2>1, <3>1,
           UnchangedCausalQueuesPreserveProgressCommitHistory
      <3>4. ProtectedDeferredProgressInvariant'
        BY <1>1, <2>1, <3>1,
           LockAdvancePreservesProtectedDeferredProgressInvariant
           DEF ProgressFrontierAction
      <3> QED BY <2>2, <3>2, <3>3, <3>4
    <2>4. CASE DeferredQueueNonempty(node)
      <3> DEFINE Command == NextDeferredCommand(node)
      <3>1. /\ AsyncCandidateTyped(Command)
             /\ Command.node = node
        BY <1>1, <2>1, <2>4,
           RuntimeSelectedCommandsAreTyped DEF Command
      <3>2. CASE DeferredHandoffAllowsExecution(node, Command)
        <4>1. /\ RemoveNextDeferredCommand(node)
               /\ ExecuteCommand(Command)
               /\ AppendCausalSuccessors(Command)
          BY <1>1, <2>4, <3>2, Isa
             DEF DeferredDrainStep, Command
        <4>2. DeferredProgressCommitHistoryInvariant'
          BY <1>1, <2>1, <2>4, <4>1,
             RemoveNextDeferredCommandPreservesProgressCommitHistory
        <4>3. CausalProgressCommitHistoryInvariant'
          BY <1>1, <3>1, <4>1,
             AppendCausalSuccessorsPreservesProgressCommitHistory
        <4>4. ProtectedDeferredProgressInvariant'
          BY <1>1, <2>1, <2>4, <4>1,
             RemoveDeferredAndAdvanceLockPreservesProtectedInvariant
             DEF ProgressFrontierAction
        <4> QED BY <2>2, <4>2, <4>3, <4>4
      <3>3. CASE /\ ~DeferredHandoffAllowsExecution(node, Command)
                   /\ DeferredHandoffBlocksExecution(node, Command)
        <4>1. UNCHANGED <<asyncDeferredProgressQueues,
                          asyncCausalQueues>>
          BY <1>1, <2>4, <3>3, Isa
             DEF DeferredDrainStep, DeferredWorkServiceable,
                 Command, LeaveCausalQueues, vars
        <4>2. DeferredProgressCommitHistoryInvariant'
          BY <1>1, <2>1, <4>1,
             UnchangedDeferredProgressQueuesPreserveCommitHistory
        <4>3. CausalProgressCommitHistoryInvariant'
          BY <1>1, <2>1, <4>1,
             UnchangedCausalQueuesPreserveProgressCommitHistory
        <4>4. ProtectedDeferredProgressInvariant'
          BY <1>1, <2>1, <4>1,
             LockAdvancePreservesProtectedDeferredProgressInvariant
             DEF ProgressFrontierAction
        <4> QED BY <2>2, <4>2, <4>3, <4>4
      <3>4. CASE /\ ~DeferredHandoffAllowsExecution(node, Command)
                   /\ ~DeferredHandoffBlocksExecution(node, Command)
                   /\ ~NodeIdle(node)
        <4>1. UNCHANGED <<asyncDeferredProgressQueues,
                          asyncCausalQueues>>
          BY <1>1, <2>4, <3>4, Isa
             DEF DeferredDrainStep, Command, LeaveCausalQueues, vars
        <4>2. DeferredProgressCommitHistoryInvariant'
          BY <1>1, <2>1, <4>1,
             UnchangedDeferredProgressQueuesPreserveCommitHistory
        <4>3. CausalProgressCommitHistoryInvariant'
          BY <1>1, <2>1, <4>1,
             UnchangedCausalQueuesPreserveProgressCommitHistory
        <4>4. ProtectedDeferredProgressInvariant'
          BY <1>1, <2>1, <4>1,
             LockAdvancePreservesProtectedDeferredProgressInvariant
             DEF ProgressFrontierAction
        <4> QED BY <2>2, <4>2, <4>3, <4>4
      <3>5. CASE /\ ~DeferredHandoffAllowsExecution(node, Command)
                   /\ ~DeferredHandoffBlocksExecution(node, Command)
                   /\ NodeIdle(node)
        <4>1. /\ RemoveNextDeferredCommand(node)
               /\ LeaveCausalQueues
          BY <1>1, <2>4, <3>5, Isa
             DEF DeferredDrainStep, Command
        <4>2. DeferredProgressCommitHistoryInvariant'
          BY <1>1, <2>1, <2>4, <4>1,
             RemoveNextDeferredCommandPreservesProgressCommitHistory
        <4>3. CausalProgressCommitHistoryInvariant'
          BY <1>1, <2>1, <4>1,
             UnchangedCausalQueuesPreserveProgressCommitHistory
             DEF LeaveCausalQueues
        <4>4. ProtectedDeferredProgressInvariant'
          BY <1>1, <2>1, <2>4, <4>1,
             RemoveDeferredAndAdvanceLockPreservesProtectedInvariant
             DEF ProgressFrontierAction
        <4> QED BY <2>2, <4>2, <4>3, <4>4
      <3>6. \/ DeferredHandoffAllowsExecution(node, Command)
             \/ /\ ~DeferredHandoffAllowsExecution(node, Command)
                    /\ DeferredHandoffBlocksExecution(node, Command)
             \/ /\ ~DeferredHandoffAllowsExecution(node, Command)
                    /\ ~DeferredHandoffBlocksExecution(node, Command)
                    /\ ~NodeIdle(node)
             \/ /\ ~DeferredHandoffAllowsExecution(node, Command)
                    /\ ~DeferredHandoffBlocksExecution(node, Command)
                    /\ NodeIdle(node)
        BY SMT
      <3> QED BY <3>2, <3>3, <3>4, <3>5, <3>6
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

NonQueueRuntimeAction(node) ==
  \/ DeferredTagStep(node)
  \/ DirectTimeoutStep(node)
  \/ DirectRetransmitStep(node)
  \/ IdleRuntimeStep(node)


THEOREM NonQueueRuntimePreservesProgressCommitSlotInvariant ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ ProgressFrontierAction
    /\ NonQueueRuntimeAction(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
       /\ ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                QueuedProgressCommitHistoryInvariant,
                DeferredProgressCommitHistoryInvariant,
                CausalProgressCommitHistoryInvariant,
                ProtectedDeferredProgressInvariant,
                ProgressFrontierAction,
                NonQueueRuntimeAction(node)
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
               /\ ProtectedDeferredProgressInvariant'
    <2>1. /\ TypeInvariant
           /\ N \in Nat \ {0}
           /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncDeferredTypeInvariant
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, TypeInvariant,
             ModelConfiguration, QuorumConfiguration
    <2>2. UNCHANGED <<asyncCommandQueues,
                      asyncDeferredProgressQueues>>
      BY <1>1, Isa
         DEF NonQueueRuntimeAction,
             DeferredTagStep, DeferredTimeoutStep,
             DeferredRetransmitStep, DirectTimeoutStep,
             DirectRetransmitStep, IdleRuntimeStep,
             AsyncDeferredVars, LeaveCausalQueues, vars
    <2>3. /\ QueuedProgressCommitHistoryInvariant'
           /\ DeferredProgressCommitHistoryInvariant'
           /\ ProtectedDeferredProgressInvariant'
      BY <1>1, <2>1, <2>2,
         UnchangedCommandQueuesPreserveProgressCommitHistory,
         UnchangedDeferredProgressQueuesPreserveCommitHistory,
         LockAdvancePreservesProtectedDeferredProgressInvariant
         DEF ProgressFrontierAction
    <2>4. \/ UNCHANGED asyncCausalQueues
           \/ AppendCausalSuccessors(TimeoutCausalCommand(node))
      BY <1>1, Isa
         DEF NonQueueRuntimeAction,
             DeferredTagStep, DeferredTimeoutStep,
             DeferredRetransmitStep, DirectTimeoutStep,
             DirectRetransmitStep, IdleRuntimeStep,
             LeaveCausalQueues
    <2>5. CASE UNCHANGED asyncCausalQueues
      BY <1>1, <2>1, <2>5,
         UnchangedCausalQueuesPreserveProgressCommitHistory
    <2>6. CASE AppendCausalSuccessors(TimeoutCausalCommand(node))
      BY <1>1, <2>6,
         AppendTimeoutCausalSuccessorsPreservesProgressCommitHistory
    <2>7. CausalProgressCommitHistoryInvariant'
      BY <2>4, <2>5, <2>6
    <2> QED BY <2>3, <2>7
  <1> QED BY <1>1

THEOREM RuntimeStepPreservesProgressCommitSlotInvariant ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ ProgressFrontierAction
    /\ RuntimeStep(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
       /\ ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                AsyncControlServiceStateTypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                QueuedProgressCommitHistoryInvariant,
                DeferredProgressCommitHistoryInvariant,
                CausalProgressCommitHistoryInvariant,
                ProtectedDeferredProgressInvariant,
                ProgressFrontierAction,
                RuntimeStep(node)
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
               /\ ProtectedDeferredProgressInvariant'
    <2>1. \/ DeferredDrainStep(node)
           \/ FifoRuntimeStep(node)
           \/ NonQueueRuntimeAction(node)
      BY <1>1 DEF RuntimeStep, NonQueueRuntimeAction
    <2>2. CASE DeferredDrainStep(node)
      BY <1>1, <2>2,
         DeferredDrainPreservesProgressCommitSlotInvariant
    <2>3. CASE FifoRuntimeStep(node)
      BY <1>1, <2>3,
         FifoRuntimePreservesProgressCommitSlotInvariantExact
    <2>4. CASE NonQueueRuntimeAction(node)
      BY <1>1, <2>4,
         NonQueueRuntimePreservesProgressCommitSlotInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM LocalAdmissionPreservesProgressCommitSlotInvariant ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ ProgressFrontierAction
    /\ LocalAdmissionStep(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
       /\ ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                QueuedProgressCommitHistoryInvariant,
                DeferredProgressCommitHistoryInvariant,
                CausalProgressCommitHistoryInvariant,
                ProtectedDeferredProgressInvariant,
                ProgressFrontierAction,
                LocalAdmissionStep(node)
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
               /\ ProtectedDeferredProgressInvariant'
    <2>1. /\ TypeInvariant
           /\ N \in Nat \ {0}
           /\ AsyncDeferredTypeInvariant
      BY <1>1 DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                    TypeInvariant, ModelConfiguration,
                    QuorumConfiguration
    <2>2. /\ QueuedProgressCommitHistoryInvariant'
           /\ DeferredProgressCommitHistoryInvariant'
           /\ CausalProgressCommitHistoryInvariant'
      BY <1>1, LocalAdmissionPreservesProgressCommitHistories
    <2>3. UNCHANGED asyncDeferredProgressQueues
      BY <1>1 DEF LocalAdmissionStep, AsyncDeferredVars
    <2>4. ProtectedDeferredProgressInvariant'
      BY <1>1, <2>1, <2>3,
         LockAdvancePreservesProtectedDeferredProgressInvariant
         DEF ProgressFrontierAction
    <2> QED BY <2>2, <2>4
  <1> QED BY <1>1

THEOREM SerializedLocalPredecessorPreservesProgressCommitSlotInvariant ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ ProgressFrontierAction
    /\ SerializedLocalPrecedesServeIngressStep(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
       /\ ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                QueuedProgressCommitHistoryInvariant,
                DeferredProgressCommitHistoryInvariant,
                CausalProgressCommitHistoryInvariant,
                ProtectedDeferredProgressInvariant,
                ProgressFrontierAction,
                SerializedLocalPrecedesServeIngressStep(node)
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
               /\ ProtectedDeferredProgressInvariant'
    <2>1. /\ TypeInvariant
           /\ N \in Nat \ {0}
           /\ AsyncDeferredTypeInvariant
      BY <1>1 DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                    TypeInvariant, ModelConfiguration,
                    QuorumConfiguration
    <2>2. SelectedLocalAdmissionAdvance(node)
      BY <1>1 DEF SerializedLocalPrecedesServeIngressStep
    <2>3. /\ QueuedProgressCommitHistoryInvariant'
           /\ DeferredProgressCommitHistoryInvariant'
           /\ CausalProgressCommitHistoryInvariant'
      BY <1>1, <2>2,
         SelectedLocalAdmissionAdvancePreservesProgressCommitHistories
    <2>4. UNCHANGED asyncDeferredProgressQueues
      BY <2>2 DEF SelectedLocalAdmissionAdvance, AsyncDeferredVars
    <2>5. ProtectedDeferredProgressInvariant'
      BY <1>1, <2>1, <2>4,
         LockAdvancePreservesProtectedDeferredProgressInvariant
         DEF ProgressFrontierAction
    <2> QED BY <2>3, <2>5
  <1> QED BY <1>1

THEOREM IngressDrainPreservesProgressCommitSlotInvariant ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ ProgressFrontierAction
    /\ IngressDrainStep(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
       /\ ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                QueuedProgressCommitHistoryInvariant,
                DeferredProgressCommitHistoryInvariant,
                CausalProgressCommitHistoryInvariant,
                ProtectedDeferredProgressInvariant,
                ProgressFrontierAction,
                IngressDrainStep(node)
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
               /\ ProtectedDeferredProgressInvariant'
    <2>1. /\ TypeInvariant
           /\ N \in Nat \ {0}
           /\ AsyncDeferredTypeInvariant
      BY <1>1 DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                    TypeInvariant, ModelConfiguration,
                    QuorumConfiguration
    <2>2. /\ QueuedProgressCommitHistoryInvariant'
           /\ DeferredProgressCommitHistoryInvariant'
           /\ CausalProgressCommitHistoryInvariant'
      BY <1>1, IngressDrainPreservesProgressCommitHistories
    <2>3. UNCHANGED asyncDeferredProgressQueues
      BY <1>1 DEF IngressDrainStep, AsyncDeferredVars
    <2>4. ProtectedDeferredProgressInvariant'
      BY <1>1, <2>1, <2>3,
         LockAdvancePreservesProtectedDeferredProgressInvariant
         DEF ProgressFrontierAction
    <2> QED BY <2>2, <2>4
  <1> QED BY <1>1

THEOREM ReplayRunNodeContinuationPreservesProgressCommitSlotInvariant ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ ProgressFrontierAction
    /\ ReplayRunNodeCandidateProducerContinuation(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
       /\ ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                AsyncControlServiceStateTypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                QueuedProgressCommitHistoryInvariant,
                DeferredProgressCommitHistoryInvariant,
                CausalProgressCommitHistoryInvariant,
                ProtectedDeferredProgressInvariant,
                ProgressFrontierAction,
                ReplayRunNodeCandidateProducerContinuation(node)
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
               /\ ProtectedDeferredProgressInvariant'
    <2>1. CASE
              AsyncCandidateProducerContinuationExactLocalReplayStep(node)
      <3> DEFINE Record ==
             AsyncCandidateProducerContinuationSelectedReplayRecord(node)
      <3> DEFINE Candidate == Record.candidate
      <3>1. /\ AsyncCandidateTyped(Candidate)
             /\ ProgressCommitSource(Candidate)
             /\ EnqueueCandidate(Candidate)
        BY <1>1, <2>1, Isa
           DEF Record, Candidate,
               AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncControlServiceStateTypeInvariant,
               AsyncCandidateProducerContinuationExactLocalReplayStep,
               AsyncCandidateProducerContinuationSelectedLocalCandidate,
               AsyncCandidateProducerContinuationSelectedReplayRecord,
               AsyncCandidateProducerContinuationSelectedResolutionRecord,
               AsyncCandidateProducerContinuationResolutionRecordsForNode,
               AsyncCandidateProducerContinuationRecordSet,
               AsyncCandidateProducerContinuationRecord,
               AsyncCandidateProducerContinuationSourceClass,
               AsyncCandidateProducerContinuationLocallyReconstructibleKinds,
               ProgressCommitSource, ProgressCommitVoteHistory
      <3>2. QueuedProgressCommitHistoryInvariant'
        BY <1>1, <3>1,
           EnqueueCandidatePreservesQueuedProgressCommitHistory
      <3>3. UNCHANGED <<asyncDeferredProgressQueues,
                        asyncCausalQueues>>
        BY <2>1, Isa
           DEF AsyncCandidateProducerContinuationExactLocalReplayStep,
               AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
               AsyncDeferredVars
      <3>4. /\ DeferredProgressCommitHistoryInvariant'
             /\ CausalProgressCommitHistoryInvariant'
             /\ ProtectedDeferredProgressInvariant'
        BY <1>1, <3>3,
           UnchangedDeferredProgressQueuesPreserveCommitHistory,
           UnchangedCausalQueuesPreserveProgressCommitHistory,
           LockAdvancePreservesProtectedDeferredProgressInvariant
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               ProgressFrontierAction
      <3> QED BY <3>2, <3>4
    <2>2. CASE
              AsyncCandidateProducerContinuationReplayTargetOnlyTurn(node)
      <3>1. UNCHANGED <<asyncCommandQueues,
                        asyncDeferredProgressQueues,
                        asyncCausalQueues>>
        BY <2>2, Isa
           DEF AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
               AsyncSchedulerExceptCausalControlRunnerAndNodeService,
               AsyncDeferredVars
      <3> QED BY <1>1, <3>1,
           UnchangedSchedulerCarriersPreserveProgressCommitSlots
    <2>3. CASE
              AsyncCandidateProducerContinuationExactRuntimeReplayStep(node)
      <3>1. RuntimeStep(node)
        BY <2>3, Isa
           DEF AsyncCandidateProducerContinuationExactRuntimeReplayStep,
               RuntimeStep
      <3> QED BY <1>1, <3>1,
           RuntimeStepPreservesProgressCommitSlotInvariant
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF ReplayRunNodeCandidateProducerContinuation
  <1> QED BY <1>1

THEOREM RunNodeWorkPreservesProgressCommitSlotInvariant ==
  \A node \in ValidatorIds:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncControlServiceStateTypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ ProgressFrontierAction
    /\ RunNodeWork(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
       /\ ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                AsyncControlServiceStateTypeInvariant,
                TypeInvariant',
                LockWithinNodeViewInvariant,
                QueuedProgressCommitHistoryInvariant,
                DeferredProgressCommitHistoryInvariant,
                CausalProgressCommitHistoryInvariant,
                ProtectedDeferredProgressInvariant,
                ProgressFrontierAction,

                RunNodeWork(node)
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
               /\ ProtectedDeferredProgressInvariant'
    <2>1. \/ ResolveRunNodeCandidateProducerContinuation(node)
           \/ ReplayRunNodeCandidateProducerContinuation(node)
           \/ LocalAdmissionStep(node)
           \/ IngressDrainStep(node)
           \/ SerializedRunnerRuntimeStep(node)
           \/ SerializedLocalPrecedesServeIngressStep(node)
           \/ AsyncServeIngressTargetOnlyTurn(node)
      BY <1>1, RunNodeWorkConcreteActionCaseSplit
    <2>1r. CASE
              ResolveRunNodeCandidateProducerContinuation(node)
      <3>1. UNCHANGED <<asyncCommandQueues,
                        asyncDeferredProgressQueues,
                        asyncCausalQueues>>
        BY <2>1r, Isa
           DEF ResolveRunNodeCandidateProducerContinuation,
               AsyncSchedulerExceptCausalControlAndNodeService
      <3> QED BY <1>1, <3>1,
           UnchangedSchedulerCarriersPreserveProgressCommitSlots
    <2>1p. CASE
              ReplayRunNodeCandidateProducerContinuation(node)
      BY <1>1, <2>1p,
         ReplayRunNodeContinuationPreservesProgressCommitSlotInvariant
    <2>2. CASE LocalAdmissionStep(node)
      BY <1>1, <2>2,
         LocalAdmissionPreservesProgressCommitSlotInvariant
    <2>3. CASE IngressDrainStep(node)
      BY <1>1, <2>3,
         IngressDrainPreservesProgressCommitSlotInvariant
    <2>4. CASE SerializedRunnerRuntimeStep(node)
      <3>1. RuntimeStep(node)
        BY <2>4, Isa
           DEF SerializedRunnerRuntimeStep, SerializedRuntimeStep,
               SerializedRuntimePrecedesServeIngressStep
      <3> QED BY <1>1, <3>1,
           RuntimeStepPreservesProgressCommitSlotInvariant
    <2>5. CASE AsyncServeIngressTargetOnlyTurn(node)
      <3>1. UNCHANGED <<asyncCommandQueues,
                        asyncDeferredProgressQueues,
                        asyncCausalQueues>>
        BY <2>5, Isa
           DEF AsyncServeIngressTargetOnlyTurn, AsyncDeferredVars
      <3> QED BY <1>1, <3>1,
           UnchangedSchedulerCarriersPreserveProgressCommitSlots
    <2>6. CASE SerializedLocalPrecedesServeIngressStep(node)
      BY <1>1, <2>6,
         SerializedLocalPredecessorPreservesProgressCommitSlotInvariant
    <2> QED BY <2>1, <2>1r, <2>1p, <2>2, <2>3, <2>4, <2>5,
                 <2>6
  <1> QED BY <1>1

THEOREM RunHistoricalServerLeavesProgressCarriers ==
  \A node:
    RunHistoricalServer(node)
      => UNCHANGED <<asyncCommandQueues,
                      asyncDeferredProgressQueues,
                      asyncCausalQueues>>
BY IsaM("blast")
   DEF RunHistoricalServer, DrainHistoricalIngressSelected,
       HistoricalIdleStep, AsyncDeferredVars

THEOREM AsyncFaultStepLeavesProgressCarriers ==
  AsyncFaultStep
    => UNCHANGED <<asyncCommandQueues,
                    asyncDeferredProgressQueues,
                    asyncCausalQueues>>
BY IsaM("blast")
   DEF AsyncFaultStep, PreGstLosePacket, PreGstCrash,
       InjectByzantineNoise, InjectUntrustedTransportCompletion,
       InjectAuthenticatedJunk,
       InjectByzantineCertifiedRequest,
       AsyncByzantineProposal, AsyncByzantineVote,
       AsyncByzantineTimeout, AsyncSchedulerVars,
       AsyncDeferredVars, LeaveCausalQueues

THEOREM AsyncNonRunnerStepLeavesProgressCarriers ==
  AsyncNonRunnerStep
    => UNCHANGED <<asyncCommandQueues,
                    asyncDeferredProgressQueues,
                    asyncCausalQueues>>
BY AsyncFaultStepLeavesProgressCarriers, IsaM("blast")
   DEF AsyncNonRunnerStep, AsyncSetGST, AsyncTick,
       AsyncNonClockVars, OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       PublishCommitCertificateRequests,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork, AsyncNetworkStep,
       AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       AsyncSchedulerVars, AsyncDeferredVars, LeaveCausalQueues

THEOREM PreGstCrashLeavesProgressCarriers ==
  \A node:
    PreGstCrash(node)
      => UNCHANGED <<asyncCommandQueues,
                      asyncDeferredProgressQueues,
                      asyncCausalQueues>>
BY DEF PreGstCrash, AsyncSchedulerVars

THEOREM PreGstResponsiveCrashLeavesProgressCarriers ==
  \A node:
    PreGstResponsiveCrash(node)
      => UNCHANGED <<asyncCommandQueues,
                      asyncDeferredProgressQueues,
                      asyncCausalQueues>>
BY DEF PreGstResponsiveCrash, AsyncSchedulerVars

THEOREM PreGstResponsiveRestartLeavesProgressCarriers ==
  PreGstResponsiveRestart
    => UNCHANGED <<asyncCommandQueues,
                    asyncDeferredProgressQueues,
                    asyncCausalQueues>>
BY DEF PreGstResponsiveRestart, AsyncSchedulerVars

THEOREM RestartSignatureReplayClassesAreCompletion ==
  \A node:
    \A command \in SequenceSet(RestartSignatureReplay(node)):
      command.class = "Completion"
BY Isa
   DEF RestartSignatureReplay, RestartTimeoutOrProposalReplay,
       RestartPrepareReplayIfActive, RestartLockedCommitReplayIfActive,
       RestartLockedCommitReplay, RestartTimeoutReplay,
       RestartPrepareReplay, RestartProposalReplay,
       RestartCandidate, AsyncCandidateAtConsumer,
       AsyncCandidateWithIdentity, SequenceSet

THEOREM RestartRunnerAssemblyClassesAreNormal ==
  \A node:
    \A command \in SequenceSet(RestartRunnerAssembly(node)):
      command.class = "Normal"
BY Isa
   DEF RestartRunnerAssembly, RestartCandidate,
       AsyncCandidateAtConsumer, AsyncCandidateWithIdentity, SequenceSet

THEOREM RestartReplayClassesAreNonProgress ==
  \A node:
    \A command \in SequenceSet(RestartReplay(node)):
      command.class \in {"Completion", "Normal"}
BY Isa
   DEF RestartReplay, RestartDecisionReplay,
       RestartLockedBodyReplay,
       RestartSignatureReplay, RestartTimeoutOrProposalReplay,
       RestartPrepareReplayIfActive, RestartLockedCommitReplayIfActive,
       RestartLockedCommitReplay, RestartTimeoutReplay,
       RestartPrepareReplay, RestartProposalReplay,
       RestartRunnerAssembly, RestartCandidate,
       AsyncCandidateAtConsumer, AsyncCandidateWithIdentity,
       SequenceSet

THEOREM RestartReplayHasProgressCommitSources ==
  \A node: ProgressCommitSourcesIn(RestartReplay(node))
BY RestartReplayClassesAreNonProgress, SMT
   DEF ProgressCommitSourcesIn, ProgressCommitSource

THEOREM RunHistoricalServerPreservesProgressCommitSlotInvariant ==
  \A node \in AsyncResponsiveAppliedArchiveServers:
    /\ AsyncTypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ ProgressFrontierAction
    /\ RunHistoricalServer(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
       /\ ProtectedDeferredProgressInvariant'
BY RunHistoricalServerLeavesProgressCarriers,
   UnchangedSchedulerCarriersPreserveProgressCommitSlots

THEOREM AsyncRunnerStepPreservesProgressCommitSlotInvariant ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncControlServiceStateTypeInvariant
  /\ TypeInvariant'
  /\ LockWithinNodeViewInvariant
  /\ QueuedProgressCommitHistoryInvariant
  /\ DeferredProgressCommitHistoryInvariant
  /\ CausalProgressCommitHistoryInvariant
  /\ ProtectedDeferredProgressInvariant
  /\ ProgressFrontierAction
  /\ AsyncRunnerStep
  => /\ QueuedProgressCommitHistoryInvariant'
     /\ DeferredProgressCommitHistoryInvariant'
     /\ CausalProgressCommitHistoryInvariant'
     /\ ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncTypeInvariant,
              AsyncControlServiceStateTypeInvariant,
              TypeInvariant',
              LockWithinNodeViewInvariant,
              QueuedProgressCommitHistoryInvariant,
              DeferredProgressCommitHistoryInvariant,
              CausalProgressCommitHistoryInvariant,
              ProtectedDeferredProgressInvariant,
              ProgressFrontierAction,
              AsyncRunnerStep
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
               /\ ProtectedDeferredProgressInvariant'
    <2>1. CASE \E node \in AsyncCurrentResponsiveVoters: RunNode(node)
      BY <1>1, <2>1, AsyncCurrentResponsiveVotersAreValidators,
         RunNodeWorkPreservesProgressCommitSlotInvariant
         DEF RunNode
    <2>2. CASE \E node \in asyncHistoricalRecoveryTargets:
                  RunHistoricalRecoveryNode(node)
      BY <1>1, <2>2, HistoricalRecoveryTargetsAreValidators,
         RunNodeWorkPreservesProgressCommitSlotInvariant
         DEF RunHistoricalRecoveryNode
    <2>3. CASE \E node \in AsyncResponsiveAppliedArchiveServers:
                  RunHistoricalServer(node)
      BY <1>1, <2>3,
         RunHistoricalServerPreservesProgressCommitSlotInvariant
    <2> QED BY <1>1, <2>1, <2>2, <2>3 DEF AsyncRunnerStep
  <1> QED BY <1>1

THEOREM AsyncNonRunnerStepPreservesProgressCommitSlotInvariant ==
  /\ AsyncTypeInvariant
  /\ TypeInvariant'
  /\ LockWithinNodeViewInvariant
  /\ QueuedProgressCommitHistoryInvariant
  /\ DeferredProgressCommitHistoryInvariant
  /\ CausalProgressCommitHistoryInvariant
  /\ ProtectedDeferredProgressInvariant
  /\ ProgressFrontierAction
  /\ AsyncNonRunnerStep
  => /\ QueuedProgressCommitHistoryInvariant'
     /\ DeferredProgressCommitHistoryInvariant'
     /\ CausalProgressCommitHistoryInvariant'
     /\ ProtectedDeferredProgressInvariant'
BY AsyncNonRunnerStepLeavesProgressCarriers,
   UnchangedSchedulerCarriersPreserveProgressCommitSlots

THEOREM PreGstCrashPreservesProgressCommitSlotInvariant ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ ProgressFrontierAction
    /\ PreGstCrash(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
       /\ ProtectedDeferredProgressInvariant'
BY PreGstCrashLeavesProgressCarriers,
   UnchangedSchedulerCarriersPreserveProgressCommitSlots

THEOREM PreGstResponsiveCrashPreservesProgressCommitSlotInvariant ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ TypeInvariant'
    /\ LockWithinNodeViewInvariant
    /\ QueuedProgressCommitHistoryInvariant
    /\ DeferredProgressCommitHistoryInvariant
    /\ CausalProgressCommitHistoryInvariant
    /\ ProtectedDeferredProgressInvariant
    /\ ProgressFrontierAction
    /\ PreGstResponsiveCrash(node)
    => /\ QueuedProgressCommitHistoryInvariant'
       /\ DeferredProgressCommitHistoryInvariant'
       /\ CausalProgressCommitHistoryInvariant'
       /\ ProtectedDeferredProgressInvariant'
BY PreGstResponsiveCrashLeavesProgressCarriers,
   UnchangedSchedulerCarriersPreserveProgressCommitSlots

THEOREM PreGstResponsiveRestartPreservesProgressCommitSlotInvariant ==
  /\ AsyncTypeInvariant
  /\ TypeInvariant'
  /\ LockWithinNodeViewInvariant
  /\ QueuedProgressCommitHistoryInvariant
  /\ DeferredProgressCommitHistoryInvariant
  /\ CausalProgressCommitHistoryInvariant
  /\ ProtectedDeferredProgressInvariant
  /\ ProgressFrontierAction
  /\ PreGstResponsiveRestart
  => /\ QueuedProgressCommitHistoryInvariant'
     /\ DeferredProgressCommitHistoryInvariant'
     /\ CausalProgressCommitHistoryInvariant'
     /\ ProtectedDeferredProgressInvariant'
BY PreGstResponsiveRestartLeavesProgressCarriers,
   UnchangedSchedulerCarriersPreserveProgressCommitSlots

THEOREM PreGstResponsiveReplayPreservesProgressCommitSlotInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ ProgressCommitSlotInvariant
  /\ PreGstResponsiveReplay
  => /\ QueuedProgressCommitHistoryInvariant'
     /\ DeferredProgressCommitHistoryInvariant'
     /\ CausalProgressCommitHistoryInvariant'
     /\ ProtectedDeferredProgressInvariant'
BY RestartReplayHasProgressCommitSources, FS_EmptySet, SMTT(30), Isa
   DEF PreGstResponsiveReplay, RecoveryCoreReplay,
       ResetNodeSchedulerForRestart,
       ResumeProposal, ResumeVote, ResumeTimeout,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ProgressCommitSlotInvariant,
       QueuedProgressCommitHistoryInvariant,
       DeferredProgressCommitHistoryInvariant,
       CausalProgressCommitHistoryInvariant,
       ProgressCommitSourcesIn, ProgressCommitSource,
       ProgressCommitVoteHistory, HistoricalLockedCommitItem,
       LockedPrepareRound, ProtectedDeferredProgressInvariant,
       ProtectedDeferredProgressIndices, ProtectedProgressCommand,
       SameProtectedProgressSlot, ProgressSlotShape, SequenceSet,
       AsyncConfiguration, ModelConfiguration, QuorumConfiguration

THEOREM DriveResponsiveReplayHeadPreservesProgressCommitSlotInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ ProgressCommitSlotInvariant
  /\ DriveResponsiveReplayHead
  => /\ QueuedProgressCommitHistoryInvariant'
     /\ DeferredProgressCommitHistoryInvariant'
     /\ CausalProgressCommitHistoryInvariant'
     /\ ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              ProgressCommitSlotInvariant,
              DriveResponsiveReplayHead
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
               /\ ProtectedDeferredProgressInvariant'
    <2> DEFINE Node == asyncRecoveryNode
    <2> DEFINE Candidate == Head(asyncRecoveryReplayQueue)
    <2> DEFINE Fresh == FreshCandidateSequence(Candidate)
    <2>1. /\ StrongInductiveInvariant
           /\ AsyncTypeInvariant
           /\ AsyncRecoveryTypeInvariant
           /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncDeferredTypeInvariant
           /\ N \in Nat \ {0}
           /\ Node \in ValidatorIds
           /\ AsyncQueueTyped(asyncRecoveryReplayQueue)
           /\ Len(asyncRecoveryReplayQueue) > 0
           /\ AsyncCandidateTyped(Candidate)
           /\ Candidate.node = Node
      BY <1>1, AsyncStrongTypeProjectsAsyncType,
         TypedQueueTailFacts, SMT
         DEF Node, Candidate, AsyncStrongTypeInvariant,
             StrongInductiveInvariant, Safety, TypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRecoveryTypeInvariant, DriveResponsiveReplayHead,
             ModelConfiguration, QuorumConfiguration, SequenceSet
    <2>2. /\ AsyncQueueTyped(Fresh)
           /\ ProgressCommitSourcesIn(Fresh)
      <3>1. AsyncQueueTyped(Fresh)
        BY <2>1, FreshTypedOwnedReplayCandidateProperties DEF Fresh
      <3>2. Candidate \in SequenceSet(asyncRecoveryReplayQueue)
        BY <2>1, PositiveSequenceIsNonempty,
           NonemptySequenceHeadIsFirst DEF Candidate, SequenceSet
      <3>3. Candidate.class = "Completion"
        BY <1>1, <2>1, <3>2,
           RestartSignatureReplayClassesAreCompletion
           DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
               Node, Candidate
      <3>4. ProgressCommitSource(Candidate)
        BY <3>3, CompletionCandidateHasProgressCommitSource
      <3>5. CASE CandidateScheduled(Candidate)
        <4>1. Fresh = <<>>
          BY <3>5 DEF Fresh, FreshCandidateSequence
        <4> QED BY <4>1, EmptySequenceHasProgressCommitSources
      <3>6. CASE ~CandidateScheduled(Candidate)
        <4>1. Fresh = <<Candidate>>
          BY <3>6 DEF Fresh, FreshCandidateSequence
        <4> QED BY <3>4, <4>1,
             SingletonSequenceHasProgressCommitSources
      <3> QED BY <3>1, <3>5, <3>6
    <2>3. AsyncNext
      BY <1>1, Isa
         DEF AsyncNext, AsyncNonCrashStep, RecoveryCoreReplay,
             ResumeProposal, ResumeVote, ResumeTimeout,
             DriveResponsiveReplayHead
    <2>4. /\ AsyncStrongTypeInvariant'
           /\ TypeInvariant'
      BY <1>1, <2>3, AsyncNextPreservesStrongTypeInvariant
         DEF AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety
    <2>5. ProgressFrontierAction
      BY <2>1, <2>3, AsyncNextEstablishesProgressFrontierAction
    <2>6. /\ QueuedProgressCommitHistoryInvariant
           /\ DeferredProgressCommitHistoryInvariant
           /\ CausalProgressCommitHistoryInvariant
           /\ ProtectedDeferredProgressInvariant
           /\ LockWithinNodeViewInvariant
      BY <1>1 DEF ProgressCommitSlotInvariant
    <2>7. /\ UNCHANGED asyncCommandQueues
           /\ UNCHANGED asyncDeferredProgressQueues
           /\ UNCHANGED <<context, prepareQCs>>
           /\ asyncCausalQueues' =
                [asyncCausalQueues EXCEPT
                   ![Node] = asyncCausalQueues[Node] \o Fresh]
      BY <1>1, Isa
         DEF Node, Candidate, Fresh, DriveResponsiveReplayHead,
             RecoveryCoreReplay, ResumeProposal, ResumeVote,
             ResumeTimeout, AsyncDeferredVars, vars
    <2>8. QueuedProgressCommitHistoryInvariant'
      BY <2>1, <2>4, <2>5, <2>6, <2>7,
         UnchangedCommandQueuesPreserveProgressCommitHistory
    <2>9. DeferredProgressCommitHistoryInvariant'
      BY <2>1, <2>4, <2>5, <2>6, <2>7,
         UnchangedDeferredProgressQueuesPreserveCommitHistory
    <2>10. CausalProgressCommitHistoryInvariant'
      BY <2>1, <2>2, <2>4, <2>5, <2>6, <2>7,
         AppendTypedCausalSequencePreservesProgressCommitHistory
    <2>11. ProtectedDeferredProgressInvariant'
      BY <2>1, <2>5, <2>6, <2>7,
         LockAdvancePreservesProtectedDeferredProgressInvariant
         DEF ProgressFrontierAction
    <2> QED BY <2>8, <2>9, <2>10, <2>11
  <1> QED BY <1>1

THEOREM FinishResponsiveReplayPreservesProgressCommitSlotInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ ProgressCommitSlotInvariant
  /\ FinishResponsiveReplay
  => /\ QueuedProgressCommitHistoryInvariant'
     /\ DeferredProgressCommitHistoryInvariant'
     /\ CausalProgressCommitHistoryInvariant'
     /\ ProtectedDeferredProgressInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              ProgressCommitSlotInvariant,
              FinishResponsiveReplay
         PROVE /\ QueuedProgressCommitHistoryInvariant'
               /\ DeferredProgressCommitHistoryInvariant'
               /\ CausalProgressCommitHistoryInvariant'
               /\ ProtectedDeferredProgressInvariant'
    <2> DEFINE Node == asyncRecoveryNode
    <2> DEFINE Runner == RestartRunnerAssembly(Node)
    <2>1. /\ StrongInductiveInvariant
           /\ AsyncTypeInvariant
           /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncDeferredTypeInvariant
           /\ N \in Nat \ {0}
           /\ Node \in ValidatorIds
           /\ AsyncQueueTyped(Runner)
           /\ Len(Runner) <= 1
      BY <1>1, AsyncStrongTypeProjectsAsyncType,
         RestartRunnerAssemblyProperties, SMT
         DEF Node, Runner, AsyncStrongTypeInvariant,
             StrongInductiveInvariant, Safety, TypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRecoveryTypeInvariant, FinishResponsiveReplay,
             ModelConfiguration, QuorumConfiguration
    <2>2. AsyncNext
      BY <1>1, Isa
         DEF AsyncNext, AsyncNonCrashStep, FinishResponsiveReplay
    <2>3. /\ AsyncStrongTypeInvariant'
           /\ TypeInvariant'
      BY <1>1, <2>2, AsyncNextPreservesStrongTypeInvariant
         DEF AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety
    <2>4. ProgressFrontierAction
      BY <2>1, <2>2, AsyncNextEstablishesProgressFrontierAction
    <2>5. /\ QueuedProgressCommitHistoryInvariant
           /\ DeferredProgressCommitHistoryInvariant
           /\ CausalProgressCommitHistoryInvariant
           /\ ProtectedDeferredProgressInvariant
           /\ LockWithinNodeViewInvariant
      BY <1>1 DEF ProgressCommitSlotInvariant
    <2>6. /\ UNCHANGED asyncCommandQueues
           /\ UNCHANGED asyncDeferredProgressQueues
           /\ UNCHANGED <<context, prepareQCs>>
      BY <1>1, Isa
         DEF FinishResponsiveReplay, AsyncDeferredVars, vars
    <2>7. QueuedProgressCommitHistoryInvariant'
      BY <2>1, <2>3, <2>4, <2>5, <2>6,
         UnchangedCommandQueuesPreserveProgressCommitHistory
    <2>8. DeferredProgressCommitHistoryInvariant'
      BY <2>1, <2>3, <2>4, <2>5, <2>6,
         UnchangedDeferredProgressQueuesPreserveCommitHistory
    <2>9. ProtectedDeferredProgressInvariant'
      BY <2>1, <2>4, <2>5, <2>6,
         LockAdvancePreservesProtectedDeferredProgressInvariant
         DEF ProgressFrontierAction
    <2>10. CausalProgressCommitHistoryInvariant'
      <3>1. CASE Len(Runner) = 0
        <4>1. UNCHANGED asyncCausalQueues
          BY <1>1, <3>1 DEF FinishResponsiveReplay, Node, Runner
        <4> QED BY <2>1, <2>3, <2>4, <2>5, <4>1,
             UnchangedCausalQueuesPreserveProgressCommitHistory
      <3>2. CASE Len(Runner) > 0
        <4> DEFINE Candidate == Runner[1]
        <4> DEFINE Fresh == FreshCandidateSequence(Candidate)
        <4>1. /\ Len(Runner) = 1
               /\ AsyncCandidateTyped(Candidate)
               /\ Candidate.node = Node
          BY <2>1, <3>2, SMT
             DEF Candidate, AsyncQueueTyped,
                 AsyncCausalQueueOwnership, SequenceSet
        <4>2. /\ AsyncQueueTyped(Fresh)
               /\ ProgressCommitSourcesIn(Fresh)
          <5>1. AsyncQueueTyped(Fresh)
            BY <4>1, FreshTypedOwnedReplayCandidateProperties DEF Fresh
          <5>2. Candidate \in SequenceSet(Runner)
            BY <4>1, Isa DEF Candidate, SequenceSet
          <5>3. Candidate.class = "Normal"
            BY <5>2, RestartRunnerAssemblyClassesAreNormal DEF Runner
          <5>4. ProgressCommitSource(Candidate)
            BY <5>3 DEF ProgressCommitSource
          <5>5. CASE CandidateScheduled(Candidate)
            <6>1. Fresh = <<>>
              BY <5>5 DEF Fresh, FreshCandidateSequence
            <6> QED BY <6>1, EmptySequenceHasProgressCommitSources
          <5>6. CASE ~CandidateScheduled(Candidate)
            <6>1. Fresh = <<Candidate>>
              BY <5>6 DEF Fresh, FreshCandidateSequence
            <6> QED BY <5>4, <6>1,
                 SingletonSequenceHasProgressCommitSources
          <5> QED BY <5>1, <5>5, <5>6
        <4>3. asyncCausalQueues' =
                 [asyncCausalQueues EXCEPT
                    ![Node] = asyncCausalQueues[Node] \o Fresh]
          BY <1>1, <4>1
             DEF FinishResponsiveReplay, Node, Runner, Candidate, Fresh
        <4> QED BY <2>1, <2>3, <2>4, <2>5, <4>2, <4>3,
             AppendTypedCausalSequencePreservesProgressCommitHistory
      <3> QED BY <2>1, <3>1, <3>2, SMT
    <2> QED BY <2>7, <2>8, <2>9, <2>10
  <1> QED BY <1>1

THEOREM AsyncNextPreservesProgressCommitSlotInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ ProgressCommitSlotInvariant
  /\ AsyncNext
  => ProgressCommitSlotInvariant'
PROOF
  <1>1. ASSUME /\ AsyncStrongTypeInvariant
                /\ ProgressCommitSlotInvariant
                /\ AsyncNext
         PROVE ProgressCommitSlotInvariant'
    <2>1. /\ StrongInductiveInvariant
           /\ AsyncTypeInvariant
           /\ AsyncControlServiceStateTypeInvariant
           /\ LockWithinNodeViewInvariant
           /\ QueuedProgressCommitHistoryInvariant
           /\ DeferredProgressCommitHistoryInvariant
           /\ CausalProgressCommitHistoryInvariant
           /\ ProtectedDeferredProgressInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType,
         AsyncStrongTypeProjectsControlServiceStateType
         DEF AsyncStrongTypeInvariant, ProgressCommitSlotInvariant
    <2>2. AsyncStrongTypeInvariant'
      BY <1>1, AsyncNextPreservesStrongTypeInvariant
    <2>3. TypeInvariant'
      BY <2>2
         DEF AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety
    <2>4. ProgressFrontierAction
      BY <1>1, <2>1, AsyncNextEstablishesProgressFrontierAction
    <2>5. /\ QueuedProgressCommitHistoryInvariant'
           /\ DeferredProgressCommitHistoryInvariant'
           /\ CausalProgressCommitHistoryInvariant'
           /\ ProtectedDeferredProgressInvariant'
      <3>1. CASE AsyncNonCrashStep
        <4>1. CASE AsyncRunnerStep
          BY <1>1, <2>1, <2>3, <2>4, <3>1, <4>1,
             AsyncRunnerStepPreservesProgressCommitSlotInvariant
        <4>2. CASE AsyncNonRunnerStep
          BY <1>1, <2>1, <2>3, <2>4, <3>1, <4>2,
             AsyncNonRunnerStepPreservesProgressCommitSlotInvariant
        <4>3. CASE DriveResponsiveReplayHead
          BY <1>1, <3>1, <4>3,
             DriveResponsiveReplayHeadPreservesProgressCommitSlotInvariant
        <4>4. CASE FinishResponsiveReplay
          BY <1>1, <3>1, <4>4,
             FinishResponsiveReplayPreservesProgressCommitSlotInvariant
        <4>5. CASE RearmResponsiveRecovery
          BY <1>1, <3>1, <4>5, Isa
             DEF RearmResponsiveRecovery,
                 QueuedProgressCommitHistoryInvariant,
                 DeferredProgressCommitHistoryInvariant,
                 CausalProgressCommitHistoryInvariant,
                 ProtectedDeferredProgressInvariant,
                 ProtectedDeferredProgressIndices,
                 ProtectedProgressCommand, SequenceSet
        <4> QED BY <3>1, <4>1, <4>2, <4>3, <4>4, <4>5
             DEF AsyncNonCrashStep
      <3>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
        <4>1. ASSUME NEW node \in ValidatorIds,
                      PreGstCrash(node)
               PROVE /\ QueuedProgressCommitHistoryInvariant'
                     /\ DeferredProgressCommitHistoryInvariant'
                     /\ CausalProgressCommitHistoryInvariant'
                     /\ ProtectedDeferredProgressInvariant'
          BY <1>1, <2>1, <2>3, <2>4, <4>1,
             PreGstCrashPreservesProgressCommitSlotInvariant
        <4> QED BY <3>2, <4>1
      <3>3. CASE \E node \in ValidatorIds:
                    PreGstResponsiveCrash(node)
        <4>1. ASSUME NEW node \in ValidatorIds,
                      PreGstResponsiveCrash(node)
               PROVE /\ QueuedProgressCommitHistoryInvariant'
                     /\ DeferredProgressCommitHistoryInvariant'
                     /\ CausalProgressCommitHistoryInvariant'
                     /\ ProtectedDeferredProgressInvariant'
          BY <1>1, <2>1, <2>3, <2>4, <4>1,
             PreGstResponsiveCrashPreservesProgressCommitSlotInvariant
        <4> QED BY <3>3, <4>1
      <3>4. CASE PreGstResponsiveRestart
        BY <1>1, <3>4,
           PreGstResponsiveRestartPreservesProgressCommitSlotInvariant
      <3>5. CASE PreGstResponsiveReplay
        BY <1>1, <3>5,
           PreGstResponsiveReplayPreservesProgressCommitSlotInvariant
      <3> QED BY <1>1, <3>1, <3>2, <3>3, <3>4, <3>5 DEF AsyncNext
    <2>6. LockWithinNodeViewInvariant'
      BY <1>1, <2>1, AsyncNextPreservesLockWithinNodeView
    <2> QED BY <2>5, <2>6 DEF ProgressCommitSlotInvariant
  <1> QED BY <1>1

THEOREM AsyncAllVarsStutterPreservesProgressCommitSlotInvariant ==
  /\ ProgressCommitSlotInvariant
  /\ UNCHANGED AsyncAllVars
  => ProgressCommitSlotInvariant'
PROOF
  <1>1. ASSUME ProgressCommitSlotInvariant,
                UNCHANGED AsyncAllVars
         PROVE ProgressCommitSlotInvariant'
    <2>1. /\ context' = context
           /\ nodeView' = nodeView
           /\ prepareQCs' = prepareQCs
           /\ lockRank' = lockRank
           /\ lockSubject' = lockSubject
           /\ asyncCommandQueues' = asyncCommandQueues
           /\ asyncDeferredProgressQueues' =
                asyncDeferredProgressQueues
           /\ asyncCausalQueues' = asyncCausalQueues
      BY <1>1, Isa
         DEF AsyncAllVars, AsyncSchedulerVars,
             AsyncDeferredVars, vars
    <2>2. LockWithinNodeViewInvariant'
      BY <1>1, <2>1, Isa
         DEF ProgressCommitSlotInvariant,
             LockWithinNodeViewInvariant
    <2>3. QueuedProgressCommitHistoryInvariant'
      BY <1>1, <2>1, Isa
         DEF ProgressCommitSlotInvariant,
             QueuedProgressCommitHistoryInvariant,
             ProgressCommitSourcesIn, ProgressCommitSource,
             ProgressCommitVoteHistory, HistoricalLockedCommitItem,
             LockedPrepareRound
    <2>4. DeferredProgressCommitHistoryInvariant'
      BY <1>1, <2>1, Isa
         DEF ProgressCommitSlotInvariant,
             DeferredProgressCommitHistoryInvariant,
             ProgressCommitSourcesIn, ProgressCommitSource,
             ProgressCommitVoteHistory,
             HistoricalLockedCommitItem, LockedPrepareRound
    <2>5. CausalProgressCommitHistoryInvariant'
      BY <1>1, <2>1, Isa
         DEF ProgressCommitSlotInvariant,
             CausalProgressCommitHistoryInvariant,
             ProgressCommitSourcesIn, ProgressCommitSource,
             ProgressCommitVoteHistory, HistoricalLockedCommitItem,
             LockedPrepareRound
    <2>6. ProtectedDeferredProgressInvariant'
      <3>1. \A node \in ValidatorIds:
               ProtectedDeferredProgressIndices(node)' =
                 ProtectedDeferredProgressIndices(node)
        BY <2>1
           DEF ProtectedDeferredProgressIndices,
               ProtectedProgressCommand, HistoricalLockedCommitItem,
               LockedPrepareRound
      <3>2. \A node \in ValidatorIds:
               ProtectedDeferredProgressCardinality(node)' =
                 ProtectedDeferredProgressCardinality(node)
        BY <3>1 DEF ProtectedDeferredProgressCardinality
      <3>3. \A node, left, right:
               ProtectedDeferredProgressSlot(node, left, right)' =
                 ProtectedDeferredProgressSlot(node, left, right)
        BY <2>1
           DEF ProtectedDeferredProgressSlot,
               SameProtectedProgressSlot, ProtectedProgressCommand,
               HistoricalLockedCommitItem, LockedPrepareRound
      <3>4. \A node \in ValidatorIds:
               ProtectedDeferredProgressUniqueness(node)' =
                 ProtectedDeferredProgressUniqueness(node)
        BY <3>1, <3>3 DEF ProtectedDeferredProgressUniqueness
      <3>5. \A node \in ValidatorIds:
               ProtectedDeferredProgressNode(node)' =
                 ProtectedDeferredProgressNode(node)
        BY <3>2, <3>4 DEF ProtectedDeferredProgressNode
      <3>6. \A node \in ValidatorIds:
               ProtectedDeferredProgressNode(node)
        BY <1>1, Isa
           DEF ProgressCommitSlotInvariant,
               ProtectedDeferredProgressInvariant,
               ProtectedDeferredProgressNode,
               ProtectedDeferredProgressCardinality,
               ProtectedDeferredProgressUniqueness,
               ProtectedDeferredProgressSlot
      <3>7. \A node \in ValidatorIds:
               ProtectedDeferredProgressNode(node)'
        BY <3>5, <3>6
      <3> QED BY <3>7,
           PrimedProtectedDeferredProgressNodesImplyInvariant
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6
         DEF ProgressCommitSlotInvariant
  <1> QED BY <1>1

THEOREM AsyncBracketNextPreservesProgressCommitSlotInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ ProgressCommitSlotInvariant
  /\ [AsyncNext]_AsyncAllVars
  => /\ AsyncStrongTypeInvariant'
     /\ ProgressCommitSlotInvariant'
PROOF
  <1>1. ASSUME /\ AsyncStrongTypeInvariant
                /\ ProgressCommitSlotInvariant
                /\ [AsyncNext]_AsyncAllVars
         PROVE /\ AsyncStrongTypeInvariant'
               /\ ProgressCommitSlotInvariant'
    <2>1. CASE AsyncNext
      BY <1>1, <2>1, AsyncNextPreservesStrongTypeInvariant,
         AsyncNextPreservesProgressCommitSlotInvariant
    <2>2. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>2,
         AsyncAllVarsStutterPreservesStrongTypeInvariant,
         AsyncAllVarsStutterPreservesProgressCommitSlotInvariant
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesProgressCommitSlotInvariant ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => /\ AsyncStrongTypeInvariant
         /\ ProgressCommitSlotInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE /\ AsyncStrongTypeInvariant
               /\ ProgressCommitSlotInvariant
    <2>1. AsyncStrongTypeInvariant
      BY <1>1, AsyncInitEstablishesStrongTypeInvariant
    <2>2. /\ QueuedProgressCommitHistoryInvariant
           /\ DeferredProgressCommitHistoryInvariant
           /\ CausalProgressCommitHistoryInvariant
      BY <1>1, AsyncInitEstablishesProgressCommitHistories
    <2>3. LockWithinNodeViewInvariant

      BY <1>1, AsyncInitEstablishesLockWithinNodeView
    <2>4. ProtectedDeferredProgressInvariant
      BY <1>1, AsyncInitEstablishesProgressWitness
         DEF ProgressWitnessInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF ProgressCommitSlotInvariant
  <1> QED BY <1>1

THEOREM ProtectedDeferredProgressInvariantObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []ProtectedDeferredProgressInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []ProtectedDeferredProgressInvariant
    <2> DEFINE Inductive ==
           /\ AsyncStrongTypeInvariant
           /\ ProgressCommitSlotInvariant
    <2>1. AsyncInitAt(initialContext) => Inductive
      BY AsyncInitEstablishesProgressCommitSlotInvariant DEF Inductive
    <2>2. Inductive /\ [AsyncNext]_AsyncAllVars => Inductive'
      BY AsyncBracketNextPreservesProgressCommitSlotInvariant
         DEF Inductive
    <2>3. AsyncSpecAt(initialContext) => []Inductive
      BY <2>1, <2>2, PTL DEF AsyncSpecAt
    <2>4. Inductive => ProtectedDeferredProgressInvariant
      BY DEF Inductive, ProgressCommitSlotInvariant
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

=============================================================================
