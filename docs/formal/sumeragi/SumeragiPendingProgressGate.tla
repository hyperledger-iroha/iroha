---- MODULE SumeragiPendingProgressGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for pending-progress accounting helpers.

This slice pins `touch_pending_progress(...)`,
`refresh_pending_activation_window(...)`,
`refresh_tip_activated_pending_progress(...)`, and
`has_recent_pending_progress_for_rbc(...)`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

OwnerCases == {
  "map_match",
  "map_hash_mismatch",
  "map_height_mismatch",
  "map_view_mismatch",
  "map_aborted",
  "inflight_match",
  "inflight_hash_mismatch",
  "inflight_height_mismatch",
  "inflight_view_mismatch",
  "inflight_aborted",
  "both_match",
  "no_owner"
}

ActivationCases == {
  "activation_extends",
  "activation_aborted",
  "activation_same_height",
  "activation_future_height",
  "activation_parent_mismatch",
  "activation_inflight_extends"
}

RecentCases == {
  "recent_zero_window",
  "recent_map_lower_bound",
  "recent_map_upper_bound",
  "recent_map_at_window",
  "recent_map_below_lower",
  "recent_map_above_upper",
  "recent_map_stale",
  "recent_map_aborted",
  "recent_inflight_recent",
  "recent_none"
}

BoolToInt(b) == IF b THEN 1 ELSE 0

SpecMapOwnerTouched(c) ==
  c \in {"map_match", "both_match"}

SpecInflightOwnerTouched(c) ==
  c \in {"inflight_match", "both_match"}

ActualMapOwnerTouched(c) ==
  CASE Bug = "touch_skips_map_match" /\ c = "map_match" -> FALSE
    [] Bug = "touch_ignores_map_hash" /\ c = "map_hash_mismatch" -> TRUE
    [] Bug = "touch_ignores_map_height" /\ c = "map_height_mismatch" -> TRUE
    [] Bug = "touch_touches_map_aborted" /\ c = "map_aborted" -> TRUE
    [] OTHER -> SpecMapOwnerTouched(c)

ActualInflightOwnerTouched(c) ==
  CASE Bug = "touch_skips_inflight_match" /\ c = "inflight_match" -> FALSE
    [] Bug = "touch_touches_inflight_aborted"
       /\ c = "inflight_aborted" -> TRUE
    [] OTHER -> SpecInflightOwnerTouched(c)

\* @type: (Str) => <<Int, Int>>;
SpecTouchOutput(c) ==
  <<BoolToInt(SpecMapOwnerTouched(c)),
    BoolToInt(SpecInflightOwnerTouched(c))>>

\* @type: (Str) => <<Int, Int>>;
ActualTouchOutput(c) ==
  <<BoolToInt(ActualMapOwnerTouched(c)),
    BoolToInt(ActualInflightOwnerTouched(c))>>

\* @type: (Str) => <<Int, Int, Int, Int>>;
SpecRefreshMapFields(c) ==
  IF SpecMapOwnerTouched(c)
  THEN <<1, 1, 1, 1>>
  ELSE <<0, 0, 0, 0>>

\* @type: (Str) => <<Int, Int, Int, Int>>;
SpecRefreshInflightFields(c) ==
  IF SpecInflightOwnerTouched(c)
  THEN <<1, 1, 1, 1>>
  ELSE <<0, 0, 0, 0>>

\* @type: (Str) => <<Int, Int, Int, Int>>;
ActualRefreshMapFields(c) ==
  CASE Bug = "refresh_skips_map_match" /\ c = "map_match" -> <<0, 0, 0, 0>>
    [] Bug = "refresh_keeps_map_inserted" /\ c = "map_match" ->
       <<0, 1, 1, 1>>
    [] Bug = "refresh_keeps_map_progress" /\ c = "map_match" ->
       <<1, 0, 1, 1>>
    [] Bug = "refresh_keeps_map_quorum" /\ c = "map_match" ->
       <<1, 1, 0, 1>>
    [] Bug = "refresh_keeps_map_redrive" /\ c = "map_match" ->
       <<1, 1, 1, 0>>
    [] Bug = "refresh_ignores_map_hash" /\ c = "map_hash_mismatch" ->
       <<1, 1, 1, 1>>
    [] OTHER -> SpecRefreshMapFields(c)

\* @type: (Str) => <<Int, Int, Int, Int>>;
ActualRefreshInflightFields(c) ==
  CASE Bug = "refresh_skips_inflight_match" /\ c = "inflight_match" ->
       <<0, 0, 0, 0>>
    [] Bug = "refreshes_inflight_aborted" /\ c = "inflight_aborted" ->
       <<1, 1, 1, 1>>
    [] OTHER -> SpecRefreshInflightFields(c)

\* @type: (Str) => <<Int, Int, Int, Int, Int, Int, Int, Int>>;
SpecRefreshOutput(c) ==
  LET m == SpecRefreshMapFields(c) IN
  LET i == SpecRefreshInflightFields(c) IN
    <<m[1], m[2], m[3], m[4], i[1], i[2], i[3], i[4]>>

\* @type: (Str) => <<Int, Int, Int, Int, Int, Int, Int, Int>>;
ActualRefreshOutput(c) ==
  LET m == ActualRefreshMapFields(c) IN
  LET i == ActualRefreshInflightFields(c) IN
    <<m[1], m[2], m[3], m[4], i[1], i[2], i[3], i[4]>>

SpecActivationRefresh(c) ==
  c = "activation_extends"

ActualActivationRefresh(c) ==
  CASE Bug = "activation_skips_extending"
       /\ c = "activation_extends" -> FALSE
    [] Bug = "activation_refreshes_aborted"
       /\ c = "activation_aborted" -> TRUE
    [] Bug = "activation_allows_same_height"
       /\ c = "activation_same_height" -> TRUE
    [] Bug = "activation_allows_future_height"
       /\ c = "activation_future_height" -> TRUE
    [] Bug = "activation_ignores_parent"
       /\ c = "activation_parent_mismatch" -> TRUE
    [] Bug = "activation_refreshes_inflight"
       /\ c = "activation_inflight_extends" -> TRUE
    [] OTHER -> SpecActivationRefresh(c)

SpecActivationOutput(c) ==
  BoolToInt(SpecActivationRefresh(c))

ActualActivationOutput(c) ==
  BoolToInt(ActualActivationRefresh(c))

SpecRecentProgress(c) ==
  c \in {
    "recent_map_lower_bound",
    "recent_map_upper_bound",
    "recent_map_at_window",
    "recent_inflight_recent"
  }

ActualRecentProgress(c) ==
  CASE Bug = "recent_allows_zero_window" /\ c = "recent_zero_window" -> TRUE
    [] Bug = "recent_skips_lower_bound" /\ c = "recent_map_lower_bound" -> FALSE
    [] Bug = "recent_skips_upper_bound" /\ c = "recent_map_upper_bound" -> FALSE
    [] Bug = "recent_excludes_equal_window" /\ c = "recent_map_at_window" -> FALSE
    [] Bug = "recent_allows_below_lower" /\ c = "recent_map_below_lower" -> TRUE
    [] Bug = "recent_allows_stale" /\ c = "recent_map_stale" -> TRUE
    [] Bug = "recent_ignores_inflight" /\ c = "recent_inflight_recent" -> FALSE
    [] OTHER -> SpecRecentProgress(c)

SpecRecentOutput(c) ==
  BoolToInt(SpecRecentProgress(c))

ActualRecentOutput(c) ==
  BoolToInt(ActualRecentProgress(c))

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "touch_skips_map_match",
       "touch_skips_inflight_match",
       "touch_ignores_map_hash",
       "touch_ignores_map_height",
       "touch_touches_map_aborted",
       "touch_touches_inflight_aborted",
       "refresh_skips_map_match",
       "refresh_keeps_map_inserted",
       "refresh_keeps_map_progress",
       "refresh_keeps_map_quorum",
       "refresh_keeps_map_redrive",
       "refresh_ignores_map_hash",
       "refresh_skips_inflight_match",
       "refreshes_inflight_aborted",
       "activation_skips_extending",
       "activation_refreshes_aborted",
       "activation_allows_same_height",
       "activation_allows_future_height",
       "activation_ignores_parent",
       "activation_refreshes_inflight",
       "recent_allows_zero_window",
       "recent_skips_lower_bound",
       "recent_skips_upper_bound",
       "recent_excludes_equal_window",
       "recent_allows_below_lower",
       "recent_allows_stale",
       "recent_ignores_inflight"
     }
  /\ checked = 0

SafetyFast ==
  /\ \A c \in OwnerCases: ActualTouchOutput(c) = SpecTouchOutput(c)
  /\ \A c \in OwnerCases: ActualRefreshOutput(c) = SpecRefreshOutput(c)
  /\ \A c \in ActivationCases:
       ActualActivationOutput(c) = SpecActivationOutput(c)
  /\ \A c \in RecentCases: ActualRecentOutput(c) = SpecRecentOutput(c)

BugTouchSkipsMapMatch ==
  ActualTouchOutput("map_match") = SpecTouchOutput("map_match")

BugTouchSkipsInflightMatch ==
  ActualTouchOutput("inflight_match") = SpecTouchOutput("inflight_match")

BugTouchIgnoresMapHash ==
  ActualTouchOutput("map_hash_mismatch") = SpecTouchOutput("map_hash_mismatch")

BugTouchIgnoresMapHeight ==
  ActualTouchOutput("map_height_mismatch") =
    SpecTouchOutput("map_height_mismatch")

BugTouchTouchesMapAborted ==
  ActualTouchOutput("map_aborted") = SpecTouchOutput("map_aborted")

BugTouchTouchesInflightAborted ==
  ActualTouchOutput("inflight_aborted") = SpecTouchOutput("inflight_aborted")

BugRefreshSkipsMapMatch ==
  ActualRefreshOutput("map_match") = SpecRefreshOutput("map_match")

BugRefreshKeepsMapInserted ==
  ActualRefreshOutput("map_match") = SpecRefreshOutput("map_match")

BugRefreshKeepsMapProgress ==
  ActualRefreshOutput("map_match") = SpecRefreshOutput("map_match")

BugRefreshKeepsMapQuorum ==
  ActualRefreshOutput("map_match") = SpecRefreshOutput("map_match")

BugRefreshKeepsMapRedrive ==
  ActualRefreshOutput("map_match") = SpecRefreshOutput("map_match")

BugRefreshIgnoresMapHash ==
  ActualRefreshOutput("map_hash_mismatch") =
    SpecRefreshOutput("map_hash_mismatch")

BugRefreshSkipsInflightMatch ==
  ActualRefreshOutput("inflight_match") = SpecRefreshOutput("inflight_match")

BugRefreshesInflightAborted ==
  ActualRefreshOutput("inflight_aborted") = SpecRefreshOutput("inflight_aborted")

BugActivationSkipsExtending ==
  ActualActivationOutput("activation_extends") =
    SpecActivationOutput("activation_extends")

BugActivationRefreshesAborted ==
  ActualActivationOutput("activation_aborted") =
    SpecActivationOutput("activation_aborted")

BugActivationAllowsSameHeight ==
  ActualActivationOutput("activation_same_height") =
    SpecActivationOutput("activation_same_height")

BugActivationAllowsFutureHeight ==
  ActualActivationOutput("activation_future_height") =
    SpecActivationOutput("activation_future_height")

BugActivationIgnoresParent ==
  ActualActivationOutput("activation_parent_mismatch") =
    SpecActivationOutput("activation_parent_mismatch")

BugActivationRefreshesInflight ==
  ActualActivationOutput("activation_inflight_extends") =
    SpecActivationOutput("activation_inflight_extends")

BugRecentAllowsZeroWindow ==
  ActualRecentOutput("recent_zero_window") = SpecRecentOutput("recent_zero_window")

BugRecentSkipsLowerBound ==
  ActualRecentOutput("recent_map_lower_bound") =
    SpecRecentOutput("recent_map_lower_bound")

BugRecentSkipsUpperBound ==
  ActualRecentOutput("recent_map_upper_bound") =
    SpecRecentOutput("recent_map_upper_bound")

BugRecentExcludesEqualWindow ==
  ActualRecentOutput("recent_map_at_window") =
    SpecRecentOutput("recent_map_at_window")

BugRecentAllowsBelowLower ==
  ActualRecentOutput("recent_map_below_lower") =
    SpecRecentOutput("recent_map_below_lower")

BugRecentAllowsStale ==
  ActualRecentOutput("recent_map_stale") = SpecRecentOutput("recent_map_stale")

BugRecentIgnoresInflight ==
  ActualRecentOutput("recent_inflight_recent") =
    SpecRecentOutput("recent_inflight_recent")

====
