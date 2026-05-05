---
lang: es
direction: ltr
source: docs/source/sns/address_checksum_failure_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d6d22baeaece2339b216260f9b963181025b4c96e3a2618ac132ddd867fa3a48
source_last_modified: "2026-01-28T17:58:57.293249+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Account Address Checksum Incident Runbook (ADDR-7)

Status: Adopted 2026-04-23  
Owners: Torii Platform Team / SRE / Wallet & Explorer Support  
Roadmap link: **ADDR-6/ADDR-7 — UX & Security acceptance criteria**

This runbook documents the operational response when Torii, SDKs, or wallet
surfaces report i105 checksum failures (`ERR_CHECKSUM_MISMATCH` /
`ChecksumMismatch`). It complements the security analysis in
[`address_security_review.md`](./address_security_review.md), the UX guidance in
[`address_display_guidelines.md`](./address_display_guidelines.md), and the
manifest procedures captured in
[`address_manifest_ops.md`](../runbooks/address_manifest_ops.md).

## 1. When to run this play

Trigger this runbook whenever one or more of the following occurs:

1. **Alerting:** `AddressInvalidRatioSlo` (from
   `dashboards/alerts/address_ingest_rules.yml`) fires with
   `torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}` appearing in the
   annotations or in Grafana
   `address_ingest.json` panels.
2. **Fixture guardrails:** `ci/account_fixture_metrics.sh` or the nightly
   `account_address_fixture_status.json` dashboard reports a discrepancy for any
   target labelled `ChecksumMismatch`.
3. **Support/escalations:** Wallet, Explorer, or SDK teams file a ticket citing
   `ERR_CHECKSUM_MISMATCH`, `ChecksumMismatch`, or IME/NFC-related checksum
   drift in customer environments.
4. **Manual discovery:** Torii logs show `address_parse_error=checksum_mismatch`
   (see `crates/iroha_torii/src/utils.rs:439`), especially when impact spans
   multiple endpoints or dataspace contexts.

If the incident is scoped to Local‑8 or Local‑12 digest collisions, refer to
the `AddressLocal8Resurgence` or `AddressLocal12Collision` playbooks; otherwise
follow the steps below.

## 2. Observability & evidence checklist

| Evidence | Location / Command | Notes |
|----------|-------------------|-------|
| Grafana snapshot | `dashboards/grafana/address_ingest.json` | Capture the invalid reason breakdown, endpoint panels, and the raw `torii_address_invalid_total` graph filtered by `reason="ERR_CHECKSUM_MISMATCH"`. |
| Alert context | `dashboards/alerts/address_ingest_rules.yml` | Include the firing alert payload (context label) and any linked PagerDuty/Slack ack. |
| Fixture health | `dashboards/grafana/account_address_fixture_status.json` + `artifacts/account_fixture/address_fixture.prom` | Confirms whether SDK copies drifted from `fixtures/account/address_vectors.json`. |
| PromQL query | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | Identifies the Torii endpoint causing the spike; export CSV for tickets. |
| Logs | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` | Collect at least 5 sample payloads (redacting PII) per context. |
| Fixture verification | `cargo xtask address-vectors --verify` | Ensures canonical fixture matches committed JSON. |
| SDK parity check | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | Run for each SDK copy referenced in alerts/tickets. |
| Clipboard/IME sanity | `iroha tools address inspect <literal>` (documented in `address_display_guidelines.md`) | Confirms problematic strings were not mangled before reaching Torii. |

## 3. Immediate response steps

1. **Acknowledge the alert** in PagerDuty/Slack and post the Grafana link +
   PromQL output to the incident thread. Note affected `context` labels (e.g.,
   `/v1/accounts/{account_id}/transactions`).
2. **Freeze risky deploys:** pause manifest promotions and SDK releases touching
   address parsing until the root cause is understood.
3. **Collect telemetry snapshots:** download PNG/JSON snapshots from
   `address_ingest.json` and `account_address_fixture_status.json`. Save them in
   the incident folder (e.g., `docs/source/sns/incidents/YYYY-MM/<ticket>/`).
4. **Pull log samples:** use `journalctl` (or your log aggregation tool) to grab
   representative `checksum_mismatch` entries, redacting addresses before
   sharing externally.
5. **Notify owning SDKs:** tag the JS/Android/Swift/Python leads in the shared
   `#sdk-parity` channel with the incident link and sample payloads so they can
   triage client regressions quickly.

## 4. Root-cause isolation

### 4.1 Fixture or generator drift

- Run `cargo xtask address-vectors --verify` to confirm the committed fixture
  matches the generator. If it fails, regenerate and treat as a regression in
  the data-model pipeline.
- For each SDK referenced in alerts, run `scripts/account_fixture_helper.py`
  (`fetch` + `check`) or the automation wrapper
  `ci/account_fixture_metrics.sh --target <label=path>` to confirm the shipped
  fixture is in sync. Attach the generated `address_fixture.prom` to the ticket.

### 4.2 Client-side encoding/IME regressions

- Use `iroha tools address inspect <literal>` to reconstruct the canonical payload and
  detect hidden characters. Cross-check with `address_display_guidelines.md`
  (copy helpers, NFC handling, IME/Kana fuzz notes) and ensure affected clients
  follow the documented dual-format display requirements.
- If the incident stems from clipboard/camera ingestion, verify that wallets
  still use the shared QR helper endpoints documented in the ADDR‑6 roadmap
  entries.

### 4.3 Manifest or registry corruption

- If the checksum mismatch only affects a single suffix/domain, inspect the
  latest manifest bundle following the
  [`address_manifest_ops.md`](../runbooks/address_manifest_ops.md) runbook.
- Use `scripts/address_local_toolkit.sh audit --input <file>` (doc:
  `docs/source/sns/local_to_global_toolkit.md`) to confirm no Local-8 entries or
  truncated digests were reintroduced.

### 4.4 Malicious or malformed traffic

- Check whether requests originate from a single IP/guard; correlate with
  `torii_http_requests_total` to decide if rate-limits or bans are required.
- Preserve at least 24 hours of logs when handing the incident to Security or
  Governance, noting whether the payloads targeted production endpoints.

## 5. Mitigation & recovery

| Scenario | Actions |
|----------|---------|
| Fixture drift | Regenerate `fixtures/account/address_vectors.json`, land the update with `cargo xtask address-vectors --verify`, run `ci/account_fixture_metrics.sh`, and coordinate SDK releases so each language vendor consumes the refreshed bundle. Record the change in `status.md` and the governance tracker. |
| SDK/client regression | File bugs against the offending SDK, link to the canonical fixtures, and provide the `iroha tools address inspect` output. Gate releases via `ci/check_address_normalize.sh` or the SDK’s existing parity jobs until the fix lands. |
| Malicious submissions | Apply Torii-level filters (e.g., block offending JWT/app IDs), throttle via the gateway, and log the abusive addresses in the governance incident tracker so any corresponding Local selectors can be tombstoned if necessary. |

After remediation, rerun the PromQL query for `ERR_CHECKSUM_MISMATCH` to confirm
a sustained zero rate over a 30-minute window and verify the alert resets.

## 6. Closure checklist

1. Attach Grafana snapshots, PromQL CSV, `address_fixture.prom`, and relevant log
   excerpts to the incident record.
2. Update `status.md` (ADDR section) and the roadmap row if the incident
   resulted in tooling or doc changes.
3. File a short post-incident note under `docs/source/sns/incidents/` (if the
   incident exposed new gaps) and link it from the governance tracker.
4. If SDK fixes were required, ensure their release notes mention the checksum
   incident and capture proof (commit hash + CI link).
5. Close the alert only after `torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}`
   stays zero (outside `/tests/*`) for 24 hours and all fixture checks are
   green.

Following this runbook keeps ADDR‑6/ADDR‑7 acceptance criteria satisfied by
ensuring checksum failures are detected, triaged, and documented with clear,
repeatable evidence.
