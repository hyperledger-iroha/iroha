---- MODULE SumeragiMembershipViewHashGate ----
EXTENDS Naturals, Sequences

(***************************************************************************
A bounded abstract model for Sumeragi membership-view hash construction.

This slice captures the preimage contract of
`compute_membership_view_hash(...)` from `main_loop/roster.rs`. The Rust helper
hashes, in order, the chain id bytes, big-endian height, big-endian view,
big-endian epoch, and every peer's Norito encoding. The model abstracts the
cryptographic hash into an exact token sequence: if a field is omitted,
reordered, normalized away, or ignored, the modeled preimage changes.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

BaseAB == "base_ab"
ChainBCase == "chain_b"
Height6Case == "height_6"
View3Case == "view_3"
Epoch2Case == "epoch_2"
ReorderedBA == "reordered_ba"
AddedABC == "added_abc"
DuplicateAA == "duplicate_aa"
SingleA == "single_a"
EmptyPeers == "empty_peers"

Cases == {
  BaseAB,
  ChainBCase,
  Height6Case,
  View3Case,
  Epoch2Case,
  ReorderedBA,
  AddedABC,
  DuplicateAA,
  SingleA,
  EmptyPeers
}

MembershipBaseCases == {
  BaseAB
}

MembershipContextChangeCases == {
  ChainBCase,
  Height6Case,
  View3Case,
  Epoch2Case
}

MembershipPeerOrderCases == {
  ReorderedBA
}

MembershipPeerCardinalityCases == {
  AddedABC,
  DuplicateAA,
  SingleA,
  EmptyPeers
}

ChainA == 1
ChainB == 2
Height5 == 3
Height6 == 4
View2 == 5
View3 == 6
Epoch1 == 7
Epoch2 == 8
PeerA == 9
PeerB == 10
PeerC == 11
PeerPlaceholder == 12

Fields == {
  ChainA,
  ChainB,
  Height5,
  Height6,
  View2,
  View3,
  Epoch1,
  Epoch2,
  PeerA,
  PeerB,
  PeerC,
  PeerPlaceholder
}

SpecPreimage(c) ==
  CASE c = BaseAB ->
      <<ChainA, Height5, View2, Epoch1, PeerA, PeerB>>
    [] c = ChainBCase ->
      <<ChainB, Height5, View2, Epoch1, PeerA, PeerB>>
    [] c = Height6Case ->
      <<ChainA, Height6, View2, Epoch1, PeerA, PeerB>>
    [] c = View3Case ->
      <<ChainA, Height5, View3, Epoch1, PeerA, PeerB>>
    [] c = Epoch2Case ->
      <<ChainA, Height5, View2, Epoch2, PeerA, PeerB>>
    [] c = ReorderedBA ->
      <<ChainA, Height5, View2, Epoch1, PeerB, PeerA>>
    [] c = AddedABC ->
      <<ChainA, Height5, View2, Epoch1, PeerA, PeerB, PeerC>>
    [] c = DuplicateAA ->
      <<ChainA, Height5, View2, Epoch1, PeerA, PeerA>>
    [] c = SingleA ->
      <<ChainA, Height5, View2, Epoch1, PeerA>>
    [] c = EmptyPeers ->
      <<ChainA, Height5, View2, Epoch1>>
    [] OTHER ->
      <<>>

ActualPreimage(c) ==
  LET spec == SpecPreimage(c) IN
  CASE Bug = "drop_chain_id"
       /\ c = BaseAB ->
      <<Height5, View2, Epoch1, PeerA, PeerB>>
    [] Bug = "drop_height"
       /\ c = BaseAB ->
      <<ChainA, View2, Epoch1, PeerA, PeerB>>
    [] Bug = "drop_view"
       /\ c = BaseAB ->
      <<ChainA, Height5, Epoch1, PeerA, PeerB>>
    [] Bug = "drop_epoch"
       /\ c = BaseAB ->
      <<ChainA, Height5, View2, PeerA, PeerB>>
    [] Bug = "swap_height_view"
       /\ c = BaseAB ->
      <<ChainA, View2, Height5, Epoch1, PeerA, PeerB>>
    [] Bug = "peers_before_epoch"
       /\ c = BaseAB ->
      <<ChainA, Height5, View2, PeerA, PeerB, Epoch1>>
    [] Bug = "reverse_peer_order"
       /\ c = BaseAB ->
      <<ChainA, Height5, View2, Epoch1, PeerB, PeerA>>
    [] Bug = "sort_peer_order"
       /\ c = ReorderedBA ->
      <<ChainA, Height5, View2, Epoch1, PeerA, PeerB>>
    [] Bug = "ignore_added_peer"
       /\ c = AddedABC ->
      <<ChainA, Height5, View2, Epoch1, PeerA, PeerB>>
    [] Bug = "dedup_duplicate_peer"
       /\ c = DuplicateAA ->
      <<ChainA, Height5, View2, Epoch1, PeerA>>
    [] Bug = "empty_inserts_placeholder"
       /\ c = EmptyPeers ->
      <<ChainA, Height5, View2, Epoch1, PeerPlaceholder>>
    [] Bug = "single_peer_dropped"
       /\ c = SingleA ->
      <<ChainA, Height5, View2, Epoch1>>
    [] Bug = "chain_change_ignored"
       /\ c = ChainBCase ->
      <<ChainA, Height5, View2, Epoch1, PeerA, PeerB>>
    [] Bug = "height_change_ignored"
       /\ c = Height6Case ->
      <<ChainA, Height5, View2, Epoch1, PeerA, PeerB>>
    [] Bug = "view_change_ignored"
       /\ c = View3Case ->
      <<ChainA, Height5, View2, Epoch1, PeerA, PeerB>>
    [] Bug = "epoch_change_ignored"
       /\ c = Epoch2Case ->
      <<ChainA, Height5, View2, Epoch1, PeerA, PeerB>>
    [] OTHER -> spec

KnownPreimages == {
  <<>>,
  <<Height5, View2, Epoch1, PeerA, PeerB>>,
  <<ChainA, View2, Epoch1, PeerA, PeerB>>,
  <<ChainA, Height5, Epoch1, PeerA, PeerB>>,
  <<ChainA, Height5, View2, PeerA, PeerB>>,
  <<ChainA, View2, Height5, Epoch1, PeerA, PeerB>>,
  <<ChainA, Height5, View2, PeerA, PeerB, Epoch1>>,
  <<ChainA, Height5, View2, Epoch1>>,
  <<ChainA, Height5, View2, Epoch1, PeerA>>,
  <<ChainA, Height5, View2, Epoch1, PeerB, PeerA>>,
  <<ChainA, Height5, View2, Epoch1, PeerA, PeerB>>,
  <<ChainB, Height5, View2, Epoch1, PeerA, PeerB>>,
  <<ChainA, Height6, View2, Epoch1, PeerA, PeerB>>,
  <<ChainA, Height5, View3, Epoch1, PeerA, PeerB>>,
  <<ChainA, Height5, View2, Epoch2, PeerA, PeerB>>,
  <<ChainA, Height5, View2, Epoch1, PeerA, PeerB, PeerC>>,
  <<ChainA, Height5, View2, Epoch1, PeerA, PeerA>>,
  <<ChainA, Height5, View2, Epoch1, PeerPlaceholder>>
}

Bugs == {
  "none",
  "drop_chain_id",
  "drop_height",
  "drop_view",
  "drop_epoch",
  "swap_height_view",
  "peers_before_epoch",
  "reverse_peer_order",
  "sort_peer_order",
  "ignore_added_peer",
  "dedup_duplicate_peer",
  "empty_inserts_placeholder",
  "single_peer_dropped",
  "chain_change_ignored",
  "height_change_ignored",
  "view_change_ignored",
  "epoch_change_ignored"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecPreimage(c) \in KnownPreimages
       /\ ActualPreimage(c) \in KnownPreimages

MembershipViewHashCoreSafety ==
  \A c \in Cases:
    ActualPreimage(c) = SpecPreimage(c)

MembershipViewHashBaseExact ==
  \A c \in MembershipBaseCases:
    ActualPreimage(c) = SpecPreimage(c)

MembershipViewHashContextExact ==
  \A c \in MembershipContextChangeCases:
    ActualPreimage(c) = SpecPreimage(c)

MembershipViewHashPeerOrderExact ==
  \A c \in MembershipPeerOrderCases:
    ActualPreimage(c) = SpecPreimage(c)

MembershipViewHashPeerCardinalityExact ==
  \A c \in MembershipPeerCardinalityCases:
    ActualPreimage(c) = SpecPreimage(c)

MembershipViewHashExactness ==
  /\ MembershipViewHashBaseExact
  /\ MembershipViewHashContextExact
  /\ MembershipViewHashPeerOrderExact
  /\ MembershipViewHashPeerCardinalityExact

MembershipViewHashCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ MembershipViewHashExactness

NoBugInvariant == MembershipViewHashCoreSafety

SafetyFast == MembershipViewHashCoreSafety

BugDropChainId == NoBugInvariant
BugDropHeight == NoBugInvariant
BugDropView == NoBugInvariant
BugDropEpoch == NoBugInvariant
BugSwapHeightView == NoBugInvariant
BugPeersBeforeEpoch == NoBugInvariant
BugReversePeerOrder == NoBugInvariant
BugSortPeerOrder == NoBugInvariant
BugIgnoreAddedPeer == NoBugInvariant
BugDedupDuplicatePeer == NoBugInvariant
BugEmptyInsertsPlaceholder == NoBugInvariant
BugSinglePeerDropped == NoBugInvariant
BugChainChangeIgnored == NoBugInvariant
BugHeightChangeIgnored == NoBugInvariant
BugViewChangeIgnored == NoBugInvariant
BugEpochChangeIgnored == NoBugInvariant

====
