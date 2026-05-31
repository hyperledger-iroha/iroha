---- MODULE SumeragiKnownBlockCommitQcRecoveryGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for known-block commit-QC recovery helpers in
`main_loop/commit.rs`.

This slice pins `known_block_commit_qc_recovery_request_plan(...)`,
`pending_extends_tip(...)`, and the pending-block admission contract inside
`known_block_commit_qc_recovery_stale_view_cert_fetch_allowed(...)`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PlanCases == {
  "plan_local_payload",
  "plan_missing_payload"
}

ExtendsCases == {
  "extends_ok",
  "extends_same_height",
  "extends_parent_mismatch",
  "extends_missing_tip"
}

FetchCases == {
  "fetch_ok_override",
  "fetch_ok_map",
  "fetch_bad_override_map_ok",
  "fetch_ok_override_map_bad",
  "fetch_wrong_height",
  "fetch_hash_mismatch",
  "fetch_height_mismatch",
  "fetch_view_mismatch",
  "fetch_invalid",
  "fetch_inactive",
  "fetch_no_local_vote",
  "fetch_parent_mismatch",
  "fetch_missing_tip",
  "fetch_no_pending"
}

BoolToInt(b) == IF b THEN 1 ELSE 0

PayloadMaterializedLocally(c) ==
  c = "plan_local_payload"

SpecPlanCommitQcOnly(c) ==
  PayloadMaterializedLocally(c)

SpecPlanBody(c) ==
  ~PayloadMaterializedLocally(c)

ActualPlanCommitQcOnly(c) ==
  CASE Bug = "plan_missing_requests_qc_only"
       /\ c = "plan_missing_payload" -> TRUE
    [] Bug = "plan_requests_both" /\ c = "plan_missing_payload" -> TRUE
    [] Bug = "plan_requests_neither" /\ c = "plan_local_payload" -> FALSE
    [] OTHER -> SpecPlanCommitQcOnly(c)

ActualPlanBody(c) ==
  CASE Bug = "plan_local_requests_body"
       /\ c = "plan_local_payload" -> TRUE
    [] Bug = "plan_requests_both" /\ c = "plan_missing_payload" -> TRUE
    [] Bug = "plan_requests_neither" /\ c = "plan_local_payload" -> FALSE
    [] OTHER -> SpecPlanBody(c)

\* @type: (Str) => <<Int, Int>>;
SpecPlanOutput(c) ==
  <<BoolToInt(SpecPlanCommitQcOnly(c)), BoolToInt(SpecPlanBody(c))>>

\* @type: (Str) => <<Int, Int>>;
ActualPlanOutput(c) ==
  <<BoolToInt(ActualPlanCommitQcOnly(c)), BoolToInt(ActualPlanBody(c))>>

ExtendsPendingHeight(c) ==
  IF c = "extends_same_height" THEN 10 ELSE 11

ExtendsStateHeight(c) == 10

ExtendsParentMatchesTip(c) ==
  c # "extends_parent_mismatch"

ExtendsTipPresent(c) ==
  c # "extends_missing_tip"

SpecExtendsTip(c) ==
  ExtendsPendingHeight(c) = ExtendsStateHeight(c) + 1
    /\ ExtendsTipPresent(c)
    /\ ExtendsParentMatchesTip(c)

ActualExtendsTip(c) ==
  CASE Bug = "extends_allows_same_height"
       /\ c = "extends_same_height" -> TRUE
    [] Bug = "extends_ignores_parent"
       /\ c = "extends_parent_mismatch" -> TRUE
    [] Bug = "extends_allows_missing_tip"
       /\ c = "extends_missing_tip" -> TRUE
    [] OTHER -> SpecExtendsTip(c)

SpecExtendsOutput(c) ==
  BoolToInt(SpecExtendsTip(c))

ActualExtendsOutput(c) ==
  BoolToInt(ActualExtendsTip(c))

FetchRequestHeight(c) ==
  IF c = "fetch_wrong_height" THEN 12 ELSE 11

CommittedHeight(c) == 10

TopHeightMatches(c) ==
  FetchRequestHeight(c) = CommittedHeight(c) + 1

OverridePresent(c) ==
  c \in {
    "fetch_ok_override",
    "fetch_bad_override_map_ok",
    "fetch_ok_override_map_bad"
  }

MapPresent(c) ==
  c \notin {"fetch_ok_override", "fetch_no_pending"}

SourcePresent(c, source) ==
  IF source = "override" THEN OverridePresent(c) ELSE MapPresent(c)

SourceHashMatches(c, source) ==
  /\ ~(c = "fetch_bad_override_map_ok" /\ source = "override")
  /\ ~(c = "fetch_ok_override_map_bad" /\ source = "map")
  /\ c # "fetch_hash_mismatch"

SourceHeightMatches(c, source) ==
  /\ c # "fetch_height_mismatch"
  /\ FetchRequestHeight(c) = CommittedHeight(c) + 1

SourceViewMatches(c, source) ==
  c # "fetch_view_mismatch"

SourceValidationOk(c, source) ==
  c # "fetch_invalid"

SourceConsensusActive(c, source) ==
  c # "fetch_inactive"

SourceLocalCommitVote(c, source) ==
  c # "fetch_no_local_vote"

SourceParentMatchesTip(c, source) ==
  c # "fetch_parent_mismatch"

SourceTipPresent(c, source) ==
  c # "fetch_missing_tip"

SourceExtendsTip(c, source) ==
  SourceHeightMatches(c, source)
    /\ SourceTipPresent(c, source)
    /\ SourceParentMatchesTip(c, source)

SpecPendingAllows(c, source) ==
  SourcePresent(c, source)
    /\ SourceHashMatches(c, source)
    /\ SourceHeightMatches(c, source)
    /\ SourceViewMatches(c, source)
    /\ SourceValidationOk(c, source)
    /\ SourceConsensusActive(c, source)
    /\ SourceLocalCommitVote(c, source)
    /\ SourceExtendsTip(c, source)

SpecFetchAllowed(c) ==
  TopHeightMatches(c)
    /\ (SpecPendingAllows(c, "override") \/ SpecPendingAllows(c, "map"))

ActualFetchAllowed(c) ==
  CASE Bug = "fetch_allows_wrong_height" /\ c = "fetch_wrong_height" -> TRUE
    [] Bug = "fetch_accepts_hash_mismatch" /\ c = "fetch_hash_mismatch" -> TRUE
    [] Bug = "fetch_accepts_height_mismatch" /\ c = "fetch_height_mismatch" -> TRUE
    [] Bug = "fetch_accepts_view_mismatch" /\ c = "fetch_view_mismatch" -> TRUE
    [] Bug = "fetch_accepts_invalid" /\ c = "fetch_invalid" -> TRUE
    [] Bug = "fetch_accepts_inactive" /\ c = "fetch_inactive" -> TRUE
    [] Bug = "fetch_accepts_without_local_vote"
       /\ c = "fetch_no_local_vote" -> TRUE
    [] Bug = "fetch_accepts_parent_mismatch"
       /\ c = "fetch_parent_mismatch" -> TRUE
    [] Bug = "fetch_accepts_missing_tip" /\ c = "fetch_missing_tip" -> TRUE
    [] Bug = "fetch_ignores_override" /\ c = "fetch_ok_override" -> FALSE
    [] Bug = "fetch_ignores_map" /\ c = "fetch_ok_map" -> FALSE
    [] Bug = "fetch_bad_override_blocks_map"
       /\ c = "fetch_bad_override_map_ok" -> FALSE
    [] Bug = "fetch_ok_override_requires_map"
       /\ c = "fetch_ok_override_map_bad" -> FALSE
    [] OTHER -> SpecFetchAllowed(c)

SpecFetchOutput(c) ==
  BoolToInt(SpecFetchAllowed(c))

ActualFetchOutput(c) ==
  BoolToInt(ActualFetchAllowed(c))

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "plan_local_requests_body",
       "plan_missing_requests_qc_only",
       "plan_requests_both",
       "plan_requests_neither",
       "extends_allows_same_height",
       "extends_ignores_parent",
       "extends_allows_missing_tip",
       "fetch_allows_wrong_height",
       "fetch_accepts_hash_mismatch",
       "fetch_accepts_height_mismatch",
       "fetch_accepts_view_mismatch",
       "fetch_accepts_invalid",
       "fetch_accepts_inactive",
       "fetch_accepts_without_local_vote",
       "fetch_accepts_parent_mismatch",
       "fetch_accepts_missing_tip",
       "fetch_ignores_override",
       "fetch_ignores_map",
       "fetch_bad_override_blocks_map",
       "fetch_ok_override_requires_map"
     }
  /\ checked = 0

SafetyFast ==
  /\ \A c \in PlanCases: ActualPlanOutput(c) = SpecPlanOutput(c)
  /\ \A c \in ExtendsCases: ActualExtendsOutput(c) = SpecExtendsOutput(c)
  /\ \A c \in FetchCases: ActualFetchOutput(c) = SpecFetchOutput(c)

BugPlanLocalRequestsBody ==
  ActualPlanOutput("plan_local_payload") = SpecPlanOutput("plan_local_payload")

BugPlanMissingRequestsQcOnly ==
  ActualPlanOutput("plan_missing_payload") =
    SpecPlanOutput("plan_missing_payload")

BugPlanRequestsBoth ==
  ActualPlanOutput("plan_missing_payload") =
    SpecPlanOutput("plan_missing_payload")

BugPlanRequestsNeither ==
  ActualPlanOutput("plan_local_payload") = SpecPlanOutput("plan_local_payload")

BugExtendsAllowsSameHeight ==
  ActualExtendsOutput("extends_same_height") =
    SpecExtendsOutput("extends_same_height")

BugExtendsIgnoresParent ==
  ActualExtendsOutput("extends_parent_mismatch") =
    SpecExtendsOutput("extends_parent_mismatch")

BugExtendsAllowsMissingTip ==
  ActualExtendsOutput("extends_missing_tip") =
    SpecExtendsOutput("extends_missing_tip")

BugFetchAllowsWrongHeight ==
  ActualFetchOutput("fetch_wrong_height") = SpecFetchOutput("fetch_wrong_height")

BugFetchAcceptsHashMismatch ==
  ActualFetchOutput("fetch_hash_mismatch") =
    SpecFetchOutput("fetch_hash_mismatch")

BugFetchAcceptsHeightMismatch ==
  ActualFetchOutput("fetch_height_mismatch") =
    SpecFetchOutput("fetch_height_mismatch")

BugFetchAcceptsViewMismatch ==
  ActualFetchOutput("fetch_view_mismatch") =
    SpecFetchOutput("fetch_view_mismatch")

BugFetchAcceptsInvalid ==
  ActualFetchOutput("fetch_invalid") = SpecFetchOutput("fetch_invalid")

BugFetchAcceptsInactive ==
  ActualFetchOutput("fetch_inactive") = SpecFetchOutput("fetch_inactive")

BugFetchAcceptsWithoutLocalVote ==
  ActualFetchOutput("fetch_no_local_vote") =
    SpecFetchOutput("fetch_no_local_vote")

BugFetchAcceptsParentMismatch ==
  ActualFetchOutput("fetch_parent_mismatch") =
    SpecFetchOutput("fetch_parent_mismatch")

BugFetchAcceptsMissingTip ==
  ActualFetchOutput("fetch_missing_tip") = SpecFetchOutput("fetch_missing_tip")

BugFetchIgnoresOverride ==
  ActualFetchOutput("fetch_ok_override") = SpecFetchOutput("fetch_ok_override")

BugFetchIgnoresMap ==
  ActualFetchOutput("fetch_ok_map") = SpecFetchOutput("fetch_ok_map")

BugFetchBadOverrideBlocksMap ==
  ActualFetchOutput("fetch_bad_override_map_ok") =
    SpecFetchOutput("fetch_bad_override_map_ok")

BugFetchOkOverrideRequiresMap ==
  ActualFetchOutput("fetch_ok_override_map_bad") =
    SpecFetchOutput("fetch_ok_override_map_bad")

====
