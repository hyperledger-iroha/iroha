# CI Helpers

This directory hosts the developer-facing shell helpers that gate CI jobs
(`ci/check_*.sh`). Most scripts assume the default Cargo artifact layout under
`target/`, so keep that layout unchanged unless the entire toolchain is updated
as part of a coordinated migration.

### Featured checks
- `check_rust_1_92_lints.sh` – runs `cargo check` with the Rust 1.92 lint set (including the new never-type fallback and macro-export checks) so stricter diagnostics surface before CI.
- `check_nexus_cross_dataspace_localnet.sh` – runs the deterministic Nexus cross-dataspace all-or-nothing localnet proof (`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`) through `scripts/run_nexus_cross_dataspace_atomic_swap.sh`.
- `check_sumeragi_formal.sh` – runs bounded Apalache checks for the Sumeragi
  commit-path, TLC-cross-checked fork-safety, TLC-cross-checked quorum-policy,
  TLC-cross-checked RBC deliver-quorum, TLC-cross-checked RBC causality gate,
  TLC-cross-checked RBC local READY emission gate, TLC-cross-checked RBC local
  DELIVER emission gate, TLC-cross-checked RBC delivered-session rebroadcast
  gate, TLC-cross-checked RBC stalled-rebroadcast cursor/action gates,
  TLC-cross-checked RBC next-due scheduler gate,
  TLC-cross-checked RBC DELIVER acceptance gate,
  TLC-cross-checked RBC commit-processing gate, TLC-cross-checked RBC chunk
  target helper, TLC-cross-checked RBC chunk
  payload-cap helper, TLC-cross-checked RBC rebroadcaster selection helper,
  TLC-cross-checked RBC weighted chunk allocation helper, TLC-cross-checked
  RBC payload chunking helper, TLC-cross-checked RBC RS16 initial fanout helper,
  TLC-cross-checked RBC chunk broadcast order helper, TLC-cross-checked RBC
  payload layout helper, TLC-cross-checked RBC session chunk-ingest helper,
  TLC-cross-checked RBC READY/DELIVER session recording helper,
  TLC-cross-checked RBC delivered-payload byte telemetry helper,
  TLC-cross-checked pending-RBC stash gate, RBC
  status lookup helper, RBC status retention/update-pruning helper, RBC
  status persistence/fallback helper, TLC-cross-checked RBC status handle
  lifecycle helper, TLC-cross-checked RBC backlog/status snapshot helper, RBC
  abort status counter helper, RBC
  mismatch status counter helper, RBC persisted chunk sampling/proof helper,
  RBC persisted session-store guard
  helper, RBC store status accounting helper, RBC store pressure log helper,
  RBC stale-message/payload-refetch helper, RBC signing-preimage gate,
  classic
  Vote/VRF signing-preimage gate, classic
  Vote/QC signature-verification gate, invalid-signature throttle/penalty
  helper, invalid-signature label helper, penalty offender-selection helper,
  consensus penalty-action helper,
  penalty status projection helper, execution-witness recorder helper,
  execution-witness access-key parser helper, execution-witness root projection
  helper, sparse-Merkle path/hash helper, RBC compact block-message helper,
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
  committed-height QC admission helper, TLC-cross-checked pending-progress
  accounting helper,
  TLC-cross-checked pending-block lifecycle helper,
  TLC-cross-checked pending-block marker/cooldown helper,
  TLC-cross-checked pending-block Kura retry helper, TLC-cross-checked
  commit-pipeline scheduling gate, TLC-cross-checked precommit vote-count
  helper, TLC-cross-checked block-known local/lock helpers,
  TLC-cross-checked active lock-reject recovery helper, TLC-cross-checked
  locked-QC helper, TLC-cross-checked precommit-QC locked-chain wrapper,
  TLC-cross-checked precommit-vote lock filter, TLC-cross-checked precommit
  vote-emission gate, TLC-cross-checked cached vote-log epoch replay helper,
  TLC-cross-checked NEW_VIEW highest-QC vote-selection helper,
  TLC-cross-checked active-frontier NEW_VIEW catch-up helper,
  TLC-cross-checked late NEW_VIEW near-quorum emission helper,
  TLC-cross-checked near-quorum NEW_VIEW rebroadcast helper,
  TLC-cross-checked requester roster-proof detection helper,
  TLC-cross-checked online-validator and relay counter helper,
  TLC-cross-checked commit-result drain gate,
  TLC-cross-checked commit-drain summary aggregation helper,
  TLC-cross-checked commit-pipeline timing sample helper,
  TLC-cross-checked commit-pipeline status recorder helper,
  TLC-cross-checked autoscale transition commit gate,
  TLC-cross-checked commit-QC signer quorum helper,
  TLC-cross-checked signature-index recovery helper,
  TLC-cross-checked commit-QC cache/history lookup helper,
  TLC-cross-checked embedded-QC roster bootstrap helper,
  TLC-cross-checked cached-QC precommit signer record helper,
  TLC-cross-checked roster-validation memo cache helper,
  TLC-cross-checked roster-validation cached wrapper helper,
  TLC-cross-checked roster-validation core helper,
  TLC-cross-checked roster artifact selection helper,
  TLC-cross-checked block roster cache helper,
  TLC-cross-checked block-sync roster evidence helper,
  TLC-cross-checked block-sync history roster helper,
  TLC-cross-checked persisted block-sync roster selection helper,
  TLC-cross-checked BlockSyncUpdate roster hydration helper,
  TLC-cross-checked roster index projection helper,
  TLC-cross-checked membership-view hash helper,
  TLC-cross-checked membership mismatch status helper,
  TLC-cross-checked membership advert publication helper,
  TLC-cross-checked membership mismatch ingress/fail-closed helper,
  TLC-cross-checked consensus params ingress helper,
  TLC-cross-checked prevalidated commit artifact trust helper,
  TLC-cross-checked commit-job dispatch gate,
  commit-worker channel capacity helper,
  slow commit-stage timing threshold helper,
  TLC-cross-checked commit-evidence replay gate,
  TLC-cross-checked Pacemaker core helper, TLC-cross-checked pacemaker
  evaluation gate, TLC-cross-checked pacing-governor helper,
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
  TLC-cross-checked execution-witness recorder helper,
  TLC-cross-checked execution-witness access-key parser helper,
  TLC-cross-checked execution-witness root projection helper,
  TLC-cross-checked sparse-Merkle path/hash helper,
  TLC-cross-checked RBC compact block-message helper,
  TLC-cross-checked block-message priority helper,
  TLC-cross-checked block-message height/view helper,
  TLC-cross-checked block-message kind helper,
  TLC-cross-checked Kura replica advert helper,
  TLC-cross-checked message projection helper,
  TLC-cross-checked pipeline event emission helper,
  TLC-cross-checked block-message wire helper,
  TLC-cross-checked BlockCreated frontier wire helper,
  TLC-cross-checked cached proposal rebroadcast helper,
  TLC-cross-checked frontier block-sync hint/direct-response helper,
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
  TLC-cross-checked round-trace status recorder helper,
  TLC-cross-checked failed-commit/block-sync helper,
  TLC-cross-checked transaction requeue branch helper,
  TLC-cross-checked tick/deadline scheduling helper,
  TLC-cross-checked proposal parent resolution gate, TLC-cross-checked
  highest-QC dependency deferral gate, TLC-cross-checked precommit-QC
  view-change selector gate,
  TLC-cross-checked block-sync recovery gate,
  direct certified-block fetch gate,
  TLC-cross-checked missing-block ingress fetch gate,
  TLC-cross-checked payload progress availability gate,
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
  TLC-cross-checked committed-edge conflict suppression gate,
  TLC-cross-checked lock-rejected branch sink gate,
  missing-block hard-cap recovery gate,
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
  TLC-cross-checked worker-loop stage helper gate, TLC-cross-checked
  worker-queue status accounting gate,
  NPoS VRF epoch-seal staging gate,
  commit-anchor QC promotion helper, committed-height QC admission helper,
  Kura durability commit retry gate, Kura persistence status helper,
  restarted-peer
  replay gate, TLC-cross-checked precommit vote-emission gate, proposal
  assembly gate, pure
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
  TLC-cross-checked same-height vote conflict helper, aggregate same-height
  vote-lock helper, TLC-cross-checked proposal stale same-height vote helper,
  same-height vote recovery view-gap helper, tip-extension helper,
  TLC-cross-checked DA gate helper, TLC-cross-checked DA gate status helper,
  TLC-cross-checked local payload availability helper,
  TLC-cross-checked highest-QC body-known fetch helper,
  TLC-cross-checked authoritative payload progress helper,
  TLC-cross-checked authoritative block payload helper,
  TLC-cross-checked block-payload canonicalization helper,
  TLC-cross-checked pending-block active-for-tip helper,
  TLC-cross-checked pending fast-unblock helper,
  TLC-cross-checked blocking pending-block counter helper,
  TLC-cross-checked quorum recovery vote-drain helper,
  TLC-cross-checked frontier body-gap payload-drain helper,
  TLC-cross-checked RBC authoritative payload progress helper,
  TLC-cross-checked slot authoritative payload helper,
  TLC-cross-checked consensus handshake capability construction helper,
  consensus handshake helper, runtime mode flip helper, TLC-cross-checked
  effective mode selection helper, TLC-cross-checked effective timing
  aggregation helper, NEW_VIEW stats helper,
  NEW_VIEW tracker helper,
  timing monitor helper, TLC-cross-checked hotspot summary accumulator helper,
  TLC-cross-checked adaptive observability timing/fanout helper, pacing
  backpressure helper, TLC-cross-checked counter-driven backpressure cooldown
  helper, TLC-cross-checked locked-QC helper, stake snapshot quorum helper,
  TLC-cross-checked live local-vote roster helper,
  TLC-cross-checked canonical round-roster helper,
  vote-roster selection helper, vote-roster cache/support helper,
  TLC-cross-checked commit-topology state/reset helper, TLC-cross-checked roster index projection
  helper, TLC-cross-checked membership-view hash helper, TLC-cross-checked
  membership mismatch status helper, TLC-cross-checked membership advert
  helper, TLC-cross-checked membership mismatch ingress helper,
  TLC-cross-checked consensus params ingress helper,
  TLC-cross-checked prevalidated commit artifact trust helper,
  TLC-cross-checked commit-job dispatch gate,
  TLC-cross-checked precommit signer-history fallback helper, missing-block request clear helper,
  missing-block clear reason helper, TLC-cross-checked recovery status counter helper,
  TLC-cross-checked recovery-FSM reason classifier helper,
  TLC-cross-checked QC rebuild status counter helper,
  TLC-cross-checked QC rebuild quorum helper,
  TLC-cross-checked collector-targeting status helper,
  TLC-cross-checked deferred recovery status helper,
  missing-QC liveness status helper,
  sidecar/no-proposal status helper,
  TLC-cross-checked deterministic committee status helper,
  timing/liveness status counter helper, roster-recovery status helper,
  peer-key policy status helper,
  view-change cause status helper,
  highest-QC selection, optional
  highest-QC selection filter, RBC block-body
  repair admission helper, and
  frontier-recovery TLA+ models, the small TLC
  frontier/QC signer-count/signer-index normalization/precommit vote-count/
  commit-quorum signers/commit-QC lookup/precommit signer record/
  TLC-cross-checked voting signer-count/collector-plan/validation ownership
  cleanup/worker-loop stage/
  worker tick-gap/vNext performance config/validation worker config/
  commit-worker config/commit-stage timing threshold/commit-inflight timeout/
  post-commit pacemaker kick/idle-view proposal budget/cached-slot timeout/
  pending fast-path timeout/live-frontier idle missing-QC/missing-QC reacquire
  admission/missing-QC reacquire action/missing commit-QC actionable/idle backlog
  signals/missing-QC height stall/missing-QC stall range-pull cross-checks, and
  expected-failure
frontier/fork/quorum/RBC/rbc-causality/rbc-deliver-acceptance/rbc-commit-processing/rbc-chunk-target/rbc-chunk-payload-cap/rbc-rebroadcast-selection/rbc-chunk-allocation/rbc-payload-chunking/rbc-rs16-initial-fanout/rbc-chunk-broadcast-order/pending-rbc-stash/pending-rbc-status/ingress-dedup-cache/ingress-status-counters/consensus-message-labels/phase-latency-status/telemetry-status/lane-detail-status/settlement-status/history-status/commit-quorum-status/commit-inflight-status/rbc-status-lookup/rbc-status-retention/rbc-status-persistence/rbc-status-handle/rbc-backlog-status/rbc-abort-status/rbc-mismatch-status/rbc-progress-stage/rbc-hot-repair/rbc-sampling/rbc-store/rbc-store-status/rbc-store-pressure-log/round-gap-status/rbc-recovery-helper/rbc-missing-block-recovery/rbc-unverified-roster/rbc-preimage/classic-preimage/classic-signature/invalid-signature-labels/invalid-signature-throttle/penalty-offender-selection/consensus-penalty-action/exec-witness-recorder/exec-witness-access-key/exec-witness-roots/smt-path-hash/block-message-rbc-compact/block-message-priority/block-message-height-view/block-message-kind/kura-replica-advert/message-projection/pipeline-event-emission/block-message-wire/block-created-frontier-wire/cached-proposal-rebroadcast/frontier-block-sync-hint/frontier-same-slot-activity/frontier-reassembly-activity/frontier-quorum-owner-actionable/frontier-sidecar-retarget/frontier-sidecar-expected-hash/contiguous-frontier-payload-hint/frontier-parent-qc-hint-retarget/live-frontier-idle-missing-qc/missing-qc-reacquire-admission/missing-qc-reacquire-action/missing-commit-qc-actionable/missing-qc-height-stall/missing-qc-stall-range-pull/missing-payload-fetch-window/canonical-frontier-reanchor/frontier-repair-view-change/frontier-recovery-advance/same-height-no-proposal-storm/vrf-admission/vote-admission/vote-duplicate-key/evidence-horizon/evidence-canonicalization/evidence-validation/double-vote-recording/invalid-qc-shape/qc-validation-evidence/qc-validation-reason/block-sync-qc-fallback/signed-quorum-fetch-fallback/commit-qc-only-fetch-response/block-sync-update-targets/apply-cached-qcs/block-sync-roster/block-sync-vote-deferral/block-sync-known-hintless/block-sync-implicit-recovery/block-sync-vote-placeholder/block-sync-snapshot-hint/block-sync-snapshot-roster/block-sync-no-roster/block-sync-known-selected-roster/block-sync-selected-signatures/block-sync-selected-qc/block-sync-selected-quorum/block-sync-recovery-mode/block-sync-selected-apply/block-sync-selected-qc-prefilter/block-sync-selected-qc-process/block-sync-selected-qc-cache/block-sync-stale-view/block-sync-commit-conflict/block-sync-warning-throttle/fetch-response-deferral/fetch-block-body-handle/background-frame-cap/fetch-pending-response-send/fetch-pending-responses-batch/pending-response-flush/deferred-block-sync-helper/deferred-block-sync-cache/deferred-block-sync-replay/block-sync-future-window/invalid-proposal-evidence/proposal-mismatch/proposal-cache/proposal-hint/stale-proposal-hint-repair/stale-rbc-hint-repair/proposal-admission/block-created-admission/missing-request-clear/missing-block-clear/proposal-budget/proposal-backpressure/proposal-defer-warning/non-rbc-payload-budget/proposal-batch/lane-interleave/commitment-snapshot-builder/collector-plan/collector-selection/topology-mutation/prf-leader-shuffle/topology-fanout/p2p-topology-trusted/p2p-topology-refresh/quorum-retransmit/retransmit-backpressure/quorum-reschedule-backoff/rbc-availability-reschedule/vote-backed-reassembly-stall/completed-quorum-view-advance/QC-signer/qc-signer-count/signer-index-normalization/commit-root/commit-pipeline-recovery/known-block-commit-qc-recovery/stale-view-commit-qc-fetch/commit-anchor-qc/pending-progress/commit-pipeline-scheduling/precommit-vote-count/voting-signer-count/distinct-vote-epochs/new-view-highest-qc-votes/online-validator-relay-counters/commit-result-drain/commit-drain-summary/commit-pipeline-sample/commit-pipeline-status/autoscale-transition/commit-quorum-signers/signature-index-recovery/commit-qc-lookup/precommit-signer-record/roster-validation-memo/roster-validation-cached/roster-validation-core/roster-artifact-selection/block-roster-caches/block-sync-roster-evidence/block-sync-history-roster/persisted-roster-selection/block-sync-update-roster/roster-index-projection/membership-view-hash/membership-mismatch-status/membership-advert/membership-mismatch-ingress/consensus-params-ingress/prevalidated-commit-artifact/commit-job-dispatch/commit-worker-config/commit-stage-timing-threshold/commit-inflight-timeout/post-commit-pacemaker-kick/idle-view-proposal-budget/pacemaker-core/pacemaker-evaluation/pacing-governor/cached-slot-timeout/pending-fast-path-timeout/stalled-pending-timeout/stalled-pending-frontier-timeout/missing-qc-timing/idle-backlog-signals/proposal-liveness/frontier-slot-tracker/frontier-slot-helpers/frontier-proposal-grace/slot-tracker-state/timeout-derivation/round-view-helpers/phase-tracker/round-trace-status/failure-recovery-helpers/requeue-transactions/tick-deadline-helpers/proposal-parent-resolution/highest-qc-dependency-deferral/precommit-QC-view-change/commit-evidence-replay/block-sync-recovery/certified-fetch/missing-block-ingress-fetch/payload-progress-availability/highest-qc-fetch-body-known/local-payload-availability/block-known-locally/block-known-for-lock/missing-locked-qc-recovery/local-signed-block-lookup/authoritative-payload-progress/authoritative-block-payload/pending-block-active-for-tip/pending-fast-unblock/blocking-pending-blocks/quorum-recovery-vote-drain/frontier-body-gap-payload-drain/rbc-authoritative-payload-progress/slot-authoritative-payload/missing-block-fetch/recovery-status-counters/deferred-recovery-status/range-pull-recovery/range-pull-status/recovery-fsm-reason/round-recovery-bundle-window/committed-edge-conflict/lock-rejected-sink/active-lock-reject-recovery/missing-block-hard-cap/missing-block-hard-cap-cleanup/missing-block-view-change/native-AMX-attestation/native-AMX-journal/native-AMX-receipt/native-AMX-ingress/vnext-chain-order/vnext-rechain/vnext-rechain-error-label/vnext-signature/vnext-signing-preimage/vnext-control-ingress/vnext-slot-lifecycle/vnext-validation/validation-worker-config/verify-cache-key/vote-verify-async/vote-verify-worker-config/qc-verify-async/qc-verify-worker-config/worker-drain/actor-gate/worker-budget/worker-ingress/worker-loop-stage/worker-queue-status/npos-vrf/kura-commit/kura-store-status/restart-replay/post-commit-cleanup/frontier-gap-realign/same-height-vote-conflict/proposal-stale-vote/same-height-vote-recovery-gap/tip-extension-helpers/da-gate/consensus-handshake-caps/handshake/mode-flip/effective-mode/effective-timing/new-view-stats/new-view-tracker/timing-monitor/hotspot-log-summary/adaptive-observability/pacing-backpressure/counter-backpressure-cooldown/locked-qc-helper/stake-snapshot/live-vote-roster/canonical-round-roster/vote-roster-selection/vote-roster-cache/commit-topology-state/precommit-signer-history/precommit/proposal/engine-initial-state/engine-read-accessors/engine-tick/engine-tick-state-preservation/engine-new-view-subject/engine-handle-dispatch/engine-handle-forwarding/engine-handle-output-relay/engine-certificate-dispatch/engine-certificate-prefilter-state/engine-certificate-prefilter-state-preservation/engine-view-advance-saturation/engine-new-view/engine-new-view-highest-qc/engine-new-view-state-preservation/engine-new-view-advance/engine-proposal/engine-proposal-output/engine-proposal-state/engine-proposal-state-preservation/engine-proposal-validation-owner/engine-proposal-lock/qc-round-compatibility/engine-QC-ref-projection/engine-QC-ref-comparator/engine-highest-QC-record/engine-commit-subject/engine-payload-lookup/engine-prepare/engine-prepare-lock-highest/engine-prepare-phase/engine-prepare-vote-cache/engine-commit/engine-commit-highest-qc/engine-commit-phase/engine-commit-state-preservation/engine-commit-available-commit/engine-commit-pending-fetch/engine-commit-validation-cleanup/engine-committed-block/engine-committed-block-record/engine-reconfiguration-staging/engine-reconfiguration-dedup/engine-committed-block-cleanup/engine-committed-block-state-preservation/engine-payload-record/engine-payload/engine-payload-state-preservation/engine-validation-result/engine-validation-state-preservation/engine-validation-ownership/engine-validation-invalid-advance/reconfiguration/recovery/view-change/validation/validation-priority/vote-backed-evidence/vote-payload-actionable/actionable-vote-backed-proposal/slot-proposal-evidence/round-liveness/frontier-live-owner-work/keep-frontier-pending-active/stale-view-pending-prune/superseded-frontier-payload-retention/stale-missing-block-request-prune/fast-finality-inline-validation/observer-signature-recovery/validation-failure-finalize/validation-reject-reason-label/validation-reject-status/peer-key-policy-status/view-change-cause-status/validation-evidence-qc/admission/highest-QC/highest-optional
  mutations.
  The fork-safety, quorum-policy, and RBC deliver-quorum families now have TLC
  cross-checks that independently exhaust the same one, six, and four
  expected-failure configs as Apalache, respectively.
  The RBC causality, READY emission, DELIVER emission, delivered-session
  rebroadcast, rebroadcast cursor, rebroadcast action, and next-due scheduler
  families now have TLC cross-checks that independently exhaust the same 25,
  24, 31, 22, 13, 24, and 26 expected-failure configs as Apalache.
  The commit-evidence replay and block-sync recovery families now have TLC
  cross-checks that independently exhaust the same twelve and fifteen
  expected-failure configs as Apalache.
  The missing locked-QC payload recovery, range-pull recovery, native AMX
  journal replay, and native AMX ingress families now have TLC cross-checks
  that independently exhaust the same 31, 47, 17, and 19 expected-failure
  configs as Apalache.
  The vNext chain-order, re-chain, re-chain error-label, aggregate signature,
  signing-preimage, control-ingress, slot-lifecycle, validation,
  deadline-protection, validation stall/redrive, verify-cache-key,
  vote-verify async, and QC-verify async families now have TLC cross-checks
  that independently exhaust the same 19, 17, 13, 16, 27, 28, 32, 15, 24,
  24, 27, 30, and 39 expected-failure configs as Apalache.
  The slash-form coverage summary also includes
  `quorum-rebroadcast-dispatch` for the pending rebroadcast dispatch helper and
  `isolated-vote-backed-handoff` for the one-vote frontier handoff helper, plus
  `preemptive-vote-backed-retransmit` for the pre-timeout retransmit handoff,
  plus `near-quorum-preemptive-escalation` for pre-timeout missing-payload
  recovery escalation, plus `paced-retransmit-targets` for deterministic
  backlog-throttled target selection.
  The Sumeragi suite also includes the `recovery-fsm-reason` helper slice for
  recovery reason string classification, deterministic ranks, and
  height/rank/peer recovery-event ordering. Its TLC cross-check independently
  exhausts the same sixteen expected-failure configs as Apalache.
  The Sumeragi suite also includes the per-reason pacemaker backpressure
  tracker helper slice for deferring gates, telemetry labels, and duration
  transitions.
  The Sumeragi suite also includes the distinct-vote-epochs helper slice for
  cached vote-log Commit-QC replay after payload recovery. Its TLC cross-check
  independently exhausts the same eleven expected-failure configs as Apalache.
  The Sumeragi suite also includes the requeue-transactions helper slice for
  committed-duplicate, routing, push-outcome, gossip, and pending-drop counters
  after failed commit recovery; the TLC helper independently cross-checks the
  same bounded mutation family.
  The `pending-rbc-status` family covers pending-RBC drop/stash/eviction
  counters, reset behavior, atomic overlay projection, and per-entry status
  snapshots. Its TLC cross-check independently exhausts the same twenty-seven
  expected-failure configs as Apalache.
  The `ingress-dedup-cache` family covers dedup TTL/capacity handling,
  duplicate refreshes, eviction cleanup, and per-payload bucket routing. Its
  TLC cross-check independently exhausts the same thirty-one expected-failure
  configs as Apalache.
  The `ingress-status-counters` family covers inbound consensus gossip,
  retransmit, background-drop, block-created, dedup-eviction, and
  message-handling status counters. Its TLC cross-check independently exhausts
  the same thirty-three expected-failure configs as Apalache.
  The `consensus-message-labels` family covers stable exported labels for
  consensus message kind, handling outcome, and handling reason dimensions.
  Its TLC cross-check independently exhausts the same twenty-five
  expected-failure configs as Apalache.
  The `qc-rebuild-status` family covers QC rebuild attempts, successful
  rebuilds, accepted QCs with missing local votes, quorum-without-QC
  observations, reset behavior, and snapshot projection. Its TLC cross-check
  independently exhausts the same twenty-one expected-failure configs as
  Apalache.
  The `qc-rebuild-quorum` family covers permissioned signer-count quorum
  reachability and NPoS signer/stake-snapshot quorum evidence. Its TLC
  cross-check independently exhausts the same fourteen expected-failure configs
  as Apalache.
  The `collector-targeting-status` family covers current collector-target
  storage, last-commit collector-target storage, redundant-send accumulation,
  reset behavior, and snapshot projection. Its TLC cross-check independently
  exhausts the same eighteen expected-failure configs as Apalache.
  The `phase-latency-status` family covers latest/max/EMA phase latency
  projection and saturated pipeline totals in `phase_latencies_snapshot()`.
  Its TLC cross-check independently exhausts the same twenty-nine
  expected-failure configs as Apalache.
  The `telemetry-status` family covers availability vote counters, QC latency
  overwrite/sort semantics, and direct RBC/pipeline status projections. Its
  TLC cross-check independently exhausts the same twenty-seven
  expected-failure configs as Apalache.
  The `lane-detail-status` family covers lane commitment, relay, governance,
  Nexus-disabled stripping, and route gating for lane-detail status fields.
  Its TLC cross-check independently exhausts the same twenty-eight
  expected-failure configs as Apalache.
  The `settlement-status` family covers DvP/PvP settlement telemetry reset,
  event counters, last-event snapshots, and JSON status projection. Its TLC
  cross-check independently exhausts the same thirty-two expected-failure
  configs as Apalache.
  The `nexus-economics-status` family covers Nexus fee debit outcome counters,
  public-lane staking deltas, status projection, reset accounting, and strip
  behavior when Nexus lane details are disabled. Its TLC cross-check
  independently exhausts the same thirty-three expected-failure configs as
  Apalache.
  The `npos-repair-coverage-status` family covers NPoS repair fanout coverage
  recording, reset behavior, direct snapshots, and mode-gated status
  projection. Its TLC cross-check independently exhausts the same seventeen
  expected-failure configs as Apalache.
  The `mode-status` family covers PRF context publication, mode tag and
  activation-lag status, mode-flip kill switch, blocked state, counters,
  timestamps, and last-error projection. Its TLC cross-check independently
  exhausts the same twenty-nine expected-failure configs as Apalache.
  The `consensus-caps-status` family covers consensus capability storage,
  getter behavior, overwrite semantics, and top-level status projection. Its
  TLC cross-check independently exhausts the same twenty-six expected-failure
  configs as Apalache.
  The `effective-timing-status` family covers effective timing scalar,
  scheduling, fanout, optional NPoS timeout, clear, overwrite, and top-level
  status projection semantics. Its TLC cross-check independently exhausts the
  same thirty expected-failure configs as Apalache.
  The `tx-queue-backpressure-status` family covers transaction queue
  backpressure depth/capacity storage, explicit saturation state, getter
  projection, overwrite behavior, and top-level status projection. Its TLC
  cross-check independently exhausts the same twenty expected-failure configs
  as Apalache.
  The `effective-mode` family covers effective consensus-mode fallback,
  inclusive activation boundaries, pre-activation inversion after a local flip,
  and staged-mode status projection. Its TLC cross-check independently
  exhausts the same fifteen expected-failure configs as Apalache.
  The `effective-timing` family covers active/effective/worker consensus
  timing aggregation, DA timeout derivation, staged-mode timing fields, and
  NPoS commit-time floors. Its TLC cross-check independently exhausts the same
  twenty-two expected-failure configs as Apalache.
  The `hotspot-log-summary` family covers hotspot summary initialization,
  saturated counter updates, due-boundary logging, suppressed-only emission,
  empty due refreshes, and reset behavior. Its TLC cross-check independently
  exhausts the same twenty-one expected-failure configs as Apalache.
  The `adaptive-observability` family covers adaptive observability enablement,
  DA burst and latency triggers, cooldown boundaries, collector fanout floors,
  resilience caps, and missing-data baselines. Its TLC cross-check
  independently exhausts the same twenty-five expected-failure configs as
  Apalache.
  The `counter-backpressure-cooldown` family covers relay/drop/block counter
  sources, cooldown boundaries, reset/disable behavior, queue projections, and
  saturating arithmetic. Its TLC cross-check independently exhausts the same
  eighteen expected-failure configs as Apalache.
  The `live-vote-roster` family covers future-height rejection, pending
  activation priority, empty-pending suppression, Permissioned/NPoS fallback
  sources, and live-key filtering. Its TLC cross-check independently exhausts
  the same sixteen expected-failure configs as Apalache.
  The `canonical-round-roster` family covers commit-QC roll-forward history,
  future-height fail-closed rules, pending/previous/active fallback ordering,
  parent-hash matching, candidate filtering, live-key filtering, and output
  canonicalization. Its TLC cross-check independently exhausts the same
  twenty-two expected-failure configs as Apalache.
  The `commit-topology-state` family covers topology hash refreshes,
  order-only versus membership-change classification, roster-change reset
  surfaces, proposals-seen preservation, runtime-cache cleanup, and commit
  handling branches. Its TLC cross-check independently exhausts the same
  twenty-nine expected-failure configs as Apalache.
  The `precommit-signer-history` family covers history ordering, exact lookup,
  roster shape and signer bounds, Permissioned/NPoS quorum fallback, stake
  snapshot checks, returned artifact shape, and cached-QC reconstruction. Its
  TLC cross-check independently exhausts the same twenty-three
  expected-failure configs as Apalache.
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
  The `rbc-status-lookup` family covers delivered/complete payload lookups,
  exact-view matching, stale summary boundaries, and next-due computations.
  Its TLC cross-check independently exhausts the same twenty-one
  expected-failure configs as Apalache.
  The `rbc-status-retention` family covers TTL/capacity pruning, newest-entry
  retention, active-count publication, and disk-persistence decisions after
  status updates. Its TLC cross-check independently exhausts the same
  seventeen expected-failure configs as Apalache.
  The `rbc-status-persistence` family covers main/temp store selection,
  invalid-store cleanup, temp promotion, parent sync, persistence disablement,
  fatal metrics, and temp-path projection. Its TLC cross-check independently
  exhausts the same thirty-three expected-failure configs as Apalache.
  The `rbc-status-handle` family covers configure/remove/clear/update
  lifecycle behavior, disk persistence gates, active-count publication,
  recovered-from-disk preservation, and global active-handle accessors. Its TLC
  cross-check independently exhausts the same thirty-one expected-failure
  configs as Apalache.
  The `rbc-backlog-status` family covers DA/active/invalid/authoritative
  filters, pending-stash tip/next inclusion, proposal-blocking semantics, soft
  backlog thresholds, and status snapshot maxima/caps. Its TLC cross-check
  independently exhausts the same twenty-five expected-failure configs as
  Apalache.
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
  The `qc-signers` family covers QC signer-bitmap admission: bitmap length must
  match the full topology width, bits outside the topology are rejected, quorum
  accounting counts only voting validators, observer and padding bits cannot
  satisfy quorum, and under-quorum signer sets are rejected. Its TLC
  cross-check independently exhausts the same four expected-failure configs as
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
  deterministic RBC chunk payload frame-cap derivation. Its TLC cross-check
  independently exhausts the same twelve expected-failure configs as Apalache.
  The `rbc-chunk-target` family covers initial RBC chunk fanout target counts,
  configured-cap clamping, quorum floors, zero-target selections, truncation,
  and local-peer exclusion. Its TLC cross-check independently exhausts the
  same ten expected-failure configs as Apalache.
  The `rbc-rebroadcast-selection` family covers payload and READY rebroadcaster
  counts, empty-roster zeroes, count caps, full-roster selection, leader
  inclusion, and absent-local rejection. Its TLC cross-check independently
  exhausts the same eleven expected-failure configs as Apalache.
  The `rbc-chunk-allocation` family covers empty/zero allocation, all-zero
  fallback, weighted floor division, largest-remainder ties, zero-weight
  exclusion, and min-one trimming. Its TLC cross-check independently exhausts
  the same eleven expected-failure configs as Apalache.
  The `rbc-payload-chunking` family covers zero-size clamping, empty payload
  chunking, ceiling division, chunk-count projection, and full payload
  coverage. Its TLC cross-check independently exhausts the same ten
  expected-failure configs as Apalache.
  The `rbc-rs16-initial-fanout` family covers full/plain/zero-required
  bypasses, data and data-plus-one fanout widths, parity clamping, sorted
  deduplicated selections, and stripe coverage. Its TLC cross-check
  independently exhausts the same twelve expected-failure configs as Apalache.
  The `rbc-chunk-broadcast-order` family covers shuffle gating, drop interval
  boundaries, drop-every-one behavior, oversized interval no-ops, and filtered
  order preservation. Its TLC cross-check independently exhausts the same
  twelve expected-failure configs as Apalache.
  The `rbc-commit-processing` family covers READY/DELIVER commit-pipeline wake
  decisions after pending-state cleanup, READY quorum, evidence changes, and
  first DELIVER observation. Its TLC cross-check independently exhausts the
  same ten expected-failure configs as Apalache.
  The `rbc-deliver-acceptance` family covers final DELIVER acceptance ordering
  across READY quorum, chunk availability, missing-chunk policy, zero-total
  sessions, and chunk-root equality. Its TLC cross-check independently
  exhausts the same ten expected-failure configs as Apalache.
  The `rbc-payload-layout` family covers invalid layouts, legacy unknown
  payload sizes, plain/RS16 chunk and payload-index mappings, parity slots, and
  encoded chunk lengths. Its TLC cross-check independently exhausts the same
  sixteen expected-failure configs as Apalache.
  The `rbc-session-chunk-ingest` family covers session allocation guards, chunk
  bounds and digest checks, duplicate chunk idempotence, and mismatched-chunk
  cleanup. Its TLC cross-check independently exhausts the same nineteen
  expected-failure configs as Apalache.
  The `rbc-session-ready-deliver` family covers READY signature idempotence,
  conflicting READY invalidation, roster-hash binding, first DELIVER recording,
  and DELIVER replay idempotence. Its TLC cross-check independently exhausts
  the same twenty expected-failure configs as Apalache.
  The `rbc-delivered-payload-bytes` family covers delivered/completeness gates,
  known-layout byte projection, legacy saturated chunk summing, missing-slot
  rejection, fallback bytes, and once-only telemetry recording. Its TLC
  cross-check independently exhausts the same sixteen expected-failure configs
  as Apalache.
  The `pending-rbc-stash` family covers chunk/READY/DELIVER stash caps,
  byte-cap drops, last-seen refreshes, TTL and session-limit eviction,
  replay-on-flush behavior, dedup release, metrics, repair requests, and
  backlog publication. Its TLC cross-check independently exhausts the same
  forty-four expected-failure configs as Apalache.
  The Sumeragi suite also includes the proposal-backpressure helper slice for
  separating pacing-only deferral from hard proposal backpressure.
  The Sumeragi suite also includes the proposal-defer-warning helper slice for
  deterministic proposal deferral warning throttling.
  The Sumeragi suite also includes the peer-admin-detection helper slice for
  signed external instruction-batch admin detection: instruction IDs are
  matched case-insensitively, substring matches for `registerpeer` and
  `unregisterpeer` are admin-sensitive, reversed words and unrelated or empty
  IDs are not admin-sensitive, only external signed instruction batches are
  inspected, and a batch is admin-sensitive when any contained instruction is
  admin-sensitive. Its TLC cross-check independently exhausts the same eighteen
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the `qc-insufficient-warning` helper slice
  for deterministic QC-insufficient warning keying, cooldown, GC, and clear
  behavior: first-warning insertion, strict within-cooldown suppression,
  cooldown-boundary emission, suppressed-count replay/reset,
  per-kind/phase/hash/height/view key separation, zero-cooldown bypass, GC
  boundary/expiry behavior, zero-cooldown GC floor, and `clear()` entry reset
  semantics. Its TLC cross-check independently exhausts the same fourteen
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the lane-interleave helper slice for
  deterministic routing-decision round-robin order across lanes.
  The Sumeragi suite also includes the prf-leader-shuffle helper slice for
  deterministic PRF topology permutations, view-cycled leader selection,
  length/distinctness preservation, and canonical shuffled roster
  construction. Its TLC cross-check independently exhausts the same fifteen
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the topology-fanout helper slice for
  redundant-send fanout, view-change quorums, configured redundant floors, and
  proxy-tail fanout targets that wrap, exclude the leader, and preserve
  uniqueness. Its TLC cross-check independently exhausts the same eighteen
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the topology-role-filter helper slice for
  role partitioning, consensus role-slice helpers, role-filtered signature
  selection, and previous-block-hash audit-role rotation. Its TLC cross-check
  independently exhausts the same thirty-two expected-failure configs as
  Apalache.
  The Sumeragi suite also includes the active-topology-selection helper slice
  for commit/world/trusted source priority, BLS/dedup/canonical output
  shaping, PoP filtering quorum guards, and empty-source trusted fallback. Its
  TLC cross-check independently exhausts the same fifteen expected-failure
  configs as Apalache.
  The Sumeragi suite also includes the p2p-topology-trusted helper slice for
  world/local/trusted topology union, BTreeSet deduplication, and observed
  outside-peer filtering that preserves online order and duplicate stray
  observations. Its TLC cross-check independently exhausts the same nine
  expected-failure configs as Apalache.
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
  candidate ranking. Its TLC cross-check independently exhausts the same
  thirteen expected-failure configs as Apalache.
  The Sumeragi suite also includes the active-frontier NEW_VIEW catch-up helper
  slice for resilience gating, non-empty remote support, local signer exclusion,
  committed-frontier height gating, canonical highest-QC matching, hash-only
  canonical payload admission, tracked-view presence, and successor-window
  bounds. Its TLC cross-check independently exhausts the same fourteen
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the late NEW_VIEW near-quorum emission
  helper slice for frontier/view/local-index gates, permissioned and NPoS
  completion policy, stake-roster and signer-map error handling, same-slot
  supersession, completion-vs-catch-up ordering, and inner emission failure
  propagation. Its TLC cross-check independently exhausts the same twenty
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the near-quorum NEW_VIEW rebroadcast helper
  slice for validator/frontier/support/quorum admission, cooldown-floor gating,
  NEW_VIEW dispatch metadata, backpressure result handling, pacemaker nudge
  deadlines, and time-overflow fail-closed behavior. Its TLC cross-check
  independently exhausts the same seventeen expected-failure configs as
  Apalache.
  The Sumeragi suite also includes the requester roster-proof helper slice for
  committed snapshot, Commit-QC cache, precommit signer record, and highest-QC
  proof sources, plus no-evidence and wrong phase/hash/height/view/epoch or
  vNext chain-order rejection. Its TLC cross-check independently exhausts the
  same twenty expected-failure configs as Apalache.
  The Sumeragi suite also includes the online-validator and relay counter
  helper slice for roster membership filtering, peer-id identity, offline and
  outsider exclusion, duplicate roster canonicalization, online-iterator
  semantics, relay lane totals, direct counter forwarding, cap-family
  collection, and saturating arithmetic. Its TLC cross-check independently
  exhausts the same twenty-four expected-failure configs as Apalache.
  The Sumeragi suite also includes the commit-result drain gate slice for
  result-id ownership, stale and ownerless result suppression, disconnected
  worker cleanup, inline fallback admission, local-outside signature recovery,
  summary/progress side effects, pacemaker kickstart, inflight cleanup, and
  drain-loop stop semantics. Its TLC cross-check independently exhausts the
  same twenty-seven expected-failure configs as Apalache.
  The Sumeragi suite also includes the commit-drain summary aggregation helper
  slice for result-count saturation, progress ownership, absent timing
  handling, per-stage timing independence, and stage accumulator saturation.
  Its TLC cross-check independently exhausts the same ten expected-failure
  configs as Apalache.
  The Sumeragi suite also includes the commit-pipeline timing sample helper
  slice for finish-total replacement, duration saturation, core duration field
  mapping, drain-stage independence, total-vs-phase separation, and
  bookkeeping exclusion from status samples. Its TLC cross-check independently
  exhausts the same eleven expected-failure configs as Apalache.
  The Sumeragi suite also includes the commit-pipeline status recorder helper
  slice for status reset behavior, last-field storage, EMA initialization and
  blending, non-EMA field preservation, snapshot projection, and test reset
  cleanup. Its TLC cross-check independently exhausts the same twenty-six
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the autoscale transition commit gate slice
  for enabled checks, exact transition-height matching, success-path queue
  reconfiguration, failed-commit suppression, and reported-height preservation.
  Its TLC cross-check independently exhausts the same nine expected-failure
  configs as Apalache.
  The Sumeragi suite also includes the signature-index recovery helper slice
  for raw-index trust, fallback scanning, BLS eligibility, no-match and
  ambiguous-match rejection, duplicate detection, raw-priority preservation, and
  replacement fail-closed behavior. Its TLC cross-check independently exhausts
  the same thirteen expected-failure configs as Apalache.
  The Sumeragi suite also includes the commit-QC cache/history lookup helper
  slice for cache priority, exact history matching, aggregate presence,
  topology matching, and absent-history rejection. Its TLC cross-check
  independently exhausts the same twelve expected-failure configs as Apalache.
  The Sumeragi suite also includes the cached-QC precommit signer record helper
  slice for permissioned quorum policy, NPoS stake-snapshot policy, bitmap and
  aggregate admission, roster length preservation, and signer-count projection.
  Its TLC cross-check independently exhausts the same fourteen
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the embedded-QC roster bootstrap helper
  slice for roster shape, authoritative anchoring, proof-of-possession,
  permissioned quorum, NPoS stake snapshot and signer-map policy, aggregate
  recovery, cache replacement, and payload recovery deferral. Its TLC
  cross-check independently exhausts the same twenty-four expected-failure
  configs as Apalache.
  The Sumeragi suite also includes the roster-validation memo cache helper
  slice for construction, get/touch behavior, zero-capacity inserts,
  insert/update semantics, live-key eviction, lane isolation, refresh clearing,
  and shared capacity. Its TLC cross-check independently exhausts the same
  twenty-two expected-failure configs as Apalache.
  The Sumeragi suite also includes the roster-validation cached wrapper helper
  slice for subject prefilters, empty-aggregate memo bypass, memo-key input
  binding, memo hit/miss behavior, success insertion, validation argument
  forwarding, and prefilter-before-memo ordering. Its TLC cross-check
  independently exhausts the same twenty-four expected-failure configs as
  Apalache.
  The Sumeragi suite also includes the core roster-validation helper slice for
  roster emptiness, validator-set hash binding, signer-bitmap length and
  bounds, genesis-stub unsigned handling, permissioned/NPoS quorum, stake
  snapshot matching, PoP lookup, checkpoint root/expiry binding, preimage
  fields, BLS input selection, and return-shape preservation. Its TLC
  cross-check independently exhausts the same thirty expected-failure configs
  as Apalache.
  The Sumeragi suite also includes the roster artifact selection helper slice
  for commit/checkpoint/block view priority, no-artifact fall-through,
  cert-only and checkpoint-only selection, commit-preferred combined
  selection, checkpoint view/root attachment gates, roster-mismatch handling,
  stake-snapshot source priority, validation input/root/epoch choices, and
  genesis-stub admission. Its TLC cross-check independently exhausts the same
  twenty-eight expected-failure configs as Apalache.
  The Sumeragi suite also includes the block roster cache helper slice for
  roster-selection key admission, NPoS stake requirements, key-field retention,
  block-view exclusion, signer-key canonicalization and PRF seed binding,
  signer-cache and roster-cache clear/get/touch/update/eviction behavior, stale
  order handling, zero-capacity no-ops, and block-scoped signer-cache removal.
  Its TLC cross-check independently exhausts the same thirty expected-failure
  configs as Apalache.
  The Sumeragi suite also includes the block-sync roster evidence helper slice
  for missing commit-proof priority, Permissioned and NPoS classification,
  NPoS stake-snapshot requirements, exact `has_roster` projection, and applying
  roster selections into commit-QC, checkpoint, and stake-snapshot update lanes
  without changing unrelated fields. Its TLC cross-check independently exhausts
  the same twenty-one expected-failure configs as Apalache.
  The Sumeragi suite also includes the block-sync history roster helper slice
  for mode-tag selection, precommit exact filters and max-view choice,
  commit-QC/checkpoint history filters and max height/view choice, precommit
  derivation admission, source labels, roster height/view adjustment,
  checkpoint height filtering, stake-snapshot forwarding, and post-validation
  fallback. Its TLC cross-check independently exhausts the same twenty-seven
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the persisted block-sync roster selection
  helper slice for mode tags, commit-journal priority, cache hit and insertion
  guards, source labels, artifact recording, sidecar allow/hash gates,
  successor previous-hash and evidence-target gates, previous-roster stake
  conversion, checkpoint-only previous evidence, and fail-closed no-source
  behavior. Its TLC cross-check independently exhausts the same twenty-five
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the BlockSyncUpdate roster hydration helper
  slice for update construction, consensus-mode resolution, persisted before
  history ordering, lookup argument forwarding, short-circuit behavior,
  uncertified fallback admission, commit-topology/world fallback material,
  saturating live-key filtering, mode canonicalization, selection application,
  unrostered no-selection preservation, and NPoS stake-snapshot fill rules. Its
  TLC cross-check independently exhausts the same twenty-four expected-failure
  configs as Apalache.
  The Sumeragi suite also includes the roster index projection helper slice for
  empty-topology projection, contiguous local fallback, sparse provider
  positions, incomplete-provider fallback, provider-index overflow, manager
  normalization, empty-projection roster-length fallback, zero-length managers,
  and overflow preservation. Its TLC cross-check independently exhausts the
  same fifteen expected-failure configs as Apalache.
  The Sumeragi suite also includes the vote-duplicate-key helper slice for
  raw vote-log keys, NEW_VIEW highest-QC duplicate equality, and public-key
  identity projection.
  The Sumeragi suite also includes the vote-validation-drop-status helper slice
  for stable drop labels, bounded newest-first recent entries, peer/roster
  aggregates, status projection, and decadic log thresholds. Its TLC
  cross-check independently exhausts the same thirty-five expected-failure
  configs as Apalache.
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
  The `exec-witness-recorder` family covers execution-witness recorder
  lifecycle/keying: active capture gating, read/write/delete semantics, sorted
  drain snapshots, FastPQ grouping/finalize and apply-digest alignment, and
  metadata/root tag preservation. Its TLC cross-check independently exhausts
  the same thirty-nine expected-failure configs as Apalache.
  The `exec-witness-access-key` family covers execution-witness access-key
  parsing: supported account/domain/asset/NFT/role and permission prefixes,
  malformed and unsupported IDs, Boolean parsing, and split-tail/prefix
  fallthrough handling. Its TLC cross-check independently exhausts the same
  twenty-nine expected-failure configs as Apalache.
  The `smt-path-hash` family covers sparse-Merkle path/hash helpers: empty
  roots, input leaf hashing, leaf/node domain tags, child order, missing
  children, duplicate-key ordering, truncation, and parent/child prefix-bit
  rules. Its TLC cross-check independently exhausts the same twenty
  expected-failure configs as Apalache.
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
  The `kura-replica-advert` family covers Kura replica advert ingress:
  authenticated remote-only admission, local self suppression,
  peer/height/hash/payload-length metadata, and zero-payload
  normalization/rejection. Its TLC cross-check independently exhausts the same
  twelve expected-failure configs as Apalache.
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
  The `frontier-block-sync-hint` family covers frontier block-sync hints and
  direct-response permits: pause-gate defaults, pressure/lane gating,
  absent-peer rejection, request recording, pending-count pruning, fresh and
  expired TTL boundaries, peer ownership, and direct-response consumption. Its
  TLC cross-check independently exhausts the same twenty-seven
  expected-failure configs as Apalache.
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
  The `missing-payload-fetch-window` family covers same-height missing-payload
  targeted-fetch pacing and lock-lag hash-miss cap widening: snapshot gating,
  window freshness, entered-at preservation, absent marker read/write/clear
  behavior, and cap widening/clamping. Its TLC cross-check independently
  exhausts the same twenty-four expected-failure configs as Apalache.
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
  The `evidence-canonicalization` family covers canonical keys, subject
  height/view extraction, block references, valid/invalid store insertion,
  canonical storage keys, persistence defaults, duplicate rejection, and unset
  penalty flags. Its TLC cross-check independently exhausts the same
  thirty-seven expected-failure configs as Apalache.
  The `evidence-validation` family covers kind/payload matching, double-vote
  signature, phase, height, epoch, signer, block/root conflict, and precedence
  checks, invalid proposal height, parent, and view-reset handling, and
  censorship receipt transaction, signer, signature, quorum, deduplication, and
  precedence checks. Its TLC cross-check independently exhausts the same
  thirty-nine expected-failure configs as Apalache.
  The `double-vote-recording` family covers height, epoch, topology-resolved
  signer identity, block/root conflict detection, phase-pair support, canonical
  evidence kind/key selection, no-evidence side effects, store rejection,
  persistence calls, persistence rejection, and duplicate handling. Its TLC
  cross-check independently exhausts the same thirty-seven expected-failure
  configs as Apalache.
  The `invalid-qc-shape` family covers empty signer bitmaps, zero height/view
  sentinel handling, height-zero or view-zero non-emitting cases, valid nonempty
  QCs, evidence kind selection, certificate payload cloning, and fixed
  diagnostic reason handling. Its TLC cross-check independently exhausts the
  same sixteen expected-failure configs as Apalache.
  The `qc-validation-evidence` family covers hard malformed-certificate QC
  validation errors that emit cloned `InvalidQc` evidence with the original
  reason, soft local-context or quorum failures that must not emit evidence,
  successful validation returning `Ok`, and validation errors remaining errors.
  Its TLC cross-check independently exhausts the same twenty-four
  expected-failure configs as Apalache.
  The `qc-validation-reason` family covers stable telemetry labels for every QC
  validation error, hard failures emitting evidence with the same label, and
  soft local-context/quorum failures retaining telemetry labels without evidence.
  Its TLC cross-check independently exhausts the same seventeen expected-failure
  configs as Apalache.
  The `block-sync-qc-fallback` family covers retryable missing-context QC
  errors, non-retryable malformed-certificate errors, COMMIT-only aggregate
  fallback, nested highest-QC rejection, aggregate and bitmap validity,
  permissioned quorum floors, and NPoS snapshot/stake-quorum rejection. Its TLC
  cross-check independently exhausts the same nineteen expected-failure configs
  as Apalache.
  The Sumeragi suite also includes the p2p-topology-refresh helper slice for
  empty, unchanged, changed, and stray refresh decisions, the local-seen latch,
  local-removal queue clearing, empty gossip updates, trusted-peer network
  topology updates, and `last_advertised` mutation. Its TLC cross-check
  independently exhausts the same twenty-two expected-failure configs as
  Apalache.
  The Sumeragi suite also includes the quorum-retransmit helper slice for
  commit-vote repair target selection, near-quorum full fanout,
  signer-mapping fallback, local exclusion, stable target ordering, duplicate
  rejection, and view-mapped peer resolution. Its TLC cross-check independently
  exhausts the same twelve expected-failure configs as Apalache.
  The Sumeragi suite also includes the retransmit-backpressure helper slice for
  transaction/RBC pressure scoring, paced target limits, cooldown
  multiplication, consensus-ingress backoff scaling, and near-quorum timeout
  clamps. Its TLC cross-check independently exhausts the same twenty-two
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the paced-retransmit-targets helper slice
  for deterministic target selection under pacing limits: zero/empty
  fail-closed behavior, sort/dedup before over-limit truncation, deterministic
  height/view offset rotation, exact truncation, and duplicate preservation
  where the list already fits. Its TLC cross-check independently exhausts the
  same seventeen expected-failure configs as Apalache.
  The Sumeragi suite also includes the quorum-reschedule-backoff helper slice
  for vote-deficit backoff multipliers, moderate/severe stall escalation,
  resend-window clamping, and contiguous-frontier fast-resend gating under
  relay, vote-queue, and RBC backpressure. Its TLC cross-check independently
  exhausts the same twenty expected-failure configs as Apalache.
  The Sumeragi suite also includes the rbc-availability-reschedule helper
  slice for DA/RBC availability gating: fail-open behavior outside DA mode,
  after timeout, for local payloads, absent/invalid/delivered/complete-ready
  sessions, and fail-closed behavior for pending entries, missing chunks, and
  missing READY quorum before timeout. Its TLC cross-check independently
  exhausts the same thirteen expected-failure configs as Apalache.
  The Sumeragi suite also includes the vote-backed-reassembly-stall helper
  slice for hard-cap arithmetic, same-height frontier slot ownership, recovery
  owner fallback after rejected slots, latest progress timestamps, and owner
  plus quorum stall-age expiry. Its TLC cross-check independently exhausts the
  same nineteen expected-failure configs as Apalache.
  The Sumeragi suite also includes the completed-quorum-view-advance helper
  slice for exact slot-event routing, generic non-exact routing, stale/no-slot
  fallback, current-view max selection, saturating view advance, timestamp and
  cause preservation, and rebroadcast-latch clearing. Its TLC cross-check
  independently exhausts the same fifteen expected-failure configs as
  Apalache.
  The Sumeragi suite also includes the quorum-rebroadcast-dispatch helper
  slice for local-vote gating, fail-closed relay/no-target/cooldown/backlog
  exits, forced fanout bypasses, vote replay before payload repair, missing
  commit-QC fetch gating, near-quorum BlockSyncUpdate fanout, BlockCreated
  replay gating, and precommit rebroadcast marker stamping. Its TLC cross-check
  independently exhausts the same twenty-four expected-failure configs as
  Apalache.
  The Sumeragi suite also includes the isolated-vote-backed-handoff helper
  slice for resilience, one-vote under-quorum, next-height, and cached-QC
  admission gates, recovery/body-event side effects, seeded slot validation,
  committed-anchor range-pull success, and reason-label preservation. Its TLC
  cross-check independently exhausts the same nineteen expected-failure configs
  as Apalache.
  The Sumeragi suite also includes the preemptive-vote-backed-retransmit helper
  slice for pre-timeout candidate admission, absent-pending fail-closed
  behavior, vote-roster target preference and commit-topology fallback, empty
  target preservation, downstream action detection, pending retention, and
  near-quorum flag accuracy. Its TLC cross-check independently exhausts the
  same twenty-two expected-failure configs as Apalache.
  The Sumeragi suite also includes the near-quorum-preemptive-escalation helper
  slice for exhausted-budget fail-closed behavior, missing-pending rejection,
  fresh request and in-flight range-pull duplicate suppression, stale/mismatched
  duplicate admission, delegate-count/progress authority, and the one-candidate
  per-tick cap. Its TLC cross-check independently exhausts the same twenty-two
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the manifest-gate-reschedule helper slice
  for manifest-gated effective-work classification, retention, marker
  selection, no-target no-ops, authoritative-rotation suppression, plain
  zero-work cleanup, and vote-backed evidence/frontier-owner effectiveness. Its
  TLC cross-check independently exhausts the same twenty-five expected-failure
  configs as Apalache.
  The Sumeragi suite also includes the build-signers-bitmap helper slice for
  empty-roster handling, exact bitmap byte length, little-endian bit placement,
  ORing multiple signers, duplicate collapse, and out-of-range/padding-bit
  filtering. Its TLC cross-check independently exhausts the same seventeen
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the commit-roots helper slice for
  permissioned and NPoS same-root QC aggregation, wrong-context vote rejection,
  deterministic low-root tie-breaks, mixed-root quorum rejection, and QC
  validation root-mismatch rejection. Its TLC cross-check independently
  exhausts the same six expected-failure mutations as Apalache using smaller
  witness-constrained TLC-specific configs; Apalache retains the broader
  bounded search.
  The Sumeragi suite also includes the commit-pipeline-recovery helper slice
  for local commit-QC formation before peer recovery, stale local-vote recovery
  admission, commit-QC marker preservation, missing-payload and off-tip
  rejection, near-quorum retransmit, and quorum missing-signer targets. Its TLC
  cross-check independently exhausts the same fourteen expected-failure configs
  as Apalache.
  The Sumeragi suite also includes the known-block-commit-qc-recovery helper
  slice for commit-QC-only vs body fetch planning, pending-tip extension,
  stale-view commit-QC fetch admission, local commit-vote and consensus-active
  gates, parent/tip continuity, and override/map source selection. Its TLC
  cross-check independently exhausts the same twenty expected-failure configs
  as Apalache.
  The Sumeragi suite also includes the stale-view-commit-qc-fetch helper slice
  for exact hash/height/view matching, valid and active pending state, local
  commit-vote gating, exact tip extension, parent/tip continuity, and the
  all-absent parent/tip case. Its TLC cross-check independently exhausts the
  same eleven expected-failure configs as Apalache.
  The Sumeragi suite also includes the commit-anchor-qc helper slice for
  highest/locked QC selection, equal/newer anchor handling, precommit vote
  pruning on lock changes, incompatible highest-QC realignment, and final
  status updates. Its TLC cross-check independently exhausts the same twelve
  expected-failure configs as Apalache.
  The Sumeragi suite also includes TLC cross-checks for the locked-QC helper,
  precommit-QC locked-chain wrapper, precommit-vote lock filter, and precommit
  vote-emission gate. These independently exhaust the same fifteen, fifteen,
  seventeen, and nine expected-failure configs as Apalache.
  The Sumeragi suite also includes the committed-height-qc helper slice for
  future-QC continuation, matching committed-block record-only side effects,
  unknown/divergent stale-drop behavior, divergent commit-QC validation
  context, genesis-stub policy, stake snapshot preservation, and finality
  evidence emission. Its TLC cross-check independently exhausts the same
  twenty-six expected-failure configs as Apalache.
  The Sumeragi suite also includes the empty-block-qc-drop helper slice for
  non-NewView empty-block QC rejection, known-block/non-empty/time-trigger
  pass-through behavior, invalid-payload recording, downstream stop/continue
  selection, and block-scoped pending/request/RBC/QC/proposal/vote/roster/signer
  cleanup. Its TLC cross-check independently exhausts the same twenty-two
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the pending-progress helper slice for exact,
  non-aborted pending-map and commit-inflight owner touches, activation-window
  refresh field resets, post-commit tip-extending pending-map activation
  refreshes, and RBC recent-progress zero-window/height/age gating. Its TLC
  cross-check independently exhausts the same twenty-seven expected-failure
  configs as Apalache.
  The Sumeragi suite also includes the pending-block-lifecycle helper slice for
  constructor defaults, same-subject lifecycle preservation, different-subject
  lifecycle reset, revive/abort/retire accessors, Kura persistence preservation
  or reset, scheduler cleanup, and retired-payload refresh behavior. Its TLC
  cross-check independently exhausts the same twenty-five expected-failure
  configs as Apalache.
  The Sumeragi suite also includes the pending-block-marker helper slice for
  local commit-vote emission, commit-QC observation, reset behavior, quorum
  reschedule cooldowns, vote-backed stale-progress and vote-count gating,
  precommit rebroadcast cooldowns, validation redrive cooldowns, and marker
  writes. Its TLC cross-check independently exhausts the same twenty-seven
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the kura-retry helper slice for retry due
  boundaries, reset and mark-persisted cleanup, zero-budget and max-attempt
  aborts, exponential backoff, checked-add overflow handling, and public
  `next_in_ms` clamping. Its TLC cross-check independently exhausts the same
  twenty-one expected-failure configs as Apalache.
  The Sumeragi suite also includes the commit-pipeline-scheduling helper slice
  for tick/event entry, wakeup clearing, deadline bypass, recovery-candidate
  inclusion, budget-exhaustion wakeups, backlog observation, last-run updates,
  candidate processing, and idle-view budget preservation. Its TLC cross-check
  independently exhausts the same thirty-two expected-failure configs as
  Apalache.
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
  The Sumeragi suite also includes the DA gate helper slice for pure
  availability gate evaluation, satisfaction transitions, and manifest labels.
  Its TLC cross-check independently exhausts the same eighteen
  expected-failure configs as Apalache.
  The Sumeragi suite also includes the DA gate status slice for
  missing-availability counter, latest reason, satisfaction, snapshot, and
  reset accounting. Its TLC cross-check independently exhausts the same
  twenty-five expected-failure configs as Apalache.
  The Sumeragi suite also includes the consensus-handshake-caps helper slice
  for deterministic mode/domain, canonical-genesis-params, and fingerprint
  construction. Its TLC cross-check independently exhausts the same
  twenty-four expected-failure configs as Apalache.
  It first runs
  `scripts/formal/check_sumeragi_formal_coverage.py` so runner modes, CI
  commands, README commands, and referenced TLA+/CFG files stay in sync before
  Apalache starts. For reproducible local setup without Docker, install the pinned
  toolchain with `bash scripts/formal/install_apalache.sh 0.52.2`.
  The `block-sync-roster-status` family covers block-sync roster source/drop
  counters, snapshot projection, and reset accounting. Its TLC cross-check
  independently exhausts the same twenty-four expected-failure configs as
  Apalache.
  The `block-sync-qc-status` family covers block-sync QC/drop counters,
  final-drop reason projection, and reset accounting. Its TLC cross-check
  independently exhausts the same twenty-four expected-failure configs as
  Apalache.
  The `block-sync-locked-qc` family covers locked-chain extension checks,
  missing locked-payload newer-view handling, parent extension and rejection,
  same-height conflict view gates, same-height recoverability gates, locked
  payload deferral/quarantine/drop side effects, and stale-lock predicates. Its
  TLC cross-check independently exhausts the same twenty-two expected-failure
  configs as Apalache.
  The `known-block-qc-enqueue` family covers canonical QC vote-key projection,
  duplicate suppression without overwrites, new-work insertion and preservation,
  deferred aggregate-verification status fields, queued debug and length
  observation, and wake-sender attempt/ignore semantics. Its TLC cross-check
  independently exhausts the same twenty-seven expected-failure configs as
  Apalache.
  The `known-block-qc-work` family covers empty-topology recovery, QC/block
  shape mismatch drops, same-height locked-QC deferral and drop side effects,
  recoverable same-height work, stale-lock drops, non-extending retention,
  extending/no-lock work admission, and work-field preservation. Its TLC
  cross-check independently exhausts the same twenty-two expected-failure
  configs as Apalache.
  The `known-block-qc-drain` family covers empty-queue returns, initial and
  mid-drain tick-budget stops, per-tick cap handling, remaining-work
  preservation, progress return projection, processed counters, debug logging,
  and remove-before-apply ordering. Its TLC cross-check independently exhausts
  the same eighteen expected-failure configs as Apalache.
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
  Its TLC cross-check independently exhausts the same fifteen expected-failure
  configs as Apalache.
  The `missing-qc-liveness-status` family covers missing-block hard-cap,
  missing-QC reacquire, forced-proposal, and stuck-round status projection
  plus reset accounting. Its TLC cross-check independently exhausts the same
  twenty-two expected-failure configs as Apalache.
  The `sidecar-no-proposal-status` family covers sidecar mismatch quarantine,
  final-drop, recovery-trigger, no-proposal storm, and storm diagnostic
  snapshot projection plus reset accounting. Its TLC cross-check independently
  exhausts the same twenty-three expected-failure configs as Apalache.
  The `deterministic-committee-status` family covers selected transport
  committee-size publication, snapshot projection, overwrite semantics, and
  reset accounting. Its TLC cross-check independently exhausts the same eight
  expected-failure configs as Apalache.
  The `timing-status-counters` family covers pacemaker backpressure, commit
  tick, prevote timeout, DA reschedule, and RBC DELIVER deferral counters,
  status/getter projection, and available reset accounting. Its TLC
  cross-check independently exhausts the same twenty-six expected-failure
  configs as Apalache.
  The `round-trace-status` family covers round-trace transition results, gap
  snapshot projection, bounded trace retention, commit-pipeline wakeups, and
  event metadata copied into operator status snapshots. Its TLC cross-check
  independently exhausts the same twenty-eight expected-failure configs as
  Apalache.
  The `roster-recovery-status` family covers roster-unavailability and
  catch-up isolation counters, recovery state/dwell snapshot projection, and
  reset accounting. Its TLC cross-check independently exhausts the same
  twenty-five expected-failure configs as Apalache.
  The `range-pull-status` family covers range-pull escalation/success/failure
  and candidate-exhausted counters, expiry-streak max/last accounting,
  snapshot projection, and reset accounting. Its TLC cross-check independently
  exhausts the same twenty expected-failure configs as Apalache.
  The `round-recovery-bundle-window` family covers source/class labels,
  height-keyed commit/non-commit reservation partitions, explicit-window
  boundaries, and zero-window flooring for same-height recovery bundle pacing.
  Its TLC cross-check independently exhausts the same nineteen
  expected-failure configs as Apalache.
  The `worker-queue-status` family covers queue-depth, blocked-duration,
  dropped-count, worker-loop diagnostic, stage, iteration, and commit-inflight
  pause/resume status accounting. Its TLC cross-check independently exhausts
  the same twenty-six expected-failure configs as Apalache.
  The `same-height-vote-conflict` family covers local same-height vote
  selection, conflicting-slot/frontier predicates, certified and NewView
  supersession gates, and pending verification deferral. Its TLC cross-check
  independently exhausts the same thirty-six expected-failure configs as
  Apalache.
  The `proposal-stale-vote` family covers proposal-side fresh/stale
  same-height vote gates, recovery-age/view-gap escapes, active-owner
  suppression, and missing-QC repair bypass rules. Its TLC cross-check
  independently exhausts the same thirty-eight expected-failure configs as
  Apalache.
  The `signed-quorum-fetch-fallback` family covers committed signed-quorum
  fallback admission for fetch/body recovery: committed-hash gating, primary
  roster versus fallback topology selection, signer validation, permissioned
  quorum floors, NPoS state/cache snapshot priority, signer-peer mapping, and
  stake-quorum rejection. Its TLC cross-check independently exhausts the same
  sixteen expected-failure configs as Apalache.
  The `commit-qc-only-fetch-response` family covers direct commit-QC and
  signed-quorum fallback dispatch for commit-QC-only fetch responses: direct
  commit-QC companion sends, certified-proof companion ordering, vote
  rebroadcast suppression/admission, requester target inclusion and
  deduplication, signed-quorum fallback dispatch, bypass flag projection,
  requester roster-proof preservation, and return-value handling. Its TLC
  cross-check independently exhausts the same sixteen expected-failure configs
  as Apalache.
  The `block-sync-update-targets` family covers BlockSyncUpdate gossip target
  selection: zero-limit and empty-peer outputs, local-peer exclusion,
  registered/trusted stray priority, unregistered stray rejection,
  trusted-stray admission, online world-peer preference, offline world
  fallback, final fanout caps, and underfill prevention. Its TLC cross-check
  independently exhausts the same eleven expected-failure configs as Apalache.
  The `apply-cached-qcs` family covers cached BlockSyncUpdate proof/vote
  attachment: commit-QC source priority, existing QC and checkpoint
  preservation, checkpoint synthesis from final QCs, no spurious checkpoint
  creation, NPoS stake snapshot repair, permissioned stake non-repair,
  record-stake cloning, cached vote attachment, existing vote preservation, and
  wrong-context vote rejection. Its TLC cross-check independently exhausts the
  same sixteen expected-failure configs as Apalache.
  The `block-sync-roster` family covers uncertified block-sync roster
  admission: explicitly requested missing blocks at stale, same, next, and
  future heights, unrequested exact-next admission from zero and nonzero
  heights, Rust-style saturated next-height admission, and unrequested
  stale/same/future rejection. Its TLC cross-check independently exhausts the
  same nine expected-failure configs as Apalache.
  The `block-sync-vote-deferral` family covers embedded commit-vote filtering,
  vote-backed request refresh, known-block vote-only fast path, and the
  vote-stripped deferral handoff: valid commit-vote processing, invalid
  phase/hash/height/view/epoch vote rejection, mixed-vote filtering,
  missing-block request refresh and explicit request preservation, known-block
  vote-only fast-path return/cleanup, QC-bearing known block deferral,
  vote-stripped deferral ordering, QC/checkpoint/stake sidecar preservation,
  and deferral reason forwarding. Its TLC cross-check independently exhausts
  the same twenty-two expected-failure configs as Apalache.
  The `block-sync-known-hintless` family covers the already-known,
  roster-hint-free BlockSyncUpdate fast path and its missing-request cleanup:
  known block skip admission, missing-request clearing with the
  `PayloadAvailable` reason, no status recording, `Ok(())` return without
  continuation, unknown-block continuation, and commit-QC/checkpoint/stake/vote
  roster-hint preservation. Its TLC cross-check independently exhausts the same
  twelve expected-failure configs as Apalache.
  The `block-sync-implicit-recovery` family covers the DA implicit
  missing-block recovery flag and verifies that it has no direct side effects:
  already-requested preservation, DA-disabled/known-local/above-frontier/
  implicit-disallowed rejection, same-height/next-height/saturated-boundary
  request admission, and the no-status/no-clear/no-deferral/no-early-return
  contract. Its TLC cross-check independently exhausts the same twelve
  expected-failure configs as Apalache.
  The `missing-block-ingress-fetch` family covers the exact-frontier
  authoritative body ingress grace gate before generic missing-block fetches.
  Its TLC cross-check independently exhausts the same twelve expected-failure
  configs as Apalache.
  The `payload-progress-availability` family covers actor-local block payload
  material that can unblock consensus progress. Its TLC cross-check
  independently exhausts the same twelve expected-failure configs as Apalache.
  The `highest-qc-fetch-body-known` family covers the body-known gate that
  suppresses highest-QC body fetches only for Kura and non-aborted local owners.
  The `local-payload-availability` family covers the broad actor-local payload
  predicate before stricter progress/fetch/lock filters are applied. Their TLC
  cross-checks independently exhaust the same twelve and twelve
  expected-failure configs as Apalache.
  The `block-known-locally` family covers local block-known routing before
  stricter lock/progress filters.
  The `block-known-for-lock` family covers lock-safety block-known routing,
  including pending-validity filtering and rejected-owner fallthrough. Their
  TLC cross-checks independently exhaust the same twelve and fifteen
  expected-failure configs as Apalache.
  The `local-signed-block-lookup` family covers normal and body-repair
  signed-block materialization, source priority, and rejected-owner fallthrough.
  Its TLC cross-check independently exhausts the same sixteen expected-failure
  configs as Apalache.
  The `authoritative-payload-progress` family covers strict progress payload
  lookup, rejected-owner fail-closed behavior, deferred-payload exclusion, and
  Kura committed-hash filtering.
  The `authoritative-block-payload` family covers hash-level authoritative
  payload availability, local-source short-circuiting, rejected-local RBC
  fallback, and RBC hash/authority filtering. Their TLC cross-checks
  independently exhaust the same twelve and thirteen expected-failure configs
  as Apalache.
  The `block-payload-canonicalization` family covers canonical proposal payload
  byte construction, result-root/signature/stale-header-root exclusion, and
  canonical field binding. Its TLC cross-check independently exhausts the same
  twelve expected-failure configs as Apalache.
  The `pending-block-active-for-tip` family covers active pending-block
  selection, consensus-inactive filtering, tip-extension checks, and the
  consensus-evidence disjunction.
  The `pending-fast-unblock` family covers zero-timeout disablement, evidence
  short-circuits, stored vote/cached QC gating, and inclusive fast-timeout age
  checks. Their TLC cross-checks independently exhaust the same sixteen and
  twelve expected-failure configs as Apalache.
  The `blocking-pending-blocks` family covers classic and progress-aware
  blocking counters, zero-quorum fallback, vote/QC evidence precedence, quorum
  reschedule release, and stall-grace/quorum-timeout window boundaries. Its
  TLC cross-check independently exhausts the same eighteen expected-failure
  configs as Apalache.
  The `quorum-recovery-vote-drain` family covers vote-drain urgency from
  quorum timeout, live tip-extending pending ownership, vote/QC evidence,
  waiting vote backlog, evidence-specific age source, and existential pending
  scans.
  The `frontier-body-gap-payload-drain` family covers exact normal
  frontier-slot shape, body absence, accepted wait phases, vote-backed quorum
  evidence, and payload/block backlog routing for urgent payload drain. Their
  TLC cross-checks independently exhaust the same seventeen and sixteen
  expected-failure configs as Apalache.
  The `rbc-authoritative-payload-progress` family covers RBC session metadata
  filtering, complete-chunk root acceptance, chunk-failure fallback, and local
  authoritative payload slot/hash matching.
  The `slot-authoritative-payload` family covers slot-level authoritative
  payload lookup, local-owner status filtering, rejected-owner fallthrough,
  Kura committed-block filtering, and RBC retained-branch filtering. Their TLC
  cross-checks independently exhaust the same nineteen and twenty-one
  expected-failure configs as Apalache.
  The `recovery-status-counters` family covers missing-block fetch counters,
  stale recovery suppression counters, snapshot projection, and reset
  accounting. Its TLC cross-check independently exhausts the same eighteen
  expected-failure configs as Apalache.
  The `block-sync-vote-placeholder` family covers exact-frontier commit-vote
  placeholder recording and vote/sidecar filtering before embedded vote
  handling: valid vote placeholder counting, invalid
  phase/hash/height/view/epoch vote filtering, mixed-vote filtering,
  exact-frontier/known-local/requested-missing gates, commit-QC and checkpoint
  sidecar exclusion, stake-sidecar allowance, vote-subject and empty payload
  marker projection, and no-status/no-clear/no-deferral/no-early-return
  continuation. Its TLC cross-check independently exhausts the same twenty
  expected-failure configs as Apalache.
  The `block-sync-snapshot-hint` family covers known-block commit-roster
  snapshot hint filtering for incoming QC, checkpoint, and stake sidecars:
  unknown/no-local snapshot preservation, matching QC preservation, same-roster
  different-QC preservation with revalidation, different-roster QC rejection,
  checkpoint and stake sidecar filtering, all-hint matching/mismatch handling,
  and the no-status/no-clear/no-deferral/no-early-return continuation contract.
  Its TLC cross-check independently exhausts the same twenty-one
  expected-failure configs as Apalache.
  The `block-sync-snapshot-roster` family covers commit-roster snapshot
  selection, snapshot cache insertion, and fallback roster-source ordering:
  nonempty snapshot gate, journal source projection, snapshot
  roster/QC/checkpoint/stake attachment, snapshot cache-key insertion,
  snapshot preemption over persisted, cached, and fresh sources,
  persisted/cache/fresh fallback ordering, fresh cache insertion policy, and
  sidecar-quarantine propagation. Its TLC cross-check independently exhausts
  the same twenty-two expected-failure configs as Apalache.
  The `block-sync-no-roster` family covers the terminal no-verifiable-roster
  BlockSyncUpdate branch, including known vote-only handling and unknown
  missing-roster defer/request/drop paths: known vote-only vote processing and
  snapshot suppression, known hinted drop cleanup, effective and trusted
  exact-frontier repair deferral, missing-QC request refresh and sidecar
  failover, missing-roster drop status/reason/metrics, `PayloadAvailable`
  cleanup, and `Ok(())` non-continuation. Its TLC cross-check independently
  exhausts the same twenty-five expected-failure configs as Apalache.
  The `block-sync-known-roster` family covers selected-roster known-block
  terminal replay, commit-roster persistence, and cleanup after vote/QC
  bookkeeping: source metrics, vote-roster caching, checkpoint recording,
  commit-roster preparation/persistence, checkpoint/stake projection,
  known-vote processing, incoming/selection/checkpoint QC replay priority,
  redundant replay suppression, work preparation, cached commit-QC cleanup,
  missing-block cleanup, known `Ok(())` return, and unknown-block continuation.
  Its TLC cross-check independently exhausts the same twenty-eight
  expected-failure configs as Apalache.
  The `block-sync-known-selected-roster` family covers selected-roster
  bookkeeping, known-block commit-roster persistence, known QC replay
  precedence/suppression, and known-block request cleanup: source metrics,
  vote-roster caching, checkpoint recording, commit-roster record
  preparation/persistence, checkpoint/stake projection, known-vote processing,
  incoming/selection/checkpoint QC replay priority, redundant replay
  suppression, work preparation, cached commit-QC cleanup, missing-block
  cleanup, known `Ok(())` return, unknown-block continuation, and unknown-path
  no-clear handling. Its TLC cross-check independently exhausts the same
  twenty-nine expected-failure configs as Apalache.
  The `block-sync-selected-signatures` family covers selected-roster signer
  cache reuse, validated-signer insertion, signature-context deferral, roster
  evidence continuation, and invalid-signature drops: cache-hit reuse without
  revalidation, validated-signer caching only with a cache key, signer-set
  projection, missing-parent and gap deferral with effective topology and
  selected-roster context, deferred status/reason recording, payload-only
  recovery forwarding, roster-evidence continuation with empty signers,
  invalid-signature status/metric/reason projection, `Ok(())` returns for
  terminal paths, QC-candidate continuation, and no missing-block cleanup on
  signature drops. Its TLC cross-check independently exhausts the same
  twenty-nine expected-failure configs as Apalache.
  The `block-sync-selected-qc` family covers selected-roster QC source
  precedence, shape filtering, validation recovery, aggregate fallback,
  locked-conflict stripping, usable-QC caching, commit-cert gating, and
  invalid-payload drops: incoming, selection, checkpoint-derived,
  world-derived, and cached source precedence; height, hash, epoch, and
  COMMIT-phase shape filtering; missing-context quarantine and final
  validation drops; replaced-QC metrics; cached-QC recovery and aggregate
  fallback acceptance; hard locked-QC conflict evidence stripping and status
  recording; usable-QC caching and quarantine cleanup; selected commit-cert
  projection; invalid-payload drop gating; `Ok(())` invalid-payload returns;
  and no missing-block cleanup on invalid QC drops. Its TLC cross-check
  independently exhausts the same thirty-three expected-failure configs as
  Apalache.
  The `block-sync-selected-quorum` family covers selected-roster quorum
  admission, sparse exact-frontier recovery, missing-QC request transitions,
  NPoS vote-only deferral, exact body-repair deferral, quorum-missing drops,
  and invalid-QC short-circuiting: QC evidence, commit-cert, block signature
  quorum, checkpoint, explicit requested sparse frontier, and tracked sparse
  frontier admission; zero-signer, commit-vote, non-frontier, and unrequested
  sparse rejection; missing-QC request admission and request marking; NPoS
  vote-only deferral; post-request quorum recomputation; exact body-repair
  deferral; quorum-missing drop status/reason/metric/return handling;
  invalid-QC short-circuit gating by block quorum or checkpoint evidence; and
  no missing-block cleanup on invalid QC drops. Its TLC cross-check
  independently exhausts the same thirty expected-failure configs as Apalache.
  The `block-sync-recovery-mode` family covers stale BlockCreated and
  block-sync recovery-mode helpers: height-at-or-below stale detection, stale
  admission by missing request, retained match, or recovery evidence,
  no-signal rejection, authoritative frontier supersede permission for
  signed-quorum and commit-evidence repair only, stale-without-request bypass
  for requested-payload and commit-evidence repair only, aborted-pending
  revival only for commit-evidence repair with the explicit flag, and observed
  commit-QC epoch projection only from commit-evidence repair. Its TLC
  cross-check independently exhausts the same twenty-five expected-failure
  configs as Apalache.
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
  commit-QC recovery, and QC application readiness: same-height and
  non-extending QC admission from selected or incoming evidence,
  payload-mismatch preserve-flag projection, authoritative frontier supersede
  from usable incoming QC, commit cert, checkpoint, or non-conflicting
  signature quorum, commit-evidence versus signed-quorum versus payload-only
  recovery-mode selection, observed epoch projection, aborted-revival gating,
  signed-quorum commit-QC repair activation and side effects, sparse
  next-height known-block commit-QC recovery, readiness for QC application,
  payload-unapplied drop recording, and commit-vote/QC application calls. Its
  TLC cross-check independently exhausts the same thirty-five expected-failure
  configs as Apalache.
  The `block-sync-selected-qc-prefilter` family covers post-apply QC topology
  recovery, shape ignores, same-height locked-QC drops, stale locked-QC drops,
  missing locked-payload quarantine, non-extending drops, and tally admission:
  empty-topology recovery without tallying, hash/height/epoch/phase shape
  mismatch ignores and `Ok(())` returns, same-height locked-QC drop
  metrics/status without tallying, recoverable same-height continuation, stale
  locked-QC drops and status recording, non-extending missing locked-payload
  quarantine and status recording, non-extending locked drops with or without
  lock context, allowed non-extending retention, and extending/no-lock tally
  plus precommit processing admission. Its TLC cross-check independently
  exhausts the same twenty expected-failure configs as Apalache.
  The `block-sync-selected-qc-process` family covers post-prefilter QC tally
  reuse/validation, precommit processing arguments, commit-QC cache/record side
  effects, known-block commit application, unknown-pending epoch observation,
  runtime DA cleanup, and unknown-block QC cache handoff: cached tally reuse
  versus fresh tally validation, no-QC no-tally handling, tally validation-error
  recording, precommit signer recording, validated-tally notes,
  `block_known_for_commit` and `allow_nonextending_qc` argument forwarding,
  process rejection side effects, commit-QC record/cache insertion, known-block
  commit application, runtime DA cleanup gating, commit-pipeline requests,
  unknown pending-epoch observation, creation-error propagation, and
  unknown-block QC cache handoff. Its TLC cross-check independently exhausts
  the same twenty-three expected-failure configs as Apalache.
  The `block-sync-selected-qc-cache` family covers unknown-block QC cache
  prefiltering, fresh signer tally validation, false `block_known` precommit
  processing, non-extending lock realignment, quarantine removal, commit-QC
  cache insertion, and transient/final validation-error handling: empty-topology
  recovery, shape mismatch ignores, same-height/stale locked-QC drops,
  non-extending missing locked-payload quarantine, non-extending drop/retain
  decisions, fresh signer tally validation, transient missing-context
  quarantine versus final validation drops, precommit signer and
  validated-tally recording, `process_precommit_qc` calls with `block_known =
  false` and forwarded non-extending permission, process-reject side-effect
  suppression, non-extending locked-QC realignment and vote pruning, highest-QC
  preservation while the payload is missing, quarantine removal, and commit-QC
  record/cache insertion. Its TLC cross-check independently exhausts the same
  thirty-three expected-failure configs as Apalache.
  The `block-sync-stale-view` family covers stale BlockSyncUpdate drops,
  requested/known/evidence-bearing stale admission, and stale-drop status
  recording: fresh views continue, stale unrequested unknown updates without
  commit evidence are recorded as `BlockSyncUpdate`/`Dropped`/`StaleView`,
  return `Ok(())`, and do not clear missing-block requests, while requested,
  locally known, incoming-QC, checkpoint, and commit-vote evidence cases
  continue to later block-sync gates. Its TLC cross-check independently
  exhausts the same fifteen expected-failure configs as Apalache.
  The `block-sync-commit-conflict` family covers committed-height
  BlockSyncUpdate conflict drops, QC validation inputs, and invalid-QC
  evidence emission on finality conflict: zero-height, absent-committed, and
  same-hash updates fall through; conflicting no-QC and invalid-QC updates
  drop, clear the missing request, record `CommitConflict`, and emit no
  evidence; conflicting valid-QC updates validate with the block subject,
  epoch, consensus mode/tag, stake snapshot, and genesis-stub allowance, emit
  `InvalidQc` evidence carrying the incoming certificate and
  `commit_conflict_finality` reason, then drop, clear, and return `Ok(())`;
  and evidence broadcast errors do not re-admit the conflict. Its TLC
  cross-check independently exhausts the same twenty-four expected-failure
  configs as Apalache.
  The `block-sync-warning-throttle` family covers per-kind/hash/height/view
  warning cooldowns, suppressed-count replay, burst caps, zero-cap and
  zero-cooldown behavior, GC boundaries, and clear/reset semantics: first
  warning insertion, strict within-cooldown suppression, cooldown-boundary
  emission, suppressed-count replay/reset, per-kind/hash/height/view key
  separation, burst-cap suppression for new and existing entries, burst-window
  reset/preservation, zero burst-cap and zero-cooldown behavior, GC
  boundary/expiry behavior, zero-cooldown GC floor, and `clear()` entry/burst
  reset semantics. Its TLC cross-check independently exhausts the same
  twenty-one expected-failure configs as Apalache.
  The `fetch-response-deferral` family covers canonical committed fetch/body
  response deferral: next-height and historical responses do not defer,
  same-height hash mismatches and unknown committed hashes do not defer,
  canonical `BlockCreated` and bare `BlockSyncUpdate` responses defer,
  commit-QC and validator-checkpoint sidecars prevent deferral, and
  non-payload messages do not defer. Its TLC cross-check independently
  exhausts the same ten expected-failure configs as Apalache.
  The `fetch-block-body-handle` family covers exact body request handling,
  canonical committed deferral, requester stashing, and dedup release:
  canonical committed exact matches defer instead of dispatching, exact local
  created/proof responses dispatch through the plain-fallback helper, identity
  mismatches and absent local blocks are not served, canonical deferral stashes
  pending requesters, dispatch does not stash, pending-window and frontier
  stashes are separated, frontier matches beat the broader pending window,
  dispatch removes the requester while deferral keeps it, non-dispatch paths
  record deferred handling, every path releases the dedup key exactly once, and
  dispatch-helper use matches actual dispatch. Its TLC cross-check
  independently exhausts the same twenty-two expected-failure configs as
  Apalache.
  The `background-frame-cap` family covers background consensus frame-cap
  trimming, downgrade, direct fallback, and drop decisions: fetch/control
  messages use the control cap while payloads use the payload cap, under-cap
  messages are preserved, oversized payloads and updates are dropped, commit
  votes are trimmed first, permissioned updates may drop stale stake but NPoS
  keeps stake, commit-QC sidecars are tried before validator checkpoints,
  oversized `BlockBodyResponse` updates trim embedded updates before
  downgrading to `BlockCreated`, direct `BlockSyncUpdate` fallback is preferred
  only when it fits, and changed/dropped status follows the prepared result.
  Its TLC cross-check independently exhausts the same nineteen
  expected-failure configs as Apalache.
  The `background-dispatch` family covers background request blocking
  eligibility, full-queue fallback, unavailable-worker drop status, kind
  labels, and request reconstruction: all request variants remain
  blocking-eligible for caller-side inline fallback, ready queues enqueue and
  do not return a request, full queues record overflow and return without drop
  status, unavailable workers record drop status and return, telemetry kind
  labels are stable, and returned requests preserve their original kind. Its
  TLC cross-check independently exhausts the same nine expected-failure configs
  as Apalache.
  The `background-bypass` family covers scheduler bypass decisions for
  prepared post/broadcast payloads, forced-queue scheduling, disabled-worker
  inline dispatch, and non-payload control/native requests: accepted post
  payloads bypass through fallback, accepted broadcast payloads bypass only for
  the broadcast-safe message set, broadcast QC/vote/fetch request messages stay
  queued, control-flow and native AMX requests stay queued, disabled workers
  dispatch accepted requests inline, and forced-queue scheduling never bypasses
  even when the message would otherwise be eligible. Its TLC cross-check
  independently exhausts the same thirteen expected-failure configs as
  Apalache.
  The `background-fallback` family covers fallback request-to-network mapping,
  peer preservation, payload class preservation, and block/control/native
  priority assignment: post requests remain P2P posts, broadcast requests
  remain broadcasts, block/control/native payload classes are preserved,
  block-message priority is projected from the embedded priority, control-flow
  and native AMX fallbacks stay high priority, posts preserve peer targets, and
  broadcasts omit peers. Its TLC cross-check independently exhausts the same
  thirteen expected-failure configs as Apalache.
  The `fetch-pending-response-send` family covers single fetch response send
  policy, bypass selection, fallback payloads, and direct-QC companion
  ordering: hintless `BlockSyncUpdate` responses require caller allowance and
  requester roster proof, invalid hintless responses downgrade to
  `BlockCreated`, cached QC sidecars apply before trimming, bypass is computed
  from force/consensus/highest/created/hintless policy and is preserved across
  fallback, update trimming falls back to `BlockCreated` or drops oversized
  payloads, direct commit-QC companions are emitted when available even if the
  payload drops, companions require QC material, and companions are sent before
  the final payload. Its TLC cross-check independently exhausts the same
  twenty-six expected-failure configs as Apalache.
  The `fetch-pending-responses-batch` family covers batch requester splitting,
  per-peer hintless downgrade decisions, exact body companions, and rostered
  update fanout ordering: empty peer sets return without payload building,
  commit-QC-only requesters dispatch first and retain their restash flag on
  failure, commit-QC-only peers are excluded from payload fanout, consensus
  payload requesters receive exact-body companions, hintless policy is decided
  per requester with allowance and roster-proof arguments preserved,
  roster-hinted updates send a fitting `BlockCreated` companion before the main
  update, created bypass requires the hintless-bypass allowance, force bypass
  and consensus priority are forwarded, and non-hintless payload sends keep the
  allow-hintless argument. Its TLC cross-check independently exhausts the same
  twenty-six expected-failure configs as Apalache.
  The `pending-response-flush` family covers pending fetch/body readiness
  wrappers, canonical deferral, queue removal, and exact body response fanout:
  absent pending keys do not build payloads or return ready, canonical deferral
  keeps pending entries, ready fetch entries build payloads, remove the pending
  entry, call batch fanout for exactly the recorded requesters with
  force/highest/hintless bypass disabled, ready body entries build exact
  `BlockBodyResponse` values bound to the block hash/height/view/payload,
  remove pending body entries, dispatch only recorded requesters, and use the
  plain-fallback helper. Its TLC cross-check independently exhausts the same
  thirty expected-failure configs as Apalache.
  The `deferred-block-sync-helper` family covers deferred BlockSyncUpdate
  reason priority, sidecar merge, evidence detection, and cap eviction order:
  validation blocks only for active conflicting work, deferral reasons
  prioritize commit work before validation and pending processing while the
  certified exact-frontier bypass suppresses all reasons, merge fills missing
  commit/checkpoint/stake sidecars without overwriting existing sidecars,
  sender replacement happens only when the incoming sender is present, commit
  evidence detects commit QC, validator checkpoint, or stake snapshot, cap zero
  is unlimited, cap enforcement evicts until within limit while retaining
  evidence and newer view/height/hash, and eviction metrics increment only when
  entries are removed. Its TLC cross-check independently exhausts the same
  twenty-nine expected-failure configs as Apalache.
  The `deferred-block-sync-cache` family covers deferred BlockSyncUpdate
  cache/defer integration, commit-vote stripping, full-key matching,
  post-cache cap enforcement, and deferred outcome recording: incoming commit
  votes are stripped before caching, matching full `(height, view, block_hash)`
  keys merge while distinct heights/views/hashes insert, missing commit-QC
  sidecars are filled without overwriting existing commit-QC sidecars, sender
  replacement follows the merge helper's Some-only rule, cap enforcement runs
  after insert and merge paths, `defer_block_sync_update` invokes the cache
  path first, forwards the deferral reason, and records
  `BlockSyncUpdate`/`Deferred`/`CommitPipelineActive` after caching. Its TLC
  cross-check independently exhausts the same twenty expected-failure configs
  as Apalache.
  The `deferred-block-sync-replay` family covers replay idle gating, ordered
  key selection, remove-before-handle, forwarding, and handler-error behavior:
  empty queues do no work, commit or validation inflight work preserves the
  deferred queue, replay selects the first ordered key, removes the selected
  entry before calling the handler, forwards the stored update and sender
  unchanged, logs handler errors while still reporting a successful replay,
  preserves later ordered entries, and treats remove-missing races as no-handle
  false returns. Its TLC cross-check independently exhausts the same sixteen
  expected-failure configs as Apalache.
  The `block-sync-future-window` family covers future BlockSyncUpdate
  drop/window behavior: known local blocks bypass all drop gates, requested
  missing-block recovery is bounded by the committed-height margin before
  parent availability can short-circuit, unresolved lower missing heights drop
  far-ahead sparse updates before known parents admit connected chains, generic
  height/view windows preserve disabled and boundary cases, stale or absent
  phase-view baselines do not drop updates, and saturated height arithmetic
  remains inclusive. Its TLC cross-check independently exhausts the same
  seventeen expected-failure configs as Apalache.
  The `block-body-repair` family covers the RBC exact body repair admission
  helper: DA/RBC must be enabled, the response height must match the current
  frontier height, the RBC session must exist with matching metadata,
  authoritative payloads already known locally suppress repair, expected
  payload hashes must be present, `BlockCreated` and `BlockSyncUpdate` bodies
  are both accepted when exact, and block hash/height/view/payload hash
  identity mismatches are rejected. Its TLC cross-check independently exhausts
  the same twelve expected-failure configs as Apalache.
  The `block-body-request-stash` family covers the exact body requester
  stash-window helper: configured missing-request margins are floored at one,
  next-height and within-margin requests are stashed, upper bounds are
  inclusive, beyond-margin, same-height, and stale-height requests are
  rejected, zero committed-height next slots are allowed, and saturating
  lower/upper boundaries preserve Rust-style inclusive range semantics. Its TLC
  cross-check independently exhausts the same eleven expected-failure configs
  as Apalache.
  The `same-height-block-body-repair` family covers same-height exact body
  repair admission: the response must target the current frontier height, a
  matching pending missing-block request, deferred missing-payload commit QC,
  or active missing commit-QC repair round may authorize repair, pending and
  deferred sources must be commit-phase records bound to the exact block hash,
  height, and view, non-actionable dependencies are rejected, and no-source
  responses remain inadmissible. Its TLC cross-check independently exhausts
  the same fifteen expected-failure configs as Apalache.
  The `block-body-repair-epoch` family covers observed commit-QC epoch source
  selection for body repair: exact cached commit QCs have highest priority,
  matching deferred missing-payload commit QCs beat pending blocks, pending
  sources require an observed commit QC with an epoch, deferred sources must be
  commit-phase records bound to the response block hash, height, and view,
  missing or mismatched sources return no epoch, and no-source cases remain
  empty. Its TLC cross-check independently exhausts the same thirteen
  expected-failure configs as Apalache.
  The `direct-commit-qc-for-block` family covers direct commit-QC source
  selection and local vote-formation quorum gating: exact cached commit QCs win
  immediately, world-derived commit QCs win before local formation, a non-empty
  exact round roster blocks fallback topology, fallback topology is used only
  when no primary roster is available, quorum floors use `max(1)`, local
  formation runs only with enough votes, uses commit phase and the target block
  subject, and a formed QC is returned only after cache readback for the same
  block. Its TLC cross-check independently exhausts the same sixteen
  expected-failure configs as Apalache.
  The `materialize-qc` family covers QC materialization, Kura recovery,
  fail-closed quorum/signature gates, and cache insertion: existing cached QCs
  win immediately, empty rosters recover only from Kura, non-empty rosters try
  local vote formation before Kura recovery and local signer aggregation, NPoS
  requires a stake roster and stake quorum, commit-root filtering and empty
  vote sets fail closed, permissioned and NPoS under-quorum cases are rejected,
  aggregation and canonical mapping failures return no QC, prepare-phase
  quorums remain admissible, and recovered/rebuilt QCs are cached. Its TLC
  cross-check independently exhausts the same seventeen expected-failure
  configs as Apalache.
  The `block-body-direct-commit-qc` family covers direct commit-QC extraction
  from BlockBodyResponse payloads: body identity must match response block
  hash, height, and view, `BlockSyncUpdate` bodies prefer embedded commit QCs
  before validator-checkpoint-derived QCs and locally available direct QCs,
  `BlockCreated` bodies can only use local direct QCs, no-source responses
  return no QC, and identity mismatches are rejected for both body kinds. Its
  TLC cross-check independently exhausts the same fourteen expected-failure
  configs as Apalache.
  The `block-body-detached-commit-qc` family covers detached commit-QC
  handling and obsolete repair clearing: responses without commit QCs do not
  call the QC handler or clear requests, already cached QCs clear obsolete
  missing commit-QC requests without re-handling, uncached QCs call the handler,
  post-handle clearing is driven by whether the QC is cached afterward, and
  handler errors still clear only when the QC became cached. Its TLC
  cross-check independently exhausts the same ten expected-failure configs as
  Apalache.
  The `block-body-response-dispatch` family covers exact BlockBodyResponse
  fallback and companion dispatch ordering: under-cap `BlockCreated` companions
  are sent before the rich response, oversized companions are skipped,
  `BlockSyncUpdate` bodies get a plain fallback before the rich response, the
  rich response is always sent, direct commit-QC companions are sent after the
  response only when available, and every dispatch uses the bypass/background
  path. Its TLC cross-check independently exhausts the same fourteen
  expected-failure configs as Apalache.
  The `invalid-proposal-evidence` family covers invalid-proposal evidence
  wrapping and building: the wrapper emits `InvalidProposal` evidence while
  preserving the proposal and validation reason, the builder derives proposer
  from the first block signature with a zero fallback, uses the block header
  view, caller epoch, and caller payload hash, carries the QC selected for
  validation evidence, preserves the parent/height relation required by
  downstream validation, and records the validation error string rather than a
  label. Its TLC cross-check independently exhausts the same sixteen
  expected-failure configs as Apalache.
  The `proposal-mismatch` family covers proposal metadata mismatch detection:
  height, view, parent hash, transaction root, state root, and payload hash are
  reported in implementation priority order, missing parent and transaction
  roots default to zero, zero proposal state roots remain compatibility values,
  payload mismatches are still checked after a zero state-root compatibility
  case, and no-mismatch results are allowed only when all compared fields are
  compatible. Its TLC cross-check independently exhausts the same fifteen
  expected-failure configs as Apalache.
  The `proposal-cache` family covers bounded ProposalCache behavior: hint and
  proposal maps enforce their configured limits independently, zero limits
  retain no inserted entries, overflow evicts the lowest key from the
  overflowing map, eviction metrics match real evictions, duplicate hint
  insertion replaces without growing, pop removes only the requested kind,
  observed timestamps are retained only while either a hint or proposal remains
  for the key, and prune removes committed entries while retaining future
  entries. Its TLC cross-check independently exhausts the same twenty-five
  expected-failure configs as Apalache.
  The `proposal-hint` family covers inbound proposal-hint admission: stale
  height/view hints, malformed highest-QC references, cached conflicts,
  committed-edge conflicts, local metadata mismatches, and locked-QC conflicts
  fail closed; missing future highest-QC parents arm exact repair and deferral
  markers without observed/highest-QC side effects, with cross-view hints
  cached only as dependency context; accepted hints update PRF context, cache
  and observe the hint, replay deferred votes, prune observed slots, and update
  highest-QC only for newer references or same-slot Commit promotion, while
  lock-lag catchup keeps metadata but defers the highest-QC mutation. Its TLC
  cross-check independently exhausts the same forty expected-failure configs
  as Apalache.
  The `stale-proposal-hint-repair` family covers the DA stale-view proposal
  hint exception for exact-frontier repair: an exact committed-QC hint seeds
  repair only when DA is enabled, the hint targets the active height, the local
  view is exactly one ahead, and the hint highest-QC identity matches the
  latest committed QC by height, view, subject hash, and epoch. Every denied
  stale hint remains a stale-view drop and does not cache metadata, mark the
  slot observed, or mutate highest-QC state. Its TLC cross-check independently
  exhausts the same fourteen expected-failure configs as Apalache.
  The `stale-rbc-hint-repair` family covers the stale RBC proposal-hint bridge
  into exact-frontier body repair: a stale RBC chunk with no session may
  continue only when DA is enabled, the message kind is allowed to seed repair,
  the height is the exact frontier, and the cached proposal hint at the same
  height/view names the same block hash. Every rejected stale RBC message still
  drops as stale and does not stash a chunk or arm exact frontier repair. Its
  TLC cross-check independently exhausts the same eleven expected-failure
  configs as Apalache.
  The `proposal-admission` family covers proposal metadata admission: stale
  height/view proposals, proposal epoch mismatches, highest-QC height/epoch
  mismatches, parent hash mismatches, stored-parent and committed-edge
  conflicts, missing committed/future highest-QC dependencies, local metadata
  mismatches, and locked-QC conflicts fail closed. Missing future highest-QC
  parents arm exact repair and a deferral marker without caching or observing
  proposal metadata; accepted proposals update PRF and leader context, cache
  and observe the proposal, sample proposal phase, replay deferred votes, and
  prune old observed slots. Highest-QC updates remain limited to newer
  references or same-slot Commit promotion, lock-lag catchup keeps metadata but
  defers the highest-QC mutation, and proposal metadata alone never wakes the
  commit pipeline or records payload-phase progress. Its TLC cross-check
  independently exhausts the same forty-three expected-failure configs as
  Apalache.
  The `block-created-admission` family covers direct `BlockCreated` payload
  admission: valid payloads update pending state, phase sampling,
  commit-pipeline wakeup, authoritative ownership, passive retention, and
  inline proposal context exactly when the admission branch requires it;
  duplicates, stale/local-removed payloads, lock rejections, missing highest-QC
  hints, proposal mismatches, RBC payload mismatches, and empty payload
  rejections fail closed while preserving their repair, evidence, cleanup,
  missing-request, and deferral side effects. Its TLC cross-check independently
  exhausts the same fifty-four expected-failure configs as Apalache.
  The `missing-request-clear` family covers missing-block request clearing:
  locked-QC rejections clear requests only when committed history, known
  ancestry, durable locks, or local ancestry disprove the branch, while
  same-hash, unresolved-parentless, uncommitted lock-conflict, and clean future
  requests remain live; stale `BlockCreated` drops clear only below the
  committed tip, when payload is locally available, or when committed history
  disproves the hash, while committed-height payload-repair requests stay
  alive. Its TLC cross-check independently exhausts the same fourteen
  expected-failure configs as Apalache.
  The `missing-block-clear` family covers the missing-block clear reason
  helper: payload-available clears are allowed only when the payload is already
  known locally, while obsolete clears are allowed regardless of local payload
  availability. Its TLC cross-check independently exhausts the same seven
  expected-failure configs as Apalache.
  The `proposal-budget` family covers proposal-side budget/cap arithmetic:
  queue caps are floored and trigger at the configured block/RBC depth, DA
  payload budget selects the minimum of payload cap, RBC chunk budget, and
  pending byte/chunk budget with zero chunk caps floored, transaction limits
  respect config and parameter caps while keeping empty queues at one
  transaction, fast-finality transaction and gas caps apply only below the
  configured threshold, and stale-window scaling handles zero-transaction,
  one-batch, grace, and max-cap cases. Its TLC cross-check independently
  exhausts the same twenty-one expected-failure configs as Apalache.
  The `non-rbc-payload-budget` family covers deterministic non-RBC proposal
  payload frame-cap derivation: fixed non-RBC frame headroom is subtracted with
  saturating arithmetic, absent block-payload caps use the adjusted frame cap,
  explicit configured caps clamp to the lower of the configured cap and
  adjusted frame cap, and zero/small frame caps cannot leak configured payload
  bytes. Its TLC cross-check independently exhausts the same nine
  expected-failure configs as Apalache.
  The `proposal-backpressure` family covers proposal backpressure
  classification: queue saturation and consensus worker-queue pressure defer
  proposals but remain pacing-only so queued proposal work can still run after
  the pacemaker deadline, while active pending blocks, RBC backlog, and relay
  pressure are hard stops that suppress pacing-only classification and block
  queued proposal work. Its TLC cross-check independently exhausts the same
  nineteen expected-failure configs as Apalache.
  The `proposal-defer-warning` family covers proposal-defer warning
  throttling: first observations insert and emit, within-cooldown repeats are
  suppressed with a strict `< cooldown` check, cooldown-boundary emissions
  replay suppressed counts, warning keys separate kind/hash/height/view except
  for empty-topology view normalization, zero-cooldown bypasses suppression,
  and GC keeps boundary entries while pruning expired entries. Its TLC
  cross-check independently exhausts the same fifteen expected-failure configs
  as Apalache.
  The `proposal-batch` family covers proposal batch trim/canonicalization
  helpers: tail trimming removes only excess transactions while preserving
  singleton and zero-size floor behavior, returned removed transactions keep
  route/plan/size companions aligned, canonicalization leaves
  empty/single/already-sorted batches stable, sorts by key deterministically,
  preserves duplicate-key stability, and keeps route/plan/size companions
  aligned without changing batch length. Its TLC cross-check independently
  exhausts the same nineteen expected-failure configs as Apalache.
  The `lane-interleave` family covers lane interleaving of routing decisions:
  empty, single-item, and single-lane inputs fall back to original index order,
  multi-lane inputs traverse sorted lane IDs, each lane preserves intra-lane
  order, skewed lanes drain round-robin without dropping the final round, and
  slot height/view offsets rotate and wrap the starting lane deterministically.
  Its TLC cross-check independently exhausts the same eleven expected-failure
  configs as Apalache.
  The `commitment-snapshot-builder` family covers lane/dataspace commitment
  snapshot construction: block height/hash, lane and dataspace IDs,
  transaction/chunk counts, RBC byte totals, TEU totals, and BTreeMap-derived
  sorted order are preserved independently when aggregate maps are projected
  into status snapshots. Its TLC cross-check independently exhausts the same
  six expected-failure configs as Apalache.
  The `collector-selection` family covers collector fanout and selection
  helpers: commit-quorum floors and non-leader caps bound the requested
  fanout, default selection starts at the proxy tail without wrapping,
  fallback selection wraps without including the leader or duplicates, PRF
  selection is distinct, in range, and leader-free, and the deterministic
  wrapper chooses fallback without a seed and PRF with a seed. Its TLC
  cross-check independently exhausts the same eighteen expected-failure
  configs as Apalache.
  The `topology-mutation` family covers ordered topology mutation helpers:
  rotations use modulo without resetting view, `nth_rotation` rejects rewinds
  while returning the forward delta, new topology construction deduplicates
  without sorting away caller order, peer-list updates preserve surviving old
  order then append new peers without duplicates, `block_committed` resets the
  view while applying the same order rules, and canonicalization
  sorts/deduplicates without resetting view. Its TLC cross-check independently
  exhausts the same twenty-two expected-failure configs as Apalache.
  The `highest-qc-dependency-deferral` family covers force/exact highest-QC
  repair selection, lock-lag range-pull reanchor, marker pruning, and deferred
  slot non-admission.
  The `committed-edge-conflict` family covers committed-edge conflicting
  highest-QC suppression, canonical state preservation, stale-frontier cleanup,
  owner gating, and bounded recovery reanchors. Its TLC cross-check
  independently exhausts the same twenty-three expected-failure configs as
  Apalache.
  The `lock-rejected-sink` family covers deterministic lock-rejected branch
  sink note/update, activity, fetch/parent suppression, replay drop, and purge
  cleanup behavior. Its TLC cross-check independently exhausts the same
  twenty-five expected-failure configs as Apalache.
  The `active-lock-reject-recovery` family covers active-height lock-rejected
  branch recovery routing through missing-QC frontier recovery and view-change
  escalation. Its TLC cross-check independently exhausts the same twenty-one
  expected-failure configs as Apalache.
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
