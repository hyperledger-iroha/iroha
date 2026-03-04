---
lang: am
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2025-12-29T18:16:35.935471+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የግንኙነቶች ስህተት Taxonomy (ፈጣን መነሻ መስመር)

ይህ ማስታወሻ IOS-CONNECT-010ን ይከታተላል እና የተጋራውን የግብር ስህተቱን ይመዘግባል
Nexus አገናኝ ኤስዲኬዎች። ስዊፍት አሁን ቀኖናዊውን `ConnectError` መጠቅለያ ወደ ውጭ ይልካል።
ሁሉንም ከግንኙነት ጋር የተገናኙ ውድቀቶችን ከስድስት ምድቦች ወደ አንዱ በቴሌሜትሪ የሚይዝ፣
ዳሽቦርዶች፣ እና UX ቅጂ በመድረኮች ላይ ተሰልፈው ይቆያሉ።

> መጨረሻ የዘመነው: 2026-01-15  
> ባለቤት፡ ስዊፍት ኤስዲኬ መሪ (የታክሶኖሚ መጋቢ)  
> ሁኔታ፡ ስዊፍት + አንድሮይድ + ጃቫስክሪፕት አተገባበር ** አረፈ**; ወረፋ ውህደት በመጠባበቅ ላይ.

## ምድቦች

| ምድብ | ዓላማ | የተለመዱ ምንጮች |
|-------|--------|-----------------|
| `transport` | ክፍለ ጊዜን የሚያቋርጡ የዌብሶኬት/ኤችቲቲፒ የትራንስፖርት ውድቀቶች። | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | ክፈፎችን በኮድ ሲገለፅ/በመግለጽ ላይ ተከታታይ/ድልድይ አለመሳካቶች። | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | የተጠቃሚ ወይም ኦፕሬተር እርማት የሚያስፈልጋቸው TLS/ማስረጃ/የፖሊሲ ውድቀቶች። | `URLError(.secureConnectionFailed)`, Torii 4xx ምላሾች |
| `timeout` | የስራ ፈት/ከመስመር ውጭ የሚያልፍበት ጊዜ እና ጠባቂዎች (TTL ወረፋ፣ የእረፍት ጊዜ ጠይቅ)። | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | አፕሊኬሽኖች ሸክሙን በሚያምር ሁኔታ ማፍሰስ እንዲችሉ FIFO የኋላ-ግፊት ምልክቶች። | `ConnectQueueError.overflow(limit:)` |
| `internal` | የተቀረው ሁሉ፡ ኤስዲኬ አላግባብ መጠቀም፣ የጠፋ Norito ድልድይ፣ የተበላሹ መጽሔቶች። | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

እያንዳንዱ ኤስዲኬ ከታክሶኖሚ ጋር የሚስማማ እና የሚያጋልጥ የስህተት አይነት ያትማል
የተዋቀሩ የቴሌሜትሪ ባህሪያት፡ `category`፣ `code`፣ `fatal`፣ እና አማራጭ
ሜታዳታ (`http_status`፣ `underlying`)።

## ፈጣን ካርታ ስራ

ስዊፍት `ConnectError`፣ `ConnectErrorCategory` እና አጋዥ ፕሮቶኮሎችን ወደ ውጭ ይልካል።
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. ሁሉም የህዝብ ግንኙነት ስህተት
ዓይነቶች ከ `ConnectErrorConvertible` ጋር ይጣጣማሉ፣ ስለዚህ መተግበሪያዎች `error.asConnectError()` መደወል ይችላሉ።
እና ውጤቱን ወደ ቴሌሜትሪ / የሎግንግ ንብርብሮች ያስተላልፉ.| ፈጣን ስህተት | ምድብ | ኮድ | ማስታወሻ |
|------------|-------|------|-------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | ድርብ `start()` ያመለክታል; የገንቢ ስህተት. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | ከተዘጋ በኋላ ሲላክ/ሲቀበል ከፍ ይላል። |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket ሁለትዮሽ እየጠበቀ ሳለ የጽሑፍ ክፍያ አሳልፏል. |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | ተቃዋሚ ፓርቲ ሳይታሰብ ዥረቱን ዘጋው። |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | ትግበራ ሲሜትሪክ ቁልፎችን ማዋቀር ረስቷል። |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito ክፍያ የሚፈለጉ መስኮች ይጎድላሉ። |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | የወደፊት ክፍያ በአሮጌው ኤስዲኬ ታይቷል። |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito ድልድይ ጠፍቷል ወይም የፍሬም ባይት ኮድ መፍታት አልቻለም። |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | ድልድይ አይገኝም ወይም የማይዛመዱ የቁልፍ ርዝመቶች። |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | ከመስመር ውጭ የወረፋ ርዝመት ከተዋቀረው ገደብ አልፏል። |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | በ `URLSessionWebSocketTask` ተሸፍኗል። |
| `URLError` TLS ጉዳዮች | `authorization` | `network.tls_failure` | የ ATS/TLS ድርድር ውድቀቶች። |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | JSON በኤስዲኬ ውስጥ ሌላ ቦታ መግለጽ/መቀየሪያ አልተሳካም፤ መልእክት የስዊፍት ዲኮደር አውድ ይጠቀማል። |
| ሌላ ማንኛውም `Error` | `internal` | `unknown_error` | የተረጋገጠ መያዣ-ሁሉንም; የመልዕክት መስተዋቶች `LocalizedError`. |

የክፍል ሙከራዎች (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) ተቆልፈዋል
የካርታ ስራው የወደፊት ተሃድሶዎች ምድቦችን ወይም ኮዶችን በዝምታ መቀየር አይችሉም።

### ምሳሌ አጠቃቀም

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

## ቴሌሜትሪ እና ዳሽቦርዶች

ስዊፍት ኤስዲኬ `ConnectError.telemetryAttributes(fatal:httpStatus:)` ያቀርባል
ቀኖናዊውን የባህሪ ካርታ የሚመልስ። ላኪዎች እነዚህን ማስተላለፍ አለባቸው
የ `connect.error` OTEL ክስተቶች ከአማራጭ ተጨማሪዎች ጋር መለያ ባህሪያት፡-

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

ዳሽቦርዶች `connect.error` ቆጣሪዎችን ከወረፋ ጥልቀት ጋር ያዛምዳሉ (`connect.queue_depth`)
እና ምዝግብ ማስታወሻዎችን ሳያስገቡ regressions ለመለየት ሂስቶግራምን እንደገና ያገናኙ።

## አንድሮይድ ካርታ ስራየአንድሮይድ ኤስዲኬ `ConnectError`፣ `ConnectErrorCategory`፣ `ConnectErrorTelemetryOptions`፣
`ConnectErrorOptions`፣ እና የረዳት መገልገያዎች ስር
`org.hyperledger.iroha.android.connect.error`. ግንበኛ አይነት ረዳቶች ማንኛውንም `Throwable` ይለውጣሉ
ታክሶኖሚ ጋር የሚስማማ የክፍያ ጭነት፣ ከትራንስፖርት/TLS/ኮዴክ ልዩ ሁኔታዎች፣
ክፍት ቴሌሜትሪ/የናሙና ቁልሎችን እንዲፈጅ የሚወስን የቴሌሜትሪ ባህሪያትን ያጋልጡ
ውጤት ያለ ቤስፖክ አስማሚዎች።【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/ስህተት/ConnectError.java:8】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect:
`ConnectQueueError` ቀድሞውንም `ConnectErrorConvertible`ን በመተግበር ወረፋውን ከመጠን በላይ ፍሰት/ጊዜ ማብቃቱን ያሳያል።
የከመስመር ውጭ ወረፋ መሳሪያ ወደ ተመሳሳይ ፍሰት እንዲሸጋገር ለትርፍ/የሚያልቅበት ሁኔታዎች ምድቦች።【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/ስህተት/ConnectQueueError.java:5】
የአንድሮይድ ኤስዲኬ README አሁን ታክሶኖሚውን ይጠቅሳል እና የትራንስፖርት ልዩ ሁኔታዎችን እንዴት መጠቅለል እንደሚቻል ያሳያል
ቴሌሜትሪ ከማሰራጨትዎ በፊት የdApp መመሪያን ከስዊፍት መነሻ መስመር ጋር በማያያዝ።【java/iroha_android/README.md:167】

## ጃቫስክሪፕት ካርታ ስራ

Node.js/browser ደንበኞች `ConnectError`፣ `ConnectQueueError`፣ `ConnectErrorCategory` እና
`connectErrorFrom()` ከ `@iroha/iroha-js`። የተጋራው ረዳት የኤችቲቲፒ ሁኔታ ኮዶችን ይመረምራል፣
የመስቀለኛ መንገድ ስህተት ኮዶች (ሶኬት፣ ቲኤልኤስ፣ የጊዜ ማብቂያ)፣ `DOMException` ስሞች እና የኮዴክ አለመሳካቶች
በዚህ ማስታወሻ ውስጥ ተመሣሣይ ምድቦች/ኮዶች ተመዝግበው ይገኛሉ፣ የTypeScript ትርጓሜዎች ደግሞ ቴሌሜትሪ ሞዴል ናቸው።
ባህሪይ ይሽራል ስለዚህ መሳሪያ ማምረቻ በእጅ ቀረጻ ሳይደረግ የኦቲኤልን ክስተቶችን ሊያሰራጭ ይችላል።【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
ኤስዲኬ README የስራ ሂደቱን ይመዘግባል እና ከዚህ ታክሶኖሚ ጋር በማገናኘት የማመልከቻ ቡድኖች እንዲችሉ
የመሳሪያውን ቅንጥቦች በቃላት ይቅዱ።【javascript/iroha_js/README.md:1387】

## ቀጣይ ደረጃዎች (ኤስዲኬ አቋራጭ)

- **የወረፋ ውህደት፡** ከመስመር ውጭ ወረፋ አንዴ ከተላከ፣ አመክንዮ መውረድ/መጣልን ያረጋግጡ።
  ወለል `ConnectQueueError` እሴቶች ስለዚህ ከመጠን ያለፈ ቴሌሜትሪ ታማኝ ሆኖ ይቆያል።