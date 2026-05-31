---- MODULE SumeragiPacemakerEvaluationGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `Actor::evaluate_pacemaker(...)`.

This slice models how proposal backpressure, the one-shot backpressure
transition tracker, and `Pacemaker::should_fire(now)` combine into the
returned `(log_initial_deferral, log_fire_deferral, should_attempt_proposal)`
tuple. The Rust helper checks the pacemaker deadline before interpreting
backpressure, so deadline advancement is observed even when hard backpressure
suppresses a proposal attempt.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  log_initial_deferral,
  \* @type: Bool;
  log_fire_deferral,
  \* @type: Bool;
  should_attempt_proposal,
  \* @type: Bool;
  tracker_deferring_after,
  \* @type: Bool;
  deadline_advanced

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool>>;
vars == <<candidate, log_initial_deferral, log_fire_deferral,
  should_attempt_proposal, tracker_deferring_after, deadline_advanced>>

Cases == {
  "healthy_before_deadline",
  "healthy_due",
  "pacing_first_before_deadline",
  "pacing_subsequent_before_deadline",
  "pacing_first_due",
  "pacing_subsequent_due",
  "hard_first_before_deadline",
  "hard_subsequent_before_deadline",
  "hard_first_due",
  "hard_subsequent_due",
  "recovered_before_deadline",
  "recovered_due"
}

HealthyBeforeDeadlineCases == {"healthy_before_deadline"}
HealthyDueCases == {"healthy_due"}
HealthyCases == HealthyBeforeDeadlineCases \union HealthyDueCases

RecoveredBeforeDeadlineCases == {"recovered_before_deadline"}
RecoveredDueCases == {"recovered_due"}
RecoveredCases == RecoveredBeforeDeadlineCases \union RecoveredDueCases

PacingFirstBeforeDeadlineCases == {"pacing_first_before_deadline"}
PacingSubsequentBeforeDeadlineCases ==
  {"pacing_subsequent_before_deadline"}
PacingBeforeDeadlineCases ==
  PacingFirstBeforeDeadlineCases \union PacingSubsequentBeforeDeadlineCases
PacingFirstDueCases == {"pacing_first_due"}
PacingSubsequentDueCases == {"pacing_subsequent_due"}
PacingDueCases == PacingFirstDueCases \union PacingSubsequentDueCases
PacingCases == PacingBeforeDeadlineCases \union PacingDueCases

HardFirstBeforeDeadlineCases == {"hard_first_before_deadline"}
HardSubsequentBeforeDeadlineCases ==
  {"hard_subsequent_before_deadline"}
HardBeforeDeadlineCases ==
  HardFirstBeforeDeadlineCases \union HardSubsequentBeforeDeadlineCases
HardFirstDueCases == {"hard_first_due"}
HardSubsequentDueCases == {"hard_subsequent_due"}
HardDueCases == HardFirstDueCases \union HardSubsequentDueCases
HardCases == HardBeforeDeadlineCases \union HardDueCases

FirstDeferralCases ==
  PacingFirstBeforeDeadlineCases \union PacingFirstDueCases
    \union HardFirstBeforeDeadlineCases \union HardFirstDueCases
SubsequentDeferralCases ==
  PacingSubsequentBeforeDeadlineCases \union PacingSubsequentDueCases
    \union HardSubsequentBeforeDeadlineCases \union HardSubsequentDueCases
DeferringCases == PacingCases \union HardCases
NonDeferringCases == HealthyCases \union RecoveredCases
DueCases == HealthyDueCases \union RecoveredDueCases
  \union PacingDueCases \union HardDueCases
BeforeDeadlineCases == HealthyBeforeDeadlineCases
  \union RecoveredBeforeDeadlineCases
  \union PacingBeforeDeadlineCases \union HardBeforeDeadlineCases

SpecLogInitial(c) == c \in FirstDeferralCases

SpecLogFire(c) ==
  \/ c \in PacingFirstBeforeDeadlineCases
  \/ c \in PacingDueCases
  \/ c \in HardDueCases

SpecAttempt(c) == c \in (HealthyDueCases \union RecoveredDueCases
  \union PacingDueCases)

SpecTrackerDeferringAfter(c) == c \in DeferringCases

SpecDeadlineAdvanced(c) == c \in DueCases

ActualLogInitial(c) ==
  \/ /\ SpecLogInitial(c)
     /\ Bug # "log_initial_missing"
     /\ Bug # "skip_first_deferral_log"
     /\ ~(Bug = "skip_pacing_initial_log"
          /\ c \in (PacingFirstBeforeDeadlineCases \union PacingFirstDueCases))
  \/ /\ c \in SubsequentDeferralCases
     /\ Bug \in {"log_initial_repeats", "repeat_initial_deferral_log"}
  \/ /\ c \in NonDeferringCases
     /\ Bug = "log_initial_without_deferral"

ActualLogFire(c) ==
  \/ /\ SpecLogFire(c)
     /\ ~(Bug = "pacing_first_before_no_fire_log"
          /\ c \in PacingFirstBeforeDeadlineCases)
     /\ ~(Bug = "pacing_due_skips_fire_log"
          /\ c \in PacingDueCases)
     /\ ~(Bug = "skip_pacing_deadline_log"
          /\ c \in PacingDueCases)
     /\ ~(Bug = "hard_due_skips_fire_log"
          /\ c \in HardDueCases)
     /\ ~(Bug = "skip_hard_deadline_log"
          /\ c \in HardDueCases)
  \/ /\ c \in PacingSubsequentBeforeDeadlineCases
     /\ Bug \in {"pacing_subsequent_before_logs_fire",
          "log_pacing_repeat_before_deadline"}
  \/ /\ c \in HardBeforeDeadlineCases
     /\ Bug \in {"hard_before_logs_fire", "log_hard_before_deadline"}
  \/ /\ c \in HealthyDueCases
     /\ Bug = "healthy_due_logs_deferral"
  \/ /\ c \in NonDeferringCases
     /\ Bug = "log_fire_without_deferral"

ActualAttempt(c) ==
  \/ /\ SpecAttempt(c)
     /\ ~(Bug = "pacing_due_skips_attempt"
          /\ c \in PacingDueCases)
     /\ ~(Bug = "skip_pacing_deadline_attempt"
          /\ c \in PacingDueCases)
     /\ ~(Bug = "healthy_due_skips_attempt"
          /\ c \in HealthyDueCases)
     /\ ~(Bug = "skip_healthy_deadline_attempt"
          /\ c \in HealthyDueCases)
     /\ ~(Bug = "recovered_due_skips_attempt"
          /\ c \in RecoveredDueCases)
     /\ ~(Bug = "skip_recovered_deadline_attempt"
          /\ c \in RecoveredDueCases)
  \/ /\ c \in HardDueCases
     /\ Bug = "hard_due_attempts"
  \/ /\ c \in HardCases
     /\ Bug = "attempt_under_hard_backpressure"
  \/ /\ c \in HealthyBeforeDeadlineCases
     /\ Bug = "healthy_before_attempts"
  \/ /\ c \in PacingBeforeDeadlineCases
     /\ Bug = "attempt_pacing_before_deadline"
  \/ /\ c \in RecoveredBeforeDeadlineCases
     /\ Bug = "attempt_recovered_before_deadline"
  \/ /\ c \in BeforeDeadlineCases
     /\ Bug \in {"attempt_without_deadline", "attempt_before_deadline"}

ActualTrackerDeferringAfter(c) ==
  \/ /\ SpecTrackerDeferringAfter(c)
     /\ ~(Bug \in {"deferral_not_tracked", "tracker_not_set_on_deferral"}
          /\ c \in DeferringCases)
  \/ /\ c \in RecoveredCases
     /\ Bug \in {"recovered_keeps_tracker_deferring",
          "tracker_not_cleared_on_recovery"}
  \/ /\ c \in NonDeferringCases
     /\ Bug = "tracker_set_without_deferral"

ActualDeadlineAdvanced(c) ==
  \/ /\ SpecDeadlineAdvanced(c)
     /\ ~(Bug = "pacing_due_no_deadline_advance"
          /\ c \in PacingDueCases)
     /\ ~(Bug = "hard_due_no_deadline_advance"
          /\ c \in HardDueCases)
     /\ ~(Bug = "deadline_not_advanced_on_fire" /\ c \in DueCases)
  \/ /\ c \in BeforeDeadlineCases
     /\ Bug \in {"deadline_advances_before_due",
          "deadline_advanced_before_fire"}

Init ==
  /\ candidate = "none"
  /\ log_initial_deferral = FALSE
  /\ log_fire_deferral = FALSE
  /\ should_attempt_proposal = FALSE
  /\ tracker_deferring_after = FALSE
  /\ deadline_advanced = FALSE

CheckCase(c) ==
  /\ candidate = "none"
  /\ candidate' = c
  /\ log_initial_deferral' = ActualLogInitial(c)
  /\ log_fire_deferral' = ActualLogFire(c)
  /\ should_attempt_proposal' = ActualAttempt(c)
  /\ tracker_deferring_after' = ActualTrackerDeferringAfter(c)
  /\ deadline_advanced' = ActualDeadlineAdvanced(c)

Next ==
  \/ \E c \in Cases : CheckCase(c)
  \/ /\ candidate # "none"
     /\ UNCHANGED vars

TypeInvariant ==
  /\ candidate \in Cases \union {"none"}
  /\ log_initial_deferral \in BOOLEAN
  /\ log_fire_deferral \in BOOLEAN
  /\ should_attempt_proposal \in BOOLEAN
  /\ tracker_deferring_after \in BOOLEAN
  /\ deadline_advanced \in BOOLEAN

InitialDeferralMatchesSpec ==
  candidate = "none" \/ log_initial_deferral = SpecLogInitial(candidate)

FireDeferralMatchesSpec ==
  candidate = "none" \/ log_fire_deferral = SpecLogFire(candidate)

ProposalAttemptMatchesSpec ==
  candidate = "none" \/ should_attempt_proposal = SpecAttempt(candidate)

TrackerStateMatchesSpec ==
  candidate = "none" \/
    tracker_deferring_after = SpecTrackerDeferringAfter(candidate)

DeadlineAdvanceMatchesSpec ==
  candidate = "none" \/ deadline_advanced = SpecDeadlineAdvanced(candidate)

FirstDeferralLoggedOnlyOnTransition ==
  log_initial_deferral => candidate \in FirstDeferralCases

SubsequentDeferralsDoNotRepeatInitialLog ==
  candidate \in SubsequentDeferralCases => ~log_initial_deferral

PacingDueAttemptsDespiteBackpressure ==
  candidate \in PacingDueCases =>
    /\ should_attempt_proposal
    /\ log_fire_deferral
    /\ deadline_advanced
    /\ tracker_deferring_after

PacingBeforeDeadlineLogsOnlyFirstTransition ==
  candidate \in PacingBeforeDeadlineCases =>
    /\ ~should_attempt_proposal
    /\ log_fire_deferral = log_initial_deferral
    /\ ~deadline_advanced
    /\ tracker_deferring_after

HardBackpressureNeverAttempts ==
  candidate \in HardCases => ~should_attempt_proposal

HardDueStillLogsAndAdvancesDeadline ==
  candidate \in HardDueCases =>
    /\ log_fire_deferral
    /\ deadline_advanced
    /\ tracker_deferring_after

HardBeforeDeadlineStaysQuiet ==
  candidate \in HardBeforeDeadlineCases =>
    /\ ~log_fire_deferral
    /\ ~deadline_advanced
    /\ tracker_deferring_after

HealthyOrRecoveredNeverLogDeferral ==
  candidate \in (HealthyCases \union RecoveredCases) =>
    /\ ~log_initial_deferral
    /\ ~log_fire_deferral

HealthyOrRecoveredDueAttempts ==
  candidate \in (HealthyDueCases \union RecoveredDueCases) =>
    /\ should_attempt_proposal
    /\ deadline_advanced

RecoveryClearsTracker ==
  candidate \in RecoveredCases => ~tracker_deferring_after

DeferralTrackerMatchesPressure ==
  tracker_deferring_after <=> candidate \in DeferringCases

DeadlineAdvancesExactlyWhenDue ==
  deadline_advanced <=> candidate \in DueCases

AttemptRequiresDeadlineAndNoHardBackpressure ==
  should_attempt_proposal =>
    /\ deadline_advanced
    /\ candidate \notin HardCases

Safety ==
  /\ InitialDeferralMatchesSpec
  /\ FireDeferralMatchesSpec
  /\ ProposalAttemptMatchesSpec
  /\ TrackerStateMatchesSpec
  /\ DeadlineAdvanceMatchesSpec
  /\ FirstDeferralLoggedOnlyOnTransition
  /\ SubsequentDeferralsDoNotRepeatInitialLog
  /\ PacingDueAttemptsDespiteBackpressure
  /\ PacingBeforeDeadlineLogsOnlyFirstTransition
  /\ HardBackpressureNeverAttempts
  /\ HardDueStillLogsAndAdvancesDeadline
  /\ HardBeforeDeadlineStaysQuiet
  /\ HealthyOrRecoveredNeverLogDeferral
  /\ HealthyOrRecoveredDueAttempts
  /\ RecoveryClearsTracker
  /\ DeferralTrackerMatchesPressure
  /\ DeadlineAdvancesExactlyWhenDue
  /\ AttemptRequiresDeadlineAndNoHardBackpressure

=============================================================================
