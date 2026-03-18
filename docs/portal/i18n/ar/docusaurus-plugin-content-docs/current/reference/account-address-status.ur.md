---
lang: ar
direction: rtl
source: docs/portal/docs/reference/account-address-status.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حالة عنوان الحساب
العنوان: اون لاين ايريس تاميل
الوصف: تعمل أداة ADDR-2 الثابتة على الخلاصة وSDK على حل المشكلة.
---

حزمة ADDR-2 الأساسية (`fixtures/account/address_vectors.json`) I105 (المفضل)، المضغوطة (`sora`، ثاني أفضل؛ نصف/عرض كامل)، التوقيعات المتعددة، والتركيبات السلبية التي تلتقط كرتا. تم تطوير SDK + Torii لسطح JSON لمنع الاختناق وإصدار برنامج الترميز الانجراف إلى أقصى حد. ملخص حالة صفحة الاندرويد (`docs/source/account_address_status.md` جهاز التوجيه) يعكس المفاتيح وقارئات البوابة التي يمكن من خلالها استخدام الريبو الأحادي لسير العمل بشكل رائع.

## الحزمة کو تجدد أو تحقق من کریں

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

الأعلام:

- `--stdout` — التفتيش المخصص لـ JSON كو stdout پر انبعاث کرتا ہے۔
- `--out <path>` — مسار مختلف لكل ھتا (مثل اختلاف لوکل وقت).
- `--verify` — نسخة عمل تعمل حاليًا على إنشاء محتوى كامل ومقارنة الكرتا (`--stdout` وهي تجمع بين لا شيء ولا سكتا).

سير عمل CI **انجراف ناقل العنوان** `cargo xtask address-vectors --verify`
هناك أيضًا جهاز تثبيت أو مولد أو مستندات للمراجعين المؤهلين الذين يقومون بتنبيه أي شخص على الفور.

## تركيبات کون استخدام کرتا ہے؟

| السطح | التحقق من الصحة |
|---------|-----------|
| نموذج بيانات الصدأ | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (الخادم) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| جافا سكريبت SDK | `javascript/iroha_js/test/address.test.js` |
| سويفت SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| أندرويد SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |يتم تسخير البايتات الأساسية + I105 + الترميزات المضغوطة (`sora`، ثاني أفضل) كرتا ذهابًا وإيابًا والحالات السلبية لرموز الخطأ على النمط Norito التي تتطابق مع الكرتا.

## الأتمتة چہئے؟

يتم تحديث أداة تثبيت الأدوات باستخدام `scripts/account_fixture_helper.py` من خلال البرنامج النصي، يمكنك النسخ/اللصق لجلب الحزمة الأساسية أو التحقق من صحة النص:

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

يقوم المساعد `--source` بتجاوز أو `IROHA_ACCOUNT_FIXTURE_URL` متغير البيئة قبول كرتا ووظائف SDK CI إعادة إنشاء مرآة رأسية. `--metrics-out` لديك جاتا ومساعد `account_address_fixture_check_status{target="…"}` الذي يحتوي على ملخص SHA-256 الكنسي (`account_address_fixture_remote_info`) ومجمعات الملفات النصية Prometheus Grafana لوحة المعلومات `account_address_fixture_status` سطح متزامن ثابت. في هذا الهدف `0` سجل التقرير لتنبيهك. أتمتة متعددة الأسطح مجمع الملفات `ci/account_fixture_metrics.sh` استخدام الكريں (`--target label=path[::source]` المتكرر قبول كرتا ہے) فرق عند الطلب عقدة مصدر مجمع ملفات نصية `.prom` فائل نشر كر سکیں.

##مختصر مکمل چاہئے؟

حالة الامتثال ADDR-2 (المالكون، خطة المراقبة، عناصر العمل المفتوحة) مكملة للتفاصيل
المسجل `docs/source/account_address_status.md` متصل ببنية العنوان RFC (`docs/account_structure.md`) موجود حاليًا. هذه الصفحة عبارة عن تذكير تشغيلي سريع للاستخدام؛ هناك المزيد من التفاصيل حول مستندات الريبو.