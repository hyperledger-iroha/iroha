---- MODULE SumeragiNonRbcPayloadBudgetGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for non-RBC payload budget derivation.

This slice pins `non_rbc_payload_budget(...)`: the actor first subtracts the
fixed non-RBC frame headroom from the consensus payload frame cap with
saturating arithmetic, then uses the explicit configured block-payload cap when
present, and finally returns the lower of the configured cap and the adjusted
frame cap.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Headroom == 8192

Cases == {
  "unset_zero_frame",
  "unset_below_headroom",
  "unset_equal_headroom",
  "unset_above_headroom",
  "large_frame_no_config",
  "config_below_cap",
  "config_equal_cap",
  "config_above_cap",
  "config_with_small_frame"
}

Min(a, b) == IF a <= b THEN a ELSE b
SaturatingSub(a, b) == IF a <= b THEN 0 ELSE a - b

\* @type: Str => Int;
PayloadFrameCap(c) ==
  CASE c = "unset_zero_frame" -> 0
    [] c = "unset_below_headroom" -> 1024
    [] c = "unset_equal_headroom" -> Headroom
    [] c = "unset_above_headroom" -> 9000
    [] c = "large_frame_no_config" -> 20000
    [] c = "config_below_cap" -> 10000
    [] c = "config_equal_cap" -> 10000
    [] c = "config_above_cap" -> 10000
    [] c = "config_with_small_frame" -> 1024
    [] OTHER -> 0

\* Zero means the optional `block_max_payload_bytes` config is absent.
\* @type: Str => Int;
ConfigCap(c) ==
  CASE c = "config_below_cap" -> 500
    [] c = "config_equal_cap" -> 1808
    [] c = "config_above_cap" -> 5000
    [] c = "config_with_small_frame" -> 500
    [] OTHER -> 0

\* @type: Str => Int;
SpecFrameCap(c) == SaturatingSub(PayloadFrameCap(c), Headroom)

\* @type: Str => Int;
SpecBudget(c) ==
  LET frameCap == SpecFrameCap(c)
      configCap == ConfigCap(c)
  IN IF configCap = 0 THEN frameCap ELSE Min(configCap, frameCap)

\* @type: Str => Int;
ActualBudget(c) ==
  CASE Bug = "underflow_below_headroom"
       /\ c = "unset_below_headroom" -> Headroom - PayloadFrameCap(c)
    [] Bug = "equal_headroom_allows_one"
       /\ c = "unset_equal_headroom" -> 1
    [] Bug = "unset_ignores_headroom"
       /\ c = "unset_above_headroom" -> PayloadFrameCap(c)
    [] Bug = "large_frame_ignores_headroom"
       /\ c = "large_frame_no_config" -> PayloadFrameCap(c)
    [] Bug = "config_below_ignored"
       /\ c = "config_below_cap" -> SpecFrameCap(c)
    [] Bug = "config_equal_dropped"
       /\ c = "config_equal_cap" -> 0
    [] Bug = "config_above_not_clamped"
       /\ c = "config_above_cap" -> ConfigCap(c)
    [] Bug = "min_uses_max"
       /\ c = "config_below_cap" -> SpecFrameCap(c)
    [] Bug = "zero_frame_allows_config"
       /\ c = "config_with_small_frame" -> ConfigCap(c)
    [] OTHER -> SpecBudget(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "underflow_below_headroom",
       "equal_headroom_allows_one",
       "unset_ignores_headroom",
       "large_frame_ignores_headroom",
       "config_below_ignored",
       "config_equal_dropped",
       "config_above_not_clamped",
       "min_uses_max",
       "zero_frame_allows_config"
     }
  /\ checked = 0

NonRbcPayloadBudgetMatchesSpec ==
  /\ \A c \in Cases:
       ActualBudget(c) = SpecBudget(c)
  /\ \A c \in Cases:
       SpecFrameCap(c) >= 0
  /\ \A c \in Cases:
       SpecBudget(c) <= SpecFrameCap(c)

NonRbcPayloadBudgetExactness ==
  /\ NonRbcPayloadBudgetMatchesSpec
NonRbcPayloadBudgetCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ NonRbcPayloadBudgetExactness

SafetyFast ==
  NonRbcPayloadBudgetExactness

BugUnderflowBelowHeadroom ==
  ActualBudget("unset_below_headroom") = SpecBudget("unset_below_headroom")

BugEqualHeadroomAllowsOne ==
  ActualBudget("unset_equal_headroom") = SpecBudget("unset_equal_headroom")

BugUnsetIgnoresHeadroom ==
  ActualBudget("unset_above_headroom") = SpecBudget("unset_above_headroom")

BugLargeFrameIgnoresHeadroom ==
  ActualBudget("large_frame_no_config") = SpecBudget("large_frame_no_config")

BugConfigBelowIgnored ==
  ActualBudget("config_below_cap") = SpecBudget("config_below_cap")

BugConfigEqualDropped ==
  ActualBudget("config_equal_cap") = SpecBudget("config_equal_cap")

BugConfigAboveNotClamped ==
  ActualBudget("config_above_cap") = SpecBudget("config_above_cap")

BugMinUsesMax ==
  ActualBudget("config_below_cap") = SpecBudget("config_below_cap")

BugZeroFrameAllowsConfig ==
  ActualBudget("config_with_small_frame") = SpecBudget("config_with_small_frame")

====
