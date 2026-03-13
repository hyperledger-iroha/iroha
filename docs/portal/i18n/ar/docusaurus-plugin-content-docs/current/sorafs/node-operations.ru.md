---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات العقدة
العنوان: عملية رانبوك
Sidebar_label: تشغيل عملية التشغيل
الوصف: مدقق داخلي `sorafs-node` منافذ Torii.
---

:::note Канонический источник
هذا الجزء يعرض `docs/source/sorafs/runbooks/sorafs_node_ops.md`. قم بالنسخ المتزامن، من خلال مجموعة كبيرة من الوثائق، أبو الهول لن ينشأ من الانتهاك.
:::

## ملاحظة

هذا المشغل المجهز من خلال التحقق من التوزيع الداخلي `sorafs-node`
Torii. كل ما عليك فعله هو تسليم التسليمات المرغوبة SF-3: دبوس/جلب،
تم تعزيزها بعد إعادة التدوير، بسبب السعر ونماذج PoR.

## 1. الخدمة المسبقة

- قم بإدراج عامل التخزين في `torii.sorafs.storage`:

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

- تأكد من أن العملية Torii ستؤدي إلى توصيل/تسجيل الدخول إلى `data_dir`.
- تأكد من أن المنتج يوصلك إلى هناك
  `GET /v2/sorafs/capacity/state` بعد كتابة الإعلانات.
- عند عرض لوحات التحكم الشاملة، يتم توضيح ما إذا كان الجو مناسبًا أم لا، وما هو الترتيب الذي تريده
  مراقبة GiB·hour/PoR لتتبع الاتجاهات دون الحاجة إلى الاهتزاز
  عبارات رائعة.

### واجهة سطر الأوامر (اختياري)

قبل فتح نقاط بروتوكول HTTP، يمكنك إثبات صحة التحقق من صحة الواجهة الخلفية باستخدام
قم بنشر CLI.[crates/sorafs_node/src/bin/sorafs-node.rs#L1]

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
```أوامر إصدار Norito السيرة الذاتية لـ JSON وتجاوز الملف الشخصي غير المرغوب فيه أو
خلاصة ما يتم تقديمه من فوائد لمختبر دخان CI قبل إضافة Torii.[crates/sorafs_node/tests/cli.rs#L1]

### التكرار PoR المصادقة

يمكن للمشغلين الآن أن ينشئوا عناصر PoR محليًا، والحوكمة الفعالة،
قبل أن تقوم بربطها في Torii. يستخدم CLI ذلك ويدخل `sorafs-node`، إلخ
أن التطورات المحلية توضح أنها عمليات التحقق من الصحة، والتي هي HTTP API.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

الأمر بإصدار السيرة الذاتية لـ JSON (بيان ملخص، مقدم معرف، ملخص تقديم،
количество сэплов, optionalьный نتيجة الحكم). تحميل `--manifest-id=<hex>`,
من أجل التأكد من أن البيان المصاحب مصاحب للخلاصة، و
`--json-out=<path>`، عندما تحتاج إلى أرشفة سيرتك الذاتية باستخدام المصنوعات اليدوية الأصلية
كيفية الإحالة للتدقيق. يتيح لك إضافة `--verdict` التكرار الكامل
سلسلة نقرات → الإحالة → الحكم في وضع عدم الاتصال بالإنترنت من خلال واجهة برمجة تطبيقات HTTP.

بعد الانتهاء من Torii، يمكنك الحصول على هذه العناصر عبر HTTP:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

تستخدم نقطة النهاية هذه عامل تخزين قوي، وهو اختبار دخان CLI و
بوابة الاختبار остаются синkhронизированными.[crates/iroha_torii/src/sorafs/api.rs#L1207] 【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. دبوس سيكل → جلب1. قم بتنسيق بيان الحزمة + الحمولة (على سبيل المثال، مع الإشارة)
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. تنفيذ البيان في الترميز base64:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   يقوم JSON بإعادة توصيل `manifest_b64` و`payload_b64`. رد سريع
   возвращает `manifest_id_hex` وهضم الحمولة.
3. احصل على البيانات المخفية:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   قم بفك تشفير القطب `data_b64` من base64 وتحقق مما إذا كان هذا متصلاً بالبيتاميات الأصلية.

## 3. تعزيز التدريب بعد الاستراحة

1. قم بإكمال الحد الأدنى من بيان أودين كما هو موضح.
2. عملية إعادة الإرسال Torii (أو كل شيء).
3. قم بالرجوع مرة أخرى للجلب. يتم إخراج الحمولة الزائدة من الحمولة، ملخصًا في
   تخلص من التكلفة العالية من خلال التوصيل المسبق.
4. تحقق من `GET /v2/sorafs/storage/state` لتتمكن من التحقق من أن `bytes_used` سيتم حذفه
   تظهر البيانات المصاحبة بعد التخفيض.

## 4. اختبار الخروج

1. حدد `torii.sorafs.storage.max_capacity_bytes` على الفور لبداية بسيطة
   (على سبيل المثال، حجم واحد فقط).
2. قم بإلغاء بيان واحد; احصل على شعر طويل الأمد بنجاح.
3. قم بمحاولة فك البيان الثاني بالحجم المناسب. Torii إلغاء الإلغاء الطويل
   قم بالاتصال عبر HTTP `400` واتصل بنا من خلال الاتصال بـ `storage capacity exceeded`.
4. من خلال الحفاظ على الجودة قبل الترطيب.

## 5. اختبر عينة PoR

1. قم بتعبئة البيان.
2. التحقق من اختيار المنتج:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```3. تحقق مما إذا كان `samples` مع المجموعة المخفضة وما هو السبب
   يتم التحقق من صحة المصادقة على بيان الرعاية الصحية الأولية.

##6.أتمتة عالية

- يمكن استخدام اختبارات CI / الدخان مرة أخرى للتحقق من مدى الصحة، مما يضيف إلى:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  التي تم إنشاؤها `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`
  و `por_sampling_returns_verified_proofs`.
- تفاصيل لوحة المفاتيح:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` و `torii_sorafs_storage_fetch_inflight`
  - مذكرات النجاح/المسؤولية PoR، متاحة للجمهور من خلال `/v2/sorafs/capacity/state`
  - تسوية منشورات الدفع عبر `sorafs_node_deal_publish_total{result=success|failure}`

ضمان هذا العنصر من الحماية أنه من الممكن أن يكون عامل التخزين جاهزًا
استيعاب البيانات، والبقاء على قيد الحياة، والإشادة بالأفكار المبهجة، وإنشاءها
يتم تحديد تفويض PoR حتى ذلك الحين، حيث تلتزم المجموعة الكاملة بالمجموعات الكاملة.