---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: دليل إلغاء النزاع
العنوان: Runbook de disputas y revocaciones de SoraFS
Sidebar_label: دليل النزاعات والإلغاءات
الوصف: تدفق الإدارة لعرض نزاعات القدرة على SoraFS، وتنسيق عمليات الإلغاء وإخلاء البيانات بشكل محدد.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/dispute_revocation_runbook.md`. حافظ على النسخ المتزامنة حتى يتم سحب الوثائق المتوارثة من Sphinx.
:::

##اقتراح

دليل التشغيل هذا يوجه مشغلي الإدارة لعرض نزاعات سعة SoraFS، وتنسيق عمليات الإلغاء وضمان إخلاء البيانات بشكل كامل بالشكل المحدد.

## 1. تقييم الحادث

- **شروط التنشيط:** اكتشاف عدم الالتزام باتفاقية مستوى الخدمة (مدة النشاط/فشل الأداء)، أو العجز في النسخ المتماثل، أو فشل التقاضي.
- **تأكيد القياس عن بعد:** التقط لقطات `/v1/sorafs/capacity/state` و`/v1/sorafs/capacity/telemetry` للمزود.
- **إخطار بالأجزاء المهمة:** فريق التخزين (عمليات المورد)، ومجلس الإدارة (تنظيم القرار)، وقابلية المراقبة (تحديث لوحات المعلومات).

## 2. تحضير حزمة الأدلة1. استنسخ القطع الأثرية بشكل مباشر (القياس عن بعد JSON، سجلات CLI، ملاحظات الاستماع).
2. تسوية ملف محدد (على سبيل المثال، كرة قماش)؛ التسجيل:
   - ملخص BLAKE3-256 (`evidence_digest`)
   - نوع الوسائط (`application/zip`، `application/jsonl`، إلخ.)
   - عنوان URL المتجدد (تخزين الكائنات، دبوس SoraFS أو نقطة النهاية يمكن الوصول إليها من خلال Torii)
3. احفظ الحزمة في دلو حفظ أدلة الإدارة من خلال الوصول إلى مكتبة فريدة.

## 3. عرض النزاع

1. إنشاء مواصفات JSON لـ `sorafs_manifest_stub capacity dispute`:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. تنفيذ CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=ih58... \
     --private-key=ed25519:<key>
   ```

3. قم بمراجعة `dispute_summary.json` (تأكيد النوع، ملخص الأدلة والطوابع الزمنية).
4. قم بإرسال طلب JSON إلى Torii `/v1/sorafs/capacity/dispute` من خلال مجموعة المعاملات الحكومية. التقاط قيمة الرد `dispute_id_hex`; قم بإضافة إجراءات الإلغاء اللاحقة ومعلومات الاستماع.

## 4. الإخلاء والإلغاء1. **نافذة الرحمة:** إشعار للمورد بشأن الإلغاء الوشيك؛ السماح بإخلاء البيانات المفقودة عندما تسمح السياسة بذلك.
2. ** الأجناس `ProviderAdmissionRevocationV1`:**
   - الولايات المتحدة الأمريكية `sorafs_manifest_stub provider-admission revoke` مع الحل المناسب.
   - التحقق من الشركة وملخص الإلغاء.
3. **نشر الإلغاء:**
   - أرسل طلب الإلغاء إلى Torii.
   - تأكد من أن إعلانات المورد محظورة (إذا كنت تتوقع أن `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` يزداد).
4. **تحديث لوحات المعلومات:** يتم وضع علامة على المورد كملغى، والإشارة إلى معرف النزاع، وتعبئة حزمة الأدلة.

## 5. تشريح الجثة وتتبعها

- قم بتسجيل خط الوقت والسبب وإجراءات العلاج في متتبع حوادث الإدارة.
- تحديد الاسترداد (خفض الحصة، واسترداد العمولات، وتعويض العملاء).
- تعلم المستندات؛ تحديث مظلات SLA أو تنبيهات المراقبة إذا كانت ضرورية.

## 6. المواد المرجعية

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (قسم المنازعات)
- `docs/source/sorafs/provider_admission_policy.md` (تدفق الإلغاء)
- لوحة معلومات المراقبة: `SoraFS / Capacity Providers`

## قائمة المراجعة- [ ] حزمة الأدلة التي تم التقاطها وتقطيعها.
- [ ] الحمولة محل النزاع صحيحة.
- [ ] تم قبول معاملة النزاع في Torii.
- [ ] إلغاء التنفيذ (إذا كان الأمر كذلك).
- [ ] تحديث لوحات المعلومات/دفاتر التشغيل.
- [ ] تم تقديم تشريح الجثة قبل مشورة الحكومة.