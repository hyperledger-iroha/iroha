---
lang: ar
direction: rtl
source: docs/portal/docs/reference/account-address-status.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حالة عنوان الحساب
العنوان: Conformidade de enderecos de conta
الوصف: يكمل التدفق تركيب ADDR-2 كمعدات SDK متزامنة.
---

حزمة Canonico ADDR-2 (`fixtures/account/address_vectors.json`) تركيبات الالتقاط I105 (المفضل)، مضغوطة (`sora`، ثاني أفضل؛ نصف/عرض كامل)، توقيع متعدد وسالب. يستخدم كل سطح SDK + Torii أو نفس JSON لاكتشاف أي انحراف في برنامج الترميز قبل البدء في الإنتاج. توضح هذه الصفحة الحالة الداخلية الموجزة (`docs/source/account_address_status.md` في المستودع الرئيسي) حتى يتمكن القراء من مراجعة البوابة أو التدفق دون الأوعية الدموية أو المستودع الأحادي.

## تجديد أو التحقق من الحزمة

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

الأعلام:

- `--stdout` - قم بإصدار JSON القياسي للفحص المخصص.
- `--out <path>` - Grava em um caminho diferente (على سبيل المثال: ao comparar mudancas localmente).
- `--verify` - قارن نسخة العمل بالمحتوى المستلم (لا يمكن دمجه مع `--stdout`).

سير العمل في CI ** انحراف ناقل العنوان ** رودا `cargo xtask address-vectors --verify`
دائمًا ما يتم تثبيت أو تشغيل أو تشغيل المستندات لتنبيه المراجعين على الفور.

## ما الذي يستهلكه أم لاعبا أساسيا؟

| السطح | التحقق من الصحة |
|---------|-----------|
| نموذج بيانات الصدأ | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (الخادم) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| جافا سكريبت SDK | `javascript/iroha_js/test/address.test.js` |
| سويفت SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| أندرويد SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |كل تسخير سريع ذهابًا وإيابًا لوحدات البايت Canonicos + I105 + الترميزات المضمنة والتحقق من عدم وجود رموز خطأ على النمط Norito كتركيبة للحالات السلبية.

## ما هي دقة السيارة؟

يمكن لأدوات الإصدار تحديث التثبيت تلقائيًا باستخدام أو مساعد
`scripts/account_fixture_helper.py`، الذي يبحث أو التحقق من حزمة الكنسي بدون نسخ/لصق:

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

يمكنك المساعدة في تجاوز `--source` أو متغير البيئة المحيطة `IROHA_ACCOUNT_FIXTURE_URL` حتى تتمكن وظائف CI من SDK من عكس تفضيلاتك. عند `--metrics-out` والطلب، أو المساعدة في فك `account_address_fixture_check_status{target=\"...\"}` جنبًا إلى جنب مع ملخص SHA-256 canonico (`account_address_fixture_remote_info`) حتى يقوم جامعو الملفات النصية بعمل Prometheus ولوحة القيادة Grafana `account_address_fixture_status` يثبت أن كل سطح دائم متزامن. يتم التنبيه عند الإبلاغ عن الهدف `0`. لاستخدام الأسطح المتعددة التلقائية للغلاف `ci/account_fixture_metrics.sh` (هذا `--target label=path[::source]` المتكرر) للمعدات التي تنشر عند الطلب ملفًا فريدًا `.prom` لتجميع مجمع الملفات النصية لمصدر العقدة.

## هل تريد الدقة الكاملة؟

حالة المطابقة الكاملة ADDR-2 (المالكون، خطة المراقبة، عناصر المراقبة المفتوحة)
Fica em `docs/source/account_address_status.md` in the repositorio juto com o Address Structure RFC (`docs/account_structure.md`). استخدم esta pagina como lembrete Operacional Rapido؛ لتوضيح التفاصيل، راجع المستندات الموجودة في المستودع.