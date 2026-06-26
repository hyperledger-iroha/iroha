---- MODULE SumeragiMembershipAdvertGate ----
EXTENDS Naturals, Sequences

(***************************************************************************
A bounded abstract model for publishing Sumeragi membership snapshots.

This slice captures the deterministic bridge in `record_membership_snapshot`
and `broadcast_consensus_params` from `main_loop.rs`: the membership hash is
computed from the exact chain/height/view/epoch/topology tuple, the same tuple
is written to operator status, and the scheduled consensus-params advert keeps
that membership payload while deriving collector parameters from the membership
height. The model also preserves the Rust `u16::try_from(...).unwrap_or(u16::MAX)`
clamp for `collectors_k`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoCase == "none"
BaseCase == "base"
ChainCase == "chain_b"
HeightCase == "height_11"
ViewCase == "view_3"
EpochCase == "epoch_4"
TopologyCase == "topology_b"
CollectorInRangeCase == "collector_in_range"
CollectorOverflowCase == "collector_overflow"
CurrentHeightPlanCase == "current_height_plan"
ViewPlanCase == "view_plan"
RedundantCase == "redundant_nonzero"

Cases == {
  BaseCase,
  ChainCase,
  HeightCase,
  ViewCase,
  EpochCase,
  TopologyCase,
  CollectorInRangeCase,
  CollectorOverflowCase,
  CurrentHeightPlanCase,
  ViewPlanCase,
  RedundantCase
}

ChainA == 1
ChainB == 2
Height10 == 10
Height11 == 11
CurrentHeight == 20
View1 == 1
View3 == 3
Epoch2 == 2
Epoch4 == 4
TopologyA == 30
TopologyB == 31

U16Max == 5
RawCollectorsKMax == 8
RedundantRMax == 3

\* @type: Seq(Int);
NoHash == <<>>

Chain(c) == IF c = ChainCase THEN ChainB ELSE ChainA
Height(c) == IF c = HeightCase THEN Height11 ELSE Height10
View(c) == IF c = ViewCase THEN View3 ELSE View1
Epoch(c) == IF c = EpochCase THEN Epoch4 ELSE Epoch2
Topology(c) == IF c = TopologyCase THEN TopologyB ELSE TopologyA

CurrentHeightFor(c) == CurrentHeight

\* @type: (Str) => Seq(Int);
SpecHash(c) ==
  <<Chain(c), Height(c), View(c), Epoch(c), Topology(c)>>

\* @type: (Str) => Seq(Int);
DropChainHash(c) == <<Height(c), View(c), Epoch(c), Topology(c)>>
\* @type: (Str) => Seq(Int);
DropHeightHash(c) == <<Chain(c), View(c), Epoch(c), Topology(c)>>
\* @type: (Str) => Seq(Int);
DropViewHash(c) == <<Chain(c), Height(c), Epoch(c), Topology(c)>>
\* @type: (Str) => Seq(Int);
DropEpochHash(c) == <<Chain(c), Height(c), View(c), Topology(c)>>
\* @type: (Str) => Seq(Int);
DropTopologyHash(c) == <<Chain(c), Height(c), View(c), Epoch(c)>>
\* @type: (Str) => Seq(Int);
CurrentHeightHash(c) ==
  <<Chain(c), CurrentHeightFor(c), View(c), Epoch(c), Topology(c)>>
\* @type: (Str) => Seq(Int);
WrongHash(c) == <<ChainB, Height(c), View(c), Epoch(c), Topology(c)>>

Hashes ==
  {NoHash}
  \union {SpecHash(c) : c \in Cases}
  \union {DropChainHash(c) : c \in Cases}
  \union {DropHeightHash(c) : c \in Cases}
  \union {DropViewHash(c) : c \in Cases}
  \union {DropEpochHash(c) : c \in Cases}
  \union {DropTopologyHash(c) : c \in Cases}
  \union {CurrentHeightHash(c) : c \in Cases}
  \union {WrongHash(c) : c \in Cases}

SpecComputedHash(c) == SpecHash(c)

ActualComputedHash(c) ==
  CASE Bug = "hash_drops_chain" /\ c = BaseCase ->
      DropChainHash(c)
    [] Bug = "hash_drops_height" /\ c = HeightCase ->
      DropHeightHash(c)
    [] Bug = "hash_drops_view" /\ c = ViewCase ->
      DropViewHash(c)
    [] Bug = "hash_drops_epoch" /\ c = EpochCase ->
      DropEpochHash(c)
    [] Bug = "hash_drops_topology" /\ c = TopologyCase ->
      DropTopologyHash(c)
    [] Bug = "hash_uses_current_height" /\ c = CurrentHeightPlanCase ->
      CurrentHeightHash(c)
    [] OTHER -> SpecComputedHash(c)

SpecStatusPresent(c) == TRUE
SpecStatusHeight(c) == Height(c)
SpecStatusView(c) == View(c)
SpecStatusEpoch(c) == Epoch(c)
SpecStatusHashPresent(c) == TRUE
SpecStatusHash(c) == SpecComputedHash(c)

ActualStatusPresent(c) ==
  CASE Bug = "status_not_set" /\ c = BaseCase -> FALSE
    [] OTHER -> TRUE

ActualStatusHeight(c) ==
  CASE Bug = "status_height_wrong" /\ c = HeightCase -> Height10
    [] OTHER -> Height(c)

ActualStatusView(c) ==
  CASE Bug = "status_view_wrong" /\ c = ViewCase -> View1
    [] OTHER -> View(c)

ActualStatusEpoch(c) ==
  CASE Bug = "status_epoch_wrong" /\ c = EpochCase -> Epoch2
    [] OTHER -> Epoch(c)

ActualStatusHashPresent(c) ==
  CASE Bug = "status_hash_omitted" /\ c = BaseCase -> FALSE
    [] OTHER -> TRUE

ActualStatusHash(c) ==
  CASE Bug = "status_hash_wrong" /\ c = BaseCase -> WrongHash(c)
    [] Bug = "status_hash_omitted" /\ c = BaseCase -> NoHash
    [] OTHER -> ActualComputedHash(c)

MembershipRawCollectorsK(c) ==
  CASE c = CollectorInRangeCase -> 4
    [] c = CollectorOverflowCase -> RawCollectorsKMax
    [] c = CurrentHeightPlanCase -> 3
    [] c = ViewPlanCase -> 3
    [] OTHER -> 2

CurrentHeightRawCollectorsK(c) ==
  CASE c = CurrentHeightPlanCase -> 4
    [] OTHER -> 1

ViewRawCollectorsK(c) ==
  CASE c = ViewPlanCase -> 4
    [] OTHER -> 1

MembershipRedundantR(c) ==
  CASE c = CurrentHeightPlanCase -> 1
    [] c = ViewPlanCase -> 1
    [] c = RedundantCase -> RedundantRMax
    [] OTHER -> 1

CurrentHeightRedundantR(c) ==
  CASE c = CurrentHeightPlanCase -> 2
    [] OTHER -> 1

ViewRedundantR(c) ==
  CASE c = ViewPlanCase -> 2
    [] OTHER -> 1

RawCollectorsKFor(h, c) ==
  CASE h = Height(c) -> MembershipRawCollectorsK(c)
    [] h = CurrentHeightFor(c) -> CurrentHeightRawCollectorsK(c)
    [] h = View(c) -> ViewRawCollectorsK(c)
    [] OTHER -> 0

RawRedundantRFor(h, c) ==
  CASE h = Height(c) -> MembershipRedundantR(c)
    [] h = CurrentHeightFor(c) -> CurrentHeightRedundantR(c)
    [] h = View(c) -> ViewRedundantR(c)
    [] OTHER -> 0

ClampCollectorsK(k) == IF k > U16Max THEN U16Max ELSE k

SpecPlanHeight(c) == Height(c)
SpecCollectorsK(c) == ClampCollectorsK(MembershipRawCollectorsK(c))
SpecRedundantR(c) == MembershipRedundantR(c)

ActualPlanHeight(c) ==
  CASE Bug = "collector_plan_uses_current_height"
       /\ c = CurrentHeightPlanCase ->
      CurrentHeightFor(c)
    [] Bug = "collector_plan_uses_view"
       /\ c = ViewPlanCase ->
      View(c)
    [] OTHER -> Height(c)

ActualCollectorsK(c) ==
  CASE Bug = "collectors_overflow_not_clamped"
       /\ c = CollectorOverflowCase ->
      MembershipRawCollectorsK(c)
    [] Bug = "collectors_in_range_clamped"
       /\ c = CollectorInRangeCase ->
      U16Max
    [] OTHER ->
      ClampCollectorsK(RawCollectorsKFor(ActualPlanHeight(c), c))

ActualRedundantR(c) ==
  CASE Bug = "redundant_send_dropped" /\ c = RedundantCase -> 0
    [] OTHER -> RawRedundantRFor(ActualPlanHeight(c), c)

SpecAdvertScheduled(c) == TRUE
SpecAdvertMembershipPresent(c) == TRUE
SpecAdvertHeight(c) == Height(c)
SpecAdvertView(c) == View(c)
SpecAdvertEpoch(c) == Epoch(c)
SpecAdvertHashPresent(c) == TRUE
SpecAdvertHash(c) == SpecComputedHash(c)

ActualAdvertScheduled(c) ==
  CASE Bug = "advert_not_scheduled" /\ c = BaseCase -> FALSE
    [] OTHER -> TRUE

ActualAdvertMembershipPresent(c) ==
  CASE Bug = "advert_membership_dropped" /\ c = BaseCase -> FALSE
    [] OTHER -> TRUE

ActualAdvertHeight(c) ==
  CASE Bug = "advert_height_wrong" /\ c = HeightCase -> Height10
    [] OTHER -> Height(c)

ActualAdvertView(c) ==
  CASE Bug = "advert_view_wrong" /\ c = ViewCase -> View1
    [] OTHER -> View(c)

ActualAdvertEpoch(c) ==
  CASE Bug = "advert_epoch_wrong" /\ c = EpochCase -> Epoch2
    [] OTHER -> Epoch(c)

ActualAdvertHashPresent(c) ==
  CASE Bug = "advert_hash_omitted" /\ c = BaseCase -> FALSE
    [] OTHER -> TRUE

ActualAdvertHash(c) ==
  CASE Bug = "advert_hash_wrong" /\ c = BaseCase -> WrongHash(c)
    [] Bug = "advert_hash_omitted" /\ c = BaseCase -> NoHash
    [] OTHER -> ActualComputedHash(c)

Bugs == {
  "none",
  "hash_drops_chain",
  "hash_drops_height",
  "hash_drops_view",
  "hash_drops_epoch",
  "hash_drops_topology",
  "hash_uses_current_height",
  "status_not_set",
  "status_height_wrong",
  "status_view_wrong",
  "status_epoch_wrong",
  "status_hash_omitted",
  "status_hash_wrong",
  "advert_not_scheduled",
  "advert_membership_dropped",
  "advert_height_wrong",
  "advert_view_wrong",
  "advert_epoch_wrong",
  "advert_hash_omitted",
  "advert_hash_wrong",
  "collector_plan_uses_current_height",
  "collector_plan_uses_view",
  "collectors_overflow_not_clamped",
  "collectors_in_range_clamped",
  "redundant_send_dropped"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecComputedHash(c) \in Hashes
       /\ ActualComputedHash(c) \in Hashes
       /\ ActualStatusPresent(c) \in BOOLEAN
       /\ ActualStatusHeight(c) \in 0..CurrentHeight
       /\ ActualStatusView(c) \in 0..View3
       /\ ActualStatusEpoch(c) \in 0..Epoch4
       /\ ActualStatusHashPresent(c) \in BOOLEAN
       /\ ActualStatusHash(c) \in Hashes
       /\ ActualPlanHeight(c) \in 0..CurrentHeight
       /\ ActualCollectorsK(c) \in 0..RawCollectorsKMax
       /\ ActualRedundantR(c) \in 0..RedundantRMax
       /\ ActualAdvertScheduled(c) \in BOOLEAN
       /\ ActualAdvertMembershipPresent(c) \in BOOLEAN
       /\ ActualAdvertHeight(c) \in 0..CurrentHeight
       /\ ActualAdvertView(c) \in 0..View3
       /\ ActualAdvertEpoch(c) \in 0..Epoch4
       /\ ActualAdvertHashPresent(c) \in BOOLEAN
       /\ ActualAdvertHash(c) \in Hashes

ComputedHashMatches(c) ==
  ActualComputedHash(c) = SpecComputedHash(c)

StatusFieldsMatch(c) ==
  /\ ActualStatusPresent(c) = SpecStatusPresent(c)
  /\ ActualStatusHeight(c) = SpecStatusHeight(c)
  /\ ActualStatusView(c) = SpecStatusView(c)
  /\ ActualStatusEpoch(c) = SpecStatusEpoch(c)
  /\ ActualStatusHashPresent(c) = SpecStatusHashPresent(c)
  /\ ActualStatusHash(c) = SpecStatusHash(c)

CollectorPlanFieldsMatch(c) ==
  /\ ActualPlanHeight(c) = SpecPlanHeight(c)
  /\ ActualCollectorsK(c) = SpecCollectorsK(c)
  /\ ActualRedundantR(c) = SpecRedundantR(c)

AdvertPayloadFieldsMatch(c) ==
  /\ ActualAdvertScheduled(c) = SpecAdvertScheduled(c)
  /\ ActualAdvertMembershipPresent(c) = SpecAdvertMembershipPresent(c)
  /\ ActualAdvertHeight(c) = SpecAdvertHeight(c)
  /\ ActualAdvertView(c) = SpecAdvertView(c)
  /\ ActualAdvertEpoch(c) = SpecAdvertEpoch(c)
  /\ ActualAdvertHashPresent(c) = SpecAdvertHashPresent(c)
  /\ ActualAdvertHash(c) = SpecAdvertHash(c)

MembershipAdvertHashExact ==
  \A c \in Cases:
    ComputedHashMatches(c)

MembershipAdvertStatusExact ==
  \A c \in Cases:
    StatusFieldsMatch(c)

MembershipAdvertCollectorPlanExact ==
  \A c \in Cases:
    CollectorPlanFieldsMatch(c)

MembershipAdvertPayloadExact ==
  \A c \in Cases:
    AdvertPayloadFieldsMatch(c)

MembershipAdvertBridgeExactness ==
  /\ MembershipAdvertHashExact
  /\ MembershipAdvertStatusExact
  /\ MembershipAdvertCollectorPlanExact
  /\ MembershipAdvertPayloadExact

MembershipAdvertBridgeCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ MembershipAdvertBridgeExactness

BridgeMatchesSpec ==
  MembershipAdvertBridgeExactness

====
