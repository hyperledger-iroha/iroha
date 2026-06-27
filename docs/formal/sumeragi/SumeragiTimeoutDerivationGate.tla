---- MODULE SumeragiTimeoutDerivationGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for Sumeragi timeout and cooldown derivation helpers.

This slice pins:
`control_plane_rebroadcast_cooldown_from_block_time(...)`,
`rebroadcast_cooldown_from_block_time(...)`,
`payload_rebroadcast_cooldown_from_block_time(...)`,
`targeted_payload_rescue_cooldown_from_block_time(...)`,
`quorum_reschedule_backoff_from_timeout(...)`,
`commit_quorum_timeout_from_durations(...)`,
`pacemaker_base_interval_with_propose_timeout(...)`,
`availability_timeout_from_quorum(...)`,
`availability_gate_timeout_exceeded(...)`,
`missing_quorum_stale(...)`, and `prevote_quorum_stale(...)`.

Durations are modeled as integer microseconds. The finite `MaxUs` cap stands in
for Rust's saturating `Duration` multiplication cap.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

MaxUs == 20000000
OneMs == 1000

Min(a, b) == IF a <= b THEN a ELSE b

Max(a, b) == IF a >= b THEN a ELSE b

Clamp(v, lo, hi) == Max(lo, Min(v, hi))

SaturatingMul(v, m) == Min(v * m, MaxUs)

ControlFloor == 25000
ControlCeiling == 200000
ControlDivisor == 8
PayloadMultiplier == 2
TargetedFloor == 500000
BackoffFloor == 100000
BackoffCeiling == 800000
BackoffDivisor == 4

ControlCases == {
  "control_zero",
  "control_floor",
  "control_scaled",
  "control_ceiling"
}

BlockTime(c) ==
  CASE c = "control_zero" -> 0
    [] c = "control_floor" -> 150000
    [] c = "control_scaled" -> 1001000
    [] c = "control_ceiling" -> 2000000
    [] OTHER -> 0

RawControl(c) ==
  IF BlockTime(c) = 0 THEN ControlFloor ELSE BlockTime(c) \div ControlDivisor

SpecControl(c) ==
  Clamp(RawControl(c), ControlFloor, ControlCeiling)

ActualControl(c) ==
  CASE Bug = "control_zero_returns_zero"
       /\ c = "control_zero" -> 0
    [] Bug = "control_skips_floor"
       /\ c = "control_floor" -> RawControl(c)
    [] Bug = "control_uses_block_time"
       /\ c = "control_scaled" -> BlockTime(c)
    [] Bug = "control_skips_ceiling"
       /\ c = "control_ceiling" -> RawControl(c)
    [] OTHER -> SpecControl(c)

SpecPayload(c) ==
  SaturatingMul(SpecControl(c), PayloadMultiplier)

SpecAlias(c) == SpecControl(c)

ActualAlias(c) ==
  CASE Bug = "alias_diverges"
       /\ c = "control_scaled" -> SpecPayload(c)
    [] OTHER -> SpecAlias(c)

ActualPayload(c) ==
  CASE Bug = "payload_skips_multiplier"
       /\ c = "control_scaled" -> SpecControl(c)
    [] Bug = "payload_uses_unclamped_control"
       /\ c = "control_floor" -> SaturatingMul(RawControl(c), PayloadMultiplier)
    [] Bug = "payload_uses_block_time"
       /\ c = "control_scaled" -> BlockTime(c)
    [] OTHER -> SpecPayload(c)

SpecTargeted(c) ==
  Max(SpecPayload(c), TargetedFloor)

ActualTargeted(c) ==
  CASE Bug = "targeted_skips_floor"
       /\ c = "control_scaled" -> SpecPayload(c)
    [] Bug = "targeted_uses_control"
       /\ c = "control_scaled" -> SpecControl(c)
    [] Bug = "targeted_uses_min"
       /\ c = "control_scaled" -> Min(SpecPayload(c), TargetedFloor)
    [] OTHER -> SpecTargeted(c)

BackoffCases == {
  "backoff_zero",
  "backoff_floor",
  "backoff_scaled",
  "backoff_ceiling"
}

QuorumTimeout(c) ==
  CASE c = "backoff_zero" -> 0
    [] c = "backoff_floor" -> 200000
    [] c = "backoff_scaled" -> 800000
    [] c = "backoff_ceiling" -> 5000000
    [] OTHER -> 0

RawBackoff(c) ==
  IF QuorumTimeout(c) = 0 THEN BackoffFloor ELSE QuorumTimeout(c) \div BackoffDivisor

SpecBackoff(c) ==
  Clamp(RawBackoff(c), BackoffFloor, BackoffCeiling)

ActualBackoff(c) ==
  CASE Bug = "backoff_zero_returns_zero"
       /\ c = "backoff_zero" -> 0
    [] Bug = "backoff_skips_floor"
       /\ c = "backoff_floor" -> RawBackoff(c)
    [] Bug = "backoff_skips_ceiling"
       /\ c = "backoff_ceiling" -> RawBackoff(c)
    [] Bug = "backoff_uses_timeout"
       /\ c = "backoff_scaled" -> QuorumTimeout(c)
    [] OTHER -> SpecBackoff(c)

CommitCases == {
  "commit_zero_block_zero",
  "commit_zero_block_large",
  "commit_no_da_block_greater",
  "commit_no_da_commit_greater",
  "commit_da_multiplier",
  "commit_da_multiplier_min_one",
  "commit_da_floor"
}

CommitBlock(c) ==
  CASE c = "commit_zero_block_large" -> 2000000
    [] c = "commit_no_da_block_greater" -> 2000000
    [] c = "commit_no_da_commit_greater" -> 1000000
    [] c = "commit_da_multiplier" -> 1000000
    [] OTHER -> 0

CommitTime(c) ==
  CASE c = "commit_no_da_block_greater" -> 500000
    [] c = "commit_no_da_commit_greater" -> 2500000
    [] c = "commit_da_multiplier" -> 2500000
    [] c = "commit_da_multiplier_min_one" -> 200000
    [] c = "commit_da_floor" -> 1
    [] OTHER -> 0

CommitDa(c) ==
  c \in {"commit_da_multiplier", "commit_da_multiplier_min_one", "commit_da_floor"}

CommitMultiplier(c) ==
  CASE c = "commit_da_multiplier_min_one" -> 0
    [] c = "commit_da_floor" -> 1
    [] OTHER -> 2

SpecCommitTimeout(c) ==
  IF CommitTime(c) = 0 THEN
    Max(CommitBlock(c), OneMs)
  ELSE
    LET base == IF CommitDa(c)
                THEN CommitBlock(c) + SaturatingMul(CommitTime(c), 3)
                ELSE Max(CommitBlock(c), CommitTime(c)) IN
    LET scaled == IF CommitDa(c)
                  THEN SaturatingMul(base, Max(CommitMultiplier(c), 1))
                  ELSE base IN
      Max(scaled, OneMs)

ActualCommitTimeout(c) ==
  CASE Bug = "commit_zero_commit_returns_zero"
       /\ c = "commit_zero_block_zero" -> 0
    [] Bug = "commit_no_da_uses_min"
       /\ c = "commit_no_da_commit_greater" ->
       Min(CommitBlock(c), CommitTime(c))
    [] Bug = "commit_da_skips_commit_factor"
       /\ c = "commit_da_multiplier" ->
       SaturatingMul(CommitBlock(c) + CommitTime(c), Max(CommitMultiplier(c), 1))
    [] Bug = "commit_da_skips_multiplier"
       /\ c = "commit_da_multiplier" ->
       CommitBlock(c) + SaturatingMul(CommitTime(c), 3)
    [] Bug = "commit_da_allows_zero_multiplier"
       /\ c = "commit_da_multiplier_min_one" -> 0
    [] Bug = "commit_omits_floor"
       /\ c = "commit_da_floor" ->
       CommitBlock(c) + SaturatingMul(CommitTime(c), 3)
    [] OTHER -> SpecCommitTimeout(c)

PacemakerCases == {
  "pacemaker_scaled",
  "pacemaker_capped",
  "pacemaker_rtt_zero",
  "pacemaker_explicit_propose",
  "pacemaker_zero_max"
}

PacemakerBlock(c) ==
  CASE c = "pacemaker_explicit_propose" -> 800000
    [] OTHER -> 1500000

PacemakerPropose(c) ==
  CASE c = "pacemaker_explicit_propose" -> 700000
    [] c = "pacemaker_zero_max" -> 300000
    [] OTHER -> 300000

PacemakerRtt(c) ==
  CASE c = "pacemaker_scaled" -> 3
    [] c = "pacemaker_capped" -> 5
    [] c = "pacemaker_rtt_zero" -> 0
    [] c = "pacemaker_explicit_propose" -> 2
    [] OTHER -> 1

PacemakerMax(c) ==
  CASE c = "pacemaker_scaled" -> 1200000
    [] c = "pacemaker_capped" -> 1200000
    [] c = "pacemaker_zero_max" -> 0
    [] OTHER -> 5000000

SpecPacemaker(c) ==
  Min(SaturatingMul(PacemakerPropose(c), Max(PacemakerRtt(c), 1)), PacemakerMax(c))

ActualPacemaker(c) ==
  CASE Bug = "pacemaker_uses_block_time"
       /\ c = "pacemaker_explicit_propose" ->
       Min(SaturatingMul(PacemakerBlock(c), Max(PacemakerRtt(c), 1)), PacemakerMax(c))
    [] Bug = "pacemaker_skips_rtt_floor"
       /\ c = "pacemaker_rtt_zero" ->
       Min(SaturatingMul(PacemakerPropose(c), PacemakerRtt(c)), PacemakerMax(c))
    [] Bug = "pacemaker_skips_cap"
       /\ c = "pacemaker_capped" ->
       SaturatingMul(PacemakerPropose(c), Max(PacemakerRtt(c), 1))
    [] OTHER -> SpecPacemaker(c)

AvailabilityCases == {
  "availability_no_da",
  "availability_da_above_floor",
  "availability_da_floor",
  "availability_da_multiplier_zero"
}

AvailabilityQuorum(c) ==
  CASE c = "availability_no_da" -> 500000
    [] c = "availability_da_above_floor" -> 500000
    [] c = "availability_da_floor" -> 100000
    [] c = "availability_da_multiplier_zero" -> 700000
    [] OTHER -> 0

AvailabilityDa(c) ==
  c # "availability_no_da"

AvailabilityMultiplier(c) ==
  CASE c = "availability_da_multiplier_zero" -> 0
    [] OTHER -> 2

AvailabilityFloor(c) ==
  CASE c = "availability_da_floor" -> 500000
    [] OTHER -> 250000

SpecAvailability(c) ==
  IF ~AvailabilityDa(c) THEN
    AvailabilityQuorum(c)
  ELSE
    SaturatingMul(
      Max(AvailabilityQuorum(c), AvailabilityFloor(c)),
      Max(AvailabilityMultiplier(c), 1)
    )

ActualAvailability(c) ==
  CASE Bug = "availability_no_da_scales"
       /\ c = "availability_no_da" ->
       SaturatingMul(
         Max(AvailabilityQuorum(c), AvailabilityFloor(c)),
         Max(AvailabilityMultiplier(c), 1)
       )
    [] Bug = "availability_da_skips_floor"
       /\ c = "availability_da_floor" ->
       SaturatingMul(AvailabilityQuorum(c), Max(AvailabilityMultiplier(c), 1))
    [] Bug = "availability_da_skips_multiplier"
       /\ c = "availability_da_above_floor" ->
       Max(AvailabilityQuorum(c), AvailabilityFloor(c))
    [] Bug = "availability_da_allows_zero_multiplier"
       /\ c = "availability_da_multiplier_zero" -> 0
    [] OTHER -> SpecAvailability(c)

AvailabilityGateCases == {
  "availability_gate_before",
  "availability_gate_at",
  "availability_gate_after",
  "availability_gate_zero"
}

AvailabilityGateAge(c) ==
  CASE c = "availability_gate_before" -> 499000
    [] c = "availability_gate_at" -> 500000
    [] c = "availability_gate_after" -> 600000
    [] OTHER -> 50000

AvailabilityGateTimeout(c) ==
  IF c = "availability_gate_zero" THEN 0 ELSE 500000

SpecAvailabilityGate(c) ==
  /\ AvailabilityGateTimeout(c) # 0
  /\ AvailabilityGateAge(c) >= AvailabilityGateTimeout(c)

ActualAvailabilityGate(c) ==
  CASE Bug = "availability_gate_zero_timeout"
       /\ c = "availability_gate_zero" -> TRUE
    [] Bug = "availability_gate_strict_age"
       /\ c = "availability_gate_at" -> FALSE
    [] OTHER -> SpecAvailabilityGate(c)

MissingQuorumCases == {
  "missing_quorum_before",
  "missing_quorum_at",
  "missing_quorum_after",
  "missing_quorum_zero_timeout",
  "missing_quorum_reached"
}

MissingQuorumAge(c) ==
  CASE c = "missing_quorum_before" -> 499000
    [] c = "missing_quorum_reached" -> 600000
    [] OTHER -> 500000

MissingQuorumTimeout(c) ==
  IF c = "missing_quorum_zero_timeout" THEN 0 ELSE 500000

MissingQuorumReached(c) ==
  c = "missing_quorum_reached"

SpecMissingQuorum(c) ==
  /\ MissingQuorumTimeout(c) # 0
  /\ ~MissingQuorumReached(c)
  /\ MissingQuorumAge(c) >= MissingQuorumTimeout(c)

ActualMissingQuorum(c) ==
  CASE Bug = "missing_quorum_zero_timeout_stale"
       /\ c = "missing_quorum_zero_timeout" -> TRUE
    [] Bug = "missing_quorum_ignores_quorum"
       /\ c = "missing_quorum_reached" -> TRUE
    [] Bug = "missing_quorum_strict_age"
       /\ c = "missing_quorum_at" -> FALSE
    [] OTHER -> SpecMissingQuorum(c)

PrevoteCases == {
  "prevote_prepare_before",
  "prevote_prepare_at",
  "prevote_prepare_after",
  "prevote_commit_after",
  "prevote_none_after",
  "prevote_prepare_zero_timeout"
}

PrevotePhase(c) ==
  CASE c \in {
       "prevote_prepare_before",
       "prevote_prepare_at",
       "prevote_prepare_after",
       "prevote_prepare_zero_timeout"
     } -> "prepare"
    [] c = "prevote_commit_after" -> "commit"
    [] OTHER -> "none"

PrevoteAge(c) ==
  CASE c = "prevote_prepare_before" -> 499000
    [] OTHER -> 500000

PrevoteTimeout(c) ==
  IF c = "prevote_prepare_zero_timeout" THEN 0 ELSE 500000

SpecPrevote(c) ==
  /\ PrevotePhase(c) = "prepare"
  /\ PrevoteTimeout(c) # 0
  /\ PrevoteAge(c) >= PrevoteTimeout(c)

ActualPrevote(c) ==
  CASE Bug = "prevote_accepts_commit_phase"
       /\ c = "prevote_commit_after" -> TRUE
    [] Bug = "prevote_zero_timeout_stale"
       /\ c = "prevote_prepare_zero_timeout" -> TRUE
    [] Bug = "prevote_strict_age"
       /\ c = "prevote_prepare_at" -> FALSE
    [] OTHER -> SpecPrevote(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "control_zero_returns_zero",
       "control_skips_floor",
       "control_uses_block_time",
       "control_skips_ceiling",
       "alias_diverges",
       "payload_skips_multiplier",
       "payload_uses_unclamped_control",
       "payload_uses_block_time",
       "targeted_skips_floor",
       "targeted_uses_control",
       "targeted_uses_min",
       "backoff_zero_returns_zero",
       "backoff_skips_floor",
       "backoff_skips_ceiling",
       "backoff_uses_timeout",
       "commit_zero_commit_returns_zero",
       "commit_no_da_uses_min",
       "commit_da_skips_commit_factor",
       "commit_da_skips_multiplier",
       "commit_da_allows_zero_multiplier",
       "commit_omits_floor",
       "pacemaker_uses_block_time",
       "pacemaker_skips_rtt_floor",
       "pacemaker_skips_cap",
       "availability_no_da_scales",
       "availability_da_skips_floor",
       "availability_da_skips_multiplier",
       "availability_da_allows_zero_multiplier",
       "availability_gate_zero_timeout",
       "availability_gate_strict_age",
       "missing_quorum_zero_timeout_stale",
       "missing_quorum_ignores_quorum",
       "missing_quorum_strict_age",
       "prevote_accepts_commit_phase",
       "prevote_zero_timeout_stale",
       "prevote_strict_age"
     }
  /\ checked = 0

ControlCooldownMatchesSpec ==
  /\ \A c \in ControlCases:
       ActualControl(c) = SpecControl(c)

AliasCooldownMatchesSpec ==
  /\ \A c \in ControlCases:
       ActualAlias(c) = SpecAlias(c)

PayloadCooldownMatchesSpec ==
  /\ \A c \in ControlCases:
       ActualPayload(c) = SpecPayload(c)

TargetedCooldownMatchesSpec ==
  /\ \A c \in ControlCases:
       ActualTargeted(c) = SpecTargeted(c)

BackoffMatchesSpec ==
  /\ \A c \in BackoffCases:
       ActualBackoff(c) = SpecBackoff(c)

CommitTimeoutMatchesSpec ==
  /\ \A c \in CommitCases:
       ActualCommitTimeout(c) = SpecCommitTimeout(c)

PacemakerMatchesSpec ==
  /\ \A c \in PacemakerCases:
       ActualPacemaker(c) = SpecPacemaker(c)

AvailabilityTimeoutMatchesSpec ==
  /\ \A c \in AvailabilityCases:
       ActualAvailability(c) = SpecAvailability(c)

AvailabilityGateMatchesSpec ==
  /\ \A c \in AvailabilityGateCases:
       ActualAvailabilityGate(c) = SpecAvailabilityGate(c)

MissingQuorumMatchesSpec ==
  /\ \A c \in MissingQuorumCases:
       ActualMissingQuorum(c) = SpecMissingQuorum(c)

PrevoteQuorumMatchesSpec ==
  /\ \A c \in PrevoteCases:
       ActualPrevote(c) = SpecPrevote(c)

ControlCooldownBounds ==
  \A c \in ControlCases:
    /\ ActualControl(c) >= ControlFloor
    /\ ActualControl(c) <= ControlCeiling

PayloadCooldownUsesControlMultiplier ==
  \A c \in ControlCases:
    ActualPayload(c) = SaturatingMul(ActualControl(c), PayloadMultiplier)

TargetedCooldownHasFloor ==
  \A c \in ControlCases:
    ActualTargeted(c) >= TargetedFloor

BackoffBounds ==
  \A c \in BackoffCases:
    /\ ActualBackoff(c) >= BackoffFloor
    /\ ActualBackoff(c) <= BackoffCeiling

CommitTimeoutHasFloor ==
  \A c \in CommitCases:
    ActualCommitTimeout(c) >= OneMs

AvailabilityGateRequiresPositiveTimeout ==
  \A c \in AvailabilityGateCases:
    ActualAvailabilityGate(c) => AvailabilityGateTimeout(c) # 0

MissingQuorumRequiresPositiveTimeoutAndMissingQuorum ==
  \A c \in MissingQuorumCases:
    ActualMissingQuorum(c) =>
      /\ MissingQuorumTimeout(c) # 0
      /\ ~MissingQuorumReached(c)

PrevoteRequiresPreparePhaseAndPositiveTimeout ==
  \A c \in PrevoteCases:
    ActualPrevote(c) =>
      /\ PrevotePhase(c) = "prepare"
      /\ PrevoteTimeout(c) # 0

ControlCooldownAnchors ==
  /\ SpecControl("control_zero") = ControlFloor
  /\ SpecControl("control_floor") = ControlFloor
  /\ SpecControl("control_scaled") = 125125
  /\ SpecControl("control_ceiling") = ControlCeiling

PayloadCooldownAnchors ==
  /\ SpecAlias("control_scaled") = SpecControl("control_scaled")
  /\ SpecPayload("control_zero") = 50000
  /\ SpecPayload("control_floor") = 50000
  /\ SpecPayload("control_scaled") = 250250
  /\ SpecPayload("control_ceiling") = 400000
  /\ SpecTargeted("control_zero") = TargetedFloor
  /\ SpecTargeted("control_floor") = TargetedFloor
  /\ SpecTargeted("control_scaled") = TargetedFloor
  /\ SpecTargeted("control_ceiling") = TargetedFloor

BackoffAnchors ==
  /\ SpecBackoff("backoff_zero") = BackoffFloor
  /\ SpecBackoff("backoff_floor") = BackoffFloor
  /\ SpecBackoff("backoff_scaled") = 200000
  /\ SpecBackoff("backoff_ceiling") = BackoffCeiling

CommitTimeoutAnchors ==
  /\ SpecCommitTimeout("commit_zero_block_zero") = OneMs
  /\ SpecCommitTimeout("commit_zero_block_large") = 2000000
  /\ SpecCommitTimeout("commit_no_da_block_greater") = 2000000
  /\ SpecCommitTimeout("commit_no_da_commit_greater") = 2500000
  /\ SpecCommitTimeout("commit_da_multiplier") = 17000000
  /\ SpecCommitTimeout("commit_da_multiplier_min_one") = 600000
  /\ SpecCommitTimeout("commit_da_floor") = OneMs

PacemakerAnchors ==
  /\ SpecPacemaker("pacemaker_scaled") = 900000
  /\ SpecPacemaker("pacemaker_capped") = 1200000
  /\ SpecPacemaker("pacemaker_rtt_zero") = 300000
  /\ SpecPacemaker("pacemaker_explicit_propose") = 1400000
  /\ SpecPacemaker("pacemaker_zero_max") = 0

AvailabilityAnchors ==
  /\ SpecAvailability("availability_no_da") = 500000
  /\ SpecAvailability("availability_da_above_floor") = 1000000
  /\ SpecAvailability("availability_da_floor") = 1000000
  /\ SpecAvailability("availability_da_multiplier_zero") = 700000

StaleGateAnchors ==
  /\ SpecAvailabilityGate("availability_gate_before") = FALSE
  /\ SpecAvailabilityGate("availability_gate_at") = TRUE
  /\ SpecAvailabilityGate("availability_gate_after") = TRUE
  /\ SpecAvailabilityGate("availability_gate_zero") = FALSE
  /\ SpecMissingQuorum("missing_quorum_before") = FALSE
  /\ SpecMissingQuorum("missing_quorum_at") = TRUE
  /\ SpecMissingQuorum("missing_quorum_after") = TRUE
  /\ SpecMissingQuorum("missing_quorum_zero_timeout") = FALSE
  /\ SpecMissingQuorum("missing_quorum_reached") = FALSE
  /\ SpecPrevote("prevote_prepare_before") = FALSE
  /\ SpecPrevote("prevote_prepare_at") = TRUE
  /\ SpecPrevote("prevote_prepare_after") = TRUE
  /\ SpecPrevote("prevote_commit_after") = FALSE
  /\ SpecPrevote("prevote_none_after") = FALSE
  /\ SpecPrevote("prevote_prepare_zero_timeout") = FALSE

TimeoutControlCooldownExact ==
  /\ ControlCooldownMatchesSpec
  /\ AliasCooldownMatchesSpec
  /\ ControlCooldownBounds
  /\ ControlCooldownAnchors

TimeoutPayloadCooldownExact ==
  /\ PayloadCooldownMatchesSpec
  /\ TargetedCooldownMatchesSpec
  /\ PayloadCooldownUsesControlMultiplier
  /\ TargetedCooldownHasFloor
  /\ PayloadCooldownAnchors

TimeoutBackoffExact ==
  /\ BackoffMatchesSpec
  /\ BackoffBounds
  /\ BackoffAnchors

TimeoutCommitPacemakerExact ==
  /\ CommitTimeoutMatchesSpec
  /\ PacemakerMatchesSpec
  /\ CommitTimeoutHasFloor
  /\ CommitTimeoutAnchors
  /\ PacemakerAnchors

TimeoutAvailabilityExact ==
  /\ AvailabilityTimeoutMatchesSpec
  /\ AvailabilityAnchors

TimeoutStaleGateExact ==
  /\ AvailabilityGateMatchesSpec
  /\ MissingQuorumMatchesSpec
  /\ PrevoteQuorumMatchesSpec
  /\ AvailabilityGateRequiresPositiveTimeout
  /\ MissingQuorumRequiresPositiveTimeoutAndMissingQuorum
  /\ PrevoteRequiresPreparePhaseAndPositiveTimeout
  /\ StaleGateAnchors

TimeoutDerivationExactness ==
  /\ TimeoutControlCooldownExact
  /\ TimeoutPayloadCooldownExact
  /\ TimeoutBackoffExact
  /\ TimeoutCommitPacemakerExact
  /\ TimeoutAvailabilityExact
  /\ TimeoutStaleGateExact

TimeoutDerivationCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ TimeoutDerivationExactness

SafetyFast ==
  TimeoutDerivationExactness

BugControlZeroReturnsZero ==
  ActualControl("control_zero") = SpecControl("control_zero")

BugControlSkipsFloor ==
  ActualControl("control_floor") = SpecControl("control_floor")

BugControlUsesBlockTime ==
  ActualControl("control_scaled") = SpecControl("control_scaled")

BugControlSkipsCeiling ==
  ActualControl("control_ceiling") = SpecControl("control_ceiling")

BugAliasDiverges ==
  ActualAlias("control_scaled") = SpecAlias("control_scaled")

BugPayloadSkipsMultiplier ==
  ActualPayload("control_scaled") = SpecPayload("control_scaled")

BugPayloadUsesUnclampedControl ==
  ActualPayload("control_floor") = SpecPayload("control_floor")

BugPayloadUsesBlockTime ==
  ActualPayload("control_scaled") = SpecPayload("control_scaled")

BugTargetedSkipsFloor ==
  ActualTargeted("control_scaled") = SpecTargeted("control_scaled")

BugTargetedUsesControl ==
  ActualTargeted("control_scaled") = SpecTargeted("control_scaled")

BugTargetedUsesMin ==
  ActualTargeted("control_scaled") = SpecTargeted("control_scaled")

BugBackoffZeroReturnsZero ==
  ActualBackoff("backoff_zero") = SpecBackoff("backoff_zero")

BugBackoffSkipsFloor ==
  ActualBackoff("backoff_floor") = SpecBackoff("backoff_floor")

BugBackoffSkipsCeiling ==
  ActualBackoff("backoff_ceiling") = SpecBackoff("backoff_ceiling")

BugBackoffUsesTimeout ==
  ActualBackoff("backoff_scaled") = SpecBackoff("backoff_scaled")

BugCommitZeroCommitReturnsZero ==
  ActualCommitTimeout("commit_zero_block_zero") =
    SpecCommitTimeout("commit_zero_block_zero")

BugCommitNoDaUsesMin ==
  ActualCommitTimeout("commit_no_da_commit_greater") =
    SpecCommitTimeout("commit_no_da_commit_greater")

BugCommitDaSkipsCommitFactor ==
  ActualCommitTimeout("commit_da_multiplier") =
    SpecCommitTimeout("commit_da_multiplier")

BugCommitDaSkipsMultiplier ==
  ActualCommitTimeout("commit_da_multiplier") =
    SpecCommitTimeout("commit_da_multiplier")

BugCommitDaAllowsZeroMultiplier ==
  ActualCommitTimeout("commit_da_multiplier_min_one") =
    SpecCommitTimeout("commit_da_multiplier_min_one")

BugCommitOmitsFloor ==
  ActualCommitTimeout("commit_da_floor") = SpecCommitTimeout("commit_da_floor")

BugPacemakerUsesBlockTime ==
  ActualPacemaker("pacemaker_explicit_propose") =
    SpecPacemaker("pacemaker_explicit_propose")

BugPacemakerSkipsRttFloor ==
  ActualPacemaker("pacemaker_rtt_zero") = SpecPacemaker("pacemaker_rtt_zero")

BugPacemakerSkipsCap ==
  ActualPacemaker("pacemaker_capped") = SpecPacemaker("pacemaker_capped")

BugAvailabilityNoDaScales ==
  ActualAvailability("availability_no_da") =
    SpecAvailability("availability_no_da")

BugAvailabilityDaSkipsFloor ==
  ActualAvailability("availability_da_floor") =
    SpecAvailability("availability_da_floor")

BugAvailabilityDaSkipsMultiplier ==
  ActualAvailability("availability_da_above_floor") =
    SpecAvailability("availability_da_above_floor")

BugAvailabilityDaAllowsZeroMultiplier ==
  ActualAvailability("availability_da_multiplier_zero") =
    SpecAvailability("availability_da_multiplier_zero")

BugAvailabilityGateZeroTimeout ==
  ActualAvailabilityGate("availability_gate_zero") =
    SpecAvailabilityGate("availability_gate_zero")

BugAvailabilityGateStrictAge ==
  ActualAvailabilityGate("availability_gate_at") =
    SpecAvailabilityGate("availability_gate_at")

BugMissingQuorumZeroTimeoutStale ==
  ActualMissingQuorum("missing_quorum_zero_timeout") =
    SpecMissingQuorum("missing_quorum_zero_timeout")

BugMissingQuorumIgnoresQuorum ==
  ActualMissingQuorum("missing_quorum_reached") =
    SpecMissingQuorum("missing_quorum_reached")

BugMissingQuorumStrictAge ==
  ActualMissingQuorum("missing_quorum_at") = SpecMissingQuorum("missing_quorum_at")

BugPrevoteAcceptsCommitPhase ==
  ActualPrevote("prevote_commit_after") = SpecPrevote("prevote_commit_after")

BugPrevoteZeroTimeoutStale ==
  ActualPrevote("prevote_prepare_zero_timeout") =
    SpecPrevote("prevote_prepare_zero_timeout")

BugPrevoteStrictAge ==
  ActualPrevote("prevote_prepare_at") = SpecPrevote("prevote_prepare_at")

====
