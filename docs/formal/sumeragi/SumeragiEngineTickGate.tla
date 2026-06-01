---- MODULE SumeragiEngineTickGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for the pure Sumeragi engine pacemaker tick gate.

This slice models `ConsensusEngine::on_tick(...)`. Every tick advances the
local view by one, returns the engine to proposal phase, clears any in-flight
proposal validation, emits one NewView vote, and emits one AdvanceView output.
If a highest QC is known, the NewView vote must use that QC subject and carry
the same highest-QC reference. Without a highest QC, the vote must use the zero
subject and carry no highest-QC reference. Pending finality is intentionally
preserved across ticks so certified payload recovery can complete later.

The model enumerates the finite state shapes that matter for this boundary.
The implementation transition records whether each tick advanced, signed,
emitted the view-advance output, moved to proposal phase, cleared validation,
preserved pending finality, and bound the correct NewView subject/highest-QC
fields.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipRoundAdvance,
  \* @type: Bool;
  BugSkipNewViewVote,
  \* @type: Bool;
  BugSkipAdvanceOutput,
  \* @type: Bool;
  BugWrongPhaseAfterTick,
  \* @type: Bool;
  BugKeepValidationInflight,
  \* @type: Bool;
  BugDropPendingFinality,
  \* @type: Bool;
  BugUseZeroDespiteHighest,
  \* @type: Bool;
  BugUseHighestWithoutHighest,
  \* @type: Bool;
  BugOmitHighestBinding,
  \* @type: Bool;
  BugBindHighestWithoutHighest

VARIABLES
  \* @type: Set(Str);
  tried,
  \* @type: Set(Str);
  advanced,
  \* @type: Set(Str);
  signed,
  \* @type: Set(Str);
  advanceOutput,
  \* @type: Set(Str);
  proposalPhase,
  \* @type: Set(Str);
  validationCleared,
  \* @type: Set(Str);
  pendingPreserved,
  \* @type: Set(Str);
  subjectFromHighest,
  \* @type: Set(Str);
  subjectZero,
  \* @type: Set(Str);
  highestBound

\* @type: <<Set(Str), Set(Str), Set(Str), Set(Str), Set(Str), Set(Str), Set(Str), Set(Str), Set(Str), Set(Str)>>;
vars == <<
  tried,
  advanced,
  signed,
  advanceOutput,
  proposalPhase,
  validationCleared,
  pendingPreserved,
  subjectFromHighest,
  subjectZero,
  highestBound
>>

Candidates == {
  "noHighestIdle",
  "highestIdle",
  "validationNoHighest",
  "validationWithHighest",
  "pendingFinalityWithHighest"
}

HasHighest(candidate) ==
  candidate \in {"highestIdle", "validationWithHighest", "pendingFinalityWithHighest"}

HasValidation(candidate) ==
  candidate \in {"validationNoHighest", "validationWithHighest"}

HasPendingFinality(candidate) ==
  candidate = "pendingFinalityWithHighest"

ImplementationAdvanced ==
  ~BugSkipRoundAdvance

ImplementationSigned ==
  ~BugSkipNewViewVote

ImplementationAdvanceOutput ==
  ~BugSkipAdvanceOutput

ImplementationProposalPhase ==
  ~BugWrongPhaseAfterTick

ImplementationValidationCleared(candidate) ==
  ~HasValidation(candidate) \/ ~BugKeepValidationInflight

ImplementationPendingPreserved(candidate) ==
  ~HasPendingFinality(candidate) \/ ~BugDropPendingFinality

ImplementationSubjectFromHighest(candidate) ==
  /\ ImplementationSigned
  /\ \/ /\ HasHighest(candidate)
        /\ ~BugUseZeroDespiteHighest
     \/ /\ ~HasHighest(candidate)
        /\ BugUseHighestWithoutHighest

ImplementationSubjectZero(candidate) ==
  /\ ImplementationSigned
  /\ \/ /\ ~HasHighest(candidate)
        /\ ~BugUseHighestWithoutHighest
     \/ /\ HasHighest(candidate)
        /\ BugUseZeroDespiteHighest

ImplementationHighestBound(candidate) ==
  /\ ImplementationSigned
  /\ \/ /\ HasHighest(candidate)
        /\ ~BugOmitHighestBinding
     \/ /\ ~HasHighest(candidate)
        /\ BugBindHighestWithoutHighest

TypeInvariant ==
  /\ BugSkipRoundAdvance \in BOOLEAN
  /\ BugSkipNewViewVote \in BOOLEAN
  /\ BugSkipAdvanceOutput \in BOOLEAN
  /\ BugWrongPhaseAfterTick \in BOOLEAN
  /\ BugKeepValidationInflight \in BOOLEAN
  /\ BugDropPendingFinality \in BOOLEAN
  /\ BugUseZeroDespiteHighest \in BOOLEAN
  /\ BugUseHighestWithoutHighest \in BOOLEAN
  /\ BugOmitHighestBinding \in BOOLEAN
  /\ BugBindHighestWithoutHighest \in BOOLEAN
  /\ tried \subseteq Candidates
  /\ advanced \subseteq Candidates
  /\ signed \subseteq Candidates
  /\ advanceOutput \subseteq Candidates
  /\ proposalPhase \subseteq Candidates
  /\ validationCleared \subseteq Candidates
  /\ pendingPreserved \subseteq Candidates
  /\ subjectFromHighest \subseteq Candidates
  /\ subjectZero \subseteq Candidates
  /\ highestBound \subseteq Candidates
  /\ subjectFromHighest \cap subjectZero = {}
  /\ subjectFromHighest \cup subjectZero = signed
  /\ highestBound \subseteq signed

Init ==
  /\ tried = {}
  /\ advanced = {}
  /\ signed = {}
  /\ advanceOutput = {}
  /\ proposalPhase = {}
  /\ validationCleared = {}
  /\ pendingPreserved = {}
  /\ subjectFromHighest = {}
  /\ subjectZero = {}
  /\ highestBound = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}
  /\ IF ImplementationAdvanced
     THEN advanced' = advanced \cup {candidate}
     ELSE advanced' = advanced
  /\ IF ImplementationSigned
     THEN signed' = signed \cup {candidate}
     ELSE signed' = signed
  /\ IF ImplementationAdvanceOutput
     THEN advanceOutput' = advanceOutput \cup {candidate}
     ELSE advanceOutput' = advanceOutput
  /\ IF ImplementationProposalPhase
     THEN proposalPhase' = proposalPhase \cup {candidate}
     ELSE proposalPhase' = proposalPhase
  /\ IF ImplementationValidationCleared(candidate)
     THEN validationCleared' = validationCleared \cup {candidate}
     ELSE validationCleared' = validationCleared
  /\ IF ImplementationPendingPreserved(candidate)
     THEN pendingPreserved' = pendingPreserved \cup {candidate}
     ELSE pendingPreserved' = pendingPreserved
  /\ IF ImplementationSubjectFromHighest(candidate)
     THEN subjectFromHighest' = subjectFromHighest \cup {candidate}
     ELSE subjectFromHighest' = subjectFromHighest
  /\ IF ImplementationSubjectZero(candidate)
     THEN subjectZero' = subjectZero \cup {candidate}
     ELSE subjectZero' = subjectZero
  /\ IF ImplementationHighestBound(candidate)
     THEN highestBound' = highestBound \cup {candidate}
     ELSE highestBound' = highestBound

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

EveryTickAdvancesView ==
  tried \subseteq advanced

EveryTickSignsNewView ==
  tried \subseteq signed

EveryTickEmitsAdvanceView ==
  tried \subseteq advanceOutput

EveryTickEntersProposalPhase ==
  tried \subseteq proposalPhase

TicksClearInflightValidation ==
  \A candidate \in tried:
    HasValidation(candidate) => candidate \in validationCleared

TicksPreservePendingFinality ==
  \A candidate \in tried:
    HasPendingFinality(candidate) => candidate \in pendingPreserved

HighestTicksUseHighestSubject ==
  \A candidate \in tried:
    HasHighest(candidate) => candidate \in subjectFromHighest

NoHighestTicksUseZeroSubject ==
  \A candidate \in tried:
    ~HasHighest(candidate) => candidate \in subjectZero

HighestTicksBindHighestQc ==
  \A candidate \in tried:
    HasHighest(candidate) => candidate \in highestBound

NoHighestTicksDoNotBindHighestQc ==
  \A candidate \in tried:
    ~HasHighest(candidate) => candidate \notin highestBound

SignedTicksHaveConsistentOutputs ==
  /\ signed = advanced
  /\ signed = advanceOutput
  /\ signed = proposalPhase

=============================================================================
====
