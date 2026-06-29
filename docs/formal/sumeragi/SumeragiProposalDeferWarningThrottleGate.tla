---- MODULE SumeragiProposalDeferWarningThrottleGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for proposal-defer warning throttling.

This slice captures `ProposalDeferWarningThrottle::allow(...)` from
`main_loop.rs`: per-kind/height/view/hash keying, cooldown suppression with a
strict `< cooldown` check, suppressed-count replay after cooldown expiry,
empty-commit-topology view coalescing, zero-cooldown behavior, and bounded GC.
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
RegularViewSeparates == 8
EmptyProposalNormalizesView == 9
EmptyFinalizeNormalizesView == 10
EmptyTopologyKindsSeparate == 11
ZeroCooldownDoesNotSuppress == 12
GcBoundaryRetains == 13
GcExpiredPrunes == 14
GcZeroCooldownUsesOneSecond == 15

Candidates == 1..15

InsertEntry == 1
UpdateLastEmit == 2
PreserveLastEmit == 3
SuppressedIncrement == 4
SuppressedReset == 5
ReturnZero == 6
ReturnSuppressedCount == 7
ReturnNone == 8
KindKeyed == 9
HashKeyed == 10
HeightKeyed == 11
ViewKeyed == 12
ViewNormalized == 13
CooldownStrictLess == 14
CooldownBoundaryInclusive == 15
ZeroCooldownBypass == 16
GcUsesCooldownTimes8 == 17
GcUsesOneSecondForZeroCooldown == 18
GcBoundaryRetained == 19
GcExpiredPruned == 20

Actions == 1..20

SpecActions(candidate) ==
  CASE candidate = FirstWarningLogs ->
      {InsertEntry, ReturnZero}
    [] candidate = WithinCooldownSuppresses ->
      {PreserveLastEmit, SuppressedIncrement, ReturnNone,
       CooldownStrictLess}
    [] candidate = CooldownBoundaryEmits ->
      {UpdateLastEmit, SuppressedReset, ReturnSuppressedCount,
       CooldownBoundaryInclusive}
    [] candidate = SuppressedCountReplayed ->
      {UpdateLastEmit, SuppressedReset, ReturnSuppressedCount}
    [] candidate = DifferentKindSeparates ->
      {KindKeyed, InsertEntry, ReturnZero}
    [] candidate = DifferentHashSeparates ->
      {HashKeyed, InsertEntry, ReturnZero}
    [] candidate = DifferentHeightSeparates ->
      {HeightKeyed, InsertEntry, ReturnZero}
    [] candidate = RegularViewSeparates ->
      {ViewKeyed, InsertEntry, ReturnZero}
    [] candidate = EmptyProposalNormalizesView ->
      {ViewNormalized, SuppressedIncrement, ReturnNone}
    [] candidate = EmptyFinalizeNormalizesView ->
      {ViewNormalized, SuppressedIncrement, ReturnNone}
    [] candidate = EmptyTopologyKindsSeparate ->
      {KindKeyed, ViewNormalized, InsertEntry, ReturnZero}
    [] candidate = ZeroCooldownDoesNotSuppress ->
      {ZeroCooldownBypass, UpdateLastEmit, SuppressedReset, ReturnZero}
    [] candidate = GcBoundaryRetains ->
      {GcUsesCooldownTimes8, GcBoundaryRetained}
    [] candidate = GcExpiredPrunes ->
      {GcUsesCooldownTimes8, GcExpiredPruned}
    [] candidate = GcZeroCooldownUsesOneSecond ->
      {GcUsesOneSecondForZeroCooldown, GcBoundaryRetained}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = FirstWarningLogs /\ Bug = "first_suppressed" ->
      (spec \ {InsertEntry, ReturnZero}) \cup
        {PreserveLastEmit, ReturnNone}
    [] candidate = WithinCooldownSuppresses /\
          Bug = "within_cooldown_logs" ->
      (spec \ {PreserveLastEmit, SuppressedIncrement, ReturnNone}) \cup
        {UpdateLastEmit, SuppressedReset, ReturnSuppressedCount}
    [] candidate = CooldownBoundaryEmits /\
          Bug = "cooldown_boundary_suppressed" ->
      (spec \ {UpdateLastEmit, SuppressedReset, ReturnSuppressedCount,
               CooldownBoundaryInclusive}) \cup
        {PreserveLastEmit, SuppressedIncrement, ReturnNone}
    [] candidate = SuppressedCountReplayed /\
          Bug = "suppressed_count_lost" ->
      spec \ {ReturnSuppressedCount}
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
    [] candidate = RegularViewSeparates /\
          Bug = "regular_view_normalized" ->
      (spec \ {ViewKeyed, InsertEntry, ReturnZero}) \cup
        {ViewNormalized, SuppressedIncrement, ReturnNone}
    [] candidate = EmptyProposalNormalizesView /\
          Bug = "empty_proposal_view_not_normalized" ->
      (spec \ {ViewNormalized, SuppressedIncrement, ReturnNone}) \cup
        {ViewKeyed, InsertEntry, ReturnZero}
    [] candidate = EmptyFinalizeNormalizesView /\
          Bug = "empty_finalize_view_not_normalized" ->
      (spec \ {ViewNormalized, SuppressedIncrement, ReturnNone}) \cup
        {ViewKeyed, InsertEntry, ReturnZero}
    [] candidate = EmptyTopologyKindsSeparate /\
          Bug = "empty_topology_kinds_collide" ->
      (spec \ {KindKeyed, InsertEntry, ReturnZero}) \cup
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
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 15
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..15

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

BugDifferentHashCollides ==
  ImplementationActions(DifferentHashSeparates) =
    SpecActions(DifferentHashSeparates)

BugDifferentHeightCollides ==
  ImplementationActions(DifferentHeightSeparates) =
    SpecActions(DifferentHeightSeparates)

BugRegularViewNormalized ==
  ImplementationActions(RegularViewSeparates) = SpecActions(RegularViewSeparates)

BugEmptyProposalViewNotNormalized ==
  ImplementationActions(EmptyProposalNormalizesView) =
    SpecActions(EmptyProposalNormalizesView)

BugEmptyFinalizeViewNotNormalized ==
  ImplementationActions(EmptyFinalizeNormalizesView) =
    SpecActions(EmptyFinalizeNormalizesView)

BugEmptyTopologyKindsCollide ==
  ImplementationActions(EmptyTopologyKindsSeparate) =
    SpecActions(EmptyTopologyKindsSeparate)

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

ProposalDeferWarningThrottleExactness ==
  /\ BugFirstSuppressed
  /\ BugWithinCooldownLogs
  /\ BugCooldownBoundarySuppressed
  /\ BugSuppressedCountLost
  /\ BugDifferentKindCollides
  /\ BugDifferentHashCollides
  /\ BugDifferentHeightCollides
  /\ BugRegularViewNormalized
  /\ BugEmptyProposalViewNotNormalized
  /\ BugEmptyFinalizeViewNotNormalized
  /\ BugEmptyTopologyKindsCollide
  /\ BugZeroCooldownSuppresses
  /\ BugGcBoundaryPruned
  /\ BugGcExpiredRetained
  /\ BugGcZeroCooldownUsesZero

ProposalDeferWarningThrottleCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ProposalDeferWarningThrottleExactness

=============================================================================
====
