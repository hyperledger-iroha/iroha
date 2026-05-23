---- MODULE SumeragiEngineValidationResultGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for the pure Sumeragi engine validation-result gate.

This slice models `ConsensusEngine::on_validation_result(...)`. Validation
callbacks may affect consensus only when their round matches the current round
and their block hash matches the exact in-flight proposal. A valid current
result consumes validation ownership without emitting consensus outputs. An
invalid current result consumes ownership, advances the view, returns to
proposal phase, signs one NewView vote, and emits one AdvanceView output.

Invalid NewView votes bind the local highest-QC subject/highest-QC reference
when one exists; otherwise they bind the invalid proposal hash as the fallback
subject with no highest-QC reference. Wrong-round, wrong-block, replayed,
no-in-flight, commit-superseded, and storage-committed callbacks are ignored.
Superseded callbacks must not drop pending finality or overwrite committed
height state.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAcceptWrongRound,
  \* @type: Bool;
  BugAcceptWrongBlockHash,
  \* @type: Bool;
  BugAcceptNoInflight,
  \* @type: Bool;
  BugAcceptSuperseded,
  \* @type: Bool;
  BugRejectCurrentValid,
  \* @type: Bool;
  BugRejectCurrentInvalid,
  \* @type: Bool;
  BugKeepValidationInflight,
  \* @type: Bool;
  BugValidEmitsOutput,
  \* @type: Bool;
  BugSkipRoundAdvance,
  \* @type: Bool;
  BugSkipNewViewVote,
  \* @type: Bool;
  BugSkipAdvanceOutput,
  \* @type: Bool;
  BugWrongPhaseAfterInvalid,
  \* @type: Bool;
  BugUseInvalidSubjectDespiteHighest,
  \* @type: Bool;
  BugUseHighestSubjectWithoutHighest,
  \* @type: Bool;
  BugOmitHighestBinding,
  \* @type: Bool;
  BugBindHighestWithoutHighest,
  \* @type: Bool;
  BugDropPendingFinality,
  \* @type: Bool;
  BugOverwriteCommitted

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Candidates == {
  "validCurrent",
  "invalidNoHighest",
  "invalidWithHighest",
  "wrongRound",
  "wrongBlockHash",
  "noInflight",
  "replayAfterValid",
  "supersededByCommit",
  "supersededByCommittedBlock"
}

CurrentValid(candidate) ==
  candidate = "validCurrent"

CurrentInvalid(candidate) ==
  candidate \in {"invalidNoHighest", "invalidWithHighest"}

SpecAccepts(candidate) ==
  CurrentValid(candidate) \/ CurrentInvalid(candidate)

WrongRound(candidate) ==
  candidate = "wrongRound"

WrongBlockHash(candidate) ==
  candidate = "wrongBlockHash"

NoInflight(candidate) ==
  candidate \in {"noInflight", "replayAfterValid"}

Superseded(candidate) ==
  candidate \in {"supersededByCommit", "supersededByCommittedBlock"}

HasHighest(candidate) ==
  candidate = "invalidWithHighest"

HasPendingFinality(candidate) ==
  candidate = "supersededByCommit"

HasCommitted(candidate) ==
  candidate = "supersededByCommittedBlock"

BugRejectsCurrent(candidate) ==
  \/ /\ CurrentValid(candidate)
     /\ BugRejectCurrentValid
  \/ /\ CurrentInvalid(candidate)
     /\ BugRejectCurrentInvalid

BugAllowsUnsafe(candidate) ==
  \/ /\ WrongRound(candidate)
     /\ BugAcceptWrongRound
  \/ /\ WrongBlockHash(candidate)
     /\ BugAcceptWrongBlockHash
  \/ /\ NoInflight(candidate)
     /\ BugAcceptNoInflight
  \/ /\ Superseded(candidate)
     /\ BugAcceptSuperseded

Accepted(candidate) ==
  IF SpecAccepts(candidate)
  THEN ~BugRejectsCurrent(candidate)
  ELSE BugAllowsUnsafe(candidate)

Ignored(candidate) ==
  ~Accepted(candidate)

ValidationCleared(candidate) ==
  /\ Accepted(candidate)
  /\ SpecAccepts(candidate)
  /\ ~BugKeepValidationInflight

RoundAdvanced(candidate) ==
  /\ Accepted(candidate)
  /\ CurrentInvalid(candidate)
  /\ ~BugSkipRoundAdvance

RoundPreserved(candidate) ==
  /\ Accepted(candidate)
  /\ CurrentValid(candidate)

ProposalPhase(candidate) ==
  /\ Accepted(candidate)
  /\ CurrentInvalid(candidate)
  /\ ~BugWrongPhaseAfterInvalid

PreparePhase(candidate) ==
  /\ Accepted(candidate)
  /\ CurrentValid(candidate)

SignedNewView(candidate) ==
  \/ /\ Accepted(candidate)
     /\ CurrentInvalid(candidate)
     /\ ~BugSkipNewViewVote
  \/ /\ Accepted(candidate)
     /\ CurrentValid(candidate)
     /\ BugValidEmitsOutput

AdvanceOutput(candidate) ==
  \/ /\ Accepted(candidate)
     /\ CurrentInvalid(candidate)
     /\ ~BugSkipAdvanceOutput
  \/ /\ Accepted(candidate)
     /\ CurrentValid(candidate)
     /\ BugValidEmitsOutput

OutputEmpty(candidate) ==
  /\ ~SignedNewView(candidate)
  /\ ~AdvanceOutput(candidate)

SubjectFromHighest(candidate) ==
  /\ SignedNewView(candidate)
  /\ CurrentInvalid(candidate)
  /\ \/ /\ HasHighest(candidate)
        /\ ~BugUseInvalidSubjectDespiteHighest
     \/ /\ ~HasHighest(candidate)
        /\ BugUseHighestSubjectWithoutHighest

SubjectFromInvalid(candidate) ==
  /\ SignedNewView(candidate)
  /\ CurrentInvalid(candidate)
  /\ \/ /\ ~HasHighest(candidate)
        /\ ~BugUseHighestSubjectWithoutHighest
     \/ /\ HasHighest(candidate)
        /\ BugUseInvalidSubjectDespiteHighest

HighestBound(candidate) ==
  /\ SignedNewView(candidate)
  /\ CurrentInvalid(candidate)
  /\ \/ /\ HasHighest(candidate)
        /\ ~BugOmitHighestBinding
     \/ /\ ~HasHighest(candidate)
        /\ BugBindHighestWithoutHighest

PendingPreserved(candidate) ==
  /\ HasPendingFinality(candidate)
  /\ ~BugDropPendingFinality

CommittedPreserved(candidate) ==
  /\ HasCommitted(candidate)
  /\ ~BugOverwriteCommitted

TypeInvariant ==
  /\ BugAcceptWrongRound \in BOOLEAN
  /\ BugAcceptWrongBlockHash \in BOOLEAN
  /\ BugAcceptNoInflight \in BOOLEAN
  /\ BugAcceptSuperseded \in BOOLEAN
  /\ BugRejectCurrentValid \in BOOLEAN
  /\ BugRejectCurrentInvalid \in BOOLEAN
  /\ BugKeepValidationInflight \in BOOLEAN
  /\ BugValidEmitsOutput \in BOOLEAN
  /\ BugSkipRoundAdvance \in BOOLEAN
  /\ BugSkipNewViewVote \in BOOLEAN
  /\ BugSkipAdvanceOutput \in BOOLEAN
  /\ BugWrongPhaseAfterInvalid \in BOOLEAN
  /\ BugUseInvalidSubjectDespiteHighest \in BOOLEAN
  /\ BugUseHighestSubjectWithoutHighest \in BOOLEAN
  /\ BugOmitHighestBinding \in BOOLEAN
  /\ BugBindHighestWithoutHighest \in BOOLEAN
  /\ BugDropPendingFinality \in BOOLEAN
  /\ BugOverwriteCommitted \in BOOLEAN
  /\ tried \subseteq Candidates

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

AcceptedMatchesSpec ==
  \A candidate \in tried:
    Accepted(candidate) <=> SpecAccepts(candidate)

IgnoredMatchesSpec ==
  \A candidate \in tried:
    Ignored(candidate) <=> ~SpecAccepts(candidate)

CurrentValidConsumesAndStops ==
  \A candidate \in tried:
    CurrentValid(candidate) =>
      /\ Accepted(candidate)
      /\ ValidationCleared(candidate)
      /\ RoundPreserved(candidate)
      /\ PreparePhase(candidate)
      /\ OutputEmpty(candidate)

CurrentInvalidAdvancesView ==
  \A candidate \in tried:
    CurrentInvalid(candidate) =>
      /\ Accepted(candidate)
      /\ ValidationCleared(candidate)
      /\ RoundAdvanced(candidate)
      /\ ProposalPhase(candidate)
      /\ SignedNewView(candidate)
      /\ AdvanceOutput(candidate)

IgnoredCallbacksHaveNoOutputs ==
  \A candidate \in tried:
    ~SpecAccepts(candidate) =>
      /\ Ignored(candidate)
      /\ OutputEmpty(candidate)

InvalidWithHighestUsesHighestSubject ==
  \A candidate \in tried:
    candidate = "invalidWithHighest" => SubjectFromHighest(candidate)

InvalidWithoutHighestUsesInvalidSubject ==
  \A candidate \in tried:
    candidate = "invalidNoHighest" => SubjectFromInvalid(candidate)

InvalidWithHighestBindsHighestQc ==
  \A candidate \in tried:
    candidate = "invalidWithHighest" => HighestBound(candidate)

InvalidWithoutHighestDoesNotBindHighestQc ==
  \A candidate \in tried:
    candidate = "invalidNoHighest" => ~HighestBound(candidate)

SupersededCommitPreservesPendingFinality ==
  \A candidate \in tried:
    HasPendingFinality(candidate) => PendingPreserved(candidate)

SupersededCommittedBlockPreservesCommittedState ==
  \A candidate \in tried:
    HasCommitted(candidate) => CommittedPreserved(candidate)

CurrentValidNeverEmitsNewView ==
  \A candidate \in tried:
    CurrentValid(candidate) =>
      /\ ~SignedNewView(candidate)
      /\ ~AdvanceOutput(candidate)
      /\ ~RoundAdvanced(candidate)
      /\ ~ProposalPhase(candidate)

OutputsStayTogether ==
  \A candidate \in tried:
    CurrentInvalid(candidate) =>
      /\ (SignedNewView(candidate) <=> AdvanceOutput(candidate))
      /\ (SignedNewView(candidate) <=> RoundAdvanced(candidate))
      /\ (SignedNewView(candidate) <=> ProposalPhase(candidate))

====
