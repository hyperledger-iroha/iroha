---
lang: ba
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2025-12-29T18:16:35.935471+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Хата таксономияһын тоташтырығыҙ (Свифт база һыҙығы)

Был иҫкәрмә IOS-CONNECT-010 күҙәтә һәм документтар дөйөм хата таксономияһы өсөн .
Nexus тоташтырыу СДК. Свифт хәҙер канонлы `ConnectError` уратып экспортлай,
был карталар бөтә Connect менән бәйле етешһеҙлектәр алты категорияның береһенә шулай телеметрия,
приборҙар таҡталары, һәм UX күсермәһе платформалар буйынса тура килтереп ҡала.

> Һуңғы яңыртылған: 2026-01-15  
> Хужаһы: Свифт SDK етәксеһе (таксономия идара итеү)  
> Статус: Свифт + Android + JavaScript тормошҡа ашырыуҙар **ергә **; сират интеграцияһы көтөлгән.

## Категориялар

| Категория | Маҡсат | Типик сығанаҡтар |
|---------|----------|-----------------|
| `transport` | WebSocket/HTTP транспорт етешһеҙлектәре, улар сессияны туҡтатыу. | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | Сериализация/күпер етешһеҙлектәре, шул уҡ ваҡытта кодлау/декодировка кадрҙары. | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | TLS/аттестация/сәйәсәт етешһеҙлектәре ҡулланыусы йәки операторҙы төҙәтеү талап итә. | `URLError(.secureConnectionFailed)`, Torii 4xx яуаптар |
| `timeout` | Ялҡау/офлайн срогы һәм күҙәтеүсе эттәр (сират TTL, тайм-аут һорап). | `URLError(.timedOut)`, `ConnectQueueError.expired` X |
| `queueOverflow` | FIFO артҡы баҫым сигналдары, шулай итеп, ҡушымталар йөк һибә ала грациозно. | `ConnectQueueError.overflow(limit:)` |
| `internal` | Ҡалған бөтә нәмә: SDK дөрөҫ ҡулланмау, юғалған Norito күпер, боҙолған журналдар. | Norito, `ConnectCryptoError.*` |

Һәр SDK таксономияға яраҡлашҡан һәм фашлаусы хата тибын баҫтыра.
структуралы телеметрия атрибуттары: `category`, `code`, `fatal`, һәм теләк буйынса
метамағлүмәттәр (`http_status`, `underlying`).

## Свифт картаһы

Свифт экспорты `ConnectError`, `ConnectErrorCategory`, һәм ярҙамсы протоколдар .
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. Бөтә йәмәғәт тоташтырыу хатаһы
типтары тура килә `ConnectErrorConvertible`, шуға күрә ҡушымталар шылтырата ала `error.asConnectError()` .
һәм һөҙөмтәне телеметрия/уҡыу ҡатламдарына йүнәлтергә.| Свифт хатаһы | Категория | Код | Иҫкәрмәләр |
|-----------|-----------|-------|------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | `start()` ике тапҡырға күрһәтә; разработчик хатаһы. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | Күтәрелгән ҡасан ебәргәндә/ҡабул итеү һуң ябыҡ. |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket тапшырылған тексты файҙалы йөк, шул уҡ ваҡытта көтөп бинар. |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | Контристия көтөлмәгәнсә ағымды япты. |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | Ҡушымта симметрик асҡыстарҙы конфигурациялауҙы онотҡан. |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito файҙалы йөк юҡ кәрәкле баҫыуҙар юҡ. |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | Киләсәк файҙалы йөк күргән иҫке SDK. |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito күпере юҡ йәки кодлау/декод рамка байттарын кодлай алманы. |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | Күпер недоступный йәки тап килмәгән асҡыс оҙонлоҡтары. |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | Офлайн сират оҙонлоғо конфигурацияланған сиктән артып киткән. |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | `URLSessionWebSocketTask` тарафынан ер өҫтөндә. |
| `URLError` TLS осраҡтары | `authorization` | `network.tls_failure` | АТС/ТЛС һөйләшеүҙәр етешһеҙлектәре. |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | JSON декодлау/кодлау башҡа урындарҙа уңышһыҙлыҡҡа осраған SDK; хәбәр Swift декодер контексын ҡуллана. |
| Башҡа теләһә ниндәй `Error` | `internal` | `unknown_error` | Гарантированный тотоу-барыһы ла; хәбәр көҙгөләре `LocalizedError`. |

Блок һынауҙары (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) бикләп тора
картаһы шулай буласаҡ рефакторҙар өнһөҙ генә категориялар йәки кодтарҙы үҙгәртә алмай.

### Миҫал ҡулланыу

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

## Телеметрия & приборҙар таҡтаһы

Swift SDK `ConnectError.telemetryAttributes(fatal:httpStatus:)` тәьмин итә.
канон атрибуты картаһын ҡайтара. Экспортерҙар был тапшырырға тейеш
атрибуттар `connect.error` OTEL ваҡиғалар менән өҫтәмә өҫтәмә:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

Приборҙар таҡталары `connect.error` корреляцияһы сират тәрәнлеге менән (`connect.queue_depth`X) ҡаршы тора.
һәм гистограммаларҙы яңынан тоташтырыу өсөн регрессияларҙы асыҡлау өсөн логик логтар.

## Android картаһыAndroid SDK экспорты `ConnectError`, `ConnectErrorCategory`, `ConnectErrorTelemetryOptions`,
`ConnectErrorOptions`, ә ярҙамсы коммуналь хеҙмәттәргә 1990 й.
`org.hyperledger.iroha.android.connect.error`. Төҙөүселәр стилендәге ярҙамсылар теләһә ниндәй `Throwable` .
таксономияға ярашлы файҙалы йөккә, категорияларҙан категорияларҙан сығып, транспорт/ТЛС/кодек осраҡтарҙан тыш,
һәм детерминистик телеметрия атрибуттарын фашлай, шуға күрә OpenTelemetrietf
һөҙөмтә заказ буйынса . адаптерҙар.【жава/ироха_андроид/срк/төп/java/org/гиперледжер/ироха/андроид/тоташтырыу/харро/КоннекЭрр.джав а:8】【java/ироха_андроид/срк/төп/java/org/гиперледжер/ироха/андроид/тоташ/харрор/КоннекЭррорс.джава:25】
`ConnectQueueError` инде `ConnectErrorConvertible`-ты тормошҡа ашыра, был сират Overflow/timeout .
категориялары өсөн ташыу/ваҡыт шарттары шулай офлайн сират приборҙары сым ала, шул уҡ ағымға.【жава/ироха_андроид/src/src/java/org/гиперледжер/ироха/андроха/комплект/хата/ConnectQueueTeue Error.java:5】
Android SDK README хәҙер таксономияға һылтанмалар һәм нисек урап транспортлау осраҡтарын күрһәтә .
телеметрия сығарыу алдынан, һаҡлау dApp етәкселек менән тура килтерелгән Swift база һыҙығы.【java/iroha_android/README.md:167】

## JavaScript картаһы

Nod
`connectErrorFrom()`X `@iroha/iroha-js` X. Дөйөм ярҙамсы HTTP статусы кодтарын тикшерә,
Төйөн хатаһы кодтары (розетка, TLS, тайм-аут), `DOMException` исемдәре, һәм кодек уңышһыҙлыҡтар сығарыу өсөн
был иҫкәрмәлә документлаштырылған бер үк категориялар/кодтар, ә TypeScript билдәләмәләре телеметрияны моделләштереүсе
атрибут өҫтөнлөклө шулай инструменттарҙы ҡул менән ҡойоуһыҙ OTEL ваҡиғалар сығара ала.【javascript/iroha_js/src/conconect Error.js:1】【жаваскретинг/ироха_js/index.d.ts:840】
SDK README документтар эш ағымы һәм һылтанмалар кире был таксономия, шулай итеп, ғариза командалары ала
Приборҙар өҙөктәрен һүҙмә-һүҙ күсерергә.【javascript/iroha_js/README.md:1387】

## Киләһе аҙымдар (СДК-ның кросс)

- **Сиаталь интеграция:** офлайн сират суднолары бер тапҡыр, логиканың ҡыҫҡартыу/тамсыһын тәьмин итеү
  өҫтөнлөктәре `ConnectQueueError` ҡиммәттәре шулай ташыу Телеметрия ышаныслы булып ҡала.