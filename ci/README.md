# CI Helpers

This directory hosts the developer-facing shell helpers that gate CI jobs
(`ci/check_*.sh`). Most scripts assume the default Cargo artifact layout under
`target/`, so keep that layout unchanged unless the entire toolchain is updated
as part of a coordinated migration.

### Featured checks
- `check_rust_1_92_lints.sh` – runs `cargo check` with the Rust 1.92 lint set (including the new never-type fallback and macro-export checks) so stricter diagnostics surface before CI.
- `check_nexus_cross_dataspace_localnet.sh` – runs the deterministic Nexus cross-dataspace all-or-nothing localnet proof (`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`) through `scripts/run_nexus_cross_dataspace_atomic_swap.sh`.
- `check_sumeragi_formal.sh` – runs bounded Apalache checks for the Sumeragi
  commit-path, fork-safety, quorum-policy, RBC deliver-quorum, RBC
  causality gate, RBC DELIVER acceptance gate, RBC commit-processing gate, RBC
  chunk target helper, RBC rebroadcaster selection helper, RBC weighted chunk
  allocation helper, RBC payload chunking helper, RBC RS16 initial fanout
  helper, RBC chunk broadcast order helper, pending-RBC stash gate, RBC
  status lookup helper, RBC status retention/update-pruning helper, RBC
  status persistence/fallback helper, RBC status handle lifecycle helper, RBC
  backlog/status snapshot helper, RBC abort status counter helper, RBC
  mismatch status counter helper, RBC persisted chunk sampling/proof helper,
  RBC persisted session-store guard
  helper, RBC store status accounting helper, RBC store pressure log helper,
  RBC stale-message/payload-refetch helper, RBC signing-preimage gate,
  classic
  Vote/VRF signing-preimage gate, classic
  Vote/QC signature-verification gate, invalid-signature throttle/penalty
  helper, invalid-signature label helper, penalty offender-selection helper,
  consensus penalty-action helper,
  penalty status projection helper, execution-witness root projection helper,
  RBC compact block-message helper,
  consensus block-message priority helper, VRF
  commit/reveal admission gate, classic inbound vote-admission gate, vote
  duplicate-key helper gate, evidence
  freshness horizon helper, evidence canonicalization/deduplication helper,
  evidence validation helper, double-vote detection/recording helper,
  invalid-QC shape helper, QC validation evidence helper, QC validation
  reason/evidence label helper, block-sync QC retry/fallback helper,
  block-sync QC status helper,
  BlockSyncUpdate vote filtering/deferral handoff gate,
  stale BlockCreated/recovery-mode helper,
  exact body fetch handler gate, RBC chunk payload frame-cap helper,
  background frame-cap preparation gate,
  fetch-pending response send gate, deferred block-sync cache/defer integration
  gate, invalid-proposal
  evidence builder helper, proposal
  mismatch helper, proposal cache helper, proposal-hint admission gate,
  stale proposal-hint repair gate, stale RBC hint repair gate,
  proposal metadata admission gate, QC
  signer-bitmap admission, raw QC signer-count helper,
  signer-bitmap construction helper,
  canonical/view signer-index normalization helper,
  direct BlockCreated admission gate,
  BlockSyncUpdate gossip target-selection helper, cached BlockSyncUpdate
  proof/vote attachment helper,
  block-sync warning throttle helper, QC-insufficient warning throttle helper,
  missing-block request clear helper, missing-block clear reason helper,
  proposal budget/cap helper,
  proposal-defer warning throttle helper, proposal batch
  trim/canonicalization helper, commitment snapshot builder helper,
  collector retry/gossip helper,
  collector fanout/selection helper, topology ordered-roster mutation helper,
  topology fanout/redundant-send helper, trusted-peer P2P topology refresh
  helper, quorum retransmit target helper, retransmit backpressure pacing
  helper, paced retransmit target selection helper, quorum reschedule backoff
  helper, DA/RBC availability reschedule gate, vote-backed reassembly stall
  helper, completed quorum view-advance helper, quorum rebroadcast dispatch
  helper, isolated vote-backed frontier
  handoff helper, pre-timeout vote-backed frontier retransmit handoff helper,
  near-quorum preemptive escalation coordinator, manifest-gated quorum
  reschedule helper,
  commit-root consistency, commit-pipeline recovery gate, known-block
  commit-QC recovery helper, commit-anchor QC promotion helper,
  committed-height QC admission helper, pending-progress accounting helper,
  pending-block lifecycle helper, pending-block marker/cooldown helper,
  commit-pipeline scheduling gate, precommit vote-count helper,
  online-validator and relay counter helper, commit-result drain gate,
  commit-drain summary aggregation helper, commit-pipeline timing sample helper,
  commit-pipeline status recorder helper,
  autoscale transition commit gate, commit-QC signer quorum helper,
  signature-index recovery helper, commit-QC cache/history lookup helper,
  cached-QC precommit signer record helper,
  roster-validation memo cache helper,
  roster-validation cached wrapper helper,
  roster-validation core helper,
  roster artifact selection helper,
  block roster cache helper,
  block-sync roster evidence helper,
  block-sync history roster helper,
  persisted block-sync roster selection helper,
  BlockSyncUpdate roster hydration helper,
  prevalidated commit artifact trust helper, commit-job dispatch gate,
  commit-worker channel capacity helper,
  slow commit-stage timing threshold helper,
  commit-evidence replay gate,
  Pacemaker core helper, pacemaker evaluation gate, pacing-governor helper,
  pending fast-path timeout helper, TLC-cross-checked stalled pending-block
  timeout decision helper, TLC-cross-checked stalled pending-frontier timeout
  helper, TLC-cross-checked missing-QC timing helper,
  idle backlog signal helper, TLC-cross-checked proposal-liveness helper,
  TLC-cross-checked actionable vote-backed proposal helper,
  TLC-cross-checked slot proposal evidence helper,
  TLC-cross-checked round-liveness helper,
  TLC-cross-checked roster recovery FSM helper,
  TLC-cross-checked consensus recovery prune helper,
  TLC-cross-checked frontier live-owner work helper,
  TLC-cross-checked keep-frontier-pending-active helper,
  TLC-cross-checked stale-view pending prune helper,
  TLC-cross-checked superseded frontier payload retention helper,
  TLC-cross-checked stale missing-block request prune helper,
  TLC-cross-checked stale missing commit-QC request prune helper,
  TLC-cross-checked stale RBC session prune helper,
  TLC-cross-checked highest-QC defer-marker prune helper,
  TLC-cross-checked fast-finality inline validation helper,
  TLC-cross-checked observer signature-recovery helper,
  TLC-cross-checked validation failure finalize helper,
  TLC-cross-checked validation reject reason-label helper,
  TLC-cross-checked validation reject status helper,
  TLC-cross-checked peer-key policy status helper,
  TLC-cross-checked view-change cause status helper,
  TLC-cross-checked view-change proof status helper,
  TLC-cross-checked QC status helper,
  TLC-cross-checked commit-quorum status helper,
  TLC-cross-checked commit-inflight status helper,
  TLC-cross-checked history status helper,
  TLC-cross-checked RBC abort status helper,
  TLC-cross-checked RBC mismatch status helper,
  TLC-cross-checked RBC progress-stage helper,
  TLC-cross-checked RBC hot-repair helper,
  TLC-cross-checked RBC repair-request helper,
  TLC-cross-checked RBC targeted-repair helper,
  TLC-cross-checked RBC outbound-flush helper,
  TLC-cross-checked RBC chunk-post/debug-mask helper,
  TLC-cross-checked RBC deferral-throttle helper,
  TLC-cross-checked RBC missing-INIT rebroadcast helper,
  TLC-cross-checked RBC sampling helper,
  TLC-cross-checked RBC persisted session-store guard helper,
  TLC-cross-checked RBC store status accounting helper,
  TLC-cross-checked RBC store pressure-log helper,
  TLC-cross-checked round-gap status helper,
  TLC-cross-checked RBC recovery helper,
  TLC-cross-checked RBC missing BlockCreated recovery helper,
  TLC-cross-checked RBC unverified-roster helper,
  TLC-cross-checked RBC signing-preimage helper,
  TLC-cross-checked classic signing-preimage helper,
  TLC-cross-checked classic Vote/QC signature helper,
  TLC-cross-checked invalid-signature label helper,
  TLC-cross-checked invalid-signature throttle/penalty helper,
  TLC-cross-checked penalty offender-selection helper,
  TLC-cross-checked consensus penalty-action helper,
  TLC-cross-checked penalty status projection helper,
  TLC-cross-checked local peer removed status helper,
  TLC-cross-checked execution-witness root projection helper,
  TLC-cross-checked RBC compact block-message helper,
  TLC-cross-checked block-message priority helper,
  TLC-cross-checked block-message height/view helper,
  TLC-cross-checked block-message kind helper,
  TLC-cross-checked message projection helper,
  TLC-cross-checked pipeline event emission helper,
  TLC-cross-checked block-message wire helper,
  TLC-cross-checked BlockCreated frontier wire helper,
  TLC-cross-checked cached proposal rebroadcast helper,
  TLC-cross-checked exact-slot frontier activity helper,
  TLC-cross-checked frontier reassembly activity helper,
  TLC-cross-checked frontier quorum-owner cleanup helper,
  TLC-cross-checked frontier sidecar retarget helper,
  TLC-cross-checked frontier sidecar expected-hash helper,
  TLC-cross-checked contiguous-frontier payload-hint helper,
  TLC-cross-checked frontier parent-QC hint retarget helper,
  TLC-cross-checked frontier proposal grace helper,
  TLC-cross-checked frontier slot tracker FSM gate, TLC-cross-checked
  frontier slot helper, TLC-cross-checked slot tracker state helper,
  TLC-cross-checked timeout/cooldown derivation helper,
  TLC-cross-checked round/view helper,
  TLC-cross-checked PhaseTracker mutable state helper,
  TLC-cross-checked failed-commit/block-sync helper,
  transaction requeue branch helper,
  tick/deadline scheduling helper,
  proposal parent resolution gate, highest-QC dependency deferral gate,
  block-sync recovery gate,
  direct certified-block fetch gate,
  missing-block ingress fetch gate, payload progress availability gate,
  highest-QC fetch body-known gate, local payload availability gate,
  local block-known routing gate, lock-safety block-known routing gate,
  local signed-block materialization gate, authoritative payload progress gate,
  authoritative block payload gate, pending-block active-for-tip gate,
  pending fast-unblock decision gate,
  blocking pending-block counter gate,
  quorum recovery vote-drain urgency gate,
  frontier body-gap payload-drain urgency gate,
  RBC authoritative payload progress gate, slot authoritative payload gate,
  missing-block fetch planner,
  committed-edge conflict suppression gate,
  lock-rejected branch sink gate, missing-block hard-cap recovery gate,
  missing-block hard-cap cleanup gate, missing-block view-change escalation gate, native AMX
  attestation gate, native AMX queue-journal replay gate, native AMX
  routing-plan projection gate, native AMX receipt validation gate, native AMX
  control-plane ingress gate, vNext chain-order helper gate, vNext re-chain
  helper gate, vNext aggregate certificate verification gate, vNext
  signing-preimage gate, vNext control-certificate ingress gate, vNext
  slot-lifecycle gate, vNext validation ownership gate, pending-block
  validation worker config helper, vote/QC verification cache-key identity
  helper, async vote-verification ownership gate,
  TLC-cross-checked vote-signature verification worker config helper, async QC
  aggregate-verification ownership gate, TLC-cross-checked QC
  aggregate-verification worker config helper, TLC-cross-checked commit-worker
  channel capacity helper,
  worker-loop drain scheduler gate, actor-gate priority/fairness gate,
  worker-loop budget/adaptive-cap gate, worker ingress routing gate,
  TLC-cross-checked worker-loop stage helper gate, worker-queue status
  accounting gate,
  NPoS VRF epoch-seal staging gate,
  commit-anchor QC promotion helper, committed-height QC admission helper,
  Kura durability commit retry gate, Kura persistence status helper,
  restarted-peer
  replay gate, precommit vote-emission gate, proposal assembly gate, pure
  engine tick gate, pure engine tick unrelated-state preservation gate, pure
  engine NewView subject projection helper, pure engine top-level
  argument-forwarding gate, pure engine certificate prefilter dispatch gate,
  pure engine certificate prefilter
  state-handoff gate, pure engine certificate prefilter unrelated-state
  preservation gate, pure engine NewView-QC gate, pure
  engine exact NewView-QC highest-QC record gate, pure engine NewView-QC
  unrelated-state preservation gate, pure engine exact NewView-QC advance
  gate, pure engine proposal-ingress gate,
  pure engine exact proposal output-field gate,
  pure engine exact proposal state-mutation gate,
  pure engine proposal unrelated-state preservation gate,
  pure engine exact proposal validation-owner gate,
  pure engine prepare-QC gate, pure engine exact Prepare-QC lock/highest-QC
  record gate, pure engine exact Prepare-QC phase-transition gate, pure engine
  Prepare-QC unrelated-state preservation gate, pure engine commit-QC gate,
  pure engine exact Commit-QC highest-QC record gate,
  pure engine exact Commit-QC phase-transition gate,
  pure engine Commit-QC unrelated-state preservation gate,
  pure engine payload-available Commit-QC exact finality gate,
  pure engine missing-payload Commit-QC pending/fetch gate,
  pure engine Commit-QC validation cleanup gate, pure engine committed-block
  gate, pure engine reconfiguration staging gate, pure engine reconfiguration
  activation-height dedup gate, pure engine committed-block cleanup gate, pure
  engine exact payload-availability record gate, pure
  engine committed-block unrelated-state preservation gate, pure engine
  payload-availability gate, pure engine payload-availability unrelated-state
  preservation gate, pure engine
  validation-result gate, pure engine validation-result unrelated-state
  preservation gate, pure engine exact validation-owner cleanup gate,
  pure engine exact invalid-validation round/output advance gate,
  pure engine constructor initial-state gate, pure engine read-only accessor
  gate, pure engine top-level output relay gate,
  view-advance saturation, QC-round
  compatibility helper, proposal-lock helper, QC reference projection helper,
  QC reference comparator helper, highest-QC record helper, commit-subject helper,
  payload-lookup helper,
  prepare-vote cache/output helper, validator-set transition, certified-recovery,
  view-change/lock-safety, validation-callback, validation-priority helper,
  vote-backed evidence helper, vote payload actionable helper,
  actionable vote-backed proposal evidence helper, slot proposal evidence
  helper, round-liveness helper, roster recovery FSM helper,
  consensus recovery prune helper,
  frontier live-owner work helper,
  keep-frontier-pending-active helper, stale-view pending prune helper,
  superseded frontier payload retention helper,
  stale missing-block request prune helper, stale missing commit-QC request
  prune helper, stale RBC session prune helper, highest-QC defer marker prune
  helper, fast-finality inline validation helper,
  observer signature-mismatch recovery
  helper, validation failure finalization helper, validation-reject reason
  label helper, validation-reject status counter helper,
  view-change proof/index status counter helper, validation
  evidence QC selector helper, certificate-admission,
  same-height vote conflict helper, aggregate same-height vote-lock helper,
  proposal stale same-height vote helper,
  same-height vote recovery view-gap helper, tip-extension helper, DA gate
  helper, consensus handshake capability construction helper, consensus
  handshake helper, runtime mode flip helper, effective mode
  selection helper, effective timing aggregation helper, NEW_VIEW stats helper,
  NEW_VIEW tracker helper,
  timing monitor helper, hotspot summary accumulator helper,
  adaptive observability timing/fanout helper, pacing backpressure helper,
  counter-driven backpressure cooldown helper, locked-QC helper, stake snapshot
  quorum helper, live local-vote roster helper, canonical round-roster helper,
  vote-roster selection helper, vote-roster cache/support helper,
  commit-topology state/reset helper, roster index projection helper,
  membership-view hash helper, membership mismatch status helper,
  membership advert helper, membership mismatch ingress helper,
  consensus params ingress helper,
  precommit signer-history fallback helper, missing-block request clear helper,
  missing-block clear reason helper, recovery status counter helper,
  recovery-FSM reason classifier helper,
  QC rebuild status counter helper,
  collector-targeting status helper, missing-QC liveness status helper,
  sidecar/no-proposal status helper, deterministic committee status helper,
  timing/liveness status counter helper, roster-recovery status helper,
  peer-key policy status helper,
  view-change cause status helper,
  highest-QC selection, optional
  highest-QC selection filter, RBC block-body
  repair admission helper, and
  frontier-recovery TLA+ models, the small TLC
  frontier/QC signer-count/signer-index normalization/precommit vote-count/
  commit-quorum signers/commit-QC lookup/precommit signer record/voting
  signer-count/collector-plan/validation ownership cleanup/worker-loop stage/
  worker tick-gap/vNext performance config/validation worker config/
  commit-worker config/commit-stage timing threshold/commit-inflight timeout/
  post-commit pacemaker kick/idle-view proposal budget/cached-slot timeout/
  pending fast-path timeout/live-frontier idle missing-QC/missing-QC reacquire
  admission/missing-QC reacquire action/missing commit-QC actionable/idle backlog
  signals/missing-QC height stall/missing-QC stall range-pull cross-checks, and
  expected-failure
frontier/fork/quorum/RBC/rbc-causality/rbc-deliver-acceptance/rbc-commit-processing/rbc-chunk-target/rbc-chunk-payload-cap/rbc-rebroadcast-selection/rbc-chunk-allocation/rbc-payload-chunking/rbc-rs16-initial-fanout/rbc-chunk-broadcast-order/pending-rbc-stash/pending-rbc-status/ingress-status-counters/consensus-message-labels/phase-latency-status/telemetry-status/lane-detail-status/settlement-status/history-status/commit-quorum-status/commit-inflight-status/rbc-status-lookup/rbc-status-retention/rbc-status-persistence/rbc-status-handle/rbc-backlog-status/rbc-abort-status/rbc-mismatch-status/rbc-progress-stage/rbc-hot-repair/rbc-sampling/rbc-store/rbc-store-status/rbc-store-pressure-log/round-gap-status/rbc-recovery-helper/rbc-missing-block-recovery/rbc-unverified-roster/rbc-preimage/classic-preimage/classic-signature/invalid-signature-labels/invalid-signature-throttle/penalty-offender-selection/consensus-penalty-action/exec-witness-roots/block-message-rbc-compact/block-message-priority/block-message-height-view/block-message-kind/message-projection/pipeline-event-emission/block-message-wire/block-created-frontier-wire/cached-proposal-rebroadcast/frontier-same-slot-activity/frontier-reassembly-activity/frontier-quorum-owner-actionable/frontier-sidecar-retarget/frontier-sidecar-expected-hash/contiguous-frontier-payload-hint/frontier-parent-qc-hint-retarget/live-frontier-idle-missing-qc/missing-qc-reacquire-admission/missing-qc-reacquire-action/missing-commit-qc-actionable/missing-qc-height-stall/missing-qc-stall-range-pull/canonical-frontier-reanchor/frontier-repair-view-change/frontier-recovery-advance/same-height-no-proposal-storm/vrf-admission/vote-admission/vote-duplicate-key/evidence-horizon/evidence-canonicalization/evidence-validation/double-vote-recording/invalid-qc-shape/qc-validation-evidence/qc-validation-reason/block-sync-qc-fallback/signed-quorum-fetch-fallback/commit-qc-only-fetch-response/block-sync-update-targets/apply-cached-qcs/block-sync-roster/block-sync-vote-deferral/block-sync-known-hintless/block-sync-implicit-recovery/block-sync-vote-placeholder/block-sync-snapshot-hint/block-sync-snapshot-roster/block-sync-no-roster/block-sync-known-selected-roster/block-sync-selected-signatures/block-sync-selected-qc/block-sync-selected-quorum/block-sync-recovery-mode/block-sync-selected-apply/block-sync-selected-qc-prefilter/block-sync-selected-qc-process/block-sync-selected-qc-cache/block-sync-stale-view/block-sync-commit-conflict/block-sync-warning-throttle/fetch-response-deferral/fetch-block-body-handle/background-frame-cap/fetch-pending-response-send/fetch-pending-responses-batch/pending-response-flush/deferred-block-sync-helper/deferred-block-sync-cache/deferred-block-sync-replay/block-sync-future-window/invalid-proposal-evidence/proposal-mismatch/proposal-cache/proposal-hint/stale-proposal-hint-repair/stale-rbc-hint-repair/proposal-admission/block-created-admission/missing-request-clear/missing-block-clear/proposal-budget/proposal-backpressure/proposal-defer-warning/non-rbc-payload-budget/proposal-batch/lane-interleave/commitment-snapshot-builder/collector-plan/collector-selection/topology-mutation/prf-leader-shuffle/topology-fanout/p2p-topology-trusted/p2p-topology-refresh/quorum-retransmit/retransmit-backpressure/quorum-reschedule-backoff/rbc-availability-reschedule/vote-backed-reassembly-stall/completed-quorum-view-advance/QC-signer/qc-signer-count/signer-index-normalization/commit-root/commit-pipeline-recovery/known-block-commit-qc-recovery/stale-view-commit-qc-fetch/commit-anchor-qc/pending-progress/commit-pipeline-scheduling/precommit-vote-count/voting-signer-count/distinct-vote-epochs/new-view-highest-qc-votes/online-validator-relay-counters/commit-result-drain/commit-drain-summary/commit-pipeline-sample/commit-pipeline-status/autoscale-transition/commit-quorum-signers/signature-index-recovery/commit-qc-lookup/precommit-signer-record/roster-validation-memo/roster-validation-cached/roster-validation-core/roster-artifact-selection/block-roster-caches/block-sync-roster-evidence/block-sync-history-roster/persisted-roster-selection/block-sync-update-roster/roster-index-projection/membership-view-hash/membership-mismatch-status/membership-advert/membership-mismatch-ingress/consensus-params-ingress/prevalidated-commit-artifact/commit-job-dispatch/commit-worker-config/commit-stage-timing-threshold/commit-inflight-timeout/post-commit-pacemaker-kick/idle-view-proposal-budget/pacemaker-core/pacemaker-evaluation/pacing-governor/cached-slot-timeout/pending-fast-path-timeout/stalled-pending-timeout/stalled-pending-frontier-timeout/missing-qc-timing/idle-backlog-signals/proposal-liveness/frontier-slot-tracker/frontier-slot-helpers/frontier-proposal-grace/slot-tracker-state/timeout-derivation/round-view-helpers/phase-tracker/round-trace-status/failure-recovery-helpers/requeue-transactions/tick-deadline-helpers/proposal-parent-resolution/highest-qc-dependency-deferral/precommit-QC-view-change/commit-evidence-replay/block-sync-recovery/certified-fetch/missing-block-ingress-fetch/payload-progress-availability/highest-qc-fetch-body-known/local-payload-availability/block-known-locally/block-known-for-lock/missing-locked-qc-recovery/local-signed-block-lookup/authoritative-payload-progress/authoritative-block-payload/pending-block-active-for-tip/pending-fast-unblock/blocking-pending-blocks/quorum-recovery-vote-drain/frontier-body-gap-payload-drain/rbc-authoritative-payload-progress/slot-authoritative-payload/missing-block-fetch/recovery-status-counters/deferred-recovery-status/range-pull-recovery/range-pull-status/active-lock-reject-recovery/missing-block-hard-cap/missing-block-hard-cap-cleanup/missing-block-view-change/native-AMX-attestation/native-AMX-journal/native-AMX-receipt/native-AMX-ingress/vnext-chain-order/vnext-rechain/vnext-rechain-error-label/vnext-signature/vnext-signing-preimage/vnext-control-ingress/vnext-slot-lifecycle/vnext-validation/validation-worker-config/verify-cache-key/vote-verify-async/vote-verify-worker-config/qc-verify-async/qc-verify-worker-config/worker-drain/actor-gate/worker-budget/worker-ingress/worker-loop-stage/npos-vrf/kura-commit/kura-store-status/restart-replay/post-commit-cleanup/frontier-gap-realign/same-height-vote-conflict/proposal-stale-vote/same-height-vote-recovery-gap/tip-extension-helpers/da-gate/consensus-handshake-caps/handshake/mode-flip/effective-mode/effective-timing/new-view-stats/new-view-tracker/timing-monitor/hotspot-log-summary/adaptive-observability/pacing-backpressure/counter-backpressure-cooldown/locked-qc-helper/stake-snapshot/live-vote-roster/canonical-round-roster/vote-roster-selection/vote-roster-cache/commit-topology-state/precommit-signer-history/precommit/proposal/engine-initial-state/engine-read-accessors/engine-tick/engine-tick-state-preservation/engine-new-view-subject/engine-handle-dispatch/engine-handle-forwarding/engine-handle-output-relay/engine-certificate-dispatch/engine-certificate-prefilter-state/engine-certificate-prefilter-state-preservation/engine-view-advance-saturation/engine-new-view/engine-new-view-highest-qc/engine-new-view-state-preservation/engine-new-view-advance/engine-proposal/engine-proposal-output/engine-proposal-state/engine-proposal-state-preservation/engine-proposal-validation-owner/engine-proposal-lock/qc-round-compatibility/engine-QC-ref-projection/engine-QC-ref-comparator/engine-highest-QC-record/engine-commit-subject/engine-payload-lookup/engine-prepare/engine-prepare-lock-highest/engine-prepare-phase/engine-prepare-vote-cache/engine-commit/engine-commit-highest-qc/engine-commit-phase/engine-commit-state-preservation/engine-commit-available-commit/engine-commit-pending-fetch/engine-commit-validation-cleanup/engine-committed-block/engine-committed-block-record/engine-reconfiguration-staging/engine-reconfiguration-dedup/engine-committed-block-cleanup/engine-committed-block-state-preservation/engine-payload-record/engine-payload/engine-payload-state-preservation/engine-validation-result/engine-validation-state-preservation/engine-validation-ownership/engine-validation-invalid-advance/reconfiguration/recovery/view-change/validation/validation-priority/vote-backed-evidence/vote-payload-actionable/actionable-vote-backed-proposal/slot-proposal-evidence/round-liveness/frontier-live-owner-work/keep-frontier-pending-active/stale-view-pending-prune/superseded-frontier-payload-retention/stale-missing-block-request-prune/fast-finality-inline-validation/observer-signature-recovery/validation-failure-finalize/validation-reject-reason-label/validation-reject-status/peer-key-policy-status/view-change-cause-status/validation-evidence-qc/admission/highest-QC/highest-optional
  mutations.
  The slash-form coverage summary also includes
  `quorum-rebroadcast-dispatch` for the pending rebroadcast dispatch helper and
  `isolated-vote-backed-handoff` for the one-vote frontier handoff helper, plus
  `preemptive-vote-backed-retransmit` for the pre-timeout retransmit handoff,
  plus `near-quorum-preemptive-escalation` for pre-timeout missing-payload
  recovery escalation, plus `paced-retransmit-targets` for deterministic
  backlog-throttled target selection.
  The Sumeragi suite also includes the `recovery-fsm-reason` helper slice for
  recovery reason string classification, deterministic ranks, and
  height/rank/peer recovery-event ordering.
  The Sumeragi suite also includes the per-reason pacemaker backpressure
  tracker helper slice for deferring gates, telemetry labels, and duration
  transitions.
  The Sumeragi suite also includes the distinct-vote-epochs helper slice for
  cached vote-log Commit-QC replay after payload recovery.
  The Sumeragi suite also includes the requeue-transactions helper slice for
  committed-duplicate, routing, push-outcome, gossip, and pending-drop counters
  after failed commit recovery.
  The `pending-rbc-status` family covers pending-RBC drop/stash/eviction
  counters, reset behavior, atomic overlay projection, and per-entry status
  snapshots.
  The `ingress-status-counters` family covers inbound consensus gossip,
  retransmit, background-drop, block-created, dedup-eviction, and
  message-handling status counters.
  The `consensus-message-labels` family covers stable exported labels for
  consensus message kind, handling outcome, and handling reason dimensions.
  The `qc-rebuild-status` family covers QC rebuild attempts, successful
  rebuilds, accepted QCs with missing local votes, quorum-without-QC
  observations, reset behavior, and snapshot projection.
  The `collector-targeting-status` family covers current collector-target
  storage, last-commit collector-target storage, redundant-send accumulation,
  reset behavior, and snapshot projection.
  The `phase-latency-status` family covers latest/max/EMA phase latency
  projection and saturated pipeline totals in `phase_latencies_snapshot()`.
  The `telemetry-status` family covers availability vote counters, QC latency
  overwrite/sort semantics, and direct RBC/pipeline status projections.
  The `lane-detail-status` family covers lane commitment, relay, governance,
  Nexus-disabled stripping, and route gating for lane-detail status fields.
  The `settlement-status` family covers DvP/PvP settlement telemetry reset,
  event counters, last-event snapshots, and JSON status projection.
  The `nexus-economics-status` family covers Nexus fee debit outcome counters,
  public-lane staking deltas, status projection, reset accounting, and strip
  behavior when Nexus lane details are disabled.
  The `npos-repair-coverage-status` family covers NPoS repair fanout coverage
  recording, reset behavior, direct snapshots, and mode-gated status projection.
  The `mode-status` family covers PRF context publication, mode tag and
  activation-lag status, mode-flip kill switch, blocked state, counters,
  timestamps, and last-error projection.
  The `consensus-caps-status` family covers consensus capability storage,
  getter behavior, overwrite semantics, and top-level status projection.
  The `effective-timing-status` family covers effective timing scalar,
  scheduling, fanout, optional NPoS timeout, clear, overwrite, and top-level
  status projection semantics.
  The `tx-queue-backpressure-status` family covers transaction queue
  backpressure depth/capacity storage, explicit saturation state, getter
  projection, overwrite behavior, and top-level status projection.
  The `view-change-proof-status` family covers view-change index storage,
  proof accepted/stale/rejected counters, local suggest/install counters,
  reset semantics, and top-level status projection.
  The `qc-status` family covers leader index storage, highest-QC tuple/subject
  projection, locked-QC monotonic updates, reset, same-tuple subject updates,
  lower-tuple rejection, and top-level status projection.
  Its TLC cross-check independently exhausts the same mutation family for
  initial subject absence, leader storage/overwrite, highest-QC tuple and
  subject storage, highest-QC getter and snapshot projection, highest-QC
  overwrites, locked-QC reset, higher locked-QC admission, lower locked-QC
  rejection, same-tuple subject updates, and locked-QC snapshot projection.
  The `history-status` family covers checkpoint, commit-QC, NPoS election, and
  consensus-key history ordering, retention, route projection, and snapshots.
  Its TLC cross-check independently exhausts the same mutation family for
  checkpoint reset/append/newest-first/cap/route projection, commit-QC
  same-block replacement, distinct-block preservation, ordering, retention,
  route windows, latest snapshot projection, NPoS latest/cap/snapshot
  projection, and consensus-key replacement/cap/route projection.
  The `commit-quorum-status` family covers commit-quorum tally reset, record,
  snapshot, JSON, and typed status projection semantics.
  Its TLC cross-check independently exhausts the same mutation family for
  reset round/hash/count/timestamp cleanup, record round/hash/count storage,
  timestamp refresh, overwrite behavior, empty/default snapshots, snapshot
  field projection, JSON projection, and typed Torii status projection.
  The `commit-inflight-status` family covers commit-inflight reset, start,
  finish, timeout, elapsed, JSON, and typed status projection semantics.
  Its TLC cross-check independently exhausts the same mutation family for
  reset active/hash/timing/counter cleanup, timeout configuration, start
  identity/hash/timing fields, pause/resume counters and queue depths,
  finish no-op and matching-finish semantics, elapsed projection, timeout
  recording, top-level snapshot projection, JSON projection, and typed Torii
  status projection.
  The `rbc-abort-status` family covers RBC abort totals, latest height/view
  updates, lower-height and zero-slot recording, direct snapshot projection,
  top-level status projection, and reset semantics.
  Its TLC cross-check independently exhausts the same mutation family for
  empty/post-record reset, first and repeated abort accounting, latest slot
  updates, lower-height acceptance, zero-slot acceptance, direct snapshots,
  and top-level status projection.
  The `rbc-mismatch-status` family covers mismatch-kind labels, per-peer and
  per-kind counters, timestamp refresh, snapshot preservation, saturation,
  and reset semantics.
  Its TLC cross-check independently exhausts the same mutation family for
  kind-label stability, first-record bucket routing, same-peer same-kind
  accumulation, same-peer different-kind independence, different-peer
  separation, timestamp set/update behavior, snapshots, saturation, and
  reset-after-records cleanup.
  The `rbc-progress-stage` family covers monotone RBC progress-stage
  advancement, progressed flags, observation sync ordering, READY-quorum
  non-advancement, delivered/no-regression handling, authoritative-roster
  quorum derivation, and roster-driven payload/READY/DELIVERED projection.
  Its TLC cross-check independently exhausts the same mutation family for
  skipped/regressing advancement, incorrect progressed flags, observation
  cascade ordering, quorum-only advancement, delivered-state regression,
  authoritative/non-authoritative roster quorum selection, and roster payload
  projection.
  The `rbc-hot-repair` family covers active hot-repair session selection,
  delivered-session suppression, exact-frontier recovered chunk repair
  admission, urgent near-tip classification, payload backpressure exemption,
  and proposal blocking for pending/inflight/processing RBC work.
  Its TLC cross-check independently exhausts the same mutation family for
  aborted/wrong-slot/invalid-session activity, delivered-session retention,
  local candidate/payload suppression, exact-repair admission gates, urgent
  near-tip filtering, payload-exemption gates, and proposal-blocking decisions.
  The `rbc-repair-request` family covers repair cooldown due decisions,
  deterministic target ordering, local-peer filtering, target deduplication and
  truncation, missing READY projection, INIT request state, and chunk repair
  request/fallback state.
  Its TLC cross-check independently exhausts the same mutation family for
  first-send/boundary/zero-cooldown/future-clock handling, target selection,
  deterministic leader preference, READY bitmap projection, INIT request
  recording/fallback, chunk progress requests, no-target preservation, and
  cooldown fallback.
  The `rbc-targeted-repair` family covers targeted READY and DELIVER repair
  rejection gates, remote-target deduplication, message counts, send-record
  updates, rescue early-reject gates, READY repair due handling, payload repair
  recording, authoritative READY repair, and delivered missing-commit-QC
  cooldown handling.
  Its TLC cross-check independently exhausts the same mutation family for
  empty/local/missing-roster READY sends, duplicate target collapse,
  READY/DELIVER message counts, send record updates, observer/DA-disabled/
  committed/suppressed/invalid rescue rejection, payload timestamp recording,
  authoritative READY payload rules, and max-cooldown missing-QC repair.
  The `rbc-outbound-flush` family covers observer and DA-disabled gates,
  empty-queue cursor clearing, relay and queue backpressure stops, budget
  flooring, cursor order and wraparound, payload-exempt queue sends,
  zero-send cleanup, budget consumption, and cursor/progress reporting.
  Its TLC cross-check independently exhausts the same mutation family for
  observer/DA-disabled/empty/relay/queue rejection, minimum-budget handling,
  cursor wraparound, exempt-only queue backpressure sends, zero-send cleanup,
  budget consumption, cursor update, and all-skipped cursor preservation.
  The `rbc-chunk-post-debug` family covers debug-mask bounds, selected-bit
  handling, withhold/equivocation mask predicates, local and out-of-range
  scheduling skips, disallowed and unexpected chunk suppression, canonical/
  compact/equivocated post frame selection, post and skip counting, and READY
  signature forking.
  Its TLC cross-check independently exhausts the same mutation family for
  mask index 64 rejection, low-bit honoring, high-chunk withholding,
  validator/chunk equivocation-mask conjunction, debug-drop/local/oob/
  disallowed/unexpected scheduling skips, cached versus fresh frame selection,
  post/skip count accounting, and READY fork mutation.
  The `rbc-deferral-throttle` family covers DELIVER and READY deferral
  first-observation logging, no-progress suppression, strictly increasing
  progress counters, regression rejection, reason/required/total-change
  progress, exact cooldown-boundary logging, zero-cooldown logging,
  backwards-clock saturation, and state replacement on admitted emissions.
  Its TLC cross-check independently exhausts the same mutation family for
  vacant-state admission, pre-cooldown suppression, READY/received progress,
  regression-only observations, total/required/reason changes, boundary and
  zero cooldown, backwards clocks, and stale-state retention bugs.
  The `rbc-missing-init-rebroadcast` family covers missing-INIT rebroadcast
  admission order: observer/DA/hot-repair/payload-recovery/activity/
  backpressure/roster gates, backpressure exemption, requested and waiting
  targeted INIT repair short-circuits, missing bundle suppression, broad INIT
  companion emission, cached chunk forwarding, and READY-count forwarding.
  Its TLC cross-check independently exhausts the same mutation family for
  early-gate leakage, non-exempt/exempt backpressure behavior, roster-missing
  suppression, repair short-circuit recording, fallback/not-needed broad
  rebroadcast admission, missing-bundle suppression, and broad payload counts.
  The `rbc-sampling` family covers persisted chunk sampling load outcomes,
  invalid sample-count rejection, incomplete/proof-error fail-closed handling,
  sorted unique in-range sample selection, sample-count cardinality,
  requested-key metadata, height/view/total metadata, chunk-root presence, and
  optional payload-hash binding.
  Its TLC cross-check independently exhausts the same mutation family for
  absent/I/O/invalid persisted loads, zero/oversized sample counts,
  incomplete digest/root state, missing proof material, excessive proof depth,
  sample ordering/duplicate/range/count drift, and metadata binding drift.
  The `rbc-store` family covers persisted session-store guard behavior:
  software-manifest compatibility, direct load temp/main precedence,
  destructive validation and deletion, chunk-integrity rejection, TTL/capacity
  eviction pressure, temporary path construction, and session filename
  classification. Its TLC cross-check independently exhausts the same
  fifty-eight expected-failure configs as Apalache.
  The `rbc-store-status` family covers status accounting for RBC store
  pressure snapshots, backpressure and persist-drop counters, eviction totals,
  recent-eviction ordering/capping, snapshot projection, and reset cleanup. Its
  TLC cross-check independently exhausts the same twenty-three
  expected-failure configs as Apalache.
  The `rbc-store-pressure-log` family covers RBC store pressure log
  throttling: pressure label mapping, transition logging, normal-repeat
  suppression, elevated repeat interval boundaries, backwards-clock saturation,
  logged-state updates, suppressed-state preservation, and reset cleanup. Its
  TLC cross-check independently exhausts the same twenty-two expected-failure
  configs as Apalache.
  The `round-gap-status` family covers marker storage, first-marker
  preservation, incomplete/mismatched isolation, duration recording,
  out-of-order saturation, EMA initialization/blending, marker-cap pruning,
  overflow clamping, snapshot projection, and reset cleanup. Its TLC
  cross-check independently exhausts the same thirty-two expected-failure
  configs as Apalache.
  The `rbc-recovery-helper` family covers stale RBC message commitment and
  payload-refetch decisions: stale/current message handling, Kura presence,
  future-message rejection, invalid-session suppression, delivered/complete
  session suppression, payload-hash mismatch fetches, missing-payload fetches,
  and zero-chunk completeness. Its TLC cross-check independently exhausts the
  same fourteen expected-failure configs as Apalache.
  The `rbc-missing-block-recovery` family covers known-local bypass,
  BlockCreated metadata recovery, forced frontier body fetches, signer fallback
  mode thresholds, near-frontier suppression rejection, far-future suppression,
  request cleanup, height-recovery cleanup, and range-pull reanchor triggers.
  Its TLC cross-check independently exhausts the same thirty expected-failure
  configs as Apalache.
  The `rbc-unverified-roster` family covers the permissioned unverified-roster
  escape hatch and roster availability: active fallback use, same-epoch
  payload fallback, vote-roster recovery, next-epoch payload rejection, NPoS
  rejection, empty active fallback handling, and non-empty roster rejection.
  Its TLC cross-check independently exhausts the same eleven expected-failure
  configs as Apalache.
  The `rbc-preimage` family covers RBC READY/DELIVER signing-preimage binding:
  chain/mode/version domain fields, message type tags, block subject metadata,
  sender and chunk root binding, self-signature exclusion, READY count binding,
  and DELIVER READY-bundle entry ordering, sender, length, and signature
  material. Its TLC cross-check independently exhausts the same twenty
  expected-failure configs as Apalache.
  The `classic-preimage` family covers classic Vote/VRF signing-preimage
  binding: consensus domain fields, vote/VRF type separation, vote subject
  roots with round and chain-order context, optional highest-QC
  presence/absence/body fields, VRF commit/reveal body binding, and mutable
  signature/certificate exclusion. Its TLC cross-check independently exhausts
  the same thirty-two expected-failure configs as Apalache.
  The `classic-signature` family covers classic Vote/QC signature
  verification: mode and validator-set binding, signer bitmap shape and roster
  bounds, count/stake quorum, aggregate signature and PoP gates, vote
  availability, vote subject/root/signature/view mapping checks, NewView
  highest-QC agreement, and returned-signer contract. Its TLC cross-check
  independently exhausts the same twenty-seven expected-failure configs as
  Apalache.
  The `invalid-signature-labels` family covers invalid-signature telemetry
  labels: kind labels, telemetry wrapper identity, logged/throttled outcome
  labels, RBC mismatch `should_log` semantics, and label-set distinctness. Its
  TLC cross-check independently exhausts the same nine expected-failure configs
  as Apalache.
  The `invalid-signature-throttle` family covers invalid-signature throttling
  and penalty behavior: invalid-vote and RBC mismatch log keying,
  first/within-window/boundary outcomes, height/view advance bypasses,
  retention pruning, RBC `should_log`, penalty threshold-zero disablement,
  cooldown suppression/expiry, window reset, zero-cooldown behavior, and
  penalty prune boundaries. Its TLC cross-check independently exhausts the same
  thirty-two expected-failure configs as Apalache.
  The Sumeragi suite also includes the non-rbc-payload-budget helper slice for
  deterministic non-RBC proposal payload frame-cap derivation.
  The Sumeragi suite also includes the rbc-chunk-payload-cap helper slice for
  deterministic RBC chunk payload frame-cap derivation.
  The Sumeragi suite also includes the proposal-backpressure helper slice for
  separating pacing-only deferral from hard proposal backpressure.
  The Sumeragi suite also includes the proposal-defer-warning helper slice for
  deterministic proposal deferral warning throttling.
  The Sumeragi suite also includes the peer-admin-detection helper slice for
  signed external instruction-batch admin detection.
  The Sumeragi suite also includes the `qc-insufficient-warning` helper slice
  for deterministic QC-insufficient warning keying, cooldown, GC, and clear
  behavior.
  The Sumeragi suite also includes the lane-interleave helper slice for
  deterministic routing-decision round-robin order across lanes.
  The Sumeragi suite also includes the prf-leader-shuffle helper slice for
  deterministic PRF topology permutations, view-cycled leader selection, and
  canonical shuffled roster construction.
  The Sumeragi suite also includes the worker-tick-gap helper slice for
  saturating tick elapsed-time gating and idle wait derivation, with a matching
  TLC mutation cross-check.
  The Sumeragi suite also includes the vnext-performance-config helper slice
  for deterministic duration-to-millisecond saturation, vNext performance
  config field preservation, and TLC mutation cross-checks.
  The Sumeragi suite also includes the validation-worker-config helper slice
  for pending-block validation worker count clamping and queue-cap derivation,
  with a matching TLC mutation cross-check.
  The Sumeragi suite also includes the vnext-stake-weight helper slice for
  first-match stake lookup, strict NPoS stake quorum, and arithmetic fail-closed
  behavior.
  The Sumeragi suite also includes the validation-redrive-label helper slice
  for stable, nonzero, pairwise distinct redrive reason labels and status
  strings.
  The Sumeragi suite also includes the validation-ownership-cleanup helper
  slice for block-scoped validation ownership removal, vNext slot cleanup, and
  empty-round pruning; the TLC helper independently cross-checks the same
  bounded mutation family.
  The Sumeragi suite also includes the proposal-liveness helper slice for
  missing-QC state transitions, slot reset/ensure behavior, and mark-state
  updates; the TLC helper independently cross-checks the same bounded mutation
  family.
  The Sumeragi suite also includes the NEW_VIEW highest-QC vote-selection
  helper slice for accepted-vote filtering, exact grouping, and deterministic
  candidate ranking.
  The Sumeragi suite also includes the vote-duplicate-key helper slice for
  raw vote-log keys, NEW_VIEW highest-QC duplicate equality, and public-key
  identity projection.
  The Sumeragi suite also includes the vote-validation-drop-status helper slice
  for stable drop labels, bounded newest-first recent entries, peer/roster
  aggregates, status projection, and decadic log thresholds.
  The `penalty-offender-selection` family covers evidence penalty attribution:
  permissioned canonical index rotation, range and empty-topology rejection,
  duplicate/sorted offender indices, evidence source selection, invalid-QC
  bitmap expansion, censorship anchors, NPoS leader binding, epoch and
  consensus-mode derivation, and roster fallback ordering. Its TLC cross-check
  independently exhausts the same thirty-two expected-failure configs as
  Apalache.
  The `consensus-penalty-action` family covers consensus evidence penalty
  actions: applied/cancelled/delay eligibility filters, missing
  roster/seed/offender/slash pending behavior, slash and applied-marker
  derivation, legitimate empty invalid-QC marking, empty non-legitimate
  evidence skips, action sort/dedup, and transaction outcome mutations for
  slash and mark actions. Its TLC cross-check independently exhausts the same
  twenty-eight expected-failure configs as Apalache.
  The `penalty-status` family covers penalty status projection: initial zero
  state, VRF penalty snapshots, late-reveal updates, epoch scheduling
  parameters, consensus/VRF applied and pending counters, accumulation and
  overwrite semantics, getter projections, and status snapshot fields. Its TLC
  cross-check independently exhausts the same twenty-three expected-failure
  configs as Apalache.
  The `local-peer-removed-status` family covers local peer removed flag
  storage and reads: initial present state, removed/present writes, getter
  projections, overwrite ordering, repeated-write idempotence, and getter
  side-effect freedom. Its TLC cross-check independently exhausts the same
  eleven expected-failure configs as Apalache.
  The `exec-witness-roots` family covers execution-witness root projection:
  post-root read/write selection, parent-root prevalue filtering, canonical
  root input ordering/deduplication, commit prevalidation root matching, and
  FASTPQ public-input root binding. Its TLC cross-check independently exhausts
  the same twenty expected-failure configs as Apalache.
  The `block-message-rbc-compact` family covers RBC chunk compact block-message
  helpers: compact boundary admission, payload field preservation,
  full-message fallback, normalization, height/view/epoch widening, and
  high-priority routing. Its TLC cross-check independently exhausts the same
  fourteen expected-failure configs as Apalache.
  The `block-message-priority` family covers high network priority for every
  consensus block-message variant: block sync, body fetch, VRF and
  execution-witness material, RBC messages, proposal hints, proposals, QC
  votes, and QCs. Its TLC cross-check independently exhausts the same
  twenty-two expected-failure configs as Apalache.
  The `block-message-height-view` family covers consensus block-message slot
  projection: no-slot exclusions, slot-bearing future-window eligibility,
  source selection, compact chunk widening, and height/view ordering. Its TLC
  cross-check independently exhausts the same fifteen expected-failure configs
  as Apalache.
  The `block-message-kind` family covers block-message log/status kind
  projection: certified-fetch subtype labels, NewView vote/certificate labels,
  compact/full RBC chunk collapse, future-window log labels, coarser status
  telemetry, and Kura status omission. Its TLC cross-check independently
  exhausts the same seventeen expected-failure configs as Apalache.
  The `message-projection` family covers consensus message projection helpers:
  timing guard admission, exact timing labels and header fields, elapsed-ms
  saturation, control evidence labels, and native-AMX request/vote and
  prepare/commit labels. Its TLC cross-check independently exhausts the same
  twenty-four expected-failure configs as Apalache.
  The `pipeline-event-emission` family covers pipeline event forwarding
  envelopes: empty no-op behavior, single-event wrapping, ordered batch
  wrapping, duplicate preservation, closed-sender failure logging, open-sender
  delivery, and no-panic behavior. Its TLC cross-check independently exhausts
  the same seventeen expected-failure configs as Apalache.
  The `block-message-wire` family covers cached block-message Norito frames:
  cache construction, cached/uncached serialization, mutation cache
  invalidation, header rejection, exact prefix consumption, trailing-byte
  preservation, decode output, and cache preservation. Its TLC cross-check
  independently exhausts the same twenty expected-failure configs as Apalache.
  The `block-created-frontier-wire` family covers `BlockCreated` frontier
  metadata helpers: constructor metadata absence, `with_frontier(...)`
  preservation, proposal/RBC metadata copy, generic/proposal/local wire rebuild
  fallback and rejection, and cached rebroadcast admission. Its TLC cross-check
  independently exhausts the same thirty-four expected-failure configs as
  Apalache.
  The `cached-proposal-rebroadcast` family covers cached proposal replay:
  unsafe-signal admission rejection, normal/recovery backpressure distinction,
  cooldown selection, hint/authoritative rebuild caching, remote-leader relay,
  remote-only fanout, and successful block-hash returns. Its TLC cross-check
  independently exhausts the same twenty-five expected-failure configs as
  Apalache.
  The `frontier-same-slot-activity` family covers exact-slot frontier recovery
  activity helpers: payload progress evidence, ingress backlog/payload gates,
  vote-backed activity evidence, missing-block, missing-commit-QC, and
  missing-payload actionability, old-view and wrong-height suppression,
  stale-window rejection, and bookkeeping-only refresh exclusion. Its TLC
  cross-check independently exhausts the same thirty-six expected-failure
  configs as Apalache.
  The `frontier-reassembly-activity` family covers frontier reassembly
  activity: fresh dependency progress with payload backlog, exact same-slot
  ingress, same-height RBC sender and deferral work, validation work, deferred
  block-sync updates, stale and wrong-height/view rejection, and no-source
  suppression. Its TLC cross-check independently exhausts the same thirty-two
  expected-failure configs as Apalache.
  The `frontier-quorum-owner-actionable` family covers live contiguous-frontier
  cleanup preservation: owner, vote, dependency backlog, RBC sender,
  missing-block, missing-commit-QC, and vote-backed recovery sources, stale or
  wrong-view rejection, passive-work rejection, committed+1 height gating,
  current-view gating, and no-actionable-source suppression. Its TLC
  cross-check independently exhausts the same twenty expected-failure configs
  as Apalache.
  The `frontier-sidecar-retarget` family covers contiguous-frontier sidecar
  retargeting: narrow override reasons, quarantine and stall/progress gates,
  confirmation by local payload, commit QC, or override, tracked and untracked
  sidecar routing, commit-certified reacquire with local evidence, and
  rejection of missing expected hashes, same-hash sidecars, and authoritative
  payloads. Its TLC cross-check independently exhausts the same twenty-seven
  expected-failure configs as Apalache.
  The `frontier-sidecar-expected-hash` family covers sidecar expected-hash
  selection: tracked request precedence, deferred-hint and observed-head source
  ordering, exact height and authoritative-payload filtering, deterministic
  phase/view/hash tie-breaks, cached Prepare/Commit QC selection, and sidecar
  Commit-QC view rejection for absent, Prepare, wrong-height, or wrong-hash
  QCs. Its TLC cross-check independently exhausts the same twenty-five
  expected-failure configs as Apalache.
  The `contiguous-frontier-payload-hint` family covers contiguous-frontier
  payload-hint selection: Commit/Prepare/NewView phase ranking, deferred-QC
  priority over proposal markers, exact height and actionable filtering,
  deferred view/hash tie-breaks, marker fallback view/hash tie-breaks, and empty
  fallback behavior. Its TLC cross-check independently exhausts the same
  thirteen expected-failure configs as Apalache.
  The `frontier-parent-qc-hint-retarget` family covers contiguous-frontier
  missing-parent retargeting: exact-frontier stall bypass, canonical reanchor
  dependency-progress gating, previous-emission requirements, parent height
  matching, absent/same-hash hint rejection, and QC-hint target rewrite. Its TLC
  cross-check independently exhausts the same twelve expected-failure configs
  as Apalache.
  The `live-frontier-idle-missing-qc` family covers live-frontier idle
  missing-QC reacquire suppression: slot/pending-block liveness, observed head
  equality/lower acceptance, resilience/dependency/prior-attempt/height and
  future-head rejection, explicit commit or missing-QC dependency escape hatches,
  no-liveness rejection, attempt recording, broad highest-QC fetch and
  anchor-pull suppression, and sidecar hint preservation. Its TLC cross-check
  independently exhausts the same seventeen expected-failure configs as
  Apalache.
  The Sumeragi suite also includes the missing-payload-fetch-window helper
  slice for same-height missing-QC targeted-fetch pacing and lock-lag hash-miss
  cap widening.
  The `vrf-epoch-boundary` family covers no-op boundaries, penalty calculation,
  snapshot preservation, seed evolution, clear/advance/reset/take semantics,
  roster canonicalization, and entropy ordering. Its TLC cross-check
  independently exhausts the same twenty-three expected-failure configs as
  Apalache.
  The `vrf-epoch-restore` family covers unfinalized and finalized record
  hydration, parameter clamps, snapshot roster and input preservation, report
  clearing, merge conflict handling, late-reveal hydration, and identity
  preservation. Its TLC cross-check independently exhausts the same twenty-two
  expected-failure configs as Apalache.
  The `vrf-material-derivation` family covers required message inputs,
  big-endian epoch and signer encoding, field order, private-key signature
  binding, reveal/commitment hash chain, return ordering, and suppression of raw
  intermediate outputs. Its TLC cross-check independently exhausts the same
  seventeen expected-failure configs as Apalache.
  The `vrf-local-state` family covers supported-mode state creation,
  unsupported-mode preservation, epoch-switch material reset, same-epoch material
  preservation, commit/reveal note mutation, and actor reset. Its TLC cross-check
  independently exhausts the same twelve expected-failure configs as Apalache.
  The `vrf-penalties-report` family covers initial emptiness, update
  keying/latest-epoch tracking, exact report fields, same-epoch replacement,
  multi-epoch preservation, missing-get behavior, clear/reset semantics,
  post-clear updates, and read side-effect freedom. Its TLC cross-check
  independently exhausts the same seventeen expected-failure configs as
  Apalache.
  The `vote-admission` family covers early height/view, lock, roster, duplicate,
  chain-order, and signature gates; NEW_VIEW highest-QC validation;
  conflict/defer/evidence handling; QC attempts; roster caching; new-view
  tracking; pipeline requests; and progress touches. Its TLC cross-check
  independently exhausts the same thirty-one expected-failure configs as
  Apalache.
  The `vote-duplicate-key` family covers raw key fields, public-key exclusion
  from raw keys, identity-key public-key binding, block-hash comparison, NEW_VIEW
  highest-QC matching, and non-NEW_VIEW highest-QC ignoring. Its TLC cross-check
  independently exhausts the same fifteen expected-failure configs as Apalache.
  The `evidence-horizon` family covers zero-horizon disablement,
  missing-subject defaulting, saturating lower-bound arithmetic, inclusive
  boundary handling, stale rejection, and future evidence admission. Its TLC
  cross-check independently exhausts the same eleven expected-failure configs as
  Apalache.
  The Sumeragi suite also includes the p2p-topology-refresh helper slice for
  empty, unchanged, changed, and stray refresh decisions, the local-seen latch,
  local-removal queue clearing, empty gossip updates, and trusted-peer network
  topology updates.
  The Sumeragi suite also includes the live-frontier-idle-missing-QC helper
  slice for committed+1 idle reacquire suppression, explicit dependency
  escape hatches, and suppressed-branch side effects.
  The `missing-qc-reacquire-admission` family covers duplicate-attempt
  rejection, proposal-observed commit/missing-QC/frontier dependency admission,
  no-dependency proposal rejection, resilience-backed concrete dependency
  requirements, no-dependency height-window throttling and cleanup,
  dependency-signal and repeated-timeout admission, no-source rejection, and
  empty-frontier fallback gating. Its TLC cross-check independently exhausts the
  same twenty-one expected-failure configs as Apalache.
  The `missing-commit-qc-actionable` family covers exact pending/local payload
  matching, cached commit-QC and higher NEW_VIEW quorum rejection,
  non-actionable dependency filtering, NEW_VIEW and Prepare subject-height
  mapping, and stale-prune preservation for local payloads owned by the
  authoritative or frontier slot. Its TLC cross-check independently exhausts the
  same twenty-five expected-failure configs as Apalache.
  The `missing-qc-height-stall` family covers same-height stall lifecycle,
  three-window activation, active window advancement, dependency-progress and
  commit-progress reset, dependency continuity across reclassification, rotation
  reservation and availability, and range-pull/rotation marker height and mode
  gating. Its TLC cross-check independently exhausts the same twenty-five
  expected-failure configs as Apalache.
  The `missing-qc-stall-range-pull` family covers same-height stall reanchor
  reason admission, exact active/canonical height gating, already-emitted and
  recovery-FSM suppression, empty-target suppression, deterministic cohort
  fanout, sorted/deduplicated cooldown handling, stall-window cooldown
  application, and successful-send marking. Its TLC cross-check independently
  exhausts the same twenty-three expected-failure configs as Apalache.
  The `canonical-frontier-reanchor` family covers canonical reanchor reason
  admission, shared frontier-window key collapse, window snapshot and
  dependency-progress watermarks, stride-based suppression, deterministic
  range-pull fanout and cooldown handling, successful-send marking, and quorum
  view-change suppression while reanchor work remains unresolved. Its TLC
  cross-check independently exhausts the same thirty-five expected-failure
  configs as Apalache.
  The `frontier-repair-view-change` family covers quorum/stake-quorum cause
  admission, committed+1 height gating, committed-edge and passive catch-up
  precedence, direct-view and authoritative-payload exits,
  exact-repair/missing-payload/reassembly repair-source admission, recovery
  seeding, urgent body fetch emission, and precedence ordering. Its TLC
  cross-check independently exhausts the same twenty-six expected-failure
  configs as Apalache.
  The `frontier-recovery-advance` family covers reason-to-cause mapping,
  committed+1 gating, committed-edge and passive catch-up preemption,
  same-height evidence seeding, exact-frontier event routing, actionable
  dependency state updates, live-work/cooldown suppression, catch-up range-pull
  and cleanup transitions, and rotate-armed view-change behavior. Its TLC
  cross-check independently exhausts the same thirty-six expected-failure
  configs as Apalache.
  The `same-height-no-proposal-storm` family covers dependency-progress
  monotonicity, progress-triggered state resets, timeout record/count behavior,
  bounded force-break admission and cleanup, and active-pending idle timeout
  integration. Its TLC cross-check independently exhausts the same thirty-six
  expected-failure configs as Apalache.
  The `vrf-admission` family covers consensus-mode and epoch-manager gating,
  signer/signature checks, commit/reveal window and duplicate handling, external
  rebroadcast policy, local state updates, and late-reveal PRF refresh
  suppression. Its TLC cross-check independently exhausts the same twenty-one
  expected-failure configs as Apalache.
  The `vrf-epoch-window` family covers zero-length and offset clamping,
  zero-height/one-based position and epoch mapping, commit/reveal window
  boundaries, empty reveal windows, and outside-window rejection. Its TLC
  cross-check independently exhausts the same seventeen expected-failure configs
  as Apalache.
  The `missing-qc-reacquire-action` family covers prior-attempt classification,
  exact attempt recording, no-signal throttle marking, dependency-signal
  throttle bypass, suppression checks and side effects, sidecar request success,
  observed-head and far-ahead highest-QC fetch gating, lock-lag range-pull
  retargeting, broad-tier promotion, cooldown clearing, anchor-pull outcomes,
  success-counter accounting, and final return values. Its TLC cross-check
  independently exhausts the same thirty-one expected-failure configs as
  Apalache.
  The Sumeragi suite also includes the stale-view-commit-QC-fetch helper slice
  for exact pending identity, active/valid pending state, local commit-vote
  evidence, and current-tip extension gates.
  The Sumeragi suite also includes the idle-backlog-signals helper slice for
  residual backlog derivation and near-quorum fast-timeout gate parity.
  The Sumeragi suite also includes the pending-fast-path-timeout helper slice
  for quorum-timeout margin/floor derivation and DA inline validation floors.
  The Sumeragi suite also includes the missing-qc-timing helper slice for
  idle-round timeouts, idle-view derivation, streak advancement, forced
  proposal caps, rotation deferral, hard-cap selection, and saturating
  multiplication; the TLC helper independently cross-checks the same bounded
  mutation family.
  The Sumeragi suite also includes the stalled-pending-timeout helper slice for
  near-quorum fallback priority and active recovery backlog classification;
  the TLC helper independently cross-checks the same bounded mutation family.
  The Sumeragi suite also includes the stalled-pending-frontier-timeout helper
  slice for backlog timeout extension, deferred-QC multiplier selection, and
  active block-production gap caps; the TLC helper independently cross-checks
  the same bounded mutation family.
  The Sumeragi suite also includes the frontier-proposal-grace helper slice
  for exact-frontier proposal grace, ingress drain, and missing-QC reacquire
  window derivation; the TLC helper independently cross-checks the same
  bounded mutation family.
  The Sumeragi suite also includes the frontier-slot-helpers slice for
  lag-start fallback, body-state predicates, local-vote locking, timeout-view
  selection, progress/lag timer updates, catch-up markers, and compatibility
  mirror synchronization; the TLC helper independently cross-checks the same
  bounded mutation family.
  The Sumeragi suite also includes the frontier-slot-tracker helper slice for
  constructor mode/phase selection, block-created and body/vote/QC evidence
  steps, authoritative supersede, fetch retry, quorum timeout, lag-expiry,
  view-advance, finalization, and compatibility-sync behavior; the TLC helper
  independently cross-checks the same bounded mutation family.
  The Sumeragi suite also includes the slot-tracker-state helper slice for
  authoritative owner/frontier replacement, retained-branch refresh and seed
  priority, clear/remove-height behavior, and committed/above-height pruning;
  the TLC helper independently cross-checks the same bounded mutation family.
  The Sumeragi suite also includes the timeout-derivation helper slice for
  rebroadcast cooldown clamps, payload and targeted rescue cooldowns, quorum
  reschedule backoff, DA/non-DA commit and availability timeouts, pacemaker
  interval caps, and stale-gate timeout predicates; the TLC helper
  independently cross-checks the same bounded mutation family.
  The Sumeragi suite also includes the round-view-helpers slice for active
  round height saturation, new-view target selection, quorum-timeout view bump
  state, retained-window pruning, pacemaker reset, and round-phase priority;
  the TLC helper independently cross-checks the same bounded mutation family.
  The Sumeragi suite also includes the phase-tracker helper slice for
  construction, round start, view-change reset, record suppression, phase
  duration/marker updates, view-age lookup, and current-view lookup; the TLC
  helper independently cross-checks the same bounded mutation family.
  The Sumeragi suite also includes the failure-recovery-helpers slice for
  failed-commit QC realignment, pending-block drop after requeue, view-change
  cause priority, block-sync readiness, and post-block QC application; the TLC
  helper independently cross-checks the same bounded mutation family.
  The Sumeragi suite also includes the manifest-guard helper slice for DA
  manifest enforcement and bundle-cap admission.
  The Sumeragi suite also includes the DA gate status slice for
  missing-availability counter, latest reason, satisfaction, snapshot, and
  reset accounting.
  The Sumeragi suite also includes the consensus-handshake-caps helper slice
  for deterministic mode/domain, canonical-genesis-params, and fingerprint
  construction.
  It first runs
  `scripts/formal/check_sumeragi_formal_coverage.py` so runner modes, CI
  commands, README commands, and referenced TLA+/CFG files stay in sync before
  Apalache starts. For reproducible local setup without Docker, install the pinned
  toolchain with `bash scripts/formal/install_apalache.sh 0.52.2`.
  The `block-sync-roster-status` family covers block-sync roster source/drop
  counters, snapshot projection, and reset accounting.
  The `block-sync-qc-status` family covers block-sync QC/drop counters,
  final-drop reason projection, and reset accounting.
  The `rbc-store-status` family covers RBC store pressure, backpressure/drop
  counters, eviction totals/history, snapshot projection, and reset
  accounting.
  The `rbc-store-pressure-log` family covers RBC store pressure label mapping,
  transition logging, repeat-throttle suppression, log-state updates, and reset
  accounting.
  The `round-gap-status` family covers round-gap marker identity, incomplete
  snapshot suppression, duration/EMA projection, marker pruning, and reset
  accounting.
  The `deferred-recovery-status` family covers deferred-QC and
  empty-commit-topology counters, snapshot projection, and reset accounting.
  The `missing-qc-liveness-status` family covers missing-block hard-cap,
  missing-QC reacquire, forced-proposal, and stuck-round status projection
  plus reset accounting.
  The `sidecar-no-proposal-status` family covers sidecar mismatch quarantine,
  final-drop, recovery-trigger, no-proposal storm, and storm diagnostic
  snapshot projection plus reset accounting.
  The `deterministic-committee-status` family covers selected transport
  committee-size publication, snapshot projection, overwrite semantics, and
  reset accounting.
  The `timing-status-counters` family covers pacemaker backpressure, commit
  tick, prevote timeout, DA reschedule, and RBC DELIVER deferral counters,
  status/getter projection, and available reset accounting.
  The `round-trace-status` family covers round-trace transition results, gap
  snapshot projection, bounded trace retention, commit-pipeline wakeups, and
  event metadata copied into operator status snapshots.
  The `roster-recovery-status` family covers roster-unavailability and
  catch-up isolation counters, recovery state/dwell snapshot projection, and
  reset accounting.
  The `range-pull-status` family covers range-pull escalation/success/failure
  and candidate-exhausted counters, expiry-streak max/last accounting,
  snapshot projection, and reset accounting.
  The `round-recovery-bundle-window` family covers source/class labels,
  height-keyed commit/non-commit reservation partitions, explicit-window
  boundaries, and zero-window flooring for same-height recovery bundle pacing.
  The `signed-quorum-fetch-fallback` family covers committed signed-quorum
  fallback admission for fetch/body recovery.
  The `commit-qc-only-fetch-response` family covers direct commit-QC and
  signed-quorum fallback dispatch for commit-QC-only fetch responses.
  The `block-sync-vote-deferral` family covers embedded commit-vote filtering,
  vote-backed request refresh, known-block vote-only fast path, and the
  vote-stripped deferral handoff.
  The `block-sync-known-hintless` family covers the already-known,
  roster-hint-free BlockSyncUpdate fast path and its missing-request cleanup.
  The `block-sync-implicit-recovery` family covers the DA implicit
  missing-block recovery flag and verifies that it has no direct side effects.
  The `missing-block-ingress-fetch` family covers the exact-frontier
  authoritative body ingress grace gate before generic missing-block fetches.
  The `payload-progress-availability` family covers actor-local block payload
  material that can unblock consensus progress.
  The `highest-qc-fetch-body-known` family covers the body-known gate that
  suppresses highest-QC body fetches only for Kura and non-aborted local owners.
  The `local-payload-availability` family covers the broad actor-local payload
  predicate before stricter progress/fetch/lock filters are applied.
  The `block-known-locally` family covers local block-known routing before
  stricter lock/progress filters.
  The `block-known-for-lock` family covers lock-safety block-known routing,
  including pending-validity filtering and rejected-owner fallthrough.
  The `local-signed-block-lookup` family covers normal and body-repair
  signed-block materialization, source priority, and rejected-owner fallthrough.
  The `authoritative-payload-progress` family covers strict progress payload
  lookup, rejected-owner fail-closed behavior, deferred-payload exclusion, and
  Kura committed-hash filtering.
  The `authoritative-block-payload` family covers hash-level authoritative
  payload availability, local-source short-circuiting, rejected-local RBC
  fallback, and RBC hash/authority filtering.
  The `pending-block-active-for-tip` family covers active pending-block
  selection, consensus-inactive filtering, tip-extension checks, and the
  consensus-evidence disjunction.
  The `pending-fast-unblock` family covers zero-timeout disablement, evidence
  short-circuits, stored vote/cached QC gating, and inclusive fast-timeout age
  checks.
  The `blocking-pending-blocks` family covers classic and progress-aware
  blocking counters, zero-quorum fallback, vote/QC evidence precedence, quorum
  reschedule release, and stall-grace/quorum-timeout window boundaries.
  The `quorum-recovery-vote-drain` family covers vote-drain urgency from
  quorum timeout, live tip-extending pending ownership, vote/QC evidence,
  waiting vote backlog, evidence-specific age source, and existential pending
  scans.
  The `frontier-body-gap-payload-drain` family covers exact normal
  frontier-slot shape, body absence, accepted wait phases, vote-backed quorum
  evidence, and payload/block backlog routing for urgent payload drain.
  The `rbc-authoritative-payload-progress` family covers RBC session metadata
  filtering, complete-chunk root acceptance, chunk-failure fallback, and local
  authoritative payload slot/hash matching.
  The `slot-authoritative-payload` family covers slot-level authoritative
  payload lookup, local-owner status filtering, rejected-owner fallthrough,
  Kura committed-block filtering, and RBC retained-branch filtering.
  The `block-sync-vote-placeholder` family covers exact-frontier commit-vote
  placeholder recording and vote/sidecar filtering before embedded vote
  handling.
  The `block-sync-snapshot-hint` family covers known-block commit-roster
  snapshot hint filtering for incoming QC, checkpoint, and stake sidecars.
  The `block-sync-snapshot-roster` family covers commit-roster snapshot
  selection, snapshot cache insertion, and fallback roster-source ordering.
  The `block-sync-no-roster` family covers the terminal no-verifiable-roster
  BlockSyncUpdate branch, including known vote-only handling and unknown
  missing-roster defer/request/drop paths.
  The `block-sync-known-roster` family covers selected-roster known-block
  terminal replay, commit-roster persistence, and cleanup after vote/QC
  bookkeeping.
  The `block-sync-known-selected-roster` family covers selected-roster
  bookkeeping, known-block commit-roster persistence, known QC replay
  precedence/suppression, and known-block request cleanup.
  The `block-sync-selected-signatures` family covers selected-roster signer
  cache reuse, validated-signer insertion, signature-context deferral, roster
  evidence continuation, and invalid-signature drops.
  The `block-sync-selected-qc` family covers selected-roster QC source
  precedence, shape filtering, validation recovery, aggregate fallback,
  locked-conflict stripping, usable-QC caching, commit-cert gating, and
  invalid-payload drops.
  The `block-sync-selected-quorum` family covers selected-roster quorum
  admission, sparse exact-frontier recovery, missing-QC request transitions,
  NPoS vote-only deferral, exact body-repair deferral, quorum-missing drops,
  and invalid-QC short-circuiting.
  The `vote-backed-evidence` family covers Prepare/Commit vote/QC phase
  filtering, slot height/view/epoch matching, locally-known block requirements,
  and height-scoped view-independent evidence.
  The `vote-payload-actionable` family covers authoritative payload,
  validation inflight, pending-processing, exact deferred BlockSyncUpdate, and
  bad-deferred suppression rules for proposal evidence.
  The `actionable-vote-backed-proposal` family covers same-slot precommit
  proposal-blocking and slot-level Prepare/Commit vote/QC proposal evidence
  once payload material is actionable.
  Its TLC cross-check independently exhausts the same mutation family for
  proposal blocking and actionable vote/QC evidence admission.
  The `slot-proposal-evidence` family covers exact-slot authoritative payload,
  seen-proposal, cache, authoritative frontier metadata, active owner, and
  fall-through behavior for wrong-slot or incomplete earlier sources.
  Its TLC cross-check independently exhausts the same mutation family for
  evidence-source acceptance, wrong-slot rejection, and fall-through.
  The `round-liveness` family covers proposal evidence, live frontier owners,
  prior-view active pending owners, contiguous-frontier local same-height vote
  history, and fall-through behavior after earlier source misses.
  Its TLC cross-check independently exhausts the same mutation family for
  liveness-source acceptance, rejection, exact-view filtering, and fall-through.
  The `roster-recovery-fsm` family covers the roster-unavailability recovery
  state machine and transition bookkeeping for dwell time, entered-at reset,
  return value, and next-state counters.
  Its TLC cross-check independently exhausts the same mutation family for
  state transitions, changed/unchanged reporting, and transition bookkeeping.
  The `consensus-recovery-prune` family covers consensus-recovery entry
  cleanup for round clearing, status/dwell reset, committed-height floor,
  age-retention boundary, timeout max, and minimum retention floor.
  Its TLC cross-check independently exhausts the same mutation family for
  clear-by-height behavior, status reset, dwell clearing, retention floors, and
  age pruning.
  The `frontier-live-owner-work` family covers the underlying active-owner
  work predicate: terminal frontier-slot modes, exact pending/commit/validation
  work, slot commit-QC evidence, later-view competing quorum lockout, and local
  lock/history handling around terminal pending wrappers.
  Its TLC cross-check independently exhausts the same mutation family for
  terminal modes, live source admission, invalid local work rejection, competing
  quorum filters, and local vote-history guards.
  The `keep-frontier-pending-active` family covers view-change pending
  preservation for the exact live frontier owner, including pending commit-QC
  evidence, local-vote bridging, exact-hash validation/commit inflight, and
  fall-through after pending misses.
  Its TLC cross-check independently exhausts the same mutation family for live
  owner eligibility, pending commit-QC/local-vote evidence, exact-hash inflight
  fallbacks, invalid pending wrappers, and no-source rejection.
  The `stale-view-pending-prune` family covers pending-block cleanup after a
  view change, including stale selection, active frontier-owner preservation,
  retired DA retention, local execution cleanup, and RBC cleanup policy.
  Its TLC cross-check independently exhausts the same mutation family for
  stale/fresh boundaries, live-owner preservation, retained DA branches,
  execution cleanup, retained-branch notes, and RBC retention versus cleanup.
  The `superseded-frontier-payload-retention` family covers the exact
  same-height frontier payload retention predicate used when stronger
  same-height evidence supersedes an active owner.
  Its TLC cross-check independently exhausts the same mutation family for DA
  gating, materialized/invalid/committed exclusion, exact tip extension, and
  retained commit-evidence classes.
  The `stale-missing-block-request-prune` family covers missing-block request
  cleanup after a view change, including stale selection, DA gating, exact
  payload availability, and removal accounting.
  Its TLC cross-check independently exhausts the same mutation family for
  stale/fresh boundaries, DA/no-DA removal, exact authoritative/Kura payload
  availability, unresolved DA retention, and removal-count coupling.
  The `stale-missing-commit-qc-prune` family covers known-block commit-QC
  request cleanup after a view change, including exact active frontier repair,
  local-payload repair, stale selection, and removal accounting.
  Its TLC cross-check independently exhausts the same mutation family for
  stale/fresh boundaries, exact-frontier repair, local-payload repair,
  invalid preserve-source rejection, gate checks, and removal-count coupling.
  The `stale-rbc-session-prune` family covers RBC session cleanup after a view
  change, including stale selection, DA-disabled purge, invalid-session purge,
  delivered exact-payload purge, retained undelivered/missing-payload sessions,
  purge-state cleanup, and removal accounting.
  Its TLC cross-check independently exhausts the same mutation family for
  stale/fresh boundaries, DA-disabled purge, invalid-session purge, exact
  delivered-payload purge, retained DA convergence sessions, and purge/count
  side effects.
  The `highest-qc-defer-marker-prune` family covers highest-QC missing defer
  marker cleanup across view-change pruning, consensus-recovery clearing, and
  committed/local/non-actionable dependency pruning.
  Its TLC cross-check independently exhausts the same mutation family for
  view-change stale marker pruning, consensus-recovery clear-through-view
  boundaries, committed-height dependency pruning, local-known dependency
  pruning, non-actionable dependency pruning, and unresolved dependency
  retention.
  The `fast-finality-inline-validation` family covers inline validation
  admission for DA-enabled next-height proposal evidence, disabled-priority and
  inflight rejection, local-payload byte matching, cap boundaries, and returned
  transaction counts.
  Its TLC cross-check independently exhausts the same mutation family for DA
  gating, priority-reason rejection, height gating, proposal-evidence gating,
  inflight rejection, payload mismatch rejection, zero/over/exact-cap
  boundaries, and returned-count accounting.
  The `observer-signature-recovery` family covers observer-only
  signature-mismatch recovery from observed or cached commit-QC evidence while
  rejecting local validators, unsupported validation errors, missing commit-QC,
  and cached-context mismatches.
  Its TLC cross-check independently exhausts the same mutation family for
  recoverable signature errors, observed/cached/single-source commit-QC
  evidence, local-validator rejection, unsupported-error rejection, missing-QC
  rejection, cached-context mismatch rejection, and dual-source independence.
  The `validation-failure-finalize` family covers validation-failure deferral
  versus invalid finalization, pending-state cleanup, abort/requeue/proposal/
  RBC/QC-cache cleanup, reason-label classification, invalid-proposal evidence
  attachment, and previous-roster recovery triggering.
  Its TLC cross-check independently exhausts the same mutation family for
  previous-height deferral boundaries, invalid pending-state cleanup,
  cleanup side effects, reason labels, deferred evidence suppression, matching
  QC evidence attachment, and previous-roster recovery isolation.
  The `validation-reject-reason-label` family covers the full validation and
  vNext rejection classifier boundary across prev-hash, prev-height, topology,
  execution, and stateless buckets.
  Its TLC cross-check independently exhausts the same mutation family for
  direct structural labels, execution-bucket labels, stateless policy/time/
  roster labels, vNext pass-through labels, and vNext normalization.
  The `validation-reject-status` family covers validation-reject total and
  per-reason counters, unknown-label behavior, last reason/slot/block/timestamp
  fields, status snapshot projection, and reset semantics.
  Its TLC cross-check independently exhausts the same mutation family for
  empty and post-record reset, known bucket routing, unknown-label handling,
  same/different reason accumulation, last-field updates, timestamp
  positivity, and status projection.
  The `peer-key-policy-status` family covers peer-key policy rejection total
  and per-reason counters, stable reason labels, last reason/timestamp fields,
  snapshot projection, top-level status projection, and reset semantics.
  Its TLC cross-check independently exhausts the same mutation family for
  total increments, bucket routing, stable labels, same/different reason
  accumulation, timestamps, snapshot projection, top-level projection, and
  reset behavior.
  The `view-change-cause-status` family covers view-change cause counters,
  unknown-cause behavior, last cause/timestamp fields, per-cause timestamps,
  snapshot projection, top-level status projection, and reset semantics.
  Its TLC cross-check independently exhausts the same mutation family for
  known bucket routing, unknown-cause rejection from known buckets,
  same/different cause accumulation, last/timestamp updates, per-cause
  timestamps, snapshots, top-level projection, and reset behavior.
  The `view-change-proof-status` family covers view-change index storage,
  proof accepted/stale/rejected counters, local suggest/install counters,
  reset semantics, and top-level status projection.
  Its TLC cross-check independently exhausts the same mutation family for
  initial state, index set/overwrite, proof-counter buckets, suggest/install
  buckets, accumulation, snapshot projection, reset behavior, and index
  preservation across resets.
  The `block-sync-selected-apply` family covers selected-roster apply-path
  non-extending QC admission, frontier-owner preservation/supersede decisions,
  recovery-mode selection, signed-quorum commit-QC repair, sparse next-height
  commit-QC recovery, and QC application readiness.
  The `block-sync-selected-qc-prefilter` family covers post-apply QC topology
  recovery, shape ignores, same-height locked-QC drops, stale locked-QC drops,
  missing locked-payload quarantine, non-extending drops, and tally admission.
  The `block-sync-selected-qc-process` family covers post-prefilter QC tally
  reuse/validation, precommit processing arguments, commit-QC cache/record side
  effects, known-block commit application, unknown-pending epoch observation,
  runtime DA cleanup, and unknown-block QC cache handoff.
  The `block-sync-selected-qc-cache` family covers unknown-block QC cache
  prefiltering, fresh signer tally validation, false `block_known` precommit
  processing, non-extending lock realignment, quarantine removal, commit-QC
  cache insertion, and transient/final validation-error handling.
  The `block-sync-stale-view` family covers stale BlockSyncUpdate drops,
  requested/known/evidence-bearing stale admission, and stale-drop status
  recording.
  The `block-sync-commit-conflict` family covers committed-height
  BlockSyncUpdate conflict drops, QC validation inputs, and invalid-QC
  evidence emission on finality conflict.
  The `block-sync-warning-throttle` family covers per-kind/hash/height/view
  warning cooldowns, suppressed-count replay, burst caps, zero-cap and
  zero-cooldown behavior, GC boundaries, and clear/reset semantics.
  The `fetch-block-body-handle` family covers exact body request handling,
  canonical committed deferral, requester stashing, and dedup release.
  The `background-frame-cap` family covers background consensus frame-cap
  trimming, downgrade, direct fallback, and drop decisions.
  The `background-dispatch` family covers background request blocking
  eligibility, full-queue fallback, unavailable-worker drop status, kind
  labels, and request reconstruction.
  The `background-bypass` family covers scheduler bypass decisions for
  prepared post/broadcast payloads, forced-queue scheduling, disabled-worker
  inline dispatch, and non-payload control/native requests.
  The `background-fallback` family covers fallback request-to-network mapping,
  peer preservation, payload class preservation, and block/control/native
  priority assignment.
  The `fetch-pending-response-send` family covers single fetch response send
  policy, bypass selection, fallback payloads, and direct-QC companion ordering.
  The `fetch-pending-responses-batch` family covers batch requester splitting,
  per-peer hintless downgrade decisions, exact body companions, and rostered
  update fanout ordering.
  The `pending-response-flush` family covers pending fetch/body readiness
  wrappers, canonical deferral, queue removal, and exact body response fanout.
  The `deferred-block-sync-helper` family covers deferred BlockSyncUpdate
  reason priority, sidecar merge, evidence detection, and cap eviction order.
  The `deferred-block-sync-cache` family covers deferred BlockSyncUpdate
  cache/defer integration, commit-vote stripping, full-key matching,
  post-cache cap enforcement, and deferred outcome recording.
  The `deferred-block-sync-replay` family covers replay idle gating, ordered
  key selection, remove-before-handle, forwarding, and handler-error behavior.
  The `block-body-repair` family covers the RBC exact body repair admission
  helper.
  The `block-body-request-stash` family covers the exact body requester
  stash-window helper.
  The `same-height-block-body-repair` family covers same-height exact body
  repair admission.
  The `block-body-repair-epoch` family covers observed commit-QC epoch source
  selection for body repair.
  The `direct-commit-qc-for-block` family covers direct commit-QC source
  selection and local vote-formation quorum gating.
  The `materialize-qc` family covers QC materialization, Kura recovery,
  fail-closed quorum/signature gates, and cache insertion.
  The `block-body-direct-commit-qc` family covers direct commit-QC extraction
  from BlockBodyResponse payloads.
  The `block-body-detached-commit-qc` family covers detached commit-QC
  handling and obsolete repair clearing.
  The `block-body-response-dispatch` family covers exact BlockBodyResponse
  fallback and companion dispatch ordering.
  The `stale-proposal-hint-repair` family covers the DA stale-view proposal
  hint exception for exact-frontier repair.
  The `stale-rbc-hint-repair` family covers the stale RBC proposal-hint bridge
  into exact-frontier body repair.
  The `highest-qc-dependency-deferral` family covers force/exact highest-QC
  repair selection, lock-lag range-pull reanchor, marker pruning, and deferred
  slot non-admission.
  The `committed-edge-conflict` family covers committed-edge conflicting
  highest-QC suppression, canonical state preservation, stale-frontier cleanup,
  owner gating, and bounded recovery reanchors.
  The `lock-rejected-sink` family covers deterministic lock-rejected branch
  sink note/update, activity, fetch/parent suppression, replay drop, and purge
  cleanup behavior.
  The `active-lock-reject-recovery` family covers active-height lock-rejected
  branch recovery routing through missing-QC frontier recovery and view-change
  escalation.
- `check_swift_spm_validation.sh` – exercises `IrohaSwift/Package.swift` with the bridge present and with the bridge intentionally missing (expects Swift-only fallback plus warning). Writes a summary + logs under `artifacts/swift_spm_validation`.
- `check_swift_pod_bridge.sh` – runs `pod lib lint` against `IrohaSwift/IrohaSwift.podspec` with the bundled `NoritoBridge.xcframework` to make sure pod consumers get the signed bridge and minimum platform/toolchain settings stay in sync with SPM.
- `check_sorafs_gateway_denylist.sh` – generates two sample denylist bundles from the canonical fixtures, runs `cargo xtask sorafs-gateway denylist diff`, and fails the build if the report is missing or lacks additions/removals. This guards the MINFO-6 workflow so releases always have working bundle-evidence tooling.
- `check_walletless_follow_bundle.sh` – repackages the walletless follow-game static bundle and asserts the tarball + `.sha256` sidecar exist. Use this in CI before publishing via the content lane workflow.

## Cargo `build-dir` decision

Rust 1.91 stabilised the `[build] build-dir` option, which allows relocating
`target/`. The workspace baseline is now Rust 1.92, but we audited the CI
wrappers and decided **not** to override this
setting:

- `ci/check_sorafs_fixtures.sh` exports `target/go-cache`, `target/go-mod-cache`,
  and other Go workdirs when it runs the cross-language chunker suite
  (`ci/check_sorafs_fixtures.sh:72-85`). Moving the build directory would break
  those cache paths as well as the `TMPDIR` wiring that assumes they live inside
  the repository.
- `ci/check_norito_enum_bench.sh` writes Criterion artefacts to
  `${ROOT_DIR}/target/criterion` so downstream tooling can scrape the JSON/HTML
  reports without extra configuration (`ci/check_norito_enum_bench.sh:6-27`).

Other scripts (Swift dashboards, Android docs, etc.) stream intermediate files
into `target/` for the same reason: shared caches and human-readable locations.
To keep CI deterministic, **do not** set `[build] build-dir` in
`.cargo/config.toml` and avoid committing `CARGO_TARGET_DIR` overrides. If you
need a custom build directory for local experimentation, export
`CARGO_TARGET_DIR` in your shell session but reset it before running any
`ci/check_*` script.
