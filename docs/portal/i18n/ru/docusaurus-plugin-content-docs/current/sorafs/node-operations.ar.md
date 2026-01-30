---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/node-operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: node-operations
title: دليل تشغيل عمليات العقد
sidebar_label: تشغيل العقد
description: تحقق من نشر `sorafs-node` المضمّن داخل Torii.
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/runbooks/sorafs_node_ops.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم سحب مجموعة توثيق Sphinx القديمة.
:::

## نظرة عامة

يرشد هذا الدليل المشغلين خلال التحقق من نشر `sorafs-node` المضمّن داخل Torii. يتطابق
كل قسم مباشرةً مع مخرجات SF-3: جولات pin/fetch، واستعادة ما بعد إعادة التشغيل،
ورفض الحصص، وأخذ عينات PoR.

## 1. المتطلبات المسبقة

- فعّل عامل التخزين في `torii.sorafs.storage`:

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

- تأكّد من أن عملية Torii تملك صلاحيات القراءة/الكتابة على `data_dir`.
- تحقّق من أن العقدة تعلن السعة المتوقعة عبر `GET /v1/sorafs/capacity/state` بعد
  تسجيل تصريح.
- عند تمكين التنعيم، تعرض لوحات المتابعة عدادات GiB·hour/PoR الخام والمُنعّمة
  لإبراز الاتجاهات الخالية من التذبذب جنبًا إلى جنب مع القيم اللحظية.

### تشغيل تجريبي عبر CLI (اختياري)

قبل إتاحة نقاط النهاية HTTP يمكنك إجراء فحص سلامة لخلفية التخزين باستخدام CLI
المرفقة.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

تطبع الأوامر ملخصات Norito JSON وترفض عدم تطابق ملفات تعريف المقاطع أو digest،
ما يجعلها مفيدة لاختبارات الدخان في CI قبل توصيل Torii.【crates/sorafs_node/tests/cli.rs#L1】

### تمرين إثبات PoR

يمكن للمشغلين الآن إعادة تشغيل آرتيفاكتات PoR الصادرة عن الحوكمة محليًا قبل
رفعها إلى Torii. تعيد CLI استخدام مسار الإدخال نفسه في `sorafs-node`، لذا تكشف
التشغيلات المحلية عن أخطاء التحقق الدقيقة نفسها التي ستعيدها واجهة HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

يُصدر الأمر ملخص JSON (digest المانيفست، معرّف المزوّد، digest الإثبات، عدد
العينات، ونتيجة الحكم الاختيارية). وفّر `--manifest-id=<hex>` لضمان تطابق
المانيفست المخزّن مع digest التحدي، و`--json-out=<path>` عندما تريد أرشفة
الملخص مع الآرتيفاكتات الأصلية كدليل تدقيق. إدراج `--verdict` يتيح لك تمرين
حلقة التحدي → الإثبات → الحكم كاملةً دون اتصال قبل استدعاء واجهة HTTP.

بمجرد تشغيل Torii يمكنك استرجاع الآرتيفاكتات نفسها عبر HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

يتم تقديم كلا نقطتي النهاية بواسطة عامل التخزين المضمّن، لذا تبقى اختبارات
الدخان عبر CLI واستقصاءات البوابة متزامنة.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. جولة Pin → Fetch

1. أنشئ حزمة مانيفست + حمولة (على سبيل المثال عبر
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. أرسل المانيفست بترميز base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   يجب أن يحتوي JSON الطلب على `manifest_b64` و`payload_b64`. تعيد الاستجابة
   الناجحة `manifest_id_hex` وdigest الحمولة.
3. اجلب البيانات المثبتة:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   فك ترميز base64 للحقل `data_b64` وتحقق من تطابقه مع البايتات الأصلية.

## 3. تمرين استعادة ما بعد إعادة التشغيل

1. ثبّت مانيفستًا واحدًا على الأقل كما في الأعلى.
2. أعد تشغيل عملية Torii (أو العقدة كاملةً).
3. أعد إرسال طلب الجلب. يجب أن تبقى الحمولة قابلة للاسترجاع وأن يتطابق digest
   المُعاد مع القيمة السابقة لإعادة التشغيل.
4. افحص `GET /v1/sorafs/storage/state` للتأكد من أن `bytes_used` يعكس
   المانيفستات المحفوظة بعد إعادة التشغيل.

## 4. اختبار رفض الحصة

1. اخفض مؤقتًا `torii.sorafs.storage.max_capacity_bytes` إلى قيمة صغيرة (مثل حجم
   مانيفست واحد).
2. ثبّت مانيفستًا واحدًا؛ يجب أن ينجح الطلب.
3. حاول تثبيت مانيفست ثانٍ بحجم مشابه. يجب أن ترفض Torii الطلب مع HTTP `400`
   ورسالة خطأ تحتوي على `storage capacity exceeded`.
4. استعد حد السعة الطبيعي عند الانتهاء.

## 5. فحص أخذ عينات PoR

1. ثبّت مانيفستًا.
2. اطلب عينة PoR:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. تحقّق من أن الاستجابة تحتوي على `samples` بعدد العينات المطلوبة وأن كل إثبات
   يصحّ مقابل جذر المانيفست المخزّن.

## 6. خطافات الأتمتة

- يمكن لاختبارات CI / الدخان إعادة استخدام الفحوصات المستهدفة المضافة في:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  والتي تغطي `pin_fetch_roundtrip` و`pin_survives_restart` و`pin_quota_rejection`
  و`por_sampling_returns_verified_proofs`.
- يجب أن تتابع لوحات المتابعة:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` و`torii_sorafs_storage_fetch_inflight`
  - عدادات نجاح/فشل PoR المعروضة عبر `/v1/sorafs/capacity/state`
  - محاولات نشر التسوية عبر `sorafs_node_deal_publish_total{result=success|failure}`

يضمن اتباع هذه التدريبات أن عامل التخزين المضمّن قادر على إدخال البيانات،
والصمود أمام عمليات إعادة التشغيل، واحترام الحصص المضبوطة، وتوليد إثباتات PoR
حتمية قبل أن تعلن العقدة السعة للشبكة الأوسع.
