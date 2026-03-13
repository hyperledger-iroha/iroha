---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/node-operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: نوڈ آپریشنز
عنوان: نوڈ استحصال رن بک
سائڈبار_لیبل: نوڈ استحصال رن بک
تفصیل: Torii میں `sorafs-node` کی سرایت شدہ تعیناتی کی توثیق کریں۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_node_ops.md` کی عکاسی کرتا ہے۔ دونوں ورژن کو مطابقت پذیری میں رکھیں جب تک کہ اسفینکس سیٹ کو ہٹا نہ دیا جائے۔
:::

## جائزہ

یہ رن بک آپریٹرز کو `sorafs-node` تعیناتی کو Torii میں سرایت کرنے کی توثیق کرنے کے لئے رہنمائی کرتا ہے۔ ہر حص section ہ براہ راست SF-3 کی فراہمی کے مساوی ہے: پن/بازیافت لوپس ، بازیافت بازیافت ، کوٹہ مسترد ، اور پور نمونے لینے۔

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

- اس بات کو یقینی بنائیں کہ عمل Torii نے `data_dir` تک پڑھنے/لکھنے تک رسائی حاصل کی ہے۔
- اس بات کی تصدیق کریں کہ نوڈ نے اعلامیہ محفوظ ہونے کے بعد `GET /v2/sorafs/capacity/state` کے ذریعے متوقع صلاحیت کا اعلان کیا ہے۔
- جب ہموار کرنے کے قابل ہوجاتے ہیں تو ، ڈیش بورڈز فوری اقدار کے ساتھ ساتھ جٹر فری رجحانات کو اجاگر کرنے کے لئے کچے اور ہموار گیب · گھنٹہ/پور کاؤنٹرز کو بے نقاب کرتے ہیں۔

### سی ایل آئی کی خالی عملدرآمد (اختیاری)

HTTP اختتامی نکات کو بے نقاب کرنے سے پہلے ، آپ فراہم کردہ CLI کے ساتھ اسٹوریج پسدید کی توثیق کرسکتے ہیں۔ 【کریٹس/sorafs_node/src/bin/sorafs-node.rs#l1】

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

کمانڈز پرنٹ Norito JSON ہضم کرتا ہے اور اس سے انکار کرتا ہے یا ہضم پروفائل تضادات سے انکار کرتا ہے ، جس سے وہ Torii کو وائرنگ سے پہلے CI دھواں چیکوں کے ل useful مفید بناتے ہیں۔

### پور پروف ریپیٹیشن

آپریٹرز اب مقامی طور پر گورننس کے ذریعہ جاری کردہ نمونے کو دوبارہ چلا سکتے ہیں جو ان کو Torii پر اپ لوڈ کرنے سے پہلے۔ سی ایل آئی اسی `sorafs-node` انجیریشن راہ کو دوبارہ استعمال کرتی ہے ، لہذا مقامی رنز توثیق کی غلطیوں کو بالکل بے نقاب کرتے ہیں جو HTTP API واپس آئے گا۔

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

کمانڈ JSON کا خلاصہ خارج کرتا ہے (منشور ڈائجسٹ ، وینڈر ID ، پروف ڈائجسٹ ، نمونوں کی تعداد ، اختیاری فیصلہ)۔ `--manifest-id=<hex>` فراہم کریں اس بات کو یقینی بنانے کے لئے کہ ذخیرہ شدہ مینی فیسٹ چیلنج ڈائجسٹ سے مماثل ہے ، اور اگر آپ اصل نمونے کے ساتھ ڈائجسٹ کو آڈٹ ثبوت کے طور پر محفوظ کرنا چاہتے ہیں تو `--json-out=<path>`۔ `--verdict` سمیت پورے چیلنج → پروف → فیصلے کے چکر کو HTTP API کو کال کرنے سے پہلے مقامی طور پر دہرانے کی اجازت دیتا ہے۔

ایک بار Torii آن لائن ہونے کے بعد ، آپ HTTP کے ذریعے وہی نمونے بازیافت کرسکتے ہیں:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

دونوں اختتامی مقامات کو جہاز والے اسٹوریج ورکر کے ذریعہ پیش کیا جاتا ہے ، تاکہ سی ایل آئی کے دھواں کے ٹیسٹ اور گیٹ وے کی تحقیقات منسلک رہیں۔

## 2. لوپ پن → بازیافت

1. ایک منشور + پے لوڈ بنڈل تیار کریں (مثال کے طور پر `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` کے ذریعے)۔
2. بیس 64 مینی فیسٹ جمع کروائیں:

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

   بیس 64 `data_b64` فیلڈ کو ڈیکوڈ کریں اور تصدیق کریں کہ یہ اصل بائٹس سے مماثل ہے۔

## 3. ریبوٹ کے بعد دوبارہ شروع کریں ڈرل

1. کم از کم ایک ظاہر اوپر کی طرح پن کریں۔
2. عمل کو دوبارہ شروع کریں Torii (یا مکمل نوڈ)۔
3. بازیافت کی درخواست واپس کریں۔ پے لوڈ کو وصولی کے قابل رہنا چاہئے اور واپس آنے والے ڈائجسٹ کو دوبارہ شروع کرنے سے پہلے قیمت کے مطابق ہونا چاہئے۔
4. `GET /v2/sorafs/storage/state` کا معائنہ کریں تاکہ اس بات کی تصدیق کی جاسکے کہ `bytes_used` دوبارہ شروع ہونے کے بعد ظاہر ہوتا ہے۔

## 4. کوٹہ مسترد ٹیسٹ

1. عارضی طور پر `torii.sorafs.storage.max_capacity_bytes` کو ایک چھوٹی سی قیمت (جیسے ایک ہی مینی فیسٹ کا سائز) تک کم کریں۔
2. ایک منشور پن پن ؛ درخواست کو کامیاب ہونا چاہئے۔
3. اسی طرح کے دوسرے مینی فیسٹ کو پن کرنے کی کوشش کریں۔ Torii کو HTTP `400` اور `storage capacity exceeded` پر مشتمل ایک غلطی پیغام کے ساتھ درخواست کو مسترد کرنا چاہئے۔
4. جانچ کے بعد عام صلاحیت کی حد کو بحال کریں۔

## 5. پور نمونے لینے کا سروے

1. ایک منشور پن۔
2. پور نمونہ کی درخواست کریں:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. تصدیق کریں کہ جواب میں `samples` درخواست کی گنتی کے ساتھ شامل ہے اور یہ کہ ہر ثبوت ذخیرہ شدہ مینی فیسٹ کی جڑ کے خلاف توثیق کرتا ہے۔

## 6. آٹومیشن ہکس

- CI / دھواں کے ٹیسٹ میں شامل کردہ ٹارگٹ کنٹرولز کو دوبارہ استعمال کیا جاسکتا ہے:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  جس کا احاطہ `pin_fetch_roundtrip` ، `pin_survives_restart` ، `pin_quota_rejection` اور `por_sampling_returns_verified_proofs`۔
- ڈیش بورڈز کو لازمی طور پر عمل کرنا چاہئے:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` اور `torii_sorafs_storage_fetch_inflight`
  - `/v2/sorafs/capacity/state` کے ذریعے بے نقاب ہونے والی کامیابی/ناکامی کاؤنٹرز
  - `sorafs_node_deal_publish_total{result=success|failure}` کے ذریعے تصفیہ شائع کرنے کی کوششیں

ان مشقوں کے بعد یہ یقینی بناتا ہے کہ جہاز والے اسٹوریج کا کارکن اعداد و شمار کو کھا سکتا ہے ، ریبوٹس سے بچ سکتا ہے ، تشکیل شدہ کوٹے کو پورا کرسکتا ہے ، اور اس سے پہلے کہ نوڈ باقی نیٹ ورک میں اپنی صلاحیت کا اعلان کرنے سے پہلے اس سے پہلے کہ اس کی صلاحیت کا اعلان کرے۔