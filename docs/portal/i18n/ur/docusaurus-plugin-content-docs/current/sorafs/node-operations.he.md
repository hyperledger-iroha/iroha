---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9211dc8496aca3caf63105d80a1f26a5e17f73fd7df259373a31d9bbb6dfa7d
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: node-operations
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/node-operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_node_ops.md` کی عکاسی کرتا ہے۔ جب تک پرانا Sphinx ڈاکیومنٹیشن سیٹ مکمل طور پر منتقل نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

## جائزہ

یہ رن بک آپریٹرز کو Torii کے اندر ایمبیڈڈ `sorafs-node` ڈیپلائمنٹ کی توثیق میں
رہنمائی کرتی ہے۔ ہر سیکشن براہِ راست SF-3 deliverables سے میپ ہوتا ہے: pin/fetch
راؤنڈ ٹرپس، ری اسٹارٹ ریکوری، کوٹا ریجیکشن، اور PoR سیمپلنگ۔

## 1. پیشگی تقاضے

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

- یقینی بنائیں کہ Torii پروسس کے پاس `data_dir` تک read/write رسائی ہو۔
- declaration ریکارڈ ہونے کے بعد `GET /v1/sorafs/capacity/state` کے ذریعے تصدیق
  کریں کہ نوڈ متوقع کپیسٹی اعلان کرتا ہے۔
- جب smoothing فعال ہو، ڈیش بورڈز raw اور smoothed GiB·hour/PoR کاؤنٹرز دونوں
  دکھاتے ہیں تاکہ jitter-free رجحانات کو spot values کے ساتھ نمایاں کیا جا سکے۔

### CLI ڈرائی رن (اختیاری)

HTTP endpoints ایکسپوز کرنے سے پہلے آپ bundled CLI کے ذریعے اسٹوریج backend کا
sanity-check کر سکتے ہیں۔【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

یہ کمانڈز Norito JSON summaries پرنٹ کرتی ہیں اور chunk-profile یا digest mismatch
کو مسترد کرتی ہیں، جس سے یہ Torii wiring سے پہلے CI smoke checks کے لیے مفید بنتی
ہیں۔【crates/sorafs_node/tests/cli.rs#L1】

### PoR پروف کی مشق

آپریٹرز اب گورننس کی طرف سے جاری کردہ PoR artifacts کو Torii پر اپ لوڈ کرنے سے
پہلے لوکل طور پر ری پلے کر سکتے ہیں۔ CLI اسی `sorafs-node` ingestion پاتھ کو reuse
کرتی ہے، اس لیے لوکل رنز وہی validation errors ظاہر کرتے ہیں جو HTTP API واپس
لوٹاتی ہے۔

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

کمانڈ ایک JSON summary emit کرتی ہے (manifest digest، provider id، proof digest،
sample count، اور optional verdict outcome)۔ `--manifest-id=<hex>` فراہم کریں تاکہ
اسٹور شدہ manifest چیلنج digest سے میچ کرے، اور `--json-out=<path>` استعمال کریں
جب آپ summary کو اصل artifacts کے ساتھ audit evidence کے طور پر آرکائیو کرنا
چاہیں۔ `--verdict` شامل کرنے سے آپ HTTP API کال کرنے سے پہلے آف لائن چیلنج → پروف
→ verdict لوپ کی پوری مشق کر سکتے ہیں۔

Torii لائیو ہونے کے بعد آپ وہی artifacts HTTP کے ذریعے حاصل کر سکتے ہیں:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

دونوں endpoints ایمبیڈڈ اسٹوریج ورکر فراہم کرتا ہے، اس لیے CLI smoke tests اور
gateway probes ہم آہنگ رہتے ہیں۔【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Pin → Fetch راؤنڈ ٹرپ

1. manifest + payload بنڈل تیار کریں (مثلاً
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` کے ذریعے)۔
2. manifest کو base64 encoding کے ساتھ جمع کریں:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   request JSON میں `manifest_b64` اور `payload_b64` شامل ہونا چاہیے۔ کامیاب
   response `manifest_id_hex` اور payload digest واپس کرتی ہے۔
3. pinned ڈیٹا fetch کریں:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   `data_b64` فیلڈ کو base64 decode کریں اور تصدیق کریں کہ یہ اصل bytes سے میچ کرتی ہے۔

## 3. ری اسٹارٹ ریکوری ڈرل

1. اوپر دیے گئے طریقے کے مطابق کم از کم ایک manifest pin کریں۔
2. Torii پروسس (یا پورا نوڈ) ری اسٹارٹ کریں۔
3. fetch ریکوئسٹ دوبارہ جمع کریں۔ payload بدستور دستیاب رہے اور واپس آنے والا
   digest ری اسٹارٹ سے پہلے والی قدر کے مطابق ہو۔
4. `GET /v1/sorafs/storage/state` چیک کریں تاکہ تصدیق ہو کہ `bytes_used` ری بوٹ کے
   بعد persisted manifests کو ظاہر کرتا ہے۔

## 4. کوٹا ریجیکشن ٹیسٹ

1. عارضی طور پر `torii.sorafs.storage.max_capacity_bytes` کو چھوٹی قدر پر لائیں
   (مثلاً ایک manifest کے سائز کے برابر)۔
2. ایک manifest pin کریں؛ ریکوئسٹ کامیاب ہونی چاہیے۔
3. اسی طرح کے سائز والا دوسرا manifest pin کرنے کی کوشش کریں۔ Torii کو ریکوئسٹ
   HTTP `400` کے ساتھ مسترد کرنی چاہیے اور ایرر میسج میں `storage capacity exceeded`
   شامل ہونا چاہیے۔
4. ختم ہونے پر نارمل کپیسٹی حد بحال کریں۔

## 5. PoR سیمپلنگ پروب

1. ایک manifest pin کریں۔
2. PoR sample ریکوئسٹ کریں:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. تصدیق کریں کہ response میں `samples` مطلوبہ count کے ساتھ موجود ہیں اور ہر
   proof اسٹور شدہ manifest root کے خلاف ویریفائی ہوتا ہے۔

## 6. آٹومیشن ہکس

- CI / smoke tests درج ذیل میں شامل ٹارگٹڈ چیکس دوبارہ استعمال کر سکتے ہیں:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  جو `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`, اور
  `por_sampling_returns_verified_proofs` کو کور کرتے ہیں۔
- ڈیش بورڈز کو یہ ٹریک کرنا چاہیے:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` اور `torii_sorafs_storage_fetch_inflight`
  - `/v1/sorafs/capacity/state` کے ذریعے ظاہر کیے گئے PoR کامیابی/ناکامی کاؤنٹرز
  - `sorafs_node_deal_publish_total{result=success|failure}` کے ذریعے settlement publish attempts

ان drills کی پیروی سے یہ یقینی ہوتا ہے کہ ایمبیڈڈ اسٹوریج ورکر ڈیٹا ingest کر
سکے، ری اسٹارٹس برداشت کرے، مقررہ کوٹاز کا احترام کرے، اور نوڈ کے وسیع نیٹ ورک
کو کپیسٹی advertise کرنے سے پہلے ڈٹرمنسٹک PoR proofs تیار کرے۔
