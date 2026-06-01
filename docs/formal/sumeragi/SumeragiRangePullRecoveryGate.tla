---- MODULE SumeragiRangePullRecoveryGate ----
EXTENDS Naturals, Sequences

(***************************************************************************
A bounded abstract model for range-pull recovery helper decisions.

This slice pins the helper contract around:
- `transition_for_missing_block_stage_observation(...)`;
- `step_missing_block_recovery_stage(...)`;
- `RangePullCandidateTier::{advance,label}`;
- `range_pull_targets_for_height_tier(...)`;
- `range_pull_targets_for_height(...)`;
- `allow_qc_missing_payload_range_pull(...)`; and
- `range_pull_anchor_hashes(...)` plus the reason classifiers that feed it.

The model keeps peer sets, times, and reasons finite while preserving the
observable recovery decisions: dependency progress only advances an existing
range-pull stage to apply-and-revalidate, target selection falls back from live
vote roster to commit topology to trusted peers only for automatic selection,
targets are local-filtered/sorted/deduplicated, QC-missing-payload range pulls
deduplicate by exact slot/hash until the cooldown boundary, and committed-anchor
selection uses the previous committed block only for explicit reanchor reasons
with a previous hash available.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

HashFetch == "HashFetch"
ParentFetch == "ParentFetch"
RangePull == "RangePullFromAnchor"
ApplyRevalidate == "ApplyAndRevalidate"

Stages == {HashFetch, ParentFetch, RangePull, ApplyRevalidate}

Keep == "Keep"
HashAttempt == "HashFetchAttempt"
ParentAttempt == "ParentFetchAttempt"
RangeRequested == "RangePullRequested"
DependencyProgress == "DependencyProgressObserved"

Transitions == {Keep, HashAttempt, ParentAttempt, RangeRequested, DependencyProgress}

SpecObservationTransition(stage) ==
  CASE stage = HashFetch -> HashAttempt
    [] stage = ParentFetch -> ParentAttempt
    [] stage = RangePull -> RangeRequested
    [] stage = ApplyRevalidate -> Keep
    [] OTHER -> Keep

ActualObservationTransition(stage) ==
  CASE Bug = "obs_apply_progress" /\ stage = ApplyRevalidate -> DependencyProgress
    [] Bug = "obs_range_keep" /\ stage = RangePull -> Keep
    [] Bug = "obs_hash_parent" /\ stage = HashFetch -> ParentAttempt
    [] Bug = "obs_parent_hash" /\ stage = ParentFetch -> HashAttempt
    [] OTHER -> SpecObservationTransition(stage)

SpecStep(stage, transition) ==
  CASE transition = Keep -> stage
    [] transition = HashAttempt -> HashFetch
    [] transition = ParentAttempt -> ParentFetch
    [] transition = RangeRequested -> RangePull
    [] transition = DependencyProgress /\ stage = RangePull -> ApplyRevalidate
    [] transition = DependencyProgress /\ stage = ApplyRevalidate -> ApplyRevalidate
    [] transition = DependencyProgress -> stage
    [] OTHER -> stage

ActualStep(stage, transition) ==
  CASE Bug = "stage_keep_resets" /\ transition = Keep -> HashFetch
    [] Bug = "stage_hash_ignored" /\ transition = HashAttempt -> stage
    [] Bug = "stage_parent_ignored" /\ transition = ParentAttempt -> stage
    [] Bug = "stage_range_ignored" /\ transition = RangeRequested -> stage
    [] Bug = "stage_dep_from_range_ignored"
       /\ stage = RangePull /\ transition = DependencyProgress -> RangePull
    [] Bug = "stage_dep_from_hash_advances"
       /\ stage = HashFetch /\ transition = DependencyProgress -> ApplyRevalidate
    [] Bug = "stage_dep_from_parent_advances"
       /\ stage = ParentFetch /\ transition = DependencyProgress -> ApplyRevalidate
    [] Bug = "stage_dep_from_apply_regresses"
       /\ stage = ApplyRevalidate /\ transition = DependencyProgress -> RangePull
    [] OTHER -> SpecStep(stage, transition)

VoteRoster == "VoteRoster"
CommitTopology == "CommitTopology"
TrustedPeers == "TrustedPeers"
NoTier == "NoTier"

Tiers == {VoteRoster, CommitTopology, TrustedPeers}

SpecAdvance(tier) ==
  CASE tier = VoteRoster -> CommitTopology
    [] tier = CommitTopology -> TrustedPeers
    [] tier = TrustedPeers -> NoTier
    [] OTHER -> NoTier

ActualAdvance(tier) ==
  CASE Bug = "tier_vote_skips_commit" /\ tier = VoteRoster -> TrustedPeers
    [] Bug = "tier_commit_stops" /\ tier = CommitTopology -> NoTier
    [] Bug = "tier_trusted_wraps" /\ tier = TrustedPeers -> VoteRoster
    [] OTHER -> SpecAdvance(tier)

SpecLabel(tier) ==
  CASE tier = VoteRoster -> "vote_roster"
    [] tier = CommitTopology -> "commit_topology"
    [] tier = TrustedPeers -> "trusted_peers"
    [] OTHER -> "unknown"

ActualLabel(tier) ==
  CASE Bug = "label_commit_wrong" /\ tier = CommitTopology -> "vote_roster"
    [] Bug = "label_trusted_wrong" /\ tier = TrustedPeers -> "commit_topology"
    [] OTHER -> SpecLabel(tier)

AutoVote == "auto_vote"
AutoCommitFallback == "auto_commit_fallback"
AutoTrustedFallback == "auto_trusted_fallback"
AutoEmpty == "auto_empty"
TierCommit == "tier_commit"
TierTrusted == "tier_trusted"
TierVoteEmpty == "tier_vote_empty"

TargetCases == {
  AutoVote,
  AutoCommitFallback,
  AutoTrustedFallback,
  AutoEmpty,
  TierCommit,
  TierTrusted,
  TierVoteEmpty
}

SpecTargets(case) ==
  CASE case = AutoVote -> <<1, 2, 3>>
    [] case = AutoCommitFallback -> <<1, 4>>
    [] case = AutoTrustedFallback -> <<2, 5>>
    [] case = AutoEmpty -> <<>>
    [] case = TierCommit -> <<4, 5>>
    [] case = TierTrusted -> <<2, 6>>
    [] case = TierVoteEmpty -> <<>>
    [] OTHER -> <<>>

ActualTargets(case) ==
  CASE Bug = "targets_unsorted" /\ case = AutoVote -> <<3, 1, 2>>
    [] Bug = "targets_keep_local" /\ case = AutoVote -> <<0, 1, 2, 3>>
    [] Bug = "targets_keep_duplicates" /\ case = AutoVote -> <<1, 2, 2, 3>>
    [] Bug = "targets_skip_vote_roster" /\ case = AutoVote -> <<4, 5>>
    [] Bug = "targets_skip_commit_fallback" /\ case = AutoCommitFallback -> <<>>
    [] Bug = "targets_skip_trusted_fallback" /\ case = AutoTrustedFallback -> <<>>
    [] Bug = "targets_empty_auto_uses_local" /\ case = AutoEmpty -> <<0>>
    [] Bug = "targets_explicit_vote_falls_back" /\ case = TierVoteEmpty -> <<4, 5>>
    [] Bug = "targets_explicit_commit_uses_vote" /\ case = TierCommit -> <<1, 2, 3>>
    [] Bug = "targets_explicit_trusted_uses_commit" /\ case = TierTrusted -> <<4, 5>>
    [] Bug = "targets_trusted_unsorted" /\ case = TierTrusted -> <<6, 2>>
    [] OTHER -> SpecTargets(case)

Floor == 5
MaxTime == 20

CooldownFirst == "cooldown_first"
CooldownDuplicateBefore == "cooldown_duplicate_before"
CooldownBoundary == "cooldown_boundary"
CooldownOtherKey == "cooldown_other_key"
CooldownExpiredUnrelated == "cooldown_expired_unrelated"
CooldownHintFloor == "cooldown_hint_floor"
CooldownTtlWins == "cooldown_ttl_wins"
CooldownHintWins == "cooldown_hint_wins"
CooldownOverflow == "cooldown_overflow"

CooldownCases == {
  CooldownFirst,
  CooldownDuplicateBefore,
  CooldownBoundary,
  CooldownOtherKey,
  CooldownExpiredUnrelated,
  CooldownHintFloor,
  CooldownTtlWins,
  CooldownHintWins,
  CooldownOverflow
}

SpecCooldownAllows(case) ==
  case # CooldownDuplicateBefore

SpecCooldownDeadline(case) ==
  CASE case = CooldownDuplicateBefore -> 14
    [] case = CooldownBoundary -> 19
    [] case = CooldownTtlWins -> 18
    [] case = CooldownHintWins -> 19
    [] case = CooldownOverflow -> 18
    [] OTHER -> 15

SpecRetainsUnrelatedCooldown(case) ==
  CASE case = CooldownOtherKey -> TRUE
    [] case = CooldownExpiredUnrelated -> FALSE
    [] OTHER -> TRUE

ActualCooldownAllows(case) ==
  CASE Bug = "cooldown_allows_duplicate"
       /\ case = CooldownDuplicateBefore -> TRUE
    [] Bug = "cooldown_blocks_boundary"
       /\ case = CooldownBoundary -> FALSE
    [] Bug = "cooldown_ignores_key"
       /\ case = CooldownOtherKey -> FALSE
    [] OTHER -> SpecCooldownAllows(case)

ActualCooldownDeadline(case) ==
  CASE Bug = "cooldown_uses_hint_only"
       /\ case = CooldownTtlWins -> 15
    [] Bug = "cooldown_uses_ttl_only"
       /\ case = CooldownHintWins -> 18
    [] Bug = "cooldown_skips_floor"
       /\ case = CooldownHintFloor -> 12
    [] Bug = "cooldown_overflow_saturates"
       /\ case = CooldownOverflow -> MaxTime
    [] OTHER -> SpecCooldownDeadline(case)

ActualRetainsUnrelatedCooldown(case) ==
  CASE Bug = "cooldown_keeps_expired"
       /\ case = CooldownExpiredUnrelated -> TRUE
    [] Bug = "cooldown_drops_unexpired"
       /\ case = CooldownOtherKey -> FALSE
    [] OTHER -> SpecRetainsUnrelatedCooldown(case)

FutureReanchor == "future_new_view_frontier_reanchor"
FrontierGap == "frontier_gap_realign"
IdleMissingQc == "idle_missing_qc_reacquire"
LockLagHighest == "lock_lag_highest_qc_defer"
NoRosterFailClosed == "no_roster_fail_closed"
QcMissingPayload == "qc_missing_payload_quorum_fast_recovery"
SidecarMismatch == "sidecar_mismatch"
UnknownReason == "unknown"

Reasons == {
  FutureReanchor,
  FrontierGap,
  IdleMissingQc,
  LockLagHighest,
  NoRosterFailClosed,
  QcMissingPayload,
  SidecarMismatch,
  UnknownReason
}

SpecLockLagReason(reason) ==
  reason \in {FutureReanchor, FrontierGap, LockLagHighest,
              QcMissingPayload, SidecarMismatch}

SpecCanonicalReason(reason) ==
  reason \in {FutureReanchor, FrontierGap, IdleMissingQc, LockLagHighest,
              QcMissingPayload, SidecarMismatch}

SpecMissingQcReason(reason) ==
  reason \in {FutureReanchor, IdleMissingQc, LockLagHighest, QcMissingPayload}

SpecPrefersPrev(reason, canonicalActive) ==
  CASE reason = IdleMissingQc -> canonicalActive
    [] reason \in {FutureReanchor, FrontierGap, LockLagHighest,
                   NoRosterFailClosed, QcMissingPayload, SidecarMismatch} -> TRUE
    [] OTHER -> FALSE

ActualLockLagReason(reason) ==
  CASE Bug = "reason_idle_lock_lag" /\ reason = IdleMissingQc -> TRUE
    [] OTHER -> SpecLockLagReason(reason)

ActualCanonicalReason(reason) ==
  CASE Bug = "reason_future_not_canonical" /\ reason = FutureReanchor -> FALSE
    [] Bug = "reason_no_roster_fail_closed_canonical"
       /\ reason = NoRosterFailClosed -> TRUE
    [] OTHER -> SpecCanonicalReason(reason)

ActualMissingQcReason(reason) ==
  CASE Bug = "reason_frontier_missing_qc" /\ reason = FrontierGap -> TRUE
    [] OTHER -> SpecMissingQcReason(reason)

ActualPrefersPrev(reason, canonicalActive) ==
  CASE Bug = "anchor_idle_ignores_canonical" /\ reason = IdleMissingQc -> ~canonicalActive
    [] Bug = "anchor_no_roster_fail_closed_not_prev"
       /\ reason = NoRosterFailClosed -> FALSE
    [] OTHER -> SpecPrefersPrev(reason, canonicalActive)

AnchorNoLatest == "anchor_no_latest"
AnchorFuturePrev == "anchor_future_prev"
AnchorFutureNoPrev == "anchor_future_no_prev"
AnchorIdleActivePrev == "anchor_idle_active_prev"
AnchorIdleInactivePrev == "anchor_idle_inactive_prev"
AnchorUnknownPrev == "anchor_unknown_prev"
AnchorNoRosterFailClosedPrev == "anchor_no_roster_fail_closed_prev"
AnchorFrontierPrev == "anchor_frontier_prev"

AnchorCases == {
  AnchorNoLatest,
  AnchorFuturePrev,
  AnchorFutureNoPrev,
  AnchorIdleActivePrev,
  AnchorIdleInactivePrev,
  AnchorUnknownPrev,
  AnchorNoRosterFailClosedPrev,
  AnchorFrontierPrev
}

SpecAnchorMode(case) ==
  CASE case = AnchorNoLatest -> "none"
    [] case = AnchorFuturePrev -> "prev_latest"
    [] case = AnchorFutureNoPrev -> "latest_latest_fallback"
    [] case = AnchorIdleActivePrev -> "prev_latest"
    [] case = AnchorIdleInactivePrev -> "latest_latest"
    [] case = AnchorUnknownPrev -> "latest_latest"
    [] case = AnchorNoRosterFailClosedPrev -> "prev_latest"
    [] case = AnchorFrontierPrev -> "prev_latest"
    [] OTHER -> "none"

ActualAnchorMode(case) ==
  CASE Bug = "anchor_no_latest_returns_latest"
       /\ case = AnchorNoLatest -> "latest_latest"
    [] Bug = "anchor_ignores_prev_preference"
       /\ case = AnchorFuturePrev -> "latest_latest"
    [] Bug = "anchor_requires_prev"
       /\ case = AnchorFutureNoPrev -> "none"
    [] Bug = "anchor_idle_ignores_canonical"
       /\ case = AnchorIdleActivePrev -> "latest_latest"
    [] Bug = "anchor_unknown_uses_prev"
       /\ case = AnchorUnknownPrev -> "prev_latest"
    [] Bug = "anchor_no_roster_fail_closed_not_prev"
       /\ case = AnchorNoRosterFailClosedPrev -> "latest_latest"
    [] OTHER -> SpecAnchorMode(case)

TypeInvariant ==
  checked \in 0..1

ObservationTransitionMatches ==
  \A stage \in Stages:
    ActualObservationTransition(stage) = SpecObservationTransition(stage)

StageStepMatches ==
  \A stage \in Stages:
    \A transition \in Transitions:
      ActualStep(stage, transition) = SpecStep(stage, transition)

TierAdvanceMatches ==
  \A tier \in Tiers:
    ActualAdvance(tier) = SpecAdvance(tier)

TierLabelMatches ==
  \A tier \in Tiers:
    ActualLabel(tier) = SpecLabel(tier)

TargetsMatch ==
  \A case \in TargetCases:
    ActualTargets(case) = SpecTargets(case)

CooldownAllowsMatch ==
  \A case \in CooldownCases:
    ActualCooldownAllows(case) = SpecCooldownAllows(case)

CooldownDeadlineMatch ==
  \A case \in CooldownCases:
    ActualCooldownDeadline(case) = SpecCooldownDeadline(case)

CooldownRetentionMatch ==
  \A case \in CooldownCases:
    ActualRetainsUnrelatedCooldown(case) = SpecRetainsUnrelatedCooldown(case)

ReasonClassifiersMatch ==
  /\ \A reason \in Reasons:
       ActualLockLagReason(reason) = SpecLockLagReason(reason)
  /\ \A reason \in Reasons:
       ActualCanonicalReason(reason) = SpecCanonicalReason(reason)
  /\ \A reason \in Reasons:
       ActualMissingQcReason(reason) = SpecMissingQcReason(reason)
  /\ \A reason \in Reasons:
       /\ ActualPrefersPrev(reason, TRUE) = SpecPrefersPrev(reason, TRUE)
       /\ ActualPrefersPrev(reason, FALSE) = SpecPrefersPrev(reason, FALSE)

AnchorModesMatch ==
  \A case \in AnchorCases:
    ActualAnchorMode(case) = SpecAnchorMode(case)

Init ==
  checked = 0

Next ==
  \/ /\ checked = 0
     /\ checked' = 1
  \/ /\ checked = 1
     /\ UNCHANGED vars

=============================================================================
====
