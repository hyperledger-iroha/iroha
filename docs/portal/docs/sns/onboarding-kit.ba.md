---
lang: ba
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c90050703841af7b2f468ead9e23445ba68344cb9c4db5d7271a8af33a8cb91
source_last_modified: "2025-12-29T18:16:35.174056+00:00"
translation_last_reviewed: 2026-02-07
title: SNS metrics & onboarding kit
description: Dashboard, pricing, and automation artifacts referenced by roadmap item SN-8.
translator: machine-google-reviewed
---

# SNS метрикаһы & Онбординг комплекты

Юл картаһы әйбер **SN-8** ике вәғәҙә өйөмдәре:

1. Баҫма приборҙар таҡталары, улар теркәү, яңыртыу, АРПУ, бәхәстәр, һәм
   `.sora` өсөн туңдырыу тәҙрәләре, I18NI000000018X, һәм `.dao`.
2. Онборд комплектын судно, шулай регистраторҙар һәм стюардтар сым DNS, хаҡтар, һәм
   API-лар эҙмә-эҙлекле ниндәй ҙә булһа ялғау алдынан йәшәй.

Был бит сығанаҡ версияһын көҙгөләй .
[I18NI000000020X] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
тимәк, тышҡы рецензенттар шул уҡ процедураны үтәй ала.

## 1. Метрика өйөмө

### I18NT0000000001X приборҙар таҡтаһы & порталы индерелгән

- Import I18NI000000021X I18NT000000002X (йәки икенсеһе
  аналитика хост) стандарт API аша:

```bash
curl -H "Content-Type: application/json" \
     -H "Authorization: Bearer ${GRAFANA_TOKEN}" \
     -X POST https://grafana.sora.net/api/dashboards/db \
     --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- Шул уҡ JSON был портал битендә ҡөҙрәттәр’ифраме (ҡара **SNS KPI приборҙар таҡтаһы**).
  Ҡасан ғына һеҙ приборҙар панелен ҡағып, йүгерергә
  `npm run build && npm run serve-verified-preview` эсендә I18NI0000000023X эсендә
  раҫлау һәм I18NT000000003X һәм синхронизациялашмалы ҡалыу.

### панелдәр & дәлилдәр

| Панель | Метрика | Идара итеү дәлилдәре |
|------|---------|--------------------|
| Теркәүҙәр & яңыртыу | `sns_registrar_status_total` (уңыш + яңыртыу розеткалары ярлыҡтары) | Пер-сиссажир үткәреүсәнлеге + SLA күҙәтеү. |
| АРПУ / таҙа берәмектәр | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | Финанс теркәүсе килемгә тап килә ала. |
| Бәхәстәр & туңдырыу | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | Әүҙем туңдырыуҙар, арбитраж каденцияһы һәм опекун эш йөкләмәһе күрһәтә. |
| SLA/хата ставкалары | I18NI000000030X, I18NI000000031X | API регрессияларын айырып күрһәтә, улар клиенттарға йоғонто яһағансы. |
| Күмәк манифест трекер | I18NI000000032X, түләү метрикаһы менән I18NI000000033Х маркалар | CSV тамсыларҙы ҡасаба билеттарына тоташтыра. |

Экспорт PDF/CSV I18NT00000000004X (йәки индерелгән fiframe) айлыҡ KPI .
тикшерергә һәм уны тейешле ҡушымта яҙмаһы буйынса беркетергә
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Стюардс шулай уҡ SHA-256-ны тота.
экспортланған өйөмдәрҙең I18NI000000035X буйынса (мәҫәлән,
I18NI000000036X) шуға күрә аудиттар дәлилдәр юлын ҡабатлай ала.

### Ҡушымта автоматлаштырыу

Ҡушымта файлдарын туранан-тура приборҙар таҡтаһы экспортынан генерациялау, шулай итеп, рецензенттар ала
эҙмә-эҙлекле үҙләштереү:

```bash
cargo xtask sns-annex \
  --suffix .sora \
  --cycle 2026-03 \
  --dashboard dashboards/grafana/sns_suffix_analytics.json \
  --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json \
  --output docs/source/sns/reports/.sora/2026-03.md \
  --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md \
  --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- Ярҙам экспортты хеш, UID/тегтар/панелдәр һанын тотоп, һәм яҙа
  Markdown ҡушымтаһы буйынса I18NI0000000037X (ҡара:
  I18NI000000038X өлгөһө был doc менән бер рәттән ҡылған).
- I18NI000000039X X күсермәләр экспортҡа 2012 йылға тиклем .
  I18NI000000040X шулай итеп, ҡушымтаға һылтанмалар
  канонлы дәлилдәр юл; ҡулланыу I18NI0000000041X тик ҡасан һеҙгә кәрәк, тип күрһәтергә
  диапазондан ситтәге архивта урынлашҡан.
- I18NI000000042X мәрәйҙәре идара итеү памяткаһында. Ярҙамсы индерә (йәки
  алмаштыра) I18NI000000043X блок, ул аннектик юлды теркәй, приборҙар таҡтаһы
  артефакт, үҙләштереү, һәм ваҡыт маркаһы шулай дәлилдәр синхронизацияла ҡала, яңынан эшләгәндән һуң.
- I18NI000000044X I18NT000000000000 күсермәһе (I18NI000000045X) һаҡлай.
  тура килтерелгән шулай рецензенттар айырым ҡушымта резюмеларын ҡул менән айырырға тейеш түгел.
- Әгәр һеҙ һикереп I18NI00000000046X/I18NI0000000047X, генерацияланған файлды беркетергә .
  ҡул менән иҫкәрмәләр һәм һаман да тейәп PDF/CSV снимоктар төшөрөлгән I18NT0000000005X.
- Ҡабатланған экспорт өсөн 2012 йылда ялғау/цикл парҙарын исемлеккә индерегеҙ.
  I18NI000000048X һәм йүгерә
  `python3 scripts/run_sns_annex_jobs.py --verbose`. Ярҙам һәр яҙма йөрөй,
  панель экспортын күсерәләр (I18NI000000050X тиклем ғәҙәттәгесә.
  ҡасан билдәләнмәгән), һәм яңырта аннексия блок эсендә һәр көйләү (һәм,
  ҡасан бар, порталы) бер пропускта памятка.
- Run `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (йәки `make check-sns-annex`) эш урындары исемлеге сорттарға бүлергә/дейкатлы ҡала, һәр иҫтәлек `sns-annex` маркеры тап килгәнен йөрөтә, ә ҡушымта стаб бар. Ярҙам `artifacts/sns/annex_schedule_summary.json` яҙа, локаль/хэш резюмелары менән идара итеү пакеттарында ҡулланылған.
Был ҡул күсермәһе/йәбештереү аҙымдарын алып ташлай һәм SN-8 ҡушымтаһы дәлилдәрен эҙмә-эҙлекле тота, ә .
графигы, маркеры, һәм локализация дрейф CI.

## 2. Онборд комплекты компоненттары

### Суффикс проводка

- Реестр схемаһы + селектор ҡағиҙәләре:
  [I18NI000000055X] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  һәм [`docs/source/sns/local_to_global_toolkit.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md).
- DNS скелет ярҙамсыһы:
  [I18NI000000057X] (https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  репетиция ағымы менән
  [ҡапҡа/DNS runbook](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md).
- Һәр регистратор өсөн старт өсөн, ҡыҫҡаса иҫкәрмәһе буйынса 2012
  `docs/source/sns/reports/` селектор өлгөләрен, GAR иҫбатлауҙары һәм DNS хештарын дөйөмләштерә.

### Хаҡтар мутлыҡ

| Ярлыҡ оҙонлоғо | База хаҡы (USD эквиваленты) |
|------------|---------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6–9 | $12 |
| 10+ | $8 |

Суффикс коэффициенттары: `.sora` = 1,0×, I18NI000000060X = 0,8×, `.dao` = 1,3 ×.  
Срок множитель: 2‐се йыллыҡ −5%, 5 йыл −12%; рәхмәт тәҙрә = 30 көн, ҡотҡарыу
= 60 көн (20% түләү, мин $5, max $200). Яҙма тайпылыштар тураһында һөйләшеүҙәр алып бара.
теркәүсе билет.

### Премиум аукциондар vs яңыртыу

1. **Премиум бассейн** — герметизацияланған заявка коммит/асыу (SN-3). Тректар менән заявкалар менән
   I18NI000000062X, һәм манифест 2019 й.
   `docs/source/sns/reports/`.
2. **Голландия яңынан асырға** — рәхмәттән һуң + ҡотҡарыу ваҡыты тамамланды, 7-се көн голланд һатыуын башлағыҙ .
   10× тип тарҡала 15% көнөнә. Ярлыҡ I18NI000000064X менән күренә, шулай итеп
   приборҙар таҡтаһы өҫкө алға китеш ала.
3. **Яңыртыуҙар** — монитор I18NI000000065X һәм
   автомонтаж тикшерелгән исемлекте тотоу (хәбәрҙәр, SLA, fallback түләү рельстар)
   регистратор билет эсендә.

### API-лар һәм автоматлаштырыу

- API контракттары: [`docs/source/sns/registrar_api.md`X] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- Күп ярҙамсы & CSV схемаһы:
  [`docs/source/sns/bulk_onboarding_toolkit.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- Миҫал командаһы:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --ndjson artifacts/sns/releases/2026q2/requests.ndjson \
  --submission-log artifacts/sns/releases/2026q2/submissions.log \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token
``` X

18NI00000000068X сығыш) KPI приборҙар панелендә фильтрҙа манифест ID (ID
тимәк, финанс яраштырырға мөмкин килем панелдәре бер релиз.

### Дәлилдәр өйөмө

1. Контакттар менән билет, суффикс күләме, һәм түләү рельстары.
2. DNS/десетсе дәлилдәр (зонафиле скелеттар + GAR иҫбатлауҙары).
3. Хаҡтар эш ҡағыҙы + идара итеү раҫлаған ниндәй ҙә булһа өҫтөнлөктәр.
4. API/CLI төтөн-һынау артефакттары (I18NI000000069X өлгөләре, CLI транскрипттары).
5. KPI приборҙар таҡтаһы скриншот + CSV экспорты, айлыҡ ҡушымтаға беркетелгән.

## 3. Башланғыс тикшерелгән исемлек

| Аҙым | Хужа | Артефакт |
|-----|-------|----------|
| Приборҙар таҡтаһы импортланған | Продукт аналитикаһы | I18NT000000006X API яуап + приборҙар таҡтаһы UID |
| Портал раҫланған | Док/ДевРел | `npm run build` журналдар + алдан ҡарау скриншот |
| DNS репетиция тулы | Селтәрҙәр/Опс | `sns_zonefile_skeleton.py` сығыштар + runbook журналы |
| Регистратор автоматлаштырыу ҡоро йүгерә | Регистратор Энг | `sns_bulk_onboard.py` тапшырыуҙары журналы |
| Идара итеү дәлилдәре бирелгән | Идара итеү советы | Ҡушымта һылтанма + SHA-256 экспортланған приборҙар таҡтаһы |

Регистратор йәки ялғауҙы әүҙемләштереү алдынан тикшерелгән исемлекте тултырырға. Ҡул ҡуйылған
өйөм SN-8 юл картаһы ҡапҡаһын таҙарта һәм аудиторҙарға бер һылтанма бирә, ҡасан
баҙар старттарын тикшергән.