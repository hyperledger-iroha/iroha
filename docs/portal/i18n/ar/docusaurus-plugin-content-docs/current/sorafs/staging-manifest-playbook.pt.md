---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/staging-manifest-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: دليل التدريج
العنوان: قواعد اللعب في البيان المرحلي
Sidebar_label: دليل التشغيل للبيان المرحلي
الوصف: قائمة مرجعية لتأهيل ملف التقطيع المعتمد من قبل البرلمان في عمليات النشر المرحلية Torii.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة تحمل عنوان `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Mantenha ambas as copias sincronzadas.
:::

## فيساو جيرال

يشرح هذا الدليل كيفية تأهيل ملف القطع المعتمد من قبل البرلمان لنشر Torii للتدريج المسبق للترويج لتعديل الإنتاج. يفترض أنه تم التصديق على المذكرة الحاكمة لـ SoraFS وأن التركيبات Canonicos متاحة في المستودع.

## 1. المتطلبات الأساسية

1. مزامنة التركيبات الكنسيه والمثبتة:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. قم بإعداد دليل مظاريف القبول الذي يوضح Torii عند عدم بدء التشغيل (مثال: `/var/lib/iroha/admission/sorafs`).
3. ضمان إمكانية تكوين Torii لاكتشاف ذاكرة التخزين المؤقت وتنفيذ القبول:

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

## 2. مظاريف القبول العامة

1. انسخ مظاريف قبول موفر نظام التشغيل المعتمدة للدليل المرجعي لـ `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. قم بالتحديث إلى Torii (أو ترحب بـ SIGHUP إذا قمت بتحميل أداة التحميل من خلال hot reload).
3. مرافقة سجلات رسائل القبول:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. التحقق من صحة نشر الاكتشاف1. نشر إعلان الموفر الذي تم تحميله (بايت Norito) من خلال خط الأنابيب المنتج:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. راجع نقطة نهاية الاكتشاف وتأكد من ظهور الإعلان باستخدام الأسماء المستعارة لـ Canon:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   ضمان أن `profile_aliases` يتضمن `"sorafs.sf1@1.0.0"` كمدخل أول.

## 4. تمرين على نقاط النهاية للخطة

1. البحث عن البيانات الوصفية (يتم فرض رمز الدفق exige مع فرض القبول):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. فحص JSON والتحقق:
   - `chunk_profile_handle` و `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` يتوافق مع علاقة التحديد.
   - `chunk_digests_blake3` alinham com os Installations regenerados.

## 5. فحوصات القياس عن بعد

- تأكد من عرض Prometheus كما تفعل المقاييس الجديدة:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- يجب أن تعرض لوحات المعلومات مزود التدريج أو الأسماء المستعارة المنتظرة وتدير متحكمات التوقف عن التشغيل عند الصفر أثناء الملف الشخصي.

## 6. الاستعداد للطرح

1. التقط رابطًا قصيرًا لعناوين URL ومعرف البيان ولقطة القياس عن بعد.
2. قم بمشاركة رابط قناة الطرح Nexus مع مخطط التنشيط المنتج.
3. معالجة قائمة التحقق من المنتج (القسم 4 في `chunker_registry_rollout_checklist.md`) عندما تتم الموافقة على جزء من الاهتمام.

بالإضافة إلى ذلك، يضمن تحديث قواعد اللعبة هذه أن كل عملية نشر للقطع/القبول ستتبع نفس الخطوات التي يتم تحديدها بين مرحلة الإنتاج والإنتاج.