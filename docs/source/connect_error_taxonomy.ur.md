---
lang: ur
direction: rtl
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2026-01-03T18:08:01.837162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# رابطہ غلطی درجہ بندی (سوئفٹ بیس لائن)

یہ نوٹ iOS کنیکٹ -010 کو ٹریک کرتا ہے اور مشترکہ غلطی کی درجہ بندی کے لئے دستاویز کرتا ہے
Nexus SDKs سے رابطہ کریں۔ سوئفٹ اب کیننیکل `ConnectError` ریپر برآمد کرتا ہے ،
کون سے رابطے سے متعلق تمام ناکامیوں کا نقشہ چھ قسموں میں سے کسی ایک میں نقشہ بناتا ہے ، لہذا ٹیلی میٹری ،
ڈیش بورڈز ، اور UX کاپی اسٹیل پلیٹ فارمز میں منسلک ہیں۔

> آخری تازہ کاری: 2026-01-15  
> مالک: سوئفٹ ایس ڈی کے لیڈ (ٹیکسنومی اسٹیورڈ)  
> حیثیت: سوئفٹ + Android + جاوا اسکرپٹ کے نفاذ ** لینڈڈ ** ؛ قطار انضمام زیر التوا ہے۔

## زمرے

| زمرہ | مقصد | عام ذرائع |
| ---------- | --------- | ----------------- |
| `transport` | ویب ساکٹ/ایچ ٹی ٹی پی ٹرانسپورٹ کی ناکامی جو سیشن کو ختم کرتی ہے۔ | `URLError(.cannotConnectToHost)` ، `ConnectClient.ClientError.closed` |
| `codec` | انکوڈنگ/ڈیکوڈنگ فریموں کے دوران سیریلائزیشن/پل کی ناکامی۔ | `ConnectEnvelopeError.invalidPayload` ، `DecodingError` |
| `authorization` | TLS/تصدیق/پالیسی کی ناکامیوں میں صارف یا آپریٹر کے تدارک کی ضرورت ہوتی ہے۔ | `URLError(.secureConnectionFailed)` ، Torii 4xx جوابات |
| `timeout` | بیکار/آف لائن میعاد ختم ہونے اور واچ ڈاگس (قطار ٹی ٹی ایل ، درخواست کا ٹائم آؤٹ)۔ | `URLError(.timedOut)` ، `ConnectQueueError.expired` |
| `queueOverflow` | FIFO بیک پریشر سگنل تاکہ ایپس خوبصورتی سے لوڈ کرسکیں۔ | `ConnectQueueError.overflow(limit:)` |
| `internal` | باقی سب کچھ: SDK غلط استعمال ، Norito برج سے محروم ، بدعنوان جرائد۔ | `ConnectSessionError.missingDecryptionKeys` ، `ConnectCryptoError.*` |

ہر ایس ڈی کے ایک غلطی کی قسم شائع کرتا ہے جو درجہ بندی کے مطابق ہوتا ہے اور بے نقاب ہوتا ہے
ساختہ ٹیلی میٹری کی صفات: `category` ، `code` ، `fatal` ، اور اختیاری
میٹا ڈیٹا (`http_status` ، `underlying`)۔

## تیز میپنگ

تیز برآمدات `ConnectError` ، `ConnectErrorCategory` ، اور مددگار پروٹوکول IN
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`۔ تمام عوامی رابطہ کی غلطی
اقسام `ConnectErrorConvertible` کے مطابق ہیں ، لہذا ایپس `error.asConnectError()` پر کال کرسکتی ہیں
اور نتیجہ ٹیلی میٹری/لاگنگ پرتوں کو آگے بھیج دیں۔| تیز غلطی | زمرہ | کوڈ | نوٹ |
| ------------- | ---------- | ------ | ------- |
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | ڈبل `start()` کی نشاندہی کرتا ہے۔ ڈویلپر کی غلطی. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | قریب بھیجنے/وصول کرتے وقت اٹھایا گیا۔ |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | بائنری کی توقع کرتے ہوئے ویب ساکٹ نے متنی پے لوڈ کی فراہمی کی۔ |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | ہم منصب نے غیر متوقع طور پر ندی کو بند کردیا۔ |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | درخواست توازن کی چابیاں تشکیل دینا بھول گئی۔ |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito پے لوڈ مطلوبہ فیلڈز سے محروم ہے۔ |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | اولڈ ایس ڈی کے کے ذریعہ مستقبل میں پے لوڈ دیکھا جاتا ہے۔ |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito پل غائب یا فریم بائٹس کو انکوڈ/ڈیکوڈ کرنے میں ناکام۔ |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | پل دستیاب نہیں یا مماثل کلیدی لمبائی۔ |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | آف لائن قطار کی لمبائی تشکیل شدہ حد سے تجاوز کر گئی۔ |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | `URLSessionWebSocketTask` کے ذریعہ منظر عام پر آیا۔ |
| `URLError` TLS کیسز | `authorization` | `network.tls_failure` | اے ٹی ایس/ٹی ایل ایس مذاکرات کی ناکامی۔ |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | JSON Dedoding/انکوڈنگ SDK میں کہیں اور ناکام ہوگئی۔ پیغام سوئفٹ ڈیکوڈر سیاق و سباق کا استعمال کرتا ہے۔ |
| کوئی دوسرا `Error` | `internal` | `unknown_error` | گارنٹیڈ کیچ آل ؛ پیغام آئینہ `LocalizedError`۔ |

یونٹ ٹیسٹ (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) لاک ڈاؤن
نقشہ سازی کرنے کے لئے مستقبل کے ریفیکٹرز خاموشی سے زمرے یا کوڈز کو تبدیل نہیں کرسکتے ہیں۔

### مثال کے استعمال

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

## ٹیلی میٹری اور ڈیش بورڈز

سوئفٹ SDK `ConnectError.telemetryAttributes(fatal:httpStatus:)` فراہم کرتا ہے
جو کیننیکل وصف کا نقشہ لوٹاتا ہے۔ برآمد کنندگان کو ان کو آگے بھیجنا چاہئے
اختیاری ایکسٹرا کے ساتھ `connect.error` OTEL واقعات میں اوصاف:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

ڈیش بورڈز `connect.error` کاؤنٹرز کو قطار کی گہرائی (`connect.queue_depth`) کے ساتھ منسلک کریں
اور ہسٹگرامس کو دوبارہ منسلک کریں تاکہ بغیر کسی لاگ ان لاگوں کے رجعتوں کا پتہ لگائیں۔

## android میپنگAndroid SDK برآمد `ConnectError` ، `ConnectErrorCategory` ، `ConnectErrorTelemetryOptions` ،
`ConnectErrorOptions` ، اور مددگار افادیت کے تحت
`org.hyperledger.iroha.android.connect.error`۔ بلڈر طرز کے مددگار کسی بھی `Throwable` کو تبدیل کرتے ہیں
ایک درجہ بندی کے مطابق پے لوڈ میں ، ٹرانسپورٹ/ٹی ایل ایس/کوڈیک مستثنیات سے متعلقہ زمرے ،
اور ڈٹرمینسٹک ٹیلی میٹری کی صفات کو بے نقاب کریں تاکہ اوپن لیمٹری/نمونے لینے والے اسٹیک استعمال کرسکتے ہیں
بیسپوک اڈاپٹر کے بغیر نتیجہ
`ConnectQueueError` پہلے ہی `ConnectErrorConvertible` کو نافذ کرتا ہے ، قطار اوور فلو/ٹائم آؤٹ کو خارج کرتے ہوئے
اوور فلو/میعاد ختم ہونے والی شرائط کے لئے زمرے لہذا آف لائن قطار کا آلہ اسی بہاؤ میں تار لگا سکتا ہے۔ 【جاوا/اروہہ_ندروڈ/ایس آر سی/مین/جاوا/آرگ/ہائپرلیجر/آراوہا/اینڈروئیڈ/کنیکٹ/کنیکٹ/غلطی/کنیکٹ کیوئیرر.جوا: 5】
Android SDK Readme اب درجہ بندی کا حوالہ دیتا ہے اور یہ ظاہر کرتا ہے کہ ٹرانسپورٹ سے مستثنیات کو کیسے سمیٹ لیا جائے
ٹیلی میٹری کا اخراج کرنے سے پہلے ، ڈی اے پی پی کی رہنمائی کو سوئفٹ بیس لائن کے ساتھ جوڑتے ہوئے۔

## جاوا اسکرپٹ میپنگ

node.js/براؤزر کلائنٹ `ConnectError` ، `ConnectQueueError` ، `ConnectErrorCategory` ، اور
`connectErrorFrom()` `@iroha/iroha-js` سے۔ مشترکہ مددگار HTTP اسٹیٹس کوڈز کا معائنہ کرتا ہے ،
نوڈ ایرر کوڈز (ساکٹ ، ٹی ایل ایس ، ٹائم آؤٹ) ، `DOMException` نام ، اور کوڈیک کی ناکامیوں کو خارج کرنے کے لئے
اس نوٹ میں ایک ہی زمرے/کوڈ دستاویزی دستاویزات ، جبکہ ٹائپ اسکرپٹ کی تعریفیں ٹیلی میٹری کا ماڈل بناتی ہیں
وصف اوور رائڈس تو ٹولنگ دستی کاسٹنگ کے بغیر اوٹیل واقعات کا اخراج کرسکتا ہے۔
ایس ڈی کے ریڈم ورک فلو کی دستاویز کرتا ہے اور اس درجہ بندی سے لنک کرتا ہے تاکہ درخواست ٹیمیں کر سکیں
آلے کے ٹکڑوں کو زبانی کاپی کریں۔

## اگلے اقدامات (کراس ایس ڈی کے)

- ** قطار انضمام: ** ایک بار آف لائن قطار بحری جہاز ، یقینی بنائیں کہ ڈی کیو/ڈراپ منطق کو یقینی بنائیں
  سطحیں `ConnectQueueError` اقدار لہذا اوور فلو ٹیلی میٹری قابل اعتماد ہے۔