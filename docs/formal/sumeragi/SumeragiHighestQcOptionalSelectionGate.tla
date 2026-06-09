---- MODULE SumeragiHighestQcOptionalSelectionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for optional highest-QC filtering.

This slice models the `select_highest_qc(...)` `filter_map(...).flatten()`
contract. Only `CertPhase::NewView` certificates whose `highest_qc` field is
`Some(QcRef)` may participate in selection. NewView certificates with no
embedded QC and non-NewView certificates are ignored, and the selected value is
the embedded QC, not the certificate subject or synthesized fallback evidence.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSynthesizeEmpty,
  \* @type: Bool;
  BugIncludeNewViewNone,
  \* @type: Bool;
  BugIncludeNonNewView,
  \* @type: Bool;
  BugDropNewViewSome,
  \* @type: Bool;
  BugPreferNoneOverSome,
  \* @type: Bool;
  BugUseCertificateSubject,
  \* @type: Bool;
  BugPreferLowerHeight,
  \* @type: Bool;
  BugIgnoreSubjectTie,
  \* @type: Bool;
  BugNoneClearsSelection,
  \* @type: Bool;
  BugNonNewViewClearsSelection

VARIABLES
  \* @type: Set(Str);
  tried,
  \* @type: Str;
  last_case,
  \* @type: Str;
  selected

vars == <<tried, last_case, selected>>

NoQc == "none"

Cases == {
  "empty",
  "only_new_view_none",
  "only_non_new_view_some",
  "single_new_view_some",
  "embedded_differs_subject",
  "mixed_none_and_some",
  "mixed_non_new_view_high",
  "two_new_view_height",
  "two_new_view_subject_tie",
  "some_then_none",
  "some_then_non_new_view"
}

Qcs == {
  "q_low",
  "q_high",
  "q_subject_low",
  "q_subject_high",
  "q_embedded",
  "q_cert_subject",
  "q_non_new_view_high"
}

SyntheticSelections == {"synthetic_empty", "synthetic_none"}

Selections == Qcs \union SyntheticSelections \union {NoQc}

QcHeight(qc) ==
  CASE qc = "q_high" -> 2
    [] qc = "q_subject_low" -> 2
    [] qc = "q_subject_high" -> 2
    [] qc = "q_embedded" -> 3
    [] qc = "q_non_new_view_high" -> 4
    [] OTHER -> 1

QcView(qc) ==
  CASE qc = "q_embedded" -> 2
    [] OTHER -> 1

QcPhase(qc) ==
  CASE qc = "q_high" -> "Commit"
    [] qc = "q_non_new_view_high" -> "Commit"
    [] OTHER -> "NewView"

QcSubject(qc) ==
  CASE qc = "q_subject_high" -> 2
    [] qc = "q_non_new_view_high" -> 9
    [] qc = "q_cert_subject" -> 8
    [] OTHER -> 1

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

SpecCandidates(c) ==
  CASE c = "single_new_view_some" -> {"q_low"}
    [] c = "embedded_differs_subject" -> {"q_embedded"}
    [] c = "mixed_none_and_some" -> {"q_low"}
    [] c = "mixed_non_new_view_high" -> {"q_low"}
    [] c = "two_new_view_height" -> {"q_low", "q_high"}
    [] c = "two_new_view_subject_tie" -> {"q_subject_low", "q_subject_high"}
    [] c = "some_then_none" -> {"q_low"}
    [] c = "some_then_non_new_view" -> {"q_low"}
    [] OTHER -> {}

SpecSelected(c) ==
  CASE c = "single_new_view_some" -> "q_low"
    [] c = "embedded_differs_subject" -> "q_embedded"
    [] c = "mixed_none_and_some" -> "q_low"
    [] c = "mixed_non_new_view_high" -> "q_low"
    [] c = "two_new_view_height" -> "q_high"
    [] c = "two_new_view_subject_tie" -> "q_subject_high"
    [] c = "some_then_none" -> "q_low"
    [] c = "some_then_non_new_view" -> "q_low"
    [] OTHER -> NoQc

CaseHasSomeNewView(c) ==
  SpecCandidates(c) # {}

ImplementationSelected(c) ==
  IF BugSynthesizeEmpty /\ c = "empty"
  THEN "synthetic_empty"
  ELSE IF BugIncludeNewViewNone /\ c = "only_new_view_none"
  THEN "synthetic_none"
  ELSE IF BugIncludeNonNewView /\ c \in {"only_non_new_view_some", "mixed_non_new_view_high"}
  THEN "q_non_new_view_high"
  ELSE IF BugDropNewViewSome /\ CaseHasSomeNewView(c)
  THEN NoQc
  ELSE IF BugPreferNoneOverSome /\ c = "mixed_none_and_some"
  THEN "synthetic_none"
  ELSE IF BugUseCertificateSubject /\ c = "embedded_differs_subject"
  THEN "q_cert_subject"
  ELSE IF BugPreferLowerHeight /\ c = "two_new_view_height"
  THEN "q_low"
  ELSE IF BugIgnoreSubjectTie /\ c = "two_new_view_subject_tie"
  THEN "q_subject_low"
  ELSE IF BugNoneClearsSelection /\ c = "some_then_none"
  THEN NoQc
  ELSE IF BugNonNewViewClearsSelection /\ c = "some_then_non_new_view"
  THEN NoQc
  ELSE SpecSelected(c)

TypeInvariant ==
  /\ BugSynthesizeEmpty \in BOOLEAN
  /\ BugIncludeNewViewNone \in BOOLEAN
  /\ BugIncludeNonNewView \in BOOLEAN
  /\ BugDropNewViewSome \in BOOLEAN
  /\ BugPreferNoneOverSome \in BOOLEAN
  /\ BugUseCertificateSubject \in BOOLEAN
  /\ BugPreferLowerHeight \in BOOLEAN
  /\ BugIgnoreSubjectTie \in BOOLEAN
  /\ BugNoneClearsSelection \in BOOLEAN
  /\ BugNonNewViewClearsSelection \in BOOLEAN
  /\ tried \subseteq Cases
  /\ last_case \in Cases \union {NoQc}
  /\ selected \in Selections

Init ==
  /\ tried = {}
  /\ last_case = NoQc
  /\ selected = NoQc

TryCase(c) ==
  /\ c \in Cases
  /\ tried' = tried \union {c}
  /\ last_case' = c
  /\ selected' = ImplementationSelected(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: TryCase(c)
  \/ Stable

SelectedMatchesSpec ==
  last_case = NoQc \/ selected = SpecSelected(last_case)

NoSelectionWithoutSomeNewView ==
  last_case = NoQc \/
    (SpecCandidates(last_case) = {} => selected = NoQc)

SelectionComesFromEmbeddedNewViewQc ==
  last_case = NoQc \/
    (selected # NoQc => selected \in SpecCandidates(last_case))

EmbeddedHighestNotCertificateSubject ==
  last_case = "embedded_differs_subject" => selected = "q_embedded"

MaxCandidateSelected ==
  last_case = NoQc \/
    /\ selected # NoQc
       => /\ selected \in SpecCandidates(last_case)
          /\ \A qc \in SpecCandidates(last_case): ~SpecGreater(qc, selected)

IgnoredEvidenceCannotClearSelection ==
  /\ last_case = "some_then_none" => selected = "q_low"
  /\ last_case = "some_then_non_new_view" => selected = "q_low"

HighestQcOptionalSelectionCoreSafety ==
  /\ SelectedMatchesSpec
  /\ NoSelectionWithoutSomeNewView
  /\ SelectionComesFromEmbeddedNewViewQc
  /\ EmbeddedHighestNotCertificateSubject
  /\ MaxCandidateSelected
  /\ IgnoredEvidenceCannotClearSelection

Safety == HighestQcOptionalSelectionCoreSafety

====
