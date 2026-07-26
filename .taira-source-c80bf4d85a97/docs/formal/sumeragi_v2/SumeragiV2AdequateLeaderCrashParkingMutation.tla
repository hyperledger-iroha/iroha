---- MODULE SumeragiV2AdequateLeaderCrashParkingMutation ----
EXTENDS TLC

(***************************************************************************
Bounded negative model for responsive pre-GST crash parking.

The exact AssembleBody owner is executable initially.  A responsive crash
removes the process from `up` while preserving the scheduler carrier, just as
the full asynchronous model does before reset/replay.  The repaired derived
invariant classifies that owner as parked for preservation only.  The
historical mutation omits the parked arm and reports the two-state trace.
***************************************************************************)

CONSTANT RequireCrashParking

ASSUME RequireCrashParking \in BOOLEAN

Candidate == "exact-current-assemble-body-owner"

VARIABLES phase, up, scheduled

vars == <<phase, up, scheduled>>

NodeIdle == TRUE

CommandDispatchable ==
  /\ scheduled
  /\ up

ExactLeaderSchedulerParked ==
  /\ scheduled
  /\ ~up

ExactLeaderSchedulerOriginReadiness ==
  scheduled /\ NodeIdle
    => \/ CommandDispatchable
       \/ /\ RequireCrashParking
             /\ ExactLeaderSchedulerParked

TypeInvariant ==
  /\ phase \in {"Online", "Crashed"}
  /\ up \in BOOLEAN
  /\ scheduled \in BOOLEAN

Init ==
  /\ phase = "Online"
  /\ up = TRUE
  /\ scheduled = TRUE

ResponsiveCrash ==
  /\ phase = "Online"
  /\ phase' = "Crashed"
  /\ up' = FALSE
  /\ UNCHANGED scheduled

Next == ResponsiveCrash

=============================================================================
