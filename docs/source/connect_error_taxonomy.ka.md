---
lang: ka
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2025-12-29T18:16:35.935471+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# დაკავშირების შეცდომის ტაქსონომია (Swift საბაზისო ხაზი)

ეს ჩანაწერი თვალყურს ადევნებს IOS-CONNECT-010-ს და ადასტურებს საერთო შეცდომების ტაქსონომიას
Nexus დააკავშირეთ SDK-ები. Swift ახლა ახორციელებს კანონიკური `ConnectError` შეფუთვის ექსპორტს,
რომელიც ასახავს Connect-თან დაკავშირებულ ყველა წარუმატებლობას ექვსი კატეგორიიდან ერთ-ერთზე, ანუ ტელემეტრია,
დაფები და UX ასლი გასწორებულია პლატფორმებზე.

> ბოლო განახლება: 2026-01-15  
> მფლობელი: Swift SDK წამყვანი (ტაქსონომიის მმართველი)  
> სტატუსი: Swift + Android + JavaScript განხორციელებები ** landed **; რიგის ინტეგრაცია მოლოდინშია.

## კატეგორიები

| კატეგორია | დანიშნულება | ტიპიური წყაროები |
|----------|---------|-----------------|
| `transport` | WebSocket/HTTP ტრანსპორტის წარუმატებლობა, რომელიც წყვეტს სესიას. | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | სერიულიზაცია/ხიდის ჩავარდნები ჩარჩოების კოდირების/გაშიფვრისას. | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | TLS/ატესტაციის/პოლიტიკის წარუმატებლობები, რომლებიც საჭიროებენ მომხმარებლის ან ოპერატორის გამოსწორებას. | `URLError(.secureConnectionFailed)`, Torii 4xx პასუხები |
| `timeout` | უმოქმედო/ხაზგარეშე ვადის გასვლები და დამკვირვებლები (რიგები TTL, მოთხოვნის ვადა). | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | FIFO უკანა წნევის სიგნალებს აძლევს, რათა აპებმა მოხდენილად დაკარგონ დატვირთვა. | `ConnectQueueError.overflow(limit:)` |
| `internal` | ყველაფერი დანარჩენი: SDK ბოროტად გამოყენება, დაკარგული Norito ხიდი, დაზიანებული ჟურნალები. | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

ყველა SDK აქვეყნებს შეცდომის ტიპს, რომელიც შეესაბამება ტაქსონომიას და ავლენს
სტრუქტურირებული ტელემეტრიის ატრიბუტები: `category`, `code`, `fatal` და სურვილისამებრ
მეტამონაცემები (`http_status`, `underlying`).

## სწრაფი რუქა

Swift ახორციელებს `ConnectError`, `ConnectErrorCategory` და დამხმარე პროტოკოლების ექსპორტს
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. ყველა საჯარო დაკავშირების შეცდომა
ტიპები შეესაბამება `ConnectErrorConvertible`-ს, ამიტომ აპებს შეუძლიათ დარეკონ `error.asConnectError()`
და გადაიტანეთ შედეგი ტელემეტრიის/ლოგინგის ფენებზე.| Swift შეცდომა | კატეგორია | კოდი | შენიშვნები |
|-------------|----------|------|-------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | მიუთითებს ორმაგ `start()`; დეველოპერის შეცდომა. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | გაიზარდა გაგზავნის/მიღების დახურვის შემდეგ. |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket-მა გადასცა ტექსტური დატვირთვა ბინარის მოლოდინში. |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | კონტრაგენტმა მოულოდნელად დახურა ნაკადი. |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | აპლიკაციას დაავიწყდა სიმეტრიული კლავიშების კონფიგურაცია. |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito ტვირთამწეობა აკლია საჭირო ველებს. |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | მომავალი დატვირთვა ჩანს ძველი SDK-ით. |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito ხიდი აკლია ან ვერ მოხერხდა ჩარჩო ბაიტების კოდირება/გაშიფვრა. |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | ხიდი მიუწვდომელია ან შეუსაბამოა გასაღების სიგრძე. |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | ხაზგარეშე რიგის სიგრძემ გადააჭარბა კონფიგურირებულ ზღვარს. |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | ზედაპირული `URLSessionWebSocketTask`. |
| `URLError` TLS ქეისები | `authorization` | `network.tls_failure` | ATS/TLS მოლაპარაკების წარუმატებლობა. |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | JSON-ის გაშიფვრა/დაშიფვრა ვერ მოხერხდა SDK-ის სხვაგან; შეტყობინება იყენებს Swift დეკოდერის კონტექსტს. |
| ნებისმიერი სხვა `Error` | `internal` | `unknown_error` | გარანტირებული დაჭერა; შეტყობინების სარკეები `LocalizedError`. |

ერთეულის ტესტები (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) ჩაკეტილია
რუკების შედგენა, ასე რომ მომავალ რეფაქტორებს არ შეუძლიათ ჩუმად შეცვალონ კატეგორიები ან კოდები.

### გამოყენების მაგალითი

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

## ტელემეტრია და დაფები

Swift SDK უზრუნველყოფს `ConnectError.telemetryAttributes(fatal:httpStatus:)`
რომელიც აბრუნებს კანონიკური ატრიბუტის რუკას. ექსპორტიორებმა ეს უნდა გაავრცელონ
ატრიბუტებს `connect.error` OTEL მოვლენებში არჩევითი დამატებებით:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

საინფორმაციო დაფები აკავშირებს `connect.error` მრიცხველებს რიგის სიღრმესთან (`connect.queue_depth`)
და ხელახლა დააკავშირეთ ჰისტოგრამები, რათა აღმოაჩინონ რეგრესია სპელუნგური ჟურნალების გარეშე.

## ანდროიდის რუქაAndroid SDK ექსპორტს ახორციელებს `ConnectError`, `ConnectErrorCategory`, `ConnectErrorTelemetryOptions`,
`ConnectErrorOptions` და დამხმარე კომუნალური საშუალებების ქვეშ
`org.hyperledger.iroha.android.connect.error`. აღმაშენებლის სტილის დამხმარეები გარდაქმნის ნებისმიერ `Throwable`-ს
ტაქსონომიის შესაბამის დატვირთვაში, გამოიტანეთ კატეგორიები ტრანსპორტიდან/TLS/კოდეკის გამონაკლისებიდან,
და გამოავლინოს დეტერმინისტული ტელემეტრიის ატრიბუტები ისე, რომ OpenTelemetry/სიმპლინგმა დასტამ მოიხმაროს
შედეგი შეკვეთის გარეშე ადაპტერები.
`ConnectQueueError` უკვე ახორციელებს `ConnectErrorConvertible`-ს, ასხივებს რიგის Overflow/timeout
კატეგორიები გადადინების/ვადის გასვლის პირობებისთვის, ასე რომ, ხაზგარეშე რიგის ინსტრუმენტაციას შეუძლია იმავე ნაკადში ჩაერთოს.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
Android SDK README ახლა მიუთითებს ტაქსონომიაზე და გვიჩვენებს, თუ როგორ უნდა გადაიტანოთ ტრანსპორტის გამონაკლისები
ტელემეტრიის გამოშვებამდე, შეინარჩუნეთ dApp-ის მითითებები Swift-ის საბაზისო ხაზთან შესაბამისობაში.【java/iroha_android/README.md:167】

## JavaScript რუკა

Node.js/ბრაუზერის კლიენტების იმპორტი `ConnectError`, `ConnectQueueError`, `ConnectErrorCategory` და
`connectErrorFrom()` `@iroha/iroha-js`-დან. საერთო დამხმარე ამოწმებს HTTP სტატუსის კოდებს,
კვანძის შეცდომის კოდები (სოკეტი, TLS, დროის ამოწურვა), `DOMException` სახელები და კოდეკის გაუმართაობა
იგივე კატეგორიები/კოდები დოკუმენტირებულია ამ შენიშვნაში, ხოლო TypeScript-ის განმარტებები ტელემეტრიის მოდელირებას ახდენს
ატრიბუტი გადაფარავს, ასე რომ ინსტრუმენტმა შეიძლება გამოაქვეყნოს OTEL მოვლენები ხელით გადაცემის გარეშე.【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
SDK README დოკუმენტაციას უწევს სამუშაო პროცესს და აკავშირებს ამ ტაქსონომიას, რათა აპლიკაციის გუნდებმა შეძლონ
დააკოპირეთ ინსტრუმენტის ფრაგმენტები სიტყვასიტყვით.【javascript/iroha_js/README.md:1387】

## შემდეგი ნაბიჯები (cross-SDK)

- ** რიგის ინტეგრაცია:** როგორც კი ხაზგარეშე რიგი გაიგზავნება, უზრუნველყავით განლაგება/ჩაშვების ლოგიკა
  ზედაპირები `ConnectQueueError` ფასდება, ასე რომ, გადაჭარბებული ტელემეტრია სანდო რჩება.