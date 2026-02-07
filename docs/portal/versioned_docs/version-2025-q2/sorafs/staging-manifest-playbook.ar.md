---
lang: ar
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2026-01-03T18:07:58.297179+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

:::ملاحظة المصدر الكنسي
المرايا `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. حافظ على محاذاة كلا النسختين عبر الإصدارات.
:::

## نظرة عامة

يتناول دليل التشغيل هذا تمكين ملف تعريف القطع الذي تم التصديق عليه من قبل البرلمان على النشر المرحلي Torii قبل الترويج للتغيير إلى الإنتاج. ويفترض أنه تم التصديق على ميثاق الحوكمة SoraFS وأن التركيبات الأساسية متوفرة في المستودع.

## 1. المتطلبات الأساسية

1. مزامنة التركيبات والتوقيعات الأساسية:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. قم بإعداد دليل مغلف القبول الذي سيقرأه Torii عند بدء التشغيل (مثال للمسار): `/var/lib/iroha/admission/sorafs`.
3. تأكد من أن تكوين Torii يمكّن اكتشاف ذاكرة التخزين المؤقت وفرض القبول:

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

## 2. نشر مظاريف القبول

1. انسخ مظاريف قبول المزود المعتمد في الدليل المشار إليه بـ `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. أعد تشغيل Torii (أو أرسل SIGHUP إذا قمت بلف أداة التحميل بإعادة التحميل الفوري).
3. ذيل سجلات رسائل القبول:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. التحقق من صحة نشر الاكتشاف

1. انشر حمولة إعلان الموفر الموقع (Norito بايت) التي تم إنتاجها بواسطة
   خط أنابيب المزود:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. استعلم عن نقطة نهاية الاكتشاف وتأكد من ظهور الإعلان باستخدام الأسماء المستعارة الأساسية:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   تأكد من أن `profile_aliases` يتضمن `"sorafs.sf1@1.0.0"` كمدخل أول.

## 4. بيان التمرين ونقاط نهاية الخطة

1. قم بإحضار البيانات الوصفية للبيان (يتطلب رمزًا مميزًا للتدفق إذا تم فرض القبول):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. افحص مخرجات JSON وتحقق:
   - `chunk_profile_handle` هو `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` يطابق تقرير الحتمية.
   - `chunk_digests_blake3` تتماشى مع التركيبات المجددة.

## 5. فحوصات القياس عن بعد

- تأكد من أن Prometheus يعرض مقاييس الملف الشخصي الجديدة:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- يجب أن تعرض لوحات المعلومات موفر التدريج تحت الاسم المستعار المتوقع وتبقي عدادات التوقف عند الصفر أثناء تنشيط ملف التعريف.

## 6. الاستعداد للطرح

1. التقط تقريرًا قصيرًا يتضمن عناوين URL ومعرف البيان ولقطة القياس عن بعد.
2. قم بمشاركة التقرير في قناة طرح Nexus بجانب نافذة تنشيط الإنتاج المخطط لها.
3. انتقل إلى قائمة مراجعة الإنتاج (القسم 4 في `chunker_registry_rollout_checklist.md`) بمجرد توقيع أصحاب المصلحة.

يضمن الحفاظ على تحديث قواعد اللعبة هذه أن كل عملية طرح للقسم/القبول تتبع نفس الخطوات الحاسمة عبر التدريج والإنتاج.