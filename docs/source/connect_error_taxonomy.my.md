---
lang: my
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2025-12-29T18:16:35.935471+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ချိတ်ဆက်မှု Error Taxonomy (Swift အခြေခံလိုင်း)

ဤမှတ်စုသည် IOS-CONNECT-010 ကို ခြေရာခံပြီး မျှဝေထားသော error taxonomy ကို မှတ်တမ်းတင်ထားသည်။
Nexus SDK များကို ချိတ်ဆက်ပါ။ ယခု Swift သည် canonical `ConnectError` wrapper ကို တင်ပို့နေသည်၊
ချိတ်ဆက်မှုဆိုင်ရာ ချို့ယွင်းချက်အားလုံးကို အမျိုးအစားခြောက်ခုအနက်မှ တစ်ခုသို့ ပုံဖော်ပေးသောကြောင့် တယ်လီမီတာ၊
ဒက်ရှ်ဘုတ်များ၊ နှင့် UX မိတ္တူများသည် ပလပ်ဖောင်းများတစ်လျှောက် ချိန်ညှိနေပါသည်။

> နောက်ဆုံးမွမ်းမံမှု- 2026-01-15  
> ပိုင်ရှင်- Swift SDK ဦးဆောင်သူ (အကောက်ခွန်ဆိုင်ရာ ဘဏ္ဍာစိုး)  
> အခြေအနေ- Swift + Android + JavaScript အကောင်အထည်ဖော်မှုများ ** ဆင်းသက်နိုင်သည်**; တန်းစီပေါင်းစည်းမှုကို ဆိုင်းငံ့ထားသည်။

## အမျိုးအစားများ

| အမျိုးအစား | ရည်ရွယ်ချက် | ပုံမှန်ရင်းမြစ်များ |
|----------|---------|-----------------|
| `transport` | စက်ရှင်တစ်ခုအား ရပ်တန့်စေသည့် WebSocket/HTTP သယ်ယူပို့ဆောင်ရေး မအောင်မြင်မှုများ။ | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | ကုဒ်/ကုဒ်ဘောင်များ ပြုလုပ်နေစဉ် အမှတ်စဉ်/တံတား ပျက်ကွက်မှုများ။ | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | အသုံးပြုသူ သို့မဟုတ် အော်ပရေတာ ပြုပြင်မှု လိုအပ်သော TLS/အတည်ပြုချက်/မူဝါဒ ပျက်ကွက်မှုများ။ | `URLError(.secureConnectionFailed)`, Torii 4xx တုံ့ပြန်မှုများ |
| `timeout` | မလှုပ်မရှား/အော့ဖ်လိုင်း သက်တမ်းကုန်ဆုံးမှုများနှင့် စောင့်ကြည့်စစ်ဆေးမှုများ (TTL တန်းစီ၊ အချိန်ကုန်ဆုံးရန် တောင်းဆိုမှု)။ | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | FIFO back-pressure signals များသည် app များကို သာယာစွာ load လုပ်နိုင်သည်။ | `ConnectQueueError.overflow(limit:)` |
| `internal` | အခြားအရာအားလုံး- SDK အလွဲသုံးစားလုပ်မှု၊ Norito တံတား ပျောက်ဆုံးမှု၊ ပျက်စီးနေသော ဂျာနယ်များ။ | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

SDK တိုင်းသည် အခွန်စည်းကြပ်ခြင်းနှင့် ကိုက်ညီသော အမှားအမျိုးအစားတစ်ခုကို ထုတ်ဝေသည်။
ဖွဲ့စည်းတည်ဆောက်ထားသော တယ်လီမီတာ ရည်ညွှန်းချက်များ- `category`၊ `code`၊ `fatal` နှင့် ရွေးချယ်နိုင်သည်
မက်တာဒေတာ (`http_status`၊ `underlying`)။

## လျင်မြန်သောမြေပုံဆွဲခြင်း။

Swift သည် `ConnectError`၊ `ConnectErrorCategory` နှင့် helper protocol များ
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`။ အများသူငှာချိတ်ဆက်မှုအားလုံး အမှားအယွင်း
အမျိုးအစားများသည် `ConnectErrorConvertible` နှင့် ကိုက်ညီသောကြောင့် အပလီကေးရှင်းများက `error.asConnectError()` သို့ခေါ်ဆိုနိုင်သည်
ရလဒ်အား telemetry/logging အလွှာများသို့ ပေးပို့ပါ။| လျင်မြန်သောအမှား | အမျိုးအစား | ကုတ် | မှတ်စုများ |
|-------------|----------|------|------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | နှစ်ဆ `start()` ကိုညွှန်ပြသည်; developer အမှား။ |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | ပိတ်ပြီးနောက် ပေးပို့/လက်ခံသည့်အခါတွင် တိုးသည်။ |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket သည် binary ကိုမျှော်လင့်နေချိန်တွင် textual payload ကိုပေးပို့ခဲ့သည်။ |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | Counterparty သည် ထုတ်လွှင့်မှုကို မမျှော်လင့်ဘဲ ပိတ်လိုက်ပါသည်။ |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | အပလီကေးရှင်းသည် အချိုးညီသောသော့များကို စီစဉ်သတ်မှတ်ရန် မေ့သွားခဲ့သည်။ |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito လိုအပ်သောအကွက်များ ပျောက်ဆုံးနေပါသည်။ |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | SDK အဟောင်းမှ မြင်တွေ့ရသော အနာဂတ် payload |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito တံတားပျောက်နေသည် သို့မဟုတ် ဘောင်ဘိုက်များကို ကုဒ်/ကုဒ်လုပ်ရန် မအောင်မြင်ပါ။ |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | တံတားမရရှိနိုင်ပါ သို့မဟုတ် သော့အရှည်များ မကိုက်ညီပါ။ |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | အော့ဖ်လိုင်းတန်းစီခြင်း အရှည်သည် ပြင်ဆင်သတ်မှတ်ထားသော ဘောင်ကို ကျော်လွန်သွားပါသည်။ |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | `URLSessionWebSocketTask` ဖြင့် ဖုံးအုပ်ထားသည်။ |
| `URLError` TLS အမှုတွဲ | `authorization` | `network.tls_failure` | ATS/TLS ညှိနှိုင်းမှု မအောင်မြင်ပါ။ |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | SDK ရှိ အခြားနေရာများတွင် JSON ကုဒ်ဖြင့် ကုဒ်လုပ်ခြင်း/ကုဒ်လုပ်ခြင်း မအောင်မြင်ပါ။ မက်ဆေ့ဂျ်သည် Swift ဒီကုဒ်ဒါဆိုင်ရာ အကြောင်းအရာကို အသုံးပြုသည်။ |
| တခြား `Error` | `internal` | `unknown_error` | အာမခံချက်-အားလုံးဖမ်း။ မက်ဆေ့ဂျ်သည် `LocalizedError` ဖြစ်သည်။ |

ယူနစ်စမ်းသပ်မှုများ (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) လော့ခ်ချခြင်း။
အနာဂတ် refactors များသည် အမျိုးအစားများ သို့မဟုတ် ကုဒ်များကို တိတ်တဆိတ် မပြောင်းလဲနိုင်စေရန် မြေပုံဆွဲခြင်း။

### ဥပမာ အသုံးပြုပုံ

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

## တယ်လီမီတာနှင့် ဒက်ရှ်ဘုတ်များ

Swift SDK သည် `ConnectError.telemetryAttributes(fatal:httpStatus:)` ကိုထောက်ပံ့ပေးသည်။
၎င်းသည် canonical attribute မြေပုံကို ပြန်ပေးသည်။ တင်ပို့သူတွေက ဒါတွေကို ပို့သင့်တယ်။
ရွေးချယ်နိုင်ဖွယ်ရာ အပိုများပါရှိသော `connect.error` OTEL အစီအစဉ်များတွင် ရည်ညွှန်းချက်များ-

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

Dashboards များသည် `connect.error` ကောင်တာများကို တန်းစီခြင်းအတိမ်အနက် (`connect.queue_depth`) နှင့် ဆက်စပ်နေသည်
နှင့် မှတ်တမ်းများကို စာလုံးပေါင်းခြင်းမရှိဘဲ ဆုတ်ယုတ်မှုများကို သိရှိနိုင်ရန် ဟီစတိုဂရမ်များကို ပြန်လည်ချိတ်ဆက်ပါ။

## Android မြေပုံဆွဲခြင်း။Android SDK သည် `ConnectError`, `ConnectErrorCategory`, `ConnectErrorTelemetryOptions`၊
`ConnectErrorOptions` နှင့် helper utilities အောက်တွင်
`org.hyperledger.iroha.android.connect.error`။ Builder-style helpers များသည် `Throwable` တစ်ခုခုကို ပြောင်းသည်။
taxonomy-compliant payload တစ်ခုသို့၊ သယ်ယူပို့ဆောင်ရေး/TLS/codec ခြွင်းချက်များမှ အမျိုးအစားများကို ကောက်နုတ်ခြင်း၊
OpenTelemetry/sampling stacks များကို စားသုံးနိုင်စေရန် အဆုံးအဖြတ်ပေးသော တယ်လီမီတာ၏ အရည်အချင်းများကို ဖော်ထုတ်ပါ။
စိတ်ကြိုက်အဒက်တာများမပါဘဲ ရလဒ်များ။
`ConnectQueueError` သည် `ConnectErrorConvertible` ကို အကောင်အထည် ဖော်ထားပြီး၊ တန်းစီခြင်းOverflow/timeout ကို ထုတ်လွှတ်သည်
ပြည့်လျှံခြင်း/သက်တမ်းကုန်ဆုံးမှုအခြေအနေများအတွက် အမျိုးအစားများ အော့ဖ်လိုင်းတန်းစီခြင်းစနစ်သည် တူညီသောစီးဆင်းမှုသို့ သွယ်တန်းသွားနိုင်သည်။【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
ယခုအခါ Android SDK README သည် အခွန်စည်းကြပ်ခြင်းကို ကိုးကားပြီး သယ်ယူပို့ဆောင်ရေးခြွင်းချက်များကို မည်သို့ခြုံငုံသုံးသပ်ရမည်ကို ပြသထားသည်။
တယ်လီမီတာမထုတ်မီ၊ dApp လမ်းညွှန်ချက်ကို Swift အခြေခံလိုင်းနှင့် လိုက်လျောညီထွေဖြစ်အောင် ထိန်းသိမ်းပါ။【java/iroha_android/README.md:167】

## JavaScript မြေပုံဆွဲခြင်း။

Node.js/browser clients များသည် `ConnectError`၊ `ConnectQueueError`၊ `ConnectErrorCategory` နှင့်
`connectErrorFrom()` မှ `@iroha/iroha-js`။ မျှဝေပေးသူသည် HTTP အခြေအနေကုဒ်များကို စစ်ဆေးသည်၊
Node အမှားကုဒ်များ (socket၊ TLS၊ အချိန်လွန်)၊ `DOMException` အမည်များနှင့် codec များကို ထုတ်လွှတ်ရန် ပျက်ကွက်မှုများ၊
TypeScript အဓိပ္ပါယ်ဖွင့်ဆိုချက်များသည် telemetry ကို နမူနာယူထားသော်လည်း ဤမှတ်စုတွင် မှတ်တမ်းတင်ထားသော တူညီသောအမျိုးအစား/ကုဒ်များ
ရည်ညွှန်းချက်သည် ကိရိယာတန်ဆာပလာများကို ကိုယ်တိုင်ကာစ်လုပ်ခြင်းမပြုဘဲ OTEL ဖြစ်ရပ်များကို ထုတ်လွှတ်နိုင်စေရန် ရည်ညွှန်းချက်အား လွှမ်းမိုးထားသည်။ 【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
SDK README သည် အလုပ်အသွားအလာကို မှတ်တမ်းတင်ပြီး ဤအစီအစဥ်ကို ပြန်လည်ချိတ်ဆက်ပေးခြင်းဖြင့် လျှောက်လွှာအဖွဲ့များ ဆောင်ရွက်နိုင်မည်ဖြစ်သည်။
စာရွက်စာတမ်းအတိုအထွာများကို စကားလုံးအတိုအထွာများကို ကူးယူပါ။【javascript/iroha_js/README.md:1387】

## နောက်အဆင့်များ (SDK ဖြတ်ကျော်)

- **တန်းစီပေါင်းစည်းခြင်း-** အော့ဖ်လိုင်းတန်းစီသင်္ဘောများ တက်လာသည်နှင့် တစ်ပြိုင်နက်၊ dequeue/drop logic ကို သေချာစစ်ဆေးပါ။
  `ConnectQueueError` တန်ဖိုးများကို မျက်နှာပြင်များပေါ်တွင် ပြည့်လျှံနေသော Telemetry သည် ယုံကြည်စိတ်ချရဆဲဖြစ်သည်။