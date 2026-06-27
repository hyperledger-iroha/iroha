---- MODULE SumeragiBlockSyncWarningThrottleGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for block-sync warning throttling.

This slice captures `BlockSyncWarningThrottle::allow(...)` and `clear()` from
`main_loop.rs`: per-kind/hash/height/view keying, strict cooldown suppression,
suppressed-count replay, global burst-window caps, zero-cap and zero-cooldown
behavior, bounded GC, and reset semantics.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

FirstWarningLogs == 1
WithinCooldownSuppresses == 2
CooldownBoundaryEmits == 3
SuppressedCountReplayed == 4
DifferentKindSeparates == 5
DifferentHashSeparates == 6
DifferentHeightSeparates == 7
DifferentViewSeparates == 8
BurstCapNewSuppresses == 9
BurstCapExistingSuppresses == 10
BurstWindowWithinPreservesCap == 11
BurstWindowBoundaryResets == 12
ZeroBurstCapUnlimited == 13
ZeroCooldownDoesNotSuppress == 14
GcBoundaryRetains == 15
GcExpiredPrunes == 16
GcZeroCooldownUsesOneSecond == 17
ClearResetsEntries == 18
ClearResetsBurst == 19

Candidates == 1..19

InsertEntry == 1
NoInsert == 2
UpdateLastEmit == 3
PreserveLastEmit == 4
SuppressedIncrement == 5
SuppressedReset == 6
ReturnZero == 7
ReturnSuppressedCount == 8
ReturnNone == 9
KindKeyed == 10
HashKeyed == 11
HeightKeyed == 12
ViewKeyed == 13
CooldownStrictLess == 14
CooldownBoundaryInclusive == 15
ZeroCooldownBypass == 16
RefreshBurstWindow == 17
BurstWindowPreserved == 18
BurstWindowReset == 19
BurstCounterIncrement == 20
BurstCounterReset == 21
BurstCounterPreserved == 22
BurstCapBlocks == 23
BurstCapDisabled == 24
GcUsesCooldownTimes8 == 25
GcUsesOneSecondForZeroCooldown == 26
GcBoundaryRetained == 27
GcExpiredPruned == 28
ClearEntries == 29
ClearBurstWindow == 30
ClearBurstCounter == 31

Actions == 1..31

SpecActions(candidate) ==
  CASE candidate = FirstWarningLogs ->
      {RefreshBurstWindow, InsertEntry, BurstCounterIncrement, ReturnZero}
    [] candidate = WithinCooldownSuppresses ->
      {CooldownStrictLess, PreserveLastEmit, SuppressedIncrement,
       BurstCounterPreserved, ReturnNone}
    [] candidate = CooldownBoundaryEmits ->
      {CooldownBoundaryInclusive, UpdateLastEmit, SuppressedReset,
       RefreshBurstWindow, BurstCounterIncrement, ReturnSuppressedCount}
    [] candidate = SuppressedCountReplayed ->
      {UpdateLastEmit, SuppressedReset, ReturnSuppressedCount}
    [] candidate = DifferentKindSeparates ->
      {KindKeyed, InsertEntry, ReturnZero}
    [] candidate = DifferentHashSeparates ->
      {HashKeyed, InsertEntry, ReturnZero}
    [] candidate = DifferentHeightSeparates ->
      {HeightKeyed, InsertEntry, ReturnZero}
    [] candidate = DifferentViewSeparates ->
      {ViewKeyed, InsertEntry, ReturnZero}
    [] candidate = BurstCapNewSuppresses ->
      {RefreshBurstWindow, BurstCapBlocks, NoInsert, BurstCounterPreserved,
       ReturnNone}
    [] candidate = BurstCapExistingSuppresses ->
      {UpdateLastEmit, SuppressedReset, RefreshBurstWindow, BurstCapBlocks,
       BurstCounterPreserved, ReturnNone}
    [] candidate = BurstWindowWithinPreservesCap ->
      {BurstWindowPreserved, BurstCounterPreserved, BurstCapBlocks, ReturnNone}
    [] candidate = BurstWindowBoundaryResets ->
      {BurstWindowReset, BurstCounterReset, BurstCounterIncrement, ReturnZero}
    [] candidate = ZeroBurstCapUnlimited ->
      {BurstCapDisabled, InsertEntry, BurstCounterPreserved, ReturnZero}
    [] candidate = ZeroCooldownDoesNotSuppress ->
      {ZeroCooldownBypass, UpdateLastEmit, SuppressedReset, ReturnZero}
    [] candidate = GcBoundaryRetains ->
      {GcUsesCooldownTimes8, GcBoundaryRetained}
    [] candidate = GcExpiredPrunes ->
      {GcUsesCooldownTimes8, GcExpiredPruned}
    [] candidate = GcZeroCooldownUsesOneSecond ->
      {GcUsesOneSecondForZeroCooldown, GcBoundaryRetained}
    [] candidate = ClearResetsEntries ->
      {ClearEntries}
    [] candidate = ClearResetsBurst ->
      {ClearBurstWindow, ClearBurstCounter}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = FirstWarningLogs /\ Bug = "first_suppressed" ->
      (spec \ {InsertEntry, ReturnZero}) \cup {NoInsert, ReturnNone}
    [] candidate = WithinCooldownSuppresses /\
          Bug = "within_cooldown_logs" ->
      (spec \ {PreserveLastEmit, SuppressedIncrement, ReturnNone}) \cup
        {UpdateLastEmit, SuppressedReset, ReturnSuppressedCount}
    [] candidate = CooldownBoundaryEmits /\
          Bug = "cooldown_boundary_suppressed" ->
      (spec \ {CooldownBoundaryInclusive, UpdateLastEmit, SuppressedReset,
               ReturnSuppressedCount}) \cup
        {CooldownStrictLess, PreserveLastEmit, SuppressedIncrement, ReturnNone}
    [] candidate = SuppressedCountReplayed /\
          Bug = "suppressed_count_lost" ->
      (spec \ {ReturnSuppressedCount}) \cup {ReturnZero}
    [] candidate = DifferentKindSeparates /\
          Bug = "different_kind_collides" ->
      (spec \ {KindKeyed, InsertEntry, ReturnZero}) \cup
        {SuppressedIncrement, ReturnNone}
    [] candidate = DifferentHashSeparates /\
          Bug = "different_hash_collides" ->
      (spec \ {HashKeyed, InsertEntry, ReturnZero}) \cup
        {SuppressedIncrement, ReturnNone}
    [] candidate = DifferentHeightSeparates /\
          Bug = "different_height_collides" ->
      (spec \ {HeightKeyed, InsertEntry, ReturnZero}) \cup
        {SuppressedIncrement, ReturnNone}
    [] candidate = DifferentViewSeparates /\
          Bug = "different_view_collides" ->
      (spec \ {ViewKeyed, InsertEntry, ReturnZero}) \cup
        {SuppressedIncrement, ReturnNone}
    [] candidate = BurstCapNewSuppresses /\
          Bug = "burst_cap_new_inserts" ->
      (spec \ {NoInsert}) \cup {InsertEntry}
    [] candidate = BurstCapNewSuppresses /\ Bug = "burst_cap_new_logs" ->
      (spec \ {BurstCapBlocks, ReturnNone, BurstCounterPreserved}) \cup
        {BurstCounterIncrement, ReturnZero}
    [] candidate = BurstCapExistingSuppresses /\
          Bug = "burst_cap_existing_logs" ->
      (spec \ {BurstCapBlocks, ReturnNone, BurstCounterPreserved}) \cup
        {BurstCounterIncrement, ReturnSuppressedCount}
    [] candidate = BurstCapExistingSuppresses /\
          Bug = "burst_cap_existing_preserves_suppressed" ->
      (spec \ {SuppressedReset}) \cup {SuppressedIncrement}
    [] candidate = BurstWindowWithinPreservesCap /\
          Bug = "burst_window_within_resets" ->
      (spec \ {BurstWindowPreserved, BurstCounterPreserved, BurstCapBlocks,
               ReturnNone}) \cup
        {BurstWindowReset, BurstCounterReset, ReturnZero}
    [] candidate = BurstWindowBoundaryResets /\
          Bug = "burst_window_boundary_not_reset" ->
      (spec \ {BurstWindowReset, BurstCounterReset, BurstCounterIncrement,
               ReturnZero}) \cup
        {BurstWindowPreserved, BurstCapBlocks, ReturnNone}
    [] candidate = ZeroBurstCapUnlimited /\
          Bug = "zero_burst_cap_blocks" ->
      (spec \ {BurstCapDisabled, InsertEntry, ReturnZero}) \cup
        {BurstCapBlocks, NoInsert, ReturnNone}
    [] candidate = ZeroCooldownDoesNotSuppress /\
          Bug = "zero_cooldown_suppresses" ->
      (spec \ {ZeroCooldownBypass, UpdateLastEmit, SuppressedReset,
               ReturnZero}) \cup
        {PreserveLastEmit, SuppressedIncrement, ReturnNone}
    [] candidate = GcBoundaryRetains /\ Bug = "gc_boundary_pruned" ->
      (spec \ {GcBoundaryRetained}) \cup {GcExpiredPruned}
    [] candidate = GcExpiredPrunes /\ Bug = "gc_expired_retained" ->
      (spec \ {GcExpiredPruned}) \cup {GcBoundaryRetained}
    [] candidate = GcZeroCooldownUsesOneSecond /\
          Bug = "gc_zero_cooldown_uses_zero" ->
      (spec \ {GcUsesOneSecondForZeroCooldown, GcBoundaryRetained}) \cup
        {GcExpiredPruned}
    [] candidate = ClearResetsEntries /\ Bug = "clear_keeps_entries" ->
      spec \ {ClearEntries}
    [] candidate = ClearResetsBurst /\ Bug = "clear_keeps_burst" ->
      spec \ {ClearBurstWindow, ClearBurstCounter}
    [] OTHER -> spec

Init ==
  checked = 0

Advance ==
  /\ checked < 19
  /\ checked' = checked + 1

Stable ==
  /\ checked = 19
  /\ checked' = checked

Next ==
  Advance \/ Stable

TypeInvariant ==
  checked \in 0..19

BlockSyncWarningThrottleExactness ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BlockSyncWarningThrottleCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncWarningThrottleExactness

Safety == BlockSyncWarningThrottleExactness

BugFirstSuppressed ==
  ImplementationActions(FirstWarningLogs) = SpecActions(FirstWarningLogs)

BugWithinCooldownLogs ==
  ImplementationActions(WithinCooldownSuppresses) =
    SpecActions(WithinCooldownSuppresses)

BugCooldownBoundarySuppressed ==
  ImplementationActions(CooldownBoundaryEmits) =
    SpecActions(CooldownBoundaryEmits)

BugSuppressedCountLost ==
  ImplementationActions(SuppressedCountReplayed) =
    SpecActions(SuppressedCountReplayed)

BugDifferentKindCollides ==
  ImplementationActions(DifferentKindSeparates) =
    SpecActions(DifferentKindSeparates)

BugDifferentHashCollides ==
  ImplementationActions(DifferentHashSeparates) =
    SpecActions(DifferentHashSeparates)

BugDifferentHeightCollides ==
  ImplementationActions(DifferentHeightSeparates) =
    SpecActions(DifferentHeightSeparates)

BugDifferentViewCollides ==
  ImplementationActions(DifferentViewSeparates) =
    SpecActions(DifferentViewSeparates)

BugBurstCapNewInserts ==
  ImplementationActions(BurstCapNewSuppresses) =
    SpecActions(BurstCapNewSuppresses)

BugBurstCapNewLogs ==
  ImplementationActions(BurstCapNewSuppresses) =
    SpecActions(BurstCapNewSuppresses)

BugBurstCapExistingLogs ==
  ImplementationActions(BurstCapExistingSuppresses) =
    SpecActions(BurstCapExistingSuppresses)

BugBurstCapExistingPreservesSuppressed ==
  ImplementationActions(BurstCapExistingSuppresses) =
    SpecActions(BurstCapExistingSuppresses)

BugBurstWindowWithinResets ==
  ImplementationActions(BurstWindowWithinPreservesCap) =
    SpecActions(BurstWindowWithinPreservesCap)

BugBurstWindowBoundaryNotReset ==
  ImplementationActions(BurstWindowBoundaryResets) =
    SpecActions(BurstWindowBoundaryResets)

BugZeroBurstCapBlocks ==
  ImplementationActions(ZeroBurstCapUnlimited) =
    SpecActions(ZeroBurstCapUnlimited)

BugZeroCooldownSuppresses ==
  ImplementationActions(ZeroCooldownDoesNotSuppress) =
    SpecActions(ZeroCooldownDoesNotSuppress)

BugGcBoundaryPruned ==
  ImplementationActions(GcBoundaryRetains) = SpecActions(GcBoundaryRetains)

BugGcExpiredRetained ==
  ImplementationActions(GcExpiredPrunes) = SpecActions(GcExpiredPrunes)

BugGcZeroCooldownUsesZero ==
  ImplementationActions(GcZeroCooldownUsesOneSecond) =
    SpecActions(GcZeroCooldownUsesOneSecond)

BugClearKeepsEntries ==
  ImplementationActions(ClearResetsEntries) = SpecActions(ClearResetsEntries)

BugClearKeepsBurst ==
  ImplementationActions(ClearResetsBurst) = SpecActions(ClearResetsBurst)

=============================================================================
====
