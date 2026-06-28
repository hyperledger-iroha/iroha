---- MODULE SumeragiEngineCommitQcPendingFetchGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for missing-payload Commit-QC pending state.

This slice models the missing-payload branch in
`ConsensusEngine::on_commit_qc(...)`. When a current-context Commit QC reaches
the handler and the certified payload is not available, the engine must:
- set `state.pending_finality` to the certified subject,
- insert the cloned certificate into the pending certificate map keyed by the
  certified block hash, and
- emit `FetchPayload` with the certificate round, certified block hash, and
  certified payload hash.

Payload-available Commit QCs, shared-prefilter rejections, and
pending-finality replay/conflict returns must not create new pending entries or
fetch requests. Replay/conflict returns must preserve the already pending
subject and certificate-map entry.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipPendingState,
  \* @type: Bool;
  BugSkipPendingMapInsert,
  \* @type: Bool;
  BugPendingMapKeyUsesPayloadHash,
  \* @type: Bool;
  BugPendingMapKeyUsesParentHash,
  \* @type: Bool;
  BugPendingMapStoresWrongCertificate,
  \* @type: Bool;
  BugSkipFetchRequest,
  \* @type: Bool;
  BugFetchWrongRound,
  \* @type: Bool;
  BugFetchWrongBlockHash,
  \* @type: Bool;
  BugFetchWrongPayloadHash,
  \* @type: Bool;
  BugPendingOnPayloadAvailable,
  \* @type: Bool;
  BugFetchOnPayloadAvailable,
  \* @type: Bool;
  BugPendingOnRejected,
  \* @type: Bool;
  BugFetchOnRejected,
  \* @type: Bool;
  BugPendingOnReplayConflict,
  \* @type: Bool;
  BugFetchOnReplayConflict

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "safe_missing_payload",
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
  "stale_view",
  "committed_height"
}

ReplayConflictCases == {"pending_replay", "pending_conflict"}

FieldValues == {
  "none",
  "block_a",
  "block_b",
  "parent_a",
  "payload_a",
  "payload_b",
  "round_current",
  "round_other",
  "cert_a",
  "cert_b"
}

SubjectBlock(candidate) ==
  IF candidate = "pending_conflict" THEN "block_b" ELSE "block_a"

SubjectParent(candidate) ==
  IF candidate = "pending_conflict" THEN "parent_b" ELSE "parent_a"

SubjectPayload(candidate) ==
  IF candidate = "pending_conflict" THEN "payload_b" ELSE "payload_a"

CertificateRound(candidate) ==
  IF candidate \in RejectedPrefilterCases
  THEN "round_other"
  ELSE "round_current"

CertificateValue(candidate) ==
  IF SubjectBlock(candidate) = "block_b" THEN "cert_b" ELSE "cert_a"

InitialPendingSubject(candidate) ==
  IF candidate \in ReplayConflictCases THEN "block_a" ELSE "none"

InitialPendingMapKey(candidate) ==
  IF candidate \in ReplayConflictCases THEN "block_a" ELSE "none"

InitialPendingMapCert(candidate) ==
  IF candidate \in ReplayConflictCases THEN "cert_a" ELSE "none"

MissingPayloadCommitQc(candidate) ==
  candidate = "safe_missing_payload"

SpecPendingSubject(candidate) ==
  IF MissingPayloadCommitQc(candidate)
  THEN SubjectBlock(candidate)
  ELSE InitialPendingSubject(candidate)

SpecPendingMapKey(candidate) ==
  IF MissingPayloadCommitQc(candidate)
  THEN SubjectBlock(candidate)
  ELSE InitialPendingMapKey(candidate)

SpecPendingMapCert(candidate) ==
  IF MissingPayloadCommitQc(candidate)
  THEN CertificateValue(candidate)
  ELSE InitialPendingMapCert(candidate)

SpecFetchRound(candidate) ==
  IF MissingPayloadCommitQc(candidate)
  THEN CertificateRound(candidate)
  ELSE "none"

SpecFetchBlock(candidate) ==
  IF MissingPayloadCommitQc(candidate)
  THEN SubjectBlock(candidate)
  ELSE "none"

SpecFetchPayload(candidate) ==
  IF MissingPayloadCommitQc(candidate)
  THEN SubjectPayload(candidate)
  ELSE "none"

WrongPendingSubject(candidate) ==
  IF SubjectBlock(candidate) = "block_a" THEN "block_b" ELSE "block_a"

ImplementationCreatesPendingOnNoop(candidate) ==
  \/ /\ candidate = "safe_payload_available"
     /\ BugPendingOnPayloadAvailable
  \/ /\ candidate \in RejectedPrefilterCases
     /\ BugPendingOnRejected
  \/ /\ candidate \in ReplayConflictCases
     /\ BugPendingOnReplayConflict

ImplementationFetchesOnNoop(candidate) ==
  \/ /\ candidate = "safe_payload_available"
     /\ BugFetchOnPayloadAvailable
  \/ /\ candidate \in RejectedPrefilterCases
     /\ BugFetchOnRejected
  \/ /\ candidate \in ReplayConflictCases
     /\ BugFetchOnReplayConflict

ImplementationPendingSubject(candidate) ==
  IF MissingPayloadCommitQc(candidate)
  THEN
    IF BugSkipPendingState
    THEN InitialPendingSubject(candidate)
    ELSE SubjectBlock(candidate)
  ELSE IF ImplementationCreatesPendingOnNoop(candidate)
       THEN SubjectBlock(candidate)
       ELSE InitialPendingSubject(candidate)

ImplementationPendingMapKey(candidate) ==
  IF MissingPayloadCommitQc(candidate)
  THEN
    IF BugSkipPendingMapInsert
    THEN InitialPendingMapKey(candidate)
    ELSE IF BugPendingMapKeyUsesPayloadHash
         THEN SubjectPayload(candidate)
         ELSE IF BugPendingMapKeyUsesParentHash
              THEN SubjectParent(candidate)
              ELSE SubjectBlock(candidate)
  ELSE IF ImplementationCreatesPendingOnNoop(candidate)
       THEN SubjectBlock(candidate)
       ELSE InitialPendingMapKey(candidate)

ImplementationPendingMapCert(candidate) ==
  IF MissingPayloadCommitQc(candidate)
  THEN
    IF BugSkipPendingMapInsert
    THEN InitialPendingMapCert(candidate)
    ELSE IF BugPendingMapStoresWrongCertificate
         THEN IF CertificateValue(candidate) = "cert_a" THEN "cert_b" ELSE "cert_a"
         ELSE CertificateValue(candidate)
  ELSE IF ImplementationCreatesPendingOnNoop(candidate)
       THEN CertificateValue(candidate)
       ELSE InitialPendingMapCert(candidate)

ImplementationFetchRound(candidate) ==
  IF MissingPayloadCommitQc(candidate)
  THEN
    IF BugSkipFetchRequest
    THEN "none"
    ELSE IF BugFetchWrongRound
         THEN "round_other"
         ELSE CertificateRound(candidate)
  ELSE IF ImplementationFetchesOnNoop(candidate)
       THEN CertificateRound(candidate)
       ELSE "none"

ImplementationFetchBlock(candidate) ==
  IF MissingPayloadCommitQc(candidate)
  THEN
    IF BugSkipFetchRequest
    THEN "none"
    ELSE IF BugFetchWrongBlockHash
         THEN WrongPendingSubject(candidate)
         ELSE SubjectBlock(candidate)
  ELSE IF ImplementationFetchesOnNoop(candidate)
       THEN SubjectBlock(candidate)
       ELSE "none"

ImplementationFetchPayload(candidate) ==
  IF MissingPayloadCommitQc(candidate)
  THEN
    IF BugSkipFetchRequest
    THEN "none"
    ELSE IF BugFetchWrongPayloadHash
         THEN IF SubjectPayload(candidate) = "payload_a" THEN "payload_b" ELSE "payload_a"
         ELSE SubjectPayload(candidate)
  ELSE IF ImplementationFetchesOnNoop(candidate)
       THEN SubjectPayload(candidate)
       ELSE "none"

TypeInvariant ==
  /\ BugSkipPendingState \in BOOLEAN
  /\ BugSkipPendingMapInsert \in BOOLEAN
  /\ BugPendingMapKeyUsesPayloadHash \in BOOLEAN
  /\ BugPendingMapKeyUsesParentHash \in BOOLEAN
  /\ BugPendingMapStoresWrongCertificate \in BOOLEAN
  /\ BugSkipFetchRequest \in BOOLEAN
  /\ BugFetchWrongRound \in BOOLEAN
  /\ BugFetchWrongBlockHash \in BOOLEAN
  /\ BugFetchWrongPayloadHash \in BOOLEAN
  /\ BugPendingOnPayloadAvailable \in BOOLEAN
  /\ BugFetchOnPayloadAvailable \in BOOLEAN
  /\ BugPendingOnRejected \in BOOLEAN
  /\ BugFetchOnRejected \in BOOLEAN
  /\ BugPendingOnReplayConflict \in BOOLEAN
  /\ BugFetchOnReplayConflict \in BOOLEAN
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

PendingSubjectMatchesSpec ==
  \A candidate \in tried:
    ImplementationPendingSubject(candidate) = SpecPendingSubject(candidate)

PendingMapKeyMatchesSpec ==
  \A candidate \in tried:
    ImplementationPendingMapKey(candidate) = SpecPendingMapKey(candidate)

PendingMapCertificateMatchesSpec ==
  \A candidate \in tried:
    ImplementationPendingMapCert(candidate) = SpecPendingMapCert(candidate)

FetchRoundMatchesSpec ==
  \A candidate \in tried:
    ImplementationFetchRound(candidate) = SpecFetchRound(candidate)

FetchBlockMatchesSpec ==
  \A candidate \in tried:
    ImplementationFetchBlock(candidate) = SpecFetchBlock(candidate)

FetchPayloadMatchesSpec ==
  \A candidate \in tried:
    ImplementationFetchPayload(candidate) = SpecFetchPayload(candidate)

MissingPayloadRecordsPendingStateAndMap ==
  "safe_missing_payload" \in tried =>
    /\ ImplementationPendingSubject("safe_missing_payload") = "block_a"
    /\ ImplementationPendingMapKey("safe_missing_payload") = "block_a"
    /\ ImplementationPendingMapCert("safe_missing_payload") = "cert_a"

MissingPayloadFetchesExactRequest ==
  "safe_missing_payload" \in tried =>
    /\ ImplementationFetchRound("safe_missing_payload") = "round_current"
    /\ ImplementationFetchBlock("safe_missing_payload") = "block_a"
    /\ ImplementationFetchPayload("safe_missing_payload") = "payload_a"

PendingMapKeyIsCertifiedBlockHash ==
  "safe_missing_payload" \in tried =>
    ImplementationPendingMapKey("safe_missing_payload") =
      SubjectBlock("safe_missing_payload")

PendingMapStoresClonedCertificate ==
  "safe_missing_payload" \in tried =>
    ImplementationPendingMapCert("safe_missing_payload") =
      CertificateValue("safe_missing_payload")

PayloadAvailableCommitQcDoesNotFetchOrStage ==
  "safe_payload_available" \in tried =>
    /\ ImplementationPendingSubject("safe_payload_available") = "none"
    /\ ImplementationPendingMapKey("safe_payload_available") = "none"
    /\ ImplementationPendingMapCert("safe_payload_available") = "none"
    /\ ImplementationFetchRound("safe_payload_available") = "none"
    /\ ImplementationFetchBlock("safe_payload_available") = "none"
    /\ ImplementationFetchPayload("safe_payload_available") = "none"

RejectedCommitQcsDoNotFetchOrStage ==
  \A candidate \in tried:
    candidate \in RejectedPrefilterCases =>
      /\ ImplementationPendingSubject(candidate) = InitialPendingSubject(candidate)
      /\ ImplementationPendingMapKey(candidate) = InitialPendingMapKey(candidate)
      /\ ImplementationPendingMapCert(candidate) = InitialPendingMapCert(candidate)
      /\ ImplementationFetchRound(candidate) = "none"
      /\ ImplementationFetchBlock(candidate) = "none"
      /\ ImplementationFetchPayload(candidate) = "none"

ReplayConflictPreservesExistingPending ==
  \A candidate \in tried:
    candidate \in ReplayConflictCases =>
      /\ ImplementationPendingSubject(candidate) = "block_a"
      /\ ImplementationPendingMapKey(candidate) = "block_a"
      /\ ImplementationPendingMapCert(candidate) = "cert_a"

ReplayConflictNeverFetches ==
  \A candidate \in tried:
    candidate \in ReplayConflictCases =>
      /\ ImplementationFetchRound(candidate) = "none"
      /\ ImplementationFetchBlock(candidate) = "none"
      /\ ImplementationFetchPayload(candidate) = "none"

NoFetchWithoutMissingPayload ==
  \A candidate \in tried:
    ~MissingPayloadCommitQc(candidate) =>
      /\ ImplementationFetchRound(candidate) = "none"
      /\ ImplementationFetchBlock(candidate) = "none"
      /\ ImplementationFetchPayload(candidate) = "none"

FieldValuesStayInDomain ==
  \A candidate \in tried:
    /\ ImplementationPendingSubject(candidate) \in FieldValues
    /\ ImplementationPendingMapKey(candidate) \in FieldValues
    /\ ImplementationPendingMapCert(candidate) \in FieldValues
    /\ ImplementationFetchRound(candidate) \in FieldValues
    /\ ImplementationFetchBlock(candidate) \in FieldValues
    /\ ImplementationFetchPayload(candidate) \in FieldValues

EngineCommitQcPendingFetchExactness ==
  /\ PendingSubjectMatchesSpec
  /\ PendingMapKeyMatchesSpec
  /\ PendingMapCertificateMatchesSpec
  /\ FetchRoundMatchesSpec
  /\ FetchBlockMatchesSpec
  /\ FetchPayloadMatchesSpec
  /\ MissingPayloadRecordsPendingStateAndMap
  /\ MissingPayloadFetchesExactRequest
  /\ PendingMapKeyIsCertifiedBlockHash
  /\ PendingMapStoresClonedCertificate
  /\ PayloadAvailableCommitQcDoesNotFetchOrStage
  /\ RejectedCommitQcsDoNotFetchOrStage
  /\ ReplayConflictPreservesExistingPending
  /\ ReplayConflictNeverFetches
  /\ NoFetchWithoutMissingPayload
  /\ FieldValuesStayInDomain

Safety ==
  EngineCommitQcPendingFetchExactness

EngineCommitQcPendingFetchCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineCommitQcPendingFetchExactness

SafetyFast == EngineCommitQcPendingFetchExactness

====
