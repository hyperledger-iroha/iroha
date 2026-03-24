---
lang: my
direction: ltr
source: docs/source/cbdc_lane_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b97d0dd24de65ba37cba0fa4d404235b63a771f49ac6ee8f07e4814d2a6db814
source_last_modified: "2026-01-22T16:26:46.564417+00:00"
translation_last_reviewed: 2026-02-07
title: CBDC Lane Playbook
sidebar_label: CBDC Lane Playbook
description: Reference configuration, whitelist flow, and compliance evidence for permissioned CBDC lanes on SORA Nexus.
translator: machine-google-reviewed
---

# CBDC Private Lane Playbook (NX-6)

> **လမ်းပြမြေပုံ ချိတ်ဆက်မှု-** NX-6 (CBDC သီးသန့်လမ်းသွား နမူနာပုံစံနှင့် လမ်းကြောင်းခွင့်ပြုစာရင်း) နှင့် NX-14 (Nexus runbooks)။  
> **Owners:** Financial Services WG, Nexus Core WG, Compliance WG.  
> **အခြေအနေ-** မူကြမ်းရေးဆွဲခြင်း — `crates/iroha_data_model::nexus`၊ `crates/iroha_core::governance::manifest` နှင့် `integration_tests/tests/nexus/lane_registry.rs` တွင် အကောင်အထည်ဖော်မှုချိတ်များ ရှိနေသော်လည်း CBDC သီးသန့်ဖော်ပြချက်များ၊ ခွင့်ပြုချက်စာရင်းများနှင့် အော်ပရေတာ runbook များ ပျောက်ဆုံးနေပါသည်။ ဤပြခန်းစာအုပ်တွင် CBDC ဖြန့်ကျက်မှုများကို အဆုံးအဖြတ်အတိုင်း ဆက်လက်လုပ်ဆောင်နိုင်စေရန် ရည်ညွှန်းဖွဲ့စည်းပုံနှင့် စတင်လုပ်ဆောင်ခြင်းလုပ်ငန်းအသွားအလာတို့ကို မှတ်တမ်းတင်ထားသည်။

## နယ်ပယ်နှင့် အခန်းကဏ္ဍ

- ** Central bank lane ("CBDC lane"):** ခွင့်ပြုချက်ရရှိထားသူများ၊ အုပ်ထိန်းမှုဆိုင်ရာ အခြေချရေးကြားခံများနှင့် ပရိုဂရမ်ထုတ်နိုင်သော ငွေကြေးမူဝါဒများ။ ၎င်း၏ကိုယ်ပိုင်အုပ်ချုပ်မှုဖော်ပြချက်နှင့်အတူ ကန့်သတ်ဒေတာနေရာ + လမ်းသွားတွဲအဖြစ် လုပ်ဆောင်သည်။
- **လက်ကား/လက်လီဘဏ်ဒေတာနေရာများ-** UAIDs များကိုင်ဆောင်ထားသော ပါဝင်သူ DS သည် စွမ်းဆောင်ရည်ပြသမှုများကို လက်ခံရရှိကာ CBDC လမ်းကြောနှင့်အတူ အဏုမြူ AXT အတွက် တရားဝင်စာရင်းသွင်းခံရနိုင်သည်။
- **Programmable-money dApps-** CBDC စားသုံးသော ပြင်ပ DS သည် `ComposabilityGroup` လမ်းကြောင်းကို တရားဝင်စာရင်းသွင်းပြီးသည်နှင့် ဖြတ်သန်းစီးဆင်းသည်။
- **အုပ်ချုပ်မှုနှင့် လိုက်နာမှု-** ပါလီမန် (သို့မဟုတ် တူညီသော သင်ခန်းစာ) သည် လမ်းကြောဆိုင်ရာ သရုပ်ပြမှုများ၊ လုပ်ဆောင်နိုင်မှု သရုပ်ပြမှုများ၊ နှင့် အဖြူရောင်စာရင်း ပြောင်းလဲမှုများကို အတည်ပြုသည်။ လိုက်နာမှု သည် Norito manifests နှင့်အတူ အထောက်အထားအစုအဝေးများကို သိမ်းဆည်းထားသည်။

**မှီခိုမှု**

1. Lane catalog + dataspace catalog wiring (`docs/source/nexus_lanes.md`, `defaults/nexus/config.toml`)။
2. လမ်းကြောဆိုင်ရာ သက်သေပြခြင်း (`crates/iroha_core/src/governance/manifest.rs`၊ `crates/iroha_core/src/queue.rs`) တွင် တန်းစီစောင့်ဆိုင်းခြင်း။
3. စွမ်းဆောင်ရည် ထင်ရှားသည် + UAIDs (`crates/iroha_data_model/src/nexus/manifest.rs`)။
4. စီစဉ်ပေးသူ TEU ခွဲတမ်း + မက်ထရစ်များ (`integration_tests/tests/scheduler_teu.rs`၊ `docs/source/telemetry.md`)။

## 1. အကိုးအကား လမ်းသွယ် အပြင်အဆင်

### 1.1 လမ်းသွယ် ကက်တလောက်နှင့် ဒေတာနေရာ ထည့်သွင်းမှုများ

`[[nexus.lane_catalog]]` နှင့် `[[nexus.dataspace_catalog]]` သို့ သီးသန့်ထည့်သွင်းမှုများကို ထည့်ပါ။ အောက်ဖော်ပြပါဥပမာသည် `defaults/nexus/config.toml` အား အပေါက်တစ်ခုလျှင် 1500 TEU သိမ်းဆည်းထားပြီး ငတ်မွတ်ခေါင်းပါးမှုကို တွန်းလှန်ပေးသည့် အကွက်ခြောက်ခုအပြင် လက်ကားဘဏ်များနှင့် လက်လီပိုက်ဆံအိတ်များအတွက် ဒေတာနေရာအမည်တူများနှင့် ကိုက်ညီသောဒေတာနေရာကို တိုးချဲ့ထားသည်။

```toml
lane_count = 5

[[nexus.lane_catalog]]
index = 3
alias = "cbdc"
description = "Central bank CBDC lane"
dataspace = "cbdc.core"
visibility = "restricted"
lane_type = "cbdc_private"
governance = "central_bank_multisig"
settlement = "xor_dual_fund"
metadata.scheduler.teu_capacity = "1500"
metadata.scheduler.starvation_bound_slots = "6"
metadata.settlement.buffer_account = "buffer::cbdc_treasury"
metadata.settlement.buffer_asset = "61CtjvNd9T3THAR65GsMVHr82Bjc"
metadata.settlement.buffer_capacity_micro = "1500000000"
metadata.telemetry.contact = "ops@cb.example"

[[nexus.dataspace_catalog]]
alias = "cbdc.core"
id = 10
description = "CBDC issuance dataspace"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "cbdc.bank.wholesale"
id = 11
description = "Wholesale bank onboarding lane"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "cbdc.dapp.retail"
id = 12
description = "Retail wallets and programmable-money dApps"
fault_tolerance = 1

[nexus.routing_policy]
default_lane = 0
default_dataspace = "universal"

[[nexus.routing_policy.rules]]
lane = 3
dataspace = "cbdc.core"
[nexus.routing_policy.rules.matcher]
instruction = "cbdc::*"
description = "Route CBDC contracts to the restricted lane"
```

**မှတ်ချက်**

- `metadata.scheduler.teu_capacity` နှင့် `metadata.scheduler.starvation_bound_slots` သည် `integration_tests/tests/scheduler_teu.rs` ကျင့်သုံးသော TEU တိုင်းထွာများကို ကျွေးမွေးသည်။ အော်ပရေတာများသည် ၎င်းတို့အား လက်ခံမှုရလဒ်များနှင့် ထပ်တူပြုထားရမည်ဖြစ်ပြီး `nexus_scheduler_lane_teu_capacity` သည် ပုံစံပလိတ်နှင့် ကိုက်ညီပါသည်။
- အထက်ဖော်ပြပါ dataspace alias တိုင်းသည် အုပ်ချုပ်မှုဖော်ပြချက်များနှင့် လုပ်ဆောင်နိုင်စွမ်းကို ထင်ရှားစေရမည် (အောက်တွင်ကြည့်ပါ) ထို့ကြောင့် ဝင်ခွင့်သည် အလိုအလျောက်ပျံ့လွင့်မှုကို ပယ်ချပါသည်။

### 1.2 လမ်းကြော ထင်ရှားသော အရိုးစု

Lane သည် `nexus.registry.manifest_directory` (`crates/iroha_config/src/parameters/actual.rs` ကိုကြည့်ပါ)။ ဖိုင်အမည်များသည် လမ်းသွားနာမည်တူများ (`cbdc.manifest.json`) နှင့် ကိုက်ညီသင့်ပါသည်။ အစီအစဉ်သည် `integration_tests/tests/nexus/lane_registry.rs` တွင် အုပ်ချုပ်မှုဆိုင်ရာ ထင်ရှားသောစစ်ဆေးမှုများကို ထင်ဟပ်စေသည်။

```json
{
  "lane": "cbdc",
  "dataspace": "cbdc.core",
  "version": 1,
  "governance": "central_bank_multisig",
  "validators": [
    "i105...",
    "i105...",
    "i105...",
    "i105..."
  ],
  "quorum": 3,
  "protected_namespaces": [
    "cbdc.core",
    "cbdc.policy",
    "cbdc.settlement"
  ],
  "hooks": {
    "runtime_upgrade": {
      "allow": true,
      "require_metadata": true,
      "metadata_key": "cbdc_upgrade_id",
      "allowed_ids": [
        "upgrade-issuance-v1",
        "upgrade-pilot-retail"
      ]
    }
  },
  "composability_group": {
    "group_id_hex": "7ab3f9b3b2777e9f8b3d6fae50264a1e0ffab7c74542ff10d67fbdd073d55710",
    "activation_epoch": 2048,
    "whitelist": [
      "ds::cbdc.bank.wholesale",
      "ds::cbdc.dapp.retail"
    ],
    "quotas": {
      "group_teu_share_max": 500,
      "per_ds_teu_max": 250
    }
  }
}
```

အဓိကလိုအပ်ချက်များ-- အတည်ပြုပေးသူများသည် ** ကက်တလောက်တွင်ရှိသော I105 အကောင့် ID များ (`@domain` မပါ၀င်ပါ၊ `@domain` ကို ထပ်ဖြည့်ပါ)** ဖြစ်ရမည်** ဖြစ်ရမည်။ `quorum` ကို multisig threshold (≥2) သို့ သတ်မှတ်ပါ။
- Protected namespaces များကို `Queue::push` (`crates/iroha_core/src/queue.rs` တွင်ကြည့်ပါ) ဖြင့် ပြဋ္ဌာန်းထားသောကြောင့် CBDC စာချုပ်များအားလုံး `gov_namespace` + `gov_contract_id` ကို သတ်မှတ်ရပါမည်။
- `composability_group` အကွက်များသည် `docs/source/nexus.md` §8.6 တွင်ဖော်ပြထားသော schema ကိုလိုက်နာသည်။ ပိုင်ရှင် (CBDC လမ်းသွား) သည် whitelist နှင့် quotas ကို ပေးဆောင်သည်။ Whitelisted DS manifests သည် `group_id_hex` + `activation_epoch` ကိုသာ သတ်မှတ်ပါသည်။
- မန်နီးဖက်စ်ကို ကူးယူပြီးနောက်၊ `LaneManifestRegistry::from_config` တင်ကြောင်းအတည်ပြုရန် `cargo test -p integration_tests nexus::lane_registry -- --nocapture` ကိုဖွင့်ပါ။

### 1.3 စွမ်းရည်ပြသမှုများ (UAID မူဝါဒများ)

စွမ်းဆောင်ရည် ထင်ရှားသည် (`crates/iroha_data_model/src/nexus/manifest.rs` တွင် `AssetPermissionManifest`) သည် `UniversalAccountId` ကို သတ်မှတ်သည့် ထောက်ပံ့ကြေးများကို စည်းနှောင်ထားသည်။ ဘဏ်များနှင့် dApps များသည် လက်မှတ်ရေးထိုးထားသော မူဝါဒများကို ရယူနိုင်ရန် Space Directory မှတစ်ဆင့် ၎င်းတို့ကို ထုတ်ဝေပါ။

```json
{
  "version": 1,
  "uaid": "uaid:5f77b4fcb89cb03a0ab8f46d98a72d585e3b115a55b6bdb2e893d3f49d9342f1",
  "dataspace": 11,
  "issued_ms": 1762723200000,
  "activation_epoch": 2050,
  "expiry_epoch": 2300,
  "entries": [
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.transfer",
        "method": "transfer",
        "asset": "CBDC#centralbank",
        "role": "Initiator"
      },
      "effect": {
        "Allow": {
          "max_amount": "1000000000",
          "window": "PerDay"
        }
      },
      "notes": "Wholesale transfer allowance (per UAID)"
    },
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.kit",
        "method": "withdraw"
      },
      "effect": {
        "Deny": {
          "reason": "Withdrawals disabled for this UAID"
        }
      }
    }
  ]
}
```

- ခွင့်ပြုထားသောစည်းမျဉ်း (`ManifestVerdict::Denied`) နှင့် ကိုက်ညီသည့်တိုင် စည်းမျဉ်းများကို ငြင်းဆိုခြင်းဖြင့် အနိုင်ရပါက သက်ဆိုင်ရာ ခွင့်ပြုပြီးနောက် ပြတ်သားစွာ ငြင်းဆိုမှုများအားလုံးကို ထားလိုက်ပါ။
- ဖောက်သည်ကန့်သတ်ချက်များကို လှည့်ပတ်ရန်အတွက် အနုမြူငွေပေးချေမှုလက်ကိုင်များအတွက် `AllowanceWindow::PerSlot` နှင့် `PerMinute`/`PerDay` ကို အသုံးပြုပါ။
- UAID/dataspace တစ်ခုချင်းအတွက် မန်နီးဖက်စ်တစ်ခု လုံလောက်ပါသည်။ လုပ်ဆောင်ချက်များနှင့် သက်တမ်းကုန်ဆုံးမှုများသည် မူဝါဒလည်ပတ်မှုပုံစံကို ကျင့်သုံးသည်။
- ယခုအခါ `expiry_epoch` ကိုရောက်ရှိသည်နှင့်တစ်ပြိုင်နက် လမ်းကြောဖွင့်ချိန်သည် အလိုအလျောက် သက်တမ်းကုန်ဆုံးသွားပါသည်။
  ထို့ကြောင့် စစ်ဆင်ရေးအဖွဲ့များသည် `SpaceDirectoryEvent::ManifestExpired` ကို စောင့်ကြည့်ရုံသာ၊
  `nexus_space_directory_revision_total` မြစ်ဝကျွန်းပေါ်ဒေသကို သိမ်းဆည်းပြီး Torii ရှိုးများကို အတည်ပြုပါ
  `status = "Expired"`။ CLI `manifest expire` အမိန့်သည် ဆက်လက်အသုံးပြုနိုင်ပါသည်။
  လူကိုယ်တိုင် အစားထိုးမှုများ သို့မဟုတ် အထောက်အထားများ ဖြည့်စွက်မှုများ။

## 2. Bank Onboarding & Whitelist Workflow

| အဆင့် | ပိုင်ရှင်(များ) | လုပ်ဆောင်ချက်များ | အထောက်အထား |
|---------|----------|---------|----------|
| 0. စားသုံးမှု | CBDC PMO | KYC စာရွက်စာတမ်း၊ နည်းပညာဆိုင်ရာ DS မန်နီးဖက်စ်၊ အတည်ပြုသူစာရင်း၊ UAID မြေပုံကို စုဆောင်းပါ။ | ဝင်ခွင့်လက်မှတ်၊ DS manifest မူကြမ်းကို ရေးထိုးထားသည်။ |
| 1. အုပ်ချုပ်မှုအတည်ပြုချက် | လွှတ်တော် / လိုက်နာမှု | စားသုံးမှုအထုပ်ကို ပြန်လည်သုံးသပ်ပါ၊ တံဆိပ် `cbdc.manifest.json`၊ `AssetPermissionManifest` ကို အတည်ပြုပါ။ | လက်မှတ်ရေးထိုးထားသော အုပ်ချုပ်မှုမိနစ်များ၊ ဖော်ပြပါ ဟက်ရှ်။ |
| 2. စွမ်းဆောင်ရည်ထုတ်ပေးခြင်း | CBDC လမ်းကြော ops | `norito::json::to_string_pretty` မှတစ်ဆင့် ကုဒ်နံပါတ်ကို ကုဒ်ဖော်ပြခြင်း၊ Space Directory အောက်တွင် သိမ်းဆည်းပါ၊ အော်ပရေတာများကို အကြောင်းကြားပါ။ | Manifest JSON + norito `.to` ဖိုင်၊ BLAKE3 မှတ်တမ်း။ |
| 3. Whitelist အသက်သွင်းခြင်း | CBDC လမ်းကြော ops | DSID ကို `composability_group.whitelist` သို့ ပေါင်းထည့်ပါ၊ bump `activation_epoch`၊ မန်နီးဖက်စ်ကို ဖြန့်ဝေပါ။ လိုအပ်ပါက dataspace routing ကို update လုပ်ပါ။ | ထင်ရှားသော ကွဲပြားမှု၊ `kagami config diff` ထုတ်ပေးမှု၊ အုပ်ချုပ်မှုအတည်ပြုချက် ID။ |
| 4. တရားဝင်အတည်ပြုချက် | QA Guild / Ops | ပေါင်းစည်းမှုစမ်းသပ်မှုများ၊ TEU ဝန်စမ်းသပ်မှုများနှင့် ပရိုဂရမ်သွင်းနိုင်သော-ငွေပြန်ဖွင့်ခြင်းတို့ကို လုပ်ဆောင်ပါ (အောက်တွင်ကြည့်ပါ)။ | `cargo test` မှတ်တမ်းများ၊ TEU ဒက်ရှ်ဘုတ်များ၊ ပရိုဂရမ်ထုတ်နိုင်သော ငွေကြေးဆိုင်ရာ ရလဒ်များ။ |
| 5. အထောက်အထား မော်ကွန်း | လိုက်နာမှု WG | အစုအဝေးဖော်ပြချက်များ၊ အတည်ပြုချက်များ၊ စွမ်းရည်အချေအတင်များ၊ စမ်းသပ်မှုရလဒ်များနှင့် `artifacts/nexus/cbdc_<stamp>/` အောက်တွင် Prometheus ခြစ်ခြင်းများ။ | စာရွက်စာတမ်း၊ ချက်လက်မှတ်ဖိုင်၊ ကောင်စီလက်မှတ်ထိုးခြင်း |

### စာရင်းစစ်အတွဲအကူSpace Directory မှ `iroha app space-directory manifest audit-bundle` အထောက်အကူကို အသုံးပြုပါ။
အထောက်အထားအစုံအလင်ကို မတင်ပြမီ စွမ်းရည်တစ်ခုစီကို လျှပ်တစ်ပြက်ရိုက်ရန် playbook။
manifest JSON (သို့မဟုတ် `.to` payload) နှင့် dataspace ပရိုဖိုင်ကို ပေးပါ။

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z/cbdc_wholesale \
  --notes "CBDC wholesale refresh"
```

ညွှန်ကြားချက်သည် Canonical JSON/Norito/hash ကော်ပီများနှင့်အတူ ထုတ်လွှတ်သည်
UAID၊ dataspace id၊ activation/expiry ကို မှတ်တမ်းတင်သည့် `audit_bundle.json`
လိုအပ်သည့်အရာများကို ပြဋ္ဌာန်းနေစဉ် အပိုင်းများ၊ ထင်ရှားသော hash နှင့် ပရိုဖိုင်စာရင်းစစ်ချိတ်များ
`SpaceDirectoryEvent` စာရင်းသွင်းမှုများ။ အထောက်အထား အစုအဝေးကို ချထားပါ။
စာရင်းစစ်များနှင့် စည်းကမ်းထိန်းသိမ်းရေးမှူးများသည် နောက်ပိုင်းတွင် အတိအကျ bytes ပြန်ဖွင့်နိုင်သောကြောင့် လမ်းညွှန်။

### 2.1 ညွှန်ကြားချက်များနှင့် အတည်ပြုချက်များ

1. **လမ်းကြော ထင်ရှားသည်-** `cargo test -p integration_tests nexus::lane_registry -- --nocapture lane_manifest_registry_loads_fixture_manifests`။
2. **စီစဉ်သူခွဲတမ်း-** `cargo test -p integration_tests scheduler_teu -- queue_teu_backlog_matches_metering queue_routes_transactions_across_configured_lanes`။
3. **Manual smoke-** `irohad --sora --config configs/soranexus/nexus/config.toml --chain 0000…` သည် CBDC ဖိုင်များကိုညွှန်ပြသည့် manifest directory ပါရှိသော၊ ထို့နောက် `/v1/sumeragi/status` ကိုနှိပ်ပြီး CBDC လမ်းကြောအတွက် `lane_governance.manifest_ready=true` ကိုအတည်ပြုပါ။
4. **Whitelist တစ်ပြေးညီစမ်းသပ်မှု-** `cargo test -p integration_tests nexus::cbdc_whitelist -- --nocapture` လေ့ကျင့်ခန်း `integration_tests/tests/nexus/cbdc_whitelist.rs`၊ `fixtures/space_directory/profile/cbdc_lane_profile.json` ကို ပိုင်းခြားပြီး ကိုးကားထားသည့် စွမ်းရည်သည် အဖြူစာရင်းထဲ ထည့်သွင်းမှု၏ UAID၊ ဒေတာနေရာလွတ်၊ အသက်ဝင်သည့် အပိုင်း 5X နှင့် I10NT0000000 အောက်တွင် တူညီကြောင်း သေချာစေရန် ရည်ညွှန်းထားသော စွမ်းဆောင်ရည်ကို ထင်ရှားစေသည်။ `fixtures/space_directory/capability/`။ စစ်ဆေးမှုမှတ်တမ်းကို NX-6 အထောက်အထားအစုအဝေးတွင် အဖြူရောင်စာရင်း သို့မဟုတ် ဖော်ပြချက်များ ပြောင်းလဲသည့်အခါတိုင်း ပူးတွဲပါ။

### 2.2 CLI အတိုအထွာများ

- `cargo run -p iroha_cli -- manifest cbdc --uaid <hash> --dataspace 11 --template cbdc_wholesale` မှတစ်ဆင့် UAID + ထင်ရှားသောအရိုးစုကို ဖန်တီးပါ။
- `iroha app space-directory manifest publish --manifest cbdc_wholesale.manifest.to` (သို့မဟုတ် `--manifest-json cbdc_wholesale.manifest.json`) ကို အသုံးပြု၍ Torii (Space Directory) သို့ ထုတ်ဝေနိုင်မှု ထင်ရှားသည်။ တင်သွင်းသည့်အကောင့်သည် CBDC ဒေတာနေရာအတွက် `CanPublishSpaceDirectoryManifest` ကို ကိုင်ထားရပါမည်။
- ops desk သည် အဝေးမှ အလိုအလျောက်စနစ် လုပ်ဆောင်နေပါက HTTP မှတဆင့် ထုတ်ဝေပါ-

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "i105...",
            "private_key": "ed25519:CiC7…",
            "manifest": '"'"'$(cat fixtures/space_directory/capability/cbdc_wholesale.manifest.json)'"'"',
            "reason": "CBDC onboarding wave 4"
          }'
  ```

  Torii သည် ထုတ်ဝေမှုငွေလွှဲခြင်းကို တန်းစီထားသည်နှင့်တပြိုင်နက် `202 Accepted` ကို ပြန်ပေးသည်။ အတူတူပါပဲ။
  CIDR/API-တိုကင်ဂိတ်များ သက်ရောက်ပြီး ကွင်းဆက်ခွင့်ပြုချက်လိုအပ်ချက်နှင့် ကိုက်ညီပါသည်။
  CLI အလုပ်အသွားအလာ။
- Torii သို့ ပို့စ်တင်ခြင်းဖြင့် အဝေးမှ အရေးပေါ် ရုတ်သိမ်းခြင်းကို ထုတ်ပေးနိုင်ပါသည်။

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests/revoke \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "i105...",
            "private_key": "ed25519:CiC7…",
            "uaid": "uaid:0f4d…ab11",
            "dataspace": 11,
            "revoked_epoch": 9216,
            "reason": "Fraud investigation #NX-16-R05"
          }'
  ```

  Torii သည် `202 Accepted` သည် ငွေလွှဲခြင်းကို ရုပ်သိမ်းပြီးသည်နှင့် တန်းစီနေသည်၊ အတူတူပါပဲ။
  CIDR/API-တိုကင်ဂိတ်များသည် အခြားအက်ပ် အဆုံးမှတ်များအဖြစ် သက်ရောက်ပြီး `CanPublishSpaceDirectoryManifest`
  ကွင်းဆက်တွင် လိုအပ်နေသေးသည်။
- ခွင့်ပြုထားသောစာရင်းတွင် အဖွဲ့ဝင်ခြင်းကို လှည့်ပါ- `cbdc.manifest.json` ကိုတည်းဖြတ်ပါ၊ Bum `activation_epoch` နှင့် လုံခြုံသောမိတ္တူကို အတည်ပြုသူအားလုံးထံ ပြန်လည်အသုံးချပါ။ `LaneManifestRegistry` ပြင်ဆင်သတ်မှတ်ထားသော မဲရုံကြားကာလတွင် ဟော့-ပြန်တင်မှုများ။

## 3. လိုက်နာမှု အထောက်အထား အစုအဝေး

`artifacts/nexus/cbdc_rollouts/<YYYYMMDDThhmmZ>/` အောက်တွင် ရှေးဟောင်းပစ္စည်းများကို သိမ်းဆည်းပြီး အုပ်ချုပ်မှုလက်မှတ်တွင် မှတ်တမ်းကို ပူးတွဲပါ။

| ဖိုင် | ဖော်ပြချက် |
|--------|-------------|
| `cbdc.manifest.json` | ခွင့်ပြုထားသောစာရင်းကွဲပြားမှု (ရှေ့/နောက်) ဖြင့် လက်မှတ်ထိုးထားသော လမ်းကြောကို ထင်ရှားစေသည်။ |
| `capability/<uaid>.manifest.json` & `.to` | Norito + JSON စွမ်းရည်သည် UAID တစ်ခုစီအတွက် ဖော်ပြသည်။ |
| `compliance/kyc_<bank>.pdf` | Regulator-facing KYC ထောက်ခံချက်။ |
| `metrics/nexus_scheduler_lane_teu_capacity.prom` | Prometheus TEU ခေါင်းခန်းကို သက်သေပြသည့်ခြစ် |
| `tests/cargo_test_nexus_lane_registry.log` | မန်နီးဖက်စ် အစမ်းပြေးမှုမှ မှတ်တမ်း။ |
| `tests/cargo_test_scheduler_teu.log` | TEU လမ်းကြောင်း ဖြတ်သန်းမှုများကို သက်သေပြသည့် မှတ်တမ်း။ |
| `programmable_money/axt_replay.json` | ပရိုဂရမ်-ငွေကြေး အပြန်အလှန်လုပ်ဆောင်နိုင်မှုကို သရုပ်ပြသည့် စာသားမှတ်တမ်းကို ပြန်ဖွင့်ပါ (အပိုင်း 4 ကိုကြည့်ပါ)။ |
| `approvals/governance_minutes.md` | လက်မှတ်ရေးထိုးထားသော အတည်ပြုချက်မိနစ်များကို ဖော်ပြခြင်း hash + activation ကာလ။ |**အတည်ပြုချက် script-** `ci/check_cbdc_rollout.sh` ကို အုပ်ချုပ်မှု လက်မှတ်တွင် မတွဲမီ အထောက်အထား အစုအဝေး ပြီးမြောက်ကြောင်း အခိုင်အမာ အတည်ပြုရန်။ ကူညီသူသည် `cbdc.manifest.json` တစ်ခုစီအတွက် `artifacts/nexus/cbdc_rollouts/` (သို့မဟုတ် `CBDC_ROLLOUT_BUNDLE=<path>`) ကို စကင်န်ဖတ်ပြီး သရုပ်ဖော်ခြင်း/ပေါင်းစပ်နိုင်မှုအုပ်စုကို ခွဲခြမ်းစိတ်ဖြာပြီး မန်နီးဖက်စ်တစ်ခုစီတွင် `.to` ဖိုင်နှင့် ကိုက်ညီသော `.to` ဖိုင်ကို စစ်ဆေးပြီး၊ 50018NIX အတွက် စစ်ဆေးမှုများ၊ မက်ထရစ်များ ပေါင်း `cargo test` မှတ်တမ်းများကို ခြစ်ပြီး ပရိုဂရမ်-ငွေဖြင့် ပြန်ဖွင့်သည့် JSON ကို အတည်ပြုပြီး အတည်ပြုချက် မိနစ်များကို အသက်ဝင်စေသည့် အပိုင်းကို ကိုးကားပြီး ထင်ရှားသော hash ကို သေချာစေသည်။ နောက်ဆုံး Rev သည် NX-6 တွင်ဖော်ပြထားသော ဘေးကင်းရေးရထားများကိုပါ ထည့်သွင်းပေးသည်- ကော်ရမ်သည် ကြေညာထားသော တရားဝင်သတ်မှတ်မှုထက် မကျော်လွန်နိုင်ပါ၊ ကာကွယ်ထားသော namespaces များသည် အချည်းနှီးမဟုတ်သော စာကြောင်းများဖြစ်ရမည်၊ စွမ်းဆောင်ရည်ဖော်ပြချက်များသည် `activation_epoch`/`expiry_epoch` အတွဲများကို ကောင်းမွန်စွာပုံစံဖြင့် တိုးမြှင့်ကြောင်း ကြေငြာရပါမည်။ `Allow`/`Deny` အထူးပြုလုပ်ချက်များ။ `fixtures/nexus/cbdc_rollouts/` အောက်တွင် နမူနာ အထောက်အထား ထုပ်ပိုးခြင်းကို ပေါင်းစပ်စမ်းသပ်မှု `integration_tests/tests/nexus/cbdc_rollout_bundle.rs` ဖြင့် ကျင့်သုံးပြီး မှန်ကန်သော အထောက်အထားကို CI သို့ ကြိုးဖြင့် ချိတ်ဆက်ထားသည်။

## 4. Programmable-Money အပြန်အလှန်လုပ်ဆောင်နိုင်မှု

ဘဏ်တစ်ခု (dataspace 11) နှင့် လက်လီ dApp (dataspace 12) နှစ်ခုလုံးကို တူညီသော `ComposabilityGroupId` တွင် တရားဝင်စာရင်းသွင်းပြီးသည်နှင့်၊ ပရိုဂရမ်ထုတ်နိုင်သော ငွေကြေးစီးဆင်းမှုများသည် `docs/source/nexus.md` §8.6 မှ AXT ပုံစံအတိုင်း လိုက်နာပါသည်-

1. လက်လီရောင်းချသည့် dApp သည် ၎င်း၏ UAID + AXT အချေအတင်နှင့် ချိတ်ဆက်ထားသော ပိုင်ဆိုင်မှုလက်ကိုင်တစ်ခုကို တောင်းဆိုသည်။ CBDC လမ်းကြောသည် `AssetPermissionManifest::evaluate` မှတစ်ဆင့် လက်ကိုင်ကို အတည်ပြုသည် (အနိုင်ရခြင်း၊ ထောက်ပံ့ကြေးများ ပြဋ္ဌာန်းထားသည်)။
2. DS နှစ်ခုလုံးသည် တူညီသောပေါင်းစပ်နိုင်မှုအုပ်စုကိုကြေငြာထားသောကြောင့် လမ်းကြောင်းသည် ၎င်းတို့အား အဏုမြူပါဝင်ရန်အတွက် CBDC လမ်းကြောသို့ ပြိုကျသွားသည် (`LaneRoutingPolicy` သည် နှစ်ဦးနှစ်ဖက်ခွင့်ပြုထားသောစာရင်းတွင် `group_id` ကိုအသုံးပြုသည်)။
3. ကွပ်မျက်စဉ်အတွင်း၊ CBDC DS သည် ၎င်း၏ဆားကစ်အတွင်း AML/KYC အထောက်အထားများကို ပြဋ္ဌာန်းသည် (`use_asset_handle` pseudocode in `nexus.md`)၊ dApp DS သည် CBDC အစိတ်စိတ်အမွှာမွှာအောင်မြင်ပြီးမှသာ ဒေသတွင်းစီးပွားရေးအခြေအနေကို အပ်ဒိတ်လုပ်သည်။
4. အထောက်အထားပစ္စည်း (FASTPQ + DA ကတိကဝတ်များ) သည် CBDC လမ်းကြောတွင် တည်ရှိနေပါသည်။ ပေါင်းစည်း-လယ်ဂျာတွင် ထည့်သွင်းမှုများသည် လျှို့ဝှက်ဒေတာပေါက်ကြားခြင်းမရှိဘဲ ကမ္ဘာ့နိုင်ငံတော်၏ အဆုံးအဖြတ်ကို ထိန်းသိမ်းထားသည်။

programmable-money replay archive တွင်-

- AXT ဖော်ပြချက် + တောင်းဆိုချက်/တုံ့ပြန်မှုများကို ကိုင်တွယ်ပါ။
- Norito-ကုဒ်ဝှက်ထားသော ငွေပေးငွေယူစာအိတ်။
- ရလဒ်လက်ခံဖြတ်ပိုင်းများ (မင်္ဂလာလမ်း၊ ငြင်းပယ်လမ်းကြောင်း)။
- `telemetry::fastpq.execution_mode`၊ `nexus_scheduler_lane_teu_slot_committed` နှင့် `lane_commitments` အတွက် Telemetry အတိုအထွာများ။

## 5. Observability & Runbooks

- **Metrics-** Monitor `nexus_scheduler_lane_teu_capacity`, `nexus_scheduler_lane_teu_slot_committed`, `nexus_scheduler_lane_teu_deferral_total{reason}`, `governance_manifest_admission_total`, နှင့် `lane_governance_sealed_total` (`docs/source/telemetry.md` ကိုကြည့်ပါ)။
- **ဒိုင်ခွက်များ-** CBDC လမ်းသွားအတန်းဖြင့် `docs/source/grafana_scheduler_teu.json` ကို တိုးချဲ့ပါ။ whitelist churn အတွက် panels များထည့်ပါ (အသက်သွင်းသည့်အချိန်တိုင်းတွင် မှတ်ချက်များ) နှင့် သက်တမ်းကုန်ဆုံးနိုင်သော ပိုက်လိုင်းများ ပါဝင်သည်။
- **သတိပေးချက်များ-** `rate(nexus_scheduler_lane_teu_deferral_total{reason="cap_exceeded"}[5m]) > 0` ကို 15 မိနစ်ကြာသည့်အခါ သို့မဟုတ် `lane_governance.manifest_ready=false` သည် စစ်တမ်းကာလတစ်ခုထက် ကျော်လွန်နေသည့်အခါ အစပျိုးပါ။
- **Runbook ညွှန်ပြချက်များ-** `docs/source/governance_api.md` တွင် ကာကွယ်ထားသော-namespace လမ်းညွှန်ချက်နှင့် `docs/source/nexus.md` တွင် ပရိုဂရမ်ထုတ်နိုင်သော ငွေကြေးပြဿနာဖြေရှင်းခြင်းသို့ လင့်ခ်ချိတ်ပါ။

## 6. လက်ခံမှုစစ်ဆေးစာရင်း- [ ] CBDC လမ်းကြောကို `nexus.lane_catalog` တွင် TEU မက်တာဒေတာဖြင့် TEU စမ်းသပ်မှုများနှင့် ကိုက်ညီကြောင်း ကြေညာထားသည်။
- [ ] `cbdc.manifest.json` ကို `cargo test -p integration_tests nexus::lane_registry` မှတစ်ဆင့် အတည်ပြုထားသော မန်နီးဖက်စ်လမ်းညွှန်တွင် ပါရှိသည့် `cbdc.manifest.json` ကို လက်မှတ်ရေးထိုးခဲ့သည်။
- [ ] စွမ်းဆောင်ရည်ကို UAID တိုင်းအတွက် ထုတ်ပြန်ပြီး Space Directory တွင် သိမ်းဆည်းထားသည်။ ယူနစ်စစ်ဆေးမှုများ (`crates/iroha_data_model/src/nexus/manifest.rs`) မှတစ်ဆင့် အတည်ပြုထားသော ရှေ့တန်းကို ငြင်းပယ်/ခွင့်ပြုပါ။
- [ ] အုပ်ချုပ်မှုခွင့်ပြုချက် ID၊ `activation_epoch` နှင့် Prometheus အထောက်အထားများဖြင့် မှတ်တမ်းတင်ထားသော Whitelist activation
- [ ] Programmable-money replay ကို သိမ်းဆည်းပြီး၊ ထုတ်ပေးခြင်း၊ ငြင်းဆိုခြင်းနှင့် စီးဆင်းမှုများကို ခွင့်ပြုခြင်းတို့ကို ကိုင်တွယ်သရုပ်ပြခြင်း။
- [ ] အထောက်အထားအစုအဝေးကို 🈯 မှ 🈺 မှ 🈺 မှ ဘွဲ့ရပြီးသည်နှင့် `status.md` မှ အုပ်ချုပ်မှုလက်မှတ်မှ လင့်ခ်ချိတ်ထားသော cryptographic digest ဖြင့် အပ်လုဒ်လုပ်ထားသော အထောက်အထားအတွဲ။

ဤပြခန်းစာအုပ်ကို လိုက်နာခြင်းဖြင့် NX-6 အတွက် ပေးပို့နိုင်သော စာရွက်စာတမ်းအား ကျေနပ်စေပြီး CBDC လမ်းကြောဖွဲ့စည်းမှုပုံစံ၊ တရားဝင်စာရင်းသွင်းနိုင်မှုနှင့် ပရိုဂရမ်လုပ်နိုင်သော ငွေကြေး အပြန်အလှန်လုပ်ဆောင်နိုင်မှုတို့အတွက် တိကျသောပုံစံကို ပေးခြင်းဖြင့် အနာဂတ်လမ်းပြမြေပုံပါအရာများ (NX-12/NX-15) ကို ပိတ်ဆို့သွားပါမည်။