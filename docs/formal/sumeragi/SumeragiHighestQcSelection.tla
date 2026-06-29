---- MODULE SumeragiHighestQcSelection ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for deterministic highest-QC selection.

Sumeragi new-view aggregation must choose the same highest QC regardless of
certificate arrival order. The implementation orders QCs by height, then view,
then phase rank, then subject hash bytes. Only NewView certificates contribute
their embedded highest_qc; Prepare/Commit certificates are ignored by the
selector.

This slice models two replicas that observe the same finite evidence in
different orders. The selected QC must equal the reference maximum for each
replica, and replicas with the same observed certificate set must agree.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugIgnoreHeightPriority,
  \* @type: Bool;
  BugIgnorePhaseRank,
  \* @type: Bool;
  BugIgnoreSubjectTieBreak,
  \* @type: Bool;
  BugIncludeNonNewView

VARIABLES
  \* @type: Set(Str);
  observedA,
  \* @type: Set(Str);
  observedB,
  \* @type: Str;
  selectedA,
  \* @type: Str;
  selectedB

vars == <<observedA, observedB, selectedA, selectedB>>

NoQc == "none"

Certs == {
  "heightWinner",
  "viewDecoy",
  "phaseWinner",
  "phaseDecoy",
  "subjectLow",
  "subjectHigh",
  "nonNewViewDecoy"
}

Selections == Certs \union {NoQc}

CertPhase(cert) ==
  CASE cert = "nonNewViewDecoy" -> "Prepare"
    [] OTHER -> "NewView"

QcHeight(cert) ==
  CASE cert = "heightWinner" -> 2
    [] cert = "viewDecoy" -> 1
    [] cert = "phaseWinner" -> 2
    [] cert = "phaseDecoy" -> 2
    [] cert = "subjectLow" -> 2
    [] cert = "subjectHigh" -> 2
    [] cert = "nonNewViewDecoy" -> 9

QcView(cert) ==
  CASE cert = "heightWinner" -> 0
    [] cert = "viewDecoy" -> 9
    [] cert = "phaseWinner" -> 1
    [] cert = "phaseDecoy" -> 1
    [] cert = "subjectLow" -> 1
    [] cert = "subjectHigh" -> 1
    [] cert = "nonNewViewDecoy" -> 9

QcPhase(cert) ==
  CASE cert = "phaseWinner" -> "Commit"
    [] cert = "phaseDecoy" -> "Prepare"
    [] cert = "nonNewViewDecoy" -> "Commit"
    [] OTHER -> "NewView"

QcSubject(cert) ==
  CASE cert = "heightWinner" -> 1
    [] cert = "viewDecoy" -> 3
    [] cert = "phaseWinner" -> 1
    [] cert = "phaseDecoy" -> 9
    [] cert = "subjectLow" -> 1
    [] cert = "subjectHigh" -> 2
    [] cert = "nonNewViewDecoy" -> 9

PhaseRank(phase) ==
  CASE phase = "Prepare" -> 0
    [] phase = "NewView" -> 1
    [] OTHER -> 2

SpecGreater(left, right) ==
  \/ QcHeight(left) > QcHeight(right)
  \/ /\ QcHeight(left) = QcHeight(right)
     /\ QcView(left) > QcView(right)
  \/ /\ QcHeight(left) = QcHeight(right)
     /\ QcView(left) = QcView(right)
     /\ PhaseRank(QcPhase(left)) > PhaseRank(QcPhase(right))
  \/ /\ QcHeight(left) = QcHeight(right)
     /\ QcView(left) = QcView(right)
     /\ PhaseRank(QcPhase(left)) = PhaseRank(QcPhase(right))
     /\ QcSubject(left) > QcSubject(right)

BuggyGreater(left, right) ==
  IF BugIgnoreHeightPriority
  THEN
    \/ QcView(left) > QcView(right)
    \/ /\ QcView(left) = QcView(right)
       /\ PhaseRank(QcPhase(left)) > PhaseRank(QcPhase(right))
    \/ /\ QcView(left) = QcView(right)
       /\ PhaseRank(QcPhase(left)) = PhaseRank(QcPhase(right))
       /\ QcSubject(left) > QcSubject(right)
  ELSE IF BugIgnorePhaseRank
  THEN
    \/ QcHeight(left) > QcHeight(right)
    \/ /\ QcHeight(left) = QcHeight(right)
       /\ QcView(left) > QcView(right)
    \/ /\ QcHeight(left) = QcHeight(right)
       /\ QcView(left) = QcView(right)
       /\ QcSubject(left) > QcSubject(right)
  ELSE IF BugIgnoreSubjectTieBreak
  THEN
    \/ QcHeight(left) > QcHeight(right)
    \/ /\ QcHeight(left) = QcHeight(right)
       /\ QcView(left) > QcView(right)
    \/ /\ QcHeight(left) = QcHeight(right)
       /\ QcView(left) = QcView(right)
       /\ PhaseRank(QcPhase(left)) > PhaseRank(QcPhase(right))
  ELSE
    SpecGreater(left, right)

ConsideredBySelector(cert) ==
  \/ CertPhase(cert) = "NewView"
  \/ BugIncludeNonNewView

BetterForUpdate(candidate, current) ==
  \/ current = NoQc
  \/ BuggyGreater(candidate, current)

UpdateSelected(current, cert) ==
  IF ConsideredBySelector(cert) /\ BetterForUpdate(cert, current)
  THEN cert
  ELSE current

ObservedNewViewQcs(observed) ==
  {cert \in observed : CertPhase(cert) = "NewView"}

SpecMaxSelected(observed, selected) ==
  IF ObservedNewViewQcs(observed) = {}
  THEN selected = NoQc
  ELSE
    /\ selected \in ObservedNewViewQcs(observed)
    /\ \A cert \in ObservedNewViewQcs(observed):
         ~SpecGreater(cert, selected)

TypeInvariant ==
  /\ BugIgnoreHeightPriority \in BOOLEAN
  /\ BugIgnorePhaseRank \in BOOLEAN
  /\ BugIgnoreSubjectTieBreak \in BOOLEAN
  /\ BugIncludeNonNewView \in BOOLEAN
  /\ observedA \subseteq Certs
  /\ observedB \subseteq Certs
  /\ selectedA \in Selections
  /\ selectedB \in Selections

Init ==
  /\ observedA = {}
  /\ observedB = {}
  /\ selectedA = NoQc
  /\ selectedB = NoQc

ObserveA(cert) ==
  /\ cert \in Certs \ observedA
  /\ observedA' = observedA \union {cert}
  /\ selectedA' = UpdateSelected(selectedA, cert)
  /\ UNCHANGED <<observedB, selectedB>>

ObserveB(cert) ==
  /\ cert \in Certs \ observedB
  /\ observedB' = observedB \union {cert}
  /\ selectedB' = UpdateSelected(selectedB, cert)
  /\ UNCHANGED <<observedA, selectedA>>

Stable ==
  UNCHANGED vars

Next ==
  \/ \E cert \in Certs: ObserveA(cert)
  \/ \E cert \in Certs: ObserveB(cert)
  \/ Stable

SelectedAEqualsSpecMax ==
  SpecMaxSelected(observedA, selectedA)

SelectedBEqualsSpecMax ==
  SpecMaxSelected(observedB, selectedB)

SelectedOnlyFromNewViewCertificates ==
  /\ selectedA # NoQc => CertPhase(selectedA) = "NewView"
  /\ selectedB # NoQc => CertPhase(selectedB) = "NewView"

EqualObservedSelectsEqualQc ==
  observedA = observedB => selectedA = selectedB

HeightPriorityDominatesView ==
  /\ observedA = {"heightWinner", "viewDecoy"} => selectedA = "heightWinner"
  /\ observedB = {"heightWinner", "viewDecoy"} => selectedB = "heightWinner"

PhaseRankDominatesSubject ==
  /\ observedA = {"phaseWinner", "phaseDecoy"} => selectedA = "phaseWinner"
  /\ observedB = {"phaseWinner", "phaseDecoy"} => selectedB = "phaseWinner"

SubjectTieBreakDominatesArrivalOrder ==
  /\ observedA = {"subjectLow", "subjectHigh"} => selectedA = "subjectHigh"
  /\ observedB = {"subjectLow", "subjectHigh"} => selectedB = "subjectHigh"

HighestQcSelectionExactness ==
  /\ SelectedAEqualsSpecMax
  /\ SelectedBEqualsSpecMax
  /\ SelectedOnlyFromNewViewCertificates
  /\ EqualObservedSelectsEqualQc
  /\ HeightPriorityDominatesView
  /\ PhaseRankDominatesSubject
  /\ SubjectTieBreakDominatesArrivalOrder

HighestQcSelectionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ HighestQcSelectionExactness

====
