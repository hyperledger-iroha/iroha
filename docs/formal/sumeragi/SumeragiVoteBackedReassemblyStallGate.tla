---- MODULE SumeragiVoteBackedReassemblyStallGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the vote-backed same-height frontier reassembly
stall helpers in `main_loop/reschedule.rs`.

This slice pins three deterministic decisions:

* the hard cap is twice the maximum of the frontier recovery window, quorum
  timeout, rebroadcast resend window, and the one millisecond floor;
* the owner stall age comes from the exact same-height frontier slot only when
  the slot is active for the same view and was last repaired by a quorum
  timeout, otherwise from a matching quorum-timeout `frontier_recovery` owner;
* the stall expires only when both the owner stall age and quorum stall age
  reach the hard cap.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

HardCapFrontierWindowDominates == "HardCapFrontierWindowDominates"
HardCapQuorumTimeoutDominates == "HardCapQuorumTimeoutDominates"
HardCapRebroadcastDominates == "HardCapRebroadcastDominates"
HardCapOneMsFloor == "HardCapOneMsFloor"
SlotOwnerActive == "SlotOwnerActive"
SlotOwnerFinalizedRejected == "SlotOwnerFinalizedRejected"
SlotOwnerPassiveRejected == "SlotOwnerPassiveRejected"
SlotOwnerWrongReasonRejected == "SlotOwnerWrongReasonRejected"
SlotOwnerWrongViewRejected == "SlotOwnerWrongViewRejected"
SlotOwnerWrongHeightRejected == "SlotOwnerWrongHeightRejected"
SlotOwnerNotExactHeightRejected == "SlotOwnerNotExactHeightRejected"
SlotOwnerUsesLatestProgress == "SlotOwnerUsesLatestProgress"
RecoveryOwnerActive == "RecoveryOwnerActive"
RecoveryOwnerWrongCauseRejected == "RecoveryOwnerWrongCauseRejected"
RecoveryOwnerWrongViewRejected == "RecoveryOwnerWrongViewRejected"
RecoveryOwnerUsesLatestProgress == "RecoveryOwnerUsesLatestProgress"
RecoveryAfterRejectedSlot == "RecoveryAfterRejectedSlot"
NoOwnerNoExpiry == "NoOwnerNoExpiry"
OwnerBelowCapNoExpiry == "OwnerBelowCapNoExpiry"
QuorumBelowCapNoExpiry == "QuorumBelowCapNoExpiry"
BothAtCapExpires == "BothAtCapExpires"

HardCapCases == {
  HardCapFrontierWindowDominates,
  HardCapQuorumTimeoutDominates,
  HardCapRebroadcastDominates,
  HardCapOneMsFloor
}

SlotValidCases ==
  HardCapCases \cup {
    SlotOwnerActive,
    SlotOwnerUsesLatestProgress,
    OwnerBelowCapNoExpiry,
    QuorumBelowCapNoExpiry,
    BothAtCapExpires
  }

SlotRejectedCases == {
  SlotOwnerFinalizedRejected,
  SlotOwnerPassiveRejected,
  SlotOwnerWrongReasonRejected,
  SlotOwnerWrongViewRejected,
  SlotOwnerWrongHeightRejected,
  SlotOwnerNotExactHeightRejected,
  RecoveryAfterRejectedSlot
}

RecoveryValidCases == {
  RecoveryOwnerActive,
  RecoveryOwnerUsesLatestProgress,
  RecoveryAfterRejectedSlot
}

Cases ==
  HardCapCases
  \cup SlotValidCases
  \cup SlotRejectedCases
  \cup RecoveryValidCases
  \cup {
    RecoveryOwnerWrongCauseRejected,
    RecoveryOwnerWrongViewRejected,
    NoOwnerNoExpiry
  }

Max(a, b) == IF a >= b THEN a ELSE b
Min(a, b) == IF a <= b THEN a ELSE b
BoolToInt(b) == IF b THEN 1 ELSE 0

Now(c) == 1000

FrontierRecoveryWindow(c) ==
  CASE c = HardCapFrontierWindowDominates -> 80
    [] c = HardCapQuorumTimeoutDominates -> 20
    [] c = HardCapRebroadcastDominates -> 20
    [] c = HardCapOneMsFloor -> 0
    [] OTHER -> 40

QuorumTimeout(c) ==
  CASE c = HardCapFrontierWindowDominates -> 10
    [] c = HardCapQuorumTimeoutDominates -> 90
    [] c = HardCapRebroadcastDominates -> 30
    [] c = HardCapOneMsFloor -> 0
    [] OTHER -> 10

RebroadcastCooldown(c) ==
  CASE c = HardCapFrontierWindowDominates -> 5
    [] c = HardCapQuorumTimeoutDominates -> 5
    [] c = HardCapRebroadcastDominates -> 70
    [] c = HardCapOneMsFloor -> 0
    [] OTHER -> 5

SpecResendWindow(c) ==
  Max(RebroadcastCooldown(c), 1)

SpecHardCapBase(c) ==
  Max(Max(Max(FrontierRecoveryWindow(c), QuorumTimeout(c)), SpecResendWindow(c)), 1)

SpecHardCap(c) ==
  SpecHardCapBase(c) * 2

ActualResendWindow(c) ==
  IF Bug = "hard_cap_skips_one_ms_floor" /\ c = HardCapOneMsFloor
  THEN RebroadcastCooldown(c)
  ELSE SpecResendWindow(c)

ActualHardCapBase(c) ==
  CASE Bug = "hard_cap_uses_min" /\ c = HardCapFrontierWindowDominates ->
       Max(Min(Min(FrontierRecoveryWindow(c), QuorumTimeout(c)), ActualResendWindow(c)), 1)
    [] Bug = "hard_cap_skips_resend_window" /\ c = HardCapRebroadcastDominates ->
       Max(Max(FrontierRecoveryWindow(c), QuorumTimeout(c)), 1)
    [] Bug = "hard_cap_skips_one_ms_floor" /\ c = HardCapOneMsFloor ->
       Max(Max(FrontierRecoveryWindow(c), QuorumTimeout(c)), ActualResendWindow(c))
    [] OTHER ->
       Max(Max(Max(FrontierRecoveryWindow(c), QuorumTimeout(c)), ActualResendWindow(c)), 1)

ActualHardCap(c) ==
  IF Bug = "hard_cap_skips_double" /\ c = HardCapQuorumTimeoutDominates
  THEN ActualHardCapBase(c)
  ELSE ActualHardCapBase(c) * 2

SlotExists(c) ==
  c \in SlotValidCases \cup SlotRejectedCases

SlotExactHeight(c) ==
  c /= SlotOwnerNotExactHeightRejected

SlotHeightMatches(c) ==
  c /= SlotOwnerWrongHeightRejected

SlotViewMatches(c) ==
  c /= SlotOwnerWrongViewRejected

SlotModeOk(c) ==
  c \notin {SlotOwnerFinalizedRejected, SlotOwnerPassiveRejected}

SlotReasonOk(c) ==
  c \notin {SlotOwnerWrongReasonRejected, RecoveryAfterRejectedSlot}

SpecSlotOwnerValid(c) ==
  /\ SlotExists(c)
  /\ SlotExactHeight(c)
  /\ SlotHeightMatches(c)
  /\ SlotViewMatches(c)
  /\ SlotModeOk(c)
  /\ SlotReasonOk(c)

RecoveryHeightMatches(c) ==
  TRUE

RecoveryViewMatches(c) ==
  c /= RecoveryOwnerWrongViewRejected

RecoveryCauseOk(c) ==
  c /= RecoveryOwnerWrongCauseRejected

RecoveryExists(c) ==
  c \in RecoveryValidCases
    \cup {RecoveryOwnerWrongCauseRejected, RecoveryOwnerWrongViewRejected}

SpecRecoveryOwnerValid(c) ==
  /\ RecoveryExists(c)
  /\ RecoveryHeightMatches(c)
  /\ RecoveryViewMatches(c)
  /\ RecoveryCauseOk(c)

SpecOwnerPresent(c) ==
  SpecSlotOwnerValid(c) \/ SpecRecoveryOwnerValid(c)

SpecSlotOwnerAge(c) ==
  CASE c = SlotOwnerUsesLatestProgress -> 50
    [] c = OwnerBelowCapNoExpiry -> 70
    [] c = BothAtCapExpires -> 80
    [] c = QuorumBelowCapNoExpiry -> 100
    [] c \in HardCapCases -> 200
    [] OTHER -> 100

SlotOldestProgressAt(c) ==
  IF c = SlotOwnerUsesLatestProgress THEN 100 ELSE Now(c) - SpecSlotOwnerAge(c)

SlotLatestProgressAt(c) ==
  Now(c) - SpecSlotOwnerAge(c)

SpecRecoveryOwnerAge(c) ==
  CASE c = RecoveryOwnerUsesLatestProgress -> 40
    [] OTHER -> 100

RecoveryOldestProgressAt(c) ==
  IF c = RecoveryOwnerUsesLatestProgress THEN 800 ELSE Now(c) - SpecRecoveryOwnerAge(c)

RecoveryLatestProgressAt(c) ==
  Now(c) - SpecRecoveryOwnerAge(c)

SpecOwnerAge(c) ==
  IF SpecSlotOwnerValid(c) THEN Now(c) - SlotLatestProgressAt(c)
  ELSE IF SpecRecoveryOwnerValid(c) THEN Now(c) - RecoveryLatestProgressAt(c)
  ELSE 0

ActualSlotOwnerValid(c) ==
  CASE Bug = "slot_accepts_finalized" /\ c = SlotOwnerFinalizedRejected ->
       TRUE
    [] Bug = "slot_accepts_passive" /\ c = SlotOwnerPassiveRejected ->
       TRUE
    [] Bug = "slot_accepts_wrong_reason" /\ c = SlotOwnerWrongReasonRejected ->
       TRUE
    [] Bug = "slot_accepts_wrong_view" /\ c = SlotOwnerWrongViewRejected ->
       TRUE
    [] Bug = "slot_accepts_wrong_height" /\ c = SlotOwnerWrongHeightRejected ->
       TRUE
    [] Bug = "slot_accepts_not_exact_height" /\
       c = SlotOwnerNotExactHeightRejected -> TRUE
    [] OTHER -> SpecSlotOwnerValid(c)

ActualRecoveryOwnerValid(c) ==
  CASE Bug = "recovery_accepts_wrong_cause" /\
       c = RecoveryOwnerWrongCauseRejected -> TRUE
    [] Bug = "recovery_accepts_wrong_view" /\
       c = RecoveryOwnerWrongViewRejected -> TRUE
    [] Bug = "rejected_slot_blocks_recovery" /\
       c = RecoveryAfterRejectedSlot -> FALSE
    [] OTHER -> SpecRecoveryOwnerValid(c)

ActualOwnerPresent(c) ==
  ActualSlotOwnerValid(c) \/ ActualRecoveryOwnerValid(c)

ActualSlotOwnerAge(c) ==
  IF Bug = "slot_uses_oldest_progress" /\ c = SlotOwnerUsesLatestProgress
  THEN Now(c) - SlotOldestProgressAt(c)
  ELSE Now(c) - SlotLatestProgressAt(c)

ActualRecoveryOwnerAge(c) ==
  IF Bug = "recovery_uses_oldest_progress" /\ c = RecoveryOwnerUsesLatestProgress
  THEN Now(c) - RecoveryOldestProgressAt(c)
  ELSE Now(c) - RecoveryLatestProgressAt(c)

ActualOwnerAge(c) ==
  IF ActualSlotOwnerValid(c) THEN ActualSlotOwnerAge(c)
  ELSE IF ActualRecoveryOwnerValid(c) THEN ActualRecoveryOwnerAge(c)
  ELSE 0

QuorumStallAge(c) ==
  CASE c = QuorumBelowCapNoExpiry -> 70
    [] c = BothAtCapExpires -> 80
    [] OTHER -> 200

SpecExpired(c) ==
  SpecOwnerPresent(c)
    /\ SpecOwnerAge(c) >= SpecHardCap(c)
    /\ QuorumStallAge(c) >= SpecHardCap(c)

ActualExpired(c) ==
  CASE Bug = "no_owner_expires" /\ c = NoOwnerNoExpiry -> TRUE
    [] Bug = "owner_below_cap_expires" /\ c = OwnerBelowCapNoExpiry -> TRUE
    [] Bug = "quorum_below_cap_expires" /\ c = QuorumBelowCapNoExpiry -> TRUE
    [] Bug = "both_at_cap_rejected" /\ c = BothAtCapExpires -> FALSE
    [] OTHER ->
       ActualOwnerPresent(c)
         /\ ActualOwnerAge(c) >= ActualHardCap(c)
         /\ QuorumStallAge(c) >= ActualHardCap(c)

\* @type: (Str) => <<Int, Int, Int, Int>>;
SpecOutput(c) ==
  <<BoolToInt(SpecOwnerPresent(c)), SpecOwnerAge(c), SpecHardCap(c),
    BoolToInt(SpecExpired(c))>>

\* @type: (Str) => <<Int, Int, Int, Int>>;
ActualOutput(c) ==
  <<BoolToInt(ActualOwnerPresent(c)), ActualOwnerAge(c), ActualHardCap(c),
    BoolToInt(ActualExpired(c))>>

BugSet == {
  "none",
  "hard_cap_uses_min",
  "hard_cap_skips_double",
  "hard_cap_skips_resend_window",
  "hard_cap_skips_one_ms_floor",
  "slot_accepts_finalized",
  "slot_accepts_passive",
  "slot_accepts_wrong_reason",
  "slot_accepts_wrong_view",
  "slot_accepts_wrong_height",
  "slot_accepts_not_exact_height",
  "slot_uses_oldest_progress",
  "recovery_accepts_wrong_cause",
  "recovery_accepts_wrong_view",
  "recovery_uses_oldest_progress",
  "rejected_slot_blocks_recovery",
  "no_owner_expires",
  "owner_below_cap_expires",
  "quorum_below_cap_expires",
  "both_at_cap_rejected"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0

SelectionExact ==
  \A c \in Cases:
    ActualOutput(c) = SpecOutput(c)

HardCapStable ==
  /\ SpecHardCap(HardCapFrontierWindowDominates) = 160
  /\ SpecHardCap(HardCapQuorumTimeoutDominates) = 180
  /\ SpecHardCap(HardCapRebroadcastDominates) = 140
  /\ SpecHardCap(HardCapOneMsFloor) = 2

OwnerSourceStable ==
  /\ SpecOwnerPresent(SlotOwnerActive)
  /\ ~SpecOwnerPresent(SlotOwnerFinalizedRejected)
  /\ ~SpecOwnerPresent(SlotOwnerPassiveRejected)
  /\ ~SpecOwnerPresent(SlotOwnerWrongReasonRejected)
  /\ ~SpecOwnerPresent(SlotOwnerWrongViewRejected)
  /\ ~SpecOwnerPresent(SlotOwnerWrongHeightRejected)
  /\ ~SpecOwnerPresent(SlotOwnerNotExactHeightRejected)
  /\ SpecOwnerAge(SlotOwnerUsesLatestProgress) = 50
  /\ SpecOwnerPresent(RecoveryOwnerActive)
  /\ ~SpecOwnerPresent(RecoveryOwnerWrongCauseRejected)
  /\ ~SpecOwnerPresent(RecoveryOwnerWrongViewRejected)
  /\ SpecOwnerAge(RecoveryOwnerUsesLatestProgress) = 40
  /\ SpecOwnerPresent(RecoveryAfterRejectedSlot)

ExpiryStable ==
  /\ ~SpecExpired(NoOwnerNoExpiry)
  /\ ~SpecExpired(OwnerBelowCapNoExpiry)
  /\ ~SpecExpired(QuorumBelowCapNoExpiry)
  /\ SpecExpired(BothAtCapExpires)

SafetyFast ==
  /\ SelectionExact
  /\ HardCapStable
  /\ OwnerSourceStable
  /\ ExpiryStable

Safety ==
  SafetyFast

=============================================================================
