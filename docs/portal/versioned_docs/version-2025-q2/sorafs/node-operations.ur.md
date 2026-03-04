---
lang: ur
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T15:38:30.655980+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

::: نوٹ کینونیکل ماخذ
آئینے `docs/source/sorafs/runbooks/sorafs_node_ops.md`۔ دونوں کاپیاں ریلیز کے پار منسلک رکھیں۔
:::

## جائزہ

یہ رن بک آپریٹرز کو Torii کے اندر ایمبیڈڈ `sorafs-node` تعیناتی کی توثیق کے ذریعے چلتا ہے۔ ہر سیکشن براہ راست SF-3 کی فراہمی کے لئے نقشہ بناتا ہے: پن/بازیافت گول ٹرپس ، دوبارہ شروع کرنے کی بازیابی ، کوٹہ مسترد ، اور POR نمونے لینے۔

## 1. شرائط

- `torii.sorafs.storage` میں اسٹوریج ورکر کو فعال کریں:

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

- یقینی بنائیں کہ Torii عمل نے `data_dir` تک پڑھنے/تحریری رسائی حاصل کی ہے۔
- اس بات کی تصدیق کریں کہ نوڈ ایک بار اعلان ہونے کے بعد `GET /v1/sorafs/capacity/state` کے ذریعہ متوقع صلاحیت کی تشہیر کرتا ہے۔
- جب ہموار کرنے کے قابل ہوجاتے ہیں تو ، ڈیش بورڈز اسپاٹ اقدار کے ساتھ ساتھ جٹر فری رجحانات کو اجاگر کرنے کے لئے کچے اور ہموار گیب · گھنٹہ/پور کاؤنٹرز کو بے نقاب کرتے ہیں۔

### CLI ڈرائی رن (اختیاری)

HTTP اختتامی نکات کو بے نقاب کرنے سے پہلے آپ بنڈل CLI کے ساتھ اسٹوریج بیکینڈ کو بے ہودہ چیک کرسکتے ہیں۔

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

کمانڈز Norito JSON کے خلاصے پرنٹ کریں اور CHUNC پروفائل یا ہضم سے انکار کریں ، جس سے وہ Torii وائرنگ سے پہلے CI دھواں چیکوں کے لئے کارآمد ثابت ہوتا ہے۔

ایک بار جب Torii براہ راست ہے تو آپ HTTP کے ذریعے ایک ہی نوادرات کو بازیافت کرسکتے ہیں:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

دونوں اختتامی مقامات ایمبیڈڈ اسٹوریج ورکر کے ذریعہ پیش کیے جاتے ہیں ، لہذا سی ایل آئی سگریٹ نوشی کے ٹیسٹ اور گیٹ وے کی تحقیقات مطابقت پذیری میں رہتی ہیں۔

## 2۔ پن → راؤنڈ ٹرپ بازیافت کریں

1. ایک منشور + پے لوڈ بنڈل تیار کریں (مثال کے طور پر `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` کے ساتھ)۔
2. بیس 64 انکوڈنگ کے ساتھ مینی فیسٹ جمع کروائیں:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   درخواست JSON میں `manifest_b64` اور `payload_b64` پر مشتمل ہونا ضروری ہے۔ ایک کامیاب جواب `manifest_id_hex` اور پے لوڈ ڈائجسٹ کو لوٹاتا ہے۔
3. پن شدہ ڈیٹا لائیں:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Base64-decode `data_b64` فیلڈ اور اس کی تصدیق کریں کہ یہ اصل بائٹس سے مماثل ہے۔

## 3. بازیافت کی مشق دوبارہ شروع کریں

1۔ کم از کم ایک ظاہر اوپر کی طرح۔
2. Torii عمل (یا پورا نوڈ) دوبارہ شروع کریں۔
3. بازیافت کی درخواست کو دوبارہ ذخیرہ کریں۔ پے لوڈ کو ابھی بھی بازیافت ہونا چاہئے اور واپس آنے والے ڈائجسٹ کو لازمی طور پر پری اسٹارٹ ویلیو سے ملنا چاہئے۔
4. `bytes_used` کی تصدیق کرنے کے لئے `GET /v1/sorafs/storage/state` کا معائنہ کریں ریبوٹ کے بعد مستقل طور پر ظاہر ہونے والی عکاسی کرتا ہے۔

## 4. کوٹہ مسترد ٹیسٹ

1. عارضی طور پر `torii.sorafs.storage.max_capacity_bytes` کو ایک چھوٹی سی قیمت پر کم کریں (مثال کے طور پر ایک ہی مینی فیسٹ کا سائز)۔
2. پن ایک منشور ؛ درخواست کامیاب ہونی چاہئے۔
3. اسی طرح کے دوسرے مینی فیسٹ کو پن کرنے کی کوشش کریں۔ Torii کو HTTP `400` اور `storage capacity exceeded` پر مشتمل ایک غلطی پیغام کے ساتھ درخواست کو مسترد کرنا ہوگا۔
4. ختم ہونے پر عام صلاحیت کی حد کو بحال کریں۔

## 5. پور نمونے لینے کی تحقیقات

1. ایک منشور پن۔
2. پور نمونہ کی درخواست کریں:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. تصدیق کریں کہ جواب کی تصدیق شدہ گنتی کے ساتھ `samples` ہے اور یہ کہ ہر ثبوت ذخیرہ شدہ مینی فیسٹ جڑ کے خلاف توثیق کرتا ہے۔

## 6. آٹومیشن ہکس

- CI / دھواں کے ٹیسٹ میں شامل کردہ ہدف چیکوں کو دوبارہ استعمال کیا جاسکتا ہے:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```جس میں `pin_fetch_roundtrip` ، `pin_survives_restart` ، `pin_quota_rejection` ، اور `por_sampling_returns_verified_proofs` کا احاطہ کیا گیا ہے۔
- ڈیش بورڈز کو ٹریک کرنا چاہئے:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` اور `torii_sorafs_storage_fetch_inflight`
  - پور کامیابی/ناکامی کاؤنٹرز `/v1/sorafs/capacity/state` کے ذریعے منظر عام پر آئے
  - تصفیہ `sorafs_node_deal_publish_total{result=success|failure}` کے ذریعے کوششیں شائع کریں

ان مشقوں کے بعد ایمبیڈڈ اسٹوریج ورکر اس بات کو یقینی بناتا ہے کہ نوڈ وسیع نیٹ ورک کی صلاحیت کی تشہیر کرنے سے پہلے اعداد و شمار کو کھا سکتا ہے ، دوبارہ شروع ہونے والے کوٹے کا احترام کرسکتا ہے ، اور تعصب کے ثبوت پیدا کرسکتا ہے۔