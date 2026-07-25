---- MODULE SumeragiV2NativeApplicationEvidence ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
Bounded durability/publication model for control-only Native AMX participant
application evidence.

The storage abstraction has three participant heights. Heights 1 and 2 are
complete standalone evidence pairs retained at startup; height 2 is named by
the descriptor-bound latest pointer. Height 3 is the newly finalized carrier.
Each per-height publication represents the production write/fsync/no-clobber
rename/readback sequence as one atomic model step, while prune-intent temp
publication and every unlink remain separate crash boundaries.

The production refinement is source-bound separately to the Native signing,
manifest/receipt validation, standalone Kura publication, descriptor-bound
latest-pointer, prune-intent completion, startup rebuild, WSV frontier, and
retirement entry points. The finite model does not prove that those Rust entry
points refine this ordering.
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
   "ForgedManifestLeaf", "DropStartupRepair", "AmbiguousLatestIndex"}

NativeEvidencePhases ==
  {"Certified", "FinalityDurable", "ManifestDurable",
   "ReceiptDurable", "LatestDurable", "Published"}

NativePruneStages ==
  {"Idle", "TempDurable", "IntentDurable",
   "ManifestUnlinked", "ReceiptUnlinked", "Completed"}

TargetHeight == 3
PreviousLatestHeight == 2
InitialEvidenceHeights == {1, 2}
EvidenceHeights == 1..TargetHeight
RetentionLimit == 2
PublicationTransientLimit == RetentionLimit + 1
ArtifactByteUnit == 1
PerKindRetainedByteLimit == RetentionLimit * ArtifactByteUnit
PerKindStartupByteLimit == PublicationTransientLimit * ArtifactByteUnit

NoHeight == 0
PruneIntentVersion == 1
ActiveRoute == 1
ActiveIncarnation == 2
PrunedHeight == 1

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
  groupAppliedAtomically

publicationVars ==
  <<phase, finalizedHeights, manifestFiles, receiptFiles,
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
    pruneIntentHeights, startupRepairRequired,
    startupRepairCompleted, durableApplicationLost>>

sameRouteVars == <<sameRouteSettled, separateParticipantMarker>>

claimVars ==
  <<sourceClaimRecorded, sourceClaimSessionCount, sourceClaimFieldsComplete>>

admissionVars ==
  <<nativeAdmissionAttempted, activeIncarnationExact, predecessorExact,
    contiguousNextHeight>>

groupVars ==
  <<groupApplied, groupUnique, groupOrdered, groupExactCover,
    groupAppliedAtomically>>

vars ==
  <<phase, finalizedHeights, manifestFiles, receiptFiles,
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
    pruneIntentHeights, startupRepairRequired,
    startupRepairCompleted, durableApplicationLost,
    sameRouteSettled, separateParticipantMarker,
    sourceClaimRecorded, sourceClaimSessionCount, sourceClaimFieldsComplete,
    nativeAdmissionAttempted, activeIncarnationExact, predecessorExact,
    contiguousNextHeight, groupApplied, groupUnique, groupOrdered,
    groupExactCover, groupAppliedAtomically>>

finalityDurable == TargetHeight \in finalizedHeights
manifestDurable == TargetHeight \in manifestFiles
receiptDurable == TargetHeight \in receiptFiles
sidecarsDurable == receiptDurable

ExactPruneIntentIdentity ==
  /\ pruneIntentStoredVersion = PruneIntentVersion
  /\ pruneIntentRoute = ActiveRoute
  /\ pruneIntentIncarnation = ActiveIncarnation
  /\ pruneIntentHeights = {PrunedHeight}
  /\ pruneIntentManifestHash = ManifestArtifactHash(PrunedHeight)
  /\ pruneIntentReceiptHash = ReceiptArtifactHash(PrunedHeight)
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

PersistFinality ==
  /\ phase = "Certified"
  /\ phase' = "FinalityDurable"
  /\ finalizedHeights' = finalizedHeights \union {TargetHeight}
  /\ UNCHANGED
       <<manifestFiles, receiptFiles, frontierPublished, frontierHeight,
         canonicalWireRetained, authenticatedProofAvailable,
         manifestLeafAuthenticated, manifestLeafExact,
         manifestTempPublicationExact, receiptTempPublicationExact,
         latestTempPublicationExact, manifestPublishedNoClobber,
         receiptPublishedNoClobber, latestIndexPublished,
         latestIndexHeight, latestIndexExact, latestIndexAmbiguous,
         latestIndexBounded, legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

PersistStandaloneManifest ==
  /\ phase = "FinalityDurable"
  /\ finalityDurable
  /\ TargetHeight \notin manifestFiles
  /\ phase' = "ManifestDurable"
  /\ manifestFiles' = manifestFiles \union {TargetHeight}
  /\ authenticatedProofAvailable' = TRUE
  /\ manifestLeafAuthenticated' = (Mode # "ForgedManifestLeaf")
  /\ manifestLeafExact' = (Mode # "ForgedManifestLeaf")
  /\ manifestTempPublicationExact' = (Mode # "AmbiguousLatestIndex")
  /\ manifestPublishedNoClobber' = (Mode # "AmbiguousLatestIndex")
  /\ UNCHANGED
       <<finalizedHeights, receiptFiles, frontierPublished, frontierHeight,
         canonicalWireRetained, receiptTempPublicationExact,
         latestTempPublicationExact, receiptPublishedNoClobber,
         latestIndexPublished, latestIndexHeight, latestIndexExact,
         latestIndexAmbiguous, latestIndexBounded,
         legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

PersistStandaloneReceipt ==
  /\ phase = "ManifestDurable"
  /\ finalityDurable
  /\ manifestDurable
  /\ TargetHeight \notin receiptFiles
  /\ phase' = "ReceiptDurable"
  /\ receiptFiles' = receiptFiles \union {TargetHeight}
  /\ receiptTempPublicationExact' = (Mode # "AmbiguousLatestIndex")
  /\ receiptPublishedNoClobber' = (Mode # "AmbiguousLatestIndex")
  /\ UNCHANGED
       <<finalizedHeights, manifestFiles, frontierPublished, frontierHeight,
         canonicalWireRetained, authenticatedProofAvailable,
         manifestLeafAuthenticated, manifestLeafExact,
         manifestTempPublicationExact, latestTempPublicationExact,
         manifestPublishedNoClobber, latestIndexPublished,
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
          /\ Cardinality(manifestFiles) <= RetentionLimit
          /\ Cardinality(receiptFiles) <= RetentionLimit
  /\ phase' = "Published"
  /\ frontierPublished' = TRUE
  /\ frontierHeight' = TargetHeight
  /\ UNCHANGED
       <<finalizedHeights, manifestFiles, receiptFiles,
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
         frontierPublished, frontierHeight, authenticatedProofAvailable,
         manifestLeafAuthenticated, manifestLeafExact,
         manifestTempPublicationExact, receiptTempPublicationExact,
         latestTempPublicationExact, manifestPublishedNoClobber,
         receiptPublishedNoClobber, latestIndexPublished,
         latestIndexHeight, latestIndexExact, latestIndexAmbiguous,
         latestIndexBounded, legacyDenseRejected, legacyDenseAccepted>>
  /\ UNCHANGED pruneVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

StageNativePruneIntentTemp ==
  /\ pruneStage = "Idle"
  /\ PrunedHeight \in manifestFiles
  /\ PrunedHeight \in receiptFiles
  /\ latestIndexHeight # PrunedHeight
  /\ pruneStage' = "TempDurable"
  /\ pruneIntentTempPresent' = TRUE
  /\ pruneIntentDurable' = FALSE
  /\ pruneTempPublishedNoClobber' = (Mode # "AmbiguousLatestIndex")
  /\ pruneIntentStoredVersion' = PruneIntentVersion
  /\ pruneIntentRoute' = ActiveRoute
  /\ pruneIntentIncarnation' = ActiveIncarnation
  /\ pruneIntentManifestHash' = ManifestArtifactHash(PrunedHeight)
  /\ pruneIntentReceiptHash' = ReceiptArtifactHash(PrunedHeight)
  /\ pruneIntentHeights' = {PrunedHeight}
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
         pruneIntentHeights, startupRepairRequired,
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
         pruneIntentHeights>>
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
         pruneIntentHeights>>
  /\ UNCHANGED publicationVars
  /\ UNCHANGED <<sameRouteVars, claimVars, admissionVars, groupVars>>

UnlinkManifestBeforeCrash ==
  /\ pruneStage = "IntentDurable"
  /\ pruneIntentDurable
  /\ PrunedHeight \in manifestFiles
  /\ PrunedHeight \in receiptFiles
  /\ pruneStage' = "ManifestUnlinked"
  /\ manifestFiles' = manifestFiles \ {PrunedHeight}
  /\ startupRepairRequired' = TRUE
  /\ startupRepairCompleted' = FALSE
  /\ durableApplicationLost' = (Mode = "DropStartupRepair")
  /\ UNCHANGED
       <<pruneIntentTempPresent, pruneIntentDurable,
         pruneTempPublishedNoClobber, pruneIntentStoredVersion,
         pruneIntentRoute, pruneIntentIncarnation,
         pruneIntentManifestHash, pruneIntentReceiptHash,
         pruneIntentHeights>>
  /\ UNCHANGED
       <<phase, finalizedHeights, receiptFiles,
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
  /\ PrunedHeight \in manifestFiles
  /\ PrunedHeight \in receiptFiles
  /\ pruneStage' = "ReceiptUnlinked"
  /\ receiptFiles' = receiptFiles \ {PrunedHeight}
  /\ startupRepairRequired' = TRUE
  /\ startupRepairCompleted' = FALSE
  /\ durableApplicationLost' = (Mode = "DropStartupRepair")
  /\ UNCHANGED
       <<pruneIntentTempPresent, pruneIntentDurable,
         pruneTempPublishedNoClobber, pruneIntentStoredVersion,
         pruneIntentRoute, pruneIntentIncarnation,
         pruneIntentManifestHash, pruneIntentReceiptHash,
         pruneIntentHeights>>
  /\ UNCHANGED
       <<phase, finalizedHeights, manifestFiles,
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
  /\ pruneIntentHeights' = {}
  /\ startupRepairRequired' = FALSE
  /\ startupRepairCompleted' = TRUE
  /\ durableApplicationLost' = FALSE
  /\ UNCHANGED pruneTempPublishedNoClobber
  /\ UNCHANGED
       <<phase, finalizedHeights, frontierPublished, frontierHeight,
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
  /\ pruneIntentHeights' = {}
  /\ startupRepairRequired' = FALSE
  /\ startupRepairCompleted' = TRUE
  /\ durableApplicationLost' = FALSE
  /\ UNCHANGED pruneTempPublishedNoClobber
  /\ UNCHANGED
       <<phase, finalizedHeights, frontierPublished, frontierHeight,
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

Next ==
  \/ PersistFinality
  \/ PersistStandaloneManifest
  \/ PersistStandaloneReceipt
  \/ PublishDescriptorBoundLatest
  \/ PublishReplicatedFrontier
  \/ PruneCanonicalWire
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

NativeEvidenceTypeInvariant ==
  /\ NativeEvidenceConfiguration
  /\ phase \in NativeEvidencePhases
  /\ finalizedHeights \subseteq EvidenceHeights
  /\ manifestFiles \subseteq EvidenceHeights
  /\ receiptFiles \subseteq EvidenceHeights
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

NativeStandaloneEvidenceInvariant ==
  /\ manifestFiles \subseteq finalizedHeights
  /\ receiptFiles \subseteq finalizedHeights
  /\ (manifestFiles \ receiptFiles)
       \subseteq ({TargetHeight} \union pruneIntentHeights)
  /\ (receiptFiles \ manifestFiles) \subseteq pruneIntentHeights
  /\ latestIndexHeight \in receiptFiles

NativeEvidenceRetentionBoundInvariant ==
  /\ Cardinality(manifestFiles) <= PublicationTransientLimit
  /\ Cardinality(receiptFiles) <= PublicationTransientLimit
  /\ Cardinality(manifestFiles) * ArtifactByteUnit
       <= PerKindStartupByteLimit
  /\ Cardinality(receiptFiles) * ArtifactByteUnit
       <= PerKindStartupByteLimit
  /\ (frontierPublished =>
       /\ Cardinality(manifestFiles) <= RetentionLimit
       /\ Cardinality(receiptFiles) <= RetentionLimit
       /\ Cardinality(manifestFiles) * ArtifactByteUnit
            <= PerKindRetainedByteLimit
       /\ Cardinality(receiptFiles) * ArtifactByteUnit
            <= PerKindRetainedByteLimit)

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
            /\ PrunedHeight \in manifestFiles
            /\ PrunedHeight \in receiptFiles
            /\ ~startupRepairRequired
            /\ ~startupRepairCompleted
            /\ ~durableApplicationLost
       [] pruneStage = "IntentDurable" ->
            /\ ~pruneIntentTempPresent
            /\ pruneIntentDurable
            /\ ExactPruneIntentIdentity
            /\ PrunedHeight \in manifestFiles
            /\ PrunedHeight \in receiptFiles
            /\ ~startupRepairCompleted
            /\ ~durableApplicationLost
       [] pruneStage = "ManifestUnlinked" ->
            /\ ~pruneIntentTempPresent
            /\ pruneIntentDurable
            /\ ExactPruneIntentIdentity
            /\ PrunedHeight \notin manifestFiles
            /\ PrunedHeight \in receiptFiles
            /\ startupRepairRequired
            /\ ~startupRepairCompleted
            /\ ~durableApplicationLost
       [] pruneStage = "ReceiptUnlinked" ->
            /\ ~pruneIntentTempPresent
            /\ pruneIntentDurable
            /\ ExactPruneIntentIdentity
            /\ PrunedHeight \in manifestFiles
            /\ PrunedHeight \notin receiptFiles
            /\ startupRepairRequired
            /\ ~startupRepairCompleted
            /\ ~durableApplicationLost
       [] pruneStage = "Completed" ->
            /\ ~pruneIntentTempPresent
            /\ ~pruneIntentDurable
            /\ ResetPruneIntentIdentity
            /\ PrunedHeight \notin manifestFiles
            /\ PrunedHeight \notin receiptFiles
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

MLNativeDurabilityPrecedesFrontier ==
  /\ NativeStandaloneEvidenceInvariant
  /\ NativeEvidenceRetentionBoundInvariant
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
  /\ MLNativeDurabilityPrecedesFrontier
  /\ MLNativeLatestIndexExact

NativeEvidenceSpec == Init /\ [][Next]_vars

NativeApplicationEvidenceProductionRefinementObligation ==
  NativeEvidenceSpec => []NativeApplicationEvidenceSafetyInvariant

====
