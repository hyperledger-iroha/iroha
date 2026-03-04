---
lang: kk
direction: ltr
source: docs/source/sns/address_security_review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 292fc6481c18ffc4232f633a29c9070a0a70cf6b6f453c650daec447ceb1cd22
source_last_modified: "2026-01-28T17:11:30.737932+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Sora Address Security Review (ADDR-7)

Roadmap link: **ADDR-7 — Security Review & Collision Monitoring**

This note captures the security review requested in roadmap item ADDR-7:

- quantify the collision probability for the Local digest selectors and
  document why Local‑12 (12-byte digest) is acceptable while Local‑8 is not;
- surface the telemetry/alerting hooks that prove Local‑8 usage has dropped to
  zero before Local-8 enforcement is activated;
- call out the checksum, kana/IME safeguards, and manifest immutability
  guarantees that prevent spoofing; and
- provide an operator checklist that ties the above controls into the existing
  toolchain.

## 1. Local digest resilience

### 1.1 Derivation summary

- The Local selector is derived by computing a keyed Blake2s MAC over the
  canonical domain string using the static key
  `SORA-LOCAL-K:v1` (`crates/iroha_data_model/src/account/address.rs:102`).
- `compute_local_digest` truncates the Blake2s output to 12 bytes (96 bits) and
  embeds it into the Local selector (`AccountAddress::compute_local_digest`,
  `crates/iroha_data_model/src/account/address.rs:885`).
- The result is referred to as “Local‑12”. Older Local‑8 payloads truncated the
  digest to 8 bytes (64 bits) and are now rejected with
  `AccountAddressError::LocalDigestTooShort`
  (`crates/iroha_data_model/src/account/address.rs:1071`).

### 1.2 Collision budget

| Estimated addresses | Expected Local‑12 collisions | Expected Local‑8 collisions |
|---------------------|------------------------------|-----------------------------|
| 10⁶ (large testnet) | ~6.3 × 10⁻¹⁸ | ~2.7 × 10⁻⁸ |
| 10⁷ (regional rollout) | ~6.3 × 10⁻¹⁶ | ~2.7 × 10⁻⁶ |
| 10⁸ (global retail) | ~6.3 × 10⁻¹⁴ | ~2.7 × 10⁻⁴ |
| 10⁹ (multi-network aggregate) | ~6.3 × 10⁻¹² | ~2.7 × 10⁻² |

The values use the birthday approximation `n(n-1)/(2·2ᵇ)` with `b = 96` or `64`.
At global scale the Local‑12 digest still yields a <10⁻¹¹ collision expectation,
while Local‑8 would have produced a measurable ~2.7 % collision probability.

### 1.3 Regression artefacts

- `fixtures/account/address_vectors.json` publishes canonical Local selectors
  (Local‑12), IH58, and compressed (`sora`, second-best) encodings. Regenerate via
  `cargo xtask address-vectors` to prove encoder determinism.
- `scripts/address_local_toolkit.sh` + `docs/source/sns/local_to_global_toolkit.md`

## 2. Telemetry & alerting

- Every Torii surface feeds `torii_address_invalid_total{context,reason}`,
  `torii_address_local8_total{context}`,
  `torii_address_collision_total{context,kind="local12_digest"}`, and
  `torii_address_collision_domain_total{context,domain}` by incrementing
  the metrics inside `parse_account_literal`
  (`crates/iroha_torii/src/routing.rs:10105`,
  `crates/iroha_telemetry/src/metrics.rs:9352`).
- `cargo xtask address-local8-gate --input <prom-range.json> [--window-days 30] [--json-out <path>]`
  consumes the Prometheus range-query JSON for
  `torii_address_local8_total` and `torii_address_collision_total`, fails when
  any counter increases or the coverage window is shorter than the requested
  days, and emits an optional JSON report that can be attached to governance
  packets. Pair it with `promtool query range --output=json` or the Prometheus
  HTTP API to harvest the 30‑day window for production/staging clusters.
- `dashboards/grafana/address_ingest.json` charts both counters and tags the
  Local‑8 series with the same `context` label so operators can prove a
  sustained zero rate ahead of Local-8/Local-12 enforcement gates.
- `dashboards/alerts/address_ingest_rules.yml` asserts that both
  `torii_address_local8_total` and `torii_address_collision_total` stay zero
  outside the `/tests/*` contexts; the Alertmanager template links back to the
  Local→Global toolkit runbook.
- `ci/check_address_normalize.sh` and `ci/check_sorafs_orchestrator_adoption.sh`
  attach scoreboard summaries plus `torii_address_local8_total` snapshots to
  every release candidate, making Local‑8 regressions release blocking material.

## 3. Checksum, kana, and parser hardening

- IH58 literals use the `IH58` alphabet and the Bech32m checksum described in
  the Account Structure RFC (§2.2, `docs/account_structure.md:124`).
- The compressed `sora…` representation appends the half-width イロハ poem to
  the same alphabet (`docs/account_structure.md:125`) so IME/Kana inputs can be
  rendered deterministically across locales.
- All domain labels (for both Local selectors and Global registry entries) run
  through Norm v1 (NFC + strict UTS‑46 + ASCII policy)
  (`docs/source/references/address_norm_v1.md:1`), preventing spoofing via
  confusables and mixed-normalization inputs.
- Wallet/explorer UX requirements in
  `docs/source/sns/address_display_guidelines.md:34` mandate the dual-format
  display (IH58 preferred + compressed (`sora`) second-best) plus localized copy helpers so operators can
  reconcile what users see with what Torii enforces.

## 4. Registry immutability & tombstones

- `NameRecordV1` stores both the `name_hash` selector and the
  `last_tx_hash` pointer (`docs/source/sns/registry_schema.md:47`), providing a
  deterministic audit trail across upgrades.
- Lifecycle state is tracked via `NameStatus`, including the explicit
  `Tombstoned(NameTombstoneStateV1)` variant that carries the governance reason
  (`docs/source/sns/registry_schema.md:71`).
- Every mutation emits a structured `RegistryEventV1`
  (`docs/source/sns/registry_schema.md:129`), so gateway caches and the DNS
  publishing pipeline can prove that sealed tombstones are never re-allocated.
- `docs/source/sns/governance_playbook.md` now lists this review as required
  evidence before activating Local-8/Local-12 enforcement or onboarding new suffixes.

## 5. Operator checklist

1. **Run the toolkit:** execute `scripts/address_local_toolkit.sh` (or the JS
   `audit.json`/`normalized.txt` pair referenced in the Local→Global runbook.
2. **Run the gate check:** export a 30‑day Prometheus range query for both
   `torii_address_local8_total` and `torii_address_collision_total`
   (`promtool query range --output=json ...`) and feed it to
   `cargo xtask address-local8-gate --input <file> --json-out artifacts/address_gate.json`.
   Attach the CLI output and JSON report to the readiness ticket; the gate must
   be clean (no increments, no counter resets, >=30 days observed) before
   Local-8 enforcement is activated in production.
3. **Attach telemetry:** capture the `address_ingest` dashboard panels for
   `torii_address_local8_total{context!~"/tests/.*"}`,
   `torii_address_collision_total{context!~"/tests/.*",kind="local12_digest"}`,
   and `torii_address_collision_domain_total{context!~"/tests/.*",domain=~"<target-domain>"}` plus the alert snapshot, proving 30 consecutive days of zero Local‑8
   detections and zero collisions scoped to production/staging domains.
4. **Document inputs:** include copies of the IH58 (preferred)/sora (second-best) checksum fixtures
   (`fixtures/account/address_vectors.json`) with the readiness ticket so IME
   behaviour can be reproduced during support escalations.
5. **Confirm registry guarantees:** export the name registry state (selected
   suffixes) via the registrar API and verify that tombstoned entries remain
   immutable in both storage and replay events.
6. **Record governance approval:** link this document and the captured artefacts
   in the SNS governance tracker so future audits can trace how Local‑8 was
   disabled and how address spoofing risks are mitigated.
