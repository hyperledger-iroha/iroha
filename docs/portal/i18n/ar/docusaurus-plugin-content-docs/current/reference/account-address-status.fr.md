---
lang: ar
direction: rtl
source: docs/portal/docs/reference/account-address-status.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حالة عنوان الحساب
العنوان: Conformite des adresses de compte
الوصف: استئناف سير العمل من خلال ADDR-2 ومزامنة معدات SDK.
---

تلتقط الحزمة الكنسي ADDR-2 (`fixtures/account/address_vectors.json`) التركيبات I105 (المفضل)، المضغوطة (`sora`، ثاني أفضل؛ نصف/عرض كامل)، التوقيعات المتعددة والسلبية. يتم تطبيق Chaque surface SDK + Torii على meme JSON للكشف عن جميع مشتقات برنامج الترميز قبل الإنتاج. تعكس هذه الصفحة ملخص الوضع الداخلي (`docs/source/account_address_status.md` في مستودع العرق) حتى يتمكن قراء المكتب من استشارة سير العمل بدون أخطاء في المستودع الأحادي.

## تجديد أو التحقق من الحزمة

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

الأعلام:

- `--stdout` - قم بإصدار JSON vers stdout للفحص المخصص.
- `--out <path>` - الكتابة عبر مسار آخر (على سبيل المثال عند المقارنة المحلية).
- `--verify` - قارن نسخة العمل بمحتوى عام ضعيف (لا يمكن الجمع بين أي منهما مع `--stdout`).

Le Workflow CI **Address Vector Drift** ينفذ `cargo xtask address-vectors --verify`
كل مرة تتغير فيها التركيبات أو المُنشئ أو المستندات لتنبيه المراجعين على الفور.

## ما هو استخدامك للتركيبة؟| السطح | التحقق من الصحة |
|---------|-----------|
| نموذج بيانات الصدأ | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (الخادم) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| جافا سكريبت SDK | `javascript/iroha_js/test/address.test.js` |
| سويفت SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| أندرويد SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

يقوم كل تسخير بإجراء إرجاع للثمانيات الأساسية + I105 + ضغطات الترميز والتحقق من أن رموز أخطاء النمط Norito تتوافق مع التركيبات السلبية.

## هل تحتاج إلى الأتمتة؟

يمكن لأدوات التحرير أتمتة عمليات تركيب التركيبات بمساعدة المساعد
`scripts/account_fixture_helper.py`، الذي يمكنك استرداده أو التحقق من حزمة Canonique بدون ناسخة/مجمع:

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

يقبل المساعد التجاوزات `--source` أو متغير البيئة `IROHA_ACCOUNT_FIXTURE_URL` لكي تشير وظائف CI لـ SDK إلى تفضيلها. عندما يتم تقديم `--metrics-out`، ستساعدك المساعدة في كتابة `account_address_fixture_check_status{target="..."}` بالإضافة إلى SHA-256 canonique (`account_address_fixture_remote_info`) لتجميع مجمعات الملفات النصية Prometheus ولوحة القيادة Grafana `account_address_fixture_status` قادر على إثبات أن كل سطح يتبقى متزامنًا. Alertez des qu'une cible reporte `0`. لأتمتة الأسطح المتعددة، استخدم الغلاف `ci/account_fixture_metrics.sh` (قبول التكرارات `--target label=path[::source]`) حتى تقوم المعدات الأساسية بنشر الملف نفسه `.prom` لتجميع مجمع الملفات النصية في عقدة التصدير.

## هل تحتاج إلى ملخص كامل؟قانون المطابقة الكامل ADDR-2 (المالكون، خطة المراقبة، الإجراءات المفتوحة)
إذا تم العثور عليها في `docs/source/account_address_status.md` في المستودع، بالإضافة إلى هيكل العنوان RFC (`docs/account_structure.md`). استخدم هذه الصفحة كعملية هبوط سريعة؛ قم بالرجوع إلى مستندات الريبو للحصول على دليل موافق عليه.