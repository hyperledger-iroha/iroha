---- MODULE SumeragiEngineHighestQcRecordGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for pure-engine highest-QC state recording.

This slice models `ConsensusEngine::record_highest_qc(...)`, which updates
engine state only when the candidate QC is strictly greater than the current
highest QC under `qc_ref_cmp(...)`. The comparator orders QCs by height, then
view, then phase rank, then subject hash bytes. Equal or lower candidates must
not overwrite local highest-QC state, while an empty state and strictly greater
candidate must record the candidate.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipNoCurrent,
  \* @type: Bool;
  BugRejectHigherHeight,
  \* @type: Bool;
  BugAcceptLowerHeight,
  \* @type: Bool;
  BugUseViewBeforeHeight,
  \* @type: Bool;
  BugRejectHigherView,
  \* @type: Bool;
  BugAcceptLowerView,
  \* @type: Bool;
  BugIgnorePhaseRank,
  \* @type: Bool;
  BugRejectHigherPhase,
  \* @type: Bool;
  BugAcceptLowerPhase,
  \* @type: Bool;
  BugIgnoreSubjectTie,
  \* @type: Bool;
  BugRejectHigherSubject,
  \* @type: Bool;
  BugAcceptLowerSubject,
  \* @type: Bool;
  BugOverwriteEqual

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "noCurrent",
  "higherHeightLowerView",
  "lowerHeightHigherView",
  "sameHeightHigherView",
  "sameHeightLowerView",
  "sameSlotCommitOverPrepare",
  "sameSlotPrepareUnderCommit",
  "sameSlotSubjectHigh",
  "sameSlotSubjectLow",
  "equalQc"
}

Selections == {"none", "current", "candidate"}

HasCurrent(candidate) ==
  candidate # "noCurrent"

CurrentHeight(candidate) ==
  CASE candidate = "higherHeightLowerView" -> 5
    [] candidate = "lowerHeightHigherView" -> 5
    [] OTHER -> 7

CandidateHeight(candidate) ==
  CASE candidate = "higherHeightLowerView" -> 6
    [] candidate = "lowerHeightHigherView" -> 4
    [] OTHER -> 7

CurrentView(candidate) ==
  CASE candidate = "higherHeightLowerView" -> 9
    [] candidate = "lowerHeightHigherView" -> 0
    [] candidate = "sameHeightHigherView" -> 5
    [] candidate = "sameHeightLowerView" -> 5
    [] OTHER -> 3

CandidateView(candidate) ==
  CASE candidate = "higherHeightLowerView" -> 0
    [] candidate = "lowerHeightHigherView" -> 9
    [] candidate = "sameHeightHigherView" -> 6
    [] candidate = "sameHeightLowerView" -> 4
    [] OTHER -> 3

CurrentPhase(candidate) ==
  CASE candidate = "sameSlotCommitOverPrepare" -> "Prepare"
    [] candidate = "sameSlotPrepareUnderCommit" -> "Commit"
    [] OTHER -> "NewView"

CandidatePhase(candidate) ==
  CASE candidate = "sameSlotCommitOverPrepare" -> "Commit"
    [] candidate = "sameSlotPrepareUnderCommit" -> "Prepare"
    [] OTHER -> "NewView"

CurrentSubject(candidate) ==
  CASE candidate = "sameSlotCommitOverPrepare" -> 9
    [] candidate = "sameSlotPrepareUnderCommit" -> 1
    [] candidate = "sameSlotSubjectHigh" -> 1
    [] candidate = "sameSlotSubjectLow" -> 2
    [] OTHER -> 5

CandidateSubject(candidate) ==
  CASE candidate = "sameSlotCommitOverPrepare" -> 1
    [] candidate = "sameSlotPrepareUnderCommit" -> 9
    [] candidate = "sameSlotSubjectHigh" -> 2
    [] candidate = "sameSlotSubjectLow" -> 1
    [] OTHER -> 5

PhaseRank(phase) ==
  CASE phase = "Prepare" -> 0
    [] phase = "NewView" -> 1
    [] OTHER -> 2

SpecGreater(candidate) ==
  \/ CandidateHeight(candidate) > CurrentHeight(candidate)
  \/ /\ CandidateHeight(candidate) = CurrentHeight(candidate)
     /\ CandidateView(candidate) > CurrentView(candidate)
  \/ /\ CandidateHeight(candidate) = CurrentHeight(candidate)
     /\ CandidateView(candidate) = CurrentView(candidate)
     /\ PhaseRank(CandidatePhase(candidate)) > PhaseRank(CurrentPhase(candidate))
  \/ /\ CandidateHeight(candidate) = CurrentHeight(candidate)
     /\ CandidateView(candidate) = CurrentView(candidate)
     /\ PhaseRank(CandidatePhase(candidate)) = PhaseRank(CurrentPhase(candidate))
     /\ CandidateSubject(candidate) > CurrentSubject(candidate)

ViewBeforeHeightGreater(candidate) ==
  \/ CandidateView(candidate) > CurrentView(candidate)
  \/ /\ CandidateView(candidate) = CurrentView(candidate)
     /\ CandidateHeight(candidate) > CurrentHeight(candidate)
  \/ /\ CandidateView(candidate) = CurrentView(candidate)
     /\ CandidateHeight(candidate) = CurrentHeight(candidate)
     /\ PhaseRank(CandidatePhase(candidate)) > PhaseRank(CurrentPhase(candidate))
  \/ /\ CandidateView(candidate) = CurrentView(candidate)
     /\ CandidateHeight(candidate) = CurrentHeight(candidate)
     /\ PhaseRank(CandidatePhase(candidate)) = PhaseRank(CurrentPhase(candidate))
     /\ CandidateSubject(candidate) > CurrentSubject(candidate)

IgnorePhaseGreater(candidate) ==
  \/ CandidateHeight(candidate) > CurrentHeight(candidate)
  \/ /\ CandidateHeight(candidate) = CurrentHeight(candidate)
     /\ CandidateView(candidate) > CurrentView(candidate)
  \/ /\ CandidateHeight(candidate) = CurrentHeight(candidate)
     /\ CandidateView(candidate) = CurrentView(candidate)
     /\ CandidateSubject(candidate) > CurrentSubject(candidate)

IgnoreSubjectGreater(candidate) ==
  \/ CandidateHeight(candidate) > CurrentHeight(candidate)
  \/ /\ CandidateHeight(candidate) = CurrentHeight(candidate)
     /\ CandidateView(candidate) > CurrentView(candidate)
  \/ /\ CandidateHeight(candidate) = CurrentHeight(candidate)
     /\ CandidateView(candidate) = CurrentView(candidate)
     /\ PhaseRank(CandidatePhase(candidate)) > PhaseRank(CurrentPhase(candidate))

BugForcesAccept(candidate) ==
  \/ /\ candidate = "lowerHeightHigherView"
     /\ BugAcceptLowerHeight
  \/ /\ candidate = "sameHeightLowerView"
     /\ BugAcceptLowerView
  \/ /\ candidate = "sameSlotPrepareUnderCommit"
     /\ BugAcceptLowerPhase
  \/ /\ candidate = "sameSlotSubjectLow"
     /\ BugAcceptLowerSubject
  \/ /\ candidate = "equalQc"
     /\ BugOverwriteEqual

BugForcesReject(candidate) ==
  \/ /\ candidate = "higherHeightLowerView"
     /\ BugRejectHigherHeight
  \/ /\ candidate = "sameHeightHigherView"
     /\ BugRejectHigherView
  \/ /\ candidate = "sameSlotCommitOverPrepare"
     /\ BugRejectHigherPhase
  \/ /\ candidate = "sameSlotSubjectHigh"
     /\ BugRejectHigherSubject

ImplementationGreater(candidate) ==
  IF BugUseViewBeforeHeight
  THEN ViewBeforeHeightGreater(candidate)
  ELSE IF BugIgnorePhaseRank
  THEN IgnorePhaseGreater(candidate)
  ELSE IF BugIgnoreSubjectTie
  THEN IgnoreSubjectGreater(candidate)
  ELSE IF BugForcesAccept(candidate)
  THEN TRUE
  ELSE IF BugForcesReject(candidate)
  THEN FALSE
  ELSE SpecGreater(candidate)

SpecUpdates(candidate) ==
  \/ ~HasCurrent(candidate)
  \/ SpecGreater(candidate)

ImplementationUpdates(candidate) ==
  IF ~HasCurrent(candidate)
  THEN ~BugSkipNoCurrent
  ELSE ImplementationGreater(candidate)

SpecSelection(candidate) ==
  IF SpecUpdates(candidate) THEN "candidate" ELSE "current"

ImplementationSelection(candidate) ==
  IF ImplementationUpdates(candidate)
  THEN "candidate"
  ELSE IF HasCurrent(candidate)
  THEN "current"
  ELSE "none"

TypeInvariant ==
  /\ BugSkipNoCurrent \in BOOLEAN
  /\ BugRejectHigherHeight \in BOOLEAN
  /\ BugAcceptLowerHeight \in BOOLEAN
  /\ BugUseViewBeforeHeight \in BOOLEAN
  /\ BugRejectHigherView \in BOOLEAN
  /\ BugAcceptLowerView \in BOOLEAN
  /\ BugIgnorePhaseRank \in BOOLEAN
  /\ BugRejectHigherPhase \in BOOLEAN
  /\ BugAcceptLowerPhase \in BOOLEAN
  /\ BugIgnoreSubjectTie \in BOOLEAN
  /\ BugRejectHigherSubject \in BOOLEAN
  /\ BugAcceptLowerSubject \in BOOLEAN
  /\ BugOverwriteEqual \in BOOLEAN
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

RecordMatchesSpec ==
  \A candidate \in tried:
    ImplementationSelection(candidate) = SpecSelection(candidate)

EmptyStateRecordsCandidate ==
  "noCurrent" \in tried =>
    ImplementationSelection("noCurrent") = "candidate"

EqualCandidateDoesNotOverwrite ==
  "equalQc" \in tried =>
    ~ImplementationUpdates("equalQc")

NoRegression ==
  \A candidate \in tried:
    /\ HasCurrent(candidate)
    /\ ImplementationUpdates(candidate)
    => SpecGreater(candidate)

GreaterCandidatesAlwaysRecord ==
  \A candidate \in tried:
    /\ HasCurrent(candidate)
    /\ SpecGreater(candidate)
    => ImplementationUpdates(candidate)

HeightDominatesView ==
  /\ "higherHeightLowerView" \in tried =>
       ImplementationUpdates("higherHeightLowerView")
  /\ "lowerHeightHigherView" \in tried =>
       ~ImplementationUpdates("lowerHeightHigherView")

ViewDominatesPhaseAndSubject ==
  /\ "sameHeightHigherView" \in tried =>
       ImplementationUpdates("sameHeightHigherView")
  /\ "sameHeightLowerView" \in tried =>
       ~ImplementationUpdates("sameHeightLowerView")

PhaseRankOrdersSameSlot ==
  /\ "sameSlotCommitOverPrepare" \in tried =>
       ImplementationUpdates("sameSlotCommitOverPrepare")
  /\ "sameSlotPrepareUnderCommit" \in tried =>
       ~ImplementationUpdates("sameSlotPrepareUnderCommit")

SubjectTieBreakOrdersSameRank ==
  /\ "sameSlotSubjectHigh" \in tried =>
       ImplementationUpdates("sameSlotSubjectHigh")
  /\ "sameSlotSubjectLow" \in tried =>
       ~ImplementationUpdates("sameSlotSubjectLow")

Safety ==
  /\ RecordMatchesSpec
  /\ EmptyStateRecordsCandidate
  /\ EqualCandidateDoesNotOverwrite
  /\ NoRegression
  /\ GreaterCandidatesAlwaysRecord
  /\ HeightDominatesView
  /\ ViewDominatesPhaseAndSubject
  /\ PhaseRankOrdersSameSlot
  /\ SubjectTieBreakOrdersSameRank

====
