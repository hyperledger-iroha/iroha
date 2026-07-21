---- MODULE SumeragiV2DeferredHandoffMutation ----
EXTENDS Naturals

(***************************************************************************
Bounded equal-rank re-Busy witness for the exact deferred handoff.

The Normal command is the held target, Progress is an always-available
foreign blocker, and Completion terminates the current Busy owner.  The
retired scheduler has a fair three-step lasso:

  Progress makes the idle reducer Busy;
  Normal is retried while Busy and remains queued;
  Completion terminates the local owner and returns the cursor to Progress.

The repaired scheduler records the exact Normal handoff on the Busy retry.
Completion is still allowed to run, but the next foreign Progress selection
is skipped while idle; it cannot create a successor Busy owner.  The cyclic
cursor then selects and consumes the exact held Normal command.
***************************************************************************)

VARIABLES targetPending, busy, nextClass, handoff

vars == <<targetPending, busy, nextClass, handoff>>

Classes == {"Completion", "Progress", "Normal"}

NextClass(commandClass) ==
  CASE commandClass = "Completion" -> "Progress"
    [] commandClass = "Progress" -> "Normal"
    [] OTHER -> "Completion"

ClassNonempty(commandClass) ==
  CASE commandClass = "Completion" -> busy
    [] commandClass = "Progress" -> TRUE
    [] OTHER -> targetPending

SelectedClass ==
  LET first == nextClass
      second == NextClass(first)
      third == NextClass(second)
  IN IF ClassNonempty(first)
     THEN first
     ELSE IF ClassNonempty(second) THEN second ELSE third

Init ==
  /\ targetPending = TRUE
  /\ busy = FALSE
  /\ nextClass = "Progress"
  /\ handoff = FALSE

OldDrain ==
  LET selected == SelectedClass
  IN /\ IF selected = "Completion"
        THEN /\ busy' = FALSE
             /\ UNCHANGED targetPending
        ELSE IF selected = "Progress"
             THEN /\ busy' = TRUE
                  /\ UNCHANGED targetPending
             ELSE IF busy
                  THEN UNCHANGED <<targetPending, busy>>
                  ELSE /\ targetPending' = FALSE
                       /\ busy' = FALSE
     /\ nextClass' = NextClass(selected)
     /\ handoff' = FALSE

HandoffDrain ==
  LET selected == SelectedClass
  IN /\ IF selected = "Completion"
        THEN /\ busy' = FALSE
             /\ UNCHANGED <<targetPending, handoff>>
        ELSE IF selected = "Progress"
             THEN /\ IF handoff /\ ~busy
                        THEN busy' = FALSE
                        ELSE busy' = TRUE
                  /\ UNCHANGED <<targetPending, handoff>>
             ELSE IF busy
                  THEN /\ UNCHANGED <<targetPending, busy>>
                       /\ handoff' = TRUE
                  ELSE /\ targetPending' = FALSE
                       /\ busy' = FALSE
                       /\ handoff' = FALSE
     /\ nextClass' = NextClass(selected)

OldSpec ==
  /\ Init
  /\ [][OldDrain]_vars
  /\ WF_vars(OldDrain)

HandoffSpec ==
  /\ Init
  /\ [][HandoffDrain]_vars
  /\ WF_vars(HandoffDrain)

HeldTargetEventuallyServed == targetPending ~> ~targetPending

=============================================================================
