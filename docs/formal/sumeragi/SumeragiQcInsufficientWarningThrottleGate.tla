---- MODULE SumeragiQcInsufficientWarningThrottleGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for QC-insufficient warning throttling.

This slice captures `QcInsufficientWarningThrottle::allow(...)` and `clear()`
from `main_loop.rs`: per-kind/phase/hash/height/view keying, strict cooldown
suppression, suppressed-count replay after cooldown expiry, zero-cooldown
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
DifferentPhaseSeparates == 6
DifferentHashSeparates == 7
DifferentHeightSeparates == 8
DifferentViewSeparates == 9
ZeroCooldownDoesNotSuppress == 10
GcBoundaryRetains == 11
GcExpiredPrunes == 12
GcZeroCooldownUsesOneSecond == 13
ClearResetsEntries == 14

Candidates == 1..14

InsertEntry == 1
UpdateLastEmit == 2
PreserveLastEmit == 3
SuppressedIncrement == 4
SuppressedReset == 5
ReturnZero == 6
ReturnSuppressedCount == 7
ReturnNone == 8
KindKeyed == 9
PhaseKeyed == 10
HashKeyed == 11
HeightKeyed == 12
ViewKeyed == 13
CooldownStrictLess == 14
CooldownBoundaryInclusive == 15
ZeroCooldownBypass == 16
GcUsesCooldownTimes8 == 17
GcUsesOneSecondForZeroCooldown == 18
GcBoundaryRetained == 19
GcExpiredPruned == 20
ClearEntries == 21

Actions == 1..21

SpecActions(candidate) ==
  CASE candidate = FirstWarningLogs ->
      {InsertEntry, ReturnZero}
    [] candidate = WithinCooldownSuppresses ->
      {CooldownStrictLess, PreserveLastEmit, SuppressedIncrement, ReturnNone}
    [] candidate = CooldownBoundaryEmits ->
      {CooldownBoundaryInclusive, UpdateLastEmit, SuppressedReset,
       ReturnSuppressedCount}
    [] candidate = SuppressedCountReplayed ->
      {UpdateLastEmit, SuppressedReset, ReturnSuppressedCount}
    [] candidate = DifferentKindSeparates ->
      {KindKeyed, InsertEntry, ReturnZero}
    [] candidate = DifferentPhaseSeparates ->
      {PhaseKeyed, InsertEntry, ReturnZero}
    [] candidate = DifferentHashSeparates ->
      {HashKeyed, InsertEntry, ReturnZero}
    [] candidate = DifferentHeightSeparates ->
      {HeightKeyed, InsertEntry, ReturnZero}
    [] candidate = DifferentViewSeparates ->
      {ViewKeyed, InsertEntry, ReturnZero}
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
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = FirstWarningLogs /\ Bug = "first_suppressed" ->
      (spec \ {InsertEntry, ReturnZero}) \cup {PreserveLastEmit, ReturnNone}
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
      spec \ {ReturnSuppressedCount}
    [] candidate = DifferentKindSeparates /\
          Bug = "different_kind_collides" ->
      (spec \ {KindKeyed, InsertEntry, ReturnZero}) \cup
        {SuppressedIncrement, ReturnNone}
    [] candidate = DifferentPhaseSeparates /\
          Bug = "different_phase_collides" ->
      (spec \ {PhaseKeyed, InsertEntry, ReturnZero}) \cup
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
    [] OTHER -> spec

Init ==
  checked = 0

Advance ==
  /\ checked < 14
  /\ checked' = checked + 1

Stable ==
  /\ checked = 14
  /\ checked' = checked

Next ==
  Advance \/ Stable

TypeInvariant ==
  checked \in 0..14

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

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

BugDifferentPhaseCollides ==
  ImplementationActions(DifferentPhaseSeparates) =
    SpecActions(DifferentPhaseSeparates)

BugDifferentHashCollides ==
  ImplementationActions(DifferentHashSeparates) =
    SpecActions(DifferentHashSeparates)

BugDifferentHeightCollides ==
  ImplementationActions(DifferentHeightSeparates) =
    SpecActions(DifferentHeightSeparates)

BugDifferentViewCollides ==
  ImplementationActions(DifferentViewSeparates) =
    SpecActions(DifferentViewSeparates)

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

=============================================================================
