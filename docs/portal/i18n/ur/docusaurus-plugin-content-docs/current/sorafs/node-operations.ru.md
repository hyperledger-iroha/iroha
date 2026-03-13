---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/node-operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: نوڈ آپریشنز
عنوان: نوڈ آپریشنز کی رن بک
سائڈبار_لیبل: نوڈ آپریشنز کی رن بک
تفصیل: Torii کے اندر بلٹ میں تعیناتی `sorafs-node` کی جانچ پڑتال۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_node_ops.md` کی عکاسی کرتا ہے۔ جب تک میراثی اسفنکس دستاویزات کا سیٹ ریٹائر نہیں ہوتا ہے اس وقت تک دونوں کاپیاں ہم وقت ساز رکھیں۔
:::

## جائزہ

یہ رن بک آپریٹرز کو بلٹ میں تعیناتی `sorafs-node` کے اندر جانچنے کے ذریعے چلتا ہے
Torii۔ ہر سیکشن براہ راست SF-3 کی فراہمی سے مطابقت رکھتا ہے: پن/بازیافت لوپس ،
بازیافت دوبارہ شروع کرنے کے بعد ، کوٹہ کی ناکامی اور پور نمونے لینے کے بعد۔

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

- اس بات کو یقینی بنائیں کہ اس عمل Torii نے `data_dir` تک پڑھنے/تحریری رسائی حاصل کی ہے۔
- اس بات کی تصدیق کریں کہ نوڈ متوقع صلاحیت کے ذریعے تشہیر کرتا ہے
  `GET /v2/sorafs/capacity/state` اعلامیہ ریکارڈ کرنے کے بعد۔
- جب اینٹی ایلیسنگ فعال ہوجاتی ہے تو ، ڈیش بورڈ دونوں کچے اور ہموار دونوں کو دکھاتے ہیں
  جِب آور/پور کاؤنٹرز جٹر فری رجحانات کو اجاگر کرنے کے لئے آگے
  فوری اقدار۔

### سی ایل آئی ٹرائل رن (اختیاری)

HTTP کے اختتامی مقامات کو کھولنے سے پہلے ، آپ بیکینڈ اسٹوریج کا غیر سنجیدہ چیک انجام دے سکتے ہیں
فراہم کردہ سی ایل آئی کا استعمال کرتے ہوئے۔

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

کمانڈز آؤٹ پٹ Norito JSON خلاصہ اور مسترد کریں پروفائل مماثلت کو مسترد کریں یا
ہضم ، Torii کو مربوط کرنے سے پہلے سی آئی کے دھواں چیکوں کے ل useful مفید بنانا۔

### پور پروف ریہرسل

آپریٹرز اب مقامی طور پر گورننس کے ذریعہ جاری کردہ پور نمونے کو دوبارہ پیش کرسکتے ہیں۔
Torii میں لوڈ کرنے سے پہلے۔ CLI وہی راستہ استعمال کرتا ہے جس میں `sorafs-node` ، تو
یہ مقامی رنز وہی توثیق کی غلطیاں دکھاتے ہیں جو HTTP API واپس آئے گی۔

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

کمانڈ JSON کا خلاصہ (منشور ڈائجسٹ ، فراہم کنندہ ID ، ڈائجسٹ ثبوت ،
نمونوں کی تعداد ، اختیاری نتیجہ کا فیصلہ)۔ `--manifest-id=<hex>` کی وضاحت کریں ،
اس بات کو یقینی بنانے کے لئے کہ محفوظ شدہ مینی فیسٹ ڈائجسٹ کال سے مماثل ہے ، اور
`--json-out=<path>` جب آپ کو ماخذ نمونے کے ساتھ خلاصہ آرکائیو کرنے کی ضرورت ہوتی ہے
آڈٹ کے ثبوت کے طور پر۔ `--verdict` شامل کرنے سے آپ کو مکمل مشق کرنے کی اجازت ملتی ہے
HTTP API پر کال کرنے سے پہلے سائیکل کال → پروف → فیصلے آف لائن۔

Torii چلانے کے بعد آپ HTTP کے ذریعے وہی نمونے حاصل کرسکتے ہیں:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

دونوں اختتامی مقامات بلٹ ان اسٹوریج ورکر کے ذریعہ پیش کیے جاتے ہیں ، لہذا سی ایل آئی سگریٹ نوشی کے ٹیسٹ اور
گیٹ وے کی تحقیقات ہم آہنگ ہیں۔ 【کریٹس/اروہ_ٹوری/ایس آر سی/سرفس/api.rs#l1207 】【 کریٹ/اروہہ_ٹوری/ایس آر سی/سورافس/api.rs#l1259】

## 2. لوپ پن → بازیافت

1. ایک منشور + پے لوڈ پیکیج بنائیں (مثال کے طور پر ، استعمال کرتے ہوئے
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`)۔
2. بیس 64 انکوڈنگ میں مینی فیسٹ بھیجیں:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   درخواست JSON میں `manifest_b64` اور `payload_b64` پر مشتمل ہونا چاہئے۔ کامیاب جواب
   `manifest_id_hex` اور ہضم پے لوڈ کو لوٹاتا ہے۔
3. پن والا ڈیٹا حاصل کریں:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```فیلڈ `data_b64` کو بیس 64 سے ڈیکوڈ کریں اور چیک کریں کہ یہ اصل بائٹس سے مماثل ہے۔

## 3. بازیافت کی تربیت دوبارہ شروع کرنے کے بعد

1. کم از کم ایک مینی فیسٹ پن کریں جیسا کہ اوپر بیان کیا گیا ہے۔
2. Torii عمل (یا پورا نوڈ) دوبارہ شروع کریں۔
3. بازیافت کی درخواست کو دوبارہ جمع کریں۔ پے لوڈ کو اب بھی بازیافت کرنا چاہئے اور ہضم ہونا چاہئے
   ردعمل کو دوبارہ شروع ہونے والے ایک سے میچ کرنا چاہئے۔
4. `GET /v2/sorafs/storage/state` چیک کریں تاکہ یہ یقینی بنایا جاسکے کہ `bytes_used` کی عکاسی ہو رہی ہے
   ریبوٹ کے بعد بچائے گئے ظاہر۔

## 4. کوٹہ انکار ٹیسٹ

1. عارضی طور پر `torii.sorafs.storage.max_capacity_bytes` کو ایک چھوٹی سی قیمت میں کم کریں
   (مثال کے طور پر ، ایک منشور کا سائز)۔
2. پن ایک منشور ؛ درخواست کامیاب ہونی چاہئے۔
3. موازنہ سائز کے دوسرے مظہر کو جوڑنے کی کوشش کریں۔ Torii کو مسترد کرنا چاہئے
   HTTP `400` اور `storage capacity exceeded` پر مشتمل ایک غلطی پیغام کے ساتھ درخواست کریں۔
4. جب ختم ہوجائے تو ، صلاحیت کی معمول کی حد کو بحال کریں۔

## 5. پور نمونے لینے کا امتحان

1. پن مینی فیسٹ۔
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

3. چیک کریں کہ جواب میں `samples` درخواست کی مقدار کے ساتھ اور ہر ایک پر مشتمل ہے
   اس کا ثبوت محفوظ شدہ منشور کی جڑ کے خلاف ہے۔

## 6. آٹومیشن ہکس

- CI/دھواں ٹیسٹ میں شامل ہدف چیکوں کو دوبارہ استعمال کرسکتے ہیں:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  جس کا احاطہ `pin_fetch_roundtrip` ، `pin_survives_restart` ، `pin_quota_rejection`
  اور `por_sampling_returns_verified_proofs`۔
- ڈیش بورڈز کو ٹریک کرنا چاہئے:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` اور `torii_sorafs_storage_fetch_inflight`
  - `/v2/sorafs/capacity/state` کے ذریعے شائع کردہ POR کامیابی/ناکامی کاؤنٹرز
  - `sorafs_node_deal_publish_total{result=success|failure}` کے ذریعے تصفیہ شائع کرنے کی کوششیں

ان مشقوں پر عمل کرنے سے یہ یقینی بنائے گا کہ بلٹ ان اسٹوریج ورکر اس قابل ہے
اعداد و شمار کو انجیر کریں ، دوبارہ شروع ہونے سے بچیں ، مخصوص کوٹے کو پورا کریں اور پیدا کریں
اس سے پہلے کہ کسی نوڈ کو وسیع تر نیٹ ورک میں اپنی صلاحیت کا اعلان کرنے سے پہلے ڈٹرمینسٹک پور ثبوت۔