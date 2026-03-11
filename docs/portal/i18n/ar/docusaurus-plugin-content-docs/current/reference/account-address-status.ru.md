---
lang: ar
direction: rtl
source: docs/portal/docs/reference/account-address-status.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حالة عنوان الحساب
العنوان: حساب العنوان المفضل
الوصف: معالجة المياه ADDR-2 لاعبا أساسيا ومزامنة الأوامر SDK.
---

تشتمل الحزمة الكنسي ADDR-2 (`fixtures/account/address_vectors.json`) على التركيبات I105 (المفضل)، المضغوطة (`sora`، ثاني أفضل؛ نصف/عرض كامل)، التوقيع المتعدد والسالب. يتم تشغيل كل من SDK + Torii على أودين وهو JSON، وذلك للتمكن من الحصول على برنامج ترميز صغير للوصول إلى السوق. يعرض هذا الجزء ملخص الحالة الداخلية (`docs/source/account_address_status.md` في المستودع الرئيسي)، بحيث يمكن فتح بوابة الكتب يتم نسخ سير العمل دون الحاجة إلى الريبو الأحادي.

## إعادة الإنشاء أو التحقق من الحزمة

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

الأعلام:

- `--stdout` - قم بإدخال JSON في stdout لإجراء تحقيقات مخصصة.
- `--out <path>` — اكتب في مكان آخر (على سبيل المثال، عند التحسين المحلي).
- `--verify` — احصل على نسخة فعالة من خلال الاتصال المعزز (يمكنك الاتصال بـ `--stdout`).

سير عمل CI ** انجراف ناقل العنوان ** اضغط على `cargo xtask address-vectors --verify`
كل مرة يتم فيها التحكم في التركيب أو المولد أو المستندات التي تسبق المتصفح بشكل غير مباشر.

## كيف تستخدم المباراة؟

| السطح | التحقق من الصحة |
|---------|-----------|
| نموذج بيانات الصدأ | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (الخادم) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| جافا سكريبت SDK | `javascript/iroha_js/test/address.test.js` |
| سويفت SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| أندرويد SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |يتم استخدام الحزام الكنسي ذهابًا وإيابًا + I105 + الترميز الكهربي والتحقق من كود أوسيبوك في النمط Norito المصاحب للتركيبات للكلمات السلبية.

## الأتمتة الجديدة؟

يمكنك تحرير الأدوات تلقائيًا من خلال المساعد
`scripts/account_fixture_helper.py`، الذي يتم الحصول عليه أو التحقق من الحزمة الأساسية بدون النسخ/التثبيت:

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

يقوم المساعد بتطبيق التجاوزات من خلال `--source` أو التوسيع الدائم `IROHA_ACCOUNT_FIXTURE_URL`، مما قد يسمح لوظائف CI SDK بتوضيح الاحتمالية على ما يبدو. قبل أن يقوم المساعد `--metrics-out` بكتابة `account_address_fixture_check_status{target="…"}` باستخدام ملخص SHA-256 الأساسي (`account_address_fixture_remote_info`)، لجامعي الملفات النصية Prometheus و يمكن للوحة البيانات Grafana `account_address_fixture_status` تحسين المزامنة. قم بإنشاء تنبيه عندما يقوم الهدف بالاتصال بـ `0`. من أجل أتمتة الأسطح المتعددة، استخدم المجمع `ci/account_fixture_metrics.sh` (يبدأ `--target label=path[::source]`)، لنشر الأوامر عند الطلب ملف `.prom` واحد لعقدة مُجمع الملفات النصية-المصدر.

## هل أنت مختصرة للغاية؟

الحالة الكاملة للدعم ADDR-2 (المالكون، مراقبة الخطة، الكشف عن الأضرار)
يوجد مستودع إدخال في `docs/source/account_address_status.md` مع بنية العنوان RFC (`docs/account_structure.md`). استخدم هذه المنطقة كتسمية عملية؛ للتحدث بشكل مباشر في مستودع المستندات.