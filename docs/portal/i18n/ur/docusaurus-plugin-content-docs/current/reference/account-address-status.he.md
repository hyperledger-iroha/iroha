---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/reference/account-address-status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7818255af32ccfd6e46af4453ddcc998e315da8417da1a8b1fa7ee3ae5758beb
source_last_modified: "2025-11-14T04:43:20.947710+00:00"
translation_last_reviewed: 2026-01-30
---

canonical ADDR-2 bundle (`fixtures/account/address_vectors.json`) IH58 (preferred), compressed (`sora`, second-best; half/full width), multisignature, اور negative fixtures کو capture کرتا ہے۔ ہر SDK + Torii surface اسی JSON پر انحصار کرتی ہے تاکہ codec drift پروڈکشن تک پہنچنے سے پہلے پکڑا جا سکے۔ یہ صفحہ اندرونی status brief (`docs/source/account_address_status.md` ریپوزٹری روٹ میں) کو mirror کرتا ہے تاکہ portal readers بغیر mono-repo میں کھوج لگائے workflow دیکھ سکیں۔

## Bundle کو regenerate یا verify کریں

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Flags:

- `--stdout` — ad-hoc inspection کیلئے JSON کو stdout پر emit کرتا ہے۔
- `--out <path>` — مختلف path پر لکھتا ہے (مثلا لوکل diff کے وقت)۔
- `--verify` — working copy کو تازہ generate شدہ content کے ساتھ compare کرتا ہے (`--stdout` کے ساتھ combine نہیں ہو سکتا)۔

CI workflow **Address Vector Drift** `cargo xtask address-vectors --verify`
ہر بار چلاتا ہے جب fixture، generator، یا docs بدلیں تاکہ reviewers کو فوراً alert کیا جا سکے۔

## Fixture کون استعمال کرتا ہے؟

| Surface | Validation |
|---------|------------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (server) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

ہر harness canonical bytes + IH58 + compressed (`sora`, second-best) encodings کا round-trip کرتا ہے اور negative cases کیلئے Norito-style error codes کو fixture کے ساتھ match کرتا ہے۔

## Automation چاہئے؟

Release tooling fixture refreshes کو `scripts/account_fixture_helper.py` کے ذریعے script کر سکتی ہے، جو copy/paste کے بغیر canonical bundle fetch یا verify کرتا ہے:

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

Helper `--source` overrides یا `IROHA_ACCOUNT_FIXTURE_URL` environment variable قبول کرتا ہے تاکہ SDK CI jobs اپنے پسندیدہ mirror کی طرف اشارہ کر سکیں۔ جب `--metrics-out` دیا جاتا ہے تو helper `account_address_fixture_check_status{target="…"}` کے ساتھ canonical SHA-256 digest (`account_address_fixture_remote_info`) لکھتا ہے تاکہ Prometheus textfile collectors اور Grafana dashboard `account_address_fixture_status` ہر surface کی sync ثابت کر سکیں۔ جب کوئی target `0` رپورٹ کرے تو alert کریں۔ multi-surface automation کیلئے wrapper `ci/account_fixture_metrics.sh` استعمال کریں (repeatable `--target label=path[::source]` قبول کرتا ہے) تاکہ on-call teams node-exporter textfile collector کیلئے ایک consolidated `.prom` فائل publish کر سکیں۔

## مکمل brief چاہئے؟

ADDR-2 compliance status (owners، monitoring plan، open action items) کی مکمل تفصیل
ریپوزٹری میں `docs/source/account_address_status.md` کے ساتھ Address Structure RFC (`docs/account_structure.md`) میں موجود ہے۔ اس صفحہ کو quick operational reminder کے طور پر استعمال کریں؛ تفصیلی رہنمائی کیلئے repo docs دیکھیں۔
