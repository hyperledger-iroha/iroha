---- MODULE SumeragiV2DeferredCursorMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Bounded adversarial witness for the deferred-class selector.

The retired strict Completion-before-Progress selector can service a fresh
Completion on every fair service step while a producer replenishes that class
between steps.  The Progress owner therefore survives a fair lasso.  The
production cyclic cursor advances to Progress after servicing Completion, so
the same replenishment cannot overtake the already-owned Progress input.
***************************************************************************)

VARIABLES completionPending, progressOwned, nextClass

vars == <<completionPending, progressOwned, nextClass>>

Classes == {"Completion", "Progress", "Normal"}

NextClass(commandClass) ==
  CASE commandClass = "Completion" -> "Progress"
    [] commandClass = "Progress" -> "Normal"
    [] OTHER -> "Completion"

ClassNonempty(commandClass) ==
  CASE commandClass = "Completion" -> completionPending
    [] commandClass = "Progress" -> progressOwned
    [] OTHER -> FALSE

OldSelectedClass ==
  IF completionPending THEN "Completion" ELSE "Progress"

CyclicSelectedClass ==
  LET first == nextClass
      second == NextClass(first)
      third == NextClass(second)
  IN IF ClassNonempty(first)
     THEN first
     ELSE IF ClassNonempty(second) THEN second ELSE third

Init ==
  /\ completionPending = TRUE
  /\ progressOwned = TRUE
  /\ nextClass = "Completion"

ReplenishCompletion ==
  /\ ~completionPending
  /\ progressOwned
  /\ completionPending' = TRUE
  /\ UNCHANGED <<progressOwned, nextClass>>

OldStrictService ==
  /\ (completionPending \/ progressOwned)
  /\ IF OldSelectedClass = "Completion"
     THEN /\ completionPending' = FALSE
          /\ UNCHANGED progressOwned
     ELSE /\ progressOwned' = FALSE
          /\ UNCHANGED completionPending
  /\ UNCHANGED nextClass

CyclicService ==
  LET selected == CyclicSelectedClass
  IN /\ (completionPending \/ progressOwned)
     /\ IF selected = "Completion"
        THEN /\ completionPending' = FALSE
             /\ UNCHANGED progressOwned
        ELSE IF selected = "Progress"
             THEN /\ progressOwned' = FALSE
                  /\ UNCHANGED completionPending
             ELSE UNCHANGED <<completionPending, progressOwned>>
     /\ nextClass' = NextClass(selected)

OldNext == OldStrictService \/ ReplenishCompletion

CyclicNext == CyclicService \/ ReplenishCompletion

OldStrictSpec ==
  /\ Init
  /\ [][OldNext]_vars
  /\ WF_vars(OldStrictService)
  /\ WF_vars(ReplenishCompletion)

CyclicSpec ==
  /\ Init
  /\ [][CyclicNext]_vars
  /\ WF_vars(CyclicService)
  /\ WF_vars(ReplenishCompletion)

ProgressEventuallyServiced == progressOwned ~> ~progressOwned

=============================================================================
