---- MODULE SumeragiIngressStatusCountersGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for inbound Sumeragi status counters.

This slice pins the status-facing accounting in `status.rs` for gossip
fallback, retransmit target/skip counters, background post drops,
`BlockCreated` drop/mismatch counters, dedup eviction buckets, and
`record_consensus_message_handling(...)` key projection.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ResetGossipClearsCounters == 1
GossipFallbackAccumulates == 2
GossipDuplicateAccumulates == 3
QuorumStallAccumulates == 4
RetransmitTargetStoresLast == 5
RetransmitTargetAccumulatesTotal == 6
RetransmitTargetCountsSamples == 7
RetransmitTargetZeroCountsSample == 8
RetransmitSkipReasonsDistinct == 9
BackgroundPostKindsCountPost == 10
BackgroundBroadcastKindsCountBroadcast == 11
BackgroundUnknownDoesNotCount == 12
BlockCreatedCountersDistinct == 13
ResetDedupClearsCounters == 14
DedupZeroNoop == 15
DedupVoteCountsCapacityExpired == 16
DedupProposalExpiredOnly == 17
DedupFetchBodyAliasesPendingBlock == 18
DedupBlockBodyResponseAliasesBlockSyncUpdate == 19
DedupRbcReadyDeliverChunkDistinct == 20
DedupRepeatedAccumulates == 21
DedupSnapshotProjects == 22
ResetMessageHandlingClearsEntries == 23
MessageHandlingRecordsEntry == 24
MessageHandlingRepeatedAccumulates == 25
MessageHandlingKindSeparatesKeys == 26
MessageHandlingOutcomeSeparatesKeys == 27
MessageHandlingReasonSeparatesKeys == 28
MessageHandlingSnapshotProjectsKey == 29
MessageHandlingSnapshotProjectsTotal == 30
ResetMessageHandlingAfterRecord == 31
ResetDedupAfterRecord == 32

Candidates == 1..32

ResetGossip == 1
IncrementGossipFallback == 2
IncrementGossipDuplicate == 3
IncrementQuorumStall == 4
RetransmitLastUpdated == 5
RetransmitTotalAccumulates == 6
RetransmitSamplesIncrement == 7
RetransmitZeroStillSamples == 8
RetransmitSkipBucketsDistinct == 9
BgPostCountsPost == 10
BgBroadcastCountsBroadcast == 11
BgUnknownNoop == 12
BlockCreatedDroppedByLock == 13
BlockCreatedHintMismatch == 14
BlockCreatedProposalMismatch == 15
BlockCreatedBucketsDistinct == 16
ResetDedup == 17
DedupZeroNoopAction == 18
DedupCapacityBucket == 19
DedupExpiredBucket == 20
DedupVoteBucket == 21
DedupProposalBucket == 22
DedupFetchPendingBucket == 23
DedupBlockSyncUpdateBucket == 24
DedupRbcReadyBucket == 25
DedupRbcDeliverBucket == 26
DedupRbcChunkBucket == 27
DedupRepeatedAccumulation == 28
DedupSnapshotMatches == 29
ResetMessageHandling == 30
MessageEntryRecorded == 31
MessageRepeatedAccumulation == 32
MessageKindInKey == 33
MessageOutcomeInKey == 34
MessageReasonInKey == 35
MessageSnapshotKeyMatches == 36
MessageSnapshotTotalMatches == 37

Actions == 1..37

SpecActions(candidate) ==
  CASE candidate = ResetGossipClearsCounters ->
      {ResetGossip}
    [] candidate = GossipFallbackAccumulates ->
      {IncrementGossipFallback}
    [] candidate = GossipDuplicateAccumulates ->
      {IncrementGossipDuplicate}
    [] candidate = QuorumStallAccumulates ->
      {IncrementQuorumStall}
    [] candidate = RetransmitTargetStoresLast ->
      {RetransmitLastUpdated}
    [] candidate = RetransmitTargetAccumulatesTotal ->
      {RetransmitTotalAccumulates}
    [] candidate = RetransmitTargetCountsSamples ->
      {RetransmitSamplesIncrement}
    [] candidate = RetransmitTargetZeroCountsSample ->
      {RetransmitZeroStillSamples, RetransmitSamplesIncrement}
    [] candidate = RetransmitSkipReasonsDistinct ->
      {RetransmitSkipBucketsDistinct}
    [] candidate = BackgroundPostKindsCountPost ->
      {BgPostCountsPost}
    [] candidate = BackgroundBroadcastKindsCountBroadcast ->
      {BgBroadcastCountsBroadcast}
    [] candidate = BackgroundUnknownDoesNotCount ->
      {BgUnknownNoop}
    [] candidate = BlockCreatedCountersDistinct ->
      {BlockCreatedDroppedByLock, BlockCreatedHintMismatch,
       BlockCreatedProposalMismatch, BlockCreatedBucketsDistinct}
    [] candidate = ResetDedupClearsCounters ->
      {ResetDedup}
    [] candidate = DedupZeroNoop ->
      {DedupZeroNoopAction}
    [] candidate = DedupVoteCountsCapacityExpired ->
      {DedupVoteBucket, DedupCapacityBucket, DedupExpiredBucket,
       DedupSnapshotMatches}
    [] candidate = DedupProposalExpiredOnly ->
      {DedupProposalBucket, DedupExpiredBucket, DedupSnapshotMatches}
    [] candidate = DedupFetchBodyAliasesPendingBlock ->
      {DedupFetchPendingBucket, DedupCapacityBucket, DedupExpiredBucket,
       DedupSnapshotMatches}
    [] candidate = DedupBlockBodyResponseAliasesBlockSyncUpdate ->
      {DedupBlockSyncUpdateBucket, DedupCapacityBucket, DedupExpiredBucket,
       DedupSnapshotMatches}
    [] candidate = DedupRbcReadyDeliverChunkDistinct ->
      {DedupRbcReadyBucket, DedupRbcDeliverBucket, DedupRbcChunkBucket,
       DedupSnapshotMatches}
    [] candidate = DedupRepeatedAccumulates ->
      {DedupVoteBucket, DedupRepeatedAccumulation, DedupSnapshotMatches}
    [] candidate = DedupSnapshotProjects ->
      {DedupSnapshotMatches}
    [] candidate = ResetMessageHandlingClearsEntries ->
      {ResetMessageHandling}
    [] candidate = MessageHandlingRecordsEntry ->
      {MessageEntryRecorded, MessageSnapshotKeyMatches,
       MessageSnapshotTotalMatches}
    [] candidate = MessageHandlingRepeatedAccumulates ->
      {MessageEntryRecorded, MessageRepeatedAccumulation,
       MessageSnapshotTotalMatches}
    [] candidate = MessageHandlingKindSeparatesKeys ->
      {MessageEntryRecorded, MessageKindInKey, MessageSnapshotKeyMatches}
    [] candidate = MessageHandlingOutcomeSeparatesKeys ->
      {MessageEntryRecorded, MessageOutcomeInKey, MessageSnapshotKeyMatches}
    [] candidate = MessageHandlingReasonSeparatesKeys ->
      {MessageEntryRecorded, MessageReasonInKey, MessageSnapshotKeyMatches}
    [] candidate = MessageHandlingSnapshotProjectsKey ->
      {MessageSnapshotKeyMatches}
    [] candidate = MessageHandlingSnapshotProjectsTotal ->
      {MessageSnapshotTotalMatches}
    [] candidate = ResetMessageHandlingAfterRecord ->
      {ResetMessageHandling}
    [] candidate = ResetDedupAfterRecord ->
      {ResetDedup}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetGossipClearsCounters /\
          Bug = "reset_gossip_keeps_counters" ->
      spec \ {ResetGossip}
    [] candidate = GossipFallbackAccumulates /\
          Bug = "gossip_fallback_not_counted" ->
      spec \ {IncrementGossipFallback}
    [] candidate = GossipDuplicateAccumulates /\
          Bug = "gossip_duplicate_counts_fallback" ->
      (spec \ {IncrementGossipDuplicate}) \cup {IncrementGossipFallback}
    [] candidate = QuorumStallAccumulates /\
          Bug = "quorum_stall_not_counted" ->
      spec \ {IncrementQuorumStall}
    [] candidate = RetransmitTargetStoresLast /\
          Bug = "retransmit_last_not_updated" ->
      spec \ {RetransmitLastUpdated}
    [] candidate = RetransmitTargetAccumulatesTotal /\
          Bug = "retransmit_total_overwrites" ->
      spec \ {RetransmitTotalAccumulates}
    [] candidate = RetransmitTargetCountsSamples /\
          Bug = "retransmit_samples_not_counted" ->
      spec \ {RetransmitSamplesIncrement}
    [] candidate = RetransmitTargetZeroCountsSample /\
          Bug = "retransmit_zero_skips_sample" ->
      spec \ {RetransmitZeroStillSamples, RetransmitSamplesIncrement}
    [] candidate = RetransmitSkipReasonsDistinct /\
          Bug = "retransmit_skip_buckets_collide" ->
      spec \ {RetransmitSkipBucketsDistinct}
    [] candidate = BackgroundPostKindsCountPost /\
          Bug = "bg_post_counts_broadcast" ->
      (spec \ {BgPostCountsPost}) \cup {BgBroadcastCountsBroadcast}
    [] candidate = BackgroundBroadcastKindsCountBroadcast /\
          Bug = "bg_broadcast_counts_post" ->
      (spec \ {BgBroadcastCountsBroadcast}) \cup {BgPostCountsPost}
    [] candidate = BackgroundUnknownDoesNotCount /\
          Bug = "bg_unknown_counts_post" ->
      (spec \ {BgUnknownNoop}) \cup {BgPostCountsPost}
    [] candidate = BlockCreatedCountersDistinct /\
          Bug = "block_created_buckets_collide" ->
      spec \ {BlockCreatedBucketsDistinct}
    [] candidate = ResetDedupClearsCounters /\
          Bug = "reset_dedup_keeps_counters" ->
      spec \ {ResetDedup}
    [] candidate = DedupZeroNoop /\
          Bug = "dedup_zero_counts_capacity" ->
      (spec \ {DedupZeroNoopAction}) \cup {DedupCapacityBucket}
    [] candidate = DedupVoteCountsCapacityExpired /\
          Bug = "dedup_vote_omits_capacity" ->
      spec \ {DedupCapacityBucket}
    [] candidate = DedupVoteCountsCapacityExpired /\
          Bug = "dedup_vote_omits_expired" ->
      spec \ {DedupExpiredBucket}
    [] candidate = DedupProposalExpiredOnly /\
          Bug = "dedup_proposal_expired_counts_capacity" ->
      spec \cup {DedupCapacityBucket}
    [] candidate = DedupFetchBodyAliasesPendingBlock /\
          Bug = "dedup_fetch_body_wrong_bucket" ->
      (spec \ {DedupFetchPendingBucket}) \cup {DedupBlockSyncUpdateBucket}
    [] candidate = DedupBlockBodyResponseAliasesBlockSyncUpdate /\
          Bug = "dedup_block_body_response_wrong_bucket" ->
      (spec \ {DedupBlockSyncUpdateBucket}) \cup {DedupFetchPendingBucket}
    [] candidate = DedupRbcReadyDeliverChunkDistinct /\
          Bug = "dedup_rbc_buckets_collide" ->
      spec \ {DedupRbcDeliverBucket, DedupRbcChunkBucket}
    [] candidate = DedupRepeatedAccumulates /\
          Bug = "dedup_repeated_overwrites" ->
      spec \ {DedupRepeatedAccumulation}
    [] candidate = DedupSnapshotProjects /\
          Bug = "dedup_snapshot_mismatch" ->
      spec \ {DedupSnapshotMatches}
    [] candidate = ResetMessageHandlingClearsEntries /\
          Bug = "reset_message_keeps_entries" ->
      spec \ {ResetMessageHandling}
    [] candidate = MessageHandlingRecordsEntry /\
          Bug = "message_record_missing" ->
      spec \ {MessageEntryRecorded}
    [] candidate = MessageHandlingRepeatedAccumulates /\
          Bug = "message_repeated_overwrites" ->
      spec \ {MessageRepeatedAccumulation}
    [] candidate = MessageHandlingKindSeparatesKeys /\
          Bug = "message_kind_ignored" ->
      spec \ {MessageKindInKey}
    [] candidate = MessageHandlingOutcomeSeparatesKeys /\
          Bug = "message_outcome_ignored" ->
      spec \ {MessageOutcomeInKey}
    [] candidate = MessageHandlingReasonSeparatesKeys /\
          Bug = "message_reason_ignored" ->
      spec \ {MessageReasonInKey}
    [] candidate = MessageHandlingSnapshotProjectsKey /\
          Bug = "message_snapshot_key_mismatch" ->
      spec \ {MessageSnapshotKeyMatches}
    [] candidate = MessageHandlingSnapshotProjectsTotal /\
          Bug = "message_snapshot_total_mismatch" ->
      spec \ {MessageSnapshotTotalMatches}
    [] candidate = ResetMessageHandlingAfterRecord /\
          Bug = "reset_message_after_record_keeps_entries" ->
      spec \ {ResetMessageHandling}
    [] candidate = ResetDedupAfterRecord /\
          Bug = "reset_dedup_after_record_keeps_counters" ->
      spec \ {ResetDedup}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "reset_gossip_keeps_counters",
       "gossip_fallback_not_counted",
       "gossip_duplicate_counts_fallback",
       "quorum_stall_not_counted",
       "retransmit_last_not_updated",
       "retransmit_total_overwrites",
       "retransmit_samples_not_counted",
       "retransmit_zero_skips_sample",
       "retransmit_skip_buckets_collide",
       "bg_post_counts_broadcast",
       "bg_broadcast_counts_post",
       "bg_unknown_counts_post",
       "block_created_buckets_collide",
       "reset_dedup_keeps_counters",
       "dedup_zero_counts_capacity",
       "dedup_vote_omits_capacity",
       "dedup_vote_omits_expired",
       "dedup_proposal_expired_counts_capacity",
       "dedup_fetch_body_wrong_bucket",
       "dedup_block_body_response_wrong_bucket",
       "dedup_rbc_buckets_collide",
       "dedup_repeated_overwrites",
       "dedup_snapshot_mismatch",
       "reset_message_keeps_entries",
       "message_record_missing",
       "message_repeated_overwrites",
       "message_kind_ignored",
       "message_outcome_ignored",
       "message_reason_ignored",
       "message_snapshot_key_mismatch",
       "message_snapshot_total_mismatch",
       "reset_message_after_record_keeps_entries",
       "reset_dedup_after_record_keeps_counters"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

IngressStatusCountersExactness ==
  Safety

IngressStatusCountersCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ IngressStatusCountersExactness

BugResetGossipKeepsCounters ==
  ImplementationActions(ResetGossipClearsCounters) =
    SpecActions(ResetGossipClearsCounters)

BugGossipFallbackNotCounted ==
  ImplementationActions(GossipFallbackAccumulates) =
    SpecActions(GossipFallbackAccumulates)

BugGossipDuplicateCountsFallback ==
  ImplementationActions(GossipDuplicateAccumulates) =
    SpecActions(GossipDuplicateAccumulates)

BugQuorumStallNotCounted ==
  ImplementationActions(QuorumStallAccumulates) =
    SpecActions(QuorumStallAccumulates)

BugRetransmitLastNotUpdated ==
  ImplementationActions(RetransmitTargetStoresLast) =
    SpecActions(RetransmitTargetStoresLast)

BugRetransmitTotalOverwrites ==
  ImplementationActions(RetransmitTargetAccumulatesTotal) =
    SpecActions(RetransmitTargetAccumulatesTotal)

BugRetransmitSamplesNotCounted ==
  ImplementationActions(RetransmitTargetCountsSamples) =
    SpecActions(RetransmitTargetCountsSamples)

BugRetransmitZeroSkipsSample ==
  ImplementationActions(RetransmitTargetZeroCountsSample) =
    SpecActions(RetransmitTargetZeroCountsSample)

BugRetransmitSkipBucketsCollide ==
  ImplementationActions(RetransmitSkipReasonsDistinct) =
    SpecActions(RetransmitSkipReasonsDistinct)

BugBgPostCountsBroadcast ==
  ImplementationActions(BackgroundPostKindsCountPost) =
    SpecActions(BackgroundPostKindsCountPost)

BugBgBroadcastCountsPost ==
  ImplementationActions(BackgroundBroadcastKindsCountBroadcast) =
    SpecActions(BackgroundBroadcastKindsCountBroadcast)

BugBgUnknownCountsPost ==
  ImplementationActions(BackgroundUnknownDoesNotCount) =
    SpecActions(BackgroundUnknownDoesNotCount)

BugBlockCreatedBucketsCollide ==
  ImplementationActions(BlockCreatedCountersDistinct) =
    SpecActions(BlockCreatedCountersDistinct)

BugResetDedupKeepsCounters ==
  ImplementationActions(ResetDedupClearsCounters) =
    SpecActions(ResetDedupClearsCounters)

BugDedupZeroCountsCapacity ==
  ImplementationActions(DedupZeroNoop) =
    SpecActions(DedupZeroNoop)

BugDedupVoteOmitsCapacity ==
  ImplementationActions(DedupVoteCountsCapacityExpired) =
    SpecActions(DedupVoteCountsCapacityExpired)

BugDedupVoteOmitsExpired ==
  ImplementationActions(DedupVoteCountsCapacityExpired) =
    SpecActions(DedupVoteCountsCapacityExpired)

BugDedupProposalExpiredCountsCapacity ==
  ImplementationActions(DedupProposalExpiredOnly) =
    SpecActions(DedupProposalExpiredOnly)

BugDedupFetchBodyWrongBucket ==
  ImplementationActions(DedupFetchBodyAliasesPendingBlock) =
    SpecActions(DedupFetchBodyAliasesPendingBlock)

BugDedupBlockBodyResponseWrongBucket ==
  ImplementationActions(DedupBlockBodyResponseAliasesBlockSyncUpdate) =
    SpecActions(DedupBlockBodyResponseAliasesBlockSyncUpdate)

BugDedupRbcBucketsCollide ==
  ImplementationActions(DedupRbcReadyDeliverChunkDistinct) =
    SpecActions(DedupRbcReadyDeliverChunkDistinct)

BugDedupRepeatedOverwrites ==
  ImplementationActions(DedupRepeatedAccumulates) =
    SpecActions(DedupRepeatedAccumulates)

BugDedupSnapshotMismatch ==
  ImplementationActions(DedupSnapshotProjects) =
    SpecActions(DedupSnapshotProjects)

BugResetMessageKeepsEntries ==
  ImplementationActions(ResetMessageHandlingClearsEntries) =
    SpecActions(ResetMessageHandlingClearsEntries)

BugMessageRecordMissing ==
  ImplementationActions(MessageHandlingRecordsEntry) =
    SpecActions(MessageHandlingRecordsEntry)

BugMessageRepeatedOverwrites ==
  ImplementationActions(MessageHandlingRepeatedAccumulates) =
    SpecActions(MessageHandlingRepeatedAccumulates)

BugMessageKindIgnored ==
  ImplementationActions(MessageHandlingKindSeparatesKeys) =
    SpecActions(MessageHandlingKindSeparatesKeys)

BugMessageOutcomeIgnored ==
  ImplementationActions(MessageHandlingOutcomeSeparatesKeys) =
    SpecActions(MessageHandlingOutcomeSeparatesKeys)

BugMessageReasonIgnored ==
  ImplementationActions(MessageHandlingReasonSeparatesKeys) =
    SpecActions(MessageHandlingReasonSeparatesKeys)

BugMessageSnapshotKeyMismatch ==
  ImplementationActions(MessageHandlingSnapshotProjectsKey) =
    SpecActions(MessageHandlingSnapshotProjectsKey)

BugMessageSnapshotTotalMismatch ==
  ImplementationActions(MessageHandlingSnapshotProjectsTotal) =
    SpecActions(MessageHandlingSnapshotProjectsTotal)

BugResetMessageAfterRecordKeepsEntries ==
  ImplementationActions(ResetMessageHandlingAfterRecord) =
    SpecActions(ResetMessageHandlingAfterRecord)

BugResetDedupAfterRecordKeepsCounters ==
  ImplementationActions(ResetDedupAfterRecord) =
    SpecActions(ResetDedupAfterRecord)

====
