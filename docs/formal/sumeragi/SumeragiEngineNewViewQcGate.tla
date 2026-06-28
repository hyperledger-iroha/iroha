---- MODULE SumeragiEngineNewViewQcGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for the pure Sumeragi engine NewView-QC gate.

This slice models `ConsensusEngine::on_certificate(...)` dispatching an
accepted NewView certificate into `on_new_view_qc(...)`. NewView certificates
may advance the local view only when they match the current height, epoch,
validator set, and quorum policy, carry a strictly newer view, and contain no
highest-QC reference that points beyond the certificate round. Accepted
NewView QCs move the engine back to proposal phase, clear any in-flight
proposal validation, preserve pending finality, emit one `AdvanceView` output,
and update highest-QC state only when the carried QC improves the local
highest-QC reference.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugAcceptWrongContext,
  \* @type: Bool;
  BugAcceptWrongQuorumPolicy,
  \* @type: Bool;
  BugAcceptStaleOrSameView,
  \* @type: Bool;
  BugAcceptIncompatibleHighest,
  \* @type: Bool;
  BugRejectSafeNoHighest,
  \* @type: Bool;
  BugRejectSafeImprovingHighest,
  \* @type: Bool;
  BugRejectSafeLowerHighest,
  \* @type: Bool;
  BugSkipAdvanceOutput,
  \* @type: Bool;
  BugWrongPhaseAfterAccept,
  \* @type: Bool;
  BugKeepValidationInflight,
  \* @type: Bool;
  BugDropPendingFinality,
  \* @type: Bool;
  BugOverwriteHighestWithLower,
  \* @type: Bool;
  BugSkipHighestRecord

VARIABLES
  \* @type: Set(Str);
  tried,
  \* @type: Set(Str);
  accepted,
  \* @type: Set(Str);
  ignored,
  \* @type: Set(Str);
  advanceOutput,
  \* @type: Set(Str);
  proposalPhase,
  \* @type: Set(Str);
  validationCleared,
  \* @type: Set(Str);
  pendingPreserved,
  \* @type: Set(Str);
  highestImproved,
  \* @type: Set(Str);
  highestPreserved

\* @type: <<Set(Str), Set(Str), Set(Str), Set(Str), Set(Str), Set(Str), Set(Str), Set(Str), Set(Str)>>;
vars == <<
  tried,
  accepted,
  ignored,
  advanceOutput,
  proposalPhase,
  validationCleared,
  pendingPreserved,
  highestImproved,
  highestPreserved
>>

Candidates == {
  "safeNoHighest",
  "safeImprovingHighest",
  "safeLowerHighest",
  "validationSafeNoHighest",
  "pendingSafeImprovingHighest",
  "wrongHeight",
  "wrongEpoch",
  "wrongValidatorSet",
  "wrongQuorumPolicy",
  "sameView",
  "lowerView",
  "futureHeightHighest",
  "futureViewHighest",
  "wrongEpochHighest"
}

SpecAccepts(candidate) ==
  candidate \in {
    "safeNoHighest",
    "safeImprovingHighest",
    "safeLowerHighest",
    "validationSafeNoHighest",
    "pendingSafeImprovingHighest"
  }

WrongContext(candidate) ==
  candidate \in {"wrongHeight", "wrongEpoch", "wrongValidatorSet"}

WrongQuorumPolicy(candidate) ==
  candidate = "wrongQuorumPolicy"

StaleOrSameView(candidate) ==
  candidate \in {"sameView", "lowerView"}

IncompatibleHighest(candidate) ==
  candidate \in {
    "futureHeightHighest",
    "futureViewHighest",
    "wrongEpochHighest"
  }

HasHighest(candidate) ==
  candidate \in {
    "safeImprovingHighest",
    "safeLowerHighest",
    "pendingSafeImprovingHighest",
    "futureHeightHighest",
    "futureViewHighest",
    "wrongEpochHighest"
  }

ImprovingHighest(candidate) ==
  candidate \in {"safeImprovingHighest", "pendingSafeImprovingHighest"}

LowerHighest(candidate) ==
  candidate = "safeLowerHighest"

HasValidation(candidate) ==
  candidate = "validationSafeNoHighest"

HasPendingFinality(candidate) ==
  candidate = "pendingSafeImprovingHighest"

BugRejectsSafe(candidate) ==
  \/ /\ ~HasHighest(candidate)
     /\ BugRejectSafeNoHighest
  \/ /\ ImprovingHighest(candidate)
     /\ BugRejectSafeImprovingHighest
  \/ /\ LowerHighest(candidate)
     /\ BugRejectSafeLowerHighest

BugAllowsUnsafe(candidate) ==
  \/ /\ WrongContext(candidate)
     /\ BugAcceptWrongContext
  \/ /\ WrongQuorumPolicy(candidate)
     /\ BugAcceptWrongQuorumPolicy
  \/ /\ StaleOrSameView(candidate)
     /\ BugAcceptStaleOrSameView
  \/ /\ IncompatibleHighest(candidate)
     /\ BugAcceptIncompatibleHighest

ImplementationAccepts(candidate) ==
  IF SpecAccepts(candidate)
  THEN ~BugRejectsSafe(candidate)
  ELSE BugAllowsUnsafe(candidate)

ImplementationAdvanceOutput(candidate) ==
  ImplementationAccepts(candidate) /\ ~BugSkipAdvanceOutput

ImplementationProposalPhase(candidate) ==
  ImplementationAccepts(candidate) /\ ~BugWrongPhaseAfterAccept

ImplementationValidationCleared(candidate) ==
  /\ ImplementationAccepts(candidate)
  /\ (~HasValidation(candidate) \/ ~BugKeepValidationInflight)

ImplementationPendingPreserved(candidate) ==
  /\ ImplementationAccepts(candidate)
  /\ (~HasPendingFinality(candidate) \/ ~BugDropPendingFinality)

ImplementationHighestImproved(candidate) ==
  /\ ImplementationAccepts(candidate)
  /\ \/ /\ ImprovingHighest(candidate)
        /\ ~BugSkipHighestRecord
     \/ /\ LowerHighest(candidate)
        /\ BugOverwriteHighestWithLower

ImplementationHighestPreserved(candidate) ==
  /\ ImplementationAccepts(candidate)
  /\ \/ /\ LowerHighest(candidate)
        /\ ~BugOverwriteHighestWithLower
     \/ ~HasHighest(candidate)

TypeInvariant ==
  /\ BugAcceptWrongContext \in BOOLEAN
  /\ BugAcceptWrongQuorumPolicy \in BOOLEAN
  /\ BugAcceptStaleOrSameView \in BOOLEAN
  /\ BugAcceptIncompatibleHighest \in BOOLEAN
  /\ BugRejectSafeNoHighest \in BOOLEAN
  /\ BugRejectSafeImprovingHighest \in BOOLEAN
  /\ BugRejectSafeLowerHighest \in BOOLEAN
  /\ BugSkipAdvanceOutput \in BOOLEAN
  /\ BugWrongPhaseAfterAccept \in BOOLEAN
  /\ BugKeepValidationInflight \in BOOLEAN
  /\ BugDropPendingFinality \in BOOLEAN
  /\ BugOverwriteHighestWithLower \in BOOLEAN
  /\ BugSkipHighestRecord \in BOOLEAN
  /\ tried \subseteq Candidates
  /\ accepted \subseteq Candidates
  /\ ignored \subseteq Candidates
  /\ advanceOutput \subseteq Candidates
  /\ proposalPhase \subseteq Candidates
  /\ validationCleared \subseteq Candidates
  /\ pendingPreserved \subseteq Candidates
  /\ highestImproved \subseteq Candidates
  /\ highestPreserved \subseteq Candidates
  /\ accepted \cap ignored = {}
  /\ accepted \cup ignored = tried
  /\ advanceOutput \subseteq accepted
  /\ proposalPhase \subseteq accepted
  /\ validationCleared \subseteq accepted
  /\ pendingPreserved \subseteq accepted
  /\ highestImproved \subseteq accepted
  /\ highestPreserved \subseteq accepted
  /\ highestImproved \cap highestPreserved = {}

Init ==
  /\ tried = {}
  /\ accepted = {}
  /\ ignored = {}
  /\ advanceOutput = {}
  /\ proposalPhase = {}
  /\ validationCleared = {}
  /\ pendingPreserved = {}
  /\ highestImproved = {}
  /\ highestPreserved = {}

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
  /\ IF ImplementationAdvanceOutput(candidate)
     THEN advanceOutput' = advanceOutput \cup {candidate}
     ELSE advanceOutput' = advanceOutput
  /\ IF ImplementationProposalPhase(candidate)
     THEN proposalPhase' = proposalPhase \cup {candidate}
     ELSE proposalPhase' = proposalPhase
  /\ IF ImplementationValidationCleared(candidate)
     THEN validationCleared' = validationCleared \cup {candidate}
     ELSE validationCleared' = validationCleared
  /\ IF ImplementationPendingPreserved(candidate)
     THEN pendingPreserved' = pendingPreserved \cup {candidate}
     ELSE pendingPreserved' = pendingPreserved
  /\ IF ImplementationHighestImproved(candidate)
     THEN highestImproved' = highestImproved \cup {candidate}
     ELSE highestImproved' = highestImproved
  /\ IF ImplementationHighestPreserved(candidate)
     THEN highestPreserved' = highestPreserved \cup {candidate}
     ELSE highestPreserved' = highestPreserved

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

AcceptedMatchesSpec ==
  accepted \subseteq {candidate \in Candidates : SpecAccepts(candidate)}

IgnoredMatchesSpec ==
  ignored \subseteq {candidate \in Candidates : ~SpecAccepts(candidate)}

SafeNewViewQcsAdvance ==
  \A candidate \in tried:
    SpecAccepts(candidate) => candidate \in accepted

UnsafeNewViewQcsAreIgnored ==
  \A candidate \in tried:
    ~SpecAccepts(candidate) => candidate \in ignored

WrongContextNeverAccepted ==
  \A candidate \in Candidates:
    WrongContext(candidate) => candidate \notin accepted

WrongQuorumPolicyNeverAccepted ==
  "wrongQuorumPolicy" \notin accepted

StaleOrSameViewNeverAccepted ==
  \A candidate \in Candidates:
    StaleOrSameView(candidate) => candidate \notin accepted

IncompatibleHighestNeverAccepted ==
  \A candidate \in Candidates:
    IncompatibleHighest(candidate) => candidate \notin accepted

AcceptedNewViewQcsEmitAdvance ==
  accepted \subseteq advanceOutput

AcceptedNewViewQcsEnterProposal ==
  accepted \subseteq proposalPhase

AcceptedNewViewQcsClearValidation ==
  \A candidate \in accepted:
    HasValidation(candidate) => candidate \in validationCleared

AcceptedNewViewQcsPreservePendingFinality ==
  \A candidate \in accepted:
    HasPendingFinality(candidate) => candidate \in pendingPreserved

ImprovingHighestQcIsRecorded ==
  \A candidate \in tried:
    ImprovingHighest(candidate) => candidate \in highestImproved

LowerHighestQcDoesNotOverwrite ==
  \A candidate \in Candidates:
    LowerHighest(candidate) => candidate \notin highestImproved

AcceptedLowerHighestPreservesCurrentHighest ==
  \A candidate \in accepted:
    LowerHighest(candidate) => candidate \in highestPreserved

AcceptedNoHighestDoesNotChangeHighest ==
  \A candidate \in accepted:
    ~HasHighest(candidate) => candidate \in highestPreserved

IgnoredNewViewQcsDoNotMutate ==
  /\ ignored \cap advanceOutput = {}
  /\ ignored \cap proposalPhase = {}
  /\ ignored \cap validationCleared = {}
  /\ ignored \cap pendingPreserved = {}
  /\ ignored \cap highestImproved = {}
  /\ ignored \cap highestPreserved = {}

EngineNewViewQcExactness ==
  /\ AcceptedMatchesSpec
  /\ IgnoredMatchesSpec
  /\ SafeNewViewQcsAdvance
  /\ UnsafeNewViewQcsAreIgnored
  /\ WrongContextNeverAccepted
  /\ WrongQuorumPolicyNeverAccepted
  /\ StaleOrSameViewNeverAccepted
  /\ IncompatibleHighestNeverAccepted
  /\ AcceptedNewViewQcsEmitAdvance
  /\ AcceptedNewViewQcsEnterProposal
  /\ AcceptedNewViewQcsClearValidation
  /\ AcceptedNewViewQcsPreservePendingFinality
  /\ ImprovingHighestQcIsRecorded
  /\ LowerHighestQcDoesNotOverwrite
  /\ AcceptedLowerHighestPreservesCurrentHighest
  /\ AcceptedNoHighestDoesNotChangeHighest
  /\ IgnoredNewViewQcsDoNotMutate

Safety ==
  EngineNewViewQcExactness

EngineNewViewQcCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineNewViewQcExactness

====
