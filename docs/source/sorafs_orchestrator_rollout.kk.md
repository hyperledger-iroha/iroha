---
lang: kk
direction: ltr
source: docs/source/sorafs_orchestrator_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 82c145e1eb0220d125e349099d2fa7810db07a134aaff4aceec1c46d73a0cde8
source_last_modified: "2026-01-05T09:28:12.087424+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Multi-Source Orchestrator Rollout
summary: Default-on checklist, overrides, and telemetry evidence required for SF-6c/SF-6d.
---

# 1. Scope & Status

- **Roadmap items:** SF-6c (default-on adoption & release gating), SF-6d (SDK parity fixtures), SF-6e (telemetry burn-in).  
- **Prerequisites:** Orchestrator architecture/spec (`docs/source/sorafs_orchestrator_plan.md`), telemetry plan (`docs/source/sorafs_orchestrator_telemetry_plan.md`), and the multi-source rollout runbook (`docs/source/sorafs/runbooks/multi_source_rollout.md`).  
- **Evidence bundles:** Shared fixtures under `fixtures/sorafs_orchestrator/multi_peer_parity_v1/` plus the SDK smoke harness (`ci/sdk_sorafs_orchestrator.sh`).
- **CI automation:** `.github/workflows/sorafs-orchestrator-sdk.yml` runs the harness nightly (macOS) and on demand, uploading the generated parity artefact bundle for release reviews.

This note focuses on the operator-facing steps required to flip the orchestrator
on by default across binaries/SDKs and to gate releases when a regression forces

# 2. Surface Defaults

| Surface | Default-on behaviour | Overrides / escape hatches | Source |
|---------|---------------------|----------------------------|--------|
| `sorafs_fetch` binary | Multi-source orchestrator enabled whenever at least one `--provider` or `--gateway-provider` is supplied. Scoreboard + telemetry are emitted automatically (`--scoreboard-out` persists the canonical JSON); pass `--telemetry-source-label=<label>` to tag the adoption artefacts with the OTLP stream name instead of the default `file:/path` / `stdin` markers. Fixture playback (e.g., `ci/check_sorafs_orchestrator_adoption.sh`) can pass `--allow-implicit-provider-metadata` to reuse the baked-in capability metadata when adverts are not available. | Use `--transport-policy{,-override}` / `--anonymity-policy{,-override}` to document staged rollbacks—the scoreboard metadata now records the effective + override labels so `cargo xtask sorafs-adoption-check` fails when captures report `transport_policy=direct-only`. Cap `--max-peers=1` or omit multi-provider inputs to simulate single-source behaviour during emergency downgrades. | `crates/sorafs_car/src/bin/sorafs_fetch.rs:883` |
| `sorafs_cli fetch` | Uses the orchestrator by default, loading transport/anonymity stages from `torii.sorafs.orchestrator` settings and now persisting both `scoreboard.json` + `summary.json` under `artifacts/sorafs_orchestrator/latest/` so adoption artefacts exist even when callers omit extra flags. Override the paths via `--scoreboard-out=<path>` / `--json-out=<path>` or supply `--scoreboard-now=<unix_secs>` + `--telemetry-source-label=<otel_stream>` when captures need explicit timestamps/labels for `cargo xtask sorafs-adoption-check --require-telemetry`. The source label is mirrored into `summary.json.telemetry_source`, and `--telemetry-region=<region>` now tags both the scoreboard metadata and the summary (`telemetry_region`) so ops can prove which fleet or rollout wave produced the capture; when regulated fleets require provenance, enforce the label with `cargo xtask sorafs-adoption-check --require-telemetry-region`. | Operators may force a stage via `--transport-policy(-override)` / `--anonymity-policy(-override)` or switch to `direct-only` when the relay fleet is unhealthy. | `crates/iroha_cli/src/commands/sorafs.rs:205` |
| JavaScript SDK | `SorafsGatewayFetchOptions` forwards multi-source config to the native binding; policy overrides (`transportPolicy`, `policyOverride.transportPolicy`) only apply when explicitly set, and scoreboard exports now embed `transport_policy`/`anonymity_policy` labels plus override flags **and** record the gateway manifest id/CID when Torii gateways are involved so adoption evidence matches the CLI. | Set `transportPolicy: "direct-only"` or clamp `maxPeers: 1` before calling `toriiClient.sorafsFetch`. | `javascript/iroha_js/src/sorafs.js:309` |
| Swift SDK | `SorafsGatewayFetchOptions` exposes telemetry region, rollout phase, policy overrides, and retry knobs; defaults map to the orchestrator profile baked into the Norito bridge. | Populate `policyOverride.transportPolicy` / `anonymityPolicy` or provide explicit `maxPeers`/`retryBudget` in app configuration. | `IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift:4` |
| Python SDK | `sorafs_multi_fetch_local` powers the bindings and enforces the same policy snapshot used by Rust; scoreboard artefacts advertise the effective `transport_policy`/`anonymity_policy` labels (and override bits) so `cargo xtask sorafs-adoption-check` can detect SDK downgrades. | Pass a single provider descriptor or inject a `policy_override` that sets `transport_policy="direct-only"` when staging a downgrade. | `python/iroha_python/iroha_python_rs/src/lib.rs:1612` |

# 3. Override & Rollback Semantics

1. **Transport stages:** `soranet-first` is now the default across every surface. Forcing `direct-only` (or pinning `max_peers=1`) requires an explicit override and a matching incident/reference to the SNNet blocker; `soranet-strict` remains reserved for PQ-only pilots (`crates/iroha_cli/src/commands/sorafs.rs:224`). Record overrides in the incident log and remove them within 24 h.
2. **Anonymity stages:** SDKs honour `stage-*` labels (e.g., `anon-guard-pq`). Use the override knobs only when instructed by governance or compliance policy (`crates/sorafs_orchestrator/src/compliance.rs:23`).
3. **Provider limits:** `--max-peers` and `--retry-budget` flags (CLI) or the SDK equivalents give SREs a deterministic way to dampen concurrency during brownouts.
4. **Direct-mode fallback:** The direct-mode pack (`docs/portal/docs/sorafs/direct-mode-pack.md`, `docs/examples/sorafs_direct_mode_smoke.sh`) remains the canonical procedure when regulators require Torii-only transport. Record the reason in the incident ticket before invoking the override.

## 3.1 Downgrade remediation & proxy toggle workflow

SNNet-16E ties downgrade telemetry, the relay `/policy/proxy-toggle` stream, and
local proxy controls together so PQ regressions can be mitigated without waiting
for a human downgrade. Operators must wire the automation before declaring the
multi-source rollout “default-on”.

- **Telemetry feed.** Relays expose downgrade-only events at
  `/policy/proxy-toggle`; the orchestrator polls the endpoint and forwards
  entries to the `downgrade_remediation` hook documented in
  `docs/source/soranet/snnet16_telemetry_plan.md` and
  `dashboards/alerts/soranet_handshake_rules.yml`. Downgrade events increment
  `soranet_proxy_policy_queue_depth`, which Alertmanager watches for stuck
  remediations.
- **Automatic toggles.** Add a `downgrade_remediation` block to the orchestrator
  config (see `docs/source/sorafs/developer/orchestrator.md` §“local proxy”)
  specifying `threshold`, `window_secs`, `target_mode`, `resume_mode`, and the
  relay modes that should trigger the hook. When the configured downgrade rate
  exceeds the threshold, the orchestrator switches the embedded QUIC proxy to
  `target_mode` (usually `metadata-only`) until the cooldown elapses. Capture
  the resulting manifest/summary in the same runbook bucket as the
  scoreboard/summary pair so burn-in reviews can trace every automatic toggle.
- **Manual overrides.** When governance orders an immediate downgrade, run
  `sorafs_cli proxy set-mode` with the active orchestrator config to flip the
  proxy between `bridge` and `metadata-only` without editing JSON. The helper
  (documented in `docs/source/sorafs_cli.md`) emits a JSON summary; archive it
  with the incident ticket and note the PagerDuty/Alertmanager IDs in the
  `--json-out` artefact.
- **Evidence & rollback.** Every proxy flip (automatic or manual) must be logged
  next to the SNNet blocker reference, include the Alertmanager ID, and record
  the `soranet_proxy_policy_queue_depth` gauge returning to `0`. When telemetry
  stabilises, clear the override and attach the same evidence bundle to the
  release packet so reviewers can see the downgrade rationale and recovery.

Roadmap note: the SoraNet Anonymity Overlay items called out above map directly to `roadmap.md:532` (SNNet-4 relay implementation through SNNet-13 broadcast tooling). Keep those statuses 🈺 in the rollout log whenever you force `direct-only`, and cite the specific SNNet blocker when requesting an exception.

# 4. Release Gating Checklist (SF-6c)

Perform these steps for every release candidate and attach the resulting artefacts
to the approval record:

1. **Fixture smoke:** Run `ci/sdk_sorafs_orchestrator.sh`. It replays the shared `multi_peer_parity_v1` fixture through the Rust, JS (Node), Python (native module), and Swift harnesses to ensure all SDKs ship the orchestrator bindings.
   - The script now writes a parity matrix (`artifacts/sorafs_orchestrator_sdk/<timestamp>/matrix.md`) alongside the machine-readable summary (`summary.json`) and snapshots the fixture JSONs used for the run. Include both artefacts (and the `fixture/` copy) in the release evidence so reviewers can see which SDKs passed, how long each run took, and which commands/tests were executed.
   - CI option: trigger or reference the `sorafs-orchestrator-sdk` GitHub Actions workflow (`.github/workflows/sorafs-orchestrator-sdk.yml`) for the release candidate; download the uploaded artefact bundle in lieu of running the script locally.
2. **CLI adoption evidence:** Execute `ci/check_sorafs_orchestrator_adoption.sh`. The helper replays the canonical fixture with `sorafs_fetch`, captures `scoreboard.json`/`summary.json`/`provider_metrics.json` under `artifacts/sorafs_orchestrator/<stamp>/`, and then runs `cargo xtask sorafs-adoption-check --require-telemetry --require-telemetry-region --scoreboard <...>/scoreboard.json --summary <...>/summary.json --report <...>/adoption_report.json` so the gate fails whenever captures drift from multi-source expectations. Use the following checklist for every run:
   - **Scoreboard metadata contract.** `scoreboard.json.metadata` must include the orchestrator crate version, concurrency knobs (`max_parallel`, `max_peers`, retry budget, provider failure threshold), the telemetric source label enforced by `--require-telemetry`, `transport_policy` + `transport_policy_override{,_label}`, `anonymity_policy`, and the provider mix fields:
     * `provider_count` (direct providers) and `gateway_provider_count` (Torii gateway sources) with `provider_mix` derived from those counts—gateway-only captures therefore persist `provider_count=0`, `gateway_provider_count=<n>`, and `provider_mix="gateway-only"` so the gate no longer mistakes them for mixed-mode runs (`crates/iroha_cli/src/commands/sorafs.rs:600`,`crates/sorafs_car/src/bin/sorafs_fetch.rs:648`).
     * `gateway_manifest_provided` plus the matching digest fields (`gateway_manifest_id`, `gateway_manifest_cid`) whenever the capture uses Torii gateways, letting `cargo xtask sorafs-adoption-check` reject scoreboard/summary pairs that omit the manifest envelope evidence mandated by governance (see also `xtask/src/sorafs.rs:5669`).
     * Confirmation that `--use-scoreboard` was active; the adoption script refuses captures that drop out of scoreboard mode or advertise `transport_policy="direct-only"` without an explicit override.
  - **Telemetry source/region parity.** `--telemetry-source-label=<otel stream>` now populates both `scoreboard.json.metadata.telemetry_source` and `summary.json.telemetry_source`, so `cargo xtask sorafs-adoption-check --require-telemetry` can prove which OTLP pipeline produced the capture; pair it with `--telemetry-region=<region>` plus `--require-telemetry-region` whenever governance needs proof of the fleet/rollout wave that produced the evidence. Mismatched or missing labels cause the gate to fail immediately, and the adoption gate cross-checks that the summary label matches the scoreboard metadata before the bundle is accepted (`crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:1595`,`crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:1970`,`xtask/src/sorafs.rs:6165`).
   - **Summary parity & provider counts.** `summary.json` mirrors the direct/gateway split (`provider_count`, `gateway_provider_count`, `provider_mix`) and the gate cross-checks these numbers against `scoreboard.json` plus the per-provider chunk receipts. Runs with zero-weight eligible providers, missing summaries, or chunk totals that disagree stop immediately; supply `--min-providers`, `--allow-zero-weight`, or `--allow-single-source` only when you are intentionally staging a downgrade and citing an incident ID, and pair direct-mode downgrades with `--require-direct-only` so the adoption report proves the fallback posture.
   - **Gateway manifest evidence.** When a fixture (or production capture) exercises Torii gateways you must pass `--manifest-envelope <path>` (or the equivalent SDK option) so the scoreboard metadata can assert `gateway_manifest_provided=true`. Gateways that rely on unsigned manifests now cause `cargo xtask sorafs-adoption-check` to fail, guaranteeing every adoption bundle records the manifest identifier/CID pair that was exercised. `sorafs_cli fetch` validates the hybrid envelope (version, suite, and non-empty KEM/ciphertext fields) before setting the flag, so malformed or empty envelopes fail fast instead of slipping past the adoption gate.
   - **Override & implicit-metadata tracking.** Because the helper feeds the fixture through `--allow-implicit-provider-metadata`, it also forwards `--allow-implicit-metadata` to the adoption check automatically; any extra flags (including `--allow-single-source`/`--require-direct-only`) can be supplied through `XTASK_SORAFS_ADOPTION_FLAGS`, which the script tokenises with `shlex.split` so quoted report labels survive untouched. For manual runs that keep the implicit metadata override, remember to add `--allow-implicit-metadata` yourself and retain the `implicit_metadata_override_used` flag from the adoption report. If an incident forces you to clamp concurrency (`--max-peers=1`, `transport_policy=direct-only`, etc.), set `XTASK_SORAFS_ADOPTION_FLAGS="--allow-single-source --require-direct-only"` before rerunning the helper and link the override to the incident record in the release packet.
3. **CLI verification:** Execute `sorafs_cli fetch` (or the standalone `sorafs_fetch` binary) against the canonical fixture with three providers, `--scoreboard-out` pointing to `artifacts/sorafs_orchestrator/<stamp>/scoreboard.json`, and `--json-out` capturing the summary. Include `--telemetry-source-label=<otel stream>` so the scoreboard metadata records which OTLP pipeline produced the concurrency snapshot; when you need deterministic timestamps for diffing, pass `--scoreboard-now=<fixture unix timestamp>`. Verify that:
   - `provider_reports` contains every provider and `max_parallel` matches the profile.
   - `scoreboard.json` weights sum to 10 000 and all providers remain eligible.
   - Run `cargo xtask sorafs-adoption-check --scoreboard artifacts/sorafs_orchestrator/<stamp>/scoreboard.json --summary artifacts/sorafs_orchestrator/<stamp>/summary.json --report artifacts/sorafs_orchestrator/<stamp>/adoption_report.json` (defaults resolve to `artifacts/sorafs_orchestrator/latest/{scoreboard,summary}.json`). Pass `--allow-single-source` **only** when intentionally downgrading with an incident ID or governance approval, add `--require-direct-only` for direct-mode downgrades so the evidence proves the policy change, and pair fixture replays that used `--allow-implicit-provider-metadata` with `--allow-implicit-metadata`; otherwise the command fails whenever fewer than the expected providers participate or the scoreboard metadata advertises implicit descriptors.
     The checker now fails if (a) fewer than two eligible providers remain, (b) the normalised weights no longer sum to 1.0 (10 000 scheduler credits), (c) any eligible provider carries zero weight, (d) any provider is marked ineligible, (e) the summary records fewer than the required number of providers serving chunks (based on `provider_reports.successes`/`chunk_receipts`), (f) `chunk_count`, `provider_success_total`, and the number of chunk receipts disagree, or (g) the summary references provider ids that were not present in the scoreboard. Use `--min-providers` / `--allow-zero-weight` overrides only when explicitly staging a downgrade and document the exception in the release ticket.
4. **SDK overlay audit:** Spot-check one SDK (rotate weekly) by invoking its helper with the same fixture inputs. Confirm the summary matches the Rust scoreboard digests.
5. **Telemetry smoke:** Load `dashboards/grafana/sorafs_fetch_observability.json` in staging and ensure `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_failures_total`, and PQ ratio panels stream live data. Alerts defined in `docs/examples/sorafs_fetch_alerts.yaml` must stay green for 24 h before GA. The new “Privacy suppression” row (snapshot tables, suppression-rate chart, and evicted-bucket stat) must also show zeroed counts outside rehearsals so SNNet-8a reviewers can see that the secure aggregator is not silently discarding buckets.
   - Capture the corresponding `telemetry::sorafs.fetch.*` log stream (NDJSON or plain-text) for the entire burn-in window and run `cargo xtask sorafs-burn-in-check --log <path> --window-days 30 --min-pq-ratio 0.95 --max-brownout-ratio 0.01 --max-no-provider-errors 0 --min-fetches 150 --out artifacts/sorafs_orchestrator/<stamp>/burn_in_summary.json` to prove the SLO was met. The helper now enforces a brownout ceiling (default 1 % of total fetches), a PQ ratio floor, and a minimum fetch sample (default 100; raise it with `--min-fetches` per rollout policy) so include the resulting JSON (or stdout capture) in the evidence bundle whenever the command passes.
6. **Evidence drop:** Store scoreboard JSONs, CLI summaries, Grafana exports, and alert test logs under `artifacts/sorafs_orchestrator/<YYYYMMDDThhmmZ>/` and reference them from `docs/source/sorafs/reports/orchestrator_ga.md`.

Releases must not proceed until every step above passes. If multi-source support regresses, set `transport_policy=direct-only`, file an incident, and keep the toggle in place until CI/SDK parity is restored.

# 5. Automation Workflow

To keep the adoption evidence reproducible, run `make check-sorafs-adoption` (wrapper around `ci/check_sorafs_orchestrator_adoption.sh`) for every release candidate, or let the `sorafs-orchestrator-adoption` GitHub workflow (pushes to `main`, nightly schedule, and manual dispatch) execute it automatically. The helper:

1. Copies the canonical fixture (`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`) under `artifacts/sorafs_orchestrator/<timestamp>/` so every run records the exact inputs.
2. Invokes `sorafs_fetch` with `--use-scoreboard` and the generated plan/providers so the scoreboard JSON always contains the concurrency knobs (`max_parallel`, `max_peers`, retry budget, provider failure threshold) and telemetry metadata.
3. Calls `cargo xtask sorafs-adoption-check --require-telemetry --scoreboard ... --summary ... --min-providers <fixture quorum> --report ... ${XTASK_SORAFS_ADOPTION_FLAGS}` to fail fast when the scoreboard is missing metadata, falls back to single-source behaviour, drops OTLP evidence, or references providers that never served chunks. The telemetry flag enforces the presence of the `telemetry_source` label so every adoption bundle proves which OTLP snapshot produced the scoreboard, and the helper automatically appends `--allow-implicit-metadata` when it reuses baked-in capability hints.
4. Emits `scoreboard.json`, `summary.json`, `provider_metrics.json`, `chunk_receipts.json`, and `adoption_report.json` plus a `latest/` symlink for reviewers. Attach these files to the release ticket; if an override is necessary, set `XTASK_SORAFS_ADOPTION_FLAGS="--allow-single-source --require-direct-only"` before re-running the script and cite the incident ID in the ticket. CI uploads the generated directory (`artifacts/sorafs_orchestrator/<stamp>/`) as a build artefact so release approvers always have access to the evidence bundle without re-running the helper.

This automation keeps the CLI/SDK gate in lock-step with the roadmap requirement: any regression to single-source fetches (e.g., `max_peers=1` or `max_parallel=1`) fails before a release is tagged, and the evidence bundle captures exactly which knobs were used for the run.

# 6. Telemetry & SLO Contract (SF-6e)

## 6.1 Observability Assets

- **Metrics:** `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_retries_total`,
  `sorafs_orchestrator_provider_failures_total`, `sorafs_orchestrator_stalls_total`,
  `sorafs_orchestrator_chunk_latency_ms_bucket`, and `sorafs_orchestrator_bytes_total`
  capture orchestrator health; Torii emits `torii_sorafs_range_fetch_concurrency_current`
  and `torii_sorafs_range_fetch_throttle_events_total` for the ingress envelope
  (`crates/iroha_telemetry/src/metrics.rs:5170-5200`).  
- **Privacy suppression analytics:** `soranet_privacy_snapshot_suppressed`,
  `soranet_privacy_snapshot_suppressed_by_mode`,
  `soranet_privacy_suppression_total`, and `soranet_privacy_evicted_buckets_total`
  surface SNNet-8a collector health. The privacy row in
  `dashboards/grafana/sorafs_fetch_observability.json` charts those metrics so
  operators can prove that buckets are not being suppressed silently; for
  machine-readable audits run `cargo xtask soranet-privacy-report --input <ndjson>`
  as documented in `docs/source/soranet/privacy_metrics_pipeline.md`.  
- **Dashboards:** `dashboards/grafana/sorafs_fetch_observability.json` (UID `sorafs-fetch`)
  exposes region/manifest selectors plus the `sorafs-fetch-default-on` /
  `sorafs-fetch-burnin` annotation streams, while
  `dashboards/grafana/soranet_pq_ratchet.json` tracks policy stages.  
- **Alerts:** Deploy `dashboards/alerts/sorafs_fetch_rules.yml` (fetch failures, retry storms,
  latency percentiles, stall spikes) and `dashboards/alerts/soranet_policy_rules.yml`
  (brownouts) through your Alertmanager stack. Run
  `promtool test rules dashboards/alerts/tests/sorafs_fetch_rules.test.yml` after every edit.
- **Log targets:** Ingest the `telemetry::sorafs.fetch.*` span in the shared log pipeline and
  pin saved searches for `event=complete status=failed` before GA.
- **Burn-in capture helper:** `ci/check_sorafs_orchestrator_adoption.sh` now accepts
  `SORAFS_BURN_IN_*` environment variables plus `SORAFS_BURN_IN_LOGS` and runs
  `cargo xtask sorafs-burn-in-check` automatically. It emits `burn_in_note.json` and
  `burn_in_summary.json` next to the scoreboard/summary/adoption report, aborting when the 30-day
  window, PQ ratio, brownout ceiling, `no_healthy_providers` cap, or minimum fetch count fail.
  Each note records the capture label (region/day), manifest, telemetry sources, thresholds, and
  artefact paths so reviewers can trace a dashboard annotation back to the exact OTLP snapshot.
- **Burn-in evidence helper:** You can still call `cargo xtask sorafs-burn-in-check` directly when
  stitching multiple log exports together; it enforces the same 30-day window/PQ ratio/brownout
  ratio/no-error/min-fetch contract, records both `stall_event_ratio` and `stall_chunk_ratio`, and
  refuses to run without at least one `--log` file. Override `--max-brownout-ratio` or
  `--min-fetches` (or their `SORAFS_BURN_IN_*` counterparts) only when governance explicitly
  relaxes the thresholds for a maintenance window and document the exception in the ticket. Attach
  both `burn_in_note.json` and `burn_in_summary.json` to the governance tracker.
- **Burn-in metadata guardrails:** `ci/check_sorafs_orchestrator_adoption.sh` now aborts when
  `SORAFS_BURN_IN_LABEL` is set but the associated metadata is missing or inconsistent.
  Provide the following environment variables whenever you capture burn-in evidence—the script
  validates the numeric fields (day index must be ≥1 and ≤ `SORAFS_BURN_IN_WINDOW_DAYS`, which
  defaults to 30), enforces a positive `SORAFS_BURN_IN_MIN_FETCHES`, and requires at least one log
  path before emitting `burn_in_note.json` and `burn_in_summary.json`:

  | Environment variable | Purpose |
  |----------------------|---------|
  | `SORAFS_BURN_IN_LABEL` | Human-readable tag inserted into Grafana annotations and the burn-in note. |
  | `SORAFS_BURN_IN_REGION` | Region or cluster that produced the capture (for example `eu-central-1`). |
  | `SORAFS_BURN_IN_MANIFEST` | Manifest identifier/CID under test so reviewers can link the run to a payload. |
  | `SORAFS_BURN_IN_LOGS` | Space-separated telemetry log paths passed to `cargo xtask sorafs-burn-in-check`. |
  | `SORAFS_BURN_IN_DAY` | 1-based index representing the day inside the active burn-in window. |
  | `SORAFS_BURN_IN_WINDOW_DAYS` | Optional override for the window length; must be a positive integer. |
  | `SORAFS_BURN_IN_NOTES` | Optional free-form context (pager rotation, maintenance blurbs, change-request IDs). |
  | `SORAFS_BURN_IN_MIN_PQ_RATIO` | Optional PQ ratio floor (float) applied to every log set; default `0.95`. |
  | `SORAFS_BURN_IN_MAX_BROWNOUT_RATIO` | Optional brownout ceiling (float) per fetch; default `0.01`. |
  | `SORAFS_BURN_IN_MAX_NO_PROVIDER_ERRORS` | Optional cap on `no_healthy_providers` errors (integer); default `0`. |
  | `SORAFS_BURN_IN_MIN_FETCHES` | Optional minimum fetch count (integer); default `150`. |

- **Artefact guardrails:** `ci/check_sorafs_orchestrator_adoption.sh` now fails fast when any
  required artefact is missing or empty (plan, payload, telemetry fixture, scoreboard, summary,
  provider metrics, chunk receipts, adoption report), so burn-in notes cannot reference stale or
  incomplete evidence.

**Annotation workflow.** The `sorafs_fetch` Grafana board (`dashboards/grafana/sorafs_fetch_observability.json`) now exposes two built-in annotation streams:

1. Tag the moment multi-source becomes the production default with a `sorafs-fetch-default-on` annotation that links to the release ticket, the `adoption_report.json`, and the `ci/check_sorafs_orchestrator_adoption.sh` log. The annotation survives dashboard exports, so Alertmanager playbooks can quickly confirm whether an incident happened during the default-on window.
2. Bracket every telemetry burn-in or chaos rehearsal with `sorafs-fetch-burnin` annotations.
   Include the burn-in window, the pager schedule that reviewed the run, the path to the
   `burn_in_note.json`, and a pointer to the `cargo xtask sorafs-burn-in-check --log ... --out artifacts/sorafs_orchestrator/<stamp>/burn_in_summary.json` output so postmortems can jump straight to the evidence bundle.

The annotations ship with the dashboard JSON, which means `grafana-toolkit dashboards apply` will keep staging and production in sync and Alertmanager can reference the tags when emitting the `sorafs-fetch` escalation.

## 6.2 Burn-In SLO Reference

| Signal / panel | SLO & acceptance gate | Evidence source |
|----------------|----------------------|-----------------|
| `sorafs_orchestrator_fetch_failures_total{failure_reason="no_healthy_providers"}` | Exactly 0 events across the rolling 30-day window; any increment is Sev 2 and restarts the clock. | “Failure mix” panel inside `dashboards/grafana/sorafs_fetch_observability.json`; the CLI enforces the bound via `cargo xtask sorafs-burn-in-check --max-no-provider-errors 0`. |
| `sorafs_orchestrator_pq_ratio`, `sorafs_orchestrator_pq_candidate_ratio` | Remain within ±5 % of the staged baseline and ≥0.95 whenever the policy stage is `anon-guard-pq` or stricter. | “PQ coverage” panel in `dashboards/grafana/sorafs_fetch_observability.json` plus the companion PQ ratchet board (`dashboards/grafana/soranet_pq_ratchet.json`). |
| `sorafs_orchestrator_fetch_duration_ms_bucket` p95 | ≤250 ms sustained; alert if exceeded for >10 minutes. | Latency panels in `dashboards/grafana/sorafs_fetch_observability.json` backed by `promtool test rules dashboards/alerts/tests/sorafs_fetch_rules.test.yml`. |
| `sorafs_orchestrator_brownouts_total` / `sorafs_orchestrator_policy_events_total{event="brownout"}` | No increments outside rehearsals; every intentional brownout must be annotated. | Policy section of `dashboards/grafana/sorafs_fetch_observability.json` and alert pack `dashboards/alerts/soranet_policy_rules.yml`. |
| `soranet_privacy_snapshot_suppressed` / `soranet_privacy_suppression_total` | Suppressed buckets must remain at 0 outside forced-flush rehearsals; any increment restarts the burn-in clock and triggers an SNNet-8a ticket that cites the affected mode/reason. | “Privacy suppression” row in `dashboards/grafana/sorafs_fetch_observability.json` plus `cargo xtask soranet-privacy-report --input <ndjson>` attachments from the secure-aggregator exports. |
| `sorafs.fetch.provider_failures_total` + `scoreboard.json.metadata.provider_mix` | <5 % of fetches serviced by a single provider once the default is multi-source; gateway-only runs must report `provider_mix="gateway-only"`. | `ci/check_sorafs_orchestrator_adoption.sh` outputs (`scoreboard.json`, `summary.json`, `adoption_report.json`). |

Telemetry burn-in completes when those thresholds hold for 30 consecutive days:

1. `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` stays flat.
2. PQ ratio charts never drift more than ±5 % from the baseline captured in the burn-in note
   (record the start/end hashes from `dashboards/grafana/soranet_pq_ratchet.json`).
3. `dashboards/alerts/sorafs_fetch_rules.yml` and `dashboards/alerts/soranet_policy_rules.yml`
   show zero alert firings outside scheduled chaos drills—attach Alertmanager silences or
   escalation transcripts when drills purposely trip an alert.

## 6.3 Daily On-Call Checklist

1. Run `ci/check_sorafs_orchestrator_adoption.sh` with the active `SORAFS_BURN_IN_*` metadata and
   `SORAFS_BURN_IN_LOGS="<log0> <log1> ..."` to regenerate `scoreboard.json`, `summary.json`,
   `adoption_report.json`, `burn_in_note.json`, and `burn_in_summary.json` (the helper invokes
   `cargo xtask sorafs-burn-in-check` with the provided log set). Confirm the telemetry labels,
   thresholds, and provider mix match the live rollout.
2. If logs rotate mid-window, rerun `cargo xtask sorafs-burn-in-check` (or the helper) with the
   updated `--log` list so `burn_in_summary.json` reflects the full 30-day span; keep the output
   under `artifacts/sorafs_orchestrator/<stamp>/burn_in_summary.json` alongside the note.
3. Export screenshots or JSON from `dashboards/grafana/sorafs_fetch_observability.json`
   (success ratio, PQ ratio, latency percentile panels) and archive them under
   `artifacts/sorafs_orchestrator/<stamp>/telemetry/`. Update the Grafana annotation with the same
   `SORAFS_BURN_IN_LABEL` recorded in `burn_in_note.json`.
4. Re-run `promtool test rules dashboards/alerts/tests/sorafs_fetch_rules.test.yml` and
   `promtool test rules dashboards/alerts/tests/soranet_policy_rules.test.yml` after any rule edits,
   storing the console output with the day’s evidence bundle.
5. If a threshold fails, follow `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md` §6 to pause
   the rollout, set `transport_policy=direct-only`, and file/assign the incident before restarting the
   burn-in.

## 6.4 Drill & Rollback Hooks

- Rehearse the direct-mode fallback weekly with `scripts/sorafs_direct_mode_smoke.sh`
  (the same flow documented in `docs/examples/sorafs_direct_mode_smoke.sh`) so the on-call rotation can
  switch to the single-source transport without improvisation.
- Log every drill/rollback outcome in `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`
  alongside the Alertmanager incident ID, `adoption_report.json`, and `burn_in_summary.json`.
- Pair the drills with a PQ ratchet review by exporting `dashboards/grafana/soranet_pq_ratchet.json`
  so governance can see how brownouts were handled whenever overrides were applied.

# 7. Evidence & Reporting Expectations

- **Scoreboard history:** Persist every `--scoreboard-out` artefact and compare it against the previous release. Changes >10 % weight require sign-off from Storage + Governance. Use `cargo xtask sorafs-scoreboard-diff --previous artifacts/sorafs_orchestrator/<old>/scoreboard.json --current artifacts/sorafs_orchestrator/<new>/scoreboard.json --threshold-percent 10 --report artifacts/sorafs_orchestrator/<new>/scoreboard_diff.json` to produce the annotated delta table and JSON evidence bundle.
- **Parity matrices:** Update `docs/source/sorafs/reports/orchestrator_ga.md` with the latest SDK smoke timestamps and attach both the raw `ci/sdk_sorafs_orchestrator.sh` log and the generated `matrix.md` artefact for the corresponding run.
- **Incident readiness:** Link back to the broader multi-source runbook for emergency blacklisting procedures and document every override in the operations changelog.
- **Provider mix evidence:** Every capture now records `provider_count`, `gateway_provider_count`, **and** the derived `provider_mix` label inside both `scoreboard.json` metadata and `summary.json`. Gateway-only sessions therefore emit `provider_count=0`, `gateway_provider_count=<n>`, `provider_mix="gateway-only"`, which prevents `cargo xtask sorafs-adoption-check` from classifying them as mixed-mode runs per SF-6c’s acceptance criteria. The adoption gate now rejects scoreboard/summary pairs that omit these fields or report inconsistent counts/labels, so CI cannot publish artefacts that would fail the provider-mix evidence review.
- **Adoption report snapshot:** `cargo xtask sorafs-adoption-check --report` now copies the provider/gateway counts, provider mix label, and transport policy override state out of `summary.json`, so governance reviewers can confirm mixed-mode coverage without re-opening the raw artefacts.
- **Summary transport evidence:** `cargo xtask sorafs-adoption-check` now inspects the `transport_policy` recorded in `summary.json`; runs that downgrade to `transport_policy="direct-only"` without `--allow-single-source` (or the SDK equivalent) fail the gate, so intentional rollbacks must be accompanied by the explicit override flag (and `--require-direct-only`) plus the incident link.
- **Gateway manifest evidence:** `scoreboard.json` now sets `gateway_manifest_provided=true` only when a valid hybrid `--gateway-manifest-envelope` is supplied alongside gateway providers. Captures that omit, truncate, or corrupt the envelope keep the flag at `false`, allowing the adoption gate to reject bundles that lack the signed manifest evidence expected by governance.
- **Hybrid envelope guardrails:** `sorafs_cli fetch` rejects empty or malformed manifest envelope files (version, suite, or missing KEM/ciphertext fields) up front so captures cannot accidentally mark `gateway_manifest_provided=true` with bad evidence; SDKs should mirror this check when wiring gateway helpers.
- **Gateway manifest digests:** Gateway captures must include `gateway_manifest_id`/`gateway_manifest_cid` inside `scoreboard.json` metadata and matching `manifest_id`/`manifest_cid` fields in `summary.json`. `cargo xtask sorafs-adoption-check` now cross-checks those four fields and fails when any are missing or mismatched, guaranteeing that every release bundle identifies the exact manifest envelope that was exercised.

Keeping this checklist up to date ensures that the orchestrator’s default-on
rollout remains deterministic, auditable, and reversible across every supported
client.
