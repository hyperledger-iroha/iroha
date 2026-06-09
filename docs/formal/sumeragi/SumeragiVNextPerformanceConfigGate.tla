---- MODULE SumeragiVNextPerformanceConfigGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for vNext performance-fault configuration helpers in
`sumeragi/vnext.rs`.

This slice pins `duration_millis(...)` and `PerformanceFaultConfig::from(...)`.
Durations are represented by bounded integer millisecond counts, with `MaxU64`
standing in for `u64::MAX`. The model preserves the helper contract: exact
millisecond values up to the bound are preserved, oversized values saturate,
non-duration config fields are copied unchanged, and suspicion-timeout and
re-chain-cooldown durations are converted independently.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

MaxU64 == 10

DurationCases == {
  "duration_zero",
  "duration_small",
  "duration_max",
  "duration_overflow"
}

BugSet == {
  "none",
  "duration_overflow_wraps",
  "duration_max_rejected",
  "duration_small_saturates",
  "config_window_defaulted",
  "config_suspicion_uses_cooldown",
  "config_suspicion_overflow_wraps",
  "config_threshold_defaulted",
  "config_max_tainted_defaulted",
  "config_cooldown_uses_suspicion",
  "config_cooldown_overflow_wraps"
}

DurationRaw(c) ==
  CASE c = "duration_zero" -> 0
    [] c = "duration_small" -> 3
    [] c = "duration_max" -> MaxU64
    [] c = "duration_overflow" -> MaxU64 + 2
    [] OTHER -> 0

SpecDurationMillisRaw(raw) ==
  IF raw <= MaxU64 THEN raw ELSE MaxU64

SpecDurationMillis(c) ==
  SpecDurationMillisRaw(DurationRaw(c))

ActualDurationMillis(c) ==
  CASE Bug = "duration_overflow_wraps"
       /\ c = "duration_overflow" -> 0
    [] Bug = "duration_max_rejected"
       /\ c = "duration_max" -> MaxU64 - 1
    [] Bug = "duration_small_saturates"
       /\ c = "duration_small" -> MaxU64
    [] OTHER -> SpecDurationMillis(c)

ConfigCases == {
  "config_normal",
  "config_zero_durations",
  "config_overflow_suspicion",
  "config_overflow_cooldown",
  "config_independent_fields"
}

InputWindow(c) ==
  CASE c = "config_normal" -> 9
    [] c = "config_zero_durations" -> 2
    [] c = "config_overflow_suspicion" -> 3
    [] c = "config_overflow_cooldown" -> 4
    [] c = "config_independent_fields" -> 6
    [] OTHER -> 0

InputSuspicionRaw(c) ==
  CASE c = "config_normal" -> 4
    [] c = "config_zero_durations" -> 0
    [] c = "config_overflow_suspicion" -> MaxU64 + 5
    [] c = "config_overflow_cooldown" -> 2
    [] c = "config_independent_fields" -> 8
    [] OTHER -> 0

InputThreshold(c) ==
  CASE c = "config_normal" -> 1234
    [] c = "config_zero_durations" -> 500
    [] c = "config_overflow_suspicion" -> 700
    [] c = "config_overflow_cooldown" -> 900
    [] c = "config_independent_fields" -> 321
    [] OTHER -> 0

InputMaxTainted(c) ==
  CASE c = "config_normal" -> 3
    [] c = "config_zero_durations" -> 1
    [] c = "config_overflow_suspicion" -> 5
    [] c = "config_overflow_cooldown" -> 7
    [] c = "config_independent_fields" -> 4
    [] OTHER -> 0

InputCooldownRaw(c) ==
  CASE c = "config_normal" -> 7
    [] c = "config_zero_durations" -> 0
    [] c = "config_overflow_suspicion" -> 1
    [] c = "config_overflow_cooldown" -> MaxU64 + 3
    [] c = "config_independent_fields" -> 8
    [] OTHER -> 0

SpecWindow(c) == InputWindow(c)
SpecSuspicionMs(c) == SpecDurationMillisRaw(InputSuspicionRaw(c))
SpecThreshold(c) == InputThreshold(c)
SpecMaxTainted(c) == InputMaxTainted(c)
SpecCooldownMs(c) == SpecDurationMillisRaw(InputCooldownRaw(c))

ActualWindow(c) ==
  CASE Bug = "config_window_defaulted"
       /\ c = "config_normal" -> 0
    [] OTHER -> SpecWindow(c)

ActualSuspicionMs(c) ==
  CASE Bug = "config_suspicion_uses_cooldown"
       /\ c = "config_normal" -> SpecDurationMillisRaw(InputCooldownRaw(c))
    [] Bug = "config_suspicion_overflow_wraps"
       /\ c = "config_overflow_suspicion" -> 0
    [] OTHER -> SpecSuspicionMs(c)

ActualThreshold(c) ==
  CASE Bug = "config_threshold_defaulted"
       /\ c = "config_normal" -> 0
    [] OTHER -> SpecThreshold(c)

ActualMaxTainted(c) ==
  CASE Bug = "config_max_tainted_defaulted"
       /\ c = "config_normal" -> 0
    [] OTHER -> SpecMaxTainted(c)

ActualCooldownMs(c) ==
  CASE Bug = "config_cooldown_uses_suspicion"
       /\ c = "config_normal" -> SpecDurationMillisRaw(InputSuspicionRaw(c))
    [] Bug = "config_cooldown_overflow_wraps"
       /\ c = "config_overflow_cooldown" -> 0
    [] OTHER -> SpecCooldownMs(c)

Init == checked = 0
Next == UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked \in 0..1

DurationBounds ==
  \A c \in DurationCases:
    /\ ActualDurationMillis(c) >= 0
    /\ ActualDurationMillis(c) <= MaxU64

DurationSaturationAnchors ==
  /\ ActualDurationMillis("duration_zero") = 0
  /\ ActualDurationMillis("duration_small") = DurationRaw("duration_small")
  /\ ActualDurationMillis("duration_max") = MaxU64
  /\ ActualDurationMillis("duration_overflow") = MaxU64

ConfigFieldPreservation ==
  \A c \in ConfigCases:
    /\ ActualWindow(c) = InputWindow(c)
    /\ ActualThreshold(c) = InputThreshold(c)
    /\ ActualMaxTainted(c) = InputMaxTainted(c)

ConfigDurationConversion ==
  \A c \in ConfigCases:
    /\ ActualSuspicionMs(c) = SpecDurationMillisRaw(InputSuspicionRaw(c))
    /\ ActualCooldownMs(c) = SpecDurationMillisRaw(InputCooldownRaw(c))

ConfigDurationIndependence ==
  /\ ActualSuspicionMs("config_normal") # ActualCooldownMs("config_normal")
  /\ ActualSuspicionMs("config_independent_fields")
     = ActualCooldownMs("config_independent_fields")
  /\ InputSuspicionRaw("config_normal") # InputCooldownRaw("config_normal")
  /\ InputSuspicionRaw("config_independent_fields")
     = InputCooldownRaw("config_independent_fields")

ConfigOverflowSaturationAnchors ==
  /\ ActualSuspicionMs("config_overflow_suspicion") = MaxU64
  /\ ActualCooldownMs("config_overflow_cooldown") = MaxU64
  /\ ActualCooldownMs("config_overflow_suspicion") =
     InputCooldownRaw("config_overflow_suspicion")
  /\ ActualSuspicionMs("config_overflow_cooldown") =
     InputSuspicionRaw("config_overflow_cooldown")

VNextPerformanceConfigCoreSafety ==
  /\ \A c \in DurationCases:
       ActualDurationMillis(c) = SpecDurationMillis(c)
  /\ \A c \in ConfigCases:
       ActualWindow(c) = SpecWindow(c)
  /\ \A c \in ConfigCases:
       ActualSuspicionMs(c) = SpecSuspicionMs(c)
  /\ \A c \in ConfigCases:
       ActualThreshold(c) = SpecThreshold(c)
  /\ \A c \in ConfigCases:
       ActualMaxTainted(c) = SpecMaxTainted(c)
  /\ \A c \in ConfigCases:
       ActualCooldownMs(c) = SpecCooldownMs(c)
  /\ ActualDurationMillis("duration_overflow") = MaxU64
  /\ ActualSuspicionMs("config_overflow_suspicion") = MaxU64
  /\ ActualCooldownMs("config_overflow_cooldown") = MaxU64
  /\ DurationBounds
  /\ DurationSaturationAnchors
  /\ ConfigFieldPreservation
  /\ ConfigDurationConversion
  /\ ConfigDurationIndependence
  /\ ConfigOverflowSaturationAnchors

SafetyFast ==
  VNextPerformanceConfigCoreSafety

BugDurationOverflowWraps ==
  ActualDurationMillis("duration_overflow") =
    SpecDurationMillis("duration_overflow")

BugDurationMaxRejected ==
  ActualDurationMillis("duration_max") = SpecDurationMillis("duration_max")

BugDurationSmallSaturates ==
  ActualDurationMillis("duration_small") = SpecDurationMillis("duration_small")

BugConfigWindowDefaulted ==
  ActualWindow("config_normal") = SpecWindow("config_normal")

BugConfigThresholdDefaulted ==
  ActualThreshold("config_normal") = SpecThreshold("config_normal")

BugConfigMaxTaintedDefaulted ==
  ActualMaxTainted("config_normal") = SpecMaxTainted("config_normal")

BugConfigSuspicionUsesCooldown ==
  ActualSuspicionMs("config_normal") = SpecSuspicionMs("config_normal")

BugConfigCooldownUsesSuspicion ==
  ActualCooldownMs("config_normal") = SpecCooldownMs("config_normal")

BugConfigSuspicionOverflowWraps ==
  ActualSuspicionMs("config_overflow_suspicion") =
    SpecSuspicionMs("config_overflow_suspicion")

BugConfigCooldownOverflowWraps ==
  ActualCooldownMs("config_overflow_cooldown") =
    SpecCooldownMs("config_overflow_cooldown")
====
