---
lang: ba
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

# CBDC Шәхси һыҙат плейбук (NX-6)

> **Родка картаһы бәйләнеше:** NX-6 (CBDC шәхси һыҙат шаблон & аҡ исемлек ағымы) һәм NX-14 (Nexus runbooks).  
> **Хужалар:** Финанс хеҙмәттәре WG, Nexus Core WG, үтәү WG.  
> **Статус:** Һүрәтләү — тормошҡа ашырыу ҡармаҡтары бар `crates/iroha_data_model::nexus`, `crates/iroha_core::governance::manifest`, һәм `integration_tests/tests/nexus/lane_registry.rs`, әммә CBDC-специфик манифест, аҡ исемлектәр, һәм оператор runbooks юҡ ине. Был playbook документ конфигурация һәм onboarding эш ағымы, шулай итеп, CBDC таратыу детерминистик дауам итә ала.

## Скап һәм ролдәр

- **Үҙәк банк һыҙаты (“CBDC һыҙаты”):** Рөхсәт ителгән валидаторҙар, опека ҡасаба буферҙары, һәм программаланған-аҡса сәйәсәте. Йүгереп, сикләнгән мәғлүмәттәр киңлеге + һыҙат пары менән үҙ идара итеү манифест.
- **Көнәсәт/ваҡлап һатыу банк мәғлүмәттәре:** Ҡатнашыусылар DS, улар UAID-тар тота, мөмкинлектәрен ала, һәм атом AXT өсөн аҡ исемлеккә индереү мөмкин CBDC һыҙаты.
- **Программа-аҡса dApps:** Тышҡы DS, ҡулланыу CBDC аша ағып `ComposabilityGroup` маршрутлаштырыу бер тапҡыр аҡ исемлеккә индерелгән.
- **Идара итеү & үтәү:** Парламент (йәки эквивалентлы модуль) һыҙатлы һыҙаттарҙы раҫлай, мөмкинлектәрен күрһәтә, һәм аҡ исемлек үҙгәрештәре; үтәлеше магазиндары дәлилдәр менән бер рәттән Norito манифесттары менән бергә өйөмдәр.

**Тәбәп**

1. Каталог + мәғлүмәттәр киңлеге каталогы проводкаһы (`docs/source/nexus_lanes.md`, `defaults/nexus/config.toml`).
2. Лейн манифесты үтәү (`crates/iroha_core/src/governance/manifest.rs`, сират ҡапҡаһы `crates/iroha_core/src/queue.rs`).
3. Мөмкинлек күрһәтә + БАПИД (`crates/iroha_data_model/src/nexus/manifest.rs`).
4. Сграфер ТЭУ квоталары + метрика (`integration_tests/tests/scheduler_teu.rs`, `docs/source/telemetry.md`).

## 1. Һылтанма лейн макеты

### 1.1 Каталог һәм мәғлүмәттәр киңлеге яҙмалары

Өҫтәү өсөн махсус яҙмалар Prometheus һәм `[[nexus.dataspace_catalog]]`. 1500 ТЭУ бер слот һәм дроссель аслыҡ алты слот, өҫтәүенә, күмәртәләп банктар һәм ваҡлап һатыу кошеталары өсөн мәғлүмәттәр киңлеге псевдонимы менән тап килгән.

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

**Иҫкәрмәләр**

- `metadata.scheduler.teu_capacity` һәм `metadata.scheduler.starvation_bound_slots` TEU датчиктарын `integration_tests/tests/scheduler_teu.rs` ярҙамында туҡландыра. Операторҙар уларҙы ҡабул итеү һөҙөмтәләре менән синхронлаштырырға тейеш, шуға күрә `nexus_scheduler_lane_teu_capacity` шаблонға тап килә.
- Һәр мәғлүмәт киңлеге псевдонимы өҫтәге идара итеүҙә күренергә тейеш манифест һәм мөмкинлектәре нәжестәр (аҫта ҡарағыҙ), шулай ҡабул итеү автоматик рәүештә дрейф кире ҡаға.

### 1.2 Лейн манифест скелеты

Lane манифестар каталог аҫтында тура эфирҙа конфигурацияланған `nexus.registry.manifest_directory` (ҡара: `crates/iroha_config/src/parameters/actual.rs`). Файл исемдәре һыҙат псевдонимы тура килергә тейеш (`cbdc.manifest.json`). Схема `integration_tests/tests/nexus/lane_registry.rs`-тағы идара итеү манифест һынауҙарын көҙгөләй.

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

Төп талаптар:-

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
- Һаҡланған исемдәр киңлеге `Queue::push` тарафынан үтәлә (ҡара: `crates/iroha_core/src/queue.rs`), шуға күрә бөтә CBDC контракттары ла `gov_namespace`X + `gov_contract_id`X X.
- `composability_group` яландары `docs/source/nexus.md` §8.6-ла һүрәтләнгән схема буйынса; хужаһы (CBDC һыҙаты) аҡ исемлек һәм квоталар менән тәьмин итә. Whitest исемлектәге DS маскировкалары ғына `group_id_hex` + `activation_epoch` X.
- Манифест күсергәндән һуң, `cargo test -p integration_tests nexus::lane_registry -- --nocapture` `LaneManifestRegistry::from_config` раҫлау өсөн эшләй.

### 1.3 Мөмкинлеклелек нәжестәре (UAID сәйәсәте)

Ҡулланыу мөмкинлеге манифестары (`AssetPermissionManifest` `crates/iroha_data_model/src/nexus/manifest.rs`) `UniversalAccountId` детерминистик пособиеларға бәйләй. Уларҙы йыһан каталогы аша баҫтырығыҙ, шуға күрә банктар һәм dApps ҡул ҡуйылған сәйәсәтте ала ала.

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

- Ҡағиҙәләр еңә, хатта рөхсәт ҡағиҙә матчтары (`ManifestVerdict::Denied`), шуға күрә урынлаштырыу бөтә асыҡ инҡар итеү һуң актуаль мөмкинлек бирә.
- Атом түләү тотҡалары өсөн `AllowanceWindow::PerSlot` һәм `PerMinute`/Prometheus клиенттар сиктәре өсөн роллинг өсөн ҡулланыу.
- UAID/мәғлүмәттәр буйынса бер генә манифест; активациялар һәм сәйәсәтте ротация каденцияһын үтәүҙе дауам итә.
- Хәҙерге һыҙат эшләү ваҡыты хәҙер автоматик рәүештә бара, бер тапҡыр `expiry_epoch` еткән,
  тимәк, операциялар командалары ябай ғына күҙәтеү `SpaceDirectoryEvent::ManifestExpired`,
  архив `nexus_space_directory_revision_total` дельта, һәм раҫлау Torii күрһәтә
  `status = "Expired"`. CLI `manifest expire` командаһы өсөн 2012 йылға ҡала.
  ҡул өҫтөндә йәки дәлилдәр кире тултырмалар.

## 2. Банк Онбординг & Аҡ исемлек эш ағымы

| Фаза | Хужа(тар) | Эштәр | Дәлилдәр |
|------|-----------|----------|---------|
| 0. Инде ҡабул итеү | CBDC ПМО | Йыйып KYC досье, техник DS манифест, валидатор исемлеге, UAID картаһы. | Ҡабул итеү билеты, ҡул ҡуйҙы DS манифест проекты. |
| 1. Идара итеү раҫлауы | Парламент / Ҡабул итеү | Обзор ҡабул итеү пакеты, контрартаж `cbdc.manifest.json`, раҫлай `AssetPermissionManifest`. | Ҡул ҡуйылған идара итеү минуттары, асыҡ ҡылыу хеш. |
| 2. Мөмкинлек бирә | CBDC һыҙат опс | Код `norito::json::to_string_pretty` аша нәжестәр, йыһан каталогы буйынса магазин операторҙарына хәбәр итә. | Манифест JSON + норито `.to` файлы, BLAKE3 distest. |
| 3. Аҡ исемлек активацияһы | CBDC һыҙат опс | Ҡушымта DSID `composability_group.whitelist`, ҡабарып `activation_epoch`, таратыу манифест; кәрәк булһа, мәғлүмәттәр киңлеге маршрутлаштырыуҙы яңыртыу. | Айырылышыу, `kagami config diff` етештереү, идара итеү раҫлау идентификаторы. |
| 4. руд-валитациялау | QA Гильдия / Опс | Интеграция һынауҙары, ТЭУ йөк һынауҙары һәм программаланған-аҡса реплей (аҫта ҡарағыҙ). | `cargo test` журналдар, ТЭУ приборҙар таҡталары, программаланған-аҡса ҡоролма һөҙөмтәләре. |
| 5. Дәлилдәр архивы | Ҡабул итеү WG | Ҡапҡас, раҫлау, мөмкинлектәрен үҙләштереү, һынау сығыштары һәм `artifacts/nexus/cbdc_<stamp>/` буйынса Norito. | Дәлилдәр татарбол, чемпионат файлы, совет ҡул ҡуйыу-өҫтөндә. |

### Аудит өйөм ярҙамсыһы`iroha app space-directory manifest audit-bundle` ярҙамсыһы ҡулланыу йыһан каталогы
playbook снимок өсөн һәр мөмкинлек манифестында дәлилдәр пакет биргәнсе.
JSON (йәки `.to` файҙалы йөк) һәм мәғлүмәттәр киңлеге профилен тәьмин итеү:

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z/cbdc_wholesale \
  --notes "CBDC wholesale refresh"
```

Команда канонлы JSON/Norito/hash күсермәләрен сығара.
`audit_bundle.json`, был теркәлгән UAID, мәғлүмәт киңлеге id, активация/ваҡыт
эпохалар, асыҡ хеш, һәм профиль аудиторлыҡ ҡармаҡтар, шул уҡ ваҡытта кәрәкле үтәү
`SpaceDirectoryEvent` яҙылыуҙары. Дәлилдәр эсендәге өйөмдө ташлағыҙ
каталог шулай аудиторҙар һәм көйләүселәр теүәл байттарҙы һуңыраҡ ҡабатлай ала.

### 2.1 Командалар һәм раҫлауҙар

1. **Лейн күрһәтә:** `cargo test -p integration_tests nexus::lane_registry -- --nocapture lane_manifest_registry_loads_fixture_manifests`.
2. **График квоталары:** `cargo test -p integration_tests scheduler_teu -- queue_teu_backlog_matches_metering queue_routes_transactions_across_configured_lanes`.
3. **Ҡулланма төтөн:** `irohad --sora --config configs/soranexus/nexus/config.toml --chain 0000…` асыҡ каталог менән CBDC файлдарына күрһәтеп, һуңынан `/v1/sumeragi/status` һәм раҫлау `lane_governance.manifest_ready=true` өсөн CBDC һыҙаты.
4. **White исемлеге эҙмә-эҙлеклелек һынау:** Norito күнекмәләр `integration_tests/tests/nexus/cbdc_whitelist.rs`, анализлау `fixtures/space_directory/profile/cbdc_lane_profile.json` һәм һылтанмалы мөмкинлектәре манифесттарын тәьмин итеү өсөн һәр аҡ исемлеккә инеү’s UAID, мәғлүмәт киңлеге, активация эпохаһы, һәм пособие исемлеге тура килә Norito аҫтында. `fixtures/space_directory/capability/`. Һынау журналын NX-6 дәлилдәренә беркетергә, ҡасан аҡ исемлек йәки үҙгәргән һайын.

### 2.2 CLI өҙөктәре

- `cargo run -p iroha_cli -- manifest cbdc --uaid <hash> --dataspace 11 --template cbdc_wholesale` аша UAID + манифест скелеты генерациялау.
- Torii (киңлек каталогы) `iroha app space-directory manifest publish --manifest cbdc_wholesale.manifest.to` (йәки `--manifest-json cbdc_wholesale.manifest.json`) ярҙамында нәшриәт мөмкинлектәрен күрһәтә; тапшырыу иҫәбенә `CanPublishSpaceDirectoryManifest` CBDC мәғлүмәт киңлеге өсөн үткәрергә тейеш.
- HTTP аша баҫығыҙ, әгәр опс өҫтәле дистанцион автоматлаштырыуҙы эшләй:

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

  Torii ҡайтарыу `202 Accepted` бер тапҡыр баҫтырып сығарыу операцияһы сиратҡа баҫылған; шул уҡ
  CIDR/API-токен ҡапҡалары ҡулланыла һәм рөхсәт талаптары сылбырлы тап килә
  CLI эш ағымы.
- Ғәҙәттән тыш хәлдәрҙе ҡайтарыуҙы Torii тиклем POSTing аша дистанцион рәүештә бирергә мөмкин:

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

  Torii ҡайтарыу `202 Accepted` бер тапҡыр ҡайтарып алыу операцияһы сиратҡа баҫылған; шул уҡ
  CIDR/API-токен ҡапҡалары башҡа ҡушымталар остары булараҡ ҡулланыла, һәм Norito .
  һаман да сылбырҙа кәрәк.
- ротат аҡ исемлеккә инеү: `cbdc.manifest.json` мөхәррирләү, `activation_epoch` ҡабарып, һәм бөтә валидаторҙарға хәүефһеҙ күсермәһе аша үҙгәртеп ҡороу; `LaneManifestRegistry` ҡыҙыу-перечать конфигурацияланған һорау алыу арауығында.

## 3. Ҡабул итеү дәлилдәре пакеты

`artifacts/nexus/cbdc_rollouts/<YYYYMMDDThhmmZ>/` буйынса магазин артефакттары һәм идара итеү билетына дарыуҙарҙы беркетергә.

| Файл | Тасуирлама |
|------|-------------|
| `cbdc.manifest.json` | Ҡултамғалы һыҙат аҡ исемлек менән айырылып тора (алдан/аҙаҡ). |
| `capability/<uaid>.manifest.json` & `.to` | Norito + JSON мөмкинлектәре һәр UAID өсөн манифесттар. |
| `compliance/kyc_<bank>.pdf` | Көйләүсе-йөҙ KYC аттестация. |
| `metrics/nexus_scheduler_lane_teu_capacity.prom` | Prometheus скреб иҫбатлау ТЭУ баш. |
| `tests/cargo_test_nexus_lane_registry.log` | Маҡсатлы һынау йүгереүҙән журнал. |
| `tests/cargo_test_scheduler_teu.log` | ТЭУ маршрутлаштырыу үткәнен иҫбатлаусы журнал. |
| `programmable_money/axt_replay.json` | Ҡабатлау стенограмма күрһәтеү программалы-аҡса үҙ-ара эш итеү (ҡара: 4-се бүлек). |
| `approvals/governance_minutes.md` | Ҡул ҡуйылған раҫлау минуттары һылтанмалар асыҡ хеш + активация эпохаһы. |**Валидация сценарийы:** йүгерергә Torii раҫлау өсөн дәлилдәр өйөмө тулы, уны идара итеү билетына беркеткәнсе. Ярҙамсы сканерлау `artifacts/nexus/cbdc_rollouts/` (йәки `CBDC_ROLLOUT_BUNDLE=<path>`) һәр `cbdc.manifest.json` өсөн, анализлау манифест/компосабельность төркөмө, тикшерергә һәр мөмкинлек манифест тура килгән Torii файл, чектар өсөн `kyc_*.pdf` аттестациялар, раҫлай TEU . метрика ҡырҡыу плюс `cargo test` журналдар, программаланған аҡса реплей раҫлай JSON, һәм раҫлау минуттарын тәьмин итә, әүҙемләштереү эпохаһы һәм асыҡ хеш килтерә. Һуңғы әйләнеш шулай уҡ NX-6-ла саҡырылған хәүефһеҙлек рельефтарын үтәй: кворум иғлан ителгән валитатор йыйылмаһынан артып китә алмай, һаҡланған исемдәр киңлеге буш булмаған телмәрҙәр булырға тейеш, ә мөмкинлектәр манифесттары ```json
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
```/```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z/cbdc_wholesale \
  --notes "CBDC wholesale refresh"
``` парҙары менән яҡшы формалашҡан парҙарҙы бер ҡатлы арттырырға тейеш. `Allow`/`Deny` эффекты. `fixtures/nexus/cbdc_rollouts/` буйынса өлгө дәлилдәр пакеты интеграция һынауы `integration_tests/tests/nexus/cbdc_rollout_bundle.rs` ярҙамында тормошҡа ашырыла, валитаторҙы CI-ға сымлы тота.

## 4. Программа-аҡса үҙ-ара эш итеү мөмкинлеге

Бер тапҡыр банк (мәғлүмәттәр 11) һәм ваҡлап һатыу dApp (мәғлүмәттәр 12) икеһе лә аҡ исемлеккә шул уҡ `ComposabilityGroupId`, программалау-аҡса ағымдары AXT өлгөһөнә эйәреп `docs/source/nexus.md` §8.6:

. CBDC һыҙаты `AssetPermissionManifest::evaluate` аша ручканы раҫлай (инҡар итеү еңә, пособиелар үтәлгән).
2. Ике DS ҙа шул уҡ композалылыҡ төркөмөн иғлан итә, шуға күрә маршрутлаштырыу уларҙы атом инклюзияһы өсөн CBDC һыҙатына тарҡата (`LaneRoutingPolicy`X XNI00000127X үҙ-ара аҡ исемлеккә ингәндә `group_id` ҡуллана).
3. Башҡарғанда, CBDC DS үҙенең схемаһы эсендә AML/KYC иҫбатлауҙарын үтәй (`use_asset_handle` псевдокод `nexus.md`), шул уҡ ваҡытта dApp DS урындағы бизнес хәлен яңырта ғына CBDC фрагменты уңышлы булғандан һуң ғына.
4. Дәлил материал (FASTPQ + DA йөкләмәләре) CBDC һыҙатында ғына ҡала; берләштереү-элемтә яҙмалары глобаль дәүләт детерминистик һаҡлай, шәхси мәғлүмәттәр ағып тормай.

Программалы-аҡса реплей архивына тейеш:

- AXT дескриптор + тотҡаһы запрос/яуаптар.
- Norito-кодланған транзакция конверты.
- Һөҙөмтәле квитанциялар (юл менән бәхетле, юлды инҡар итеү).
- `telemetry::fastpq.execution_mode` өсөн телеметрия өҙөктәре, `nexus_scheduler_lane_teu_slot_committed`, һәм `lane_commitments`.

## 5. Күҙәтеүсәнлек һәм йүнһеҙҙәр

- **Метрика:** Монитор `nexus_scheduler_lane_teu_capacity`, `nexus_scheduler_lane_teu_slot_committed`, `nexus_scheduler_lane_teu_deferral_total{reason}`, `governance_manifest_admission_total`, һәм `lane_governance_sealed_total` (`docs/source/telemetry.md` X).
- ** Приборҙар таҡталары:** CBDC һыҙаты менән `docs/source/grafana_scheduler_teu.json` оҙайтыу; өҫтәү өсөн панелдәр өсөн аҡ исемлек churn (аннотациялар һәр активация эпохаһы) һәм мөмкинлектәрҙең ваҡыты үткән торбалар.
- **Иҫкәртмәләр:** Триггер ҡасан `rate(nexus_scheduler_lane_teu_deferral_total{reason="cap_exceeded"}[5m]) > 0` өсөн 15минут йәки ҡасан `lane_governance.manifest_ready=false` бер һорау алыу арауығынан тыш һаҡлана.
- **Ронбук күрһәткестәре:** Һылтанма һаҡланған-исемдәре йүнәлеше `docs/source/governance_api.md` һәм программаланған-аҡса проблемаларын хәл итеү `docs/source/nexus.md`.

## 6. Ҡабул итеү тикшерелгән исемлек- [ ] CBDC һыҙаты `nexus.lane_catalog`-та иғлан ителгән ТЭУ метамағлүмәттәренә тап килгән ТЭУ һынауҙары.
- [ ] `cbdc.manifest.json`-һы асыҡ каталогта булған, `cargo test -p integration_tests nexus::lane_registry` аша раҫланған.
- [ ] Һәр УАИД өсөн сығарылған һәм йыһан каталогында һаҡланған мөмкинлектәрҙең мөмкинлектәре; инҡар/рөхсәт өҫтөнлөк аша тикшерелгән берәмек һынауҙары (`crates/iroha_data_model/src/nexus/manifest.rs`).
- [ ] Аҡ исемлек активацияһы менән теркәлгән идара итеү раҫлау идентификаторы, `activation_epoch`, һәм Prometheus дәлилдәре.
- [ ] Программалы-аҡса реплей архивланған, ручка сығарыуҙы күрһәтеү, инҡар итеү, һәм рөхсәт ағымдар.
- [ ] Дәлилдәр өйөмдәре менән тейәп криптографик диестер, идара итеү билет һәм `status.md` бер тапҡыр NX-6 сығарылыш уҡыусылары 🈯 🈺 тиклем.

Был playbook-тан һуң NX-6 өсөн тапшырыуҙы ҡәнәғәтләндерә һәм киләсәктә юл картаһы әйберҙәрен (NX-12/NX-15) блоктарын аса, был CBDC һыҙат конфигурацияһы өсөн детерминистик шаблон бирә, аҡ исемлектә бортинг һәм программаланған аҡса үҙ-ара эш итеү мөмкинлеге.
