---
lang: ar
direction: rtl
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c1214f4d0ad86449c0ef4b8f8cbaa38fe265bab4afcc2930cd30a57c089e6d7
source_last_modified: "2025-11-15T05:05:33.914289+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/account_address_status.md -->

## حالة امتثال عنوان الحساب (ADDR-2)

الحالة: معتمد 2026-03-30  
المالكون: فريق نموذج البيانات / نقابة QA  
مرجع خارطة الطريق: ADDR-2 — Dual-Format Compliance Suite

### 1. نظرة عامة

- Fixture: `fixtures/account/address_vectors.json` (I105 + multisig حالات ايجابية/سلبية).
- النطاق: payloads V1 حتمية تغطي implicit-default و Local-12 و Global registry و multisig controllers مع تصنيف كامل للاخطاء.
- التوزيع: مشترك بين Rust data-model و Torii و SDKs لـ JS/TS و Swift و Android؛ يفشل CI اذا انحرف اي مستهلك.
- مصدر الحقيقة: المولد موجود في `crates/iroha_data_model/src/account/address/compliance_vectors.rs` ويكشف عبر `cargo xtask address-vectors`.

### 2. اعادة التوليد والتحقق

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

الخيارات:

- `--out <path>` — تجاوز اختياري عند انتاج bundles مخصصة (الافتراضي `fixtures/account/address_vectors.json`).
- `--stdout` — يخرج JSON الى stdout بدلا من الكتابة على القرص.
- `--verify` — يقارن الملف الحالي بالمحتوى المولد حديثا (يفشل بسرعة عند الانحراف؛ غير متوافق مع `--stdout`).

### 3. مصفوفة artefact

| السطح | الانفاذ | ملاحظات |
|---------|-------------|-------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` | يفك JSON، يعيد بناء payloads canonical، ويتحقق من تحويلات I105/canonical + اخطاء منظمة. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | يتحقق من codecs جهة الخادم لكي يرفض Torii payloads I105 غير الصحيحة بشكل حتمي. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | يعكس fixtures V1 (I105/fullwidth) ويتحقق من رموز الاخطاء بنمط Norito لكل حالة سلبية. |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | يمتحن فك ترميز I105 و multisig payloads واظهار الاخطاء على منصات Apple. |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | يضمن بقاء bindings Kotlin/Java متوافقة مع fixture canonical. |

### 4. المراقبة والعمل المتبقي

- تقارير الحالة: هذا المستند مرتبط من `status.md` و roadmap لكي تتمكن المراجعات الاسبوعية من التحقق من صحة fixture.
- ملخص بوابة المطورين: راجع **Reference -> Account address compliance** في بوابة المستندات (`docs/portal/docs/reference/account-address-status.md`).
- Prometheus واللوحات: عند التحقق من نسخة SDK، شغل helper مع `--metrics-out` (وبشكل اختياري `--metrics-label`) كي يلتقط textfile collector في Prometheus `account_address_fixture_check_status{target=...}`. لوحة Grafana **Account Address Fixture Status** (`dashboards/grafana/account_address_fixture_status.json`) تعرض عدادات pass/fail لكل سطح وتعرض digest SHA-256 الكنسي كدليل تدقيق. نبه عند اي target يبلغ `0`.
- Torii metrics: `torii_address_domain_total{endpoint,domain_kind}` يصدر الان لكل account literal تم تحليله بنجاح، مع مرآة `torii_address_invalid_total`/`torii_address_local8_total`. نبه على اي حركة مرور `domain_kind="local12"` في الانتاج وكرر العدادات في لوحة SRE `address_ingest` كي يكون لتقاعد Local-12 دليل تدقيق.
- Fixture helper: `scripts/account_fixture_helper.py` يقوم بتنزيل او تحقق من JSON الكنسي لكي تتمكن اتمتة اصدار SDK من جلب/فحص bundle دون نسخ يدوي، مع كتابة مقاييس Prometheus اختياريا. مثال:

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

  يكتب helper `account_address_fixture_check_status{target="android"} 1` عندما يتطابق target، بالاضافة الى gauges `account_address_fixture_remote_info` / `account_address_fixture_local_info` التي تعرض digests SHA-256. الملفات المفقودة تبلغ `account_address_fixture_local_missing`.
  Automation wrapper: استدع `ci/account_fixture_metrics.sh` من cron/CI لاصدار textfile موحد (الافتراضي `artifacts/account_fixture/address_fixture.prom`). مرر ادخالات متكررة `--target label=path` (مع امكانية اضافة `::https://mirror/...` لكل target لتجاوز المصدر) كي يقوم Prometheus بسحب ملف واحد يغطي كل نسخ SDK/CLI. مسار GitHub `address-vectors-verify.yml` يشغل هذا helper بالفعل مقابل fixture الكنسي ويرفع artifact `account-address-fixture-metrics` لاستهلاك SRE.

</div>
