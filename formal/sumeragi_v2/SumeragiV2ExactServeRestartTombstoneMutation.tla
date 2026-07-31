---- MODULE SumeragiV2ExactServeRestartTombstoneMutation ----
EXTENDS TLC, Naturals

(***************************************************************************
Finite mutation for same-height exact Serve restart reconstruction.

The drained request owns terminal ordinal 1.  Same-height replay must retain
that tombstone and the next ordinal; an exact retransmission then replays the
terminal outcome without recreating the Serve stage.  Only successor rollover
may reset the table.
***************************************************************************)

CONSTANT PreserveServeTombstoneOnRestart

ASSUME PreserveServeTombstoneOnRestart \in BOOLEAN

VARIABLES phase, nextOrdinal, tombstoneOwned, serveStageOwned

vars == <<phase, nextOrdinal, tombstoneOwned, serveStageOwned>>

TypeInvariant ==
  /\ phase \in {"Active", "Drained", "Restarted", "Retried"}
  /\ nextOrdinal \in 1..2
  /\ tombstoneOwned \in BOOLEAN
  /\ serveStageOwned \in BOOLEAN

SameHeightRestartPreservesServeHighWatermark ==
  phase = "Restarted"
    => /\ tombstoneOwned
       /\ nextOrdinal = 2

DrainedLogicalRequestCannotRecreateOldServeStage ==
  phase = "Retried" => ~serveStageOwned

Init ==
  /\ phase = "Active"
  /\ nextOrdinal = 2
  /\ ~tombstoneOwned
  /\ serveStageOwned

DrainRequest ==
  /\ phase = "Active"
  /\ phase' = "Drained"
  /\ tombstoneOwned'
  /\ ~serveStageOwned'
  /\ UNCHANGED nextOrdinal

RestartSameHeight ==
  /\ phase = "Drained"
  /\ phase' = "Restarted"
  /\ tombstoneOwned' =
       IF PreserveServeTombstoneOnRestart THEN tombstoneOwned ELSE FALSE
  /\ nextOrdinal' =
       IF PreserveServeTombstoneOnRestart THEN nextOrdinal ELSE 1
  /\ ~serveStageOwned'

RetransmitExactRequest ==
  /\ phase = "Restarted"
  /\ phase' = "Retried"
  /\ serveStageOwned' = ~tombstoneOwned
  /\ UNCHANGED <<nextOrdinal, tombstoneOwned>>

Next ==
  \/ DrainRequest
  \/ RestartSameHeight
  \/ RetransmitExactRequest

=============================================================================
