---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/staging-manifest-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: دليل التدريج
العنوان: Playbook de Manifest en Stage
Sidebar_label: دليل التشغيل للبيان والتدريج
الوصف: قائمة مرجعية لتأهيل ملف التقسيم المعتمد من قبل البرلمان بناءً على Torii من التدريج.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. احتفظ بنسخ متزامنة.
:::

## السيرة الذاتية

يصف كتاب اللعب هذا كيفية تأهيل ملف القطع الذي تم التصديق عليه من قبل البرلمان في عرض Torii للتجهيز المسبق للترويج للتغيير في الإنتاج. افترض أن بطاقة الإدارة SoraFS قد تم التصديق عليها وأن التركيبات الأساسية متاحة في المستودع.

## 1. المتطلبات الأساسية

1. مزامنة التركيبات القانونية والشركات:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. قم بإعداد دليل القبول الذي سيقرأه Torii في البداية (طريق المثال): `/var/lib/iroha/admission/sorafs`.
3. تأكد من أن تكوين Torii يمكنه ذاكرة التخزين المؤقت للاكتشاف وتطبيق القبول:

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. نشر معلومات عن القبول

1. انسخ عناوين القبول المعتمدة إلى الدليل المرجعي لـ `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Reinicia Torii (أو قم بإرسال SIGHUP إذا قمت بتشغيل اللودر مع إعادة الشحن الساخن).
3. قم بمراجعة سجلات رسائل القبول:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. التحقق من نشر الاكتشاف1. نشر إعلان موفر الحمولة (Bytes Norito) الذي تم إنتاجه من خلال خط الأنابيب الخاص بك:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. راجع نقطة نهاية الاكتشاف وتأكد من ظهور الإعلان بأسماء مستعارة قانونية:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   تأكد من أن `profile_aliases` يتضمن `"sorafs.sf1@1.0.0"` كمدخل أول.

## 4. اختبر نقاط النهاية للخطة والبيان

1. احصل على البيانات الوصفية للبيان (تتطلب رمزًا مميزًا للتدفق إذا كان القبول نشطًا):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. فحص خروج JSON والتحقق:
   - `chunk_profile_handle` هو `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` يتزامن مع تقرير التحديد.
   - `chunk_digests_blake3` يتم تجديده مع التركيبات.

## 5. اختبارات القياس عن بعد

- تأكيد أن Prometheus يعرض مقاييس الملف الشخصي الجديدة:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- يجب أن تعرض لوحات المعلومات مزود التدريج تحت الاسم المستعار المنتظر وتحافظ على مقاييس اللون البني بينما يكون الملف نشطًا.

## 6. التحضير للطرح

1. التقط تقريرًا قصيرًا باستخدام عناوين URL ومعرف البيان ولقطة القياس عن بعد.
2. قم بمقارنة التقرير في قناة بدء التشغيل Nexus مع نافذة التنشيط المخططة للإنتاج.
3. تابع قائمة التحقق من الإنتاج (القسم 4 في `chunker_registry_rollout_checklist.md`) عندما تكون الأجزاء المثيرة للاهتمام في المشاهدة جيدة.استمر في تحديث قواعد اللعبة هذه للتأكد من أن كل عملية نشر/قبول ستتبع نفس الخطوات التي يتم تحديدها بين العرض والإنتاج.