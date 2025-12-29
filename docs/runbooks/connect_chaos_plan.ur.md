---
lang: ur
direction: rtl
source: docs/runbooks/connect_chaos_plan.md
status: complete
translator: Codex (automated)
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-11-18T04:13:57.609769+00:00"
translation_last_reviewed: 2025-11-18
---

<div dir="rtl">

<!-- docs/runbooks/connect_chaos_plan.md کا اردو ترجمہ -->

# Connect کے لیے Chaos اور Fault ریہرسل پلان (IOS3 / IOS7)

یہ پلے بک، IOS3/IOS7 روڈ میپ کی کارروائی _“مشترکہ chaos ریہرسل کی منصوبہ بندی”_
(`roadmap.md:1527`) کو پورا کرنے والے دہرائے جا سکنے والے drills بیان کرتی ہے۔ جب
بھی کراس-SDK ڈیموز اسٹیج کریں تو اسے Connect پری ویو رن بک
(`docs/runbooks/connect_session_preview_runbook.md`) کے ساتھ جوڑیں۔

## مقاصد اور کامیابی کے معیارات
- مشترکہ Connect retry/back-off پالیسی، آف لائن کیو حدود، اور ٹیلی میٹری
  ایکسپورٹرز کو کنٹرولڈ فالٹس کے تحت exercise کریں، بغیر پروڈکشن کوڈ بدلے۔
- ڈیٹرمنسٹک artefacts (`iroha connect queue inspect` آؤٹ پٹ،
  `connect.*` میٹرک اسنیپ شاٹس، Swift/Android/JS SDK لاگز) جمع کریں تاکہ
  گورننس ہر drill کا آڈٹ کر سکے۔
- دکھائیں کہ والٹس اور dApps کنفیگ تبدیلیوں (manifest drift، salt rotation،
  attestation failures) کا احترام کرتے ہیں، یعنی معیاری `ConnectError`
  زمرہ اور ریڈیکشن-محفوظ ٹیلی میٹری ایونٹس سامنے آتے ہیں۔

## پیشگی شرائط
1. **انوائرمنٹ بوٹ اسٹرپ**
   - ڈیمو Torii اسٹیک شروع کریں: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - کم از کم ایک SDK sample لانچ کریں (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android کا `demo-connect`, JS `examples/connect`)۔
2. **انسٹرومنٹیشن**
   - SDK ڈائیگناسٹکس فعال کریں (`ConnectQueueDiagnostics`,
     `ConnectQueueStateTracker`, `ConnectSessionDiagnostics` Swift میں؛
     Android/JS میں `ConnectQueueJournal` + `ConnectQueueJournalTests` کے مساوی)۔
   - یقین کریں کہ CLI `iroha connect queue inspect --sid <sid> --metrics`
     اس کیو پاتھ کو resolve کرتا ہے جو SDK بناتا ہے (`~/.iroha/connect/<sid>/state.json`
     اور `metrics.ndjson`)۔
   - ٹیلی میٹری ایکسپورٹرز کو وائر کریں تاکہ درج ذیل time series Grafana میں اور
     `scripts/swift_status_export.py telemetry` کے ذریعے دکھائی دیں:
     `connect.queue_depth`, `connect.queue_dropped_total`,
     `connect.reconnects_total`, `connect.resume_latency_ms`,
     `swift.connect.frame_latency`, `android.telemetry.redaction.salt_version`.
3. **ثبوتی فولڈرز** – `artifacts/connect-chaos/<date>/` بنائیں اور اس میں محفوظ کریں:
   - خام لاگز (`*.log`)، میٹرک اسنیپ شاٹس (`*.json`)، ڈیش بورڈ ایکسپورٹس (`*.png`)،
     CLI آؤٹ پٹس، اور PagerDuty IDs۔

## منظرنامہ میٹرکس

| ID | خرابی | انجیکشن کے مراحل | متوقع سگنلز | ثبوت |
|----|-------|------------------|-------------|------|
| C1 | WebSocket آؤٹیج اور reconnect | `/v1/connect/ws` کو پراکسی کے پیچھے لپیٹیں (جیسے `kubectl -n demo port-forward svc/torii 18080:8080` کے ساتھ `toxiproxy-cli toxic add ... timeout`) یا عارضی طور پر سروس کو بلاک کریں (`kubectl scale deploy/torii --replicas=0` زیادہ سے زیادہ 60 s کے لیے)۔ والٹ کو مجبور کریں کہ فریم بھیجتا رہے تاکہ آف لائن کیوز بھر جائیں۔ | `connect.reconnects_total` میں اضافہ، `connect.resume_latency_ms` اسپائک کرتا ہے مگر P95 < 1 s رہتا ہے، کیوز `ConnectQueueStateTracker` کے ذریعے `state=Draining` میں جاتی ہیں۔ SDKs ایک بار `ConnectError.Transport.reconnecting` خارج کرتے ہیں، پھر معمول پر آتے ہیں۔ | - `iroha connect queue inspect --sid <sid>` آؤٹ پٹ جس میں غیر صفر `resume_attempts_total` ہو۔<br>- outage ونڈو کے لیے ڈیش بورڈ اینوٹیشن۔<br>- reconnect + drain پیغامات والا لاگ اقتباس۔ |
| C2 | آف لائن کیو overflow / TTL میعاد ختم | sample میں ترمیم کر کے کیو حدود کم کریں (Swift: `ConnectSessionDiagnostics` کے اندر `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` پاس کریں؛ Android/JS میں متعلقہ constructor استعمال کریں)۔ جب dApp درخواستیں enqueuing کرتا رہے تو والٹ کو کم از کم `retentionInterval` کے دو گنا وقت کے لیے معطل کریں۔ | `connect.queue_dropped_total{reason="overflow"}` اور `{reason="ttl"}` میں اضافہ، `connect.queue_depth` نئی حد پر فلیٹ ہو جاتا ہے، SDKs `ConnectError.QueueOverflow(limit: 4)` (یا `.QueueExpired`) دکھاتے ہیں۔ `iroha connect queue inspect` میں `state=Overflow` اور `warn/drop` واٹر مارکس 100% پر نظر آتے ہیں۔ | - میٹرک کاؤنٹرز کا اسکرین شاٹ۔<br>- overflow کو قید کرنے والا CLI JSON آؤٹ پٹ۔<br>- Swift/Android کا لاگ اقتباس جس میں `ConnectError` لائن ہو۔ |
| C3 | Manifest drift / admission rejection | والٹس کو فراہم کردہ Connect manifest میں چھیڑ چھاڑ کریں (مثلاً `docs/connect_swift_ios.md` کا sample manifest بدلیں، یا Torii کو `--connect-manifest-path` کے ساتھ شروع کریں جو کسی کاپی کی طرف اشارہ کرے جہاں `chain_id` یا `permissions` مختلف ہوں)۔ dApp سے approval مانگوائیں اور یقینی بنائیں کہ والٹ پالیسی کے ذریعے مسترد کرے۔ | Torii `/v1/connect/session` کے لیے `HTTP 409` لوٹاتا ہے جس میں `manifest_mismatch` ہوتا ہے، SDKs `ConnectError.Authorization.manifestMismatch(manifestVersion)` نکالتے ہیں، ٹیلی میٹری `connect.manifest_mismatch_total` بڑھاتی ہے، اور کیوز خالی (`state=Idle`) رہتی ہیں۔ | - mismatch detection والا Torii لاگ اقتباس۔<br>- ظاہر شدہ غلطی کا SDK اسکرین شاٹ۔<br>- ٹیسٹ کے دوران کیوز خالی ہونے کا ثبوت دینے والا میٹرک اسنیپ شاٹ۔ |
| C4 | Key rotation / salt-version bump | سیشن کے دوران Connect salt یا AEAD key گھمائیں۔ ڈیو اسٹیکس میں Torii کو `CONNECT_SALT_VERSION=$((old+1))` کے ساتھ دوبارہ شروع کریں (Android redaction salt ٹیسٹ `docs/source/sdk/android/telemetry_schema_diff.md` کے مماثل)۔ والٹ کو salt rotation مکمل ہونے تک آف لائن رکھیں، پھر دوبارہ چلائیں۔ | پہلا resume `ConnectError.Authorization.invalidSalt` کے ساتھ ناکام ہوتا ہے، کیوز flush ہوتی ہیں (dApp محفوظ فریمز کو `salt_version_mismatch` وجہ کے ساتھ drop کرتا ہے)، ٹیلی میٹری `android.telemetry.redaction.salt_version` (Android) اور `swift.connect.session_event{event="salt_rotation"}` جاری کرتی ہے۔ دوسرا سیشن SID ریفریش کے بعد کامیاب ہوتا ہے۔ | - salt epoch سے پہلے/بعد ڈیش بورڈ اینوٹیشن۔<br>- invalid-salt غلطی اور بعد ازاں کامیابی کے لاگ۔<br>- `iroha connect queue inspect` آؤٹ پٹ جس میں پہلے `state=Stalled` اور پھر نیا `state=Active` ہو۔ |
| C5 | Attestation / StrongBox ناکامی | StrongBox فیلئر کی نقل کرتے ہوئے `scripts/android_keystore_attestation.sh` کے ذریعے attestation bundle بنائیں (`--inject-failure strongbox-simulated`)۔ والٹ اس bundle کو اپنے `ConnectApproval` API کے ذریعے منسلک کرے تاکہ dApp payload کو validate کر کے مسترد کرے۔ | والٹ کی درخواست `ConnectError.AttestationFailed` دکھاتی ہے، ٹیلی میٹری `connect.attestation_failed_total` اور متعلقہ Swift/Android incident میٹرکس بڑھاتی ہے، کیوز خراب انٹری کو drop کر دیتی ہیں۔ | - invalid attestation کے ساتھ اپلوڈ شدہ bundle۔<br>- مستردی کا SDK/Torii لاگ۔<br>- `connect.attestation_failed_total` اور Cached queue state کے اسکرین شاٹس۔ |

## تفصیلی اقدامات

### C1 — WebSocket آؤٹیج اور reconnect
1. Torii کی WebSocket سروس کو پراکسی کے پیچھے چلائیں یا عارضی طور پر scale کریں تاکہ کنیکشن کٹ جائے۔
2. والٹ میں ٹریفک جاری رکھیں (مثلاً approvals یا frame ارسال کرنا) تاکہ آف لائن کیوز بڑھیں۔
3. پراکسی/سروس بحال کریں، پھر `iroha connect queue inspect` کے ذریعے
   `resume_attempts_total` اور `state=Draining` کی تصدیق کریں۔
4. کامیابی = ایک reconnect غلطی، metrics میں spikes، اور لوڈ کے drain ہونے کا ثبوت۔

### C2 — آف لائن کیو overflow / TTL میعاد ختم
1. مقامی بلڈز میں کیو کی حدیں کم کریں:
   - Swift: اپنے sample (مثلاً `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     میں `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` پاس کریں۔
   - Android/JS: `ConnectQueueJournal` بناتے وقت ہم معنی کنفیگ دیں۔
2. والٹ کو ≥60 s کے لیے معطل کریں (simulator پس منظر یا airplane mode) جبکہ dApp
   `ConnectClient.requestSignature(...)` کالز جاری رکھے۔
3. `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) یا JS ڈائیگناسٹک
   ہیلپر کے ذریعے ثبوتی بنڈل (`state.json`, `journal/*.to`, `metrics.ndjson`)
   ایکسپورٹ کریں۔
4. کامیابی = overflow کاؤنٹرز میں اضافہ، SDK ایک بار `ConnectError.QueueOverflow`
   دکھائے، اور والٹ بحالی کے بعد کیو نارمل ہو جائے۔

### C3 — Manifest drift / admission rejection
1. admission manifest کی کاپی بنائیں، مثلاً:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. Torii کو `--connect-manifest-path /tmp/manifest_drift.json` کے ساتھ لانچ کریں
   (یا drill کے لیے docker compose/k8s کنفیگ اپ ڈیٹ کریں)۔
3. والٹ سے سیشن شروع کرنے کی کوشش کریں؛ توقع کریں HTTP 409 ملے گا۔
4. Torii + SDK لاگز اور ٹیلی میٹری کا `connect.manifest_mismatch_total` قید کریں۔
5. کامیابی = مستردی کے دوران کیو بڑھنے نہ پائے اور والٹ مشترکہ ٹیکسونومی
   غلطی (`ConnectError.Authorization.manifestMismatch`) دکھائے۔

### C4 — Key rotation / salt bump
1. ٹیلی میٹری سے موجودہ salt ورژن ریکارڈ کریں:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. Torii کو نئے salt کے ساتھ ری اسٹارٹ کریں (`CONNECT_SALT_VERSION=$((OLD+1))`
   یا کنفیگ میپ اپ ڈیٹ کریں)۔ ری اسٹارٹ کے دوران والٹ آف لائن رکھیں۔
3. والٹ کو دوبارہ چلائیں؛ پہلا resume invalid-salt کے ساتھ ناکام ہونا چاہیے اور
   `connect.queue_dropped_total{reason="salt_version_mismatch"}` میں اضافہ ہو۔
4. cached فریمز ڈراپ کرنے کے لیے سیشن ڈائریکٹری حذف کریں (`rm -rf ~/.iroha/connect/<sid>`
   یا پلیٹ فارم مخصوص cache کلئیر)، پھر نئے ٹوکنز سے سیشن دوبارہ شروع کریں۔
5. کامیابی = ٹیلی میٹری salt bump دکھائے، ایک ہی invalid resume واقعہ لاگ ہو،
   اور اگلا سیشن دستی مداخلت کے بغیر کامیاب ہو۔

### C5 — Attestation / StrongBox ناکامی
1. `scripts/android_keystore_attestation.sh` چلا کر attestation bundle بنائیں
   (`--inject-failure strongbox-simulated`) تاکہ دستخط بٹ پلٹا جا سکے۔
2. والٹ اس bundle کو `ConnectApproval` API کے ذریعے منسلک کرے؛ dApp اسے validate
   کر کے مسترد کرے۔
3. ٹیلی میٹری (`connect.attestation_failed_total`, Swift/Android incident میٹرکس)
   اور کیو کے drop شدہ اندراجات ریکارڈ کریں۔
4. کامیابی = خراب approval تک محدود مستردی، صحت مند کیوز، اور drill ثبوت کے ساتھ
   محفوظ شدہ attestation لاگ۔

## ثبوتی چیک لسٹ
- `artifacts/connect-chaos/<date>/c*_metrics.json` (بذریعہ
  `scripts/swift_status_export.py telemetry`)۔
- `iroha connect queue inspect` سے CLI آؤٹ پٹس (`c*_queue.txt`)۔
- SDK + Torii لاگز بمعہ ٹائم اسٹیمپ اور SID ہیشز۔
- ہر منظرنامے کے لیے اینوٹیشن کے ساتھ ڈیش بورڈ اسکرین شاٹس۔
- اگر Sev 1/2 الرٹس فائر ہوں تو PagerDuty/انسڈنٹ IDs۔

ہر سہ ماہی میں مکمل میٹرکس چلانا روڈ میپ گیٹ کو پورا کرتا ہے اور ظاہر
کرتا ہے کہ Swift/Android/JS Connect نفاذات بلند ترین خطرے والے فالٹس میں بھی
ڈیٹرمنسٹک انداز سے ردعمل دیتے ہیں۔

</div>
