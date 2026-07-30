---- MODULE SumeragiV2CausalDebtMutation ----
EXTENDS Naturals, Sequences, TLC

(***************************************************************************
Small deterministic mutation for the causal-admission reservation.  The old
policy records debt only after producer service and lets outer ingress,
producer completions, and Control I/O refill the slot just released by the
serialized runtime/worker.  The repaired policy records every observed causal
head, preserves that reservation until the head moves, and retains the exact
Completion producer exception needed to retire outstanding work.
***************************************************************************)

CONSTANTS Scenario, DebtThrottle

Scenarios ==
  {"ProgressIngress", "ProducerRefill", "NoProducer",
   "Duplicate", "Completion"}

ASSUME Scenario \in Scenarios
ASSUME DebtThrottle \in BOOLEAN

VARIABLES phase,
          commandDepth,
          causalClass,
          causalPresent,
          causalDuplicate,
          debt,
          producerReady,
          ingressReady,
          ioDepth,
          outstanding,
          controlReady,
          moved

vars ==
  <<phase, commandDepth, causalClass, causalPresent, causalDuplicate,
    debt, producerReady, ingressReady, ioDepth, outstanding,
    controlReady, moved>>

NormalLimit == 1
ProgressLimit == 2
CompletionLimit == 3
ConsensusIoLimit == 1
ControlIoLimit == 2
WorkLimit == 1

TypeInvariant ==
  /\ phase \in {"Local", "Ingress", "Runtime", "Done"}
  /\ commandDepth \in Nat
  /\ causalClass \in {"Normal", "Progress", "Completion"}
  /\ causalPresent \in BOOLEAN
  /\ causalDuplicate \in BOOLEAN
  /\ debt \in BOOLEAN
  /\ producerReady \in BOOLEAN
  /\ ingressReady \in BOOLEAN
  /\ ioDepth \in Nat
  /\ outstanding \in Nat
  /\ controlReady \in BOOLEAN
  /\ moved \in BOOLEAN

CommandLimit(commandClass) ==
  CASE commandClass = "Normal" -> NormalLimit
    [] commandClass = "Progress" -> ProgressLimit
    [] commandClass = "Completion" -> CompletionLimit

CanEnqueueCommand(commandClass) ==
  commandDepth < CommandLimit(commandClass)

CausalCanAdvance ==
  /\ causalPresent
  /\ \/ causalDuplicate
     \/ /\ causalClass = "Completion"
           /\ ioDepth < ConsensusIoLimit
           /\ outstanding < WorkLimit
     \/ /\ causalClass # "Completion"
           /\ CanEnqueueCommand(causalClass)

CausalDebtActive == debt /\ causalPresent

NonCompletionCausalDebt ==
  CausalDebtActive /\ causalClass # "Completion"

CompletionCausalDebt ==
  CausalDebtActive /\ causalClass = "Completion"

ProducerCanAdvance ==
  /\ producerReady
  /\ CanEnqueueCommand("Completion")
  /\ (~DebtThrottle \/ ~NonCompletionCausalDebt)

IngressCommandCanRefill ==
  /\ Scenario \in {"ProgressIngress", "NoProducer"}
  /\ ingressReady
  /\ CanEnqueueCommand("Progress")
  /\ (~DebtThrottle \/ ~NonCompletionCausalDebt)

IngressWorkCanRefill ==
  /\ Scenario = "Completion"
  /\ ingressReady
  /\ outstanding < WorkLimit
  /\ (~DebtThrottle \/ ~CompletionCausalDebt)

ControlCanRefill ==
  /\ Scenario = "Completion"
  /\ controlReady
  /\ ioDepth < ControlIoLimit
  /\ (~DebtThrottle \/ ~CompletionCausalDebt)

Init ==
  /\ phase = "Local"
  /\ commandDepth =
       IF Scenario = "Duplicate" THEN CompletionLimit ELSE NormalLimit
  /\ causalClass = IF Scenario = "Completion" THEN "Completion" ELSE "Normal"
  /\ causalPresent = TRUE
  /\ causalDuplicate = (Scenario = "Duplicate")
  /\ debt = (Scenario # "NoProducer")
  /\ producerReady = (Scenario \in {"ProducerRefill", "Completion"})
  /\ ingressReady =
       (Scenario \in {"ProgressIngress", "NoProducer", "Completion"})
  /\ ioDepth = IF (Scenario \in {"Duplicate", "Completion"}) THEN 1 ELSE 0
  /\ outstanding =
       IF (Scenario \in {"Duplicate", "Completion"}) THEN 1 ELSE 0
  /\ controlReady = TRUE
  /\ moved = FALSE

LocalStep ==
  /\ phase = "Local"
  /\ IF CausalCanAdvance
     THEN /\ phase' = "Done"
          /\ causalPresent' = FALSE
          /\ debt' = FALSE
          /\ moved' = TRUE
          /\ UNCHANGED <<commandDepth, causalClass, causalDuplicate,
                          producerReady, ingressReady, ioDepth, outstanding,
                          controlReady>>
     ELSE IF ProducerCanAdvance
          THEN /\ phase' = "Ingress"
               /\ commandDepth' = commandDepth + 1
               /\ outstanding' =
                    IF outstanding > 0 THEN outstanding - 1 ELSE 0
               /\ producerReady' = (Scenario = "ProducerRefill")
               /\ debt' = (debt \/ causalPresent)
               /\ UNCHANGED <<causalClass, causalPresent, causalDuplicate,
                               ingressReady, ioDepth, controlReady, moved>>
          ELSE /\ phase' = "Ingress"
               /\ debt' =
                    IF DebtThrottle THEN debt \/ causalPresent ELSE debt
               /\ UNCHANGED <<commandDepth, causalClass, causalPresent,
                               causalDuplicate, producerReady, ingressReady,
                               ioDepth, outstanding, controlReady, moved>>

IngressStep ==
  /\ phase = "Ingress"
  /\ phase' = "Runtime"
  /\ IF IngressCommandCanRefill
     THEN /\ commandDepth' = commandDepth + 1
          /\ UNCHANGED <<producerReady, outstanding>>
     ELSE IF IngressWorkCanRefill
          THEN /\ outstanding' = outstanding + 1
               /\ producerReady' = TRUE
               /\ UNCHANGED commandDepth
          ELSE UNCHANGED <<commandDepth, producerReady, outstanding>>
  /\ UNCHANGED <<causalClass, causalPresent, causalDuplicate, debt,
                  ingressReady, ioDepth, controlReady, moved>>

RuntimeStep ==
  /\ phase = "Runtime"
  /\ phase' = "Local"
  /\ commandDepth' = IF commandDepth > 0 THEN commandDepth - 1 ELSE 0
  /\ UNCHANGED <<causalClass, causalPresent, causalDuplicate, debt,
                  producerReady, ingressReady, ioDepth, outstanding,
                  controlReady, moved>>

IoWorkerStep ==
  /\ ioDepth > 0
  /\ ioDepth' = ioDepth - 1
  /\ controlReady' = TRUE
  /\ UNCHANGED <<phase, commandDepth, causalClass, causalPresent,
                  causalDuplicate, debt, producerReady, ingressReady,
                  outstanding, moved>>

ControlRefillStep ==
  /\ ControlCanRefill
  /\ ioDepth' = ioDepth + 1
  /\ controlReady' = FALSE
  /\ UNCHANGED <<phase, commandDepth, causalClass, causalPresent,
                  causalDuplicate, debt, producerReady, ingressReady,
                  outstanding, moved>>

Next ==
  \/ LocalStep
  \/ IngressStep
  \/ RuntimeStep
  \/ IoWorkerStep
  \/ ControlRefillStep

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(LocalStep)
  /\ WF_vars(IngressStep)
  /\ WF_vars(RuntimeStep)
  /\ WF_vars(IoWorkerStep)
  /\ WF_vars(ControlRefillStep)

CausalEventuallyMoves == <>moved

DebtAlwaysHasCausalOwner == [](debt => causalPresent)

NonCompletionDebtBlocksRefill ==
  [](NonCompletionCausalDebt
       => /\ ~ProducerCanAdvance
          /\ ~IngressCommandCanRefill)

CompletionDebtBlocksRefill ==
  [](CompletionCausalDebt
       => /\ ~IngressWorkCanRefill
          /\ ~ControlCanRefill)

=============================================================================
