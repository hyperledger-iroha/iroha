---
id: norito-rpc-adoption
lang: my
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito-RPC Adoption Schedule
sidebar_label: Norito-RPC adoption
description: Cross-SDK rollout plan, evidence checklist, and automation hooks for roadmap item NRPC-4.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Canonical Planning မှတ်စုများသည် `docs/source/torii/norito_rpc_adoption_schedule.md` တွင် နေထိုင်ပါသည်။  
> ဤပေါ်တယ်မိတ္တူသည် SDK စာရေးဆရာများ၊ အော်ပရေတာများနှင့် ပြန်လည်သုံးသပ်သူများအတွက် ဖြန့်ချိရေးမျှော်လင့်ချက်များကို ကွဲပြားစေသည်။

## ရည်ရွယ်ချက်များ

- AND4 ထုတ်လုပ်မှုခလုတ်မတိုင်မီ binary Norito-RPC သယ်ယူပို့ဆောင်ရေးတွင် SDK (Rust CLI၊ Python၊ JavaScript၊ Swift၊ Android) တိုင်းကို ချိန်ညှိပါ။
- အဆင့်ဂိတ်များ၊ သက်သေအစုအဝေးများနှင့် တယ်လီမီတာချိတ်များကို အဆုံးအဖြတ်အတိုင်း ထားရှိထားပါက အုပ်ချုပ်မှုစနစ်သည် စတင်ထုတ်ဝေမှုကို စစ်ဆေးနိုင်သည်။
- လမ်းပြမြေပုံ NRPC-4 ခေါ်ဆိုသော မျှဝေထားသော ကူညီသူများနှင့်အတူ မီးပုံးပျံနှင့် ကိန္နရီအထောက်အထားများကို ဖမ်းယူရန် အသေးအဖွဲလေးဖြစ်အောင် ပြုလုပ်ပါ။

## အဆင့်အချိန်ဇယား

| အဆင့် | ပြတင်းပေါက် | နယ်ပယ် | သတ်မှတ်ချက် | ထွက်ရန်
|---------|--------|-------|----------------|
| **P0 – Lab parity** | Q22025 | Rust CLI + Python smoke suites သည် CI တွင် `/v2/norito-rpc` ကို run သည်၊ JS helper သည် ယူနစ်စမ်းသပ်မှုများ အောင်မြင်သည်၊ Android mock harness လေ့ကျင့်ခန်းသည် သယ်ယူပို့ဆောင်မှုနှစ်ခုကို လုပ်ဆောင်သည်။ | CI တွင် `python/iroha_python/scripts/run_norito_rpc_smoke.sh` နှင့် `javascript/iroha_js/test/noritoRpcClient.test.js` အစိမ်းရောင်၊ Android ကြိုးကြိုးကို `./gradlew test` သို့ ကြိုးတပ်ထားသည်။ |
| **P1 – SDK အစမ်းကြည့်ရှုခြင်း** | Q32025 | မျှဝေထားသော အတွဲအစပ်၊ `scripts/run_norito_rpc_fixtures.sh --sdk <label>` မှတ်တမ်းများ + `artifacts/norito_rpc/` တွင် JSON၊ ရွေးချယ်နိုင်သော Norito သယ်ယူပို့ဆောင်ရေးအလံများကို SDK နမူနာများတွင် ပြသထားသည်။ | Fixture manifest တွင် လက်မှတ်ထိုးထားပြီး၊ README အပ်ဒိတ်များတွင် ရွေးချယ်အသုံးပြုမှုကို ပြသသည်၊ IOS2 အလံနောက်တွင် ရနိုင်သော Swift preview API။ |
| **P2 – Staging / AND4 အကြိုကြည့်ရှုခြင်း** | Q12026 | Staging Torii pools များသည် Norito၊ Android AND4 အစမ်းကြည့်ရှုသော client များနှင့် Swift IOS2 parity suites များကို binary transport၊ telemetry dashboard `dashboards/grafana/torii_norito_rpc_observability.json` တွင် default အနေဖြင့် ဦးစားပေးပါသည်။ | `docs/source/torii/norito_rpc_stage_reports.md` သည် ကိန္နရီကို ဖမ်းယူသည်၊ `scripts/telemetry/test_torii_norito_rpc_alerts.sh` ဖြတ်သန်းမှုများ၊ Android ပုံသဏ္ဍန်ကြိုးကြိုး ပြန်လည်ပြသခြင်းသည် အောင်မြင်မှု/အမှားအယွင်းများကို ဖမ်းယူသည်။ |
| **P3 – ထုတ်လုပ်မှု GA** | Q42026 | Norito သည် SDK များအားလုံးအတွက် မူရင်းသယ်ယူပို့ဆောင်ရေးဖြစ်လာသည်။ JSON သည် နောက်ပြန်ဆုတ်မှုတစ်ခုအဖြစ် ကျန်ရှိနေပါသေးသည်။ တဂ်တိုင်းဖြင့် အလုပ်များကို မော်ကွန်းတင်ထားသော တူညီသည့် တူညီသည့်အရာများကို ထုတ်ဝေပါ။ | Rust/JS/Python/Swift/Android အတွက် Norito မီးခိုးထွက်ရှိမှု စစ်ဆေးစာရင်း အစုအဝေးများကို ထုတ်ပြန်ပါ။ Norito နှင့် JSON အမှား-နှုန်း SLO များအတွက် သတိပေးချက် ကန့်သတ်ချက်များ၊ `status.md` နှင့် ထုတ်ပြန်သည့် မှတ်စုများသည် GA အထောက်အထားများကို ကိုးကားပါသည်။ |

## SDK နှင့် CI ချိတ်များ

- **Rust CLI နှင့် ပေါင်းစပ်ကြိုး** – `cargo xtask norito-rpc-verify` သည် `cargo xtask norito-rpc-verify` တစ်ကြိမ် ဆင်းသက်လာသည်နှင့်တစ်ပြိုင်နက် Norito သယ်ယူပို့ဆောင်ရေးကို တွန်းအားပေးရန် `iroha_cli pipeline` မီးခိုးစမ်းသပ်မှုများကို သက်တမ်းတိုးပါ။ `cargo test -p integration_tests -- norito_streaming` (ဓာတ်ခွဲခန်း) နှင့် `cargo xtask norito-rpc-verify` (staging/GA)၊ `artifacts/norito_rpc/` အောက်တွင် ရှေးဟောင်းပစ္စည်းများကို သိမ်းဆည်းခြင်း။
- **Python SDK** – ထွက်လာသည့်မီးခိုး (`python/iroha_python/scripts/release_smoke.sh`) ကို Norito RPC သို့ ပုံသေသတ်မှတ်ပြီး I18NI000000036X ကို CI ဝင်ခွင့်အမှတ်အဖြစ် ထားရှိကာ `python/iroha_python/README.md` တွင် စာရွက်စာတမ်း ညီမျှခြင်း ကိုင်တွယ်မှု။ CI ပစ်မှတ်- `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`။
- **JavaScript SDK** – `NoritoRpcClient` ကို တည်ငြိမ်အောင်ပြုလုပ်ပြီး အုပ်ချုပ်မှု/မေးမြန်းမှုအကူအညီများကို `toriiClientConfig.transport.preferred === "norito_rpc"` တွင် ပုံသေအဖြစ် Norito အဖြစ် သတ်မှတ်ပေးပြီး `javascript/iroha_js/recipes/` တွင် အဆုံးမှအစနမူနာများကို ရိုက်ကူးခွင့်ပြုပါ။ CI သည် ထုတ်ဝေခြင်းမပြုမီ `npm test` နှင့် dockerised `npm run test:norito-rpc` အလုပ်တို့ကို လုပ်ဆောင်ရပါမည်။ သက်သေပြချက်သည် Norito မီးခိုးမှတ်တမ်းများကို `javascript/iroha_js/artifacts/` အောက်တွင် အပ်လုဒ်လုပ်သည်။
- **Swift SDK** - IOS2 အလံနောက်ကွယ်ရှိ Norito တံတားသယ်ယူပို့ဆောင်ရေးကို ကြိုးဖြင့် သွယ်တန်းကာ တပ်ဆင်မှုပုံစံကို ရောင်ပြန်ဟပ်ကာ Connect/Norito parity suite သည် I18NI000000045X တွင် ရည်ညွှန်းထားသော Buildkite လမ်းကြောင်းများအတွင်းတွင် အလုပ်လုပ်ကြောင်း သေချာပါစေ။
- **Android SDK** – AND4 အစမ်းကြည့်ရှုသည့် ဖောက်သည်များနှင့် အတုအယောင် Torii ကြိုးသိုင်းသည် Norito ကို `docs/source/sdk/android/networking.md` တွင် မှတ်တမ်းတင်ထားသော ပြန်စမ်းခြင်း/အလှည့်ကျ တယ်လီမီတာဖြင့် မှတ်တမ်းတင်ထားသော Norito ကို လက်ခံပါသည်။ သံကြိုးသည် `scripts/run_norito_rpc_fixtures.sh --sdk android` မှတစ်ဆင့် အခြား SDK များနှင့် ပစ္စည်းများ မျှဝေပါသည်။

## အထောက်အထားနှင့် အလိုအလျောက်စနစ်

- `scripts/run_norito_rpc_fixtures.sh` သည် `cargo xtask norito-rpc-verify` ကို ခြုံငုံပြီး stdout/stderr ဖမ်းယူကာ `fixtures.<sdk>.summary.json` ကို ထုတ်လွှတ်သောကြောင့် SDK ပိုင်ရှင်များသည် `status.md` နှင့် ပူးတွဲရန် အဆုံးအဖြတ်ပေးသည့် လက်ရာတစ်ခုရှိသည်။ CI အစုအဝေးများကို သပ်ရပ်အောင်ထားရန် `--sdk <label>` နှင့် `--out artifacts/norito_rpc/<stamp>/` ကိုသုံးပါ။
- `cargo xtask norito-rpc-verify` သည် schema hash parity (`fixtures/norito_rpc/schema_hashes.json`) ကို တွန်းအားပေးပြီး Torii သည် `X-Iroha-Error-Code: schema_mismatch` သို့ ပြန်သွားပါက မအောင်မြင်ပါ။ အမှားရှာပြင်ခြင်းအတွက် JSON တုံ့ပြန်မှုဖမ်းယူမှုဖြင့် ရှုံးနိမ့်မှုတိုင်းကို တွဲချိတ်ပါ။
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` နှင့် `dashboards/grafana/torii_norito_rpc_observability.json` သည် NRPC-2 အတွက် သတိပေးချက် စာချုပ်များကို သတ်မှတ်ပါသည်။ ဒက်ရှ်ဘုတ်တိုင်းကို တည်းဖြတ်ပြီးနောက် script ကို run ပြီး `promtool` output ကို canary bundle တွင် သိမ်းဆည်းပါ။
- `docs/source/runbooks/torii_norito_rpc_canary.md` သည် ဇာတ်ခုံနှင့် ထုတ်လုပ်မှုလေ့ကျင့်ခန်းများကို ဖော်ပြသည်။ fixture hashes သို့မဟုတ် alert gates ပြောင်းလဲသည့်အခါတိုင်း ၎င်းကို update လုပ်ပါ။

## သုံးသပ်သူစာရင်း

NRPC-4 မှတ်တိုင်ကို မစစ်ဆေးမီ အတည်ပြုပါ-

1. နောက်ဆုံးထွက်ရှိထားသောအတွဲများ ဟက်ခ်များသည် `fixtures/norito_rpc/schema_hashes.json` နှင့် `artifacts/norito_rpc/<stamp>/` အောက်တွင် မှတ်တမ်းတင်ထားသော သက်ဆိုင်ရာ CI ပစ္စည်းများနှင့် ကိုက်ညီပါသည်။
2. SDK README / portal docs သည် JSON ဆုတ်ယုတ်မှုကို မည်သို့တွန်းအားပေးရမည်ကို ဖော်ပြပြီး Norito သယ်ယူပို့ဆောင်ရေးမူရင်းကို ကိုးကားဖော်ပြသည်။
3. တယ်လီမီတာ ဒက်ရှ်ဘုတ်များသည် သတိပေးချက်လင့်ခ်များပါရှိသော အမှားအယွင်း-နှုန်းအကန့်နှစ်ခုကို ပြသထားပြီး Alertmanager dry run (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) ကို ခြေရာခံကိရိယာတွင် တွဲထားသည်။
4. ဤနေရာ၌ မွေးစားခြင်းအချိန်ဇယားသည် ခြေရာခံဝင်ရောက်မှု (`docs/source/torii/norito_rpc_tracker.md`) နှင့် ကိုက်ညီပြီး လမ်းပြမြေပုံ (NRPC-4) သည် တူညီသောအထောက်အထားအစုအဝေးကို ရည်ညွှန်းပါသည်။

အချိန်ဇယားအတိုင်း စည်းကမ်းရှိရှိနေထိုင်ခြင်းသည် SDK ဖြတ်ကျော်သည့်အပြုအမူကို ခန့်မှန်းနိုင်စေပြီး အုပ်ချုပ်မှုစာရင်းစစ် Norito-RPC မွေးစားခြင်းကို စိတ်ကြိုက်တောင်းဆိုမှုများမပါဘဲ ခွင့်ပြုပေးပါသည်။