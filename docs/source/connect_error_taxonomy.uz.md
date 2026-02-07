---
lang: uz
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2025-12-29T18:16:35.935471+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Ulanish xatosi taksonomiyasi (Swift bazaviy chiziq)

Ushbu eslatma IOS-CONNECT-010 ni kuzatib boradi va umumiy xato taksonomiyasini hujjatlashtiradi
Nexus SDK larni ulash. Swift endi kanonik `ConnectError` o'ramini eksport qiladi,
u Connect bilan bog'liq barcha nosozliklarni oltita toifadan biriga ko'rsatadi, shuning uchun telemetriya,
asboblar paneli va UX nusxasi platformalar bo'ylab mos keladi.

> Oxirgi yangilangan: 2026-01-15  
> Egasi: Swift SDK Lead (taksonomiya styuard)  
> Holat: Swift + Android + JavaScript ilovalari **qo'ndi**; navbat integratsiyasi kutilmoqda.

## Kategoriyalar

| Kategoriya | Maqsad | Oddiy manbalar |
|----------|---------|-----------------|
| `transport` | Seansni tugatuvchi WebSocket/HTTP transport nosozliklari. | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | Kadrlarni kodlash/dekodlashda seriyalash/ko'prikdagi nosozliklar. | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | Foydalanuvchi yoki operatorni tuzatishni talab qiluvchi TLS/attestatsiya/siyosat xatoliklari. | `URLError(.secureConnectionFailed)`, Torii 4xx javoblari |
| `timeout` | Bo'sh/oflayn muddatlar va kuzatuvlar (navbat TTL, so'rov kutish vaqti). | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | FIFO orqa bosim signallari, shuning uchun ilovalar qulay tarzda yuklanishi mumkin. | `ConnectQueueError.overflow(limit:)` |
| `internal` | Qolgan hamma narsa: SDK noto'g'ri foydalanish, etishmayotgan Norito ko'prigi, buzilgan jurnallar. | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

Har bir SDK taksonomiyaga mos keladigan va fosh qiladigan xato turini nashr etadi
tuzilgan telemetriya atributlari: `category`, `code`, `fatal` va ixtiyoriy
metama'lumotlar (`http_status`, `underlying`).

## Tezkor xaritalash

Swift `ConnectError`, `ConnectErrorCategory` va yordamchi protokollarni eksport qiladi
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. Barcha umumiy ulanish xatosi
turlari `ConnectErrorConvertible` ga mos keladi, shuning uchun ilovalar `error.asConnectError()` ga qo'ng'iroq qilishlari mumkin
va natijani telemetriya/logging qatlamlariga yuboring.| Tez xato | Kategoriya | Kod | Eslatmalar |
|-------------|----------|------|-------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | Ikki marta `start()` ni bildiradi; dasturchi xatosi. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | Yopishdan keyin yuborish/qabul qilishda ko'tariladi. |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket ikkilik faylni kutayotganda matnli yukni yetkazib berdi. |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | Kontragent oqimni kutilmaganda yopdi. |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | Ilova simmetrik kalitlarni sozlashni unutdi. |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito foydali yukda majburiy maydonlar yetishmayapti. |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | Eski SDK tomonidan ko'rilgan kelajakdagi foydali yuk. |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito koʻprigi yoʻq yoki freym baytlarini kodlash/dekodlash muvaffaqiyatsiz tugadi. |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | Ko'prik mavjud emas yoki kalit uzunligi mos emas. |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | Oflayn navbat uzunligi sozlangan chegaradan oshib ketdi. |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | `URLSessionWebSocketTask` bilan qoplangan. |
| `URLError` TLS holatlari | `authorization` | `network.tls_failure` | ATS/TLS muzokaralarida muvaffaqiyatsizliklar. |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | JSON kodini dekodlash/kodlash SDKning boshqa joyida bajarilmadi; xabar Swift dekoder kontekstidan foydalanadi. |
| Boshqa har qanday `Error` | `internal` | `unknown_error` | Kafolatlangan qo'lga olish; xabar oynalari `LocalizedError`. |

Birlik sinovlari (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) bloklanadi
xaritalash, shuning uchun kelajakdagi refaktorlar toifalar yoki kodlarni jimgina o'zgartira olmaydi.

### Foydalanishga misol

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

## Telemetriya va asboblar paneli

Swift SDK `ConnectError.telemetryAttributes(fatal:httpStatus:)` ni taqdim etadi
kanonik atribut xaritasini qaytaradi. Eksportchilar bularni yuborishlari kerak
`connect.error` OTEL tadbirlaridagi atributlar ixtiyoriy qoʻshimchalar bilan:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

Boshqaruv paneli `connect.error` hisoblagichlarini navbat chuqurligi bilan bog'laydi (`connect.queue_depth`)
va spelunking jurnallarsiz regressiyalarni aniqlash uchun histogrammalarni qayta ulang.

## Android xaritalashAndroid SDK eksport qiladi `ConnectError`, `ConnectErrorCategory`, `ConnectErrorTelemetryOptions`,
`ConnectErrorOptions` va yordamchi yordamchi dasturlar
`org.hyperledger.iroha.android.connect.error`. Quruvchi uslubidagi yordamchilar har qanday `Throwable` ni aylantiradi
taksonomiyaga mos keladigan foydali yukga, transport/TLS/kodek istisnolaridan toifalarni chiqarish,
va deterministik telemetriya atributlarini oching, shuning uchun OpenTelemetry/namuna olish steklari iste'mol qilishi mumkin.
buyurtmasiz natija adapterlar.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectError.jav a:8】【Java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectErrors.java:25】
`ConnectQueueError` allaqachon `ConnectErrorConvertible` ni amalga oshirib, queueOverflow/vaqt tugashini chiqaradi
toshib ketish/muddati tugash shartlari uchun toifalar, shuning uchun oflayn navbat asboblari bir xil oqimga ulanishi mumkin.【Java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
Android SDK README endi taksonomiyaga havola qiladi va transport istisnolarini qanday o'rashni ko'rsatadi
telemetriyani chiqarishdan oldin, dApp yoʻriqnomasini Swift bazaviy chizigʻiga moslab saqlang.【java/iroha_android/README.md:167】

## JavaScript xaritalash

Node.js/brauzer mijozlari `ConnectError`, `ConnectQueueError`, `ConnectErrorCategory` va
`connectErrorFrom()` dan `@iroha/iroha-js`. Umumiy yordamchi HTTP holat kodlarini tekshiradi,
Tugun xato kodlari (rozetka, TLS, vaqt tugashi), `DOMException` nomlari va kodeklarni chiqarishdagi nosozliklar
Ushbu eslatmada hujjatlashtirilgan bir xil toifalar/kodlar, TypeScript ta'riflari esa telemetriyani modellashtiradi
atributni bekor qiladi, shuning uchun asboblar OTEL hodisalarini qo'lda translyatsiya qilmasdan chiqarishi mumkin.【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
SDK README ish jarayonini hujjatlashtiradi va dastur guruhlari bu taksonomiyaga havola qiladi
asboblar parchalarini so'zma-so'z nusxalash.【javascript/iroha_js/README.md:1387】

## Keyingi qadamlar (SDK o'zaro)

- **Navbat integratsiyasi:** oflayn navbat yuborilgandan so'ng, navbatni bekor qilish/tutish mantiqini ta'minlang
  yuzalar `ConnectQueueError` qiymatlari, shuning uchun toshib ketish Telemetriya ishonchliligicha qolmoqda.