---
lang: ar
direction: rtl
source: docs/portal/docs/reference/account-address-status.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حالة عنوان الحساب
العنوان: Cumplimiento de Direcciones de Cuenta
الوصف: استئناف تدفق العمل من تركيب ADDR-2، كما يتم الحفاظ على أجهزة SDK متزامنة.
---

حزمة كانون ADDR-2 (`fixtures/account/address_vectors.json`) تركيبات الالتقاط I105 (المفضل)، مضغوطة (`sora`، ثاني أفضل؛ نصف/عرض كامل)، التوقيعات المتعددة والسلبية. كل سطح SDK + Torii يدعم نفس JSON لاكتشاف أي برنامج ترميز مشتق قبل أن يبدأ الإنتاج. تعرض هذه الصفحة ملخص الحالة الداخلية (`docs/source/account_address_status.md` في المستودع الرئيسي) حتى يتمكن قراء البوابة من الاطلاع على التدفق دون البحث في المستودع الأحادي.

## تجديد أو التحقق من الحزمة

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

الأعلام:

- `--stdout` - قم بإصدار JSON بشكل قياسي للفحص المخصص.
- `--out <path>` - اكتب في طريق مختلف (ص. على سبيل المثال، مقارنة التغييرات المحلية).
- `--verify` - لمقارنة نسخة العمل بالمحتوى الذي تم إنشاؤه (لا يمكن دمجه مع `--stdout`).

سير العمل في CI **Address Vector Drift** يقوم بتشغيل `cargo xtask address-vectors --verify`
كل مرة تقوم فيها بتغيير التركيب أو المولد أو المستندات لتنبيه المراجعين الفوريين.

## من يستهلك التركيبة؟| سطحية | التحقق من الصحة |
|---------|-----------|
| نموذج بيانات الصدأ | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (الخادم) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| جافا سكريبت SDK | `javascript/iroha_js/test/address.test.js` |
| سويفت SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| أندرويد SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

يقوم كل تسخير بإجراء رحلة ذهابًا وإيابًا لوحدات البايت Canonicos + I105 + الكودات المضمنة والتحقق من أن رموز الخطأ ذات النمط Norito تتزامن مع التركيبات الخاصة بالحالات السلبية.

## هل تحتاج إلى التشغيل الآلي؟

يمكن لأدوات الإصدار أتمتة تحديثات التركيبات باستخدام المساعد
`scripts/account_fixture_helper.py`، يمكنك الحصول على أو التحقق من الحزمة Canonico دون الحاجة إلى النسخ/التثبيت:

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

يقبل المساعد تجاوزات `--source` أو متغير الإدخال `IROHA_ACCOUNT_FIXTURE_URL` بحيث تعكس مهام CI SDK المفضلة لديك. عند تقديم `--metrics-out`، المساعد في كتابة `account_address_fixture_check_status{target="..."}` جنبًا إلى جنب مع الملخص SHA-256 Canonico (`account_address_fixture_remote_info`) لمجمعات الملفات النصية Prometheus ولوحة القيادة Grafana `account_address_fixture_status` يمكن أن يختبر أن كل سطح موجود بشكل متزامن. تنبيه عندما يكون هناك تقرير مستهدف `0`. لأتمتة الأسطح المتعددة باستخدام الغلاف `ci/account_fixture_metrics.sh` (يقبل `--target label=path[::source]` المتكررة) حتى تقوم الأجهزة عند الطلب بنشر أرشيف فريد `.prom` لتجميع الملفات النصية من مصدر العقدة.

## هل تحتاج إلى ملخص كامل؟الحالة الكاملة للوفاء ADDR-2 (المالكون، خطة المراقبة، الإجراءات المفتوحة) موجودة في `docs/source/account_address_status.md` داخل المستودع جنبًا إلى جنب مع بنية العنوان RFC (`docs/account_structure.md`). الولايات المتحدة الأمريكية هذه الصفحة هي عبارة عن سجل التشغيل السريع؛ للحصول على معلومات عميقة، راجع مستندات الريبو.