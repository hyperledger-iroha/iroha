---- MODULE SumeragiV2HistoricalProducerContinuationMutation ----
EXTENDS TLC

(***************************************************************************
Finite mutation for a producer continuation owned by historical recovery.

The representative owner is responsive and belongs to the historical
recovery target set, but is deliberately outside the frozen voter set.  The
ordinary voter resolver therefore cannot service its Reserved or Materialized
continuation.  The repaired model lets the already-fair historical runner
perform one resolution-only turn and publish the terminal tombstone.  The
mutation restores the former RunNodeWork guard: an active continuation
disables the historical runner, so its weak fairness is vacuous and the
continuation can stutter forever.
***************************************************************************)

CONSTANT HistoricalRunnerResolvesContinuation

ASSUME HistoricalRunnerResolvesContinuation \in BOOLEAN

FrozenVoters == {"voter-a", "voter-b", "voter-c", "voter-d"}
HistoricalOwner == "historical-peer"
HistoricalRecoveryTargets == {HistoricalOwner}
ActiveStatuses == {"Reserved", "Materialized"}

VARIABLE continuationStatus

vars == <<continuationStatus>>

TypeInvariant ==
  continuationStatus \in ActiveStatuses \cup {"Terminal"}

HistoricalOwnerIsOutsideFrozenVoters ==
  /\ HistoricalOwner \in HistoricalRecoveryTargets
  /\ HistoricalOwner \notin FrozenVoters

Init ==
  continuationStatus \in ActiveStatuses

\* This is the ordinary producer-continuation fairness owner.  It is
\* intentionally unavailable for the historical non-voter.
VoterContinuationResolver ==
  /\ HistoricalOwner \in FrozenVoters
  /\ continuationStatus \in ActiveStatuses
  /\ continuationStatus' = "Terminal"

\* With the repair enabled, the historical runner acknowledges the selected
\* handoff and terminalizes it in the same serialized turn.  With the
\* mutation enabled, the old continuation-required guard disables the runner.
HistoricalRecoveryRunner ==
  /\ HistoricalOwner \in HistoricalRecoveryTargets
  /\ continuationStatus \in ActiveStatuses
  /\ HistoricalRunnerResolvesContinuation
  /\ continuationStatus' = "Terminal"

Next ==
  \/ VoterContinuationResolver
  \/ HistoricalRecoveryRunner

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(VoterContinuationResolver)
  /\ WF_vars(HistoricalRecoveryRunner)

HistoricalContinuationReachesTerminal ==
  <>(continuationStatus = "Terminal")

=============================================================================
