---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: دليل إلغاء النزاع
العنوان: دليل تشغيل النزاعات وإلغاءات SoraFS
Sidebar_label: تشغيل الدليل والإلغاء
الوصف: سير عمل الـ تور تشهد براءات صلاح SoraFS وتنسيق الإلغاءات و حتمي البيانات بشكل كامل.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/dispute_revocation_runbook.md`. احرص على جميع النسختين متزامنتين إلى أن يتم تكريم أبو الهول القديم.
:::

## الهدف

يرشد هذا الدليل مشغلي الشبكه من خلال تقديم نزاعات SoraFS، وتنسيق الإلغاءات، والمسؤول عن التحكم في البيانات بشكل جزئي حتمي.

## 1. تقييم الحادث

- **الشروط غير واضحة:** وضوح كامل SLA (التوفر/فشل PoR)، عيوب التكرار، أو فشل في الفوطرة.
- **التأكيد التليميرية:** لقطات `/v2/sorafs/capacity/state` و`/v2/sorafs/capacity/telemetry` للمنظم.
- ** سجل البيانات الحديثة: ** فريق التخزين (عمليات المتخصص)، مجلس الإدارة (جهة خاصة)، إمكانية الملاحظة (تحديثات لوحات المتابعة).

## 2. إعداد حزمة الأدلة

1. اجمع الآرتيفاكتات الخام (القياس عن بعد JSON، سجلات CLI، تقارير المدققين).
2.وحّدها في أرشيف حتمي (مثل القطران)؛ وتسجيل:
   - ملخص BLAKE3-256 (`evidence_digest`)
   - نوع الوسائط (`application/zip`، `application/jsonl`، وما إلى ذلك)
   - URI الاستضافة (object store، أو SoraFS pin، أو نقطة نهاية المطاف عبر Torii)
3. مخزن الأدلة الخاصة بجمع الأدلة الخاصة بالنتو مع وصول كتابة لمرة واحدة.

##3. تقديم متناقض

1. أنشئ موصفة JSON لـ `sorafs_manifest_stub capacity dispute`:

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

2. شغّل CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=i105... \
     --private-key=ed25519:<key>
   ```3. راجع `dispute_summary.json` ( تأكد من النوع، ملخص الأدلة، والطوابع الزمنية).
4. أرسل طلب JSON إلى Torii `/v2/sorafs/capacity/dispute` عبر الطابور التداولي. قطّر قيمة القيمة `dispute_id_hex`؛ فهي تثبّت إجراءات الإلغاء اللاحقة وتقارير التدقيق.

## 4.الكائن والإلغاء

1. **نافذة الدوثر:** أخطر المتحكم بقرب الإلغاء؛ واسمح بتوفر البيانات المثبتة عندما تسمح السياسة.
2. **أنشئ `ProviderAdmissionRevocationV1`:**
   - استخدم `sorafs_manifest_stub provider-admission revoke` مع شخص مؤهل.
   - تحقّق من التواقيع وملخص الإلغاء.
3. **نشر الإلغاء:**
   - أرسل طلب الإلغاء إلى Torii.
   - تأكد من حظر الإعلانات الخاصة بالمتحكم (توقع الارتفاع `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **تعديل لوحات المتابعة:** علّم المتحكم على أنه مُلغى، وأشر إلى معترف به معترض، واربط حزمة الأدلة.

## 5. ما بعد الحادث والمتابعة

- سجل الجدول التاريخي والسبب حاولي تتبع نقاط المعالجة الخاصة بالشبكة.
- تعويض (slashing rehan، clawbacks للرسوم، وتعويضات العملاء).
- وثّق الدروس المستفادة؛ تحديث اعتبات SLA أو تنبيهات المراقبة إذا لزم الأمر.

## 6. مواد مرجعية

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (قسم الرقابة)
- `docs/source/sorafs/provider_admission_policy.md` (سير عمل الإلغاء)
- لوحة المراقبة: `SoraFS / Capacity Providers`

## قائمة التحقق

- [ ] تم التقاط حزمة الأدلة واحتساب التجزئة.
- [ ] تم التحقق من عدم تطابق محلي.
- [ ] تم قبول التفاوض في Torii.
- [ ] تم إلغاء الإلغاء (إن الموافقة على الموافقة).
- [ ] تم تحديث لوحات المتابعة/الأدلة التشغيلية.
- [ ] تم إرسالنا بعد تدهور حالتنا إلى المجلس.