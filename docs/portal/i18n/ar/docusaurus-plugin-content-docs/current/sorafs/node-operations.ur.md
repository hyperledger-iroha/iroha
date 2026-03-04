---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات العقدة
العنوان: نوڈ ابريشنز رن بک
Sidebar_label: التعليقات الجديدة رن بک
الوصف: Torii اندرويد `sorafs-node` مناسب للكريكر الصحيح.
---

:::ملاحظة مستند ماخذ
هذه هي الصفحة `docs/source/sorafs/runbooks/sorafs_node_ops.md`. عندما ينتقل برانا أبو الهول إلى عالم كامل، لا يمكننا أن نقول ما هو أفضل.
:::

##جائزہ

هناك ماكينات اندرويد Torii مناسبة للاندرويد `sorafs-node`
رہنماي كرتی ہے۔ سلسلة من أجل تسليم مخرجات SF-3: دبوس/جلب
راوند بريس، السجل الفني، تقليد الكوت، وبسيطة PoR.

## 1. پیشگی مطلوبے

- `torii.sorafs.storage` العامل النشط في العمل:

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

- استخدم هذه الأداة Torii التي تستخدم `data_dir` لقراءة/كتابة الرسائل.
- إعلان التسجيل منذ فترة طويلة بعد `GET /v1/sorafs/capacity/state` بعد التحديث
  الإعلان الجديد عن السجل المتوقع جديد.
- تجانس فعال، كل من النحاس الخام والملسى GiB·hour/PoR دونوں
  تتميز هذه المنتجات بأنها خالية من الارتعاش، مما يؤدي إلى ارتفاع القيم الفورية إلى ما هو أبعد من ذلك.

### CLI رن (اختاري)

نقاط نهاية HTTP تعمل على واجهة سطر الأوامر المجمعة وهي واجهة خلفية متجددة لـ HTTP
التحقق من سلامة العقل.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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
```قم بإنهاء ملخصات Norito JSON وعدم تطابق ملف تعريف القطعة أو الملخص
تم تسليم البطاقة، جس س أو Torii الأسلاك فحص الدخان CI لملف بنتي
ہیں.【crates/sorafs_node/tests/cli.rs#L1】

### PoR پروف کی مشقوق

تم تنزيل عناصر PoR ذات الأطراف الاصطناعية التي تم تنزيلها من Torii
لقد تم بالفعل نشر المزيد من المعلومات في جميع أنحاء العالم. CLI هو `sorafs-node` طريقة الابتلاع وإعادة الاستخدام
هذه هي الأخطاء الشائعة في البحث وأخطاء التحقق من صحة بطاقة HTTP API WAPPS
ٹاتی ہے۔

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

ينبعث ملخص JSON من ملخص (ملخص البيان، معرف الموفر، ملخص الإثبات،
عدد العينات ونتائج الحكم الاختياري)۔ `--manifest-id=<hex>` عملية الإيداع
تم حفظ ملخص تشيلينج واضح، واستخدام `--json-out=<path>`
فيما يلي ملخص للعناصر الأصلية وأدلة التدقيق المقدمة في هذا المقال
چہيں. يتضمن `--verdict` شبكة HTTP API لشبكة الإنترنت عبر الإنترنت → جيد
→ الحكم لوپ کي بوري مشق کر کرتہیں.

تمت الموافقة على Torii بعد أن تم الحصول على عناصر HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

تساعد نقاط النهاية دون تحديد كيفية عمل اختبارات الدخان CLI و
تحقيقات البوابة ہنگ رہتے ہیں. 【crates/iroha_torii/src/sorafs/api.rs#L1207 】】Quates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. دبوس → جلب الجلب1. البيان + الحمولة الصافية (مثلاً
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` ذریعے).
2. بيان ترميز Base64 الذي تم جمعه:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   طلب JSON يشمل `manifest_b64` و`payload_b64` ويشمل هذا. كامياب
   الاستجابة `manifest_id_hex` وملخص الحمولة الصافية
3. جلب الجلب المثبت:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   `data_b64` يقوم بفك تشفير قاعدة 64 وقراءة ما يصل إلى أكبر عدد من وحدات البايت.

## 3. سجل رائع يا إيرل

1. يتم إجراء العملية الأولى وفقًا لرقم التعريف الشخصي الواضح.
2. Torii Pross (أو بورا جديدة) هو جديد.
3. جلب سجل جمع البيانات مرة أخرى. حمولة بدستور دستیاب و واپس عليها والا
   الملخص هو أعلى قدر ممكن من القوة بما يتوافق مع ذلك.
4. `GET /v1/sorafs/storage/state` قم بفحص زر صغير `bytes_used`
   بعد استمرار استمرار ظهور كرتا ہے۔

## 4. كل هذا التسجيل ٹیستٹ

1. اعرض على `torii.sorafs.storage.max_capacity_bytes` ما هو أكثر قوة
   (مثلاً بيان کے سائز کے برابر).
2. رقم تعريف شخصي واحد؛ لقد تم إنشاء نسخة احتياطية من اللعبة.
3. هذه الخطط الناجحة والجميلة هي دبوس نقش بارز. Torii للريكويسٹ
   HTTP `400` وهو الرابط التالي ورسالة `storage capacity exceeded`
   يشمل ہونا چاہيے.
4. إنهاء السجل غير القانوني إلى حد كبير.

## 5.PoR سيمپلنگ پروب

1. دبوس بياني واحد.
2. اختبار عينة PoR:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```3. التحقق من الاستجابة `samples` عدد الأقمار الصناعية المتوفرة والموجودة حاليًا
   تم إثبات الجذر الواضح لفشل الفائض.

## 6.آلية ہكس

- تتضمن اختبارات CI / الدخان في الغلاف الخارجي عددًا من الاستخدامات التالية:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  جو `pin_fetch_roundtrip`، `pin_survives_restart`، `pin_quota_rejection`، و
  `por_sampling_returns_verified_proofs` بطاقة العمل.
- كل ما تحتاجه من اللون الوردي:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` و`torii_sorafs_storage_fetch_inflight`
  - `/v1/sorafs/capacity/state` ذريعة لبطاقة PoR/البطاقة الائتمانية
  - `sorafs_node_deal_publish_total{result=success|failure}` محاولات نشر التسوية

مناورات البيروقراطية أو البيروقراطية التي ستبدأ في استيعابها
ممتاز، اختراع جديد، مبدأ مقبول للاعتراف به، وعمل جديد جديد
يقوم المعلنون بالإعلان عن أحدث إثباتات PoR.