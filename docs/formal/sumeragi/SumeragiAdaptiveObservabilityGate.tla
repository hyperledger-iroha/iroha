---- MODULE SumeragiAdaptiveObservabilityGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi adaptive observability.

This slice captures `AdaptiveObservabilityState::new(...)`,
`AdaptiveObservabilityState::update_base_collector_limit(...)`, and
`AdaptiveObservabilityState::evaluate(...)`. It abstracts time and metrics into
representative boundary cases while preserving the observable consensus-facing
contract: adaptive mode is enabled by either the adaptive-observability config
or resilience mode, disabled mode resets only when an adaptive change is active,
DA/QC/queue alerts apply only outside the cooldown window, cooldown boundaries
are inclusive, collector fanout never decreases on apply and is floored to one
for the baseline, resilience can raise the adaptive collector limit, pacemaker
intervals are boosted from the base interval and reset back to that base, missing
local-data deltas saturate before comparison, and the missing-data baseline is
updated on every evaluation.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

DisabledNoop == "disabled_noop"
DisabledResetApplied == "disabled_reset_applied"
AdaptiveDaBurstApplies == "adaptive_da_burst_applies"
AdaptiveQcLatencyApplies == "adaptive_qc_latency_applies"
AdaptiveQueueApplies == "adaptive_queue_applies"
ResilienceQcApplies == "resilience_qc_applies"
ResilienceQueueApplies == "resilience_queue_applies"
CollectorBaselineFloored == "collector_baseline_floored"
CollectorLimitMaxesBase == "collector_limit_maxes_base"
ResilienceLimitMaxesAdaptive == "resilience_limit_maxes_adaptive"
CurrentCollectorPreserved == "current_collector_preserved"
CooldownBlocksReapply == "cooldown_blocks_reapply"
CooldownBoundaryAllowsReapply == "cooldown_boundary_allows_reapply"
AppliedNoAlertsBeforeCooldownStays == "applied_no_alerts_before_cooldown_stays"
AppliedNoAlertsAfterCooldownResets == "applied_no_alerts_after_cooldown_resets"
AppliedAlertsAfterCooldownReapplies == "applied_alerts_after_cooldown_reapplies"
MissingDeltaSaturates == "missing_delta_saturates"
MissingBaselineUpdates == "missing_baseline_updates"
NoAlertIdleNoop == "no_alert_idle_noop"
LastTriggerRecordedOnApply == "last_trigger_recorded_on_apply"
LastTriggerPreservedOnNoop == "last_trigger_preserved_on_noop"
ResetClearsTrigger == "reset_clears_trigger"
BoostedIntervalApplied == "boosted_interval_applied"
ResetRestoresBaseInterval == "reset_restores_base_interval"
UpdateBaseCollectorFloors == "update_base_collector_floors"

Cases == {
  DisabledNoop,
  DisabledResetApplied,
  AdaptiveDaBurstApplies,
  AdaptiveQcLatencyApplies,
  AdaptiveQueueApplies,
  ResilienceQcApplies,
  ResilienceQueueApplies,
  CollectorBaselineFloored,
  CollectorLimitMaxesBase,
  ResilienceLimitMaxesAdaptive,
  CurrentCollectorPreserved,
  CooldownBlocksReapply,
  CooldownBoundaryAllowsReapply,
  AppliedNoAlertsBeforeCooldownStays,
  AppliedNoAlertsAfterCooldownResets,
  AppliedAlertsAfterCooldownReapplies,
  MissingDeltaSaturates,
  MissingBaselineUpdates,
  NoAlertIdleNoop,
  LastTriggerRecordedOnApply,
  LastTriggerPreservedOnNoop,
  ResetClearsTrigger,
  BoostedIntervalApplied,
  ResetRestoresBaseInterval,
  UpdateBaseCollectorFloors
}

ActionApplied == 1
ActionReset == 2
ActionNone == 3
AdaptiveEnabled == 4
AdaptiveDisabled == 5
ResilienceEnabled == 6
DaBurstAlert == 7
QcLatencyAlert == 8
QueueAlert == 9
NoAlert == 10
CooldownOpen == 11
CooldownClosed == 12
CooldownBoundaryInclusive == 13
CollectorBaseFloored == 14
CollectorUsesAdaptiveMax == 15
CollectorUsesBaseMax == 16
CollectorUsesResilienceMax == 17
CollectorPreservedCurrent == 18
CollectorRestoredBase == 19
PacemakerBoosted == 20
PacemakerRestoredBase == 21
PacemakerUnchanged == 22
StateAppliedTrue == 23
StateAppliedFalse == 24
LastTriggerSet == 25
LastTriggerCleared == 26
LastTriggerPreserved == 27
MissingDeltaSaturated == 28
MissingDeltaCompared == 29
MissingBaselineUpdated == 30
NoReapplyBeforeCooldown == 31
NoResetBeforeCooldown == 32
ResetRequiresApplied == 33
SaturatingIntervalAdd == 34
UpdateBaseFloored == 35

Actions == 1..35

ApplyBase ==
  {CooldownOpen, ActionApplied, PacemakerBoosted, StateAppliedTrue,
   LastTriggerSet, MissingBaselineUpdated, MissingDeltaCompared,
   SaturatingIntervalAdd}

ResetBase ==
  {ActionReset, PacemakerRestoredBase, CollectorRestoredBase,
   StateAppliedFalse, LastTriggerCleared, MissingBaselineUpdated,
   ResetRequiresApplied}

NoopBase ==
  {ActionNone, PacemakerUnchanged, LastTriggerPreserved,
   MissingBaselineUpdated}

SpecActions(c) ==
  CASE c = DisabledNoop ->
      NoopBase \cup {AdaptiveDisabled, StateAppliedFalse,
        CollectorPreservedCurrent}
    [] c = DisabledResetApplied ->
      ResetBase \cup {AdaptiveDisabled}
    [] c = AdaptiveDaBurstApplies ->
      ApplyBase \cup {AdaptiveEnabled, DaBurstAlert, CollectorUsesAdaptiveMax}
    [] c = AdaptiveQcLatencyApplies ->
      ApplyBase \cup {AdaptiveEnabled, QcLatencyAlert, CollectorUsesAdaptiveMax}
    [] c = AdaptiveQueueApplies ->
      ApplyBase \cup {AdaptiveEnabled, QueueAlert, CollectorUsesAdaptiveMax}
    [] c = ResilienceQcApplies ->
      ApplyBase \cup {AdaptiveDisabled, ResilienceEnabled, QcLatencyAlert,
        CollectorUsesResilienceMax}
    [] c = ResilienceQueueApplies ->
      ApplyBase \cup {AdaptiveDisabled, ResilienceEnabled, QueueAlert,
        CollectorUsesResilienceMax}
    [] c = CollectorBaselineFloored ->
      {CollectorBaseFloored, StateAppliedFalse}
    [] c = CollectorLimitMaxesBase ->
      ApplyBase \cup {AdaptiveEnabled, QcLatencyAlert, CollectorUsesBaseMax}
    [] c = ResilienceLimitMaxesAdaptive ->
      ApplyBase \cup {ResilienceEnabled, QcLatencyAlert,
        CollectorUsesResilienceMax}
    [] c = CurrentCollectorPreserved ->
      ApplyBase \cup {AdaptiveEnabled, QueueAlert, CollectorPreservedCurrent}
    [] c = CooldownBlocksReapply ->
      NoopBase \cup {AdaptiveEnabled, DaBurstAlert, CooldownClosed,
        StateAppliedTrue, CollectorPreservedCurrent, NoReapplyBeforeCooldown}
    [] c = CooldownBoundaryAllowsReapply ->
      ApplyBase \cup {AdaptiveEnabled, QcLatencyAlert,
        CooldownBoundaryInclusive, CollectorUsesAdaptiveMax}
    [] c = AppliedNoAlertsBeforeCooldownStays ->
      NoopBase \cup {AdaptiveEnabled, NoAlert, CooldownClosed,
        StateAppliedTrue, CollectorPreservedCurrent, NoResetBeforeCooldown}
    [] c = AppliedNoAlertsAfterCooldownResets ->
      ResetBase \cup {AdaptiveEnabled, NoAlert, CooldownOpen}
    [] c = AppliedAlertsAfterCooldownReapplies ->
      ApplyBase \cup {AdaptiveEnabled, QueueAlert, CollectorUsesAdaptiveMax}
    [] c = MissingDeltaSaturates ->
      NoopBase \cup {AdaptiveEnabled, NoAlert, MissingDeltaSaturated,
        StateAppliedFalse, CollectorPreservedCurrent}
    [] c = MissingBaselineUpdates ->
      NoopBase \cup {AdaptiveEnabled, NoAlert, StateAppliedFalse,
        CollectorPreservedCurrent}
    [] c = NoAlertIdleNoop ->
      NoopBase \cup {AdaptiveEnabled, NoAlert, StateAppliedFalse,
        CollectorPreservedCurrent}
    [] c = LastTriggerRecordedOnApply ->
      ApplyBase \cup {AdaptiveEnabled, DaBurstAlert, CollectorUsesAdaptiveMax}
    [] c = LastTriggerPreservedOnNoop ->
      NoopBase \cup {AdaptiveEnabled, QcLatencyAlert, CooldownClosed,
        StateAppliedTrue, CollectorPreservedCurrent}
    [] c = ResetClearsTrigger ->
      ResetBase \cup {AdaptiveEnabled, NoAlert, CooldownOpen}
    [] c = BoostedIntervalApplied ->
      ApplyBase \cup {AdaptiveEnabled, QueueAlert, CollectorUsesAdaptiveMax}
    [] c = ResetRestoresBaseInterval ->
      ResetBase \cup {AdaptiveEnabled, NoAlert, CooldownOpen}
    [] c = UpdateBaseCollectorFloors ->
      {UpdateBaseFloored, CollectorBaseFloored}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "disabled_applies_without_resilience"
       /\ c = DisabledNoop ->
      (spec \ {ActionNone, PacemakerUnchanged, StateAppliedFalse})
        \cup {ActionApplied, PacemakerBoosted, StateAppliedTrue}
    [] Bug = "disabled_skips_reset"
       /\ c = DisabledResetApplied ->
      (spec \ {ActionReset, PacemakerRestoredBase, CollectorRestoredBase,
               StateAppliedFalse, LastTriggerCleared})
        \cup {ActionNone, PacemakerUnchanged, CollectorPreservedCurrent,
              StateAppliedTrue, LastTriggerPreserved}
    [] Bug = "resilience_ignored"
       /\ c = ResilienceQcApplies ->
      (spec \ {ActionApplied, PacemakerBoosted, StateAppliedTrue,
               LastTriggerSet})
        \cup {ActionNone, PacemakerUnchanged, StateAppliedFalse,
              LastTriggerPreserved}
    [] Bug = "da_burst_ignored"
       /\ c = AdaptiveDaBurstApplies ->
      (spec \ {DaBurstAlert, ActionApplied, PacemakerBoosted})
        \cup {NoAlert, ActionNone, PacemakerUnchanged}
    [] Bug = "qc_alert_strict_threshold"
       /\ c = AdaptiveQcLatencyApplies ->
      (spec \ {QcLatencyAlert, ActionApplied, PacemakerBoosted})
        \cup {NoAlert, ActionNone, PacemakerUnchanged}
    [] Bug = "queue_alert_ignored"
       /\ c = AdaptiveQueueApplies ->
      (spec \ {QueueAlert, ActionApplied, PacemakerBoosted})
        \cup {NoAlert, ActionNone, PacemakerUnchanged}
    [] Bug = "cooldown_ignored"
       /\ c = CooldownBlocksReapply ->
      (spec \ {ActionNone, CooldownClosed, NoReapplyBeforeCooldown,
               PacemakerUnchanged, LastTriggerPreserved})
        \cup {ActionApplied, CooldownOpen, PacemakerBoosted, LastTriggerSet}
    [] Bug = "cooldown_boundary_blocked"
       /\ c = CooldownBoundaryAllowsReapply ->
      (spec \ {ActionApplied, CooldownOpen, CooldownBoundaryInclusive,
               PacemakerBoosted, LastTriggerSet})
        \cup {ActionNone, CooldownClosed, PacemakerUnchanged,
              LastTriggerPreserved}
    [] Bug = "reset_before_cooldown"
       /\ c = AppliedNoAlertsBeforeCooldownStays ->
      (spec \ {ActionNone, NoResetBeforeCooldown, StateAppliedTrue,
               LastTriggerPreserved})
        \cup {ActionReset, StateAppliedFalse, LastTriggerCleared}
    [] Bug = "reset_skips_collector"
       /\ c = AppliedNoAlertsAfterCooldownResets ->
      (spec \ {CollectorRestoredBase}) \cup {CollectorPreservedCurrent}
    [] Bug = "reset_skips_interval"
       /\ c = ResetRestoresBaseInterval ->
      (spec \ {PacemakerRestoredBase}) \cup {PacemakerUnchanged}
    [] Bug = "reset_keeps_trigger"
       /\ c = ResetClearsTrigger ->
      (spec \ {LastTriggerCleared}) \cup {LastTriggerPreserved}
    [] Bug = "apply_skips_pacemaker"
       /\ c = BoostedIntervalApplied ->
      (spec \ {PacemakerBoosted, SaturatingIntervalAdd})
        \cup {PacemakerUnchanged}
    [] Bug = "apply_skips_trigger"
       /\ c = LastTriggerRecordedOnApply ->
      (spec \ {LastTriggerSet}) \cup {LastTriggerPreserved}
    [] Bug = "apply_decreases_collector"
       /\ c = CurrentCollectorPreserved ->
      (spec \ {CollectorPreservedCurrent}) \cup {CollectorUsesAdaptiveMax}
    [] Bug = "apply_uses_cfg_below_base"
       /\ c = CollectorLimitMaxesBase ->
      (spec \ {CollectorUsesBaseMax}) \cup {CollectorUsesAdaptiveMax}
    [] Bug = "resilience_limit_ignored"
       /\ c = ResilienceLimitMaxesAdaptive ->
      (spec \ {CollectorUsesResilienceMax}) \cup {CollectorUsesAdaptiveMax}
    [] Bug = "missing_delta_not_saturating"
       /\ c = MissingDeltaSaturates ->
      (spec \ {MissingDeltaSaturated, NoAlert, ActionNone})
        \cup {DaBurstAlert, ActionApplied}
    [] Bug = "missing_baseline_not_updated"
       /\ c = MissingBaselineUpdates ->
      spec \ {MissingBaselineUpdated}
    [] Bug = "no_alert_idle_applies"
       /\ c = NoAlertIdleNoop ->
      (spec \ {ActionNone, PacemakerUnchanged, StateAppliedFalse,
               LastTriggerPreserved})
        \cup {ActionApplied, PacemakerBoosted, StateAppliedTrue,
              LastTriggerSet}
    [] Bug = "no_alert_applied_after_cooldown_stays"
       /\ c = AppliedNoAlertsAfterCooldownResets ->
      (spec \ {ActionReset, PacemakerRestoredBase, CollectorRestoredBase,
               StateAppliedFalse, LastTriggerCleared})
        \cup {ActionNone, PacemakerUnchanged, CollectorPreservedCurrent,
              StateAppliedTrue, LastTriggerPreserved}
    [] Bug = "reapply_after_cooldown_ignored"
       /\ c = AppliedAlertsAfterCooldownReapplies ->
      (spec \ {ActionApplied, PacemakerBoosted, LastTriggerSet})
        \cup {ActionNone, PacemakerUnchanged, LastTriggerPreserved}
    [] Bug = "base_collector_zero_preserved"
       /\ c = CollectorBaselineFloored ->
      spec \ {CollectorBaseFloored}
    [] Bug = "interval_overwrites_not_adds"
       /\ c = BoostedIntervalApplied ->
      (spec \ {SaturatingIntervalAdd}) \cup {PacemakerRestoredBase}
    [] Bug = "update_base_zero_preserved"
       /\ c = UpdateBaseCollectorFloors ->
      spec \ {UpdateBaseFloored, CollectorBaseFloored}
    [] OTHER -> spec

Bugs == {
  "none",
  "disabled_applies_without_resilience",
  "disabled_skips_reset",
  "resilience_ignored",
  "da_burst_ignored",
  "qc_alert_strict_threshold",
  "queue_alert_ignored",
  "cooldown_ignored",
  "cooldown_boundary_blocked",
  "reset_before_cooldown",
  "reset_skips_collector",
  "reset_skips_interval",
  "reset_keeps_trigger",
  "apply_skips_pacemaker",
  "apply_skips_trigger",
  "apply_decreases_collector",
  "apply_uses_cfg_below_base",
  "resilience_limit_ignored",
  "missing_delta_not_saturating",
  "missing_baseline_not_updated",
  "no_alert_idle_applies",
  "no_alert_applied_after_cooldown_stays",
  "reapply_after_cooldown_ignored",
  "base_collector_zero_preserved",
  "interval_overwrites_not_adds",
  "update_base_zero_preserved"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

AdaptiveObservabilityExactness ==
  /\ ActionsMatchSpec

AdaptiveObservabilityCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ AdaptiveObservabilityExactness

NoBugInvariant ==
  AdaptiveObservabilityExactness

SafetyFast == AdaptiveObservabilityExactness

BugDisabledAppliesWithoutResilience == NoBugInvariant
BugDisabledSkipsReset == NoBugInvariant
BugResilienceIgnored == NoBugInvariant
BugDaBurstIgnored == NoBugInvariant
BugQcAlertStrictThreshold == NoBugInvariant
BugQueueAlertIgnored == NoBugInvariant
BugCooldownIgnored == NoBugInvariant
BugCooldownBoundaryBlocked == NoBugInvariant
BugResetBeforeCooldown == NoBugInvariant
BugResetSkipsCollector == NoBugInvariant
BugResetSkipsInterval == NoBugInvariant
BugResetKeepsTrigger == NoBugInvariant
BugApplySkipsPacemaker == NoBugInvariant
BugApplySkipsTrigger == NoBugInvariant
BugApplyDecreasesCollector == NoBugInvariant
BugApplyUsesCfgBelowBase == NoBugInvariant
BugResilienceLimitIgnored == NoBugInvariant
BugMissingDeltaNotSaturating == NoBugInvariant
BugMissingBaselineNotUpdated == NoBugInvariant
BugNoAlertIdleApplies == NoBugInvariant
BugNoAlertAppliedAfterCooldownStays == NoBugInvariant
BugReapplyAfterCooldownIgnored == NoBugInvariant
BugBaseCollectorZeroPreserved == NoBugInvariant
BugIntervalOverwritesNotAdds == NoBugInvariant
BugUpdateBaseZeroPreserved == NoBugInvariant

====
