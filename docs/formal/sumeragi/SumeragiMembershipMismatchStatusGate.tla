---- MODULE SumeragiMembershipMismatchStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi membership snapshot and mismatch status.

This slice captures `set_membership_view_hash(...)`,
`membership_snapshot()`, `record_membership_mismatch(...)`,
`clear_membership_mismatch(...)`, `membership_mismatch_consecutive(...)`,
`membership_mismatch_snapshot()`, and the test reset helpers in
`sumeragi/status.rs`.

The Rust code records operator-facing state, but it is fed by the
deterministic membership-view hash checked by
`SumeragiMembershipViewHashGate.tla`. The observable contract is that a set
hash exposes exactly the latest height/view/epoch/hash tuple, reset hides the
snapshot, recorded mismatches activate only the offending peer, consecutive
counts are per-peer and saturating, `last` carries the exact newest mismatch
context, clearing a peer removes its active/consecutive entry without erasing
the global last context, and reset clears both active entries and last context.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

NoPeer == "none"
PeerA == "peer_a"
PeerB == "peer_b"
PeerC == "peer_c"
Peers == {PeerA, PeerB, PeerC}

NoHash == "none"
ViewHashA == "view_hash_a"
ViewHashB == "view_hash_b"
LocalHashA == "local_hash_a"
LocalHashB == "local_hash_b"
LocalHashC == "local_hash_c"
RemoteHashA == "remote_hash_a"
RemoteHashB == "remote_hash_b"
RemoteHashC == "remote_hash_c"
Hashes == {
  NoHash,
  ViewHashA,
  ViewHashB,
  LocalHashA,
  LocalHashB,
  LocalHashC,
  RemoteHashA,
  RemoteHashB,
  RemoteHashC
}

MaxCounter == 3
Counter == 0..MaxCounter

Cases == {
  "snapshot_unset",
  "snapshot_set",
  "snapshot_reset",
  "record_first_peer",
  "record_repeat_peer",
  "record_saturating_peer",
  "record_second_peer",
  "clear_existing_peer",
  "clear_last_peer",
  "clear_absent_peer",
  "reset_registry"
}

SnapshotSetCases == {"snapshot_set"}
SnapshotResetCases == {"snapshot_reset"}
RecordCases == {
  "record_first_peer",
  "record_repeat_peer",
  "record_saturating_peer",
  "record_second_peer"
}
ClearExistingCases == {"clear_existing_peer", "clear_last_peer"}
ClearCases == ClearExistingCases \union {"clear_absent_peer"}
ResetRegistryCases == {"reset_registry"}

SpecSnapshotPresent(c) == c \in SnapshotSetCases
SpecSnapshotHeight(c) == IF c \in SnapshotSetCases THEN 10 ELSE 0
SpecSnapshotView(c) == IF c \in SnapshotSetCases THEN 1 ELSE 0
SpecSnapshotEpoch(c) == IF c \in SnapshotSetCases THEN 2 ELSE 0
SpecSnapshotHash(c) == IF c \in SnapshotSetCases THEN ViewHashA ELSE NoHash

ContextPeer(c) ==
  CASE c = "record_second_peer" -> PeerB
    [] OTHER -> PeerA

ContextHeight(c) ==
  CASE c = "record_first_peer" -> 10
    [] c = "record_repeat_peer" -> 11
    [] c = "record_saturating_peer" -> 12
    [] c = "record_second_peer" -> 12
    [] c = "clear_existing_peer" -> 12
    [] c = "clear_last_peer" -> 10
    [] c = "clear_absent_peer" -> 10
    [] OTHER -> 0

ContextView(c) ==
  CASE c = "record_first_peer" -> 1
    [] c = "record_repeat_peer" -> 2
    [] c = "record_saturating_peer" -> 3
    [] c = "record_second_peer" -> 1
    [] c = "clear_existing_peer" -> 1
    [] c = "clear_last_peer" -> 1
    [] c = "clear_absent_peer" -> 1
    [] OTHER -> 0

ContextEpoch(c) ==
  CASE c = "record_first_peer" -> 2
    [] c = "record_repeat_peer" -> 2
    [] c = "record_saturating_peer" -> 2
    [] c = "record_second_peer" -> 2
    [] c = "clear_existing_peer" -> 2
    [] c = "clear_last_peer" -> 2
    [] c = "clear_absent_peer" -> 2
    [] OTHER -> 0

ContextLocalHash(c) ==
  CASE c = "record_first_peer" -> LocalHashA
    [] c = "record_repeat_peer" -> LocalHashB
    [] c = "record_saturating_peer" -> LocalHashC
    [] c = "record_second_peer" -> LocalHashA
    [] c = "clear_existing_peer" -> LocalHashA
    [] c = "clear_last_peer" -> LocalHashA
    [] c = "clear_absent_peer" -> LocalHashA
    [] OTHER -> NoHash

ContextRemoteHash(c) ==
  CASE c = "record_first_peer" -> RemoteHashA
    [] c = "record_repeat_peer" -> RemoteHashB
    [] c = "record_saturating_peer" -> RemoteHashC
    [] c = "record_second_peer" -> RemoteHashB
    [] c = "clear_existing_peer" -> RemoteHashB
    [] c = "clear_last_peer" -> RemoteHashA
    [] c = "clear_absent_peer" -> RemoteHashA
    [] OTHER -> NoHash

SpecReturnCount(c) ==
  CASE c = "record_first_peer" -> 1
    [] c = "record_repeat_peer" -> 2
    [] c = "record_saturating_peer" -> MaxCounter
    [] c = "record_second_peer" -> 1
    [] OTHER -> 0

SpecActivePeers(c) ==
  CASE c = "record_first_peer" -> {PeerA}
    [] c = "record_repeat_peer" -> {PeerA}
    [] c = "record_saturating_peer" -> {PeerA}
    [] c = "record_second_peer" -> {PeerA, PeerB}
    [] c = "clear_existing_peer" -> {PeerB}
    [] c = "clear_last_peer" -> {}
    [] c = "clear_absent_peer" -> {PeerA}
    [] OTHER -> {}

SpecCount(peer, c) ==
  CASE peer = PeerA /\ c = "record_first_peer" -> 1
    [] peer = PeerA /\ c = "record_repeat_peer" -> 2
    [] peer = PeerA /\ c = "record_saturating_peer" -> MaxCounter
    [] peer = PeerA /\ c = "record_second_peer" -> 1
    [] peer = PeerA /\ c = "clear_absent_peer" -> 1
    [] peer = PeerB /\ c = "record_second_peer" -> 1
    [] peer = PeerB /\ c = "clear_existing_peer" -> 1
    [] OTHER -> 0

SpecLastPresent(c) == c \in RecordCases \union ClearCases

SpecLastPeer(c) ==
  IF SpecLastPresent(c)
  THEN IF c \in {"record_second_peer", "clear_existing_peer"} THEN PeerB ELSE PeerA
  ELSE NoPeer

SpecLastHeight(c) == IF SpecLastPresent(c) THEN ContextHeight(c) ELSE 0
SpecLastView(c) == IF SpecLastPresent(c) THEN ContextView(c) ELSE 0
SpecLastEpoch(c) == IF SpecLastPresent(c) THEN ContextEpoch(c) ELSE 0
SpecLastLocalHash(c) == IF SpecLastPresent(c) THEN ContextLocalHash(c) ELSE NoHash
SpecLastRemoteHash(c) == IF SpecLastPresent(c) THEN ContextRemoteHash(c) ELSE NoHash

ActualSnapshotPresent(c) ==
  CASE Bug = "snapshot_set_not_present" /\ c = "snapshot_set" -> FALSE
    [] Bug = "snapshot_reset_keeps_present" /\ c = "snapshot_reset" -> TRUE
    [] OTHER -> SpecSnapshotPresent(c)

ActualSnapshotHeight(c) ==
  CASE Bug = "snapshot_set_drops_height" /\ c = "snapshot_set" -> 0
    [] Bug = "snapshot_reset_keeps_height" /\ c = "snapshot_reset" -> 10
    [] OTHER -> SpecSnapshotHeight(c)

ActualSnapshotView(c) ==
  CASE Bug = "snapshot_set_drops_view" /\ c = "snapshot_set" -> 0
    [] OTHER -> SpecSnapshotView(c)

ActualSnapshotEpoch(c) ==
  CASE Bug = "snapshot_set_drops_epoch" /\ c = "snapshot_set" -> 0
    [] OTHER -> SpecSnapshotEpoch(c)

ActualSnapshotHash(c) ==
  CASE Bug = "snapshot_set_drops_hash" /\ c = "snapshot_set" -> NoHash
    [] OTHER -> SpecSnapshotHash(c)

ActualReturnCount(c) ==
  CASE Bug = "record_new_returns_zero" /\ c = "record_first_peer" -> 0
    [] Bug = "record_repeat_not_incremented" /\ c = "record_repeat_peer" -> 1
    [] Bug = "record_saturation_wraps" /\ c = "record_saturating_peer" -> 0
    [] OTHER -> SpecReturnCount(c)

ActualActivePeers(c) ==
  CASE Bug = "record_new_inactive" /\ c = "record_first_peer" -> {}
    [] Bug = "record_second_drops_first" /\ c = "record_second_peer" -> {PeerB}
    [] Bug = "clear_keeps_peer" /\ c = "clear_existing_peer" -> {PeerA, PeerB}
    [] Bug = "clear_keeps_peer" /\ c = "clear_last_peer" -> {PeerA}
    [] Bug = "clear_absent_mutates" /\ c = "clear_absent_peer" -> {}
    [] Bug = "reset_keeps_active" /\ c = "reset_registry" -> {PeerA}
    [] OTHER -> SpecActivePeers(c)

ActualCount(peer, c) ==
  CASE Bug = "record_new_returns_zero" /\ peer = PeerA /\ c = "record_first_peer" -> 0
    [] Bug = "record_repeat_not_incremented" /\ peer = PeerA /\ c = "record_repeat_peer" -> 1
    [] Bug = "record_saturation_wraps" /\ peer = PeerA /\ c = "record_saturating_peer" -> 0
    [] Bug = "record_second_drops_first" /\ peer = PeerA /\ c = "record_second_peer" -> 0
    [] Bug = "clear_keeps_peer" /\ peer = PeerA /\ c \in ClearExistingCases -> 1
    [] Bug = "clear_absent_mutates" /\ peer = PeerA /\ c = "clear_absent_peer" -> 0
    [] Bug = "reset_keeps_active" /\ peer = PeerA /\ c = "reset_registry" -> 1
    [] OTHER -> SpecCount(peer, c)

ActualLastPresent(c) ==
  CASE Bug = "clear_clears_last" /\ c \in ClearCases -> FALSE
    [] Bug = "reset_keeps_last" /\ c = "reset_registry" -> TRUE
    [] OTHER -> SpecLastPresent(c)

ActualLastPeer(c) ==
  CASE Bug = "record_last_not_updated" /\ c = "record_second_peer" -> PeerA
    [] Bug = "reset_keeps_last" /\ c = "reset_registry" -> PeerA
    [] OTHER -> SpecLastPeer(c)

ActualLastHeight(c) ==
  CASE Bug = "record_drops_height" /\ c = "record_repeat_peer" -> 0
    [] Bug = "record_last_not_updated" /\ c = "record_second_peer" -> 10
    [] Bug = "reset_keeps_last" /\ c = "reset_registry" -> 10
    [] OTHER -> SpecLastHeight(c)

ActualLastView(c) ==
  CASE Bug = "record_drops_view" /\ c = "record_repeat_peer" -> 0
    [] Bug = "record_last_not_updated" /\ c = "record_second_peer" -> 1
    [] Bug = "reset_keeps_last" /\ c = "reset_registry" -> 1
    [] OTHER -> SpecLastView(c)

ActualLastEpoch(c) ==
  CASE Bug = "record_drops_epoch" /\ c = "record_repeat_peer" -> 0
    [] Bug = "reset_keeps_last" /\ c = "reset_registry" -> 2
    [] OTHER -> SpecLastEpoch(c)

ActualLastLocalHash(c) ==
  CASE Bug = "record_drops_local_hash" /\ c = "record_repeat_peer" -> NoHash
    [] Bug = "reset_keeps_last" /\ c = "reset_registry" -> LocalHashA
    [] OTHER -> SpecLastLocalHash(c)

ActualLastRemoteHash(c) ==
  CASE Bug = "record_drops_remote_hash" /\ c = "record_repeat_peer" -> NoHash
    [] Bug = "reset_keeps_last" /\ c = "reset_registry" -> RemoteHashA
    [] OTHER -> SpecLastRemoteHash(c)

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  snapshot_present,
  \* @type: Int;
  snapshot_height,
  \* @type: Int;
  snapshot_view,
  \* @type: Int;
  snapshot_epoch,
  \* @type: Str;
  snapshot_hash,
  \* @type: Set(Str);
  active_peers,
  \* @type: Int;
  count_a,
  \* @type: Int;
  count_b,
  \* @type: Int;
  count_c,
  \* @type: Int;
  returned_count,
  \* @type: Bool;
  last_present,
  \* @type: Str;
  last_peer,
  \* @type: Int;
  last_height,
  \* @type: Int;
  last_view,
  \* @type: Int;
  last_epoch,
  \* @type: Str;
  last_local_hash,
  \* @type: Str;
  last_remote_hash

\* @type: <<Str, Bool, Int, Int, Int, Str, Set(Str), Int, Int, Int, Int, Bool, Str, Int, Int, Int, Str, Str>>;
vars == <<candidate, snapshot_present, snapshot_height, snapshot_view,
  snapshot_epoch, snapshot_hash, active_peers, count_a, count_b, count_c,
  returned_count, last_present, last_peer, last_height, last_view, last_epoch,
  last_local_hash, last_remote_hash>>

Init ==
  /\ candidate = "none"
  /\ snapshot_present = FALSE
  /\ snapshot_height = 0
  /\ snapshot_view = 0
  /\ snapshot_epoch = 0
  /\ snapshot_hash = NoHash
  /\ active_peers = {}
  /\ count_a = 0
  /\ count_b = 0
  /\ count_c = 0
  /\ returned_count = 0
  /\ last_present = FALSE
  /\ last_peer = NoPeer
  /\ last_height = 0
  /\ last_view = 0
  /\ last_epoch = 0
  /\ last_local_hash = NoHash
  /\ last_remote_hash = NoHash

Next ==
  /\ candidate = "none"
  /\ candidate' \in Cases
  /\ snapshot_present' = ActualSnapshotPresent(candidate')
  /\ snapshot_height' = ActualSnapshotHeight(candidate')
  /\ snapshot_view' = ActualSnapshotView(candidate')
  /\ snapshot_epoch' = ActualSnapshotEpoch(candidate')
  /\ snapshot_hash' = ActualSnapshotHash(candidate')
  /\ active_peers' = ActualActivePeers(candidate')
  /\ count_a' = ActualCount(PeerA, candidate')
  /\ count_b' = ActualCount(PeerB, candidate')
  /\ count_c' = ActualCount(PeerC, candidate')
  /\ returned_count' = ActualReturnCount(candidate')
  /\ last_present' = ActualLastPresent(candidate')
  /\ last_peer' = ActualLastPeer(candidate')
  /\ last_height' = ActualLastHeight(candidate')
  /\ last_view' = ActualLastView(candidate')
  /\ last_epoch' = ActualLastEpoch(candidate')
  /\ last_local_hash' = ActualLastLocalHash(candidate')
  /\ last_remote_hash' = ActualLastRemoteHash(candidate')

TypeInvariant ==
  /\ candidate \in Cases \union {"none"}
  /\ snapshot_present \in BOOLEAN
  /\ snapshot_height \in 0..12
  /\ snapshot_view \in 0..3
  /\ snapshot_epoch \in 0..2
  /\ snapshot_hash \in Hashes
  /\ active_peers \subseteq Peers
  /\ count_a \in Counter
  /\ count_b \in Counter
  /\ count_c \in Counter
  /\ returned_count \in Counter
  /\ last_present \in BOOLEAN
  /\ last_peer \in Peers \union {NoPeer}
  /\ last_height \in 0..12
  /\ last_view \in 0..3
  /\ last_epoch \in 0..2
  /\ last_local_hash \in Hashes
  /\ last_remote_hash \in Hashes

ResultMatchesSpec ==
  \/ candidate = "none"
  \/ /\ snapshot_present = SpecSnapshotPresent(candidate)
     /\ snapshot_height = SpecSnapshotHeight(candidate)
     /\ snapshot_view = SpecSnapshotView(candidate)
     /\ snapshot_epoch = SpecSnapshotEpoch(candidate)
     /\ snapshot_hash = SpecSnapshotHash(candidate)
     /\ active_peers = SpecActivePeers(candidate)
     /\ count_a = SpecCount(PeerA, candidate)
     /\ count_b = SpecCount(PeerB, candidate)
     /\ count_c = SpecCount(PeerC, candidate)
     /\ returned_count = SpecReturnCount(candidate)
     /\ last_present = SpecLastPresent(candidate)
     /\ last_peer = SpecLastPeer(candidate)
     /\ last_height = SpecLastHeight(candidate)
     /\ last_view = SpecLastView(candidate)
     /\ last_epoch = SpecLastEpoch(candidate)
     /\ last_local_hash = SpecLastLocalHash(candidate)
     /\ last_remote_hash = SpecLastRemoteHash(candidate)

====
