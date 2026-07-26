---
lang: mn
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2025-12-29T18:16:35.935471+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Холболтын алдааны ангилал зүй (Swift суурь)

Энэхүү тэмдэглэл нь IOS-CONNECT-010-г дагаж, хуваалцсан алдааны ангилал зүйг баримтжуулдаг
Nexus SDK холбоно. Свифт одоо каноник `ConnectError` боодолыг экспортолж байна.
Энэ нь холбогчтой холбоотой бүх алдааг зургаан ангиллын аль нэгэнд нь буулгадаг тул телеметр,
хяналтын самбар болон UX хуулбар нь платформ дээр зэрэгцсэн хэвээр байна.

> Сүүлд шинэчлэгдсэн: 2026-01-15  
> Эзэмшигч: Swift SDK-ийн ахлагч (тогтоомжийн менежер)  
> Статус: Swift + Android + JavaScript хэрэгжүүлэлтүүд ** газардсан**; дарааллыг нэгтгэх хүлээгдэж байна.

## Ангилал

| Ангилал | Зорилго | Ердийн эх сурвалжууд |
|----------|---------|-----------------|
| `transport` | Сешн дуусгавар болох WebSocket/HTTP тээвэрлэлтийн алдаа. | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | Фреймүүдийг кодлох/декодлох үед цуваа/гүүр алдаа. | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | Хэрэглэгч эсвэл операторыг засах шаардлагатай TLS/аттестатчлал/ бодлогын алдаа. | `URLError(.secureConnectionFailed)`, Torii 4xx хариулт |
| `timeout` | Сул зогсолт/офлайн хугацаа дуусах ба харуулууд (цэтгэл TTL, хүсэлт гаргах хугацаа). | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | FIFO нь буцах даралтын дохиог өгдөг бөгөөд ингэснээр програмууд ачааллыг хялбархан буулгах боломжтой. | `ConnectQueueError.overflow(limit:)` |
| `internal` | Бусад бүх зүйл: SDK буруу ашиглалт, дутуу Norito гүүр, эвдэрсэн сэтгүүл. | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

SDK бүр ангилал зүйд нийцсэн алдааны төрлийг нийтэлж, илрүүлдэг
бүтэцлэгдсэн телеметрийн шинж чанарууд: `category`, `code`, `fatal` болон нэмэлт
мета өгөгдөл (`http_status`, `underlying`).

## Шуурхай зураглал

Swift нь `ConnectError`, `ConnectErrorCategory` болон туслах протоколуудыг дараахад экспортлодог.
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. Бүх нийтийн холболтын алдаа
төрлүүд `ConnectErrorConvertible`-д нийцдэг тул програмууд `error.asConnectError()` руу залгах боломжтой
мөн үр дүнг телеметрийн/бүртгэлийн давхаргууд руу дамжуулна.| Шуурхай алдаа | Ангилал | Код | Тэмдэглэл |
|-------------|----------|------|-------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | Давхар `start()`-ийг заана; хөгжүүлэгчийн алдаа. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | Хаалтын дараа илгээх/хүлээн авах үед нэмэгддэг. |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket нь хоёртын хувилбарыг хүлээж байхад текстийн ачааллыг хүргэсэн. |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | Эсрэг тал урсгалыг гэнэт хаасан. |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | Аппликейшн нь тэгш хэмтэй товчлууруудыг тохируулахаа мартсан байна. |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito ачааллын шаардлагатай талбарууд дутуу байна. |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | Ирээдүйн ачааллыг хуучин SDK харж байна. |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito гүүр байхгүй эсвэл фрэймийн байтыг кодлох/тайлж чадсангүй. |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | Гүүр боломжгүй эсвэл түлхүүрийн урт таарахгүй байна. |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | Офлайн дарааллын урт нь тохируулсан хязгаараас хэтэрсэн байна. |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | `URLSessionWebSocketTask` гадаргуутай. |
| `URLError` TLS тохиолдол | `authorization` | `network.tls_failure` | ATS/TLS хэлэлцээрийн бүтэлгүйтэл. |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | SDK-ийн өөр газар JSON код тайлах/кодлох ажиллагаа амжилтгүй болсон; мессеж нь Swift декодчилогч контекстийг ашигладаг. |
| Бусад ямар ч `Error` | `internal` | `unknown_error` | Бүх зүйлд хүрэх баталгаатай; мессеж толин тусгал `LocalizedError`. |

Нэгжийн туршилтууд (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) түгжигдэнэ
зураглал нь ирээдүйн рефакторууд ангилал эсвэл кодыг чимээгүйхэн өөрчлөх боломжгүй юм.

### Хэрэглээний жишээ

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

## Телеметр ба хяналтын самбар

Swift SDK нь `ConnectError.telemetryAttributes(fatal:httpStatus:)`-г хангадаг
Энэ нь каноник шинж чанарын газрын зургийг буцаана. Экспортлогчид эдгээрийг дамжуулах ёстой
Нэмэлт нэмэлтүүдтэй `connect.error` OTEL арга хэмжээний шинж чанарууд:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

Хяналтын самбар нь `connect.error` тоолуурыг дарааллын гүнтэй (`connect.queue_depth`) уялдуулдаг.
мөн гистограммуудыг дахин холбож, логуудыг задлахгүйгээр регрессийг илрүүлэх.

## Android зураглалAndroid SDK нь `ConnectError`, `ConnectErrorCategory`, `ConnectErrorTelemetryOptions`,
`ConnectErrorOptions`, болон туслах хэрэгслүүдийн дагуу
`org.hyperledger.iroha.android.connect.error`. Барилгачин маягийн туслахууд нь дурын `Throwable` хувиргадаг
ангилал зүйд нийцсэн ачаалал болгон тээвэрлэх/TLS/кодекийн үл хамаарах зүйлээс ангиллыг гаргах,
болон детерминист телеметрийн шинж чанаруудыг ил гаргахын тулд OpenTelemetry/түүвэрлэлтийн стекүүд нь
захиалгагүйгээр үр дүн адаптерууд.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectError.jav a:8】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectErrors.java:25】
`ConnectQueueError` нь `ConnectErrorConvertible`-г аль хэдийн хэрэгжүүлсэн бөгөөд queueOverflow/цаг завсарлага үүсгэдэг.
халих/хугацаа дуусах нөхцлийн категориуд байдаг тул офлайн дарааллын хэрэгсэл нь ижил урсгал руу залгах боломжтой.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
Android SDK README нь ангилал зүйд хандсан бөгөөд тээвэрлэлтийн үл хамаарах зүйлсийг хэрхэн яаж боож болохыг харуулж байна
телеметрийг дамжуулахын өмнө dApp-н удирдамжийг Swift-ийн үндсэн шугамтай нийцүүлэн байлга.【java/iroha_android/README.md:167】

## JavaScript зураглал

Node.js/хөтчийн үйлчлүүлэгчид `ConnectError`, `ConnectQueueError`, `ConnectErrorCategory`, болон
`connectErrorFrom()` `@iroha/iroha-js`. Хуваалцсан туслагч нь HTTP төлөвийн кодыг шалгадаг.
Зангилааны алдааны код (сокет, TLS, завсарлага), `DOMException` нэр, кодлогчийн алдаа
ижил категори/кодууд энэ тэмдэглэлд бичигдсэн байдаг бол TypeScript тодорхойлолтууд нь телеметрийг загварчлах болно
шинж чанар нь дарагдсан тул багаж нь гараар дамжуулахгүйгээр OTEL үйл явдлуудыг ялгаруулж чаддаг.【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
SDK README нь ажлын явцыг баримтжуулж, энэ ангилал зүйд буцаж холбогддог тул програмын багууд боломжтой
багаж хэрэгслийн хэсгүүдийг үгчлэн хуулах.【javascript/iroha_js/README.md:1387】

## Дараагийн алхамууд (SDK хоорондын)

- **Дараалал интеграци:** офлайн дараалал илгээгдсэний дараа дараалал буулгах/унах логикийг баталгаажуулна уу.
  гадаргуу `ConnectQueueError` утгууд нь халих Telemetry найдвартай хэвээр байна.