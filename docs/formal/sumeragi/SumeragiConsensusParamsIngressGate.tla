---- MODULE SumeragiConsensusParamsIngressGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for inbound consensus-params advert handling.

This slice captures `expected_collector_params_for_advert(...)` and
`handle_consensus_params(...)` from `main_loop/proposal_handlers.rs`. Absent
membership uses the current collector-plan parameters; present membership uses
the membership height and ignores view, epoch, and optional view hash for this
collector-parameter expectation. The handler emits independent mismatch
diagnostics for `collectors_k` and `redundant_send_r`, updates telemetry from
the advertised values, and remains fail-open.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoCase == "none"
NoMembershipMatch == "no_membership_match"
NoMembershipKMismatch == "no_membership_k_mismatch"
NoMembershipRMismatch == "no_membership_r_mismatch"
NoMembershipBothMismatch == "no_membership_both_mismatch"
MemberH10Match == "member_h10_match"
MemberH10KMismatch == "member_h10_k_mismatch"
MemberH10RMismatch == "member_h10_r_mismatch"
MemberH10BothMismatch == "member_h10_both_mismatch"
MemberH11Match == "member_h11_match"
MemberSameHeightDifferentViewEpoch == "member_same_height_different_view_epoch"
MemberHashAbsent == "member_hash_absent"
MemberHashPresent == "member_hash_present"

Cases == {
  NoMembershipMatch,
  NoMembershipKMismatch,
  NoMembershipRMismatch,
  NoMembershipBothMismatch,
  MemberH10Match,
  MemberH10KMismatch,
  MemberH10RMismatch,
  MemberH10BothMismatch,
  MemberH11Match,
  MemberSameHeightDifferentViewEpoch,
  MemberHashAbsent,
  MemberHashPresent
}

MembershipCases == {
  MemberH10Match,
  MemberH10KMismatch,
  MemberH10RMismatch,
  MemberH10BothMismatch,
  MemberH11Match,
  MemberSameHeightDifferentViewEpoch,
  MemberHashAbsent,
  MemberHashPresent
}

NoMembershipCases == Cases \ MembershipCases

Height10 == 10
Height11 == 11
CurrentK == 2
CurrentR == 1
K10 == 3
R10 == 2
K11 == 4
R11 == 3
MismatchedK == 5
MismatchedR == 4
NoHeight == 0

AdvertMembershipPresent(c) == c \in MembershipCases

MembershipHeight(c) ==
  CASE c = MemberH11Match -> Height11
    [] OTHER -> Height10

MembershipView(c) ==
  CASE c = MemberSameHeightDifferentViewEpoch -> Height11
    [] OTHER -> 1

MembershipEpoch(c) ==
  CASE c = MemberSameHeightDifferentViewEpoch -> Height11
    [] OTHER -> 0

MembershipHashPresent(c) ==
  c \notin (NoMembershipCases \union {MemberHashAbsent})

KForHeight(h) ==
  CASE h = Height10 -> K10
    [] h = Height11 -> K11
    [] OTHER -> 0

RForHeight(h) ==
  CASE h = Height10 -> R10
    [] h = Height11 -> R11
    [] OTHER -> 0

AdvertK(c) ==
  CASE c \in {NoMembershipKMismatch, NoMembershipBothMismatch,
              MemberH10KMismatch, MemberH10BothMismatch} ->
      MismatchedK
    [] c = MemberH11Match -> K11
    [] c \in MembershipCases -> K10
    [] OTHER -> CurrentK

AdvertR(c) ==
  CASE c \in {NoMembershipRMismatch, NoMembershipBothMismatch,
              MemberH10RMismatch, MemberH10BothMismatch} ->
      MismatchedR
    [] c = MemberH11Match -> R11
    [] c \in MembershipCases -> R10
    [] OTHER -> CurrentR

SpecExpectedK(c) ==
  IF AdvertMembershipPresent(c)
  THEN KForHeight(MembershipHeight(c))
  ELSE CurrentK

SpecExpectedR(c) ==
  IF AdvertMembershipPresent(c)
  THEN RForHeight(MembershipHeight(c))
  ELSE CurrentR

ActualExpectedK(c) ==
  CASE Bug = "membership_ignored"
       /\ c = MemberH10Match ->
      CurrentK
    [] Bug = "none_uses_membership_height"
       /\ c = NoMembershipMatch ->
      KForHeight(Height10)
    [] Bug = "membership_uses_view"
       /\ c = MemberSameHeightDifferentViewEpoch ->
      KForHeight(MembershipView(c))
    [] Bug = "membership_uses_epoch"
       /\ c = MemberSameHeightDifferentViewEpoch ->
      KForHeight(MembershipEpoch(c))
    [] Bug = "membership_hash_required"
       /\ c = MemberHashAbsent ->
      CurrentK
    [] Bug = "expected_k_from_advert"
       /\ c = MemberH10KMismatch ->
      AdvertK(c)
    [] OTHER -> SpecExpectedK(c)

ActualExpectedR(c) ==
  CASE Bug = "membership_ignored"
       /\ c = MemberH10Match ->
      CurrentR
    [] Bug = "none_uses_membership_height"
       /\ c = NoMembershipMatch ->
      RForHeight(Height10)
    [] Bug = "membership_uses_view"
       /\ c = MemberSameHeightDifferentViewEpoch ->
      RForHeight(MembershipView(c))
    [] Bug = "membership_uses_epoch"
       /\ c = MemberSameHeightDifferentViewEpoch ->
      RForHeight(MembershipEpoch(c))
    [] Bug = "membership_hash_required"
       /\ c = MemberHashAbsent ->
      CurrentR
    [] Bug = "expected_r_from_advert"
       /\ c = MemberH10RMismatch ->
      AdvertR(c)
    [] OTHER -> SpecExpectedR(c)

SpecKWarn(c) == AdvertK(c) /= SpecExpectedK(c)
SpecRWarn(c) == AdvertR(c) /= SpecExpectedR(c)

ActualKWarn(c) ==
  CASE Bug = "k_warning_suppressed"
       /\ c = MemberH10KMismatch ->
      FALSE
    [] Bug = "k_warning_spurious"
       /\ c = MemberH10Match ->
      TRUE
    [] OTHER -> AdvertK(c) /= ActualExpectedK(c)

ActualRWarn(c) ==
  CASE Bug = "r_warning_suppressed"
       /\ c = MemberH10RMismatch ->
      FALSE
    [] Bug = "r_warning_spurious"
       /\ c = MemberH10Match ->
      TRUE
    [] Bug = "r_warning_short_circuited_by_k"
       /\ c = MemberH10BothMismatch ->
      FALSE
    [] OTHER -> AdvertR(c) /= ActualExpectedR(c)

SpecTelemetrySet(c) == TRUE
SpecTelemetryK(c) == AdvertK(c)
SpecTelemetryR(c) == AdvertR(c)

ActualTelemetrySet(c) ==
  CASE Bug = "telemetry_not_set" /\ c = MemberH10Match -> FALSE
    [] OTHER -> TRUE

ActualTelemetryK(c) ==
  CASE Bug = "telemetry_uses_expected_k"
       /\ c = MemberH10KMismatch ->
      ActualExpectedK(c)
    [] Bug = "telemetry_k_dropped"
       /\ c = MemberH10Match ->
      0
    [] OTHER -> AdvertK(c)

ActualTelemetryR(c) ==
  CASE Bug = "telemetry_uses_expected_r"
       /\ c = MemberH10RMismatch ->
      ActualExpectedR(c)
    [] Bug = "telemetry_r_dropped"
       /\ c = MemberH10Match ->
      0
    [] OTHER -> AdvertR(c)

SpecResultOk(c) == TRUE

ActualResultOk(c) ==
  CASE Bug = "mismatch_returns_error"
       /\ c = MemberH10BothMismatch ->
      FALSE
    [] Bug = "match_returns_error"
       /\ c = MemberH10Match ->
      FALSE
    [] OTHER -> TRUE

Bugs == {
  "none",
  "membership_ignored",
  "none_uses_membership_height",
  "membership_uses_view",
  "membership_uses_epoch",
  "membership_hash_required",
  "expected_k_from_advert",
  "expected_r_from_advert",
  "k_warning_suppressed",
  "k_warning_spurious",
  "r_warning_suppressed",
  "r_warning_spurious",
  "r_warning_short_circuited_by_k",
  "telemetry_not_set",
  "telemetry_uses_expected_k",
  "telemetry_uses_expected_r",
  "telemetry_k_dropped",
  "telemetry_r_dropped",
  "mismatch_returns_error",
  "match_returns_error"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ AdvertMembershipPresent(c) \in BOOLEAN
       /\ MembershipHeight(c) \in {Height10, Height11}
       /\ MembershipView(c) \in 0..Height11
       /\ MembershipEpoch(c) \in 0..Height11
       /\ MembershipHashPresent(c) \in BOOLEAN
       /\ AdvertK(c) \in 0..MismatchedK
       /\ AdvertR(c) \in 0..MismatchedR
       /\ SpecExpectedK(c) \in 0..MismatchedK
       /\ SpecExpectedR(c) \in 0..MismatchedR
       /\ ActualExpectedK(c) \in 0..MismatchedK
       /\ ActualExpectedR(c) \in 0..MismatchedR
       /\ SpecKWarn(c) \in BOOLEAN
       /\ SpecRWarn(c) \in BOOLEAN
       /\ ActualKWarn(c) \in BOOLEAN
       /\ ActualRWarn(c) \in BOOLEAN
       /\ ActualTelemetrySet(c) \in BOOLEAN
       /\ ActualTelemetryK(c) \in 0..MismatchedK
       /\ ActualTelemetryR(c) \in 0..MismatchedR
       /\ ActualResultOk(c) \in BOOLEAN

ExpectedCollectorParamsMatch(c) ==
  /\ ActualExpectedK(c) = SpecExpectedK(c)
  /\ ActualExpectedR(c) = SpecExpectedR(c)

WarningFieldsMatch(c) ==
  /\ ActualKWarn(c) = SpecKWarn(c)
  /\ ActualRWarn(c) = SpecRWarn(c)

TelemetryFieldsMatch(c) ==
  /\ ActualTelemetrySet(c) = SpecTelemetrySet(c)
  /\ ActualTelemetryK(c) = SpecTelemetryK(c)
  /\ ActualTelemetryR(c) = SpecTelemetryR(c)

ResultFieldsMatch(c) ==
  ActualResultOk(c) = SpecResultOk(c)

ConsensusParamsExpectedCollectorExact ==
  \A c \in Cases:
    ExpectedCollectorParamsMatch(c)

ConsensusParamsWarningsExact ==
  \A c \in Cases:
    WarningFieldsMatch(c)

ConsensusParamsTelemetryExact ==
  \A c \in Cases:
    TelemetryFieldsMatch(c)

ConsensusParamsResultExact ==
  \A c \in Cases:
    ResultFieldsMatch(c)

ConsensusParamsIngressExactness ==
  /\ ConsensusParamsExpectedCollectorExact
  /\ ConsensusParamsWarningsExact
  /\ ConsensusParamsTelemetryExact
  /\ ConsensusParamsResultExact

IngressMatchesSpec ==
  ConsensusParamsIngressExactness

====
