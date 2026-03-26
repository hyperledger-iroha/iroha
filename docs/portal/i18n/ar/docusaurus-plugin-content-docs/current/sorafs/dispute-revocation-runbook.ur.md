---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: دليل إلغاء النزاع
العنوان: SoraFS تنازع ومنسوخی رن بک
Sidebar_label: تنازع ومنسوخی رن بک
الوصف: SoraFS عبارة عن سجل تسلسلي مُجمَّع، منسوخيو، والعديد من الأشياء التي هي في الواقع عبارة عن فوضى في العمل.
---

:::ملاحظة مستند ماخذ
هذه هي الصفحة `docs/source/sorafs/dispute_revocation_runbook.md`. عندما لا يكون هناك أي دليل على اكتشاف أبو الهول، فلا داعي للقلق.
:::

## القصد

هناك عدد من الأفلام الإباحية التي SoraFS وهي عبارة عن سلسلات مجمعة ومنسوخية من آنج، وأكثر من ذلك بكثير من الإنجليز. أنا أفكر في الأمر.

## 1. حقيقة جائزة

- ** ٹرگر التوزيع: ** خلل في اتفاق مستوى الخدمة (فشل وقت التشغيل/فشل PoR)، أو نقص النسخ المتماثل، أو عدم الاتفاق على الفواتير.
- **التتبع:** يمكنك التقاط لقطات `/v1/sorafs/capacity/state` و`/v1/sorafs/capacity/telemetry`.
- **مراجعي الحسابات الاسترالية:** فريق التخزين (عمليات المزود)، مجلس الإدارة (هيئة القرار)، إمكانية المراقبة (تحديثات لوحة المعلومات).

## 2. قم بإعادة رسم الصورة

1. القطع الأثرية الخام مجموعة کریں (القياس عن بعد JSON، سجلات CLI، ملاحظات المدقق)۔
2. يعمل الأرشيف الرقمي (مثل كرة القطران) على تطبيع الوضع؛ درجة الدراسة:
   - BLAKE3-256 خلاصة (`evidence_digest`)
   - نوع الوسائط (`application/zip`, `application/jsonl` وغيرہ)
   - استضافة URI (تخزين الكائنات، دبوس SoraFS، أو نقطة نهاية يمكن الوصول إليها Torii)
3. يمكن لدلو جمع الأدلة أن يكتب مرة واحدة فقط كما هو محفوظ للكتابة.## 3.تنازع جمع کرائیں

1. `sorafs_manifest_stub capacity dispute` لمواصفات JSON:

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

2. اختصار CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=soraカタカナ... \
     --private-key=ed25519:<key>
   ```

3.`dispute_summary.json` ريويو كريں (النوع، ملخص الأدلة، الطوابع الزمنية).
4. تم تحديث الإصدار Torii `/v1/sorafs/capacity/dispute` إلى ريكوست JSON. جواب کي تحت `dispute_id_hex` محفوظ کریں؛ هذا بعد منسوخ وتقرير آخر.

## 4. انخلا اور منسوخی

1. **نافذة النعمة:** تم الإعلان عن التوقع المتوقع مؤخرًا؛ سمحت لك باليس بتثبيت البيانات مما يسمح لك بإكمالها.
2. **`ProviderAdmissionRevocationV1` اسم:**
   - يتم استخدام الجهاز باستخدام `sorafs_manifest_stub provider-admission revoke`.
   - ملخص الخط والإلغاء ویریفائی کریں.
3. **منسوخی شاع کریں:**
   - منسوخي ريكوئست Torii تم جمعه كريں.
   - إعلانات یقینی بنائیں کہ پروائيیڈر بلاک ہیں (متوقع ہے `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` بڑھے).
4. **سجلات لوحات المعلومات:** تم إبطال البراءات للبطاقة، ومعرف النزاع، وحزمة الأدلة لنك.

## 5. بعد الوفاة وفالو اپ

- إجراءات تعقب الحوادث عبر الإنترنت والسبب الجذري والعلاجي.
- الاسترداد (خفض الحصص، استرداد الرسوم، استرداد الأموال للعملاء)۔
- سباق دستاويز كریں؛ من الضروري تحديد عتبات SLA أو مراقبة التنبيهات.

## 6. حوالہ جاتی المواد

-`sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (قسم النزاعات)
- `docs/source/sorafs/provider_admission_policy.md` (سير عمل الإلغاء)
- لوحة معلومات إمكانية الملاحظة: `SoraFS / Capacity Providers`

## چیک لٹ- [ ] حزمة الأدلة تمكنك من الحصول على تجزئة.
- [ ] يتم التحقق من صحة الحمولة النافعة للنزاع.
- [ ] نزاع Torii ٹرانزیکشن قبول ہوئی۔
- [ ] منسوخي نافف کي گئي (إذا كان طعاما)۔
- [ ] لوحات المعلومات/دفاتر التشغيل
- [ ] تم جمع الجثث بعد الوفاة بواسطة المستشار.