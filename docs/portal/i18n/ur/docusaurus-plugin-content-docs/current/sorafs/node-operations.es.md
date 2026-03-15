---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/node-operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: نوڈ آپریشنز
عنوان: نوڈ آپریشنز رن بک
سائڈبار_لیبل: نوڈ آپریشنز رن بک
تفصیل: `sorafs-node` کی مربوط تعیناتی کو Torii کے اندر توثیق کرتا ہے۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_node_ops.md` کی عکاسی کرتا ہے۔ اسفینکس سیٹ ریٹائر ہونے تک دونوں ورژن کو مطابقت پذیری میں رکھیں۔
:::

## خلاصہ

یہ رن بک `sorafs-node` تعیناتی کی توثیق کرنے میں آپریٹرز کی رہنمائی کرتا ہے جس میں Torii میں سرایت کی گئی ہے۔ ہر سیکشن براہ راست SF-3 کی فراہمی کے مساوی ہے: پن/بازیافت کرنے والے ٹریورلز ، ری سیٹ وصولی ، کوٹہ مسترد ، اور پور نمونے لینے۔

## 1. شرائط

- `torii.sorafs.storage` پر اسٹوریج ورکر کو فعال کریں:

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

- اس بات کو یقینی بنائیں کہ اس عمل Torii نے `data_dir` تک پڑھنے/تحریری رسائی حاصل کی ہے۔
- اس بات کی تصدیق کرتی ہے کہ نوڈ ایک بار اعلان ریکارڈ ہونے کے بعد `GET /v2/sorafs/capacity/state` کے ذریعے متوقع صلاحیت کا اعلان کرتا ہے۔
- جب ہموار کرنے کے قابل ہوجاتے ہیں تو ، ڈیش بورڈز فوری اقدار کے ساتھ ساتھ جٹر فری رجحانات کو اجاگر کرنے کے لئے کچے اور ہموار گیب آور/پور کاؤنٹرز کو ظاہر کرتے ہیں۔

### خشک رن سی ایل آئی (اختیاری)

HTTP اختتامی نکات کو بے نقاب کرنے سے پہلے آپ مربوط CLI کے ساتھ اسٹوریج بیکینڈ کا فوری چیک کرسکتے ہیں۔

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

کمانڈز Norito JSON ڈائجسٹ اور ڈائجسٹ یا کونک پروفائل مماثل کو مسترد کرتے ہیں ، جس سے وہ Torii کو وائرنگ سے پہلے CI دھواں چیکوں کے ل useful مفید بناتے ہیں۔

### پور ٹیسٹنگ ریہرسل

تاجر اب مقامی طور پر گورننس جاری کردہ پور نمونے کھیل سکتے ہیں اس سے پہلے کہ وہ Torii پر اپ لوڈ کریں۔ سی ایل آئی اسی انجسٹ پاتھ `sorafs-node` کو دوبارہ استعمال کرتی ہے ، لہذا مقامی پھانسیوں سے توثیق کی غلطیوں کو بالکل بے نقاب کیا جاتا ہے جو HTTP API واپس آئے گا۔

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

کمانڈ ایک JSON DISTECT (منشور ڈائجسٹ ، وینڈر ID ، ٹیسٹ ڈائجسٹ ، نمونوں کی تعداد ، اور اختیاری فیصلے کے نتائج) کو آؤٹ پٹ کرتا ہے۔ `--manifest-id=<hex>` فراہم کریں تاکہ یہ یقینی بنایا جاسکے کہ ذخیرہ شدہ مینی فیسٹ چیلنج ڈائجسٹ سے مماثل ہے ، اور جب آپ آڈٹ ثبوت کے طور پر اصل نمونے کے ساتھ خلاصہ محفوظ کرنا چاہتے ہیں تو `--json-out=<path>`۔ `--verdict` سمیت آپ کو HTTP API پر کال کرنے سے پہلے پورے چیلنج → ٹیسٹ → فیصلے کے بہاؤ آف لائن کی مشق کرنے کی اجازت ملتی ہے۔

ایک بار جب Torii فعال ہوجائے تو آپ HTTP کے ذریعہ ایک ہی نمونے بازیافت کرسکتے ہیں:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

دونوں اختتامی نکات ایمبیڈڈ اسٹوریج ورکر کے ذریعہ پیش کیے جاتے ہیں ، لہذا سی ایل آئی کے دھواں ٹیسٹ اور گیٹ وے کی تحقیقات ہم آہنگ رہتے ہیں۔

## 2۔ روٹ پن → بازیافت

1. ایک منشور + پے لوڈ پیکیج تیار کریں (مثال کے طور پر `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` کے ساتھ)۔
2. بیس 64 انکوڈنگ کے ساتھ مینی فیسٹ بھیجیں:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```درخواست JSON میں `manifest_b64` اور `payload_b64` پر مشتمل ہونا چاہئے۔ ایک کامیاب جواب `manifest_id_hex` اور پے لوڈ ڈائجسٹ کو لوٹاتا ہے۔
3. پن شدہ ڈیٹا کی بازیافت:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   بیس 64 `data_b64` فیلڈ کو ڈیکوڈ کرتا ہے اور تصدیق کرتا ہے کہ یہ اصل بائٹس سے مماثل ہے۔

## 3. ریبوٹ کے بعد بازیافت ڈرل

1. کم از کم ایک منشور کو اوپر کی طرح مقرر کریں۔
2. Torii عمل (یا پورا نوڈ) دوبارہ شروع کریں۔
3. بازیافت کی درخواست کو دوبارہ جاری کریں۔ پے لوڈ کو ابھی بھی وصولی کے قابل ہونا چاہئے اور واپس آنے والے ڈائجسٹ کو دوبارہ ترتیب دینے سے پہلے قیمت سے ملنا چاہئے۔
4. `GET /v2/sorafs/storage/state` کا معائنہ کریں تاکہ اس بات کی تصدیق کی جاسکے کہ `bytes_used` دوبارہ شروع ہونے کے بعد ظاہر ہوتا ہے۔

## 4. کوٹہ کے ذریعہ مسترد ہونے کا ثبوت

1. عارضی طور پر `torii.sorafs.storage.max_capacity_bytes` کو ایک چھوٹی سی قدر میں کم کریں (مثال کے طور پر ایک ہی مینی فیسٹ کا سائز)۔
2. ایک منشور طے کریں ؛ درخواست کامیاب ہونی چاہئے۔
3. اسی طرح کے دوسرے مینی فیسٹ کو پن کرنے کی کوشش کریں۔ Torii کو `400` اور ایک غلطی والے پیغام کے ساتھ HTTP درخواست کو مسترد کرنا چاہئے جس میں `storage capacity exceeded` شامل ہے۔
4. تکمیل کے بعد عام صلاحیت کی حد کو بحال کرتا ہے۔

## 5. پور نمونے لینے کی تحقیقات

1. ایک منشور مرتب کریں۔
2. پور نمونے لینے کی درخواست کریں:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. تصدیق کریں کہ جواب میں `samples` درخواست کی گنتی کے ساتھ ہوتا ہے اور یہ کہ ہر ٹیسٹ ذخیرہ شدہ مینی فیسٹ کی جڑ کے خلاف توثیق کرتا ہے۔

## 6. آٹومیشن ہکس

- CI/دھواں کے ٹیسٹ میں شامل کردہ ٹارگٹ چیکوں کو دوبارہ استعمال کیا جاسکتا ہے:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  `pin_fetch_roundtrip` ، `pin_survives_restart` ، `pin_quota_rejection` اور `por_sampling_returns_verified_proofs` کا احاطہ کرتا ہے۔
- ڈیش بورڈز کو لازمی طور پر عمل کرنا چاہئے:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` اور `torii_sorafs_storage_fetch_inflight`
  - `/v2/sorafs/capacity/state` کے ذریعے بے نقاب ہونے والی کامیابی/ناکامی کاؤنٹرز
  - `sorafs_node_deal_publish_total{result=success|failure}` کے ذریعے تصفیہ شائع کرنے کی کوششیں

ان مشقوں کے بعد یہ یقینی بناتا ہے کہ ایمبیڈڈ اسٹوریج ورکر ڈیٹا کو کھا سکتا ہے ، ریبوٹس سے بچ سکتا ہے ، تشکیل شدہ کوٹے کو اعزاز بخش سکتا ہے ، اور نوڈ کے وسیع تر نیٹ ورک کی صلاحیت کا اعلان کرنے سے پہلے ڈٹرمینسٹک پور ٹیسٹ تیار کرسکتا ہے۔