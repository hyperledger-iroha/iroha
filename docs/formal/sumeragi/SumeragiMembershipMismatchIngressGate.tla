---- MODULE SumeragiMembershipMismatchIngressGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for membership-mismatch ingress handling.

This slice captures the runtime bridge from inbound consensus-params adverts to
membership-mismatch status, plus the fail-closed inbound consensus-message gate
in `main_loop.rs` and `main_loop/proposal_handlers.rs`.

The observable contract is:

* consensus-params adverts without membership, without a remote hash, without a
  local membership snapshot, with a different height/view/epoch, or without an
  authenticated sender do not mutate mismatch state;
* authenticated adverts for the same height/view/epoch record mismatches and
  clear state when the advertised hash matches the local snapshot;
* mismatch alerting starts only after the configured consecutive threshold;
* fail-closed ingress drops only non-`ConsensusParams` messages from
  authenticated peers whose active mismatch count has reached the threshold;
* dropped messages are counted with the `membership_mismatch` status reason.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

NoPeer == "none"
PeerA == "peer_a"
Peers == {PeerA}

BlockCreated == "block_created"
ConsensusParams == "consensus_params"
UnknownKind == "unknown_kind"
Kinds == {BlockCreated, ConsensusParams, UnknownKind}

NoReason == "none"
MembershipMismatchReason == "membership_mismatch"
FutureWindowReason == "future_window"
Reasons == {NoReason, MembershipMismatchReason, FutureWindowReason}

Threshold == 2
MaxCounter == 3
Counter == 0..MaxCounter

LocalHeight == 5
LocalView == 1
LocalEpoch == 0

NoMembership == "no_membership"
NoRemoteHash == "no_remote_hash"
NoLocalSnapshot == "no_local_snapshot"
ContextHeightMismatch == "context_height_mismatch"
ContextViewMismatch == "context_view_mismatch"
ContextEpochMismatch == "context_epoch_mismatch"
UnauthenticatedMismatch == "unauthenticated_mismatch"
RecordFirstBelowThreshold == "record_first_below_threshold"
RecordReachesThreshold == "record_reaches_threshold"
RecordSaturates == "record_saturates"
ClearExistingMatch == "clear_existing_match"
ClearAbsentMatch == "clear_absent_match"
DropFailClosedActive == "drop_fail_closed_active"
DropFailOpenActive == "drop_fail_open_active"
DropBelowThreshold == "drop_below_threshold"
DropConsensusParamsActive == "drop_consensus_params_active"
DropNoSender == "drop_no_sender"
DropUnknownKindActive == "drop_unknown_kind_active"

AdvertCases == {
  NoMembership,
  NoRemoteHash,
  NoLocalSnapshot,
  ContextHeightMismatch,
  ContextViewMismatch,
  ContextEpochMismatch,
  UnauthenticatedMismatch,
  RecordFirstBelowThreshold,
  RecordReachesThreshold,
  RecordSaturates,
  ClearExistingMatch,
  ClearAbsentMatch
}

DropCases == {
  DropFailClosedActive,
  DropFailOpenActive,
  DropBelowThreshold,
  DropConsensusParamsActive,
  DropNoSender,
  DropUnknownKindActive
}

Cases == AdvertCases \union DropCases

MinCounter(n) == IF n > MaxCounter THEN MaxCounter ELSE n

HasMembership(c) == c /= NoMembership
RemoteHashPresent(c) == c /= NoRemoteHash
LocalSnapshotPresent(c) == c /= NoLocalSnapshot
Sender(c) ==
  IF c \in {UnauthenticatedMismatch, DropNoSender} THEN NoPeer ELSE PeerA

RemoteHeight(c) ==
  IF c = ContextHeightMismatch THEN LocalHeight + 1 ELSE LocalHeight

RemoteView(c) ==
  IF c = ContextViewMismatch THEN LocalView + 1 ELSE LocalView

RemoteEpoch(c) ==
  IF c = ContextEpochMismatch THEN LocalEpoch + 1 ELSE LocalEpoch

HashesMatch(c) == c \in {ClearExistingMatch, ClearAbsentMatch}

InitialCount(c) ==
  CASE c = RecordReachesThreshold -> 1
    [] c = RecordSaturates -> MaxCounter
    [] c = ClearExistingMatch -> Threshold
    [] c = DropBelowThreshold -> Threshold - 1
    [] c \in DropCases -> Threshold
    [] OTHER -> 0

InitialActive(c) == InitialCount(c) > 0

SameContext(c) ==
  /\ RemoteHeight(c) = LocalHeight
  /\ RemoteView(c) = LocalView
  /\ RemoteEpoch(c) = LocalEpoch

EligibleAdvert(c) ==
  /\ c \in AdvertCases
  /\ HasMembership(c)
  /\ RemoteHashPresent(c)
  /\ LocalSnapshotPresent(c)
  /\ SameContext(c)
  /\ Sender(c) /= NoPeer

SpecRecord(c) == EligibleAdvert(c) /\ ~HashesMatch(c)
SpecClear(c) == EligibleAdvert(c) /\ HashesMatch(c)

SpecCount(c) ==
  CASE SpecRecord(c) -> MinCounter(InitialCount(c) + 1)
    [] SpecClear(c) -> 0
    [] OTHER -> InitialCount(c)

SpecActive(c) ==
  CASE SpecRecord(c) -> TRUE
    [] SpecClear(c) -> FALSE
    [] OTHER -> InitialActive(c)

SpecWarn(c) == SpecRecord(c) /\ SpecCount(c) >= Threshold
SpecTelemetryMismatch(c) == SpecRecord(c)
SpecTelemetryClear(c) == SpecClear(c)

FailClosed(c) == c /= DropFailOpenActive

MessageKind(c) ==
  CASE c = DropConsensusParamsActive -> ConsensusParams
    [] c = DropUnknownKindActive -> UnknownKind
    [] OTHER -> BlockCreated

StatusKindKnown(c) == MessageKind(c) /= UnknownKind

SpecDropped(c) ==
  /\ c \in DropCases
  /\ FailClosed(c)
  /\ MessageKind(c) /= ConsensusParams
  /\ Sender(c) /= NoPeer
  /\ InitialCount(c) >= Threshold
  /\ StatusKindKnown(c)

SpecStatusRecorded(c) == SpecDropped(c)
SpecStatusReason(c) ==
  IF SpecStatusRecorded(c) THEN MembershipMismatchReason ELSE NoReason

ActualCount(c) ==
  CASE Bug = "record_without_membership" /\ c = NoMembership -> 1
    [] Bug = "record_without_remote_hash" /\ c = NoRemoteHash -> 1
    [] Bug = "record_without_local_snapshot" /\ c = NoLocalSnapshot -> 1
    [] Bug = "record_context_height_ignored" /\ c = ContextHeightMismatch -> 1
    [] Bug = "record_context_view_ignored" /\ c = ContextViewMismatch -> 1
    [] Bug = "record_context_epoch_ignored" /\ c = ContextEpochMismatch -> 1
    [] Bug = "record_unauthenticated" /\ c = UnauthenticatedMismatch -> 1
    [] Bug = "mismatch_not_recorded" /\ c = RecordFirstBelowThreshold -> 0
    [] Bug = "mismatch_not_incremented" /\ c = RecordReachesThreshold -> 1
    [] Bug = "record_counter_wraps" /\ c = RecordSaturates -> 0
    [] Bug = "match_does_not_clear" /\ c = ClearExistingMatch -> InitialCount(c)
    [] Bug = "match_records_mismatch" /\ c = ClearAbsentMatch -> 1
    [] OTHER -> SpecCount(c)

ActualActive(c) ==
  CASE Bug \in {
         "record_without_membership",
         "record_without_remote_hash",
         "record_without_local_snapshot",
         "record_context_height_ignored",
         "record_context_view_ignored",
         "record_context_epoch_ignored",
         "record_unauthenticated",
         "match_records_mismatch"
       } /\ c \in AdvertCases -> TRUE
    [] Bug = "mismatch_not_recorded" /\ c = RecordFirstBelowThreshold -> FALSE
    [] Bug = "record_counter_wraps" /\ c = RecordSaturates -> FALSE
    [] Bug = "match_does_not_clear" /\ c = ClearExistingMatch -> TRUE
    [] OTHER -> SpecActive(c)

ActualWarn(c) ==
  CASE Bug = "threshold_warn_suppressed" /\ c = RecordReachesThreshold -> FALSE
    [] Bug = "below_threshold_warns" /\ c = RecordFirstBelowThreshold -> TRUE
    [] OTHER -> SpecWarn(c)

ActualTelemetryMismatch(c) ==
  CASE Bug = "telemetry_mismatch_missing" /\ c = RecordFirstBelowThreshold -> FALSE
    [] OTHER -> SpecTelemetryMismatch(c)

ActualTelemetryClear(c) ==
  CASE Bug = "telemetry_clear_missing" /\ c = ClearExistingMatch -> FALSE
    [] OTHER -> SpecTelemetryClear(c)

ActualDropped(c) ==
  CASE Bug = "fail_closed_ignored" /\ c = DropFailClosedActive -> FALSE
    [] Bug = "fail_open_drops" /\ c = DropFailOpenActive -> TRUE
    [] Bug = "threshold_ignored_for_drop" /\ c = DropBelowThreshold -> TRUE
    [] Bug = "params_dropped" /\ c = DropConsensusParamsActive -> TRUE
    [] Bug = "unauthenticated_drop" /\ c = DropNoSender -> TRUE
    [] Bug = "unknown_kind_dropped" /\ c = DropUnknownKindActive -> TRUE
    [] OTHER -> SpecDropped(c)

ActualStatusRecorded(c) ==
  CASE Bug = "drop_status_missing" /\ c = DropFailClosedActive -> FALSE
    [] Bug = "status_recorded_without_drop" /\ c = DropBelowThreshold -> TRUE
    [] OTHER -> ActualDropped(c)

ActualStatusReason(c) ==
  CASE Bug = "drop_reason_wrong" /\ c = DropFailClosedActive -> FutureWindowReason
    [] ActualStatusRecorded(c) -> MembershipMismatchReason
    [] OTHER -> NoReason

Bugs == {
  "none",
  "record_without_membership",
  "record_without_remote_hash",
  "record_without_local_snapshot",
  "record_context_height_ignored",
  "record_context_view_ignored",
  "record_context_epoch_ignored",
  "record_unauthenticated",
  "mismatch_not_recorded",
  "mismatch_not_incremented",
  "record_counter_wraps",
  "match_does_not_clear",
  "match_records_mismatch",
  "threshold_warn_suppressed",
  "below_threshold_warns",
  "telemetry_mismatch_missing",
  "telemetry_clear_missing",
  "fail_closed_ignored",
  "fail_open_drops",
  "threshold_ignored_for_drop",
  "params_dropped",
  "unauthenticated_drop",
  "unknown_kind_dropped",
  "drop_status_missing",
  "status_recorded_without_drop",
  "drop_reason_wrong"
}

VARIABLE
  \* @type: Str;
  candidate,
  \* @type: Int;
  count,
  \* @type: Bool;
  active,
  \* @type: Bool;
  warn,
  \* @type: Bool;
  telemetry_mismatch,
  \* @type: Bool;
  telemetry_clear,
  \* @type: Bool;
  dropped,
  \* @type: Bool;
  status_recorded,
  \* @type: Str;
  status_reason

\* @type: <<Str, Int, Bool, Bool, Bool, Bool, Bool, Bool, Str>>;
vars == <<candidate, count, active, warn, telemetry_mismatch,
  telemetry_clear, dropped, status_recorded, status_reason>>

Init ==
  /\ candidate = "none"
  /\ count = 0
  /\ active = FALSE
  /\ warn = FALSE
  /\ telemetry_mismatch = FALSE
  /\ telemetry_clear = FALSE
  /\ dropped = FALSE
  /\ status_recorded = FALSE
  /\ status_reason = NoReason

Next ==
  /\ candidate = "none"
  /\ candidate' \in Cases
  /\ count' = ActualCount(candidate')
  /\ active' = ActualActive(candidate')
  /\ warn' = ActualWarn(candidate')
  /\ telemetry_mismatch' = ActualTelemetryMismatch(candidate')
  /\ telemetry_clear' = ActualTelemetryClear(candidate')
  /\ dropped' = ActualDropped(candidate')
  /\ status_recorded' = ActualStatusRecorded(candidate')
  /\ status_reason' = ActualStatusReason(candidate')

TypeInvariant ==
  /\ Bug \in Bugs
  /\ candidate \in Cases \union {"none"}
  /\ count \in Counter
  /\ active \in BOOLEAN
  /\ warn \in BOOLEAN
  /\ telemetry_mismatch \in BOOLEAN
  /\ telemetry_clear \in BOOLEAN
  /\ dropped \in BOOLEAN
  /\ status_recorded \in BOOLEAN
  /\ status_reason \in Reasons

MismatchStateFieldsMatch(c) ==
  /\ count = SpecCount(c)
  /\ active = SpecActive(c)

MismatchTelemetryFieldsMatch(c) ==
  /\ warn = SpecWarn(c)
  /\ telemetry_mismatch = SpecTelemetryMismatch(c)
  /\ telemetry_clear = SpecTelemetryClear(c)

MismatchFailClosedFieldsMatch(c) ==
  /\ dropped = SpecDropped(c)
  /\ status_recorded = SpecStatusRecorded(c)
  /\ status_reason = SpecStatusReason(c)

MembershipMismatchIngressStateExact ==
  \/ candidate = "none"
  \/ MismatchStateFieldsMatch(candidate)

MembershipMismatchIngressTelemetryExact ==
  \/ candidate = "none"
  \/ MismatchTelemetryFieldsMatch(candidate)

MembershipMismatchIngressFailClosedExact ==
  \/ candidate = "none"
  \/ MismatchFailClosedFieldsMatch(candidate)

MembershipMismatchIngressExactness ==
  /\ MembershipMismatchIngressStateExact
  /\ MembershipMismatchIngressTelemetryExact
  /\ MembershipMismatchIngressFailClosedExact

ResultMatchesSpec ==
  MembershipMismatchIngressExactness

====
