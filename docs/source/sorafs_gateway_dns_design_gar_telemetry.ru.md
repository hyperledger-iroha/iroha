---
lang: ru
direction: ltr
source: docs/source/sorafs_gateway_dns_design_gar_telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ede73438cc4f9013ee34e9047353195be2e4273c7df94b5497fc21d2da2dcc70
source_last_modified: "2026-01-03T18:08:00.655051+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: GAR Telemetry Snapshot — 2025-02-21
summary: Staging metrics for SoraFS gateway GAR enforcement ahead of the kickoff, including the 2025-02-21 baseline and 2025-03-02 refresh.
---

# GAR Telemetry Snapshot (2025-02-21 14:05 UTC)

This note captures the latest staging telemetry used to brief stakeholders
before the SoraFS Gateway & DNS kickoff. Raw scrape is stored in
`docs/source/sorafs_gateway_dns_design_metrics_20250221.prom`.

## Key Findings — 2025-02-21 Snapshot

- **GAR violations remain low.** Two manifest digest mismatches were recorded on
  `gw-stage-1` over the past 24 h, both tied to outdated manifests. Rate-limit
  violations are isolated to stream-token quota exhaustion.
- **Refusals align with expectations.** All refusals stem from missing required
  headers (`X-SoraFS-*` set). No unsupported chunker handles observed.
- **Fixture version parity.** Both stage gateways advertise `torii_sorafs_gateway_fixture_version{version="1.0.0"} == 1`, matching the current conformance bundle.
- **TLS expiry buffer.** Certificates expire in ~7 days (604 800 s), well within
  the automation SLA; no immediate rotation required.

## Metric Breakdown — 2025-02-21

| Metric | Labels | Value | Interpretation |
|--------|--------|-------|----------------|
| `torii_sorafs_gar_violations_total` | `{gateway="gw-stage-1", reason="manifest_not_admitted", detail="manifest_digest_mismatch"}` | `2` | Old manifest envelopes hit the gateway; resolved after cache refresh. |
| `torii_sorafs_gar_violations_total` | `{gateway="gw-stage-1", reason="rate_limited", detail="stream_token_quota_exceeded"}` | `1` | Client exceeded assigned stream token quota; highlights need for better retry messaging. |
| `torii_sorafs_gateway_refusals_total` | `{gateway="gw-stage-1", profile="sf1", reason="missing_headers"}` | `3` | Requests missing `X-SoraFS-Manifest-Envelope` and nonce headers; triggers remain deterministic. |
| `torii_sorafs_gateway_refusals_total` | `{gateway="gw-stage-2", profile="sf1", reason="missing_headers"}` | `1` | Single refusal, already acknowledged by Tooling WG as harness test. |
| `torii_sorafs_gateway_fixture_version` | `{gateway="gw-stage-1", version="1.0.0"}` | `1` | Confirms fixture parity; alert would fire if zero. |
| `torii_sorafs_tls_cert_expiry_seconds` | _global_ | `604 800` | Cert rotation planned for 2025-02-28; Ops lead to confirm automation run. |

## Action Items

1. **Manifest cache hygiene:** Ensure DNS/gateway agenda covers deterministic cache busting to minimise digest mismatches.
2. **Header validation UX:** QA Guild to verify error messaging surfaces missing headers clearly in conformance harness.
3. **TLS rotation reminder:** Ops Lead to include upcoming rotation in kickoff deck; automation run scheduled 2025-02-28.

## Refresh — 2025-03-02 15:10 UTC

Per the kickoff checklist, telemetry was re-scraped 24 hours ahead of the
session. Raw metrics live in
`docs/source/sorafs_gateway_dns_design_metrics_20250302.prom`.

### Key Findings — 2025-03-02 Snapshot

- **Manifest mismatches cleared.** `torii_sorafs_gar_violations_total` now
  reports `0` `manifest_digest_mismatch` events on both staging gateways after
  the cache flush that landed on 2025-02-25.
- **Rate limiting steady.** `gw-stage-1` still records a single
  `stream_token_quota_exceeded` violation (the harness regression test),
  while `gw-stage-2` continues to report `0`.
- **Header refusals down to one.** Only one `missing_headers` refusal remains
  across both gateways, confirming the conformance harness updates landed.
- **Fixture parity intact.** Both gateways advertise
  `torii_sorafs_gateway_fixture_version{version="1.0.0"} == 1`.
- **TLS buffer healthy.** `torii_sorafs_tls_cert_expiry_seconds` reports
  `518 400` (~6 days), keeping the 2025-02-28 automation run on schedule.

### Metric Breakdown — 2025-03-02

| Metric | Labels | Value | Interpretation |
|--------|--------|-------|----------------|
| `torii_sorafs_gar_violations_total` | `{gateway="gw-stage-1", reason="manifest_not_admitted", detail="manifest_digest_mismatch"}` | `0` | Cache flush removed stale manifest digests. |
| `torii_sorafs_gar_violations_total` | `{gateway="gw-stage-1", reason="rate_limited", detail="stream_token_quota_exceeded"}` | `1` | Expected harness regression: confirms quota enforcement still fires. |
| `torii_sorafs_gar_violations_total` | `{gateway="gw-stage-2", reason="manifest_not_admitted", detail="manifest_digest_mismatch"}` | `0` | Gateway two remains clean post-flush. |
| `torii_sorafs_gar_violations_total` | `{gateway="gw-stage-2", reason="rate_limited", detail="stream_token_quota_exceeded"}` | `0` | No rate-limit events recorded for gateway two. |
| `torii_sorafs_gateway_refusals_total` | `{gateway="gw-stage-1", profile="sf1", reason="missing_headers"}` | `1` | Single refusal captured during harness validation. |
| `torii_sorafs_gateway_refusals_total` | `{gateway="gw-stage-1", profile="sf1", reason="unsupported_chunker"}` | `0` | Confirms clients only advertise profile `sf1`. |
| `torii_sorafs_gateway_refusals_total` | `{gateway="gw-stage-2", profile="sf1", reason="missing_headers"}` | `0` | No header issues observed on gateway two. |
| `torii_sorafs_gateway_fixture_version` | `{gateway="gw-stage-1", version="1.0.0"}` | `1` | Fixture parity intact; alarms would trigger on `0`. |
| `torii_sorafs_gateway_fixture_version` | `{gateway="gw-stage-2", version="1.0.0"}` | `1` | Mirrors gateway one status. |
| `torii_sorafs_tls_cert_expiry_seconds` | _global_ | `518 400` | Six-day buffer ahead of the planned 2025-02-28 rotation. |

Action items from the 2025-02-21 snapshot remain valid; no additional risks
were identified during this refresh.
