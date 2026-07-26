---
lang: ar
direction: rtl
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2026-01-03T18:08:01.837162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ربط تصنيف الأخطاء (خط الأساس السريع)

تتتبع هذه المذكرة IOS-CONNECT-010 وتوثق تصنيف الأخطاء المشتركة لـ
Nexus قم بتوصيل مجموعات SDK. يقوم Swift الآن بتصدير غلاف `ConnectError` الأساسي،
الذي يعين جميع حالات الفشل المتعلقة بالاتصال إلى واحدة من ست فئات، لذا القياس عن بعد،
تظل لوحات المعلومات ونسخة UX متسقة عبر الأنظمة الأساسية.

> آخر تحديث: 15-01-2026  
> المالك: قائد Swift SDK (مشرف التصنيف)  
> الحالة: تطبيقات Swift + Android + JavaScript ** وصلت **؛ تكامل قائمة الانتظار معلق.

## الفئات

| الفئة | الغرض | المصادر النموذجية |
|----------|--------|-----------------|
| `transport` | فشل نقل WebSocket/HTTP الذي يؤدي إلى إنهاء الجلسة. | `URLError(.cannotConnectToHost)`، `ConnectClient.ClientError.closed` |
| `codec` | فشل التسلسل/الجسر أثناء تشفير/فك تشفير الإطارات. | `ConnectEnvelopeError.invalidPayload`، `DecodingError` |
| `authorization` | تتطلب حالات فشل TLS/الشهادة/السياسة معالجة المستخدم أو المشغل. | `URLError(.secureConnectionFailed)`، Torii استجابات 4xx |
| `timeout` | فترات انتهاء الصلاحية/غير المتصلة بالإنترنت والمراقبة (قائمة الانتظار TTL، مهلة الطلب). | `URLError(.timedOut)`، `ConnectQueueError.expired` |
| `queueOverflow` | إشارات الضغط الخلفي FIFO حتى تتمكن التطبيقات من التخلص من الأحمال بأمان. | `ConnectQueueError.overflow(limit:)` |
| `internal` | كل شيء آخر: سوء استخدام SDK، فقدان جسر Norito، المجلات التالفة. | `ConnectSessionError.missingDecryptionKeys`، `ConnectCryptoError.*` |

ينشر كل SDK نوع خطأ يتوافق مع التصنيف ويكشفه
سمات القياس عن بعد المنظمة: `category`، و`code`، و`fatal`، واختيارية
البيانات الوصفية (`http_status`، `underlying`).

## رسم الخرائط السريعة

تقوم Swift بتصدير `ConnectError` و`ConnectErrorCategory` والبروتوكولات المساعدة في
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. كل خطأ الاتصال العام
تتوافق الأنواع مع `ConnectErrorConvertible`، لذا يمكن للتطبيقات الاتصال بـ `error.asConnectError()`
وأرسل النتيجة إلى طبقات القياس عن بعد/التسجيل.| خطأ سريع | الفئة | الكود | ملاحظات |
|-------------|----------|------|-------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | يشير إلى `start()` المزدوج؛ خطأ المطور. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | يتم رفعه عند الإرسال/الاستلام بعد الإغلاق. |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | قام WebSocket بتسليم حمولة نصية أثناء توقع وجود ثنائي. |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | أغلق الطرف المقابل البث بشكل غير متوقع. |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | نسي التطبيق تكوين المفاتيح المتماثلة. |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | حمولة Norito تفتقد الحقول المطلوبة. |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | الحمولة المستقبلية التي شاهدتها SDK القديمة. |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | جسر Norito مفقود أو فشل في تشفير/فك تشفير بايتات الإطار. |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | الجسر غير متوفر أو أطوال المفاتيح غير متطابقة. |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | تجاوز طول قائمة الانتظار غير المتصلة الحد الذي تم تكوينه. |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | ظهرت على السطح بواسطة `URLSessionWebSocketTask`. |
| حالات `URLError` TLS | `authorization` | `network.tls_failure` | فشل مفاوضات ATS/TLS. |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | فشل فك/تشفير JSON في مكان آخر في SDK؛ تستخدم الرسالة سياق وحدة فك ترميز Swift. |
| أي `Error` أخرى | `internal` | `unknown_error` | ضمان شامل؛ مرايا الرسائل `LocalizedError`. |

تم إغلاق اختبارات الوحدة (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`).
التعيين حتى لا تتمكن شركات إعادة البناء المستقبلية من تغيير الفئات أو الرموز بصمت.

### مثال للاستخدام

```swift
do {
    try await session.nextEnvelope()
} catch {
    let connectError = error.asConnectError()
    telemetry.emit(event: "connect.error",
                   attributes: connectError.telemetryAttributes(fatal: true))
    logger.error("Connect failure: \(connectError.code) – \(connectError.message)")
}
```

## القياس عن بعد ولوحات المعلومات

يوفر Swift SDK `ConnectError.telemetryAttributes(fatal:httpStatus:)`
الذي يُرجع خريطة السمات الأساسية. يجب على المصدرين إعادة توجيه هذه
السمات في أحداث `connect.error` OTEL مع إضافات اختيارية:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

تربط لوحات المعلومات عدادات `connect.error` بعمق قائمة الانتظار (`connect.queue_depth`)
وأعد توصيل الرسوم البيانية لاكتشاف التراجعات دون وجود سجلات لاستكشاف الكهوف.

## رسم خرائط أندرويديقوم Android SDK بتصدير `ConnectError`، و`ConnectErrorCategory`، و`ConnectErrorTelemetryOptions`،
`ConnectErrorOptions`، والأدوات المساعدة المساعدة ضمن
`org.hyperledger.iroha.android.connect.error`. يقوم المساعدون على نمط المنشئ بتحويل أي `Throwable`
في حمولة متوافقة مع التصنيف، واستنتج الفئات من استثناءات النقل/TLS/الترميز،
وكشف سمات القياس عن بعد الحتمية حتى تتمكن مكدسات OpenTelemetry/sampling من استهلاك
النتيجة بدون محولات مخصصة. 【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectError.java:8】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectErrors.java:25】
يقوم `ConnectQueueError` بالفعل بتنفيذ `ConnectErrorConvertible`، مما يؤدي إلى إصدار قائمة الانتظار/المهلة
فئات لظروف التجاوز/انتهاء الصلاحية بحيث يمكن توصيل أدوات قائمة الانتظار غير المتصلة بالإنترنت في نفس التدفق.
يشير الآن ملف Android SDK README إلى التصنيف ويوضح كيفية تغليف استثناءات النقل
قبل إرسال القياس عن بعد، مع الحفاظ على محاذاة توجيهات dApp مع خط الأساس لـ Swift.[java/iroha_android/README.md:167]

## رسم خرائط جافا سكريبت

يقوم عملاء Node.js/browser باستيراد `ConnectError`، و`ConnectQueueError`، و`ConnectErrorCategory`، و
`connectErrorFrom()` من `@iroha/iroha-js`. يقوم المساعد المشترك بفحص رموز حالة HTTP،
رموز خطأ العقدة (المقبس، TLS، المهلة)، وأسماء `DOMException`، وفشل برنامج الترميز في إصدار
نفس الفئات/الرموز الموثقة في هذه المذكرة، في حين أن تعريفات TypeScript تمثل القياس عن بعد
يتم تجاوز السمة بحيث يمكن للأدوات إصدار أحداث OTEL دون الإرسال اليدوي.
يقوم ملف SDK README بتوثيق سير العمل والروابط التي تعود إلى هذا التصنيف حتى تتمكن فرق التطبيق من القيام بذلك
انسخ مقتطفات الأجهزة حرفيًا. 【javascript/iroha_js/README.md:1387】

## الخطوات التالية (عبر SDK)

- **تكامل قائمة الانتظار:** بمجرد شحن قائمة الانتظار غير المتصلة، تأكد من منطق إزالة قائمة الانتظار/الإفلات
  الأسطح تحمل قيم `ConnectQueueError` لذا يظل قياس الفائض عن بعد جديرًا بالثقة.