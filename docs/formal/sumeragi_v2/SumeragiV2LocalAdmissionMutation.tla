---- MODULE SumeragiV2LocalAdmissionMutation ----
EXTENDS Naturals

(***************************************************************************
Small executable witness for the production local-admission boundary.  The
buggy configuration keeps producer-first selection even after causal debt is
recorded; the fixed configuration consumes the same debt/cursor state used by
SumeragiV2AsyncNetwork.  A capacity-one slot and a producer replenished after
every runtime service expose the original fair producer/runtime lasso.
***************************************************************************)

CONSTANT FairSelection

Sources == {"Producer", "Causal"}
Slots == {"Empty"} \cup Sources

VARIABLES phase,
          slot,
          producerReady,
          causalPending,
          nextLocalSource,
          causalAdmissionOwed,
          lastSelected,
          causalWasSelected

vars ==
  <<phase, slot, producerReady, causalPending, nextLocalSource,
    causalAdmissionOwed, lastSelected, causalWasSelected>>

OtherSource(source) ==
  IF source = "Producer" THEN "Causal" ELSE "Producer"

SourceCanAdmit(source) ==
  IF source = "Producer" THEN producerReady ELSE causalPending

PreferredSource ==
  IF causalAdmissionOwed
  THEN "Causal"
  ELSE nextLocalSource

FairSelectedSource ==
  IF SourceCanAdmit(PreferredSource)
  THEN PreferredSource
  ELSE OtherSource(PreferredSource)

BuggySelectedSource ==
  IF producerReady THEN "Producer" ELSE "Causal"

SelectedSource ==
  IF FairSelection THEN FairSelectedSource ELSE BuggySelectedSource

Init ==
  /\ phase = "Local"
  /\ slot = "Empty"
  /\ producerReady = TRUE
  /\ causalPending = TRUE
  /\ nextLocalSource = "Producer"
  /\ causalAdmissionOwed = FALSE
  /\ lastSelected = "None"
  /\ causalWasSelected = FALSE

LocalStep ==
  /\ phase = "Local"
  /\ slot = "Empty"
  /\ (producerReady \/ causalPending)
  /\ SourceCanAdmit(SelectedSource)
  /\ slot' = SelectedSource
  /\ lastSelected' = SelectedSource
  /\ causalWasSelected' =
       (causalWasSelected \/ SelectedSource = "Causal")
  /\ causalPending' =
       IF SelectedSource = "Causal" THEN FALSE ELSE causalPending
  /\ producerReady' =
       IF SelectedSource = "Producer" THEN FALSE ELSE producerReady
  /\ nextLocalSource' = OtherSource(SelectedSource)
  /\ causalAdmissionOwed' =
       IF SelectedSource = "Causal"
       THEN FALSE
       ELSE causalAdmissionOwed \/ causalPending
  /\ phase' = "Runtime"

RuntimeStep ==
  /\ phase = "Runtime"
  /\ slot \in Sources
  /\ phase' = "Local"
  /\ slot' = "Empty"
  /\ producerReady' = TRUE
  /\ UNCHANGED <<causalPending, nextLocalSource,
                  causalAdmissionOwed, lastSelected,
                  causalWasSelected>>

Next == LocalStep \/ RuntimeStep

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(LocalStep)
  /\ WF_vars(RuntimeStep)

TypeInvariant ==
  /\ FairSelection \in BOOLEAN
  /\ phase \in {"Local", "Runtime"}
  /\ slot \in Slots
  /\ producerReady \in BOOLEAN
  /\ causalPending \in BOOLEAN
  /\ nextLocalSource \in Sources
  /\ causalAdmissionOwed \in BOOLEAN
  /\ lastSelected \in Sources \cup {"None"}
  /\ causalWasSelected \in BOOLEAN

DebtImpliesCausalPending ==
  causalAdmissionOwed => causalPending

CausalClearanceHasWitness ==
  ~causalPending => causalWasSelected

CausalAdmissionProgress ==
  causalPending ~> ~causalPending

=============================================================================
