---- MODULE SumeragiEngineQcRefComparatorGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `qc_ref_cmp(...)`.

Sumeragi uses QC reference ordering in lock checks, highest-QC recording, and
highest-QC selection. The implementation compares height, then view, then phase
rank, then subject hash bytes. This model proves that bounded comparator is a
strict total lexicographic order and that each field has the intended priority.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugReverseHeight,
  \* @type: Bool;
  BugIgnoreView,
  \* @type: Bool;
  BugIgnorePhase,
  \* @type: Bool;
  BugReversePhase,
  \* @type: Bool;
  BugIgnoreSubject,
  \* @type: Bool;
  BugReverseSubject,
  \* @type: Bool;
  BugSubjectBeforePhase,
  \* @type: Bool;
  BugNonReflexive,
  \* @type: Bool;
  BugNonAntisymmetric

VARIABLES
  \* @type: Bool;
  checked

\* @type: <<Bool>>;
vars == <<checked>>

Qcs == {
  "h0v0_prepare_a",
  "h0v0_prepare_b",
  "h0v0_new_view_a",
  "h0v0_commit_a",
  "h0v0_commit_b",
  "h0v1_prepare_a",
  "h1v0_prepare_a",
  "h1v1_commit_b"
}

CmpValues == {"lt", "eq", "gt"}

Height(qc) ==
  IF qc \in {"h1v0_prepare_a", "h1v1_commit_b"} THEN 1 ELSE 0

View(qc) ==
  IF qc \in {"h0v1_prepare_a", "h1v1_commit_b"} THEN 1 ELSE 0

Phase(qc) ==
  CASE qc \in {"h0v0_prepare_a", "h0v0_prepare_b", "h0v1_prepare_a", "h1v0_prepare_a"} -> "Prepare"
    [] qc = "h0v0_new_view_a" -> "NewView"
    [] OTHER -> "Commit"

Subject(qc) ==
  IF qc \in {"h0v0_prepare_b", "h0v0_commit_b", "h1v1_commit_b"}
  THEN "B"
  ELSE "A"

PhaseRank(phase) ==
  CASE phase = "Prepare" -> 0
    [] phase = "NewView" -> 1
    [] OTHER -> 2

SubjectRank(subject) ==
  IF subject = "B" THEN 1 ELSE 0

FieldCmp(left, right) ==
  IF left < right THEN "lt" ELSE IF left > right THEN "gt" ELSE "eq"

SpecCmp(left, right) ==
  LET h == FieldCmp(Height(left), Height(right)) IN
    IF h # "eq" THEN h ELSE
  LET v == FieldCmp(View(left), View(right)) IN
    IF v # "eq" THEN v ELSE
  LET p == FieldCmp(PhaseRank(Phase(left)), PhaseRank(Phase(right))) IN
    IF p # "eq" THEN p ELSE
  FieldCmp(SubjectRank(Subject(left)), SubjectRank(Subject(right)))

ImplHeight(qc) ==
  IF BugReverseHeight THEN 1 - Height(qc) ELSE Height(qc)

ImplView(qc) ==
  IF BugIgnoreView THEN 0 ELSE View(qc)

ImplPhaseRank(qc) ==
  IF BugIgnorePhase
  THEN 0
  ELSE IF BugReversePhase
       THEN 2 - PhaseRank(Phase(qc))
       ELSE PhaseRank(Phase(qc))

ImplSubjectRank(qc) ==
  IF BugIgnoreSubject
  THEN 0
  ELSE IF BugReverseSubject
       THEN 1 - SubjectRank(Subject(qc))
       ELSE SubjectRank(Subject(qc))

ImplCmpByPhaseBeforeSubject(left, right) ==
  LET h == FieldCmp(ImplHeight(left), ImplHeight(right)) IN
    IF h # "eq" THEN h ELSE
  LET v == FieldCmp(ImplView(left), ImplView(right)) IN
    IF v # "eq" THEN v ELSE
  LET p == FieldCmp(ImplPhaseRank(left), ImplPhaseRank(right)) IN
    IF p # "eq" THEN p ELSE
  FieldCmp(ImplSubjectRank(left), ImplSubjectRank(right))

ImplCmpBySubjectBeforePhase(left, right) ==
  LET h == FieldCmp(ImplHeight(left), ImplHeight(right)) IN
    IF h # "eq" THEN h ELSE
  LET v == FieldCmp(ImplView(left), ImplView(right)) IN
    IF v # "eq" THEN v ELSE
  LET s == FieldCmp(ImplSubjectRank(left), ImplSubjectRank(right)) IN
    IF s # "eq" THEN s ELSE
  FieldCmp(ImplPhaseRank(left), ImplPhaseRank(right))

ImplCmp(left, right) ==
  IF BugNonReflexive /\ left = right
  THEN "gt"
  ELSE IF BugNonAntisymmetric /\
          ((left = "h0v0_prepare_a" /\ right = "h0v0_prepare_b") \/
           (left = "h0v0_prepare_b" /\ right = "h0v0_prepare_a"))
       THEN "gt"
       ELSE IF BugSubjectBeforePhase
            THEN ImplCmpBySubjectBeforePhase(left, right)
            ELSE ImplCmpByPhaseBeforeSubject(left, right)

TypeInvariant ==
  /\ BugReverseHeight \in BOOLEAN
  /\ BugIgnoreView \in BOOLEAN
  /\ BugIgnorePhase \in BOOLEAN
  /\ BugReversePhase \in BOOLEAN
  /\ BugIgnoreSubject \in BOOLEAN
  /\ BugReverseSubject \in BOOLEAN
  /\ BugSubjectBeforePhase \in BOOLEAN
  /\ BugNonReflexive \in BOOLEAN
  /\ BugNonAntisymmetric \in BOOLEAN
  /\ checked \in BOOLEAN

Init ==
  checked = FALSE

Next ==
  \/ /\ ~checked
     /\ checked' = TRUE
  \/ UNCHANGED vars

ComparatorMatchesSpec ==
  \A left \in Qcs:
    \A right \in Qcs:
      ImplCmp(left, right) = SpecCmp(left, right)

ComparatorReturnsOnlyCmpValues ==
  \A left \in Qcs:
    \A right \in Qcs:
      ImplCmp(left, right) \in CmpValues

ReflexiveEq ==
  \A qc \in Qcs:
    ImplCmp(qc, qc) = "eq"

Antisymmetric ==
  \A left \in Qcs:
    \A right \in Qcs:
      /\ (ImplCmp(left, right) = "eq" <=> ImplCmp(right, left) = "eq")
      /\ (ImplCmp(left, right) = "gt" <=> ImplCmp(right, left) = "lt")
      /\ (ImplCmp(left, right) = "lt" <=> ImplCmp(right, left) = "gt")

TransitiveGt ==
  \A left \in Qcs:
    \A middle \in Qcs:
      \A right \in Qcs:
        ImplCmp(left, middle) = "gt" /\ ImplCmp(middle, right) = "gt" =>
          ImplCmp(left, right) = "gt"

Total ==
  \A left \in Qcs:
    \A right \in Qcs:
      ImplCmp(left, right) \in CmpValues

HeightDominates ==
  \A left \in Qcs:
    \A right \in Qcs:
      Height(left) > Height(right) => ImplCmp(left, right) = "gt"

ViewDominatesWhenHeightEqual ==
  \A left \in Qcs:
    \A right \in Qcs:
      Height(left) = Height(right) /\ View(left) > View(right) =>
        ImplCmp(left, right) = "gt"

PhaseDominatesWhenSlotEqual ==
  \A left \in Qcs:
    \A right \in Qcs:
      /\ Height(left) = Height(right)
      /\ View(left) = View(right)
      /\ PhaseRank(Phase(left)) > PhaseRank(Phase(right))
      => ImplCmp(left, right) = "gt"

SubjectTieBreaksWhenRankEqual ==
  \A left \in Qcs:
    \A right \in Qcs:
      /\ Height(left) = Height(right)
      /\ View(left) = View(right)
      /\ PhaseRank(Phase(left)) = PhaseRank(Phase(right))
      /\ SubjectRank(Subject(left)) > SubjectRank(Subject(right))
      => ImplCmp(left, right) = "gt"

EqualOnlyForSameFields ==
  \A left \in Qcs:
    \A right \in Qcs:
      ImplCmp(left, right) = "eq" =>
        /\ Height(left) = Height(right)
        /\ View(left) = View(right)
        /\ Phase(left) = Phase(right)
        /\ Subject(left) = Subject(right)

PhasePrecedesSubjectInPriority ==
  ImplCmp("h0v0_commit_a", "h0v0_prepare_b") = "gt"

EngineQcRefComparatorExactness ==
  /\ ComparatorMatchesSpec
  /\ ComparatorReturnsOnlyCmpValues
  /\ ReflexiveEq
  /\ Antisymmetric
  /\ TransitiveGt
  /\ Total
  /\ HeightDominates
  /\ ViewDominatesWhenHeightEqual
  /\ PhaseDominatesWhenSlotEqual
  /\ SubjectTieBreaksWhenRankEqual
  /\ EqualOnlyForSameFields
  /\ PhasePrecedesSubjectInPriority

EngineQcRefComparatorCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineQcRefComparatorExactness

Safety == EngineQcRefComparatorExactness

====
