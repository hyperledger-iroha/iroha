---
lang: mn
direction: ltr
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9ce6010594e495116c1397b984000d1ee5d45d064294eca046f8dc762fa73b6
source_last_modified: "2026-01-05T09:28:11.999442+00:00"
translation_last_reviewed: 2026-02-07
---

# Env → Config Migration Tracker

This tracker summarizes production-facing environment-variable toggles surfaced
by `docs/source/agents/env_var_inventory.{json,md}` and the intended migration
path into `iroha_config` (or explicit dev/test-only scoping).


Note: `ci/check_env_config_surface.sh` now fails when new **production** env
shims appear relative to `AGENTS_BASE_REF` unless `ENV_CONFIG_GUARD_ALLOW=1` is
set; document intentional additions here before using the override.

## Completed migrations

- **IVM ABI opt-out** — Removed `IVM_ALLOW_NON_V1_ABI`; the compiler now rejects
  non-v1 ABIs unconditionally with a unit test guarding the error path.
- **IVM debug banner env shim** — Dropped the `IVM_SUPPRESS_BANNER` env opt-out;
  banner suppression remains available via the programmatic setter.
- **IVM cache/sizing** — Threaded cache/prover/GPU sizing through
  `iroha_config` (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`,
  `accel.max_gpus`) and removed runtime env shims. Hosts now call
  `ivm::ivm_cache::configure_limits` and `ivm::zk::set_prover_threads`, tests use
  `CacheLimitsGuard` instead of env overrides.
- **Connect queue root** — Added `connect.queue.root` (default:
  `~/.iroha/connect`) to the client config and threaded it through the CLI and
  JS diagnostics. JS helpers resolve the config (or an explicit `rootDir`) and
  only honour `IROHA_CONNECT_QUEUE_ROOT` in dev/test via `allowEnvOverride`;
  templates document the knob so operators no longer need env overrides.
- **Izanami network opt-in** — Added an explicit `allow_net` CLI/config flag for
  the Izanami chaos tool; runs now require `allow_net=true`/`--allow-net` and
- **IVM banner beep** — Replaced the `IROHA_BEEP` env shim with config-driven
  `ivm.banner.{show,beep}` toggles (default: true/true). Startup banner/beep
  wiring now reads configuration only in production; dev/test builds still honour
  the env override for manual toggles.
- **DA spool override (tests only)** — The `IROHA_DA_SPOOL_DIR` override is now
  fenced behind `cfg(test)` helpers; production code always sources the spool
  path from configuration.
- **Crypto intrinsics** — Replaced `IROHA_DISABLE_SM_INTRINSICS` /
  `IROHA_ENABLE_SM_INTRINSICS` with the config-driven
  `crypto.sm_intrinsics` policy (`auto`/`force-enable`/`force-disable`) and
  removed the `IROHA_SM_OPENSSL_PREVIEW` guard. Hosts apply the policy at
  startup, benches/tests may opt in via `CRYPTO_SM_INTRINSICS`, and the OpenSSL
  preview now respects only the config flag.
  Izanami already requires `--allow-net`/persisted config, and tests now rely on
  that knob rather than ambient env toggles.
- **FastPQ GPU tuning** — Added `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}`
  config knobs (defaults: `None`/`None`/`false`/`false`/`false`) and thread them through CLI parsing
  `FASTPQ_METAL_*` / `FASTPQ_DEBUG_*` shims now behave as dev/test fallbacks and
  are ignored once configuration loads (even when the config leaves them unset); docs/inventory were
  refreshed to flag the migration.【crates/irohad/src/main.rs:2609】【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】
  (`IVM_DECODE_TRACE`, `IVM_DEBUG_WSV`, `IVM_DEBUG_COMPACT`, `IVM_DEBUG_INVALID`,
  `IVM_DEBUG_REGALLOC`, `IVM_DEBUG_METAL_ENUM`, `IVM_DEBUG_METAL_SELFTEST`,
  `IVM_FORCE_METAL_ENUM`, `IVM_FORCE_METAL_SELFTEST_FAIL`, `IVM_FORCE_CUDA_SELFTEST_FAIL`,
  `IVM_DISABLE_METAL`, `IVM_DISABLE_CUDA`) are now gated behind debug/test builds via a shared
  helper so production binaries ignore them while preserving the knobs for local diagnostics. Env
  inventory was regenerated to reflect the dev/test-only scope.
- **FASTPQ fixture updates** — `FASTPQ_UPDATE_FIXTURES` now appears only in FASTPQ integration
  tests; production sources no longer read the env toggle and the inventory reflects the test-only
  scope.
- **Inventory refresh + scope detection** — The env inventory tooling now tags `build.rs` files as
  build scope and tracks `#[cfg(test)]`/integration harness modules so test-only toggles (e.g.,
  `IROHA_TEST_*`, `IROHA_RUN_IGNORED`) and CUDA build flags show up outside the production count.
  Inventory regenerated Dec 07, 2025 (518 refs / 144 vars) to keep the env-config guard diff green.
- **P2P topology env shim release guard** — `IROHA_P2P_TOPOLOGY_UPDATE_MS` now triggers a deterministic
  startup error in release builds (warn-only in debug/test) so production nodes rely solely on
  `network.peer_gossip_period_ms`. The env inventory was regenerated to reflect the guard and the
  updated classifier now scopes `cfg!`-guarded toggles as debug/test.

## High-priority migrations (production paths)

- _None (inventory refreshed with cfg!/debug detection; env-config guard green after P2P shim hardening)._

## Dev/test-only toggles to fence

- Current sweep (Dec 07, 2025): build-only CUDA flags (`IVM_CUDA_*`) are scoped as `build` and the
  harness toggles (`IROHA_TEST_*`, `IROHA_RUN_IGNORED`, `IROHA_SKIP_BIND_CHECKS`) now register as
  `test`/`debug` in the inventory (including `cfg!`-guarded shims). No additional fencing is required;
  keep future additions behind `cfg(test)`/bench-only helpers with TODO markers when shims are temporary.

## Build-time envs (leave as-is)

- Cargo/feature envs (`CARGO_*`, `OUT_DIR`, `DOCS_RS`, `PROFILE`, `CUDA_HOME`,
  `CUDA_PATH`, `JSONSTAGE1_CUDA_ARCH`, `FASTPQ_SKIP_GPU_BUILD`, etc.) remain
  build-script concerns and are out-of-scope for runtime config migration.

## Next actions

1) Run `make check-env-config-surface` after config-surface updates to catch new production env shims
   early and assign subsystem owners/ETAs.  
2) Refresh the inventory (`make check-env-config-surface`) after each sweep so
   the tracker stays aligned with new guardrails and the env-config guard diff stays noise-free.
