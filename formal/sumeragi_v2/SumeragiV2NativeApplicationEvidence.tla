---- MODULE SumeragiV2NativeApplicationEvidence ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
Bounded durability/publication model for control-only Native AMX participant
application evidence plus unified ordinary/Native startup repair.

The storage abstraction has four participant heights. Heights 1 and 2 are
complete standalone evidence pairs retained at startup; height 2 is named by
the descriptor-bound latest pointer. Height 3 is the newly finalized carrier,
and height 4 exists only for the mutation that attempts to reserve a second
incoming-pair publication window. Manifest and receipt temporary fsync and
authenticated promotion are separate crash boundaries, as are prune-intent
temporary publication and every unlink.

The production refinement is source-bound separately to the Native signing,
manifest/receipt validation, standalone Kura publication, descriptor-bound
latest-pointer, prune-intent completion, startup rebuild, WSV frontier, and
retirement entry points. It additionally source-binds the read-only all-group
startup planner, generic canonical-body recovery, reverse merge-carrier pass,
all-group preflight/application/readback, and Queue reconciliation order. The
finite model does not prove that those Rust entry points refine this ordering.
***************************************************************************)

CONSTANTS
  \* @type: Str;
  Mode,
  \* @type: Int;
  SourceCount

NativeSourceClaimV4FieldMutationModes ==
  {"DivergentSourceClaimSourceId",
   "DivergentSourceClaimTxEntrypointHash",
   "DivergentSourceClaimPlanDigest",
   "DivergentSourceClaimRoundContextId",
   "DivergentSourceClaimRoundHeight",
   "DivergentSourceClaimRoundView",
   "DivergentSourceClaimEpoch",
   "DivergentSourceClaimChainIdHash",
   "DivergentSourceClaimAuthorityContextHeight",
   "DivergentSourceClaimCoordinatorLaneId",
   "DivergentSourceClaimCoordinatorDataspaceId",
   "DivergentSourceClaimCoordinatorLaneIncarnation",
   "DivergentSourceClaimPlannedCoordinatorBlockHeight",
   "DivergentSourceClaimCoordinatorLaneBlockView",
   "DivergentSourceClaimCoordinatorProposalHash",
   "DivergentSourceClaimParticipantLaneId",
   "DivergentSourceClaimParticipantDataspaceId",
   "DivergentSourceClaimParticipantLaneIncarnation"}

NativeSourceClaimParticipantMembershipMutationMode ==
  "DivergentSourceClaimParticipantMembership"

NativeSourceClaimPreciseMutationModes ==
  NativeSourceClaimV4FieldMutationModes
    \union {NativeSourceClaimParticipantMembershipMutationMode}

NativeSourceClaimMutationModes ==
  NativeSourceClaimPreciseMutationModes \union {"DivergentSourceClaim"}

NativeEvidenceModes ==
  {"Fixed", "PublishFrontierEarly", "PruneWithHashOnly",
   "SeparateSameRouteMarker", "DivergentSourceClaim",
   "NonContiguousRoute", "PartialGroupApplication",
   "ForgedManifestLeaf", "DropStartupRepair", "AmbiguousLatestIndex",
   "SeparateArtifactBudgets", "TwoIncomingPairHeadroom",
   "UnauthenticatedTempPromotion", "PuncturedRetainedHistory",
   "NonOldestPrefixPrune", "NonHighestRepairHalf",
   "MultipleRepairHalves", "ConflictingRetainedPairIdentity",
   "RetainedPredecessorDrift", "MutatingUnifiedStartupPlan",
   "UncoalescedCanonicalBodyNeeds", "PartialUnifiedStartupPreflight",
   "QueueBeforeEvidenceReadback", "MissingReverseMergeCarrier",
   "OrphanMergeCarrier", "SkipPostCacheCarrierReconcile",
   "RepairHistoricalSiblingAsActive", "PruneWithoutProtectedLatest",
   "PruneNamespaceRebind", "DiscardAuthenticatedLatestTemp"}
  \union NativeSourceClaimPreciseMutationModes

NativeEvidencePhases ==
  {"Certified", "FinalityDurable", "ManifestTempDurable",
   "ManifestDurable", "ReceiptTempDurable", "ReceiptDurable",
   "LatestTempDurable", "LatestTempCrashed", "LatestDurable", "Published"}

NativeLatestTempPhases == {"LatestTempDurable", "LatestTempCrashed"}

NativePruneStages ==
  {"Idle", "TempDurable", "IntentDurable",
   "ManifestUnlinked", "ReceiptUnlinked", "Completed"}

NativeSourceClaimPhases ==
  {"Unrecorded", "Durable", "Crashed", "Reloaded",
   "ExactReplayAccepted", "RetryChecked"}

UnifiedStartupRepairStages ==
  {"Unplanned", "NeedBodies", "BodiesRecovered", "CarrierPreflighted",
   "PlanReady", "GroupsPreflight", "EvidenceApplied",
   "ReadBackVerified", "QueueReconciled"}

OrdinaryReceiptRepairGroup == "OrdinaryReceipt"
NativeMarkerRepairGroup == "NativeMarker"
MergeCarrierRepairGroup == "MergeCarrier"
UnifiedEvidenceRepairGroups ==
  {OrdinaryReceiptRepairGroup, NativeMarkerRepairGroup,
   MergeCarrierRepairGroup}
UnifiedEvidenceRepairGroupCount == Cardinality(UnifiedEvidenceRepairGroups)

\* A finalized global carrier authenticates every Native participant leaf it
\* committed, including a historical incarnation that State has since
\* advanced, retired, and recreated. Startup repair must authenticate that
\* complete manifest without treating every authenticated leaf as a current
\* State-marker repair target.
ActiveStateMarkerRouteIdentity ==
  "lane=1/dataspace=1/incarnation=2"
HistoricalSiblingManifestRouteIdentity ==
  "lane=1/dataspace=1/incarnation=1"
QcAuthenticatedCarrierManifestRoutes ==
  {ActiveStateMarkerRouteIdentity,
   HistoricalSiblingManifestRouteIdentity}
ExactActiveStateMarkerRepairRoutes ==
  {ActiveStateMarkerRouteIdentity}

TargetHeight == 3
ExtraIncomingHeight == 4
PreviousLatestHeight == 2
InitialEvidenceHeights == {1, 2}
EvidenceHeights == 1..ExtraIncomingHeight
RetentionLimit == 2
ArtifactByteUnit == 1
EvidencePairByteUnits == 2 * ArtifactByteUnit
StableAggregateByteLimit == RetentionLimit * EvidencePairByteUnits
IncomingPairByteLimit == EvidencePairByteUnits
PublicationTransientByteLimit ==
  StableAggregateByteLimit + IncomingPairByteLimit

NoHeight == 0
PruneIntentVersion == 2
ActiveRoute == 1
ActiveIncarnation == 2
OldestPrunableHeight == 1
NonOldestPrunableHeight == 2
PruneTargetHeight ==
  IF Mode = "NonOldestPrefixPrune"
  THEN NonOldestPrunableHeight
  ELSE OldestPrunableHeight

ManifestArtifactHash(height) == height + 10
ReceiptArtifactHash(height) == height + 20

(***************************************************************************
The V4 signing journal is represented as an append-only durable record plus
the route-keyed source claim reconstructed from it. Every bound scalar uses a
separate record field. The transaction entrypoint hash also carries its type
tag, so it cannot be confused with an untyped plan, proposal, or settlement
hash. Participant membership is independent of the three participant-route
fields: membership says which participant leg the routing plan authorizes,
while the participant claim binds that member to one route and incarnation.

The bounded trace persists one exact record, crashes after durability, loses
all volatile state, reloads the exact source-to-claim map, accepts an exact
idempotent replay, and finally checks one divergent retry. Fixed mode rejects
that retry. Each mutation mode changes one field and weakens only that field's
comparison; the legacy aggregate mutation changes and weakens every field.
***************************************************************************)

ExactClaimValue == 1
DivergentClaimValue == 2

ExactSourceClaimKey == ExactClaimValue
SourceClaimKeys == {ExactSourceClaimKey}
ExactParticipantMember == ExactClaimValue
UnexpectedParticipantMember == DivergentClaimValue

TypedTransactionEntrypointHash(digest) ==
  [type_tag |-> "TransactionEntrypoint", digest |-> digest]

NativeSourceSessionClaimV4(
    sourceId,
    txEntrypointHash,
    planDigest,
    roundContextId,
    roundHeight,
    roundView,
    epochValue,
    chainIdHash,
    authorityContextHeight,
    coordinatorLaneId,
    coordinatorDataspaceId,
    coordinatorLaneIncarnation,
    plannedCoordinatorBlockHeight,
    coordinatorLaneBlockView,
    coordinatorProposalHash) ==
  [source_id |-> sourceId,
   tx_entrypoint_hash |-> TypedTransactionEntrypointHash(txEntrypointHash),
   plan_digest |-> planDigest,
   round |-> [context_id |-> roundContextId,
              height |-> roundHeight,
              view |-> roundView],
   epoch |-> epochValue,
   chain_id_hash |-> chainIdHash,
   authority_context_height |-> authorityContextHeight,
   coordinator_lane_id |-> coordinatorLaneId,
   coordinator_dataspace_id |-> coordinatorDataspaceId,
   coordinator_lane_incarnation |-> coordinatorLaneIncarnation,
   planned_coordinator_block_height |-> plannedCoordinatorBlockHeight,
   coordinator_lane_block_view |-> coordinatorLaneBlockView,
   coordinator_proposal_hash |-> coordinatorProposalHash]

NativeSourceParticipantClaimV4(laneId, dataspaceId, laneIncarnation) ==
  [lane_id |-> laneId,
   dataspace_id |-> dataspaceId,
   lane_incarnation |-> laneIncarnation]

NativeDurableSourceClaimV4(sessionClaim, participants) ==
  [session |-> sessionClaim, participants |-> participants]

NativeSourceClaimAuthorization(
    sourceKey, sessionClaim, participantMember, participantClaim) ==
  [source_key |-> sourceKey,
   session |-> sessionClaim,
   participant_member |-> participantMember,
   participant |-> participantClaim]

ExactSourceSessionClaim ==
  NativeSourceSessionClaimV4(
    ExactClaimValue,
    ExactClaimValue,
    ExactClaimValue,
    ExactClaimValue,
    ExactClaimValue,
    ExactClaimValue,
    ExactClaimValue,
    ExactClaimValue,
    ExactClaimValue,
    ExactClaimValue,
    ExactClaimValue,
    ExactClaimValue,
    ExactClaimValue,
    ExactClaimValue,
    ExactClaimValue)

ExactSourceParticipantClaim ==
  NativeSourceParticipantClaimV4(
    ExactClaimValue, ExactClaimValue, ExactClaimValue)

ExactParticipantClaims ==
  [member \in {ExactParticipantMember} |-> ExactSourceParticipantClaim]

ExactDurableSourceClaim ==
  NativeDurableSourceClaimV4(
    ExactSourceSessionClaim, ExactParticipantClaims)

EmptySourceClaimMap ==
  [source \in {} |-> ExactDurableSourceClaim]

ExactSourceClaimMap ==
  [source \in SourceClaimKeys |-> ExactDurableSourceClaim]

ExactSourceClaimAuthorization ==
  NativeSourceClaimAuthorization(
    ExactSourceClaimKey,
    ExactSourceSessionClaim,
    ExactParticipantMember,
    ExactSourceParticipantClaim)

SourceClaimMapFromJournalAuthorization(authorization) ==
  [source \in {authorization.source_key} |->
     NativeDurableSourceClaimV4(
       authorization.session,
       [member \in {authorization.participant_member} |->
          authorization.participant])]

ReconstructSourceClaimMapFromJournal(records) ==
  IF records = {}
  THEN EmptySourceClaimMap
  ELSE
    LET authorization == CHOOSE record \in records: TRUE
    IN SourceClaimMapFromJournalAuthorization(authorization)

RetryFieldDiverges(fieldMode) ==
  IF Mode \in NativeSourceClaimMutationModes
  THEN Mode = "DivergentSourceClaim" \/ Mode = fieldMode
  ELSE fieldMode = "DivergentSourceClaimPlanDigest"

RetryClaimValue(fieldMode) ==
  IF RetryFieldDiverges(fieldMode)
  THEN DivergentClaimValue
  ELSE ExactClaimValue

RetrySourceSessionClaim ==
  NativeSourceSessionClaimV4(
    RetryClaimValue("DivergentSourceClaimSourceId"),
    RetryClaimValue("DivergentSourceClaimTxEntrypointHash"),
    RetryClaimValue("DivergentSourceClaimPlanDigest"),
    RetryClaimValue("DivergentSourceClaimRoundContextId"),
    RetryClaimValue("DivergentSourceClaimRoundHeight"),
    RetryClaimValue("DivergentSourceClaimRoundView"),
    RetryClaimValue("DivergentSourceClaimEpoch"),
    RetryClaimValue("DivergentSourceClaimChainIdHash"),
    RetryClaimValue("DivergentSourceClaimAuthorityContextHeight"),
    RetryClaimValue("DivergentSourceClaimCoordinatorLaneId"),
    RetryClaimValue("DivergentSourceClaimCoordinatorDataspaceId"),
    RetryClaimValue("DivergentSourceClaimCoordinatorLaneIncarnation"),
    RetryClaimValue("DivergentSourceClaimPlannedCoordinatorBlockHeight"),
    RetryClaimValue("DivergentSourceClaimCoordinatorLaneBlockView"),
    RetryClaimValue("DivergentSourceClaimCoordinatorProposalHash"))

RetrySourceParticipantClaim ==
  NativeSourceParticipantClaimV4(
    RetryClaimValue("DivergentSourceClaimParticipantLaneId"),
    RetryClaimValue("DivergentSourceClaimParticipantDataspaceId"),
    RetryClaimValue("DivergentSourceClaimParticipantLaneIncarnation"))

RetryParticipantMember ==
  IF Mode \in
       {"DivergentSourceClaim",
        NativeSourceClaimParticipantMembershipMutationMode}
  THEN UnexpectedParticipantMember
  ELSE ExactParticipantMember

RetrySourceClaimAuthorization ==
  NativeSourceClaimAuthorization(
    ExactSourceClaimKey,
    RetrySourceSessionClaim,
    RetryParticipantMember,
    RetrySourceParticipantClaim)

SourceClaimFieldAccepted(fieldMode, stored, candidate) ==
  \/ stored = candidate
  \/ Mode = fieldMode
  \/ Mode = "DivergentSourceClaim"

SourceSessionClaimAccepted(stored, candidate) ==
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimSourceId",
       stored.source_id,
       candidate.source_id)
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimTxEntrypointHash",
       stored.tx_entrypoint_hash,
       candidate.tx_entrypoint_hash)
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimPlanDigest",
       stored.plan_digest,
       candidate.plan_digest)
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimRoundContextId",
       stored.round.context_id,
       candidate.round.context_id)
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimRoundHeight",
       stored.round.height,
       candidate.round.height)
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimRoundView",
       stored.round.view,
       candidate.round.view)
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimEpoch", stored.epoch, candidate.epoch)
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimChainIdHash",
       stored.chain_id_hash,
       candidate.chain_id_hash)
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimAuthorityContextHeight",
       stored.authority_context_height,
       candidate.authority_context_height)
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimCoordinatorLaneId",
       stored.coordinator_lane_id,
       candidate.coordinator_lane_id)
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimCoordinatorDataspaceId",
       stored.coordinator_dataspace_id,
       candidate.coordinator_dataspace_id)
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimCoordinatorLaneIncarnation",
       stored.coordinator_lane_incarnation,
       candidate.coordinator_lane_incarnation)
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimPlannedCoordinatorBlockHeight",
       stored.planned_coordinator_block_height,
       candidate.planned_coordinator_block_height)
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimCoordinatorLaneBlockView",
       stored.coordinator_lane_block_view,
       candidate.coordinator_lane_block_view)
  /\ SourceClaimFieldAccepted(
       "DivergentSourceClaimCoordinatorProposalHash",
       stored.coordinator_proposal_hash,
       candidate.coordinator_proposal_hash)

SourceParticipantClaimAccepted(stored, member, candidate) ==
  IF ExactParticipantMember \in DOMAIN stored.participants
  THEN
    /\ \/ member \in DOMAIN stored.participants
          \/ Mode = NativeSourceClaimParticipantMembershipMutationMode
          \/ Mode = "DivergentSourceClaim"
    /\ SourceClaimFieldAccepted(
         "DivergentSourceClaimParticipantLaneId",
         stored.participants[ExactParticipantMember].lane_id,
         candidate.lane_id)
    /\ SourceClaimFieldAccepted(
         "DivergentSourceClaimParticipantDataspaceId",
         stored.participants[ExactParticipantMember].dataspace_id,
         candidate.dataspace_id)
    /\ SourceClaimFieldAccepted(
         "DivergentSourceClaimParticipantLaneIncarnation",
         stored.participants[ExactParticipantMember].lane_incarnation,
         candidate.lane_incarnation)
  ELSE FALSE

SourceClaimGuardAccepts(
    sourceClaimMap, sessionClaim, participantMember, participantClaim) ==
  IF ExactSourceClaimKey \in DOMAIN sourceClaimMap
  THEN LET stored == sourceClaimMap[ExactSourceClaimKey]
       IN /\ SourceSessionClaimAccepted(stored.session, sessionClaim)
          /\ SourceParticipantClaimAccepted(
               stored, participantMember, participantClaim)
  ELSE FALSE

NativeEvidenceConfiguration ==
  /\ Mode \in NativeEvidenceModes
  /\ SourceCount \in 1..4096

VARIABLES
  \* @type: Str;
  phase,
  \* @type: Set(Int);
  finalizedHeights,
  \* @type: Set(Int);
  manifestFiles,
  \* @type: Set(Int);
  receiptFiles,
  \* @type: Set(Int);
  manifestTempFiles,
  \* @type: Set(Int);
  receiptTempFiles,
  \* @type: Set(Int);
  latestIndexTempFiles,
  \* @type: Bool;
  manifestTempAuthenticated,
  \* @type: Bool;
  receiptTempAuthenticated,
  \* @type: Bool;
  unauthenticatedTempPromoted,
  \* @type: Bool;
  publicationPairPendingCleanup,
  \* @type: Bool;
  retainedPairIdentitiesExact,
  \* @type: Bool;
  retainedPredecessorChainExact,
  \* @type: Bool;
  frontierPublished,
  \* @type: Int;
  frontierHeight,
  \* @type: Bool;
  canonicalWireRetained,
  \* @type: Bool;
  authenticatedProofAvailable,
  \* @type: Bool;
  manifestLeafAuthenticated,
  \* @type: Bool;
  manifestLeafExact,
  \* @type: Bool;
  manifestTempPublicationExact,
  \* @type: Bool;
  receiptTempPublicationExact,
  \* @type: Bool;
  latestTempPublicationExact,
  \* @type: Bool;
  manifestPublishedNoClobber,
  \* @type: Bool;
  receiptPublishedNoClobber,
  \* @type: Bool;
  latestIndexPublished,
  \* @type: Int;
  latestIndexHeight,
  \* @type: Bool;
  latestIndexExact,
  \* @type: Bool;
  latestIndexAmbiguous,
  \* @type: Bool;
  latestIndexBounded,
  \* @type: Bool;
  legacyDenseRejected,
  \* @type: Bool;
  legacyDenseAccepted,
  \* @type: Str;
  pruneStage,
  \* @type: Bool;
  pruneIntentTempPresent,
  \* @type: Bool;
  pruneIntentDurable,
  \* @type: Bool;
  pruneTempPublishedNoClobber,
  \* @type: Int;
  pruneIntentStoredVersion,
  \* @type: Int;
  pruneIntentRoute,
  \* @type: Int;
  pruneIntentIncarnation,
  \* @type: Int;
  pruneIntentManifestHash,
  \* @type: Int;
  pruneIntentReceiptHash,
  \* @type: Int;
  pruneIntentProtectedLatestHeight,
  \* @type: Int;
  pruneIntentProtectedLatestManifestHash,
  \* @type: Int;
  pruneIntentProtectedLatestReceiptHash,
  \* @type: Set(Int);
  pruneIntentHeights,
  \* @type: Set(Int);
  removedEvidenceHeights,
  \* @type: Bool;
  startupRepairRequired,
  \* @type: Bool;
  startupRepairCompleted,
  \* @type: Bool;
  durableApplicationLost,
  \* True only when every unlink removed the same single-link regular file
  \* whose open handle, bytes, metadata, and namespace were authenticated.
  \* @type: Bool;
  pruneExactObjectRemoval,
  \* @type: Bool;
  sameRouteSettled,
  \* @type: Bool;
  separateParticipantMarker,
  \* Exact append-only V4 signing-claim crash/reload state.
  \* @type: Str;
  sourceClaimPhase,
  volatileSourceClaimMap,
  durableSourceClaimJournalRecords,
  \* @type: Bool;
  sourceClaimReloadReconstructed,
  \* @type: Bool;
  sourceClaimExactReplayAccepted,
  \* @type: Bool;
  sourceClaimDivergentRetryAttempted,
  \* @type: Bool;
  sourceClaimDivergentRetryAccepted,
  \* @type: Bool;
  sourceClaimDivergentRetryRejected,
  \* @type: Bool;
  nativeAdmissionAttempted,
  \* @type: Bool;
  activeIncarnationExact,
  \* @type: Bool;
  predecessorExact,
  \* @type: Bool;
  contiguousNextHeight,
  \* @type: Bool;
  groupApplied,
  \* @type: Bool;
  groupUnique,
  \* @type: Bool;
  groupOrdered,
  \* @type: Bool;
  groupExactCover,
  \* @type: Bool;
  groupAppliedAtomically,
  \* @type: Str;
  startupRepairStage,
  \* @type: Set(Str);
  plannedEvidenceRepairGroups,
  \* @type: Bool;
  startupRepairPlanReadOnly,
  \* @type: Int;
  canonicalBodyNeedCount,
  \* @type: Int;
  canonicalBodiesRecovered,
  \* @type: Set(Str);
  recoveredCanonicalBodyGroups,
  \* @type: Bool;
  planRevalidatedAfterRecovery,
  \* @type: Set(Str);
  preflightedEvidenceRepairGroups,
  \* @type: Set(Str);
  appliedEvidenceRepairGroups,
  \* @type: Bool;
  evidenceRepairReadBackVerified,
  \* @type: Bool;
  queueGateOpen,
  \* @type: Bool;
  queueReservationReconciled,
  \* @type: Bool;
  finalityDeclaresMergeCarrier,
  \* @type: Bool;
  mergeCarrierRecordPresent,
  \* @type: Bool;
  mergeCarrierRecordExact,
  \* @type: Bool;
  mergeCarrierRepairPlanned,
  \* @type: Bool;
  bodyCachePopulated,
  \* @type: Bool;
  postCacheCarrierPreflighted,
  \* Full route set whose manifest leaves were authenticated against the
  \* carrier QC, deliberately broader than the current State-marker target.
  \* @type: Set(Str);
  authenticatedCarrierManifestRoutes,
  \* @type: Set(Str);
  plannedNativeMarkerRepairRoutes,
  \* @type: Set(Str);
  preflightedNativeMarkerRepairRoutes,
  \* @type: Set(Str);
  appliedNativeMarkerRepairRoutes

publicationVars ==
  <<phase, finalizedHeights, manifestFiles, receiptFiles,
    manifestTempFiles, receiptTempFiles, latestIndexTempFiles,
    manifestTempAuthenticated,
    receiptTempAuthenticated, unauthenticatedTempPromoted,
    publicationPairPendingCleanup,
    retainedPairIdentitiesExact, retainedPredecessorChainExact,
    frontierPublished, frontierHeight, canonicalWireRetained,
    authenticatedProofAvailable, manifestLeafAuthenticated,
    manifestLeafExact, manifestTempPublicationExact,
    receiptTempPublicationExact, latestTempPublicationExact,
    manifestPublishedNoClobber, receiptPublishedNoClobber,
    latestIndexPublished, latestIndexHeight, latestIndexExact,
    latestIndexAmbiguous, latestIndexBounded,
    legacyDenseRejected, legacyDenseAccepted>>

pruneVars ==
  <<pruneStage, pruneIntentTempPresent, pruneIntentDurable,
    pruneTempPublishedNoClobber, pruneIntentStoredVersion,
    pruneIntentRoute, pruneIntentIncarnation,
    pruneIntentManifestHash, pruneIntentReceiptHash,
    pruneIntentProtectedLatestHeight,
    pruneIntentProtectedLatestManifestHash,
    pruneIntentProtectedLatestReceiptHash,
    pruneIntentHeights, removedEvidenceHeights, startupRepairRequired,
    startupRepairCompleted, durableApplicationLost,
    pruneExactObjectRemoval>>

sameRouteVars == <<sameRouteSettled, separateParticipantMarker>>

sourceClaimVars ==
  <<sourceClaimPhase, volatileSourceClaimMap,
    durableSourceClaimJournalRecords, sourceClaimReloadReconstructed,
    sourceClaimExactReplayAccepted, sourceClaimDivergentRetryAttempted,
    sourceClaimDivergentRetryAccepted, sourceClaimDivergentRetryRejected>>

startupRepairRouteVars ==
  <<authenticatedCarrierManifestRoutes,
    plannedNativeMarkerRepairRoutes,
    preflightedNativeMarkerRepairRoutes,
    appliedNativeMarkerRepairRoutes>>

startupRepairVars ==
  <<startupRepairStage, plannedEvidenceRepairGroups,
    startupRepairPlanReadOnly, canonicalBodyNeedCount,
    canonicalBodiesRecovered, recoveredCanonicalBodyGroups,
    planRevalidatedAfterRecovery, preflightedEvidenceRepairGroups,
    appliedEvidenceRepairGroups, evidenceRepairReadBackVerified,
    queueGateOpen, queueReservationReconciled,
    finalityDeclaresMergeCarrier, mergeCarrierRecordPresent,
    mergeCarrierRecordExact, mergeCarrierRepairPlanned,
    bodyCachePopulated, postCacheCarrierPreflighted,
    startupRepairRouteVars>>

claimVars == <<sourceClaimVars, startupRepairVars>>

admissionVars ==
  <<nativeAdmissionAttempted, activeIncarnationExact, predecessorExact,
    contiguousNextHeight>>

groupVars ==
  <<groupApplied, groupUnique, groupOrdered, groupExactCover,
    groupAppliedAtomically>>

vars ==
  <<phase, finalizedHeights, manifestFiles, receiptFiles,
    manifestTempFiles, receiptTempFiles, latestIndexTempFiles,
    manifestTempAuthenticated,
    receiptTempAuthenticated, unauthenticatedTempPromoted,
    publicationPairPendingCleanup,
    retainedPairIdentitiesExact, retainedPredecessorChainExact,
    frontierPublished, frontierHeight, canonicalWireRetained,
    authenticatedProofAvailable, manifestLeafAuthenticated,
    manifestLeafExact, manifestTempPublicationExact,
    receiptTempPublicationExact, latestTempPublicationExact,
    manifestPublishedNoClobber, receiptPublishedNoClobber,
    latestIndexPublished, latestIndexHeight, latestIndexExact,
    latestIndexAmbiguous, latestIndexBounded,
    legacyDenseRejected, legacyDenseAccepted,
    pruneStage, pruneIntentTempPresent, pruneIntentDurable,
    pruneTempPublishedNoClobber, pruneIntentStoredVersion,
    pruneIntentRoute, pruneIntentIncarnation,
    pruneIntentManifestHash, pruneIntentReceiptHash,
    pruneIntentProtectedLatestHeight,
    pruneIntentProtectedLatestManifestHash,
    pruneIntentProtectedLatestReceiptHash,
    pruneIntentHeights, removedEvidenceHeights, startupRepairRequired,
    startupRepairCompleted, durableApplicationLost,
    pruneExactObjectRemoval,
    sameRouteSettled, separateParticipantMarker,
    sourceClaimPhase, volatileSourceClaimMap,
    durableSourceClaimJournalRecords, sourceClaimReloadReconstructed,
    sourceClaimExactReplayAccepted, sourceClaimDivergentRetryAttempted,
    sourceClaimDivergentRetryAccepted, sourceClaimDivergentRetryRejected,
    nativeAdmissionAttempted, activeIncarnationExact, predecessorExact,
    contiguousNextHeight, groupApplied, groupUnique, groupOrdered,
    groupExactCover, groupAppliedAtomically,
    startupRepairStage, plannedEvidenceRepairGroups,
    startupRepairPlanReadOnly, canonicalBodyNeedCount,
    canonicalBodiesRecovered, recoveredCanonicalBodyGroups,
    planRevalidatedAfterRecovery, preflightedEvidenceRepairGroups,
    appliedEvidenceRepairGroups, evidenceRepairReadBackVerified,
    queueGateOpen, queueReservationReconciled,
    finalityDeclaresMergeCarrier, mergeCarrierRecordPresent,
    mergeCarrierRecordExact, mergeCarrierRepairPlanned,
    bodyCachePopulated, postCacheCarrierPreflighted,
    authenticatedCarrierManifestRoutes,
    plannedNativeMarkerRepairRoutes,
    preflightedNativeMarkerRepairRoutes,
    appliedNativeMarkerRepairRoutes>>

finalityDurable == TargetHeight \in finalizedHeights
manifestDurable == TargetHeight \in manifestFiles
receiptDurable == TargetHeight \in receiptFiles
sidecarsDurable == receiptDurable

StableEvidencePayloadBytes ==
  (Cardinality(manifestFiles) + Cardinality(receiptFiles)) * ArtifactByteUnit

TemporaryEvidencePayloadBytes ==
  (Cardinality(manifestTempFiles) + Cardinality(receiptTempFiles)) * ArtifactByteUnit

TotalEvidencePayloadBytes ==
  StableEvidencePayloadBytes + TemporaryEvidencePayloadBytes

RetainedStableEvidencePayloadBytes ==
  IF publicationPairPendingCleanup
  THEN
    (Cardinality(manifestFiles \ {TargetHeight}) +
     Cardinality(receiptFiles \ {TargetHeight})) * ArtifactByteUnit
  ELSE StableEvidencePayloadBytes

IncomingEvidenceHeights ==
  ((manifestFiles \union receiptFiles \union
    manifestTempFiles \union receiptTempFiles) \ InitialEvidenceHeights)

EffectiveManifestHeights == manifestFiles \ pruneIntentHeights
EffectiveReceiptHeights == receiptFiles \ pruneIntentHeights
EffectiveRetainedHeights ==
  EffectiveManifestHeights \union EffectiveReceiptHeights
EffectivePartialHeights ==
  (EffectiveManifestHeights \ EffectiveReceiptHeights) \union
  (EffectiveReceiptHeights \ EffectiveManifestHeights)

ContiguousHeightInterval(heights) ==
  \A lower \in heights:
    \A upper \in heights:
      lower <= upper => (lower..upper) \subseteq heights

OldestPrefix(heights) ==
  \/ heights = {}
  \/ \E highest \in EvidenceHeights: heights = 1..highest

HighestHalfRepairOnly ==
  /\ Cardinality(EffectivePartialHeights) <= 1
  /\ \A partial \in EffectivePartialHeights:
       \A retained \in EffectiveRetainedHeights: retained <= partial

PruneIntentProtectedLatestExact ==
  /\ pruneIntentProtectedLatestHeight \in
       (manifestFiles \intersect receiptFiles \intersect finalizedHeights)
  /\ pruneIntentProtectedLatestManifestHash =
       ManifestArtifactHash(pruneIntentProtectedLatestHeight)
  /\ pruneIntentProtectedLatestReceiptHash =
       ReceiptArtifactHash(pruneIntentProtectedLatestHeight)
  /\ pruneIntentProtectedLatestHeight \notin pruneIntentHeights
  /\ \A removedHeight \in pruneIntentHeights:
       removedHeight < pruneIntentProtectedLatestHeight

ExactPruneIntentIdentity ==
  /\ pruneIntentStoredVersion = PruneIntentVersion
  /\ pruneIntentRoute = ActiveRoute
  /\ pruneIntentIncarnation = ActiveIncarnation
  /\ pruneIntentHeights = {PruneTargetHeight}
  /\ pruneIntentManifestHash = ManifestArtifactHash(PruneTargetHeight)
  /\ pruneIntentReceiptHash = ReceiptArtifactHash(PruneTargetHeight)
  /\ pruneIntentProtectedLatestHeight = latestIndexHeight
  /\ pruneIntentProtectedLatestManifestHash =
       ManifestArtifactHash(latestIndexHeight)
  /\ pruneIntentProtectedLatestReceiptHash =
       ReceiptArtifactHash(latestIndexHeight)
  /\ PruneIntentProtectedLatestExact

ResetPruneIntentIdentity ==
  /\ pruneIntentStoredVersion = 0
  /\ pruneIntentRoute = 0
  /\ pruneIntentIncarnation = 0
  /\ pruneIntentManifestHash = 0
  /\ pruneIntentReceiptHash = 0
  /\ pruneIntentProtectedLatestHeight = NoHeight
  /\ pruneIntentProtectedLatestManifestHash = 0
  /\ pruneIntentProtectedLatestReceiptHash = 0
  /\ pruneIntentHeights = {}

Init ==
  /\ NativeEvidenceConfiguration
  /\ phase = "Certified"
  /\ finalizedHeights = InitialEvidenceHeights
  /\ manifestFiles = InitialEvidenceHeights
  /\ receiptFiles = InitialEvidenceHeights
  /\ manifestTempFiles = {}
  /\ receiptTempFiles = {}
  /\ latestIndexTempFiles = {}
  /\ manifestTempAuthenticated = FALSE
  /\ receiptTempAuthenticated = FALSE
  /\ unauthenticatedTempPromoted = FALSE
  /\ publicationPairPendingCleanup = FALSE
  /\ retainedPairIdentitiesExact = TRUE
  /\ retainedPredecessorChainExact = TRUE
  /\ frontierPublished = FALSE
  /\ frontierHeight = NoHeight
  /\ canonicalWireRetained = TRUE
  /\ authenticatedProofAvailable = FALSE
  /\ manifestLeafAuthenticated = FALSE
  /\ manifestLeafExact = FALSE
  /\ manifestTempPublicationExact = TRUE
  /\ receiptTempPublicationExact = TRUE
  /\ latestTempPublicationExact = TRUE
  /\ manifestPublishedNoClobber = TRUE
  /\ receiptPublishedNoClobber = TRUE
  /\ latestIndexPublished = FALSE
  /\ latestIndexHeight = PreviousLatestHeight
  /\ latestIndexExact = TRUE
  /\ latestIndexAmbiguous = FALSE
  /\ latestIndexBounded = TRUE
  /\ legacyDenseRejected = TRUE
  /\ legacyDenseAccepted = FALSE
  /\ pruneStage = "Idle"
  /\ pruneIntentTempPresent = FALSE
  /\ pruneIntentDurable = FALSE
  /\ pruneTempPublishedNoClobber = TRUE
  /\ pruneIntentStoredVersion = 0
  /\ pruneIntentRoute = 0
  /\ pruneIntentIncarnation = 0
  /\ pruneIntentManifestHash = 0
  /\ pruneIntentReceiptHash = 0
  /\ pruneIntentProtectedLatestHeight = NoHeight
  /\ pruneIntentProtectedLatestManifestHash = 0
  /\ pruneIntentProtectedLatestReceiptHash = 0
  /\ pruneIntentHeights = {}
  /\ removedEvidenceHeights = {}
  /\ startupRepairRequired = FALSE
  /\ startupRepairCompleted = FALSE
  /\ durableApplicationLost = FALSE
  /\ pruneExactObjectRemoval = TRUE
  /\ sameRouteSettled = FALSE
  /\ separateParticipantMarker = FALSE
  /\ sourceClaimPhase = "Unrecorded"
  /\ volatileSourceClaimMap = EmptySourceClaimMap
  /\ durableSourceClaimJournalRecords = {}
  /\ sourceClaimReloadReconstructed = FALSE
  /\ sourceClaimExactReplayAccepted = FALSE
  /\ sourceClaimDivergentRetryAttempted = FALSE
  /\ sourceClaimDivergentRetryAccepted = FALSE
  /\ sourceClaimDivergentRetryRejected = FALSE
  /\ nativeAdmissionAttempted = FALSE
  /\ activeIncarnationExact = TRUE
  /\ predecessorExact = TRUE
  /\ contiguousNextHeight = TRUE
  /\ groupApplied = FALSE
  /\ groupUnique = TRUE
  /\ groupOrdered = TRUE
  /\ groupExactCover = TRUE
  /\ groupAppliedAtomically = TRUE
  /\ startupRepairStage = "Unplanned"
  /\ plannedEvidenceRepairGroups = {}
  /\ startupRepairPlanReadOnly = TRUE
  /\ canonicalBodyNeedCount = 0
  /\ canonicalBodiesRecovered = 0
  /\ recoveredCanonicalBodyGroups = {}
  /\ planRevalidatedAfterRecovery = FALSE
  /\ preflightedEvidenceRepairGroups = {}
  /\ appliedEvidenceRepairGroups = {}
  /\ evidenceRepairReadBackVerified = FALSE
  /\ queueGateOpen = FALSE
  /\ queueReservationReconciled = FALSE
  /\ finalityDeclaresMergeCarrier = (Mode # "OrphanMergeCarrier")
  /\ mergeCarrierRecordPresent = (Mode = "OrphanMergeCarrier")
  /\ mergeCarrierRecordExact = (Mode = "OrphanMergeCarrier")
  /\ mergeCarrierRepairPlanned = FALSE
  /\ bodyCachePopulated = FALSE
  /\ postCacheCarrierPreflighted = FALSE
  /\ authenticatedCarrierManifestRoutes = {}
  /\ plannedNativeMarkerRepairRoutes = {}
  /\ preflightedNativeMarkerRepairRoutes = {}
  /\ appliedNativeMarkerRepairRoutes = {}

PersistFinality ==
  /\ phase = "Certified"
  /\ phase' = "FinalityDurable"
  /\ finalizedHeights' = finalizedHeights \union {TargetHeight}
  /\ UNCHANGED
       <<manifestFiles, receiptFiles, manifestTempFiles, receiptTempFiles,
         latestIndexTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPairIdentitiesExact,
         retainedPredecessorChainExact, frontierPublished, frontierHeight,
         canonicalWireRetained, authenticatedProofAvailable,
         manifestLeafAuthenticated, manifestLeafExact,
         manifestTempPublicationExact, receiptTempPublicationExact,
         latestTempPublicationExact, manifestPublishedNoClobber,
         receiptPublishedNoClobber, latestIndexPublished,
         latestIndexHeight, latestIndexExact, latestIndexAmbiguous,
         latestIndexBounded, legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

StageStandaloneManifestTemp ==
  /\ phase = "FinalityDurable"
  /\ finalityDurable
  /\ TargetHeight \notin manifestFiles
  /\ TargetHeight \notin manifestTempFiles
  /\ phase' = "ManifestTempDurable"
  /\ manifestTempFiles' = manifestTempFiles \union {TargetHeight}
  /\ manifestTempAuthenticated' =
       Mode \notin {"ForgedManifestLeaf", "UnauthenticatedTempPromotion"}
  /\ publicationPairPendingCleanup' = TRUE
  /\ authenticatedProofAvailable' = TRUE
  /\ manifestLeafAuthenticated' = (Mode # "ForgedManifestLeaf")
  /\ manifestLeafExact' = (Mode # "ForgedManifestLeaf")
  /\ manifestTempPublicationExact' = (Mode # "AmbiguousLatestIndex")
  /\ manifestPublishedNoClobber' = (Mode # "AmbiguousLatestIndex")
  /\ UNCHANGED
       <<finalizedHeights, manifestFiles, receiptFiles, receiptTempFiles,
         latestIndexTempFiles,
         receiptTempAuthenticated, unauthenticatedTempPromoted,
         retainedPairIdentitiesExact, retainedPredecessorChainExact,
         frontierPublished, frontierHeight, canonicalWireRetained,
         receiptTempPublicationExact, latestTempPublicationExact,
         receiptPublishedNoClobber, latestIndexPublished,
         latestIndexHeight, latestIndexExact, latestIndexAmbiguous,
         latestIndexBounded, legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

PersistStandaloneManifest ==
  /\ phase = "ManifestTempDurable"
  /\ finalityDurable
  /\ TargetHeight \in manifestTempFiles
  /\ TargetHeight \notin manifestFiles
  /\ (manifestTempAuthenticated
       \/ Mode \in {"ForgedManifestLeaf", "UnauthenticatedTempPromotion"})
  /\ phase' = "ManifestDurable"
  /\ manifestFiles' = manifestFiles \union {TargetHeight}
  /\ manifestTempFiles' = manifestTempFiles \ {TargetHeight}
  /\ manifestTempAuthenticated' = FALSE
  /\ unauthenticatedTempPromoted' =
       unauthenticatedTempPromoted \/ ~manifestTempAuthenticated
  /\ UNCHANGED
       <<finalizedHeights, receiptFiles, receiptTempFiles,
         latestIndexTempFiles,
         receiptTempAuthenticated, retainedPairIdentitiesExact,
         publicationPairPendingCleanup,
         retainedPredecessorChainExact, frontierPublished, frontierHeight,
         canonicalWireRetained, authenticatedProofAvailable,
         manifestLeafAuthenticated, manifestLeafExact,
         manifestTempPublicationExact, receiptTempPublicationExact,
         latestTempPublicationExact, manifestPublishedNoClobber,
         receiptPublishedNoClobber,
         latestIndexPublished, latestIndexHeight, latestIndexExact,
         latestIndexAmbiguous, latestIndexBounded,
         legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

StageStandaloneReceiptTemp ==
  /\ phase = "ManifestDurable"
  /\ finalityDurable
  /\ manifestDurable
  /\ TargetHeight \notin receiptFiles
  /\ TargetHeight \notin receiptTempFiles
  /\ phase' = "ReceiptTempDurable"
  /\ receiptTempFiles' = receiptTempFiles \union {TargetHeight}
  /\ receiptTempAuthenticated' =
       manifestLeafAuthenticated /\ Mode # "UnauthenticatedTempPromotion"
  /\ receiptTempPublicationExact' = (Mode # "AmbiguousLatestIndex")
  /\ receiptPublishedNoClobber' = (Mode # "AmbiguousLatestIndex")
  /\ UNCHANGED
       <<finalizedHeights, manifestFiles, receiptFiles, manifestTempFiles,
         latestIndexTempFiles,
         manifestTempAuthenticated, unauthenticatedTempPromoted,
         publicationPairPendingCleanup,
         retainedPairIdentitiesExact, retainedPredecessorChainExact,
         frontierPublished, frontierHeight, canonicalWireRetained,
         authenticatedProofAvailable, manifestLeafAuthenticated,
         manifestLeafExact, manifestTempPublicationExact,
         latestTempPublicationExact, manifestPublishedNoClobber,
         latestIndexPublished, latestIndexHeight, latestIndexExact,
         latestIndexAmbiguous, latestIndexBounded,
         legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

PersistStandaloneReceipt ==
  /\ phase = "ReceiptTempDurable"
  /\ finalityDurable
  /\ manifestDurable
  /\ TargetHeight \in receiptTempFiles
  /\ TargetHeight \notin receiptFiles
  /\ (receiptTempAuthenticated
       \/ Mode \in {"ForgedManifestLeaf", "UnauthenticatedTempPromotion"})
  /\ phase' = "ReceiptDurable"
  /\ receiptFiles' = receiptFiles \union {TargetHeight}
  /\ receiptTempFiles' = receiptTempFiles \ {TargetHeight}
  /\ receiptTempAuthenticated' = FALSE
  /\ unauthenticatedTempPromoted' =
       unauthenticatedTempPromoted \/ ~receiptTempAuthenticated
  /\ UNCHANGED
       <<finalizedHeights, manifestFiles, manifestTempFiles,
         latestIndexTempFiles,
         manifestTempAuthenticated, retainedPairIdentitiesExact,
         publicationPairPendingCleanup,
         retainedPredecessorChainExact, frontierPublished, frontierHeight,
         canonicalWireRetained, authenticatedProofAvailable,
         manifestLeafAuthenticated, manifestLeafExact,
         manifestTempPublicationExact, receiptTempPublicationExact,
         latestTempPublicationExact, manifestPublishedNoClobber,
         receiptPublishedNoClobber, latestIndexPublished,
         latestIndexHeight, latestIndexExact, latestIndexAmbiguous,
         latestIndexBounded, legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

StageDescriptorBoundLatestTemp ==
  /\ phase = "ReceiptDurable"
  /\ receiptDurable
  /\ TargetHeight \in manifestFiles
  /\ latestIndexTempFiles = {}
  /\ phase' = "LatestTempDurable"
  /\ latestIndexTempFiles' = {TargetHeight}
  /\ UNCHANGED
       <<finalizedHeights, manifestFiles, receiptFiles,
         manifestTempFiles, receiptTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPairIdentitiesExact, retainedPredecessorChainExact,
         frontierPublished, frontierHeight, canonicalWireRetained,
         authenticatedProofAvailable, manifestLeafAuthenticated,
         manifestLeafExact, manifestTempPublicationExact,
         receiptTempPublicationExact, latestTempPublicationExact,
         manifestPublishedNoClobber, receiptPublishedNoClobber,
         latestIndexPublished, latestIndexHeight, latestIndexExact,
         latestIndexAmbiguous, latestIndexBounded,
         legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

PublishDescriptorBoundLatest ==
  /\ phase = "LatestTempDurable"
  /\ latestIndexTempFiles = {TargetHeight}
  /\ receiptDurable
  /\ TargetHeight \in manifestFiles
  /\ phase' = "LatestDurable"
  /\ latestIndexTempFiles' = {}
  /\ latestIndexPublished' = TRUE
  /\ latestIndexHeight' = TargetHeight
  /\ latestIndexExact' = (Mode # "AmbiguousLatestIndex")
  /\ latestIndexAmbiguous' = (Mode = "AmbiguousLatestIndex")
  /\ latestIndexBounded' = TRUE
  /\ latestTempPublicationExact' = (Mode # "AmbiguousLatestIndex")
  /\ legacyDenseRejected' = (Mode # "AmbiguousLatestIndex")
  /\ legacyDenseAccepted' = (Mode = "AmbiguousLatestIndex")
  /\ UNCHANGED
       <<finalizedHeights, manifestFiles, receiptFiles,
         manifestTempFiles, receiptTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPairIdentitiesExact,
         retainedPredecessorChainExact,
         frontierPublished, frontierHeight, canonicalWireRetained,
         authenticatedProofAvailable, manifestLeafAuthenticated,
         manifestLeafExact, manifestTempPublicationExact,
         receiptTempPublicationExact, manifestPublishedNoClobber,
         receiptPublishedNoClobber>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

CrashAfterDescriptorBoundLatestTemp ==
  /\ Mode \in {"Fixed", "DiscardAuthenticatedLatestTemp"}
  /\ phase = "LatestTempDurable"
  /\ latestIndexTempFiles = {TargetHeight}
  /\ phase' = "LatestTempCrashed"
  /\ UNCHANGED
       <<finalizedHeights, manifestFiles, receiptFiles,
         manifestTempFiles, receiptTempFiles, latestIndexTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPairIdentitiesExact, retainedPredecessorChainExact,
         frontierPublished, frontierHeight, canonicalWireRetained,
         authenticatedProofAvailable, manifestLeafAuthenticated,
         manifestLeafExact, manifestTempPublicationExact,
         receiptTempPublicationExact, latestTempPublicationExact,
         manifestPublishedNoClobber, receiptPublishedNoClobber,
         latestIndexPublished, latestIndexHeight, latestIndexExact,
         latestIndexAmbiguous, latestIndexBounded,
         legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

RecoverDescriptorBoundLatestTempAtStartup ==
  /\ phase = "LatestTempCrashed"
  /\ latestIndexTempFiles = {TargetHeight}
  /\ TargetHeight \in
       (finalizedHeights \intersect manifestFiles \intersect receiptFiles)
  /\ manifestLeafAuthenticated
  /\ manifestLeafExact
  /\ latestTempPublicationExact
  /\ phase' = "LatestDurable"
  /\ latestIndexTempFiles' = {}
  /\ latestIndexPublished' = TRUE
  /\ latestIndexHeight' = TargetHeight
  /\ latestIndexExact' = TRUE
  /\ latestIndexAmbiguous' = FALSE
  /\ latestIndexBounded' = TRUE
  /\ UNCHANGED
       <<finalizedHeights, manifestFiles, receiptFiles,
         manifestTempFiles, receiptTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPairIdentitiesExact, retainedPredecessorChainExact,
         frontierPublished, frontierHeight, canonicalWireRetained,
         authenticatedProofAvailable, manifestLeafAuthenticated,
         manifestLeafExact, manifestTempPublicationExact,
         receiptTempPublicationExact, latestTempPublicationExact,
         manifestPublishedNoClobber, receiptPublishedNoClobber,
         legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

DiscardAuthenticatedLatestTempAtStartup ==
  /\ Mode = "DiscardAuthenticatedLatestTemp"
  /\ phase = "LatestTempCrashed"
  /\ latestIndexTempFiles = {TargetHeight}
  /\ TargetHeight \in
       (finalizedHeights \intersect manifestFiles \intersect receiptFiles)
  /\ manifestLeafAuthenticated
  /\ manifestLeafExact
  /\ latestTempPublicationExact
  /\ latestIndexTempFiles' = {}
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestFiles, receiptFiles,
         manifestTempFiles, receiptTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPairIdentitiesExact, retainedPredecessorChainExact,
         frontierPublished, frontierHeight, canonicalWireRetained,
         authenticatedProofAvailable, manifestLeafAuthenticated,
         manifestLeafExact, manifestTempPublicationExact,
         receiptTempPublicationExact, latestTempPublicationExact,
         manifestPublishedNoClobber, receiptPublishedNoClobber,
         latestIndexPublished, latestIndexHeight, latestIndexExact,
         latestIndexAmbiguous, latestIndexBounded,
         legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

PublishReplicatedFrontier ==
  /\ IF Mode = "PublishFrontierEarly"
     THEN /\ phase = "FinalityDurable"
          /\ finalityDurable
     ELSE /\ phase = "LatestDurable"
          /\ latestIndexPublished
          /\ latestIndexHeight = TargetHeight
          /\ manifestTempFiles = {}
          /\ receiptTempFiles = {}
          /\ TotalEvidencePayloadBytes <= PublicationTransientByteLimit
  /\ phase' = "Published"
  /\ frontierPublished' = TRUE
  /\ frontierHeight' = TargetHeight
  /\ publicationPairPendingCleanup' =
       IF Mode = "SeparateArtifactBudgets"
       THEN FALSE
       ELSE publicationPairPendingCleanup
  /\ UNCHANGED
       <<finalizedHeights, manifestFiles, receiptFiles,
         manifestTempFiles, receiptTempFiles, latestIndexTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, retainedPairIdentitiesExact,
         retainedPredecessorChainExact,
         canonicalWireRetained, authenticatedProofAvailable,
         manifestLeafAuthenticated, manifestLeafExact,
         manifestTempPublicationExact, receiptTempPublicationExact,
         latestTempPublicationExact, manifestPublishedNoClobber,
         receiptPublishedNoClobber, latestIndexPublished,
         latestIndexHeight, latestIndexExact, latestIndexAmbiguous,
         latestIndexBounded, legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

PruneCanonicalWire ==
  /\ IF Mode = "PruneWithHashOnly"
     THEN /\ phase = "FinalityDurable"
          /\ finalityDurable
     ELSE /\ frontierPublished
          /\ finalityDurable
          /\ manifestDurable
          /\ receiptDurable
          /\ authenticatedProofAvailable
  /\ canonicalWireRetained
  /\ canonicalWireRetained' = FALSE
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestFiles, receiptFiles,
         manifestTempFiles, receiptTempFiles, latestIndexTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPairIdentitiesExact,
         retainedPredecessorChainExact,
         frontierPublished, frontierHeight, authenticatedProofAvailable,
         manifestLeafAuthenticated, manifestLeafExact,
         manifestTempPublicationExact, receiptTempPublicationExact,
         latestTempPublicationExact, manifestPublishedNoClobber,
         receiptPublishedNoClobber, latestIndexPublished,
         latestIndexHeight, latestIndexExact, latestIndexAmbiguous,
         latestIndexBounded, legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

StageSecondIncomingPair ==
  /\ Mode = "TwoIncomingPairHeadroom"
  /\ phase = "ReceiptDurable"
  /\ manifestTempFiles = {}
  /\ receiptTempFiles = {}
  /\ manifestTempFiles' = {ExtraIncomingHeight}
  /\ receiptTempFiles' = {ExtraIncomingHeight}
  /\ manifestTempAuthenticated' = FALSE
  /\ receiptTempAuthenticated' = FALSE
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestFiles, receiptFiles,
         latestIndexTempFiles,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPairIdentitiesExact,
         retainedPredecessorChainExact, frontierPublished, frontierHeight,
         canonicalWireRetained, authenticatedProofAvailable,
         manifestLeafAuthenticated, manifestLeafExact,
         manifestTempPublicationExact, receiptTempPublicationExact,
         latestTempPublicationExact, manifestPublishedNoClobber,
         receiptPublishedNoClobber, latestIndexPublished,
         latestIndexHeight, latestIndexExact, latestIndexAmbiguous,
         latestIndexBounded, legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

PunctureRetainedHistory ==
  /\ Mode = "PuncturedRetainedHistory"
  /\ phase = "ReceiptDurable"
  /\ NonOldestPrunableHeight \in manifestFiles
  /\ NonOldestPrunableHeight \in receiptFiles
  /\ manifestFiles' = manifestFiles \ {NonOldestPrunableHeight}
  /\ receiptFiles' = receiptFiles \ {NonOldestPrunableHeight}
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestTempFiles, receiptTempFiles,
         latestIndexTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPairIdentitiesExact,
         retainedPredecessorChainExact, frontierPublished, frontierHeight,
         canonicalWireRetained, authenticatedProofAvailable,
         manifestLeafAuthenticated, manifestLeafExact,
         manifestTempPublicationExact, receiptTempPublicationExact,
         latestTempPublicationExact, manifestPublishedNoClobber,
         receiptPublishedNoClobber, latestIndexPublished,
         latestIndexHeight, latestIndexExact, latestIndexAmbiguous,
         latestIndexBounded, legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

CreateNonHighestRepairHalf ==
  /\ Mode = "NonHighestRepairHalf"
  /\ phase = "Certified"
  /\ OldestPrunableHeight \in receiptFiles
  /\ receiptFiles' = receiptFiles \ {OldestPrunableHeight}
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestFiles,
         manifestTempFiles, receiptTempFiles, latestIndexTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPairIdentitiesExact,
         retainedPredecessorChainExact, frontierPublished, frontierHeight,
         canonicalWireRetained, authenticatedProofAvailable,
         manifestLeafAuthenticated, manifestLeafExact,
         manifestTempPublicationExact, receiptTempPublicationExact,
         latestTempPublicationExact, manifestPublishedNoClobber,
         receiptPublishedNoClobber, latestIndexPublished,
         latestIndexHeight, latestIndexExact, latestIndexAmbiguous,
         latestIndexBounded, legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

CreateMultipleRepairHalves ==
  /\ Mode = "MultipleRepairHalves"
  /\ phase = "Certified"
  /\ receiptFiles = InitialEvidenceHeights
  /\ receiptFiles' = {}
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestFiles,
         manifestTempFiles, receiptTempFiles, latestIndexTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPairIdentitiesExact,
         retainedPredecessorChainExact, frontierPublished, frontierHeight,
         canonicalWireRetained, authenticatedProofAvailable,
         manifestLeafAuthenticated, manifestLeafExact,
         manifestTempPublicationExact, receiptTempPublicationExact,
         latestTempPublicationExact, manifestPublishedNoClobber,
         receiptPublishedNoClobber, latestIndexPublished,
         latestIndexHeight, latestIndexExact, latestIndexAmbiguous,
         latestIndexBounded, legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

CorruptRetainedPairIdentity ==
  /\ Mode = "ConflictingRetainedPairIdentity"
  /\ retainedPairIdentitiesExact
  /\ retainedPairIdentitiesExact' = FALSE
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestFiles, receiptFiles,
         manifestTempFiles, receiptTempFiles, latestIndexTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPredecessorChainExact,
         frontierPublished, frontierHeight, canonicalWireRetained,
         authenticatedProofAvailable, manifestLeafAuthenticated,
         manifestLeafExact, manifestTempPublicationExact,
         receiptTempPublicationExact, latestTempPublicationExact,
         manifestPublishedNoClobber, receiptPublishedNoClobber,
         latestIndexPublished, latestIndexHeight, latestIndexExact,
         latestIndexAmbiguous, latestIndexBounded,
         legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

DriftRetainedPredecessor ==
  /\ Mode = "RetainedPredecessorDrift"
  /\ retainedPredecessorChainExact
  /\ retainedPredecessorChainExact' = FALSE
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestFiles, receiptFiles,
         manifestTempFiles, receiptTempFiles, latestIndexTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPairIdentitiesExact,
         frontierPublished, frontierHeight, canonicalWireRetained,
         authenticatedProofAvailable, manifestLeafAuthenticated,
         manifestLeafExact, manifestTempPublicationExact,
         receiptTempPublicationExact, latestTempPublicationExact,
         manifestPublishedNoClobber, receiptPublishedNoClobber,
         latestIndexPublished, latestIndexHeight, latestIndexExact,
         latestIndexAmbiguous, latestIndexBounded,
         legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

StageNativePruneIntentTemp ==
  /\ pruneStage = "Idle"
  /\ (frontierPublished \/ Mode = "DropStartupRepair")
  /\ PruneTargetHeight \in manifestFiles
  /\ PruneTargetHeight \in receiptFiles
  /\ latestIndexHeight # PruneTargetHeight
  /\ pruneStage' = "TempDurable"
  /\ pruneIntentTempPresent' = TRUE
  /\ pruneIntentDurable' = FALSE
  /\ pruneTempPublishedNoClobber' = (Mode # "AmbiguousLatestIndex")
  /\ pruneIntentStoredVersion' = PruneIntentVersion
  /\ pruneIntentRoute' = ActiveRoute
  /\ pruneIntentIncarnation' = ActiveIncarnation
  /\ pruneIntentManifestHash' = ManifestArtifactHash(PruneTargetHeight)
  /\ pruneIntentReceiptHash' = ReceiptArtifactHash(PruneTargetHeight)
  /\ pruneIntentProtectedLatestHeight' =
       IF Mode = "PruneWithoutProtectedLatest"
       THEN NoHeight
       ELSE latestIndexHeight
  /\ pruneIntentProtectedLatestManifestHash' =
       IF Mode = "PruneWithoutProtectedLatest"
       THEN 0
       ELSE ManifestArtifactHash(latestIndexHeight)
  /\ pruneIntentProtectedLatestReceiptHash' =
       IF Mode = "PruneWithoutProtectedLatest"
       THEN 0
       ELSE ReceiptArtifactHash(latestIndexHeight)
  /\ pruneIntentHeights' = {PruneTargetHeight}
  /\ removedEvidenceHeights' = removedEvidenceHeights
  /\ startupRepairRequired' = FALSE
  /\ startupRepairCompleted' = FALSE
  /\ durableApplicationLost' = FALSE
  /\ pruneExactObjectRemoval' = pruneExactObjectRemoval
  /\ UNCHANGED publicationVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

PublishNativePruneIntent ==
  /\ pruneStage = "TempDurable"
  /\ pruneIntentTempPresent
  /\ pruneTempPublishedNoClobber
  /\ ExactPruneIntentIdentity
  /\ pruneStage' = "IntentDurable"
  /\ pruneIntentTempPresent' = FALSE
  /\ pruneIntentDurable' = TRUE
  /\ UNCHANGED
       <<pruneTempPublishedNoClobber, pruneIntentStoredVersion,
         pruneIntentRoute, pruneIntentIncarnation,
         pruneIntentManifestHash, pruneIntentReceiptHash,
         pruneIntentProtectedLatestHeight,
         pruneIntentProtectedLatestManifestHash,
         pruneIntentProtectedLatestReceiptHash,
         pruneIntentHeights, removedEvidenceHeights, startupRepairRequired,
         startupRepairCompleted, durableApplicationLost,
         pruneExactObjectRemoval>>
  /\ UNCHANGED publicationVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

RecoverPruneIntentTempAtStartup ==
  /\ pruneStage = "TempDurable"
  /\ pruneIntentTempPresent
  /\ pruneTempPublishedNoClobber
  /\ ExactPruneIntentIdentity
  /\ pruneStage' = "IntentDurable"
  /\ pruneIntentTempPresent' = FALSE
  /\ pruneIntentDurable' = TRUE
  /\ startupRepairRequired' = TRUE
  /\ startupRepairCompleted' = FALSE
  /\ durableApplicationLost' = (Mode = "DropStartupRepair")
  /\ pruneExactObjectRemoval' = pruneExactObjectRemoval
  /\ UNCHANGED
       <<pruneTempPublishedNoClobber, pruneIntentStoredVersion,
         pruneIntentRoute, pruneIntentIncarnation,
         pruneIntentManifestHash, pruneIntentReceiptHash,
         pruneIntentProtectedLatestHeight,
         pruneIntentProtectedLatestManifestHash,
         pruneIntentProtectedLatestReceiptHash,
         pruneIntentHeights, removedEvidenceHeights>>
  /\ UNCHANGED publicationVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

CrashAfterPruneIntent ==
  /\ pruneStage = "IntentDurable"
  /\ pruneIntentDurable
  /\ ~startupRepairRequired
  /\ pruneStage' = "IntentDurable"
  /\ startupRepairRequired' = TRUE
  /\ startupRepairCompleted' = FALSE
  /\ durableApplicationLost' = (Mode = "DropStartupRepair")
  /\ pruneExactObjectRemoval' = pruneExactObjectRemoval
  /\ UNCHANGED
       <<pruneIntentTempPresent, pruneIntentDurable,
         pruneTempPublishedNoClobber, pruneIntentStoredVersion,
         pruneIntentRoute, pruneIntentIncarnation,
         pruneIntentManifestHash, pruneIntentReceiptHash,
         pruneIntentProtectedLatestHeight,
         pruneIntentProtectedLatestManifestHash,
         pruneIntentProtectedLatestReceiptHash,
         pruneIntentHeights, removedEvidenceHeights>>
  /\ UNCHANGED publicationVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

UnlinkManifestBeforeCrash ==
  /\ pruneStage = "IntentDurable"
  /\ pruneIntentDurable
  /\ PruneTargetHeight \in manifestFiles
  /\ PruneTargetHeight \in receiptFiles
  /\ pruneStage' = "ManifestUnlinked"
  /\ manifestFiles' = manifestFiles \ {PruneTargetHeight}
  /\ startupRepairRequired' = TRUE
  /\ startupRepairCompleted' = FALSE
  /\ durableApplicationLost' = (Mode = "DropStartupRepair")
  /\ pruneExactObjectRemoval' = (Mode # "PruneNamespaceRebind")
  /\ UNCHANGED
       <<pruneIntentTempPresent, pruneIntentDurable,
         pruneTempPublishedNoClobber, pruneIntentStoredVersion,
         pruneIntentRoute, pruneIntentIncarnation,
         pruneIntentManifestHash, pruneIntentReceiptHash,
         pruneIntentProtectedLatestHeight,
         pruneIntentProtectedLatestManifestHash,
         pruneIntentProtectedLatestReceiptHash,
         pruneIntentHeights, removedEvidenceHeights>>
  /\ UNCHANGED
       <<phase, finalizedHeights, receiptFiles,
         manifestTempFiles, receiptTempFiles, latestIndexTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPairIdentitiesExact,
         retainedPredecessorChainExact,
         frontierPublished, frontierHeight, canonicalWireRetained,
         authenticatedProofAvailable, manifestLeafAuthenticated,
         manifestLeafExact, manifestTempPublicationExact,
         receiptTempPublicationExact, latestTempPublicationExact,
         manifestPublishedNoClobber, receiptPublishedNoClobber,
         latestIndexPublished, latestIndexHeight, latestIndexExact,
         latestIndexAmbiguous, latestIndexBounded,
         legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

UnlinkReceiptBeforeCrash ==
  /\ pruneStage = "IntentDurable"
  /\ pruneIntentDurable
  /\ PruneTargetHeight \in manifestFiles
  /\ PruneTargetHeight \in receiptFiles
  /\ pruneStage' = "ReceiptUnlinked"
  /\ receiptFiles' = receiptFiles \ {PruneTargetHeight}
  /\ startupRepairRequired' = TRUE
  /\ startupRepairCompleted' = FALSE
  /\ durableApplicationLost' = (Mode = "DropStartupRepair")
  /\ pruneExactObjectRemoval' = (Mode # "PruneNamespaceRebind")
  /\ UNCHANGED
       <<pruneIntentTempPresent, pruneIntentDurable,
         pruneTempPublishedNoClobber, pruneIntentStoredVersion,
         pruneIntentRoute, pruneIntentIncarnation,
         pruneIntentManifestHash, pruneIntentReceiptHash,
         pruneIntentProtectedLatestHeight,
         pruneIntentProtectedLatestManifestHash,
         pruneIntentProtectedLatestReceiptHash,
         pruneIntentHeights, removedEvidenceHeights>>
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestFiles,
         manifestTempFiles, receiptTempFiles, latestIndexTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, publicationPairPendingCleanup,
         retainedPairIdentitiesExact,
         retainedPredecessorChainExact,
         frontierPublished, frontierHeight, canonicalWireRetained,
         authenticatedProofAvailable, manifestLeafAuthenticated,
         manifestLeafExact, manifestTempPublicationExact,
         receiptTempPublicationExact, latestTempPublicationExact,
         manifestPublishedNoClobber, receiptPublishedNoClobber,
         latestIndexPublished, latestIndexHeight, latestIndexExact,
         latestIndexAmbiguous, latestIndexBounded,
         legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

CompletePruneWithoutCrash ==
  /\ pruneStage = "IntentDurable"
  /\ pruneIntentDurable
  /\ ~startupRepairRequired
  /\ ExactPruneIntentIdentity
  /\ pruneStage' = "Completed"
  /\ manifestFiles' = manifestFiles \ pruneIntentHeights
  /\ receiptFiles' = receiptFiles \ pruneIntentHeights
  /\ pruneIntentTempPresent' = FALSE
  /\ pruneIntentDurable' = FALSE
  /\ pruneIntentStoredVersion' = 0
  /\ pruneIntentRoute' = 0
  /\ pruneIntentIncarnation' = 0
  /\ pruneIntentManifestHash' = 0
  /\ pruneIntentReceiptHash' = 0
  /\ pruneIntentProtectedLatestHeight' = NoHeight
  /\ pruneIntentProtectedLatestManifestHash' = 0
  /\ pruneIntentProtectedLatestReceiptHash' = 0
  /\ removedEvidenceHeights' = removedEvidenceHeights \union pruneIntentHeights
  /\ pruneIntentHeights' = {}
  /\ publicationPairPendingCleanup' =
       publicationPairPendingCleanup /\ ~frontierPublished
  /\ startupRepairRequired' = FALSE
  /\ startupRepairCompleted' = TRUE
  /\ durableApplicationLost' = FALSE
  /\ pruneExactObjectRemoval' = (Mode # "PruneNamespaceRebind")
  /\ UNCHANGED pruneTempPublishedNoClobber
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestTempFiles, receiptTempFiles,
         latestIndexTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, retainedPairIdentitiesExact,
         retainedPredecessorChainExact, frontierPublished, frontierHeight,
         canonicalWireRetained, authenticatedProofAvailable,
         manifestLeafAuthenticated, manifestLeafExact,
         manifestTempPublicationExact, receiptTempPublicationExact,
         latestTempPublicationExact, manifestPublishedNoClobber,
         receiptPublishedNoClobber, latestIndexPublished,
         latestIndexHeight, latestIndexExact, latestIndexAmbiguous,
         latestIndexBounded, legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

RunStartupPruneCompletion ==
  /\ Mode # "DropStartupRepair"
  /\ pruneStage \in {"IntentDurable", "ManifestUnlinked", "ReceiptUnlinked"}
  /\ pruneIntentDurable
  /\ startupRepairRequired
  /\ ExactPruneIntentIdentity
  /\ pruneStage' = "Completed"
  /\ manifestFiles' = manifestFiles \ pruneIntentHeights
  /\ receiptFiles' = receiptFiles \ pruneIntentHeights
  /\ pruneIntentTempPresent' = FALSE
  /\ pruneIntentDurable' = FALSE
  /\ pruneIntentStoredVersion' = 0
  /\ pruneIntentRoute' = 0
  /\ pruneIntentIncarnation' = 0
  /\ pruneIntentManifestHash' = 0
  /\ pruneIntentReceiptHash' = 0
  /\ pruneIntentProtectedLatestHeight' = NoHeight
  /\ pruneIntentProtectedLatestManifestHash' = 0
  /\ pruneIntentProtectedLatestReceiptHash' = 0
  /\ removedEvidenceHeights' = removedEvidenceHeights \union pruneIntentHeights
  /\ pruneIntentHeights' = {}
  /\ publicationPairPendingCleanup' =
       publicationPairPendingCleanup /\ ~frontierPublished
  /\ startupRepairRequired' = FALSE
  /\ startupRepairCompleted' = TRUE
  /\ durableApplicationLost' = FALSE
  /\ pruneExactObjectRemoval' = (Mode # "PruneNamespaceRebind")
  /\ UNCHANGED pruneTempPublishedNoClobber
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestTempFiles, receiptTempFiles,
         latestIndexTempFiles,
         manifestTempAuthenticated, receiptTempAuthenticated,
         unauthenticatedTempPromoted, retainedPairIdentitiesExact,
         retainedPredecessorChainExact, frontierPublished, frontierHeight,
         canonicalWireRetained, authenticatedProofAvailable,
         manifestLeafAuthenticated, manifestLeafExact,
         manifestTempPublicationExact, receiptTempPublicationExact,
         latestTempPublicationExact, manifestPublishedNoClobber,
         receiptPublishedNoClobber, latestIndexPublished,
         latestIndexHeight, latestIndexExact, latestIndexAmbiguous,
         latestIndexBounded, legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

RepeatStartupPruneCompletion ==
  /\ pruneStage = "Completed"
  /\ startupRepairCompleted
  /\ ~startupRepairRequired
  /\ ~pruneIntentDurable
  /\ ~pruneIntentTempPresent
  /\ UNCHANGED vars

SettleSameRouteControl ==
  /\ ~sameRouteSettled
  /\ sameRouteSettled' = TRUE
  /\ separateParticipantMarker' = (Mode = "SeparateSameRouteMarker")
  /\ UNCHANGED publicationVars
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<claimVars, admissionVars, groupVars>>

RecordSourceSessionClaim ==
  /\ sourceClaimPhase = "Unrecorded"
  /\ sourceClaimPhase' = "Durable"
  /\ durableSourceClaimJournalRecords' = {ExactSourceClaimAuthorization}
  /\ volatileSourceClaimMap' =
       ReconstructSourceClaimMapFromJournal(
         {ExactSourceClaimAuthorization})
  /\ UNCHANGED
       <<sourceClaimReloadReconstructed, sourceClaimExactReplayAccepted,
         sourceClaimDivergentRetryAttempted,
         sourceClaimDivergentRetryAccepted,
         sourceClaimDivergentRetryRejected>>
  /\ UNCHANGED publicationVars
  /\ UNCHANGED pruneVars
  /\ UNCHANGED startupRepairVars
  /\ UNCHANGED <<sameRouteVars, admissionVars, groupVars>>

CrashAfterSourceClaimDurable ==
  /\ sourceClaimPhase = "Durable"
  /\ sourceClaimPhase' = "Crashed"
  /\ volatileSourceClaimMap' = EmptySourceClaimMap
  /\ UNCHANGED
       <<durableSourceClaimJournalRecords,
         sourceClaimReloadReconstructed, sourceClaimExactReplayAccepted,
         sourceClaimDivergentRetryAttempted,
         sourceClaimDivergentRetryAccepted,
         sourceClaimDivergentRetryRejected>>
  /\ UNCHANGED publicationVars
  /\ UNCHANGED pruneVars
  /\ UNCHANGED startupRepairVars
  /\ UNCHANGED <<sameRouteVars, admissionVars, groupVars>>

ReloadDurableSourceClaim ==
  /\ sourceClaimPhase = "Crashed"
  /\ durableSourceClaimJournalRecords = {ExactSourceClaimAuthorization}
  /\ sourceClaimPhase' = "Reloaded"
  /\ volatileSourceClaimMap' =
       ReconstructSourceClaimMapFromJournal(
         durableSourceClaimJournalRecords)
  /\ sourceClaimReloadReconstructed' = TRUE
  /\ UNCHANGED
       <<durableSourceClaimJournalRecords,
         sourceClaimExactReplayAccepted,
         sourceClaimDivergentRetryAttempted,
         sourceClaimDivergentRetryAccepted,
         sourceClaimDivergentRetryRejected>>
  /\ UNCHANGED publicationVars
  /\ UNCHANGED pruneVars
  /\ UNCHANGED startupRepairVars
  /\ UNCHANGED <<sameRouteVars, admissionVars, groupVars>>

ReplayExactSourceClaim ==
  /\ sourceClaimPhase = "Reloaded"
  /\ SourceClaimGuardAccepts(
       volatileSourceClaimMap,
       ExactSourceSessionClaim,
       ExactParticipantMember,
       ExactSourceParticipantClaim)
  /\ sourceClaimPhase' = "ExactReplayAccepted"
  /\ sourceClaimExactReplayAccepted' = TRUE
  /\ UNCHANGED
       <<volatileSourceClaimMap,
         durableSourceClaimJournalRecords,
         sourceClaimReloadReconstructed,
         sourceClaimDivergentRetryAttempted,
         sourceClaimDivergentRetryAccepted,
         sourceClaimDivergentRetryRejected>>
  /\ UNCHANGED publicationVars
  /\ UNCHANGED pruneVars
  /\ UNCHANGED startupRepairVars
  /\ UNCHANGED <<sameRouteVars, admissionVars, groupVars>>

DivergentSourceClaimRetryAccepted ==
  SourceClaimGuardAccepts(
    volatileSourceClaimMap,
    RetrySourceSessionClaim,
    RetryParticipantMember,
    RetrySourceParticipantClaim)

RetryDivergentSourceClaim ==
  /\ sourceClaimPhase = "ExactReplayAccepted"
  /\ sourceClaimPhase' = "RetryChecked"
  /\ durableSourceClaimJournalRecords' =
       IF DivergentSourceClaimRetryAccepted
       THEN
         durableSourceClaimJournalRecords
           \union {RetrySourceClaimAuthorization}
       ELSE durableSourceClaimJournalRecords
  /\ sourceClaimDivergentRetryAttempted' = TRUE
  /\ sourceClaimDivergentRetryAccepted' =
       DivergentSourceClaimRetryAccepted
  /\ sourceClaimDivergentRetryRejected' =
       ~DivergentSourceClaimRetryAccepted
  /\ UNCHANGED
       <<volatileSourceClaimMap,
         sourceClaimReloadReconstructed, sourceClaimExactReplayAccepted>>
  /\ UNCHANGED publicationVars
  /\ UNCHANGED pruneVars
  /\ UNCHANGED startupRepairVars
  /\ UNCHANGED <<sameRouteVars, admissionVars, groupVars>>

AdmitNativeControl ==
  /\ ~nativeAdmissionAttempted
  /\ nativeAdmissionAttempted' = TRUE
  /\ activeIncarnationExact' = (Mode # "NonContiguousRoute")
  /\ predecessorExact' = (Mode # "NonContiguousRoute")
  /\ contiguousNextHeight' = (Mode # "NonContiguousRoute")
  /\ UNCHANGED publicationVars
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, groupVars>>

ApplyNativeGroup ==
  /\ ~groupApplied
  /\ groupApplied' = TRUE
  /\ groupUnique' = (Mode # "PartialGroupApplication")
  /\ groupOrdered' = TRUE
  /\ groupExactCover' = (Mode # "PartialGroupApplication")
  /\ groupAppliedAtomically' = (Mode # "PartialGroupApplication")
  /\ UNCHANGED publicationVars
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars>>

\* Startup repair first inventories the ordinary receipt, Native marker, and
\* reverse merge-carrier groups. In the fixed mode this step is observational:
\* it does not publish a group, alter the Queue, or install a carrier record.
\* The groups name the same missing canonical global carrier, so the generic
\* recovery owner emits one coalesced body need.
PlanUnifiedStartupEvidenceRepair ==
  /\ startupRepairStage = "Unplanned"
  /\ startupRepairStage' =
       IF Mode \in {"MissingReverseMergeCarrier", "OrphanMergeCarrier"}
       THEN "PlanReady"
       ELSE "NeedBodies"
  /\ plannedEvidenceRepairGroups' = UnifiedEvidenceRepairGroups
  /\ startupRepairPlanReadOnly' = (Mode # "MutatingUnifiedStartupPlan")
  /\ canonicalBodyNeedCount' =
       IF Mode = "UncoalescedCanonicalBodyNeeds" THEN 2 ELSE 1
  /\ planRevalidatedAfterRecovery' =
       Mode \in {"MissingReverseMergeCarrier", "OrphanMergeCarrier"}
  /\ appliedEvidenceRepairGroups' =
       IF Mode = "MutatingUnifiedStartupPlan"
       THEN {OrdinaryReceiptRepairGroup}
       ELSE appliedEvidenceRepairGroups
  /\ authenticatedCarrierManifestRoutes' =
       QcAuthenticatedCarrierManifestRoutes
  /\ plannedNativeMarkerRepairRoutes' =
       IF Mode = "RepairHistoricalSiblingAsActive"
       THEN QcAuthenticatedCarrierManifestRoutes
       ELSE ExactActiveStateMarkerRepairRoutes
  /\ UNCHANGED
       <<canonicalBodiesRecovered, recoveredCanonicalBodyGroups,
         preflightedEvidenceRepairGroups, evidenceRepairReadBackVerified,
         queueGateOpen, queueReservationReconciled,
         finalityDeclaresMergeCarrier, mergeCarrierRecordPresent,
         mergeCarrierRecordExact, mergeCarrierRepairPlanned,
         bodyCachePopulated, postCacheCarrierPreflighted,
         preflightedNativeMarkerRepairRoutes,
         appliedNativeMarkerRepairRoutes>>
  /\ UNCHANGED
       <<publicationVars, pruneVars, sameRouteVars, sourceClaimVars,
         admissionVars, groupVars>>

\* One authenticated canonical body satisfies the complete plan, including
\* both the ordinary and Native evidence groups.
\* The signed complete-wire length and hash are abstracted by this single
\* successful recovery transition; transport authentication is modeled in
\* SumeragiV2AutonomousReservationCarrier.
RecoverSharedCanonicalBody ==
  /\ startupRepairStage = "NeedBodies"
  /\ canonicalBodyNeedCount > 0
  /\ startupRepairStage' = "BodiesRecovered"
  /\ canonicalBodiesRecovered' = 1
  /\ recoveredCanonicalBodyGroups' = UnifiedEvidenceRepairGroups
  /\ bodyCachePopulated' = TRUE
  /\ UNCHANGED
       <<plannedEvidenceRepairGroups, startupRepairPlanReadOnly,
         canonicalBodyNeedCount, planRevalidatedAfterRecovery,
         preflightedEvidenceRepairGroups, appliedEvidenceRepairGroups,
         evidenceRepairReadBackVerified, queueGateOpen,
         queueReservationReconciled, finalityDeclaresMergeCarrier,
         mergeCarrierRecordPresent, mergeCarrierRecordExact,
         mergeCarrierRepairPlanned, postCacheCarrierPreflighted,
         startupRepairRouteVars>>
  /\ UNCHANGED
       <<publicationVars, pruneVars, sameRouteVars, sourceClaimVars,
         admissionVars, groupVars>>

\* Cache installation is followed by the reverse carrier pass. The pass
\* preflights the exact carrier reconstruction named by finality before a
\* recovered body need may retire; publication remains owned by the subsequent
\* all-item application plan.
ReconcilePostCacheMergeCarrier ==
  /\ startupRepairStage = "BodiesRecovered"
  /\ bodyCachePopulated
  /\ Mode # "SkipPostCacheCarrierReconcile"
  /\ finalityDeclaresMergeCarrier
  /\ startupRepairStage' = "CarrierPreflighted"
  /\ postCacheCarrierPreflighted' = TRUE
  /\ UNCHANGED
       <<plannedEvidenceRepairGroups, startupRepairPlanReadOnly,
         canonicalBodyNeedCount, canonicalBodiesRecovered,
         recoveredCanonicalBodyGroups, planRevalidatedAfterRecovery,
         preflightedEvidenceRepairGroups, appliedEvidenceRepairGroups,
         evidenceRepairReadBackVerified, queueGateOpen,
         queueReservationReconciled, finalityDeclaresMergeCarrier,
         mergeCarrierRecordPresent, mergeCarrierRecordExact,
         mergeCarrierRepairPlanned, bodyCachePopulated,
         startupRepairRouteVars>>
  /\ UNCHANGED
       <<publicationVars, pruneVars, sameRouteVars, sourceClaimVars,
         admissionVars, groupVars>>

\* Planning is repeated after body recovery and carrier reconstruction
\* preflight. The SkipPostCache mutation deliberately permits the old one-way
\* behavior.
ReplanUnifiedStartupEvidenceRepair ==
  /\ \/ startupRepairStage = "CarrierPreflighted"
     \/ /\ Mode = "SkipPostCacheCarrierReconcile"
        /\ startupRepairStage = "BodiesRecovered"
  /\ startupRepairStage' = "PlanReady"
  /\ planRevalidatedAfterRecovery' = TRUE
  /\ mergeCarrierRepairPlanned' =
       (finalityDeclaresMergeCarrier /\ ~mergeCarrierRecordPresent)
  /\ UNCHANGED
       <<plannedEvidenceRepairGroups, startupRepairPlanReadOnly,
         canonicalBodyNeedCount, canonicalBodiesRecovered,
         recoveredCanonicalBodyGroups, preflightedEvidenceRepairGroups,
         appliedEvidenceRepairGroups, evidenceRepairReadBackVerified,
         queueGateOpen, queueReservationReconciled,
         finalityDeclaresMergeCarrier, mergeCarrierRecordPresent,
         mergeCarrierRecordExact, bodyCachePopulated,
         postCacheCarrierPreflighted, startupRepairRouteVars>>
  /\ UNCHANGED
       <<publicationVars, pruneVars, sameRouteVars, sourceClaimVars,
         admissionVars, groupVars>>

\* Every ordinary and Native group is preflighted before any durable
\* publication. A partial prefix is never an application authorization.
PreflightUnifiedStartupEvidenceGroups ==
  /\ startupRepairStage = "PlanReady"
  /\ startupRepairStage' = "GroupsPreflight"
  /\ preflightedEvidenceRepairGroups' =
       IF Mode = "PartialUnifiedStartupPreflight"
       THEN {OrdinaryReceiptRepairGroup}
       ELSE UnifiedEvidenceRepairGroups
  /\ preflightedNativeMarkerRepairRoutes' =
       IF Mode = "PartialUnifiedStartupPreflight"
       THEN {}
       ELSE plannedNativeMarkerRepairRoutes
  /\ UNCHANGED
       <<plannedEvidenceRepairGroups, startupRepairPlanReadOnly,
         canonicalBodyNeedCount, canonicalBodiesRecovered,
         recoveredCanonicalBodyGroups, planRevalidatedAfterRecovery,
         appliedEvidenceRepairGroups, evidenceRepairReadBackVerified,
         queueGateOpen, queueReservationReconciled,
         finalityDeclaresMergeCarrier, mergeCarrierRecordPresent,
         mergeCarrierRecordExact, mergeCarrierRepairPlanned,
         bodyCachePopulated, postCacheCarrierPreflighted,
         authenticatedCarrierManifestRoutes,
         plannedNativeMarkerRepairRoutes,
         appliedNativeMarkerRepairRoutes>>
  /\ UNCHANGED
       <<publicationVars, pruneVars, sameRouteVars, sourceClaimVars,
         admissionVars, groupVars>>

ApplyUnifiedStartupEvidenceGroups ==
  /\ startupRepairStage = "GroupsPreflight"
  /\ \/ preflightedEvidenceRepairGroups = UnifiedEvidenceRepairGroups
     \/ Mode = "PartialUnifiedStartupPreflight"
  /\ startupRepairStage' = "EvidenceApplied"
  /\ appliedEvidenceRepairGroups' = preflightedEvidenceRepairGroups
  /\ appliedNativeMarkerRepairRoutes' =
       preflightedNativeMarkerRepairRoutes
  /\ mergeCarrierRecordPresent' =
       IF mergeCarrierRepairPlanned THEN TRUE ELSE mergeCarrierRecordPresent
  /\ mergeCarrierRecordExact' =
       IF mergeCarrierRepairPlanned THEN TRUE ELSE mergeCarrierRecordExact
  /\ mergeCarrierRepairPlanned' = FALSE
  /\ UNCHANGED
       <<plannedEvidenceRepairGroups, startupRepairPlanReadOnly,
         canonicalBodyNeedCount, canonicalBodiesRecovered,
         recoveredCanonicalBodyGroups, planRevalidatedAfterRecovery,
         preflightedEvidenceRepairGroups, evidenceRepairReadBackVerified,
         queueGateOpen, queueReservationReconciled,
         finalityDeclaresMergeCarrier, bodyCachePopulated,
         postCacheCarrierPreflighted,
         authenticatedCarrierManifestRoutes,
         plannedNativeMarkerRepairRoutes,
         preflightedNativeMarkerRepairRoutes>>
  /\ UNCHANGED
       <<publicationVars, pruneVars, sameRouteVars, sourceClaimVars,
         admissionVars, groupVars>>

ReadBackUnifiedStartupEvidence ==
  /\ startupRepairStage = "EvidenceApplied"
  /\ startupRepairStage' = "ReadBackVerified"
  /\ evidenceRepairReadBackVerified' =
       (appliedEvidenceRepairGroups = UnifiedEvidenceRepairGroups)
  /\ UNCHANGED
       <<plannedEvidenceRepairGroups, startupRepairPlanReadOnly,
         canonicalBodyNeedCount, canonicalBodiesRecovered,
         recoveredCanonicalBodyGroups, planRevalidatedAfterRecovery,
         preflightedEvidenceRepairGroups, appliedEvidenceRepairGroups,
         queueGateOpen, queueReservationReconciled,
         finalityDeclaresMergeCarrier, mergeCarrierRecordPresent,
         mergeCarrierRecordExact, mergeCarrierRepairPlanned,
         bodyCachePopulated, postCacheCarrierPreflighted,
         startupRepairRouteVars>>
  /\ UNCHANGED
       <<publicationVars, pruneVars, sameRouteVars, sourceClaimVars,
         admissionVars, groupVars>>

\* Only after exact readback may reservation reconciliation finish and expose
\* ordinary Queue selection. The mutation reopens directly after application.
ReconcileQueueAfterUnifiedStartupEvidence ==
  /\ \/ /\ startupRepairStage = "ReadBackVerified"
        /\ evidenceRepairReadBackVerified
     \/ /\ Mode = "QueueBeforeEvidenceReadback"
        /\ startupRepairStage = "EvidenceApplied"
  /\ startupRepairStage' = "QueueReconciled"
  /\ queueGateOpen' = TRUE
  /\ queueReservationReconciled' = TRUE
  /\ UNCHANGED
       <<plannedEvidenceRepairGroups, startupRepairPlanReadOnly,
         canonicalBodyNeedCount, canonicalBodiesRecovered,
         recoveredCanonicalBodyGroups, planRevalidatedAfterRecovery,
         preflightedEvidenceRepairGroups, appliedEvidenceRepairGroups,
         evidenceRepairReadBackVerified, finalityDeclaresMergeCarrier,
         mergeCarrierRecordPresent, mergeCarrierRecordExact,
         mergeCarrierRepairPlanned, bodyCachePopulated,
         postCacheCarrierPreflighted, startupRepairRouteVars>>
  /\ UNCHANGED
       <<publicationVars, pruneVars, sameRouteVars, sourceClaimVars,
         admissionVars, groupVars>>

Next ==
  \/ PersistFinality
  \/ StageStandaloneManifestTemp
  \/ PersistStandaloneManifest
  \/ StageStandaloneReceiptTemp
  \/ PersistStandaloneReceipt
  \/ StageDescriptorBoundLatestTemp
  \/ PublishDescriptorBoundLatest
  \/ CrashAfterDescriptorBoundLatestTemp
  \/ RecoverDescriptorBoundLatestTempAtStartup
  \/ DiscardAuthenticatedLatestTempAtStartup
  \/ PublishReplicatedFrontier
  \/ PruneCanonicalWire
  \/ StageSecondIncomingPair
  \/ PunctureRetainedHistory
  \/ CreateNonHighestRepairHalf
  \/ CreateMultipleRepairHalves
  \/ CorruptRetainedPairIdentity
  \/ DriftRetainedPredecessor
  \/ StageNativePruneIntentTemp
  \/ PublishNativePruneIntent
  \/ RecoverPruneIntentTempAtStartup
  \/ CrashAfterPruneIntent
  \/ UnlinkManifestBeforeCrash
  \/ UnlinkReceiptBeforeCrash
  \/ CompletePruneWithoutCrash
  \/ RunStartupPruneCompletion
  \/ RepeatStartupPruneCompletion
  \/ SettleSameRouteControl
  \/ RecordSourceSessionClaim
  \/ CrashAfterSourceClaimDurable
  \/ ReloadDurableSourceClaim
  \/ ReplayExactSourceClaim
  \/ RetryDivergentSourceClaim
  \/ AdmitNativeControl
  \/ ApplyNativeGroup
  \/ PlanUnifiedStartupEvidenceRepair
  \/ RecoverSharedCanonicalBody
  \/ ReconcilePostCacheMergeCarrier
  \/ ReplanUnifiedStartupEvidenceRepair
  \/ PreflightUnifiedStartupEvidenceGroups
  \/ ApplyUnifiedStartupEvidenceGroups
  \/ ReadBackUnifiedStartupEvidence
  \/ ReconcileQueueAfterUnifiedStartupEvidence

NativeEvidenceTypeInvariant ==
  /\ NativeEvidenceConfiguration
  /\ phase \in NativeEvidencePhases
  /\ finalizedHeights \subseteq EvidenceHeights
  /\ manifestFiles \subseteq EvidenceHeights
  /\ receiptFiles \subseteq EvidenceHeights
  /\ manifestTempFiles \subseteq EvidenceHeights
  /\ receiptTempFiles \subseteq EvidenceHeights
  /\ latestIndexTempFiles \subseteq EvidenceHeights
  /\ manifestTempAuthenticated \in BOOLEAN
  /\ receiptTempAuthenticated \in BOOLEAN
  /\ unauthenticatedTempPromoted \in BOOLEAN
  /\ publicationPairPendingCleanup \in BOOLEAN
  /\ retainedPairIdentitiesExact \in BOOLEAN
  /\ retainedPredecessorChainExact \in BOOLEAN
  /\ frontierPublished \in BOOLEAN
  /\ frontierHeight \in 0..TargetHeight
  /\ canonicalWireRetained \in BOOLEAN
  /\ authenticatedProofAvailable \in BOOLEAN
  /\ manifestLeafAuthenticated \in BOOLEAN
  /\ manifestLeafExact \in BOOLEAN
  /\ manifestTempPublicationExact \in BOOLEAN
  /\ receiptTempPublicationExact \in BOOLEAN
  /\ latestTempPublicationExact \in BOOLEAN
  /\ manifestPublishedNoClobber \in BOOLEAN
  /\ receiptPublishedNoClobber \in BOOLEAN
  /\ latestIndexPublished \in BOOLEAN
  /\ latestIndexHeight \in EvidenceHeights
  /\ latestIndexExact \in BOOLEAN
  /\ latestIndexAmbiguous \in BOOLEAN
  /\ latestIndexBounded \in BOOLEAN
  /\ legacyDenseRejected \in BOOLEAN
  /\ legacyDenseAccepted \in BOOLEAN
  /\ pruneStage \in NativePruneStages
  /\ pruneIntentTempPresent \in BOOLEAN
  /\ pruneIntentDurable \in BOOLEAN
  /\ pruneTempPublishedNoClobber \in BOOLEAN
  /\ pruneIntentStoredVersion \in 0..PruneIntentVersion
  /\ pruneIntentRoute \in 0..ActiveRoute
  /\ pruneIntentIncarnation \in 0..ActiveIncarnation
  /\ pruneIntentManifestHash \in 0..ManifestArtifactHash(TargetHeight)
  /\ pruneIntentReceiptHash \in 0..ReceiptArtifactHash(TargetHeight)
  /\ pruneIntentProtectedLatestHeight \in NoHeight..ExtraIncomingHeight
  /\ pruneIntentProtectedLatestManifestHash
       \in 0..ManifestArtifactHash(ExtraIncomingHeight)
  /\ pruneIntentProtectedLatestReceiptHash
       \in 0..ReceiptArtifactHash(ExtraIncomingHeight)
  /\ pruneIntentHeights \subseteq EvidenceHeights
  /\ removedEvidenceHeights \subseteq EvidenceHeights
  /\ startupRepairRequired \in BOOLEAN
  /\ startupRepairCompleted \in BOOLEAN
  /\ durableApplicationLost \in BOOLEAN
  /\ pruneExactObjectRemoval \in BOOLEAN
  /\ sameRouteSettled \in BOOLEAN
  /\ separateParticipantMarker \in BOOLEAN
  /\ sourceClaimPhase \in NativeSourceClaimPhases
  /\ volatileSourceClaimMap \in {EmptySourceClaimMap, ExactSourceClaimMap}
  /\ durableSourceClaimJournalRecords
       \subseteq
         {ExactSourceClaimAuthorization, RetrySourceClaimAuthorization}
  /\ sourceClaimReloadReconstructed \in BOOLEAN
  /\ sourceClaimExactReplayAccepted \in BOOLEAN
  /\ sourceClaimDivergentRetryAttempted \in BOOLEAN
  /\ sourceClaimDivergentRetryAccepted \in BOOLEAN
  /\ sourceClaimDivergentRetryRejected \in BOOLEAN
  /\ nativeAdmissionAttempted \in BOOLEAN
  /\ activeIncarnationExact \in BOOLEAN
  /\ predecessorExact \in BOOLEAN
  /\ contiguousNextHeight \in BOOLEAN
  /\ groupApplied \in BOOLEAN
  /\ groupUnique \in BOOLEAN
  /\ groupOrdered \in BOOLEAN
  /\ groupExactCover \in BOOLEAN
  /\ groupAppliedAtomically \in BOOLEAN
  /\ startupRepairStage \in UnifiedStartupRepairStages
  /\ plannedEvidenceRepairGroups \subseteq UnifiedEvidenceRepairGroups
  /\ startupRepairPlanReadOnly \in BOOLEAN
  /\ canonicalBodyNeedCount \in 0..UnifiedEvidenceRepairGroupCount
  /\ canonicalBodiesRecovered \in 0..1
  /\ recoveredCanonicalBodyGroups \subseteq UnifiedEvidenceRepairGroups
  /\ planRevalidatedAfterRecovery \in BOOLEAN
  /\ preflightedEvidenceRepairGroups \subseteq UnifiedEvidenceRepairGroups
  /\ appliedEvidenceRepairGroups \subseteq UnifiedEvidenceRepairGroups
  /\ evidenceRepairReadBackVerified \in BOOLEAN
  /\ queueGateOpen \in BOOLEAN
  /\ queueReservationReconciled \in BOOLEAN
  /\ finalityDeclaresMergeCarrier \in BOOLEAN
  /\ mergeCarrierRecordPresent \in BOOLEAN
  /\ mergeCarrierRecordExact \in BOOLEAN
  /\ mergeCarrierRepairPlanned \in BOOLEAN
  /\ bodyCachePopulated \in BOOLEAN
  /\ postCacheCarrierPreflighted \in BOOLEAN
  /\ authenticatedCarrierManifestRoutes
       \subseteq QcAuthenticatedCarrierManifestRoutes
  /\ plannedNativeMarkerRepairRoutes
       \subseteq QcAuthenticatedCarrierManifestRoutes
  /\ preflightedNativeMarkerRepairRoutes
       \subseteq QcAuthenticatedCarrierManifestRoutes
  /\ appliedNativeMarkerRepairRoutes
       \subseteq QcAuthenticatedCarrierManifestRoutes

NativeStandaloneEvidenceInvariant ==
  /\ manifestFiles \subseteq finalizedHeights
  /\ receiptFiles \subseteq finalizedHeights
  /\ (manifestFiles \ receiptFiles)
       \subseteq ({TargetHeight} \union pruneIntentHeights)
  /\ (receiptFiles \ manifestFiles) \subseteq pruneIntentHeights
  /\ latestIndexHeight \in receiptFiles

NativeEvidenceRetentionBoundInvariant ==
  /\ TotalEvidencePayloadBytes <= PublicationTransientByteLimit
  /\ Cardinality(IncomingEvidenceHeights) <= 1
  /\ Cardinality(manifestTempFiles) <= 1
  /\ Cardinality(receiptTempFiles) <= 1
  /\ Cardinality(latestIndexTempFiles) <= 1
  /\ (frontierPublished =>
       RetainedStableEvidencePayloadBytes <= StableAggregateByteLimit)

MLNativeSharedEvidenceBudget ==
  /\ RetainedStableEvidencePayloadBytes <= StableAggregateByteLimit
  /\ (~publicationPairPendingCleanup =>
       StableEvidencePayloadBytes <= StableAggregateByteLimit)

MLNativeSingleIncomingPairHeadroom ==
  /\ TotalEvidencePayloadBytes <= PublicationTransientByteLimit
  /\ Cardinality(IncomingEvidenceHeights) <= 1

MLNativeTempPromotionAuthenticated ==
  /\ ~unauthenticatedTempPromoted
  /\ manifestTempFiles \cap manifestFiles = {}
  /\ receiptTempFiles \cap receiptFiles = {}

MLNativeRetainedHistoryExact ==
  /\ ContiguousHeightInterval(EffectiveRetainedHeights)
  /\ HighestHalfRepairOnly
  /\ retainedPairIdentitiesExact
  /\ retainedPredecessorChainExact

MLNativePruneOldestPrefix ==
  /\ OldestPrefix(removedEvidenceHeights)
  /\ ((pruneIntentTempPresent \/ pruneIntentDurable) =>
       /\ removedEvidenceHeights \cap pruneIntentHeights = {}
       /\ OldestPrefix(removedEvidenceHeights \union pruneIntentHeights))

NativeNoClobberPublicationInvariant ==
  /\ manifestTempPublicationExact
  /\ receiptTempPublicationExact
  /\ latestTempPublicationExact
  /\ manifestPublishedNoClobber
  /\ receiptPublishedNoClobber
  /\ pruneTempPublishedNoClobber

NativeLegacyDenseRejectedInvariant ==
  /\ legacyDenseRejected
  /\ ~legacyDenseAccepted

MLNativePruneProtectedLatestExact ==
  (pruneIntentTempPresent \/ pruneIntentDurable) =>
    PruneIntentProtectedLatestExact

MLNativePruneExactObjectRemoval ==
  pruneExactObjectRemoval

NativePruneJournalInvariant ==
  /\ ((pruneIntentTempPresent \/ pruneIntentDurable) =>
       ExactPruneIntentIdentity)
  /\ (pruneIntentDurable => ~pruneIntentTempPresent)
  /\ (pruneIntentTempPresent => ~pruneIntentDurable)
  /\ latestIndexHeight \notin pruneIntentHeights
  /\ CASE pruneStage = "Idle" ->
            /\ ~pruneIntentTempPresent
            /\ ~pruneIntentDurable
            /\ ResetPruneIntentIdentity
            /\ ~startupRepairRequired
            /\ ~startupRepairCompleted
            /\ ~durableApplicationLost
       [] pruneStage = "TempDurable" ->
            /\ pruneIntentTempPresent
            /\ ~pruneIntentDurable
            /\ ExactPruneIntentIdentity
            /\ PruneTargetHeight \in manifestFiles
            /\ PruneTargetHeight \in receiptFiles
            /\ ~startupRepairRequired
            /\ ~startupRepairCompleted
            /\ ~durableApplicationLost
       [] pruneStage = "IntentDurable" ->
            /\ ~pruneIntentTempPresent
            /\ pruneIntentDurable
            /\ ExactPruneIntentIdentity
            /\ PruneTargetHeight \in manifestFiles
            /\ PruneTargetHeight \in receiptFiles
            /\ ~startupRepairCompleted
            /\ ~durableApplicationLost
       [] pruneStage = "ManifestUnlinked" ->
            /\ ~pruneIntentTempPresent
            /\ pruneIntentDurable
            /\ ExactPruneIntentIdentity
            /\ PruneTargetHeight \notin manifestFiles
            /\ PruneTargetHeight \in receiptFiles
            /\ startupRepairRequired
            /\ ~startupRepairCompleted
            /\ ~durableApplicationLost
       [] pruneStage = "ReceiptUnlinked" ->
            /\ ~pruneIntentTempPresent
            /\ pruneIntentDurable
            /\ ExactPruneIntentIdentity
            /\ PruneTargetHeight \in manifestFiles
            /\ PruneTargetHeight \notin receiptFiles
            /\ startupRepairRequired
            /\ ~startupRepairCompleted
            /\ ~durableApplicationLost
       [] pruneStage = "Completed" ->
            /\ ~pruneIntentTempPresent
            /\ ~pruneIntentDurable
            /\ ResetPruneIntentIdentity
            /\ PruneTargetHeight \notin manifestFiles
            /\ PruneTargetHeight \notin receiptFiles
            /\ ~startupRepairRequired
            /\ startupRepairCompleted
            /\ ~durableApplicationLost

SidecarsRequireManifestInvariant ==
  receiptDurable => finalityDurable /\ manifestDurable

FrontierPublicationInvariant ==
  frontierPublished =>
    /\ frontierHeight = TargetHeight
    /\ finalityDurable
    /\ manifestDurable
    /\ receiptDurable
    /\ latestIndexPublished
    /\ latestIndexHeight = TargetHeight
    /\ latestIndexHeight \in receiptFiles

PrunedEvidenceVerifiableInvariant ==
  ~canonicalWireRetained =>
    /\ finalityDurable
    /\ manifestDurable
    /\ authenticatedProofAvailable

SameRouteControlOnlyInvariant ==
  sameRouteSettled => ~separateParticipantMarker

MLSeparateParticipantApplication == SameRouteControlOnlyInvariant

MLNativeSourceClaimJournalReconstructionExact ==
  /\ ReconstructSourceClaimMapFromJournal({}) = EmptySourceClaimMap
  /\ LET authorization == ExactSourceClaimAuthorization
         reconstructed ==
           ReconstructSourceClaimMapFromJournal({authorization})
     IN /\ DOMAIN reconstructed = {authorization.source_key}
        /\ reconstructed[authorization.source_key].session =
             authorization.session
        /\ DOMAIN reconstructed[authorization.source_key].participants =
             {authorization.participant_member}
        /\ reconstructed[authorization.source_key].participants[
             authorization.participant_member] = authorization.participant
        /\ reconstructed = ExactSourceClaimMap

MLNativeSourceClaimInjective ==
  /\ Cardinality(durableSourceClaimJournalRecords) <= 1
  /\ MLNativeSourceClaimJournalReconstructionExact
  /\ CASE sourceClaimPhase = "Unrecorded" ->
            /\ volatileSourceClaimMap = EmptySourceClaimMap
            /\ durableSourceClaimJournalRecords = {}
            /\ ~sourceClaimReloadReconstructed
            /\ ~sourceClaimExactReplayAccepted
            /\ ~sourceClaimDivergentRetryAttempted
            /\ ~sourceClaimDivergentRetryAccepted
            /\ ~sourceClaimDivergentRetryRejected
       [] sourceClaimPhase = "Durable" ->
            /\ volatileSourceClaimMap =
                 ReconstructSourceClaimMapFromJournal(
                   durableSourceClaimJournalRecords)
            /\ durableSourceClaimJournalRecords =
                 {ExactSourceClaimAuthorization}
            /\ ~sourceClaimReloadReconstructed
            /\ ~sourceClaimExactReplayAccepted
            /\ ~sourceClaimDivergentRetryAttempted
            /\ ~sourceClaimDivergentRetryAccepted
            /\ ~sourceClaimDivergentRetryRejected
       [] sourceClaimPhase = "Crashed" ->
            /\ volatileSourceClaimMap = EmptySourceClaimMap
            /\ durableSourceClaimJournalRecords =
                 {ExactSourceClaimAuthorization}
            /\ ~sourceClaimReloadReconstructed
            /\ ~sourceClaimExactReplayAccepted
            /\ ~sourceClaimDivergentRetryAttempted
            /\ ~sourceClaimDivergentRetryAccepted
            /\ ~sourceClaimDivergentRetryRejected
       [] sourceClaimPhase = "Reloaded" ->
            /\ volatileSourceClaimMap =
                 ReconstructSourceClaimMapFromJournal(
                   durableSourceClaimJournalRecords)
            /\ durableSourceClaimJournalRecords =
                 {ExactSourceClaimAuthorization}
            /\ sourceClaimReloadReconstructed
            /\ ~sourceClaimExactReplayAccepted
            /\ ~sourceClaimDivergentRetryAttempted
            /\ ~sourceClaimDivergentRetryAccepted
            /\ ~sourceClaimDivergentRetryRejected
       [] sourceClaimPhase = "ExactReplayAccepted" ->
            /\ volatileSourceClaimMap =
                 ReconstructSourceClaimMapFromJournal(
                   durableSourceClaimJournalRecords)
            /\ durableSourceClaimJournalRecords =
                 {ExactSourceClaimAuthorization}
            /\ sourceClaimReloadReconstructed
            /\ sourceClaimExactReplayAccepted
            /\ ~sourceClaimDivergentRetryAttempted
            /\ ~sourceClaimDivergentRetryAccepted
            /\ ~sourceClaimDivergentRetryRejected
       [] sourceClaimPhase = "RetryChecked" ->
            /\ volatileSourceClaimMap = ExactSourceClaimMap
            /\ durableSourceClaimJournalRecords =
                 {ExactSourceClaimAuthorization}
            /\ sourceClaimReloadReconstructed
            /\ sourceClaimExactReplayAccepted
            /\ sourceClaimDivergentRetryAttempted
            /\ ~sourceClaimDivergentRetryAccepted
            /\ sourceClaimDivergentRetryRejected

MLNativeContiguousActiveRoute ==
  nativeAdmissionAttempted =>
    /\ activeIncarnationExact
    /\ predecessorExact
    /\ contiguousNextHeight

MLNativeGroupExactCover ==
  groupApplied =>
    /\ SourceCount \in 1..4096
    /\ groupUnique
    /\ groupOrdered
    /\ groupExactCover
    /\ groupAppliedAtomically

MLNativeManifestAuthenticates ==
  frontierPublished =>
    /\ manifestDurable
    /\ manifestLeafAuthenticated
    /\ manifestLeafExact
    /\ authenticatedProofAvailable

\* QC authentication covers the carrier's complete manifest, including a
\* historical sibling. State-marker repair is a narrower operation: its plan,
\* preflight, and application may name only the exact currently active marker
\* route. Authentication therefore cannot promote a retired incarnation into
\* the current repair target set.
MLNativeStartupRepairTargetsExactActiveMarkerRoutes ==
  /\ (startupRepairStage = "Unplanned" =>
       /\ authenticatedCarrierManifestRoutes = {}
       /\ plannedNativeMarkerRepairRoutes = {}
       /\ preflightedNativeMarkerRepairRoutes = {}
       /\ appliedNativeMarkerRepairRoutes = {})
  /\ (startupRepairStage # "Unplanned" =>
       /\ authenticatedCarrierManifestRoutes =
            QcAuthenticatedCarrierManifestRoutes
       /\ plannedNativeMarkerRepairRoutes =
            ExactActiveStateMarkerRepairRoutes)
  /\ (NativeMarkerRepairGroup \in preflightedEvidenceRepairGroups =>
       preflightedNativeMarkerRepairRoutes =
         ExactActiveStateMarkerRepairRoutes)
  /\ (NativeMarkerRepairGroup \notin preflightedEvidenceRepairGroups =>
       preflightedNativeMarkerRepairRoutes = {})
  /\ (NativeMarkerRepairGroup \in appliedEvidenceRepairGroups =>
       appliedNativeMarkerRepairRoutes =
         ExactActiveStateMarkerRepairRoutes)
  /\ (NativeMarkerRepairGroup \notin appliedEvidenceRepairGroups =>
       appliedNativeMarkerRepairRoutes = {})
  /\ HistoricalSiblingManifestRouteIdentity
       \notin plannedNativeMarkerRepairRoutes
  /\ HistoricalSiblingManifestRouteIdentity
       \notin preflightedNativeMarkerRepairRoutes
  /\ HistoricalSiblingManifestRouteIdentity
       \notin appliedNativeMarkerRepairRoutes

\* Planning spans both ordinary receipt and Native marker groups and remains
\* observational. Because both groups name the same carrier, exactly one
\* generic canonical-body need and one recovered body serve the whole plan.
MLUnifiedStartupPlanningReadOnly ==
  /\ startupRepairPlanReadOnly
  /\ (startupRepairStage # "Unplanned" =>
       plannedEvidenceRepairGroups = UnifiedEvidenceRepairGroups)
  /\ (startupRepairStage \in
        {"Unplanned", "NeedBodies", "BodiesRecovered",
         "CarrierPreflighted", "PlanReady", "GroupsPreflight"} =>
       appliedEvidenceRepairGroups = {})

MLUnifiedCanonicalBodyNeedCoalesced ==
  /\ (startupRepairStage # "Unplanned" => canonicalBodyNeedCount = 1)
  /\ (bodyCachePopulated =>
       /\ canonicalBodiesRecovered = 1
       /\ recoveredCanonicalBodyGroups = UnifiedEvidenceRepairGroups)
  /\ (~bodyCachePopulated =>
       /\ canonicalBodiesRecovered = 0
       /\ recoveredCanonicalBodyGroups = {})

\* No group is publishable until the complete mixed ordinary/Native plan has
\* passed read-only preflight. Application is all-or-nothing across the plan.
MLUnifiedStartupAllGroupsPreflight ==
  /\ (appliedEvidenceRepairGroups # {} =>
       /\ preflightedEvidenceRepairGroups = UnifiedEvidenceRepairGroups
       /\ appliedEvidenceRepairGroups = UnifiedEvidenceRepairGroups)
  /\ (evidenceRepairReadBackVerified =>
       appliedEvidenceRepairGroups = UnifiedEvidenceRepairGroups)

\* Queue reconciliation and ordinary selection are one terminal publication
\* boundary, strictly after exact evidence application and durable readback.
MLUnifiedStartupQueueAfterReadback ==
  /\ (queueGateOpen = queueReservationReconciled)
  /\ (queueGateOpen =>
       /\ startupRepairStage = "QueueReconciled"
       /\ planRevalidatedAfterRecovery
       /\ preflightedEvidenceRepairGroups = UnifiedEvidenceRepairGroups
       /\ appliedEvidenceRepairGroups = UnifiedEvidenceRepairGroups
       /\ evidenceRepairReadBackVerified)

\* After re-planning, the durable carrier index is a two-way projection of
\* finality: Some has one exact record or one exact pending repair, while None
\* has neither. After application no pending projection may remain.
MLMergeCarrierEvidenceBidirectional ==
  /\ (planRevalidatedAfterRecovery =>
       /\ (~finalityDeclaresMergeCarrier =>
            /\ ~mergeCarrierRecordPresent
            /\ ~mergeCarrierRepairPlanned)
       /\ (finalityDeclaresMergeCarrier =>
            \/ /\ mergeCarrierRecordPresent
                  /\ mergeCarrierRecordExact
               /\ ~mergeCarrierRepairPlanned
            \/ /\ ~mergeCarrierRecordPresent
                  /\ mergeCarrierRepairPlanned))
  /\ (startupRepairStage \in
        {"EvidenceApplied", "ReadBackVerified", "QueueReconciled"} =>
       /\ (finalityDeclaresMergeCarrier = mergeCarrierRecordPresent)
       /\ ~mergeCarrierRepairPlanned
       /\ (mergeCarrierRecordPresent => mergeCarrierRecordExact))

\* Installing a recovered body in the canonical cache is not the end of
\* recovery. The reverse carrier preflight must finish before that body can
\* make a re-planned application-evidence group Ready.
MLPostCacheCarrierPreflighted ==
  bodyCachePopulated /\ planRevalidatedAfterRecovery =>
    postCacheCarrierPreflighted

MLUnifiedStartupEvidenceRepairSafe ==
  /\ MLUnifiedStartupPlanningReadOnly
  /\ MLUnifiedCanonicalBodyNeedCoalesced
  /\ MLUnifiedStartupAllGroupsPreflight
  /\ MLUnifiedStartupQueueAfterReadback
  /\ MLMergeCarrierEvidenceBidirectional
  /\ MLPostCacheCarrierPreflighted
  /\ MLNativeStartupRepairTargetsExactActiveMarkerRoutes

MLNativeDurabilityPrecedesFrontier ==
  /\ NativeStandaloneEvidenceInvariant
  /\ NativeEvidenceRetentionBoundInvariant
  /\ MLNativeSharedEvidenceBudget
  /\ MLNativeSingleIncomingPairHeadroom
  /\ MLNativeTempPromotionAuthenticated
  /\ MLNativeRetainedHistoryExact
  /\ MLNativePruneOldestPrefix
  /\ MLNativePruneProtectedLatestExact
  /\ MLNativePruneExactObjectRemoval
  /\ MLUnifiedStartupEvidenceRepairSafe
  /\ NativePruneJournalInvariant
  /\ FrontierPublicationInvariant
  /\ (startupRepairRequired =>
       /\ pruneIntentDurable
       /\ ExactPruneIntentIdentity
       /\ latestIndexHeight \notin pruneIntentHeights
       /\ ~startupRepairCompleted
       /\ ~durableApplicationLost)
  /\ (startupRepairCompleted =>
       /\ ~startupRepairRequired
       /\ ~pruneIntentDurable
       /\ ~pruneIntentTempPresent
       /\ ~durableApplicationLost)

MLNativeLatestIndexExact ==
  /\ latestIndexHeight \in receiptFiles
  /\ latestIndexHeight \notin pruneIntentHeights
  /\ latestIndexExact
  /\ ~latestIndexAmbiguous
  /\ latestIndexBounded
  /\ NativeNoClobberPublicationInvariant
  /\ NativeLegacyDenseRejectedInvariant
  /\ (latestIndexPublished =>
       /\ latestIndexHeight = TargetHeight
       /\ receiptDurable)
  /\ (~latestIndexPublished =>
       /\ latestIndexHeight = PreviousLatestHeight
       /\ ~frontierPublished)
  /\ (phase \in NativeLatestTempPhases =>
       \/ /\ latestIndexTempFiles = {TargetHeight}
             /\ TargetHeight \in
                  (finalizedHeights \intersect
                   manifestFiles \intersect receiptFiles)
             /\ manifestLeafAuthenticated
             /\ manifestLeafExact
             /\ latestTempPublicationExact
          \/ /\ latestIndexTempFiles = {}
             /\ latestIndexPublished
             /\ latestIndexHeight = TargetHeight
             /\ latestIndexExact
             /\ ~latestIndexAmbiguous
             /\ latestIndexBounded)
  /\ (phase \notin NativeLatestTempPhases => latestIndexTempFiles = {})

NativeApplicationEvidenceSafetyInvariant ==
  /\ NativeEvidenceTypeInvariant
  /\ NativeStandaloneEvidenceInvariant
  /\ NativeEvidenceRetentionBoundInvariant
  /\ MLNativeSharedEvidenceBudget
  /\ MLNativeSingleIncomingPairHeadroom
  /\ MLNativeTempPromotionAuthenticated
  /\ MLNativeRetainedHistoryExact
  /\ MLNativePruneOldestPrefix
  /\ MLNativePruneProtectedLatestExact
  /\ MLNativePruneExactObjectRemoval
  /\ NativeNoClobberPublicationInvariant
  /\ NativeLegacyDenseRejectedInvariant
  /\ NativePruneJournalInvariant
  /\ SidecarsRequireManifestInvariant
  /\ FrontierPublicationInvariant
  /\ PrunedEvidenceVerifiableInvariant
  /\ SameRouteControlOnlyInvariant
  /\ MLSeparateParticipantApplication
  /\ MLNativeSourceClaimInjective
  /\ MLNativeContiguousActiveRoute
  /\ MLNativeGroupExactCover
  /\ MLNativeManifestAuthenticates
  /\ MLUnifiedStartupEvidenceRepairSafe
  /\ MLNativeDurabilityPrecedesFrontier
  /\ MLNativeLatestIndexExact

NativeEvidenceSpec == Init /\ [][Next]_vars

NativeApplicationEvidenceProductionRefinementObligation ==
  NativeEvidenceSpec => []NativeApplicationEvidenceSafetyInvariant

====
