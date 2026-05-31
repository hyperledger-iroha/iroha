---- MODULE SumeragiEngineNewViewAdvanceGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for exact NewView-QC round advancement.

This slice models the exact state/output fields in
`ConsensusEngine::on_new_view_qc(...)`. A compatible NewView certificate with a
strictly newer view must set the engine round to `certificate.round`, return
to proposal phase, clear validation ownership, preserve pending finality, and
emit exactly one `AdvanceView { round: certificate.round }`.

Certificates rejected by the shared prefilter, stale/same-view certificates,
and certificates with incompatible carried highest-QC evidence must not update
the stored round or emit an advance output.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipRoundUpdate,
  \* @type: Bool;
  BugRoundWrongHeight,
  \* @type: Bool;
  BugRoundWrongView,
  \* @type: Bool;
  BugRoundWrongEpoch,
  \* @type: Bool;
  BugRoundWrongValidatorSet,
  \* @type: Bool;
  BugSkipAdvanceOutput,
  \* @type: Bool;
  BugOutputWrongHeight,
  \* @type: Bool;
  BugOutputWrongView,
  \* @type: Bool;
  BugOutputWrongEpoch,
  \* @type: Bool;
  BugOutputWrongValidatorSet,
  \* @type: Bool;
  BugKeepValidationInflight,
  \* @type: Bool;
  BugWrongPhaseAfterAccept,
  \* @type: Bool;
  BugDropPendingFinality,
  \* @type: Bool;
  BugRoundUpdateOnRejected,
  \* @type: Bool;
  BugOutputOnRejected

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "safe_no_highest",
  "safe_improving_highest",
  "safe_lower_highest",
  "validation_safe",
  "pending_safe",
  "wrong_height",
  "wrong_epoch",
  "wrong_validator_set",
  "wrong_quorum_policy",
  "same_view",
  "lower_view",
  "future_height_highest",
  "future_view_highest",
  "wrong_epoch_highest"
}

AcceptedCases == {
  "safe_no_highest",
  "safe_improving_highest",
  "safe_lower_highest",
  "validation_safe",
  "pending_safe"
}

RejectedCases == Cases \ AcceptedCases

Values == {
  "none",
  "height_current",
  "height_other",
  "view_current",
  "view_future",
  "view_lower",
  "view_other",
  "epoch_current",
  "epoch_other",
  "validators_current",
  "validators_other",
  "phase_proposal",
  "phase_pending_finality"
}

InitialHeight == "height_current"
InitialView == "view_current"
InitialEpoch == "epoch_current"
InitialValidatorSet == "validators_current"

InitialValidation(candidate) ==
  TRUE

InitialPendingFinality(candidate) ==
  TRUE

InitialPhase(candidate) ==
  "phase_pending_finality"

CertificateHeight(candidate) ==
  IF candidate = "wrong_height" \/ candidate = "future_height_highest"
  THEN "height_other"
  ELSE "height_current"

CertificateView(candidate) ==
  IF candidate = "same_view"
  THEN "view_current"
  ELSE IF candidate = "lower_view"
       THEN "view_lower"
       ELSE "view_future"

CertificateEpoch(candidate) ==
  IF candidate = "wrong_epoch" \/ candidate = "wrong_epoch_highest"
  THEN "epoch_other"
  ELSE "epoch_current"

CertificateValidatorSet(candidate) ==
  IF candidate = "wrong_validator_set"
  THEN "validators_other"
  ELSE "validators_current"

WrongHeight(height) ==
  IF height = "height_current" THEN "height_other" ELSE "height_current"

WrongView(view) ==
  IF view = "view_future" THEN "view_other" ELSE "view_future"

WrongEpoch(epoch) ==
  IF epoch = "epoch_current" THEN "epoch_other" ELSE "epoch_current"

WrongValidatorSet(validator_set) ==
  IF validator_set = "validators_current"
  THEN "validators_other"
  ELSE "validators_current"

Accepted(candidate) ==
  candidate \in AcceptedCases

SpecRoundHeight(candidate) ==
  IF Accepted(candidate) THEN CertificateHeight(candidate) ELSE InitialHeight

SpecRoundView(candidate) ==
  IF Accepted(candidate) THEN CertificateView(candidate) ELSE InitialView

SpecRoundEpoch(candidate) ==
  IF Accepted(candidate) THEN CertificateEpoch(candidate) ELSE InitialEpoch

SpecRoundValidatorSet(candidate) ==
  IF Accepted(candidate)
  THEN CertificateValidatorSet(candidate)
  ELSE InitialValidatorSet

SpecOutputHeight(candidate) ==
  IF Accepted(candidate) THEN CertificateHeight(candidate) ELSE "none"

SpecOutputView(candidate) ==
  IF Accepted(candidate) THEN CertificateView(candidate) ELSE "none"

SpecOutputEpoch(candidate) ==
  IF Accepted(candidate) THEN CertificateEpoch(candidate) ELSE "none"

SpecOutputValidatorSet(candidate) ==
  IF Accepted(candidate) THEN CertificateValidatorSet(candidate) ELSE "none"

SpecValidationAfter(candidate) ==
  IF Accepted(candidate) THEN FALSE ELSE InitialValidation(candidate)

SpecPendingFinalityAfter(candidate) ==
  InitialPendingFinality(candidate)

SpecPhaseAfter(candidate) ==
  IF Accepted(candidate) THEN "phase_proposal" ELSE InitialPhase(candidate)

ImplementationRoundHeight(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugSkipRoundUpdate
    THEN InitialHeight
    ELSE IF BugRoundWrongHeight
         THEN WrongHeight(CertificateHeight(candidate))
         ELSE CertificateHeight(candidate)
  ELSE IF BugRoundUpdateOnRejected
       THEN CertificateHeight(candidate)
       ELSE InitialHeight

ImplementationRoundView(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugSkipRoundUpdate
    THEN InitialView
    ELSE IF BugRoundWrongView
         THEN WrongView(CertificateView(candidate))
         ELSE CertificateView(candidate)
  ELSE IF BugRoundUpdateOnRejected
       THEN CertificateView(candidate)
       ELSE InitialView

ImplementationRoundEpoch(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugSkipRoundUpdate
    THEN InitialEpoch
    ELSE IF BugRoundWrongEpoch
         THEN WrongEpoch(CertificateEpoch(candidate))
         ELSE CertificateEpoch(candidate)
  ELSE IF BugRoundUpdateOnRejected
       THEN CertificateEpoch(candidate)
       ELSE InitialEpoch

ImplementationRoundValidatorSet(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugSkipRoundUpdate
    THEN InitialValidatorSet
    ELSE IF BugRoundWrongValidatorSet
         THEN WrongValidatorSet(CertificateValidatorSet(candidate))
         ELSE CertificateValidatorSet(candidate)
  ELSE IF BugRoundUpdateOnRejected
       THEN CertificateValidatorSet(candidate)
       ELSE InitialValidatorSet

ImplementationOutputHeight(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugSkipAdvanceOutput
    THEN "none"
    ELSE IF BugOutputWrongHeight
         THEN WrongHeight(CertificateHeight(candidate))
         ELSE CertificateHeight(candidate)
  ELSE IF BugOutputOnRejected
       THEN CertificateHeight(candidate)
       ELSE "none"

ImplementationOutputView(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugSkipAdvanceOutput
    THEN "none"
    ELSE IF BugOutputWrongView
         THEN WrongView(CertificateView(candidate))
         ELSE CertificateView(candidate)
  ELSE IF BugOutputOnRejected
       THEN CertificateView(candidate)
       ELSE "none"

ImplementationOutputEpoch(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugSkipAdvanceOutput
    THEN "none"
    ELSE IF BugOutputWrongEpoch
         THEN WrongEpoch(CertificateEpoch(candidate))
         ELSE CertificateEpoch(candidate)
  ELSE IF BugOutputOnRejected
       THEN CertificateEpoch(candidate)
       ELSE "none"

ImplementationOutputValidatorSet(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugSkipAdvanceOutput
    THEN "none"
    ELSE IF BugOutputWrongValidatorSet
         THEN WrongValidatorSet(CertificateValidatorSet(candidate))
         ELSE CertificateValidatorSet(candidate)
  ELSE IF BugOutputOnRejected
       THEN CertificateValidatorSet(candidate)
       ELSE "none"

ImplementationValidationAfter(candidate) ==
  IF Accepted(candidate)
  THEN BugKeepValidationInflight
  ELSE InitialValidation(candidate)

ImplementationPendingFinalityAfter(candidate) ==
  IF Accepted(candidate) /\ BugDropPendingFinality
  THEN FALSE
  ELSE InitialPendingFinality(candidate)

ImplementationPhaseAfter(candidate) ==
  IF Accepted(candidate)
  THEN
    IF BugWrongPhaseAfterAccept
    THEN InitialPhase(candidate)
    ELSE "phase_proposal"
  ELSE InitialPhase(candidate)

TypeInvariant ==
  /\ BugSkipRoundUpdate \in BOOLEAN
  /\ BugRoundWrongHeight \in BOOLEAN
  /\ BugRoundWrongView \in BOOLEAN
  /\ BugRoundWrongEpoch \in BOOLEAN
  /\ BugRoundWrongValidatorSet \in BOOLEAN
  /\ BugSkipAdvanceOutput \in BOOLEAN
  /\ BugOutputWrongHeight \in BOOLEAN
  /\ BugOutputWrongView \in BOOLEAN
  /\ BugOutputWrongEpoch \in BOOLEAN
  /\ BugOutputWrongValidatorSet \in BOOLEAN
  /\ BugKeepValidationInflight \in BOOLEAN
  /\ BugWrongPhaseAfterAccept \in BOOLEAN
  /\ BugDropPendingFinality \in BOOLEAN
  /\ BugRoundUpdateOnRejected \in BOOLEAN
  /\ BugOutputOnRejected \in BOOLEAN
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

RoundHeightMatchesSpec ==
  \A candidate \in tried:
    ImplementationRoundHeight(candidate) = SpecRoundHeight(candidate)

RoundViewMatchesSpec ==
  \A candidate \in tried:
    ImplementationRoundView(candidate) = SpecRoundView(candidate)

RoundEpochMatchesSpec ==
  \A candidate \in tried:
    ImplementationRoundEpoch(candidate) = SpecRoundEpoch(candidate)

RoundValidatorSetMatchesSpec ==
  \A candidate \in tried:
    ImplementationRoundValidatorSet(candidate) = SpecRoundValidatorSet(candidate)

OutputHeightMatchesSpec ==
  \A candidate \in tried:
    ImplementationOutputHeight(candidate) = SpecOutputHeight(candidate)

OutputViewMatchesSpec ==
  \A candidate \in tried:
    ImplementationOutputView(candidate) = SpecOutputView(candidate)

OutputEpochMatchesSpec ==
  \A candidate \in tried:
    ImplementationOutputEpoch(candidate) = SpecOutputEpoch(candidate)

OutputValidatorSetMatchesSpec ==
  \A candidate \in tried:
    ImplementationOutputValidatorSet(candidate) = SpecOutputValidatorSet(candidate)

ValidationMatchesSpec ==
  \A candidate \in tried:
    ImplementationValidationAfter(candidate) = SpecValidationAfter(candidate)

PendingFinalityMatchesSpec ==
  \A candidate \in tried:
    ImplementationPendingFinalityAfter(candidate) =
      SpecPendingFinalityAfter(candidate)

PhaseMatchesSpec ==
  \A candidate \in tried:
    ImplementationPhaseAfter(candidate) = SpecPhaseAfter(candidate)

AcceptedNewViewUpdatesStoredRoundExactly ==
  \A candidate \in tried:
    Accepted(candidate) =>
      /\ ImplementationRoundHeight(candidate) = CertificateHeight(candidate)
      /\ ImplementationRoundView(candidate) = CertificateView(candidate)
      /\ ImplementationRoundEpoch(candidate) = CertificateEpoch(candidate)
      /\ ImplementationRoundValidatorSet(candidate) =
        CertificateValidatorSet(candidate)

AcceptedNewViewEmitsExactAdvanceView ==
  \A candidate \in tried:
    Accepted(candidate) =>
      /\ ImplementationOutputHeight(candidate) = CertificateHeight(candidate)
      /\ ImplementationOutputView(candidate) = CertificateView(candidate)
      /\ ImplementationOutputEpoch(candidate) = CertificateEpoch(candidate)
      /\ ImplementationOutputValidatorSet(candidate) =
        CertificateValidatorSet(candidate)

AcceptedNewViewClearsValidation ==
  \A candidate \in tried:
    Accepted(candidate) => ImplementationValidationAfter(candidate) = FALSE

AcceptedNewViewPreservesPendingFinality ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationPendingFinalityAfter(candidate) =
        InitialPendingFinality(candidate)

AcceptedNewViewEntersProposalPhase ==
  \A candidate \in tried:
    Accepted(candidate) => ImplementationPhaseAfter(candidate) = "phase_proposal"

RejectedNewViewDoesNotUpdateStoredRound ==
  \A candidate \in tried:
    candidate \in RejectedCases =>
      /\ ImplementationRoundHeight(candidate) = InitialHeight
      /\ ImplementationRoundView(candidate) = InitialView
      /\ ImplementationRoundEpoch(candidate) = InitialEpoch
      /\ ImplementationRoundValidatorSet(candidate) = InitialValidatorSet

RejectedNewViewEmitsNoAdvanceView ==
  \A candidate \in tried:
    candidate \in RejectedCases =>
      /\ ImplementationOutputHeight(candidate) = "none"
      /\ ImplementationOutputView(candidate) = "none"
      /\ ImplementationOutputEpoch(candidate) = "none"
      /\ ImplementationOutputValidatorSet(candidate) = "none"

RejectedNewViewPreservesOwnershipAndPhase ==
  \A candidate \in tried:
    candidate \in RejectedCases =>
      /\ ImplementationValidationAfter(candidate) = InitialValidation(candidate)
      /\ ImplementationPendingFinalityAfter(candidate) =
        InitialPendingFinality(candidate)
      /\ ImplementationPhaseAfter(candidate) = InitialPhase(candidate)

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ ImplementationRoundHeight(candidate) \in Values
    /\ ImplementationRoundView(candidate) \in Values
    /\ ImplementationRoundEpoch(candidate) \in Values
    /\ ImplementationRoundValidatorSet(candidate) \in Values
    /\ ImplementationOutputHeight(candidate) \in Values
    /\ ImplementationOutputView(candidate) \in Values
    /\ ImplementationOutputEpoch(candidate) \in Values
    /\ ImplementationOutputValidatorSet(candidate) \in Values
    /\ ImplementationPhaseAfter(candidate) \in Values

Safety ==
  /\ RoundHeightMatchesSpec
  /\ RoundViewMatchesSpec
  /\ RoundEpochMatchesSpec
  /\ RoundValidatorSetMatchesSpec
  /\ OutputHeightMatchesSpec
  /\ OutputViewMatchesSpec
  /\ OutputEpochMatchesSpec
  /\ OutputValidatorSetMatchesSpec
  /\ ValidationMatchesSpec
  /\ PendingFinalityMatchesSpec
  /\ PhaseMatchesSpec
  /\ AcceptedNewViewUpdatesStoredRoundExactly
  /\ AcceptedNewViewEmitsExactAdvanceView
  /\ AcceptedNewViewClearsValidation
  /\ AcceptedNewViewPreservesPendingFinality
  /\ AcceptedNewViewEntersProposalPhase
  /\ RejectedNewViewDoesNotUpdateStoredRound
  /\ RejectedNewViewEmitsNoAdvanceView
  /\ RejectedNewViewPreservesOwnershipAndPhase
  /\ ValuesStayInDomain

====
