---
lang: ur
direction: rtl
source: docs/source/soranet/snnet16_telemetry_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 818351072a7153e6052fa805b8506801b9c71e7401827dff072cd6e9068fd029
source_last_modified: "2026-01-04T01:11:09.949539+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SNNet-16E Telemetry & Downgrade Detection Plan

This document captures the implementation plan for the SNNet-16E roadmap
deliverable (“Telemetry & downgrade detection”). It ties the runtime metrics in
`tools/soranet-relay`, the privacy aggregation path introduced in SNNet-8, and
the control-plane hooks used by orchestrators into a single checklist so the
Q2 2027 gate has concrete evidence attached.

## Objectives

1. Emit deterministic counters for negotiated suites, downgrade reasons,
   transferred bytes, and Argon2 ticket verification times so PQ rollouts can
   be audited (`sn16_handshake_*`, `sn16_puzzle_verify_seconds_*`).
2. Mirror downgrade events in the privacy NDJSON/log pipeline so Torii,
   collectors, and orchestrators share a common `downgrade.reason` vocabulary.
3. Publish Grafana dashboards + Prometheus alert rules covering mixed-mode
   detection, downgrade bursts, puzzle latency, and proxy remediation status.
4. Wire downgrade bursts into the proxy toggle feed so relays can flip local
   QUIC proxies into metadata-only mode and recover automatically.
5. Document validation (promtool tests, harness fixtures, governance evidence)
   so operators can sign off the telemetry gate ahead of SNNet-16 default-on.

## Instrumentation Inventory

| Signal | Definition & Labels | Source | Notes |
|--------|---------------------|--------|-------|
| `sn16_handshake_mode_total{mode,handshake}` | Counter of negotiated suites per relay mode (`Entry/Middle/Exit`) and handshake profile (`nk2`, `nk3`). | `tools/soranet-relay/src/metrics.rs` | Drives the mixed-mode detector panel in `dashboards/grafana/soranet_sn16_handshake.json`. |
| `sn16_handshake_bytes_total{mode}` | Total bytes transferred per relay mode during handshakes. | `tools/soranet-relay/src/metrics.rs` | Used to correlate PQ suite adoption with bandwidth impact. |
| `sn16_handshake_downgrade_total{mode,reason}` | Count of downgrade detections grouped by a canonical slug (see `normalize_downgrade_reason` in `tools/soranet-relay/src/metrics.rs`). | Relay runtime (`runtime.rs` emits structured privacy events; `metrics.rs` exposes counters). | Reasons include `suite_no_overlap`, `relay_suite_list_missing`, `pow_classical_only`, etc. |
| `sn16_puzzle_verify_seconds_{count,sum}{mode}` | Summary pair representing Argon2 ticket verification cost at each relay mode. | `tools/soranet-relay/src/metrics.rs` | Surfaces ticket solve regressions during PQ ratchets. |
| `soranet_abuse_*` gauges | Active cooldown/quota buckets enforced by the relay (remote/descriptor/emergency scopes). | `tools/soranet-relay/src/metrics.rs` | Needed to prove throttles engage when downgrade floods occur. |
| `soranet_padding_*` counters | Global padding budget: `soranet_padding_cells_sent_total`, `soranet_padding_bytes_sent_total`, and `soranet_padding_cells_throttled_total` grouped by relay mode. | `tools/soranet-relay/src/metrics.rs` | Evidence for the SNNet-4 congestion-aware scheduler; dashboards track how much cover traffic is emitted versus throttled per relay profile. |
| `/privacy/events` NDJSON stream (`SoranetPrivacyEventV1`) | Structured log of handshake success/failure, throttles, verified bytes, GAR categories. | `tools/soranet-relay/src/privacy.rs`, Torii ingestors in `crates/iroha_torii/src/lib.rs` | Downgrade events surface as `SoranetPrivacyEventKindV1::HandshakeFailure` with `reason="Downgrade"`. |
| `/policy/proxy-toggle` NDJSON stream | Minimal buffer of downgrade events for orchestrator-driven proxy flips. | `ProxyPolicyEventBuffer` in `tools/soranet-relay/src/privacy.rs` and HTTP handler in `tools/soranet-relay/src/runtime.rs`. | Enables the “automatic metadata-only” hook cited in the roadmap. |
| `soranet_privacy_*` metrics | Aggregated privacy counters (suppression, throttles, RTT, verified bytes). | `crates/iroha_telemetry/src/privacy.rs` | Provide coarse fleet view while the raw SN16 counters catch regressions. |

The plan does not introduce new metric names; instead it standardises their
labels and log schemas so mixed-mode detectors can reason about relay mode,
downgrade reason, and ticket cost without bespoke parsing.

## Log Schema & Vocabulary

- Downgrade reasons follow the `normalize_downgrade_reason` helper in
  `tools/soranet-relay/src/metrics.rs`. The slug set is frozen in the
  harness fixtures under `fixtures/soranet_handshake/capabilities/*.json`, so
  any new reason requires a fixture bump plus README/documentation updates.
- The NDJSON payloads reuse the Norito types in
  `crates/iroha_data_model/src/soranet/privacy.rs`. Operators consuming
  `/privacy/events` or Torii’s `POST /v1/soranet/privacy/event` will see
  `SoranetPrivacyEventKindV1::HandshakeFailure` with `reason="Downgrade"` plus
  an optional RTT, keeping metrics and logs aligned.
- Proxy toggle events are the same `SoranetPrivacyEventV1` entries, but the
  buffer constrains output to downgrade failures so orchestrators can throttle
  without parsing unrelated telemetry. `/policy/proxy-toggle` honours
  `privacy.proxy_toggle_capacity` and drains events atomically, ensuring
 automations cannot replay stale downgrades.

## Dashboards & Alerts

1. **Grafana board (`dashboards/grafana/soranet_sn16_handshake.json`).**
   - Panels cover handshake mix, downgrade breakdown, byte deltas, Argon2
     latency, and the two SN16-specific additions:
     - `Downgrade to suite ratio`
       (`increase(sn16_handshake_downgrade_total)/clamp_min(increase(sn16_handshake_mode_total), 1)`).
     - `Proxy toggle backlog` visualising the size of
       `ProxyPolicyEventBuffer` via the `soranet_proxy_policy_queue_depth`
       Prometheus gauge.
2. **Alert rules (`dashboards/alerts/soranet_handshake_rules.yml`).**
   - Existing rules `SoranetSn16MixedMode` and `SoranetSn16DowngradeSpike`
     stay, with thresholds documented inline.
   - Add `SoranetSn16PuzzleSlow` (puzzle median > 750 ms for 10 minutes) and
     `SoranetSn16ProxyToggleStuck` (proxy buffer not drained for > 5 minutes).
   - Keep tests in `dashboards/alerts/tests/soranet_handshake_rules.test.yml`
     plus new fixtures for the added rules.
3. **Privacy dashboard cross-links.** The privacy board described in
   `docs/source/soranet/privacy_metrics_pipeline.md` now embeds the SN16
   downgrade signals directly via two new panels in
   `dashboards/grafana/soranet_privacy_metrics.json`:
   - **SN16 Downgrade Ratio (5m)** — renders
     `sum by (mode)(increase(sn16_handshake_downgrade_total[5m])) /
     sum by (mode)(increase(sn16_handshake_mode_total[5m]))` as a percentage so
     SREs can correlate mixed-mode spikes with privacy throttling.
   - **SN16 Downgrade Reasons (5m)** — mirrors the downgrade counter by reason
     so privacy and networking reviewers share the same vocabulary when
     triaging regressions.
   The panels honour the existing `$mode` variable, meaning privacy operators
   can stay on a single board to validate suppression, throttles, and PQ
   downgrade rates without context switches.

## Remediation Hooks

- `ProxyPolicyEventBuffer` (see `tools/soranet-relay/src/privacy.rs`) already
  stores downgrade events; the runtime exposes `/policy/proxy-toggle` for
  orchestrators (`tools/soranet-relay/src/runtime.rs:2779`). The stream now
  includes a deterministic `detail` slug derived from
  `normalize_downgrade_reason` so automations see the same vocabulary as the
  Prometheus counters, and the `soranet_proxy_policy_queue_depth` gauge is
  exported alongside the other SN16 metrics
  (`tools/soranet-relay/src/metrics.rs:247`) to flag backlogs in Alertmanager.
  The Sorafs orchestrator guide (`docs/source/sorafs_orchestrator_rollout.md`)
  captures the toggle workflow and rollback expectations so operators have a
  single source of truth next to the proxy feed.

##### Proxy Toggle Schema

Every `/policy/proxy-toggle` response line is an ordinary
`SoranetPrivacyEventV1` entry encoded as NDJSON. The relay now stamps downgrade
events with the canonical slug so orchestration scripts can react without
parsing free-form strings:

| Field | Description |
|-------|-------------|
| `timestamp_unix` | Seconds since Unix epoch when the downgrade was observed. |
| `mode` | Relay mode that issued the downgrade (`entry`/`middle`/`exit`). |
| `kind.handshakeFailure.reason` | Always `"Downgrade"` for proxy toggle entries. |
| `kind.handshakeFailure.detail` | Normalised slug (`suite_no_overlap`, `mlkem_missing`, `downgrade`, …) derived in `tools/soranet-relay/src/runtime.rs:1311` using `normalize_downgrade_reason`. |
| `kind.handshakeFailure.rtt_ms` | Optional RTT captured for the failed handshake. |

Consumers should treat the presence of `detail` as mandatory for downgrade
automation; `null` indicates the relay could not determine a slug or the
field was intentionally scrubbed.
- The privacy event stream continues to feed Torii via
  `crates/iroha_torii/src/lib.rs::record_soranet_privacy_event`. The
  telemetry pipeline aggregates downgrades (see
  `crates/iroha_telemetry/src/privacy.rs`) so mixed-mode alerts can leverage
  either the raw SN16 counters or the privacy-preserving aggregates.

## Validation & Evidence

- **Unit + integration tests.**
  - `tools/soranet-relay/src/runtime.rs` now anchors the downgrade telemetry
    contract in `downgrade_events_hit_metrics_and_proxy_queue`, proving the
    slugged detail flows into both the `sn16_handshake_downgrade_total` counter
    and the `/policy/proxy-toggle` NDJSON buffer alongside the existing
    `HandshakeError::Downgrade` coverage.
  - Keep `promtool` tests under
    `dashboards/alerts/tests/soranet_handshake_rules.test.yml` up to date
    whenever thresholds change.
  - Ensure `crates/iroha_torii/tests/soranet_privacy_endpoints.rs` exercises
    the downgrade reason payload so schema contracts remain stable.
- **Harness captures.** `tools/soranet-handshake-harness` should emit fixture
  sets for `nk2↔nk2` success, `nk2` downgrade, and argon2 failure. The JSON
  bundles under `fixtures/soranet_handshake/` double as documentation for
  the allowed downgrade reasons.
- **Governance evidence.** Each release that touches telemetry must append a
  note to `status.md` and attach:
  - Grafana screenshot of the SN16 board (downgrade panel + mixed mode ratio).
  - `promtool test rules` output for `soranet_handshake_rules.yml`.
  - Proxy toggle drain log excerpt proving orchestrator ↔ relay integration.

## Delivery Checklist

| Item | Owner | Status Notes |
|------|-------|--------------|
| Metric schema + docs | Networking TL / Observability | Completed via this plan; metrics already exported from `tools/soranet-relay/src/metrics.rs`. |
| Dashboard refresh + alerts | Observability | Completed — downgrade ratio + proxy backlog panels ship in `dashboards/grafana/soranet_sn16_handshake.json`, and the alert pack now includes the puzzle slow/proxy queue rules with promtool fixtures (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/tests/soranet_handshake_rules.test.yml`). |
| Proxy toggle telemetry | Networking TL | Completed — the rollout guide (`docs/source/sorafs_orchestrator_rollout.md`) now documents the `/policy/proxy-toggle` workflow, downgrade_remediation block, and `sorafs_cli proxy set-mode` evidence flow on top of the existing gauge + schema updates. |
| Privacy overlay panel | Observability / Privacy WG | Completed — the privacy metrics board (`dashboards/grafana/soranet_privacy_metrics.json`) now exposes the SN16 downgrade ratio/reason panels so PQ regressions appear alongside suppression data. |
| Validation harness updates | QA Guild | Completed — `downgrade_events_hit_metrics_and_proxy_queue` in `tools/soranet-relay/src/runtime.rs` locks in the slug/metric/NDJSON contract so `/policy/proxy-toggle` and `sn16_handshake_downgrade_total` stay in sync. |

With the checklist closed, SNNet-16 telemetry sign-off focuses on keeping the
dashboards and alerts green during the 30-day burn-in window.
