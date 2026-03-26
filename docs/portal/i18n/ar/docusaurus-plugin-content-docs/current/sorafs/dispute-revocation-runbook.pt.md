---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: دليل إلغاء النزاع
العنوان: Runbook de disputas e revogacoes da SoraFS
Sidebar_label: Runbook de disputas and Revogacoes
الوصف: تدفق الإدارة لتسجيل نزاعات السعة في SoraFS، وتنسيق عمليات الإصلاح وإخلاء البيانات بالشكل الحتمي.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/dispute_revocation_runbook.md`. قم بحفظ النسخ المتزامنة حتى يتم سحب وثيقة أبو الهول.
:::

## اقتراح

هذا الدليل يوجه مشغلي الإدارة لفتح نزاعات القدرة على SoraFS، من خلال تنسيق عمليات التجديد ويضمن أن إخلاء البيانات سينتهي من شكل حتمي.

## 1. معرفة الحادث

- **شروط الاسترداد:** اكتشاف تداخل SLA (وقت التشغيل/خطأ PoR)، أو عجز النسخ، أو تباين النحاس.
- **تأكيد القياس عن بعد:** التقط لقطات `/v1/sorafs/capacity/state` و`/v1/sorafs/capacity/telemetry`.
- **Notificar parts interessadas:** فريق التخزين (عمليات إثبات)، مجلس الإدارة (orgao decisor)، إمكانية المراقبة (تحقيق لوحات المعلومات).

## 2. إعداد حزمة الأدلة1. Colete artefatos brutos (القياس عن بعد JSON، سجلات CLI، ملاحظات الاستماع).
2. تطبيع الملف الحتمي (على سبيل المثال، القطران)؛ سجل:
   - ملخص BLAKE3-256 (`evidence_digest`)
   - نوع الوسائط (`application/zip`، `application/jsonl`، إلخ.)
   - URI للمستشفى (تخزين الكائنات، دبوس da SoraFS أو نقطة النهاية يمكن الوصول إليها عبر Torii)
3. قم بتخزين أو تخزين مجموعة من أدلة الإدارة مع إمكانية الوصول إليها مرة واحدة.

## 3. سجل النزاع

1. صرخ بمواصفات JSON للفقرة `sorafs_manifest_stub capacity dispute`:

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
     --authority=<katakana-i105-account-id> \
     --private-key=ed25519:<key>
   ```

3. قم بمراجعة `dispute_summary.json` (تأكيد النوع، ملخص الأدلة والطوابع الزمنية).
4. قم بإرسال طلب JSON لـ Torii `/v1/sorafs/capacity/dispute` عبر ملف التحويلات الحاكمة. التقاط قيمة الرد `dispute_id_hex`; إنها أخرى مثل نصائح التجديد اللاحقة وعلاقات الاستماع.

## 4. الإخلاء والإعادة1. **Janela de graca:** إشعار أو إثبات حول إعادة وشيكة؛ يسمح بالإخلاء بعد إصلاحه عندما تسمح به السياسة.
2. ** جير `ProviderAdmissionRevocationV1`: **
   - استخدم `sorafs_manifest_stub provider-admission revoke` كدافع مصدق.
   - التحقق من الاغتيالات وخلاصة المراجعة.
3. **نشر تجديد:**
   - أرسل طلب الإصلاح لـ Torii.
   - تأكد من أن الإعلانات تثبت أنها محظورة (نأمل أن يكون `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` أكثر).
4. **تحقيق لوحات المعلومات:** علامة أو إثبات كمراجع أو مرجع أو معرف النزاع وتسجيل أو حزمة الأدلة.

## 5. مرافقة بعد الوفاة

- قم بالتسجيل في خط الزمن والسبب والأدوات العلاجية دون تعقب أحداث الإدارة.
- تحديد الاسترداد (خفض الحصة، استرداد الضرائب، المكافآت للعملاء).
- توثيق نظام التشغيل؛ تحقيق حدود SLA أو تنبيهات المراقبة إذا كانت ضرورية.

## 6. المواد المرجعية

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (سيكاو دي ديسبوتاس)
- `docs/source/sorafs/provider_admission_policy.md` (تدفق التجديد)
- لوحة القيادة للمراقبة: `SoraFS / Capacity Providers`

## قائمة المراجعة

- [ ] حزمة الأدلة التي تم التقاطها وتقطيعها.
- [ ] الحمولة هي محل النزاع الصحيح.
- [ ] نزاع حول رقم Torii.
- [ ] إعادة التنفيذ (إذا تمت الموافقة عليه).
- [ ] تم تحديث لوحات المعلومات/دفاتر التشغيل.
- [ ] أرشيف ما بعد الوفاة جنبًا إلى جنب مع نصيحة الإدارة.