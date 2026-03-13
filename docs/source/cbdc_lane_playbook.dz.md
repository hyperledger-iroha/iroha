---
lang: dz
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

# CBDC སྒེར་གྱི་ལམ་དེབ་ (NX-6)

> **རད་མེཔ་མཐུད་ལམ་:** NX-6 (CBDC སྒེར་གྱི་ལེན་ལེན་ཊེམ་པེལེཊི་དང་ དཀར་མདངས་རྒྱུན་འབབ་) དང་ NX-14 (Nexus རན་བུཀསི)།  
>*ཇོ་བདག་:** དངུལ་འབྲེལ་ཞབས་ཏོག་ WG, Nexus ཀོར་ཌབ་ལུ་ཇི་, མཐུན་སྒྲིག་ཌབ་ལུ་ཨེག།  
> **Status:** ཟིན་བྲིས་ — ལག་ལེན་འཐབ་པའི་ ཧུཀ་ཚུ་ `crates/iroha_data_model::nexus`, `crates/iroha_core::governance::manifest`, དང་ `integration_tests/tests/nexus/lane_registry.rs` ཚུ་ནང་ ཡོདཔ་ཨིན། འ་ནི་རྩེད་དེབ་འདི་གིས་ གཞི་བསྟུན་རིམ་སྒྲིག་དང་ ལཱ་གི་རྒྱུན་རིམ་ཚུ་ ཡིག་ཐོག་ལུ་བཀོད་དེ་ཡོདཔ་ལས་ སི་བི་ཌི་སི་བཀྲམ་སྤེལ་འདི་ གཏན་འབེབས་བཟོ་ཚུགས།

## ཁྱབ་ཁོངས་དང་འགན་ཁུར།

- **དབུས་དངུལ་ཁང་གི་ལམ་ (“CBDC ལམ་”):** ཆོག་མཆན་བདེན་དཔྱད་འབད་མི་ དོ་དམ་གཞིས་ཆགས་བཱ་ཕར་ཚུ་ དེ་ལས་ ལས་རིམ་བཟོ་ཚུགས་པའི་ ཏི་རུ་གི་སྲིད་བྱུས། བཀག་ཆ་འབད་ཡོད་པའི་གནད་སྡུད་ས་སྟོང་ + ལྕགས་ཆ་ཆ་གཅིག་སྦེ་གཡོག་བཀོལཝ་ཨིན།
- **Wholesale/retail bank dataspass:** UAIDs ཚུ་འཛིན་མི་ DS བཅའ་མར་གཏོགས་མི་ DS གིས་ ལྕོགས་གྲུབ་ཀྱི་མངོན་གསལ་ཚུ་ཐོབ་ཚུགསཔ་ཨིནམ་དང་ རྡུལ་ཕྲན་ AXT གི་དོན་ལུ་ CBDC ལམ་དང་གཅིག་ཁར་ དཀརཔོ་ཐོ་ཡིག་བཀོད་ཚུགས།
- **ལས་རིམ་བཟོ་བཏུབ་པའི་དངུལ་ dApps:** ཕྱིའི་ཌི་ཨེསི་གིས་ CBDC འདི་ དཀརཔོ་ཐོ་ཡིག་བཟོ་ཚརཝ་ད་ `ComposabilityGroup` འགྲུལ་ལམ་བརྒྱུད་དེ་ འགྱོཝ་ཨིན།
- **གཞུང་སྐྱོང་དང་ བསྟར་སྤྱོད་:** སྤྱི་ཚོགས་ (ཡང་ན་ དེ་དང་འདྲ་མཉམ་གྱི་ཚད་གཞི་) གིས་ ལམ་ཕྱོགས་ཀྱི་གསལ་སྟོན་ཚུ་ ཆ་འཇོག་འབདཝ་ཨིན། བསྟར་སྤྱོད་ཀྱིས་ I1NT00000003X གི་གསལ་སྟོན་དང་གཅིག་ཁར་ སྒྲུབ་བྱེད་ཚུ་ གསོག་འཇོག་འབདཝ་ཨིན།

**བརྟེན་པ་**

1. ལམ་གྱི་ཐོ་གཞུང་ + གནས་སྡུད་ས་སྟོང་ཐོ་གཞུང་གློག་ཐག་ (`docs/source/nexus_lanes.md`, `defaults/nexus/config.toml`).
2. ལམ་གྱི་གསལ་སྟོན་བསྟར་སྤྱོད་ (`crates/iroha_core/src/governance/manifest.rs`, `crates/iroha_core/src/queue.rs` ནང་ གྱལ་རིམ་གྱི་ གྱལ་རིམ་ཐོབ་ཐངས།)
3. ལྕོགས་གྲུབ་ཀྱི་གསལ་སྟོན་ + ཡུ་ཨེ་ཨའི་ཌི་ (`crates/iroha_data_model/src/nexus/manifest.rs`).
4. ཐོ་གཞུང་ TEU quotas + མེ་ཊིགསི་ (`integration_tests/tests/scheduler_teu.rs`, `docs/source/telemetry.md`).

## 1. དཔྱད་གཞིའི་སྒྲིག་ལམ།

### 1.1 ལམ་གྱི་ཐོ་གཞུང་དང་ གནད་སྡུད་ས་སྒོའི་ཐོ་འགོད་ཚུ།

`[[nexus.lane_catalog]]` དང་ `[[nexus.dataspace_catalog]]` ལུ་ བརྩོན་ཤུགས་ཅན་གྱི་ཐོ་བཀོད་ཚུ་ཁ་སྐོང་འབད། དཔེར་ན་ Norito འདི་ CBDC ལམ་དང་གཅིག་ཁར་ རྒྱ་སྐྱེད་འབད་དེ་ཡོདཔ་ད་ དེ་ཡང་ CBDC གི་ལམ་དང་གཅིག་ཁར་ 1500 TEU རེ་ལུ་ TEU དང་ ཐོར་ཊིལ་ཚུ་ བཀྲེས་ལྟོགས་དྲུག་ལུ་ བཞག་སྟེ་ཡོདཔ་མ་ཚད་ དངུལ་ཁང་དང་ཚོང་ཁང་ཚུ་གི་དོན་ལུ་ མཐུན་སྒྲིག་གནས་སྡུད་ས་སྒོ་ཚུ་ མཐུན་སྒྲིག་འབདཝ་ཨིན།

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
metadata.settlement.buffer_asset = "xor#sora"
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

**དྲན་འཛིན་**

- `metadata.scheduler.teu_capacity` དང་ `metadata.scheduler.starvation_bound_slots` `integration_tests/tests/scheduler_teu.rs` གིས་ལག་ལེན་འཐབ་མི་ ཊི་ཨི་ཡུ་ འཇལ་ཚད་ཚུ་ ཕིཌ་འབདཝ་ཨིན། བཀོལ་སྤྱོད་པ་ཚུ་གིས་ ངོས་ལེན་གྲུབ་འབྲས་དང་གཅིག་ཁར་ དུས་མཉམ་སྦེ་བཞག་དགོཔ་ལས་ `nexus_scheduler_lane_teu_capacity` ཚུ་ ཊེམ་པེལེཊི་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།
- གོང་འཁོད་ཀྱི་གནད་སྡུད་གཞན་ཆ་མཉམ་ གཞུང་སྐྱོང་མངོན་གསལ་ནང་འོང་དགོཔ་དང་ ལྕོགས་གྲུབ་ཀྱི་གསལ་སྟོན་ཚུ་ ༼འོག་ལུ་བལྟ་༽ དེ་འབདཝ་ལས་ འཛུལ་ཞུགས་བཀག་ཆ་བཀག་ཆ་འདི་ རང་བཞིན་གྱིས་ཨིན།

### 1.2 ལམ་ཕྱོགས་ཀྱི་ཀེང་རུས་དང་།

ལམ་ཐིག་འདི་གིས་ `nexus.registry.manifest_directory` བརྒྱུད་དེ་ རིམ་སྒྲིག་འབད་ཡོད་པའི་ སྣོད་ཐོ་འོག་ལུ་ ཐད་རི་བ་རི་ གསལ་སྟོན་འབདཝ་ཨིན། ཡིག་སྣོད་མིང་ཚུ་ ལམ་གྱི་མཉམ་མཐུན་ཚུ་དང་མཐུན་དགོཔ་ཨིན། (`cbdc.manifest.json`) འཆར་གཞི་འདི་གིས་ གཞུང་སྐྱོང་གསལ་སྟོན་གྱི་ བརྟག་དཔྱད་ཚུ་ `integration_tests/tests/nexus/lane_registry.rs` ནང་ལུ་ གསལ་སྟོན་འབདཝ་ཨིན།

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

གཙོ་བོའི་དགོས་མཁོ།- བདེན་དཔྱད་འབད་མི་ **must** be canonical I105 རྩིས་ཐོ་ IDs (མེད་ `@domain`; མེན) ཐོ་གཞུང་ནང་ལུ་ཡོད་པའི་ `@domain` འདི་ གསལ་ཏོག་ཏོ་སྦེ་ ལམ་སྟོན་འབད་མི་བརྡ་སྟོན་སྦེ་རྐྱངམ་ཅིག་ཨིན། མང་སིག་ཚད་གཞི་ (≥2) ལུ་ `quorum` གཞི་སྒྲིག་འབད།
- ཉེན་སྲུང་འབད་ཡོད་པའི་མིང་གི་ས་སྒོ་ཚུ་ `Queue::push` གིས་ བསྟར་སྤྱོད་འབད་ཡོདཔ་ལས་ (Norito བལྟ།) དེ་འབདཝ་ལས་ CBDC གན་རྒྱ་ཆ་མཉམ་གྱིས་ `gov_namespace` + `gov_contract_id` གསལ་བཀོད་འབད་དགོ།
- `composability_group` ས་ཁོངས་ཚུ་གིས་ `docs/source/nexus.md` §8.6 ནང་གསལ་བཀོད་འབད་ཡོད་པའི་ལས་རིམ་གྱི་རྗེས་སུ་འབྲང་ཡོདཔ་ཨིན། ཇོ་བདག་ (CBDC lane) གིས་ ཐོ་ཡིག་དཀརཔོ་དང་ བསྡོམས་རྩིས་ཚུ་ བཀྲམ་སྤེལ་འབདཝ་ཨིན། དཀརཔོ་ཐོ་ཡིག་བཀོད་ཡོད་པའི་ཌི་ཨེསི་གིས་ `group_id_hex` + `activation_epoch` རྐྱངམ་ཅིག་གསལ་བཀོད་འབདཝ་ཨིན།
- གསལ་སྟོན་འདི་འདྲ་བཤུས་རྐྱབ་པའི་ཤུལ་ལས་ `cargo test -p integration_tests nexus::lane_registry -- --nocapture` འདི་ གཡོག་བཀོལ་མི་འདི་གིས་ Norito འདི་མངོན་གསལ་འབདཝ་ཨིན།

### 1.3 ལྕོགས་གྲུབ་གསལ་སྟོན་ (ཡུ་ཨེ་ཨའི་ཌི་སྲིད་བྱུས།)

ལྕོགས་གྲུབ་ཀྱི་གསལ་སྟོན་ (`AssetPermissionManifest` in Norito) གིས་ `UniversalAccountId` གིས་ གཏན་འབེབས་འཐུས་ལུ་ མཐུད་ཡོདཔ་ཨིན། དེ་ཚུ་ གནམ་སྟོང་སྣོད་ཐོ་བརྒྱུད་དེ་ དཔར་བསྐྲུན་འབད་དེ་ དངུལ་ཁང་དང་ ཌི་ཨེཔ་ཚུ་གིས་ མཚན་རྟགས་བཀོད་ཡོད་པའི་ སྲིད་བྱུས་ཚུ་ ཐོབ་ཚུགས།

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

- ལམ་ལུགས་མཐུན་སྒྲིག་འབད་ཆོག་པའི་དུས་ཚོད་ (`ManifestVerdict::Denied`) ནང་ལུ་ཡང་ ལམ་ལུགས་ཚུ་ཆ་འཇོག་འབད་ནི་ལས་ ཁས་མ་ལེན།
- རྡུལ་ཕྲན་སྤྲོད་ལེན་གྱི་ ལག་ཆ་ཚུ་གི་དོན་ལུ་ `AllowanceWindow::PerSlot` ལག་ལེན་འཐབ།
- ཡུ་ཨེ་ཨའི་ཌི་/གནས་སྡུད་ས་ཆའི་ལངམ་རེ་ལུ་ མངོན་གསལ་གཅིག་པ། སྲིད་བྱུས་བསྒྱིར་ཚད་ཀྱི་ འཛུལ་སྤྱོད་ཚུ་ ཤུགས་ལྡན་བཟོ་ནི་དང་ དུས་ཡོལ་ཚུ་ བསྟར་སྤྱོད་འབདཝ་ཨིན།
- ལམ་གྱི་གཡོག་བཀོལ་བའི་དུས་ཚོད་འདི་ ད་ལྟོ་ `expiry_epoch` ལུ་ལྷོད་པའི་སྐབས་ རང་བཞིན་གྱིས་ གསལ་སྟོན་འབདཝ་ཨིན།
  དེ་འབདཝ་ལས་ བཀོལ་སྤྱོད་སྡེ་ཚན་ཚུ་གིས་ `SpaceDirectoryEvent::ManifestExpired`, གིས་ འཇམ་ཏོང་ཏོ་སྦེ་ ལྟ་རྟོག་འབདཝ་ཨིན།
  `nexus_space_directory_revision_total` ཌེལ་ཊ་དང་ Torii ཚུ་བདེན་དཔྱད་འབད།
  `status = "Expired"`. སི་ཨེལ་ཨའི་ `manifest expire` བརྡ་བཀོད་འདི་ དོན་ལུ་ འཐོབ་ཚུགསཔ་ཨིན།
  ལག་དེབ་འདི་ རྒྱབ་བཤུད་ཡང་ན་ སྒྲུབ་བྱེད་རྒྱབ་བསྐྱོར་བཀང་ཡོདཔ།

## 2. དངུལ་ཁང་ནང་བཀོད་དང་ དཀར་ཆག་ལཱ་གི་རྒྱུན་རིམ།

| དུས་རིམ་ | ཇོ་བདག་(ཚུ་) | བྱ་བ་ | སྒྲུབ་བྱེད་ |
|--|-|-|-|----------------------------------------- |
| ༠. ནང་བསྐྱོད་ | CBDC PMO | ཀེ་ཝའི་སི་ ཡིག་ཆ་ཚུ་ བསྡུ་སྒྲིག་འབད། འཕྲུལ་རིག་ ཌི་ཨེསི་ གསལ་སྟོན་ བདེན་དཔྱད་ཐོ་ཡིག་ ཡུ་ཨའི་ཨའི་ཌི་ སབ་ཁྲ་ཚུ། | འཛུལ་སྤྱོད་ཀྱི་ ཤོག་འཛིན་ DS མཚན་རྟགས་བཀོད་ཡོདཔ། |
| 1. གཞུང་སྐྱོང་གནང་བ། | སྤྱི་ཚོགས་ / བསྟར་སྤྱོད་ | བསྐྱར་ཞིབ་ནང་ ཐུམ་སྒྲིལ་ `cbdc.manifest.json` ལུ་ མིང་རྟགས་བཀོད། `AssetPermissionManifest` ཆ་འཇོག་འབད། | མཚན་རྟགས་བཀོད་པའི་གཞུང་གི་སྐར་མ་དང་ ཧེཤ་ གསལ་སྟོན་འབད། |
| 2. ལྕོགས་གྲུབ་སྤྲོད་ལེན། | CBDC ལམ་གྱི་ཨོཔ་ཚུ་ | ཨིན་ཀོཌི་གིས་ `norito::json::to_string_pretty` བརྒྱུད་དེ་ གསལ་སྟོན་འབདཝ་ཨིན་ གནམ་སྟོང་སྣོད་ཐོ་འོག་ལུ་གསོག་འཇོག་འབད་ བཀོལ་སྤྱོད་པ་ཚུ་ལུ་བརྡ་སྟོན། | Manifest JSON + norito Prometheus ཡིག་སྣོད་, BLAKE3 ཟས་བཅུད་འཇལ། |
| 3. དཀར་པོའི་ཤུགས་རྐྱེན། | CBDC ལམ་གྱི་ཨོཔ་ཚུ་ | DSID ལུ་ `composability_group.whitelist`, bump `activation_epoch`, གསལ་སྟོན་འདི་ བཀྲམ་སྤེལ་འབད། དགོས་མཁོ་ཡོད་པ་ཅིན་ གནད་སྡུད་ས་སྟོང་རྒྱུན་ལམ་དུས་མཐུན་བཟོ་ནི། | གསལ་སྟོན་གྱི་ཁྱད་པར་ `kagami config diff` ཐོན་འབྲས་, གཞུང་སྐྱོང་ཆ་འཇོག་ ID. |
| 4. བཤུད་བརྙན་བདེན་དཔྱད། | QA Guild / Ops | མཉམ་བསྡོམས་བརྟག་དཔྱད་དང་ ཊི་ཨི་ཡུ་ མངོན་གསལ་བརྟག་དཔྱད་ དེ་ལས་ ལས་རིམ་བཟོ་ཚུགས་པའི་ ཏི་རུ་བསྐྱར་རྩེད་ཚུ་ གཡོག་བཀོལ་དགོ།(འོག་ལུ་བལྟ།) | `cargo test` དྲན་ཐོ་ཚུ་, ཊི་ཨི་ཡུ་ ཌེཤ་བོརཌི་ ལས་རིམ་བཟོ་ཚུགས་པའི་-དངུལ་སྒྲིག་གྲུབ་འབྲས། |
| 5. སྒྲུབ་བྱེད་ཡིག་མཛོད་ | བསྟར་སྤྱོད་ WG | བཱན་ཌལ་གྱིས་ གནང་བ་དང་ ལྕོགས་གྲུབ་བཞུ་ནི་ བརྟག་དཔྱད་ཐོན་སྐྱེད་ དེ་ལས་ `artifacts/nexus/cbdc_<stamp>/` གི་འོག་ལུ་ Prometheus གི་ བརྡ་རྟགས་ཚུ་ གསལ་སྟོན་འབདཝ་ཨིན། | སྒྲུབ་བྱེད་ཀྱི་ ཏར་བཱོལ་ ཅེག་སམ་ཡིག་སྣོད་ ཚོགས་སྡེའི་མིང་རྟགས་བཀོད་ནི། |

### རྩིས་ཞིབ་བང་མཛོད་རོགས་སྐྱོར།གནམ་སྟོང་སྣོད་ཐོ་ནང་ལས་ `iroha app space-directory manifest audit-bundle` གྲོགས་རམ་འདི་ལག་ལེན་འཐབ།
སྒྲུབ་བྱེད་ཐུམ་སྒྲིལ་མ་བཙུགས་པའི་ཧེ་མ་ ལྕོགས་གྲུབ་རེ་རེ་གིས་ གསལ་སྟོན་འབད་ནི།
གསལ་སྟོན་འདི་ JSON (ཡང་ན་ `.to` པེ་ལོཌི་) དང་ གནད་སྡུད་ས་སྒོ་གསལ་སྡུད་ཚུ་བྱིན།

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z/cbdc_wholesale \
  --notes "CBDC wholesale refresh"
```

བརྡ་བཀོད་འདི་གིས་ ཀེ་ནོ་ནིཀ་ཇེ་ཨེསི་ཨོ་ཨེན་/Norito/hash འདྲ་བཤུས་ཚུ་ མཉམ་དུ་བཏོནམ་ཨིན།
ཡུ་ཨེ་ཨའི་ཌི་དང་ གནད་སྡུད་ས་སྟོང་ཨི་ཌི་ ཤུགས་ལྡན་བཟོ་ནི/འཚོལཝ་ཨིན།
དགོས་མཁོ་འདི་བཀག་དམ་འབད་བའི་སྐབས་ epochs དང་ གསལ་སྟོན་ཧེ་ཤི་ དེ་ལས་ གསལ་སྡུད་རྩིས་ཞིབ་ཀྱི་ ཧུཀ་ཚུ་ བསྟར་སྤྱོད་འབད་བའི་སྐབས་ བཀོད་སྒྲིག་འབད་བའི་སྐབས་ བཀོད་སྒྲིག་འབདཝ་ཨིན།
`SpaceDirectoryEvent` མཁོ་མངགས་འབད་མི་ཚུ། སྒྲུབ་བྱེད་ཀྱི་ནང་དུ་བསྡམས་བཞག་པ།
རྩིས་ཞིབ་པ་དང་ ཁྲིམས་ལུགས་ཚུ་གིས་ ཤུལ་ལས་ བཱའིཊ་ངོ་མ་ཚུ་ ལོག་རྩེད་ཚུགས།

### 2.1 བརྡ་བཀོད་དང་བདེན་དཔང་།

1. **ལེན་ནེ་ མངོན་གསལ་:** `cargo test -p integration_tests nexus::lane_registry -- --nocapture lane_manifest_registry_loads_fixture_manifests`.
2. **Shechuler quotas:** `cargo test -p integration_tests scheduler_teu -- queue_teu_backlog_matches_metering queue_routes_transactions_across_configured_lanes`.
3. **ལག་ཐོག་ཐ་མག:** `irohad --sora --config configs/soranexus/nexus/config.toml --chain 0000…` གསལ་སྟོན་སྣོད་ཐོ་ CBDC ཡིག་སྣོད་ཚུ་ལུ་སྟོན་ཞིནམ་ལས་ `/v2/sumeragi/status` ལུ་ཨེབ་སྟེ་ CBDC ལམ་གྱི་དོན་ལུ་ `lane_governance.manifest_ready=true` བདེན་དཔྱད་འབད།
4. **དཀརཔོ་གི་རིམ་མཐུན་བརྟག་དཔྱད་:** `cargo test -p integration_tests nexus::cbdc_whitelist -- --nocapture` ལག་ལེན་ཚུ་ `integration_tests/tests/nexus/cbdc_whitelist.rs`, `fixtures/space_directory/profile/cbdc_lane_profile.json` གིས་ `fixtures/space_directory/profile/cbdc_lane_profile.json` དབྱེ་དཔྱད་འབདཝ་ཨིན། `fixtures/space_directory/profile/cbdc_lane_profile.json` གིས་ དཀརཔོ་ཐོ་ཡིག་ཐོ་བཀོད་ཀྱི་ཡུ་ཨེ་ཨའི་ཌི་ གནད་སྡུད་ས་སྟོང་ ཤུགས་ལྡན་བཟོ་བའི་དུས་སྐབས་ཚུ་དང་མཐུན་སྒྲིག་འབད་ནི་ལུ་ ངེས་གཏན་བཟོ་ནི་དང་ ཆོག་ཐམ་གྱི་ཐོ་ཡིག་འདི་ འོག་ལུ་ Norito གསལ་སྟོན་འབདཝ་ཨིན། `fixtures/space_directory/capability/`. བརྟག་དཔྱད་དྲན་ཐོ་འདི་ NX-6 སྒྲུབ་བྱེད་ཀྱི་བང་རིམ་ལུ་མཉམ་སྦྲགས་འབད།

### ༢.༢ ཀླི་ཨའི་ སྦུང་ཚད།

- UAID + `cargo run -p iroha_cli -- manifest cbdc --uaid <hash> --dataspace 11 --template cbdc_wholesale` བརྒྱུད་དེ་ ཀེང་རུས་གསལ་སྟོན་འབད།
- Torii (Space Directory) ལུ་ `iroha app space-directory manifest publish --manifest cbdc_wholesale.manifest.to` (ཡང་ན་ `--manifest-json cbdc_wholesale.manifest.json`) ལག་ལེན་འཐབ་སྟེ་ གསལ་སྟོན་འབད། ཕུལ་མི་རྩིས་ཐོ་འདི་གིས་ སི་བི་ཌི་སི་གནས་སྡུད་གནམ་སྟོང་གི་དོན་ལུ་ `CanPublishSpaceDirectoryManifest` འཛིན་དགོ།
- ཨོཔ་སི་ ཌེཀསི་གིས་ ཐག་རིང་རང་བཞིན་རང་བཞིན་གཡོག་བཀོལ་དོ་ཡོད་པ་ཅིན་ ཨེཆ་ཊི་ཊི་པི་བརྒྱུད་དེ་ དཔར་བསྐྲུན་འབད་ནི།

  ```bash
  curl -X POST https://torii.soranexus/v2/space-directory/manifests \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "i105...",
            "private_key": "ed25519:CiC7…",
            "manifest": '"'"'$(cat fixtures/space_directory/capability/cbdc_wholesale.manifest.json)'"'"',
            "reason": "CBDC onboarding wave 4"
          }'
  ```

  Torii གིས་ དཔར་བསྐྲུན་ཚོང་འབྲེལ་འདི་ གྱལ་རིམ་འབད་ཚར་བའི་ཤུལ་ལས་ `202 Accepted` སླར་ལོག་འབདཝ་ཨིན། དེ་བཟུམ་སྦེ་
  སི་ཨའི་ཌི་ཨར་/ཨེ་པི་ཨའི་-ཊོ་ཀེན་གཱེཊིསི་གིས་ འཇུག་སྤྱོད་འབད་ཞིནམ་ལས་ རྒྱུན་རིམ་གནང་བའི་གནང་བ་དགོས་མཁོ་འདི་ མཐུན་སྒྲིག་འབདཝ་ཨིན།
  CLI ལཱ་གི་རྒྱུན་རིམ།
- ཛ་དྲག་ཆ་མེད་བཟོ་ནི་འདི་ POSTing ལུ་ POSTing ལུ་ ཐག་རིང་ལས་ བཏོན་ཚུགས།

  ```bash
  curl -X POST https://torii.soranexus/v2/space-directory/manifests/revoke \
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

  Torii གིས་ བཏོན་གཏང་ནིའི་ཚོང་འབྲེལ་འདི་ གྱལ་རིམ་འབད་ཚར་བའི་ཤུལ་ལས་ `202 Accepted` སླར་ལོག་འབདཝ་ཨིན། དེ་བཟུམ་སྦེ་
  CIDR/API-ཊོ་ཀེན་གཱེཊིསི་འདི་ གློག་རིམ་མཐའ་མ་གཞན་ཚུ་སྦེ་འཇུག་སྤྱོད་འབད་ཡོདཔ་དང་ `CanPublishSpaceDirectoryManifest`
  འདི་ ད་ལྟོ་ཡང་ རིམ་ཐེངས་དགོཔ་ཨིན།
- ཐོ་ཡིག་དཀརཔོ་གི་འཐུས་མི་: ཞུན་དག་ `cbdc.manifest.json`, bump `activation_epoch`, དང་ བདེན་དཔྱད་འབད་མི་ཆ་མཉམ་ལུ་ ཉེན་སྲུང་ཅན་གྱི་འདྲ་བཤུས་བརྒྱུད་དེ་ ལོག་ལེན། རིམ་སྒྲིག་འབད་ཡོད་པའི་འོས་བསྡུའི་བར་མཚམས་གུ་ Nexus ཧོཊ་-བསྐྱར་ལོག།

## 3. མཐུན་སྒྲིག་སྒྲུབ་པའི་བདེན་པ།

`artifacts/nexus/cbdc_rollouts/<YYYYMMDDThhmmZ>/` གི་འོག་ལུ་ ཅ་རྙིང་ཚུ་ གསོག་འཇོག་འབད་ཞིནམ་ལས་ ཟས་བཅུད་འདི་ གཞུང་སྐྱོང་གི་ཤོག་འཛིན་ལུ་ མཐུད་དགོ།

| ཡིག་སྣོད་ | འགྲེལ་བཤད་ |
|-------|-------------------------------------------------------------------------------------------------
| Prometheus ཐོ་ཡིག་དཀརཔོ་གི་ཌིཕ་དང་གཅིག་ཁར་ མཚན་རྟགས་བཀོད་ཡོད་པའི་ལམ་འདི་ གསལ་སྟོན་ (ཧེ་མ་/ཤུལ་ལས་)། |
| `capability/<uaid>.manifest.json` & `.to` | Norito + JSON ལྕོགས་གྲུབ་འདི་གིས་ ཡུ་ཨའི་ཨའི་ཌི་རེ་རེ་གི་དོན་ལུ་ གསལ་སྟོན་འབདཝ་ཨིན། |
| `compliance/kyc_<bank>.pdf` | བཀག་འཛིན་འབད་མི་-གདོང་ལན་འབད་མི་ KYC བདེན་དཔང་། |
| `metrics/nexus_scheduler_lane_teu_capacity.prom` | Prometheus ཊི་ཨི་ཡུ་གི་མགུ་ཏོ་ཁང་མིག་བདེན་དཔང་འབད་ཡོདཔ། |
| `tests/cargo_test_nexus_lane_registry.log` | གསལ་སྟོན་བརྟག་དཔྱད་གཡོག་བཀོལ་ནི་ལས་ ནང་བསྐྱོད་འབད། |
| `tests/cargo_test_scheduler_teu.log` | དྲན་ཐོ་ TEU འགྲུལ་ལམ་ཚུ་ བདེན་ཁུངས་བཀལ་ནི། |
| `programmable_money/axt_replay.json` | ལས་རིམ་བཟོ་ཚུགས་པའི་ ཏི་རུ་གི་ཕན་ཚུན་འབྲེལ་བ་ཡོདཔ་སྦེ་ བསྐྱར་དུ་གཏང་ནི། (དོན་ཚན་༤ པ་བལྟ།) |
| `approvals/governance_minutes.md` | མཚན་རྟགས་ཆ་འཇོག་སྐར་མ་གཞི་བསྟུན་འབད་མི་འདི་གིས་ གསལ་སྟོན་ཧེཤ་ + ཤུགས་ལྡན་བཟོ་བའི་དུས་སྐབས་ལུ། |**བདེན་དཔྱད་ཡིག་གཟུགས་:** run `ci/check_cbdc_rollout.sh` གིས་ གཞུང་སྐྱོང་ཤོག་བྱང་ལུ་མ་བཙུགས་པའི་ཧེ་མ་ སྒྲུབ་བྱེད་བསྡུ་སྒྲིག་འདི་ མཇུག་བསྡུ་ཡི་ཟེར་ བདེན་ཁུངས་བཀལ་ནིའི་དོན་ལུ་ཨིན། གྲོགས་རམ་པ་འདི་གིས་ `artifacts/nexus/cbdc_rollouts/` (ཡང་ན་ `CBDC_ROLLOUT_BUNDLE=<path>`) ལུ་ `cbdc.manifest.json` རེ་རེ་ལུ་ གསལ་སྟོན་/བརྟན་ཚུགས་པའི་སྡེ་ཚན་ལུ་དབྱེ་དཔྱད་འབདཝ་ཨིན། metrics scrapes དང་ `cargo test` དྲན་ཐོ་ཚུ་གིས་ ལས་རིམ་བཟོ་ཚུགས་པའི་ ཏི་རུ་བསྐྱར་རྩེད་འདི་ བདེན་དཔྱད་འབདཝ་ཨིནམ་དང་ ཆ་འཇོག་སྐར་མ་ཚུ་ ཤུགས་ལྡན་བཟོ་བའི་དུས་སྐབས་དང་ གསལ་སྟོན་གྱི་ཧེཤ་ཚུ་ ངེས་གཏན་བཟོཝ་ཨིན། གསར་ཤོས་ rev གིས་ NX-6 ནང་ལུ་ཡོད་པའི་ ཉེན་སྲུང་གི་རེ་ལི་ཚུ་ཡང་ བསྟར་སྤྱོད་འབདཝ་ཨིན། quorum འདི་ གསལ་བསྒྲགས་འབད་ཡོད་པའི་ བདེན་དཔྱད་ཆ་ཚན་ལས་ བརྒལ་མི་ཚུགས། ཉེན་སྲུང་འབད་ཡོད་པའི་མིང་གི་ས་སྒོ་ཚུ་ སྟོངམ་མེན་པའི་ ཡིག་རྒྱུན་ཚུ་ དགོཔ་ཨིན་ དེ་ལས་ ལྕོགས་གྲུབ་ཀྱི་མངོན་རྟགས་ཚུ་གིས་ གཅིག་མཚུངས་སྦེ་ ཡར་སེང་འབད་ཡོད་པའི་ `activation_epoch`/`expiry_epoch` ཆ་ཚུ་ ལེགས་ཤོམ་སྦེ་གྲུབ་ཡོདཔ་ཨིན། `Allow`/`Deny` ནུས་པ། `fixtures/nexus/cbdc_rollouts/` གི་འོག་ལུ་ དཔེ་ཚད་སྒྲུབ་བྱེད་ཀྱི་ཐུམ་སྒྲིལ་འདི་ མཉམ་བསྡོམས་བརྟག་དཔྱད་ `integration_tests/tests/nexus/cbdc_rollout_bundle.rs` གིས་ བདེན་དཔྱད་འབད་མི་འདི་ CI ནང་ལུ་བཞག་སྟེ་ ལག་ལེན་འཐབ་ཨིན།

## 4. ལས་རིམ-དངུལ་འབོར་གྱི་ཕན་ཚུན་འབྲེལ་བ།

དངུལ་ཁང་ཅིག་ (གནས་སྡུད་ས་སྟོང་༡༡) དང་ སིལ་སིབ་ཌི་ཨེ་པི་ (གནད་སྡུད་ས་སྟོང་༡༢) གཉིས་ཆ་ར་ `ComposabilityGroupId` ནང་ དཀརཔོ་སྦེ་ ཐོ་བཀོད་འབད་བའི་སྐབས་ ལས་རིམ་བཟོ་ཚུགས་པའི་ ཏི་རུ་ཚུ་ `docs/source/nexus.md` §8.6 ལས་ AXT གི་དཔེ་རིས་ལུ་ གཞི་བཞག་སྟེ་ ཐོ་བཀོད་འབདཝ་ཨིན།

༡ ཚོང་འབྲེལ་ཌི་ཨེ་པི་གིས་ དེ་གི་ཡུ་ཨེ་ཨའི་ཌི་ + ཨེགསི་ཊི་ བཞུ་བཅོས་ལུ་ མཐུད་ཡོད་པའི་ རྒྱུ་དངོས་ཀྱི་ ལག་ཆས་ཅིག་ ཞུ་བ་འབདཝ་ཨིན། CBDC lane གིས་ `AssetPermissionManifest::evaluate` བརྒྱུད་དེ་ ལག་ཆ་འདི་ བདེན་དཔྱད་འབདཝ་ཨིན།
2. DS གཉིས་ཆ་ར་གིས་ མཉམ་སྦྱོར་གྱི་སྡེ་ཚན་ཅོག་འཐདཔ་སྦེ་གསལ་བསྒྲགས་འབདཝ་ལས་ རྡུལ་ཕྲན་ཚུ་གིས་ CBDC ལམ་ནང་ལུ་ རྡུལ་ཕྲན་ཚུ་ བཙུགས་ནིའི་དོན་ལུ་ བརྡལ་བཤིག་གཏངམ་ཨིན།
༣༽ ལག་ལེན་འཐབ་པའི་སྐབས་ CBDC DS གིས་ AML/KYC གི་བདེན་ཁུངས་ཚུ་ དེ་གི་གློག་ལམ་ནང་ བསྟར་སྤྱོད་འབདཝ་ཨིན།
༤ བདེན་ཁུངས་རྒྱུ་ཆ་ (FASTPQ + DA ཁས་བླངས་) ཚུ་ CBDC ལམ་ཁར་རྐྱངམ་ཅིག་སྡོད་ཡོདཔ་ཨིན། མཉམ་བསྡོམས་འབད་ཡོད་པའི་ཐོ་བཀོད་ཚུ་གིས་ སྒེར་གྱི་གནས་སྡུད་མ་བཏོན་པར་ འཛམ་གླིང་མངའ་སྡེ་གི་གཏན་འབེབས་འདི་ བཞགཔ་ཨིན།

ལས་རིམ་བཟོ་བཏུབ་པའི་དངུལ་བསྐྱར་རྩེད་ཀྱི་གཏན་མཛོད་ནང་ཚུད་དགོཔ།

- AXT འགྲེལ་བཤད་ + འཛིན་སྐྱོང་ཞུ་བ་/ལན་གསལ་ཚུ།
- Norito-incoded ཚོང་འབྲེལ་ཡིག་ཤུགས།
- གྲུབ་འབྲས་ཐོབ་ཐངས་ (ལམ་ལུ་དགའ་སྤྲོ་, ལམ་བཀག་ཆ་)།
- `telemetry::fastpq.execution_mode` དང་ `nexus_scheduler_lane_teu_slot_committed`, དང་ `lane_commitments` གི་དོན་ལུ་ ཊེ་ལི་མི་ཊི་གི་ཆ་ཤས་ཚུ་.

## 5. བལྟ་རྟོག་དང་རུན་དེབ་ཚུ།

- **མེ་ཊིགསི་:** བལྟ་རྟོག་པ་ `nexus_scheduler_lane_teu_capacity`, `nexus_scheduler_lane_teu_slot_committed`, `nexus_scheduler_lane_teu_deferral_total{reason}`, `governance_manifest_admission_total`, དང་ `governance_manifest_admission_total`, དང་ `lane_governance_sealed_total` (Torii, (Torii, (`docs/source/telemetry.md`)
- **གནད་སྡུད་བཀོད་སྒྲིག་ཚུ་:** སི་བྷི་ཌི་སི་ལེན་གྲལ་ཐིག་དང་གཅིག་ཁར་ `docs/source/grafana_scheduler_teu.json` རྒྱ་སྐྱེད་འབད། ཐོ་ཡིག་དཀརཔོ་གི་དོན་ལུ་ པེ་ནཱལ་ཚུ་ཁ་སྐོང་བརྐྱབ། (མཆན་འགྲེལ་རེ་རེ་བཞིན་) དང་ ལྕོགས་གྲུབ་ཀྱི་དུས་ཚོད་རྫོགས་པའི་མདོང་ལམ་ཚུ་ཨིན།
- **Alerts:** སྐར་མ་༡༥ གི་དོན་ལུ་ `rate(nexus_scheduler_lane_teu_deferral_total{reason="cap_exceeded"}[5m]) > 0` དང་ ཡང་ན་ `lane_governance.manifest_ready=false` འདི་ འོས་བསྡུའི་བར་མཚམས་གཅིག་ལས་ལྷག་སྟེ་ གནས་ཏེ་ཡོདཔ་ཨིན།
- **Runbook pointers:** `docs/source/governance_api.md` ནང་ མིང་ས་སྒོ་ལམ་སྟོན་དང་ ལས་རིམ་བཟོ་ཚུགས་པའི་-དངུལ་གྱི་དཀའ་ངལ་སེལ་ཐབས་འབད་ནི་ལུ་ `docs/source/nexus.md` ནང་ འབྲེལ་མཐུད།

## 6. ངོས་ལེན་དཔྱད་གཞི།- [ ] CBDC ལམ་ཐིག་འདི་ `nexus.lane_catalog` ནང་ TEU མེ་ཊ་ཌེ་ཊ་མཐུན་སྒྲིག་འབད་དེ་ ཊི་ཨི་ཡུ་བརྟག་དཔྱད་ཚུ་དང་བཅས་གསལ་བསྒྲགས་འབད་ཡོདཔ་ཨིན།
- [ ] མཚན་རྟགས་བཀོད་ཡོད་པའི་ `cbdc.manifest.json` གསལ་སྟོན་སྣོད་ཐོ་ནང་ཡོད་པའི་ `cargo test -p integration_tests nexus::lane_registry` བརྒྱུད་དེ་ བདེན་དཔྱད་འབད་ཡོདཔ་ཨིན།
- [ ] ནུས་སྟོབས་གསལ་སྟོན་ཚུ་ ཡུ་ཨའི་ཌི་རེ་རེ་གི་དོན་ལུ་ བཏོན་ཡོདཔ་དང་ གནམ་སྟོང་སྣོད་ཐོ་ནང་ གསོག་འཇོག་འབད་ཡོདཔ་ཨིན། ཁས་ལེན་འབད་ནི་/ངེས་པར་དུ་ ཡུ་ནིཊ་བརྟག་དཔྱད་ (`crates/iroha_data_model/src/nexus/manifest.rs`) བརྒྱུད་དེ་ བདེན་དཔྱད་འབད་བཅུག།
- [ ] གཞུང་སྐྱོང་ཆ་འཇོག་ཨའི་ཌི་, `activation_epoch`, དང་ Prometheus སྒྲུབ་བྱེད་ཚུ་དང་གཅིག་ཁར་ཐོ་བཀོད་འབད་མི་ ཐོ་ཡིག་དཀརཔོ་གི་ཤུགས་ལྡན་བཟོ་ཡོདཔ།
- [ ] ལས་རིམ་བཟོ་བའི་དངུལ་བསྐྱར་རྩེད་ཀྱི་གཏན་མཛོད་དང་ ལག་ཆ་སྤྲོད་ཐངས་དང་ ངོས་ལེན་མེད་པའི་ དེ་ལས་ རྒྱུན་འགྲུལ་ཚུ་ འབད་བཅུགཔ་ཨིན།
- [ ] ཀིརིཔ་ཊོ་གཱ་ར་ཕིག་ བཞུ་བཅོས་དང་གཅིག་ཁར་ སྐྱེལ་བཙུགས་འབད་མི་ སྒྲུབ་བྱེད་ཚུ་ གཞུང་སྐྱོང་གི་ ཤོག་འཛིན་ལས་ མཐུད་དེ་ཡོདཔ་ད་ དེ་ལས་ NX-6 མཐོ་རིམ་ཤེས་ཚད་མཐར་འཁྱོལ་མི་ཚུ་ 🈯 ལས་ 🈺 ཚུན་ཚོད་ཨིན།

འདི་གི་ཤུལ་ལས་ རྩེད་དེབ་འདི་ NX-6 གི་དོན་ལུ་ ཡིག་ཆ་ཚུ་ བསྒྲུབ་ཚུགསཔ་ཨིནམ་དང་ མ་འོངས་པའི་ ལམ་གྱི་ས་ཁྲ་རྣམ་གྲངས་ (NX-12/NX-15) གིས་ CBDC ལམ་གྱི་རིམ་སྒྲིག་གི་དོན་ལུ་ ཊེམ་པེལེཊི་ཅིག་བྱིན་ཏེ་ ཐོ་ཡིག་དཀརཔོ་བཙུགས་ནི་དང་ ལས་རིམ་བཟོ་ཚུགས་པའི་ ཏི་རུ་གི་ཕན་ཚུན་ལག་ལེན་འཐབ་ཚུགསཔ་ཨིན།