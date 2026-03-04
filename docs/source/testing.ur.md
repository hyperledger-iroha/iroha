---
lang: ur
direction: rtl
source: docs/source/testing.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7d9bce40727d178bcc7d780c608d82bcd14b0814a7b537cbe9c39a539a200c8
source_last_modified: "2025-12-19T22:31:17.718007+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/testing.md -->

# ٹیسٹنگ اور ٹربل شوٹنگ گائیڈ

یہ گائیڈ بتاتا ہے کہ انضمامی منظرنامے کیسے دوبارہ بنائیں، کون سا انفراسٹرکچر آن لائن ہونا چاہیے، اور قابل عمل لاگز کیسے جمع کریں۔ شروع کرنے سے پہلے منصوبے کی [status report](../../status.md) دیکھیں تاکہ معلوم ہو سکے کہ کون سے اجزا اس وقت گرین ہیں۔

## دوبارہ پیدا کرنے کے مراحل

### انضمامی ٹیسٹس (`integration_tests` crate)

1. یقینی بنائیں کہ workspace کی dependencies بن چکی ہیں: `cargo build --workspace`.
2. مکمل logs کے ساتھ انضمامی ٹیسٹ سوٹ چلائیں: `cargo test -p integration_tests -- --nocapture`.
3. اگر کسی مخصوص منظرنامے کو دوبارہ چلانا ہو تو اس کا module path استعمال کریں، مثلاً `cargo test -p integration_tests settlement::happy_path -- --nocapture`.
4. Norito کے ساتھ serialized fixtures capture کریں تاکہ نوڈز کے درمیان یکساں inputs رہیں:
   ```rust
   use norito::json;

   let genesis_payload = json::to_string_pretty(&json::json!({
       "chain" : "testnet",
       "peers" : ["127.0.0.1:1337"],
       "accounts" : [{
           "id" : "ih58...",
           "public_key" : "ed0120..."
       }]
   }))?;
   ```
   حاصل شدہ Norito JSON کو test artifacts کے ساتھ محفوظ کریں تاکہ peers وہی state دوبارہ چلا سکیں۔

### Python کلائنٹ ٹیسٹس (`pytests` directory)

1. Python requirements کو virtual environment میں `pip install -r pytests/requirements.txt` سے انسٹال کریں۔
2. اوپر بنائے گئے Norito format fixtures کو کسی shared path یا environment variable کے ذریعے export کریں۔
3. verbose output کے ساتھ سوٹ چلائیں: `pytest -vv pytests`.
4. targeted debugging کے لیے `pytest -k "Query" pytests/tests/test_queries.py --log-cli-level=INFO` چلائیں۔

## ضروری پورٹس اور سروسز

کسی بھی سوٹ کو چلانے سے پہلے درج ذیل سروسز قابل رسائی ہونی چاہئیں:

- **Torii HTTP API**: ڈیفالٹ `127.0.0.1:1337`. `torii.address` کے ذریعے config میں override کریں (دیکھیے `docs/source/references/peer.template.toml`).
- **Torii WebSocket notifications**: ڈیفالٹ `127.0.0.1:8080` جو `pytests` کے کلائنٹ سبسکرپشنز استعمال کرتے ہیں۔
- **Telemetry exporter**: ڈیفالٹ `127.0.0.1:8180`. انضمامی ٹیسٹس صحت کی جانچ کے لیے metrics کو یہاں آنا توقع کرتے ہیں۔
- **PostgreSQL** (جب enabled ہو): ڈیفالٹ `127.0.0.1:5432`. یقینی بنائیں کہ credentials, [`defaults/docker-compose.local.yml`](../../defaults/docker-compose.local.yml) میں compose profile سے align ہوں۔

اگر کوئی endpoint دستیاب نہ ہو تو [telemetry troubleshooting guide](telemetry.md) دیکھیں۔

### Embedded peer stability

`NetworkBuilder::start()` اب ہر embedded peer کے لیے genesis کے بعد پانچ سیکنڈ کی liveness window enforce کرتا ہے۔ اگر کوئی process اس guard period میں ختم ہو جائے تو builder ایک تفصیلی error دیتا ہے جو cached stdout/stderr logs کی طرف اشارہ کرتا ہے۔ کم وسائل والی مشینوں پر آپ `IROHA_TEST_POST_GENESIS_LIVENESS_MS` سیٹ کر کے window (ملی سیکنڈز میں) بڑھا سکتے ہیں؛ اسے `0` پر لانے سے guard مکمل طور پر بند ہو جاتا ہے۔ یقینی بنائیں کہ ہر انضمامی سوٹ کے ابتدائی چند سیکنڈز میں کافی CPU headroom موجود ہو تاکہ peers بلاک 1 تک پہنچ سکیں اور watchdog trigger نہ ہو۔

## لاگ جمع کرنا اور تجزیہ

صاف run directory سے شروع کریں تاکہ پچھلے artifacts نئی خرابیوں کو نہ چھپائیں۔ نیچے دیے گئے اسکرپٹس ایسے فارمیٹس میں logs جمع کرتے ہیں جنہیں Norito tooling استعمال کر سکتی ہے۔

- ٹیسٹ کے بعد [`scripts/analyze_telemetry.sh`](../../scripts/analyze_telemetry.sh) استعمال کریں تاکہ node metrics کو timestamped Norito JSON snapshots میں aggregate کیا جا سکے۔
- نیٹ ورکنگ مسائل کی تفتیش میں [`scripts/run_iroha_monitor_demo.py`](../../scripts/run_iroha_monitor_demo.py) چلائیں تاکہ Torii events کو `monitor_output.norito.json` میں stream کیا جا سکے۔
- انضمامی ٹیسٹ logs `integration_tests/target/` کے تحت ہوتے ہیں؛ انہیں [`scripts/profile_build.sh`](../../scripts/profile_build.sh) کے ذریعے compress کریں تاکہ دوسری ٹیموں کے ساتھ شیئر کیا جا سکے۔
- Python client logs `pytests/.pytest_cache` میں لکھے جاتے ہیں۔ انہیں captured telemetry کے ساتھ export کریں:
  ```bash
  ./scripts/report_red_team_failures.py --tests pytests --artifacts out/logs
  ```

Issue کھولنے سے پہلے مکمل bundle (integration, Python, telemetry) جمع کریں تاکہ maintainers Norito traces کو replay کر سکیں۔

## اگلے اقدامات

Release-specific checklists کے لیے [pipeline](pipeline.md) دیکھیں۔ اگر regressions یا failures سامنے آئیں تو انہیں مشترکہ [status tracker](../../status.md) میں درج کریں اور متعلقہ [sumeragi troubleshooting](sumeragi.md) entries سے cross-reference کریں۔

</div>
