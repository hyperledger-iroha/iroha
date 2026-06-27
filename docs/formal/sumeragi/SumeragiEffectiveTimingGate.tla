---- MODULE SumeragiEffectiveTimingGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for effective consensus timing aggregation.

This slice captures `EffectiveConsensusTiming::from_world(...)`,
`EffectiveConsensusTiming::quorum_timeout_for_da(...)`, and the actor
`block_time_for_mode_from_world(...)` / `commit_timeout_for_mode_from_world(...)`
helpers. The individual timeout arithmetic is covered by
`SumeragiTimeoutDerivationGate`; this model pins the deterministic wiring:
active-mode timing drives proposal/commit windows, worker-mode timing is
resolved from the active runtime mode, NPoS timing is resolved whenever active,
effective, or worker mode needs it, NPoS commit timing is floored by canonical
commit time, DA availability derives from the active commit-quorum timeout, and
DA quorum timeout recomputation only happens when the requested DA flag differs
from the cached timing snapshot.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

TimingAllPermissioned == 1
TimingActiveNpos == 2
TimingEffectiveNposRequiresNpos == 3
TimingWorkerNposRequiresNpos == 4
TimingActiveNposCommitFloorsCanonical == 5
TimingWorkerNposCommitFloorsCanonical == 6
TimingDaEnabledTimeouts == 7
TimingDaDisabledTimeouts == 8
TimingCooldownsFromActiveBlock == 9
TimingRbcDeliverCooldownFromControl == 10
TimingStagedModeForwarded == 11
TimingStatusParamsForwarded == 12
QuorumSameDaReturnsCached == 13
QuorumDifferentDaRecomputes == 14
QuorumDifferentDaUsesRequestedDa == 15
QuorumDifferentDaUsesRequestedMultiplier == 16
ActorBlockTimePermissioned == 17
ActorBlockTimeNpos == 18
ActorCommitTimePermissioned == 19
ActorCommitTimeNposFloorsCanonical == 20
ActorCommitTimeNposUsesResolved == 21
TimingWorkerModeUsesActiveFallback == 22

Candidates == 1..22

ResolveEffectiveMode == 1
ResolveWorkerMode == 2
NeedNposTiming == 3
SkipNposTiming == 4
ActivePermBlock == 5
ActivePermCommit == 6
ActiveNposBlock == 7
ActiveNposCommit == 8
ActiveNposCommitMaxCanonical == 9
WorkerPermBlock == 10
WorkerPermCommit == 11
WorkerNposBlock == 12
WorkerNposCommit == 13
WorkerNposCommitMaxCanonical == 14
DaFromParams == 15
CommitQuorumFromActiveTiming == 16
AvailabilityFromCommitQuorum == 17
AvailabilityUsesDaFlag == 18
CooldownsFromActiveBlock == 19
RbcDeliverFromControlCooldown == 20
StagedModeInfo == 21
MinFinalityFromParams == 22
PacingFactorFromParams == 23
ReturnCachedQuorumTimeout == 24
RecomputeQuorumTimeout == 25
UseRequestedDaFlag == 26
UseRequestedDaMultiplier == 27
ActorPermBlock == 28
ActorNposBlock == 29
ActorPermCommit == 30
ActorNposCommit == 31
ActorNposCommitMaxCanonical == 32
RbcDeliverFromPayloadCooldown == 33

Actions == 1..33

TimingBase ==
  {ResolveEffectiveMode, ResolveWorkerMode, DaFromParams,
   CommitQuorumFromActiveTiming, AvailabilityFromCommitQuorum,
   AvailabilityUsesDaFlag, CooldownsFromActiveBlock, StagedModeInfo,
   MinFinalityFromParams, PacingFactorFromParams}

SpecActions(candidate) ==
  CASE candidate = TimingAllPermissioned ->
      TimingBase \cup {SkipNposTiming, ActivePermBlock, ActivePermCommit,
        WorkerPermBlock, WorkerPermCommit}
    [] candidate = TimingActiveNpos ->
      TimingBase \cup {NeedNposTiming, ActiveNposBlock, ActiveNposCommit,
        ActiveNposCommitMaxCanonical, WorkerNposBlock, WorkerNposCommit,
        WorkerNposCommitMaxCanonical}
    [] candidate = TimingEffectiveNposRequiresNpos ->
      {ResolveEffectiveMode, NeedNposTiming, ActivePermBlock,
       ActivePermCommit}
    [] candidate = TimingWorkerNposRequiresNpos ->
      {ResolveWorkerMode, NeedNposTiming, WorkerNposBlock, WorkerNposCommit}
    [] candidate = TimingActiveNposCommitFloorsCanonical ->
      {ActiveNposCommit, ActiveNposCommitMaxCanonical}
    [] candidate = TimingWorkerNposCommitFloorsCanonical ->
      {WorkerNposCommit, WorkerNposCommitMaxCanonical}
    [] candidate = TimingDaEnabledTimeouts ->
      {DaFromParams, CommitQuorumFromActiveTiming,
       AvailabilityFromCommitQuorum, AvailabilityUsesDaFlag}
    [] candidate = TimingDaDisabledTimeouts ->
      {DaFromParams, CommitQuorumFromActiveTiming,
       AvailabilityFromCommitQuorum, AvailabilityUsesDaFlag}
    [] candidate = TimingCooldownsFromActiveBlock ->
      {CooldownsFromActiveBlock}
    [] candidate = TimingRbcDeliverCooldownFromControl ->
      {CooldownsFromActiveBlock, RbcDeliverFromControlCooldown}
    [] candidate = TimingStagedModeForwarded ->
      {StagedModeInfo}
    [] candidate = TimingStatusParamsForwarded ->
      {MinFinalityFromParams, PacingFactorFromParams}
    [] candidate = QuorumSameDaReturnsCached ->
      {ReturnCachedQuorumTimeout}
    [] candidate = QuorumDifferentDaRecomputes ->
      {RecomputeQuorumTimeout, UseRequestedDaFlag, UseRequestedDaMultiplier}
    [] candidate = QuorumDifferentDaUsesRequestedDa ->
      {RecomputeQuorumTimeout, UseRequestedDaFlag}
    [] candidate = QuorumDifferentDaUsesRequestedMultiplier ->
      {RecomputeQuorumTimeout, UseRequestedDaMultiplier}
    [] candidate = ActorBlockTimePermissioned ->
      {ActorPermBlock}
    [] candidate = ActorBlockTimeNpos ->
      {ActorNposBlock}
    [] candidate = ActorCommitTimePermissioned ->
      {ActorPermCommit}
    [] candidate = ActorCommitTimeNposFloorsCanonical ->
      {ActorNposCommit, ActorNposCommitMaxCanonical}
    [] candidate = ActorCommitTimeNposUsesResolved ->
      {ActorNposCommit}
    [] candidate = TimingWorkerModeUsesActiveFallback ->
      {ResolveWorkerMode, WorkerNposBlock, WorkerNposCommit}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = TimingAllPermissioned /\
          Bug = "all_perm_resolves_npos" ->
      (spec \ {SkipNposTiming}) \cup {NeedNposTiming}
    [] candidate = TimingEffectiveNposRequiresNpos /\
          Bug = "active_perm_uses_npos_block" ->
      (spec \ {ActivePermBlock}) \cup {ActiveNposBlock}
    [] candidate = TimingEffectiveNposRequiresNpos /\
          Bug = "active_perm_uses_npos_commit" ->
      (spec \ {ActivePermCommit}) \cup {ActiveNposCommit}
    [] candidate = TimingActiveNpos /\
          Bug = "active_npos_uses_canonical_block" ->
      (spec \ {ActiveNposBlock}) \cup {ActivePermBlock}
    [] candidate = TimingActiveNposCommitFloorsCanonical /\
          Bug = "active_npos_skips_commit_floor" ->
      spec \ {ActiveNposCommitMaxCanonical}
    [] candidate = TimingWorkerModeUsesActiveFallback /\
          Bug = "worker_mode_uses_config_fallback" ->
      (spec \ {ResolveWorkerMode}) \cup {ResolveEffectiveMode}
    [] candidate = TimingWorkerNposCommitFloorsCanonical /\
          Bug = "worker_npos_skips_commit_floor" ->
      spec \ {WorkerNposCommitMaxCanonical}
    [] candidate = TimingEffectiveNposRequiresNpos /\
          Bug = "effective_npos_skips_npos_resolution" ->
      spec \ {NeedNposTiming}
    [] candidate = TimingDaEnabledTimeouts /\
          Bug = "da_flag_inverted" ->
      spec \ {DaFromParams}
    [] candidate = TimingDaEnabledTimeouts /\
          Bug = "commit_quorum_uses_worker_timing" ->
      (spec \ {CommitQuorumFromActiveTiming}) \cup {WorkerNposCommit}
    [] candidate = TimingDaDisabledTimeouts /\
          Bug = "availability_uses_block_time" ->
      spec \ {AvailabilityFromCommitQuorum}
    [] candidate = TimingCooldownsFromActiveBlock /\
          Bug = "cooldowns_use_worker_block" ->
      (spec \ {CooldownsFromActiveBlock}) \cup {WorkerPermBlock}
    [] candidate = TimingRbcDeliverCooldownFromControl /\
          Bug = "rbc_deliver_uses_payload" ->
      (spec \ {RbcDeliverFromControlCooldown}) \cup
        {RbcDeliverFromPayloadCooldown}
    [] candidate = TimingStagedModeForwarded /\
          Bug = "drops_staged_tag" ->
      spec \ {StagedModeInfo}
    [] candidate = TimingStatusParamsForwarded /\
          Bug = "drops_status_params" ->
      spec \ {MinFinalityFromParams, PacingFactorFromParams}
    [] candidate = QuorumSameDaReturnsCached /\
          Bug = "quorum_da_same_recomputes" ->
      (spec \ {ReturnCachedQuorumTimeout}) \cup {RecomputeQuorumTimeout}
    [] candidate = QuorumDifferentDaRecomputes /\
          Bug = "quorum_da_toggle_returns_cached" ->
      (spec \ {RecomputeQuorumTimeout}) \cup {ReturnCachedQuorumTimeout}
    [] candidate = QuorumDifferentDaUsesRequestedDa /\
          Bug = "quorum_da_toggle_uses_cached_da" ->
      spec \ {UseRequestedDaFlag}
    [] candidate = QuorumDifferentDaUsesRequestedMultiplier /\
          Bug = "quorum_da_multiplier_ignored" ->
      spec \ {UseRequestedDaMultiplier}
    [] candidate = ActorBlockTimePermissioned /\
          Bug = "actor_block_time_perm_uses_npos" ->
      (spec \ {ActorPermBlock}) \cup {ActorNposBlock}
    [] candidate = ActorCommitTimeNposFloorsCanonical /\
          Bug = "actor_commit_npos_skips_floor" ->
      spec \ {ActorNposCommitMaxCanonical}
    [] candidate = ActorCommitTimePermissioned /\
          Bug = "actor_commit_perm_uses_npos" ->
      (spec \ {ActorPermCommit}) \cup {ActorNposCommit}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "all_perm_resolves_npos",
       "active_perm_uses_npos_block",
       "active_perm_uses_npos_commit",
       "active_npos_uses_canonical_block",
       "active_npos_skips_commit_floor",
       "worker_mode_uses_config_fallback",
       "worker_npos_skips_commit_floor",
       "effective_npos_skips_npos_resolution",
       "da_flag_inverted",
       "commit_quorum_uses_worker_timing",
       "availability_uses_block_time",
       "cooldowns_use_worker_block",
       "rbc_deliver_uses_payload",
       "drops_staged_tag",
       "drops_status_params",
       "quorum_da_same_recomputes",
       "quorum_da_toggle_returns_cached",
       "quorum_da_toggle_uses_cached_da",
       "quorum_da_multiplier_ignored",
       "actor_block_time_perm_uses_npos",
       "actor_commit_npos_skips_floor",
       "actor_commit_perm_uses_npos"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

EffectiveTimingExactness ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

EffectiveTimingCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EffectiveTimingExactness

Safety ==
  EffectiveTimingCorrectnessEnvelope

BugAllPermResolvesNpos ==
  ImplementationActions(TimingAllPermissioned) =
    SpecActions(TimingAllPermissioned)

BugActivePermUsesNposBlock ==
  ImplementationActions(TimingEffectiveNposRequiresNpos) =
    SpecActions(TimingEffectiveNposRequiresNpos)

BugActivePermUsesNposCommit ==
  ImplementationActions(TimingEffectiveNposRequiresNpos) =
    SpecActions(TimingEffectiveNposRequiresNpos)

BugActiveNposUsesCanonicalBlock ==
  ImplementationActions(TimingActiveNpos) = SpecActions(TimingActiveNpos)

BugActiveNposSkipsCommitFloor ==
  ImplementationActions(TimingActiveNposCommitFloorsCanonical) =
    SpecActions(TimingActiveNposCommitFloorsCanonical)

BugWorkerModeUsesConfigFallback ==
  ImplementationActions(TimingWorkerModeUsesActiveFallback) =
    SpecActions(TimingWorkerModeUsesActiveFallback)

BugWorkerNposSkipsCommitFloor ==
  ImplementationActions(TimingWorkerNposCommitFloorsCanonical) =
    SpecActions(TimingWorkerNposCommitFloorsCanonical)

BugEffectiveNposSkipsNposResolution ==
  ImplementationActions(TimingEffectiveNposRequiresNpos) =
    SpecActions(TimingEffectiveNposRequiresNpos)

BugDaFlagInverted ==
  ImplementationActions(TimingDaEnabledTimeouts) =
    SpecActions(TimingDaEnabledTimeouts)

BugCommitQuorumUsesWorkerTiming ==
  ImplementationActions(TimingDaEnabledTimeouts) =
    SpecActions(TimingDaEnabledTimeouts)

BugAvailabilityUsesBlockTime ==
  ImplementationActions(TimingDaDisabledTimeouts) =
    SpecActions(TimingDaDisabledTimeouts)

BugCooldownsUseWorkerBlock ==
  ImplementationActions(TimingCooldownsFromActiveBlock) =
    SpecActions(TimingCooldownsFromActiveBlock)

BugRbcDeliverUsesPayload ==
  ImplementationActions(TimingRbcDeliverCooldownFromControl) =
    SpecActions(TimingRbcDeliverCooldownFromControl)

BugDropsStagedTag ==
  ImplementationActions(TimingStagedModeForwarded) =
    SpecActions(TimingStagedModeForwarded)

BugDropsStatusParams ==
  ImplementationActions(TimingStatusParamsForwarded) =
    SpecActions(TimingStatusParamsForwarded)

BugQuorumDaSameRecomputes ==
  ImplementationActions(QuorumSameDaReturnsCached) =
    SpecActions(QuorumSameDaReturnsCached)

BugQuorumDaToggleReturnsCached ==
  ImplementationActions(QuorumDifferentDaRecomputes) =
    SpecActions(QuorumDifferentDaRecomputes)

BugQuorumDaToggleUsesCachedDa ==
  ImplementationActions(QuorumDifferentDaUsesRequestedDa) =
    SpecActions(QuorumDifferentDaUsesRequestedDa)

BugQuorumDaMultiplierIgnored ==
  ImplementationActions(QuorumDifferentDaUsesRequestedMultiplier) =
    SpecActions(QuorumDifferentDaUsesRequestedMultiplier)

BugActorBlockTimePermUsesNpos ==
  ImplementationActions(ActorBlockTimePermissioned) =
    SpecActions(ActorBlockTimePermissioned)

BugActorCommitNposSkipsFloor ==
  ImplementationActions(ActorCommitTimeNposFloorsCanonical) =
    SpecActions(ActorCommitTimeNposFloorsCanonical)

BugActorCommitPermUsesNpos ==
  ImplementationActions(ActorCommitTimePermissioned) =
    SpecActions(ActorCommitTimePermissioned)

====
