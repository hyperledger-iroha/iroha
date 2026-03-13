---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/staging-manifest-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: دليل التدريج
العنوان: دليل مانيفست الـstaging
Sidebar_label: دليل مانيفست الـstaging
الوصف: قم بقائمة التحقق الخاصة بك من ملف Chunker المصادق عليه سياسياً على نشرات Torii بـ staging.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. احرص على نسخة Docusaurus ونسخة Markdown القديمة متطابقتين حتى يتم إيقاف مجموعة Sphinx بالكامل.
:::

## نظرة عامة

يرشد هذا الدليل إلى فاس ملف Chunker المصادق عليه برلمانياً في نشر Torii الخاص بـ التدريج قبل تطوير التغيير الجديد للإنتاج. حيث أنها تعتبر ميثاقاً SoraFS تم التصديق عليه وأن الـ Installations معتمدة داخل المستودع.

## 1.المتطلبات المسبقة

1. زمان الـ التركيبات المعتمدة والتواقيع:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. حصل على دليل أظرف النسخة التي تمت نسخه Torii عند الإقلاع (مسار المثال): `/var/lib/iroha/admission/sorafs`.
3. تأكد من أن إعدادات Torii فعّل مخبأ الاكتشاف وتطبيقات:

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

## 2. نشر أظرف تقبل

1. انسخ أظرف قبول المحاسبين المعتمدين للدليل المرسل إليه في `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. إعادة تشغيل Torii (أو أرسل SIGHUP إذا كانت أداة التحميل تدعم إعادة التحميل الفوري).
3. راقب تسجيل لرسائل التراث:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. التحقق من اكتشاف الانتشار

1. موقع إعلان مزود خدمة النشر (بايتات Norito) يحدث عن خط المتحكم:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. استعلم عن نقطة الاكتشاف وتأكد من ظهور الإعلان مع الأسماء المستعارة المعتمدة:```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   تأكد من أن `profile_aliases` تتضمن `"sorafs.sf1@1.0.0"` كأول التدفق.

## 4. اختبار نقاط نهاية البيان والخطة

1. جلب البيانات الوصفية الواضحة (يطلب رمز الدفق المميز إذا كان مفعّلًا):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. يتم فحص إخراج JSON والتحقق من:
   - أن `chunk_profile_handle` يساوي `sorafs.sf1@1.0.0`.
   - أن `manifest_digest_hex` يتوافق مع تقرير الحتمية.
   - أن `chunk_digests_blake3` تتطابق مع الـ تركيبات المعاد توليدها.

## 5. الفحوصات التليميترية

- التأكد من أن Prometheus المعدة للمعايير الملف الجديد:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- يجب أن يتم التقاط لوحات متابعة من خلال التدريج تحت الاسم المستعار وأن تعيش عدادات Brownout عند تفعيل الملف.

##6.جاهزية للطلاق

1. تم التقاط تقريرًا مختصرًا يشمل عناوين URL ومعرف البيان ولقطة التليميرية.
2. شارك التقرير في قناة Nexus rollout مع نافذة التفعيل فهي في الإنتاج.
3. انتقل إلى تسجيل الشهادات الخاصة بالإنتاج (القسم 4 في `chunker_registry_rollout_checklist.md`) بعد تسجيل الأشخاص الأصليين.

يضمن هذا الدليل محدثًا أن كل مجموعة متكاملة لـchunker/admission تتبع نفس الخطوات الحتمية بين التدريج والإنتاج.