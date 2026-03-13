---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/node-operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: نوڈ آپریشنز
عنوان: معاہدہ آپریشنز آپریشنز دستی
سائڈبار_لیبل: چلانے والے نوڈس
تفصیل: `sorafs-node` کی تعیناتی کی تصدیق Torii میں سرایت شدہ۔
---

::: منظور شدہ ماخذ کو نوٹ کریں
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_node_ops.md` کی عکاسی کرتا ہے۔ اس بات کو یقینی بنائیں کہ اس وقت تک دونوں کاپیاں مطابقت پذیری میں رکھیں جب تک کہ پرانی اسفنکس دستاویزات کا سیٹ ریٹائر نہ ہو۔
:::

## جائزہ

یہ گائیڈ آپریٹرز کو `sorafs-node` کی تعیناتی کی تصدیق کے ذریعے Torii میں سرایت کرتا ہے۔ میچز
ہر سیکشن SF-3 آؤٹ پٹس کے ساتھ رواں دواں ہے: پن/بازیافت رنز ، ریبوٹ کی بازیابی کے بعد ،
کوٹہ مسترد ، پور نمونے لینے۔

## 1. شرائط

- `torii.sorafs.storage` میں اسٹوریج عنصر کو چالو کریں:

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

- اس بات کو یقینی بنائیں کہ اس عمل Torii نے `data_dir` پر مراعات پڑھیں/لکھیں۔
- تصدیق کریں کہ نوڈ ابھی تک `GET /v2/sorafs/capacity/state` کے ذریعہ متوقع صلاحیت کا اعلان کرتا ہے
  اجازت نامہ درج کریں۔
- جب ہموار کرنے کے قابل ہوجاتے ہیں تو ، ڈیش بورڈز خام اور ہموار گیب · گھنٹہ/پور کاؤنٹرز دکھاتے ہیں
  انٹراڈے اقدار کے ساتھ اتار چڑھاو سے پاک رجحانات کو بھی اجاگر کرنا۔

### ٹیسٹ CLI کے ذریعے چلائیں (اختیاری)

HTTP اختتامی نکات کو دستیاب کرنے سے پہلے ، آپ CLI کا استعمال کرتے ہوئے اسٹوریج بیک گراؤنڈ ہیلتھ چیک انجام دے سکتے ہیں
منسلک.

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

کمانڈ Norito JSON کو پرنٹ کرتا ہے اور اس کو مسترد کرتا ہے اور اس کو مسترد کرتا ہے یا ڈیفینیشن ڈیفینیشن فائل مماثلتوں کو مسترد کرتا ہے۔
جو Torii کو مربوط کرنے سے پہلے CI میں دھواں کے ٹیسٹوں کے ل useful مفید بناتا ہے۔

### پور پروف ورزش

آپریٹرز اب اس سے پہلے مقامی طور پر گورننس سے جاری کردہ پور نمونے کو دوبارہ چلا سکتے ہیں
اسے Torii پر اپ لوڈ کریں۔ CLI `sorafs-node` میں ایک ہی ان پٹ راستے کو دوبارہ استعمال کرتا ہے ، لہذا اس کا پتہ لگاتا ہے
مقامی محرکات وہی درست توثیق کی غلطیوں کی اطلاع دیتے ہیں جو HTTP انٹرفیس واپس آجائے گی۔

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

کمانڈ JSON ڈائجسٹ (ڈائجسٹ مینی فیسٹ ، فراہم کنندہ شناخت کنندہ ، ڈائجسٹ پروف ، نمبر برآمد کرتا ہے
نمونے ، اور اختیاری فیصلے کا نتیجہ)۔ میچ کو یقینی بنانے کے لئے `--manifest-id=<hex>` کو بچائیں
جب آپ محفوظ شدہ دستاویزات کرنا چاہتے ہیں تو چیلنج ڈائجسٹ کے ساتھ ذخیرہ شدہ ، اور `--json-out=<path>`
آڈٹ ثبوت کے طور پر اصل نمونے کے ساتھ خلاصہ۔ `--verdict` داخل کرنا آپ کو ورزش کرنے کی اجازت دیتا ہے
HTTP انٹرفیس کو کال کرنے سے پہلے پورا چیلنج → پروف → فیصلے کا لوپ آف لائن مکمل ہوجاتا ہے۔

ایک بار جب Torii چل رہا ہے تو آپ HTTP کے توسط سے نمونے خود بازیافت کرسکتے ہیں:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

دونوں اختتامی مقامات بلٹ ان اسٹوریج ایجنٹ کے ذریعہ پیش کیے جاتے ہیں ، لہذا وہ ٹیسٹ باقی رہتے ہیں
سی ایل آئی اور گیٹ وے پول کے ذریعے دھواں ہم آہنگ کیا جاتا ہے۔ 【کریٹس/آئروہ_ٹوری/ایس آر سی/سرفس/آپی. آر ایس#ایل 12207 】【 کریٹس/آئروہ_ٹوری/ایس آر سی/سرفس/api.rs#l1259】

## 2. گول پن → بازیافت

1. ایک منشور + پے لوڈ پیکیج بنائیں (جیسے کے ذریعے
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`)۔
2. بیس 64 انکوڈنگ میں مینی فیسٹ بھیجیں:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   درخواست JSON میں `manifest_b64` اور `payload_b64` پر مشتمل ہونا چاہئے۔ جواب واپس کرتا ہے
   کامیاب `manifest_id_hex` اور پے لوڈ کو ہضم کریں۔
3. انسٹال شدہ ڈیٹا کو بازیافت کریں:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   فیلڈ `data_b64` کے بیس 64 انکوڈ کو ڈیکوڈ کریں اور تصدیق کریں کہ یہ اصل بائٹس سے مماثل ہے۔

## 3. دوبارہ بازیافت کی بحالی کی مشق

1. کم از کم ایک منشور کو اوپر کی طرح انسٹال کریں۔
2. Torii عمل (یا پورا نوڈ) دوبارہ شروع کریں۔
3. بازیافت کی درخواست کو دوبارہ جمع کریں۔ پے لوڈ کو بازیافت اور ہضم ہونا چاہئے
   پچھلی دوبارہ شروع والی قیمت کے ساتھ لوٹ آیا۔
4. `GET /v2/sorafs/storage/state` چیک کریں تاکہ یہ یقینی بنایا جاسکے کہ `bytes_used` کی عکاسی ہوتی ہے
   ریبوٹ کے بعد ظاہر ہوا۔

## 4. کوٹہ مسترد ٹیسٹ1. عارضی طور پر `torii.sorafs.storage.max_capacity_bytes` کو ایک چھوٹی سی قیمت پر کم کریں (جیسے
   ظاہر ہوتا ہے)۔
2. ایک منشور انسٹال کریں ؛ درخواست کو کامیاب ہونا چاہئے۔
3. اسی طرح کے دوسرے مینی فیسٹ کو انسٹال کرنے کی کوشش کریں۔ Torii کو HTTP `400` کے ساتھ درخواست کو مسترد کرنا چاہئے
   غلطی کے پیغام میں `storage capacity exceeded` ہوتا ہے۔
4. ختم ہونے پر عام صلاحیت کی حد کو بحال کریں۔

## 5۔ POR نمونے لینے کی جانچ پڑتال

1. ایک منشور انسٹال کریں۔
2. POR نمونہ کی درخواست کریں:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. تصدیق کریں کہ جواب میں درخواست کردہ نمونوں کی تعداد میں `samples` پر مشتمل ہے اور ہر ثبوت
   یہ ذخیرہ شدہ مینی فیسٹ جڑ کے لئے درست ہے۔

## 6. آٹومیشن ہکس

- CI/دھواں کے ٹیسٹ میں شامل کردہ ٹارگٹ چیکوں کو دوبارہ استعمال کیا جاسکتا ہے:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  جس میں `pin_fetch_roundtrip` ، `pin_survives_restart` اور `pin_quota_rejection` کا احاطہ کیا گیا ہے
  اور `por_sampling_returns_verified_proofs`۔
- مانیٹرنگ بورڈز کی پیروی کرنی چاہئے:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` اور `torii_sorafs_storage_fetch_inflight`
  - `/v2/sorafs/capacity/state` کے ذریعے دکھائے جانے والے پور کامیابی/ناکامی کاؤنٹرز
  - `sorafs_node_deal_publish_total{result=success|failure}` کے ذریعے تصفیہ شائع کرنے کی کوششیں

ان مشقوں کے بعد یہ یقینی بناتا ہے کہ ایمبیڈڈ اسٹوریج آپریٹر ڈیٹا داخل کرنے کے قابل ہے ،
دوبارہ شروع کرنے کے خلاف مزاحمت کریں ، کوٹہ کا احترام کریں ، اور پور پیدا کریں
نوڈ سے پہلے ایک نوڈ وسیع نیٹ ورک کی صلاحیت کا اعلان کرتا ہے۔