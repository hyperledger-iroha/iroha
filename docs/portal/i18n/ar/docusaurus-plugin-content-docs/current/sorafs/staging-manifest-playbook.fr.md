---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/staging-manifest-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: دليل التدريج
العنوان: Playbook de Manifest en Stage
Sidebar_label: دليل التشغيل للبيان والتدريج
الوصف: قائمة مرجعية لتنشيط ملف التعريف الخاص بـ Chunker المعتمد من قبل Parlement sur les déploiements Torii de staging.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. قم بحفظ النسخة Docusaurus وعلامة Markdown الموروثة حتى النهاية الكاملة لمجموعة Sphinx.
:::

## عرض الفرقة

يوضح هذا الدليل تنشيط ملف التعريف الذي تم التصديق عليه من قبل البرلمان حول نشر Torii للتدريج قبل تشجيع التغيير في الإنتاج. لنفترض أنه تم التصديق على ميثاق الحوكمة SoraFS وأن التركيبات الأساسية متاحة في الوديعة.

## 1. المتطلبات الأساسية

1. مزامنة التركيبات الأساسية والتوقيعات:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. قم بإعداد قائمة مغلفات القبول التي Torii من خلال البدء (طريق المثال): `/var/lib/iroha/admission/sorafs`.
3. تأكد من التكوين Torii النشط لاكتشاف ذاكرة التخزين المؤقت وتطبيق القبول:

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

## 2. نشر مغلفات القبول

1. انسخ مغلفات القبول المعتمدة في المرجع المرجعي وفقًا لـ `torii.sorafs.discovery.admission.envelopes_dir` :

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```2. Redémarrez Torii (أو قم بإرسال SIGHUP إذا قمت بتغليف المحمل بإعادة شحن ساخنة).
3. قم بمتابعة السجلات لرسائل القبول :

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. التحقق من اكتشاف الانتشار

1. ضع علامة الحمولة النافعة على مزود الإعلان (الثمانيات Norito) المنتج من قبل مزود خط الأنابيب الخاص بك:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. استفسر عن اكتشاف نقطة النهاية وتأكد من ظهور الإعلان باستخدام الأسماء المستعارة التقليدية:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   تأكد من أن `profile_aliases` يتضمن `"sorafs.sf1@1.0.0"` عند الإدخال الأول.

## 4. ممارسة نقاط النهاية والخطة

1. استرجع بيانات البيان (تحتاج إلى رمز دفق مميز إذا كان القبول مطبقًا):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. افحص عملية JSON وتحقق منها:
   - `chunk_profile_handle` هو `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` يتوافق مع تقرير التحديد.
   - `chunk_digests_blake3` تتم محاذاته مع التركيبات المُعاد تركيبها.

## 5. التحقق من القياس عن بعد

- تأكد من أن Prometheus يعرض مقاييس الملف الشخصي الجديدة:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- يجب أن تقوم لوحات المعلومات بضبط مزود التدريج تحت الاسم المستعار للانتظار والحفاظ على عدادات الطاقة المنخفضة حتى الصفر حتى يكون الملف الشخصي نشطًا.

## 6. التحضير للطرح1. قم بالتقاط محضر المحكمة باستخدام عناوين URL ومعرف البيان ولقطة الفيديو عن بعد.
2. قم بمشاركة الرابط في قناة الطرح Nexus مع نافذة التنشيط المخططة للإنتاج.
3. انتقل إلى قائمة التحقق من الإنتاج (القسم 4 في `chunker_registry_rollout_checklist.md`) حتى لا تتفق الأطراف المطروحة عليها.

الحفاظ على قواعد اللعبة هذه في اليوم تضمن أن كل عملية طرح/قبول تناسب الخطوات المماثلة التي تحدد بين العرض والإنتاج.