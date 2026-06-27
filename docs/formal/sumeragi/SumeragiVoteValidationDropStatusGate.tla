---- MODULE SumeragiVoteValidationDropStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for vote-validation drop telemetry status.

This slice captures `VoteValidationDropReason::as_str(...)`,
`record_vote_validation_drop(...)`, `vote_validation_drop_snapshot()`,
`should_log_vote_drop_count(...)`, and the top-level `snapshot()` projection.
It pins the status-only contract: resets clear global and per-peer state,
records always append to the bounded recent-entry history and increment the
global total, peer-qualified records update the `(peer, roster_hash)` aggregate
with independent per-reason counters and latest slot context, unqualified
records do not create peer aggregates, snapshots expose newest-first bounded
entries, reason labels remain stable, and log thresholds fire only for `1` and
powers of ten.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ResetClearsState == 1
ReasonLabelsStable == 2
RecordWithoutPeer == 3
RecordWithPeer == 4
RepeatSameReason == 5
DifferentReasonsSamePeer == 6
DifferentPeersSeparate == 7
RosterHashSeparates == 8
RecentEntriesNewestFirst == 9
RecentEntriesCapDropsOldest == 10
PeerLastFieldsUpdate == 11
SnapshotProjectsStatus == 12
LogThresholds == 13
SaturatingCounters == 14

Candidates == 1..14

ResetClearsTotal == 1
ResetClearsEntries == 2
ResetClearsPeerEntries == 3
BackpressureLabelStable == 4
SignatureInvalidLabelStable == 5
DuplicateLabelStable == 6
ChainOrderLabelStable == 7
RecordIncrementsTotal == 8
RecordAppendsEntry == 9
RecordEntryFields == 10
RecordTimestampPositive == 11
NoPeerEntryWithoutPeer == 12
PeerEntryCreated == 13
PeerTotalIncrements == 14
PeerReasonIncrements == 15
PeerRosterContextStored == 16
PeerLastFieldsStored == 17
PeerTimestampPositive == 18
RepeatTotalAccumulates == 19
RepeatReasonAccumulates == 20
DifferentReasonsIndependent == 21
SamePeerTotalAccumulates == 22
DifferentPeersIndependent == 23
RosterHashPartOfKey == 24
RecentNewestFirst == 25
RecentCapEnforced == 26
RecentDropsOldest == 27
SnapshotUsesDropSnapshot == 28
LogFirstTrue == 29
LogTenTrue == 30
LogHundredTrue == 31
LogZeroFalse == 32
LogTwoFalse == 33
LogElevenFalse == 34
LogTwentyFalse == 35
SaturatingNoWrap == 36

Actions == 1..36

ResetActions ==
  {ResetClearsTotal, ResetClearsEntries, ResetClearsPeerEntries}

LabelActions ==
  {BackpressureLabelStable, SignatureInvalidLabelStable, DuplicateLabelStable,
   ChainOrderLabelStable}

EntryActions ==
  {RecordIncrementsTotal, RecordAppendsEntry, RecordEntryFields,
   RecordTimestampPositive}

PeerActions ==
  {PeerEntryCreated, PeerTotalIncrements, PeerReasonIncrements,
   PeerRosterContextStored, PeerLastFieldsStored, PeerTimestampPositive}

LogActions ==
  {LogFirstTrue, LogTenTrue, LogHundredTrue, LogZeroFalse, LogTwoFalse,
   LogElevenFalse, LogTwentyFalse}

SpecActions(candidate) ==
  CASE candidate = ResetClearsState ->
      ResetActions
    [] candidate = ReasonLabelsStable ->
      LabelActions
    [] candidate = RecordWithoutPeer ->
      EntryActions \cup {NoPeerEntryWithoutPeer}
    [] candidate = RecordWithPeer ->
      EntryActions \cup PeerActions
    [] candidate = RepeatSameReason ->
      {RepeatTotalAccumulates, RepeatReasonAccumulates}
    [] candidate = DifferentReasonsSamePeer ->
      {DifferentReasonsIndependent, SamePeerTotalAccumulates}
    [] candidate = DifferentPeersSeparate ->
      {DifferentPeersIndependent}
    [] candidate = RosterHashSeparates ->
      {RosterHashPartOfKey}
    [] candidate = RecentEntriesNewestFirst ->
      {RecentNewestFirst}
    [] candidate = RecentEntriesCapDropsOldest ->
      {RecentCapEnforced, RecentDropsOldest}
    [] candidate = PeerLastFieldsUpdate ->
      {PeerLastFieldsStored, PeerTimestampPositive}
    [] candidate = SnapshotProjectsStatus ->
      {SnapshotUsesDropSnapshot}
    [] candidate = LogThresholds ->
      LogActions
    [] candidate = SaturatingCounters ->
      {SaturatingNoWrap}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetClearsState /\ Bug = "reset_keeps_total" ->
      spec \ {ResetClearsTotal}
    [] candidate = ResetClearsState /\ Bug = "reset_keeps_entries" ->
      spec \ {ResetClearsEntries}
    [] candidate = ResetClearsState /\ Bug = "reset_keeps_peer_entries" ->
      spec \ {ResetClearsPeerEntries}
    [] candidate = ReasonLabelsStable /\ Bug = "backpressure_label_wrong" ->
      spec \ {BackpressureLabelStable}
    [] candidate = ReasonLabelsStable /\ Bug = "invalid_signature_label_wrong" ->
      spec \ {SignatureInvalidLabelStable}
    [] candidate = ReasonLabelsStable /\ Bug = "duplicate_label_wrong" ->
      spec \ {DuplicateLabelStable}
    [] candidate = ReasonLabelsStable /\ Bug = "chain_order_label_wrong" ->
      spec \ {ChainOrderLabelStable}
    [] candidate \in {RecordWithoutPeer, RecordWithPeer} /\
          Bug = "record_not_counted" ->
      spec \ {RecordIncrementsTotal}
    [] candidate \in {RecordWithoutPeer, RecordWithPeer} /\
          Bug = "record_not_appended" ->
      spec \ {RecordAppendsEntry}
    [] candidate \in {RecordWithoutPeer, RecordWithPeer} /\
          Bug = "entry_fields_dropped" ->
      spec \ {RecordEntryFields}
    [] candidate \in {RecordWithoutPeer, RecordWithPeer} /\
          Bug = "entry_timestamp_zero" ->
      spec \ {RecordTimestampPositive}
    [] candidate = RecordWithoutPeer /\ Bug = "unqualified_creates_peer_entry" ->
      spec \ {NoPeerEntryWithoutPeer}
    [] candidate = RecordWithPeer /\ Bug = "peer_entry_missing" ->
      spec \ {PeerEntryCreated}
    [] candidate = RecordWithPeer /\ Bug = "peer_total_not_incremented" ->
      spec \ {PeerTotalIncrements}
    [] candidate = RecordWithPeer /\ Bug = "peer_reason_not_incremented" ->
      spec \ {PeerReasonIncrements}
    [] candidate = RecordWithPeer /\ Bug = "roster_context_not_stored" ->
      spec \ {PeerRosterContextStored}
    [] candidate \in {RecordWithPeer, PeerLastFieldsUpdate} /\
          Bug = "peer_last_fields_not_updated" ->
      spec \ {PeerLastFieldsStored}
    [] candidate \in {RecordWithPeer, PeerLastFieldsUpdate} /\
          Bug = "peer_timestamp_zero" ->
      spec \ {PeerTimestampPositive}
    [] candidate = RepeatSameReason /\ Bug = "same_reason_overwrites_total" ->
      spec \ {RepeatTotalAccumulates}
    [] candidate = RepeatSameReason /\ Bug = "same_reason_overwrites_reason" ->
      spec \ {RepeatReasonAccumulates}
    [] candidate = DifferentReasonsSamePeer /\ Bug = "different_reasons_collide" ->
      spec \ {DifferentReasonsIndependent}
    [] candidate = DifferentReasonsSamePeer /\ Bug = "same_peer_total_wrong" ->
      spec \ {SamePeerTotalAccumulates}
    [] candidate = DifferentPeersSeparate /\ Bug = "different_peers_merge" ->
      spec \ {DifferentPeersIndependent}
    [] candidate = RosterHashSeparates /\ Bug = "roster_hash_ignored" ->
      spec \ {RosterHashPartOfKey}
    [] candidate = RecentEntriesNewestFirst /\ Bug = "snapshot_not_newest_first" ->
      spec \ {RecentNewestFirst}
    [] candidate = RecentEntriesCapDropsOldest /\ Bug = "recent_cap_not_enforced" ->
      spec \ {RecentCapEnforced}
    [] candidate = RecentEntriesCapDropsOldest /\ Bug = "recent_cap_keeps_oldest" ->
      spec \ {RecentDropsOldest}
    [] candidate = SnapshotProjectsStatus /\
          Bug = "status_snapshot_drops_vote_validation" ->
      spec \ {SnapshotUsesDropSnapshot}
    [] candidate = LogThresholds /\ Bug = "log_first_false" ->
      spec \ {LogFirstTrue}
    [] candidate = LogThresholds /\ Bug = "log_ten_false" ->
      spec \ {LogTenTrue}
    [] candidate = LogThresholds /\ Bug = "log_hundred_false" ->
      spec \ {LogHundredTrue}
    [] candidate = LogThresholds /\ Bug = "log_zero_true" ->
      spec \ {LogZeroFalse}
    [] candidate = LogThresholds /\ Bug = "log_two_true" ->
      spec \ {LogTwoFalse}
    [] candidate = LogThresholds /\ Bug = "log_non_power_ten_true" ->
      spec \ {LogElevenFalse, LogTwentyFalse}
    [] candidate = SaturatingCounters /\ Bug = "saturating_wraps" ->
      spec \ {SaturatingNoWrap}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 14
  /\ checked' = checked + 1

TypeInvariant ==
  /\ Bug \in {
       "none",
       "reset_keeps_total",
       "reset_keeps_entries",
       "reset_keeps_peer_entries",
       "backpressure_label_wrong",
       "invalid_signature_label_wrong",
       "duplicate_label_wrong",
       "chain_order_label_wrong",
       "record_not_counted",
       "record_not_appended",
       "entry_fields_dropped",
       "entry_timestamp_zero",
       "unqualified_creates_peer_entry",
       "peer_entry_missing",
       "peer_total_not_incremented",
       "peer_reason_not_incremented",
       "roster_context_not_stored",
       "peer_last_fields_not_updated",
       "peer_timestamp_zero",
       "same_reason_overwrites_total",
       "same_reason_overwrites_reason",
       "different_reasons_collide",
       "same_peer_total_wrong",
       "different_peers_merge",
       "roster_hash_ignored",
       "snapshot_not_newest_first",
       "recent_cap_not_enforced",
       "recent_cap_keeps_oldest",
       "status_snapshot_drops_vote_validation",
       "log_first_false",
       "log_ten_false",
       "log_hundred_false",
       "log_zero_true",
       "log_two_true",
       "log_non_power_ten_true",
       "saturating_wraps"
     }
  /\ checked \in 0..14
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

VoteValidationDropStatusExactness ==
  Safety

VoteValidationDropStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ VoteValidationDropStatusExactness

BugResetKeepsTotal ==
  ImplementationActions(ResetClearsState) = SpecActions(ResetClearsState)

BugResetKeepsEntries ==
  ImplementationActions(ResetClearsState) = SpecActions(ResetClearsState)

BugResetKeepsPeerEntries ==
  ImplementationActions(ResetClearsState) = SpecActions(ResetClearsState)

BugBackpressureLabelWrong ==
  ImplementationActions(ReasonLabelsStable) =
    SpecActions(ReasonLabelsStable)

BugInvalidSignatureLabelWrong ==
  ImplementationActions(ReasonLabelsStable) =
    SpecActions(ReasonLabelsStable)

BugDuplicateLabelWrong ==
  ImplementationActions(ReasonLabelsStable) =
    SpecActions(ReasonLabelsStable)

BugChainOrderLabelWrong ==
  ImplementationActions(ReasonLabelsStable) =
    SpecActions(ReasonLabelsStable)

BugRecordNotCounted ==
  ImplementationActions(RecordWithPeer) = SpecActions(RecordWithPeer)

BugRecordNotAppended ==
  ImplementationActions(RecordWithPeer) = SpecActions(RecordWithPeer)

BugEntryFieldsDropped ==
  ImplementationActions(RecordWithPeer) = SpecActions(RecordWithPeer)

BugEntryTimestampZero ==
  ImplementationActions(RecordWithPeer) = SpecActions(RecordWithPeer)

BugUnqualifiedCreatesPeerEntry ==
  ImplementationActions(RecordWithoutPeer) = SpecActions(RecordWithoutPeer)

BugPeerEntryMissing ==
  ImplementationActions(RecordWithPeer) = SpecActions(RecordWithPeer)

BugPeerTotalNotIncremented ==
  ImplementationActions(RecordWithPeer) = SpecActions(RecordWithPeer)

BugPeerReasonNotIncremented ==
  ImplementationActions(RecordWithPeer) = SpecActions(RecordWithPeer)

BugRosterContextNotStored ==
  ImplementationActions(RecordWithPeer) = SpecActions(RecordWithPeer)

BugPeerLastFieldsNotUpdated ==
  ImplementationActions(PeerLastFieldsUpdate) =
    SpecActions(PeerLastFieldsUpdate)

BugPeerTimestampZero ==
  ImplementationActions(PeerLastFieldsUpdate) =
    SpecActions(PeerLastFieldsUpdate)

BugSameReasonOverwritesTotal ==
  ImplementationActions(RepeatSameReason) = SpecActions(RepeatSameReason)

BugSameReasonOverwritesReason ==
  ImplementationActions(RepeatSameReason) = SpecActions(RepeatSameReason)

BugDifferentReasonsCollide ==
  ImplementationActions(DifferentReasonsSamePeer) =
    SpecActions(DifferentReasonsSamePeer)

BugSamePeerTotalWrong ==
  ImplementationActions(DifferentReasonsSamePeer) =
    SpecActions(DifferentReasonsSamePeer)

BugDifferentPeersMerge ==
  ImplementationActions(DifferentPeersSeparate) =
    SpecActions(DifferentPeersSeparate)

BugRosterHashIgnored ==
  ImplementationActions(RosterHashSeparates) = SpecActions(RosterHashSeparates)

BugSnapshotNotNewestFirst ==
  ImplementationActions(RecentEntriesNewestFirst) =
    SpecActions(RecentEntriesNewestFirst)

BugRecentCapNotEnforced ==
  ImplementationActions(RecentEntriesCapDropsOldest) =
    SpecActions(RecentEntriesCapDropsOldest)

BugRecentCapKeepsOldest ==
  ImplementationActions(RecentEntriesCapDropsOldest) =
    SpecActions(RecentEntriesCapDropsOldest)

BugStatusSnapshotDropsVoteValidation ==
  ImplementationActions(SnapshotProjectsStatus) =
    SpecActions(SnapshotProjectsStatus)

BugLogFirstFalse ==
  ImplementationActions(LogThresholds) = SpecActions(LogThresholds)

BugLogTenFalse ==
  ImplementationActions(LogThresholds) = SpecActions(LogThresholds)

BugLogHundredFalse ==
  ImplementationActions(LogThresholds) = SpecActions(LogThresholds)

BugLogZeroTrue ==
  ImplementationActions(LogThresholds) = SpecActions(LogThresholds)

BugLogTwoTrue ==
  ImplementationActions(LogThresholds) = SpecActions(LogThresholds)

BugLogNonPowerTenTrue ==
  ImplementationActions(LogThresholds) = SpecActions(LogThresholds)

BugSaturatingWraps ==
  ImplementationActions(SaturatingCounters) = SpecActions(SaturatingCounters)

====
