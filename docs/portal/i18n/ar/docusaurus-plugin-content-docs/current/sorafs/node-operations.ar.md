---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات العقدة
العنوان: دليل تشغيل عمليات العقد
Sidebar_label: تشغيل العقد
الوصف: تحقق من نشر `sorafs-node` المزخرف داخل Torii.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/runbooks/sorafs_node_ops.md`. احرص على جميع النسختين متزامنتين إلى أن يتم تكريم مجموعة Sphinx القديمة.
:::

## نظرة عامة

يرشد هذا الدليل للبدء من خلال التحقق من نشر I18NI0000 داخل المتن Torii. يتطابق
كل قسم النباتي مع مخرجات SF-3: توقف الدبوس/الجلب، واستعادة ما بعد إعادة التشغيل،
ورفض الحصص، وغير ذلك PoR.

## 1.المتطلبات المسبقة

- عامل تشغيل في `torii.sorafs.storage`:

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
- تحقّق من أن تعلن السعة عبر `GET /v1/sorafs/capacity/state` بعد
  أكمل التسجيل.
- عند استخدام التنعيم، تم التقاط اللوحات بعد عددات GiB·hour/PoR الخام والمُنعّمة
  براز اتجاهات من الذبائح جنباً إلى جنب مع القيم اللحظية.

### تشغيل السعر عبر CLI (اختياري)

قبل إتاحة نقاط النهاية HTTP يمكنك إجراء فحص آمن لخيار التخزين باستخدام CLI
مرفقة.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

تطبع ملخصات Norito JSON وترفض عدم تطابق ملفات تعريف المقاطع أو الهضم،
ما يجب أن يكون دمجات تندرج في CI قبل توصيل Torii.【crates/sorafs_node/tests/cli.rs#L1】

### تمرين يؤكد PoRيمكن الآن تشغيل إعادة مقالة PoR الصادرة عن الإقليم المحلي من قبل
رفعها إلى Torii. يتضمن CLI استخدام مسار الإدخال بنفسه في `sorafs-node`، لذا نسيت
التشغيل التلقائي عن الأخطاء التحقق من صحة التفاصيل التي استعادتها واجهة HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

يُصدر الأمر ملخص JSON (ملخص المانيفست، معرّف المدقق، ملخص الإثبات، عدد
اتخذ القرار الاختياري). وفّر `--manifest-id=<hex>` للتطابق
المانيفست المخزّن مع الهضم، و`--json-out=<path>` عندما تريد أرشفة
الملخص مع الآرتيفاكتات الأصلية كدليل دقيق. إدراج `--verdict` يتيح لك التمرين
الحلقة الصحيحة → الإثبات → الحكم الكامل دون الاتصال قبل الاتصال بواجهة HTTP.

بمجرد تشغيل Torii يمكنك استرجاع الآرتيفاكتات بنفسك عبر HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

يتم تقديم نقطتي النهائية بواسطة عامل التخزين المتوفر، لذا تستمر السيولة
ترانيم عبر CLI واستقصاءات البوابة متزامنة.

## 2. دبوس دائري → جلب

1. أنشئ حزمة مانيفست + حمولة (على سبيل المثال عبر
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. أرسل المانيفست بترميز base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   يجب أن يحتوي على طلب JSON على `manifest_b64` و`payload_b64`. شكله
   الفائز `manifest_id_hex` والملخص المحمول.
3. احضار البيانات المثبتة :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   فك ترميز base64 للقل `data_b64` والتحقق من تطابقه مع البايتات الأصلية.

## 3. تمرين ما بعد إعادة التشغيل1. ثبت مانيفيستًا على الأقل كما في الأعلى.
2. إعادة تشغيل العملية Torii (أو العقدة الكاملة).
3. إعادة صياغة طلب الجلب. يجب أن تبقى قابلة للنقل للاسترجاع وأن يتطابق الملخص
   المُعاد مع القيمة السابقة لإعادة التشغيل.
4. قم بالفحص `GET /v1/sorafs/storage/state` للتأكد من أن `bytes_used`
   المانيفيستات المحفوظة بعد إعادة التشغيل.

## 4. اختبار الرفض الحصة

1. خفض مؤقتًا `torii.sorafs.storage.max_capacity_bytes` إلى قيمة صغيرة (مثل الحجم
   بيان واحد).
2. ثبت مانيفيستًا؛ يجب أن ينجح الطلب.
3. حاول تثبيت مانيفيست ثانٍ بحجم مماثل. يجب أن ترفض الطلب Torii مع HTTP `400`
   ورسالة تحتوي على خطأ على `storage capacity exceeded`.
4. استعد لحد السعة الطبيعية عند الانتهاء.

## 5. فحص عينات PoR

1. ثبت مانيفستًا.
2. استعادة PoR:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. تحتوي على حق التقدم بأن `samples` المتقدمة المتقدمة وأن كل ما يثبت
   يصحّ مقابل جذر المانيفيست المخزّن.

## 6. خطافات المطبوعات

- يمكن دمجات CI / إعادة استخدام الفرق إضافة إلى:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  والتي تغطي `pin_fetch_roundtrip` و`pin_survives_restart` و`pin_quota_rejection`
  و`por_sampling_returns_verified_proofs`.
- يجب أن تتابع اللوحات للمتابعة:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` و`torii_sorafs_storage_fetch_inflight`
  - عدد النجاح/الفشل PoR وبالتالي عبر `/v1/sorafs/capacity/state`
  - بهدف نشر الترخيص عبر `sorafs_node_deal_publish_total{result=success|failure}`بما في ذلك اتباع هذه التدريبات أن عامل التخزين المتنوع قادر على تخزين البيانات،
والصمود أمام عمليات إعادة التشغيل، للجميع الحصص المضبوطة، وتوليد إثباتات PoR
حتمية قبل أن تعلن مؤتمر السعة للشبكة الأوسع.