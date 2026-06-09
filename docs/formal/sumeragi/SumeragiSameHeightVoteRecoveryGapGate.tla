---- MODULE SumeragiSameHeightVoteRecoveryGapGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for same-height vote recovery view-gap helpers.

This slice pins `same_height_vote_recovery_view_gap_exhausted(...)` and
`same_height_vote_recovery_escalation_view_gap_exhausted(...)`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

StandardCases == {
  "std_subject_ahead",
  "std_zero_gap0",
  "std_zero_gap7",
  "std_zero_gap8",
  "std_one_gap7",
  "std_one_gap8",
  "std_two_gap15",
  "std_two_gap16",
  "std_four_gap31",
  "std_four_gap32",
  "std_five_gap10",
  "std_five_gap40"
}

EscalationCases == {
  "esc_subject_ahead",
  "esc_zero_gap0",
  "esc_zero_gap7",
  "esc_zero_gap8",
  "esc_one_gap7",
  "esc_one_gap8",
  "esc_four_gap7",
  "esc_four_gap8",
  "esc_five_gap9",
  "esc_five_gap10",
  "esc_five_gap39",
  "esc_five_gap40"
}

BoolToInt(b) == IF b THEN 1 ELSE 0

\* @type: (Str) => <<Int, Int, Int>>;
StandardParams(c) ==
  CASE c = "std_subject_ahead" -> <<10, 9, 1>>
    [] c = "std_zero_gap0" -> <<0, 0, 0>>
    [] c = "std_zero_gap7" -> <<0, 7, 0>>
    [] c = "std_zero_gap8" -> <<0, 8, 0>>
    [] c = "std_one_gap7" -> <<0, 7, 1>>
    [] c = "std_one_gap8" -> <<0, 8, 1>>
    [] c = "std_two_gap15" -> <<0, 15, 2>>
    [] c = "std_two_gap16" -> <<0, 16, 2>>
    [] c = "std_four_gap31" -> <<0, 31, 4>>
    [] c = "std_four_gap32" -> <<0, 32, 4>>
    [] c = "std_five_gap10" -> <<0, 10, 5>>
    [] OTHER -> <<0, 40, 5>>

\* @type: (Str) => <<Int, Int, Int>>;
EscalationParams(c) ==
  CASE c = "esc_subject_ahead" -> <<10, 9, 1>>
    [] c = "esc_zero_gap0" -> <<0, 0, 0>>
    [] c = "esc_zero_gap7" -> <<0, 7, 0>>
    [] c = "esc_zero_gap8" -> <<0, 8, 0>>
    [] c = "esc_one_gap7" -> <<0, 7, 1>>
    [] c = "esc_one_gap8" -> <<0, 8, 1>>
    [] c = "esc_four_gap7" -> <<0, 7, 4>>
    [] c = "esc_four_gap8" -> <<0, 8, 4>>
    [] c = "esc_five_gap9" -> <<0, 9, 5>>
    [] c = "esc_five_gap10" -> <<0, 10, 5>>
    [] c = "esc_five_gap39" -> <<0, 39, 5>>
    [] OTHER -> <<0, 40, 5>>

ViewGap(subject, proposal) ==
  IF proposal >= subject THEN proposal - subject ELSE 0

StandardThreshold(validators) ==
  IF validators * 8 >= 8 THEN validators * 8 ELSE 8

EscalationThreshold(validators) ==
  IF validators * 2 >= 8 THEN validators * 2 ELSE 8

SpecStandard(c) ==
  LET p == StandardParams(c) IN
    ViewGap(p[1], p[2]) >= StandardThreshold(p[3])

SpecEscalation(c) ==
  LET p == EscalationParams(c) IN
    ViewGap(p[1], p[2]) >= EscalationThreshold(p[3])

ActualStandard(c) ==
  CASE Bug = "std_allows_subject_ahead"
       /\ c = "std_subject_ahead" -> TRUE
    [] Bug = "std_drops_min_floor"
       /\ c = "std_zero_gap0" -> TRUE
    [] Bug = "std_allows_below_floor"
       /\ c = "std_zero_gap7" -> TRUE
    [] Bug = "std_rejects_floor_boundary"
       /\ c = "std_zero_gap8" -> FALSE
    [] Bug = "std_uses_raw_validator_count"
       /\ c = "std_one_gap7" -> TRUE
    [] Bug = "std_threshold_too_low"
       /\ c = "std_two_gap15" -> TRUE
    [] Bug = "std_uses_strict_threshold"
       /\ c = "std_two_gap16" -> FALSE
    [] Bug = "std_threshold_too_high"
       /\ c = "std_four_gap32" -> FALSE
    [] Bug = "std_uses_escalation_multiplier"
       /\ c = "std_five_gap10" -> TRUE
    [] OTHER -> SpecStandard(c)

ActualEscalation(c) ==
  CASE Bug = "esc_allows_subject_ahead"
       /\ c = "esc_subject_ahead" -> TRUE
    [] Bug = "esc_drops_min_floor"
       /\ c = "esc_zero_gap0" -> TRUE
    [] Bug = "esc_allows_below_floor"
       /\ c = "esc_zero_gap7" -> TRUE
    [] Bug = "esc_rejects_floor_boundary"
       /\ c = "esc_zero_gap8" -> FALSE
    [] Bug = "esc_uses_raw_validator_count"
       /\ c = "esc_one_gap7" -> TRUE
    [] Bug = "esc_threshold_too_low"
       /\ c = "esc_five_gap9" -> TRUE
    [] Bug = "esc_uses_strict_threshold"
       /\ c = "esc_five_gap10" -> FALSE
    [] Bug = "esc_threshold_too_high"
       /\ c = "esc_five_gap10" -> FALSE
    [] Bug = "esc_uses_standard_multiplier"
       /\ c = "esc_five_gap10" -> FALSE
    [] OTHER -> SpecEscalation(c)

SpecStandardOutput(c) ==
  BoolToInt(SpecStandard(c))

ActualStandardOutput(c) ==
  BoolToInt(ActualStandard(c))

SpecEscalationOutput(c) ==
  BoolToInt(SpecEscalation(c))

ActualEscalationOutput(c) ==
  BoolToInt(ActualEscalation(c))

SpecStandardImpliesEscalation ==
  \A c \in {"std_zero_gap8", "std_one_gap8", "std_two_gap16",
            "std_four_gap32", "std_five_gap40"}:
    LET p == StandardParams(c) IN
      ViewGap(p[1], p[2]) >= StandardThreshold(p[3])
        => ViewGap(p[1], p[2]) >= EscalationThreshold(p[3])

ActualStandardImpliesEscalation ==
  CASE Bug = "relation_escalation_above_standard" -> FALSE
    [] OTHER -> SpecStandardImpliesEscalation

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "std_allows_subject_ahead",
       "std_drops_min_floor",
       "std_allows_below_floor",
       "std_rejects_floor_boundary",
       "std_uses_raw_validator_count",
       "std_threshold_too_low",
       "std_uses_strict_threshold",
       "std_threshold_too_high",
       "std_uses_escalation_multiplier",
       "esc_allows_subject_ahead",
       "esc_drops_min_floor",
       "esc_allows_below_floor",
       "esc_rejects_floor_boundary",
       "esc_uses_raw_validator_count",
       "esc_threshold_too_low",
       "esc_uses_strict_threshold",
       "esc_threshold_too_high",
       "esc_uses_standard_multiplier",
       "relation_escalation_above_standard"
     }
  /\ checked = 0

SameHeightVoteRecoveryGapMatchesSpec ==
  /\ \A c \in StandardCases:
       ActualStandardOutput(c) = SpecStandardOutput(c)
  /\ \A c \in EscalationCases:
       ActualEscalationOutput(c) = SpecEscalationOutput(c)
  /\ ActualStandardImpliesEscalation = SpecStandardImpliesEscalation

SafetyFast ==
  SameHeightVoteRecoveryGapMatchesSpec

BugStdAllowsSubjectAhead ==
  ActualStandardOutput("std_subject_ahead") =
    SpecStandardOutput("std_subject_ahead")

BugStdDropsMinFloor ==
  ActualStandardOutput("std_zero_gap0") =
    SpecStandardOutput("std_zero_gap0")

BugStdAllowsBelowFloor ==
  ActualStandardOutput("std_zero_gap7") =
    SpecStandardOutput("std_zero_gap7")

BugStdRejectsFloorBoundary ==
  ActualStandardOutput("std_zero_gap8") =
    SpecStandardOutput("std_zero_gap8")

BugStdUsesRawValidatorCount ==
  ActualStandardOutput("std_one_gap7") =
    SpecStandardOutput("std_one_gap7")

BugStdThresholdTooLow ==
  ActualStandardOutput("std_two_gap15") =
    SpecStandardOutput("std_two_gap15")

BugStdUsesStrictThreshold ==
  ActualStandardOutput("std_two_gap16") =
    SpecStandardOutput("std_two_gap16")

BugStdThresholdTooHigh ==
  ActualStandardOutput("std_four_gap32") =
    SpecStandardOutput("std_four_gap32")

BugStdUsesEscalationMultiplier ==
  ActualStandardOutput("std_five_gap10") =
    SpecStandardOutput("std_five_gap10")

BugEscAllowsSubjectAhead ==
  ActualEscalationOutput("esc_subject_ahead") =
    SpecEscalationOutput("esc_subject_ahead")

BugEscDropsMinFloor ==
  ActualEscalationOutput("esc_zero_gap0") =
    SpecEscalationOutput("esc_zero_gap0")

BugEscAllowsBelowFloor ==
  ActualEscalationOutput("esc_zero_gap7") =
    SpecEscalationOutput("esc_zero_gap7")

BugEscRejectsFloorBoundary ==
  ActualEscalationOutput("esc_zero_gap8") =
    SpecEscalationOutput("esc_zero_gap8")

BugEscUsesRawValidatorCount ==
  ActualEscalationOutput("esc_one_gap7") =
    SpecEscalationOutput("esc_one_gap7")

BugEscThresholdTooLow ==
  ActualEscalationOutput("esc_five_gap9") =
    SpecEscalationOutput("esc_five_gap9")

BugEscUsesStrictThreshold ==
  ActualEscalationOutput("esc_five_gap10") =
    SpecEscalationOutput("esc_five_gap10")

BugEscThresholdTooHigh ==
  ActualEscalationOutput("esc_five_gap10") =
    SpecEscalationOutput("esc_five_gap10")

BugEscUsesStandardMultiplier ==
  ActualEscalationOutput("esc_five_gap10") =
    SpecEscalationOutput("esc_five_gap10")

BugRelationEscalationAboveStandard ==
  ActualStandardImpliesEscalation = SpecStandardImpliesEscalation

====
