---
lang: zh-hans
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b92bdfc323a4bc031ca7f2237f238d5d515f7238791a6ec9c50b55e361c85560
source_last_modified: "2026-01-28T17:11:30.639071+00:00"
translation_last_reviewed: 2026-02-07
id: account-address-status
title: Account address compliance
description: Summary of the ADDR-2 fixture workflow and how SDK teams stay in sync.
---

The canonical ADDR-2 bundle (`fixtures/account/address_vectors.json`) captures
IH58 (preferred), compressed (`sora`, second-best; half/full width), multisignature, and negative fixtures.
Every SDK + Torii surface relies on the same JSON so we can detect any codec
drift before it hits production. This page mirrors the internal status brief
(`docs/source/account_address_status.md` in the root repository) so portal
readers can reference the workflow without digging through the mono-repo.

## Regenerate or verify the bundle

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Flags:

- `--stdout` — emit the JSON to stdout for ad-hoc inspection.
- `--out <path>` — write to a different path (e.g., when diffing changes locally).
- `--verify` — compare the working copy against freshly generated content (cannot
  be combined with `--stdout`).

The CI workflow **Address Vector Drift** runs `cargo xtask address-vectors --verify`
any time the fixture, generator, or docs change to alert reviewers immediately.

## Who consumes the fixture?

| Surface | Validation |
|---------|------------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (server) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Each harness round-trips canonical bytes + IH58 + compressed (`sora`, second-best) encodings and
checks that Norito-style error codes line up with the fixture for negative cases.

## Need automation?

Release tooling can script fixture refreshes with the helper
`scripts/account_fixture_helper.py`, which fetches or verifies the canonical
bundle without copy/paste steps:

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

The helper accepts `--source` overrides or the `IROHA_ACCOUNT_FIXTURE_URL`
environment variable so SDK CI jobs can point at their preferred mirror.
When `--metrics-out` is supplied the helper writes
`account_address_fixture_check_status{target=\"…\"}` along with the canonical
SHA-256 digest (`account_address_fixture_remote_info`) so Prometheus textfile
collectors and Grafana dashboard `account_address_fixture_status` can prove
every surface remains in sync. Alert whenever a target reports `0`. For
multi-surface automation use the wrapper `ci/account_fixture_metrics.sh`
(accepts repeated `--target label=path[::source]`) so on-call teams can publish
one consolidated `.prom` file for the node-exporter textfile collector.

## Need the full brief?

The full ADDR-2 compliance status (owners, monitoring plan, open action items)
lives in `docs/source/account_address_status.md` within the repository along
with the Address Structure RFC (`docs/account_structure.md`). Use this page as a
quick operational reminder; defer to the repo docs for in-depth guidance.
