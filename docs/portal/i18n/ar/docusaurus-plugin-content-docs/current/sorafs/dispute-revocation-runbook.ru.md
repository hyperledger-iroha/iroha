---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: دليل إلغاء النزاع
العنوان: رانبوك سبوروف وملاحظات SoraFS
Sidebar_label: Ранбук споров وملاحظات
الوصف: حوكمة العملية لدعم التبرعات SoraFS، ملاحظات التنسيق وتحديد إخلاء البيانات.
---

:::note Канонический источник
هذا الجزء يعرض `docs/source/sorafs/dispute_revocation_runbook.md`. بعد ذلك، قم بالنسخ المتزامن، حتى لا يتم إصدار وثائق أبو الهول عن طريق الخطأ.
:::

## روعة

يوفر هذا التدريب حوكمة المشغلين من خلال تحفيز الاتصال بـ SoraFS وملاحظات التنسيق وتحديد الدقة عمليات الإخلاء.

## 1. تحديد المصدر

- **تفعيل التحفيز:** زيادة مستوى SLA (وقت التشغيل/نسبة PoR)، أو نقص النسخ المتماثل أو تغيير الفواتير.
- **التحقق من القياس عن بعد:** قم بمسح اللقطات `/v1/sorafs/capacity/state` و`/v1/sorafs/capacity/telemetry` للتجربة.
- **الموافقة على steekholderov:** فريق التخزين (عملية التحقق)، مجلس الإدارة (قرار الجهاز)، إمكانية المراقبة (مراقبة البيانات).

## 2. قم بتسليم حزمة المصادقة1. تعرف على المصنوعات اليدوية (القياس عن بعد JSON، سجل CLI، مدققي الحسابات).
2. التطبيع في تحديد الأرشيف (على سبيل المثال، كرة القطران)؛ الفاكس:
   - ملخص BLAKE3-256 (`evidence_digest`)
   - نوع الوسائط (`application/zip`، `application/jsonl` وما إلى ذلك)
   - تغيير حجم URI (تخزين الكائنات، SoraFS pin أو نقطة النهاية، متاح عبر Torii)
3. قم بتضمين الحزمة في مجموعة أدلة الحوكمة مع إمكانية الكتابة مرة واحدة.

## 3. مارس الرياضة

1. قم بإدراج مواصفات JSON لـ `sorafs_manifest_stub capacity dispute`:

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

2. قم بتثبيت CLI:

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

3. تحقق من `dispute_summary.json` (تحقق من النوع، ولخص البيانات، والبيانات المؤقتة).
4. قم بإصلاح مشكلة JSON في Torii `/v1/sorafs/capacity/dispute` من خلال مراقبة معاملات الحوكمة. قم بتأكيد الإجابة على `dispute_id_hex`; هذه هي الطريقة التالية للملاحظة وملاحظات مدققي الحسابات.

## 4. الإخلاء والإخلاء1. **الكلمات الرئيسية:** قم بمقدمة الرسالة الكبيرة; قم بإنهاء عمليات الإخلاء المزعجة عندما يؤدي ذلك إلى دعم السياسة.
2. **الهندسة `ProviderAdmissionRevocationV1`:**
   - استخدم `sorafs_manifest_stub provider-admission revoke` بسعر رائع.
   - التحقق من الوصفة والخلاصة.
3. **أنشر الملاحظة:**
   - قم بإجراء التحقيق في Torii.
   - تأكد من أن الإعلانات توفر لك الحجب (توجد قائمة `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **التعرف على لوحات الاتصال:** قم بفحص المدقق عن طريق تسجيل الدخول، وتسجيل بيانات المعرف، واستخدام سلك الحزمة المصاحبة.

## 5. تشريح الجثة والحالة اللاحقة

- الالتزام بالجدول الزمني، وأسس العمل، وطرق العلاج في حوكمة الأحداث.
- منح الأولوية (خفض الحصة، عمولة الاسترجاع، تفويض العملاء).
- توثيق البيانات؛ قم بملاحظة أخطاء اتفاقية مستوى الخدمة (SLA) أو تنبيهات المراقبة عند الحاجة.

## 6. مواد مناسبة

-`sorafs_manifest_stub capacity dispute --help`
-`docs/source/sorafs/storage_capacity_marketplace.md` (مباشر)
- `docs/source/sorafs/provider_admission_policy.md` (ملاحظة سير العمل)
- مراقبة لوحة البيانات: `SoraFS / Capacity Providers`

## قائمة الاختيار

- [ ] حزمة الإحالة للمحتوى والملفات.
- [ ] تم التحقق من الحمولة النافعة محليًا.
- [ ] Torii-معاملة الجراثيم.
- [ ] Отзыв выполнен (إذا كان موافقًا).
- [ ] أجهزة الكمبيوتر/الشبكات.
- [ ] تشكيل بعد الوفاة في الحكم الشامل.