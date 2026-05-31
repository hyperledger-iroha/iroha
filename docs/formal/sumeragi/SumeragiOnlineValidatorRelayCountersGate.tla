---- MODULE SumeragiOnlineValidatorRelayCountersGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for two deterministic Sumeragi helper surfaces:

`count_online_validators(...)` filters the online peer iterator through a
canonical `BTreeSet` built from the voting roster. Empty rosters return zero,
membership is by `PeerId`, online outsiders are ignored, offline roster members
are not counted, duplicate roster entries do not inflate the result, and the
online iterator supplies the counted peers.

`RelayDropCounters::total(...)` sums every stored drop lane with saturating
addition, while `RelayDropCounters::collect()` forwards the direct drop metrics
and stores the saturating sum of every p2p cap-violation family.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

OnlineEmptyRosterZero == 1
OnlineMemberCounted == 2
OnlineOutsiderIgnored == 3
OnlineOfflineRosterIgnored == 4
OnlineAllMembersCounted == 5
OnlineRosterDuplicatesCanonicalized == 6
OnlineUsesPeerIdMembership == 7
OnlinePreservesOnlineIteration == 8
RelayTotalZero == 9
RelayTotalSubscriberQueue == 10
RelayTotalDroppedPost == 11
RelayTotalDroppedBroadcast == 12
RelayTotalPostOverflow == 13
RelayTotalCapViolations == 14
RelayTotalSaturating == 15
RelayCollectDirectCounters == 16
RelayCollectConsensusCap == 17
RelayCollectControlCap == 18
RelayCollectBlockSyncCap == 19
RelayCollectTxGossipCap == 20
RelayCollectPeerGossipCap == 21
RelayCollectHealthCap == 22
RelayCollectOtherCap == 23
RelayCollectSaturatingCap == 24

Candidates == 1..24

ReturnZero == 1
BuildRosterSet == 2
UsePeerId == 3
CountOnlineMember == 4
IgnoreOnlineOutsider == 5
IgnoreOfflineRoster == 6
DedupRoster == 7
IgnoreRosterOrder == 8
PreserveOnlineIteration == 9
ReturnOnlineCount == 10
SumSubscriberQueue == 11
SumDroppedPost == 12
SumDroppedBroadcast == 13
SumPostOverflow == 14
SumCapViolations == 15
SaturatingAdd == 16
ReturnTotal == 17
ReadSubscriberQueue == 18
ReadDroppedPost == 19
ReadDroppedBroadcast == 20
ReadPostOverflow == 21
ReadCapViolations == 22
ReadCapConsensus == 23
ReadCapControl == 24
ReadCapBlockSync == 25
ReadCapTxGossip == 26
ReadCapPeerGossip == 27
ReadCapHealth == 28
ReadCapOther == 29
StoreCapViolations == 30
ForwardDirectCounters == 31
CountEachOnlineMember == 32
CountRosterEntry == 33
UsePeerObject == 34

Actions == 1..34

OnlineCountBase ==
  {BuildRosterSet, UsePeerId, CountOnlineMember, ReturnOnlineCount}

RelayTotalAllLanes ==
  {SumSubscriberQueue, SumDroppedPost, SumDroppedBroadcast, SumPostOverflow,
   SumCapViolations, SaturatingAdd, ReturnTotal}

RelayCapAllFamilies ==
  {ReadCapConsensus, ReadCapControl, ReadCapBlockSync, ReadCapTxGossip,
   ReadCapPeerGossip, ReadCapHealth, ReadCapOther, SaturatingAdd,
   StoreCapViolations}

SpecActions(candidate) ==
  CASE candidate = OnlineEmptyRosterZero ->
      {ReturnZero}
    [] candidate = OnlineMemberCounted ->
      OnlineCountBase
    [] candidate = OnlineOutsiderIgnored ->
      {BuildRosterSet, UsePeerId, IgnoreOnlineOutsider, ReturnZero}
    [] candidate = OnlineOfflineRosterIgnored ->
      {BuildRosterSet, UsePeerId, IgnoreOfflineRoster, ReturnZero}
    [] candidate = OnlineAllMembersCounted ->
      OnlineCountBase \cup {CountEachOnlineMember, IgnoreRosterOrder}
    [] candidate = OnlineRosterDuplicatesCanonicalized ->
      OnlineCountBase \cup {DedupRoster}
    [] candidate = OnlineUsesPeerIdMembership ->
      OnlineCountBase \cup {IgnoreOnlineOutsider}
    [] candidate = OnlinePreservesOnlineIteration ->
      {BuildRosterSet, UsePeerId, CountEachOnlineMember,
       PreserveOnlineIteration, ReturnOnlineCount}
    [] candidate = RelayTotalZero ->
      {ReturnTotal}
    [] candidate = RelayTotalSubscriberQueue ->
      {SumSubscriberQueue, ReturnTotal}
    [] candidate = RelayTotalDroppedPost ->
      {SumDroppedPost, ReturnTotal}
    [] candidate = RelayTotalDroppedBroadcast ->
      {SumDroppedBroadcast, ReturnTotal}
    [] candidate = RelayTotalPostOverflow ->
      {SumPostOverflow, ReturnTotal}
    [] candidate = RelayTotalCapViolations ->
      {SumCapViolations, ReturnTotal}
    [] candidate = RelayTotalSaturating ->
      RelayTotalAllLanes
    [] candidate = RelayCollectDirectCounters ->
      {ReadSubscriberQueue, ReadDroppedPost, ReadDroppedBroadcast,
       ReadPostOverflow, ForwardDirectCounters}
    [] candidate = RelayCollectConsensusCap ->
      {ReadCapConsensus, StoreCapViolations}
    [] candidate = RelayCollectControlCap ->
      {ReadCapControl, StoreCapViolations}
    [] candidate = RelayCollectBlockSyncCap ->
      {ReadCapBlockSync, StoreCapViolations}
    [] candidate = RelayCollectTxGossipCap ->
      {ReadCapTxGossip, StoreCapViolations}
    [] candidate = RelayCollectPeerGossipCap ->
      {ReadCapPeerGossip, StoreCapViolations}
    [] candidate = RelayCollectHealthCap ->
      {ReadCapHealth, StoreCapViolations}
    [] candidate = RelayCollectOtherCap ->
      {ReadCapOther, StoreCapViolations}
    [] candidate = RelayCollectSaturatingCap ->
      RelayCapAllFamilies
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = OnlineEmptyRosterZero /\
          Bug = "online_empty_roster_counts_all" ->
      {BuildRosterSet, CountEachOnlineMember, ReturnOnlineCount}
    [] candidate = OnlineMemberCounted /\ Bug = "online_drops_member" ->
      (spec \ {CountOnlineMember, ReturnOnlineCount}) \cup {ReturnZero}
    [] candidate = OnlineOutsiderIgnored /\ Bug = "online_counts_outsider" ->
      (spec \ {IgnoreOnlineOutsider, ReturnZero}) \cup
        {CountOnlineMember, ReturnOnlineCount}
    [] candidate = OnlineOfflineRosterIgnored /\
          Bug = "online_counts_offline_roster" ->
      (spec \ {IgnoreOfflineRoster, ReturnZero}) \cup
        {CountRosterEntry, ReturnOnlineCount}
    [] candidate = OnlineAllMembersCounted /\
          Bug = "online_uses_roster_order" ->
      spec \ {IgnoreRosterOrder}
    [] candidate = OnlineAllMembersCounted /\
          Bug = "online_all_members_zero" ->
      (spec \ {CountOnlineMember, CountEachOnlineMember, ReturnOnlineCount})
        \cup {ReturnZero}
    [] candidate = OnlineRosterDuplicatesCanonicalized /\
          Bug = "online_roster_duplicates_inflate" ->
      (spec \ {DedupRoster}) \cup {CountRosterEntry}
    [] candidate = OnlineUsesPeerIdMembership /\
          Bug = "online_uses_peer_object_identity" ->
      (spec \ {UsePeerId, IgnoreOnlineOutsider}) \cup {UsePeerObject}
    [] candidate = OnlinePreservesOnlineIteration /\
          Bug = "online_deduplicates_online_iteration" ->
      (spec \ {CountEachOnlineMember, PreserveOnlineIteration}) \cup
        {CountOnlineMember}
    [] candidate = RelayTotalSubscriberQueue /\
          Bug = "relay_total_drops_subscriber_queue" ->
      spec \ {SumSubscriberQueue}
    [] candidate = RelayTotalDroppedPost /\
          Bug = "relay_total_drops_post" ->
      spec \ {SumDroppedPost}
    [] candidate = RelayTotalDroppedBroadcast /\
          Bug = "relay_total_drops_broadcast" ->
      spec \ {SumDroppedBroadcast}
    [] candidate = RelayTotalPostOverflow /\
          Bug = "relay_total_drops_overflow" ->
      spec \ {SumPostOverflow}
    [] candidate = RelayTotalCapViolations /\
          Bug = "relay_total_drops_cap_violations" ->
      spec \ {SumCapViolations}
    [] candidate = RelayTotalSaturating /\
          Bug = "relay_total_not_saturating" ->
      spec \ {SaturatingAdd}
    [] candidate = RelayCollectDirectCounters /\
          Bug = "relay_collect_drops_direct_counters" ->
      spec \ {ForwardDirectCounters}
    [] candidate = RelayCollectConsensusCap /\
          Bug = "relay_collect_drops_consensus_cap" ->
      spec \ {ReadCapConsensus}
    [] candidate = RelayCollectControlCap /\
          Bug = "relay_collect_drops_control_cap" ->
      spec \ {ReadCapControl}
    [] candidate = RelayCollectBlockSyncCap /\
          Bug = "relay_collect_drops_block_sync_cap" ->
      spec \ {ReadCapBlockSync}
    [] candidate = RelayCollectTxGossipCap /\
          Bug = "relay_collect_drops_tx_gossip_cap" ->
      spec \ {ReadCapTxGossip}
    [] candidate = RelayCollectPeerGossipCap /\
          Bug = "relay_collect_drops_peer_gossip_cap" ->
      spec \ {ReadCapPeerGossip}
    [] candidate = RelayCollectHealthCap /\
          Bug = "relay_collect_drops_health_cap" ->
      spec \ {ReadCapHealth}
    [] candidate = RelayCollectOtherCap /\
          Bug = "relay_collect_drops_other_cap" ->
      spec \ {ReadCapOther}
    [] candidate = RelayCollectSaturatingCap /\
          Bug = "relay_collect_cap_not_saturating" ->
      spec \ {SaturatingAdd}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "online_empty_roster_counts_all",
       "online_drops_member",
       "online_counts_outsider",
       "online_counts_offline_roster",
       "online_uses_roster_order",
       "online_all_members_zero",
       "online_roster_duplicates_inflate",
       "online_uses_peer_object_identity",
       "online_deduplicates_online_iteration",
       "relay_total_drops_subscriber_queue",
       "relay_total_drops_post",
       "relay_total_drops_broadcast",
       "relay_total_drops_overflow",
       "relay_total_drops_cap_violations",
       "relay_total_not_saturating",
       "relay_collect_drops_direct_counters",
       "relay_collect_drops_consensus_cap",
       "relay_collect_drops_control_cap",
       "relay_collect_drops_block_sync_cap",
       "relay_collect_drops_tx_gossip_cap",
       "relay_collect_drops_peer_gossip_cap",
       "relay_collect_drops_health_cap",
       "relay_collect_drops_other_cap",
       "relay_collect_cap_not_saturating"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

BugOnlineEmptyRosterCountsAll ==
  ImplementationActions(OnlineEmptyRosterZero) =
    SpecActions(OnlineEmptyRosterZero)

BugOnlineDropsMember ==
  ImplementationActions(OnlineMemberCounted) =
    SpecActions(OnlineMemberCounted)

BugOnlineCountsOutsider ==
  ImplementationActions(OnlineOutsiderIgnored) =
    SpecActions(OnlineOutsiderIgnored)

BugOnlineCountsOfflineRoster ==
  ImplementationActions(OnlineOfflineRosterIgnored) =
    SpecActions(OnlineOfflineRosterIgnored)

BugOnlineUsesRosterOrder ==
  ImplementationActions(OnlineAllMembersCounted) =
    SpecActions(OnlineAllMembersCounted)

BugOnlineAllMembersZero ==
  ImplementationActions(OnlineAllMembersCounted) =
    SpecActions(OnlineAllMembersCounted)

BugOnlineRosterDuplicatesInflate ==
  ImplementationActions(OnlineRosterDuplicatesCanonicalized) =
    SpecActions(OnlineRosterDuplicatesCanonicalized)

BugOnlineUsesPeerObjectIdentity ==
  ImplementationActions(OnlineUsesPeerIdMembership) =
    SpecActions(OnlineUsesPeerIdMembership)

BugOnlineDeduplicatesOnlineIteration ==
  ImplementationActions(OnlinePreservesOnlineIteration) =
    SpecActions(OnlinePreservesOnlineIteration)

BugRelayTotalDropsSubscriberQueue ==
  ImplementationActions(RelayTotalSubscriberQueue) =
    SpecActions(RelayTotalSubscriberQueue)

BugRelayTotalDropsPost ==
  ImplementationActions(RelayTotalDroppedPost) =
    SpecActions(RelayTotalDroppedPost)

BugRelayTotalDropsBroadcast ==
  ImplementationActions(RelayTotalDroppedBroadcast) =
    SpecActions(RelayTotalDroppedBroadcast)

BugRelayTotalDropsOverflow ==
  ImplementationActions(RelayTotalPostOverflow) =
    SpecActions(RelayTotalPostOverflow)

BugRelayTotalDropsCapViolations ==
  ImplementationActions(RelayTotalCapViolations) =
    SpecActions(RelayTotalCapViolations)

BugRelayTotalNotSaturating ==
  ImplementationActions(RelayTotalSaturating) =
    SpecActions(RelayTotalSaturating)

BugRelayCollectDropsDirectCounters ==
  ImplementationActions(RelayCollectDirectCounters) =
    SpecActions(RelayCollectDirectCounters)

BugRelayCollectDropsConsensusCap ==
  ImplementationActions(RelayCollectConsensusCap) =
    SpecActions(RelayCollectConsensusCap)

BugRelayCollectDropsControlCap ==
  ImplementationActions(RelayCollectControlCap) =
    SpecActions(RelayCollectControlCap)

BugRelayCollectDropsBlockSyncCap ==
  ImplementationActions(RelayCollectBlockSyncCap) =
    SpecActions(RelayCollectBlockSyncCap)

BugRelayCollectDropsTxGossipCap ==
  ImplementationActions(RelayCollectTxGossipCap) =
    SpecActions(RelayCollectTxGossipCap)

BugRelayCollectDropsPeerGossipCap ==
  ImplementationActions(RelayCollectPeerGossipCap) =
    SpecActions(RelayCollectPeerGossipCap)

BugRelayCollectDropsHealthCap ==
  ImplementationActions(RelayCollectHealthCap) =
    SpecActions(RelayCollectHealthCap)

BugRelayCollectDropsOtherCap ==
  ImplementationActions(RelayCollectOtherCap) =
    SpecActions(RelayCollectOtherCap)

BugRelayCollectCapNotSaturating ==
  ImplementationActions(RelayCollectSaturatingCap) =
    SpecActions(RelayCollectSaturatingCap)

====
