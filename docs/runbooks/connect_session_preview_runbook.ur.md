---
lang: ur
direction: rtl
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b4dbba7711a733a9c2736410db29b035ce8f13bb50b532fe509a6492f239a1fe
source_last_modified: "2025-11-19T04:38:08.010772+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/runbooks/connect_session_preview_runbook.md کا اردو ترجمہ -->

# Connect سیشن پری ویو رن بک (IOS7 / JS4)

یہ رن بک IOS7 اور JS4 روڈ میپ مائل اسٹونز (`roadmap.md:1340`, `roadmap.md:1656`) کی ضرورت کے مطابق Connect پری ویو سیشنز کو اسٹیج کرنے، ویلیڈیٹ کرنے اور ختم کرنے کا end-to-end طریقہ کار بیان کرتی ہے۔ جب بھی آپ Connect strawman (`docs/source/connect_architecture_strawman.md`) کا ڈیمو دیں، SDK روڈ میپس میں وعدہ کئے گئے کیو/ٹیلی میٹری ہکس ایکسرسائز کریں، یا `status.md` کے لئے شواہد جمع کریں تو انہی مراحل پر عمل کریں۔

## 1. پری فلائٹ چیک لسٹ

| آئٹم | تفصیلات | حوالہ جات |
|------|---------|------------|
| Torii اینڈپوائنٹ + Connect پالیسی | Torii کا بیس URL، `chain_id`، اور Connect پالیسی (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`) کی تصدیق کریں۔ JSON اسنیپ شاٹ رن بک ٹکٹ میں محفوظ کریں۔ | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| Fixture + bridge ورژنز | Norito fixture کا ہیش اور bridge build نوٹ کریں (Swift کو `NoritoBridge.xcframework` درکار ہے، JS کو `@iroha/iroha-js` >= وہ ورژن درکار ہے جس میں `bootstrapConnectPreviewSession` شپ ہوا)۔ | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| ٹیلی میٹری ڈیش بورڈز | یقینی بنائیں کہ `connect.queue_depth`, `connect.queue_overflow_total`, `connect.resume_latency_ms`, `swift.connect.session_event` وغیرہ دکھانے والے ڈیش بورڈز قابل رسائی ہوں (Grafana `Android/Swift Connect` بورڈ + ایکسپورٹڈ Prometheus اسنیپ شاٹس)۔ | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| شواہد فولڈرز | `docs/source/status/swift_weekly_digest.md` (ہفتہ وار ڈائجسٹ) اور `docs/source/sdk/swift/connect_risk_tracker.md` (رسک ٹریکر) جیسی منزل منتخب کریں۔ لاگز، میٹرکس اسکرین شاٹس، اور acknowledgements کو `docs/source/sdk/swift/readiness/archive/<date>/connect/` کے تحت محفوظ کریں۔ | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. پری ویو سیشن کو بوٹ اسٹرپ کریں

1. **پالیسی اور کوٹاز ویلیڈیٹ کریں۔** کال کریں:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   اگر `queue_max` یا TTL آپ کی پلان کی گئی کنفیگ سے مختلف ہو تو رن فیل کریں۔
2. **ڈیٹرمنسٹک SID/URI بنائیں۔** `@iroha/iroha-js` کا `bootstrapConnectPreviewSession` ہیلپر SID/URI جنریشن کو Torii سیشن رجسٹریشن کے ساتھ جوڑتا ہے؛ Swift اگر WebSocket لیئر چلائے تب بھی اسے استعمال کریں۔
   ```js
   import {
     ToriiClient,
     bootstrapConnectPreviewSession,
   } from "@iroha/iroha-js";

   const client = new ToriiClient(process.env.TORII_BASE_URL, { chainId: "sora-mainnet" });
   const { preview, session, tokens } = await bootstrapConnectPreviewSession(client, {
     chainId: "sora-mainnet",
     appBundle: "dev.sora.example.dapp",
     walletBundle: "dev.sora.example.wallet",
     register: true,
   });
   console.log("sid", preview.sidBase64Url, "ws url", preview.webSocketUrl);
   ```
   - `register: false` کو QR/deep-link کے dry-run کیلئے سیٹ کریں۔
   - واپس ملنے والا `sidBase64Url`، deeplink URLs، اور `tokens` blob کو شواہد فولڈر میں محفوظ کریں؛ گورننس ریویو میں ان artefacts کی توقع ہوتی ہے۔
3. **راز تقسیم کریں۔** deeplink URI کو والٹ آپریٹر کے ساتھ شیئر کریں (Swift dApp sample، Android wallet، یا QA harness)۔ چیٹ میں raw tokens کبھی پیسٹ نہ کریں؛ enablement پیکٹ میں درج encrypted vault استعمال کریں۔

## 3. سیشن چلائیں

1. **WebSocket کھولیں۔** Swift کلائنٹس عام طور پر یوں استعمال کرتے ہیں:
   ```swift
   let connectURL = URL(string: preview.webSocketUrl)!
   let client = ConnectClient(url: connectURL)
   let sid: Data = /* decode preview.sidBase64Url into raw bytes using your harness helper */
   let session = ConnectSession(sessionID: sid, client: client)
   let recorder = ConnectReplayRecorder(sessionID: sid)
   session.addObserver(ConnectEventObserver(queue: .main) { event in
       logger.info("connect event", metadata: ["kind": "\(event.kind)"])
   })
   try client.open()
   ```
   اضافی سیٹ اپ کیلئے `docs/connect_swift_integration.md` دیکھیں (bridge imports، concurrency adapters)۔
2. **اپروول اور سائننگ فلو۔** dApps `ConnectSession.requestSignature(...)` کال کرتی ہیں، جبکہ wallets `approveSession` / `reject` کے ذریعے جواب دیتی ہیں۔ ہر اپروول میں hashed alias + permissions لاگ ہونا چاہیے تاکہ Connect governance charter سے مطابقت رہے۔
3. **کیو اور ریزیوم راستے ایکسرسائز کریں۔** نیٹ ورک کنیکٹیویٹی ٹوگل کریں یا والٹ suspend کریں تاکہ bounded queue اور replay hooks انٹریز لاگ کریں۔ JS/Android SDKs `ConnectQueueError.overflow(limit)` / `.expired(ttlMs)` امیٹ کرتے ہیں جب وہ فریمز ڈراپ کریں؛ Swift کو بھی یہی رویہ نظر آنا چاہیے جب IOS7 کیو اسکافولڈنگ پہنچے (`docs/source/connect_architecture_strawman.md`)۔ کم از کم ایک reconnect ریکارڈ کرنے کے بعد چلائیں
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (یا `ConnectSessionDiagnostics` کی جانب سے واپس ہونے والی export directory پاس کریں) اور رینڈرڈ ٹیبل/JSON کو رن بک ٹکٹ کے ساتھ منسلک کریں۔ CLI وہی `state.json` / `metrics.ndjson` جوڑا پڑھتا ہے جو `ConnectQueueStateTracker` بناتا ہے، اس لئے گورننس ریویورز ڈرل شواہد کو بغیر bespoke tooling کے ٹریس کر سکتے ہیں۔

## 4. ٹیلی میٹری اور آبزرویبیلٹی

- **جمع کرنے کیلئے میٹرکس:**
  - `connect.queue_depth{direction}` gauge (پالیسی cap سے نیچے رہنا چاہیے)۔
  - `connect.queue_dropped_total{reason="overflow|ttl"}` counter (صرف fault injection کے دوران non-zero)۔
  - `connect.resume_latency_ms` histogram (reconnect فورس کرنے کے بعد p95 ریکارڈ کریں)۔
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Swift-specific exports `swift.connect.session_event` اور `swift.connect.frame_latency` (`docs/source/sdk/swift/telemetry_redaction.md`).
- **ڈیش بورڈز:** Connect بورڈ کے bookmarks کو annotation markers کے ساتھ اپ ڈیٹ کریں۔ کیپچرز (یا JSON exports) کو شواہد فولڈر میں شامل کریں اور ٹیلی میٹری exporter CLI سے حاصل کردہ OTLP/Prometheus snapshots بھی ساتھ رکھیں۔
- **Alerting:** اگر Sev 1/2 thresholds ٹرگر ہوں (`docs/source/android_support_playbook.md` سیکشن 5)، تو SDK Program Lead کو پیج کریں اور PagerDuty incident ID کو رن بک ٹکٹ میں درج کریں، پھر آگے بڑھیں۔

## 5. کلین اپ اور رول بیک

1. **اسٹیجڈ سیشنز حذف کریں۔** ہمیشہ پری ویو سیشنز حذف کریں تاکہ کیو ڈیپتھ الارمز بامعنی رہیں:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   صرف Swift ٹیسٹ رنز میں بھی یہی endpoint Rust/CLI helper سے کال کریں۔
2. **جرنلز صاف کریں۔** کسی بھی persisted queue journals (`ApplicationSupport/ConnectQueue/<sid>.to`, IndexedDB stores وغیرہ) کو حذف کریں تاکہ اگلا رن صاف شروع ہو۔ اگر replay مسئلہ ڈیبگ کرنا ہو تو حذف کرنے سے پہلے فائل ہیش ریکارڈ کریں۔
3. **انسیڈنٹ نوٹس فائل کریں۔** رن کا خلاصہ یہاں درج کریں:
   - `docs/source/status/swift_weekly_digest.md` (deltas بلاک),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (ٹیلی میٹری آنے کے بعد CR-2 کلئیر یا ڈاؤن گریڈ کریں),
   - JS SDK changelog یا recipe اگر نیا رویہ ویلیڈیٹ ہوا ہو۔
4. **فالٹ اسکیلیشن:**
   - بغیر injected faults کے کیو overflow => اس SDK پر بگ فائل کریں جس کی پالیسی Torii سے diverge کرتی ہے۔
   - ریزیوم errors => `connect.queue_depth` + `connect.resume_latency_ms` کے snapshots انسیڈنٹ رپورٹ کے ساتھ جوڑیں۔
   - گورننس mismatches (tokens ری یوز، TTL تجاوز) => SDK Program Lead کے ساتھ اسکیلیٹ کریں اور اگلی ریویژن میں `roadmap.md` نوٹ کریں۔

## 6. شواہد چیک لسٹ

| Artefact | لوکیشن |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Dashboard exports (`connect.queue_depth`, etc.) | `.../metrics/` subfolder |
| PagerDuty / incident IDs | `.../notes.md` |
| کلین اپ کنفرمیشن (Torii delete, journal wipe) | `.../cleanup.log` |

اس چیک لسٹ کی تکمیل IOS7/JS4 کیلئے "docs/runbooks updated" ایگزٹ معیار پورا کرتی ہے اور ہر Connect پری ویو سیشن کیلئے گورننس ریویورز کو ڈیٹرمنسٹک ٹریل فراہم کرتی ہے۔

</div>
