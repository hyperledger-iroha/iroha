---- MODULE SumeragiRbcMissingBlockRecoveryGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for RBC-specific missing-block recovery helpers.

This slice captures the decisions around:
- `rbc_session_needs_block_created_recovery_from_session(...)`;
- `rbc_session_should_force_frontier_authoritative_body_fetch(...)`;
- `pending_rbc_missing_block_fetch_mode(...)`; and
- `suppress_far_future_rbc_missing_block_fetch(...)`.

The generic missing-block fetch planner is modeled separately. This gate pins
the RBC-specific admission and escalation boundary before that planner runs:
locally known blocks bypass recovery, incomplete/invalid RBC metadata requires
BlockCreated recovery, exact-frontier payload gaps prefer body repair, signer
targeting stays strict until the configured fallback attempt threshold, and
far-future RBC dependencies are retained as RBC state while generic
missing-block requests are cleared.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NeedsKnownLocal == "needs_known_local"
NeedsKnownLocalInvalid == "needs_known_local_invalid"
NeedsMissingSession == "needs_missing_session"
NeedsCompleteSession == "needs_complete_session"
NeedsMissingPayloadHash == "needs_missing_payload_hash"
NeedsMissingHeader == "needs_missing_header"
NeedsMissingLeaderSignature == "needs_missing_leader_signature"
NeedsInvalidSession == "needs_invalid_session"

NeedsCases == {
  NeedsKnownLocal,
  NeedsKnownLocalInvalid,
  NeedsMissingSession,
  NeedsCompleteSession,
  NeedsMissingPayloadHash,
  NeedsMissingHeader,
  NeedsMissingLeaderSignature,
  NeedsInvalidSession
}

SpecNeedsRecovery(c) ==
  CASE c \in {NeedsKnownLocal, NeedsKnownLocalInvalid} -> FALSE
    [] c = NeedsCompleteSession -> FALSE
    [] c \in {
         NeedsMissingSession,
         NeedsMissingPayloadHash,
         NeedsMissingHeader,
         NeedsMissingLeaderSignature,
         NeedsInvalidSession
       } -> TRUE

ImplementationNeedsRecovery(c) ==
  CASE Bug = "needs_known_local_fetches"
       /\ c \in {NeedsKnownLocal, NeedsKnownLocalInvalid} -> TRUE
    [] Bug = "needs_missing_session_skips"
       /\ c = NeedsMissingSession -> FALSE
    [] Bug = "needs_payload_hash_skips"
       /\ c = NeedsMissingPayloadHash -> FALSE
    [] Bug = "needs_header_skips"
       /\ c = NeedsMissingHeader -> FALSE
    [] Bug = "needs_signature_skips"
       /\ c = NeedsMissingLeaderSignature -> FALSE
    [] Bug = "needs_invalid_skips"
       /\ c = NeedsInvalidSession -> FALSE
    [] Bug = "needs_complete_fetches"
       /\ c = NeedsCompleteSession -> TRUE
    [] OTHER -> SpecNeedsRecovery(c)

ForceInvalidSession == "force_invalid_session"
ForcePayloadAvailable == "force_payload_available"
ForceExactFrontierGap == "force_exact_frontier_gap"
ForceNextSlotGap == "force_next_slot_gap"
ForceCommittedGap == "force_committed_gap"
ForceFarFutureGap == "force_far_future_gap"
ForceAuthoritativeProgress == "force_authoritative_progress"

ForceCases == {
  ForceInvalidSession,
  ForcePayloadAvailable,
  ForceExactFrontierGap,
  ForceNextSlotGap,
  ForceCommittedGap,
  ForceFarFutureGap,
  ForceAuthoritativeProgress
}

SpecForceFrontierFetch(c) ==
  CASE c \in {
         ForceInvalidSession,
         ForcePayloadAvailable,
         ForceFarFutureGap,
         ForceAuthoritativeProgress
       } -> FALSE
    [] c \in {
         ForceExactFrontierGap,
         ForceNextSlotGap,
         ForceCommittedGap
       } -> TRUE

ImplementationForceFrontierFetch(c) ==
  CASE Bug = "force_invalid_fetches"
       /\ c = ForceInvalidSession -> TRUE
    [] Bug = "force_payload_available_fetches"
       /\ c = ForcePayloadAvailable -> TRUE
    [] Bug = "force_frontier_gap_skips"
       /\ c = ForceExactFrontierGap -> FALSE
    [] Bug = "force_next_slot_skips"
       /\ c = ForceNextSlotGap -> FALSE
    [] Bug = "force_committed_gap_skips"
       /\ c = ForceCommittedGap -> FALSE
    [] Bug = "force_far_future_fetches"
       /\ c = ForceFarFutureGap -> TRUE
    [] Bug = "force_authoritative_progress_fetches"
       /\ c = ForceAuthoritativeProgress -> TRUE
    [] OTHER -> SpecForceFrontierFetch(c)

StrictSigners == "StrictSigners"
DefaultMode == "Default"
AggressiveTopology == "AggressiveTopology"

FetchModes == {StrictSigners, DefaultMode, AggressiveTopology}

FetchNoFallbackAttempts == "fetch_no_fallback_attempts"
FetchNoExistingRequest == "fetch_no_existing_request"
FetchWrongHeight == "fetch_wrong_height"
FetchWrongView == "fetch_wrong_view"
FetchWrongPhase == "fetch_wrong_phase"
FetchAttemptsBelowThreshold == "fetch_attempts_below_threshold"
FetchAttemptsAtThreshold == "fetch_attempts_at_threshold"
FetchAttemptsAboveThreshold == "fetch_attempts_above_threshold"
FetchReasonIgnoredAtThreshold == "fetch_reason_ignored_at_threshold"

FetchCases == {
  FetchNoFallbackAttempts,
  FetchNoExistingRequest,
  FetchWrongHeight,
  FetchWrongView,
  FetchWrongPhase,
  FetchAttemptsBelowThreshold,
  FetchAttemptsAtThreshold,
  FetchAttemptsAboveThreshold,
  FetchReasonIgnoredAtThreshold
}

SpecFetchMode(c) ==
  CASE c \in {
         FetchAttemptsAtThreshold,
         FetchAttemptsAboveThreshold,
         FetchReasonIgnoredAtThreshold
       } -> DefaultMode
    [] OTHER -> StrictSigners

ImplementationFetchMode(c) ==
  CASE Bug = "mode_zero_fallback_defaults"
       /\ c = FetchNoFallbackAttempts -> DefaultMode
    [] Bug = "mode_missing_request_defaults"
       /\ c = FetchNoExistingRequest -> DefaultMode
    [] Bug = "mode_wrong_slot_defaults"
       /\ c \in {FetchWrongHeight, FetchWrongView, FetchWrongPhase} ->
      DefaultMode
    [] Bug = "mode_below_threshold_defaults"
       /\ c = FetchAttemptsBelowThreshold -> DefaultMode
    [] Bug = "mode_at_threshold_strict"
       /\ c = FetchAttemptsAtThreshold -> StrictSigners
    [] Bug = "mode_above_threshold_strict"
       /\ c = FetchAttemptsAboveThreshold -> StrictSigners
    [] Bug = "mode_reason_changes_result"
       /\ c = FetchReasonIgnoredAtThreshold -> StrictSigners
    [] Bug = "mode_uses_aggressive"
       /\ c = FetchAttemptsAtThreshold -> AggressiveTopology
    [] OTHER -> SpecFetchMode(c)

SuppressExactFrontier == "suppress_exact_frontier"
SuppressNextSlot == "suppress_next_slot"
SuppressFarFutureNoTrigger == "suppress_far_future_no_trigger"
SuppressFarFutureExistingRequest == "suppress_far_future_existing_request"
SuppressFarFutureLockLag == "suppress_far_future_lock_lag"
SuppressFarFutureDroppedWindow == "suppress_far_future_dropped_window"
SuppressFarFutureLockLagExisting == "suppress_far_future_lock_lag_existing"

SuppressCases == {
  SuppressExactFrontier,
  SuppressNextSlot,
  SuppressFarFutureNoTrigger,
  SuppressFarFutureExistingRequest,
  SuppressFarFutureLockLag,
  SuppressFarFutureDroppedWindow,
  SuppressFarFutureLockLagExisting
}

FarFutureCases ==
  SuppressCases \ {SuppressExactFrontier, SuppressNextSlot}

SpecSuppressed(c) ==
  c \in FarFutureCases

SpecClearsExistingRequest(c) ==
  c \in {SuppressFarFutureExistingRequest, SuppressFarFutureLockLagExisting}

SpecClearsHeightRecovery(c) ==
  c \in FarFutureCases

SpecRequestsRangePull(c) ==
  c \in {
    SuppressFarFutureLockLag,
    SuppressFarFutureDroppedWindow,
    SuppressFarFutureLockLagExisting
  }

ImplementationSuppressed(c) ==
  CASE Bug = "suppress_near_frontier"
       /\ c = SuppressExactFrontier -> TRUE
    [] Bug = "suppress_next_slot"
       /\ c = SuppressNextSlot -> TRUE
    [] Bug = "far_future_not_suppressed"
       /\ c = SuppressFarFutureNoTrigger -> FALSE
    [] OTHER -> SpecSuppressed(c)

ImplementationClearsExistingRequest(c) ==
  CASE Bug = "far_future_keeps_request"
       /\ c = SuppressFarFutureExistingRequest -> FALSE
    [] OTHER -> SpecClearsExistingRequest(c)

ImplementationClearsHeightRecovery(c) ==
  CASE Bug = "far_future_skips_height_recovery"
       /\ c = SuppressFarFutureNoTrigger -> FALSE
    [] OTHER -> SpecClearsHeightRecovery(c)

ImplementationRequestsRangePull(c) ==
  CASE Bug = "far_future_lock_lag_no_reanchor"
       /\ c \in {SuppressFarFutureLockLag, SuppressFarFutureLockLagExisting} ->
      FALSE
    [] Bug = "far_future_dropped_window_no_reanchor"
       /\ c = SuppressFarFutureDroppedWindow -> FALSE
    [] Bug = "far_future_reanchors_without_trigger"
       /\ c = SuppressFarFutureNoTrigger -> TRUE
    [] OTHER -> SpecRequestsRangePull(c)

Bugs == {
  "none",
  "needs_known_local_fetches",
  "needs_missing_session_skips",
  "needs_payload_hash_skips",
  "needs_header_skips",
  "needs_signature_skips",
  "needs_invalid_skips",
  "needs_complete_fetches",
  "force_invalid_fetches",
  "force_payload_available_fetches",
  "force_frontier_gap_skips",
  "force_next_slot_skips",
  "force_committed_gap_skips",
  "force_far_future_fetches",
  "force_authoritative_progress_fetches",
  "mode_zero_fallback_defaults",
  "mode_missing_request_defaults",
  "mode_wrong_slot_defaults",
  "mode_below_threshold_defaults",
  "mode_at_threshold_strict",
  "mode_above_threshold_strict",
  "mode_reason_changes_result",
  "mode_uses_aggressive",
  "suppress_near_frontier",
  "suppress_next_slot",
  "far_future_not_suppressed",
  "far_future_keeps_request",
  "far_future_skips_height_recovery",
  "far_future_lock_lag_no_reanchor",
  "far_future_dropped_window_no_reanchor",
  "far_future_reanchors_without_trigger"
}

Init ==
  checked = 0

Next ==
  \/ /\ checked < 30
     /\ checked' = checked + 1
  \/ /\ checked = 30
     /\ checked' = checked

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..30
  /\ \A c \in NeedsCases:
       /\ SpecNeedsRecovery(c) \in BOOLEAN
       /\ ImplementationNeedsRecovery(c) \in BOOLEAN
  /\ \A c \in ForceCases:
       /\ SpecForceFrontierFetch(c) \in BOOLEAN
       /\ ImplementationForceFrontierFetch(c) \in BOOLEAN
  /\ \A c \in FetchCases:
       /\ SpecFetchMode(c) \in FetchModes
       /\ ImplementationFetchMode(c) \in FetchModes
  /\ \A c \in SuppressCases:
       /\ SpecSuppressed(c) \in BOOLEAN
       /\ SpecClearsExistingRequest(c) \in BOOLEAN
       /\ SpecClearsHeightRecovery(c) \in BOOLEAN
       /\ SpecRequestsRangePull(c) \in BOOLEAN
       /\ ImplementationSuppressed(c) \in BOOLEAN
       /\ ImplementationClearsExistingRequest(c) \in BOOLEAN
       /\ ImplementationClearsHeightRecovery(c) \in BOOLEAN
       /\ ImplementationRequestsRangePull(c) \in BOOLEAN

RbcMissingBlockRecoveryMatchesSpec ==
  /\ \A c \in NeedsCases:
       ImplementationNeedsRecovery(c) = SpecNeedsRecovery(c)
  /\ \A c \in ForceCases:
       ImplementationForceFrontierFetch(c) = SpecForceFrontierFetch(c)
  /\ \A c \in FetchCases:
       ImplementationFetchMode(c) = SpecFetchMode(c)
  /\ \A c \in SuppressCases:
       /\ ImplementationSuppressed(c) = SpecSuppressed(c)
       /\ ImplementationClearsExistingRequest(c) =
            SpecClearsExistingRequest(c)
       /\ ImplementationClearsHeightRecovery(c) = SpecClearsHeightRecovery(c)
       /\ ImplementationRequestsRangePull(c) = SpecRequestsRangePull(c)

SafetyFast ==
  RbcMissingBlockRecoveryMatchesSpec

AllNeedsRecoveryMatches ==
  \A c \in NeedsCases:
    ImplementationNeedsRecovery(c) = SpecNeedsRecovery(c)

AllForceFrontierFetchMatches ==
  \A c \in ForceCases:
    ImplementationForceFrontierFetch(c) = SpecForceFrontierFetch(c)

AllFetchModesMatch ==
  \A c \in FetchCases:
    ImplementationFetchMode(c) = SpecFetchMode(c)

AllSuppressionMatches ==
  \A c \in SuppressCases:
    /\ ImplementationSuppressed(c) = SpecSuppressed(c)
    /\ ImplementationClearsExistingRequest(c) = SpecClearsExistingRequest(c)
    /\ ImplementationClearsHeightRecovery(c) = SpecClearsHeightRecovery(c)
    /\ ImplementationRequestsRangePull(c) = SpecRequestsRangePull(c)

NeedsRecoveryAnchors ==
  /\ ~ImplementationNeedsRecovery(NeedsKnownLocal)
  /\ ~ImplementationNeedsRecovery(NeedsKnownLocalInvalid)
  /\ ~ImplementationNeedsRecovery(NeedsCompleteSession)
  /\ ImplementationNeedsRecovery(NeedsMissingSession)
  /\ ImplementationNeedsRecovery(NeedsMissingPayloadHash)
  /\ ImplementationNeedsRecovery(NeedsMissingHeader)
  /\ ImplementationNeedsRecovery(NeedsMissingLeaderSignature)
  /\ ImplementationNeedsRecovery(NeedsInvalidSession)

ForceFrontierFetchAnchors ==
  /\ ~ImplementationForceFrontierFetch(ForceInvalidSession)
  /\ ~ImplementationForceFrontierFetch(ForcePayloadAvailable)
  /\ ImplementationForceFrontierFetch(ForceExactFrontierGap)
  /\ ImplementationForceFrontierFetch(ForceNextSlotGap)
  /\ ImplementationForceFrontierFetch(ForceCommittedGap)
  /\ ~ImplementationForceFrontierFetch(ForceFarFutureGap)
  /\ ~ImplementationForceFrontierFetch(ForceAuthoritativeProgress)

FetchModeAnchors ==
  /\ ImplementationFetchMode(FetchNoFallbackAttempts) = StrictSigners
  /\ ImplementationFetchMode(FetchNoExistingRequest) = StrictSigners
  /\ ImplementationFetchMode(FetchWrongHeight) = StrictSigners
  /\ ImplementationFetchMode(FetchWrongView) = StrictSigners
  /\ ImplementationFetchMode(FetchWrongPhase) = StrictSigners
  /\ ImplementationFetchMode(FetchAttemptsBelowThreshold) = StrictSigners
  /\ ImplementationFetchMode(FetchAttemptsAtThreshold) = DefaultMode
  /\ ImplementationFetchMode(FetchAttemptsAboveThreshold) = DefaultMode
  /\ ImplementationFetchMode(FetchReasonIgnoredAtThreshold) = DefaultMode

SuppressionDecisionAnchors ==
  /\ ~ImplementationSuppressed(SuppressExactFrontier)
  /\ ~ImplementationSuppressed(SuppressNextSlot)
  /\ ImplementationSuppressed(SuppressFarFutureNoTrigger)
  /\ ImplementationSuppressed(SuppressFarFutureExistingRequest)
  /\ ImplementationSuppressed(SuppressFarFutureLockLag)
  /\ ImplementationSuppressed(SuppressFarFutureDroppedWindow)
  /\ ImplementationSuppressed(SuppressFarFutureLockLagExisting)

SuppressionCleanupAnchors ==
  /\ ~ImplementationClearsExistingRequest(SuppressFarFutureNoTrigger)
  /\ ImplementationClearsExistingRequest(SuppressFarFutureExistingRequest)
  /\ ~ImplementationClearsExistingRequest(SuppressFarFutureLockLag)
  /\ ~ImplementationClearsExistingRequest(SuppressFarFutureDroppedWindow)
  /\ ImplementationClearsExistingRequest(SuppressFarFutureLockLagExisting)
  /\ ImplementationClearsHeightRecovery(SuppressFarFutureNoTrigger)
  /\ ImplementationClearsHeightRecovery(SuppressFarFutureExistingRequest)
  /\ ImplementationClearsHeightRecovery(SuppressFarFutureLockLag)
  /\ ImplementationClearsHeightRecovery(SuppressFarFutureDroppedWindow)
  /\ ImplementationClearsHeightRecovery(SuppressFarFutureLockLagExisting)

SuppressionRangePullAnchors ==
  /\ ~ImplementationRequestsRangePull(SuppressFarFutureNoTrigger)
  /\ ~ImplementationRequestsRangePull(SuppressFarFutureExistingRequest)
  /\ ImplementationRequestsRangePull(SuppressFarFutureLockLag)
  /\ ImplementationRequestsRangePull(SuppressFarFutureDroppedWindow)
  /\ ImplementationRequestsRangePull(SuppressFarFutureLockLagExisting)

MissingBlockRecoverySafetyAnchors ==
  /\ AllNeedsRecoveryMatches
  /\ AllForceFrontierFetchMatches
  /\ AllFetchModesMatch
  /\ AllSuppressionMatches
  /\ NeedsRecoveryAnchors
  /\ ForceFrontierFetchAnchors
  /\ FetchModeAnchors
  /\ SuppressionDecisionAnchors
  /\ SuppressionCleanupAnchors
  /\ SuppressionRangePullAnchors

====
