---
lang: hy
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

> **Ճանապարհային քարտեզի կապը.** NX-6 (CBDC մասնավոր գծի ձևանմուշ և սպիտակ ցուցակի հոսք) և NX-14 (Nexus աշխատատեղեր):  
> **Սեփականատերեր.** Financial Services WG, Nexus Core WG, Compliance WG:  
> **Կարգավիճակ.** Նախագծում. իրականացման կեռիկներ գոյություն ունեն `crates/iroha_data_model::nexus`, `crates/iroha_core::governance::manifest` և `integration_tests/tests/nexus/lane_registry.rs`-ում, սակայն բացակայում էին CBDC-ին հատուկ մանիֆեստները, սպիտակ ցուցակները և օպերատորների աշխատաթերթերը: Այս գրքույկը փաստում է հղման կազմաձևումը և ներբեռնման աշխատանքային հոսքը, որպեսզի CBDC-ի տեղակայումները կարողանան դետերմինիստական ​​կերպով շարունակվել:

## Շրջանակ և դերեր

- **Կենտրոնական բանկի երթուղի («CBDC lane»).** Թույլատրված վավերացուցիչներ, պահառու հաշվարկային բուֆերներ և ծրագրավորվող փողի քաղաքականություն: Գործում է որպես սահմանափակ տվյալների տարածություն + գոտի զույգ՝ իր կառավարման մանիֆեստով:
- **Մեծածախ/մանրածախ բանկային տվյալների տարածքներ.** Մասնակիցների DS, որոնք ունեն UAID-ներ, ստանում են հնարավորությունների մանիֆեստներ և կարող են ներառվել ատոմային AXT-ի սպիտակ ցուցակում CBDC գծով:
- **Ծրագրավորվող փողի dApps.** Արտաքին DS, որոնք սպառում են CBDC, հոսում են `ComposabilityGroup` երթուղիչով, երբ հայտնվեն սպիտակ ցուցակում:
- **Կառավարում և համապատասխանություն. ** Խորհրդարանը (կամ համարժեք մոդուլը) հաստատում է գոտիների մանիֆեստները, կարողությունների դրսևորումները և սպիտակ ցուցակի փոփոխությունները. համապատասխանությունը պահպանում է ապացույցների փաթեթները Norito մանիֆեստների կողքին:

**Կախվածություններ**

1. Lane catalog + dataspace catalog wiring (`docs/source/nexus_lanes.md`, `defaults/nexus/config.toml`):
2. Գոտի մանիֆեստի կիրառում (`crates/iroha_core/src/governance/manifest.rs`, հերթերի անցում `crates/iroha_core/src/queue.rs`-ում):
3. Կարողությունների դրսևորումներ + UAIDs (`crates/iroha_data_model/src/nexus/manifest.rs`):
4. Scheduler TEU քվոտա + չափումներ (`integration_tests/tests/scheduler_teu.rs`, `docs/source/telemetry.md`):

## 1. Հղման գծի դասավորություն

### 1.1 Գոտի կատալոգ և տվյալների տարածքի գրառումներ

Ավելացնել հատուկ գրառումներ `[[nexus.lane_catalog]]` և `[[nexus.dataspace_catalog]]`: Ստորև բերված օրինակը ընդլայնում է `defaults/nexus/config.toml`-ը CBDC գծով, որը պահում է 1500 TEU յուրաքանչյուր բնիկ և նվազեցնում է քաղցը մինչև վեց սլոտներ, գումարած մեծածախ բանկերի և մանրածախ դրամապանակների համար տվյալների տարածության համընկնող կեղծանունները:

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

**Նշումներ**

- `metadata.scheduler.teu_capacity` և `metadata.scheduler.starvation_bound_slots` սնուցում են `integration_tests/tests/scheduler_teu.rs`-ի կողմից իրականացվող TEU չափիչները: Օպերատորները պետք է դրանք համաժամեցվեն ընդունման արդյունքների հետ, որպեսզի `nexus_scheduler_lane_teu_capacity`-ը համապատասխանի ձևանմուշին:
- Վերևում գտնվող տվյալների տարածության ցանկացած այլանուն պետք է հայտնվի կառավարման ցուցումներում և կարողությունների դրսևորումներում (տես ստորև), այնպես որ ընդունումը ինքնաբերաբար մերժում է դրեյֆը:

### 1.2 Lane manifest կմախք

Գոտի դրսևորվում է `nexus.registry.manifest_directory`-ի միջոցով կազմաձևված գրացուցակի տակ (տես `crates/iroha_config/src/parameters/actual.rs`): Ֆայլերի անունները պետք է համընկնեն գծերի անունների հետ (`cbdc.manifest.json`): Սխեման արտացոլում է կառավարման մանիֆեստի թեստերը `integration_tests/tests/nexus/lane_registry.rs`-ում:

```json
{
  "lane": "cbdc",
  "dataspace": "cbdc.core",
  "version": 1,
  "governance": "central_bank_multisig",
  "validators": [
    { "validator": "<i105-account-id>", "peer_id": "<peer-id>" },
    { "validator": "<i105-account-id>", "peer_id": "<peer-id>" },
    { "validator": "<i105-account-id>", "peer_id": "<peer-id>" },
    { "validator": "<i105-account-id>", "peer_id": "<peer-id>" }
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

Հիմնական պահանջներ.-

- Validators **must** be declared as explicit bindings with a canonical I105
  authority account plus a concrete `peer_id`. Legacy string-only validator
  arrays are rejected.
- Each manifest `peer_id` must resolve to a registered runtime peer with a live
  consensus key that is present in the current commit topology; Torii routes
  only to those authoritative peer bindings and fails closed when the runtime
  truth disagrees with the manifest.
- Validator accounts should remain stable governance identities even if the
  underlying host or peer keys rotate; update the manifest `peer_id` binding
  when the serving peer changes. Set `quorum` to the multisig threshold (≥2).
- Պաշտպանված անունների տարածքները պարտադրվում են `Queue::push`-ով (տես `crates/iroha_core/src/queue.rs`), ուստի CBDC-ի բոլոր պայմանագրերը պետք է նշեն `gov_namespace` + `gov_contract_id`:
- `composability_group` դաշտերը հետևում են `docs/source/nexus.md` §8.6-ում նկարագրված սխեմային; սեփականատերը (CBDC lane) տրամադրում է սպիտակ ցուցակը և քվոտաները: Սպիտակ ցուցակում նշված DS մանիֆեստները նշում են միայն `group_id_hex` + `activation_epoch`:
- Մանիֆեստը պատճենելուց հետո գործարկեք `cargo test -p integration_tests nexus::lane_registry -- --nocapture`՝ հաստատելու համար, որ `LaneManifestRegistry::from_config`-ը բեռնում է այն:

### 1.3 Կարողությունների դրսևորումներ (UAID քաղաքականություն)

Կարողությունների դրսևորումները (`AssetPermissionManifest` `crates/iroha_data_model/src/nexus/manifest.rs`-ում) կապում են `UniversalAccountId`-ին դետերմինիստական նպաստների հետ: Հրապարակեք դրանք Space Directory-ի միջոցով, որպեսզի բանկերը և dApps-ը կարողանան բեռնել ստորագրված քաղաքականությունները:

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

- Մերժման կանոնները հաղթում են նույնիսկ այն դեպքում, երբ թույլատրելի կանոնը համընկնում է (`ManifestVerdict::Denied`), այնպես որ բոլոր բացահայտ մերժումները տեղադրեք համապատասխան թույլտվություններից հետո:
- Օգտագործեք `AllowanceWindow::PerSlot` ատոմային վճարման բռնակների համար և `PerMinute`/`PerDay`՝ հաճախորդների սահմանաչափերը գլորելու համար:
- Յուրաքանչյուր UAID/տվյալների տարածքի մեկ մանիֆեստը բավարար է. ակտիվացումները և ժամկետների ավարտը պարտադրում են քաղաքականության ռոտացիայի արագությունը:
- Գոտի գործարկման ժամանակը այժմ լրանում է, ինքնաբերաբար դրսևորվում է `expiry_epoch`-ին հասնելուց հետո,
  այնպես որ գործառնական թիմերը պարզապես վերահսկում են `SpaceDirectoryEvent::ManifestExpired`,
  արխիվացրեք `nexus_space_directory_revision_total` դելտան և ստուգեք Torii ցուցադրությունները
  `status = "Expired"`. CLI `manifest expire` հրամանը մնում է հասանելի
  ձեռքով անտեսումներ կամ ապացույցների լրացումներ:

## 2. Բանկի մուտքագրում և սպիտակ ցուցակի աշխատանքային հոսք

| Փուլ | Սեփականատեր(ներ) | Գործողություններ | Ապացույցներ |
|-------|----------|---------|----------|
| 0. Ընդունում | CBDC PMO | Հավաքեք KYC դոսյե, տեխնիկական DS մանիֆեստ, վավերացնող ցուցակ, UAID քարտեզագրում: | Ընդունման տոմս, ստորագրված DS մանիֆեստի նախագիծ: |
| 1. Կառավարման հաստատում | Խորհրդարան / Համապատասխանություն | Վերանայեք ընդունման փաթեթը, նշեք `cbdc.manifest.json`, հաստատեք `AssetPermissionManifest`: | Ստորագրված կառավարման արձանագրություն, մանիֆեստի կատարման հեշ: |
| 2. Կարողությունների թողարկում | CBDC lane ops | Կոդավորեք դրսևորումները `norito::json::to_string_pretty`-ի միջոցով, պահեք Space Directory-ում, տեղեկացրեք օպերատորներին: | Մանիֆեստ JSON + norito `.to` ֆայլ, BLAKE3 digest: |
| 3. Սպիտակ ցուցակի ակտիվացում | CBDC lane ops | Ավելացրեք DSID-ը `composability_group.whitelist`-ին, զարկեք `activation_epoch`, տարածեք մանիֆեստը; անհրաժեշտության դեպքում թարմացրեք տվյալների տարածության երթուղին: | Մանիֆեստի տարբերություն, `kagami config diff` ելք, կառավարման հաստատման ID: |
| 4. Գործարկման վավերացում | QA գիլդիա / Ops | Գործարկեք ինտեգրման թեստեր, TEU-ի բեռնվածության թեստեր և ծրագրավորվող գումարի վերարտադրում (տես ստորև): | `cargo test` տեղեկամատյաններ, TEU վահանակներ, ծրագրավորվող գումարով հարմարանքների արդյունքներ: |
| 5. Ապացույցների արխիվ | Համապատասխանության WG | Փաթեթի մանիֆեստներ, հաստատումներ, հնարավորությունների մարսումներ, փորձարկման արդյունքներ և Prometheus քերծվածքներ `artifacts/nexus/cbdc_<stamp>/`-ի ներքո: | Ապացույցների թարբոլ, ստուգիչ գումարի ֆայլ, խորհրդի ստորագրություն: |

### Աուդիտ փաթեթի օգնականՕգտագործեք `iroha app space-directory manifest audit-bundle` օգնականը Space Directory-ից
խաղագիրք, որպեսզի նկարահանվի յուրաքանչյուր կարողության դրսևորում նախքան ապացույցների փաթեթը ներկայացնելը:
Տրամադրեք մանիֆեստը JSON (կամ `.to` բեռնվածություն) և տվյալների տարածության պրոֆիլը՝

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z/cbdc_wholesale \
  --notes "CBDC wholesale refresh"
```

Հրամանը թողարկում է կանոնական JSON/Norito/հեշ պատճեններ կողքին
`audit_bundle.json`, որը գրանցում է UAID-ը, տվյալների տարածության ID-ն, ակտիվացումը/ժամկետը
դարաշրջաններ, մանիֆեստ հեշ և պրոֆիլի աուդիտի կեռիկներ՝ պահանջվողը կիրառելիս
`SpaceDirectoryEvent` բաժանորդագրություններ: Գցեք փաթեթը ապացույցների ներսում
գրացուցակը, որպեսզի աուդիտորներն ու կարգավորողները կարողանան ավելի ուշ վերարտադրել ճշգրիտ բայթերը:

### 2.1 Հրամաններ և վավերացումներ

1. **Գոտի մանիֆեստներ՝** `cargo test -p integration_tests nexus::lane_registry -- --nocapture lane_manifest_registry_loads_fixture_manifests`.
2. **Ժամանակացույցի քվոտաները՝** `cargo test -p integration_tests scheduler_teu -- queue_teu_backlog_matches_metering queue_routes_transactions_across_configured_lanes`.
3. **Ձեռքով ծուխ.** `irohad --sora --config configs/soranexus/nexus/config.toml --chain 0000…` մանիֆեստի գրացուցակով, որը ցույց է տալիս CBDC ֆայլերը, այնուհետև սեղմեք `/v1/sumeragi/status` և ստուգեք `lane_governance.manifest_ready=true`-ը CBDC գծի համար:
4. **Սպիտակ ցուցակի հետևողականության թեստ.** `cargo test -p integration_tests nexus::cbdc_whitelist -- --nocapture` վարժություններ `integration_tests/tests/nexus/cbdc_whitelist.rs`, վերլուծելով `fixtures/space_directory/profile/cbdc_lane_profile.json`-ը և մատնանշված կարողությունը դրսևորվում է՝ ապահովելու համար սպիտակ ցուցակի յուրաքանչյուր մուտքի UAID-ը, տվյալների տարածությունը, ակտիվացման դարաշրջանը և թույլտվությունների ցանկը I180X0001-ի ներքո: `fixtures/space_directory/capability/`. Կցեք թեստի գրանցամատյանը NX-6 ապացույցների փաթեթին, երբ սպիտակ ցուցակը կամ մանիֆեստները փոխվում են:

### 2.2 CLI հատվածներ

- Ստեղծեք UAID + մանիֆեստի կմախք `cargo run -p iroha_cli -- manifest cbdc --uaid <hash> --dataspace 11 --template cbdc_wholesale`-ի միջոցով:
- Հրապարակել կարողությունների մանիֆեստը Torii-ում (Տիեզերական գրացուցակ)՝ օգտագործելով `iroha app space-directory manifest publish --manifest cbdc_wholesale.manifest.to` (կամ `--manifest-json cbdc_wholesale.manifest.json`); ներկայացվող հաշիվը պետք է ունենա `CanPublishSpaceDirectoryManifest` CBDC տվյալների տարածության համար:
- Հրապարակեք HTTP-ի միջոցով, եթե օպերացիոն սեղանն աշխատում է հեռակառավարման ավտոմատացման միջոցով.

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "<i105-account-id>",
            "private_key": "ed25519:CiC7…",
            "manifest": '"'"'$(cat fixtures/space_directory/capability/cbdc_wholesale.manifest.json)'"'"',
            "reason": "CBDC onboarding wave 4"
          }'
  ```

  Torii-ը վերադարձնում է `202 Accepted`, երբ հրապարակման գործարքը հերթագրվի; նույնը
  Կիրառվում են CIDR/API-token gates, և շղթայական թույլտվության պահանջը համապատասխանում է
  CLI աշխատանքային հոսք:
- Արտակարգ իրավիճակի չեղարկումը կարող է տրվել հեռակա կարգով՝ POST ուղարկելով Torii հասցեին՝

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests/revoke \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "<i105-account-id>",
            "private_key": "ed25519:CiC7…",
            "uaid": "uaid:0f4d…ab11",
            "dataspace": 11,
            "revoked_epoch": 9216,
            "reason": "Fraud investigation #NX-16-R05"
          }'
  ```

  Torii-ը վերադարձնում է `202 Accepted`, երբ չեղյալ գործարքը հերթագրվի. նույնը
  CIDR/API-token-ի դարպասները կիրառվում են որպես այլ հավելվածների վերջնակետեր, և `CanPublishSpaceDirectoryManifest`
  դեռ պահանջվում է շղթայի վրա:
- Պտտեցնել սպիտակ ցուցակի անդամակցությունը. խմբագրել `cbdc.manifest.json`, զարկել `activation_epoch` և վերաբաշխել անվտանգ պատճենի միջոցով բոլոր վավերացնողներին; `LaneManifestRegistry`-ը թեժ վերաբեռնում է կազմաձևված հարցման միջակայքում:

## 3. Համապատասխանության ապացույցների փաթեթ

Պահպանեք արտեֆակտները `artifacts/nexus/cbdc_rollouts/<YYYYMMDDThhmmZ>/` տակ և կցեք ամփոփագիրը կառավարման տոմսին:

| Ֆայլ | Նկարագրություն |
|------|-------------|
| `cbdc.manifest.json` | Ստորագրված երթուղու մանիֆեստ սպիտակ ցուցակի տարբերությամբ (առաջ/հետո): |
| `capability/<uaid>.manifest.json` & `.to` | Norito + JSON կարողությունը դրսևորվում է յուրաքանչյուր UAID-ի համար: |
| `compliance/kyc_<bank>.pdf` | Կարգավորողին ուղղված KYC ատեստավորում: |
| `metrics/nexus_scheduler_lane_teu_capacity.prom` | Prometheus քերծվածք, որն ապացուցում է TEU գլխի սենյակը: |
| `tests/cargo_test_nexus_lane_registry.log` | Մուտքագրեք մանիֆեստի փորձնական գործարկումից: |
| `tests/cargo_test_scheduler_teu.log` | Մատյան, որն ապացուցում է TEU երթուղային անցումները: |
| `programmable_money/axt_replay.json` | Կրկնել տեքստը, որը ցույց է տալիս ծրագրավորվող փողի փոխգործունակությունը (տես բաժին 4): |
| `approvals/governance_minutes.md` | Ստորագրված հաստատման րոպեները հղում են անում մանիֆեստի հեշ + ակտիվացման դարաշրջանին: |**Վավերացման սկրիպտ.** գործարկեք `ci/check_cbdc_rollout.sh`՝ հաստատելու համար, որ ապացույցների փաթեթն ամբողջական է, նախքան այն կցեք կառավարման տոմսին: Օգնականը սկանավորում է `artifacts/nexus/cbdc_rollouts/` (կամ `CBDC_ROLLOUT_BUNDLE=<path>`) յուրաքանչյուր `cbdc.manifest.json`-ի համար, վերլուծում է մանիֆեստի/կազմելու խումբը, ստուգում է, որ յուրաքանչյուր կարողությունների մանիֆեստն ունի համապատասխան `.to` ֆայլ, ստուգում է I1811150000-ի համար I1811000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000-ը, հաստատում է I1810000000-ը: scrape plus `cargo test` գրանցամատյանները, վավերացնում է ծրագրավորվող գումարով JSON-ի վերարտադրումը և երաշխավորում է, որ հաստատման րոպեները նշում են ակտիվացման դարաշրջանը և մանիֆեստի հեշը: Վերջին պտույտը նաև ամրացնում է NX-6-ում նշված անվտանգության ռելսերը. քվորումը չի կարող գերազանցել հայտարարված վավերացնողի հավաքածուն, պաշտպանված անունների տարածքները պետք է լինեն ոչ դատարկ տողեր, և կարողությունների մանիֆեստները պետք է հայտարարեն միապաղաղ աճող `activation_epoch`/`expiry_epoch` զույգերը լավ ձևավորվածներով: `Allow`/`Deny` էֆեկտներ. `fixtures/nexus/cbdc_rollouts/`-ի տակ գտնվող ապացույցների նմուշի փաթեթն իրականացվում է `integration_tests/tests/nexus/cbdc_rollout_bundle.rs` ինտեգրման թեստի միջոցով՝ վավերացնողը միացված պահելով CI-ում:

## 4. Ծրագրավորվող փողի փոխգործունակություն

Երբ բանկը (տվյալների տարածություն 11) և մանրածախ dApp-ը (տվյալների տարածք 12) երկուսն էլ ներառված են նույն `ComposabilityGroupId`-ի սպիտակ ցուցակում, ծրագրավորվող փողի հոսքերը հետևում են AXT օրինակին `docs/source/nexus.md` §8.6-ից:

1. Մանրածախ dApp-ը պահանջում է ակտիվների բռնակ՝ կապված իր UAID + AXT digest-ի հետ: CBDC գիծը ստուգում է բռնակը `AssetPermissionManifest::evaluate`-ի միջոցով (մերժել հաղթանակները, նպաստները ուժի մեջ են):
2. Երկու DS-ն էլ հայտարարում են բաղադրելիության միևնույն խումբը, ուստի երթուղավորումը դրանք փլուզում է դեպի CBDC գոտի՝ ատոմային ընդգրկման համար (`LaneRoutingPolicy`-ն օգտագործում է `group_id`, երբ փոխադարձաբար սպիտակ ցուցակում է):
3. Կատարման ընթացքում CBDC DS-ն կիրառում է AML/KYC ապացույցները իր շղթայի ներսում (`use_asset_handle` կեղծ կոդը `nexus.md`-ում), մինչդեռ dApp DS-ը թարմացնում է տեղական բիզնես վիճակը միայն այն բանից հետո, երբ CBDC հատվածը հաջողվի:
4. Ապացուցիչ նյութը (FASTPQ + DA պարտավորություններ) մնում է սահմանափակված CBDC գծով; Միաձուլման մատյանների գրառումները գլոբալ վիճակը վճռական են պահում՝ առանց մասնավոր տվյալների արտահոսքի:

Ծրագրավորվող գումարի կրկնօրինակման արխիվը պետք է ներառի.

- AXT նկարագրիչ + կարգավորել հարցումը/պատասխանները:
- Norito-կոդավորված գործարքի ծրար:
- Ստացված անդորրագրեր (երջանիկ ուղի, հերքել ուղին):
- Հեռաչափության հատվածներ `telemetry::fastpq.execution_mode`, `nexus_scheduler_lane_teu_slot_committed` և `lane_commitments` համար:

## 5. Դիտորդականություն և աշխատատեղեր

- **Չափանիշներ.** Մոնիտոր `nexus_scheduler_lane_teu_capacity`, `nexus_scheduler_lane_teu_slot_committed`, `nexus_scheduler_lane_teu_deferral_total{reason}`, `governance_manifest_admission_total` և `lane_governance_sealed_total` (տես `docs/source/telemetry.md`):
- **Վահանակներ.** Ընդլայնել `docs/source/grafana_scheduler_teu.json`-ը CBDC գծի տողով; ավելացնել պանելներ սպիտակ ցուցակի անջատման համար (ծանոթագրություններ յուրաքանչյուր ակտիվացման դարաշրջան) և կարողությունների ժամկետանց խողովակաշարերի համար:
- **Զգուշացումներ.** Գործարկվում է, երբ `rate(nexus_scheduler_lane_teu_deferral_total{reason="cap_exceeded"}[5m]) > 0` 15 րոպեով կամ երբ `lane_governance.manifest_ready=false` պահպանվում է մեկ հարցման ընդմիջումից հետո:
- **Runbook-ի ցուցիչներ.** Հղում դեպի պաշտպանված անվանատարածքի ուղեցույցը `docs/source/governance_api.md`-ում և ծրագրավորվող գումարի անսարքությունների վերացում `docs/source/nexus.md`-ում:

## 6. Ընդունման ստուգաթերթ- [ ] CBDC գոտի հայտարարված `nexus.lane_catalog`-ում՝ TEU մետատվյալներով, որոնք համապատասխանում են TEU թեստերին:
- [ ] Ստորագրված `cbdc.manifest.json` առկա է մանիֆեստի գրացուցակում, վավերացված `cargo test -p integration_tests nexus::lane_registry`-ի միջոցով:
- [ ] Կարողությունների մանիֆեստներ, որոնք տրվում են յուրաքանչյուր UAID-ի համար և պահվում Space Directory-ում; մերժել/թույլատրել գերակայությունը ստուգված միավորի թեստերի միջոցով (`crates/iroha_data_model/src/nexus/manifest.rs`):
- [ ] Սպիտակ ցուցակի ակտիվացումը գրանցված է կառավարման հաստատման ID-ով, `activation_epoch` և Prometheus ապացույցներով:
- [ ] Ծրագրավորվող փողի վերարտադրումը արխիվացված է՝ ցույց տալով բռնակի թողարկումը, մերժումը և թույլատրելի հոսքերը:
- [ ] Ապացույցների փաթեթը վերբեռնված է ծածկագրային ամփոփագրով, որը կցված է կառավարման տոմսից և `status.md`-ից, երբ NX-6-ն ավարտում է 🈯-ից մինչև 🈺:

Այս գրքույկին հետևելը բավարարում է NX-6-ի համար հասանելի փաստաթղթերը և ապաշրջափակում է ապագա ճանապարհային քարտեզի տարրերը (NX-12/NX-15)՝ տրամադրելով CBDC գծի կազմաձևման որոշիչ ձևանմուշ, սպիտակ ցուցակի մուտքագրման և ծրագրավորվող փողի փոխգործունակության համար:
