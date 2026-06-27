---- MODULE SumeragiStalledPendingTimeoutDecisionGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for stalled pending-block timeout decisions.

This slice pins `stalled_pending_timeout_decision(...)`. The helper derives a
base timeout from the commit quorum timeout with a one-millisecond floor, caps
the near-quorum payload timeout by that base timeout, classifies near-quorum
missing-payload repair before active recovery backlog, and otherwise falls back
to the base quorum timeout. Commit-pipeline backlog requires valid pending work
plus recovery evidence from an observed commit QC, a validated commit artifact,
near-quorum votes, or active commit-QC repair. The decision also returns the
observed vote count, minimum commit votes, missing-local-data flag, and
same-block recovery flag.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

BaseClass == "base_quorum"
NearClass == "near_quorum_payload_missing_fast"
ActiveClass == "active_recovery_backlog"

Cases == {
  "base_zero_quorum",
  "base_normal",
  "near_fast",
  "near_capped_by_base",
  "near_zero_votes",
  "near_already_quorum",
  "near_payload_present",
  "near_da_disabled",
  "near_gate_closed_no_recovery",
  "same_block_recovery",
  "near_with_worker_recovery",
  "worker_recovery",
  "residual_recovery",
  "unresolved_rbc_recovery",
  "validation_recovery",
  "validation_inflight_not_pending",
  "commit_qc_repair",
  "commit_pipeline_queue",
  "commit_pipeline_commit_qc_observed",
  "commit_pipeline_validated_artifact",
  "invalid_commit_pipeline",
  "queue_without_evidence"
}

Min(a, b) == IF a <= b THEN a ELSE b
Max(a, b) == IF a >= b THEN a ELSE b

\* @type: Str => Int;
QuorumTimeout(c) ==
  CASE c = "base_zero_quorum" -> 0
    [] c = "near_capped_by_base" -> 100
    [] OTHER -> 1000

\* @type: Str => Int;
SpecBaseTimeout(c) == Max(QuorumTimeout(c), 1)

\* @type: Str => Int;
NearRawTimeout(c) ==
  CASE c = "near_capped_by_base" -> 500
    [] OTHER -> 200

\* @type: Str => Int;
SpecNearTimeout(c) == Min(NearRawTimeout(c), SpecBaseTimeout(c))

\* @type: Str => Int;
FrontierPendingTimeout(c) ==
  CASE c = "near_with_worker_recovery" -> 3000
    [] OTHER -> 2500

\* @type: Str => Int;
MinVotesForCommit(c) == 3

\* @type: Str => Int;
VoteCount(c) ==
  CASE c = "near_zero_votes" -> 0
    [] c = "near_already_quorum" -> 3
    [] c \in {
         "near_fast",
         "near_capped_by_base",
         "near_payload_present",
         "near_da_disabled",
         "near_gate_closed_no_recovery",
         "same_block_recovery",
         "near_with_worker_recovery"
       } -> 2
    [] OTHER -> 0

\* @type: Str => Bool;
DaEnabled(c) == c /= "near_da_disabled"

\* @type: Str => Bool;
PayloadAvailable(c) == c = "near_payload_present"

\* @type: Str => Bool;
SpecMissingLocalData(c) == DaEnabled(c) /\ ~PayloadAvailable(c)

\* @type: Str => Bool;
SpecNearCommitQuorum(c) ==
  /\ VoteCount(c) > 0
  /\ VoteCount(c) < MinVotesForCommit(c)
  /\ VoteCount(c) + 1 >= MinVotesForCommit(c)

\* @type: Str => Bool;
NearQuorumGateOpen(c) == c /= "near_gate_closed_no_recovery"

\* @type: Str => Bool;
SameBlockRecoveryActive(c) == c = "same_block_recovery"

\* @type: Str => Bool;
WorkerRecoveryBacklog(c) == c \in {"near_with_worker_recovery", "worker_recovery"}

\* @type: Str => Bool;
ResidualRoundBacklog(c) == c = "residual_recovery"

\* @type: Str => Bool;
UnresolvedRbcBacklog(c) == c = "unresolved_rbc_recovery"

\* @type: Str => Bool;
ValidationStatusPending(c) == c = "validation_recovery"

\* @type: Str => Bool;
ValidationInflight(c) == c \in {"validation_recovery", "validation_inflight_not_pending"}

\* @type: Str => Bool;
CommitQcRepairActive(c) == c = "commit_qc_repair"

\* @type: Str => Bool;
QueueActiveBacklog(c) ==
  c \in {
    "commit_pipeline_queue",
    "commit_pipeline_commit_qc_observed",
    "commit_pipeline_validated_artifact",
    "invalid_commit_pipeline",
    "queue_without_evidence"
  }

\* @type: Str => Bool;
PendingInvalid(c) == c = "invalid_commit_pipeline"

\* @type: Str => Bool;
CommitQcObserved(c) ==
  c \in {"commit_pipeline_queue", "commit_pipeline_commit_qc_observed"}

\* @type: Str => Bool;
ValidatedCommitArtifact(c) == c = "commit_pipeline_validated_artifact"

\* @type: Str => Bool;
CommitPipelineEvidence(c) ==
  \/ c = "invalid_commit_pipeline"
  \/ CommitQcObserved(c)
  \/ ValidatedCommitArtifact(c)

\* @type: Str => Bool;
SpecValidationRecoveryActive(c) ==
  ValidationStatusPending(c) /\ ValidationInflight(c)

\* @type: Str => Bool;
SpecCommitPipelineBacklogActive(c) ==
  /\ QueueActiveBacklog(c)
  /\ ~PendingInvalid(c)
  /\ (CommitPipelineEvidence(c) \/ SpecNearCommitQuorum(c) \/ CommitQcRepairActive(c))

\* @type: Str => Bool;
SpecNearQuorumFastTimeoutAllowed(c) ==
  /\ SpecNearCommitQuorum(c)
  /\ SpecMissingLocalData(c)
  /\ NearQuorumGateOpen(c)
  /\ ~SameBlockRecoveryActive(c)

\* @type: Str => Bool;
SpecRecoveryBacklogActive(c) ==
  \/ WorkerRecoveryBacklog(c)
  \/ ResidualRoundBacklog(c)
  \/ UnresolvedRbcBacklog(c)
  \/ SpecValidationRecoveryActive(c)
  \/ SpecCommitPipelineBacklogActive(c)
  \/ CommitQcRepairActive(c)
  \/ SameBlockRecoveryActive(c)

\* @type: Str => Str;
SpecClass(c) ==
  IF SpecNearQuorumFastTimeoutAllowed(c)
  THEN NearClass
  ELSE IF SpecRecoveryBacklogActive(c)
       THEN ActiveClass
       ELSE BaseClass

\* @type: Str => Int;
SpecTimeout(c) ==
  CASE SpecClass(c) = BaseClass -> SpecBaseTimeout(c)
    [] SpecClass(c) = NearClass -> SpecNearTimeout(c)
    [] SpecClass(c) = ActiveClass -> FrontierPendingTimeout(c)
    [] OTHER -> SpecBaseTimeout(c)

\* @type: Str => Bool;
ActualNearCommitQuorum(c) ==
  CASE Bug = "near_allows_zero_votes"
       /\ c = "near_zero_votes" -> TRUE
    [] Bug = "near_allows_existing_quorum"
       /\ c = "near_already_quorum" -> TRUE
    [] OTHER -> SpecNearCommitQuorum(c)

\* @type: Str => Bool;
ActualMissingLocalData(c) ==
  CASE Bug = "near_ignores_payload_presence"
       /\ c = "near_payload_present" -> TRUE
    [] Bug = "near_ignores_da_disabled"
       /\ c = "near_da_disabled" -> TRUE
    [] OTHER -> SpecMissingLocalData(c)

\* @type: Str => Bool;
ActualNearGateOpen(c) ==
  CASE Bug = "near_ignores_gate"
       /\ c = "near_gate_closed_no_recovery" -> TRUE
    [] OTHER -> NearQuorumGateOpen(c)

\* @type: Str => Bool;
ActualSameBlockRecoveryActive(c) == SameBlockRecoveryActive(c)

\* @type: Str => Bool;
ActualNearQuorumFastTimeoutAllowed(c) ==
  CASE Bug = "same_block_does_not_block_near"
       /\ c = "same_block_recovery" ->
          /\ ActualNearCommitQuorum(c)
          /\ ActualMissingLocalData(c)
          /\ ActualNearGateOpen(c)
    [] OTHER ->
          /\ ActualNearCommitQuorum(c)
          /\ ActualMissingLocalData(c)
          /\ ActualNearGateOpen(c)
          /\ ~ActualSameBlockRecoveryActive(c)

\* @type: Str => Bool;
ActualValidationRecoveryActive(c) ==
  CASE Bug = "validation_inflight_without_pending"
       /\ c = "validation_inflight_not_pending" -> ValidationInflight(c)
    [] OTHER -> SpecValidationRecoveryActive(c)

\* @type: Str => Bool;
ActualCommitPipelineBacklogActive(c) ==
  CASE Bug = "invalid_commit_pipeline_backlog"
       /\ c = "invalid_commit_pipeline" ->
          QueueActiveBacklog(c) /\ CommitPipelineEvidence(c)
    [] Bug = "queue_without_evidence_active"
       /\ c = "queue_without_evidence" -> QueueActiveBacklog(c)
    [] Bug = "skip_commit_qc_observed_backlog"
       /\ c = "commit_pipeline_commit_qc_observed" -> FALSE
    [] Bug = "skip_validated_artifact_backlog"
       /\ c = "commit_pipeline_validated_artifact" -> FALSE
    [] OTHER -> SpecCommitPipelineBacklogActive(c)

\* @type: Str => Bool;
ActualRecoveryBacklogActive(c) ==
  CASE Bug = "skip_worker_recovery"
       /\ c = "worker_recovery" -> FALSE
    [] Bug = "skip_residual_recovery"
       /\ c = "residual_recovery" -> FALSE
    [] Bug = "skip_unresolved_rbc_recovery"
       /\ c = "unresolved_rbc_recovery" -> FALSE
    [] Bug = "skip_commit_qc_repair"
       /\ c = "commit_qc_repair" -> FALSE
    [] OTHER ->
          \/ WorkerRecoveryBacklog(c)
          \/ ResidualRoundBacklog(c)
          \/ UnresolvedRbcBacklog(c)
          \/ ActualValidationRecoveryActive(c)
          \/ ActualCommitPipelineBacklogActive(c)
          \/ CommitQcRepairActive(c)
          \/ ActualSameBlockRecoveryActive(c)

\* @type: Str => Str;
ActualClass(c) ==
  CASE Bug = "recovery_preempts_near"
       /\ c = "near_with_worker_recovery"
       /\ ActualRecoveryBacklogActive(c) -> ActiveClass
    [] ActualNearQuorumFastTimeoutAllowed(c) -> NearClass
    [] ActualRecoveryBacklogActive(c) -> ActiveClass
    [] OTHER -> BaseClass

\* @type: Str => Int;
ActualBaseTimeout(c) ==
  CASE Bug = "base_timeout_no_floor"
       /\ c = "base_zero_quorum" -> QuorumTimeout(c)
    [] OTHER -> SpecBaseTimeout(c)

\* @type: Str => Int;
ActualNearTimeout(c) ==
  CASE Bug = "near_timeout_not_capped"
       /\ c = "near_capped_by_base" -> NearRawTimeout(c)
    [] OTHER -> SpecNearTimeout(c)

\* @type: Str => Int;
ActualTimeout(c) ==
  CASE Bug = "near_uses_base_timeout"
       /\ c = "near_fast" -> ActualBaseTimeout(c)
    [] Bug = "active_uses_base_timeout"
       /\ c = "worker_recovery" -> ActualBaseTimeout(c)
    [] ActualClass(c) = BaseClass -> ActualBaseTimeout(c)
    [] ActualClass(c) = NearClass -> ActualNearTimeout(c)
    [] ActualClass(c) = ActiveClass -> FrontierPendingTimeout(c)
    [] OTHER -> ActualBaseTimeout(c)

\* @type: Str => <<Str, Int, Int, Int, Bool, Bool>>;
SpecDecision(c) ==
  <<SpecClass(c), SpecTimeout(c), VoteCount(c), MinVotesForCommit(c),
    SpecMissingLocalData(c), SameBlockRecoveryActive(c)>>

\* @type: Str => <<Str, Int, Int, Int, Bool, Bool>>;
ActualDecision(c) ==
  <<ActualClass(c), ActualTimeout(c), VoteCount(c), MinVotesForCommit(c),
    ActualMissingLocalData(c), ActualSameBlockRecoveryActive(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "base_timeout_no_floor",
       "near_timeout_not_capped",
       "near_uses_base_timeout",
       "active_uses_base_timeout",
       "near_allows_zero_votes",
       "near_allows_existing_quorum",
       "near_ignores_payload_presence",
       "near_ignores_da_disabled",
       "near_ignores_gate",
       "same_block_does_not_block_near",
       "recovery_preempts_near",
       "skip_worker_recovery",
       "skip_residual_recovery",
       "skip_unresolved_rbc_recovery",
       "skip_commit_qc_repair",
       "invalid_commit_pipeline_backlog",
       "queue_without_evidence_active",
       "validation_inflight_without_pending",
       "skip_commit_qc_observed_backlog",
       "skip_validated_artifact_backlog"
     }
  /\ checked = 0

DecisionMatchesSpec ==
  \A c \in Cases:
    ActualDecision(c) = SpecDecision(c)

BaseTimeoutMatchesSpec ==
  \A c \in Cases:
    ActualBaseTimeout(c) = SpecBaseTimeout(c)

NearTimeoutMatchesSpec ==
  \A c \in Cases:
    ActualNearTimeout(c) = SpecNearTimeout(c)

NearCommitQuorumMatchesSpec ==
  \A c \in Cases:
    ActualNearCommitQuorum(c) = SpecNearCommitQuorum(c)

MissingLocalDataMatchesSpec ==
  \A c \in Cases:
    ActualMissingLocalData(c) = SpecMissingLocalData(c)

NearFastGateMatchesSpec ==
  \A c \in Cases:
    ActualNearQuorumFastTimeoutAllowed(c) =
      SpecNearQuorumFastTimeoutAllowed(c)

ValidationRecoveryMatchesSpec ==
  \A c \in Cases:
    ActualValidationRecoveryActive(c) = SpecValidationRecoveryActive(c)

CommitPipelineBacklogMatchesSpec ==
  \A c \in Cases:
    ActualCommitPipelineBacklogActive(c) =
      SpecCommitPipelineBacklogActive(c)

RecoveryBacklogMatchesSpec ==
  \A c \in Cases:
    ActualRecoveryBacklogActive(c) = SpecRecoveryBacklogActive(c)

ClassMatchesSpec ==
  \A c \in Cases:
    ActualClass(c) = SpecClass(c)

TimeoutMatchesSpec ==
  \A c \in Cases:
    ActualTimeout(c) = SpecTimeout(c)

BaseTimeoutAnchors ==
  /\ SpecBaseTimeout("base_zero_quorum") = 1
  /\ SpecBaseTimeout("base_normal") = 1000
  /\ SpecBaseTimeout("near_capped_by_base") = 100

NearTimeoutAnchors ==
  /\ SpecNearTimeout("near_fast") = 200
  /\ SpecNearTimeout("near_capped_by_base") = 100

NearCommitQuorumAnchors ==
  /\ SpecNearCommitQuorum("near_fast") = TRUE
  /\ SpecNearCommitQuorum("near_zero_votes") = FALSE
  /\ SpecNearCommitQuorum("near_already_quorum") = FALSE

MissingLocalDataAnchors ==
  /\ SpecMissingLocalData("near_fast") = TRUE
  /\ SpecMissingLocalData("near_payload_present") = FALSE
  /\ SpecMissingLocalData("near_da_disabled") = FALSE

RecoveryBacklogAnchors ==
  /\ SpecRecoveryBacklogActive("worker_recovery") = TRUE
  /\ SpecRecoveryBacklogActive("residual_recovery") = TRUE
  /\ SpecRecoveryBacklogActive("unresolved_rbc_recovery") = TRUE
  /\ SpecRecoveryBacklogActive("validation_recovery") = TRUE
  /\ SpecRecoveryBacklogActive("validation_inflight_not_pending") = FALSE
  /\ SpecRecoveryBacklogActive("commit_qc_repair") = TRUE
  /\ SpecRecoveryBacklogActive("commit_pipeline_queue") = TRUE
  /\ SpecRecoveryBacklogActive("commit_pipeline_commit_qc_observed") = TRUE
  /\ SpecRecoveryBacklogActive("commit_pipeline_validated_artifact") = TRUE
  /\ SpecRecoveryBacklogActive("invalid_commit_pipeline") = FALSE
  /\ SpecRecoveryBacklogActive("queue_without_evidence") = FALSE
  /\ SpecRecoveryBacklogActive("same_block_recovery") = TRUE

ClassPriorityAnchors ==
  /\ SpecClass("near_fast") = NearClass
  /\ SpecClass("near_with_worker_recovery") = NearClass
  /\ SpecClass("same_block_recovery") = ActiveClass
  /\ SpecClass("near_gate_closed_no_recovery") = BaseClass
  /\ SpecClass("worker_recovery") = ActiveClass
  /\ SpecClass("commit_pipeline_commit_qc_observed") = ActiveClass
  /\ SpecClass("commit_pipeline_validated_artifact") = ActiveClass
  /\ SpecClass("invalid_commit_pipeline") = BaseClass
  /\ SpecClass("queue_without_evidence") = BaseClass

TimeoutAnchors ==
  /\ SpecTimeout("base_zero_quorum") = 1
  /\ SpecTimeout("near_capped_by_base") = SpecBaseTimeout("near_capped_by_base")
  /\ SpecTimeout("near_fast") = 200
  /\ SpecTimeout("worker_recovery") = 2500
  /\ SpecTimeout("commit_pipeline_commit_qc_observed") = 2500
  /\ SpecTimeout("commit_pipeline_validated_artifact") = 2500
  /\ SpecTimeout("near_with_worker_recovery") = 200

DecisionProjectionAnchors ==
  /\ SpecDecision("near_fast") =
       <<NearClass, 200, 2, 3, TRUE, FALSE>>
  /\ SpecDecision("same_block_recovery") =
       <<ActiveClass, 2500, 2, 3, TRUE, TRUE>>
  /\ SpecDecision("near_payload_present") =
       <<BaseClass, 1000, 2, 3, FALSE, FALSE>>
  /\ SpecDecision("validation_inflight_not_pending") =
       <<BaseClass, 1000, 0, 3, TRUE, FALSE>>

StalledPendingTimeoutBaseNearExact ==
  /\ BaseTimeoutMatchesSpec
  /\ NearTimeoutMatchesSpec
  /\ TimeoutMatchesSpec
  /\ BaseTimeoutAnchors
  /\ NearTimeoutAnchors
  /\ TimeoutAnchors

StalledPendingTimeoutNearGateExact ==
  /\ NearCommitQuorumMatchesSpec
  /\ MissingLocalDataMatchesSpec
  /\ NearFastGateMatchesSpec
  /\ NearCommitQuorumAnchors
  /\ MissingLocalDataAnchors

StalledPendingTimeoutRecoveryExact ==
  /\ ValidationRecoveryMatchesSpec
  /\ CommitPipelineBacklogMatchesSpec
  /\ RecoveryBacklogMatchesSpec
  /\ RecoveryBacklogAnchors

StalledPendingTimeoutClassExact ==
  /\ ClassMatchesSpec
  /\ ClassPriorityAnchors

StalledPendingTimeoutDecisionExact ==
  /\ DecisionMatchesSpec
  /\ DecisionProjectionAnchors

StalledPendingTimeoutExactness ==
  /\ StalledPendingTimeoutBaseNearExact
  /\ StalledPendingTimeoutNearGateExact
  /\ StalledPendingTimeoutRecoveryExact
  /\ StalledPendingTimeoutClassExact
  /\ StalledPendingTimeoutDecisionExact

StalledPendingTimeoutCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ StalledPendingTimeoutExactness

SafetyFast ==
  StalledPendingTimeoutExactness

BugBaseTimeoutNoFloor ==
  ActualDecision("base_zero_quorum") = SpecDecision("base_zero_quorum")

BugNearTimeoutNotCapped ==
  ActualDecision("near_capped_by_base") = SpecDecision("near_capped_by_base")

BugNearUsesBaseTimeout ==
  ActualDecision("near_fast") = SpecDecision("near_fast")

BugActiveUsesBaseTimeout ==
  ActualDecision("worker_recovery") = SpecDecision("worker_recovery")

BugNearAllowsZeroVotes ==
  ActualDecision("near_zero_votes") = SpecDecision("near_zero_votes")

BugNearAllowsExistingQuorum ==
  ActualDecision("near_already_quorum") = SpecDecision("near_already_quorum")

BugNearIgnoresPayloadPresence ==
  ActualDecision("near_payload_present") = SpecDecision("near_payload_present")

BugNearIgnoresDaDisabled ==
  ActualDecision("near_da_disabled") = SpecDecision("near_da_disabled")

BugNearIgnoresGate ==
  ActualDecision("near_gate_closed_no_recovery") =
    SpecDecision("near_gate_closed_no_recovery")

BugSameBlockDoesNotBlockNear ==
  ActualDecision("same_block_recovery") = SpecDecision("same_block_recovery")

BugRecoveryPreemptsNear ==
  ActualDecision("near_with_worker_recovery") =
    SpecDecision("near_with_worker_recovery")

BugSkipWorkerRecovery ==
  ActualDecision("worker_recovery") = SpecDecision("worker_recovery")

BugSkipResidualRecovery ==
  ActualDecision("residual_recovery") = SpecDecision("residual_recovery")

BugSkipUnresolvedRbcRecovery ==
  ActualDecision("unresolved_rbc_recovery") = SpecDecision("unresolved_rbc_recovery")

BugSkipCommitQcRepair ==
  ActualDecision("commit_qc_repair") = SpecDecision("commit_qc_repair")

BugInvalidCommitPipelineBacklog ==
  ActualDecision("invalid_commit_pipeline") = SpecDecision("invalid_commit_pipeline")

BugQueueWithoutEvidenceActive ==
  ActualDecision("queue_without_evidence") = SpecDecision("queue_without_evidence")

BugValidationInflightWithoutPending ==
  ActualDecision("validation_inflight_not_pending") =
    SpecDecision("validation_inflight_not_pending")

BugSkipCommitQcObservedBacklog ==
  ActualDecision("commit_pipeline_commit_qc_observed") =
    SpecDecision("commit_pipeline_commit_qc_observed")

BugSkipValidatedArtifactBacklog ==
  ActualDecision("commit_pipeline_validated_artifact") =
    SpecDecision("commit_pipeline_validated_artifact")

====
