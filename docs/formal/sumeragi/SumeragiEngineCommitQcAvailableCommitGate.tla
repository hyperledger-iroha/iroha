---- MODULE SumeragiEngineCommitQcAvailableCommitGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for payload-available Commit-QC finality.

This slice models the payload-available branch in
`ConsensusEngine::on_commit_qc(...)`. When a current-context Commit QC reaches
the handler and the certified payload is already locally available, the engine
must commit exactly the certified subject at the current height, clear
validation ownership, return to proposal phase, emit exactly one
`CommitBlock { subject: certificate.subject }`, and avoid pending/fetch side
effects.

Shared-prefilter rejections, already committed heights, and pending-finality
replay/conflict returns must not commit or emit finality. Pending-finality
replay/conflict returns still clear validation because they enter
`on_commit_qc(...)` before the pending-finality guard.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipCommitRecord,
  \* @type: Bool;
  BugCommitWrongHeight,
  \* @type: Bool;
  BugCommitWrongBlock,
  \* @type: Bool;
  BugKeepValidationAfterCommit,
  \* @type: Bool;
  BugWrongPhaseAfterCommit,
  \* @type: Bool;
  BugSkipCommitBlockOutput,
  \* @type: Bool;
  BugOutputWrongParent,
  \* @type: Bool;
  BugOutputWrongBlock,
  \* @type: Bool;
  BugOutputWrongPayload,
  \* @type: Bool;
  BugFetchDespitePayloadAvailable,
  \* @type: Bool;
  BugPendingDespitePayloadAvailable,
  \* @type: Bool;
  BugCommitOnRejected,
  \* @type: Bool;
  BugCommitOnReplayConflict,
  \* @type: Bool;
  BugOverwriteCommittedHeight

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "safe_payload_available",
  "wrong_height",
  "wrong_epoch",
  "wrong_validator_set",
  "wrong_quorum_policy",
  "stale_view",
  "committed_height",
  "pending_replay",
  "pending_conflict"
}

RejectedPrefilterCases == {
  "wrong_height",
  "wrong_epoch",
  "wrong_validator_set",
  "wrong_quorum_policy",
  "stale_view"
}

ReplayConflictCases == {"pending_replay", "pending_conflict"}

HandlerCases == {"safe_payload_available"} \cup ReplayConflictCases

FieldValues == {
  "none",
  "height_current",
  "height_other",
  "parent_a",
  "parent_b",
  "block_a",
  "block_b",
  "block_existing",
  "payload_a",
  "payload_b",
  "cert_a",
  "cert_b",
  "phase_commit",
  "phase_pending_finality",
  "phase_proposal"
}

SubjectParent(candidate) ==
  IF candidate = "pending_conflict" THEN "parent_b" ELSE "parent_a"

SubjectBlock(candidate) ==
  IF candidate \in {"pending_conflict", "committed_height"}
  THEN "block_b"
  ELSE "block_a"

SubjectPayload(candidate) ==
  IF candidate = "pending_conflict" THEN "payload_b" ELSE "payload_a"

CertificateValue(candidate) ==
  IF SubjectBlock(candidate) = "block_b" THEN "cert_b" ELSE "cert_a"

WrongParent(parent) ==
  IF parent = "parent_a" THEN "parent_b" ELSE "parent_a"

WrongBlock(block) ==
  IF block = "block_a" THEN "block_b" ELSE "block_a"

WrongPayload(payload) ==
  IF payload = "payload_a" THEN "payload_b" ELSE "payload_a"

SafeAvailable(candidate) ==
  candidate = "safe_payload_available"

InitialCommittedHeight(candidate) ==
  IF candidate = "committed_height" THEN "height_current" ELSE "none"

InitialCommittedBlock(candidate) ==
  IF candidate = "committed_height" THEN "block_existing" ELSE "none"

InitialPendingSubject(candidate) ==
  IF candidate \in ReplayConflictCases THEN "block_a" ELSE "none"

InitialPendingMapKey(candidate) ==
  IF candidate \in ReplayConflictCases THEN "block_a" ELSE "none"

InitialPendingMapCert(candidate) ==
  IF candidate \in ReplayConflictCases THEN "cert_a" ELSE "none"

InitialValidation(candidate) ==
  TRUE

InitialPhase(candidate) ==
  IF candidate \in ReplayConflictCases
  THEN "phase_pending_finality"
  ELSE "phase_commit"

SpecCommits(candidate) ==
  SafeAvailable(candidate)

SpecCommittedHeight(candidate) ==
  IF SpecCommits(candidate)
  THEN "height_current"
  ELSE InitialCommittedHeight(candidate)

SpecCommittedBlock(candidate) ==
  IF SpecCommits(candidate)
  THEN SubjectBlock(candidate)
  ELSE InitialCommittedBlock(candidate)

SpecValidationAfter(candidate) ==
  IF candidate \in HandlerCases
  THEN FALSE
  ELSE InitialValidation(candidate)

SpecPhaseAfter(candidate) ==
  IF SpecCommits(candidate)
  THEN "phase_proposal"
  ELSE InitialPhase(candidate)

SpecPendingSubject(candidate) ==
  IF SpecCommits(candidate)
  THEN "none"
  ELSE InitialPendingSubject(candidate)

SpecPendingMapKey(candidate) ==
  IF SpecCommits(candidate)
  THEN "none"
  ELSE InitialPendingMapKey(candidate)

SpecPendingMapCert(candidate) ==
  IF SpecCommits(candidate)
  THEN "none"
  ELSE InitialPendingMapCert(candidate)

SpecOutputParent(candidate) ==
  IF SpecCommits(candidate) THEN SubjectParent(candidate) ELSE "none"

SpecOutputBlock(candidate) ==
  IF SpecCommits(candidate) THEN SubjectBlock(candidate) ELSE "none"

SpecOutputPayload(candidate) ==
  IF SpecCommits(candidate) THEN SubjectPayload(candidate) ELSE "none"

SpecFetchBlock(candidate) ==
  "none"

SpecFetchPayload(candidate) ==
  "none"

ImplementationCommits(candidate) ==
  IF SafeAvailable(candidate)
  THEN ~BugSkipCommitRecord
  ELSE IF candidate = "committed_height"
       THEN BugOverwriteCommittedHeight
       ELSE IF candidate \in RejectedPrefilterCases
            THEN BugCommitOnRejected
            ELSE IF candidate \in ReplayConflictCases
                 THEN BugCommitOnReplayConflict
                 ELSE FALSE

ImplementationCommittedHeight(candidate) ==
  IF ImplementationCommits(candidate)
  THEN
    IF BugCommitWrongHeight
    THEN "height_other"
    ELSE "height_current"
  ELSE InitialCommittedHeight(candidate)

ImplementationCommittedBlock(candidate) ==
  IF ImplementationCommits(candidate)
  THEN
    IF BugCommitWrongBlock
    THEN WrongBlock(SubjectBlock(candidate))
    ELSE SubjectBlock(candidate)
  ELSE InitialCommittedBlock(candidate)

ImplementationValidationAfter(candidate) ==
  IF SafeAvailable(candidate)
  THEN BugKeepValidationAfterCommit
  ELSE IF candidate \in ReplayConflictCases
       THEN FALSE
       ELSE InitialValidation(candidate)

ImplementationPhaseAfter(candidate) ==
  IF SafeAvailable(candidate)
  THEN
    IF BugWrongPhaseAfterCommit
    THEN "phase_commit"
    ELSE "phase_proposal"
  ELSE InitialPhase(candidate)

ImplementationPendingSubject(candidate) ==
  IF SafeAvailable(candidate) /\ BugPendingDespitePayloadAvailable
  THEN SubjectBlock(candidate)
  ELSE InitialPendingSubject(candidate)

ImplementationPendingMapKey(candidate) ==
  IF SafeAvailable(candidate) /\ BugPendingDespitePayloadAvailable
  THEN SubjectBlock(candidate)
  ELSE InitialPendingMapKey(candidate)

ImplementationPendingMapCert(candidate) ==
  IF SafeAvailable(candidate) /\ BugPendingDespitePayloadAvailable
  THEN CertificateValue(candidate)
  ELSE InitialPendingMapCert(candidate)

ImplementationEmitsCommitBlock(candidate) ==
  ImplementationCommits(candidate) /\ ~BugSkipCommitBlockOutput

ImplementationOutputParent(candidate) ==
  IF ImplementationEmitsCommitBlock(candidate)
  THEN
    IF BugOutputWrongParent
    THEN WrongParent(SubjectParent(candidate))
    ELSE SubjectParent(candidate)
  ELSE "none"

ImplementationOutputBlock(candidate) ==
  IF ImplementationEmitsCommitBlock(candidate)
  THEN
    IF BugOutputWrongBlock
    THEN WrongBlock(SubjectBlock(candidate))
    ELSE SubjectBlock(candidate)
  ELSE "none"

ImplementationOutputPayload(candidate) ==
  IF ImplementationEmitsCommitBlock(candidate)
  THEN
    IF BugOutputWrongPayload
    THEN WrongPayload(SubjectPayload(candidate))
    ELSE SubjectPayload(candidate)
  ELSE "none"

ImplementationFetchBlock(candidate) ==
  IF SafeAvailable(candidate) /\ BugFetchDespitePayloadAvailable
  THEN SubjectBlock(candidate)
  ELSE "none"

ImplementationFetchPayload(candidate) ==
  IF SafeAvailable(candidate) /\ BugFetchDespitePayloadAvailable
  THEN SubjectPayload(candidate)
  ELSE "none"

TypeInvariant ==
  /\ BugSkipCommitRecord \in BOOLEAN
  /\ BugCommitWrongHeight \in BOOLEAN
  /\ BugCommitWrongBlock \in BOOLEAN
  /\ BugKeepValidationAfterCommit \in BOOLEAN
  /\ BugWrongPhaseAfterCommit \in BOOLEAN
  /\ BugSkipCommitBlockOutput \in BOOLEAN
  /\ BugOutputWrongParent \in BOOLEAN
  /\ BugOutputWrongBlock \in BOOLEAN
  /\ BugOutputWrongPayload \in BOOLEAN
  /\ BugFetchDespitePayloadAvailable \in BOOLEAN
  /\ BugPendingDespitePayloadAvailable \in BOOLEAN
  /\ BugCommitOnRejected \in BOOLEAN
  /\ BugCommitOnReplayConflict \in BOOLEAN
  /\ BugOverwriteCommittedHeight \in BOOLEAN
  /\ tried \subseteq Cases

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Cases \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Cases: TryCandidate(candidate)
  \/ Stable

CommittedHeightMatchesSpec ==
  \A candidate \in tried:
    ImplementationCommittedHeight(candidate) = SpecCommittedHeight(candidate)

CommittedBlockMatchesSpec ==
  \A candidate \in tried:
    ImplementationCommittedBlock(candidate) = SpecCommittedBlock(candidate)

ValidationMatchesSpec ==
  \A candidate \in tried:
    ImplementationValidationAfter(candidate) = SpecValidationAfter(candidate)

PhaseMatchesSpec ==
  \A candidate \in tried:
    ImplementationPhaseAfter(candidate) = SpecPhaseAfter(candidate)

PendingSubjectMatchesSpec ==
  \A candidate \in tried:
    ImplementationPendingSubject(candidate) = SpecPendingSubject(candidate)

PendingMapKeyMatchesSpec ==
  \A candidate \in tried:
    ImplementationPendingMapKey(candidate) = SpecPendingMapKey(candidate)

PendingMapCertMatchesSpec ==
  \A candidate \in tried:
    ImplementationPendingMapCert(candidate) = SpecPendingMapCert(candidate)

OutputParentMatchesSpec ==
  \A candidate \in tried:
    ImplementationOutputParent(candidate) = SpecOutputParent(candidate)

OutputBlockMatchesSpec ==
  \A candidate \in tried:
    ImplementationOutputBlock(candidate) = SpecOutputBlock(candidate)

OutputPayloadMatchesSpec ==
  \A candidate \in tried:
    ImplementationOutputPayload(candidate) = SpecOutputPayload(candidate)

FetchBlockMatchesSpec ==
  \A candidate \in tried:
    ImplementationFetchBlock(candidate) = SpecFetchBlock(candidate)

FetchPayloadMatchesSpec ==
  \A candidate \in tried:
    ImplementationFetchPayload(candidate) = SpecFetchPayload(candidate)

SafeAvailableCommitsExactSubject ==
  "safe_payload_available" \in tried =>
    /\ ImplementationCommittedHeight("safe_payload_available") = "height_current"
    /\ ImplementationCommittedBlock("safe_payload_available") = "block_a"
    /\ ImplementationOutputParent("safe_payload_available") = "parent_a"
    /\ ImplementationOutputBlock("safe_payload_available") = "block_a"
    /\ ImplementationOutputPayload("safe_payload_available") = "payload_a"

SafeAvailableClearsOwnershipAndReturnsProposal ==
  "safe_payload_available" \in tried =>
    /\ ImplementationValidationAfter("safe_payload_available") = FALSE
    /\ ImplementationPhaseAfter("safe_payload_available") = "phase_proposal"
    /\ ImplementationPendingSubject("safe_payload_available") = "none"
    /\ ImplementationPendingMapKey("safe_payload_available") = "none"
    /\ ImplementationPendingMapCert("safe_payload_available") = "none"

SafeAvailableDoesNotFetch ==
  "safe_payload_available" \in tried =>
    /\ ImplementationFetchBlock("safe_payload_available") = "none"
    /\ ImplementationFetchPayload("safe_payload_available") = "none"

RejectedCommitQcsDoNotCommitOrOutput ==
  \A candidate \in tried:
    candidate \in RejectedPrefilterCases =>
      /\ ImplementationCommittedHeight(candidate) = "none"
      /\ ImplementationCommittedBlock(candidate) = "none"
      /\ ImplementationOutputParent(candidate) = "none"
      /\ ImplementationOutputBlock(candidate) = "none"
      /\ ImplementationOutputPayload(candidate) = "none"

ReplayConflictCommitQcsDoNotCommitOrOutput ==
  \A candidate \in tried:
    candidate \in ReplayConflictCases =>
      /\ ImplementationCommittedHeight(candidate) = "none"
      /\ ImplementationCommittedBlock(candidate) = "none"
      /\ ImplementationOutputParent(candidate) = "none"
      /\ ImplementationOutputBlock(candidate) = "none"
      /\ ImplementationOutputPayload(candidate) = "none"

ReplayConflictClearsValidationAndPreservesPending ==
  \A candidate \in tried:
    candidate \in ReplayConflictCases =>
      /\ ImplementationValidationAfter(candidate) = FALSE
      /\ ImplementationPendingSubject(candidate) = "block_a"
      /\ ImplementationPendingMapKey(candidate) = "block_a"
      /\ ImplementationPendingMapCert(candidate) = "cert_a"

CommittedHeightPreserved ==
  "committed_height" \in tried =>
    /\ ImplementationCommittedHeight("committed_height") = "height_current"
    /\ ImplementationCommittedBlock("committed_height") = "block_existing"
    /\ ImplementationOutputParent("committed_height") = "none"
    /\ ImplementationOutputBlock("committed_height") = "none"
    /\ ImplementationOutputPayload("committed_height") = "none"

NoCommitWithoutPayloadAvailable ==
  \A candidate \in tried:
    ~SafeAvailable(candidate) =>
      ImplementationOutputBlock(candidate) = "none"

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ ImplementationCommittedHeight(candidate) \in FieldValues
    /\ ImplementationCommittedBlock(candidate) \in FieldValues
    /\ ImplementationPhaseAfter(candidate) \in FieldValues
    /\ ImplementationPendingSubject(candidate) \in FieldValues
    /\ ImplementationPendingMapKey(candidate) \in FieldValues
    /\ ImplementationPendingMapCert(candidate) \in FieldValues
    /\ ImplementationOutputParent(candidate) \in FieldValues
    /\ ImplementationOutputBlock(candidate) \in FieldValues
    /\ ImplementationOutputPayload(candidate) \in FieldValues
    /\ ImplementationFetchBlock(candidate) \in FieldValues
    /\ ImplementationFetchPayload(candidate) \in FieldValues

EngineCommitQcAvailableCommitExactness ==
  /\ CommittedHeightMatchesSpec
  /\ CommittedBlockMatchesSpec
  /\ ValidationMatchesSpec
  /\ PhaseMatchesSpec
  /\ PendingSubjectMatchesSpec
  /\ PendingMapKeyMatchesSpec
  /\ PendingMapCertMatchesSpec
  /\ OutputParentMatchesSpec
  /\ OutputBlockMatchesSpec
  /\ OutputPayloadMatchesSpec
  /\ FetchBlockMatchesSpec
  /\ FetchPayloadMatchesSpec
  /\ SafeAvailableCommitsExactSubject
  /\ SafeAvailableClearsOwnershipAndReturnsProposal
  /\ SafeAvailableDoesNotFetch
  /\ RejectedCommitQcsDoNotCommitOrOutput
  /\ ReplayConflictCommitQcsDoNotCommitOrOutput
  /\ ReplayConflictClearsValidationAndPreservesPending
  /\ CommittedHeightPreserved
  /\ NoCommitWithoutPayloadAvailable
  /\ ValuesStayInDomain

Safety ==
  EngineCommitQcAvailableCommitExactness

EngineCommitQcAvailableCommitCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineCommitQcAvailableCommitExactness

SafetyFast == EngineCommitQcAvailableCommitExactness

====
