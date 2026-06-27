---- MODULE SumeragiRbcRebroadcastCursorGate ----
EXTENDS Naturals, Sequences

(***************************************************************************
A bounded abstract model for the session-selection/cursor portion of
`rebroadcast_stalled_rbc_payloads(...)`.

This slice captures the deterministic loop before per-session repair work:
the session budget is floored to one and clamped by the number of sessions;
urgent near-tip sessions are selected first, starting after the current cursor
when the cursor is present in the urgent list; selected urgent sessions count
against the same per-tick budget; normal scanning resumes after the cursor and
wraps through the prefix; duplicate urgent/normal candidates are skipped; only
active sessions are selected; and the rebroadcast cursor advances to the last
session key scanned or urgent-selected, not merely the last key that produced
work.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

None == 0
A == 1
B == 2
C == 3
D == 4

Keys == {A, B, C, D}

Empty == "empty"
BudgetZeroFloored == "budget_zero_floored"
BudgetClampTotal == "budget_clamp_total"
UrgentNoCursor == "urgent_no_cursor"
UrgentCursorInList == "urgent_cursor_in_list"
UrgentDedup == "urgent_dedup"
UrgentFillsBudget == "urgent_fills_budget"
CursorWrap == "cursor_wrap"
CursorSkipsInactive == "cursor_skips_inactive"
CursorAbsentScan == "cursor_absent_scan"
UrgentNormalDedup == "urgent_normal_dedup"
NoActiveCursorAdvances == "no_active_cursor_advances"

Cases == {
  Empty,
  BudgetZeroFloored,
  BudgetClampTotal,
  UrgentNoCursor,
  UrgentCursorInList,
  UrgentDedup,
  UrgentFillsBudget,
  CursorWrap,
  CursorSkipsInactive,
  CursorAbsentScan,
  UrgentNormalDedup,
  NoActiveCursorAdvances
}

SpecSelection(c) ==
  CASE c = Empty -> [selected |-> <<>>, cursor |-> None]
    [] c = BudgetZeroFloored -> [selected |-> <<A>>, cursor |-> A]
    [] c = BudgetClampTotal -> [selected |-> <<A, B, C>>, cursor |-> C]
    [] c = UrgentNoCursor -> [selected |-> <<C, A>>, cursor |-> A]
    [] c = UrgentCursorInList -> [selected |-> <<C, A>>, cursor |-> A]
    [] c = UrgentDedup -> [selected |-> <<B, C>>, cursor |-> C]
    [] c = UrgentFillsBudget -> [selected |-> <<B>>, cursor |-> B]
    [] c = CursorWrap -> [selected |-> <<C, D, A>>, cursor |-> A]
    [] c = CursorSkipsInactive -> [selected |-> <<C>>, cursor |-> A]
    [] c = CursorAbsentScan -> [selected |-> <<A, D>>, cursor |-> D]
    [] c = UrgentNormalDedup -> [selected |-> <<B, A, C>>, cursor |-> C]
    [] c = NoActiveCursorAdvances -> [selected |-> <<>>, cursor |-> D]

ActualSelection(c) ==
  CASE Bug = "empty_keeps_cursor"
       /\ c = Empty -> [selected |-> <<>>, cursor |-> B]
    [] Bug = "budget_zero_selects_none"
       /\ c = BudgetZeroFloored -> [selected |-> <<>>, cursor |-> None]
    [] Bug = "budget_not_clamped"
       /\ c = BudgetClampTotal -> [selected |-> <<A, B, C, D>>, cursor |-> D]
    [] Bug = "urgent_ignored"
       /\ c = UrgentNoCursor -> [selected |-> <<A, B>>, cursor |-> B]
    [] Bug = "urgent_cursor_not_rotated"
       /\ c = UrgentCursorInList -> [selected |-> <<A, B>>, cursor |-> B]
    [] Bug = "urgent_duplicates_kept"
       /\ c = UrgentDedup -> [selected |-> <<B, B>>, cursor |-> B]
    [] Bug = "urgent_over_budget_scans_normal"
       /\ c = UrgentFillsBudget -> [selected |-> <<B, A>>, cursor |-> A]
    [] Bug = "cursor_scan_no_wrap"
       /\ c = CursorWrap -> [selected |-> <<C, D>>, cursor |-> D]
    [] Bug = "inactive_selected"
       /\ c = CursorSkipsInactive -> [selected |-> <<B, C>>, cursor |-> C]
    [] Bug = "cursor_not_advanced_on_inactive"
       /\ c = CursorSkipsInactive -> [selected |-> <<C>>, cursor |-> C]
    [] Bug = "absent_cursor_starts_after_first"
       /\ c = CursorAbsentScan -> [selected |-> <<D, A>>, cursor |-> A]
    [] Bug = "normal_duplicate_selected"
       /\ c = UrgentNormalDedup -> [selected |-> <<B, A, B>>, cursor |-> B]
    [] Bug = "no_active_keeps_old_cursor"
       /\ c = NoActiveCursorAdvances -> [selected |-> <<>>, cursor |-> None]
    [] OTHER -> SpecSelection(c)

BugSet == {
  "none",
  "empty_keeps_cursor",
  "budget_zero_selects_none",
  "budget_not_clamped",
  "urgent_ignored",
  "urgent_cursor_not_rotated",
  "urgent_duplicates_kept",
  "urgent_over_budget_scans_normal",
  "cursor_scan_no_wrap",
  "inactive_selected",
  "cursor_not_advanced_on_inactive",
  "absent_cursor_starts_after_first",
  "normal_duplicate_selected",
  "no_active_keeps_old_cursor"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0
  /\ \A c \in Cases:
       /\ ActualSelection(c).cursor \in Keys \cup {None}
       /\ Len(ActualSelection(c).selected) <= 4
       /\ \A i \in 1..Len(ActualSelection(c).selected):
            ActualSelection(c).selected[i] \in Keys

SelectionExact ==
  \A c \in Cases:
    ActualSelection(c) = SpecSelection(c)

StableSelections ==
  /\ ActualSelection(Empty) = [selected |-> <<>>, cursor |-> None]
  /\ ActualSelection(BudgetZeroFloored) = [selected |-> <<A>>, cursor |-> A]
  /\ ActualSelection(BudgetClampTotal) = [selected |-> <<A, B, C>>, cursor |-> C]
  /\ ActualSelection(UrgentNoCursor) = [selected |-> <<C, A>>, cursor |-> A]
  /\ ActualSelection(UrgentCursorInList) = [selected |-> <<C, A>>, cursor |-> A]
  /\ ActualSelection(UrgentDedup) = [selected |-> <<B, C>>, cursor |-> C]
  /\ ActualSelection(UrgentFillsBudget) = [selected |-> <<B>>, cursor |-> B]
  /\ ActualSelection(CursorWrap) = [selected |-> <<C, D, A>>, cursor |-> A]
  /\ ActualSelection(CursorSkipsInactive) = [selected |-> <<C>>, cursor |-> A]
  /\ ActualSelection(CursorAbsentScan) = [selected |-> <<A, D>>, cursor |-> D]
  /\ ActualSelection(UrgentNormalDedup) = [selected |-> <<B, A, C>>, cursor |-> C]
  /\ ActualSelection(NoActiveCursorAdvances) = [selected |-> <<>>, cursor |-> D]

RbcRebroadcastCursorCoreSafety ==
  /\ SelectionExact
  /\ StableSelections

RbcRebroadcastCursorExactness == RbcRebroadcastCursorCoreSafety

RbcRebroadcastCursorCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcRebroadcastCursorExactness

SafetyFast ==
  RbcRebroadcastCursorExactness

====
