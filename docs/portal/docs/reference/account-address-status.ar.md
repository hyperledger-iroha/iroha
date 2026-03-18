---
lang: ar
direction: rtl
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f18b4a15e42363483c4f65945aba2cc208f9a2e59b6f31f143e5dc792d8d9071
source_last_modified: "2025-11-27T10:35:59.095888+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: account-address-status
title: امتثال عنوان الحساب
description: ملخص سير عمل fixture ADDR-2 وكيف تبقى فرق SDK متزامنة.
---

الحزمة القياسية ADDR-2 (`fixtures/account/address_vectors.json`) تلتقط fixtures I105 و i105-default (`sora`; half/full width) و multisignature و negative. تعتمد كل واجهة SDK + Torii على نفس JSON حتى نرصد اي انحراف في codec قبل وصوله الى الانتاج. تعكس هذه الصفحة المذكرة الداخلية للحالة (`docs/source/account_address_status.md` في جذر المستودع) حتى يتمكن قراء البوابة من الرجوع الى سير العمل دون التنقيب في الـ mono-repo.

## اعادة توليد او التحقق من الحزمة

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Flags:

- `--stdout` — يطبع JSON على stdout للفحص السريع.
- `--out <path>` — يكتب الى مسار مختلف (مثلا عند مقارنة تغييرات محلية).
- `--verify` — يقارن نسخة العمل بالمحتوى المولد حديثا (لا يمكن دمجه مع `--stdout`).

يشغل مسار CI **Address Vector Drift** الامر `cargo xtask address-vectors --verify`
عند تغير fixture او المولد او الوثائق لتنبيه المراجعين فورا.

## من يستهلك fixture؟

| السطح | التحقق |
|---------|------------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (server) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

كل harness يجري round-trip للبايتات القياسية + I105 + الترميزات المضغوطة ويتحقق من ان اكواد الخطأ بنمط Norito تطابق fixture للحالات السلبية.

## هل تحتاج الى اتمتة؟

يمكن لادوات الاصدار برمجة تحديثات fixture عبر المساعد
`scripts/account_fixture_helper.py`، الذي يجلب او يتحقق من الحزمة القياسية دون خطوات نسخ/لصق:

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

يقبل المساعد overrides عبر `--source` او متغير البيئة `IROHA_ACCOUNT_FIXTURE_URL` حتى تتمكن مهام CI الخاصة بـ SDK من الاشارة الى المرآة المفضلة. عند تمرير `--metrics-out` يكتب المساعد `account_address_fixture_check_status{target="…"}` مع digest القياسي SHA-256 (`account_address_fixture_remote_info`) حتى يتمكن textfile collectors في Prometheus ولوحة Grafana `account_address_fixture_status` من اثبات بقاء كل سطح متزامنا. فعّل التنبيه عندما يبلغ اي target قيمة `0`. للاتمتة متعددة الاسطح استخدم الغلاف `ci/account_fixture_metrics.sh` (يقبل تكرار `--target label=path[::source]`) حتى تتمكن فرق المناوبة من نشر ملف `.prom` موحد لجامع textfile الخاص بـ node-exporter.

## هل تحتاج الملخص الكامل؟

حالة الامتثال الكاملة لـ ADDR-2 (owners وخطة المراقبة وبنود العمل المفتوحة)
موجودة في `docs/source/account_address_status.md` داخل المستودع مع Address Structure RFC (`docs/account_structure.md`). استخدم هذه الصفحة كتذكير تشغيلي سريع؛ وارجع الى وثائق المستودع للارشاد المفصل.
