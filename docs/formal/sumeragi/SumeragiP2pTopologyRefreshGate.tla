---- MODULE SumeragiP2pTopologyRefreshGate ----
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
A bounded abstract model for P2P topology refresh coordination.

This slice captures `topology_refresh_decision(...)`,
`topology_advertisement_for_refresh(...)`,
`topology_update_for_local_removal(...)`, and the branch structure in
`refresh_p2p_topology_with_current(...)`. The refresh path must skip empty and
unchanged peer sets, rebroadcast unchanged topology when strays are online,
advertise changed world topology, latch the local peer once seen, and disconnect
through an empty gossip topology only after the local peer has previously
appeared in a non-empty world peer set and is then removed.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoPeers == 1
Unchanged == 2
AdvertiseChanged == 3
AdvertiseForStrays == 4
LocalRemoved == 5

Local == 1
Trusted == {4}

Cases == {
  "no_peers_empty_unseen",
  "unchanged_clean",
  "changed_topology",
  "changed_with_trusted_network",
  "unchanged_with_strays",
  "first_seen_local",
  "absent_before_seen",
  "removed_after_seen",
  "empty_after_seen",
  "local_returns"
}

Current(c) ==
  CASE c = "no_peers_empty_unseen" -> {}
    [] c = "unchanged_clean" -> {1, 2}
    [] c = "changed_topology" -> {1, 2, 3}
    [] c = "changed_with_trusted_network" -> {1, 2}
    [] c = "unchanged_with_strays" -> {1, 2}
    [] c = "first_seen_local" -> {1, 3}
    [] c = "absent_before_seen" -> {2, 3}
    [] c = "removed_after_seen" -> {2, 3}
    [] c = "empty_after_seen" -> {}
    [] c = "local_returns" -> {1, 2}
    [] OTHER -> {}

LastAdvertised(c) ==
  CASE c = "no_peers_empty_unseen" -> {2}
    [] c = "unchanged_clean" -> {1, 2}
    [] c = "changed_topology" -> {1, 2}
    [] c = "changed_with_trusted_network" -> {1}
    [] c = "unchanged_with_strays" -> {1, 2}
    [] c = "first_seen_local" -> {}
    [] c = "absent_before_seen" -> {}
    [] c = "removed_after_seen" -> {1, 2, 3}
    [] c = "empty_after_seen" -> {1, 2}
    [] c = "local_returns" -> {}
    [] OTHER -> {}

InitialSeen(c) ==
  CASE c = "no_peers_empty_unseen" -> FALSE
    [] c = "unchanged_clean" -> TRUE
    [] c = "changed_topology" -> TRUE
    [] c = "changed_with_trusted_network" -> TRUE
    [] c = "unchanged_with_strays" -> TRUE
    [] c = "first_seen_local" -> FALSE
    [] c = "absent_before_seen" -> FALSE
    [] c = "removed_after_seen" -> TRUE
    [] c = "empty_after_seen" -> TRUE
    [] c = "local_returns" -> TRUE
    [] OTHER -> FALSE

OnlineStrays(c) ==
  CASE c = "unchanged_with_strays" -> <<5, 6>>
    [] OTHER -> <<>>

NetworkTopology(advertise) ==
  advertise \cup {Local} \cup Trusted

LocalInWorld(c) ==
  Local \in Current(c)

SpecSeenAfter(c) ==
  InitialSeen(c) \/ LocalInWorld(c)

SpecRemoved(c) ==
  SpecSeenAfter(c) /\ ~LocalInWorld(c) /\ Current(c) # {}

SpecDecision(c) ==
  IF SpecRemoved(c) THEN LocalRemoved
  ELSE IF Current(c) = {} THEN NoPeers
  ELSE IF Current(c) = LastAdvertised(c) THEN
    IF Len(OnlineStrays(c)) = 0 THEN Unchanged ELSE AdvertiseForStrays
  ELSE AdvertiseChanged

SpecLastAfter(c) ==
  CASE SpecDecision(c) = LocalRemoved -> {}
    [] SpecDecision(c) \in {AdvertiseChanged, AdvertiseForStrays} -> Current(c)
    [] OTHER -> LastAdvertised(c)

SpecGossiperSent(c) ==
  SpecDecision(c) \in {AdvertiseChanged, AdvertiseForStrays, LocalRemoved}

SpecGossiperTopology(c) ==
  IF SpecDecision(c) = LocalRemoved THEN {}
  ELSE IF SpecDecision(c) \in {AdvertiseChanged, AdvertiseForStrays} THEN Current(c)
  ELSE {}

SpecNetworkSent(c) ==
  SpecDecision(c) \in {AdvertiseChanged, AdvertiseForStrays}

SpecNetworkTopology(c) ==
  IF SpecNetworkSent(c) THEN NetworkTopology(Current(c)) ELSE {}

SpecStrayCount(c) ==
  IF SpecDecision(c) = AdvertiseForStrays THEN Len(OnlineStrays(c)) ELSE 0

ActualSeenAfter(c) ==
  CASE c = "first_seen_local" /\ Bug = "local_seen_not_latched" -> FALSE
    [] OTHER -> SpecSeenAfter(c)

ActualRemoved(c) ==
  CASE c = "no_peers_empty_unseen" /\ Bug = "no_peers_sets_removed" -> TRUE
    [] c = "absent_before_seen" /\ Bug = "removed_before_seen" -> TRUE
    [] c = "empty_after_seen" /\ Bug = "removed_when_current_empty" -> TRUE
    [] c = "local_returns" /\ Bug = "local_return_keeps_removed" -> TRUE
    [] c = "removed_after_seen" /\ Bug = "removed_status_not_set" -> FALSE
    [] c = "changed_topology" /\ Bug = "normal_refresh_sets_removed" -> TRUE
    [] OTHER -> SpecRemoved(c)

ActualDecision(c) ==
  CASE c = "unchanged_with_strays" /\ Bug = "strays_ignored" -> Unchanged
    [] c = "absent_before_seen" /\ Bug = "removed_before_seen" -> LocalRemoved
    [] c = "empty_after_seen" /\ Bug = "removed_when_current_empty" -> LocalRemoved
    [] c = "local_returns" /\ Bug = "local_return_keeps_removed" -> LocalRemoved
    [] OTHER -> SpecDecision(c)

ActualLastAfter(c) ==
  CASE c = "unchanged_clean" /\ Bug = "unchanged_mutates_last" -> {}
    [] c = "changed_topology" /\ Bug = "changed_skips_advertise" ->
      LastAdvertised(c)
    [] c = "changed_topology" /\ Bug = "changed_advertises_last" ->
      LastAdvertised(c)
    [] c = "unchanged_with_strays" /\ Bug \in {"strays_ignored", "strays_skip_advertise"} ->
      LastAdvertised(c)
    [] c = "removed_after_seen" /\ Bug = "removed_keeps_last" ->
      LastAdvertised(c)
    [] ActualDecision(c) = LocalRemoved -> {}
    [] ActualDecision(c) \in {AdvertiseChanged, AdvertiseForStrays} -> Current(c)
    [] OTHER -> LastAdvertised(c)

ActualGossiperSent(c) ==
  CASE c = "no_peers_empty_unseen" /\ Bug = "no_peers_advertises" -> TRUE
    [] c = "unchanged_clean" /\ Bug = "unchanged_rebroadcasts" -> TRUE
    [] c = "changed_topology" /\ Bug = "changed_skips_advertise" -> FALSE
    [] c = "unchanged_with_strays" /\ Bug \in {"strays_ignored", "strays_skip_advertise"} ->
      FALSE
    [] c = "removed_after_seen" /\ Bug = "removed_skips_gossip" -> FALSE
    [] OTHER -> ActualDecision(c) \in {AdvertiseChanged, AdvertiseForStrays, LocalRemoved}

ActualGossiperTopology(c) ==
  CASE c = "unchanged_clean" /\ Bug = "unchanged_rebroadcasts" -> Current(c)
    [] c = "changed_topology" /\ Bug = "changed_advertises_last" ->
      LastAdvertised(c)
    [] c = "changed_with_trusted_network" /\ Bug = "changed_gossip_includes_trusted" ->
      NetworkTopology(Current(c))
    [] c = "removed_after_seen" /\ Bug = "removed_gossip_keeps_last" ->
      LastAdvertised(c)
    [] ActualDecision(c) = LocalRemoved -> {}
    [] ActualDecision(c) \in {AdvertiseChanged, AdvertiseForStrays} -> Current(c)
    [] OTHER -> {}

ActualNetworkSent(c) ==
  CASE c = "removed_after_seen" /\ Bug = "removed_runs_normal_network" -> TRUE
    [] c = "changed_topology" /\ Bug = "changed_skips_advertise" -> FALSE
    [] c = "unchanged_with_strays" /\ Bug \in {"strays_ignored", "strays_skip_advertise"} ->
      FALSE
    [] OTHER -> ActualDecision(c) \in {AdvertiseChanged, AdvertiseForStrays}

ActualNetworkTopology(c) ==
  CASE c = "changed_with_trusted_network" /\ Bug = "changed_network_omits_trusted" ->
      Current(c) \cup {Local}
    [] c = "removed_after_seen" /\ Bug = "removed_runs_normal_network" ->
      NetworkTopology(LastAdvertised(c))
    [] ActualNetworkSent(c) -> NetworkTopology(Current(c))
    [] OTHER -> {}

ActualQueueCleared(c) ==
  CASE c = "removed_after_seen" /\ Bug = "removed_does_not_clear_queue" -> FALSE
    [] OTHER -> ActualDecision(c) = LocalRemoved

ActualStrayCount(c) ==
  CASE c = "unchanged_with_strays" /\ Bug = "strays_wrong_count" -> 0
    [] ActualDecision(c) = AdvertiseForStrays -> Len(OnlineStrays(c))
    [] OTHER -> 0

SpecCase(c) ==
  <<SpecDecision(c),
    SpecLastAfter(c),
    SpecRemoved(c),
    SpecSeenAfter(c),
    SpecDecision(c) = LocalRemoved,
    SpecGossiperSent(c),
    SpecGossiperTopology(c),
    SpecNetworkSent(c),
    SpecNetworkTopology(c),
    SpecStrayCount(c)>>

ActualCase(c) ==
  <<ActualDecision(c),
    ActualLastAfter(c),
    ActualRemoved(c),
    ActualSeenAfter(c),
    ActualQueueCleared(c),
    ActualGossiperSent(c),
    ActualGossiperTopology(c),
    ActualNetworkSent(c),
    ActualNetworkTopology(c),
    ActualStrayCount(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "no_peers_advertises",
       "no_peers_sets_removed",
       "unchanged_rebroadcasts",
       "changed_skips_advertise",
       "changed_advertises_last",
       "changed_network_omits_trusted",
       "changed_gossip_includes_trusted",
       "strays_ignored",
       "strays_wrong_count",
       "strays_skip_advertise",
       "local_seen_not_latched",
       "removed_before_seen",
       "removed_when_current_empty",
       "removed_status_not_set",
       "removed_does_not_clear_queue",
       "removed_keeps_last",
       "removed_skips_gossip",
       "removed_runs_normal_network",
       "local_return_keeps_removed",
       "normal_refresh_sets_removed",
       "unchanged_mutates_last",
       "removed_gossip_keeps_last"
     }
  /\ checked = 0

\* @type: Seq(<<Int, Set(Int), Bool, Bool, Bool, Bool, Set(Int), Bool, Set(Int), Int>>);
SpecOutput ==
  <<SpecCase("no_peers_empty_unseen"),
    SpecCase("unchanged_clean"),
    SpecCase("changed_topology"),
    SpecCase("changed_with_trusted_network"),
    SpecCase("unchanged_with_strays"),
    SpecCase("first_seen_local"),
    SpecCase("absent_before_seen"),
    SpecCase("removed_after_seen"),
    SpecCase("empty_after_seen"),
    SpecCase("local_returns")>>

\* @type: Seq(<<Int, Set(Int), Bool, Bool, Bool, Bool, Set(Int), Bool, Set(Int), Int>>);
ActualOutput ==
  <<ActualCase("no_peers_empty_unseen"),
    ActualCase("unchanged_clean"),
    ActualCase("changed_topology"),
    ActualCase("changed_with_trusted_network"),
    ActualCase("unchanged_with_strays"),
    ActualCase("first_seen_local"),
    ActualCase("absent_before_seen"),
    ActualCase("removed_after_seen"),
    ActualCase("empty_after_seen"),
    ActualCase("local_returns")>>

Safety ==
  ActualOutput = SpecOutput

BugNoPeersAdvertises ==
  ActualCase("no_peers_empty_unseen") = SpecCase("no_peers_empty_unseen")

BugNoPeersSetsRemoved ==
  ActualCase("no_peers_empty_unseen") = SpecCase("no_peers_empty_unseen")

BugUnchangedRebroadcasts ==
  ActualCase("unchanged_clean") = SpecCase("unchanged_clean")

BugChangedSkipsAdvertise ==
  ActualCase("changed_topology") = SpecCase("changed_topology")

BugChangedAdvertisesLast ==
  ActualCase("changed_topology") = SpecCase("changed_topology")

BugChangedNetworkOmitsTrusted ==
  ActualCase("changed_with_trusted_network") =
    SpecCase("changed_with_trusted_network")

BugChangedGossipIncludesTrusted ==
  ActualCase("changed_with_trusted_network") =
    SpecCase("changed_with_trusted_network")

BugStraysIgnored ==
  ActualCase("unchanged_with_strays") = SpecCase("unchanged_with_strays")

BugStraysWrongCount ==
  ActualCase("unchanged_with_strays") = SpecCase("unchanged_with_strays")

BugStraysSkipAdvertise ==
  ActualCase("unchanged_with_strays") = SpecCase("unchanged_with_strays")

BugLocalSeenNotLatched ==
  ActualCase("first_seen_local") = SpecCase("first_seen_local")

BugRemovedBeforeSeen ==
  ActualCase("absent_before_seen") = SpecCase("absent_before_seen")

BugRemovedWhenCurrentEmpty ==
  ActualCase("empty_after_seen") = SpecCase("empty_after_seen")

BugRemovedStatusNotSet ==
  ActualCase("removed_after_seen") = SpecCase("removed_after_seen")

BugRemovedDoesNotClearQueue ==
  ActualCase("removed_after_seen") = SpecCase("removed_after_seen")

BugRemovedKeepsLast ==
  ActualCase("removed_after_seen") = SpecCase("removed_after_seen")

BugRemovedSkipsGossip ==
  ActualCase("removed_after_seen") = SpecCase("removed_after_seen")

BugRemovedRunsNormalNetwork ==
  ActualCase("removed_after_seen") = SpecCase("removed_after_seen")

BugLocalReturnKeepsRemoved ==
  ActualCase("local_returns") = SpecCase("local_returns")

BugNormalRefreshSetsRemoved ==
  ActualCase("changed_topology") = SpecCase("changed_topology")

BugUnchangedMutatesLast ==
  ActualCase("unchanged_clean") = SpecCase("unchanged_clean")

BugRemovedGossipKeepsLast ==
  ActualCase("removed_after_seen") = SpecCase("removed_after_seen")

====
