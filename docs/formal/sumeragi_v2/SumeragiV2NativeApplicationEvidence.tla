---- MODULE SumeragiV2NativeApplicationEvidence ----
EXTENDS Naturals

(***************************************************************************
Bounded durability/publication model for control-only Native AMX participant
application evidence.

The production refinement is source-bound separately to
`NativeAmxSourceSessionClaimV4`,
`NativeAmxSourceParticipantClaimV4`,
`NativeAmxSigningGuard::record_locked`,
`Kura::native_amx_participant_application_evidence_for_block_under_publication_guard`,
`Kura::persist_native_amx_participant_application_evidence`,
`Kura::repair_native_amx_participant_application_evidence`,
`Kura::validate_native_amx_participant_application_receipt_artifact`,
`Kura::latest_native_amx_participant_application_receipt_matching`,
`State::native_amx_participant_frontier_marker_payloads`, and
`StateBlock::stage_native_amx_participant_frontiers`. The finite model does not
prove that those Rust entry points refine this ordering.
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
  {"Certified", "FinalityDurable", "SidecarsDurable", "Published", "Pruned"}

NativeEvidenceConfiguration ==
  /\ Mode \in NativeEvidenceModes
  /\ SourceCount \in 1..4096

VARIABLES
  \* @type: Str;
  phase,
  \* @type: Bool;
  finalityDurable,
  \* @type: Bool;
  manifestDurable,
  \* @type: Bool;
  sidecarsDurable,
  \* @type: Bool;
  frontierPublished,
  \* @type: Bool;
  canonicalWireRetained,
  \* @type: Bool;
  authenticatedProofAvailable,
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
  \* @type: Bool;
  manifestLeafAuthenticated,
  \* @type: Bool;
  manifestLeafExact,
  \* @type: Bool;
  startupRepairRequired,
  \* @type: Bool;
  startupRepairCompleted,
  \* @type: Bool;
  durableApplicationLost,
  \* @type: Bool;
  latestIndexPublished,
  \* @type: Bool;
  latestIndexExact,
  \* @type: Bool;
  latestIndexAmbiguous,
  \* @type: Bool;
  latestIndexBounded

vars ==
  <<phase, finalityDurable, manifestDurable, sidecarsDurable,
    frontierPublished, canonicalWireRetained, authenticatedProofAvailable,
    sameRouteSettled, separateParticipantMarker, sourceClaimRecorded,
    sourceClaimSessionCount, sourceClaimFieldsComplete,
    nativeAdmissionAttempted, activeIncarnationExact, predecessorExact,
    contiguousNextHeight, groupApplied, groupUnique, groupOrdered,
    groupExactCover, groupAppliedAtomically, manifestLeafAuthenticated,
    manifestLeafExact, startupRepairRequired, startupRepairCompleted,
    durableApplicationLost, latestIndexPublished, latestIndexExact,
    latestIndexAmbiguous, latestIndexBounded>>

claimVars ==
  <<sourceClaimRecorded, sourceClaimSessionCount, sourceClaimFieldsComplete>>

\* @type: Seq(Bool);
admissionVars ==
  <<nativeAdmissionAttempted, activeIncarnationExact, predecessorExact,
    contiguousNextHeight>>

\* @type: Seq(Bool);
groupVars ==
  <<groupApplied, groupUnique, groupOrdered, groupExactCover,
    groupAppliedAtomically>>

\* @type: Seq(Bool);
manifestVars == <<manifestLeafAuthenticated, manifestLeafExact>>

\* @type: Seq(Bool);
repairVars ==
  <<startupRepairRequired, startupRepairCompleted, durableApplicationLost>>

\* @type: Seq(Bool);
indexVars ==
  <<latestIndexPublished, latestIndexExact, latestIndexAmbiguous,
    latestIndexBounded>>

mainEvidenceVars ==
  <<phase, finalityDurable, manifestDurable, sidecarsDurable,
    frontierPublished, canonicalWireRetained, authenticatedProofAvailable,
    sameRouteSettled, separateParticipantMarker>>

Init ==
  /\ NativeEvidenceConfiguration
  /\ phase = "Certified"
  /\ finalityDurable = FALSE
  /\ manifestDurable = FALSE
  /\ sidecarsDurable = FALSE
  /\ frontierPublished = FALSE
  /\ canonicalWireRetained = TRUE
  /\ authenticatedProofAvailable = FALSE
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
  /\ manifestLeafAuthenticated = FALSE
  /\ manifestLeafExact = FALSE
  /\ startupRepairRequired = FALSE
  /\ startupRepairCompleted = FALSE
  /\ durableApplicationLost = FALSE
  /\ latestIndexPublished = FALSE
  /\ latestIndexExact = TRUE
  /\ latestIndexAmbiguous = FALSE
  /\ latestIndexBounded = TRUE

PersistFinalityAndManifest ==
  /\ phase = "Certified"
  /\ phase' = "FinalityDurable"
  /\ finalityDurable' = TRUE
  /\ manifestDurable' = (Mode # "PruneWithHashOnly")
  /\ authenticatedProofAvailable' = (Mode # "PruneWithHashOnly")
  /\ manifestLeafAuthenticated' =
       (Mode # "PruneWithHashOnly" /\ Mode # "ForgedManifestLeaf")
  /\ manifestLeafExact' =
       (Mode # "PruneWithHashOnly" /\ Mode # "ForgedManifestLeaf")
  /\ UNCHANGED <<sidecarsDurable, frontierPublished,
                 canonicalWireRetained, sameRouteSettled,
                 separateParticipantMarker>>
  /\ UNCHANGED <<claimVars, admissionVars, groupVars, repairVars, indexVars>>

PersistExactSidecars ==
  /\ phase = "FinalityDurable"
  /\ finalityDurable
  /\ manifestDurable
  /\ phase' = "SidecarsDurable"
  /\ sidecarsDurable' = TRUE
  /\ UNCHANGED <<finalityDurable, manifestDurable, frontierPublished,
                 canonicalWireRetained, authenticatedProofAvailable,
                 sameRouteSettled, separateParticipantMarker>>
  /\ UNCHANGED <<claimVars, admissionVars, groupVars, manifestVars,
                 repairVars, indexVars>>

PublishReplicatedFrontier ==
  /\ IF Mode = "PublishFrontierEarly"
     THEN /\ phase = "FinalityDurable"
          /\ finalityDurable
     ELSE /\ phase = "SidecarsDurable"
          /\ sidecarsDurable
  /\ phase' = "Published"
  /\ frontierPublished' = TRUE
  /\ UNCHANGED <<finalityDurable, manifestDurable, sidecarsDurable,
                 canonicalWireRetained, authenticatedProofAvailable,
                 sameRouteSettled, separateParticipantMarker>>
  /\ UNCHANGED <<claimVars, admissionVars, groupVars, manifestVars,
                 repairVars, indexVars>>

PruneCanonicalWire ==
  /\ IF Mode = "PruneWithHashOnly"
     THEN /\ phase = "FinalityDurable"
          /\ finalityDurable
     ELSE /\ phase \in {"SidecarsDurable", "Published"}
          /\ manifestDurable
          /\ sidecarsDurable
          /\ authenticatedProofAvailable
  /\ phase' = "Pruned"
  /\ canonicalWireRetained' = FALSE
  /\ UNCHANGED <<finalityDurable, manifestDurable, sidecarsDurable,
                 frontierPublished, authenticatedProofAvailable,
                 sameRouteSettled, separateParticipantMarker>>
  /\ UNCHANGED <<claimVars, admissionVars, groupVars, manifestVars,
                 repairVars, indexVars>>

SettleSameRouteControl ==
  /\ ~sameRouteSettled
  /\ sameRouteSettled' = TRUE
  /\ separateParticipantMarker' = (Mode = "SeparateSameRouteMarker")
  /\ UNCHANGED <<phase, finalityDurable, manifestDurable,
                 sidecarsDurable, frontierPublished,
                 canonicalWireRetained, authenticatedProofAvailable>>
  /\ UNCHANGED <<claimVars, admissionVars, groupVars, manifestVars,
                 repairVars, indexVars>>

RecordSourceSessionClaim ==
  /\ ~sourceClaimRecorded
  /\ sourceClaimRecorded' = TRUE
  /\ sourceClaimSessionCount' =
       IF Mode = "DivergentSourceClaim" THEN 2 ELSE 1
  /\ sourceClaimFieldsComplete' = (Mode # "DivergentSourceClaim")
  /\ UNCHANGED <<mainEvidenceVars, admissionVars, groupVars, manifestVars,
                 repairVars, indexVars>>

AdmitNativeControl ==
  /\ ~nativeAdmissionAttempted
  /\ nativeAdmissionAttempted' = TRUE
  /\ activeIncarnationExact' = (Mode # "NonContiguousRoute")
  /\ predecessorExact' = (Mode # "NonContiguousRoute")
  /\ contiguousNextHeight' = (Mode # "NonContiguousRoute")
  /\ UNCHANGED <<mainEvidenceVars, claimVars, groupVars, manifestVars,
                 repairVars, indexVars>>

ApplyNativeGroup ==
  /\ ~groupApplied
  /\ groupApplied' = TRUE
  /\ groupUnique' = (Mode # "PartialGroupApplication")
  /\ groupOrdered' = TRUE
  /\ groupExactCover' = (Mode # "PartialGroupApplication")
  /\ groupAppliedAtomically' = (Mode # "PartialGroupApplication")
  /\ UNCHANGED <<mainEvidenceVars, claimVars, admissionVars, manifestVars,
                 repairVars, indexVars>>

RunStartupRepair ==
  /\ finalityDurable
  /\ ~startupRepairRequired
  /\ startupRepairRequired' = TRUE
  /\ startupRepairCompleted' = (Mode # "DropStartupRepair")
  /\ durableApplicationLost' = (Mode = "DropStartupRepair")
  /\ UNCHANGED <<mainEvidenceVars, claimVars, admissionVars, groupVars,
                 manifestVars, indexVars>>

PublishLatestIndex ==
  /\ sidecarsDurable
  /\ ~latestIndexPublished
  /\ latestIndexPublished' = TRUE
  /\ latestIndexExact' = (Mode # "AmbiguousLatestIndex")
  /\ latestIndexAmbiguous' = (Mode = "AmbiguousLatestIndex")
  /\ latestIndexBounded' = TRUE
  /\ UNCHANGED <<mainEvidenceVars, claimVars, admissionVars, groupVars,
                 manifestVars, repairVars>>

Next ==
  \/ PersistFinalityAndManifest
  \/ PersistExactSidecars
  \/ PublishReplicatedFrontier
  \/ PruneCanonicalWire
  \/ SettleSameRouteControl
  \/ RecordSourceSessionClaim
  \/ AdmitNativeControl
  \/ ApplyNativeGroup
  \/ RunStartupRepair
  \/ PublishLatestIndex

NativeEvidenceTypeInvariant ==
  /\ NativeEvidenceConfiguration
  /\ phase \in NativeEvidencePhases
  /\ finalityDurable \in BOOLEAN
  /\ manifestDurable \in BOOLEAN
  /\ sidecarsDurable \in BOOLEAN
  /\ frontierPublished \in BOOLEAN
  /\ canonicalWireRetained \in BOOLEAN
  /\ authenticatedProofAvailable \in BOOLEAN
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
  /\ manifestLeafAuthenticated \in BOOLEAN
  /\ manifestLeafExact \in BOOLEAN
  /\ startupRepairRequired \in BOOLEAN
  /\ startupRepairCompleted \in BOOLEAN
  /\ durableApplicationLost \in BOOLEAN
  /\ latestIndexPublished \in BOOLEAN
  /\ latestIndexExact \in BOOLEAN
  /\ latestIndexAmbiguous \in BOOLEAN
  /\ latestIndexBounded \in BOOLEAN

SidecarsRequireManifestInvariant ==
  sidecarsDurable => finalityDurable /\ manifestDurable

FrontierPublicationInvariant ==
  frontierPublished =>
    /\ finalityDurable
    /\ manifestDurable
    /\ sidecarsDurable

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
  /\ FrontierPublicationInvariant
  /\ (startupRepairRequired =>
       /\ startupRepairCompleted
       /\ ~durableApplicationLost)

MLNativeLatestIndexExact ==
  latestIndexPublished =>
    /\ latestIndexExact
    /\ ~latestIndexAmbiguous
    /\ latestIndexBounded

NativeApplicationEvidenceSafetyInvariant ==
  /\ NativeEvidenceTypeInvariant
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
