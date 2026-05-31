---- MODULE SumeragiSignerIndexNormalizationGate ----
EXTENDS Integers, Sequences, FiniteSets

(***************************************************************************
A bounded abstract model for signer-index normalization between canonical and
view-specific Sumeragi topologies.

This slice models `normalize_signer_indices_to_canonical(...)`,
`view_index_for_canonical_signer(...)`, and
`normalize_signer_indices_to_view(...)` from `main_loop.rs`. The helpers must
map indexes through peer identity rather than numeric position so PRF-shuffled
or otherwise view-specific topologies do not corrupt QC/vote signer
projections. Empty inputs stay empty, out-of-range or absent peers are ignored
or return `None`, and round-tripping common peers through view indexes restores
the canonical signer set.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoIndex == 99

Cases == {
  "empty_to_canonical",
  "rotated_to_canonical_single",
  "rotated_to_canonical_many",
  "to_canonical_oob",
  "to_canonical_absent_peer",
  "to_canonical_duplicate_peer",
  "empty_to_view",
  "rotated_to_view_single",
  "rotated_to_view_many",
  "view_index_empty_canonical",
  "view_index_oob",
  "view_index_absent_peer",
  "view_index_rotated",
  "to_view_empty_canonical",
  "to_view_absent_peer_filters",
  "to_view_keeps_zero",
  "roundtrip_rotated_common",
  "roundtrip_missing_peer_filters"
}

\* @type: Str => Seq(Int);
CanonicalTopology(c) ==
  CASE c \in {"view_index_empty_canonical", "to_view_empty_canonical"} -> <<>>
    [] c = "to_canonical_duplicate_peer" -> <<1, 2>>
    [] OTHER -> <<1, 2, 3>>

\* @type: Str => Seq(Int);
SignatureTopology(c) ==
  CASE c \in {
         "rotated_to_canonical_single",
         "rotated_to_canonical_many",
         "rotated_to_view_single",
         "rotated_to_view_many",
         "view_index_empty_canonical",
         "view_index_oob",
         "view_index_rotated",
         "to_view_empty_canonical",
         "roundtrip_rotated_common"
       } -> <<3, 1, 2>>
    [] c = "to_canonical_absent_peer" -> <<4, 1>>
    [] c = "to_canonical_duplicate_peer" -> <<2, 2, 1>>
    [] c \in {
         "view_index_absent_peer",
         "to_view_absent_peer_filters",
         "roundtrip_missing_peer_filters"
       } -> <<3, 1>>
    [] c = "to_view_keeps_zero" -> <<1, 3>>
    [] OTHER -> <<1, 2, 3>>

\* @type: Str => Set(Int);
InputSigners(c) ==
  CASE c = "empty_to_canonical" -> {}
    [] c = "rotated_to_canonical_single" -> {0}
    [] c = "rotated_to_canonical_many" -> {0, 2}
    [] c = "to_canonical_oob" -> {3}
    [] c = "to_canonical_absent_peer" -> {0, 1}
    [] c = "to_canonical_duplicate_peer" -> {0, 1, 2}
    [] c = "empty_to_view" -> {}
    [] c = "rotated_to_view_single" -> {2}
    [] c = "rotated_to_view_many" -> {0, 2}
    [] c = "to_view_empty_canonical" -> {0}
    [] c = "to_view_absent_peer_filters" -> {1, 2}
    [] c = "to_view_keeps_zero" -> {0, 2}
    [] c = "roundtrip_rotated_common" -> {0, 2}
    [] c = "roundtrip_missing_peer_filters" -> {0, 1, 2}
    [] OTHER -> {}

\* @type: Str => Int;
InputIndex(c) ==
  CASE c = "view_index_oob" -> 3
    [] c = "view_index_absent_peer" -> 1
    [] c = "view_index_rotated" -> 2
    [] OTHER -> 0

\* @type: (Seq(Int), Int) => Bool;
HasIndex(topology, idx) ==
  idx >= 0 /\ idx < Len(topology)

\* @type: (Seq(Int), Int) => Int;
PeerAt(topology, idx) ==
  topology[idx + 1]

\* @type: (Seq(Int), Int) => Bool;
HasPeer(topology, peer) ==
  \/ Len(topology) >= 1 /\ topology[1] = peer
  \/ Len(topology) >= 2 /\ topology[2] = peer
  \/ Len(topology) >= 3 /\ topology[3] = peer

\* @type: (Seq(Int), Int) => Int;
IndexOfPeer(topology, peer) ==
  CASE Len(topology) >= 1 /\ topology[1] = peer -> 0
    [] Len(topology) >= 2 /\ topology[2] = peer -> 1
    [] Len(topology) >= 3 /\ topology[3] = peer -> 2
    [] OTHER -> NoIndex

\* @type: (Seq(Int), Seq(Int), Int) => Int;
CanonicalIndexForViewSigner(signers_topology, canonical_topology, signer) ==
  IF HasIndex(signers_topology, signer) THEN
    LET peer == PeerAt(signers_topology, signer) IN
      IF HasPeer(canonical_topology, peer) THEN
        IndexOfPeer(canonical_topology, peer)
      ELSE
        NoIndex
  ELSE
    NoIndex

\* @type: (Set(Int), Seq(Int), Seq(Int)) => Set(Int);
ValidViewSigners(signers, signers_topology, canonical_topology) ==
  {signer \in signers:
    CanonicalIndexForViewSigner(signers_topology, canonical_topology, signer) # NoIndex}

\* @type: (Set(Int), Seq(Int), Seq(Int)) => Set(Int);
NormalizeToCanonical(signers, signers_topology, canonical_topology) ==
  {CanonicalIndexForViewSigner(signers_topology, canonical_topology, signer):
    signer \in ValidViewSigners(signers, signers_topology, canonical_topology)}

\* @type: (Seq(Int), Seq(Int), Int) => Int;
ViewIndexForCanonicalSigner(signers_topology, canonical_topology, signer) ==
  IF Len(canonical_topology) = 0 THEN
    NoIndex
  ELSE IF HasIndex(canonical_topology, signer) THEN
    LET peer == PeerAt(canonical_topology, signer) IN
      IF HasPeer(signers_topology, peer) THEN
        IndexOfPeer(signers_topology, peer)
      ELSE
        NoIndex
  ELSE
    NoIndex

\* @type: (Set(Int), Seq(Int), Seq(Int)) => Set(Int);
ValidCanonicalSigners(signers, signers_topology, canonical_topology) ==
  {signer \in signers:
    ViewIndexForCanonicalSigner(signers_topology, canonical_topology, signer) # NoIndex}

\* @type: (Set(Int), Seq(Int), Seq(Int)) => Set(Int);
NormalizeToView(signers, signers_topology, canonical_topology) ==
  {ViewIndexForCanonicalSigner(signers_topology, canonical_topology, signer):
    signer \in ValidCanonicalSigners(signers, signers_topology, canonical_topology)}

\* @type: Str => Set(Int);
SpecCanonicalSet(c) ==
  NormalizeToCanonical(InputSigners(c), SignatureTopology(c), CanonicalTopology(c))

\* @type: Str => Int;
SpecViewIndex(c) ==
  ViewIndexForCanonicalSigner(SignatureTopology(c), CanonicalTopology(c), InputIndex(c))

\* @type: Str => Set(Int);
SpecViewSet(c) ==
  NormalizeToView(InputSigners(c), SignatureTopology(c), CanonicalTopology(c))

\* @type: Str => Set(Int);
SpecRoundTripSet(c) ==
  NormalizeToCanonical(
    NormalizeToView(InputSigners(c), SignatureTopology(c), CanonicalTopology(c)),
    SignatureTopology(c),
    CanonicalTopology(c)
  )

\* @type: Str => Set(Int);
ActualCanonicalSet(c) ==
  CASE Bug = "canonical_empty_kept"
       /\ c = "empty_to_canonical" -> {0}
    [] Bug = "canonical_uses_numeric_index"
       /\ c = "rotated_to_canonical_single" -> {0}
    [] Bug = "canonical_keeps_out_of_range"
       /\ c = "to_canonical_oob" -> {3}
    [] Bug = "canonical_keeps_absent_peer"
       /\ c = "to_canonical_absent_peer" -> {0, 1}
    [] Bug = "canonical_drops_valid"
       /\ c = "rotated_to_canonical_many" -> {2}
    [] Bug = "canonical_duplicate_peer_drops_other"
       /\ c = "to_canonical_duplicate_peer" -> {1}
    [] OTHER -> SpecCanonicalSet(c)

\* @type: Str => Int;
ActualViewIndex(c) ==
  CASE Bug = "view_empty_canonical_returns_zero"
       /\ c = "view_index_empty_canonical" -> 0
    [] Bug = "view_oob_wraps"
       /\ c = "view_index_oob" -> 0
    [] Bug = "view_missing_peer_returns_input"
       /\ c = "view_index_absent_peer" -> 1
    [] Bug = "view_uses_numeric_index"
       /\ c = "view_index_rotated" -> 2
    [] OTHER -> SpecViewIndex(c)

\* @type: Str => Set(Int);
ActualViewSet(c) ==
  CASE Bug = "view_set_empty_kept"
       /\ c = "empty_to_view" -> {0}
    [] Bug = "view_set_uses_numeric_index"
       /\ c = "rotated_to_view_many" -> {0, 2}
    [] Bug = "view_set_keeps_invalid"
       /\ c = "to_view_absent_peer_filters" -> {1, 2}
    [] Bug = "view_set_empty_canonical_nonempty"
       /\ c = "to_view_empty_canonical" -> {0}
    [] Bug = "view_set_drops_zero"
       /\ c = "to_view_keeps_zero" -> {1}
    [] OTHER -> SpecViewSet(c)

\* @type: Str => Set(Int);
ActualRoundTripSet(c) ==
  CASE Bug = "roundtrip_uses_view_indices"
       /\ c = "roundtrip_rotated_common" ->
         NormalizeToView(InputSigners(c), SignatureTopology(c), CanonicalTopology(c))
    [] Bug = "roundtrip_keeps_missing_peer"
       /\ c = "roundtrip_missing_peer_filters" -> {0, 1, 2}
    [] Bug = "roundtrip_drops_present"
       /\ c = "roundtrip_rotated_common" -> {0}
    [] OTHER -> SpecRoundTripSet(c)

\* @type: Str => <<Set(Int), Int, Set(Int), Set(Int)>>;
SpecResult(c) ==
  <<SpecCanonicalSet(c), SpecViewIndex(c), SpecViewSet(c), SpecRoundTripSet(c)>>

\* @type: Str => <<Set(Int), Int, Set(Int), Set(Int)>>;
ActualResult(c) ==
  <<ActualCanonicalSet(c), ActualViewIndex(c), ActualViewSet(c), ActualRoundTripSet(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in {
       "none",
       "canonical_empty_kept",
       "canonical_uses_numeric_index",
       "canonical_keeps_out_of_range",
       "canonical_keeps_absent_peer",
       "canonical_drops_valid",
       "canonical_duplicate_peer_drops_other",
       "view_empty_canonical_returns_zero",
       "view_oob_wraps",
       "view_missing_peer_returns_input",
       "view_uses_numeric_index",
       "view_set_empty_kept",
       "view_set_uses_numeric_index",
       "view_set_keeps_invalid",
       "view_set_empty_canonical_nonempty",
       "view_set_drops_zero",
       "roundtrip_uses_view_indices",
       "roundtrip_keeps_missing_peer",
       "roundtrip_drops_present"
     }
  /\ \A c \in Cases:
       /\ InputSigners(c) \subseteq 0..4
       /\ InputIndex(c) \in 0..4
       /\ Len(CanonicalTopology(c)) \in 0..3
       /\ Len(SignatureTopology(c)) \in 0..3
       /\ SpecCanonicalSet(c) \subseteq 0..4
       /\ ActualCanonicalSet(c) \subseteq 0..4
       /\ SpecViewIndex(c) \in 0..NoIndex
       /\ ActualViewIndex(c) \in 0..NoIndex
       /\ SpecViewSet(c) \subseteq 0..4
       /\ ActualViewSet(c) \subseteq 0..4
       /\ SpecRoundTripSet(c) \subseteq 0..4
       /\ ActualRoundTripSet(c) \subseteq 0..4

CanonicalNormalizationAnchors ==
  /\ SpecCanonicalSet("empty_to_canonical") = {}
  /\ SpecCanonicalSet("rotated_to_canonical_single") = {2}
  /\ SpecCanonicalSet("rotated_to_canonical_many") = {1, 2}
  /\ SpecCanonicalSet("to_canonical_oob") = {}
  /\ SpecCanonicalSet("to_canonical_absent_peer") = {0}
  /\ SpecCanonicalSet("to_canonical_duplicate_peer") = {0, 1}

ViewIndexAnchors ==
  /\ SpecViewIndex("view_index_empty_canonical") = NoIndex
  /\ SpecViewIndex("view_index_oob") = NoIndex
  /\ SpecViewIndex("view_index_absent_peer") = NoIndex
  /\ SpecViewIndex("view_index_rotated") = 0

ViewSetAnchors ==
  /\ SpecViewSet("empty_to_view") = {}
  /\ SpecViewSet("rotated_to_view_single") = {0}
  /\ SpecViewSet("rotated_to_view_many") = {0, 1}
  /\ SpecViewSet("to_view_empty_canonical") = {}
  /\ SpecViewSet("to_view_absent_peer_filters") = {0}
  /\ SpecViewSet("to_view_keeps_zero") = {0, 1}

RoundTripAnchors ==
  /\ SpecRoundTripSet("roundtrip_rotated_common") = {0, 2}
  /\ SpecRoundTripSet("roundtrip_missing_peer_filters") = {0, 2}

SafetyFast ==
  \A c \in Cases: ActualResult(c) = SpecResult(c)

BugCanonicalEmptyKept ==
  ActualResult("empty_to_canonical") = SpecResult("empty_to_canonical")

BugCanonicalUsesNumericIndex ==
  ActualResult("rotated_to_canonical_single") =
    SpecResult("rotated_to_canonical_single")

BugCanonicalKeepsOutOfRange ==
  ActualResult("to_canonical_oob") = SpecResult("to_canonical_oob")

BugCanonicalKeepsAbsentPeer ==
  ActualResult("to_canonical_absent_peer") =
    SpecResult("to_canonical_absent_peer")

BugCanonicalDropsValid ==
  ActualResult("rotated_to_canonical_many") =
    SpecResult("rotated_to_canonical_many")

BugCanonicalDuplicatePeerDropsOther ==
  ActualResult("to_canonical_duplicate_peer") =
    SpecResult("to_canonical_duplicate_peer")

BugViewEmptyCanonicalReturnsZero ==
  ActualResult("view_index_empty_canonical") =
    SpecResult("view_index_empty_canonical")

BugViewOobWraps ==
  ActualResult("view_index_oob") = SpecResult("view_index_oob")

BugViewMissingPeerReturnsInput ==
  ActualResult("view_index_absent_peer") =
    SpecResult("view_index_absent_peer")

BugViewUsesNumericIndex ==
  ActualResult("view_index_rotated") = SpecResult("view_index_rotated")

BugViewSetEmptyKept ==
  ActualResult("empty_to_view") = SpecResult("empty_to_view")

BugViewSetUsesNumericIndex ==
  ActualResult("rotated_to_view_many") = SpecResult("rotated_to_view_many")

BugViewSetKeepsInvalid ==
  ActualResult("to_view_absent_peer_filters") =
    SpecResult("to_view_absent_peer_filters")

BugViewSetEmptyCanonicalNonempty ==
  ActualResult("to_view_empty_canonical") =
    SpecResult("to_view_empty_canonical")

BugViewSetDropsZero ==
  ActualResult("to_view_keeps_zero") = SpecResult("to_view_keeps_zero")

BugRoundtripUsesViewIndices ==
  ActualResult("roundtrip_rotated_common") =
    SpecResult("roundtrip_rotated_common")

BugRoundtripKeepsMissingPeer ==
  ActualResult("roundtrip_missing_peer_filters") =
    SpecResult("roundtrip_missing_peer_filters")

BugRoundtripDropsPresent ==
  ActualResult("roundtrip_rotated_common") =
    SpecResult("roundtrip_rotated_common")

====
