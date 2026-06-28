---- MODULE SumeragiEngineCommitQcValidationCleanupGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for commit-QC validation cleanup in the pure engine.

This slice models the early `self.validating = None` side effect at the start
of `ConsensusEngine::on_commit_qc(...)`, as reached through
`ConsensusEngine::on_certificate(...)`. A current-context Commit QC that reaches
the handler must clear in-flight proposal-validation ownership before any
pending-finality replay or conflict return. Commit QCs rejected by the shared
certificate prefilter must not clear validation ownership as a side effect.

The cleanup matters because a late invalid validation result for a proposal
must not advance the view after a same-round Commit QC has superseded that
validation owner.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipAcceptedValidationClear,
  \* @type: Bool;
  BugSkipPendingReplayValidationClear,
  \* @type: Bool;
  BugSkipPendingConflictValidationClear,
  \* @type: Bool;
  BugClearWrongContextValidation,
  \* @type: Bool;
  BugClearWrongQuorumPolicyValidation,
  \* @type: Bool;
  BugClearStaleViewValidation,
  \* @type: Bool;
  BugClearCommittedHeightValidation,
  \* @type: Bool;
  BugLateInvalidAdvancesAfterCommitQc

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "safe_payload_available",
  "safe_payload_missing",
  "safe_payload_available_no_validation",
  "pending_replay",
  "pending_conflict",
  "wrong_height",
  "wrong_epoch",
  "wrong_validator_set",
  "wrong_quorum_policy",
  "stale_view",
  "committed_height"
}

HandlerCases == {
  "safe_payload_available",
  "safe_payload_missing",
  "safe_payload_available_no_validation",
  "pending_replay",
  "pending_conflict"
}

AcceptedCommitQcCases == {
  "safe_payload_available",
  "safe_payload_missing",
  "safe_payload_available_no_validation"
}

WrongContextCases == {"wrong_height", "wrong_epoch", "wrong_validator_set"}

RejectedPrefilterCases ==
  Cases \ HandlerCases

InitialValidating(candidate) ==
  candidate # "safe_payload_available_no_validation"

SpecHandlerReached(candidate) ==
  candidate \in HandlerCases

SpecValidationAfter(candidate) ==
  IF SpecHandlerReached(candidate)
  THEN FALSE
  ELSE InitialValidating(candidate)

ImplementationValidationAfter(candidate) ==
  IF candidate \in AcceptedCommitQcCases
  THEN
    IF BugSkipAcceptedValidationClear
    THEN InitialValidating(candidate)
    ELSE FALSE
  ELSE IF candidate = "pending_replay"
       THEN
         IF BugSkipPendingReplayValidationClear
         THEN InitialValidating(candidate)
         ELSE FALSE
       ELSE IF candidate = "pending_conflict"
            THEN
              IF BugSkipPendingConflictValidationClear
              THEN InitialValidating(candidate)
              ELSE FALSE
            ELSE IF candidate \in WrongContextCases
                 THEN
                   IF BugClearWrongContextValidation
                   THEN FALSE
                   ELSE InitialValidating(candidate)
                 ELSE IF candidate = "wrong_quorum_policy"
                      THEN
                        IF BugClearWrongQuorumPolicyValidation
                        THEN FALSE
                        ELSE InitialValidating(candidate)
                      ELSE IF candidate = "stale_view"
                           THEN
                             IF BugClearStaleViewValidation
                             THEN FALSE
                             ELSE InitialValidating(candidate)
                           ELSE IF candidate = "committed_height"
                                THEN
                                  IF BugClearCommittedHeightValidation
                                  THEN FALSE
                                  ELSE InitialValidating(candidate)
                                ELSE InitialValidating(candidate)

ImplementationLateInvalidAdvances(candidate) ==
  IF /\ BugLateInvalidAdvancesAfterCommitQc
     /\ SpecHandlerReached(candidate)
     /\ InitialValidating(candidate)
  THEN TRUE
  ELSE ImplementationValidationAfter(candidate)

TypeInvariant ==
  /\ BugSkipAcceptedValidationClear \in BOOLEAN
  /\ BugSkipPendingReplayValidationClear \in BOOLEAN
  /\ BugSkipPendingConflictValidationClear \in BOOLEAN
  /\ BugClearWrongContextValidation \in BOOLEAN
  /\ BugClearWrongQuorumPolicyValidation \in BOOLEAN
  /\ BugClearStaleViewValidation \in BOOLEAN
  /\ BugClearCommittedHeightValidation \in BOOLEAN
  /\ BugLateInvalidAdvancesAfterCommitQc \in BOOLEAN
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

ValidationAfterMatchesSpec ==
  \A candidate \in tried:
    ImplementationValidationAfter(candidate) = SpecValidationAfter(candidate)

HandlerCommitQcsClearValidation ==
  \A candidate \in tried:
    /\ candidate \in HandlerCases
    /\ InitialValidating(candidate)
    => ~ImplementationValidationAfter(candidate)

AcceptedCommitQcsClearValidation ==
  \A candidate \in tried:
    /\ candidate \in AcceptedCommitQcCases
    /\ InitialValidating(candidate)
    => ~ImplementationValidationAfter(candidate)

PendingReplayClearsValidationBeforeReturn ==
  "pending_replay" \in tried => ~ImplementationValidationAfter("pending_replay")

PendingConflictClearsValidationBeforeReturn ==
  "pending_conflict" \in tried => ~ImplementationValidationAfter("pending_conflict")

RejectedPrefilterCommitQcsPreserveValidation ==
  \A candidate \in tried:
    candidate \in RejectedPrefilterCases
      => ImplementationValidationAfter(candidate) = InitialValidating(candidate)

WrongContextCommitQcsPreserveValidation ==
  \A candidate \in tried:
    candidate \in WrongContextCases
      => ImplementationValidationAfter(candidate) = InitialValidating(candidate)

WrongQuorumPolicyPreservesValidation ==
  "wrong_quorum_policy" \in tried
    => ImplementationValidationAfter("wrong_quorum_policy") =
        InitialValidating("wrong_quorum_policy")

StaleViewPreservesValidation ==
  "stale_view" \in tried
    => ImplementationValidationAfter("stale_view") = InitialValidating("stale_view")

CommittedHeightPreservesValidation ==
  "committed_height" \in tried
    => ImplementationValidationAfter("committed_height") =
        InitialValidating("committed_height")

NoSyntheticValidationOwnership ==
  \A candidate \in tried:
    ~InitialValidating(candidate) => ~ImplementationValidationAfter(candidate)

LateInvalidResultSuppressedAfterHandlerCommitQc ==
  \A candidate \in tried:
    /\ SpecHandlerReached(candidate)
    /\ InitialValidating(candidate)
    => ~ImplementationLateInvalidAdvances(candidate)

EngineCommitQcValidationCleanupExactness ==
  /\ ValidationAfterMatchesSpec
  /\ HandlerCommitQcsClearValidation
  /\ AcceptedCommitQcsClearValidation
  /\ PendingReplayClearsValidationBeforeReturn
  /\ PendingConflictClearsValidationBeforeReturn
  /\ RejectedPrefilterCommitQcsPreserveValidation
  /\ WrongContextCommitQcsPreserveValidation
  /\ WrongQuorumPolicyPreservesValidation
  /\ StaleViewPreservesValidation
  /\ CommittedHeightPreservesValidation
  /\ NoSyntheticValidationOwnership
  /\ LateInvalidResultSuppressedAfterHandlerCommitQc

Safety ==
  EngineCommitQcValidationCleanupExactness

EngineCommitQcValidationCleanupCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineCommitQcValidationCleanupExactness

SafetyFast == EngineCommitQcValidationCleanupExactness

====
