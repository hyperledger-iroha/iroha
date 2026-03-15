---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f168351fabbd55ee1400aaf51e4329b0c268a4ea07c58094a0f943be48d3baf
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: staging-manifest-playbook
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/staging-manifest-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. احرص على إبقاء نسخة Docusaurus ونسخة Markdown القديمة متطابقتين حتى يتم إيقاف مجموعة Sphinx بالكامل.
:::

## نظرة عامة

يرشد هذا الدليل إلى تمكين ملف chunker المصادق عليه برلمانياً في نشر Torii الخاص بـ staging قبل ترقية التغيير إلى الإنتاج. يفترض أن ميثاق حوكمة SoraFS تم التصديق عليه وأن الـ fixtures المعتمدة متاحة داخل المستودع.

## 1. المتطلبات المسبقة

1. زامن الـ fixtures المعتمدة والتواقيع:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. حضّر دليل أظرف القبول الذي يقرأه Torii عند الإقلاع (مسار مثال): `/var/lib/iroha/admission/sorafs`.
3. تأكد من أن إعدادات Torii تفعّل مخبأ discovery وتطبيق القبول:

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

## 2. نشر أظرف القبول

1. انسخ أظرف قبول المزوّدين المعتمدة إلى الدليل المشار إليه في `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. أعد تشغيل Torii (أو أرسل SIGHUP إذا كانت أداة التحميل تدعم إعادة التحميل الفورية).
3. راقب السجلات لرسائل القبول:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. التحقق من انتشار discovery

1. انشر حمولة provider advert الموقعة (بايتات Norito) الناتجة عن خط المزوّد:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. استعلم عن نقطة discovery وتأكد من ظهور الإعلان مع الأسماء المستعارة المعتمدة:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   تأكد من أن `profile_aliases` تتضمن `"sorafs.sf1@1.0.0"` كأول إدخال.

## 4. اختبار نقاط نهاية manifest وplan

1. اجلب بيانات manifest الوصفية (يتطلب stream token إذا كان القبول مفعّلًا):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. افحص إخراج JSON وتحقق من:
   - أن `chunk_profile_handle` يساوي `sorafs.sf1@1.0.0`.
   - أن `manifest_digest_hex` يطابق تقرير الحتمية.
   - أن `chunk_digests_blake3` تتطابق مع الـ fixtures المعاد توليدها.

## 5. فحوصات التليمترية

- تأكد من أن Prometheus يعرض مقاييس الملف الجديد:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- يجب أن تعرض لوحات المتابعة مزوّد staging تحت الاسم المستعار المتوقع وأن تبقى عدادات brownout عند الصفر أثناء تفعيل الملف.

## 6. الجاهزية للإطلاق

1. التقط تقريرًا مختصرًا يتضمن عناوين URL ومعرّف manifest ولقطة التليمترية.
2. شارك التقرير في قناة Nexus rollout مع نافذة التفعيل المخطط لها في الإنتاج.
3. انتقل إلى قائمة التحقق الخاصة بالإنتاج (Section 4 في `chunker_registry_rollout_checklist.md`) بعد موافقة أصحاب المصلحة.

الحفاظ على هذا الدليل محدثًا يضمن أن كل إطلاق لـ chunker/admission يتبع نفس الخطوات الحتمية بين staging والإنتاج.
