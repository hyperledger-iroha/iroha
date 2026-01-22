---
lang: ar
direction: rtl
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9e5fce128259ae2b2c40782b3c96c38048fce6f3b4522319bd60b59db87a8252
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# أتمتة نموذج تهديد توفر البيانات (DA-1)

<div dir="rtl">

يتطلب بند DA-1 في خارطة الطريق وملف `status.md` حلقة أتمتة حتمية تنتج ملخصات
نموذج تهديد Norito PDP/PoTR المعروضة في `docs/source/da/threat_model.md` ونسخة
Docusaurus. يلتقط هذا الدليل المخرجات المشار إليها بواسطة:

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (يشغّل `scripts/docs/render_da_threat_model_tables.py`)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## التدفق

1. **أنشئ التقرير**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   يسجل الملخص بصيغة JSON معدل فشل النسخ المتماثل المحاكى، وحدود الـ chunker،
   وأي انتهاكات للسياسة يكتشفها مُعِد PDP/PoTR في `integration_tests/src/da/pdp_potr.rs`.
2. **اعرض جداول Markdown**
   ```bash
   make docs-da-threat-model
   ```
   يشغّل ذلك `scripts/docs/render_da_threat_model_tables.py` لإعادة كتابة
   `docs/source/da/threat_model.md` و `docs/portal/docs/da/threat-model.md`.
3. **أرشِف الأثر** بنسخ تقرير JSON (وسجل CLI الاختياري) إلى
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`. عندما تعتمد
   قرارات الحوكمة على تشغيل محدد، أضف تجزئة الالتزام وبذرة المُحاكي في ملف
   `<timestamp>-metadata.md` المرافق.

## توقعات الأدلة

- يجب أن تبقى ملفات JSON أقل من 100 كيلوبايت كي تتوافق مع git. يجب حفظ التتبعات
  الأكبر في التخزين الخارجي، مع الإشارة إلى التجزئة الموقعة في ملاحظة الميتاداتا.
- يجب أن يسرد كل ملف مُؤرشف البذرة ومسار الإعداد وإصدار المُحاكي حتى يمكن إعادة
  التشغيل بشكل قابل لإعادة الإنتاج تمامًا.
- اربط الملف المؤرشف من `status.md` أو من بند خارطة الطريق كلما تقدمت معايير
  قبول DA-1، لضمان أن المراجعين يمكنهم التحقق من خط الأساس دون إعادة تشغيل
  الـ harness.

## تسوية الالتزامات (إغفال المُسلسل)

استخدم `cargo xtask da-commitment-reconcile` لمقارنة إيصالات إدخال DA بسجلات
التزامات DA واكتشاف إغفال المُسلسل أو العبث:

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- يقبل الإيصالات بصيغة Norito أو JSON والالتزامات من `SignedBlockWire` أو `.norito`
  أو حزم JSON.
- يفشل عند غياب أي تذكرة من سجل الكتل أو عند اختلاف التجزئات؛ `--allow-unexpected`
  يتجاهل التذاكر الموجودة في الكتل فقط عندما تضيق مجموعة الإيصالات عمدًا.
- أرفق JSON الصادر بحزم الحوكمة/Alertmanager لتنبيهات الإغفال؛ القيمة الافتراضية
  هي `artifacts/da/commitment_reconciliation.json`.

## تدقيق الامتيازات (مراجعة الوصول ربع السنوية)

استخدم `cargo xtask da-privilege-audit` لفحص أدلة manifest/replay الخاصة بـ DA
(ومسارات إضافية اختيارية) بحثًا عن إدخالات مفقودة أو غير أدلة أو قابلة للكتابة
عالميًا:

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- يقرأ مسارات إدخال DA من إعدادات Torii ويفحص أذونات Unix عندما تكون متاحة.
- يعلّم المسارات المفقودة/غير الأدلة/القابلة للكتابة عالميًا ويعيد رمز خروج
  غير صفري عند وجود مشاكل.
- وقّع وأرفق حزمة JSON (`artifacts/da/privilege_audit.json` افتراضيًا) مع حزم
  ولوحات مراجعة الوصول ربع السنوية.

</div>
