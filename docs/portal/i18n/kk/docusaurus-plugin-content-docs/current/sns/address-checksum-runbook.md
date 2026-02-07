---
id: address-checksum-runbook
lang: kk
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Account Address Checksum Incident Runbook
sidebar_label: Checksum incidents
description: Operational response for IH58 (preferred) / compressed (`sora`, second-best) checksum failures (ADDR-7).
---

:::note Canonical Source
This page mirrors `docs/source/sns/address_checksum_failure_runbook.md`. Update
the source file first, then sync this copy.
:::

Checksum failures surface as `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`) across
Torii, SDKs, and wallet/explorer clients. The ADDR-6/ADDR-7 roadmap items now
require operators to follow this runbook whenever checksum alerts or support
tickets fire.

## When to run the play

- **Alerts:** `AddressInvalidRatioSlo` (defined in
  `dashboards/alerts/address_ingest_rules.yml`) trips and the annotations list
  `reason="ERR_CHECKSUM_MISMATCH"`.
- **Fixture drift:** The `account_address_fixture_status` Prometheus textfile or
  Grafana dashboard reports a checksum mismatch for any SDK copy.
- **Support escalations:** Wallet/explorer/SDK teams cite checksum errors, IME
  corruption, or clipboard scans that no longer decode.
- **Manual observation:** Torii logs show repeated `address_parse_error=checksum_mismatch`
  for production endpoints.

If the incident is specifically about Local-8/Local-12 collisions, follow the
`AddressLocal8Resurgence` or `AddressLocal12Collision` playbooks instead.

## Evidence checklist

| Evidence | Command / Location | Notes |
|----------|-------------------|-------|
| Grafana snapshot | `dashboards/grafana/address_ingest.json` | Capture invalid reason breakdowns and affected endpoints. |
| Alert payload | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | Include context labels and timestamps. |
| Fixture health | `artifacts/account_fixture/address_fixture.prom` + Grafana | Proves whether SDK copies drifted from `fixtures/account/address_vectors.json`. |
| PromQL query | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | Export CSV for the incident doc. |
| Logs | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (or log aggregation) | Scrub PII before sharing. |
| Fixture verification | `cargo xtask address-vectors --verify` | Confirms canonical generator and committed JSON agree. |
| SDK parity check | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | Run for every SDK reported in alerts/tickets. |
| Clipboard/IME sanity | `iroha tools address inspect <literal>` | Detects hidden characters or IME rewrites; cite `address_display_guidelines.md`. |

## Immediate response

1. Acknowledge the alert, link Grafana snapshots + PromQL output in the incident
   thread, and note affected Torii contexts.
2. Freeze manifest promotions / SDK releases touching address parsing.
3. Save dashboard snapshots and the generated Prometheus textfile artefacts in
   the incident folder (`docs/source/sns/incidents/YYYY-MM/<ticket>/`).
4. Pull log samples showing `checksum_mismatch` payloads.
5. Notify SDK owners (`#sdk-parity`) with sample payloads so they can triage.

## Root-cause isolation

### Fixture or generator drift

- Re-run `cargo xtask address-vectors --verify`; regenerate if it fails.
- Execute `ci/account_fixture_metrics.sh` (or individual
  `scripts/account_fixture_helper.py check`) for each SDK to confirm bundled
  fixtures match the canonical JSON.

### Client encoders / IME regressions

- Inspect user-provided literals via `iroha tools address inspect` to find zero-width
  joins, kana conversions, or truncated payloads.
- Cross-check wallet/explorer flows with
  `docs/source/sns/address_display_guidelines.md` (dual copy targets, warnings,
  QR helpers) to ensure they follow the approved UX.

### Manifest or registry issues

- Follow `address_manifest_ops.md` to re-validate the latest manifest bundle and
  ensure no Local-8 selectors resurfaced.
  appear in payloads.

### Malicious or malformed traffic

- Break down offending IPs/app IDs via Torii logs and `torii_http_requests_total`.
- Preserve at least 24 hours of logs for Security/Governance follow-up.

## Mitigation & recovery

| Scenario | Actions |
|----------|---------|
| Fixture drift | Regenerate `fixtures/account/address_vectors.json`, rerun `cargo xtask address-vectors --verify`, update SDK bundles, and attach `address_fixture.prom` snapshots to the ticket. |
| SDK/client regression | File issues referencing the canonical fixture + `iroha tools address inspect` output, and gate releases behind the SDK parity CI (e.g., `ci/check_address_normalize.sh`). |
| Malicious submissions | Rate-limit or block offending principals, escalate to Governance if tombstoning selectors is required. |

Once mitigations land, rerun the PromQL query above to confirm
`ERR_CHECKSUM_MISMATCH` stays at zero (excluding `/tests/*`) for at least
30 minutes before downgrading the incident.

## Closure

1. Archive Grafana snapshots, PromQL CSV, log excerpts, and `address_fixture.prom`.
2. Update `status.md` (ADDR section) plus the roadmap row if tooling/docs
   changed.
3. File post-incident notes under `docs/source/sns/incidents/` when new lessons
   emerge.
4. Ensure SDK release notes mention checksum fixes when applicable.
5. Confirm the alert stays green for 24h and fixture checks remain green before
   resolving.
