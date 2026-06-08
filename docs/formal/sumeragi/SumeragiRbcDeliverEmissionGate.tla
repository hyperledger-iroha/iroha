---- MODULE SumeragiRbcDeliverEmissionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `maybe_emit_rbc_deliver_at_with_local_ready_bypass(...)`.

This slice captures the top-level RBC DELIVER emission decision, assuming the
lower-level helpers are modeled separately.  It pins the order and observable
side effects around terminal sessions, local hydration reloads, roster and
payload gates, local READY preconditions, chunk-root mismatch handling, READY
quorum deferral, authoritative local READY bypass, builder failure, committed
delivery suppression, targeted DELIVER fallback, broadcast, recovery, and
status/backlog publication.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Noop == "noop"
ClearTerminal == "clear_terminal"
DeferRoster == "defer_roster"
DeferUnverified == "defer_unverified"
DeferPayload == "defer_payload"
WaitLocalReady == "wait_local_ready"
InvalidateRoot == "invalidate_root"
DeferReady == "defer_ready"
WaitBuilder == "wait_builder"
Deliver == "deliver"

Actions == {
  Noop,
  ClearTerminal,
  DeferRoster,
  DeferUnverified,
  DeferPayload,
  WaitLocalReady,
  InvalidateRoot,
  DeferReady,
  WaitBuilder,
  Deliver
}

NoRequest == "none"
FrontierFetch == "frontier_fetch"
MetadataFetch == "metadata_fetch"

Requests == {NoRequest, FrontierFetch, MetadataFetch}

Retired == "retired"
NoSession == "no_session"
AlreadyDelivered == "already_delivered"
InvalidSession == "invalid_session"
HydrationReloadMissing == "hydration_reload_missing"
HydrationReloadDelivered == "hydration_reload_delivered"
HydrationReloadInvalid == "hydration_reload_invalid"
RosterMissing == "roster_missing"
UnverifiedNoEvidence == "unverified_no_evidence"
UnverifiedWithEvidence == "unverified_with_evidence"
PayloadMissingNoRepair == "payload_missing_no_repair"
PayloadMissingForceFetch == "payload_missing_force_fetch"
PayloadMissingMetadataFetch == "payload_missing_metadata_fetch"
LocalReadyNotSent == "local_ready_not_sent"
ChunkRootMismatch == "chunk_root_mismatch"
RootAdoptedDeliver == "root_adopted_deliver"
ReadyBelowQuorum == "ready_below_quorum"
LocalBypassDeliver == "local_bypass_deliver"
LocalBypassNoReadyBlocked == "local_bypass_no_ready_blocked"
BuildDeliverMissing == "build_deliver_missing"
DeliverCommittedSuppressed == "deliver_committed_suppressed"
DeliverReadyRepairSent == "deliver_ready_repair_sent"
DeliverNoReadyRepair == "deliver_no_ready_repair"

Cases == {
  Retired,
  NoSession,
  AlreadyDelivered,
  InvalidSession,
  HydrationReloadMissing,
  HydrationReloadDelivered,
  HydrationReloadInvalid,
  RosterMissing,
  UnverifiedNoEvidence,
  UnverifiedWithEvidence,
  PayloadMissingNoRepair,
  PayloadMissingForceFetch,
  PayloadMissingMetadataFetch,
  LocalReadyNotSent,
  ChunkRootMismatch,
  RootAdoptedDeliver,
  ReadyBelowQuorum,
  LocalBypassDeliver,
  LocalBypassNoReadyBlocked,
  BuildDeliverMissing,
  DeliverCommittedSuppressed,
  DeliverReadyRepairSent,
  DeliverNoReadyRepair
}

Result(
  action,
  session_inserted,
  ready_deferral_removed,
  deliver_deferral_removed,
  deferral_logged,
  missing_block_request,
  payload_rebroadcast,
  ready_rebroadcast,
  missing_ready_rescue_called,
  root_invalidated,
  expected_root_set,
  status_persisted,
  backlog_published,
  deliver_recorded,
  telemetry_incremented,
  availability_driven,
  targeted_deliver_sent,
  broadcast_deliver,
  recover_block,
  process_commit
) ==
  [
    action |-> action,
    session_inserted |-> session_inserted,
    ready_deferral_removed |-> ready_deferral_removed,
    deliver_deferral_removed |-> deliver_deferral_removed,
    deferral_logged |-> deferral_logged,
    missing_block_request |-> missing_block_request,
    payload_rebroadcast |-> payload_rebroadcast,
    ready_rebroadcast |-> ready_rebroadcast,
    missing_ready_rescue_called |-> missing_ready_rescue_called,
    root_invalidated |-> root_invalidated,
    expected_root_set |-> expected_root_set,
    status_persisted |-> status_persisted,
    backlog_published |-> backlog_published,
    deliver_recorded |-> deliver_recorded,
    telemetry_incremented |-> telemetry_incremented,
    availability_driven |-> availability_driven,
    targeted_deliver_sent |-> targeted_deliver_sent,
    broadcast_deliver |-> broadcast_deliver,
    recover_block |-> recover_block,
    process_commit |-> process_commit
  ]

NoopResult ==
  Result(Noop, FALSE, FALSE, FALSE, FALSE, NoRequest, FALSE, FALSE, FALSE,
    FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)

TerminalResult ==
  Result(ClearTerminal, TRUE, TRUE, TRUE, FALSE, NoRequest, FALSE, FALSE, FALSE,
    FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)

RosterMissingResult ==
  Result(DeferRoster, TRUE, FALSE, FALSE, TRUE, NoRequest, FALSE, FALSE, FALSE,
    FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)

UnverifiedResult(payload, ready) ==
  Result(DeferUnverified, TRUE, FALSE, FALSE, TRUE, NoRequest, payload, ready,
    FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE,
    FALSE, FALSE)

PayloadMissingResult(request) ==
  Result(DeferPayload, TRUE, FALSE, FALSE, TRUE, request, TRUE, TRUE, FALSE,
    FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)

WaitLocalReadyResult ==
  Result(WaitLocalReady, TRUE, FALSE, FALSE, FALSE, NoRequest, FALSE, FALSE,
    FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE,
    FALSE, FALSE)

InvalidateRootResult ==
  Result(InvalidateRoot, TRUE, FALSE, TRUE, FALSE, NoRequest, FALSE, FALSE,
    FALSE, TRUE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE,
    FALSE)

DeferReadyResult ==
  Result(DeferReady, TRUE, FALSE, FALSE, TRUE, NoRequest, FALSE, FALSE, TRUE,
    FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)

WaitBuilderResult ==
  Result(WaitBuilder, TRUE, FALSE, FALSE, FALSE, NoRequest, FALSE, FALSE,
    FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE,
    FALSE, FALSE)

DeliverResult(committed, ready_repair_sent, root_set) ==
  Result(Deliver, TRUE, FALSE, TRUE, FALSE, NoRequest, FALSE, FALSE, TRUE,
    FALSE, root_set, TRUE, TRUE, TRUE, TRUE, TRUE,
    ~committed /\ ~ready_repair_sent,
    ~committed,
    ~committed,
    ~committed)

SpecDecision(c) ==
  CASE c \in {Retired, NoSession, HydrationReloadMissing} -> NoopResult
    [] c \in {AlreadyDelivered, InvalidSession, HydrationReloadDelivered,
        HydrationReloadInvalid} -> TerminalResult
    [] c = RosterMissing -> RosterMissingResult
    [] c = UnverifiedNoEvidence -> UnverifiedResult(FALSE, FALSE)
    [] c = UnverifiedWithEvidence -> UnverifiedResult(TRUE, TRUE)
    [] c = PayloadMissingNoRepair -> PayloadMissingResult(NoRequest)
    [] c = PayloadMissingForceFetch -> PayloadMissingResult(FrontierFetch)
    [] c = PayloadMissingMetadataFetch -> PayloadMissingResult(MetadataFetch)
    [] c = LocalReadyNotSent -> WaitLocalReadyResult
    [] c = ChunkRootMismatch -> InvalidateRootResult
    [] c = ReadyBelowQuorum -> DeferReadyResult
    [] c = LocalBypassNoReadyBlocked -> DeferReadyResult
    [] c = BuildDeliverMissing -> WaitBuilderResult
    [] c = RootAdoptedDeliver -> DeliverResult(FALSE, FALSE, TRUE)
    [] c = LocalBypassDeliver -> DeliverResult(FALSE, FALSE, FALSE)
    [] c = DeliverCommittedSuppressed -> DeliverResult(TRUE, FALSE, FALSE)
    [] c = DeliverReadyRepairSent -> DeliverResult(FALSE, TRUE, FALSE)
    [] c = DeliverNoReadyRepair -> DeliverResult(FALSE, FALSE, FALSE)
    [] OTHER -> NoopResult

ActualDecision(c) ==
  CASE Bug = "retired_emits"
       /\ c = Retired -> DeliverResult(FALSE, FALSE, FALSE)
    [] Bug = "missing_session_inserts"
       /\ c = NoSession -> WaitLocalReadyResult
    [] Bug = "terminal_keeps_deferrals"
       /\ c = AlreadyDelivered ->
         [SpecDecision(c) EXCEPT !.ready_deferral_removed = FALSE,
          !.deliver_deferral_removed = FALSE]
    [] Bug = "terminal_drops_session"
       /\ c = InvalidSession -> NoopResult
    [] Bug = "hydration_missing_reinserts"
       /\ c = HydrationReloadMissing -> WaitLocalReadyResult
    [] Bug = "hydration_terminal_keeps_deferrals"
       /\ c = HydrationReloadDelivered ->
         [SpecDecision(c) EXCEPT !.ready_deferral_removed = FALSE,
          !.deliver_deferral_removed = FALSE]
    [] Bug = "roster_missing_delivers"
       /\ c = RosterMissing -> DeliverResult(FALSE, FALSE, FALSE)
    [] Bug = "unverified_delivers"
       /\ c = UnverifiedNoEvidence -> DeliverResult(FALSE, FALSE, FALSE)
    [] Bug = "unverified_skips_payload_rebroadcast"
       /\ c = UnverifiedWithEvidence ->
         [SpecDecision(c) EXCEPT !.payload_rebroadcast = FALSE]
    [] Bug = "unverified_skips_ready_rebroadcast"
       /\ c = UnverifiedWithEvidence ->
         [SpecDecision(c) EXCEPT !.ready_rebroadcast = FALSE]
    [] Bug = "payload_missing_delivers"
       /\ c = PayloadMissingNoRepair -> DeliverResult(FALSE, FALSE, FALSE)
    [] Bug = "payload_missing_skip_force_fetch"
       /\ c = PayloadMissingForceFetch ->
         [SpecDecision(c) EXCEPT !.missing_block_request = NoRequest]
    [] Bug = "payload_missing_skip_metadata_fetch"
       /\ c = PayloadMissingMetadataFetch ->
         [SpecDecision(c) EXCEPT !.missing_block_request = NoRequest]
    [] Bug = "payload_missing_skips_payload_rebroadcast"
       /\ c = PayloadMissingNoRepair ->
         [SpecDecision(c) EXCEPT !.payload_rebroadcast = FALSE]
    [] Bug = "payload_missing_skips_ready_rebroadcast"
       /\ c = PayloadMissingNoRepair ->
         [SpecDecision(c) EXCEPT !.ready_rebroadcast = FALSE]
    [] Bug = "sent_ready_false_delivers"
       /\ c = LocalReadyNotSent -> DeliverResult(FALSE, FALSE, FALSE)
    [] Bug = "root_mismatch_delivers"
       /\ c = ChunkRootMismatch -> DeliverResult(FALSE, FALSE, FALSE)
    [] Bug = "root_mismatch_not_invalidated"
       /\ c = ChunkRootMismatch ->
         [SpecDecision(c) EXCEPT !.root_invalidated = FALSE]
    [] Bug = "root_mismatch_keeps_deferral"
       /\ c = ChunkRootMismatch ->
         [SpecDecision(c) EXCEPT !.deliver_deferral_removed = FALSE]
    [] Bug = "root_adoption_skipped"
       /\ c = RootAdoptedDeliver ->
         [SpecDecision(c) EXCEPT !.expected_root_set = FALSE]
    [] Bug = "ready_quorum_ignored"
       /\ c = ReadyBelowQuorum -> DeliverResult(FALSE, FALSE, FALSE)
    [] Bug = "local_bypass_ignored"
       /\ c = LocalBypassDeliver -> DeferReadyResult
    [] Bug = "local_bypass_without_ready_allowed"
       /\ c = LocalBypassNoReadyBlocked -> DeliverResult(FALSE, FALSE, FALSE)
    [] Bug = "builder_missing_delivers"
       /\ c = BuildDeliverMissing -> DeliverResult(FALSE, FALSE, FALSE)
    [] Bug = "committed_broadcasts"
       /\ c = DeliverCommittedSuppressed ->
         [SpecDecision(c) EXCEPT !.targeted_deliver_sent = TRUE,
          !.broadcast_deliver = TRUE, !.recover_block = TRUE,
          !.process_commit = TRUE]
    [] Bug = "ready_repair_targeted_deliver_not_skipped"
       /\ c = DeliverReadyRepairSent ->
         [SpecDecision(c) EXCEPT !.targeted_deliver_sent = TRUE]
    [] Bug = "deliver_omits_broadcast"
       /\ c = DeliverNoReadyRepair ->
         [SpecDecision(c) EXCEPT !.broadcast_deliver = FALSE]
    [] Bug = "deliver_keeps_deferral"
       /\ c = DeliverNoReadyRepair ->
         [SpecDecision(c) EXCEPT !.deliver_deferral_removed = FALSE]
    [] Bug = "deliver_skips_recover"
       /\ c = DeliverNoReadyRepair ->
         [SpecDecision(c) EXCEPT !.recover_block = FALSE,
          !.process_commit = FALSE]
    [] Bug = "deliver_skips_availability"
       /\ c = DeliverNoReadyRepair ->
         [SpecDecision(c) EXCEPT !.availability_driven = FALSE]
    [] Bug = "deliver_skips_status_persist"
       /\ c = DeliverNoReadyRepair ->
         [SpecDecision(c) EXCEPT !.status_persisted = FALSE]
    [] OTHER -> SpecDecision(c)

BugSet == {
  "none",
  "retired_emits",
  "missing_session_inserts",
  "terminal_keeps_deferrals",
  "terminal_drops_session",
  "hydration_missing_reinserts",
  "hydration_terminal_keeps_deferrals",
  "roster_missing_delivers",
  "unverified_delivers",
  "unverified_skips_payload_rebroadcast",
  "unverified_skips_ready_rebroadcast",
  "payload_missing_delivers",
  "payload_missing_skip_force_fetch",
  "payload_missing_skip_metadata_fetch",
  "payload_missing_skips_payload_rebroadcast",
  "payload_missing_skips_ready_rebroadcast",
  "sent_ready_false_delivers",
  "root_mismatch_delivers",
  "root_mismatch_not_invalidated",
  "root_mismatch_keeps_deferral",
  "root_adoption_skipped",
  "ready_quorum_ignored",
  "local_bypass_ignored",
  "local_bypass_without_ready_allowed",
  "builder_missing_delivers",
  "committed_broadcasts",
  "ready_repair_targeted_deliver_not_skipped",
  "deliver_omits_broadcast",
  "deliver_keeps_deferral",
  "deliver_skips_recover",
  "deliver_skips_availability",
  "deliver_skips_status_persist"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0
  /\ \A c \in Cases:
       LET r == ActualDecision(c) IN
       /\ r.action \in Actions
       /\ r.session_inserted \in BOOLEAN
       /\ r.ready_deferral_removed \in BOOLEAN
       /\ r.deliver_deferral_removed \in BOOLEAN
       /\ r.deferral_logged \in BOOLEAN
       /\ r.missing_block_request \in Requests
       /\ r.payload_rebroadcast \in BOOLEAN
       /\ r.ready_rebroadcast \in BOOLEAN
       /\ r.missing_ready_rescue_called \in BOOLEAN
       /\ r.root_invalidated \in BOOLEAN
       /\ r.expected_root_set \in BOOLEAN
       /\ r.status_persisted \in BOOLEAN
       /\ r.backlog_published \in BOOLEAN
       /\ r.deliver_recorded \in BOOLEAN
       /\ r.telemetry_incremented \in BOOLEAN
       /\ r.availability_driven \in BOOLEAN
       /\ r.targeted_deliver_sent \in BOOLEAN
       /\ r.broadcast_deliver \in BOOLEAN
       /\ r.recover_block \in BOOLEAN
       /\ r.process_commit \in BOOLEAN

DecisionExact ==
  \A c \in Cases:
    ActualDecision(c) = SpecDecision(c)

GateStable ==
  /\ ActualDecision(Retired).action = Noop
  /\ ActualDecision(AlreadyDelivered).ready_deferral_removed
  /\ ActualDecision(InvalidSession).deliver_deferral_removed
  /\ ActualDecision(RosterMissing).deferral_logged
  /\ ActualDecision(UnverifiedWithEvidence).payload_rebroadcast
  /\ ActualDecision(UnverifiedWithEvidence).ready_rebroadcast
  /\ ActualDecision(PayloadMissingForceFetch).missing_block_request = FrontierFetch
  /\ ActualDecision(PayloadMissingMetadataFetch).missing_block_request = MetadataFetch
  /\ ActualDecision(LocalReadyNotSent).action = WaitLocalReady
  /\ ActualDecision(ChunkRootMismatch).root_invalidated
  /\ ActualDecision(ChunkRootMismatch).deliver_deferral_removed
  /\ ActualDecision(RootAdoptedDeliver).expected_root_set
  /\ ActualDecision(ReadyBelowQuorum).action = DeferReady
  /\ ActualDecision(LocalBypassDeliver).action = Deliver
  /\ ActualDecision(LocalBypassNoReadyBlocked).action = DeferReady
  /\ ActualDecision(BuildDeliverMissing).action = WaitBuilder
  /\ ~ActualDecision(DeliverCommittedSuppressed).broadcast_deliver
  /\ ~ActualDecision(DeliverReadyRepairSent).targeted_deliver_sent
  /\ ActualDecision(DeliverNoReadyRepair).targeted_deliver_sent
  /\ ActualDecision(DeliverNoReadyRepair).broadcast_deliver
  /\ ActualDecision(DeliverNoReadyRepair).recover_block
  /\ ActualDecision(DeliverNoReadyRepair).process_commit

RbcDeliverEmissionCoreSafety ==
  /\ DecisionExact
  /\ GateStable

SafetyFast ==
  RbcDeliverEmissionCoreSafety

====
