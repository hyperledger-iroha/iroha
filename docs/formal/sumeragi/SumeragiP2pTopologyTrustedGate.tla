---- MODULE SumeragiP2pTopologyTrustedGate ----

EXTENDS Integers, Sequences, FiniteSets

(***************************************************************************
A bounded abstract model for trusted-peer P2P topology refresh helpers.

`p2p_topology_with_trusted(...)` forms the expected P2P topology by unioning
world-state peers, the local trusted peer, and configured trusted peers into a
deduplicated `BTreeSet`. `peer_ids_outside_topology(...)` then preserves the
network-observed peer order while returning only online peers that are absent
from that expected topology.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "world_only",
  "trusted_observer",
  "trusted_dedup",
  "local_absent_world",
  "online_order_duplicates",
  "empty_world_trusted"
}

World(c) ==
  CASE c = "world_only" -> {2, 3}
    [] c = "trusted_observer" -> {1, 2}
    [] c = "trusted_dedup" -> {1, 2, 3}
    [] c = "local_absent_world" -> {2}
    [] c = "online_order_duplicates" -> {1}
    [] c = "empty_world_trusted" -> {}
    [] OTHER -> {}

Local(c) ==
  1

Trusted(c) ==
  CASE c = "trusted_observer" -> {3}
    [] c = "trusted_dedup" -> {2, 3, 4}
    [] c = "online_order_duplicates" -> {4}
    [] c = "empty_world_trusted" -> {2}
    [] OTHER -> {}

Online(c) ==
  CASE c = "world_only" -> <<1, 2, 3, 4>>
    [] c = "trusted_observer" -> <<1, 2, 3, 4>>
    [] c = "trusted_dedup" -> <<4, 5, 4>>
    [] c = "local_absent_world" -> <<1, 2, 3>>
    [] c = "online_order_duplicates" -> <<5, 2, 4, 5, 3>>
    [] c = "empty_world_trusted" -> <<1, 2, 3>>
    [] OTHER -> <<>>

SeqAtIfOutside(topology, seq, idx) ==
  IF Len(seq) >= idx /\ seq[idx] \notin topology THEN <<seq[idx]>> ELSE <<>>

SeqAtIfInside(topology, seq, idx) ==
  IF Len(seq) >= idx /\ seq[idx] \in topology THEN <<seq[idx]>> ELSE <<>>

OutsideFilter(topology, seq) ==
  SeqAtIfOutside(topology, seq, 1) \o
  SeqAtIfOutside(topology, seq, 2) \o
  SeqAtIfOutside(topology, seq, 3) \o
  SeqAtIfOutside(topology, seq, 4) \o
  SeqAtIfOutside(topology, seq, 5)

InsideFilter(topology, seq) ==
  SeqAtIfInside(topology, seq, 1) \o
  SeqAtIfInside(topology, seq, 2) \o
  SeqAtIfInside(topology, seq, 3) \o
  SeqAtIfInside(topology, seq, 4) \o
  SeqAtIfInside(topology, seq, 5)

SpecTopology(c) ==
  World(c) \cup {Local(c)} \cup Trusted(c)

SpecTopologySize(c) ==
  Cardinality(SpecTopology(c))

SpecStrays(c) ==
  OutsideFilter(SpecTopology(c), Online(c))

ActualTopology(c) ==
  IF Bug = 1 THEN World(c) \cup Trusted(c)
  ELSE IF Bug = 2 THEN World(c) \cup {Local(c)}
  ELSE IF Bug = 3 THEN {Local(c)} \cup Trusted(c)
  ELSE SpecTopology(c)

ActualTopologySize(c) ==
  IF Bug = 9
  THEN Cardinality(World(c)) + 1 + Cardinality(Trusted(c))
  ELSE Cardinality(ActualTopology(c))

SortedDedupStrays(c) ==
  IF c = "online_order_duplicates" THEN <<2, 3, 5>> ELSE SpecStrays(c)

FirstOccurrenceStrays(c) ==
  IF c = "online_order_duplicates" THEN <<5, 2, 3>> ELSE SpecStrays(c)

ActualStrays(c) ==
  IF Bug = 4 THEN InsideFilter(ActualTopology(c), Online(c))
  ELSE IF Bug = 5 THEN <<>>
  ELSE IF Bug = 6 THEN SortedDedupStrays(c)
  ELSE IF Bug = 7 THEN FirstOccurrenceStrays(c)
  ELSE IF Bug = 8 THEN OutsideFilter(ActualTopology(c) \ Trusted(c), Online(c))
  ELSE OutsideFilter(ActualTopology(c), Online(c))

SpecCase(c) ==
  <<SpecTopology(c), SpecTopologySize(c), SpecStrays(c)>>

ActualCase(c) ==
  <<ActualTopology(c), ActualTopologySize(c), ActualStrays(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

\* @type: <<<<Set(Int), Int, Seq(Int)>>, <<Set(Int), Int, Seq(Int)>>, <<Set(Int), Int, Seq(Int)>>, <<Set(Int), Int, Seq(Int)>>, <<Set(Int), Int, Seq(Int)>>, <<Set(Int), Int, Seq(Int)>>>>;
SpecOutput ==
  <<SpecCase("world_only"),
    SpecCase("trusted_observer"),
    SpecCase("trusted_dedup"),
    SpecCase("local_absent_world"),
    SpecCase("online_order_duplicates"),
    SpecCase("empty_world_trusted")>>

\* @type: <<<<Set(Int), Int, Seq(Int)>>, <<Set(Int), Int, Seq(Int)>>, <<Set(Int), Int, Seq(Int)>>, <<Set(Int), Int, Seq(Int)>>, <<Set(Int), Int, Seq(Int)>>, <<Set(Int), Int, Seq(Int)>>>>;
ActualOutput ==
  <<ActualCase("world_only"),
    ActualCase("trusted_observer"),
    ActualCase("trusted_dedup"),
    ActualCase("local_absent_world"),
    ActualCase("online_order_duplicates"),
    ActualCase("empty_world_trusted")>>

TrustedP2pTopologyOutputMatchesSpec ==
  ActualOutput = SpecOutput

SafetyFast ==
  TrustedP2pTopologyOutputMatchesSpec

TrustedTopologyUnionExact ==
  \A c \in Cases:
    /\ ActualTopology(c) = SpecTopology(c)
    /\ ActualTopologySize(c) = SpecTopologySize(c)

OutsidePeerFilteringExact ==
  \A c \in Cases:
    ActualStrays(c) = SpecStrays(c)

TrustedObserverNonStrayExact ==
  ActualCase("trusted_observer") = SpecCase("trusted_observer")

OnlineStrayOrderDuplicateExact ==
  ActualCase("online_order_duplicates") =
    SpecCase("online_order_duplicates")

TrustedP2pTopologyExactness ==
  /\ TrustedP2pTopologyOutputMatchesSpec
  /\ TrustedTopologyUnionExact
  /\ OutsidePeerFilteringExact
  /\ TrustedObserverNonStrayExact
  /\ OnlineStrayOrderDuplicateExact

BugIncludesLocal ==
  ActualCase("local_absent_world") = SpecCase("local_absent_world")

BugIncludesTrusted ==
  ActualCase("trusted_observer") = SpecCase("trusted_observer")

BugIncludesWorld ==
  ActualCase("world_only") = SpecCase("world_only")

BugOnlyOutside ==
  ActualCase("world_only") = SpecCase("world_only")

BugKeepsStrays ==
  ActualCase("world_only") = SpecCase("world_only")

BugPreservesOnlineOrder ==
  ActualCase("online_order_duplicates") = SpecCase("online_order_duplicates")

BugPreservesDuplicateStrays ==
  ActualCase("online_order_duplicates") = SpecCase("online_order_duplicates")

BugTrustedNotStray ==
  ActualCase("trusted_observer") = SpecCase("trusted_observer")

BugTopologyDedupSize ==
  ActualCase("trusted_dedup") = SpecCase("trusted_dedup")

====
