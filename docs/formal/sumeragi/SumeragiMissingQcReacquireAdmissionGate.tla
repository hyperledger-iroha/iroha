---- MODULE SumeragiMissingQcReacquireAdmissionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for missing-QC reacquire admission.

This slice pins `should_attempt_missing_qc_reacquire(...)`. The helper must
reject duplicate attempts for the same height/view, admit proposal-observed
rounds only with explicit commit-phase or resilience-backed unresolved
dependencies, throttle no-dependency attempts per height until the reacquire
window elapses, admit dependency and repeated-timeout recovery, and allow the
empty committed+1 no-proposal fallback only when the current view has not
already timed out and there is no active pending block.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoBug == "none"

Bugs == {
  NoBug,
  "duplicate_accept_same_view",
  "duplicate_reject_different_view",
  "proposal_reject_commit_dependency",
  "proposal_reject_missing_qc_dependency",
  "proposal_reject_frontier_dependency",
  "proposal_accept_no_dependency",
  "proposal_accept_dependency_without_resilience",
  "proposal_accept_dependency_signal_only",
  "no_dependency_accept_recent_window",
  "no_dependency_reject_after_window",
  "no_dependency_reject_no_prior",
  "no_dependency_drop_fresh_throttle",
  "no_dependency_keep_stale_throttle",
  "dependency_reject_signals",
  "streak_reject_repeated_timeout",
  "empty_frontier_reject_first_timeout",
  "empty_frontier_accept_non_frontier",
  "empty_frontier_accept_pending_block",
  "empty_frontier_accept_proposal_seen",
  "empty_frontier_accept_current_timed_out",
  "accept_no_source"
}

SameViewDuplicateRejected ==
  IF Bug = "duplicate_accept_same_view" THEN FALSE ELSE TRUE

DifferentViewNotDuplicate ==
  IF Bug = "duplicate_reject_different_view" THEN FALSE ELSE TRUE

ProposalCommitDependencyAllowed ==
  IF Bug = "proposal_reject_commit_dependency" THEN FALSE ELSE TRUE

ProposalMissingQcDependencyAllowed ==
  IF Bug = "proposal_reject_missing_qc_dependency" THEN FALSE ELSE TRUE

ProposalFrontierDependencyAllowed ==
  IF Bug = "proposal_reject_frontier_dependency" THEN FALSE ELSE TRUE

ProposalNoDependencyRejected ==
  IF Bug = "proposal_accept_no_dependency" THEN FALSE ELSE TRUE

ProposalDependencyNeedsResilience ==
  IF Bug = "proposal_accept_dependency_without_resilience" THEN FALSE ELSE TRUE

ProposalDependencyNeedsConcreteUnresolved ==
  IF Bug = "proposal_accept_dependency_signal_only" THEN FALSE ELSE TRUE

RecentNoDependencyWindowRejected ==
  IF Bug = "no_dependency_accept_recent_window" THEN FALSE ELSE TRUE

ElapsedNoDependencyWindowAllowed ==
  IF Bug = "no_dependency_reject_after_window" THEN FALSE ELSE TRUE

NoPriorNoDependencyAttemptAllowed ==
  IF Bug = "no_dependency_reject_no_prior" THEN FALSE ELSE TRUE

FreshThrottleEntryRetained ==
  IF Bug = "no_dependency_drop_fresh_throttle" THEN FALSE ELSE TRUE

StaleThrottleEntryPruned ==
  IF Bug = "no_dependency_keep_stale_throttle" THEN FALSE ELSE TRUE

DependencySignalsAllowed ==
  IF Bug = "dependency_reject_signals" THEN FALSE ELSE TRUE

RepeatedTimeoutStreakAllowed ==
  IF Bug = "streak_reject_repeated_timeout" THEN FALSE ELSE TRUE

EmptyFrontierFirstTimeoutAllowed ==
  IF Bug = "empty_frontier_reject_first_timeout" THEN FALSE ELSE TRUE

EmptyFallbackRejectsNonFrontier ==
  IF Bug = "empty_frontier_accept_non_frontier" THEN FALSE ELSE TRUE

EmptyFallbackRejectsPendingBlock ==
  IF Bug = "empty_frontier_accept_pending_block" THEN FALSE ELSE TRUE

EmptyFallbackRejectsProposalSeen ==
  IF Bug = "empty_frontier_accept_proposal_seen" THEN FALSE ELSE TRUE

EmptyFallbackRejectsCurrentTimedOut ==
  IF Bug = "empty_frontier_accept_current_timed_out" THEN FALSE ELSE TRUE

NoAdmissionSourceRejected ==
  IF Bug = "accept_no_source" THEN FALSE ELSE TRUE

Init ==
  checked = 0

Next ==
  \/ /\ checked < 21
     /\ checked' = checked + 1
  \/ /\ checked = 21
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..21

DuplicateSafety ==
  /\ SameViewDuplicateRejected
  /\ DifferentViewNotDuplicate

ProposalObservedSafety ==
  /\ ProposalCommitDependencyAllowed
  /\ ProposalMissingQcDependencyAllowed
  /\ ProposalFrontierDependencyAllowed
  /\ ProposalNoDependencyRejected
  /\ ProposalDependencyNeedsResilience
  /\ ProposalDependencyNeedsConcreteUnresolved

NoDependencyWindowSafety ==
  /\ RecentNoDependencyWindowRejected
  /\ ElapsedNoDependencyWindowAllowed
  /\ NoPriorNoDependencyAttemptAllowed
  /\ FreshThrottleEntryRetained
  /\ StaleThrottleEntryPruned

GeneralAdmissionSafety ==
  /\ DependencySignalsAllowed
  /\ RepeatedTimeoutStreakAllowed
  /\ NoAdmissionSourceRejected

EmptyFrontierFallbackSafety ==
  /\ EmptyFrontierFirstTimeoutAllowed
  /\ EmptyFallbackRejectsNonFrontier
  /\ EmptyFallbackRejectsPendingBlock
  /\ EmptyFallbackRejectsProposalSeen
  /\ EmptyFallbackRejectsCurrentTimedOut

SafetyFast ==
  /\ DuplicateSafety
  /\ ProposalObservedSafety
  /\ NoDependencyWindowSafety
  /\ GeneralAdmissionSafety
  /\ EmptyFrontierFallbackSafety

DuplicateAnchors ==
  /\ DuplicateSafety
  /\ SameViewDuplicateRejected
  /\ DifferentViewNotDuplicate

ProposalObservedAnchors ==
  /\ ProposalObservedSafety
  /\ ProposalCommitDependencyAllowed
  /\ ProposalMissingQcDependencyAllowed
  /\ ProposalFrontierDependencyAllowed
  /\ ProposalNoDependencyRejected
  /\ ProposalDependencyNeedsResilience
  /\ ProposalDependencyNeedsConcreteUnresolved

NoDependencyWindowAnchors ==
  /\ NoDependencyWindowSafety
  /\ RecentNoDependencyWindowRejected
  /\ ElapsedNoDependencyWindowAllowed
  /\ NoPriorNoDependencyAttemptAllowed
  /\ FreshThrottleEntryRetained
  /\ StaleThrottleEntryPruned

GeneralAdmissionAnchors ==
  /\ GeneralAdmissionSafety
  /\ DependencySignalsAllowed
  /\ RepeatedTimeoutStreakAllowed
  /\ NoAdmissionSourceRejected

EmptyFrontierFallbackAnchors ==
  /\ EmptyFrontierFallbackSafety
  /\ EmptyFrontierFirstTimeoutAllowed
  /\ EmptyFallbackRejectsNonFrontier
  /\ EmptyFallbackRejectsPendingBlock
  /\ EmptyFallbackRejectsProposalSeen
  /\ EmptyFallbackRejectsCurrentTimedOut

MissingQcReacquireAdmissionSafetyAnchors ==
  /\ DuplicateAnchors
  /\ ProposalObservedAnchors
  /\ NoDependencyWindowAnchors
  /\ GeneralAdmissionAnchors
  /\ EmptyFrontierFallbackAnchors

Safety ==
  MissingQcReacquireAdmissionSafetyAnchors

====
