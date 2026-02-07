---
lang: my
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c2cbb5e965350648a30607bbd0f1588212ee0021b412ec55654993c18cc198e
source_last_modified: "2026-01-28T17:11:30.739162+00:00"
translation_last_reviewed: 2026-02-07
---

## Account Address Compliance Status (ADDR-2)

Status: Accepted 2026-03-30  
Owners: Data Model Team / QA Guild  
Roadmap reference: ADDR-2 — Dual-Format Compliance Suite

### 1. Overview

- Fixture: `fixtures/account/address_vectors.json` (IH58 (preferred) + compressed (`sora`, second-best) + multisig positive/negative cases).
- Scope: deterministic V1 payloads spanning implicit-default, Local-12, Global registry, and multisig controllers with full error taxonomy.
- Distribution: shared across Rust data-model, Torii, JS/TS, Swift, and Android SDKs; CI fails if any consumer deviates.
- Source of truth: the generator lives in `crates/iroha_data_model/src/account/address/compliance_vectors.rs` and is exposed via `cargo xtask address-vectors`.
### 2. Regeneration & Verification

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

Flags:

- `--out <path>` — optional override when producing ad-hoc bundles (defaults to `fixtures/account/address_vectors.json`).
- `--stdout` — emit JSON to stdout instead of writing to disk.
- `--verify` — compare the current file against freshly generated content (fails fast on drift; cannot be used with `--stdout`).

### 3. Artefact Matrix

| Surface | Enforcement | Notes |
|---------|-------------|-------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` | Parses the JSON, reconstructs canonical payloads, and checks IH58 (preferred)/compressed (`sora`, second-best)/canonical conversions + structured errors. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | Validates server-side codecs so Torii refuses malformed IH58 (preferred)/compressed (`sora`, second-best) payloads deterministically. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | Mirrors V1 fixtures (IH58 preferred/compressed (`sora`) second-best/fullwidth) and asserts Norito-style error codes for every negative case. |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | Exercises IH58 (preferred)/compressed (`sora`, second-best) decoding, multisig payloads, and error surfacing on Apple platforms. |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | Ensures Kotlin/Java bindings stay aligned with the canonical fixture. |

### 4. Monitoring & Outstanding Work

- Status reporting: this document is linked from `status.md` and the roadmap so weekly reviews can verify fixture health.
- Developer portal summary: see **Reference → Account address compliance** in the docs portal (`docs/portal/docs/reference/account-address-status.md`) for the externally-facing synopsis.
- Prometheus and dashboards: whenever you verify an SDK copy, run the helper with `--metrics-out` (and optionally `--metrics-label`) so the Prometheus textfile collector can ingest `account_address_fixture_check_status{target=…}`. Grafana dashboard **Account Address Fixture Status** (`dashboards/grafana/account_address_fixture_status.json`) renders pass/fail counts per surface and surfaces the canonical SHA-256 digest for audit evidence. Alert when any target reports `0`.
- Torii metrics: `torii_address_domain_total{endpoint,domain_kind}` now emits for every successfully parsed account literal, mirroring `torii_address_invalid_total`/`torii_address_local8_total`. Alert on any `domain_kind="local12"` traffic in production and mirror the counters into the SRE `address_ingest` dashboard so the Local-12 retirement gate has auditable evidence.
- Fixture helper: `scripts/account_fixture_helper.py` downloads or verifies the canonical JSON so SDK release automation can fetch/check the bundle without manual copy/paste while optionally writing Prometheus metrics. Example:

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \\
    --target path/to/sdk/address_vectors.json \\
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \\
    --metrics-label android
  ```

  The helper writes `account_address_fixture_check_status{target="android"} 1` when the target matches, plus `account_address_fixture_remote_info` / `account_address_fixture_local_info` gauges that expose SHA-256 digests. Missing files report `account_address_fixture_local_missing`.
  Automation wrapper: call `ci/account_fixture_metrics.sh` from cron/CI to emit a consolidated textfile (default `artifacts/account_fixture/address_fixture.prom`). Pass repeated `--target label=path` entries (optionally append `::https://mirror/...` per target to override the source) so Prometheus scrapes one file covering every SDK/CLI copy. The GitHub workflow `address-vectors-verify.yml` already runs this helper against the canonical fixture and uploads the `account-address-fixture-metrics` artifact for SRE ingestion.
