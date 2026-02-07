---
id: pq-rollout-plan
lang: my
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNNet-16G Post-Quantum Rollout Playbook
sidebar_label: PQ Rollout Plan
description: Operational guide for promoting the SoraNet hybrid X25519+ML-KEM handshake from canary to default across relays, clients, and SDKs.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
:::

SNNet-16G သည် SoraNet သယ်ယူပို့ဆောင်ရေးအတွက် ကွမ်တမ်လွန် ဖြန့်ချိမှုကို အပြီးသတ်သည်။ `rollout_phase` ခလုတ်များသည် အော်ပရေတာများအား လက်ရှိ Stage A အစောင့်လိုအပ်ချက်မှ Stage B အများစုလွှမ်းခြုံမှုအထိ အဆုံးအဖြတ်ပေးသည့် ပရိုမိုးရှင်းတစ်ခုနှင့် မျက်နှာပြင်တိုင်းအတွက် JSON/TOML ကြမ်းများကို မတည်းဖြတ်ဘဲ Stage C တင်းကျပ်သော PQ ကိုယ်ဟန်အနေအထားကို ပေါင်းစပ်နိုင်စေပါသည်။

ဤကစားနည်းစာအုပ်တွင် ပါဝင်သည်-

- ကုဒ်ဘေ့စ် (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`) တွင် ကြိုးတပ်ထားသည့် အဆင့်သတ်မှတ်ချက်များနှင့် ဖွဲ့စည်းမှုခလုတ်အသစ်များ (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`)။
- SDK နှင့် CLI အလံကို မြေပုံဆွဲခြင်းဖြင့် သုံးစွဲသူတိုင်းသည် ထုတ်လွှင့်မှုကို ခြေရာခံနိုင်သည်။
- Relay/client ကိန္နရီအချိန်ဇယားဆွဲခြင်းမျှော်လင့်ချက်များအပြင် ဂိတ်ပေါက်မြှင့်တင်ရေး (`dashboards/grafana/soranet_pq_ratchet.json`) ဖြစ်သော အုပ်ချုပ်မှုဒက်ရှ်ဘုတ်များ။
- နောက်ပြန်ချိတ်များနှင့် မီးတူးခြင်းဆိုင်ရာစာအုပ် ([PQ ratchet runbook](./pq-ratchet-runbook.md))။

## အဆင့်မြေပုံ

| `rollout_phase` | ထိရောက်သော အမည်ဝှက်ခြင်း အဆင့် | ပုံသေအကျိုးသက်ရောက်မှု | ပုံမှန်အသုံးပြုမှု |
|--------------------|------------------------------------------------------------------------------------------------|
| `canary` | `anon-guard-pq` (Stage A) | ရေယာဉ်များ ပူနွေးလာချိန်တွင် ဆားကစ်တစ်ခုလျှင် အနည်းဆုံး PQ အစောင့်တစ်ခု လိုအပ်ပါသည်။ | အခြေခံနှင့် ကိန္နရီ အစောပိုင်း ရက်သတ္တပတ်များ။ |
| `ramp` | `anon-majority-pq` (Stage B) | >= သုံးပုံနှစ်ပုံ လွှမ်းခြုံမှုအတွက် PQ relay များဆီသို့ ဘက်လိုက်မှု ရွေးချယ်မှု။ classical relay များသည် အမှားအယွင်းများအဖြစ် ကျန်ရှိနေပါသည်။ | ဒေသအလိုက် တစ်ဆင့်ခံ ကိန္နရီ၊ SDK အစမ်းကြည့်ရှုမှုခလုတ်များ |
| `default` | `anon-strict-pq` (Stage C) | PQ-သီးသန့် ဆားကစ်များကို တွန်းအားပေးပြီး နှိုးစက်များကို အဆင့်နှိမ့်ရန် တင်းကျပ်ပါ။ | တယ်လီမီတာနှင့် အုပ်ချုပ်မှု လက်မှတ်ရေးထိုးခြင်း ပြီးသည်နှင့် နောက်ဆုံး ပရိုမိုးရှင်း။ |

အကယ်၍ မျက်နှာပြင်တစ်ခုသည် တိကျပြတ်သားသော `anonymity_policy` ကို သတ်မှတ်ပါက၊ ၎င်းသည် ထိုအစိတ်အပိုင်းအတွက် အဆင့်ကို ကျော်လွန်သွားမည်ဖြစ်သည်။ ရှင်းလင်းပြတ်သားသောအဆင့်ကို ချန်လှပ်ထားခြင်းသည် ယခု `rollout_phase` တန်ဖိုးသို့ ရွှေ့ဆိုင်းသွားသည့်အတွက် အော်ပရေတာများသည် အဆင့်ကို ပတ်ဝန်းကျင်တစ်ခုလျှင် တစ်ကြိမ်လှန်နိုင်ပြီး သုံးစွဲသူများကို ၎င်းကို အမွေဆက်ခံခွင့်ပေးလိုက်ပါ။

## ဖွဲ့စည်းမှုအကိုးအကား

### သံစုံတီးဝိုင်း (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Orchestrator loader သည် runtime (`crates/sorafs_orchestrator/src/lib.rs:2229`) တွင် fallback stage ကိုဖြေရှင်းပေးပြီး `sorafs_orchestrator_policy_events_total` နှင့် `sorafs_orchestrator_pq_ratio_*` မှတဆင့် ၎င်းကိုမျက်နှာပြင်တင်ပေးသည်။ အဆင်သင့်အသုံးပြုနိုင်သည့် အတိုအထွာများအတွက် `docs/examples/sorafs_rollout_stage_b.toml` နှင့် `docs/examples/sorafs_rollout_stage_c.toml` ကို ကြည့်ပါ။

### သံချေးဖောက်သည် / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

ယခု `iroha::Client` သည် ခွဲခြမ်းစိတ်ဖြာသည့်အဆင့် (`crates/iroha/src/client.rs:2315`) ကို မှတ်တမ်းတင်ထားသောကြောင့် အကူအညီပေးသည့်အမိန့်များ (ဥပမာ `iroha_cli app sorafs fetch`) သည် မူရင်းအမည်ဝှက်ခြင်းမူဝါဒနှင့်အတူ လက်ရှိအဆင့်ကို အစီရင်ခံနိုင်ပါသည်။

## အလိုအလျောက်စနစ်

`cargo xtask` အကူအညီပေးသူ နှစ်ဦးသည် အချိန်ဇယား ထုတ်လုပ်မှုနှင့် အနုပညာလက်ရာများကို အလိုအလျောက် ဖမ်းယူပေးသည်။

1. **ဒေသဆိုင်ရာ အချိန်ဇယားကို ဖန်တီးပါ**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   ကြာချိန်များသည် `s`၊ `m`၊ `h` သို့မဟုတ် `d` ၏ နောက်ဆက်တွဲများကို လက်ခံပါသည်။ ပြောင်းလဲမှုတောင်းဆိုချက်နှင့်အတူ ပေးပို့နိုင်သည့် `artifacts/soranet_pq_rollout_plan.json` နှင့် Markdown အနှစ်ချုပ် (`artifacts/soranet_pq_rollout_plan.md`) ကို ထုတ်လွှတ်သည်။

2. ** လက်မှတ်များဖြင့် တူးထားသော ပစ္စည်းများကို ဖမ်းယူပါ**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   command သည် ပံ့ပိုးပေးထားသောဖိုင်များကို `artifacts/soranet_pq_rollout/<timestamp>_<label>/` သို့ကူးယူပြီး၊ artefact တစ်ခုစီအတွက် BLAKE3 အချေအတင်များကိုတွက်ချက်ကာ payload ပေါ်တွင် metadata နှင့် Ed25519 လက်မှတ်ပါရှိသော `rollout_capture.json` ကိုရေးသည်။ အုပ်ချုပ်မှုစနစ်က ဖမ်းယူမှုကို အမြန်အတည်ပြုနိုင်စေရန် မီးလေ့ကျင့်ခန်းမိနစ်များကို နိမိတ်လက္ခဏာပြသည့် တူညီသောသီးသန့်သော့ကို အသုံးပြုပါ။

## SDK & CLI အလံ matrix

| မျက်နှာပြင် | Canary (Stage A) | အဲလိုမျိုး (Stage B) | ပုံသေ (Stage C) |
|---------|------------------|------------------------------------------------|
| `sorafs_cli` အကျိူးဆောင် | `--anonymity-policy stage-a` သို့မဟုတ် အဆင့် | အားကိုး `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Orchestrator config JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust client config (`iroha.toml`) | `rollout_phase = "canary"` (မူရင်း) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` လက်မှတ်ရေးထိုးထားသော အမိန့်များ | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`၊ လုပ်နိုင်သော `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`၊ လုပ်နိုင်သော `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`၊ လုပ်နိုင်သော `.ANON_STRICT_PQ` |
| JavaScript သံစုံတီးဝိုင်းကူညီသူများ | `rolloutPhase: "canary"` သို့မဟုတ် `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

SDK များအားလုံးသည် သံစုံတီးဝိုင်းအဖွဲ့ (`crates/sorafs_orchestrator/src/lib.rs:365`) အသုံးပြုသည့် တူညီသော အဆင့်ခွဲခြမ်းစိတ်ဖြာမှုသို့ မြေပုံပြောင်းသွားခြင်းဖြစ်သည်၊ ထို့ကြောင့် ဘာသာစကား ရောနှောအသုံးပြုမှုများသည် စီစဉ်သတ်မှတ်ထားသော အဆင့်နှင့်အတူ လော့ခ်ချသည့်အဆင့်တွင် ရှိနေပါသည်။

## Canary စာရင်းဇယားဆွဲခြင်း။

1. ** Preflight (T အနှုတ် 2 ပတ်)**

- အဆင့် A ပြတ်တောက်မှုနှုန်း <1% ယခင်နှစ်ပတ်ကျော်နှင့် PQ လွှမ်းခြုံမှု >=70% (`sorafs_orchestrator_pq_candidate_ratio`) ကို အတည်ပြုပါ။
   - Canary Window ကို အတည်ပြုသည့် အုပ်ချုပ်မှုပြန်လည်သုံးသပ်ခြင်း slot ကို အချိန်ဇယားဆွဲပါ။
   - အဆင့်မြှင့်တင်ခြင်းတွင် `sorafs.gateway.rollout_phase = "ramp"` ကို အပ်ဒိတ်လုပ်ပါ (သံစုံတီးဝိုင်း JSON ကို တည်းဖြတ်ပြီး ပြန်လည်အသုံးချခြင်း) နှင့် ပရိုမိုးရှင်းပိုက်လိုင်းကို ခြောက်သွားအောင် လုပ်ဆောင်ပါ။

2. **Relay Canary (T day)**

   - သံစုံတီးဝိုင်းဆရာနှင့် ပါ၀င်သော relay manifests များပေါ်တွင် `rollout_phase = "ramp"` ကို သတ်မှတ်ခြင်းဖြင့် တစ်ကြိမ်လျှင် ဒေသတစ်ခုကို မြှင့်တင်ပါ။
   - ရလဒ်တစ်ခုအတွက် မူဝါဒဖြစ်ရပ်များနှင့် "Brownout Rate" ကို PQ Ratchet ဒက်ရှ်ဘုတ်တွင် (ယခုထုတ်လွှတ်သည့်အကန့်ကို ပါ၀င်သည်) တွင် guard cache TTL နှစ်ကြိမ်ကို စောင့်ကြည့်ပါ။
   - စာရင်းစစ်သိုလှောင်မှုအတွက် `sorafs_cli guard-directory fetch` လျှပ်တစ်ပြက်ရိုက်ချက်များကို ဖြတ်တောက်ပါ။

3. **Client/SDK Canary (T အပေါင်း 1 ပတ်)**

   - client configs တွင် `rollout_phase = "ramp"` ကိုလှန်ပါ သို့မဟုတ် သတ်မှတ်ထားသော SDK အစုအဝေးများအတွက် `stage-b` ကို ကျော်ဖြတ်ပါ။
   - တယ်လီမီတာ ကွဲပြားမှုများကို ဖမ်းယူပါ (`sorafs_orchestrator_policy_events_total` နှင့် `region` ဖြင့် အုပ်စုဖွဲ့ထားသည်) နှင့် ၎င်းတို့ကို စတင်ထုတ်သည့် ဖြစ်ရပ်မှတ်တမ်းတွင် ပူးတွဲပါရှိသည်။

4. ** မူရင်းပရိုမိုးရှင်း (T နှင့် 3 ပတ်)**

   - အုပ်ချုပ်မှုဆိုင်းဘုတ်ပိတ်ပြီးသည်နှင့်၊ သံစုံတီးဝိုင်းနှင့်ဖောက်သည် configs နှစ်ခုလုံးကို `rollout_phase = "default"` သို့ပြောင်းပြီး လက်မှတ်ရေးထိုးထားသည့် အဆင်သင့်စစ်ဆေးစာရင်းကို ထုတ်ဝေသည့်ပစ္စည်းများအဖြစ်သို့ လှည့်ပါ။

## အုပ်ချုပ်ရေးနှင့် သက်သေစာရင်း

| အဆင့်ပြောင်းလဲမှု | ပရိုမိုးရှင်းဂိတ် | သက်သေအတွဲ | ဒက်ရှ်ဘုတ်များနှင့် သတိပေးချက်များ |
|-----------------|----------------|-----------------|--------------------------------|
| Canary → ချဉ်းကပ်လမ်း *(အဆင့် B အစမ်းကြည့်ရှုခြင်း)* | အဆင့်-A သည် နောက်လိုက်နေသည့် 14 ရက်အတွင်း အပျက်အစီးနှုန်း <1%၊ ရာထူးတိုးဒေသတစ်ခုလျှင် `sorafs_orchestrator_pq_candidate_ratio` ≥ 0.7၊ Argon2 လက်မှတ် p95 < 50 ms မှန်ကန်ကြောင်း နှင့် ပရိုမိုးရှင်းအတွက် ကြိုတင်စာရင်းသွင်းထားသော အုပ်ချုပ်မှုအထိုင်။ | `cargo xtask soranet-rollout-plan` JSON/Markdown အတွဲ၊ `sorafs_cli guard-directory fetch` လျှပ်တစ်ပြက်တွဲများ (ရှေ့/နောက်)၊ လက်မှတ်ထိုးထားသော `cargo xtask soranet-rollout-capture --label canary` အတွဲ၊ နှင့် ကိန္နရီမိနစ်ကိုးကားခြင်း [PQ ratchet runbook](I18NU0000005X)။ | `dashboards/grafana/soranet_pq_ratchet.json` (မူဝါဒဖြစ်ရပ်များ + ဘရောင်းထွက်နှုန်း)၊ `dashboards/grafana/soranet_privacy_metrics.json` (SN16 အဆင့်နှိမ့်ချအချိုး)၊ `docs/source/soranet/snnet16_telemetry_plan.md` ရှိ တယ်လီမီတာ ကိုးကားချက်များ။ |
| ချဉ်းကပ်လမ်း → ပုံသေ *(အဆင့် C ပြဋ္ဌာန်းချက်)* | ရက်ပေါင်း 30 ကြာ SN16 တယ်လီမီတာ လောင်ကျွမ်းမှုတွင် ကိုက်ညီမှု၊ အခြေခံလိုင်းတွင် `sn16_handshake_downgrade_total`၊ လိုင်းခန်းအတွင်း `sorafs_orchestrator_brownouts_total` သုည၊ နှင့် ပရောက်စီ အစမ်းလေ့ကျင့်မှု အဖွင့်အပိတ် မှတ်တမ်းဝင်ထားသည်။ | `sorafs_cli proxy set-mode --mode gateway|direct` မှတ်တမ်း၊ `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` အထွက်၊ `sorafs_cli guard-directory verify` မှတ်တမ်းနှင့် `cargo xtask soranet-rollout-capture --label default` အစုအဝေးကို ရေးထိုးထားသည်။ | တူညီသော PQ Ratchet board နှင့် `docs/source/sorafs_orchestrator_rollout.md` နှင့် `dashboards/grafana/soranet_privacy_metrics.json` တွင် မှတ်တမ်းတင်ထားသော SN16 အဆင့်နှိမ့်မှုအကန့်များ။ |
| အရေးပေါ်အခြေအနေရွှေ့ဆိုင်းခြင်း/ နောက်ပြန်ဆုတ်ရန် အဆင်သင့် | အဆင့်နှိမ့်ရန်ကောင်တာများ တိုးလာသောအခါ၊ အစောင့်-လမ်းညွှန်အတည်ပြုခြင်း မအောင်မြင်ပါ သို့မဟုတ် `/policy/proxy-toggle` ကြားခံမှတ်တမ်းများသည် အဆင့်နှိမ့်ချသည့်ဖြစ်ရပ်များကို ဆက်လက်လုပ်ဆောင်နေချိန်တွင် အစပျိုးသည်။ | `docs/source/ops/soranet_transport_rollback.md`၊ `sorafs_cli guard-directory import` / `guard-cache prune` မှတ်တမ်းများ၊ `cargo xtask soranet-rollout-capture --label rollback`၊ အဖြစ်အပျက်လက်မှတ်များနှင့် အကြောင်းကြားချက် နမူနာများ။ | `dashboards/grafana/soranet_pq_ratchet.json`၊ `dashboards/grafana/soranet_privacy_metrics.json` နှင့် အချက်ပေးအထုပ်များ (`dashboards/alerts/soranet_handshake_rules.yml`၊ `dashboards/alerts/soranet_privacy_rules.yml`)။ |

- ထုတ်လုပ်ထားသော `rollout_capture.json` ဖြင့် `artifacts/soranet_pq_rollout/<timestamp>_<label>/` အောက်တွင် အနုပညာပစ္စည်းများအားလုံးကို သိမ်းဆည်းထားသောကြောင့် အုပ်ချုပ်မှုပက်ကေ့ဂျ်များတွင် အမှတ်စာရင်းဘုတ်များ၊ ပရိုမီတူးလ်ခြေရာများ၊ နှင့် မှတ်တမ်းများပါရှိသည်။
- အပ်လုဒ်လုပ်ထားသော အထောက်အထားများ (မိနစ် PDF၊ အစုအဝေး၊ အစောင့်လျှပ်တစ်ပြက်ပုံများ) ကို SHA256 ၏ မြှင့်တင်ရေးမိနစ်များတွင် ပူးတွဲပါရှိသောကြောင့် ပါလီမန်၏ အတည်ပြုချက်များအား ဇာတ်ညွှန်းအစုသို့ ဝင်ရောက်ခြင်းမရှိဘဲ ပြန်လည်ပြသနိုင်ပါသည်။
- `docs/source/soranet/snnet16_telemetry_plan.md` သည် ဝေါဟာရအဆင့်နှိမ့်ချခြင်းနှင့် သတိပေးချက်အဆင့်သတ်မှတ်ခြင်းအတွက် ကျမ်းရင်းမြစ်ဖြစ်ကြောင်း သက်သေပြရန် ပရိုမိုးရှင်းလက်မှတ်တွင် တယ်လီမီတာအစီအစဉ်ကို ကိုးကားပါ။

## ဒက်ရှ်ဘုတ်နှင့် တယ်လီမီတာ အပ်ဒိတ်များ

ယခု `dashboards/grafana/soranet_pq_ratchet.json` သည် ဤဖွင့်စစာအုပ်သို့ ပြန်လည်ချိတ်ဆက်ပြီး လက်ရှိအဆင့်ကို ချိတ်ဆက်ပေးသည့် "Rollout Plan" မှတ်ချက်အကန့်တစ်ခုဖြင့် ပို့ဆောင်ပေးနေပြီဖြစ်သောကြောင့် အုပ်ချုပ်မှုပြန်လည်သုံးသပ်ခြင်းများသည် မည်သည့်အဆင့်ကို လုပ်ဆောင်နေကြောင်း အတည်ပြုနိုင်ပါသည်။ config knobs များအတွက် အနာဂတ်ပြောင်းလဲမှုများနှင့် အကန့်ဖော်ပြချက်ကို ထပ်တူကျအောင်ထားပါ။

သတိပေးချက်အတွက်၊ ရှိပြီးသားစည်းမျဉ်းများသည် `stage` အညွှန်းကိုသုံးပါ သေချာစေရန် ကိန္နရီနှင့် ပုံသေအဆင့်များသည် သီးခြားမူဝါဒသတ်မှတ်ချက်များ (`dashboards/alerts/soranet_handshake_rules.yml`) ကို အစပျိုးစေသည်။

## နောက်ပြန်ချိတ်

### ပုံသေ → ချဉ်းကပ်လမ်း (အဆင့် C → အဆင့် B)

1. `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` ဖြင့် သံစုံတီးဝိုင်းကို ရာထူးချပါ (ပြီး SDK စီစဉ်သတ်မှတ်မှုများတွင် တူညီသောအဆင့်ကို ရောင်ပြန်ဟပ်ပါ) ထို့ကြောင့် အဆင့် B သည် အစုအဝေးတစ်ခုလုံးကို ပြန်လည်စတင်ပါသည်။
2. စာသားမှတ်တမ်းကိုဖမ်းယူ၍ `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` မှတစ်ဆင့် လုံခြုံသောသယ်ယူပို့ဆောင်ရေးပရိုဖိုင်သို့ဖောက်သည်များအား အတင်းအကျပ်ခိုင်းစေခြင်းဖြင့် `/policy/proxy-toggle` ပြန်လည်ပြင်ဆင်ခြင်းလုပ်ငန်းအသွားအလာကို ဆက်လက်စစ်ဆေးပါ။
3. `cargo xtask soranet-rollout-capture --label rollback-default` ကို run ပြီး guard-directory diffs၊ promtool output နှင့် `artifacts/soranet_pq_rollout/` အောက်တွင် ဒက်ရှ်ဘုတ်ဖန်သားပြင်ဓာတ်ပုံများကို သိမ်းဆည်းရန်။

### ချဉ်းကပ်လမ်း → ကိန္နရီ (အဆင့် B → အဆင့် A)

1. `sorafs_cli guard-directory import --guard-directory guards.json` ဖြင့် ပရိုမိုးရှင်းမပြုလုပ်မီ ရိုက်ကူးထားသော အစောင့်-လမ်းညွှန်လျှပ်တစ်ပြက်ဓာတ်ပုံကို တင်သွင်းပြီး `sorafs_cli guard-directory verify` ကို ပြန်ဖွင့်ပါ ထို့ကြောင့် နှိမ့်ချမှုပက်ကတ်တွင် hashe များပါဝင်ပါသည်။
2. `rollout_phase = "canary"` (သို့မဟုတ် `anonymity_policy stage-a`) ကို သံစုံတီးဝိုင်းနှင့် ကလိုင်းယင့်ပုံစံများပေါ်တွင် `rollout_phase = "canary"` (သို့မဟုတ် `anonymity_policy stage-a` ဖြင့် အစားထိုးရန်) သတ်မှတ်ပါ၊ ထို့နောက် အဆင့်နှိမ့်ချသည့်ပိုက်လိုင်းကို သက်သေပြရန် [PQ ratchet runbook](./pq-ratchet-runbook.md) မှ PQ ratchet drill ကို ပြန်ဖွင့်ပါ။
3. အပ်ဒိတ်လုပ်ထားသော PQ Ratchet နှင့် SN16 တယ်လီမီတာစခရင်ပုံများအပြင် အုပ်ချုပ်ရေးကိုအကြောင်းကြားခြင်းမပြုမီ အဖြစ်အပျက်မှတ်တမ်းတွင် သတိပေးချက်ရလဒ်များကို ပူးတွဲပါ။

### Guardrail သတိပေးချက်များ- အကိုးအကား `docs/source/ops/soranet_transport_rollback.md` နှိမ့်ချမှုတစ်ခုဖြစ်ပေါ်သည့်အခါတိုင်းနှင့် နောက်ဆက်တွဲအလုပ်အတွက် ဖြန့်ချိသည့်ခြေရာခံကိရိယာရှိ `TODO:` တွင် ယာယီလျော့ပါးသွားသည့်အရာတစ်ခုခုကို မှတ်တမ်းတင်ပါ။
- နောက်ပြန်မဆွဲမီနှင့် အပြီးတွင် `dashboards/alerts/soranet_handshake_rules.yml` နှင့် `dashboards/alerts/soranet_privacy_rules.yml` တို့ကို `promtool test rules` အောက်တွင် သိမ်းထားပါ ထို့ကြောင့် သတိပေးချက်ကို ဖမ်းယူထားသည့်အတွဲနှင့်အတူ မှတ်တမ်းတင်ထားသည်။