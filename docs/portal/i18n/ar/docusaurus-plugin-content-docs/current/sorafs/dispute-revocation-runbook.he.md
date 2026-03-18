---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/dispute-revocation-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8a69f1268c3e53b1f193ee53e2038bedd88edf63e307f94b44d3fe2ee3e594c6
source_last_modified: "2026-01-22T15:55:18+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: dispute-revocation-runbook
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/dispute_revocation_runbook.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم سحب توثيق Sphinx القديم.
:::

## الهدف

يرشد هذا الدليل مشغلي الحوكمة خلال تقديم نزاعات سعة SoraFS، وتنسيق الإلغاءات، وضمان اكتمال إخلاء البيانات بشكل حتمي.

## 1. تقييم الحادث

- **شروط الإطلاق:** رصد خرق SLA (التوفر/فشل PoR)، نقص التكرار، أو خلاف في الفوترة.
- **تأكيد التليمترية:** التقط لقطات `/v1/sorafs/capacity/state` و`/v1/sorafs/capacity/telemetry` للمزوّد.
- **إخطار أصحاب المصلحة:** Storage Team (عمليات المزوّد)، Governance Council (جهة القرار)، Observability (تحديثات لوحات المتابعة).

## 2. إعداد حزمة الأدلة

1. اجمع الآرتيفاكتات الخام (telemetry JSON، سجلات CLI، ملاحظات المدققين).
2. وحّدها في أرشيف حتمي (مثل tarball)؛ وسجّل:
   - digest BLAKE3-256 (`evidence_digest`)
   - نوع الوسائط (`application/zip`، `application/jsonl`، وما إلى ذلك)
   - URI الاستضافة (object storage، أو SoraFS pin، أو نقطة نهاية متاحة عبر Torii)
3. خزّن الحزمة في حاوية جمع الأدلة الخاصة بالحوكمة مع وصول كتابة لمرة واحدة.

## 3. تقديم النزاع

1. أنشئ مواصفة JSON لـ `sorafs_manifest_stub capacity dispute`:

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
   ```

3. راجع `dispute_summary.json` (تأكد من النوع، digest الأدلة، والطوابع الزمنية).
4. أرسل JSON الطلب إلى Torii `/v1/sorafs/capacity/dispute` عبر طابور معاملات الحوكمة. التقط قيمة الاستجابة `dispute_id_hex`؛ فهي تثبّت إجراءات الإلغاء اللاحقة وتقارير التدقيق.

## 4. الإخلاء والإلغاء

1. **نافذة السماح:** أخطر المزوّد بقرب الإلغاء؛ واسمح بإخلاء البيانات المثبتة عندما تسمح السياسة.
2. **أنشئ `ProviderAdmissionRevocationV1`:**
   - استخدم `sorafs_manifest_stub provider-admission revoke` مع السبب المعتمد.
   - تحقّق من التواقيع وdigest الإلغاء.
3. **نشر الإلغاء:**
   - أرسل طلب الإلغاء إلى Torii.
   - تأكد من حظر adverts الخاصة بالمزوّد (توقع ارتفاع `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **حدّث لوحات المتابعة:** علّم المزوّد على أنه مُلغى، وأشر إلى معرّف النزاع، واربط حزمة الأدلة.

## 5. ما بعد الحادث والمتابعة

- سجّل الجدول الزمني والسبب الجذري وإجراءات المعالجة في متتبع حوادث الحوكمة.
- حدّد التعويض (slashing للرهان، clawbacks للرسوم، وتعويضات العملاء).
- وثّق الدروس المستفادة؛ حدّث عتبات SLA أو تنبيهات المراقبة إذا لزم الأمر.

## 6. مواد مرجعية

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (قسم النزاعات)
- `docs/source/sorafs/provider_admission_policy.md` (سير عمل الإلغاء)
- لوحة المراقبة: `SoraFS / Capacity Providers`

## قائمة التحقق

- [ ] تم التقاط حزمة الأدلة واحتساب التجزئة.
- [ ] تم التحقق من حمولة النزاع محليًا.
- [ ] تم قبول معاملة النزاع في Torii.
- [ ] تم تنفيذ الإلغاء (إن تمت الموافقة).
- [ ] تم تحديث لوحات المتابعة/الأدلة التشغيلية.
- [ ] تم إيداع ما بعد الحادث لدى مجلس الحوكمة.
