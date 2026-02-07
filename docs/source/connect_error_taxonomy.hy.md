---
lang: hy
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2025-12-29T18:16:35.935471+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Connect Error Taxonomy (Swift բազային)

Այս նշումը հետևում է IOS-CONNECT-010-ին և փաստաթղթավորում է ընդհանուր սխալների դասակարգումը
Nexus Միացրեք SDK-ները: Swift-ն այժմ արտահանում է `ConnectError` կանոնական փաթաթան,
որը քարտեզագրում է Connect-ի հետ կապված բոլոր ձախողումները վեց կատեգորիաներից մեկին, այսինքն՝ հեռաչափություն,
վահանակները և UX պատճենները հավասարեցված են մնում հարթակներում:

> Վերջին թարմացումը՝ 2026-01-15  
> Սեփականատեր՝ Swift SDK առաջատար (տաքսոնոմիայի կառավարիչ)  
> Կարգավիճակ. Swift + Android + JavaScript ներդրում ** վայրէջք**; հերթի ինտեգրումը սպասվում է:

## Կատեգորիաներ

| Կարգավիճակ | Նպատակը | Տիպիկ աղբյուրներ |
|----------|---------|-----------------|
| `transport` | WebSocket/HTTP տրանսպորտի ձախողումներ, որոնք դադարեցնում են նիստը: | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | Շարքերի կոդավորման/վերծանման ժամանակ սերիալիզացիայի/կամուրջի ձախողումներ: | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | TLS/ատեստավորում/քաղաքականության ձախողումներ, որոնք պահանջում են օգտվողի կամ օպերատորի վերականգնում: | `URLError(.secureConnectionFailed)`, Torii 4xx պատասխաններ |
| `timeout` | Անգործուն/անցանց ժամկետների ավարտ և պահակ (հերթ TTL, հարցումների ժամանակի ավարտ): | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | FIFO-ի հետադարձ ճնշման ազդանշանները, որպեսզի հավելվածները կարողանան նրբագեղորեն թուլացնել բեռը: | `ConnectQueueError.overflow(limit:)` |
| `internal` | Մնացած ամեն ինչ՝ SDK-ի չարաշահում, բացակայող Norito կամուրջ, կոռումպացված ամսագրեր: | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

Յուրաքանչյուր SDK հրապարակում է սխալի տեսակ, որը համապատասխանում է դասակարգմանը և բացահայտում է
կառուցված հեռաչափության հատկանիշներ՝ `category`, `code`, `fatal` և կամընտիր
մետատվյալներ (`http_status`, `underlying`):

## Արագ քարտեզագրում

Swift-ն արտահանում է `ConnectError`, `ConnectErrorCategory` և օգնական արձանագրություններ
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. Բոլոր հանրային կապի սխալը
տեսակները համապատասխանում են `ConnectErrorConvertible`-ին, ուստի հավելվածները կարող են զանգահարել `error.asConnectError()`
և արդյունքը փոխանցեք հեռաչափության/լոգերի շերտերին:| Swift սխալ | Կարգավիճակ | Կոդ | Ծանոթագրություններ |
|-------------|----------|------|-------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | Ցույց է տալիս կրկնակի `start()`; մշակողի սխալ. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | Բարձրացվել է փակումից հետո ուղարկելու/ստանալու ժամանակ: |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket-ը տրամադրեց տեքստային բեռնվածություն՝ սպասելով երկուական: |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | Կոնտերկողմն անսպասելիորեն փակեց հոսքը: |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | Հավելվածը մոռացել է կարգավորել սիմետրիկ ստեղները: |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito օգտակար բեռը բացակայում է պարտադիր դաշտերը: |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | Ապագա օգտակար բեռը տեսել է հին SDK-ն: |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito կամուրջը բացակայում է կամ չկարողացավ կոդավորել/վերծանել շրջանակի բայթերը: |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | Կամուրջի անհասանելի կամ անհամապատասխան բանալու երկարություններ: |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | Անցանց հերթի երկարությունը գերազանցել է կազմաձևված սահմանը: |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | Մակերեսով `URLSessionWebSocketTask`: |
| `URLError` TLS պատյաններ | `authorization` | `network.tls_failure` | ATS/TLS բանակցությունների ձախողումներ. |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | JSON-ի վերծանումը/կոդավորումը ձախողվեց SDK-ի այլ վայրերում. հաղորդագրությունն օգտագործում է Swift ապակոդավորիչի համատեքստը: |
| Ցանկացած այլ `Error` | `internal` | `unknown_error` | Երաշխավորված գրավիչ; հաղորդագրության հայելիներ `LocalizedError`. |

Միավորի թեստերը (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) արգելափակվում են
քարտեզագրում, այնպես որ ապագա ռեֆակտորները չեն կարող լուռ փոխել կատեգորիաները կամ ծածկագրերը:

### Օգտագործման օրինակ

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

## Հեռուստաչափություն և վահանակներ

Swift SDK-ն ապահովում է `ConnectError.telemetryAttributes(fatal:httpStatus:)`
որը վերադարձնում է կանոնական հատկանիշի քարտեզը։ Արտահանողները պետք է դրանք փոխանցեն
վերագրում է `connect.error` OTEL իրադարձություններին կամընտիր հավելումներով.

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

Վահանակները փոխկապակցում են `connect.error` հաշվիչները հերթի խորության հետ (`connect.queue_depth`)
և նորից միացրեք հիստոգրամները՝ հետընթացները հայտնաբերելու համար՝ առանց ուղղագրության տեղեկամատյանների:

## Android քարտեզագրումAndroid SDK-ն արտահանում է `ConnectError`, `ConnectErrorCategory`, `ConnectErrorTelemetryOptions`,
`ConnectErrorOptions`, իսկ օգնականի կոմունալ ծառայությունների տակ
`org.hyperledger.iroha.android.connect.error`. Շինարարական ոճի օգնականները փոխակերպում են ցանկացած `Throwable`
Տաքսոնոմիային համապատասխանող ծանրաբեռնվածության մեջ, կատեգորիաները եզրակացնել տրանսպորտից/TLS/codec բացառություններից,
և ցուցադրել դետերմինիստական հեռաչափության ատրիբուտներ, որպեսզի OpenTelemetry/sampling stacks-ը կարողանա սպառել
արդյունք առանց պատվերի ադապտեր.
`ConnectQueueError`-ն արդեն իրականացնում է `ConnectErrorConvertible`՝ թողարկելով հերթը Overflow/timeout
կատեգորիաներ գերհոսքի/ժամկետանց պայմանների համար, այնպես որ անցանց հերթի գործիքավորումը կարող է միացնել նույն հոսքին:【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
Android SDK README-ն այժմ հղում է կատարում դասակարգմանը և ցույց է տալիս, թե ինչպես կարելի է փաթեթավորել տրանսպորտի բացառությունները
նախքան հեռաչափությունը արտանետելը, dApp-ի ուղեցույցը համապատասխանեցնելով Swift-ի բազային գծին:【java/iroha_android/README.md:167】

## JavaScript քարտեզագրում

Node.js/browser-ի հաճախորդները ներմուծում են `ConnectError`, `ConnectQueueError`, `ConnectErrorCategory` և
`connectErrorFrom()` `@iroha/iroha-js`-ից: Համօգտագործվող օգնականը ստուգում է HTTP կարգավիճակի կոդերը,
Հանգույցի սխալի կոդերը (վարդակ, TLS, ժամանակի ավարտ), `DOMException` անուններ և կոդեկի սխալներ
նույն կատեգորիաները/կոդերը փաստագրված են այս նշումում, մինչդեռ TypeScript սահմանումները մոդելավորում են հեռաչափությունը
հատկանիշը փոխարինում է, այնպես որ գործիքավորումը կարող է արտանետել OTEL իրադարձություններ առանց ձեռքով հեռարձակման:【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
SDK README-ը փաստաթղթավորում է աշխատանքային հոսքը և կապվում է այս դասակարգման հետ, որպեսզի կիրառական թիմերը կարողանան
Պատճենեք գործիքավորման հատվածները բառացի։【javascript/iroha_js/README.md:1387】

## Հաջորդ քայլերը (cross-SDK)

- **Հերթի ինտեգրում.** երբ անցանց հերթը ուղարկվի, ապահովեք հերթերի/թողարկման տրամաբանություն
  մակերևույթների `ConnectQueueError` արժեքները, ուստի գերհոսող Հեռաչափությունը մնում է վստահելի: