---
lang: my
direction: ltr
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-12-29T18:16:35.913527+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Chaos & Fault Rehearsal Plan (IOS3 / IOS7) ကို ချိတ်ဆက်ပါ။

ဤပြခန်းစာအုပ်သည် IOS3/IOS7 ကို ကျေနပ်စေသည့် ထပ်ခါတလဲလဲ ပရမ်းပတာလေ့ကျင့်မှုများကို သတ်မှတ်ဖော်ပြပါသည်။
လမ်းပြမြေပုံလုပ်ဆောင်ချက် _“ပူးတွဲမငြိမ်မသက် အစမ်းလေ့ကျင့်မှု အစီအစဉ်”_ (`roadmap.md:1527`)။ ၎င်းကိုတွဲပါ။
ချိတ်ဆက်မှု အကြိုကြည့်ရှုမှု ပြေးစာအုပ် (`docs/runbooks/connect_session_preview_runbook.md`)
Cross-SDK သရုပ်ပြများကို အဆင့်မြှင့်တင်သည့်အခါ။

## ရည်မှန်းချက် နှင့် အောင်မြင်မှု သတ်မှတ်ချက်
- မျှဝေထားသော ချိတ်ဆက်မှု ပြန်လည်ကြိုးစားခြင်း/နောက်ပြန်ပိတ်ခြင်း မူဝါဒ၊ အော့ဖ်လိုင်း တန်းစီခြင်း ကန့်သတ်ချက်တို့ကို ကျင့်သုံးပါ။
  တယ်လီမီတာပို့ကုန်များသည် ထုတ်လုပ်မှုကုဒ်ကို ပြောင်းလဲခြင်းမရှိဘဲ ထိန်းချုပ်ထားသော ချို့ယွင်းချက်များအောက်တွင် ရှိနေသည်။
- အဆုံးအဖြတ်ပေးသော ပစ္စည်းများကို ဖမ်းယူပါ (`iroha connect queue inspect` အထွက်၊
  `connect.*` မက်ထရစ်များ လျှပ်တစ်ပြက်ရိုက်ချက်များ၊ Swift/Android/JS SDK မှတ်တမ်းများ) ထို့ကြောင့် အုပ်ချုပ်မှု လုပ်နိုင်သည်
  လေ့ကျင့်မှုတိုင်းကို စစ်ဆေးပါ။
- ပိုက်ဆံအိတ်များနှင့် dApps များသည် config အပြောင်းအလဲများကို ဂုဏ်ပြုကြောင်း သက်သေပြပါ (ပျံ့လွင့်မှုများ၊ ဆား
  Canonical `ConnectError` ကို ကျော်လွှားခြင်းဖြင့် လည်ပတ်ခြင်း၊ သက်သေမအောင်မြင်ခြင်း)
  အမျိုးအစားနှင့် တုံ့ပြန်ခြင်း-ဘေးကင်းသော တယ်လီမီတာ ဖြစ်ရပ်များ။

## လိုအပ်ချက်များ
1. **ပတ်ဝန်းကျင် bootstrap**
   - ဒီမို Torii စတက်ခ်- `scripts/ios_demo/start.sh --telemetry-profile full` ကို စတင်ပါ။
   - အနည်းဆုံး SDK နမူနာ (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`၊
     `examples/ios/NoritoDemo`၊ Android `demo-connect`၊ JS `examples/connect`)။
2. **တူရိယာ**
   - SDK ရောဂါရှာဖွေမှုများကို ဖွင့်ပါ (`ConnectQueueDiagnostics`၊ `ConnectQueueStateTracker`၊
     Swift တွင် `ConnectSessionDiagnostics`; `ConnectQueueJournal` + `ConnectQueueJournalTests`
     Android/JS တွင် ညီမျှသည်)။
   - CLI `iroha connect queue inspect --sid <sid> --metrics` ဖြေရှင်းကြောင်း သေချာပါစေ။
     SDK (`~/.iroha/connect/<sid>/state.json` နှင့်
     `metrics.ndjson`)။
   - ဝါယာကြိုး တယ်လီမီတာ တင်ပို့သူများ ဖြစ်သောကြောင့် အောက်ပါ အချိန်စီးရီးများကို မြင်နိုင်သည်
     Grafana နှင့် `scripts/swift_status_export.py telemetry`: `connect.queue_depth`၊
     `connect.queue_dropped_total`, `connect.reconnects_total`၊
     `connect.resume_latency_ms`, `swift.connect.frame_latency`၊
     `android.telemetry.redaction.salt_version`။
3. **အထောက်အထားဖိုင်တွဲများ** – `artifacts/connect-chaos/<date>/` ကို ဖန်တီးပြီး သိမ်းဆည်းပါ-
   - ကုန်ကြမ်းမှတ်တမ်းများ (`*.log`)၊ မက်ထရစ်များ လျှပ်တစ်ပြက်ရိုက်ချက်များ (`*.json`)၊ ဒက်ရှ်ဘုတ် တင်ပို့မှု
     (`*.png`)၊ CLI အထွက်များနှင့် PagerDuty ID များ။

## Scenario Matrix| ID | ပြတ်ရွေ့ | ဆေးထိုးအဆင့် | မျှော်လင့်ထားသောအချက်များ | အထောက်အထား |
|----|-------|-----------------|-----------------|----------------|
| C1 | WebSocket ပြတ်တောက်ပြီး ပြန်လည်ချိတ်ဆက် | `/v1/connect/ws` ကို ပရောက်စီတစ်ခု၏ နောက်ကွယ်တွင် ခြုံပါ (ဥပမာ၊ `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) သို့မဟုတ် ဝန်ဆောင်မှုကို ယာယီပိတ်ပါ (`kubectl scale deploy/torii --replicas=0` အတွက် ≤60s)။ ပိုက်ဆံအိတ်ကို ဘောင်များ ဆက်ပို့ရန် အတင်းအကြပ် အော့ဖ်လိုင်း တန်းစီခြင်းများကို ဖြည့်ပါ။ | `connect.reconnects_total` တိုးမှုများ၊ `connect.resume_latency_ms` တိုးသွားသော်လည်း - ပြတ်တောက်မှုဝင်းဒိုးအတွက် ဒက်ရှ်ဘုတ်မှတ်ချက်။- ပြန်လည်ချိတ်ဆက်မှု + ယိုထုတ်သည့်စာတိုများပါသည့် နမူနာမှတ်တမ်း ကောက်နုတ်ချက်။ |
| C2 | အော့ဖ်လိုင်းတန်းစီလျှံမှု / TTL သက်တမ်းကုန် | တန်းစီကန့်သတ်ချက်များကို လျှော့ချရန် နမူနာကို ဖာထေးပါ (Swift- `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` တွင် `ConnectSessionDiagnostics`၊ Android/JS နှင့် သက်ဆိုင်သော တည်ဆောက်မှုများကို အသုံးပြုသည်)။ dApp သည် တောင်းဆိုမှုများကို တန်းစီစောင့်ဆိုင်းနေချိန်တွင် ≥2× `retentionInterval` အတွက် ပိုက်ဆံအိတ်ကို ဆိုင်းငံ့လိုက်ပါ။ | `connect.queue_dropped_total{reason="overflow"}` နှင့် `{reason="ttl"}` တိုးခြင်း၊ `connect.queue_depth` ကန့်သတ်ချက်အသစ်တွင် ကုန်းပြင်မြင့်၊ SDKs မျက်နှာပြင် `ConnectError.QueueOverflow(limit: 4)` (သို့မဟုတ် `.QueueExpired`)။ `iroha connect queue inspect` သည် `state=Overflow` နှင့် `warn/drop` ရေစာများကို 100% ပြသည်။ | - မက်ထရစ်ကောင်တာများ၏ ဖန်သားပြင်ဓာတ်ပုံ။- CLI JSON အထွက်နှုန်း ပြည့်လျှံမှုကို ဖမ်းယူခြင်း။- `ConnectError` လိုင်းပါရှိသော Swift/Android မှတ်တမ်း အတိုအထွာ။ |
| C3 | ပျံ့ / ဝန်ခံချက်ပယ်ချကြောင်းဖော်ပြ | Connect manifest ကို ပိုက်ဆံအိတ်များတွင် ဆောင်ရွက်ပေးခဲ့သည် (ဥပမာ၊ `docs/connect_swift_ios.md` နမူနာ မန်နီးဖက်စ်ကို မွမ်းမံပါ၊ သို့မဟုတ် Torii ဖြင့် `--connect-manifest-path` ဖြင့် `chain_id` သို့မဟုတ် `--connect-manifest-path` ဖြင့် စတင်ပါ) diff. dApp ၏ တောင်းဆိုချက်ကို ခွင့်ပြုချက်ရယူပြီး မူဝါဒမှတစ်ဆင့် ပိုက်ဆံအိတ်ကို ငြင်းပယ်ကြောင်း သေချာပါစေ။ | Torii သည် `HTTP 409` အတွက် `/v1/connect/session` နှင့် `manifest_mismatch`၊ SDKs သည် `ConnectError.Authorization.manifestMismatch(manifestVersion)` ကို ထုတ်လွှတ်သည်၊ တယ်လီမီတာက Grafana ကို မြှင့်ပေးသည် (18NI00000077X) အချည်းနှီးဖြစ်သည် (01) တန်းစီကျန်နေပါသည်။ | - Torii မှတ်တမ်း ကောက်နုတ်ချက် မကိုက်ညီမှု ထောက်လှမ်းမှုကို ပြသသည်။- ပေါ်လာသည့် အမှား၏ SDK ဖန်သားပြင်ဓာတ်ပုံ။- စမ်းသပ်မှုအတွင်း တန်းစီထားသောဘောင်များ မရှိကြောင်း မက်ထရစ် လျှပ်တစ်ပြက် ရိုက်ချက်။ |
| C4 | သော့လည်ပတ်/ဆား-ဗားရှင်းအဖု| ချိတ်ဆက်ဆား သို့မဟုတ် AEAD သော့ကို ချိတ်ဆက်မှုအလယ်တွင် လှည့်ပါ။ dev stacks တွင်၊ Torii ကို `CONNECT_SALT_VERSION=$((old+1))` ဖြင့် ပြန်လည်စတင်ပါ (`docs/source/sdk/android/telemetry_schema_diff.md` တွင် Android redaction ဆားစမ်းသပ်မှုကို မှန်ကြည့်သည်)။ ဆားလည်ပတ်မှု မပြီးမချင်း ပိုက်ဆံအိတ်ကို အော့ဖ်လိုင်းမှာထားပါ၊ ထို့နောက် ပြန်စပါ။ | `ConnectError.Authorization.invalidSalt` ဖြင့် ပထမအကြိမ် ပြန်လည်စတင်ရန် ကြိုးပမ်းမှု မအောင်မြင်ပါ၊ တန်းစီနေသော flush (dApp သည် အကြောင်းပြချက် `salt_version_mismatch` ဖြင့် ကက်ရှ်ဘောင်များကို ချလိုက်သည်)၊ တယ်လီမီတာမှ `android.telemetry.redaction.salt_version` (Android) နှင့် `swift.connect.session_event{event="salt_rotation"}` တို့ကို ထုတ်ပေးပါသည်။ SID ပြန်လည်စတင်ပြီးနောက် ဒုတိယစက်ရှင် အောင်မြင်သည်။ | - ဆားခေတ်မတိုင်မီ/နောက်တွင် ပါရှိသည့် ဒက်ရှ်ဘုတ်မှတ်ချက်။- မမှန်ကန်သော ဆားအမှားနှင့် နောက်ဆက်တွဲအောင်မြင်မှု ပါဝင်သော မှတ်တမ်းများ။- `iroha connect queue inspect` အထွက်တွင် `state=Stalled` နှင့် နောက်တွင် အသစ်အသစ်သော `state=Active` ကို ပြသထားသည်။ || C5 | ထောက်ခံချက် / StrongBox ရှုံးနိမ့် | Android ပိုက်ဆံအိတ်များတွင် `ConnectApproval` ကို `attachments[]` + StrongBox အထောက်အထားထည့်သွင်းရန် စီစဉ်သတ်မှတ်ပါ။ အထောက်အထားကြိုးကြိုး (`scripts/android_keystore_attestation.sh` နှင့် `--inject-failure strongbox-simulated`) ကိုသုံးပါ သို့မဟုတ် dApp သို့မလွှဲပြောင်းမီ သက်သေခံချက် JSON ကို ချိုးဖျက်ပါ။ | DApp သည် `ConnectError.Authorization.invalidAttestation` ဖြင့် အတည်ပြုချက်ကို ပယ်ချသည်၊ Torii သည် ပျက်ကွက်ရသည့်အကြောင်းရင်းကို မှတ်တမ်းတင်ပြီး၊ ပို့ကုန်လုပ်ငန်းရှင်များက `connect.attestation_failed_total` ကို ထိမှန်ကာ တန်းစီသည် ချိုးဖောက်ဝင်ရောက်မှုကို ဖယ်ရှားပေးပါသည်။ Swift/JS dApps သည် စက်ရှင်ကို ဆက်လက်ရှင်သန်နေချိန်တွင် အမှားအယွင်းကို မှတ်တမ်းတင်ပါသည်။ | - ထိုးသွင်းထားသော ချို့ယွင်းမှု ID ပါရှိသော ကြိုးခွေမှတ်တမ်း။- SDK အမှားမှတ်တမ်း + တယ်လီမီတာ တန်ပြန်ဖမ်းယူမှု။- တန်းစီသည် မကောင်းတဲ့ဘောင်ကို ဖယ်ရှားခဲ့ကြောင်း အထောက်အထား (`recordsRemoved > 0`)။ |

## ဇာတ်လမ်းအသေးစိတ်

### C1 — WebSocket ပြတ်တောက်ပြီး ပြန်လည်ချိတ်ဆက်ပါ။
1. Torii ကို ပရောက်စီ (toxiproxy၊ Envoy သို့မဟုတ် `kubectl port-forward`) ၏နောက်တွင် ခြုံထားပါ
   node တစ်ခုလုံးကို မသတ်ဘဲ ရရှိနိုင်မှုကို သင်ပြောင်းနိုင်သည်။
2. 45s ပြတ်တောက်မှုကို ဖြစ်ပေါ်စေသည်-
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. telemetry dashboards နှင့် `scripts/swift_status_export.py telemetry ကို ကြည့်ရှုပါ
   --json-out artifacts/connect-chaos//c1_metrics.json`။
4. ပြတ်တောက်ပြီးပြီးချင်း တန်းစီခြင်းကို စွန့်ပစ်ပါ-
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. အောင်မြင်မှု = ပြန်လည်ချိတ်ဆက်ရန် ကြိုးပမ်းချက်တစ်ခု၊ ကန့်သတ်ထားသော လူတန်းကြီးထွားမှုနှင့် အလိုအလျောက်ဖြစ်သည်။
   proxy ပြန်တက်လာပြီးနောက် ရေထုတ်လိုက်ပါ။

### C2 — အော့ဖ်လိုင်းတန်းစီလျှံမှု / TTL သက်တမ်းကုန်ဆုံး
1. Local builds များတွင် တန်းစီရမည့် သတ်မှတ်ချက်များကို လျှော့ချပါ-
   - Swift- သင်၏နမူနာအတွင်း၌ `ConnectQueueJournal` ကို အပ်ဒိတ်လုပ်ပါ။
     (ဥပမာ၊ `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` အောင်မြင်ရန်။
   - Android/JS- တည်ဆောက်သည့်အခါ တူညီသော config အရာဝတ္တုကို ဖြတ်သန်းပါ။
     `ConnectQueueJournal`။
2. ≥60s အတွက် ပိုက်ဆံအိတ် (simulator နောက်ခံ သို့မဟုတ် စက်လေယာဉ်ပျံမုဒ်) ကို ဆိုင်းငံ့ပါ။
   dApp သည် `ConnectClient.requestSignature(...)` ခေါ်ဆိုမှုများကို ထုတ်နေစဉ်။
3. `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) သို့မဟုတ် JS ကိုသုံးပါ
   အထောက်အထားအစုအဝေးကို တင်ပို့ရန် ရောဂါရှာဖွေရေးအကူအညီပေးသူ (`state.json`၊ `journal/*.to`၊
   `metrics.ndjson`)။
4. အောင်မြင်မှု = overflow counters increment၊ SDK မျက်နှာပြင်များ `ConnectError.QueueOverflow`
   တစ်ကြိမ်၊ ပိုက်ဆံအိတ်ပြန်စပြီးနောက် တန်းစီသည် ပြန်တက်လာသည်။

### C3 — ပျံ့လွင့်ခြင်း/ဝင်ခွင့် ငြင်းပယ်ခြင်းကို ဖော်ပြပါ။
1. ဝင်ခွင့်ဖော်ပြချက် မိတ္တူကို ပြုလုပ်ပါ၊ ဥပမာ-
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. Torii ကို `--connect-manifest-path /tmp/manifest_drift.json` ဖြင့် စတင်ပါ (သို့မဟုတ်
   လေ့ကျင့်ခန်းအတွက် docker compose/k8s config ကို အပ်ဒိတ်လုပ်ပါ။)
3. ပိုက်ဆံအိတ်မှ session တစ်ခုစတင်ရန်ကြိုးစားပါ။ HTTP 409 ကို မျှော်လင့်ထားသည်။
4. Torii + SDK မှတ်တမ်းများ အပေါင်း `connect.manifest_mismatch_total` မှ ရိုက်ယူပါ
   တယ်လီမီတာ ဒက်ရှ်ဘုတ်။
5. အောင်မြင်မှု = တန်းစီခြင်းမရှိဘဲ ငြင်းပယ်ခံရခြင်း၊ ပေါင်းထည့်ထားသော ပိုက်ဆံအိတ်သည် မျှဝေထားသည့်အရာကို ပြသသည်။
   အဘိဓမ္မာဆိုင်ရာ အမှား (`ConnectError.Authorization.manifestMismatch`)။### C4 — သော့လှည့်ခြင်း / ဆားအဖု
1. လက်ရှိ ဆားဗားရှင်းကို telemetry မှ မှတ်တမ်းတင်ပါ-
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. Torii ကို ဆားအသစ်ဖြင့် ပြန်စပါ (`CONNECT_SALT_VERSION=$((OLD+1))` သို့မဟုတ် အပ်ဒိတ်လုပ်ပါ။
   config map)။ ပြန်လည်စတင်ခြင်း မပြီးမချင်း ပိုက်ဆံအိတ်ကို အော့ဖ်လိုင်းတွင်ထားပါ။
3. ပိုက်ဆံအိတ်ကို ပြန်လည်စတင်ပါ။ ပထမကိုယ်ရေးရာဇဝင်သည် မမှန်ကန်သော ဆားအမှားတစ်ခုကြောင့် ကျရှုံးသင့်သည်။
   နှင့် `connect.queue_dropped_total{reason="salt_version_mismatch"}` တိုးမြင့်မှုများ။
4. စက်ရှင်လမ်းညွှန်ကို ဖျက်ခြင်းဖြင့် အက်ပ်အား ကက်ရှ်ဘောင်များကို ချခိုင်းပါ။
   (`rm -rf ~/.iroha/connect/<sid>` သို့မဟုတ် ပလပ်ဖောင်းသတ်မှတ်ထားသော ကက်ရှ်ရှင်းလင်းခြင်း)၊ ထို့နောက်
   တိုကင်အသစ်များဖြင့် session ကို ပြန်လည်စတင်ပါ။
5. အောင်မြင်မှု = telemetry သည် ဆားအဖုအထစ်ကို ပြသသည်၊ မမှန်ကန်သော ကိုယ်ရေးရာဇဝင်ဖြစ်ရပ်ကို မှတ်တမ်းတင်ထားသည်။
   တစ်ကြိမ်၊ လူကိုယ်တိုင်ဝင်ရောက်စွက်ဖက်မှုမရှိဘဲ နောက်ဆက်ရှင်သည် အောင်မြင်သည်။

### C5 — အထောက်အထား/ StrongBox ကျရှုံးမှု
1. `scripts/android_keystore_attestation.sh` ကို အသုံးပြု၍ သက်သေခံချက်အတွဲတစ်ခုကို ဖန်တီးပါ။
   (လက်မှတ်ဘစ်ကိုလှန်ရန် `--inject-failure strongbox-simulated` ကို သတ်မှတ်ပါ။)
2. ပိုက်ဆံအိတ်ကို ၎င်း၏ `ConnectApproval` API မှတစ်ဆင့် ဤအစုအဝေးကို ပူးတွဲပါရှိစေပါ။ dApp
   payload ကိုအတည်ပြုပြီး ငြင်းပယ်သင့်သည်။
3. တယ်လီမီတာကို အတည်ပြုပါ (`connect.attestation_failed_total`၊ Swift/Android ဖြစ်ရပ်မှန်
   မက်ထရစ်များ) နှင့် လူတန်းသည် အဆိပ်သင့်သော ဝင်ရောက်မှုကို ကျဆင်းသွားကြောင်း သေချာပါစေ။
4. အောင်မြင်မှု = အပယ်ခံရခြင်းသည် မကောင်းမှုကို ခွဲထုတ်ရန် စီတန်းကျန်းမာခြင်း၊
   သက်သေအထောက်အထားများနှင့်အတူ သက်သေခံမှတ်တမ်းကို သိမ်းဆည်းထားသည်။

## သက်သေစာရင်း
- `artifacts/connect-chaos/<date>/c*_metrics.json` တို့မှ တင်ပို့သည်။
  `scripts/swift_status_export.py telemetry`။
- `iroha connect queue inspect` မှ CLI အထွက်များ (`c*_queue.txt`)။
- အချိန်တံဆိပ်တုံးများနှင့် SID တွဲဆိုင်းများဖြင့် SDK + Torii မှတ်တမ်းများ။
- အခြေအနေတစ်ခုစီအတွက် မှတ်စာများပါသော ဒက်ရှ်ဘုတ်စခရင်ပုံများ။
- Sev1/2 သတိပေးချက်များ အလုပ်ဖြုတ်ပါက PagerDuty / အဖြစ်အပျက် ID များ။

လေးပုံတစ်ပုံလျှင် တစ်ကြိမ်လျှင် matrix အပြည့်ဖြည့်ခြင်းသည် လမ်းပြမြေပုံဂိတ်ကို ကျေနပ်စေသည်။
Swift/Android/JS Connect အကောင်အထည်ဖော်မှုများသည် တိကျပြတ်သားစွာတုံ့ပြန်ကြောင်းပြသသည်။
ဖြစ်နိုင်ခြေအမြင့်ဆုံးသော ကျရှုံးမှုမုဒ်များတစ်လျှောက်။