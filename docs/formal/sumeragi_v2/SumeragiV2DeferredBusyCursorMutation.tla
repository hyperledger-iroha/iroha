---- MODULE SumeragiV2DeferredBusyCursorMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Bounded adversarial witness for the production Busy branch of deferred
draining.  Completion is permanently Busy: popping and requeueing its head
does not change queue ownership.  busyAttemptParity records the observable
drain attempt so that the retired no-advance behavior forms an explicit fair
two-state lasso rather than relying on implicit temporal stuttering.

The repaired service advances the cyclic cursor even when Completion is
requeued.  The already-owned Progress command is therefore selected and
serviced on the next drain attempt.
***************************************************************************)

VARIABLES completionPending, progressOwned, nextClass, busyAttemptParity

vars ==
  <<completionPending, progressOwned, nextClass, busyAttemptParity>>

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
  /\ busyAttemptParity = FALSE

OldBusyService ==
  /\ completionPending \/ progressOwned
  /\ IF OldSelectedClass = "Completion"
     THEN /\ completionPending' = TRUE
          /\ UNCHANGED progressOwned
     ELSE /\ progressOwned' = FALSE
          /\ UNCHANGED completionPending
  /\ UNCHANGED nextClass
  /\ busyAttemptParity' = ~busyAttemptParity

CyclicBusyService ==
  LET selected == CyclicSelectedClass
  IN /\ completionPending \/ progressOwned
     /\ IF selected = "Completion"
        THEN /\ completionPending' = TRUE
             /\ UNCHANGED progressOwned
        ELSE IF selected = "Progress"
             THEN /\ progressOwned' = FALSE
                  /\ UNCHANGED completionPending
             ELSE UNCHANGED <<completionPending, progressOwned>>
     /\ nextClass' = NextClass(selected)
     /\ busyAttemptParity' = ~busyAttemptParity

OldBusySpec ==
  /\ Init
  /\ [][OldBusyService]_vars
  /\ WF_vars(OldBusyService)

CyclicBusySpec ==
  /\ Init
  /\ [][CyclicBusyService]_vars
  /\ WF_vars(CyclicBusyService)

ProgressEventuallyServiced == progressOwned ~> ~progressOwned

=============================================================================
