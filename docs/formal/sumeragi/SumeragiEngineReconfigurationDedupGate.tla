---- MODULE SumeragiEngineReconfigurationDedupGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for committed-block reconfiguration deduplication.

This slice models the `already_scheduled` guard in
`ConsensusEngine::on_committed_block(...)`:

    pending.activation_height == change.activation_height

The guard is keyed by activation height, not by the full validator-set-change
payload. A same-hash committed-block replay carrying a boundary
reconfiguration may still stage and emit activation when that activation height
is not already scheduled, which covers the "plain commit first, reconfiguration
metadata later" path. Once a change for that activation height is pending, a
later notification with the same activation height preserves the existing
pending change and emits no second activation, even if the later change payload
differs.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugRequireFreshRecordForActivation,
  \* @type: Bool;
  BugIgnoreAlreadyScheduledHeight,
  \* @type: Bool;
  BugCompareFullChangeForDedup,
  \* @type: Bool;
  BugOverwriteScheduledSameHeight,
  \* @type: Bool;
  BugClearScheduledSameHeight,
  \* @type: Bool;
  BugSuppressDifferentHeightBoundary,
  \* @type: Bool;
  BugPreserveOldOnDifferentHeightBoundary,
  \* @type: Bool;
  BugEmitOldOnDifferentHeightBoundary,
  \* @type: Bool;
  BugStageNonBoundary,
  \* @type: Bool;
  BugActivateNonBoundary,
  \* @type: Bool;
  BugStageConflict,
  \* @type: Bool;
  BugActivateConflict,
  \* @type: Bool;
  BugActivatePlain

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

ChangeValues == {"none", "A", "B"}
ActivationValues == {"none", "next", "other"}

Cases == {
  "fresh_boundary_none",
  "fresh_boundary_prior_same_a",
  "fresh_boundary_prior_same_b",
  "fresh_boundary_prior_other_b",
  "duplicate_boundary_none",
  "duplicate_boundary_prior_same_a",
  "duplicate_boundary_prior_same_b",
  "duplicate_boundary_prior_other_b",
  "fresh_non_boundary_none",
  "duplicate_non_boundary_prior_same_b",
  "conflict_boundary_none",
  "plain_commit_prior_same_b"
}

Fresh(candidate) ==
  candidate \in {
    "fresh_boundary_none",
    "fresh_boundary_prior_same_a",
    "fresh_boundary_prior_same_b",
    "fresh_boundary_prior_other_b",
    "fresh_non_boundary_none"
  }

DuplicateSameHash(candidate) ==
  candidate \in {
    "duplicate_boundary_none",
    "duplicate_boundary_prior_same_a",
    "duplicate_boundary_prior_same_b",
    "duplicate_boundary_prior_other_b",
    "duplicate_non_boundary_prior_same_b"
  }

Conflict(candidate) ==
  candidate = "conflict_boundary_none"

Plain(candidate) ==
  candidate = "plain_commit_prior_same_b"

HasReconfiguration(candidate) ==
  ~Plain(candidate)

BoundaryReconfiguration(candidate) ==
  candidate \in {
    "fresh_boundary_none",
    "fresh_boundary_prior_same_a",
    "fresh_boundary_prior_same_b",
    "fresh_boundary_prior_other_b",
    "duplicate_boundary_none",
    "duplicate_boundary_prior_same_a",
    "duplicate_boundary_prior_same_b",
    "duplicate_boundary_prior_other_b",
    "conflict_boundary_none"
  }

NonBoundaryReconfiguration(candidate) ==
  candidate \in {
    "fresh_non_boundary_none",
    "duplicate_non_boundary_prior_same_b"
  }

CanReachReconfigurationGuard(candidate) ==
  ~Conflict(candidate) /\ HasReconfiguration(candidate)

InitialPendingChange(candidate) ==
  IF candidate \in {
    "fresh_boundary_prior_same_a",
    "duplicate_boundary_prior_same_a"
  }
  THEN "A"
  ELSE IF candidate \in {
    "fresh_boundary_prior_same_b",
    "fresh_boundary_prior_other_b",
    "duplicate_boundary_prior_same_b",
    "duplicate_boundary_prior_other_b",
    "duplicate_non_boundary_prior_same_b",
    "plain_commit_prior_same_b"
  }
  THEN "B"
  ELSE "none"

InitialPendingActivation(candidate) ==
  IF candidate \in {
    "fresh_boundary_prior_same_a",
    "fresh_boundary_prior_same_b",
    "duplicate_boundary_prior_same_a",
    "duplicate_boundary_prior_same_b",
    "duplicate_non_boundary_prior_same_b",
    "plain_commit_prior_same_b"
  }
  THEN "next"
  ELSE IF candidate \in {
    "fresh_boundary_prior_other_b",
    "duplicate_boundary_prior_other_b"
  }
  THEN "other"
  ELSE "none"

CandidateChange(candidate) ==
  IF HasReconfiguration(candidate) THEN "A" ELSE "none"

CandidateActivation(candidate) ==
  IF BoundaryReconfiguration(candidate)
  THEN "next"
  ELSE IF NonBoundaryReconfiguration(candidate)
       THEN "other"
       ELSE "none"

SpecAlreadyScheduled(candidate) ==
  /\ CandidateActivation(candidate) # "none"
  /\ InitialPendingActivation(candidate) = CandidateActivation(candidate)

SpecStagesAndActivates(candidate) ==
  /\ CanReachReconfigurationGuard(candidate)
  /\ BoundaryReconfiguration(candidate)
  /\ ~SpecAlreadyScheduled(candidate)

SpecStageAfter(candidate) ==
  IF SpecStagesAndActivates(candidate)
  THEN CandidateChange(candidate)
  ELSE InitialPendingChange(candidate)

SpecEmit(candidate) ==
  IF SpecStagesAndActivates(candidate)
  THEN CandidateChange(candidate)
  ELSE "none"

ImplementationAlreadyScheduled(candidate) ==
  IF BugIgnoreAlreadyScheduledHeight
  THEN FALSE
  ELSE
    /\ CandidateActivation(candidate) # "none"
    /\ InitialPendingActivation(candidate) = CandidateActivation(candidate)
    /\ IF BugCompareFullChangeForDedup
       THEN InitialPendingChange(candidate) = CandidateChange(candidate)
       ELSE TRUE

ImplementationStageAfter(candidate) ==
  IF Conflict(candidate)
  THEN IF BugStageConflict THEN CandidateChange(candidate) ELSE InitialPendingChange(candidate)
  ELSE IF Plain(candidate)
       THEN InitialPendingChange(candidate)
       ELSE IF NonBoundaryReconfiguration(candidate)
            THEN IF BugStageNonBoundary
                 THEN CandidateChange(candidate)
                 ELSE InitialPendingChange(candidate)
            ELSE IF BoundaryReconfiguration(candidate)
                 THEN IF ImplementationAlreadyScheduled(candidate)
                      THEN IF BugOverwriteScheduledSameHeight
                           THEN CandidateChange(candidate)
                           ELSE IF BugClearScheduledSameHeight
                                THEN "none"
                                ELSE InitialPendingChange(candidate)
                      ELSE IF BugRequireFreshRecordForActivation
                           /\ DuplicateSameHash(candidate)
                           THEN InitialPendingChange(candidate)
                           ELSE IF BugSuppressDifferentHeightBoundary
                                THEN InitialPendingChange(candidate)
                                ELSE IF BugPreserveOldOnDifferentHeightBoundary
                                     /\ InitialPendingActivation(candidate) = "other"
                                     THEN InitialPendingChange(candidate)
                                     ELSE CandidateChange(candidate)
                 ELSE InitialPendingChange(candidate)

ImplementationEmit(candidate) ==
  IF Conflict(candidate)
  THEN IF BugActivateConflict THEN CandidateChange(candidate) ELSE "none"
  ELSE IF Plain(candidate)
       THEN IF BugActivatePlain THEN "A" ELSE "none"
       ELSE IF NonBoundaryReconfiguration(candidate)
            THEN IF BugActivateNonBoundary THEN CandidateChange(candidate) ELSE "none"
            ELSE IF BoundaryReconfiguration(candidate)
                 THEN IF ImplementationAlreadyScheduled(candidate)
                      THEN "none"
                      ELSE IF BugRequireFreshRecordForActivation
                           /\ DuplicateSameHash(candidate)
                           THEN "none"
                           ELSE IF BugSuppressDifferentHeightBoundary
                                THEN "none"
                                ELSE IF BugEmitOldOnDifferentHeightBoundary
                                     /\ InitialPendingActivation(candidate) = "other"
                                     THEN InitialPendingChange(candidate)
                                     ELSE CandidateChange(candidate)
                 ELSE "none"

TypeInvariant ==
  /\ BugRequireFreshRecordForActivation \in BOOLEAN
  /\ BugIgnoreAlreadyScheduledHeight \in BOOLEAN
  /\ BugCompareFullChangeForDedup \in BOOLEAN
  /\ BugOverwriteScheduledSameHeight \in BOOLEAN
  /\ BugClearScheduledSameHeight \in BOOLEAN
  /\ BugSuppressDifferentHeightBoundary \in BOOLEAN
  /\ BugPreserveOldOnDifferentHeightBoundary \in BOOLEAN
  /\ BugEmitOldOnDifferentHeightBoundary \in BOOLEAN
  /\ BugStageNonBoundary \in BOOLEAN
  /\ BugActivateNonBoundary \in BOOLEAN
  /\ BugStageConflict \in BOOLEAN
  /\ BugActivateConflict \in BOOLEAN
  /\ BugActivatePlain \in BOOLEAN
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

StageAfterMatchesSpec ==
  \A candidate \in tried:
    ImplementationStageAfter(candidate) = SpecStageAfter(candidate)

EmitMatchesSpec ==
  \A candidate \in tried:
    ImplementationEmit(candidate) = SpecEmit(candidate)

AlreadyScheduledSameHeightPreservesPending ==
  \A candidate \in tried:
    SpecAlreadyScheduled(candidate) /\ CanReachReconfigurationGuard(candidate) =>
      /\ ImplementationStageAfter(candidate) = InitialPendingChange(candidate)
      /\ ImplementationEmit(candidate) = "none"

AlreadyScheduledUsesActivationHeightOnly ==
  \A candidate \in {
    "fresh_boundary_prior_same_b",
    "duplicate_boundary_prior_same_b",
    "duplicate_non_boundary_prior_same_b"
  }:
    candidate \in tried =>
      /\ ImplementationStageAfter(candidate) = "B"
      /\ ImplementationEmit(candidate) = "none"

DuplicateSameHashCanActivateWhenUnscheduled ==
  \A candidate \in {
    "duplicate_boundary_none",
    "duplicate_boundary_prior_other_b"
  }:
    candidate \in tried =>
      /\ ImplementationStageAfter(candidate) = "A"
      /\ ImplementationEmit(candidate) = "A"

DifferentActivationHeightBoundaryReplacesPending ==
  \A candidate \in {
    "fresh_boundary_prior_other_b",
    "duplicate_boundary_prior_other_b"
  }:
    candidate \in tried =>
      /\ ImplementationStageAfter(candidate) = "A"
      /\ ImplementationEmit(candidate) = "A"

ActivationRequiresBoundaryAndNoSameHeightSchedule ==
  \A candidate \in tried:
    ImplementationEmit(candidate) # "none" =>
      /\ CanReachReconfigurationGuard(candidate)
      /\ BoundaryReconfiguration(candidate)
      /\ ~SpecAlreadyScheduled(candidate)

StageChangeRequiresBoundaryAndNoSameHeightSchedule ==
  \A candidate \in tried:
    ImplementationStageAfter(candidate) # InitialPendingChange(candidate) =>
      /\ CanReachReconfigurationGuard(candidate)
      /\ BoundaryReconfiguration(candidate)
      /\ ~SpecAlreadyScheduled(candidate)

ActivationMatchesStagedChange ==
  \A candidate \in tried:
    ImplementationEmit(candidate) # "none" =>
      ImplementationEmit(candidate) = ImplementationStageAfter(candidate)

NonBoundaryReconfigurationPreservesPendingAndStaysSilent ==
  \A candidate \in tried:
    NonBoundaryReconfiguration(candidate) =>
      /\ ImplementationStageAfter(candidate) = InitialPendingChange(candidate)
      /\ ImplementationEmit(candidate) = "none"

ConflictsPreservePendingAndStaySilent ==
  \A candidate \in tried:
    Conflict(candidate) =>
      /\ ImplementationStageAfter(candidate) = InitialPendingChange(candidate)
      /\ ImplementationEmit(candidate) = "none"

PlainCommitsPreservePendingAndStaySilent ==
  \A candidate \in tried:
    Plain(candidate) =>
      /\ ImplementationStageAfter(candidate) = InitialPendingChange(candidate)
      /\ ImplementationEmit(candidate) = "none"

NoSyntheticChangeValues ==
  \A candidate \in tried:
    /\ ImplementationStageAfter(candidate) \in ChangeValues
    /\ ImplementationEmit(candidate) \in ChangeValues
    /\ InitialPendingActivation(candidate) \in ActivationValues
    /\ CandidateActivation(candidate) \in ActivationValues

Safety ==
  /\ StageAfterMatchesSpec
  /\ EmitMatchesSpec
  /\ AlreadyScheduledSameHeightPreservesPending
  /\ AlreadyScheduledUsesActivationHeightOnly
  /\ DuplicateSameHashCanActivateWhenUnscheduled
  /\ DifferentActivationHeightBoundaryReplacesPending
  /\ ActivationRequiresBoundaryAndNoSameHeightSchedule
  /\ StageChangeRequiresBoundaryAndNoSameHeightSchedule
  /\ ActivationMatchesStagedChange
  /\ NonBoundaryReconfigurationPreservesPendingAndStaysSilent
  /\ ConflictsPreservePendingAndStaySilent
  /\ PlainCommitsPreservePendingAndStaySilent
  /\ NoSyntheticChangeValues

====
