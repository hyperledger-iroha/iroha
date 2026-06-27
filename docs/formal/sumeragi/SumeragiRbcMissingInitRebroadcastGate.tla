---- MODULE SumeragiRbcMissingInitRebroadcastGate ----
EXTENDS Naturals, Sequences

(***************************************************************************
A bounded abstract model for `rebroadcast_rbc_payload_for_missing_init(...)`.

This slice pins the high-level missing-INIT rebroadcast decision order:
observers, DA-disabled nodes, suppressed hot repair, sessions without payload
recovery, inactive rebroadcast windows, and non-exempt queue backpressure all
return before peer discovery; missing rosters return before targeted INIT
repair; targeted INIT repair `Requested` and `Waiting` outcomes short-circuit
the broad rebroadcast path; `Fallback` and `NotNeeded` continue to payload
bundle lookup; missing bundles do not rebroadcast; and eligible broad
rebroadcasts send exactly one INIT companion, all cached chunks, and the
current READY count to `rebroadcast_rbc_payload_bundle(...)`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Requested == "requested"
Waiting == "waiting"
Fallback == "fallback"
NotNeeded == "not_needed"

RepairOutcomes == {Requested, Waiting, Fallback, NotNeeded}

ObserverCase == "observer"
DaDisabledCase == "da_disabled"
HotRepairSuppressedCase == "hot_repair_suppressed"
NoPayloadRecoveryCase == "no_payload_recovery"
InactiveCase == "inactive"
BackpressureBlockedCase == "backpressure_blocked"
BackpressureExemptCase == "backpressure_exempt"
RosterMissingCase == "roster_missing"
RepairRequestedCase == "repair_requested"
RepairWaitingCase == "repair_waiting"
RepairFallbackMissingBundleCase == "repair_fallback_missing_bundle"
RepairNotNeededMissingBundleCase == "repair_not_needed_missing_bundle"
RepairFallbackRebroadcastCase == "repair_fallback_rebroadcast"
RepairNotNeededRebroadcastCase == "repair_not_needed_rebroadcast"

Cases == {
  ObserverCase,
  DaDisabledCase,
  HotRepairSuppressedCase,
  NoPayloadRecoveryCase,
  InactiveCase,
  BackpressureBlockedCase,
  BackpressureExemptCase,
  RosterMissingCase,
  RepairRequestedCase,
  RepairWaitingCase,
  RepairFallbackMissingBundleCase,
  RepairNotNeededMissingBundleCase,
  RepairFallbackRebroadcastCase,
  RepairNotNeededRebroadcastCase
}

Observer(c) ==
  c = ObserverCase

DaEnabled(c) ==
  c /= DaDisabledCase

HotRepairSuppressed(c) ==
  c = HotRepairSuppressedCase

AllowsPayloadRecovery(c) ==
  c /= NoPayloadRecoveryCase

RebroadcastActive(c) ==
  c /= InactiveCase

QueueBackpressure(c) ==
  c \in {BackpressureBlockedCase, BackpressureExemptCase}

BackpressureExempt(c) ==
  c = BackpressureExemptCase

RosterAvailable(c) ==
  c /= RosterMissingCase

RepairAttempt(c) ==
  CASE c = RepairRequestedCase -> Requested
    [] c = RepairWaitingCase -> Waiting
    [] c \in {
        RepairFallbackMissingBundleCase,
        RepairFallbackRebroadcastCase
      } -> Fallback
    [] OTHER -> NotNeeded

BundleAvailable(c) ==
  ~(c \in {RepairFallbackMissingBundleCase, RepairNotNeededMissingBundleCase})

ChunkCount(c) ==
  CASE c = BackpressureExemptCase -> 3
    [] c = RepairNotNeededRebroadcastCase -> 1
    [] OTHER -> 2

ReadyCount(c) ==
  CASE c = BackpressureExemptCase -> 3
    [] c = RepairNotNeededRebroadcastCase -> 1
    [] OTHER -> 2

Result(rebroadcast, repair_requested, repair_waiting, init_messages, chunk_messages, ready_forwarded) ==
  [
    rebroadcast |-> rebroadcast,
    repair_requested |-> repair_requested,
    repair_waiting |-> repair_waiting,
    init_messages |-> init_messages,
    chunk_messages |-> chunk_messages,
    ready_forwarded |-> ready_forwarded
  ]

NoSend ==
  Result(FALSE, FALSE, FALSE, 0, 0, 0)

BroadRebroadcast(c) ==
  Result(TRUE, FALSE, FALSE, 1, ChunkCount(c), ReadyCount(c))

SpecDecision(c) ==
  IF Observer(c)
     \/ ~DaEnabled(c)
     \/ HotRepairSuppressed(c)
     \/ ~AllowsPayloadRecovery(c)
     \/ ~RebroadcastActive(c)
  THEN
    NoSend
  ELSE IF QueueBackpressure(c) /\ ~BackpressureExempt(c) THEN
    NoSend
  ELSE IF ~RosterAvailable(c) THEN
    NoSend
  ELSE IF RepairAttempt(c) = Requested THEN
    Result(FALSE, TRUE, FALSE, 0, 0, 0)
  ELSE IF RepairAttempt(c) = Waiting THEN
    Result(FALSE, FALSE, TRUE, 0, 0, 0)
  ELSE IF ~BundleAvailable(c) THEN
    NoSend
  ELSE
    BroadRebroadcast(c)

ActualDecision(c) ==
  CASE Bug = "observer_rebroadcasts"
       /\ c = ObserverCase -> BroadRebroadcast(c)
    [] Bug = "da_disabled_rebroadcasts"
       /\ c = DaDisabledCase -> BroadRebroadcast(c)
    [] Bug = "hot_repair_suppressed_rebroadcasts"
       /\ c = HotRepairSuppressedCase -> BroadRebroadcast(c)
    [] Bug = "no_payload_recovery_rebroadcasts"
       /\ c = NoPayloadRecoveryCase -> BroadRebroadcast(c)
    [] Bug = "inactive_rebroadcasts"
       /\ c = InactiveCase -> BroadRebroadcast(c)
    [] Bug = "backpressure_rebroadcasts"
       /\ c = BackpressureBlockedCase -> BroadRebroadcast(c)
    [] Bug = "backpressure_exempt_blocked"
       /\ c = BackpressureExemptCase -> NoSend
    [] Bug = "roster_missing_rebroadcasts"
       /\ c = RosterMissingCase -> BroadRebroadcast(c)
    [] Bug = "requested_falls_through"
       /\ c = RepairRequestedCase -> BroadRebroadcast(c)
    [] Bug = "requested_not_recorded"
       /\ c = RepairRequestedCase -> NoSend
    [] Bug = "waiting_falls_through"
       /\ c = RepairWaitingCase -> BroadRebroadcast(c)
    [] Bug = "fallback_blocks_rebroadcast"
       /\ c = RepairFallbackRebroadcastCase -> NoSend
    [] Bug = "not_needed_blocks_rebroadcast"
       /\ c = RepairNotNeededRebroadcastCase -> NoSend
    [] Bug = "missing_bundle_rebroadcasts"
       /\ c = RepairFallbackMissingBundleCase -> BroadRebroadcast(c)
    [] Bug = "broad_omits_init"
       /\ c = RepairFallbackRebroadcastCase ->
         Result(TRUE, FALSE, FALSE, 0, ChunkCount(c), ReadyCount(c))
    [] Bug = "broad_drops_chunks"
       /\ c = RepairFallbackRebroadcastCase ->
         Result(TRUE, FALSE, FALSE, 1, 0, ReadyCount(c))
    [] Bug = "broad_wrong_ready_count"
       /\ c = RepairFallbackRebroadcastCase ->
         Result(TRUE, FALSE, FALSE, 1, ChunkCount(c), ReadyCount(c) + 1)
    [] OTHER -> SpecDecision(c)

BugSet == {
  "none",
  "observer_rebroadcasts",
  "da_disabled_rebroadcasts",
  "hot_repair_suppressed_rebroadcasts",
  "no_payload_recovery_rebroadcasts",
  "inactive_rebroadcasts",
  "backpressure_rebroadcasts",
  "backpressure_exempt_blocked",
  "roster_missing_rebroadcasts",
  "requested_falls_through",
  "requested_not_recorded",
  "waiting_falls_through",
  "fallback_blocks_rebroadcast",
  "not_needed_blocks_rebroadcast",
  "missing_bundle_rebroadcasts",
  "broad_omits_init",
  "broad_drops_chunks",
  "broad_wrong_ready_count"
}

Init ==
  checked = 0

Next ==
  \/ /\ checked < 17
     /\ checked' = checked + 1
  \/ /\ checked = 17
     /\ checked' = checked

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked \in 0..17
  /\ \A c \in Cases:
       LET r == ActualDecision(c) IN
       /\ r.rebroadcast \in BOOLEAN
       /\ r.repair_requested \in BOOLEAN
       /\ r.repair_waiting \in BOOLEAN
       /\ r.init_messages \in Nat
       /\ r.chunk_messages \in Nat
       /\ r.ready_forwarded \in Nat
  /\ \A c \in Cases:
       /\ RepairAttempt(c) \in RepairOutcomes
       /\ ChunkCount(c) \in Nat
       /\ ReadyCount(c) \in Nat

DecisionExact ==
  \A c \in Cases:
    ActualDecision(c) = SpecDecision(c)

GateStable ==
  /\ ~ActualDecision(ObserverCase).rebroadcast
  /\ ~ActualDecision(DaDisabledCase).rebroadcast
  /\ ~ActualDecision(HotRepairSuppressedCase).rebroadcast
  /\ ~ActualDecision(NoPayloadRecoveryCase).rebroadcast
  /\ ~ActualDecision(InactiveCase).rebroadcast
  /\ ~ActualDecision(BackpressureBlockedCase).rebroadcast
  /\ ActualDecision(BackpressureExemptCase).rebroadcast
  /\ ~ActualDecision(RosterMissingCase).rebroadcast
  /\ ActualDecision(RepairRequestedCase).repair_requested
  /\ ~ActualDecision(RepairRequestedCase).rebroadcast
  /\ ActualDecision(RepairWaitingCase).repair_waiting
  /\ ~ActualDecision(RepairWaitingCase).rebroadcast
  /\ ~ActualDecision(RepairFallbackMissingBundleCase).rebroadcast
  /\ ~ActualDecision(RepairNotNeededMissingBundleCase).rebroadcast
  /\ ActualDecision(RepairFallbackRebroadcastCase).rebroadcast
  /\ ActualDecision(RepairNotNeededRebroadcastCase).rebroadcast
  /\ ActualDecision(RepairFallbackRebroadcastCase).init_messages = 1
  /\ ActualDecision(RepairFallbackRebroadcastCase).chunk_messages = 2
  /\ ActualDecision(RepairFallbackRebroadcastCase).ready_forwarded = 2
  /\ ActualDecision(BackpressureExemptCase).chunk_messages = 3
  /\ ActualDecision(RepairNotNeededRebroadcastCase).ready_forwarded = 1

RbcMissingInitRebroadcastCoreSafety ==
  /\ DecisionExact
  /\ GateStable

SafetyFast ==
  RbcMissingInitRebroadcastCoreSafety

AllDecisionsMatchSpec ==
  \A c \in Cases:
    ActualDecision(c) = SpecDecision(c)

EarlyRejectAnchors ==
  /\ ActualDecision(ObserverCase) = NoSend
  /\ ActualDecision(DaDisabledCase) = NoSend
  /\ ActualDecision(HotRepairSuppressedCase) = NoSend
  /\ ActualDecision(NoPayloadRecoveryCase) = NoSend
  /\ ActualDecision(InactiveCase) = NoSend
  /\ ActualDecision(BackpressureBlockedCase) = NoSend
  /\ ActualDecision(RosterMissingCase) = NoSend

BackpressureExemptAnchors ==
  /\ ActualDecision(BackpressureExemptCase) =
       Result(TRUE, FALSE, FALSE, 1, 3, 3)

RepairAttemptAnchors ==
  /\ ActualDecision(RepairRequestedCase) =
       Result(FALSE, TRUE, FALSE, 0, 0, 0)
  /\ ActualDecision(RepairWaitingCase) =
       Result(FALSE, FALSE, TRUE, 0, 0, 0)

MissingBundleAnchors ==
  /\ ActualDecision(RepairFallbackMissingBundleCase) = NoSend
  /\ ActualDecision(RepairNotNeededMissingBundleCase) = NoSend

BroadRebroadcastAnchors ==
  /\ ActualDecision(RepairFallbackRebroadcastCase) =
       Result(TRUE, FALSE, FALSE, 1, 2, 2)
  /\ ActualDecision(RepairNotNeededRebroadcastCase) =
       Result(TRUE, FALSE, FALSE, 1, 1, 1)

SafetyAnchors ==
  /\ AllDecisionsMatchSpec
  /\ EarlyRejectAnchors
  /\ BackpressureExemptAnchors
  /\ RepairAttemptAnchors
  /\ MissingBundleAnchors
  /\ BroadRebroadcastAnchors

RbcMissingInitRebroadcastCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ SafetyFast
  /\ SafetyAnchors

====
