---- MODULE SumeragiInvalidSignatureThrottleGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for invalid-signature throttling helpers.

This slice captures `InvalidSigThrottle`, `RbcMismatchThrottle`, and
`InvalidSigPenalty` from `main_loop.rs`. Time is collapsed into representative
boundary cases while preserving observable contracts: per-kind/per-signer or
per-kind/per-peer keying, first log admission, within-window suppression,
inclusive throttle/retention boundaries, height/view advancement bypasses,
RBC mismatch `should_log`, threshold-zero disablement, penalty window reset,
cooldown suppression, zero-cooldown behavior, and penalty pruning.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

InvalidFirstLogs == 1
InvalidWithinWindowThrottles == 2
InvalidAtWindowLogs == 3
InvalidHeightAdvanceLogs == 4
InvalidViewAdvanceLogs == 5
InvalidDifferentSignerLogs == 6
InvalidDifferentKindLogs == 7
InvalidRetentionBoundaryKeeps == 8
InvalidRetentionExpiredPrunes == 9
RbcFirstLogs == 10
RbcWithinWindowThrottles == 11
RbcAtWindowLogs == 12
RbcHeightAdvanceLogs == 13
RbcViewAdvanceLogs == 14
RbcDifferentPeerLogs == 15
RbcDifferentKindLogs == 16
RbcRetentionBoundaryKeeps == 17
RbcRetentionExpiredPrunes == 18
RbcLoggedShouldLogTrue == 19
RbcThrottledShouldLogFalse == 20
PenaltyThresholdZeroDisabled == 21
PenaltyNoEntryNotSuppressed == 22
PenaltyFirstBelowThreshold == 23
PenaltyThresholdTriggers == 24
PenaltyWithinCooldownSuppressed == 25
PenaltyCooldownExpiryClears == 26
PenaltyRecordDuringCooldownNoTrigger == 27
PenaltyWindowBoundaryKeepsCount == 28
PenaltyWindowExpiredResets == 29
PenaltyZeroCooldownNoSuppress == 30
PenaltyPruneBoundaryKeeps == 31
PenaltyPruneExpiredDrops == 32

Candidates == 1..32

PruneRetained == 1
PruneExpired == 2
InsertEntry == 3
UpdateEntry == 4
PreserveEntry == 5
ReturnLogged == 6
ReturnThrottled == 7
KeyKindSeparated == 8
KeySignerSeparated == 9
KeyPeerSeparated == 10
HeightAdvance == 11
ViewAdvance == 12
ThrottleBoundaryInclusive == 13
RetentionBoundaryInclusive == 14
ShouldLogTrue == 15
ShouldLogFalse == 16
ThresholdZeroReturnFalse == 17
NoEntryReturnFalse == 18
LastSeenUpdated == 19
InsertPenalty == 20
CountIncrement == 21
CountReset == 22
WindowReset == 23
SuppressedSet == 24
SuppressedCleared == 25
SuppressedPreserved == 26
RecordReturnsTrue == 27
RecordReturnsFalse == 28
IsSuppressedTrue == 29
IsSuppressedFalse == 30
CooldownBoundaryClears == 31
CooldownZeroNoSuppress == 32
WindowBoundaryInclusive == 33
RecordCooldownNoTrigger == 34
PenaltyPruneRetained == 35
PenaltyPruneExpired == 36

Actions == 1..36

SpecActions(candidate) ==
  CASE candidate = InvalidFirstLogs ->
      {InsertEntry, ReturnLogged}
    [] candidate = InvalidWithinWindowThrottles ->
      {PruneRetained, PreserveEntry, ReturnThrottled}
    [] candidate = InvalidAtWindowLogs ->
      {PruneRetained, UpdateEntry, ReturnLogged, ThrottleBoundaryInclusive}
    [] candidate = InvalidHeightAdvanceLogs ->
      {PruneRetained, UpdateEntry, ReturnLogged, HeightAdvance}
    [] candidate = InvalidViewAdvanceLogs ->
      {PruneRetained, UpdateEntry, ReturnLogged, ViewAdvance}
    [] candidate = InvalidDifferentSignerLogs ->
      {PruneRetained, InsertEntry, ReturnLogged, KeySignerSeparated}
    [] candidate = InvalidDifferentKindLogs ->
      {PruneRetained, InsertEntry, ReturnLogged, KeyKindSeparated}
    [] candidate = InvalidRetentionBoundaryKeeps ->
      {PruneRetained, PreserveEntry, ReturnThrottled,
       RetentionBoundaryInclusive}
    [] candidate = InvalidRetentionExpiredPrunes ->
      {PruneExpired, InsertEntry, ReturnLogged}
    [] candidate = RbcFirstLogs ->
      {InsertEntry, ReturnLogged}
    [] candidate = RbcWithinWindowThrottles ->
      {PruneRetained, PreserveEntry, ReturnThrottled}
    [] candidate = RbcAtWindowLogs ->
      {PruneRetained, UpdateEntry, ReturnLogged, ThrottleBoundaryInclusive}
    [] candidate = RbcHeightAdvanceLogs ->
      {PruneRetained, UpdateEntry, ReturnLogged, HeightAdvance}
    [] candidate = RbcViewAdvanceLogs ->
      {PruneRetained, UpdateEntry, ReturnLogged, ViewAdvance}
    [] candidate = RbcDifferentPeerLogs ->
      {PruneRetained, InsertEntry, ReturnLogged, KeyPeerSeparated}
    [] candidate = RbcDifferentKindLogs ->
      {PruneRetained, InsertEntry, ReturnLogged, KeyKindSeparated}
    [] candidate = RbcRetentionBoundaryKeeps ->
      {PruneRetained, PreserveEntry, ReturnThrottled,
       RetentionBoundaryInclusive}
    [] candidate = RbcRetentionExpiredPrunes ->
      {PruneExpired, InsertEntry, ReturnLogged}
    [] candidate = RbcLoggedShouldLogTrue ->
      {ReturnLogged, ShouldLogTrue}
    [] candidate = RbcThrottledShouldLogFalse ->
      {ReturnThrottled, ShouldLogFalse}
    [] candidate = PenaltyThresholdZeroDisabled ->
      {ThresholdZeroReturnFalse, RecordReturnsFalse, IsSuppressedFalse}
    [] candidate = PenaltyNoEntryNotSuppressed ->
      {PenaltyPruneRetained, NoEntryReturnFalse, IsSuppressedFalse}
    [] candidate = PenaltyFirstBelowThreshold ->
      {PenaltyPruneRetained, InsertPenalty, LastSeenUpdated,
       CountIncrement, RecordReturnsFalse}
    [] candidate = PenaltyThresholdTriggers ->
      {PenaltyPruneRetained, LastSeenUpdated, CountIncrement, CountReset,
       WindowReset, SuppressedSet, RecordReturnsTrue}
    [] candidate = PenaltyWithinCooldownSuppressed ->
      {PenaltyPruneRetained, LastSeenUpdated, SuppressedPreserved,
       IsSuppressedTrue}
    [] candidate = PenaltyCooldownExpiryClears ->
      {PenaltyPruneRetained, LastSeenUpdated, SuppressedCleared, CountReset,
       WindowReset, IsSuppressedFalse, CooldownBoundaryClears}
    [] candidate = PenaltyRecordDuringCooldownNoTrigger ->
      {PenaltyPruneRetained, LastSeenUpdated, SuppressedPreserved,
       RecordReturnsFalse, RecordCooldownNoTrigger}
    [] candidate = PenaltyWindowBoundaryKeepsCount ->
      {PenaltyPruneRetained, LastSeenUpdated, CountIncrement,
       RecordReturnsFalse, WindowBoundaryInclusive}
    [] candidate = PenaltyWindowExpiredResets ->
      {PenaltyPruneRetained, LastSeenUpdated, WindowReset, CountReset,
       CountIncrement, RecordReturnsFalse}
    [] candidate = PenaltyZeroCooldownNoSuppress ->
      {PenaltyPruneRetained, LastSeenUpdated, CountIncrement, CountReset,
       WindowReset, RecordReturnsTrue, CooldownZeroNoSuppress}
    [] candidate = PenaltyPruneBoundaryKeeps ->
      {PenaltyPruneRetained, RetentionBoundaryInclusive}
    [] candidate = PenaltyPruneExpiredDrops ->
      {PenaltyPruneExpired}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = InvalidFirstLogs /\ Bug = "invalid_first_throttled" ->
      (spec \ {InsertEntry, ReturnLogged}) \cup
        {PreserveEntry, ReturnThrottled}
    [] candidate = InvalidWithinWindowThrottles /\
          Bug = "invalid_within_window_logs" ->
      (spec \ {PreserveEntry, ReturnThrottled}) \cup
        {UpdateEntry, ReturnLogged}
    [] candidate = InvalidAtWindowLogs /\
          Bug = "invalid_boundary_throttled" ->
      (spec \ {UpdateEntry, ReturnLogged, ThrottleBoundaryInclusive}) \cup
        {PreserveEntry, ReturnThrottled}
    [] candidate = InvalidHeightAdvanceLogs /\
          Bug = "invalid_height_advance_throttled" ->
      (spec \ {UpdateEntry, ReturnLogged, HeightAdvance}) \cup
        {PreserveEntry, ReturnThrottled}
    [] candidate = InvalidViewAdvanceLogs /\
          Bug = "invalid_view_advance_throttled" ->
      (spec \ {UpdateEntry, ReturnLogged, ViewAdvance}) \cup
        {PreserveEntry, ReturnThrottled}
    [] candidate = InvalidDifferentSignerLogs /\
          Bug = "invalid_uses_signer_only_key" ->
      (spec \ {InsertEntry, ReturnLogged, KeySignerSeparated}) \cup
        {PreserveEntry, ReturnThrottled}
    [] candidate = InvalidDifferentKindLogs /\
          Bug = "invalid_uses_kind_only_key" ->
      (spec \ {InsertEntry, ReturnLogged, KeyKindSeparated}) \cup
        {PreserveEntry, ReturnThrottled}
    [] candidate = InvalidRetentionBoundaryKeeps /\
          Bug = "invalid_retention_boundary_pruned" ->
      (spec \ {PruneRetained, PreserveEntry, ReturnThrottled,
               RetentionBoundaryInclusive}) \cup
        {PruneExpired, InsertEntry, ReturnLogged}
    [] candidate = InvalidRetentionExpiredPrunes /\
          Bug = "invalid_expired_retained" ->
      (spec \ {PruneExpired, InsertEntry, ReturnLogged}) \cup
        {PruneRetained, PreserveEntry, ReturnThrottled}
    [] candidate = RbcFirstLogs /\ Bug = "rbc_first_throttled" ->
      (spec \ {InsertEntry, ReturnLogged}) \cup
        {PreserveEntry, ReturnThrottled}
    [] candidate = RbcWithinWindowThrottles /\
          Bug = "rbc_within_window_logs" ->
      (spec \ {PreserveEntry, ReturnThrottled}) \cup
        {UpdateEntry, ReturnLogged}
    [] candidate = RbcAtWindowLogs /\ Bug = "rbc_boundary_throttled" ->
      (spec \ {UpdateEntry, ReturnLogged, ThrottleBoundaryInclusive}) \cup
        {PreserveEntry, ReturnThrottled}
    [] candidate = RbcHeightAdvanceLogs /\
          Bug = "rbc_height_advance_throttled" ->
      (spec \ {UpdateEntry, ReturnLogged, HeightAdvance}) \cup
        {PreserveEntry, ReturnThrottled}
    [] candidate = RbcViewAdvanceLogs /\
          Bug = "rbc_view_advance_throttled" ->
      (spec \ {UpdateEntry, ReturnLogged, ViewAdvance}) \cup
        {PreserveEntry, ReturnThrottled}
    [] candidate = RbcDifferentPeerLogs /\
          Bug = "rbc_uses_peer_only_key" ->
      (spec \ {InsertEntry, ReturnLogged, KeyPeerSeparated}) \cup
        {PreserveEntry, ReturnThrottled}
    [] candidate = RbcDifferentKindLogs /\
          Bug = "rbc_uses_kind_only_key" ->
      (spec \ {InsertEntry, ReturnLogged, KeyKindSeparated}) \cup
        {PreserveEntry, ReturnThrottled}
    [] candidate = RbcRetentionBoundaryKeeps /\
          Bug = "rbc_retention_boundary_pruned" ->
      (spec \ {PruneRetained, PreserveEntry, ReturnThrottled,
               RetentionBoundaryInclusive}) \cup
        {PruneExpired, InsertEntry, ReturnLogged}
    [] candidate = RbcRetentionExpiredPrunes /\
          Bug = "rbc_expired_retained" ->
      (spec \ {PruneExpired, InsertEntry, ReturnLogged}) \cup
        {PruneRetained, PreserveEntry, ReturnThrottled}
    [] candidate = RbcLoggedShouldLogTrue /\
          Bug = "rbc_logged_should_log_false" ->
      (spec \ {ShouldLogTrue}) \cup {ShouldLogFalse}
    [] candidate = RbcThrottledShouldLogFalse /\
          Bug = "rbc_throttled_should_log_true" ->
      (spec \ {ShouldLogFalse}) \cup {ShouldLogTrue}
    [] candidate = PenaltyThresholdZeroDisabled /\
          Bug = "penalty_threshold_zero_active" ->
      (spec \ {ThresholdZeroReturnFalse, RecordReturnsFalse,
               IsSuppressedFalse}) \cup
        {InsertPenalty, CountIncrement, RecordReturnsTrue, IsSuppressedTrue}
    [] candidate = PenaltyNoEntryNotSuppressed /\
          Bug = "penalty_no_entry_suppressed" ->
      (spec \ {NoEntryReturnFalse, IsSuppressedFalse}) \cup
        {IsSuppressedTrue}
    [] candidate = PenaltyFirstBelowThreshold /\
          Bug = "penalty_first_triggers" ->
      (spec \ {RecordReturnsFalse}) \cup
        {CountReset, WindowReset, SuppressedSet, RecordReturnsTrue}
    [] candidate = PenaltyThresholdTriggers /\
          Bug = "penalty_threshold_not_triggered" ->
      (spec \ {CountReset, WindowReset, SuppressedSet,
               RecordReturnsTrue}) \cup
        {RecordReturnsFalse}
    [] candidate = PenaltyWithinCooldownSuppressed /\
          Bug = "penalty_within_cooldown_allowed" ->
      (spec \ {SuppressedPreserved, IsSuppressedTrue}) \cup
        {SuppressedCleared, IsSuppressedFalse}
    [] candidate = PenaltyCooldownExpiryClears /\
          Bug = "penalty_cooldown_expiry_keeps_suppressed" ->
      (spec \ {SuppressedCleared, CountReset, WindowReset,
               IsSuppressedFalse, CooldownBoundaryClears}) \cup
        {SuppressedPreserved, IsSuppressedTrue}
    [] candidate = PenaltyRecordDuringCooldownNoTrigger /\
          Bug = "penalty_record_during_cooldown_retriggers" ->
      (spec \ {SuppressedPreserved, RecordReturnsFalse,
               RecordCooldownNoTrigger}) \cup
        {CountReset, WindowReset, SuppressedSet, RecordReturnsTrue}
    [] candidate = PenaltyWindowBoundaryKeepsCount /\
          Bug = "penalty_window_boundary_resets" ->
      (spec \ {CountIncrement, WindowBoundaryInclusive}) \cup
        {WindowReset, CountReset}
    [] candidate = PenaltyWindowExpiredResets /\
          Bug = "penalty_window_expired_keeps_count" ->
      (spec \ {WindowReset, CountReset}) \cup {SuppressedSet}
    [] candidate = PenaltyZeroCooldownNoSuppress /\
          Bug = "penalty_zero_cooldown_suppresses" ->
      (spec \ {CooldownZeroNoSuppress}) \cup {SuppressedSet}
    [] candidate = PenaltyPruneBoundaryKeeps /\
          Bug = "penalty_prune_boundary_drops" ->
      (spec \ {PenaltyPruneRetained, RetentionBoundaryInclusive}) \cup
        {PenaltyPruneExpired}
    [] candidate = PenaltyPruneExpiredDrops /\
          Bug = "penalty_prune_expired_keeps" ->
      (spec \ {PenaltyPruneExpired}) \cup {PenaltyPruneRetained}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "invalid_first_throttled",
       "invalid_within_window_logs",
       "invalid_boundary_throttled",
       "invalid_height_advance_throttled",
       "invalid_view_advance_throttled",
       "invalid_uses_signer_only_key",
       "invalid_uses_kind_only_key",
       "invalid_retention_boundary_pruned",
       "invalid_expired_retained",
       "rbc_first_throttled",
       "rbc_within_window_logs",
       "rbc_boundary_throttled",
       "rbc_height_advance_throttled",
       "rbc_view_advance_throttled",
       "rbc_uses_peer_only_key",
       "rbc_uses_kind_only_key",
       "rbc_retention_boundary_pruned",
       "rbc_expired_retained",
       "rbc_logged_should_log_false",
       "rbc_throttled_should_log_true",
       "penalty_threshold_zero_active",
       "penalty_no_entry_suppressed",
       "penalty_first_triggers",
       "penalty_threshold_not_triggered",
       "penalty_within_cooldown_allowed",
       "penalty_cooldown_expiry_keeps_suppressed",
       "penalty_record_during_cooldown_retriggers",
       "penalty_window_boundary_resets",
       "penalty_window_expired_keeps_count",
       "penalty_zero_cooldown_suppresses",
       "penalty_prune_boundary_drops",
       "penalty_prune_expired_keeps"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

InvalidThrottleActionsMatchSpec ==
  \A c \in InvalidFirstLogs..InvalidRetentionExpiredPrunes:
    ImplementationActions(c) = SpecActions(c)

RbcThrottleActionsMatchSpec ==
  \A c \in RbcFirstLogs..RbcThrottledShouldLogFalse:
    ImplementationActions(c) = SpecActions(c)

PenaltyActionsMatchSpec ==
  \A c \in PenaltyThresholdZeroDisabled..PenaltyPruneExpiredDrops:
    ImplementationActions(c) = SpecActions(c)

InvalidThrottleKeyAndBoundaryAnchors ==
  /\ ReturnLogged \in ImplementationActions(InvalidFirstLogs)
  /\ ReturnThrottled \in ImplementationActions(InvalidWithinWindowThrottles)
  /\ ThrottleBoundaryInclusive \in ImplementationActions(InvalidAtWindowLogs)
  /\ HeightAdvance \in ImplementationActions(InvalidHeightAdvanceLogs)
  /\ ViewAdvance \in ImplementationActions(InvalidViewAdvanceLogs)
  /\ KeySignerSeparated \in ImplementationActions(InvalidDifferentSignerLogs)
  /\ KeyKindSeparated \in ImplementationActions(InvalidDifferentKindLogs)
  /\ RetentionBoundaryInclusive \in
       ImplementationActions(InvalidRetentionBoundaryKeeps)
  /\ PruneExpired \in ImplementationActions(InvalidRetentionExpiredPrunes)

RbcThrottleKeyBoundaryAndOutcomeAnchors ==
  /\ ReturnLogged \in ImplementationActions(RbcFirstLogs)
  /\ ReturnThrottled \in ImplementationActions(RbcWithinWindowThrottles)
  /\ ThrottleBoundaryInclusive \in ImplementationActions(RbcAtWindowLogs)
  /\ HeightAdvance \in ImplementationActions(RbcHeightAdvanceLogs)
  /\ ViewAdvance \in ImplementationActions(RbcViewAdvanceLogs)
  /\ KeyPeerSeparated \in ImplementationActions(RbcDifferentPeerLogs)
  /\ KeyKindSeparated \in ImplementationActions(RbcDifferentKindLogs)
  /\ RetentionBoundaryInclusive \in
       ImplementationActions(RbcRetentionBoundaryKeeps)
  /\ PruneExpired \in ImplementationActions(RbcRetentionExpiredPrunes)
  /\ ShouldLogTrue \in ImplementationActions(RbcLoggedShouldLogTrue)
  /\ ShouldLogFalse \in ImplementationActions(RbcThrottledShouldLogFalse)

PenaltyThresholdCooldownAndPruneAnchors ==
  /\ ThresholdZeroReturnFalse \in
       ImplementationActions(PenaltyThresholdZeroDisabled)
  /\ NoEntryReturnFalse \in ImplementationActions(PenaltyNoEntryNotSuppressed)
  /\ RecordReturnsFalse \in ImplementationActions(PenaltyFirstBelowThreshold)
  /\ RecordReturnsTrue \in ImplementationActions(PenaltyThresholdTriggers)
  /\ IsSuppressedTrue \in
       ImplementationActions(PenaltyWithinCooldownSuppressed)
  /\ CooldownBoundaryClears \in
       ImplementationActions(PenaltyCooldownExpiryClears)
  /\ RecordCooldownNoTrigger \in
       ImplementationActions(PenaltyRecordDuringCooldownNoTrigger)
  /\ WindowBoundaryInclusive \in
       ImplementationActions(PenaltyWindowBoundaryKeepsCount)
  /\ WindowReset \in ImplementationActions(PenaltyWindowExpiredResets)
  /\ CooldownZeroNoSuppress \in
       ImplementationActions(PenaltyZeroCooldownNoSuppress)
  /\ PenaltyPruneRetained \in ImplementationActions(PenaltyPruneBoundaryKeeps)
  /\ PenaltyPruneExpired \in ImplementationActions(PenaltyPruneExpiredDrops)

InvalidSignatureThrottleSafetyAnchors ==
  /\ InvalidThrottleActionsMatchSpec
  /\ RbcThrottleActionsMatchSpec
  /\ PenaltyActionsMatchSpec
  /\ InvalidThrottleKeyAndBoundaryAnchors
  /\ RbcThrottleKeyBoundaryAndOutcomeAnchors
  /\ PenaltyThresholdCooldownAndPruneAnchors

InvalidSignatureThrottleExactness ==
  /\ \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)
  /\ InvalidSignatureThrottleSafetyAnchors

InvalidSignatureThrottleCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ InvalidSignatureThrottleExactness

BugInvalidFirstThrottled ==
  ImplementationActions(InvalidFirstLogs) = SpecActions(InvalidFirstLogs)

BugInvalidWithinWindowLogs ==
  ImplementationActions(InvalidWithinWindowThrottles) =
    SpecActions(InvalidWithinWindowThrottles)

BugInvalidBoundaryThrottled ==
  ImplementationActions(InvalidAtWindowLogs) = SpecActions(InvalidAtWindowLogs)

BugInvalidHeightAdvanceThrottled ==
  ImplementationActions(InvalidHeightAdvanceLogs) =
    SpecActions(InvalidHeightAdvanceLogs)

BugInvalidViewAdvanceThrottled ==
  ImplementationActions(InvalidViewAdvanceLogs) =
    SpecActions(InvalidViewAdvanceLogs)

BugInvalidUsesSignerOnlyKey ==
  ImplementationActions(InvalidDifferentSignerLogs) =
    SpecActions(InvalidDifferentSignerLogs)

BugInvalidUsesKindOnlyKey ==
  ImplementationActions(InvalidDifferentKindLogs) =
    SpecActions(InvalidDifferentKindLogs)

BugInvalidRetentionBoundaryPruned ==
  ImplementationActions(InvalidRetentionBoundaryKeeps) =
    SpecActions(InvalidRetentionBoundaryKeeps)

BugInvalidExpiredRetained ==
  ImplementationActions(InvalidRetentionExpiredPrunes) =
    SpecActions(InvalidRetentionExpiredPrunes)

BugRbcFirstThrottled ==
  ImplementationActions(RbcFirstLogs) = SpecActions(RbcFirstLogs)

BugRbcWithinWindowLogs ==
  ImplementationActions(RbcWithinWindowThrottles) =
    SpecActions(RbcWithinWindowThrottles)

BugRbcBoundaryThrottled ==
  ImplementationActions(RbcAtWindowLogs) = SpecActions(RbcAtWindowLogs)

BugRbcHeightAdvanceThrottled ==
  ImplementationActions(RbcHeightAdvanceLogs) = SpecActions(RbcHeightAdvanceLogs)

BugRbcViewAdvanceThrottled ==
  ImplementationActions(RbcViewAdvanceLogs) = SpecActions(RbcViewAdvanceLogs)

BugRbcUsesPeerOnlyKey ==
  ImplementationActions(RbcDifferentPeerLogs) =
    SpecActions(RbcDifferentPeerLogs)

BugRbcUsesKindOnlyKey ==
  ImplementationActions(RbcDifferentKindLogs) =
    SpecActions(RbcDifferentKindLogs)

BugRbcRetentionBoundaryPruned ==
  ImplementationActions(RbcRetentionBoundaryKeeps) =
    SpecActions(RbcRetentionBoundaryKeeps)

BugRbcExpiredRetained ==
  ImplementationActions(RbcRetentionExpiredPrunes) =
    SpecActions(RbcRetentionExpiredPrunes)

BugRbcLoggedShouldLogFalse ==
  ImplementationActions(RbcLoggedShouldLogTrue) =
    SpecActions(RbcLoggedShouldLogTrue)

BugRbcThrottledShouldLogTrue ==
  ImplementationActions(RbcThrottledShouldLogFalse) =
    SpecActions(RbcThrottledShouldLogFalse)

BugPenaltyThresholdZeroActive ==
  ImplementationActions(PenaltyThresholdZeroDisabled) =
    SpecActions(PenaltyThresholdZeroDisabled)

BugPenaltyNoEntrySuppressed ==
  ImplementationActions(PenaltyNoEntryNotSuppressed) =
    SpecActions(PenaltyNoEntryNotSuppressed)

BugPenaltyFirstTriggers ==
  ImplementationActions(PenaltyFirstBelowThreshold) =
    SpecActions(PenaltyFirstBelowThreshold)

BugPenaltyThresholdNotTriggered ==
  ImplementationActions(PenaltyThresholdTriggers) =
    SpecActions(PenaltyThresholdTriggers)

BugPenaltyWithinCooldownAllowed ==
  ImplementationActions(PenaltyWithinCooldownSuppressed) =
    SpecActions(PenaltyWithinCooldownSuppressed)

BugPenaltyCooldownExpiryKeepsSuppressed ==
  ImplementationActions(PenaltyCooldownExpiryClears) =
    SpecActions(PenaltyCooldownExpiryClears)

BugPenaltyRecordDuringCooldownRetriggers ==
  ImplementationActions(PenaltyRecordDuringCooldownNoTrigger) =
    SpecActions(PenaltyRecordDuringCooldownNoTrigger)

BugPenaltyWindowBoundaryResets ==
  ImplementationActions(PenaltyWindowBoundaryKeepsCount) =
    SpecActions(PenaltyWindowBoundaryKeepsCount)

BugPenaltyWindowExpiredKeepsCount ==
  ImplementationActions(PenaltyWindowExpiredResets) =
    SpecActions(PenaltyWindowExpiredResets)

BugPenaltyZeroCooldownSuppresses ==
  ImplementationActions(PenaltyZeroCooldownNoSuppress) =
    SpecActions(PenaltyZeroCooldownNoSuppress)

BugPenaltyPruneBoundaryDrops ==
  ImplementationActions(PenaltyPruneBoundaryKeeps) =
    SpecActions(PenaltyPruneBoundaryKeeps)

BugPenaltyPruneExpiredKeeps ==
  ImplementationActions(PenaltyPruneExpiredDrops) =
    SpecActions(PenaltyPruneExpiredDrops)

====
