---- MODULE SumeragiSameHeightBlockBodyRepairGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `allow_same_height_block_body_repair(...)`.

The helper admits a same-height exact block-body response only for the current
frontier height. Once the frontier gate holds, any one of three repair sources
may authorize the payload: a matching pending missing-block request, a matching
deferred missing-payload commit QC, or an active missing commit-QC repair round.
Pending requests and deferred QCs must match commit phase, block hash, height,
view, and their actionable-dependency predicates.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Block == "block"
OtherBlock == "other_block"
Commit == "commit"
Prepare == "prepare"

Cases == {
  "pending_actionable",
  "deferred_qc_actionable",
  "active_commit_repair",
  "multiple_sources",
  "not_frontier_pending_actionable",
  "pending_wrong_phase",
  "pending_hash_mismatch",
  "pending_height_mismatch",
  "pending_view_mismatch",
  "pending_not_actionable",
  "deferred_wrong_phase",
  "deferred_hash_mismatch",
  "deferred_height_mismatch",
  "deferred_view_mismatch",
  "deferred_not_actionable",
  "no_sources"
}

ResponseHash(c) == Block

ResponseHeight(c) == 4

ResponseView(c) == 1

FrontierSlotExact(c) ==
  c # "not_frontier_pending_actionable"

PendingExistsForResponseHash(c) ==
  c \in {
    "pending_actionable",
    "multiple_sources",
    "not_frontier_pending_actionable",
    "pending_wrong_phase",
    "pending_height_mismatch",
    "pending_view_mismatch",
    "pending_not_actionable"
  }

PendingPhase(c) ==
  IF c = "pending_wrong_phase" THEN Prepare ELSE Commit

PendingHeight(c) ==
  IF c = "pending_height_mismatch" THEN 5 ELSE ResponseHeight(c)

PendingView(c) ==
  IF c = "pending_view_mismatch" THEN 2 ELSE ResponseView(c)

PendingActionable(c) ==
  c # "pending_not_actionable"

PendingSource(c) ==
  /\ PendingExistsForResponseHash(c)
  /\ PendingPhase(c) = Commit
  /\ PendingHeight(c) = ResponseHeight(c)
  /\ PendingView(c) = ResponseView(c)
  /\ PendingActionable(c)

DeferredExists(c) ==
  c \in {
    "deferred_qc_actionable",
    "multiple_sources",
    "deferred_wrong_phase",
    "deferred_hash_mismatch",
    "deferred_height_mismatch",
    "deferred_view_mismatch",
    "deferred_not_actionable"
  }

DeferredPhase(c) ==
  IF c = "deferred_wrong_phase" THEN Prepare ELSE Commit

DeferredHash(c) ==
  IF c = "deferred_hash_mismatch" THEN OtherBlock ELSE ResponseHash(c)

DeferredHeight(c) ==
  IF c = "deferred_height_mismatch" THEN 5 ELSE ResponseHeight(c)

DeferredView(c) ==
  IF c = "deferred_view_mismatch" THEN 2 ELSE ResponseView(c)

DeferredActionable(c) ==
  c # "deferred_not_actionable"

DeferredSource(c) ==
  /\ DeferredExists(c)
  /\ DeferredPhase(c) = Commit
  /\ DeferredHash(c) = ResponseHash(c)
  /\ DeferredHeight(c) = ResponseHeight(c)
  /\ DeferredView(c) = ResponseView(c)
  /\ DeferredActionable(c)

ActiveCommitQcRepair(c) ==
  c \in {"active_commit_repair", "multiple_sources"}

SpecAllow(c) ==
  /\ FrontierSlotExact(c)
  /\ \/ PendingSource(c)
     \/ DeferredSource(c)
     \/ ActiveCommitQcRepair(c)

ActualAllow(c) ==
  CASE Bug = "skip_frontier_gate"
       /\ c = "not_frontier_pending_actionable" -> TRUE
    [] Bug = "pending_source_ignored"
       /\ c = "pending_actionable" -> FALSE
    [] Bug = "deferred_source_ignored"
       /\ c = "deferred_qc_actionable" -> FALSE
    [] Bug = "active_source_ignored"
       /\ c = "active_commit_repair" -> FALSE
    [] Bug = "pending_wrong_phase_allowed"
       /\ c = "pending_wrong_phase" -> TRUE
    [] Bug = "pending_hash_mismatch_allowed"
       /\ c = "pending_hash_mismatch" -> TRUE
    [] Bug = "pending_height_mismatch_allowed"
       /\ c = "pending_height_mismatch" -> TRUE
    [] Bug = "pending_view_mismatch_allowed"
       /\ c = "pending_view_mismatch" -> TRUE
    [] Bug = "pending_not_actionable_allowed"
       /\ c = "pending_not_actionable" -> TRUE
    [] Bug = "deferred_wrong_phase_allowed"
       /\ c = "deferred_wrong_phase" -> TRUE
    [] Bug = "deferred_hash_mismatch_allowed"
       /\ c = "deferred_hash_mismatch" -> TRUE
    [] Bug = "deferred_height_mismatch_allowed"
       /\ c = "deferred_height_mismatch" -> TRUE
    [] Bug = "deferred_view_mismatch_allowed"
       /\ c = "deferred_view_mismatch" -> TRUE
    [] Bug = "deferred_not_actionable_allowed"
       /\ c = "deferred_not_actionable" -> TRUE
    [] Bug = "no_source_allowed"
       /\ c = "no_sources" -> TRUE
    [] OTHER -> SpecAllow(c)

Matches(c) ==
  ActualAllow(c) = SpecAllow(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "skip_frontier_gate",
       "pending_source_ignored",
       "deferred_source_ignored",
       "active_source_ignored",
       "pending_wrong_phase_allowed",
       "pending_hash_mismatch_allowed",
       "pending_height_mismatch_allowed",
       "pending_view_mismatch_allowed",
       "pending_not_actionable_allowed",
       "deferred_wrong_phase_allowed",
       "deferred_hash_mismatch_allowed",
       "deferred_height_mismatch_allowed",
       "deferred_view_mismatch_allowed",
       "deferred_not_actionable_allowed",
       "no_source_allowed"
     }
  /\ checked = 0

SafetyFast ==
  \A c \in Cases: Matches(c)

PendingSourceAllowed ==
  Matches("pending_actionable")

DeferredSourceAllowed ==
  Matches("deferred_qc_actionable")

ActiveSourceAllowed ==
  Matches("active_commit_repair")

NotFrontierRejected ==
  Matches("not_frontier_pending_actionable")

PendingWrongPhaseRejected ==
  Matches("pending_wrong_phase")

PendingHashMismatchRejected ==
  Matches("pending_hash_mismatch")

PendingHeightMismatchRejected ==
  Matches("pending_height_mismatch")

PendingViewMismatchRejected ==
  Matches("pending_view_mismatch")

PendingNotActionableRejected ==
  Matches("pending_not_actionable")

DeferredWrongPhaseRejected ==
  Matches("deferred_wrong_phase")

DeferredHashMismatchRejected ==
  Matches("deferred_hash_mismatch")

DeferredHeightMismatchRejected ==
  Matches("deferred_height_mismatch")

DeferredViewMismatchRejected ==
  Matches("deferred_view_mismatch")

DeferredNotActionableRejected ==
  Matches("deferred_not_actionable")

NoSourceRejected ==
  Matches("no_sources")

====
