---
lang: kk
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

> **Жол картасының байланысы:** NX-6 (CBDC жеке жолақ үлгісі және ақ тізім ағыны) және NX-14 (Nexus runbooks).  
> **Иелер:** Қаржы қызметтері ЖТ, Nexus Негізгі ЖТ, Сәйкестік ЖТ.  
> **Күй:** Жобалау — іске асыру ілмектері `crates/iroha_data_model::nexus`, `crates/iroha_core::governance::manifest` және `integration_tests/tests/nexus/lane_registry.rs` бойынша бар, бірақ CBDC-арнайы манифесттер, ақ тізімдер және оператордың жұмыс кітапшалары жоқ болды. Бұл оқу кітапшасы анықтамалық конфигурацияны және қосымша жұмыс процесін құжаттайды, осылайша CBDC орналастырулары айқын түрде жалғаса алады.

## Ауқым және рөлдер

- **Орталық банк жолы («CBDC жолағы»):** Рұқсат етілген валидаторлар, кастодиандық есеп айырысу буферлері және бағдарламаланатын ақша саясаты. Жеке басқару манифестімен шектелген деректер кеңістігі + жолақ жұбы ретінде жұмыс істейді.
- **Көтерме/бөлшек банк деректер кеңістігі:** UAID ұстайтын, мүмкіндік манифесттерін алатын және CBDC жолағы бар атомдық AXT үшін ақ тізімге енгізілуі мүмкін қатысушы DS.
- **Бағдарламаланатын ақша dApps:** CBDC ағындарын тұтынатын сыртқы DS ақ тізімге енгізілгеннен кейін `ComposabilityGroup` маршруттауы арқылы өтеді.
- **Басқару және сәйкестік:** Парламент (немесе баламалы модуль) жолақ манифесттерін, мүмкіндік манифесттерін және ақ тізімдегі өзгерістерді бекітеді; сәйкестік Norito манифестерімен қатар дәлелдемелерді сақтайды.

**Тәуелділіктер**

1. Жолдық каталог + деректер кеңістігі каталогының сымдары (`docs/source/nexus_lanes.md`, `defaults/nexus/config.toml`).
2. Жолдық манифестті орындау (`crates/iroha_core/src/governance/manifest.rs`, `crates/iroha_core/src/queue.rs` ішіндегі кезек қақпасы).
3. Мүмкіндік манифесттері + UAID (`crates/iroha_data_model/src/nexus/manifest.rs`).
4. Жоспарлағыш TEU квоталары + көрсеткіштер (`integration_tests/tests/scheduler_teu.rs`, `docs/source/telemetry.md`).

## 1. Анықтамалық жолақ орналасуы

### 1.1 Жол каталогы және деректер кеңістігі жазбалары

`[[nexus.lane_catalog]]` және `[[nexus.dataspace_catalog]]` үшін арнайы жазбаларды қосыңыз. Төмендегі мысал `defaults/nexus/config.toml` ұяшығына 1500 TEU сақтайтын және алты слотқа аштықты азайтатын CBDC жолағымен, сонымен қатар көтерме банктер мен бөлшек сауда әмияндары үшін сәйкес деректер кеңістігінің бүркеншік аттарын кеңейтеді.

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

**Ескертулер**

- `metadata.scheduler.teu_capacity` және `metadata.scheduler.starvation_bound_slots` `integration_tests/tests/scheduler_teu.rs` орындайтын TEU өлшемдерін береді. `nexus_scheduler_lane_teu_capacity` үлгіге сәйкес келуі үшін операторлар оларды қабылдау нәтижелерімен синхрондауы керек.
- Жоғарыдағы әрбір деректер кеңістігінің бүркеншік аты басқару манифесттерінде және мүмкіндік манифесттерінде (төменде қараңыз) пайда болуы керек, осылайша қабылдау дрейфті автоматты түрде қабылдамайды.

### 1.2 Жолақты манифест қаңқасы

Жолақ манифесттері `nexus.registry.manifest_directory` арқылы конфигурацияланған каталогта тұрады (`crates/iroha_config/src/parameters/actual.rs` қараңыз). Файл атаулары жолақ бүркеншік аттарымен сәйкес келуі керек (`cbdc.manifest.json`). Схема `integration_tests/tests/nexus/lane_registry.rs` ішіндегі басқару манифестінің сынақтарын көрсетеді.

```json
{
  "lane": "cbdc",
  "dataspace": "cbdc.core",
  "version": 1,
  "governance": "central_bank_multisig",
  "validators": [
    "soraカタカナ...",
    "soraカタカナ...",
    "soraカタカナ...",
    "soraカタカナ..."
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

Негізгі талаптар:- Валидаторлар каталогта бар канондық I105 тіркелгі идентификаторлары ** болуы керек (`@domain` жоқ; `@domain` тек нақты бағыттау анықтамасы ретінде қосыңыз). `quorum` параметрін мультисиг шегіне (≥2) орнатыңыз.
- Қорғалған аттар кеңістігі `Queue::push` арқылы орындалады (`crates/iroha_core/src/queue.rs` қараңыз), сондықтан барлық CBDC келісімшарттары `gov_namespace` + `gov_contract_id` көрсетуі керек.
- `composability_group` өрістері `docs/source/nexus.md` §8.6-да сипатталған схемаға сәйкес келеді; иесі (CBDC жолағы) ақ тізім мен квоталарды қамтамасыз етеді. Ақ тізімдегі DS манифесі тек `group_id_hex` + `activation_epoch` көрсетеді.
- Манифестті көшіргеннен кейін `LaneManifestRegistry::from_config` оны жүктейтінін растау үшін `cargo test -p integration_tests nexus::lane_registry -- --nocapture` іске қосыңыз.

### 1.3 Мүмкіндік манифесттері (UAID саясаттары)

Мүмкіндік манифесттері (`AssetPermissionManifest` `crates/iroha_data_model/src/nexus/manifest.rs`) `UniversalAccountId` детерминирленген рұқсаттармен байланыстырады. Оларды Space Directory арқылы жариялаңыз, осылайша банктер мен dApps қол қойылған саясаттарды ала алады.

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

- Рұқсат ету ережесі (`ManifestVerdict::Denied`) сәйкес келсе де, бас тарту ережелері жеңеді, сондықтан барлық ашық бас тартуларды тиісті рұқсаттардан кейін қойыңыз.
- Атомдық төлем тұтқалары үшін `AllowanceWindow::PerSlot` және тұтынушы шектеулерін жылжыту үшін `PerMinute`/`PerDay` пайдаланыңыз.
- UAID/деректер кеңістігі үшін бір манифест жеткілікті; белсендірулер мен жарамдылық мерзімі саясаттың айналу каденциясын қамтамасыз етеді.
- Енді `expiry_epoch` жеткенде жолақты орындау уақыты автоматты түрде аяқталады,
  сондықтан операциялық топтар жай ғана `SpaceDirectoryEvent::ManifestExpired` бақылайды,
  `nexus_space_directory_revision_total` дельтасын мұрағаттаңыз және Torii шоуларын тексеріңіз
  `status = "Expired"`. CLI `manifest expire` пәрмені қол жетімді болып қалады
  қолмен қайта анықтау немесе дәлелдемелерді толтыру.

## 2. Банкке қосылу және ақ тізім жұмыс процесі

| кезең | Ие(лер) | Әрекеттер | Дәлелдер |
|-------|----------|---------|----------|
| 0. Қабылдау | CBDC PMO | KYC деректерін, техникалық DS манифестін, валидатор тізімін, UAID картасын жинаңыз. | Қабылдау билеті, қол қойылған DS манифест жобасы. |
| 1. Басқаруды бекіту | Парламент / Сәйкестік | Қабылдау пакетін қарап шығыңыз, `cbdc.manifest.json` қарсы белгісі, `AssetPermissionManifest` бекітіңіз. | Қол қойылған басқару хаттамалары, манифест хэшін орындау. |
| 2. Қабілетті шығару | CBDC жолақ операциялары | `norito::json::to_string_pretty` арқылы манифесттерді кодтау, Space Directory астында сақтау, операторларға хабарлау. | Манифест JSON + norito `.to` файлы, BLAKE3 дайджест. |
| 3. Ақ тізімді белсендіру | CBDC жолақ операциялары | `composability_group.whitelist` ішіне DSID қосыңыз, `activation_epoch` соққысын жасаңыз, манифестті таратыңыз; қажет болса, деректер кеңістігінің маршруттауын жаңартыңыз. | Манифест айырмашылығы, `kagami config diff` шығысы, басқаруды мақұлдау идентификаторы. |
| 4. Шығарылымды тексеру | QA Guild / Ops | Интеграция сынақтарын, TEU жүктеме сынақтарын және бағдарламаланатын ақшаны қайталауды іске қосыңыз (төменде қараңыз). | `cargo test` журналдары, TEU бақылау тақталары, бағдарламаланатын ақша арматурасының нәтижелері. |
| 5. Дәлелдеме мұрағаты | Сәйкестік ЖТ | Бума манифесі, мақұлдаулары, мүмкіндік дайджесттері, сынақ нәтижелері және `artifacts/nexus/cbdc_<stamp>/` астындағы Prometheus сынықтары. | Дәлелдеме tarball, бақылау сомасы файлы, кеңестің қолтаңбасы. |

### Аудит жинағының көмекшісіSpace Directory ішінен `iroha app space-directory manifest audit-bundle` көмекшісін пайдаланыңыз
дәлелдер пакетін толтырар алдында әрбір мүмкіндік манифестінің суретін алу үшін ойын кітабы.
Манифест JSON (немесе `.to` пайдалы жүктемесі) және деректер кеңістігі профилін қамтамасыз етіңіз:

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z/cbdc_wholesale \
  --notes "CBDC wholesale refresh"
```

Пәрмен канондық JSON/Norito/хэш көшірмелерін шығарады.
`audit_bundle.json`, ол UAID, деректер кеңістігі идентификаторы, белсендіру/жарамдылық мерзімін жазады
талапты орындау кезінде дәуірлер, манифест хэштері және профильді тексеру ілгектері
`SpaceDirectoryEvent` жазылымдары. Буманы дәлелдердің ішіне тастаңыз
аудиторлар мен реттеушілер нақты байтты кейінірек қайталай алатындай каталог.

### 2.1 Пәрмендер және тексерулер

1. **Жолақ манифесті:** `cargo test -p integration_tests nexus::lane_registry -- --nocapture lane_manifest_registry_loads_fixture_manifests`.
2. **Жоспарлаушы квоталары:** `cargo test -p integration_tests scheduler_teu -- queue_teu_backlog_matches_metering queue_routes_transactions_across_configured_lanes`.
3. **Қол түтіні:** CBDC файлдарын көрсететін манифест каталогы бар `irohad --sora --config configs/soranexus/nexus/config.toml --chain 0000…`, содан кейін `/v1/sumeragi/status` түймесін басып, CBDC жолағы үшін `lane_governance.manifest_ready=true` түймесін басыңыз.
4. **Ақ тізімнің сәйкестігі сынағы:** `cargo test -p integration_tests nexus::cbdc_whitelist -- --nocapture` `integration_tests/tests/nexus/cbdc_whitelist.rs` жаттығулары, `fixtures/space_directory/profile/cbdc_lane_profile.json` талдауы және сілтеме жасалған мүмкіндік ақ тізімдегі әрбір жазбаның UAID, деректер кеңістігі, белсендіру дәуірі мен man1800000 тізімдеріне сәйкес келуін қамтамасыз етеді. `fixtures/space_directory/capability/` астында. Ақ тізім немесе манифестер өзгерген сайын сынақ журналын NX-6 дәлелдер жинағына тіркеңіз.

### 2.2 CLI үзінділері

- `cargo run -p iroha_cli -- manifest cbdc --uaid <hash> --dataspace 11 --template cbdc_wholesale` арқылы UAID + манифест қаңқасын жасаңыз.
- `iroha app space-directory manifest publish --manifest cbdc_wholesale.manifest.to` (немесе `--manifest-json cbdc_wholesale.manifest.json`) арқылы Torii (Ғарыш каталогы) мүмкіндігі манифестін жариялау; жіберуші тіркелгіде CBDC деректер кеңістігі үшін `CanPublishSpaceDirectoryManifest` болуы керек.
- Егер операциялық үстел қашықтан автоматтандыруды іске қосып жатса, HTTP арқылы жариялау:

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "soraカタカナ...",
            "private_key": "ed25519:CiC7…",
            "manifest": '"'"'$(cat fixtures/space_directory/capability/cbdc_wholesale.manifest.json)'"'"',
            "reason": "CBDC onboarding wave 4"
          }'
  ```

  Torii жариялау транзакциясы кезекке қойылғаннан кейін `202 Accepted` қайтарады; бірдей
  CIDR/API таңбалауыш қақпалары қолданылады және тізбектегі рұқсат талабы сәйкес келеді
  CLI жұмыс процесі.
- Төтенше жағдайдан бас тартуды Torii нөміріне POSTing арқылы қашықтан шығаруға болады:

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests/revoke \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "soraカタカナ...",
            "private_key": "ed25519:CiC7…",
            "uaid": "uaid:0f4d…ab11",
            "dataspace": 11,
            "revoked_epoch": 9216,
            "reason": "Fraud investigation #NX-16-R05"
          }'
  ```

  Torii қайтарып алу транзакциясы кезекке қойылғаннан кейін `202 Accepted` қайтарады; бірдей
  CIDR/API таңбалауыш қақпалары басқа қолданбаның соңғы нүктелері ретінде қолданылады және `CanPublishSpaceDirectoryManifest`
  әлі де тізбекте қажет.
- Ақ тізім мүшелігін айналдырыңыз: `cbdc.manifest.json` өңдеңіз, `activation_epoch` қатесін өзгертіңіз және қауіпсіз көшірме арқылы барлық валидаторларға қайта орналастырыңыз; `LaneManifestRegistry` конфигурацияланған сұрау интервалында ыстық қайта жүктейді.

## 3. Сәйкестік дәлелдемелері жинағы

Артефактілерді `artifacts/nexus/cbdc_rollouts/<YYYYMMDDThhmmZ>/` астында сақтаңыз және дайджестті басқару билетіне тіркеңіз.

| Файл | Сипаттама |
|------|-------------|
| `cbdc.manifest.json` | Ақ тізім айырмашылығы бар қол қойылған жолақ манифесті (бұрын/кейін). |
| `capability/<uaid>.manifest.json` & `.to` | Norito + JSON мүмкіндігі әрбір UAID үшін көрсетіледі. |
| `compliance/kyc_<bank>.pdf` | Реттеушіге арналған KYC аттестаттау. |
| `metrics/nexus_scheduler_lane_teu_capacity.prom` | Prometheus TEU биіктігін дәлелдейтін қырғыш. |
| `tests/cargo_test_nexus_lane_registry.log` | Манифестті тексеруден журналға өтіңіз. |
| `tests/cargo_test_scheduler_teu.log` | TEU бағдарлау рұқсаттарын растайтын журнал. |
| `programmable_money/axt_replay.json` | Бағдарламаланатын ақша мен ақшаның өзара әрекеттесу мүмкіндігін көрсететін транскриптті қайталау (4-бөлімді қараңыз). |
| `approvals/governance_minutes.md` | Манифест хэшіне + белсендіру дәуіріне сілтеме жасайтын қол қойылған бекіту хаттамасы. |**Тексеру сценарийі:** басқару билетіне тіркемес бұрын дәлелдер жинағының аяқталғанын растау үшін `ci/check_cbdc_rollout.sh` іске қосыңыз. Көмекші әрбір `artifacts/nexus/cbdc_rollouts/` (немесе `CBDC_ROLLOUT_BUNDLE=<path>`) әрбір `cbdc.manifest.json` үшін сканерлейді, манифест/құрастыру тобын талдайды, әрбір мүмкіндік манифестінде сәйкес `.to` файлы бар екенін тексереді, `.to` файлын тексереді, `.to` тексереді, Prometheus метриканың скраб плюс `cargo test` журналдары, JSON бағдарламаланатын ақшаны қайталауды растайды және бекіту минуттарында белсендіру дәуірі мен манифест хэшіне сілтеме жасайды. Соңғы нұсқа сонымен қатар NX-6-да шақырылған қауіпсіздік рельстерін күшейтеді: кворум жарияланған валидатор жиынынан аспауы керек, қорғалған аттар кеңістігі бос емес жолдар болуы керек және мүмкіндік манифесттері жақсы пішіні бар `activation_epoch`/`expiry_epoch` монотонды өсетінін жариялауы керек. `Allow`/`Deny` әсерлері. `fixtures/nexus/cbdc_rollouts/` астында үлгі дәлелдеме пакеті `integration_tests/tests/nexus/cbdc_rollout_bundle.rs` біріктіру сынағы арқылы орындалады, валидаторды CI жүйесіне жалғайды.

## 4. Бағдарламаланатын-ақшамен өзара әрекеттесу

Банк (11-деректер кеңістігі) және бөлшек dApp (12-деректер кеңістігі) екеуі де бірдей `ComposabilityGroupId` тізімінде ақ тізімге енгізілгеннен кейін, бағдарламаланатын ақша ағындары `docs/source/nexus.md` §8.6 AXT үлгісіне сәйкес келеді:

1. Retail dApp өзінің UAID + AXT дайджестіне байланыстырылған актив дескрипторын сұрайды. CBDC жолағы тұтқаны `AssetPermissionManifest::evaluate` арқылы тексереді (жеңістерді қабылдамау, жәрдемақылар орындалды).
2. Екі DS бірдей құрамдас топты жариялайды, сондықтан маршруттау оларды атомдық қосу үшін CBDC жолағына қысқартады (`LaneRoutingPolicy` өзара ақ тізімге енгізілген кезде `group_id` пайдаланады).
3. Орындау барысында CBDC DS өз тізбегіндегі AML/KYC дәлелдеулерін (`use_asset_handle` псевдокоды `nexus.md`) енгізеді, ал dApp DS жергілікті бизнес күйін CBDC фрагменті сәтті болғаннан кейін ғана жаңартады.
4. Дәлелдеу материалы (FASTPQ + DA міндеттемелері) CBDC жолағында қалады; біріктіру журналының жазбалары жеке деректерді сыртқа шығармай, жаһандық күйді детерминистік күйде сақтайды.

Бағдарламаланатын ақшаны қайталау мұрағаты мыналарды қамтуы керек:

- AXT дескрипторы + өңдеу сұрауы/жауаптары.
- Norito кодталған транзакция конверті.
- Нәтижесі түбіртектері (бақытты жол, жолды жоққа шығару).
- `telemetry::fastpq.execution_mode`, `nexus_scheduler_lane_teu_slot_committed` және `lane_commitments` үшін телеметриялық үзінділер.

## 5. Бақылану және іске қосу кітаптары

- **Көрсеткіштер:** Монитор `nexus_scheduler_lane_teu_capacity`, `nexus_scheduler_lane_teu_slot_committed`, `nexus_scheduler_lane_teu_deferral_total{reason}`, `governance_manifest_admission_total` және `lane_governance_sealed_total` (`docs/source/telemetry.md` қараңыз).
- **Бақылау тақталары:** CBDC жолақ жолымен `docs/source/grafana_scheduler_teu.json` кеңейтіңіз; ақ тізімді бұзу (әрбір белсендіру дәуірінде аннотациялар) және мүмкіндіктердің жарамдылық мерзімі аяқталатын құбыр желілері үшін панельдерді қосыңыз.
- **Ескертулер:** `rate(nexus_scheduler_lane_teu_deferral_total{reason="cap_exceeded"}[5m]) > 0` 15 минут бойы немесе `lane_governance.manifest_ready=false` бір сауалнама интервалынан кейін сақталған кезде іске қосылады.
- **Runbook көрсеткіштері:** `docs/source/governance_api.md` жүйесіндегі қорғалған аттар кеңістігі нұсқаулығына және `docs/source/nexus.md` ішіндегі бағдарламаланатын ақша ақаулықтарын жоюға сілтеме.

## 6. Қабылдауды бақылау парағы- [ ] CBDC жолағы `nexus.lane_catalog` TEU сынақтарына сәйкес TEU метадеректерімен жарияланған.
- [ ] Қол қойылған `cbdc.manifest.json` манифест каталогында бар, `cargo test -p integration_tests nexus::lane_registry` арқылы расталған.
- [ ] Әрбір UAID үшін шығарылған және Ғарыш каталогында сақталған мүмкіндік манифестері; бірлік сынақтары (`crates/iroha_data_model/src/nexus/manifest.rs`) арқылы расталған басымдылықты жоққа шығару/рұқсат ету.
- [ ] Басқаруды мақұлдау идентификаторы, `activation_epoch` және Prometheus дәлелдерімен жазылған ақ тізімді белсендіру.
- [ ] Бағдарламаланатын ақшаны қайталау мұрағатталған, өңдеуді, бас тартуды және ағындарды рұқсат етуді көрсетеді.
- [ ] Криптографиялық дайджестпен жүктеп салынған, басқару билетінен және `status.md` сілтемесі NX-6 🈯 мен 🈺 аралығында біткен кезде дәлелдер жинағы.

Осы оқу кітабынан кейін NX-6 үшін жеткізілетін құжаттаманы қанағаттандырады және CBDC жолағын конфигурациялау, ақ тізімді қосу және бағдарламаланатын ақшамен өзара әрекеттесу үшін детерминирленген үлгіні қамтамасыз ету арқылы болашақ жол картасы элементтерін (NX-12/NX-15) бұғаттаудан шығарады.