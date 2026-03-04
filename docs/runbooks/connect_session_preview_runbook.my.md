---
lang: my
direction: ltr
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d04af2ad3ae5cc6a9254236f5627850aab7c6517308e9f3a09650cbc1490168
source_last_modified: "2026-01-05T18:22:23.401292+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Session Preview Runbook (IOS7/JS4) ကို ချိတ်ဆက်ပါ

ဤစာစုသည် အဆင့်လိုက်၊ မှန်ကန်ကြောင်းနှင့်၊
လမ်းပြမြေပုံမှတ်တိုင်များ **IOS7** မှ လိုအပ်သည့်အတိုင်း Connect preview sessions များကို ဖြိုဖျက်ပါ
နှင့် **JS4** (`roadmap.md:1340`၊ `roadmap.md:1656`)။ ဤအဆင့်များကို အချိန်တိုင်းလိုက်နာပါ။
သင်သည် Connect strawman (`docs/source/connect_architecture_strawman.md`) ကိုသရုပ်ပြသည်။
SDK လမ်းပြမြေပုံတွင် ကတိပြုထားသော တန်းစီခြင်း/ကြေးနန်းချိတ်များကို လေ့ကျင့်ပါ သို့မဟုတ် စုဆောင်းပါ။
`status.md` အတွက် အထောက်အထား။

## 1. Preflight စစ်ဆေးရန်စာရင်း

| ပစ္စည်း | အသေးစိတ် | ကိုးကား |
|------|---------|------------|
| Torii အဆုံးမှတ် + ချိတ်ဆက်မှုမူဝါဒ | Torii အခြေခံ URL၊ `chain_id` နှင့် ချိတ်ဆက်မှုမူဝါဒ (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`) ကို အတည်ပြုပါ။ runbook လက်မှတ်တွင် JSON လျှပ်တစ်ပြက်ရိုက်ပါ။ | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| Fixture + တံတားဗားရှင်းများ | သင်အသုံးပြုမည့် Norito fixture hash and bridge build ကို သတိပြုပါ (Swift သည် `NoritoBridge.xcframework` လိုအပ်သည်၊ JS သည် `@iroha/iroha-js` ≥ `bootstrapConnectPreviewSession` လိုအပ်သည်)။ | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| Telemetry ဒက်ရှ်ဘုတ်များ | `connect.queue_depth`၊ `connect.queue_overflow_total`၊ `connect.resume_latency_ms`၊ `swift.connect.session_event` စသည်ဖြင့် လက်လှမ်းမီနိုင်သည် (Grafana Norito နှင့် ထုတ်ယူထားသော ဘုတ်ပြားများပါသည့် ဒက်ရှ်ဘုတ်များ Prometheus ဓာတ်ပုံများ)။ | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| ဖိုင်တွဲများ | `docs/source/status/swift_weekly_digest.md` (အပတ်စဉ်အကျဉ်းချုပ်) နှင့် `docs/source/sdk/swift/connect_risk_tracker.md` (risk tracker) ကဲ့သို့သော ဦးတည်ရာတစ်ခုကို ရွေးပါ။ `docs/source/sdk/swift/readiness/archive/<date>/connect/` အောက်တွင် မှတ်တမ်းများ၊ တိုင်းတာချက်များ ဖန်သားပြင်ဓာတ်ပုံများနှင့် အသိအမှတ်ပြုချက်များကို သိမ်းဆည်းပါ။ | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. Preview Session ကို Bootstrap လုပ်ပါ။

1. **မူဝါဒ + ခွဲတမ်းကို အတည်ပြုပါ။** ခေါ်ဆိုရန်-
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   `queue_max` သို့မဟုတ် TTL သည် သင်စီစဉ်ထားသည့် config နှင့် ကွဲပြားပါက လည်ပတ်မှု မအောင်မြင်ပါ။
   စမ်းသပ်။
2. **အဆုံးအဖြတ်ပေးသော SID/URI များကို ဖန်တီးပါ။** `@iroha/iroha-js`
   `bootstrapConnectPreviewSession` အကူအညီပေးသူက SID/URI မျိုးဆက် Torii သို့ ချိတ်ဆက်သည်
   session မှတ်ပုံတင်ခြင်း; Swift သည် WebSocket အလွှာကို မောင်းနှင်သည့်အခါတွင်ပင် ၎င်းကို အသုံးပြုပါ။
   ```js
   import {
     ToriiClient,
     bootstrapConnectPreviewSession,
   } from "@iroha/iroha-js";

   const client = new ToriiClient(process.env.TORII_BASE_URL, { chainId: "sora-mainnet" });
   const { preview, session, tokens } = await bootstrapConnectPreviewSession(client, {
     chainId: "sora-mainnet",
     appBundle: "dev.sora.example.dapp",
     walletBundle: "dev.sora.example.wallet",
     register: true,
   });
   console.log("sid", preview.sidBase64Url, "ws url", preview.webSocketUrl);
   ```
   - ခြောက်သွေ့သော QR/deep-link မြင်ကွင်းများဆီသို့ `register: false` ကို သတ်မှတ်ပါ။
   - ပြန်ပေးထားသော `sidBase64Url`၊ deeplink URLs နှင့် `tokens` blob တို့ကို ဆက်ထားပါ
     အထောက်အထားဖိုင်တွဲ; အုပ်ချုပ်မှုပြန်လည်သုံးသပ်ခြင်းသည် ဤအရာများကို မျှော်လင့်ပါသည်။
3. **လျှို့ဝှက်ချက်များကို ဖြန့်ဝေပါ။** wallet operator နှင့် deeplink URI ကို မျှဝေပါ။
   (လျင်မြန်သော dApp နမူနာ၊ Android ပိုက်ဆံအိတ် သို့မဟုတ် QA ကြိုး)။ တိုကင်အစိမ်းကို ဘယ်တော့မှ မထည့်ပါနဲ့။
   စကားပြောခန်းထဲသို့; enablement packet တွင် မှတ်တမ်းတင်ထားသော ကုဒ်ဝှက်ထားသော တဲကို အသုံးပြုပါ။

## 3. Session ကို မောင်းနှင်ပါ။1. **WebSocket ကိုဖွင့်ပါ။** Swift clients များသည် ပုံမှန်အားဖြင့်-
   ```swift
   let connectURL = URL(string: preview.webSocketUrl)!
   let client = ConnectClient(url: connectURL)
   let sid: Data = /* decode preview.sidBase64Url into raw bytes using your harness helper */
   let session = ConnectSession(sessionID: sid, client: client)
   let recorder = ConnectReplayRecorder(sessionID: sid)
   session.addObserver(ConnectEventObserver(queue: .main) { event in
       logger.info("connect event", metadata: ["kind": "\(event.kind)"])
   })
   try client.open()
   ```
   ထပ်လောင်းထည့်သွင်းမှုအတွက် အကိုးအကား `docs/connect_swift_integration.md` (တံတား
   တင်သွင်းမှုများ၊ တူညီသော အဒက်တာများ)။
2. **Approve + sign flows.** DApps ခေါ်ဆိုမှု `ConnectSession.requestSignature(...)`၊
   ပိုက်ဆံအိတ်များသည် `approveSession` / `reject` မှတစ်ဆင့် တုံ့ပြန်နေစဉ်။ အတည်ပြုချက်တစ်ခုစီတိုင်းတွင် မှတ်တမ်းရှိရမည်။
   ချိတ်ဆက်အုပ်ချုပ်မှု ပဋိဉာဉ်နှင့် ကိုက်ညီရန် hashed alias + ခွင့်ပြုချက်များ။
3. **လေ့ကျင့်ခန်းတန်းစီခြင်း + လမ်းကြောင်းများကို ပြန်လည်စတင်ပါ။** ကွန်ရက်ချိတ်ဆက်မှုကို ပြောင်းရန် သို့မဟုတ် ဆိုင်းငံ့ရန်
   ကန့်သတ်ထားသော တန်းစီခြင်းကို သေချာစေရန် ပိုက်ဆံအိတ်နှင့် ချိတ်များ မှတ်တမ်းများကို ပြန်ဖွင့်ပါ။ JS/Android
   SDK များသည် `ConnectQueueError.overflow(limit)` ကို ထုတ်လွှတ်သည် /
   ဘောင်များချသောအခါ `.expired(ttlMs)`၊ Swift သည် တစ်ကြိမ်တည်းကို သတိပြုသင့်သည်။
   IOS7 တန်းစီငြမ်းမြေများ (`docs/source/connect_architecture_strawman.md`)။
   သင်သည် အနည်းဆုံး ပြန်လည်ချိတ်ဆက်မှုတစ်ခုကို မှတ်တမ်းတင်ပြီးနောက် လုပ်ဆောင်ပါ။
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (သို့မဟုတ် `ConnectSessionDiagnostics` ဖြင့် ပြန်ပေးသော ပို့ကုန်လမ်းညွှန်ကို ကျော်ဖြတ်ပါ) နှင့်
   ပြန်ဆိုထားသောဇယား/JSON ကို runbook လက်မှတ်တွင် ပူးတွဲပါ။ CLI က ဒီလိုပဲ ဖတ်တယ်။
   `state.json` / `metrics.ndjson` အတွဲ `ConnectQueueStateTracker` ထုတ်လုပ်သည်၊
   ထို့ကြောင့် အုပ်ချုပ်မှုပြန်လည်သုံးသပ်သူများသည် စိတ်ကြိုက်ကိရိယာမပါဘဲ တူးဖေါ်သောအထောက်အထားများကို ခြေရာခံနိုင်သည်။

## 4. Telemetry & Observability

- **ဖမ်းယူရန် မက်ထရစ်များ-**
  - `connect.queue_depth{direction}` gauge (မူဝါဒထုပ်အောက်တွင်ရှိနေသင့်သည်)။
  - `connect.queue_dropped_total{reason="overflow|ttl"}` ကောင်တာ (သုညမဟုတ်သော သီးသန့်
    မှားယွင်းထိုးသွင်းနေစဉ်)။
  - `connect.resume_latency_ms` histogram (အတင်းအကျပ်ခိုင်းပြီးနောက် p95 ကို မှတ်တမ်းတင်ပါ။
    ပြန်လည်ချိတ်ဆက်ပါ။)
  - `connect.replay_success_total`/`connect.replay_error_total`။
  - Swift-တိကျသော `swift.connect.session_event` နှင့်
    `swift.connect.frame_latency` တင်ပို့မှု (`docs/source/sdk/swift/telemetry_redaction.md`)။
- **ဒိုင်ခွက်များ-** မှတ်ချက်အမှတ်အသားများဖြင့် ချိတ်ဆက်ဘုတ်ခ်များကို အပ်ဒိတ်လုပ်ပါ။
  ဖန်သားပြင်ဓာတ်ပုံများ (သို့မဟုတ် JSON ထုတ်ယူမှုများ) ကို အကြမ်းထည်နှင့်အတူ အထောက်အထားဖိုင်တွဲတွင် ပူးတွဲပါ။
  OTLP/Prometheus တယ်လီမီတာတင်ပို့သူ CLI မှတစ်ဆင့် ဆွဲယူထားသော လျှပ်တစ်ပြက်ဓာတ်ပုံများ။
- **သတိပေးချက်-** မည်သည့် Sev1/2 သတ်မှတ်ချက်များ အစပျိုးပါက (`docs/source/android_support_playbook.md` §5)၊
  SDK ပရိုဂရမ် ဦးဆောင်သည့် စာမျက်နှာတွင် PagerDuty အဖြစ်အပျက် ID ကို runbook တွင် မှတ်တမ်းတင်ပါ။
  လက်မှတ်မဆက်မီ။

## 5. Cleanup & Rollback

1. **အဆင့်သတ်မှတ်ထားသော ဆက်ရှင်များကို ဖျက်ပါ။** အကြိုကြည့်ရှုသည့် ဆက်ရှင်များကို အမြဲတမ်း ဖျက်ပါ ထို့ကြောင့် တန်းစီခြင်း၏ အနက်
   နှိုးစက်များသည် အဓိပ္ပါယ်ရှိနေဆဲဖြစ်သည်-
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   Swift-only test runs အတွက် Rust/CLI helper မှတဆင့် တူညီသော endpoint ကို ခေါ်ဆိုပါ။
2. **ဂျာနယ်များကို ဖယ်ရှားပါ။** တန်းစီနေသော ဂျာနယ်များကို ဖယ်ရှားပါ။
   (`ApplicationSupport/ConnectQueue/<sid>.to`၊ IndexedDB စတိုးဆိုင်များ စသည်ဖြင့်)
   နောက်ပြေးတာက သန့်ရှင်းတယ်။ လိုအပ်ပါက မဖျက်မီ ဖိုင် hash ကို မှတ်တမ်းတင်ပါ။
   ပြန်ဖွင့်သည့် ပြဿနာကို အမှားရှာပါ။
3. ** ဖိုင်ဖြစ်ရပ်မှတ်စုများ။** လည်ပတ်မှုကို အကျဉ်းချုပ်ဖော်ပြပါ-
   - `docs/source/status/swift_weekly_digest.md` (မြစ်ဝကျွန်းပေါ်ရပ်ကွက်)၊
   - `docs/source/sdk/swift/connect_risk_tracker.md` (CR-2 ကို ရှင်းပါ သို့မဟုတ် အဆင့်နှိမ့်ချပါ။
     တယ်လီမီတာ တခါတည်း ၊
   - အမူအကျင့်အသစ်ကိုအတည်ပြုပါက JS SDK ပြောင်းလဲမှုမှတ်တမ်း သို့မဟုတ် စာရွက်။
4. **မအောင်မြင်မှုများကို တိုးမြင့်စေသည်-**
   - ထိုးသွင်းအမှားအယွင်းမရှိဘဲ တန်းစီပြည့်လျှံနေသော ⇒ ချွတ်ယွင်းချက်တစ်ခုကို SDK တွင်တင်ပါ။
     မူဝါဒသည် Torii မှ ကွဲပြားသည်။
   - အမှားများကို ပြန်လည်စတင်ရန် ⇒ `connect.queue_depth` + `connect.resume_latency_ms` ပူးတွဲပါ
     ဖြစ်စဉ်မှတ်တမ်းဓာတ်ပုံများ။
   - အုပ်ချုပ်မှုမတူညီမှုများ (တိုကင်များကို ပြန်သုံးသည်၊ TTL ကျော်လွန်သွားသည်) ⇒ SDK ဖြင့် မြှင့်တင်ခြင်း
     ပရိုဂရမ်ကို ဦးဆောင်ပြီး `roadmap.md` ကို နောက်တစ်ကြိမ် ပြင်ဆင်မှုအတွင်း မှတ်သားပါ။

## 6. အထောက်အထားစစ်ဆေးမှုစာရင်း| Artefact | တည်နေရာ |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| ဒက်ရှ်ဘုတ် တင်ပို့မှု (`connect.queue_depth`, etc.) | `.../metrics/` ဖိုင်တွဲခွဲ |
| PagerDuty / အဖြစ်အပျက် IDs | `.../notes.md` |
| ရှင်းလင်းချက်အတည်ပြုချက် (Torii ဖျက်ရန်၊ ဂျာနယ်ရှင်းလင်းခြင်း) | `.../cleanup.log` |

ဤစစ်ဆေးစာရင်းကို ဖြည့်သွင်းခြင်းသည် “docs/runbooks အပ်ဒိတ်လုပ်ထားသော” ထွက်ပေါက်သတ်မှတ်ချက်ကို ကျေနပ်စေသည်။
IOS7/JS4 အတွက် အုပ်ချုပ်မှုပြန်လည်သုံးသပ်သူများကို လူတိုင်းအတွက် အဆုံးအဖြတ်ပေးသည့်လမ်းကြောင်းကို ပေးသည်။
အစမ်းကြည့်ရှုခြင်းကဏ္ဍကို ချိတ်ဆက်ပါ။