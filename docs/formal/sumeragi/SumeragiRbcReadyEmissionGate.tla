---- MODULE SumeragiRbcReadyEmissionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `maybe_emit_rbc_ready(...)`.

This slice pins the local READY emission contract: retired and missing sessions
return without side effects; invalid or hydration-invalid sessions clear READY
deferral without publishing fresh status; already-sent READY synchronizes
progress and schedules the normal DELIVER follow-up without rebroadcasting
READY; missing or unauthoritative rosters defer with the matching reason;
missing authoritative payload requests missing-block repair when applicable and
otherwise defers; enough remote READY evidence may bypass missing-payload
deferral; chunk-root mismatches invalidate the session, clear pending RBC, and
stop before DELIVER follow-up; computed chunk roots are adopted when the
expected root is absent; missing roots defer; successful READY records the
local signature, removes deferral, broadcasts READY, optionally broadcasts a
debug forked READY, and runs the local-ready DELIVER path; observers mark READY
sent without broadcasting; local-not-in-roster defers; and a builder miss with
local roster membership does not fabricate READY.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoReason == "none"
CommitRosterMissing == "commit_roster_missing"
CommitRosterUnverified == "commit_roster_unverified"
MissingPayload == "missing_payload"
ChunkRootMissing == "chunk_root_missing"
LocalNotInCommitTopology == "local_not_in_commit_topology"

Reasons == {
  NoReason,
  CommitRosterMissing,
  CommitRosterUnverified,
  MissingPayload,
  ChunkRootMissing,
  LocalNotInCommitTopology
}

Retired == "retired"
MissingSession == "missing_session"
InvalidSession == "invalid_session"
HydratedInvalid == "hydrated_invalid"
AlreadySentReady == "already_sent_ready"
RosterMissing == "roster_missing"
RosterUnverified == "roster_unverified"
MissingPayloadRequest == "missing_payload_request"
MissingPayloadNoRequest == "missing_payload_no_request"
ReadyRelayBypass == "ready_relay_bypass"
ChunkRootMismatch == "chunk_root_mismatch"
ChunkRootAdopted == "chunk_root_adopted"
ChunkRootMissingCase == "chunk_root_missing"
BuildReady == "build_ready"
BuildReadyFork == "build_ready_fork"
ObserverMarksSent == "observer_marks_sent"
LocalNotInRoster == "local_not_in_roster"
BuilderMissingLocal == "builder_missing_local"

Cases == {
  Retired,
  MissingSession,
  InvalidSession,
  HydratedInvalid,
  AlreadySentReady,
  RosterMissing,
  RosterUnverified,
  MissingPayloadRequest,
  MissingPayloadNoRequest,
  ReadyRelayBypass,
  ChunkRootMismatch,
  ChunkRootAdopted,
  ChunkRootMissingCase,
  BuildReady,
  BuildReadyFork,
  ObserverMarksSent,
  LocalNotInRoster,
  BuilderMissingLocal
}

Decision(
  status_persisted,
  backlog_published,
  sync_progress,
  ready_deferral_removed,
  ready_deferral_reason,
  missing_block_requested,
  invalidated,
  pending_cleared,
  chunk_root_adopted,
  sent_ready,
  ready_recorded,
  ready_broadcast,
  fork_broadcast,
  deliver_attempt,
  deliver_after_local_ready
) ==
  [
    status_persisted |-> status_persisted,
    backlog_published |-> backlog_published,
    sync_progress |-> sync_progress,
    ready_deferral_removed |-> ready_deferral_removed,
    ready_deferral_reason |-> ready_deferral_reason,
    missing_block_requested |-> missing_block_requested,
    invalidated |-> invalidated,
    pending_cleared |-> pending_cleared,
    chunk_root_adopted |-> chunk_root_adopted,
    sent_ready |-> sent_ready,
    ready_recorded |-> ready_recorded,
    ready_broadcast |-> ready_broadcast,
    fork_broadcast |-> fork_broadcast,
    deliver_attempt |-> deliver_attempt,
    deliver_after_local_ready |-> deliver_after_local_ready
  ]

NoDecision ==
  Decision(FALSE, FALSE, FALSE, FALSE, NoReason, FALSE, FALSE, FALSE, FALSE,
    FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)

Persisted(reason) ==
  Decision(TRUE, TRUE, FALSE, FALSE, reason, FALSE, FALSE, FALSE, FALSE,
    FALSE, FALSE, FALSE, FALSE, TRUE, FALSE)

ReadySuccess(forked, adopted) ==
  Decision(TRUE, TRUE, TRUE, TRUE, NoReason, FALSE, FALSE, FALSE, adopted,
    TRUE, TRUE, TRUE, forked, FALSE, TRUE)

SpecDecision(c) ==
  CASE c \in {Retired, MissingSession} -> NoDecision
    [] c \in {InvalidSession, HydratedInvalid} ->
      Decision(FALSE, FALSE, FALSE, TRUE, NoReason, FALSE, FALSE, FALSE, FALSE,
        FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] c = AlreadySentReady ->
      Decision(TRUE, TRUE, TRUE, TRUE, NoReason, FALSE, FALSE, FALSE, FALSE,
        TRUE, FALSE, FALSE, FALSE, TRUE, FALSE)
    [] c = RosterMissing -> Persisted(CommitRosterMissing)
    [] c = RosterUnverified -> Persisted(CommitRosterUnverified)
    [] c = MissingPayloadRequest ->
      [Persisted(MissingPayload) EXCEPT !.missing_block_requested = TRUE]
    [] c = MissingPayloadNoRequest -> Persisted(MissingPayload)
    [] c = ReadyRelayBypass -> ReadySuccess(FALSE, FALSE)
    [] c = ChunkRootMismatch ->
      Decision(TRUE, TRUE, FALSE, TRUE, NoReason, FALSE, TRUE, TRUE, FALSE,
        FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] c = ChunkRootAdopted -> ReadySuccess(FALSE, TRUE)
    [] c = ChunkRootMissingCase -> Persisted(ChunkRootMissing)
    [] c = BuildReady -> ReadySuccess(FALSE, FALSE)
    [] c = BuildReadyFork -> ReadySuccess(TRUE, FALSE)
    [] c = ObserverMarksSent ->
      Decision(TRUE, TRUE, TRUE, TRUE, NoReason, FALSE, FALSE, FALSE, FALSE,
        TRUE, FALSE, FALSE, FALSE, TRUE, FALSE)
    [] c = LocalNotInRoster -> Persisted(LocalNotInCommitTopology)
    [] c = BuilderMissingLocal ->
      Decision(TRUE, TRUE, FALSE, FALSE, NoReason, FALSE, FALSE, FALSE, FALSE,
        FALSE, FALSE, FALSE, FALSE, TRUE, FALSE)
    [] OTHER -> NoDecision

ActualDecision(c) ==
  CASE Bug = "retired_mutates"
       /\ c = Retired -> ReadySuccess(FALSE, FALSE)
    [] Bug = "missing_session_publishes"
       /\ c = MissingSession -> Persisted(NoReason)
    [] Bug = "invalid_keeps_deferral"
       /\ c = InvalidSession -> NoDecision
    [] Bug = "invalid_persists"
       /\ c = InvalidSession ->
         Decision(TRUE, TRUE, FALSE, TRUE, NoReason, FALSE, FALSE, FALSE, FALSE,
           FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "already_sent_rebroadcasts"
       /\ c = AlreadySentReady ->
         Decision(TRUE, TRUE, TRUE, TRUE, NoReason, FALSE, FALSE, FALSE, FALSE,
           TRUE, FALSE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "already_sent_skips_deliver"
       /\ c = AlreadySentReady ->
         Decision(TRUE, TRUE, TRUE, TRUE, NoReason, FALSE, FALSE, FALSE, FALSE,
           TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "roster_missing_no_deferral"
       /\ c = RosterMissing -> Persisted(NoReason)
    [] Bug = "roster_missing_emits_ready"
       /\ c = RosterMissing -> ReadySuccess(FALSE, FALSE)
    [] Bug = "unverified_emits_ready"
       /\ c = RosterUnverified -> ReadySuccess(FALSE, FALSE)
    [] Bug = "missing_payload_no_request"
       /\ c = MissingPayloadRequest -> Persisted(MissingPayload)
    [] Bug = "missing_payload_emits_ready"
       /\ c = MissingPayloadNoRequest -> ReadySuccess(FALSE, FALSE)
    [] Bug = "ready_relay_defers_payload"
       /\ c = ReadyRelayBypass -> Persisted(MissingPayload)
    [] Bug = "root_mismatch_emits_ready"
       /\ c = ChunkRootMismatch -> ReadySuccess(FALSE, FALSE)
    [] Bug = "root_mismatch_keeps_pending"
       /\ c = ChunkRootMismatch ->
         Decision(TRUE, TRUE, FALSE, TRUE, NoReason, FALSE, TRUE, FALSE, FALSE,
           FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "root_mismatch_delivers"
       /\ c = ChunkRootMismatch ->
         Decision(TRUE, TRUE, FALSE, TRUE, NoReason, FALSE, TRUE, TRUE, FALSE,
           FALSE, FALSE, FALSE, FALSE, TRUE, FALSE)
    [] Bug = "root_adoption_skipped"
       /\ c = ChunkRootAdopted -> ReadySuccess(FALSE, FALSE)
    [] Bug = "chunk_root_missing_emits_ready"
       /\ c = ChunkRootMissingCase -> ReadySuccess(FALSE, FALSE)
    [] Bug = "build_ready_no_broadcast"
       /\ c = BuildReady ->
         Decision(TRUE, TRUE, TRUE, TRUE, NoReason, FALSE, FALSE, FALSE, FALSE,
           TRUE, TRUE, FALSE, FALSE, FALSE, TRUE)
    [] Bug = "build_ready_keeps_deferral"
       /\ c = BuildReady ->
         Decision(TRUE, TRUE, TRUE, FALSE, NoReason, FALSE, FALSE, FALSE, FALSE,
           TRUE, TRUE, TRUE, FALSE, FALSE, TRUE)
    [] Bug = "build_ready_skips_deliver_after_local"
       /\ c = BuildReady ->
         Decision(TRUE, TRUE, TRUE, TRUE, NoReason, FALSE, FALSE, FALSE, FALSE,
           TRUE, TRUE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "fork_skipped"
       /\ c = BuildReadyFork -> ReadySuccess(FALSE, FALSE)
    [] Bug = "observer_broadcasts_ready"
       /\ c = ObserverMarksSent ->
         Decision(TRUE, TRUE, TRUE, TRUE, NoReason, FALSE, FALSE, FALSE, FALSE,
           TRUE, FALSE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "local_not_in_roster_emits_ready"
       /\ c = LocalNotInRoster -> ReadySuccess(FALSE, FALSE)
    [] Bug = "builder_missing_marks_sent"
       /\ c = BuilderMissingLocal ->
         Decision(TRUE, TRUE, FALSE, TRUE, NoReason, FALSE, FALSE, FALSE, FALSE,
           TRUE, TRUE, FALSE, FALSE, TRUE, FALSE)
    [] OTHER -> SpecDecision(c)

BugSet == {
  "none",
  "retired_mutates",
  "missing_session_publishes",
  "invalid_keeps_deferral",
  "invalid_persists",
  "already_sent_rebroadcasts",
  "already_sent_skips_deliver",
  "roster_missing_no_deferral",
  "roster_missing_emits_ready",
  "unverified_emits_ready",
  "missing_payload_no_request",
  "missing_payload_emits_ready",
  "ready_relay_defers_payload",
  "root_mismatch_emits_ready",
  "root_mismatch_keeps_pending",
  "root_mismatch_delivers",
  "root_adoption_skipped",
  "chunk_root_missing_emits_ready",
  "build_ready_no_broadcast",
  "build_ready_keeps_deferral",
  "build_ready_skips_deliver_after_local",
  "fork_skipped",
  "observer_broadcasts_ready",
  "local_not_in_roster_emits_ready",
  "builder_missing_marks_sent"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0
  /\ \A c \in Cases:
       LET d == ActualDecision(c) IN
       /\ d.status_persisted \in BOOLEAN
       /\ d.backlog_published \in BOOLEAN
       /\ d.sync_progress \in BOOLEAN
       /\ d.ready_deferral_removed \in BOOLEAN
       /\ d.ready_deferral_reason \in Reasons
       /\ d.missing_block_requested \in BOOLEAN
       /\ d.invalidated \in BOOLEAN
       /\ d.pending_cleared \in BOOLEAN
       /\ d.chunk_root_adopted \in BOOLEAN
       /\ d.sent_ready \in BOOLEAN
       /\ d.ready_recorded \in BOOLEAN
       /\ d.ready_broadcast \in BOOLEAN
       /\ d.fork_broadcast \in BOOLEAN
       /\ d.deliver_attempt \in BOOLEAN
       /\ d.deliver_after_local_ready \in BOOLEAN

DecisionExact ==
  \A c \in Cases:
    ActualDecision(c) = SpecDecision(c)

TerminalCasesDoNotPublish ==
  /\ ~ActualDecision(Retired).status_persisted
  /\ ~ActualDecision(Retired).backlog_published
  /\ ~ActualDecision(Retired).sent_ready
  /\ ~ActualDecision(Retired).ready_broadcast
  /\ ~ActualDecision(Retired).deliver_attempt
  /\ ~ActualDecision(MissingSession).status_persisted
  /\ ~ActualDecision(MissingSession).backlog_published
  /\ ~ActualDecision(MissingSession).sent_ready
  /\ ~ActualDecision(MissingSession).ready_broadcast
  /\ ~ActualDecision(MissingSession).deliver_attempt
  /\ ~ActualDecision(InvalidSession).status_persisted
  /\ ActualDecision(InvalidSession).ready_deferral_removed
  /\ ~ActualDecision(HydratedInvalid).status_persisted
  /\ ActualDecision(HydratedInvalid).ready_deferral_removed

DeferralCasesDoNotEmitReady ==
  \A c \in {RosterMissing, RosterUnverified, MissingPayloadRequest,
      MissingPayloadNoRequest, ChunkRootMissingCase, LocalNotInRoster}:
    /\ ActualDecision(c).ready_deferral_reason # NoReason
    /\ ~ActualDecision(c).sent_ready
    /\ ~ActualDecision(c).ready_broadcast
    /\ ActualDecision(c).deliver_attempt

ReadySuccessRecordsAndBroadcasts ==
  \A c \in {ReadyRelayBypass, ChunkRootAdopted, BuildReady, BuildReadyFork}:
    /\ ActualDecision(c).sent_ready
    /\ ActualDecision(c).ready_recorded
    /\ ActualDecision(c).ready_broadcast
    /\ ActualDecision(c).ready_deferral_removed
    /\ ActualDecision(c).deliver_after_local_ready
    /\ ~ActualDecision(c).deliver_attempt

AlreadySentReadySchedulesDeliver ==
  ActualDecision(AlreadySentReady).deliver_attempt

InvalidationStopsDelivery ==
  /\ ActualDecision(ChunkRootMismatch).invalidated
  /\ ActualDecision(ChunkRootMismatch).pending_cleared
  /\ ActualDecision(ChunkRootMismatch).ready_deferral_removed
  /\ ~ActualDecision(ChunkRootMismatch).sent_ready
  /\ ~ActualDecision(ChunkRootMismatch).deliver_attempt
  /\ ~ActualDecision(ChunkRootMismatch).deliver_after_local_ready

SpecialCasesStable ==
  /\ ActualDecision(MissingPayloadRequest).missing_block_requested
  /\ ActualDecision(ReadyRelayBypass).ready_broadcast
  /\ ActualDecision(ChunkRootAdopted).chunk_root_adopted
  /\ ActualDecision(BuildReadyFork).fork_broadcast
  /\ ActualDecision(ObserverMarksSent).sent_ready
  /\ ~ActualDecision(ObserverMarksSent).ready_broadcast
  /\ ActualDecision(ObserverMarksSent).deliver_attempt
  /\ ~ActualDecision(BuilderMissingLocal).sent_ready
  /\ ActualDecision(BuilderMissingLocal).deliver_attempt

RbcReadyEmissionCoreSafety ==
  /\ DecisionExact
  /\ TerminalCasesDoNotPublish
  /\ DeferralCasesDoNotEmitReady
  /\ ReadySuccessRecordsAndBroadcasts
  /\ InvalidationStopsDelivery
  /\ SpecialCasesStable

RbcReadyEmissionExactness == RbcReadyEmissionCoreSafety

RbcReadyEmissionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcReadyEmissionExactness

SafetyFast ==
  RbcReadyEmissionExactness

====
