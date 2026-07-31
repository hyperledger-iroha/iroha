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
   "OrphanMergeCarrier", "SkipPostCacheCarrierReconcile"}

NativeEvidencePhases ==
  {"Certified", "FinalityDurable", "ManifestTempDurable",
   "ManifestDurable", "ReceiptTempDurable", "ReceiptDurable",
   "LatestDurable", "Published"}

NativePruneStages ==
  {"Idle", "TempDurable", "IntentDurable",
   "ManifestUnlinked", "ReceiptUnlinked", "Completed"}

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
PruneIntentVersion == 1
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
  \* @type: Bool;
  sameRouteSettled,
  \* @type: Bool;
  separateParticipantMarker,
  \* @type: Bool;
  sourceClaimRecorded,
  \* @type: Int;
  sourceClaimSessionCount,
  \* @type: Bool;
  sourceClaimFieldsComplete,
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
  postCacheCarrierPreflighted

publicationVars ==
  <<phase, finalizedHeights, manifestFiles, receiptFiles,
    manifestTempFiles, receiptTempFiles, manifestTempAuthenticated,
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
    pruneIntentHeights, removedEvidenceHeights, startupRepairRequired,
    startupRepairCompleted, durableApplicationLost>>

sameRouteVars == <<sameRouteSettled, separateParticipantMarker>>

sourceClaimVars ==
  <<sourceClaimRecorded, sourceClaimSessionCount, sourceClaimFieldsComplete>>

startupRepairVars ==
  <<startupRepairStage, plannedEvidenceRepairGroups,
    startupRepairPlanReadOnly, canonicalBodyNeedCount,
    canonicalBodiesRecovered, recoveredCanonicalBodyGroups,
    planRevalidatedAfterRecovery, preflightedEvidenceRepairGroups,
    appliedEvidenceRepairGroups, evidenceRepairReadBackVerified,
    queueGateOpen, queueReservationReconciled,
    finalityDeclaresMergeCarrier, mergeCarrierRecordPresent,
    mergeCarrierRecordExact, mergeCarrierRepairPlanned,
    bodyCachePopulated, postCacheCarrierPreflighted>>

claimVars == <<sourceClaimVars, startupRepairVars>>

admissionVars ==
  <<nativeAdmissionAttempted, activeIncarnationExact, predecessorExact,
    contiguousNextHeight>>

groupVars ==
  <<groupApplied, groupUnique, groupOrdered, groupExactCover,
    groupAppliedAtomically>>

vars ==
  <<phase, finalizedHeights, manifestFiles, receiptFiles,
    manifestTempFiles, receiptTempFiles, manifestTempAuthenticated,
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
    pruneIntentHeights, removedEvidenceHeights, startupRepairRequired,
    startupRepairCompleted, durableApplicationLost,
    sameRouteSettled, separateParticipantMarker,
    sourceClaimRecorded, sourceClaimSessionCount, sourceClaimFieldsComplete,
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
    bodyCachePopulated, postCacheCarrierPreflighted>>

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

ExactPruneIntentIdentity ==
  /\ pruneIntentStoredVersion = PruneIntentVersion
  /\ pruneIntentRoute = ActiveRoute
  /\ pruneIntentIncarnation = ActiveIncarnation
  /\ pruneIntentHeights = {PruneTargetHeight}
  /\ pruneIntentManifestHash = ManifestArtifactHash(PruneTargetHeight)
  /\ pruneIntentReceiptHash = ReceiptArtifactHash(PruneTargetHeight)
  /\ latestIndexHeight \notin pruneIntentHeights

ResetPruneIntentIdentity ==
  /\ pruneIntentStoredVersion = 0
  /\ pruneIntentRoute = 0
  /\ pruneIntentIncarnation = 0
  /\ pruneIntentManifestHash = 0
  /\ pruneIntentReceiptHash = 0
  /\ pruneIntentHeights = {}

Init ==
  /\ NativeEvidenceConfiguration
  /\ phase = "Certified"
  /\ finalizedHeights = InitialEvidenceHeights
  /\ manifestFiles = InitialEvidenceHeights
  /\ receiptFiles = InitialEvidenceHeights
  /\ manifestTempFiles = {}
  /\ receiptTempFiles = {}
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
  /\ pruneIntentHeights = {}
  /\ removedEvidenceHeights = {}
  /\ startupRepairRequired = FALSE
  /\ startupRepairCompleted = FALSE
  /\ durableApplicationLost = FALSE
  /\ sameRouteSettled = FALSE
  /\ separateParticipantMarker = FALSE
  /\ sourceClaimRecorded = FALSE
  /\ sourceClaimSessionCount = 0
  /\ sourceClaimFieldsComplete = TRUE
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

PersistFinality ==
  /\ phase = "Certified"
  /\ phase' = "FinalityDurable"
  /\ finalizedHeights' = finalizedHeights \union {TargetHeight}
  /\ UNCHANGED
       <<manifestFiles, receiptFiles, manifestTempFiles, receiptTempFiles,
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

PublishDescriptorBoundLatest ==
  /\ phase = "ReceiptDurable"
  /\ receiptDurable
  /\ TargetHeight \in manifestFiles
  /\ phase' = "LatestDurable"
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
         manifestTempFiles, receiptTempFiles,
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
         manifestTempFiles, receiptTempFiles,
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
         manifestTempFiles, receiptTempFiles,
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
         manifestTempFiles, receiptTempFiles,
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
         manifestTempFiles, receiptTempFiles,
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
         manifestTempFiles, receiptTempFiles,
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
  /\ pruneIntentHeights' = {PruneTargetHeight}
  /\ removedEvidenceHeights' = removedEvidenceHeights
  /\ startupRepairRequired' = FALSE
  /\ startupRepairCompleted' = FALSE
  /\ durableApplicationLost' = FALSE
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
         pruneIntentHeights, removedEvidenceHeights, startupRepairRequired,
         startupRepairCompleted, durableApplicationLost>>
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
  /\ UNCHANGED
       <<pruneTempPublishedNoClobber, pruneIntentStoredVersion,
         pruneIntentRoute, pruneIntentIncarnation,
         pruneIntentManifestHash, pruneIntentReceiptHash,
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
  /\ UNCHANGED
       <<pruneIntentTempPresent, pruneIntentDurable,
         pruneTempPublishedNoClobber, pruneIntentStoredVersion,
         pruneIntentRoute, pruneIntentIncarnation,
         pruneIntentManifestHash, pruneIntentReceiptHash,
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
  /\ UNCHANGED
       <<pruneIntentTempPresent, pruneIntentDurable,
         pruneTempPublishedNoClobber, pruneIntentStoredVersion,
         pruneIntentRoute, pruneIntentIncarnation,
         pruneIntentManifestHash, pruneIntentReceiptHash,
         pruneIntentHeights, removedEvidenceHeights>>
  /\ UNCHANGED
       <<phase, finalizedHeights, receiptFiles,
         manifestTempFiles, receiptTempFiles,
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
  /\ UNCHANGED
       <<pruneIntentTempPresent, pruneIntentDurable,
         pruneTempPublishedNoClobber, pruneIntentStoredVersion,
         pruneIntentRoute, pruneIntentIncarnation,
         pruneIntentManifestHash, pruneIntentReceiptHash,
         pruneIntentHeights, removedEvidenceHeights>>
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestFiles,
         manifestTempFiles, receiptTempFiles,
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
  /\ removedEvidenceHeights' = removedEvidenceHeights \union pruneIntentHeights
  /\ pruneIntentHeights' = {}
  /\ publicationPairPendingCleanup' =
       publicationPairPendingCleanup /\ ~frontierPublished
  /\ startupRepairRequired' = FALSE
  /\ startupRepairCompleted' = TRUE
  /\ durableApplicationLost' = FALSE
  /\ UNCHANGED pruneTempPublishedNoClobber
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestTempFiles, receiptTempFiles,
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
  /\ removedEvidenceHeights' = removedEvidenceHeights \union pruneIntentHeights
  /\ pruneIntentHeights' = {}
  /\ publicationPairPendingCleanup' =
       publicationPairPendingCleanup /\ ~frontierPublished
  /\ startupRepairRequired' = FALSE
  /\ startupRepairCompleted' = TRUE
  /\ durableApplicationLost' = FALSE
  /\ UNCHANGED pruneTempPublishedNoClobber
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestTempFiles, receiptTempFiles,
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
  /\ ~sourceClaimRecorded
  /\ sourceClaimRecorded' = TRUE
  /\ sourceClaimSessionCount' =
       IF Mode = "DivergentSourceClaim" THEN 2 ELSE 1
  /\ sourceClaimFieldsComplete' = (Mode # "DivergentSourceClaim")
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
  /\ UNCHANGED
       <<canonicalBodiesRecovered, recoveredCanonicalBodyGroups,
         preflightedEvidenceRepairGroups, evidenceRepairReadBackVerified,
         queueGateOpen, queueReservationReconciled,
         finalityDeclaresMergeCarrier, mergeCarrierRecordPresent,
         mergeCarrierRecordExact, mergeCarrierRepairPlanned,
         bodyCachePopulated, postCacheCarrierPreflighted>>
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
         mergeCarrierRepairPlanned, postCacheCarrierPreflighted>>
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
         mergeCarrierRepairPlanned, bodyCachePopulated>>
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
         postCacheCarrierPreflighted>>
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
  /\ UNCHANGED
       <<plannedEvidenceRepairGroups, startupRepairPlanReadOnly,
         canonicalBodyNeedCount, canonicalBodiesRecovered,
         recoveredCanonicalBodyGroups, planRevalidatedAfterRecovery,
         appliedEvidenceRepairGroups, evidenceRepairReadBackVerified,
         queueGateOpen, queueReservationReconciled,
         finalityDeclaresMergeCarrier, mergeCarrierRecordPresent,
         mergeCarrierRecordExact, mergeCarrierRepairPlanned,
         bodyCachePopulated, postCacheCarrierPreflighted>>
  /\ UNCHANGED
       <<publicationVars, pruneVars, sameRouteVars, sourceClaimVars,
         admissionVars, groupVars>>

ApplyUnifiedStartupEvidenceGroups ==
  /\ startupRepairStage = "GroupsPreflight"
  /\ \/ preflightedEvidenceRepairGroups = UnifiedEvidenceRepairGroups
     \/ Mode = "PartialUnifiedStartupPreflight"
  /\ startupRepairStage' = "EvidenceApplied"
  /\ appliedEvidenceRepairGroups' = preflightedEvidenceRepairGroups
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
         postCacheCarrierPreflighted>>
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
         bodyCachePopulated, postCacheCarrierPreflighted>>
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
         postCacheCarrierPreflighted>>
  /\ UNCHANGED
       <<publicationVars, pruneVars, sameRouteVars, sourceClaimVars,
         admissionVars, groupVars>>

Next ==
  \/ PersistFinality
  \/ StageStandaloneManifestTemp
  \/ PersistStandaloneManifest
  \/ StageStandaloneReceiptTemp
  \/ PersistStandaloneReceipt
  \/ PublishDescriptorBoundLatest
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
  /\ pruneIntentHeights \subseteq EvidenceHeights
  /\ removedEvidenceHeights \subseteq EvidenceHeights
  /\ startupRepairRequired \in BOOLEAN
  /\ startupRepairCompleted \in BOOLEAN
  /\ durableApplicationLost \in BOOLEAN
  /\ sameRouteSettled \in BOOLEAN
  /\ separateParticipantMarker \in BOOLEAN
  /\ sourceClaimRecorded \in BOOLEAN
  /\ sourceClaimSessionCount \in 0..2
  /\ sourceClaimFieldsComplete \in BOOLEAN
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

MLNativeSourceClaimInjective ==
  sourceClaimRecorded =>
    /\ sourceClaimSessionCount = 1
    /\ sourceClaimFieldsComplete

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

MLNativeDurabilityPrecedesFrontier ==
  /\ NativeStandaloneEvidenceInvariant
  /\ NativeEvidenceRetentionBoundInvariant
  /\ MLNativeSharedEvidenceBudget
  /\ MLNativeSingleIncomingPairHeadroom
  /\ MLNativeTempPromotionAuthenticated
  /\ MLNativeRetainedHistoryExact
  /\ MLNativePruneOldestPrefix
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

NativeApplicationEvidenceSafetyInvariant ==
  /\ NativeEvidenceTypeInvariant
  /\ NativeStandaloneEvidenceInvariant
  /\ NativeEvidenceRetentionBoundInvariant
  /\ MLNativeSharedEvidenceBudget
  /\ MLNativeSingleIncomingPairHeadroom
  /\ MLNativeTempPromotionAuthenticated
  /\ MLNativeRetainedHistoryExact
  /\ MLNativePruneOldestPrefix
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
