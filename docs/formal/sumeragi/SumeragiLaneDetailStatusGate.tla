---- MODULE SumeragiLaneDetailStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi lane-detail status projection.

This slice captures the status-facing lane/dataspace detail helpers in
`status.rs`: lane and dataspace commitment replacement, settlement commitment
projection, lane relay envelope upsert/cap behavior, governance sealed-summary
projection, and `StatusSnapshot::strip_lane_details()` as used by Torii routes
when Nexus is disabled.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

LaneCommitmentsReplace == 1
DataspaceCommitmentsReplace == 2
SettlementCommitmentsReplace == 3
RelaySetClearsExisting == 4
RelayRejectsInvalid == 5
RelayReplacesSameKey == 6
RelayAppendsDistinct == 7
RelayCapDropsOldest == 8
RelayPushPreservesExisting == 9
RelaySnapshotProjects == 10
GovernanceSnapshotReplace == 11
GovernanceSealedFiltersReady == 12
GovernanceSealedFiltersNotRequired == 13
GovernanceSealedAliasesProject == 14
GovernanceSealedTotalSaturates == 15
StripClearsActivityAndBacklogs == 16
StripClearsCommitments == 17
StripClearsRelayAndGovernance == 18
StripClearsNexusAndElection == 19
StripPreservesCoreConsensus == 20
DisabledRouteStrips == 21
EnabledRoutePreserves == 22
SnapshotProjectsCommitments == 23
SnapshotProjectsGovernance == 24
SnapshotProjectsNexus == 25

Candidates == 1..25

SetLaneCommitments == 1
SetDataspaceCommitments == 2
ReplaceLaneCommitments == 3
ReplaceDataspaceCommitments == 4
SetSettlementCommitments == 5
ReplaceSettlementCommitments == 6
RelayClearBeforeSet == 7
RelayVerifyEnvelope == 8
RelayRejectInvalid == 9
RelayCompareKey == 10
RelayReplaceSameKey == 11
RelayAppendDistinct == 12
RelayCapEnforced == 13
RelayDropsOldest == 14
RelayPushNoClear == 15
RelaySnapshotMatches == 16
GovernanceSet == 17
GovernanceSnapshotMatches == 18
SealedRequiresManifest == 19
SealedRequiresNotReady == 20
SealedAliasMatches == 21
SealedTotalMatches == 22
SealedTotalSaturatesAction == 23
StripClearLaneActivity == 24
StripClearDataspaceActivity == 25
StripClearRbcLaneBacklog == 26
StripClearRbcDataspaceBacklog == 27
StripClearLaneCommitments == 28
StripClearDataspaceCommitments == 29
StripClearSettlementCommitments == 30
StripClearRelayEnvelopes == 31
StripZeroGovernanceTotal == 32
StripClearGovernanceAliases == 33
StripClearGovernanceEntries == 34
StripDefaultNexusFee == 35
StripDefaultNexusStaking == 36
StripClearNposElection == 37
StripPreserveCore == 38
DisabledAppliesStrip == 39
EnabledSkipsStrip == 40
SnapshotLaneCommitments == 41
SnapshotDataspaceCommitments == 42
SnapshotSettlementCommitments == 43
SnapshotGovernance == 44
SnapshotNexus == 45

Actions == 1..45

StripActivityBacklogActions ==
  {StripClearLaneActivity, StripClearDataspaceActivity,
   StripClearRbcLaneBacklog, StripClearRbcDataspaceBacklog}

StripCommitmentActions ==
  {StripClearLaneCommitments, StripClearDataspaceCommitments,
   StripClearSettlementCommitments}

StripRelayGovernanceActions ==
  {StripClearRelayEnvelopes, StripZeroGovernanceTotal,
   StripClearGovernanceAliases, StripClearGovernanceEntries}

StripNexusElectionActions ==
  {StripDefaultNexusFee, StripDefaultNexusStaking, StripClearNposElection}

SpecActions(candidate) ==
  CASE candidate = LaneCommitmentsReplace ->
      {SetLaneCommitments, ReplaceLaneCommitments, SnapshotLaneCommitments}
    [] candidate = DataspaceCommitmentsReplace ->
      {SetDataspaceCommitments, ReplaceDataspaceCommitments,
       SnapshotDataspaceCommitments}
    [] candidate = SettlementCommitmentsReplace ->
      {SetSettlementCommitments, ReplaceSettlementCommitments,
       SnapshotSettlementCommitments}
    [] candidate = RelaySetClearsExisting ->
      {RelayClearBeforeSet, RelaySnapshotMatches}
    [] candidate = RelayRejectsInvalid ->
      {RelayVerifyEnvelope, RelayRejectInvalid, RelaySnapshotMatches}
    [] candidate = RelayReplacesSameKey ->
      {RelayCompareKey, RelayReplaceSameKey, RelaySnapshotMatches}
    [] candidate = RelayAppendsDistinct ->
      {RelayCompareKey, RelayAppendDistinct, RelaySnapshotMatches}
    [] candidate = RelayCapDropsOldest ->
      {RelayAppendDistinct, RelayCapEnforced, RelayDropsOldest,
       RelaySnapshotMatches}
    [] candidate = RelayPushPreservesExisting ->
      {RelayPushNoClear, RelayAppendDistinct, RelaySnapshotMatches}
    [] candidate = RelaySnapshotProjects ->
      {RelaySnapshotMatches}
    [] candidate = GovernanceSnapshotReplace ->
      {GovernanceSet, GovernanceSnapshotMatches}
    [] candidate = GovernanceSealedFiltersReady ->
      {SealedRequiresManifest, SealedRequiresNotReady, SealedTotalMatches}
    [] candidate = GovernanceSealedFiltersNotRequired ->
      {SealedRequiresManifest, SealedRequiresNotReady, SealedTotalMatches}
    [] candidate = GovernanceSealedAliasesProject ->
      {SealedAliasMatches}
    [] candidate = GovernanceSealedTotalSaturates ->
      {SealedTotalMatches, SealedTotalSaturatesAction}
    [] candidate = StripClearsActivityAndBacklogs ->
      StripActivityBacklogActions
    [] candidate = StripClearsCommitments ->
      StripCommitmentActions
    [] candidate = StripClearsRelayAndGovernance ->
      StripRelayGovernanceActions
    [] candidate = StripClearsNexusAndElection ->
      StripNexusElectionActions
    [] candidate = StripPreservesCoreConsensus ->
      {StripPreserveCore}
    [] candidate = DisabledRouteStrips ->
      {DisabledAppliesStrip} \cup StripActivityBacklogActions \cup
      StripCommitmentActions \cup StripRelayGovernanceActions \cup
      StripNexusElectionActions
    [] candidate = EnabledRoutePreserves ->
      {EnabledSkipsStrip, StripPreserveCore}
    [] candidate = SnapshotProjectsCommitments ->
      {SnapshotLaneCommitments, SnapshotDataspaceCommitments,
       SnapshotSettlementCommitments}
    [] candidate = SnapshotProjectsGovernance ->
      {GovernanceSnapshotMatches, SnapshotGovernance, SealedAliasMatches,
       SealedTotalMatches}
    [] candidate = SnapshotProjectsNexus ->
      {SnapshotNexus}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = LaneCommitmentsReplace /\
          Bug = "lane_commitments_wrong_replace" ->
      spec \ {ReplaceLaneCommitments, SnapshotLaneCommitments}
    [] candidate = DataspaceCommitmentsReplace /\
          Bug = "dataspace_commitments_dropped" ->
      spec \ {ReplaceDataspaceCommitments, SnapshotDataspaceCommitments}
    [] candidate = SettlementCommitmentsReplace /\
          Bug = "settlement_commitments_dropped" ->
      spec \ {ReplaceSettlementCommitments, SnapshotSettlementCommitments}
    [] candidate = RelaySetClearsExisting /\
          Bug = "relay_set_keeps_existing" ->
      spec \ {RelayClearBeforeSet, RelaySnapshotMatches}
    [] candidate = RelayRejectsInvalid /\
          Bug = "relay_accepts_invalid" ->
      (spec \ {RelayRejectInvalid}) \cup {RelayAppendDistinct}
    [] candidate = RelayReplacesSameKey /\
          Bug = "relay_duplicate_appends" ->
      (spec \ {RelayReplaceSameKey}) \cup {RelayAppendDistinct}
    [] candidate = RelayAppendsDistinct /\
          Bug = "relay_distinct_replaces" ->
      (spec \ {RelayAppendDistinct}) \cup {RelayReplaceSameKey}
    [] candidate = RelayCapDropsOldest /\
          Bug = "relay_cap_keeps_oldest" ->
      spec \ {RelayCapEnforced, RelayDropsOldest, RelaySnapshotMatches}
    [] candidate = RelayPushPreservesExisting /\
          Bug = "relay_push_clears_existing" ->
      spec \ {RelayPushNoClear, RelaySnapshotMatches}
    [] candidate = RelaySnapshotProjects /\
          Bug = "relay_snapshot_dropped" ->
      spec \ {RelaySnapshotMatches}
    [] candidate = GovernanceSnapshotReplace /\
          Bug = "governance_snapshot_dropped" ->
      spec \ {GovernanceSnapshotMatches}
    [] candidate = GovernanceSealedFiltersReady /\
          Bug = "governance_counts_ready" ->
      spec \ {SealedRequiresNotReady, SealedTotalMatches}
    [] candidate = GovernanceSealedFiltersNotRequired /\
          Bug = "governance_counts_not_required" ->
      spec \ {SealedRequiresManifest, SealedTotalMatches}
    [] candidate = GovernanceSealedAliasesProject /\
          Bug = "governance_aliases_dropped" ->
      spec \ {SealedAliasMatches}
    [] candidate = GovernanceSealedTotalSaturates /\
          Bug = "governance_total_overflows" ->
      spec \ {SealedTotalSaturatesAction}
    [] candidate = StripClearsActivityAndBacklogs /\
          Bug = "strip_keeps_activity" ->
      spec \ {StripClearLaneActivity, StripClearDataspaceActivity}
    [] candidate = StripClearsActivityAndBacklogs /\
          Bug = "strip_keeps_backlogs" ->
      spec \ {StripClearRbcLaneBacklog, StripClearRbcDataspaceBacklog}
    [] candidate = StripClearsCommitments /\
          Bug = "strip_keeps_commitments" ->
      spec \ {StripClearLaneCommitments, StripClearDataspaceCommitments,
              StripClearSettlementCommitments}
    [] candidate = StripClearsRelayAndGovernance /\
          Bug = "strip_keeps_relay" ->
      spec \ {StripClearRelayEnvelopes}
    [] candidate = StripClearsRelayAndGovernance /\
          Bug = "strip_keeps_governance" ->
      spec \ {StripZeroGovernanceTotal, StripClearGovernanceAliases,
              StripClearGovernanceEntries}
    [] candidate = StripClearsNexusAndElection /\
          Bug = "strip_keeps_nexus" ->
      spec \ {StripDefaultNexusFee, StripDefaultNexusStaking}
    [] candidate = StripClearsNexusAndElection /\
          Bug = "strip_keeps_npos_election" ->
      spec \ {StripClearNposElection}
    [] candidate = StripPreservesCoreConsensus /\
          Bug = "strip_mutates_core" ->
      spec \ {StripPreserveCore}
    [] candidate = DisabledRouteStrips /\
          Bug = "disabled_route_leaks_lanes" ->
      spec \ {DisabledAppliesStrip, StripClearLaneActivity,
              StripClearRbcLaneBacklog, StripClearLaneCommitments,
              StripClearRelayEnvelopes, StripClearGovernanceEntries}
    [] candidate = EnabledRoutePreserves /\
          Bug = "enabled_route_strips_lanes" ->
      (spec \ {EnabledSkipsStrip}) \cup StripActivityBacklogActions
    [] candidate = SnapshotProjectsCommitments /\
          Bug = "snapshot_drops_commitments" ->
      spec \ {SnapshotLaneCommitments, SnapshotDataspaceCommitments,
              SnapshotSettlementCommitments}
    [] candidate = SnapshotProjectsGovernance /\
          Bug = "snapshot_drops_governance" ->
      spec \ {GovernanceSnapshotMatches, SnapshotGovernance,
              SealedAliasMatches, SealedTotalMatches}
    [] candidate = SnapshotProjectsNexus /\
          Bug = "snapshot_drops_nexus" ->
      spec \ {SnapshotNexus}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 25
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..25

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

LaneDetailStatusActionsMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

LaneDetailStatusExactness ==
  /\ LaneDetailStatusActionsMatchSpec

LaneDetailStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ LaneDetailStatusExactness

BugLaneCommitmentsWrongReplace ==
  ImplementationActions(LaneCommitmentsReplace) =
    SpecActions(LaneCommitmentsReplace)

BugDataspaceCommitmentsDropped ==
  ImplementationActions(DataspaceCommitmentsReplace) =
    SpecActions(DataspaceCommitmentsReplace)

BugSettlementCommitmentsDropped ==
  ImplementationActions(SettlementCommitmentsReplace) =
    SpecActions(SettlementCommitmentsReplace)

BugRelaySetKeepsExisting ==
  ImplementationActions(RelaySetClearsExisting) =
    SpecActions(RelaySetClearsExisting)

BugRelayAcceptsInvalid ==
  ImplementationActions(RelayRejectsInvalid) =
    SpecActions(RelayRejectsInvalid)

BugRelayDuplicateAppends ==
  ImplementationActions(RelayReplacesSameKey) =
    SpecActions(RelayReplacesSameKey)

BugRelayDistinctReplaces ==
  ImplementationActions(RelayAppendsDistinct) =
    SpecActions(RelayAppendsDistinct)

BugRelayCapKeepsOldest ==
  ImplementationActions(RelayCapDropsOldest) =
    SpecActions(RelayCapDropsOldest)

BugRelayPushClearsExisting ==
  ImplementationActions(RelayPushPreservesExisting) =
    SpecActions(RelayPushPreservesExisting)

BugRelaySnapshotDropped ==
  ImplementationActions(RelaySnapshotProjects) =
    SpecActions(RelaySnapshotProjects)

BugGovernanceSnapshotDropped ==
  ImplementationActions(GovernanceSnapshotReplace) =
    SpecActions(GovernanceSnapshotReplace)

BugGovernanceCountsReady ==
  ImplementationActions(GovernanceSealedFiltersReady) =
    SpecActions(GovernanceSealedFiltersReady)

BugGovernanceCountsNotRequired ==
  ImplementationActions(GovernanceSealedFiltersNotRequired) =
    SpecActions(GovernanceSealedFiltersNotRequired)

BugGovernanceAliasesDropped ==
  ImplementationActions(GovernanceSealedAliasesProject) =
    SpecActions(GovernanceSealedAliasesProject)

BugGovernanceTotalOverflows ==
  ImplementationActions(GovernanceSealedTotalSaturates) =
    SpecActions(GovernanceSealedTotalSaturates)

BugStripKeepsActivity ==
  ImplementationActions(StripClearsActivityAndBacklogs) =
    SpecActions(StripClearsActivityAndBacklogs)

BugStripKeepsBacklogs ==
  ImplementationActions(StripClearsActivityAndBacklogs) =
    SpecActions(StripClearsActivityAndBacklogs)

BugStripKeepsCommitments ==
  ImplementationActions(StripClearsCommitments) =
    SpecActions(StripClearsCommitments)

BugStripKeepsRelay ==
  ImplementationActions(StripClearsRelayAndGovernance) =
    SpecActions(StripClearsRelayAndGovernance)

BugStripKeepsGovernance ==
  ImplementationActions(StripClearsRelayAndGovernance) =
    SpecActions(StripClearsRelayAndGovernance)

BugStripKeepsNexus ==
  ImplementationActions(StripClearsNexusAndElection) =
    SpecActions(StripClearsNexusAndElection)

BugStripKeepsNposElection ==
  ImplementationActions(StripClearsNexusAndElection) =
    SpecActions(StripClearsNexusAndElection)

BugStripMutatesCore ==
  ImplementationActions(StripPreservesCoreConsensus) =
    SpecActions(StripPreservesCoreConsensus)

BugDisabledRouteLeaksLanes ==
  ImplementationActions(DisabledRouteStrips) =
    SpecActions(DisabledRouteStrips)

BugEnabledRouteStripsLanes ==
  ImplementationActions(EnabledRoutePreserves) =
    SpecActions(EnabledRoutePreserves)

BugSnapshotDropsCommitments ==
  ImplementationActions(SnapshotProjectsCommitments) =
    SpecActions(SnapshotProjectsCommitments)

BugSnapshotDropsGovernance ==
  ImplementationActions(SnapshotProjectsGovernance) =
    SpecActions(SnapshotProjectsGovernance)

BugSnapshotDropsNexus ==
  ImplementationActions(SnapshotProjectsNexus) =
    SpecActions(SnapshotProjectsNexus)

=============================================================================
====
