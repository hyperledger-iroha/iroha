## Account Address Compliance Status (ADDR-2)

Status: Accepted 2026-03-30  
Owners: Data Model Team / QA Guild  
Roadmap reference: ADDR-2 — Canonical I105 Compliance Suite

### 1. Overview

- Fixture: `fixtures/account/address_vectors.json` (canonical I105 +
  multisig positive/negative cases).
- Scope: deterministic V1 payloads covering canonical account-id rendering,
  multisig controllers, and explicit rejection of non-canonical account-id
  forms.
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
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` | Parses the JSON, reconstructs canonical payloads, and checks canonical I105 plus canonical-hex conversions + structured errors. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | Validates server-side codecs so Torii refuses malformed i105 payloads deterministically. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | Mirrors V1 fixtures (i105/fullwidth) and asserts Norito-style error codes for every negative case. |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | Exercises i105 decoding, multisig payloads, and error surfacing on Apple platforms. |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | Ensures Kotlin/Java bindings stay aligned with the canonical fixture. |

### 4. Monitoring & Outstanding Work

- Status reporting: this document is linked from `status.md` and the roadmap so weekly reviews can verify fixture health.
- Developer portal summary: see **Reference → Account address compliance** in the docs portal (`docs/portal/docs/reference/account-address-status.md`) for the externally-facing synopsis.
- Prometheus and dashboards: whenever you verify an SDK copy, run the helper with `--metrics-out` (and optionally `--metrics-label`) so the Prometheus textfile collector can ingest `account_address_fixture_check_status{target=…}`. Grafana dashboard **Account Address Fixture Status** (`dashboards/grafana/account_address_fixture_status.json`) renders pass/fail counts per surface and surfaces the canonical SHA-256 digest for audit evidence. Alert when any target reports `0`.
- Torii metrics: monitor canonical i105 parse success/failure counters and
  alias-resolution counters together so SRE reviews can prove that account-id
  traffic stays canonical while alias traffic remains explicit.
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
