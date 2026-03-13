---
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Capacity Marketplace Validation
tags: [SF-2c, acceptance, checklist]
summary: Acceptance checklist covering provider onboarding, dispute workflows, and treasury reconciliation gating the SoraFS capacity marketplace general availability.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# I18NT0000000005X ҡәҙерле баҙарҙы раҫлаусы исемлек

**Теҙлекте тикшерергә:** 2026-03-18 → 2026-03-24  
**Программа хужалары:** Һаҡлау командаһы (`@storage-wg`), Идара итеү советы (I18NI0000008X), ҡаҙна гильдияһы (I18NI0000009X)  
**Сода:** Провайдер onboarding торбалары, бәхәстәр суд ағымы, һәм ҡаҙна ярашыу процестары өсөн кәрәкле SF-2c GA.

Түбәндәге тикшерелгән исемлек тикшерергә тейеш, тышҡы операторҙар өсөн баҙарға мөмкинлек биргәнсе. Һәр рәт һылтанмалар детерминистик дәлилдәр (һынауҙар, ҡоролма, йәки документация), аудиторҙар ҡабатлай ала.

## Ҡабул итеү тикшерелгән исемлек

### Провайдер Онбординг

| Тикшерергә | Валидация | Дәлилдәр |
|-------|------------|----------|
| Реестр канонлы ҡәҙерле декларацияларҙы ҡабул итә | Интеграция тест күнекмәләре `/v2/sorafs/capacity/declare` аша ҡушымта API, тикшерергә ҡултамға менән эш итеү, метамағлүмәттәрҙе тотоу, һәм ҡул-төйөн реестрына ҡул. | I18NI0000011X |
| Аҡыллы килешәү тап килмәгән файҙалы йөктәрҙе кире ҡаға | Блок һынау тәьмин итеү провайдер идентификаторҙары һәм ҡылған GiB яландарына тап килгән ҡул ҡуйылған декларация ныҡышмалы алдынан. | I18NI0000012X |
| CLI канонлы onboarding артефакттарын сығара | CLI йүгән яҙа детерминистик I18NT0000000002X/JSON/Base64 сығыштар һәм раҫлай түңәрәк-сәйәхәт, шулай итеп, операторҙар спектакль декларация офлайн. | I18NI0000013X |
| Оператор етәксеһе ҡабул итеү эш ағымын һәм идара итеү ҡоршауҙарын тота | Документация декларация схемаһын иҫәпләй, сәйәсәт дефолттары, һәм совет өсөн аҙымдарҙы ҡарау. | I18NI0000014X |

### Бәхәсле хәл итеү

| Тикшерергә | Валидация | Дәлилдәр |
|-------|------------|----------|
| Бәхәс яҙмалары канонлы файҙалы йөкләмәләр менән һаҡлана | Блок тесты бәхәсте теркәй, һаҡланған файҙалы йөктө расшифровкалай, ә статусты көткән раҫлай, уларҙы баш кейеме детерминизмына гарантиялай. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI бәхәс генераторы тап килә канон схемаһы | CLI тесты ҡаплай Base64/I18NT000000003X сығыштары һәм JSON резюмелары өсөн I18NI000000016X, дәлилдәр өйөмдәре хеш детерминистик. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Ҡабатлау һынау иҫбатлай бәхәс/штраф детерминизм | Корпус-уңышһыҙлыҡҡа осраған телеметрия ике тапҡыр етештерә бер үк леджер, кредит, һәм бәхәс снимоктары, шуға күрә slashs тиҫтерҙәре буйынса детерминистик. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Яҙмышлы документтар эскалация һәм ҡайтарыу ағымы | Операциялар етәкселеге совет эш ағымын, дәлилдәр талаптарын һәм кире ҡағыу процедураларын тота. | `../dispute-revocation-runbook.md` |

### ҡаҙна ярашыу

| Тикшерергә | Валидация | Дәлилдәр |
|-------|------------|----------|
| көнлөк матчтар 30 көнлөк һыу һибеү проекцияһы | 1990 йылдарҙа был йүнәлештәге эштәрҙең иң мөһимдәренең береһе булып биш провайдер 30 ҡасаба тәҙрәләре аша үтә, уларҙы көтөлгән түләү белешмәһенә ҡаршы леджер яҙмаларын таратты. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Клеггер экспорты ярашыуын төндә теркәлгән | `capacity_reconcile.py` гонорарҙарҙы башҡарылған XOR күсермә экспорты менән сағыштыра, I18NT000000000000000 метрикаһын сығара, ә ҡапҡалар ҡаҙна раҫлауы аша Alertmanager аша. | I18NI00000022Х, `docs/source/sorafs/runbooks/capacity_reconciliation.md:1`, `dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Биллинг приборҙар таҡталары ер өҫтө штрафтары һәм тупланған телеметрия | I18NT000000001X импорт участкалары GiB·our acctrual, забастовка иҫәпләүселәр, һәм облигация залог өсөн шылтыратыуҙа күренеш. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Баҫылған доклад архивтары методологияһын һәм реплей командаларын һыуыта | Отчет реквизиттары күләмен һеңдерергә, башҡарыу командалары, һәм күҙәтеүсәнлек ҡармаҡтар өсөн аудиторҙар. | `./sf2c-capacity-soak.md` |

## Башҡарма иҫкәрмәләр

Ҡабаттан идара итеү валидация пакеты алдынан ҡул ҡуйыу:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Операторҙар I18NI000000027X менән onboarding/booking запрос файҙалы йөктәрҙе яңыртырға һәм идара итеү билеттары менән бер рәттән һөҙөмтәлә барлыҡҡа килгән JSON/I18NT00000000004X байттарын архивлау тейеш.

## Яҙма Артефакттар

| Артефакт | Юл | блейк2б-256 |
|--------|-------|-------------|
| Провайдер onboarding раҫлау пакеты | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Бәхәс хәл итеү раҫлау пакеты | I18NI000000030X | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Ҡаҙна ярашыу раҫлау пакеты | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Был артефакттар ҡул ҡуйылған күсермәләрен һаҡлау менән сығарыу өйөмө һәм уларҙы идара итеү үҙгәрештәре яҙмаһында бәйләй.

## раҫлауҙар

- Һаҡлау командаһы етәксеһе — @storage-tl (2026-03-24)  
- Идара итеү советы секретары — @concil-sec (2026-03-24)  
- ҡаҙна операциялары ҡурғаш — @treasury-опс (2026-03-24)