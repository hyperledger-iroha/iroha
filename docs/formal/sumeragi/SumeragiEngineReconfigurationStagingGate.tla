---- MODULE SumeragiEngineReconfigurationStagingGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for committed-block reconfiguration staging.

This slice models the reconfiguration side effect in
`ConsensusEngine::on_committed_block(...)`. A fresh committed-block
notification carrying a validator-set change whose activation height is the
next block height must both stage that exact change in
`pending_reconfiguration` and emit `ActivateValidatorSet(change)`. Plain
commits and non-boundary reconfigurations must preserve any previously staged
change and emit no activation. Duplicate committed-block notifications in this
slice represent already-scheduled duplicate reconfiguration notifications;
`SumeragiEngineReconfigurationDedupGate.tla` separately covers the replay path
where a plain same-hash commit is followed by boundary reconfiguration metadata
and may still activate.

The model is separate from `SumeragiEngineCommittedBlockGate.tla`, which
checks that activation only appears for fresh boundary notifications. This
slice checks the internal staging/output parity and no-op preservation.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipBoundaryStaging,
  \* @type: Bool;
  BugSkipBoundaryActivation,
  \* @type: Bool;
  BugStageWithoutBoundary,
  \* @type: Bool;
  BugActivateWithoutBoundary,
  \* @type: Bool;
  BugStageNonBoundary,
  \* @type: Bool;
  BugActivateNonBoundary,
  \* @type: Bool;
  BugStageDuplicate,
  \* @type: Bool;
  BugActivateDuplicate,
  \* @type: Bool;
  BugStageConflict,
  \* @type: Bool;
  BugActivateConflict,
  \* @type: Bool;
  BugStageWrongChange,
  \* @type: Bool;
  BugEmitWrongChange,
  \* @type: Bool;
  BugPreserveOldOnBoundary,
  \* @type: Bool;
  BugClearExistingOnNoop

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Changes == {"none", "A", "B"}

Cases == {
  "fresh_plain_none",
  "fresh_plain_prior_b",
  "fresh_boundary_a_none",
  "fresh_boundary_a_prior_b",
  "fresh_boundary_b_none",
  "fresh_non_boundary_a_none",
  "fresh_non_boundary_a_prior_b",
  "duplicate_boundary_a_none",
  "duplicate_boundary_a_prior_b",
  "conflict_boundary_a_none",
  "conflict_boundary_a_prior_b",
  "conflict_non_boundary_a_prior_b"
}

Fresh(candidate) ==
  candidate \in {
    "fresh_plain_none",
    "fresh_plain_prior_b",
    "fresh_boundary_a_none",
    "fresh_boundary_a_prior_b",
    "fresh_boundary_b_none",
    "fresh_non_boundary_a_none",
    "fresh_non_boundary_a_prior_b"
  }

Duplicate(candidate) ==
  candidate \in {"duplicate_boundary_a_none", "duplicate_boundary_a_prior_b"}

Conflict(candidate) ==
  candidate \in {
    "conflict_boundary_a_none",
    "conflict_boundary_a_prior_b",
    "conflict_non_boundary_a_prior_b"
  }

Plain(candidate) ==
  candidate \in {"fresh_plain_none", "fresh_plain_prior_b"}

BoundaryReconfiguration(candidate) ==
  candidate \in {
    "fresh_boundary_a_none",
    "fresh_boundary_a_prior_b",
    "fresh_boundary_b_none",
    "duplicate_boundary_a_none",
    "duplicate_boundary_a_prior_b",
    "conflict_boundary_a_none",
    "conflict_boundary_a_prior_b"
  }

NonBoundaryReconfiguration(candidate) ==
  candidate \in {
    "fresh_non_boundary_a_none",
    "fresh_non_boundary_a_prior_b",
    "conflict_non_boundary_a_prior_b"
  }

InitialStaged(candidate) ==
  IF candidate \in {
    "fresh_plain_prior_b",
    "fresh_boundary_a_prior_b",
    "fresh_non_boundary_a_prior_b",
    "duplicate_boundary_a_prior_b",
    "conflict_boundary_a_prior_b",
    "conflict_non_boundary_a_prior_b"
  }
  THEN "B"
  ELSE "none"

ChangeOf(candidate) ==
  IF candidate = "fresh_boundary_b_none"
  THEN "B"
  ELSE IF BoundaryReconfiguration(candidate) \/ NonBoundaryReconfiguration(candidate)
       THEN "A"
       ELSE "none"

AlternateChange(change) ==
  IF change = "A" THEN "B" ELSE "A"

FreshBoundary(candidate) ==
  Fresh(candidate) /\ BoundaryReconfiguration(candidate)

FreshNonBoundary(candidate) ==
  Fresh(candidate) /\ NonBoundaryReconfiguration(candidate)

SpecStageAfter(candidate) ==
  IF FreshBoundary(candidate)
  THEN ChangeOf(candidate)
  ELSE InitialStaged(candidate)

SpecEmit(candidate) ==
  IF FreshBoundary(candidate)
  THEN ChangeOf(candidate)
  ELSE "none"

ImplementationStageAfter(candidate) ==
  IF FreshBoundary(candidate)
  THEN
    IF BugSkipBoundaryStaging \/ BugPreserveOldOnBoundary
    THEN InitialStaged(candidate)
    ELSE IF BugStageWrongChange
         THEN AlternateChange(ChangeOf(candidate))
         ELSE ChangeOf(candidate)
  ELSE IF Plain(candidate)
       THEN
         IF BugStageWithoutBoundary
         THEN "A"
         ELSE IF BugClearExistingOnNoop
              THEN "none"
              ELSE InitialStaged(candidate)
       ELSE IF FreshNonBoundary(candidate)
            THEN
              IF BugStageNonBoundary
              THEN ChangeOf(candidate)
              ELSE IF BugClearExistingOnNoop
                   THEN "none"
                   ELSE InitialStaged(candidate)
            ELSE IF Duplicate(candidate)
                 THEN
                   IF BugStageDuplicate
                   THEN ChangeOf(candidate)
                   ELSE IF BugClearExistingOnNoop
                        THEN "none"
                        ELSE InitialStaged(candidate)
                 ELSE IF Conflict(candidate)
                      THEN
                        IF BugStageConflict
                        THEN ChangeOf(candidate)
                        ELSE IF BugClearExistingOnNoop
                             THEN "none"
                             ELSE InitialStaged(candidate)
                      ELSE InitialStaged(candidate)

ImplementationEmit(candidate) ==
  IF FreshBoundary(candidate)
  THEN
    IF BugSkipBoundaryActivation
    THEN "none"
    ELSE IF BugEmitWrongChange
         THEN AlternateChange(ChangeOf(candidate))
         ELSE ChangeOf(candidate)
  ELSE IF Plain(candidate)
       THEN
         IF BugActivateWithoutBoundary
         THEN "A"
         ELSE "none"
       ELSE IF FreshNonBoundary(candidate)
            THEN
              IF BugActivateNonBoundary
              THEN ChangeOf(candidate)
              ELSE "none"
            ELSE IF Duplicate(candidate)
                 THEN
                   IF BugActivateDuplicate
                   THEN ChangeOf(candidate)
                   ELSE "none"
                 ELSE IF Conflict(candidate)
                      THEN
                        IF BugActivateConflict
                        THEN ChangeOf(candidate)
                        ELSE "none"
                      ELSE "none"

TypeInvariant ==
  /\ BugSkipBoundaryStaging \in BOOLEAN
  /\ BugSkipBoundaryActivation \in BOOLEAN
  /\ BugStageWithoutBoundary \in BOOLEAN
  /\ BugActivateWithoutBoundary \in BOOLEAN
  /\ BugStageNonBoundary \in BOOLEAN
  /\ BugActivateNonBoundary \in BOOLEAN
  /\ BugStageDuplicate \in BOOLEAN
  /\ BugActivateDuplicate \in BOOLEAN
  /\ BugStageConflict \in BOOLEAN
  /\ BugActivateConflict \in BOOLEAN
  /\ BugStageWrongChange \in BOOLEAN
  /\ BugEmitWrongChange \in BOOLEAN
  /\ BugPreserveOldOnBoundary \in BOOLEAN
  /\ BugClearExistingOnNoop \in BOOLEAN
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

FreshBoundaryStagesAndActivates ==
  \A candidate \in tried:
    FreshBoundary(candidate) =>
      /\ ImplementationStageAfter(candidate) = ChangeOf(candidate)
      /\ ImplementationEmit(candidate) = ChangeOf(candidate)

BoundaryActivationMatchesStagedChange ==
  \A candidate \in tried:
    FreshBoundary(candidate) =>
      ImplementationEmit(candidate) = ImplementationStageAfter(candidate)

NoStageWithoutFreshBoundary ==
  \A candidate \in tried:
    ~FreshBoundary(candidate) =>
      ImplementationStageAfter(candidate) = InitialStaged(candidate)

NoActivationWithoutFreshBoundary ==
  \A candidate \in tried:
    ~FreshBoundary(candidate) => ImplementationEmit(candidate) = "none"

PlainCommitsPreserveStagingAndStaySilent ==
  \A candidate \in tried:
    Plain(candidate) =>
      /\ ImplementationStageAfter(candidate) = InitialStaged(candidate)
      /\ ImplementationEmit(candidate) = "none"

NonBoundaryReconfigurationsPreserveStagingAndStaySilent ==
  \A candidate \in tried:
    FreshNonBoundary(candidate) =>
      /\ ImplementationStageAfter(candidate) = InitialStaged(candidate)
      /\ ImplementationEmit(candidate) = "none"

DuplicatesPreserveStagingAndStaySilent ==
  \A candidate \in tried:
    Duplicate(candidate) =>
      /\ ImplementationStageAfter(candidate) = InitialStaged(candidate)
      /\ ImplementationEmit(candidate) = "none"

ConflictsPreserveStagingAndStaySilent ==
  \A candidate \in tried:
    Conflict(candidate) =>
      /\ ImplementationStageAfter(candidate) = InitialStaged(candidate)
      /\ ImplementationEmit(candidate) = "none"

BoundaryReplacesExistingStaging ==
  "fresh_boundary_a_prior_b" \in tried =>
    ImplementationStageAfter("fresh_boundary_a_prior_b") = "A"

NoSyntheticChangeValues ==
  \A candidate \in tried:
    /\ ImplementationStageAfter(candidate) \in Changes
    /\ ImplementationEmit(candidate) \in Changes

EngineReconfigurationStagingExactness ==
  /\ StageAfterMatchesSpec
  /\ EmitMatchesSpec
  /\ FreshBoundaryStagesAndActivates
  /\ BoundaryActivationMatchesStagedChange
  /\ NoStageWithoutFreshBoundary
  /\ NoActivationWithoutFreshBoundary
  /\ PlainCommitsPreserveStagingAndStaySilent
  /\ NonBoundaryReconfigurationsPreserveStagingAndStaySilent
  /\ DuplicatesPreserveStagingAndStaySilent
  /\ ConflictsPreserveStagingAndStaySilent
  /\ BoundaryReplacesExistingStaging
  /\ NoSyntheticChangeValues

Safety ==
  EngineReconfigurationStagingExactness

EngineReconfigurationStagingCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineReconfigurationStagingExactness

====
