---
lang: ar
direction: rtl
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b4dbba7711a733a9c2736410db29b035ce8f13bb50b532fe509a6492f239a1fe
source_last_modified: "2025-11-19T04:38:08.010772+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- ترجمة عربية لملف docs/runbooks/connect_session_preview_runbook.md -->

# دليل تشغيل معاينة جلسات Connect (IOS7 / JS4)

يوثق هذا الدليل الاجراء من البداية الى النهاية لتجهيز جلسات معاينة Connect والتحقق منها وانهائها كما تتطلبه معالم خارطة الطريق **IOS7** و **JS4** (`roadmap.md:1340`, `roadmap.md:1656`). اتبع هذه الخطوات كلما عرضت نموذج Connect الاولي (`docs/source/connect_architecture_strawman.md`)، او اختبرت نقاط الربط الخاصة بالطابور/التيليمترية الموعودة في خرائط SDK، او جمعت ادلة لـ `status.md`.

## 1. قائمة التحقق قبل التنفيذ

| البند | التفاصيل | المراجع |
|------|---------|------------|
| عنوان Torii + سياسة Connect | تاكد من عنوان Torii الاساسي و `chain_id` وسياسة Connect (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`). التقط لقطة JSON في تذكرة الدليل. | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| اصدارات fixture + bridge | سجل تجزئة fixture الخاصة بـ Norito وبناء bridge الذي ستستخدمه (Swift يتطلب `NoritoBridge.xcframework`، و JS يتطلب `@iroha/iroha-js` >= الاصدار الذي شحن `bootstrapConnectPreviewSession`). | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| لوحات التيليمترية | تاكد من توفر اللوحات التي ترسم `connect.queue_depth` و `connect.queue_overflow_total` و `connect.resume_latency_ms` و `swift.connect.session_event` وغيرها (لوحة Grafana `Android/Swift Connect` + لقطات Prometheus المصدرة). | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| مجلدات الادلة | اختر وجهة مثل `docs/source/status/swift_weekly_digest.md` (ملخص اسبوعي) و `docs/source/sdk/swift/connect_risk_tracker.md` (متعقب المخاطر). احفظ السجلات ولقطات المقاييس والتاكيدات تحت `docs/source/sdk/swift/readiness/archive/<date>/connect/`. | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. تهيئة جلسة المعاينة

1. **تحقق من السياسة والحصص.** نفذ:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   افشل التشغيل اذا اختلف `queue_max` او TTL عن الاعداد الذي خططت لاختباره.
2. **انشئ SID/URI حتمية.** مساعد `bootstrapConnectPreviewSession` في `@iroha/iroha-js` يربط توليد SID/URI بتسجيل الجلسة في Torii؛ استخدمه حتى عندما يقود Swift طبقة WebSocket.
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
   - اضبط `register: false` لاختبارات QR/deep-link بدون تسجيل.
   - احتفظ بـ `sidBase64Url` المعاد وروابط deeplink وكتلة `tokens` في مجلد الادلة؛ مراجعة الحوكمة تتوقع هذه القطع.
3. **وزع الاسرار.** شارك URI deeplink مع مشغل المحفظة (عينة dApp Swift، محفظة Android، او harness QA). لا تنسخ التوكنات الخام في الدردشة؛ استخدم الخزنة المشفرة الموثقة في حزمة التمكين.

## 3. تشغيل الجلسة

1. **افتح WebSocket.** يستخدم عملاء Swift عادة:
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
   راجع `docs/connect_swift_integration.md` لاعدادات اضافية (استيرادات bridge ومحولات التوازي).
2. **تدفقات الموافقة والتوقيع.** تستدعي dApps `ConnectSession.requestSignature(...)`، بينما ترد المحافظ عبر `approveSession` / `reject`. يجب ان يسجل كل قبول alias مجزأ + الصلاحيات ليتوافق مع ميثاق حوكمة Connect.
3. **اختبر مسارات الطابور والاستئناف.** بدل اتصال الشبكة او علق المحفظة للتأكد من ان الطابور المحدود وخطافات replay تسجل الاحداث. تصدر SDKs الخاصة بـ JS/Android الخطا `ConnectQueueError.overflow(limit)` / `.expired(ttlMs)` عند اسقاط الاطارات؛ يجب ان يلاحظ Swift الامر نفسه حالما يصل هيكل طابور IOS7 (`docs/source/connect_architecture_strawman.md`). بعد تسجيل اعادة اتصال واحدة على الاقل، شغل
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (او مرر دليل التصدير المعاد من `ConnectSessionDiagnostics`) وارفق الجدول/JSON المعروض في تذكرة الدليل. يقرأ CLI نفس الزوج `state.json` / `metrics.ndjson` الذي ينتجه `ConnectQueueStateTracker`، بحيث يتمكن مراجعو الحوكمة من تتبع ادلة التمرين دون ادوات مخصصة.

## 4. التيليمترية والمراقبة

- **مقاييس يجب التقاطها:**
  - `connect.queue_depth{direction}` gauge (يجب ان يبقى اقل من حد السياسة).
  - `connect.queue_dropped_total{reason="overflow|ttl"}` counter (غير صفري فقط اثناء حقن الاعطال).
  - `connect.resume_latency_ms` histogram (سجل p95 بعد فرض اعادة اتصال).
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - صادرات Swift `swift.connect.session_event` و `swift.connect.frame_latency` (`docs/source/sdk/swift/telemetry_redaction.md`).
- **لوحات المعلومات:** حدث علامات لوحة Connect مع مؤشرات توضيحية. ارفق لقطات (او JSON exports) في مجلد الادلة مع لقطات OTLP/Prometheus الملتقطة عبر CLI لمصدر التيليمترية.
- **التنبيه:** اذا تم تفعيل حدود Sev 1/2 (انظر `docs/source/android_support_playbook.md` القسم 5)، ناد SDK Program Lead ووثق معرف حادث PagerDuty في تذكرة الدليل قبل المتابعة.

## 5. التنظيف والتراجع

1. **احذف الجلسات الممرحلة.** احذف جلسات المعاينة دائما حتى تبقى تنبيهات عمق الطابور ذات معنى:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   لعمليات اختبار Swift فقط، استدع نفس النهاية عبر مساعد Rust/CLI.
2. **نظف السجلات.** ازل اي سجلات طابور محفوظة (`ApplicationSupport/ConnectQueue/<sid>.to`، مخازن IndexedDB، وغيرها) ليبدا التشغيل التالي نظيفا. سجل تجزئة الملف قبل الحذف اذا احتجت لتشخيص مشكلة replay.
3. **سجل ملاحظات الحادث.** لخص التشغيل في:
   - `docs/source/status/swift_weekly_digest.md` (كتلة الفروق),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (امسح او خفف CR-2 بعد اكتمال التيليمترية),
   - سجل تغييرات SDK JS او الوصفة اذا تم التحقق من سلوك جديد.
4. **تصعيد الاعطال:**
   - فيضان الطابور بدون اعطال محقونة => افتح بلاغا ضد SDK الذي تختلف سياسته عن Torii.
   - اخطاء الاستئناف => ارفق لقطات `connect.queue_depth` + `connect.resume_latency_ms` بتقرير الحادث.
   - عدم تطابق الحوكمة (اعادة استخدام التوكنات، تجاوز TTL) => صعد الامر مع SDK Program Lead واشر الى `roadmap.md` في المراجعة التالية.

## 6. قائمة تحقق الادلة

| الاثر | الموقع |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Dashboard exports (`connect.queue_depth`, etc.) | `.../metrics/` subfolder |
| PagerDuty / incident IDs | `.../notes.md` |
| تاكيد التنظيف (Torii delete, journal wipe) | `.../cleanup.log` |

اكمال هذه القائمة يحقق معيار الخروج "docs/runbooks updated" لـ IOS7/JS4 ويمنح مراجعي الحوكمة مسارا حتميا لكل جلسة معاينة Connect.

</div>
