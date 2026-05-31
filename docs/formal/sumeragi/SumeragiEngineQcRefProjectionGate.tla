---- MODULE SumeragiEngineQcRefProjectionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for pure-engine QC reference projection.

This slice models `qc_ref_from_certificate(...)`. Prepare and Commit handling
record QCs through this helper before lock/highest-QC state can affect later
proposal and view-change safety. The helper must project exactly the
certificate round height, view, epoch, certificate phase, and certified block
hash. It must not advance height, collapse view/epoch, rewrite phase, use the
parent hash, or synthesize a zero subject.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugDropHeight,
  \* @type: Bool;
  BugAdvanceHeight,
  \* @type: Bool;
  BugDropView,
  \* @type: Bool;
  BugDropEpoch,
  \* @type: Bool;
  BugUseParentSubject,
  \* @type: Bool;
  BugZeroSubject,
  \* @type: Bool;
  BugForcePreparePhase,
  \* @type: Bool;
  BugForceCommitPhase,
  \* @type: Bool;
  BugForceNewViewPhase

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Candidates == {
  "prepare_base",
  "commit_base",
  "new_view_base",
  "height_two",
  "view_three",
  "epoch_four",
  "subject_b"
}

SpecHeight(candidate) ==
  IF candidate = "height_two" THEN 2 ELSE 1

SpecView(candidate) ==
  IF candidate = "view_three" THEN 3 ELSE 1

SpecEpoch(candidate) ==
  IF candidate = "epoch_four" THEN 4 ELSE 1

SpecSubject(candidate) ==
  IF candidate = "subject_b" THEN "block_b" ELSE "block_a"

SpecParent(candidate) ==
  IF candidate = "subject_b" THEN "parent_b" ELSE "parent_a"

SpecPhase(candidate) ==
  CASE candidate = "commit_base" -> "Commit"
    [] candidate = "new_view_base" -> "NewView"
    [] OTHER -> "Prepare"

ImplementationHeight(candidate) ==
  IF BugDropHeight
  THEN 0
  ELSE IF BugAdvanceHeight
       THEN SpecHeight(candidate) + 1
       ELSE SpecHeight(candidate)

ImplementationView(candidate) ==
  IF BugDropView THEN 0 ELSE SpecView(candidate)

ImplementationEpoch(candidate) ==
  IF BugDropEpoch THEN 0 ELSE SpecEpoch(candidate)

ImplementationSubject(candidate) ==
  IF BugUseParentSubject
  THEN SpecParent(candidate)
  ELSE IF BugZeroSubject
       THEN "zero"
       ELSE SpecSubject(candidate)

ImplementationPhase(candidate) ==
  IF BugForcePreparePhase
  THEN "Prepare"
  ELSE IF BugForceCommitPhase
       THEN "Commit"
       ELSE IF BugForceNewViewPhase
            THEN "NewView"
            ELSE SpecPhase(candidate)

TypeInvariant ==
  /\ BugDropHeight \in BOOLEAN
  /\ BugAdvanceHeight \in BOOLEAN
  /\ BugDropView \in BOOLEAN
  /\ BugDropEpoch \in BOOLEAN
  /\ BugUseParentSubject \in BOOLEAN
  /\ BugZeroSubject \in BOOLEAN
  /\ BugForcePreparePhase \in BOOLEAN
  /\ BugForceCommitPhase \in BOOLEAN
  /\ BugForceNewViewPhase \in BOOLEAN
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

ProjectionMatchesSpec ==
  \A candidate \in tried:
    /\ ImplementationHeight(candidate) = SpecHeight(candidate)
    /\ ImplementationView(candidate) = SpecView(candidate)
    /\ ImplementationEpoch(candidate) = SpecEpoch(candidate)
    /\ ImplementationSubject(candidate) = SpecSubject(candidate)
    /\ ImplementationPhase(candidate) = SpecPhase(candidate)

HeightMatchesCertificateRound ==
  \A candidate \in tried:
    ImplementationHeight(candidate) = SpecHeight(candidate)

ViewMatchesCertificateRound ==
  \A candidate \in tried:
    ImplementationView(candidate) = SpecView(candidate)

EpochMatchesCertificateRound ==
  \A candidate \in tried:
    ImplementationEpoch(candidate) = SpecEpoch(candidate)

PhaseMatchesCertificate ==
  \A candidate \in tried:
    ImplementationPhase(candidate) = SpecPhase(candidate)

SubjectMatchesCertifiedBlock ==
  \A candidate \in tried:
    ImplementationSubject(candidate) = SpecSubject(candidate)

ParentHashIsNeverSubject ==
  \A candidate \in tried:
    ImplementationSubject(candidate) # SpecParent(candidate)

ZeroHashIsNeverSynthesized ==
  \A candidate \in tried:
    ImplementationSubject(candidate) # "zero"

Safety ==
  /\ ProjectionMatchesSpec
  /\ HeightMatchesCertificateRound
  /\ ViewMatchesCertificateRound
  /\ EpochMatchesCertificateRound
  /\ PhaseMatchesCertificate
  /\ SubjectMatchesCertifiedBlock
  /\ ParentHashIsNeverSubject
  /\ ZeroHashIsNeverSynthesized

====
