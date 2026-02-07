---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/staging-manifest-playbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: دليل التدريج
العنوان: SoraFS دليل التشغيل للبيان المرحلي
Sidebar_label: SoraFS دليل التشغيل لبيان التدريج
الوصف: Torii عمليات النشر المرحلية وفقًا لملف تعريف القطع المعتمد من البرلمان، قائمة التحقق الفعالة.
---

:::ملاحظة مستند ماخذ
:::

## نظرة عامة

نشر دليل التشغيل المرحلي Torii في الملف التعريفي للمقطع المعتمد من البرلمان، وهو عبارة عن مراحل فعالة من الإنشاءات وبدء الإنتاج مما يؤدي إلى تعزيز عملية الإنتاج. يفترض أن ميثاق الحوكمة SoraFS يصادق على ما هو موجود ومستودع التركيبات الأساسي موجود.

## 1. المتطلبات الأساسية

1. مزامنة التركيبات والتوقيعات الأساسية:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. قم ببدء تشغيل مظاريف القبول في دليل بدء التشغيل Torii (مثال للمسار): `/var/lib/iroha/admission/sorafs`.
3. أداة Torii لذاكرة التخزين المؤقت لاكتشاف التكوين وإنفاذ القبول لتمكين البطاقة:

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

## 2. نشر مظاريف القبول کریں

1. نسخة من مظاريف قبول مقدم الخدمة المعتمد `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. قم بإعادة تشغيل Torii (أو إذا تم إعادة التحميل السريع للمحمل ثم قم بالتسجيل).
3. رسائل القبول الخاصة بسجلات الدخول:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. التحقق من صحة انتشار الاكتشاف

1. خط أنابيب الموفر سے بنے ہوئے حمولة إعلان الموفر الموقعة (Norito بايت) کو نشر کریں:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```2. اكتشف استعلام نقطة النهاية وتأكد من كتابة الأسماء المستعارة الأساسية التي ينظر إليها على النحو التالي:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   هذا هو الإدخال الأول الذي يشمل `profile_aliases` `"sorafs.sf1@1.0.0"` وهو شامل تمامًا.

## 4. تمرين نقاط النهاية للبيان والتخطيط

1. جلب بيانات التعريف الواضحة (إذا تم فرض القبول، قم بدفق الرمز المميز مرة أخرى):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. فحص مخرجات JSON والتحقق من الكتابة:
   - `chunk_profile_handle`، `sorafs.sf1@1.0.0` ہو۔
   - تقرير الحتمية `manifest_digest_hex` سے تطابق کرے۔
   - `chunk_digests_blake3` التركيبات المجددة سے محاذاة ہوں۔

## 5. فحوصات القياس عن بعد

- التحقق من Prometheus مقاييس الملف الشخصي الجديدة تكشف عن الهوية:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- لوحات المعلومات وموفر التدريج الاسم المستعار المتوقع کے تحت دکھانا چاہیے والملف الشخصي نشط ہونے پر عدادات انقطاع التيار صفر رہنے چاہییں.

## 6. الاستعداد للطرح

1. عناوين URL ومعرف البيان ولقطة القياس عن بعد هي عبارة عن تقرير مختصر.
2. يعرض التقرير Nexus قناة الطرح نافذة تنشيط الإنتاج المخطط لها والتي ستستمر في العمل.
3. يقوم أصحاب المصلحة بالتوقيع بعد قائمة مراجعة الإنتاج (القسم 4 في `chunker_registry_rollout_checklist.md`).

يتضمن كتاب قواعد اللعبة هذا العديد من الخطوات التي تتضمن مراحل طرح القبول/التقطيع والإنتاج، وهي خطوة حتمية في كل شيء.