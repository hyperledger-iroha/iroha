---- MODULE SumeragiEngineProposalGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for the pure Sumeragi engine proposal-ingress gate.

This slice models `ConsensusEngine::on_proposal(...)`. A proposal may enter
validation only when the engine is in proposal phase, the proposal round is the
current height/epoch/validator set/view, any carried highest QC is compatible
with that round, and the proposal satisfies the current lock. Accepted
proposals must request validation, sign one prepare vote, and move the engine
into prepare phase. Rejected proposals must not emit either output or mutate
the phase.

The model enumerates the finite guard cases that matter for this engine
boundary. `SpecAccepts` is the reference contract; the implementation
transition records whether each candidate proposal is accepted, ignored,
validated, signed in prepare, and advanced into prepare phase.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAcceptWrongPhase,
  \* @type: Bool;
  BugAcceptWrongRound,
  \* @type: Bool;
  BugAcceptIncompatibleHighest,
  \* @type: Bool;
  BugAcceptLockedConflictNoQc,
  \* @type: Bool;
  BugAcceptLockedConflictEqualQc,
  \* @type: Bool;
  BugAcceptLockedConflictLowerQc,
  \* @type: Bool;
  BugRejectUnlocked,
  \* @type: Bool;
  BugRejectLockedSubject,
  \* @type: Bool;
  BugRejectHigherQc,
  \* @type: Bool;
  BugSkipValidationRequest,
  \* @type: Bool;
  BugSkipPrepareVote,
  \* @type: Bool;
  BugSkipPreparePhase

VARIABLES
  \* @type: Set(Str);
  tried,
  \* @type: Set(Str);
  accepted,
  \* @type: Set(Str);
  ignored,
  \* @type: Set(Str);
  validated,
  \* @type: Set(Str);
  prepared,
  \* @type: Set(Str);
  preparePhase

\* @type: <<Set(Str), Set(Str), Set(Str), Set(Str), Set(Str), Set(Str)>>;
vars == <<tried, accepted, ignored, validated, prepared, preparePhase>>

Candidates == {
  "safeUnlocked",
  "safeLockedSubject",
  "safeConflictHigherQc",
  "wrongPhase",
  "wrongHeight",
  "wrongEpoch",
  "wrongValidatorSet",
  "wrongView",
  "futureHeightHighest",
  "futureViewHighest",
  "wrongEpochHighest",
  "lockedConflictNoQc",
  "lockedConflictEqualQc",
  "lockedConflictLowerQc"
}

SpecAccepts(candidate) ==
  candidate \in {"safeUnlocked", "safeLockedSubject", "safeConflictHigherQc"}

BugRejectsSafe(candidate) ==
  \/ /\ candidate = "safeUnlocked"
     /\ BugRejectUnlocked
  \/ /\ candidate = "safeLockedSubject"
     /\ BugRejectLockedSubject
  \/ /\ candidate = "safeConflictHigherQc"
     /\ BugRejectHigherQc

BugAllowsUnsafe(candidate) ==
  \/ /\ candidate = "wrongPhase"
     /\ BugAcceptWrongPhase
  \/ /\ candidate \in {"wrongHeight", "wrongEpoch", "wrongValidatorSet", "wrongView"}
     /\ BugAcceptWrongRound
  \/ /\ candidate \in {"futureHeightHighest", "futureViewHighest", "wrongEpochHighest"}
     /\ BugAcceptIncompatibleHighest
  \/ /\ candidate = "lockedConflictNoQc"
     /\ BugAcceptLockedConflictNoQc
  \/ /\ candidate = "lockedConflictEqualQc"
     /\ BugAcceptLockedConflictEqualQc
  \/ /\ candidate = "lockedConflictLowerQc"
     /\ BugAcceptLockedConflictLowerQc

ImplementationAccepts(candidate) ==
  IF SpecAccepts(candidate)
  THEN ~BugRejectsSafe(candidate)
  ELSE BugAllowsUnsafe(candidate)

ImplementationValidates(candidate) ==
  ImplementationAccepts(candidate) /\ ~BugSkipValidationRequest

ImplementationPrepares(candidate) ==
  ImplementationAccepts(candidate) /\ ~BugSkipPrepareVote

ImplementationAdvances(candidate) ==
  ImplementationAccepts(candidate) /\ ~BugSkipPreparePhase

TypeInvariant ==
  /\ BugAcceptWrongPhase \in BOOLEAN
  /\ BugAcceptWrongRound \in BOOLEAN
  /\ BugAcceptIncompatibleHighest \in BOOLEAN
  /\ BugAcceptLockedConflictNoQc \in BOOLEAN
  /\ BugAcceptLockedConflictEqualQc \in BOOLEAN
  /\ BugAcceptLockedConflictLowerQc \in BOOLEAN
  /\ BugRejectUnlocked \in BOOLEAN
  /\ BugRejectLockedSubject \in BOOLEAN
  /\ BugRejectHigherQc \in BOOLEAN
  /\ BugSkipValidationRequest \in BOOLEAN
  /\ BugSkipPrepareVote \in BOOLEAN
  /\ BugSkipPreparePhase \in BOOLEAN
  /\ tried \subseteq Candidates
  /\ accepted \subseteq Candidates
  /\ ignored \subseteq Candidates
  /\ validated \subseteq Candidates
  /\ prepared \subseteq Candidates
  /\ preparePhase \subseteq Candidates
  /\ accepted \cap ignored = {}
  /\ accepted \cup ignored = tried
  /\ validated \subseteq accepted
  /\ prepared \subseteq accepted
  /\ preparePhase \subseteq accepted

Init ==
  /\ tried = {}
  /\ accepted = {}
  /\ ignored = {}
  /\ validated = {}
  /\ prepared = {}
  /\ preparePhase = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}
  /\ IF ImplementationAccepts(candidate)
     THEN
       /\ accepted' = accepted \cup {candidate}
       /\ ignored' = ignored
     ELSE
       /\ accepted' = accepted
       /\ ignored' = ignored \cup {candidate}
  /\ IF ImplementationValidates(candidate)
     THEN validated' = validated \cup {candidate}
     ELSE validated' = validated
  /\ IF ImplementationPrepares(candidate)
     THEN prepared' = prepared \cup {candidate}
     ELSE prepared' = prepared
  /\ IF ImplementationAdvances(candidate)
     THEN preparePhase' = preparePhase \cup {candidate}
     ELSE preparePhase' = preparePhase

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

AcceptedMatchesSpec ==
  accepted \subseteq {candidate \in Candidates : SpecAccepts(candidate)}

IgnoredMatchesSpec ==
  ignored \subseteq {candidate \in Candidates : ~SpecAccepts(candidate)}

SafeProposalsValidate ==
  \A candidate \in tried:
    SpecAccepts(candidate) => candidate \in validated

SafeProposalsSignPrepare ==
  \A candidate \in tried:
    SpecAccepts(candidate) => candidate \in prepared

SafeProposalsEnterPreparePhase ==
  \A candidate \in tried:
    SpecAccepts(candidate) => candidate \in preparePhase

UnsafeProposalsAreIgnored ==
  \A candidate \in tried:
    ~SpecAccepts(candidate) => candidate \in ignored

WrongPhaseNeverAccepted ==
  "wrongPhase" \notin accepted

WrongRoundNeverAccepted ==
  /\ "wrongHeight" \notin accepted
  /\ "wrongEpoch" \notin accepted
  /\ "wrongValidatorSet" \notin accepted
  /\ "wrongView" \notin accepted

IncompatibleHighestNeverAccepted ==
  /\ "futureHeightHighest" \notin accepted
  /\ "futureViewHighest" \notin accepted
  /\ "wrongEpochHighest" \notin accepted

LockedConflictWithoutUnlockNeverAccepted ==
  /\ "lockedConflictNoQc" \notin accepted
  /\ "lockedConflictEqualQc" \notin accepted
  /\ "lockedConflictLowerQc" \notin accepted

AcceptedProposalsRequestValidation ==
  accepted \subseteq validated

AcceptedProposalsSignPrepareVote ==
  accepted \subseteq prepared

AcceptedProposalsEnterPrepare ==
  accepted \subseteq preparePhase

IgnoredProposalsDoNotEmit ==
  /\ ignored \cap validated = {}
  /\ ignored \cap prepared = {}
  /\ ignored \cap preparePhase = {}

OutputsStayTogether ==
  /\ validated = prepared
  /\ validated = preparePhase

EngineProposalExactness ==
  /\ AcceptedMatchesSpec
  /\ IgnoredMatchesSpec
  /\ SafeProposalsValidate
  /\ SafeProposalsSignPrepare
  /\ SafeProposalsEnterPreparePhase
  /\ UnsafeProposalsAreIgnored
  /\ WrongPhaseNeverAccepted
  /\ WrongRoundNeverAccepted
  /\ IncompatibleHighestNeverAccepted
  /\ LockedConflictWithoutUnlockNeverAccepted
  /\ AcceptedProposalsRequestValidation
  /\ AcceptedProposalsSignPrepareVote
  /\ AcceptedProposalsEnterPrepare
  /\ IgnoredProposalsDoNotEmit
  /\ OutputsStayTogether

Safety ==
  EngineProposalExactness

EngineProposalCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineProposalExactness

=============================================================================
====
