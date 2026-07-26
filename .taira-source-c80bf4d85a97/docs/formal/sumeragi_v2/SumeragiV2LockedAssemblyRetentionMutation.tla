---- MODULE SumeragiV2LockedAssemblyRetentionMutation ----
EXTENDS Integers, Sequences, TLC

(***************************************************************************
Bounded red/green witness for the local locked-body assembly Busy exception.

The production runner owns local assembly outside the serialized reducer and
returns only LocalProposalReady through Completion ingress.  The retired model
treated AssembleBody as ordinary Normal work while the reducer was Busy.  Its
FIFO turn removed the runtime owner before attempting bounded Normal deferral;
an unrelated full Normal queue therefore erased the exact assembly owner.

The repaired dispatch predicate admits only the item-free Normal AssembleBody
constructor while Busy.  It executes atomically and installs the Completion
successor without displacing the already-owned Normal filler.
***************************************************************************)

CONSTANT AllowBusyLockedAssembly

AssemblyCommand ==
  [class |-> "Normal", kind |-> "AssembleBody", item |-> "NoAsyncItem"]

NormalFiller ==
  [class |-> "Normal", kind |-> "DeliverVote", item |-> "AuthenticatedVote"]

DeferredNormalCapacity == 1

VARIABLES runtimeQueue, deferredNormalQueue, bodyAssembled,
          beginProposalOwned

vars ==
  <<runtimeQueue, deferredNormalQueue, bodyAssembled, beginProposalOwned>>

SequenceContains(queue, command) ==
  \E index \in 1..Len(queue): queue[index] = command

LocalAssemblyBusyDispatchAllowed(command) ==
  /\ AllowBusyLockedAssembly
  /\ command.class = "Normal"
  /\ command.kind = "AssembleBody"
  /\ command.item = "NoAsyncItem"

CommandDispatchableWhileBusy(command) ==
  \/ command.class = "Completion"
  \/ LocalAssemblyBusyDispatchAllowed(command)

DeferredNormalAfter(command) ==
  IF SequenceContains(deferredNormalQueue, command)
  THEN deferredNormalQueue
  ELSE IF Len(deferredNormalQueue) < DeferredNormalCapacity
       THEN Append(deferredNormalQueue, command)
       ELSE deferredNormalQueue

Init ==
  /\ runtimeQueue = <<AssemblyCommand>>
  /\ deferredNormalQueue = <<NormalFiller>>
  /\ bodyAssembled = FALSE
  /\ beginProposalOwned = FALSE

DispatchRuntimeHead ==
  LET command == Head(runtimeQueue)
      succeeds == CommandDispatchableWhileBusy(command)
  IN /\ Len(runtimeQueue) > 0
     /\ runtimeQueue' = Tail(runtimeQueue)
     /\ IF succeeds
        THEN /\ bodyAssembled' = TRUE
             /\ beginProposalOwned' = TRUE
             /\ UNCHANGED deferredNormalQueue
        ELSE /\ deferredNormalQueue' = DeferredNormalAfter(command)
             /\ UNCHANGED <<bodyAssembled, beginProposalOwned>>

Next == DispatchRuntimeHead

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(DispatchRuntimeHead)

TypeInvariant ==
  /\ runtimeQueue \in Seq({AssemblyCommand})
  /\ deferredNormalQueue \in Seq({AssemblyCommand, NormalFiller})
  /\ Len(deferredNormalQueue) <= DeferredNormalCapacity
  /\ bodyAssembled \in BOOLEAN
  /\ beginProposalOwned \in BOOLEAN

AssemblyOwned ==
  \/ SequenceContains(runtimeQueue, AssemblyCommand)
  \/ SequenceContains(deferredNormalQueue, AssemblyCommand)
  \/ bodyAssembled /\ beginProposalOwned

AssemblyOwnershipPreserved == AssemblyOwned

AssemblyEventuallyExecutes == <>bodyAssembled

=============================================================================
