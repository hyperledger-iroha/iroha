---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/node-operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: نوڈ آپریشنز
عنوان: نوڈ آپریشنز رن بک
سائڈبار_لیبل: نوڈ آپریشنز رن بک
تفصیل: Torii کے اندر `sorafs-node` کے سرایت شدہ نفاذ کی توثیق کریں۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_node_ops.md` کا آئینہ دار ہے۔ اسفینکس سیٹ کو ہٹانے تک دونوں ورژن ہم آہنگ رکھیں۔
:::

## جائزہ

یہ رن بک آپریٹرز کو `sorafs-node` تعیناتی کی توثیق کرنے میں رہنمائی کرتا ہے جو Torii میں سرایت کرتا ہے۔ ہر حص section ہ براہ راست SF-3 کی فراہمی سے منسلک ہوتا ہے: پن/بازیافت سائیکل ، دوبارہ شروع ہونے کے بعد بازیافت ، کوٹہ مسترد ، اور پور نمونے لینے۔

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

- اس بات کو یقینی بنائیں کہ عمل Torii نے `data_dir` تک پڑھنے/تحریری رسائی حاصل کی ہے۔
- تصدیق کریں کہ جب کوئی اعلامیہ ریکارڈ کیا جاتا ہے تو NO `GET /v1/sorafs/capacity/state` کے ذریعہ متوقع صلاحیت کا اعلان کرتا ہے۔
- جب ہموار کرنے کے قابل ہوجاتے ہیں تو ، ڈیش بورڈز فوری اقدار کے ساتھ ساتھ جٹر فری رجحانات کو اجاگر کرنے کے لئے خام اور ہموار گیب · گھنٹہ/پور کاؤنٹرز کو ظاہر کرتے ہیں۔

### CLI ڈرائی رن (اختیاری)

ایچ ٹی ٹی پی کے اختتامی نکات کو بے نقاب کرنے سے پہلے ، آپ ایمبیڈڈ سی ایل آئی کے ساتھ اسٹوریج بیک اینڈ کی توثیق کرسکتے ہیں۔

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

کمانڈز Norito JSON کو ہضم کرتے ہیں اور ان سے انکار کرتے ہیں اور ان سے انکار کرتے ہیں یا نہیں ہضم کرتے ہیں ، جس سے وہ Torii وائرنگ سے پہلے CI دھواں چیکوں کے ل useful مفید بنتے ہیں۔

### پور پروف ٹیسٹ

آپریٹرز اب گورننس سے جاری کردہ پور نمونے کو مقامی طور پر Torii پر بھیجنے سے پہلے دوبارہ چلا سکتے ہیں۔ سی ایل آئی اسی `sorafs-node` انجسٹ پاتھ کو دوبارہ استعمال کرتی ہے ، لہذا مقامی رنز توثیق کی غلطیوں کو بالکل بے نقاب کرتے ہیں جو HTTP API واپس آئے گا۔

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

کمانڈ ایک JSON خلاصہ (مینی فیسٹ ڈائجسٹ ، فراہم کنندہ ID ، پروف ڈائجسٹ ، نمونہ گنتی اور اختیاری فیصلہ) کو آؤٹ کرتا ہے۔ `--manifest-id=<hex>` فراہم کریں تاکہ یہ یقینی بنایا جاسکے کہ ذخیرہ شدہ مینی فیسٹ چیلنج ڈائجسٹ سے مماثل ہے ، اور جب آپ آڈٹ ثبوت کے طور پر اصل نمونے کے ساتھ خلاصہ محفوظ کرنا چاہتے ہیں تو `--json-out=<path>`۔ `--verdict` سمیت آپ کو HTTP API پر کال کرنے سے پہلے مکمل چیلنج → پروف → فیصلے کے بہاؤ آف لائن کی مشق کرنے کی اجازت ملتی ہے۔

Torii فعال ہونے کے بعد آپ HTTP کے ذریعے وہی نمونے بازیافت کرسکتے ہیں:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

دونوں اختتامی مقامات بلٹ ان اسٹوریج ورکر کے ذریعہ پیش کیے جاتے ہیں ، لہذا سی ایل آئی سگریٹ نوشی کے ٹیسٹ اور گیٹ وے کی تحقیقات ہم آہنگی میں رہتی ہیں۔

## 2. پن → بازیافت سائیکل

1. ایک منشور + پے لوڈ بنڈل بنائیں (مثال کے طور پر `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` کے ساتھ)۔
2. بیس 64 انکوڈنگ کے ساتھ مینی فیسٹ بھیجیں:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   درخواست JSON میں `manifest_b64` اور `payload_b64` پر مشتمل ہونا چاہئے۔ ایک کامیاب جواب `manifest_id_hex` اور پے لوڈ ڈائجسٹ کو لوٹاتا ہے۔
3. فکسڈ ڈیٹا کی تلاش:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ````data_b64` فیلڈ کو بیس 64 میں ڈیکوڈ کریں اور تصدیق کریں کہ یہ اصل بائٹس سے مماثل ہے۔

## 3. بازیافت کی ورزش دوبارہ شروع کرنے کے بعد

1۔ کم از کم ایک ظاہر اوپر کی طرح۔
2. عمل کو دوبارہ شروع کریں Torii (یا پورا نمبر)۔
3. بازیافت کی درخواست کو دوبارہ جاری کریں۔ پے لوڈ کو دستیاب ہونا ضروری ہے اور واپس آنے والے ڈائجسٹ کو دوبارہ شروع ہونے سے پہلے قیمت سے ملنا چاہئے۔
4. `GET /v1/sorafs/storage/state` کا معائنہ کریں تاکہ اس بات کی تصدیق کی جاسکے کہ `bytes_used` ریبوٹ کے بعد مستقل ظاہر ہونے کی عکاسی کرتا ہے۔

## 4. کوٹہ مسترد ٹیسٹ

1. عارضی طور پر `torii.sorafs.storage.max_capacity_bytes` کو ایک چھوٹی سی قدر میں کم کریں (مثال کے طور پر ایک ہی مینی فیسٹ کا سائز)۔
2. ایک منشور طے کریں ؛ درخواست کامیاب ہونی چاہئے۔
3. اسی طرح کے دوسرے مظہر کو پن کرنے کی کوشش کریں۔ Torii کو HTTP `400` اور `storage capacity exceeded` پر مشتمل ایک غلطی پیغام کے ساتھ درخواست کو مسترد کرنا چاہئے۔
4. ختم ہونے پر عام صلاحیت کی حد کو بحال کریں۔

## 5. پور نمونے لینے کی تحقیقات

1. ایک منشور پن۔
2. نمونہ por کی درخواست کریں:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. تصدیق کریں کہ جواب میں `samples` درخواست کی گنتی کے ساتھ شامل ہے اور یہ کہ ہر ثبوت ذخیرہ شدہ مینی فیسٹ کی جڑ کے خلاف توثیق کرتا ہے۔

## 6. آٹومیشن ہکس

- CI/دھواں کے ٹیسٹ میں شامل کردہ ٹارگٹ چیکوں کو دوبارہ استعمال کیا جاسکتا ہے:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  جس کا احاطہ `pin_fetch_roundtrip` ، `pin_survives_restart` ، `pin_quota_rejection` اور `por_sampling_returns_verified_proofs`۔
- ڈیش بورڈز کو مانیٹر کرنا ہوگا:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` اور `torii_sorafs_storage_fetch_inflight`
  - `/v1/sorafs/capacity/state` کے ذریعے بے نقاب ہونے والی کامیابی/ناکامی کاؤنٹرز
  - `sorafs_node_deal_publish_total{result=success|failure}` کے ذریعے تصفیہ شائع کرنے کی کوششیں

ان مشقوں کے بعد یہ یقینی بناتا ہے کہ بلٹ ان اسٹوریج ورکر ڈیٹا کو کھا سکتا ہے ، دوبارہ شروع سے بچ سکتا ہے ، تشکیل شدہ کوٹے کا احترام کرسکتا ہے ، اور وسیع تر نیٹ ورک کی صلاحیت کا اعلان کرنے سے پہلے ڈٹرمینسٹک پور ثبوت تیار کرسکتا ہے۔