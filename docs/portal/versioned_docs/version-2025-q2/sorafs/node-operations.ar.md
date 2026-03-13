---
lang: ar
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T15:38:30.655980+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: node-operations-ar
slug: /sorafs/node-operations-ar
---

:::ملاحظة المصدر الكنسي
المرايا `docs/source/sorafs/runbooks/sorafs_node_ops.md`. حافظ على محاذاة كلا النسختين عبر الإصدارات.
:::

## نظرة عامة

يوجه دليل التشغيل هذا المشغلين عبر التحقق من صحة نشر `sorafs-node` المضمن داخل Torii. يعين كل قسم مباشرة مخرجات SF-3: رحلات الذهاب والإياب ذهابًا وإيابًا، وإعادة تشغيل الاسترداد، ورفض الحصص، وأخذ عينات PoR.

## 1. المتطلبات الأساسية

- تمكين عامل التخزين في `torii.sorafs.storage`:

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- تأكد من أن عملية Torii لديها حق الوصول للقراءة/الكتابة إلى `data_dir`.
- تأكد من أن العقدة تعلن عن السعة المتوقعة عبر `GET /v2/sorafs/capacity/state` بمجرد تسجيل الإعلان.
- عند تمكين التجانس، تعرض لوحات المعلومات كلاً من عدادات GiB·hour/PoR الأولية والملساء لتسليط الضوء على الاتجاهات الخالية من الارتعاش إلى جانب القيم الفورية.

### التشغيل الجاف لـ CLI (اختياري)

قبل الكشف عن نقاط نهاية HTTP، يمكنك التحقق من سلامة الواجهة الخلفية للتخزين باستخدام واجهة سطر الأوامر (CLI) المجمعة.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

تقوم الأوامر بطباعة ملخصات Norito JSON وترفض ملف تعريف القطعة أو عدم تطابق الملخص، مما يجعلها مفيدة لعمليات فحص دخان CI قبل توصيل أسلاك Torii.

بمجرد نشر Torii، يمكنك استرداد نفس العناصر عبر HTTP:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

تتم خدمة كلا نقطتي النهاية بواسطة عامل التخزين المضمن، لذلك تظل اختبارات دخان CLI ومسبارات البوابة متزامنة.

## 2. دبوس → جلب رحلة ذهابًا وإيابًا

1. قم بإنشاء بيان + حزمة حمولة (على سبيل المثال مع `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. أرسل البيان بتشفير base64:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   يجب أن يحتوي طلب JSON على `manifest_b64` و`payload_b64`. تؤدي الاستجابة الناجحة إلى إرجاع `manifest_id_hex` وملخص الحمولة.
3. جلب البيانات المثبتة:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Base64-فك تشفير الحقل `data_b64` والتحقق من مطابقته للبايتات الأصلية.

## 3. أعد تشغيل تمرين الاسترداد

1. قم بتثبيت بيان واحد على الأقل كما هو مذكور أعلاه.
2. أعد تشغيل عملية Torii (أو العقدة بأكملها).
3. إعادة تقديم طلب الجلب. يجب أن تظل الحمولة قابلة للاسترجاع ويجب أن يتطابق الملخص الذي تم إرجاعه مع قيمة ما قبل إعادة التشغيل.
4. افحص `GET /v2/sorafs/storage/state` للتأكد من أن `bytes_used` يعكس البيانات المستمرة بعد إعادة التشغيل.

## 4. اختبار رفض الحصص

1. قم بخفض `torii.sorafs.storage.max_capacity_bytes` مؤقتًا إلى قيمة صغيرة (على سبيل المثال حجم بيان واحد).
2. قم بتثبيت بيان واحد؛ يجب أن ينجح الطلب.
3. حاول تثبيت بيان ثانٍ بنفس الحجم. يجب أن يرفض Torii الطلب باستخدام HTTP `400` ورسالة خطأ تحتوي على `storage capacity exceeded`.
4. قم باستعادة الحد الطبيعي للسعة عند الانتهاء.

## 5. مسبار أخذ عينات PoR

1. قم بتثبيت البيان.
2. اطلب عينة PoR:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. تحقق من أن الاستجابة تحتوي على `samples` مع العدد المطلوب وأن كل دليل يتم التحقق من صحته مقابل جذر البيان المخزن.

## 6. خطافات الأتمتة

- يمكن لاختبارات CI / الدخان إعادة استخدام الفحوصات المستهدفة المضافة في:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```والذي يغطي `pin_fetch_roundtrip`، و`pin_survives_restart`، و`pin_quota_rejection`، و`por_sampling_returns_verified_proofs`.
- يجب على لوحات المعلومات تتبع:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` و`torii_sorafs_storage_fetch_inflight`
  - ظهرت عدادات نجاح/فشل PoR عبر `/v2/sorafs/capacity/state`
  - محاولات نشر التسوية عبر `sorafs_node_deal_publish_total{result=success|failure}`

يضمن اتباع هذه التدريبات أن يتمكن عامل التخزين المضمن من استيعاب البيانات، والبقاء على قيد الحياة في عمليات إعادة التشغيل، واحترام الحصص التي تم تكوينها، وإنشاء أدلة إثبات الأداء الحتمية قبل أن تعلن العقدة عن السعة للشبكة الأوسع.
