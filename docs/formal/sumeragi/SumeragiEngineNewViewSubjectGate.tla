---- MODULE SumeragiEngineNewViewSubjectGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for pure-engine NewView vote subject projection.

This slice models the subject selected by `ConsensusEngine::on_tick(...)` and
the invalid branch of `ConsensusEngine::on_validation_result(...)` when they
emit a NewView vote. If a highest QC exists, both paths project the QC subject
with `qc_subject(...)`: parent block and block hash are both the QC subject
hash, the payload hash is zero, and the same QC reference is bound into the
vote. If no highest QC exists, ticks sign `zero_subject()`, while invalid
validation signs the rejected block hash as both parent and block with a zero
payload hash. No-highest votes must not bind a highest-QC reference.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugUseZeroDespiteHighest,
  \* @type: Bool;
  BugUseInvalidDespiteHighest,
  \* @type: Bool;
  BugUseHighestWithoutHighest,
  \* @type: Bool;
  BugTickNoHighestUsesInvalid,
  \* @type: Bool;
  BugInvalidNoHighestUsesZero,
  \* @type: Bool;
  BugParentNotSubjectHash,
  \* @type: Bool;
  BugBlockNotSubjectHash,
  \* @type: Bool;
  BugPayloadNotZero,
  \* @type: Bool;
  BugOmitHighestBinding,
  \* @type: Bool;
  BugBindHighestWithoutHighest

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "tick_no_highest",
  "tick_prepare_highest",
  "tick_commit_highest",
  "tick_new_view_highest",
  "invalid_no_highest",
  "invalid_prepare_highest",
  "invalid_commit_highest",
  "invalid_new_view_highest"
}

Sources == {"qc", "zero", "invalid", "other"}

IsTick(candidate) ==
  candidate \in {
    "tick_no_highest",
    "tick_prepare_highest",
    "tick_commit_highest",
    "tick_new_view_highest"
  }

IsInvalidValidation(candidate) ==
  candidate \in {
    "invalid_no_highest",
    "invalid_prepare_highest",
    "invalid_commit_highest",
    "invalid_new_view_highest"
  }

HasHighest(candidate) ==
  candidate \notin {"tick_no_highest", "invalid_no_highest"}

SpecSubjectSource(candidate) ==
  CASE HasHighest(candidate) -> "qc"
    [] IsTick(candidate) -> "zero"
    [] OTHER -> "invalid"

ImplementationSubjectSource(candidate) ==
  CASE /\ HasHighest(candidate)
       /\ BugUseZeroDespiteHighest -> "zero"
    [] /\ HasHighest(candidate)
       /\ IsInvalidValidation(candidate)
       /\ BugUseInvalidDespiteHighest -> "invalid"
    [] /\ ~HasHighest(candidate)
       /\ BugUseHighestWithoutHighest -> "qc"
    [] /\ candidate = "tick_no_highest"
       /\ BugTickNoHighestUsesInvalid -> "invalid"
    [] /\ candidate = "invalid_no_highest"
       /\ BugInvalidNoHighestUsesZero -> "zero"
    [] OTHER -> SpecSubjectSource(candidate)

ImplementationParent(candidate) ==
  IF BugParentNotSubjectHash
  THEN "other"
  ELSE ImplementationSubjectSource(candidate)

ImplementationBlock(candidate) ==
  IF BugBlockNotSubjectHash
  THEN "other"
  ELSE ImplementationSubjectSource(candidate)

ImplementationPayload(candidate) ==
  IF BugPayloadNotZero
  THEN "other"
  ELSE "zero"

ImplementationHighestBound(candidate) ==
  IF HasHighest(candidate)
  THEN ~BugOmitHighestBinding
  ELSE BugBindHighestWithoutHighest

TypeInvariant ==
  /\ BugUseZeroDespiteHighest \in BOOLEAN
  /\ BugUseInvalidDespiteHighest \in BOOLEAN
  /\ BugUseHighestWithoutHighest \in BOOLEAN
  /\ BugTickNoHighestUsesInvalid \in BOOLEAN
  /\ BugInvalidNoHighestUsesZero \in BOOLEAN
  /\ BugParentNotSubjectHash \in BOOLEAN
  /\ BugBlockNotSubjectHash \in BOOLEAN
  /\ BugPayloadNotZero \in BOOLEAN
  /\ BugOmitHighestBinding \in BOOLEAN
  /\ BugBindHighestWithoutHighest \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ ImplementationSubjectSource(candidate) \in Sources
    /\ ImplementationParent(candidate) \in Sources
    /\ ImplementationBlock(candidate) \in Sources
    /\ ImplementationPayload(candidate) \in Sources

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

SubjectFieldsMatchSpec ==
  \A candidate \in tried:
    /\ ImplementationParent(candidate) = SpecSubjectSource(candidate)
    /\ ImplementationBlock(candidate) = SpecSubjectSource(candidate)
    /\ ImplementationPayload(candidate) = "zero"

HighestCasesUseQcSubject ==
  \A candidate \in tried:
    HasHighest(candidate) =>
      /\ ImplementationParent(candidate) = "qc"
      /\ ImplementationBlock(candidate) = "qc"
      /\ ImplementationPayload(candidate) = "zero"

TickFallbackUsesZeroSubject ==
  \A candidate \in tried:
    /\ candidate = "tick_no_highest" =>
      /\ ImplementationParent(candidate) = "zero"
      /\ ImplementationBlock(candidate) = "zero"
      /\ ImplementationPayload(candidate) = "zero"

InvalidFallbackUsesRejectedBlockSubject ==
  \A candidate \in tried:
    /\ candidate = "invalid_no_highest" =>
      /\ ImplementationParent(candidate) = "invalid"
      /\ ImplementationBlock(candidate) = "invalid"
      /\ ImplementationPayload(candidate) = "zero"

HighestBindingMatchesPresence ==
  \A candidate \in tried:
    ImplementationHighestBound(candidate) = HasHighest(candidate)

PayloadHashAlwaysZero ==
  \A candidate \in tried:
    ImplementationPayload(candidate) = "zero"

EngineNewViewSubjectExactness ==
  /\ SubjectFieldsMatchSpec
  /\ HighestCasesUseQcSubject
  /\ TickFallbackUsesZeroSubject
  /\ InvalidFallbackUsesRejectedBlockSubject
  /\ HighestBindingMatchesPresence
  /\ PayloadHashAlwaysZero

Safety ==
  EngineNewViewSubjectExactness

EngineNewViewSubjectCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineNewViewSubjectExactness

=============================================================================
====
