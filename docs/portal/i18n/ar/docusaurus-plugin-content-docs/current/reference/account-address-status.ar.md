---
lang: ar
direction: rtl
source: docs/portal/docs/reference/account-address-status.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حالة عنوان الحساب
العنوان: لا عنوان الحساب
الوصف: ملخص سير عمل تركيبات ADDR-2 والبقاء فرق SDK متزامنة.
---

الحزمة القياسية ADDR-2 (`fixtures/account/address_vectors.json`) تركيبات الالتقاط IH58 والمضغوطة (`sora`، ثاني أفضل؛ نصف/عرض كامل) والتوقيعات المتعددة والسلبية. تعتمد كل واجهة SDK + Torii على نفس JSON حتى نرصد اي انحراف في برنامج الترميز قبل وصوله الى الانتاج. احترام هذه الصفحة المذكرة الداخلية للحالة (`docs/source/account_address_status.md` في جذر المستودع) حتى يتمكن قراء البوابة من الرجوع الى سير العمل دون التنقيب في الـ mono-repo.

## إعادة إنشاء او التحقق من الحزمة

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

الأعلام:

- `--stdout` — يطبع JSON على stdout للفحص السريع.
- `--out <path>` — كاتب الى مسار مختلف (مثلا عند مقارنة التطورات المحلية).
- `--verify` — يقارن العمل بالمحتوى المولد حديثا (لا يمكن دمجه مع `--stdout`).

أشعر بمسار CI **Address Vector Drift** الأمر `cargo xtask address-vectors --verify`
عند تغير المباراة او المولد او الوثائق لتنبيه المراجعين فورا.

## من يستهلك التركيبات؟

| السطح | التحقق |
|---------|-----------|
| نموذج بيانات الصدأ | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (الخادم) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| جافا سكريبت SDK | `javascript/iroha_js/test/address.test.js` |
| سويفت SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| أندرويد SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

كل تسخير آيجي ذهابًا وإيابًا للبايتات الممتازة + IH58 + الترميزات الأصلية ويتحقق من ان اكواد خطأ بنمط Norito تطابق التركيبات للحالات التجارية.

## هل تحتاج الى التمتعة؟يمكن تنزيل برامج تحديث تحديثات التثبيت عبر المساعد
`scripts/account_fixture_helper.py`، الذي يأتي او يحقق من الحزمة القياسية دون خطوات النسخة/لصق:

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

وتقبل المساعدة overrides عبر `--source` او متغيرة البيئة `IROHA_ACCOUNT_FIXTURE_URL` حتى للمبتدئين مهام CI الخاصة بـ SDK من الاشارة الى الترحيب بالحب. عند تسارع `--metrics-out` الكاتب المساعد `account_address_fixture_check_status{target="…"}` مع الملخص القياسي SHA-256 (`account_address_fixture_remote_info`) حتى يبدأ جامعي الملفات النصية في Prometheus ولوحة Grafana `account_address_fixture_status` من اثبات البقاء كل سطح متزامنا. فعّل التنبيه عندما يتعلق الأمر بقيمة الهدف `0`. للتمتع بميزات متعددة الاسطح مستخدم اللعب `ci/account_fixture_metrics.sh` (يقبل `--target label=path[::source]`) حتى بدء تشغيل المناوبة من نشر ملف `.prom` المتوافق مع الجامع textfile الخاص بـnode-exporter.

## هل تحتاج الملخص الكامل؟

لـ ADDR-2 (أصحاب القصة الكاملة للمحاكمة وبنود العمل المفتوح)
موجود في `docs/source/account_address_status.md` داخل المستودع مع هيكل العنوان RFC (`docs/account_structure.md`). استخدم هذه الصفحة كتذكيري سريعًا؛ وارجع الى مستودع الوثائق للاتجاه التطوري.