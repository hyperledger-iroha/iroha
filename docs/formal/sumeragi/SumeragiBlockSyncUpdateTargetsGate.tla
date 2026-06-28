---- MODULE SumeragiBlockSyncUpdateTargetsGate ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
A bounded abstract model for BlockSyncUpdate gossip target selection.

This slice captures `Actor::block_sync_update_targets_for_peers(...)`. It
abstracts deterministic hash ordering to fixed representatives while preserving
the helper contract: zero limits and empty world-peer lists produce no targets;
the local peer is never targeted; online registered/trusted strays are preferred
before world peers; unregistered strays are ignored; online world peers are used
before offline world fallback; and the final fanout is capped by the gossip
limit.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ZeroLimit == "zero_limit"
NoPeers == "no_peers"
StraysFillLimit == "strays_fill_limit"
StraysThenWorldOnline == "strays_then_world_online"
UnregisteredStrayIgnored == "unregistered_stray_ignored"
TrustedStrayAllowed == "trusted_stray_allowed"
WorldOnlinePreferred == "world_online_preferred"
WorldFallbackWhenNoOnline == "world_fallback_when_no_online"
CapWorldOnly == "cap_world_only"
OnlyLocalNoTargets == "only_local_no_targets"

Cases == {
  ZeroLimit,
  NoPeers,
  StraysFillLimit,
  StraysThenWorldOnline,
  UnregisteredStrayIgnored,
  TrustedStrayAllowed,
  WorldOnlinePreferred,
  WorldFallbackWhenNoOnline,
  CapWorldOnly,
  OnlyLocalNoTargets
}

LocalPeer == "local"
WorldA == "world_a"
WorldB == "world_b"
WorldOffline == "world_offline"
RegisteredStray == "registered_stray"
TrustedStray == "trusted_stray"
UnregisteredStray == "unregistered_stray"

PeerValues == {
  LocalPeer,
  WorldA,
  WorldB,
  WorldOffline,
  RegisteredStray,
  TrustedStray,
  UnregisteredStray
}

Limit(c) ==
  CASE c = ZeroLimit -> 0
    [] c = TrustedStrayAllowed -> 1
    [] c = WorldFallbackWhenNoOnline -> 1
    [] c = CapWorldOnly -> 1
    [] c = StraysThenWorldOnline -> 3
    [] OTHER -> 2

WorldPeers(c) ==
  CASE c = NoPeers -> {}
    [] c = TrustedStrayAllowed -> {LocalPeer}
    [] c = OnlyLocalNoTargets -> {LocalPeer}
    [] c \in {StraysThenWorldOnline, CapWorldOnly} ->
      {LocalPeer, WorldA, WorldB}
    [] c \in {WorldOnlinePreferred, WorldFallbackWhenNoOnline} ->
      {LocalPeer, WorldA, WorldOffline}
    [] OTHER -> {LocalPeer, WorldA}

RegisteredPeers(c) ==
  CASE c = StraysFillLimit ->
      {LocalPeer, WorldA, RegisteredStray}
    [] c = StraysThenWorldOnline ->
      {LocalPeer, WorldA, WorldB, RegisteredStray}
    [] OTHER -> WorldPeers(c)

TrustedPeers(c) ==
  CASE c \in {StraysFillLimit, TrustedStrayAllowed} -> {TrustedStray}
    [] OTHER -> {}

OnlinePeers(c) ==
  CASE c = NoPeers -> {LocalPeer, WorldA}
    [] c = StraysFillLimit ->
      {LocalPeer, WorldA, RegisteredStray, TrustedStray}
    [] c = StraysThenWorldOnline ->
      {LocalPeer, WorldA, WorldB, RegisteredStray}
    [] c = UnregisteredStrayIgnored ->
      {LocalPeer, WorldA, UnregisteredStray}
    [] c = TrustedStrayAllowed ->
      {LocalPeer, TrustedStray}
    [] c = WorldOnlinePreferred ->
      {LocalPeer, WorldA}
    [] c = WorldFallbackWhenNoOnline ->
      {LocalPeer}
    [] c = CapWorldOnly ->
      {LocalPeer, WorldA, WorldB}
    [] c = OnlyLocalNoTargets ->
      {LocalPeer}
    [] OTHER -> {LocalPeer, WorldA}

RegisteredOrTrusted(c) == RegisteredPeers(c) \cup TrustedPeers(c)

Strays(c) ==
  ((OnlinePeers(c) \ {LocalPeer}) \ WorldPeers(c)) \cap RegisteredOrTrusted(c)

WorldOnline(c) ==
  (OnlinePeers(c) \ {LocalPeer}) \cap WorldPeers(c)

WorldCandidatesAll(c) ==
  WorldPeers(c) \ {LocalPeer}

WorldCandidates(c) ==
  IF WorldOnline(c) = {} THEN WorldCandidatesAll(c) ELSE WorldOnline(c)

EligibleTargets(c) ==
  Strays(c) \cup WorldCandidates(c)

SpecTargets(c) ==
  CASE c = ZeroLimit -> {}
    [] c = NoPeers -> {}
    [] c = StraysFillLimit -> {RegisteredStray, TrustedStray}
    [] c = StraysThenWorldOnline -> {RegisteredStray, WorldA, WorldB}
    [] c = UnregisteredStrayIgnored -> {WorldA}
    [] c = TrustedStrayAllowed -> {TrustedStray}
    [] c = WorldOnlinePreferred -> {WorldA}
    [] c = WorldFallbackWhenNoOnline -> {WorldA}
    [] c = CapWorldOnly -> {WorldA}
    [] c = OnlyLocalNoTargets -> {}
    [] OTHER -> {}

ActualTargets(c) ==
  CASE Bug = "zero_limit_nonempty"
       /\ c = ZeroLimit ->
      {WorldA}
    [] Bug = "no_peers_nonempty"
       /\ c = NoPeers ->
      {WorldA}
    [] Bug = "include_local"
       /\ c = CapWorldOnly ->
      SpecTargets(c) \cup {LocalPeer}
    [] Bug = "skip_strays_priority"
       /\ c = StraysFillLimit ->
      {WorldA}
    [] Bug = "accept_unregistered_stray"
       /\ c = UnregisteredStrayIgnored ->
      {UnregisteredStray, WorldA}
    [] Bug = "skip_trusted_stray"
       /\ c = TrustedStrayAllowed ->
      {}
    [] Bug = "use_offline_world_when_online_world_exists"
       /\ c = WorldOnlinePreferred ->
      {WorldOffline}
    [] Bug = "skip_world_fallback_when_no_online"
       /\ c = WorldFallbackWhenNoOnline ->
      {}
    [] Bug = "exceed_limit"
       /\ c = CapWorldOnly ->
      {WorldA, WorldB}
    [] Bug = "underfill_after_stray"
       /\ c = StraysThenWorldOnline ->
      {RegisteredStray}
    [] Bug = "ignore_stray_cap"
       /\ c = StraysFillLimit ->
      {RegisteredStray, TrustedStray, WorldA}
    [] OTHER -> SpecTargets(c)

Bugs == {
  "none",
  "zero_limit_nonempty",
  "no_peers_nonempty",
  "include_local",
  "skip_strays_priority",
  "accept_unregistered_stray",
  "skip_trusted_stray",
  "use_offline_world_when_online_world_exists",
  "skip_world_fallback_when_no_online",
  "exceed_limit",
  "underfill_after_stray",
  "ignore_stray_cap"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ Limit(c) \in 0..3
       /\ WorldPeers(c) \subseteq PeerValues
       /\ RegisteredPeers(c) \subseteq PeerValues
       /\ TrustedPeers(c) \subseteq PeerValues
       /\ OnlinePeers(c) \subseteq PeerValues
       /\ Strays(c) \subseteq PeerValues
       /\ WorldOnline(c) \subseteq PeerValues
       /\ WorldCandidates(c) \subseteq PeerValues
       /\ SpecTargets(c) \subseteq PeerValues
       /\ ActualTargets(c) \subseteq PeerValues

TargetSelectionMatchesSpec ==
  \A c \in Cases:
    ActualTargets(c) = SpecTargets(c)

CapAndLocalExclusion ==
  \A c \in Cases:
    /\ Cardinality(ActualTargets(c)) <= Limit(c)
    /\ LocalPeer \notin ActualTargets(c)

EligibilityMatchesInputs ==
  \A c \in Cases:
    ActualTargets(c) \subseteq EligibleTargets(c)

StrayPriorityPreserved ==
  \A c \in Cases:
    /\ (Cardinality(Strays(c)) >= Limit(c) =>
          ActualTargets(c) \subseteq Strays(c))
    /\ (Cardinality(Strays(c)) > 0 /\ Cardinality(Strays(c)) < Limit(c) =>
          Strays(c) \subseteq ActualTargets(c))

WorldOnlinePreferencePreserved ==
  \A c \in Cases:
    WorldOnline(c) # {} =>
      ActualTargets(c) \cap (WorldCandidatesAll(c) \ WorldOnline(c)) = {}

NoBugInvariant ==
  /\ TargetSelectionMatchesSpec
  /\ CapAndLocalExclusion
  /\ EligibilityMatchesInputs
  /\ StrayPriorityPreserved
  /\ WorldOnlinePreferencePreserved

BlockSyncUpdateTargetsExactness ==
  /\ TargetSelectionMatchesSpec
  /\ CapAndLocalExclusion
  /\ EligibilityMatchesInputs
  /\ StrayPriorityPreserved
  /\ WorldOnlinePreferencePreserved

BlockSyncUpdateTargetsCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncUpdateTargetsExactness

SafetyFast ==
  BlockSyncUpdateTargetsExactness

BugZeroLimitNonempty == NoBugInvariant
BugNoPeersNonempty == NoBugInvariant
BugIncludeLocal == NoBugInvariant
BugSkipStraysPriority == NoBugInvariant
BugAcceptUnregisteredStray == NoBugInvariant
BugSkipTrustedStray == NoBugInvariant
BugUseOfflineWorldWhenOnlineWorldExists == NoBugInvariant
BugSkipWorldFallbackWhenNoOnline == NoBugInvariant
BugExceedLimit == NoBugInvariant
BugUnderfillAfterStray == NoBugInvariant
BugIgnoreStrayCap == NoBugInvariant

====
