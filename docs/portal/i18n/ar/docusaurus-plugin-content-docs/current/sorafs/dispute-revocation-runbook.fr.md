---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: دليل إلغاء النزاع
العنوان: Runbook des litiges et révocations SoraFS
Sidebar_label: الدعاوى القضائية والإبطالات
الوصف: تدفق الإدارة لإيداع الدعاوى المتعلقة بالسعة SoraFS، وتنسيق الإلغاءات وإخلاء البيانات بالطريقة المحددة.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sorafs/dispute_revocation_runbook.md`. قم بمزامنة النسختين حتى يتم سحب التوثيق الذي ورثه أبو الهول.
:::

## موضوعي

يرشد هذا الدليل مشغلي الإدارة عند إنشاء دعاوى السعة SoraFS وتنسيق الإلغاءات وضمان الإخلاء المحدد للبيانات.

## 1. تقييم الحادث

- **شروط التنازل:** اكتشاف انتهاك لاتفاقية مستوى الخدمة (التوفر/فحص PoR)، أو عجز النسخ، أو إلغاء التسوية.
- **تأكيد اللقطة البعيدة:** التقاط اللقطات `/v1/sorafs/capacity/state` و`/v1/sorafs/capacity/telemetry` من أجل المورد.
- **Notifier lesطراف prenantes:** فريق التخزين (opérations du fournisseur)، مجلس الإدارة (organe décisionnel)، Observability (mises à jour des Dashboards).

## 2. قم بإعداد الحزمة المسبقة1. جمع القطع الأثرية الخام (القياس عن بعد JSON، سجلات CLI، ملاحظات التدقيق).
2. التطبيع في أرشيف محدد (على سبيل المثال، كرة من القطر)؛ المرسل :
   - ملخص BLAKE3-256 (`evidence_digest`)
   - نوع الوسائط (`application/zip`، `application/jsonl`، إلخ.)
   - رابط URI (تخزين الكائنات، الدبوس SoraFS أو نقطة النهاية التي يمكن الوصول إليها عبر Torii)
3. قم بتخزين الحزمة في مجموعة إجراءات الحوكمة مع إمكانية الوصول إليها مرة واحدة.

## 3. إيداع الدعوى

1. إنشاء مواصفات JSON لـ `sorafs_manifest_stub capacity dispute` :

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

2. إطلاق CLI :

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

3. تحقق من `dispute_summary.json` (تأكيد النوع، ملخص التوصيات، الطوابع الزمنية).
4. قم بطلب JSON إلى Torii `/v1/sorafs/capacity/dispute` عبر ملف معاملات الإدارة. التقاط قيمة الاستجابة `dispute_id_hex` ; بالإضافة إلى إجراءات الإلغاء اللاحقة وتقارير التدقيق.

## 4. الإخلاء والإلغاء1. **نافذة الرحمة:** الإعلان عن مقدم الإلغاء الوشيك ; قم بتفويض إخلاء البيانات عند السماح بالسياسة.
2. **جينيريز `ProviderAdmissionRevocationV1`:**
   -استخدم `sorafs_manifest_stub provider-admission revoke` مع السبب المعتمد.
   - التحقق من التوقيعات وملخص الإبطال.
3. **نشر الإلغاء :**
   - قم بإرجاع طلب الإلغاء إلى Torii.
   - تأكد من أن إعلانات المورد محظورة (احضر إلى منزل `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **Mettez à jour les Dashboards:** signalez le fournisseur comme révoqué, référencez'id du litige et liez le package de preuves.

## 5. بعد الوفاة وتتبعها

- قم بتسجيل التسلسل الزمني وسبب العنصر وإجراءات العلاج في متتبع حوادث الحوكمة.
- تحديد الاسترداد (خفض الحصة، استرداد الرسوم، تعويضات العملاء).
- توثيق الدروس ; قم بمتابعة جميع مستويات SLA أو تنبيهات المراقبة إذا كانت ضرورية.

## 6. وثائق مرجعية

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (قسم الدعاوى)
- `docs/source/sorafs/provider_admission_policy.md` (سير العمل للإبطال)
- لوحة التحكم: `SoraFS / Capacity Providers`

## قائمة المراجعة- [ ] مجموعة من اللقطات المسبقة والتقطيع.
- [ ] حمولة التقاضي المحلية الصحيحة.
- [ ] تم قبول معاملة التقاضي Torii.
- [ ] تم تنفيذ الإلغاء (إذا تمت الموافقة عليه).
- [ ] لوحات المعلومات/دفاتر التشغيل غير متوفرة حاليًا.
- [ ] إيداع بعد الوفاة بعد الوفاة لدى مجلس الحوكمة.