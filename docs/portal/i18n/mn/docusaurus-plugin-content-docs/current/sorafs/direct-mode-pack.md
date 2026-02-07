---
id: direct-mode-pack
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Direct-Mode Fallback Pack (SNNet-5a)
sidebar_label: Direct-Mode Fallback Pack
description: Required configuration, compliance checks, and rollout steps when operating SoraFS in direct Torii/QUIC mode during the SNNet-5a transition.
---

:::note Canonical Source
:::

SoraNet circuits remain the default transport for SoraFS, but roadmap item **SNNet-5a** requires a regulated fallback so operators can keep deterministic read-access while the anonymity rollout completes. This pack captures the CLI / SDK knobs, configuration profiles, compliance tests, and deployment checklist needed to run SoraFS in direct Torii/QUIC mode without touching the privacy transports.

The fallback applies to staging and regulated production environments until SNNet-5 through SNNet-9 clear their readiness gates. Keep the artefacts below alongside the usual SoraFS deployment collateral so operators can swap between anonymous and direct modes on demand.

## 1. CLI & SDK Flags

- `sorafs_cli fetch --transport-policy=direct-only …` disables relay scheduling and enforces Torii/QUIC transports. CLI help now lists `direct-only` as an accepted value.
- SDKs must set `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` whenever they expose a “direct mode” toggle. The generated bindings in `iroha::ClientOptions` and `iroha_android` forward the same enum.
- Gateway harnesses (`sorafs_fetch`, Python bindings) can parse the direct-only toggle via the shared Norito JSON helpers so automation receives identical behaviour.

Document the flag in partner-facing runbooks and wire feature toggles through `iroha_config` rather than environment variables.

## 2. Gateway Policy Profiles

Use Norito JSON to persist deterministic orchestrator configuration. The example profile in `docs/examples/sorafs_direct_mode_policy.json` encodes:

- `transport_policy: "direct_only"` — reject providers that only advertise SoraNet relay transports.
- `max_providers: 2` — cap direct peers to the most reliable Torii/QUIC endpoints. Adjust based on regional compliance allowances.
- `telemetry_region: "regulated-eu"` — label emitted metrics so telemetry dashboards and audits distinguish fallback runs.
- Conservative retry budgets (`retry_budget: 2`, `provider_failure_threshold: 3`) to avoid masking misconfigured gateways.

Load the JSON through `sorafs_cli fetch --config` (automation) or the SDK bindings (`config_from_json`) before exposing the policy to operators. Persist the scoreboard output (`persist_path`) for audit trails.

Gateway-side enforcement knobs are captured in `docs/examples/sorafs_gateway_direct_mode.toml`. The template mirrors the output from `iroha app sorafs gateway direct-mode enable`, disabling envelope/admission checks, wiring rate-limit defaults, and populating the `direct_mode` table with plan-derived hostnames and manifest digests. Replace the placeholder values with your rollout plan before committing the snippet to configuration management.

## 3. Compliance Test Suite

Direct-mode readiness now includes coverage in both the orchestrator and CLI crates:

- `direct_only_policy_rejects_soranet_only_providers` guarantees that `TransportPolicy::DirectOnly` fails fast when every candidate advert only supports SoraNet relays.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` ensures Torii/QUIC transports are used when present and that SoraNet relays are excluded from the session.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` parses `docs/examples/sorafs_direct_mode_policy.json` to ensure documentation stays aligned with the helper utilities.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` exercises `sorafs_cli fetch --transport-policy=direct-only` against a mocked Torii gateway, providing a smoke test for regulated environments that pin direct transports.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` wraps the same command with the policy JSON and scoreboard persistence for rollout automation.

Run the focused suite before publishing updates:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

If workspace compilation fails because of upstream changes, record the blocking error in `status.md` and rerun once the dependency catches up.

## 4. Automated Smoke Runs

CLI coverage alone does not surface environment-specific regressions (e.g., gateway policy drift or manifest mismatches). A dedicated smoke helper lives in `scripts/sorafs_direct_mode_smoke.sh` and wraps `sorafs_cli fetch` with the direct-mode orchestrator policy, scoreboard persistence, and summary capture.

Example usage:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- The script respects both CLI flags and key=value config files (see `docs/examples/sorafs_direct_mode_smoke.conf`). Populate the manifest digest and provider advert entries with production values before running.
- `--policy` defaults to `docs/examples/sorafs_direct_mode_policy.json`, but any orchestrator JSON produced by `sorafs_orchestrator::bindings::config_to_json` can be supplied. The CLI accepts the policy via `--orchestrator-config=PATH`, enabling reproducible runs without hand-tuning flags.
- When `sorafs_cli` is not on `PATH` the helper builds it from the
  `sorafs_orchestrator` crate (release profile) so smoke runs exercise the
  shipping direct-mode plumbing.
- Outputs:
  - Assembled payload (`--output`, defaults to `artifacts/sorafs_direct_mode/payload.bin`).
  - Fetch summary (`--summary`, defaults alongside the payload) containing the telemetry region and provider reports used for rollout evidence.
  - Scoreboard snapshot persisted to the path declared in the policy JSON (e.g., `fetch_state/direct_mode_scoreboard.json`). Archive this alongside the summary in change tickets.
- Adoption gate automation: once the fetch completes the helper invokes `cargo xtask sorafs-adoption-check` using the persisted scoreboard and summary paths. The required quorum defaults to the number of providers supplied on the command line; override it with `--min-providers=<n>` when you need a larger sample. Adoption reports are written next to the summary (`--adoption-report=<path>` can set a custom location) and the helper passes `--require-direct-only` by default (matching the fallback) and `--require-telemetry` whenever you supply the matching CLI flag. Use `XTASK_SORAFS_ADOPTION_FLAGS` to forward additional xtask arguments (for example `--allow-single-source` during an approved downgrade so the gate both tolerates and enforces the fallback). Only skip the adoption gate with `--skip-adoption-check` when running local diagnostics; the roadmap requires every regulated direct-mode run to include the adoption report bundle.

## 5. Rollout Checklist

1. **Configuration freeze:** Store the direct-mode JSON profile in your `iroha_config` repository and record the hash in your change ticket.
2. **Gateway audit:** Confirm Torii endpoints enforce TLS, capability TLVs, and audit logging prior to flipping direct mode. Publish the gateway policy profile to operators.
3. **Compliance sign-off:** Share the updated playbook with compliance / regulatory reviewers and capture approvals for running outside the anonymity overlay.
4. **Dry run:** Execute the compliance test suite plus a staging fetch against known-good Torii providers. Archive scoreboard outputs and CLI summaries.
5. **Production cutover:** Announce the change window, flip `transport_policy` to `direct_only` (if you had opted into `soranet-first`), and monitor the direct-mode dashboards (`sorafs_fetch` latency, provider failure counters). Document the rollback plan so you can return to SoraNet-first once SNNet-4/5/5a/5b/6a/7/8/12/13 graduate in `roadmap.md:532`.
6. **Post-change review:** Attach scoreboard snapshots, fetch summaries, and monitoring results to the change ticket. Update `status.md` with the effective date and any anomalies.

Keep the checklist alongside the `sorafs_node_ops` runbook so operators can rehearse the workflow before a live switchover. When SNNet-5 graduates to GA, retire the fallback after confirming parity in production telemetry.

## 6. Evidence & Adoption Gate Requirements

Direct-mode captures still need to satisfy the SF-6c adoption gate. Bundle the
scoreboard, summary, manifest envelope, and adoption report for every run so
`cargo xtask sorafs-adoption-check` can validate the fallback posture. Missing
fields force the gate to fail, so record the expected metadata in change
tickets.

- **Transport metadata:** `scoreboard.json` must declare
  `transport_policy="direct_only"` (and flip `transport_policy_override=true`
  when you forced the downgrade). Keep the paired anonymity policy fields
  populated even when they inherit defaults so reviewers can see whether you
  deviated from the staged anonymity plan.
- **Provider counters:** Gateway-only sessions must persist `provider_count=0`
  and populate `gateway_provider_count=<n>` with the number of Torii providers
  used. Avoid hand-editing the JSON—the CLI/SDK already derives the counts and
  the adoption gate rejects captures that omit the split.
- **Manifest evidence:** When Torii gateways participate, pass the signed
  `--gateway-manifest-envelope <path>` (or SDK equivalent) so
  `gateway_manifest_provided` plus the `gateway_manifest_id`/`gateway_manifest_cid`
  are recorded in `scoreboard.json`. Ensure `summary.json` carries the matching
  `manifest_id`/`manifest_cid`; the adoption check fails if either file is
  missing the pair.
- **Telemetry expectations:** When telemetry accompanies the capture, run the
  gate with `--require-telemetry` so the adoption report proves the metrics were
  emitted. Air-gapped rehearsals can omit the flag, but CI and change tickets
  should document the absence.

Example:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

Attach `adoption_report.json` alongside the scoreboard, summary, manifest
envelope, and smoke log bundle. These artefacts mirror what the CI adoption job
(`ci/check_sorafs_orchestrator_adoption.sh`) enforces and keep direct-mode
downgrades auditable.
