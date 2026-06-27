---- MODULE SumeragiIdleBacklogSignalsGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for idle backlog signal derivation.

This slice pins `idle_backlog_signals_for_height(...)`,
`IdleBacklogSignals::near_quorum_fast_timeout_gate_open(...)`, and
`Actor::stalled_pending_near_quorum_fast_timeout_gate_open(...)`. The raw
near-quorum queue signal follows only existing worker backlog, the raw
near-quorum RBC signal follows only the near-quorum RBC backlog predicate, and
residual round backlog is added to the derived near-quorum signals before the
instance fast-timeout gate is evaluated. The actor static helper uses the raw
signals plus an explicit residual check, so it must agree with the instance
gate while still ignoring recovery-only, queue-active-only, and unresolved-only
RBC backlog.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "no_backlog",
  "worker_backlog",
  "recovery_worker_only",
  "queue_active_only",
  "unresolved_rbc_only",
  "near_rbc_raw",
  "residual_only",
  "residual_with_raw",
  "worker_and_residual",
  "all_backlogs"
}

\* @type: Str => Bool;
ExistingWorker(c) ==
  c \in {"worker_backlog", "worker_and_residual", "all_backlogs"}

\* @type: Str => Bool;
WorkerRecovery(c) ==
  c \in {"recovery_worker_only", "all_backlogs"}

\* @type: Str => Bool;
QueueActive(c) ==
  c \in {"queue_active_only", "all_backlogs"}

\* @type: Str => Bool;
ResidualRound(c) ==
  c \in {"residual_only", "residual_with_raw", "worker_and_residual", "all_backlogs"}

\* @type: Str => Bool;
UnresolvedRbc(c) ==
  c \in {"unresolved_rbc_only", "near_rbc_raw", "residual_with_raw", "all_backlogs"}

\* @type: Str => Bool;
NearRbcRawInput(c) ==
  c \in {"near_rbc_raw", "residual_with_raw", "all_backlogs"}

\* @type: Str => Bool;
SpecNearQueueRaw(c) == ExistingWorker(c)

\* @type: Str => Bool;
SpecNearRbcRaw(c) == NearRbcRawInput(c)

\* @type: Str => Bool;
SpecConsensusQueueBacklog(c) == ExistingWorker(c) \/ ResidualRound(c)

\* @type: Str => Bool;
SpecRbcBacklog(c) == UnresolvedRbc(c) \/ ResidualRound(c)

\* @type: Str => Bool;
SpecNearQueueBacklog(c) == SpecNearQueueRaw(c) \/ ResidualRound(c)

\* @type: Str => Bool;
SpecNearRbcBacklog(c) == SpecNearRbcRaw(c) \/ ResidualRound(c)

\* @type: Str => Bool;
SpecMethodGate(c) == ~SpecNearQueueBacklog(c) /\ ~SpecNearRbcBacklog(c)

\* @type: Str => Bool;
SpecActorGate(c) == ~SpecNearQueueRaw(c) /\ ~SpecNearRbcRaw(c) /\ ~ResidualRound(c)

\* @type: Str => Bool;
ActualNearQueueRaw(c) ==
  CASE Bug = "near_queue_uses_recovery"
       /\ c = "recovery_worker_only" -> WorkerRecovery(c)
    [] OTHER -> SpecNearQueueRaw(c)

\* @type: Str => Bool;
ActualNearRbcRaw(c) ==
  CASE Bug = "near_rbc_uses_unresolved"
       /\ c = "unresolved_rbc_only" -> UnresolvedRbc(c)
    [] OTHER -> SpecNearRbcRaw(c)

\* @type: Str => Bool;
ActualConsensusQueueBacklog(c) ==
  CASE Bug = "drop_residual_consensus"
       /\ c = "residual_only" -> ExistingWorker(c)
    [] OTHER -> ExistingWorker(c) \/ ResidualRound(c)

\* @type: Str => Bool;
ActualRbcBacklog(c) ==
  CASE Bug = "drop_residual_rbc"
       /\ c = "residual_only" -> UnresolvedRbc(c)
    [] OTHER -> UnresolvedRbc(c) \/ ResidualRound(c)

\* @type: Str => Bool;
ActualNearQueueBacklog(c) ==
  CASE Bug = "drop_residual_near_queue"
       /\ c = "residual_only" -> ActualNearQueueRaw(c)
    [] OTHER -> ActualNearQueueRaw(c) \/ ResidualRound(c)

\* @type: Str => Bool;
ActualNearRbcBacklog(c) ==
  CASE Bug = "drop_residual_near_rbc"
       /\ c = "residual_only" -> ActualNearRbcRaw(c)
    [] OTHER -> ActualNearRbcRaw(c) \/ ResidualRound(c)

\* @type: Str => Bool;
ActualMethodGate(c) ==
  CASE Bug = "method_uses_raw_fields"
       /\ c = "residual_only" -> ~ActualNearQueueRaw(c) /\ ~ActualNearRbcRaw(c)
    [] Bug = "queue_active_closes_gate"
       /\ c = "queue_active_only" -> FALSE
    [] Bug = "recovery_worker_closes_gate"
       /\ c = "recovery_worker_only" -> FALSE
    [] OTHER -> ~ActualNearQueueBacklog(c) /\ ~ActualNearRbcBacklog(c)

\* @type: Str => Bool;
ActualActorGate(c) ==
  CASE Bug = "actor_ignores_residual"
       /\ c = "residual_only" -> ~ActualNearQueueRaw(c) /\ ~ActualNearRbcRaw(c)
    [] Bug = "actor_uses_unresolved_rbc"
       /\ c = "unresolved_rbc_only" -> ~ActualNearQueueRaw(c) /\ ~UnresolvedRbc(c) /\ ~ResidualRound(c)
    [] OTHER -> ~ActualNearQueueRaw(c) /\ ~ActualNearRbcRaw(c) /\ ~ResidualRound(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "drop_residual_consensus",
       "drop_residual_rbc",
       "drop_residual_near_queue",
       "drop_residual_near_rbc",
       "near_queue_uses_recovery",
       "near_rbc_uses_unresolved",
       "method_uses_raw_fields",
       "actor_ignores_residual",
       "actor_uses_unresolved_rbc",
       "queue_active_closes_gate",
       "recovery_worker_closes_gate"
     }
  /\ checked = 0

NearQueueRawMatchesSpec ==
  /\ \A c \in Cases:
       ActualNearQueueRaw(c) = SpecNearQueueRaw(c)

NearRbcRawMatchesSpec ==
  /\ \A c \in Cases:
       ActualNearRbcRaw(c) = SpecNearRbcRaw(c)

ConsensusBacklogMatchesSpec ==
  /\ \A c \in Cases:
       ActualConsensusQueueBacklog(c) = SpecConsensusQueueBacklog(c)

RbcBacklogMatchesSpec ==
  /\ \A c \in Cases:
       ActualRbcBacklog(c) = SpecRbcBacklog(c)

NearQueueBacklogMatchesSpec ==
  /\ \A c \in Cases:
       ActualNearQueueBacklog(c) = SpecNearQueueBacklog(c)

NearRbcBacklogMatchesSpec ==
  /\ \A c \in Cases:
       ActualNearRbcBacklog(c) = SpecNearRbcBacklog(c)

MethodGateMatchesSpec ==
  /\ \A c \in Cases:
       ActualMethodGate(c) = SpecMethodGate(c)

ActorGateMatchesSpec ==
  /\ \A c \in Cases:
       ActualActorGate(c) = SpecActorGate(c)

MethodActorGateAgreement ==
  /\ \A c \in Cases:
       SpecMethodGate(c) = SpecActorGate(c)

BenignBacklogGateAnchors ==
  /\ SpecMethodGate("recovery_worker_only")
  /\ SpecMethodGate("queue_active_only")
  /\ SpecMethodGate("unresolved_rbc_only")

ClosingBacklogGateAnchors ==
  /\ ~SpecMethodGate("worker_backlog")
  /\ ~SpecMethodGate("near_rbc_raw")
  /\ ~SpecMethodGate("residual_only")

ResidualBacklogAnchors ==
  /\ SpecConsensusQueueBacklog("residual_only")
  /\ SpecRbcBacklog("residual_only")
  /\ SpecNearQueueBacklog("residual_only")
  /\ SpecNearRbcBacklog("residual_only")
  /\ ~SpecNearQueueRaw("residual_only")
  /\ ~SpecNearRbcRaw("residual_only")

RawInputIsolationAnchors ==
  /\ ~SpecNearQueueRaw("recovery_worker_only")
  /\ ~SpecNearQueueRaw("queue_active_only")
  /\ ~SpecNearRbcRaw("unresolved_rbc_only")
  /\ SpecNearRbcRaw("near_rbc_raw")

IdleBacklogRawSignalExact ==
  /\ NearQueueRawMatchesSpec
  /\ NearRbcRawMatchesSpec
  /\ RawInputIsolationAnchors

IdleBacklogDerivedSignalExact ==
  /\ ConsensusBacklogMatchesSpec
  /\ RbcBacklogMatchesSpec
  /\ NearQueueBacklogMatchesSpec
  /\ NearRbcBacklogMatchesSpec
  /\ ResidualBacklogAnchors

IdleBacklogGateExact ==
  /\ MethodGateMatchesSpec
  /\ ActorGateMatchesSpec
  /\ MethodActorGateAgreement
  /\ BenignBacklogGateAnchors
  /\ ClosingBacklogGateAnchors

IdleBacklogSignalsExactness ==
  /\ IdleBacklogRawSignalExact
  /\ IdleBacklogDerivedSignalExact
  /\ IdleBacklogGateExact

IdleBacklogSignalsCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ IdleBacklogSignalsExactness

SafetyFast ==
  IdleBacklogSignalsExactness

BugDropResidualConsensus ==
  ActualConsensusQueueBacklog("residual_only") = SpecConsensusQueueBacklog("residual_only")

BugDropResidualRbc ==
  ActualRbcBacklog("residual_only") = SpecRbcBacklog("residual_only")

BugDropResidualNearQueue ==
  ActualNearQueueBacklog("residual_only") = SpecNearQueueBacklog("residual_only")

BugDropResidualNearRbc ==
  ActualNearRbcBacklog("residual_only") = SpecNearRbcBacklog("residual_only")

BugNearQueueUsesRecovery ==
  ActualNearQueueRaw("recovery_worker_only") = SpecNearQueueRaw("recovery_worker_only")

BugNearRbcUsesUnresolved ==
  ActualNearRbcRaw("unresolved_rbc_only") = SpecNearRbcRaw("unresolved_rbc_only")

BugMethodUsesRawFields ==
  ActualMethodGate("residual_only") = SpecMethodGate("residual_only")

BugActorIgnoresResidual ==
  ActualActorGate("residual_only") = SpecActorGate("residual_only")

BugActorUsesUnresolvedRbc ==
  ActualActorGate("unresolved_rbc_only") = SpecActorGate("unresolved_rbc_only")

BugQueueActiveClosesGate ==
  ActualMethodGate("queue_active_only") = SpecMethodGate("queue_active_only")

BugRecoveryWorkerClosesGate ==
  ActualMethodGate("recovery_worker_only") = SpecMethodGate("recovery_worker_only")

====
