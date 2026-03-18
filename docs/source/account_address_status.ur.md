---
lang: ur
direction: rtl
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c1214f4d0ad86449c0ef4b8f8cbaa38fe265bab4afcc2930cd30a57c089e6d7
source_last_modified: "2025-11-15T05:05:33.914289+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ: docs/source/account_address_status.md -->

## اکاؤنٹ ایڈریس کمپلائنس اسٹیٹس (ADDR-2)

اسٹیٹس: منظور شدہ 2026-03-30  
مالکان: ڈیٹا ماڈل ٹیم / QA گلڈ  
روڈمیپ حوالہ: ADDR-2 — Dual-Format Compliance Suite

### 1. جائزہ

- Fixture: `fixtures/account/address_vectors.json` (I105 + multisig مثبت/منفی کیسز).
- دائرہ کار: implicit-default، Local-12، Global registry، اور multisig controllers پر مشتمل حتمی V1 payloads اور مکمل error taxonomy.
- تقسیم: Rust data-model، Torii، JS/TS، Swift، اور Android SDKs میں مشترک؛ اگر کوئی consumer deviate کرے تو CI fail ہو جاتا ہے.
- حقیقی ماخذ: generator `crates/iroha_data_model/src/account/address/compliance_vectors.rs` میں ہے اور `cargo xtask address-vectors` کے ذریعے دستیاب ہے.

### 2. دوبارہ تخلیق اور تصدیق

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

فلیگز:

- `--out <path>` — ad-hoc bundles بنانے پر اختیاری override (ڈیفالٹ `fixtures/account/address_vectors.json`).
- `--stdout` — JSON کو stdout پر output کرتا ہے بجائے ڈسک پر لکھنے کے.
- `--verify` — موجودہ فائل کو تازہ بنائے گئے مواد سے compare کرتا ہے (drift پر فوری fail؛ `--stdout` کے ساتھ combine نہیں ہو سکتا).

### 3. آرٹیفیکٹ میٹرکس

| سطح | نفاذ | نوٹس |
|---------|-------------|-------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` | JSON parse کرتا ہے، canonical payloads دوبارہ بناتا ہے، اور I105/canonical conversions + structured errors کو check کرتا ہے. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | server-side codecs validate کرتا ہے تاکہ Torii خراب I105 payloads کو deterministically reject کرے. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | V1 fixtures (I105/fullwidth) کو mirror کرتا ہے اور ہر منفی کیس کیلئے Norito طرز error codes کی تصدیق کرتا ہے. |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | I105 decoding، multisig payloads، اور Apple پلیٹ فارمز پر error surfacing کو exercise کرتا ہے. |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | یقینی بناتا ہے کہ Kotlin/Java bindings canonical fixture کے ساتھ aligned رہیں. |

### 4. نگرانی اور باقی کام

- اسٹیٹس رپورٹنگ: یہ دستاویز `status.md` اور roadmap سے منسلک ہے تاکہ ہفتہ وار جائزے fixture کی صحت کی تصدیق کر سکیں.
- ڈویلپر پورٹل خلاصہ: بیرونی خلاصے کیلئے docs پورٹل میں **Reference -> Account address compliance** دیکھیں (`docs/portal/docs/reference/account-address-status.md`).
- Prometheus اور ڈیش بورڈز: جب بھی آپ SDK کی کاپی verify کریں، helper کو `--metrics-out` (اور اختیاری `--metrics-label`) کے ساتھ چلائیں تاکہ Prometheus textfile collector `account_address_fixture_check_status{target=...}` ingest کر سکے. Grafana ڈیش بورڈ **Account Address Fixture Status** (`dashboards/grafana/account_address_fixture_status.json`) ہر سطح کے pass/fail counts دکھاتا ہے اور audit evidence کیلئے canonical SHA-256 digest سامنے لاتا ہے. اگر کوئی target `0` رپورٹ کرے تو alert کریں.
- Torii metrics: `torii_address_domain_total{endpoint,domain_kind}` اب ہر کامیاب account literal parsing کیلئے emit ہوتا ہے، اور `torii_address_invalid_total`/`torii_address_local8_total` کو mirror کرتا ہے. پروڈکشن میں `domain_kind="local12"` ٹریفک پر alert کریں اور counters کو SRE `address_ingest` ڈیش بورڈ میں mirror کریں تاکہ Local-12 retirement کیلئے audit evidence موجود ہو.
- Fixture helper: `scripts/account_fixture_helper.py` canonical JSON کو download یا verify کرتا ہے تاکہ SDK release automation manual copy/paste کے بغیر bundle حاصل/چیک کر سکے، اور اختیاری طور پر Prometheus metrics لکھ سکے. مثال:

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \
    --target path/to/sdk/address_vectors.json \
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
    --metrics-label android
  ```

  helper `account_address_fixture_check_status{target="android"} 1` لکھتا ہے جب target match ہو، اور ساتھ ہی `account_address_fixture_remote_info` / `account_address_fixture_local_info` gauges میں SHA-256 digests بھی دیتا ہے. اگر فائل موجود نہ ہو تو `account_address_fixture_local_missing` رپورٹ ہوتا ہے.
  Automation wrapper: `ci/account_fixture_metrics.sh` کو cron/CI سے چلائیں تاکہ consolidated textfile (ڈیفالٹ `artifacts/account_fixture/address_fixture.prom`) بن سکے. `--target label=path` کی repeated entries دیں (اور ہر target کیلئے `::https://mirror/...` optionally شامل کر کے source override کریں) تاکہ Prometheus ایک ہی فائل سے ہر SDK/CLI copy scrape کر سکے. GitHub workflow `address-vectors-verify.yml` پہلے ہی اس helper کو canonical fixture پر چلاتا ہے اور SRE ingestion کیلئے `account-address-fixture-metrics` artifact upload کرتا ہے.

</div>
