---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/staging-manifest-playbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: دليل التدريج
عنوان: بيان مسرحي للتدريج
Sidebar_label: بيان مسرحي للتدريج
الوصف: قائمة اختيار تتضمن ملف تعريف Chunker، ومناقشة التصديق، وإعلان التدريج Torii.
---

:::note Канонический источник
:::

## ملاحظة

يشتمل هذا المنشور على ملف تعريف Chunker، ومناقشة التصديق، وكشف التدريج Torii قبل الإنتاج изменений в прод. من المفترض أن يتم التصديق على التثبيت SoraFS، ويتم تركيب التركيبات القياسية في المستودعات.

## 1. الخدمة المسبقة

1. مزامنة التركيبات والمواصفات الأساسية:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. قم بقراءة مظاريف قبول الكتالوج، التي Torii قبل البدء (الخطوات التمهيدية): `/var/lib/iroha/admission/sorafs`.
3. تابع أن التكوين Torii يتضمن ذاكرة التخزين المؤقت للاكتشاف وقبول التنفيذ:

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

## 2. مظاريف قبول النشر

1. انسخ مظاريف قبول الموفر المحددة في الكتالوج، الموضح في `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. قم بالدخول إلى Torii (أو قم بتفعيل SIGHUP، إذا قمت بإيقاف التشغيل السريع).
3. شروط تسجيل الدخول:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. التحقق من الاكتشاف

1. نشر حمولة إعلان موفر الخدمة (الصفحة Norito)، تنسيق مزود خط الأنابيب الخاص بك:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```2. قم بدعم اكتشاف نقطة النهاية والتحقق من أن الإعلان يتم الإعلان عنه باستخدام الأسماء المستعارة الأساسية:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   يرجى ملاحظة أن `profile_aliases` يربط `"sorafs.sf1@1.0.0"` بالعنصر الأول.

## 4. بيان وخطة التحقق من نقاط النهاية

1. الحصول على بيان البيانات الوصفية (يلزم استخدام رمز الدفق المميز، في حالة فرض القبول):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. التحقق من صحة JSON والتأكد من أن:
   - `chunk_profile_handle` رافين `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` متضمن بحتمية عالية.
   - `chunk_digests_blake3` مرتبط بالتركيبات المجددة.

## 5. قياس المسافة

- يرجى ملاحظة أن Prometheus نشر ملف تعريف المقاييس الجديد:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- تحتاج لوحة المفاتيح إلى إظهار التدريج من خلال الاسم المستعار المثبت وحذف ملفات تعريف الارتباط من اللون، حتى يكون الملف الشخصي نشطًا.

## 6.بدء التشغيل

1. قم بتنسيق القائمة باستخدام عناوين URL وبيان المعرف وأجهزة قياس اللقطة عن بعد.
2. قم بتمكين الطرح عبر القناة Nexus من خلال تنشيط علامة التبويب المخطط لها في البيع.
3. قم بالمتابعة إلى قائمة البيع (القسم 4 في `chunker_registry_rollout_checklist.md`) بعد الاتصال بحاملي الزجاجات.

يضمن دعم هذا النوع من الخدمات فعليًا أن كل من قطع الطرح/القبول هو أحد العناصر المحددة والموضوع بين التدريج والإنتاج.