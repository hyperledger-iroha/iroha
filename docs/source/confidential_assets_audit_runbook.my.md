---
lang: my
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T14:35:37.510319+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! `roadmap.md:M4` မှကိုးကားထားသော လျှို့ဝှက်ပိုင်ဆိုင်မှုစာရင်းစစ်နှင့် လည်ပတ်ဆောင်ရွက်မှုပြစာအုပ်။

# လျှို့ဝှက်ပိုင်ဆိုင်မှုများ စာရင်းစစ်နှင့် လည်ပတ်မှုဆိုင်ရာ လုပ်ငန်းစဉ်များ

ဤလမ်းညွှန်ချက်သည် စာရင်းစစ်များနှင့် အော်ပရေတာများအပေါ် မှီခိုနေသည့် မျက်နှာပြင်များဆိုင်ရာ အထောက်အထားများကို စုစည်းပေးသည်။
လျှို့ဝှက်-ပိုင်ဆိုင်မှုစီးဆင်းမှုများကို သက်သေပြသောအခါ။ ၎င်းသည် လှည့်ပတ်ကစားစာအုပ်ကို ဖြည့်စွက်ပေးသည်။
(`docs/source/confidential_assets_rotation.md`) နှင့် စံကိုက်ညှိခြင်း လယ်ဂျာ
(`docs/source/confidential_assets_calibration.md`)။

## 1. ရွေးချယ်ထားသော ထုတ်ဖော်မှုနှင့် ဖြစ်ရပ်ဖိဒ်များ

- လျှို့ဝှက်ညွှန်ကြားချက်တိုင်းသည် ဖွဲ့စည်းတည်ဆောက်ထားသော `ConfidentialEvent` ပေးဆောင်မှုကို ထုတ်လွှတ်သည်။
  (`Shielded`, `Transferred`, `Unshielded`) ၌ ရိုက်ကူးခဲ့သည်
  `crates/iroha_data_model/src/events/data/events.rs:198` ဖြင့် နံပါတ်စဉ်တပ်ထားသည်။
  စီမံအုပ်ချုပ်သူများ (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`–`4021`)။
  regression suite သည် စာရင်းစစ်များကို အားကိုးနိုင်စေရန် ကွန်ကရစ်ပေးချေမှုများကို လေ့ကျင့်ခန်းလုပ်သည်။
  သတ်မှတ်ထားသော JSON အပြင်အဆင်များ (`crates/iroha_core/tests/zk_confidential_events.rs:19`–`299`)။
- Torii သည် ပုံမှန် SSE/WebSocket ပိုက်လိုင်းမှတစ်ဆင့် ဤဖြစ်ရပ်များကို ဖော်ထုတ်ပေးပါသည်။ စာရင်းစစ်များ
  `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`) ကို အသုံးပြု၍ စာရင်းသွင်းပါ။
  တစ်ခုတည်းသော ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုမှုကို စိတ်ကြိုက်ရွေးချယ်နိုင်သည်။ CLI ဥပမာ-

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "rose#wonderland" } }'
  ```

- မူဝါဒ မက်တာဒေတာနှင့် ဆိုင်းငံ့ထားသော အကူးအပြောင်းများမှတဆင့် ရရှိနိုင်ပါသည်။
  `GET /v2/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`)၊ Swift SDK မှ ရောင်ပြန်ဟပ်သည်။
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) နဲ့ မှတ်တမ်းတင်ထားပါတယ်။
  လျှို့ဝှက်ပိုင်ဆိုင်မှုဒီဇိုင်းနှင့် SDK လမ်းညွှန်များ နှစ်ခုလုံး
  (`docs/source/confidential_assets.md:70`၊ `docs/source/sdk/swift/index.md:334`)။

## 2. Telemetry၊ Dashboards နှင့် Calibration အထောက်အထား

- Runtime metrics မျက်နှာပြင်သစ်ပင်အတိမ်အနက်၊ ကတိကဝတ်ပြုမှု / ရှေ့တန်းသမိုင်း၊ အမြစ်ထုတ်ခြင်း
  ကောင်တာများနှင့် verifier-cache hit အချိုးများ
  (`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`)။ Grafana ဒိုင်ခွက်များ
  `dashboards/grafana/confidential_assets.json` သည် ဆက်စပ်အကန့်များနှင့် ပေးပို့သည်။
  `docs/source/confidential_assets.md:401` တွင် မှတ်တမ်းတင်ထားသော အလုပ်အသွားအလာနှင့်အတူ သတိပေးချက်များ။
- လက်မှတ်ထိုးထားသောမှတ်တမ်းများတိုက်ရိုက်ထုတ်လွှဖြင့် (NS/op၊ gas/op၊ ns/gas) ကို Calibration လုပ်ဆောင်သည်
  `docs/source/confidential_assets_calibration.md`။ နောက်ဆုံးထွက် Apple Silicon ပါ။
  NEON run မှာ မော်ကွန်းတင်ထားပါတယ်။
  `docs/source/confidential_assets_calibration_neon_20260428.log` နှင့် အတူတူပင်
  SIMD-neutral နှင့် AVX2 ပရိုဖိုင်များအတွက် ယာယီစွန့်လွှတ်မှုများကို လယ်ဂျာမှ မှတ်တမ်းတင်ပါသည်။
  x86 တန်ဆာပလာများသည် အွန်လိုင်းပေါ် ရောက်လာသည်။

## 3. Incident Response & Operator Tasks

- အလှည့်အပြောင်း/ အဆင့်မြှင့်တင်ခြင်း လုပ်ငန်းစဉ်များ တည်ရှိနေပါသည်။
  `docs/source/confidential_assets_rotation.md`၊ ဇာတ်ခုံအသစ်လုပ်နည်းကို လွှမ်းခြုံထားသည်။
  ကန့်သတ်ဘောင်များ ၊ မူဝါဒ အဆင့်မြှင့်တင်မှုများ အချိန်ဇယား နှင့် ပိုက်ဆံအိတ်/စာရင်းစစ်များကို အကြောင်းကြားပါ။ ဟိ
  ခြေရာခံကိရိယာ (`docs/source/project_tracker/confidential_assets_phase_c.md`) စာရင်းများ
  runbook ပိုင်ရှင်များနှင့် rehearsal မျှော်လင့်ချက်များ။
- ထုတ်လုပ်ရေး အစမ်းလေ့ကျင့်မှုများ သို့မဟုတ် အရေးပေါ်ပြတင်းပေါက်များအတွက်၊ အော်ပရေတာများမှ အထောက်အထားများ ပူးတွဲပါရှိသည်။
  `status.md` ထည့်သွင်းမှုများ (ဥပမာ၊ လမ်းသွားပေါင်းစုံ အစမ်းလေ့ကျင့်မှုမှတ်တမ်း) နှင့် ပါဝင်သည်-
  `curl` မူဝါဒအကူးအပြောင်းများ၏ အထောက်အထား၊ Grafana လျှပ်တစ်ပြက်ရိုက်ချက်များနှင့် သက်ဆိုင်ရာဖြစ်ရပ်
  စာရင်းစစ်များသည် mint → လွှဲပြောင်းခြင်း → အချိန်စာရင်းများကို ဖော်ထုတ်နိုင်စေရန် ပြန်လည်တည်ဆောက်နိုင်သည် ။

## 4. External Review Cadence

- လုံခြုံရေးပြန်လည်သုံးသပ်ခြင်းနယ်ပယ်- လျှို့ဝှက်ဆားကစ်များ၊ ကန့်သတ်ချက်စာရင်းသွင်းမှုများ၊ မူဝါဒ
  အကူးအပြောင်းများနှင့် တယ်လီမီတာ။ ဤစာရွက်စာတမ်းနှင့် စံကိုက်ညှိခြင်းစာရင်းဇယားပုံစံများ
  ရောင်းချသူများထံ ပေးပို့သည့် အထောက်အထားထုပ်ပိုးမှု၊ သုံးသပ်ချက်အချိန်ဇယားကို တစ်ဆင့်ခံ ခြေရာခံသည်။
  M4 တွင် `docs/source/project_tracker/confidential_assets_phase_c.md`။
- အော်ပရေတာများသည် ရောင်းချသူတွေ့ရှိချက် သို့မဟုတ် နောက်ဆက်တွဲအနေဖြင့် `status.md` ကို အပ်ဒိတ်လုပ်ထားရပါမည်။
  လုပ်ဆောင်ချက်ပစ္စည်းများ။ ပြင်ပပြန်လည်သုံးသပ်မှု မပြီးမချင်း၊ ဤအနှစ်ချုပ်စာအုပ်အဖြစ် ဆောင်ရွက်ပါသည်။
  လုပ်ငန်းလည်ပတ်မှုဆိုင်ရာ အခြေခံစာရင်းစစ်များသည် စမ်းသပ်စစ်ဆေးနိုင်သည်။