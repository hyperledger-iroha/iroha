---
lang: ar
direction: rtl
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-11-18T04:13:57.609769+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- ترجمة عربية لملف docs/runbooks/connect_chaos_plan.md -->

# خطة بروفة الفوضى والاعطال لـ Connect (IOS3 / IOS7)

تحدد هذه الخطة تدريبات فوضى قابلة للتكرار تفي باجراء خارطة الطريق _"plan joint chaos rehearsal"_ (`roadmap.md:1527`). اربطها مع رن بك معاينة Connect (`docs/runbooks/connect_session_preview_runbook.md`) عند تنفيذ عروض cross-SDK.

## الاهداف ومعايير النجاح
- تمرين سياسة retry/back-off المشتركة لـ Connect وحدود طوابير الاوفلاين ومصدري التيليمترية تحت اعطال مضبوطة دون تعديل كود الانتاج.
- التقاط ادلة حتمية (مخرجات `iroha connect queue inspect`، لقطات متريكس `connect.*`، سجلات SDK Swift/Android/JS) حتى تتمكن الحوكمة من تدقيق كل تدريب.
- اثبات ان المحافظ و dApps تحترم تغييرات الاعداد (انجراف manifest، تدوير الملح، فشل attestation) عبر اظهار فئة `ConnectError` القياسية واحداث تيليمترية آمنة للردكشن.

## المتطلبات
1. **تهيئة البيئة**
   - شغل مكدس Torii التجريبي: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - شغل عينة SDK واحدة على الاقل (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`).
2. **التهيئة القياسية**
   - فعل تشخيصات SDK (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     `ConnectSessionDiagnostics` في Swift؛ مكافئات `ConnectQueueJournal` + `ConnectQueueJournalTests`
     في Android/JS).
   - تاكد ان CLI `iroha connect queue inspect --sid <sid> --metrics` يحل مسار الطابور
     الذي ينتجه SDK (`~/.iroha/connect/<sid>/state.json` و `metrics.ndjson`).
   - اربط مصدري التيليمترية لتكون السلاسل التالية مرئية في Grafana
     وعبر `scripts/swift_status_export.py telemetry`: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **مجلدات الادلة** - انشئ `artifacts/connect-chaos/<date>/` واحفظ:
   - سجلات خام (`*.log`)، لقطات متريكس (`*.json`)، صادرات لوحة
     (`*.png`)، مخرجات CLI و PagerDuty IDs.

## مصفوفة السيناريوهات

| ID | العطل | خطوات الحقن | الاشارات المتوقعة | الادلة |
|----|-------|-------------|------------------|--------|
| C1 | انقطاع WebSocket واعادة اتصال | ضع `/v2/connect/ws` خلف proxy (مثل `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) او احجب الخدمة مؤقتا (`kubectl scale deploy/torii --replicas=0` لمدة <=60 s). اجبر المحفظة على الاستمرار في ارسال الاطارات لتمتلئ طوابير الاوفلاين. | يزيد `connect.reconnects_total`، ويقفز `connect.resume_latency_ms` لكنه يبقى <1 s p95، وتدخل الطوابير `state=Draining` عبر `ConnectQueueStateTracker`. تصدر SDKs `ConnectError.Transport.reconnecting` مرة واحدة ثم تستعيد الاتصال. | - مخرجات `iroha connect queue inspect --sid <sid>` تظهر `resume_attempts_total` غير صفري.<br>- وسم لوحة للنافذة الزمنية للانقطاع.<br>- مقتطف سجل برسائل reconnect + drain. |
| C2 | امتلاء طابور اوفلاين / انتهاء TTL | عدل العينة لتقليص حدود الطابور (Swift: انشئ `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` داخل `ConnectSessionDiagnostics`; Android/JS تستخدم منشئات مكافئة). علق المحفظة لمدة >=2x `retentionInterval` بينما تستمر dApp في الارسال. | ترتفع `connect.queue_dropped_total{reason="overflow"}` و `{reason="ttl"}`، ويثبت `connect.queue_depth` عند الحد الجديد، وتظهر SDKs `ConnectError.QueueOverflow(limit: 4)` (او `.QueueExpired`). يظهر `iroha connect queue inspect` حالة `state=Overflow` مع علامات `warn/drop` عند 100%. | - لقطة لعدادات المتريكس.<br>- JSON من CLI يوضح الامتلاء.<br>- سجل Swift/Android يحوي سطر `ConnectError`. |
| C3 | انجراف manifest / رفض admission | عبث بملف manifest المقدم للمحافظ (مثلا عدل manifest النموذجي `docs/connect_swift_ios.md` او شغل Torii مع `--connect-manifest-path` يشير الى نسخة تختلف فيها `chain_id` او `permissions`). اطلب موافقة من dApp وتاكد ان المحفظة ترفض حسب السياسة. | يعيد Torii `HTTP 409` لـ `/v2/connect/session` مع `manifest_mismatch`، وتصدر SDKs `ConnectError.Authorization.manifestMismatch(manifestVersion)`، وترفع التيليمترية `connect.manifest_mismatch_total`، وتبقى الطوابير فارغة (`state=Idle`). | - مقتطف سجل Torii يظهر كشف عدم التطابق.<br>- لقطة SDK للخطا المعروض.<br>- لقطة متريكس تثبت عدم وجود frames في الطابور. |
| C4 | تدوير مفتاح / قفزة نسخة الملح | دوّر الملح او مفتاح AEAD لـ Connect اثناء الجلسة. في بيئات dev، اعد تشغيل Torii مع `CONNECT_SALT_VERSION=$((old+1))` (يماثل اختبار ملح Android في `docs/source/sdk/android/telemetry_schema_diff.md`). ابق المحفظة اوفلاين حتى تكتمل عملية التدوير ثم استأنف. | يفشل الاستئناف الاول مع `ConnectError.Authorization.invalidSalt`، وتفرغ الطوابير (تسقط dApp الاطارات المخزنة بسبب `salt_version_mismatch`)، وتصدر التيليمترية `android.telemetry.redaction.salt_version` (Android) و `swift.connect.session_event{event="salt_rotation"}`. تنجح الجلسة الثانية بعد تحديث SID. | - وسم لوحة يظهر حقبة الملح قبل/بعد.<br>- سجلات بخطا invalid-salt والنجاح اللاحق.<br>- مخرجات `iroha connect queue inspect` تظهر `state=Stalled` ثم `state=Active`. |
| C5 | فشل attestation / StrongBox | في محافظ Android، اضبط `ConnectApproval` ليشمل `attachments[]` + attestation StrongBox. استخدم harness الخاص بـ attestation (`scripts/android_keystore_attestation.sh` مع `--inject-failure strongbox-simulated`) او عدل JSON قبل تسليمه لـ dApp. | ترفض dApp الموافقة بـ `ConnectError.Authorization.invalidAttestation`، ويسجل Torii سبب الفشل، وترفع exporters `connect.attestation_failed_total`، ويزيل الطابور الادخال السيء. تقوم dApps Swift/JS بتسجيل الخطا مع بقاء الجلسة نشطة. | - سجل harness يحوي معرف الفشل المحقون.<br>- سجل خطا SDK + لقطة عداد التيليمترية.<br>- دليل ان الطابور ازال الاطار السيء (`recordsRemoved > 0`). |

## تفاصيل السيناريوهات

### C1 - انقطاع WebSocket واعادة اتصال
1. ضع Torii خلف proxy (toxiproxy, Envoy, او `kubectl port-forward`) حتى تتمكن من
   تبديل الاتاحة دون قتل العقدة بالكامل.
2. نفذ انقطاعا لمدة 45 s:
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. راقب لوحات التيليمترية و `scripts/swift_status_export.py telemetry
   --json-out artifacts/connect-chaos/<date>/c1_metrics.json`.
4. اسحب حالة الطابور مباشرة بعد الانقطاع:
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. النجاح = محاولة اعادة اتصال واحدة، نمو محدود للطابور، وتصريف تلقائي بعد عودة proxy.

### C2 - امتلاء طابور اوفلاين / انتهاء TTL
1. خفض عتبات الطابور في البناءات المحلية:
   - Swift: حدث منشئ `ConnectQueueJournal` داخل العينة
     (مثل `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     لتمرير `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`.
   - Android/JS: مرر الاعداد المكافئ عند انشاء `ConnectQueueJournal`.
2. علق المحفظة (خلفية المحاكي او وضع الطيران) لمدة >=60 s
   بينما تصدر dApp نداءات `ConnectClient.requestSignature(...)`.
3. استخدم `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) او مساعد
   تشخيص JS لتصدير حزمة الادلة (`state.json`, `journal/*.to`, `metrics.ndjson`).
4. النجاح = ارتفاع عدادات overflow، تعرض SDK `ConnectError.QueueOverflow` مرة واحدة،
   وتعافي الطابور بعد استئناف المحفظة.

### C3 - انجراف manifest / رفض admission
1. انسخ manifest الخاص بالقبول، مثلا:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. شغل Torii مع `--connect-manifest-path /tmp/manifest_drift.json` (او
   حدث اعداد docker compose/k8s للتمرين).
3. حاول بدء جلسة من المحفظة وتوقع HTTP 409.
4. التقط سجلات Torii + SDK و `connect.manifest_mismatch_total` من لوحة التيليمترية.
5. النجاح = رفض دون نمو للطابور، وتعرض المحفظة خطا التصنيف المشترك
   (`ConnectError.Authorization.manifestMismatch`).

### C4 - تدوير مفتاح / قفزة الملح
1. سجل نسخة الملح الحالية من التيليمترية:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. اعد تشغيل Torii بملح جديد (`CONNECT_SALT_VERSION=$((OLD+1))` او حدث
   config map). ابق المحفظة اوفلاين حتى يكتمل التشغيل.
3. استأنف المحفظة؛ يجب ان تفشل المحاولة الاولى بخطا invalid-salt
   وترتفع `connect.queue_dropped_total{reason="salt_version_mismatch"}`.
4. اجبر التطبيق على حذف الاطارات المخزنة بحذف دليل الجلسة
   (`rm -rf ~/.iroha/connect/<sid>` او تنظيف الكاش حسب المنصة)، ثم
   اعد الجلسة بتوكنات جديدة.
5. النجاح = تظهر التيليمترية قفزة الملح، ويسجل حدث invalid-resume مرة واحدة،
   والجلسة التالية تنجح دون تدخل يدوي.

### C5 - فشل attestation / StrongBox
1. انشئ حزمة attestation باستخدام `scripts/android_keystore_attestation.sh`
   (استخدم `--inject-failure strongbox-simulated` لقلب التوقيع).
2. اجعل المحفظة تلحق هذه الحزمة عبر API `ConnectApproval`; يجب على dApp
   التحقق ورفض الـ payload.
3. تحقق من التيليمترية (`connect.attestation_failed_total`, مقاييس الحوادث
   Swift/Android) وتاكد من ان الطابور ازال الادخال المسموم.
4. النجاح = الرفض معزول للموافقة السيئة، الطوابير تبقى سليمة،
   وسجل attestation محفوظ مع ادلة التمرين.

## قائمة ادلة الاثبات
- صادرات `artifacts/connect-chaos/<date>/c*_metrics.json` من
  `scripts/swift_status_export.py telemetry`.
- مخرجات CLI (`c*_queue.txt`) من `iroha connect queue inspect`.
- سجلات SDK + Torii مع الطوابع الزمنية وهاشات SID.
- لقطات لوحة مع تعليقات لكل سيناريو.
- PagerDuty / incident IDs اذا تم اطلاق تنبيهات Sev 1/2.

اكمال المصفوفة مرة كل ربع سنة يفي ببوابة خارطة الطريق ويظهر ان تطبيقات
Swift/Android/JS لـ Connect تستجيب بحتمية عبر اعلى اوضاع الاعطال خطورة.

</div>
