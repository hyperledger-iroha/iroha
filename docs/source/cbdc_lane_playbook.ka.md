---
lang: ka
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

> **საგზაო რუქის კავშირი:** NX-6 (CBDC კერძო ზოლის შაბლონი და თეთრი სიის ნაკადი) და NX-14 (Nexus runbooks).  
> **მფლობელები:** Financial Services WG, Nexus Core WG, Compliance WG.  
> **სტატუსები:** შედგენა — განხორციელების კაუჭები არსებობს `crates/iroha_data_model::nexus`, `crates/iroha_core::governance::manifest` და `integration_tests/tests/nexus/lane_registry.rs`-ში, მაგრამ CBDC-ს სპეციფიკური მანიფესტები, თეთრი სიები და ოპერატორის გაშვებები არ იყო. ეს სათამაშო წიგნი ადასტურებს საცნობარო კონფიგურაციას და ბორტზე მუშაობის პროცესს, რათა CBDC განლაგება განვითარდეს დეტერმინისტულად.

## სფერო და როლები

- **ცენტრალური ბანკის შესახვევი („CBDC lane“):** ნებადართული ვალიდატორები, პატიმრობის ანგარიშსწორების ბუფერები და პროგრამირებადი ფულის პოლიტიკა. მუშაობს როგორც შეზღუდული მონაცემთა სივრცე + ზოლის წყვილი თავისი მართვის მანიფესტით.
- **საბითუმო/საცალო ბანკის მონაცემთა სივრცეები:** მონაწილე DS, რომელიც ფლობს UAID-ებს, იღებს შესაძლებლობების მანიფესტებს და შეიძლება მოხვდეს ატომური AXT-ის თეთრ სიაში CBDC ხაზით.
- **პროგრამირებადი ფულის dApps:** გარე DS, რომელიც მოიხმარს CBDC-ს, მიედინება `ComposabilityGroup` მარშრუტით, როგორც კი თეთრ სიაში მოხვდება.
- **მმართველობა და შესაბამისობა:** პარლამენტი (ან ეკვივალენტური მოდული) ამტკიცებს ხაზების მანიფესტებს, შესაძლებლობების მანიფესტებს და თეთრ სიაში ცვლილებებს; შესაბამისობა ინახავს მტკიცებულებების პაკეტებს Norito მანიფესტებთან ერთად.

**დამოკიდებულებები**

1. ზოლის კატალოგი + მონაცემთა სივრცის კატალოგის გაყვანილობა (`docs/source/nexus_lanes.md`, `defaults/nexus/config.toml`).
2. ხაზის მანიფესტის აღსრულება (`crates/iroha_core/src/governance/manifest.rs`, რიგის კარიბჭე `crates/iroha_core/src/queue.rs`-ში).
3. შესაძლებლობების მანიფესტები + UAIDs (`crates/iroha_data_model/src/nexus/manifest.rs`).
4. Scheduler TEU კვოტები + მეტრიკა (`integration_tests/tests/scheduler_teu.rs`, `docs/source/telemetry.md`).

## 1. საცნობარო ზოლის განლაგება

### 1.1 ზოლის კატალოგი და მონაცემთა სივრცის ჩანაწერები

დაამატეთ გამოყოფილი ჩანაწერები `[[nexus.lane_catalog]]` და `[[nexus.dataspace_catalog]]`. ქვემოთ მოყვანილი მაგალითი აფართოებს `defaults/nexus/config.toml`-ს CBDC ზოლით, რომელიც ინახავს 1500 TEU-ს თითო სლოტზე და ამცირებს შიმშილს ექვს სლოტამდე, პლუს მონაცემთა სივრცის შესატყვისი მეტსახელები საბითუმო ბანკებისა და საცალო საფულეებისთვის.

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

** შენიშვნები **

- `metadata.scheduler.teu_capacity` და `metadata.scheduler.starvation_bound_slots` კვებავს `integration_tests/tests/scheduler_teu.rs` TEU ლიანდაგებს. ოპერატორებმა უნდა შეინარჩუნონ ისინი სინქრონიზებული მიღების შედეგებთან, რათა `nexus_scheduler_lane_teu_capacity` ემთხვეოდეს შაბლონს.
- ზემოთ მოცემული მონაცემთა სივრცის ყველა მეტსახელი უნდა გამოჩნდეს მმართველობის მანიფესტებში და შესაძლებლობების მანიფესტებში (იხ. ქვემოთ), ასე რომ დაშვება ავტომატურად უარყოფს დრიფტს.

### 1.2 შესახვევის მანიფესტის ჩონჩხი

Lane მანიფესტი პირდაპირ ეთერში `nexus.registry.manifest_directory`-ის მეშვეობით კონფიგურირებული დირექტორიაში (იხ. `crates/iroha_config/src/parameters/actual.rs`). ფაილის სახელები უნდა ემთხვეოდეს ხაზის მეტსახელებს (`cbdc.manifest.json`). სქემა ასახავს მართვის მანიფესტის ტესტებს `integration_tests/tests/nexus/lane_registry.rs`-ში.

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

ძირითადი მოთხოვნები:-

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
- დაცული სახელების სივრცეები ახორციელებს `Queue::push`-ს (იხ. `crates/iroha_core/src/queue.rs`), ამიტომ ყველა CBDC კონტრაქტში უნდა იყოს მითითებული `gov_namespace` + `gov_contract_id`.
- `composability_group` ველები მიჰყვება `docs/source/nexus.md` §8.6-ში აღწერილ სქემას; მფლობელი (CBDC ხაზი) ​​აწვდის თეთრ სიას და კვოტებს. თეთრ სიაში შეყვანილი DS მანიფესტები მიუთითებს მხოლოდ `group_id_hex` + `activation_epoch`.
- მანიფესტის კოპირების შემდეგ, გაუშვით `cargo test -p integration_tests nexus::lane_registry -- --nocapture`, რათა დაადასტუროთ `LaneManifestRegistry::from_config` ჩატვირთვა.

### 1.3 შესაძლებლობების გამოვლინებები (UAID პოლიტიკა)

შესაძლებლობების გამოვლინებები (`AssetPermissionManifest` `crates/iroha_data_model/src/nexus/manifest.rs`-ში) აკავშირებს `UniversalAccountId`-ს დეტერმინისტულ შეღავათებთან. გამოაქვეყნეთ ისინი Space Directory-ის მეშვეობით, რათა ბანკებმა და dApps-მა შეძლონ ხელმოწერილი წესების მიღება.

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

- უარყოფის წესები იმარჯვებს მაშინაც კი, როდესაც დაშვების წესი ემთხვევა (`ManifestVerdict::Denied`), ამიტომ განათავსეთ ყველა აშკარა უარყოფა შესაბამისი ნებართვის შემდეგ.
- გამოიყენეთ `AllowanceWindow::PerSlot` ატომური გადახდის სახელურებისთვის და `PerMinute`/`PerDay` მომხმარებელთა ლიმიტების გადასატანად.
- საკმარისია ერთი მანიფესტი UAID/მონაცემთა სივრცეში; გააქტიურება და ვადა ახორციელებს პოლიტიკის როტაციის კადენციას.
- ზოლის გაშვების დრო ახლა იწურება, ავტომატურად გამოიხატება `expiry_epoch`-ის მიღწევის შემდეგ,
  ასე რომ, ოპერაციული გუნდები უბრალოდ აკონტროლებენ `SpaceDirectoryEvent::ManifestExpired`-ს,
  დაარქივეთ `nexus_space_directory_revision_total` დელტა და გადაამოწმეთ Torii შოუები
  `status = "Expired"`. CLI `manifest expire` ბრძანება რჩება ხელმისაწვდომი
  ხელით უგულებელყოფა ან მტკიცებულების შევსება.

## 2. ბანკის ჩართვა და Whitelist სამუშაო პროცესი

| ფაზა | მფლობელ(ებ)ი | მოქმედებები | მტკიცებულება |
|-------|----------|---------|----------|
| 0. მიღება | CBDC PMO | შეაგროვეთ KYC დოსიე, ტექნიკური DS მანიფესტი, ვალიდატორების სია, UAID-ის რუქები. | შესასვლელი ბილეთი, ხელმოწერილი DS მანიფესტის პროექტი. |
| 1. მმართველობის დამტკიცება | პარლამენტი / შესაბამისობა | გადახედეთ მიმღების პაკეტს, კონტრასაგზაო ნიშანი `cbdc.manifest.json`, დაადასტურეთ `AssetPermissionManifest`. | ხელმოწერილი მმართველობის ოქმები, მანიფესტის ჩადენის ჰეში. |
| 2. შესაძლებლობების გაცემა | CBDC შესახვევის ოპერაციები | დაშიფვრეთ მანიფესტები `norito::json::to_string_pretty`-ით, შეინახეთ Space Directory-ში, აცნობეთ ოპერატორებს. | მანიფესტი JSON + norito `.to` ფაილი, BLAKE3 დაიჯესტი. |
| 3. თეთრი სიის გააქტიურება | CBDC შესახვევის ოპერაციები | DSID-ის დამატება `composability_group.whitelist`-ზე, შეკუმშვა `activation_epoch`, გავრცელება manifest; საჭიროების შემთხვევაში მონაცემთა სივრცის მარშრუტიზაციის განახლება. | მანიფესტის განსხვავება, `kagami config diff` გამომავალი, მმართველობის დამტკიცების ID. |
| 4. Rollout validation | QA გილდია / ოპერაციები | ჩაატარეთ ინტეგრაციის ტესტები, TEU ჩატვირთვის ტესტები და პროგრამირებადი ფულის გამეორება (იხ. ქვემოთ). | `cargo test` ჟურნალები, TEU დაფები, პროგრამირებადი ფულის მოწყობილობების შედეგები. |
| 5. მტკიცებულებათა არქივი | შესაბამისობა WG | ნაკრების მანიფესტები, დამტკიცებები, შესაძლებლობების დამუშავება, ტესტის შედეგები და Prometheus იშლება `artifacts/nexus/cbdc_<stamp>/` ქვეშ. | მტკიცებულება tarball, საკონტროლო ფაილი, საბჭოს ხელმოწერა. |

### აუდიტის ნაკრების დამხმარეგამოიყენეთ `iroha app space-directory manifest audit-bundle` დამხმარე Space Directory-დან
სათამაშო წიგნი, რათა გადაიღოს თითოეული შესაძლებლობების მანიფესტი მტკიცებულებათა პაკეტის შეტანამდე.
მოგვაწოდეთ manifest JSON (ან `.to` payload) და მონაცემთა სივრცის პროფილი:

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z/cbdc_wholesale \
  --notes "CBDC wholesale refresh"
```

ბრძანება გამოსცემს კანონიკურ JSON/Norito/ჰეშ ასლებს გვერდით
`audit_bundle.json`, რომელიც აღრიცხავს UAID-ს, მონაცემთა სივრცის ID-ს, აქტივაციას/ვადის გასვლას
ეპოქები, მანიფესტი ჰეში და პროფილის აუდიტის კაუჭები საჭიროების შესრულებისას
`SpaceDirectoryEvent` გამოწერები. ჩაყარეთ შეკვრა მტკიცებულებების შიგნით
დირექტორია, რათა აუდიტორებმა და მარეგულირებლებმა შეძლონ ზუსტი ბაიტების გამეორება მოგვიანებით.

### 2.1 ბრძანებები და ვალიდაციები

1. **ჩიხი გამოიხატება:** `cargo test -p integration_tests nexus::lane_registry -- --nocapture lane_manifest_registry_loads_fixture_manifests`.
2. **განრიგის კვოტები:** `cargo test -p integration_tests scheduler_teu -- queue_teu_backlog_matches_metering queue_routes_transactions_across_configured_lanes`.
3. **მექანიკური კვამლი:** `irohad --sora --config configs/soranexus/nexus/config.toml --chain 0000…` manifest დირექტორიათ, რომელიც მიუთითებს CBDC ფაილებზე, შემდეგ დააჭირეთ `/v1/sumeragi/status` და გადაამოწმეთ `lane_governance.manifest_ready=true` CBDC ზოლისთვის.
4. **თეთრი სიის თანმიმდევრულობის ტესტი:** `cargo test -p integration_tests nexus::cbdc_whitelist -- --nocapture` სავარჯიშოები `integration_tests/tests/nexus/cbdc_whitelist.rs`, `fixtures/space_directory/profile/cbdc_lane_profile.json`-ის ანალიზი და მითითებული შესაძლებლობა ცხადყოფს, რომ თეთრ სიაში ყველა ჩანაწერის UAID, მონაცემთა სივრცე, აქტივაციის ეპოქა და დაშვებული სია800X001-ის ქვეშ შეესაბამება Nexus-ს. `fixtures/space_directory/capability/`. მიამაგრეთ ტესტის ჟურნალი NX-6 მტკიცებულების პაკეტს, როდესაც იცვლება თეთრი სია ან მანიფესტები.

### 2.2 CLI ფრაგმენტები

- შექმენით UAID + მანიფესტის ჩონჩხი `cargo run -p iroha_cli -- manifest cbdc --uaid <hash> --dataspace 11 --template cbdc_wholesale`-ის საშუალებით.
- გამოაქვეყნეთ შესაძლებლობების მანიფესტი Torii-ში (Space Directory) `iroha app space-directory manifest publish --manifest cbdc_wholesale.manifest.to` (ან `--manifest-json cbdc_wholesale.manifest.json`) გამოყენებით; გაგზავნის ანგარიშში უნდა იყოს `CanPublishSpaceDirectoryManifest` CBDC მონაცემთა სივრცისთვის.
- გამოაქვეყნეთ HTTP-ით, თუ ოპერაციული მაგიდა მუშაობს დისტანციურ ავტომატიზაციაზე:

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

  Torii აბრუნებს `202 Accepted`-ს, როგორც კი გამოქვეყნების ტრანზაქცია რიგში დადგება; იგივე
  CIDR/API-token gates ვრცელდება და ჯაჭვზე ნებართვის მოთხოვნა ემთხვევა
  CLI სამუშაო პროცესი.
- გადაუდებელი გაუქმება შეიძლება გაიცეს დისტანციურად POST-ით Torii-ზე:

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

  Torii აბრუნებს `202 Accepted`-ს, როგორც კი გაუქმების ტრანზაქცია რიგში დადგება; იგივე
  CIDR/API-ტოკენის კარიბჭეები გამოიყენება როგორც სხვა აპის საბოლოო წერტილები და `CanPublishSpaceDirectoryManifest`
  ჯერ კიდევ საჭიროა ჯაჭვზე.
- შეცვალეთ თეთრი სიის წევრობა: დაარედაქტირეთ `cbdc.manifest.json`, შეცვალეთ `activation_epoch` და გადაანაწილეთ უსაფრთხო ასლის მეშვეობით ყველა ვალიდატორზე; `LaneManifestRegistry` ცხელ-ხელახლა იტვირთება კონფიგურირებულ გამოკითხვის ინტერვალზე.

## 3. შესაბამისობის მტკიცებულებათა ნაკრები

შეინახეთ არტეფაქტები `artifacts/nexus/cbdc_rollouts/<YYYYMMDDThhmmZ>/` ქვეშ და დაურთოთ დაიჯესტი მმართველობის ბილეთს.

| ფაილი | აღწერა |
|------|-------------|
| `cbdc.manifest.json` | ხელმოწერილი ხაზის მანიფესტი თეთრი სიის სხვაობით (ადრე/შემდეგ). |
| `capability/<uaid>.manifest.json` & `.to` | Norito + JSON შესაძლებლობა ვლინდება თითოეული UAID-ისთვის. |
| `compliance/kyc_<bank>.pdf` | მარეგულირებლის წინაშე KYC ატესტაცია. |
| `metrics/nexus_scheduler_lane_teu_capacity.prom` | Prometheus ნაკაწრი, რომელიც ადასტურებს TEU თავსახურს. |
| `tests/cargo_test_nexus_lane_registry.log` | შესვლა manifest ტესტის გაშვებიდან. |
| `tests/cargo_test_scheduler_teu.log` | ჟურნალი, რომელიც ადასტურებს TEU მარშრუტიზაციის პასებს. |
| `programmable_money/axt_replay.json` | ხელახლა ჩანაწერი, რომელიც აჩვენებს პროგრამირებადი ფულის თავსებადობას (იხ. ნაწილი 4). |
| `approvals/governance_minutes.md` | ხელმოწერილი დამტკიცების წუთები მანიფესტის ჰეშის + აქტივაციის ეპოქის მითითებით. |**გადამოწმების სკრიპტი:** გაუშვით `ci/check_cbdc_rollout.sh`, რათა დაადასტუროთ, რომ მტკიცებულებების ნაკრები დასრულებულია, სანამ მას მმართველობის ბილეთზე მიამაგრებთ. დამხმარე ასკანირებს `artifacts/nexus/cbdc_rollouts/`-ს (ან `CBDC_ROLLOUT_BUNDLE=<path>`) ყოველი `cbdc.manifest.json`-ისთვის, აანალიზებს მანიფესტის/კომპოზიტორობის ჯგუფს, ამოწმებს, რომ თითოეულ შესაძლებლობის მანიფესტს აქვს `.to` შესატყვისი ფაილი, ამოწმებს Prometheus-ზე Prometheus-ის შესატყვისი ფაილი. scrape plus `cargo test` ჩაწერს, ამოწმებს პროგრამირებადი ფულის განმეორებით JSON-ს და უზრუნველყოფს დამტკიცების წუთებში ციტირებს აქტივაციის ეპოქას და მანიფესტ ჰეშის. უახლესი ბრუნი ასევე ახორციელებს უსაფრთხოების რელსებს, რომლებიც გამოიძახეს NX-6-ში: კვორუმი არ შეიძლება აღემატებოდეს დეკლარირებული ვალიდატორის კომპლექტს, დაცული სახელების სივრცეები არ უნდა იყოს ცარიელი სტრიქონები და შესაძლებლობების მანიფესტებმა უნდა გამოაცხადონ მონოტონურად მზარდი `activation_epoch`/`expiry_epoch` წყვილები კარგად ჩამოყალიბებული. `Allow`/`Deny` ეფექტები. ნიმუშის მტკიცებულებათა პაკეტი `fixtures/nexus/cbdc_rollouts/`-ის ქვეშ განხორციელდება ინტეგრაციის ტესტით `integration_tests/tests/nexus/cbdc_rollout_bundle.rs`, ვალიდატორის შენარჩუნებით CI-ში.

## 4. პროგრამირებადი ფულის თავსებადობა

მას შემდეგ, რაც ბანკი (მონაცემთა სივრცე 11) და საცალო dApp (მონაცემთა სივრცე 12) ორივე შედის თეთრ სიაში იმავე `ComposabilityGroupId`-ში, პროგრამირებადი ფულის ნაკადები მიჰყვება AXT შაბლონს `docs/source/nexus.md` §8.6-დან:

1. საცალო dApp ითხოვს აქტივის სახელურს, რომელიც დაკავშირებულია მის UAID + AXT დაიჯესტთან. CBDC ზოლი ამოწმებს სახელურს `AssetPermissionManifest::evaluate`-ის მეშვეობით (უარი მოგება, შემწეობები აღსრულებულია).
2. ორივე DS აცხადებს კომპოზიციადობის ერთსა და იმავე ჯგუფს, ამიტომ მარშრუტიზაცია აქცევს მათ CBDC ზოლში ატომური ჩართვისთვის (`LaneRoutingPolicy` იყენებს `group_id`-ს, როდესაც ორმხრივად შედის თეთრ სიაში).
3. შესრულების დროს, CBDC DS ახორციელებს AML/KYC მტკიცებულებებს მის წრეში (`use_asset_handle` ფსევდოკოდი `nexus.md`-ში), ხოლო dApp DS განაახლებს ადგილობრივ ბიზნეს მდგომარეობას მხოლოდ CBDC ფრაგმენტის წარმატების შემდეგ.
4. დამადასტურებელი მასალა (FASTPQ + DA ვალდებულებები) რჩება CBDC ზოლში; შერწყმის წიგნის ჩანაწერები ინარჩუნებს გლობალურ მდგომარეობას დეტერმინისტულად, პირადი მონაცემების გაჟონვის გარეშე.

პროგრამირებადი ფულის გამეორების არქივი უნდა შეიცავდეს:

- AXT აღმწერი + მოთხოვნა/პასუხების დამუშავება.
- Norito-ში კოდირებული ტრანზაქციის კონვერტი.
- მიღებული ქვითრები (ბედნიერი გზა, უარყავით გზა).
- ტელემეტრიის ფრაგმენტები `telemetry::fastpq.execution_mode`, `nexus_scheduler_lane_teu_slot_committed` და `lane_commitments`.

## 5. დაკვირვება და გაშვების წიგნები

- **მეტრიკა:** მონიტორი `nexus_scheduler_lane_teu_capacity`, `nexus_scheduler_lane_teu_slot_committed`, `nexus_scheduler_lane_teu_deferral_total{reason}`, `governance_manifest_admission_total` და `lane_governance_sealed_total` (იხ. `docs/source/telemetry.md`).
- **Dashboards:** გააფართოვეთ `docs/source/grafana_scheduler_teu.json` CBDC ზოლის მწკრივით; დაამატეთ პანელები თეთრი სიის ჩაქრობისთვის (ანოტაციები ყოველი გააქტიურების ეპოქაში) და შესაძლებლობების ვადის გასვლის მილსადენებისთვის.
- **გაფრთხილებები:** გააქტიურდება, როდესაც `rate(nexus_scheduler_lane_teu_deferral_total{reason="cap_exceeded"}[5m]) > 0` 15 წუთის განმავლობაში ან როდესაც `lane_governance.manifest_ready=false` გრძელდება ერთი გამოკითხვის ინტერვალის მიღმა.
- **Runbook მაჩვენებლები:** ბმული დაცულ სახელთა სივრცის მითითებაზე `docs/source/governance_api.md`-ში და პროგრამირებადი ფულის პრობლემების მოგვარება `docs/source/nexus.md`-ში.

## 6. მიღების ჩამონათვალი- [ ] CBDC ზოლი გამოცხადებულია `nexus.lane_catalog`-ში TEU მეტამონაცემების შესატყვისი TEU ტესტებით.
- [ ] ხელმოწერილი `cbdc.manifest.json` იმყოფება manifest დირექტორიაში, დამოწმებული `cargo test -p integration_tests nexus::lane_registry`-ით.
- [ ] შესაძლებლობების მანიფესტები, რომლებიც გაცემულია ყველა UAID-ისთვის და ინახება Space Directory-ში; უარის თქმის/დაშვების უპირატესობის დადასტურება ერთეულის ტესტების მეშვეობით (`crates/iroha_data_model/src/nexus/manifest.rs`).
- [ ] თეთრი სიის გააქტიურება ჩაწერილია მმართველობის დამტკიცების ID-ით, `activation_epoch` და Prometheus მტკიცებულებებით.
- [ ] პროგრამირებადი ფულის ხელახალი დაკვრა დაარქივებულია, სახელურის გაცემის, უარყოფისა და ნაკადების დაშვების დემონსტრირება.
- [ ] მტკიცებულებების ნაკრები ატვირთული კრიპტოგრაფიული დაიჯესტით, მიბმული მმართველობის ბილეთიდან და `status.md` ერთხელ NX-6 დაამთავრებს 🈯-დან 🈺-მდე.

ამ სათამაშო წიგნის დაცვა აკმაყოფილებს NX-6-ისთვის მიწოდებულ დოკუმენტაციას და განბლოკავს სამომავლო საგზაო რუქის ერთეულებს (NX-12/NX-15) CBDC ზოლის კონფიგურაციის განმსაზღვრელი შაბლონის მიწოდებით, თეთრი სიის ჩართვა და პროგრამირებადი ფულის თავსებადობა.
