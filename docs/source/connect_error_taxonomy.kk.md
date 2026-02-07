---
lang: kk
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2025-12-29T18:16:35.935471+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Қосылу қатесі таксономиясы (Swift негізгі)

Бұл жазба IOS-CONNECT-010 қадағалайды және ортақ қате таксономиясын құжаттайды
Nexus SDK файлдарын қосыңыз. Swift енді канондық `ConnectError` қаптамасын экспорттайды,
ол барлық Connect-қа қатысты ақауларды алты санаттың біріне салыстырады, сондықтан телеметрия,
бақылау тақталары және UX көшірмелері платформалар арасында тураланады.

> Соңғы жаңартылған күні: 15.01.2026  
> Иесі: Swift SDK жетекші (таксономия бойынша басқарушы)  
> Күй: Swift + Android + JavaScript іске асырулары **қонды**; кезекті біріктіру күтілуде.

## Санаттар

| Санат | Мақсаты | Типтік көздер |
|----------|---------|-----------------|
| `transport` | Сеансты тоқтататын WebSocket/HTTP тасымалдау ақаулары. | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | Фреймдерді кодтау/декодтау кезінде сериялау/көпір ақаулары. | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | Пайдаланушыны немесе операторды түзетуді қажет ететін TLS/аттестация/саясат қателері. | `URLError(.secureConnectionFailed)`, Torii 4xx жауаптары |
| `timeout` | Бос/офлайн мерзімдер және бақылаушылар (кезегі TTL, сұрау күту уақыты). | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | FIFO кері қысым сигналдары, осылайша қолданбалар жүкті жеңілдетеді. | `ConnectQueueError.overflow(limit:)` |
| `internal` | Қалғанының бәрі: SDK қате пайдалану, Norito көпірі жоқ, журналдар бүлінген. | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

Әрбір SDK таксономияға сәйкес келетін және ашатын қате түрін жариялайды
құрылымдық телеметрия атрибуттары: `category`, `code`, `fatal` және қосымша
метадеректер (`http_status`, `underlying`).

## Жылдам карталау

Swift `ConnectError`, `ConnectErrorCategory` және көмекші протоколдарды экспорттайды.
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. Барлық жалпыға ортақ қосылу қатесі
түрлері `ConnectErrorConvertible` сәйкес келеді, сондықтан қолданбалар `error.asConnectError()` телефонына қоңырау шала алады.
және нәтижені телеметрия/тіркеу қабаттарына жіберіңіз.| Жылдам қате | Санат | Код | Ескертпелер |
|-------------|----------|------|-------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | Қосарланған `start()` көрсетеді; әзірлеуші ​​қатесі. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | Жабылғаннан кейін жіберу/қабылдау кезінде көтеріледі. |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket екілік күту кезінде мәтіндік пайдалы жүктемені жеткізді. |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | Контрагент күтпеген жерден ағынды жауып тастады. |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | Қолданба симметриялық пернелерді конфигурациялауды ұмытып кетті. |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito пайдалы жүктеме міндетті өрістер жоқ. |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | Болашақ пайдалы жүктеме ескі SDK арқылы көрінеді. |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito көпірі жоқ немесе кадр байттарын кодтау/декодтау мүмкін болмады. |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | Көпір қолжетімсіз немесе кілт ұзындығы сәйкес емес. |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | Офлайн кезек ұзындығы конфигурацияланған шектен асып кетті. |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | `URLSessionWebSocketTask` бетінде. |
| `URLError` TLS жағдайлары | `authorization` | `network.tls_failure` | ATS/TLS келіссөздерінің сәтсіздігі. |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | JSON декодтау/кодтау SDK басқа жерде орындалмады; хабарлама Swift декодер контекстін пайдаланады. |
| Кез келген басқа `Error` | `internal` | `unknown_error` | Барлығына кепілдік берілген; хабар айналары `LocalizedError`. |

Құрылғы сынақтары (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) құлыпталады
болашақ рефакторлар санаттарды немесе кодтарды үнсіз өзгерте алмайтындай етіп салыстыру.

### Мысал пайдалану

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

## Телеметрия және бақылау тақталары

Swift SDK `ConnectError.telemetryAttributes(fatal:httpStatus:)` қамтамасыз етеді
ол канондық атрибут картасын қайтарады. Экспорттаушылар бұларды жіберуі керек
Қосымша қосымшалары бар `connect.error` OTEL оқиғаларына атрибуттар:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

Бақылау тақталары `connect.error` есептегіштерін кезек тереңдігімен корреляциялайды (`connect.queue_depth`)
және регрессияларды тіркеу журналдарынсыз анықтау үшін гистограммаларды қайта қосыңыз.

## Android картасын жасауAndroid SDK экспорты `ConnectError`, `ConnectErrorCategory`, `ConnectErrorTelemetryOptions`,
`ConnectErrorOptions` және көмекші утилиталар
`org.hyperledger.iroha.android.connect.error`. Құрастырушы стиліндегі көмекшілер кез келген `Throwable` түрлендіреді
таксономияға сәйкес келетін пайдалы жүктемеге, көлік/TLS/кодек ерекшеліктерінен санаттарды шығару,
және детерминирленген телеметрия атрибуттарын ашыңыз, осылайша OpenTelemetry/іріктеу стектері
тапсырыссыз нәтиже адаптерлер.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectError.jav a:8】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectErrors.java:25】
`ConnectQueueError` `ConnectErrorConvertible` іске асырып, queueOverflow/тайм-аутын шығарады.
толып кету/аяқтау жағдайлары үшін санаттар, сондықтан желіден тыс кезек құралдары бірдей ағынға қосыла алады.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
Android SDK README енді таксономияға сілтеме жасайды және тасымалдау ерекшеліктерін орау жолын көрсетеді
телеметрияны шығармас бұрын, dApp нұсқаулығын Swift базалық сызығына сәйкестендіру.【java/iroha_android/README.md:167】

## JavaScript салыстыру

Node.js/браузер клиенттері `ConnectError`, `ConnectQueueError`, `ConnectErrorCategory` және
`connectErrorFrom()` `@iroha/iroha-js` бастап. Ортақ көмекші HTTP күй кодтарын тексереді,
Түйін қате кодтары (розетка, TLS, күту уақыты), `DOMException` атаулары және кодектерді шығарудағы қателер
осы жазбада құжатталған бірдей санаттар/кодтар, ал TypeScript анықтамалары телеметрияны үлгілейді
атрибут қайта анықтайды, осылайша құралдар қолмен трансляциясыз OTEL оқиғаларын шығара алады.【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
SDK README жұмыс процесін құжаттайды және қолданба топтары жасай алатындай осы таксономияға сілтеме жасайды
құрал үзінділерін сөзбе-сөз көшіріңіз.【javascript/iroha_js/README.md:1387】

## Келесі қадамдар (SDK аралық)

- **Кезек интеграциясы:** желіден тыс кезек жіберілгеннен кейін, кезектен шығару/түсіру логикасын қамтамасыз етіңіз
  беттердің `ConnectQueueError` мәндері, сондықтан асып кету Телеметрия сенімді болып қалады.