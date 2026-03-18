---
lang: dz
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2025-12-29T18:16:35.935471+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# འཛོལ་བའི་ ཊེག་སོ་ནོ་མི་ (ཆུ་རུད་གཞི་རྟེན་) མཐུད་།

དྲན་འཛིན་འདི་གིས་ IOS-CONNECT-010 བརྟག་ཞིབ་འབདཝ་ཨིནམ་དང་ འདི་གི་དོན་ལུ་ ནོར་བཅོས་འབད་བའི་འཛོལ་བ་གི་དབྱེ་ཚན་འདི་ ཡིག་ཆ་བཟོཝ་ཨིན།
Nexus ཨེསི་ཌི་ཀེ་ཚུ་མཐུད། ད་ལྟོ་ སུའིཕཊི་གིས་ ཀེ་ནོ་ནིག་ `ConnectError` wrapper ཕྱིར་འདྲེན་འབདཝ་ཨིན།
འདི་ཡང་ དབྱེ་ཁག་དྲུག་ལས་གཅིག་ལུ་ མཐུད་འབྲེལ་དང་འབྲེལ་བའི་ འཐུས་ཤོར་ཚུ་ག་ར་ སབ་ཁྲ་བཟོཝ་ཨིན་མས།
dashboard དང་ UX འདྲ་བཤུས་འདི་ སྟེགས་བུ་ཚུ་ནང་ལས་ཕར་ ཕྲང་སྒྲིག་འབད་ཡོདཔ་ཨིན།

> མཐའ་མའི་དུས་མཐུན་བཟོ་ཡོདཔ།: ༢༠༢༦-༠༡-༡༥  
> ཇོ་བདག་: སུའིཕཊ་ཨེསི་ཌི་ཀེ་ ལིཌ་ (taxonomy གི་བདག་འཛིན་པ)།  
> གནས་སྟངས་: སའིཕཊི་ + ཨེན་ཌོཌ་ + ཇ་བ་ཨིསི་ཀིརིཔཊི་ལག་ལེན་འཐབ་ཐངས་ཚུ་ **landed**; གྱལ་རིམ་གཅིག་བསྡོམས་འབད་ནི།

## དབྱེ་རིམ།

| དབྱེ་ཁག་ | དམིགས་ཡུལ། | སྤྱིར་བཏང་གི་འབྱུང་ཁུངས། |
|--------------------------------------------- |
| `transport` | ལཱ་ཡུན་ཅིག་མཇུག་བསྡུ་མི་ ཝེབ་སོ་ཀེཊི་/ཨེཆ་ཊི་ཊི་པི་སྐྱེལ་འདྲེན་འཐུས་ཤོར་ཚུ། | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | རིམ་སྒྲིག་/ཟམ་གྱི་འཐུས་ཤོར་ཚུ་ ཨིན་ཀོ་ཌིང་/ཌི་ཀོཌི་གཞི་ཁྲམ་ཚུ་ཨིན། | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | ལག་ལེན་པ་ཡང་ན་ བཀོལ་སྤྱོད་པའི་བཅོ་ཐབས་དགོ་པའི་ TLS/testation/porgicy འཐུས་ཤོར་ཚུ། | `URLError(.secureConnectionFailed)`, Torii ༤xx ལན་འདེབས་ཚུ། |
| `timeout` | Idle/offline གི་དུས་ཡུན་དང་ བལྟ་རྟོག་པ་ཚུ་ (ཀིའུ་ཊི་ཊི་ཨེལ་ ཞུ་བ་དུས་ཚོད་མཇུག་བསྡུ་ནི།) | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | FIFO རྒྱབ་ལོག་གནོན་ཤུགས་བརྡ་རྟགས་ཚུ་ དེ་ལས་ གློག་རིམ་ཚུ་གིས་ ལེགས་ཤོམ་སྦེ་ མངོན་གསལ་འབད་ཚུགས། | `ConnectQueueError.overflow(limit:)` |
| `internal` | གཞན་ག་ར་: SDK འཛོལ་བ་, I1NT000000000X ཟམ་མེདཔ་ཨིན་, དུས་དེབ་ཚུ་ ངན་ལྷད་འབད་ཡོདཔ། | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

ཨེསི་ཌི་ཀེ་རེ་རེ་གིས་ དབྱེ་རིམ་དང་གསལ་སྟོན་ལུ་མཐུན་པའི་འཛོལ་བའི་དབྱེ་བ་ཅིག་དཔར་བསྐྲུན་འབདཝ་ཨིན།
བཀོད་སྒྲིག་འབད་ཡོད་པའི་ བརྒྱུད་འཕྲིན་ཁྱད་ཆོས་ཚུ་: `category`, `code`, `fatal`, དང་ གདམ་ཁ་ཅན་ཨིན།
མེ་ཊ་ཌེ་ཊ་ (`http_status`, `underlying`).

## སོར་ཆུད་ས་ཁྲ་བཟོ་ནི།

སུའིཕཊ་ཕྱིར་འདྲེན་ `ConnectError`, `ConnectErrorCategory`, དང་ རོགས་རམ་མཐུན་རྐྱེན།
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. མི་མང་མཐུད་པའི་འཛོལ་བ།
དབྱེ་བ་ཚུ་ `ConnectErrorConvertible` དང་མཐུན་སྒྲིག་འབདཝ་ལས་ གློག་རིམ་ཚུ་གིས་ `error.asConnectError()` ལུ་འབོ་ཚུགས།
དང་ གྲུབ་འབྲས་འདི་ ཊེ་ལི་མི་ཊི་རི་/དྲན་ཐོ་བང་རིམ་ཚུ་ལུ་ གདོང་བསྐྱོད་འབད།| མགྱོགས་མྱུར་འཛོལ་བ་ | དབྱེ་ཁག་ | ཨང་རྟགས་ | དྲན་ཐོ། |
|----------------------------------------------- -------------------------------
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | `start()` གཉིས་ལྡན་བརྡ་སྟོནམ་ཨིན། གོང་འཕེལ་གཏོང་མཁན་འཛོལ་བ། |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | སྒོ་བསྡམ་པའི་ཤུལ་ལས་ གཏང་ནི་/ལེན་པའི་སྐབས་ ཡར་འཛེགས་ཡོདཔ། |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | ཝེབ་སོ་ཀེཊ་གིས་ རེ་བ་བསྐྱེད་པའི་སྐབས་ ཚིག་ཡིག་གི་ པེ་ལོཌ་ཚུ་ བཀྲམ་སྤེལ་འབད་ཡོདཔ་ཨིན། |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | ཕྱོགས་གྲངས་ཀྱིས་ རྒྱུན་ལམ་འདི་ རེ་བ་མེད་པར་ཁ་བསྡམས་ཡོདཔ་ཨིན། |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | མཉམ་མཐུན་ལྡེ་མིག་ཚུ་རིམ་སྒྲིག་འབད་ནི་བརྗེད་སོང་། |
| `ConnectEnvelopeError.invalidPayload` | `codec` | Norito | Norito འབབ་ཁུངས་མེད་པའི་ས་སྒོ། |
| Nexus | `codec` | ```swift
do {
    try await session.nextEnvelope()
} catch {
    let connectError = error.asConnectError()
    telemetry.emit(event: "connect.error",
                   attributes: connectError.telemetryAttributes(fatal: true))
    logger.error("Connect failure: \(connectError.code) – \(connectError.message)")
}
``` | SDK རྙིངམོ་གིས་མཐོང་མི་ མ་འོངས་པའི་ གླ་ཆ་ཚུ། |
| ```swift
do {
    try await session.nextEnvelope()
} catch {
    let connectError = error.asConnectError()
    telemetry.emit(event: "connect.error",
                   attributes: connectError.telemetryAttributes(fatal: true))
    logger.error("Connect failure: \(connectError.code) – \(connectError.message)")
}
``` | `ConnectError` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito ཟམ་ཟམ་མེདཔ་ཡང་ན་ ཡང་ན་ ཨིན་ཀོ་ཌི་/ཌི་ཀོཌི་གཞི་ཁྲམ་བཱའིཊིསི་ འབད་མ་ཚུགས། |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | ཟམ་མེད་ ཡང་ན་ མཐུན་སྒྲིག་མེད་པའི་ལྡེ་མིག་རིང་ཚད་ཚུ། |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | ```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
``` | རིམ་སྒྲིག་འབད་ཡོད་པའི་བང་རིམ་རིང་ཚད་འདི་རིམ་སྒྲིག་འབད་ཡོད་པའི་མཚམས་ལས་ལྷག་ཡོདཔ་ཨིན། |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | ཁ་ཐོག་ལུ་ `URLSessionWebSocketTask`. |
| `URLError` ཊི་ཨེལ་ཨེསི་གི་གནད་དོན། | `authorization` | `network.tls_failure` | ATS/TLS མཐུན་སྒྲིག་འཐུས་ཤོར་ཚུ། |
| `DecodingError` / `EncodingError` | `codec` | `ConnectError` | JSON ཌི་ཀོ་ཌིང་/ཨེན་ཀོ་ཌིང་འདི་ ཨེསི་ཌི་ཀེ་ནང་ ས་གནས་གཞན་ཁར་ འཐུས་ཤོར་བྱུང་ཡོདཔ། འཕྲིན་དོན་འདི་གིས་ སུའིཕཊི་ཌི་ཀོཌར་སྐབས་དོན་འདི་ལག་ལེན་འཐབ་ཨིན། |
| གཞན་ཡང་ `Error` | `internal` | `unknown_error` | འགན་ལེན་གྱི་ ག་ར་འཛིན་དགོ། འཕྲིན་དོན་མེ་ལོང་ `LocalizedError`. |

ཡན་ལག་བརྟག་དཔྱད་ (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) སྒོ་བསྡམས།
སབ་ཁྲ་བཟོ་ནི་འདི་ མ་འོངས་པའི་བསྐྱར་བཟོ་ཚུ་གིས་ དབྱེ་རིམ་ཡང་ན་ གསང་ཡིག་ཚུ་ ཁུ་སིམ་སིམ་སྦེ་ བསྒྱུར་བཅོས་འབད་མི་ཚུགས།

###དཔེར་བརྗོད།

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

## བརྒྱུད་འཕྲིན་དང་ བརྡ་བཀོད་བཀོད་ཚོགས།

སུའིཕཊི་ཨེསི་ཌི་ཀེ་གིས་ `ConnectError.telemetryAttributes(fatal:httpStatus:)` བྱིནམ་ཨིན།
ག་ཅི་གིས་ ཀེ་ནོ་ནིག་ཁྱད་ཆོས་སབ་ཁྲ་འདི་སླར་ལོག་འབདཝ་ཨིན། ཕྱིར་འདྲེན་འབད་མི་ཚུ་གིས་ འདི་ཚུ་གདོང་བསྐྱོད་འབད་དགོ།
གདམ་ཁ་ཅན་གྱི་ཁ་སྐོང་ཚུ་དང་གཅིག་ཁར་ `connect.error` OTEL བྱུང་ལས་ཚུ་:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

ཌེཤ་བོརཌ་ཚུ་གིས་ `connect.error` གྱངས་ཁ་ཚུ་ བང་རིམ་གཏིང་ཚད་ (`connect.queue_depth`) དང་ཅིག་ཁར་ འབྲེལ་མཐུད་འབདཝ་ཨིན།
དང་ དྲན་ཐོ་ཚུ་མེད་པར་ འགྱུར་ལྡོག་ཚུ་ ཤེས་རྟོགས་འབད་ནི་ལུ་ ཧིསི་ཊོ་གཱརམ་ཚུ་ ལོག་མཐུད་དགོ།

## ཨེན་ཌྲོའིཌ་ས་ཁྲ།Android SDK གིས་ `ConnectError`, `transport`, `ConnectErrorTelemetryOptions`, ཕྱིར་འདྲེན་འབདཝ་ཨིན།
`ConnectErrorOptions`, དང་རོགས་རམ་གྱི་ལག་ཆས།
`org.hyperledger.iroha.android.connect.error`. བཟོ་བསྐྲུན་བཟོ་རྣམ་གྱི་གྲོགས་རམ་པ་ཚུ་གིས་ `Throwable` གང་རུང་ཅིག་བསྒྱུར་བཅོས་འབདཝ་ཨིན།
དབྱེ་ཁག་དང་མཐུན་སྒྲིག་ཅན་གྱི་ པེ་ལོཌ་ནང་ སྐྱེལ་འདྲེན་/ཊི་ཨེལ་ཨེསི་/ཀོཌེག་ དམིགས་བསལ་ལས་ དབྱེ་ཁག་ཚུ་ དབྱེ་ཁག་བཟོ་ནི།
དང་ གཏན་འབེབས་བརྡ་འཕྲིན་ཁྱད་ཆོས་ཚུ་ གསལ་སྟོན་འབདཝ་ལས་ OpenTelemetry/དཔེ་ཚད་བརྩེགས་བརྩེགས་ཚུ་གིས་ བཀོལ་སྤྱོད་འབད་ཚུགས།
འབྲས་བུ་མེད་པར་འབྲས་བུ་། མཐུན་སྒྲིག་འབད་མི་.【jav/iroha_android/src/src/src/src/ཇ་བ་/ཨོར་ག/ཨི་རོ་ཧ/ཨན་ཌོར/ཨན་ཌོར/ཨེནཌྲོཌ། ཀ:༨】 】  】                                                                                                                         】 】 】 】 】                                                མིན་མོང་མ་ཡིན་པ/ཇ་ཝ/ཨོར་ག/ཨི་རོ་ཧ/ཨནྡྲོའི་/འཛོལ་བ་/ཀོན་ནེག་ཨེར་རར་ཇ་བ་:༢༥】
`ConnectQueueError` གིས་ཧེ་མ་ལས་རང་ `ConnectErrorConvertible` གིས་ ཀིའུ་ཨོ་ཝར་ཕོལོ་/དུས་ཚོད་ཨའུཊི་འདི་ བཤུབ་བཏང་ཡོདཔ་ཨིན།
འདི་ཡང་ 【jav/iroha_android/src/src/main/main/མ་ཝ་/org/hyperledger/iroha/android/ དང་ /////// ཨེནཌརའི/ཨེནཌ་ཨེནཌ་/ཨེནཌི་/ཨེནཌི་/ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌ་ཨེནཌལ་/ཀོན་ནེག་ཀའུ་ཨི་རོར.ཇ་བ་:】 དབྱེ་ཁག་ཚུ་
ད་ལྟོ་ Android SDK README གིས་ དབྱེ་ཁག་འདི་ གཞི་བསྟུན་འབད་དེ་ སྐྱེལ་འདྲེན་གྱི་ ཁྱད་པར་ཚུ་ ག་དེ་སྦེ་ བཀབ་ཚུགསཔ་ཨིན་ན་ སྟོནམ་ཨིན།
【java/iroha_android/README.md:167】

## ཇ་བ་ཨིསི་ཀིརིཔཊི་སབ་ཁྲ་བཟོ་ནི།

Node.js/བརའུ་ཟར་གྱི་མཁོ་མངགས་འབད་མི་ཚུ་གིས་ `ConnectError`, `ConnectQueueError`, `ConnectErrorCategory`, དང་།
`connectErrorFrom()` ལས་ `@iroha/iroha-js` ལས་། བརྗེ་སོར་གྱི་གྲོགས་རམ་པ་གིས་ ཨེཆ་ཊི་ཊི་པི་གནས་ཚད་ཀྱི་ཨང་རྟགས་ཚུ་ཞིབ་དཔྱད་འབདཝ་ཨིན།
ནའུཊི་འཛོལ་བ་ཨང་རྟགས་ (སོ་ཀེཊི་ ཊི་ཨེལ་ཨེསི་ དུས་ཚོད་རྫོགས་ནི), `DOMException` མིང་ཚུ་ དེ་ལས་ ཀོཌེཀ་འཐུས་ཤོར་ཚུ་གིས་ བཏོན་གཏང་ནི་ལུ་ བཏོན་གཏང་།
དྲན་འཛིན་འདི་ནང་ ཡིག་ཐོག་ལུ་བཀོད་ཡོད་པའི་དབྱེ་རིམ་/ཨང་རྟགས་ཚུ་ ཅོག་འཐདཔ་འབད་ཡོདཔ་ད་ ཊའིཔ་སི་ཀིརིཔཊི་ངེས་ཚིག་ཚུ་གིས་ ཊེ་ལི་མི་ཊི་རི་འདི་ དཔེ་སྟོན་འབདཝ་ཨིན།
ཁྱད་ཆོས་འདི་ ལག་ཐོག་བཀལ་མ་དགོ་པར་ OTEL བྱུང་ལས་ཚུ་ བཏོནམ་ཨིན། 【javscript/iroha_js/src/connectError.js:1】】 ཨི་རོ་ཧ་_ཇེ་ཨེསི་/ཨིན་ཌེགསི་ཌི་.ཊི་ཨེསི་:༨༤༠】།
SDK README གིས་ ལཱ་གི་རྒྱུན་རིམ་འདི་ཡིག་ཆ་བཟོ་སྟེ་ དབྱེ་ཁག་འདི་ལུ་ལོག་འབྲེལ་མཐུད་འབདཝ་ལས་ ལག་ལེན་སྡེ་ཚན་གྱིས་ འབད་ཚུགས།
འདྲ་བཤུས་འཇོག་པའི་ཚིག་ཕྲེང་ཁ་དོན།【javascript/iroha_js/README.md:1387】།

## ཤུལ་མམ་གྱི་གོམ་པ་ ("SDK)

- **ཀིའུའུ་མཉམ་བསྡོམས་:** ཨོཕ་ལ་ཡིན་གྱི་གྱལ་གྲུ་ཚུ་ཚར་གཅིག་ ཀིའུ་ཌི་ཀིའུ་/བཀོག་བཞག་ཚད་མ་འདི་ངེས་གཏན་བཟོ།
  ཁ་ཐོག་ `ConnectQueueError` གནས་གོང་ཚུ་ དེ་འབདཝ་ལས་ བློ་གཏད་ཅན་གྱི་ ཊེ་ལི་མི་ཊི་རི་ལུ་ བློ་གཏད་ཅན་སྦེ་རང་ ལུས་ཡོདཔ་ཨིན།