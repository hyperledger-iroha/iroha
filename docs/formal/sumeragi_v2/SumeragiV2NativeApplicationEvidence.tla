---- MODULE SumeragiV2NativeApplicationEvidence ----
EXTENDS Naturals

(***************************************************************************
Bounded durability/publication model for control-only Native AMX participant
application evidence.

The production refinement is source-bound separately to
`Kura::native_amx_participant_application_evidence_for_block_under_publication_guard`,
`Kura::persist_native_amx_participant_application_evidence`,
`Kura::repair_native_amx_participant_application_evidence`,
`Kura::validate_native_amx_participant_application_receipt_artifact`,
`Kura::latest_native_amx_participant_application_receipt_matching`,
`State::native_amx_participant_frontier_marker_payloads`, and
`StateBlock::stage_native_amx_participant_frontiers`.  The finite model does
not prove that those Rust entry points refine this ordering.
***************************************************************************)

CONSTANTS
  \* @type: Str;
  Mode,
  \* @type: Int;
  SourceCount

NativeEvidenceModes ==
  {"Fixed", "PublishFrontierEarly", "PruneWithHashOnly",
   "SeparateSameRouteMarker"}

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
  separateParticipantMarker

vars ==
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

PersistFinalityAndManifest ==
  /\ phase = "Certified"
  /\ phase' = "FinalityDurable"
  /\ finalityDurable' = TRUE
  /\ manifestDurable' = (Mode # "PruneWithHashOnly")
  /\ authenticatedProofAvailable' = (Mode # "PruneWithHashOnly")
  /\ UNCHANGED <<sidecarsDurable, frontierPublished,
                 canonicalWireRetained, sameRouteSettled,
                 separateParticipantMarker>>

PersistExactSidecars ==
  /\ phase = "FinalityDurable"
  /\ finalityDurable
  /\ manifestDurable
  /\ phase' = "SidecarsDurable"
  /\ sidecarsDurable' = TRUE
  /\ UNCHANGED <<finalityDurable, manifestDurable, frontierPublished,
                 canonicalWireRetained, authenticatedProofAvailable,
                 sameRouteSettled, separateParticipantMarker>>

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

SettleSameRouteControl ==
  /\ ~sameRouteSettled
  /\ sameRouteSettled' = TRUE
  /\ separateParticipantMarker' = (Mode = "SeparateSameRouteMarker")
  /\ UNCHANGED <<phase, finalityDurable, manifestDurable,
                 sidecarsDurable, frontierPublished,
                 canonicalWireRetained, authenticatedProofAvailable>>

Next ==
  \/ PersistFinalityAndManifest
  \/ PersistExactSidecars
  \/ PublishReplicatedFrontier
  \/ PruneCanonicalWire
  \/ SettleSameRouteControl

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

NativeApplicationEvidenceSafetyInvariant ==
  /\ NativeEvidenceTypeInvariant
  /\ SidecarsRequireManifestInvariant
  /\ FrontierPublicationInvariant
  /\ PrunedEvidenceVerifiableInvariant
  /\ SameRouteControlOnlyInvariant

NativeEvidenceSpec == Init /\ [][Next]_vars

NativeApplicationEvidenceProductionRefinementObligation ==
  NativeEvidenceSpec => []NativeApplicationEvidenceSafetyInvariant

====
