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
  causality gate, pending-RBC stash gate, RBC signing-preimage gate, classic Vote/VRF
  signing-preimage gate, classic Vote/QC signature-verification gate, VRF
  commit/reveal admission gate, classic inbound vote-admission gate,
  proposal-hint admission gate, proposal metadata admission gate, QC
  signer-bitmap admission, direct BlockCreated admission gate,
  commit-root consistency, commit-pipeline recovery gate, commit-pipeline
  scheduling gate, commit-result drain gate, commit-evidence replay gate,
  block-sync recovery gate, direct certified-block fetch gate,
  missing-block fetch planner, missing-block hard-cap recovery gate,
  missing-block hard-cap cleanup gate, missing-block view-change escalation gate, native AMX
  attestation gate, native AMX queue-journal replay gate, native AMX
  routing-plan projection gate, native AMX receipt validation gate, native AMX
  control-plane ingress gate, vNext chain-order helper gate, vNext re-chain
  helper gate, vNext aggregate certificate verification gate, vNext
  signing-preimage gate, vNext control-certificate ingress gate, vNext
  slot-lifecycle gate, vNext validation ownership gate, async
  vote-verification ownership gate, async QC aggregate-verification ownership
  gate, worker-loop drain scheduler gate, actor-gate priority/fairness gate,
  worker-loop budget/adaptive-cap gate, worker ingress routing gate, NPoS VRF
  epoch-seal staging gate, Kura durability commit retry gate, restarted-peer
  replay gate, precommit vote-emission gate, proposal assembly gate, pure
  engine tick gate, pure engine NewView subject projection helper, pure engine
  certificate prefilter dispatch gate, pure engine certificate prefilter
  state-handoff gate, pure engine NewView-QC gate, pure
  engine exact NewView-QC advance gate, pure engine proposal-ingress gate,
  pure engine exact proposal output-field gate,
  pure engine exact proposal state-mutation gate,
  pure engine exact proposal validation-owner gate,
  pure engine prepare-QC gate, pure engine exact Prepare-QC lock/highest-QC
  record gate, pure engine exact Prepare-QC phase-transition gate, pure engine
  commit-QC gate, pure engine exact Commit-QC highest-QC record gate,
  pure engine payload-available Commit-QC exact finality gate,
  pure engine missing-payload Commit-QC pending/fetch gate,
  pure engine Commit-QC validation cleanup gate, pure engine committed-block
  gate, pure engine reconfiguration staging gate, pure engine committed-block
  cleanup gate, pure engine exact payload-availability record gate, pure
  engine payload-availability gate, pure engine
  validation-result gate, pure engine exact validation-owner cleanup gate,
  pure engine exact invalid-validation round/output advance gate,
  view-advance saturation, QC-round
  compatibility helper, proposal-lock helper, QC reference projection helper,
  highest-QC record helper, commit-subject helper, payload-lookup helper,
  prepare-vote cache/output helper, validator-set transition, certified-recovery,
  view-change/lock-safety, validation-callback, certificate-admission,
  highest-QC selection, and frontier-recovery TLA+ models, the small TLC
  frontier cross-check, and expected-failure
  frontier/fork/quorum/RBC/rbc-causality/pending-rbc-stash/rbc-preimage/classic-preimage/classic-signature/vrf-admission/vote-admission/proposal-hint/proposal-admission/block-created-admission/QC-signer/commit-root/commit-pipeline-recovery/commit-pipeline-scheduling/commit-result-drain/commit-job-dispatch/commit-inflight-timeout/post-commit-pacemaker-kick/idle-view-proposal-budget/pacemaker-evaluation/cached-slot-timeout/proposal-parent-resolution/precommit-QC-view-change/commit-evidence-replay/block-sync-recovery/certified-fetch/missing-block-fetch/missing-block-hard-cap/missing-block-hard-cap-cleanup/missing-block-view-change/native-AMX-attestation/native-AMX-journal/native-AMX-routing-plan/native-AMX-receipt/native-AMX-ingress/vnext-chain-order/vnext-rechain/vnext-signature/vnext-signing-preimage/vnext-control-ingress/vnext-slot-lifecycle/vnext-validation/vote-verify-async/qc-verify-async/worker-drain/actor-gate/worker-budget/worker-ingress/npos-vrf/kura-commit/restart-replay/post-commit-cleanup/frontier-gap-realign/precommit/proposal/engine-tick/engine-new-view-subject/engine-handle-dispatch/engine-certificate-dispatch/engine-certificate-prefilter-state/engine-view-advance-saturation/engine-new-view/engine-new-view-highest-qc/engine-new-view-advance/engine-proposal/engine-proposal-output/engine-proposal-state/engine-proposal-validation-owner/engine-proposal-lock/qc-round-compatibility/engine-QC-ref-projection/engine-highest-QC-record/engine-commit-subject/engine-payload-lookup/engine-prepare/engine-prepare-lock-highest/engine-prepare-phase/engine-prepare-vote-cache/engine-commit/engine-commit-highest-qc/engine-commit-available-commit/engine-commit-pending-fetch/engine-commit-validation-cleanup/engine-committed-block/engine-committed-block-record/engine-reconfiguration-staging/engine-committed-block-cleanup/engine-payload-record/engine-payload/engine-validation-result/engine-validation-ownership/engine-validation-invalid-advance/reconfiguration/recovery/view-change/validation/admission/highest-QC
  mutations. For reproducible local setup without Docker, install the pinned
  toolchain with `bash scripts/formal/install_apalache.sh 0.52.2`.
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
