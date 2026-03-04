---
lang: ba
direction: ltr
source: docs/source/ministry/review_panel_summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7325e72d18ec406eb134622ab51211fbb6582ebcc26bd719499e209db70f761b
source_last_modified: "2025-12-29T18:16:35.983094+00:00"
translation_last_reviewed: 2026-02-07
title: Review Panel Summary Workflow (MINFO-4a)
summary: Generate the neutral referendum summary with balanced citations, AI manifest references, and volunteer brief coverage.
translator: machine-google-reviewed
---

# панель панелендә резюме (МИНФО-4а)

Юл картаһы пункты **MINFO-4a — Нейтраль резюме генераторы** ҡабул ителгән көн тәртибе тәҡдимен, ирекмәндәр ҡыҫҡа корпусын һәм аттестлы ЯИ модерацияһын нейтраль референдум референдум резюмеһына әйләндергән ҡабатланған эш ағымы талап итә. Тапшырыу тейеш:

- Norito структураһы (`ReviewPanelSummaryV1`) булараҡ сығышты яҙып алығыҙ, шуға күрә идара итеү уны манифест һәм бюллетендәр менән бергә архивлай ала.
- Сығанаҡ материалын линт, тиҙ етешһеҙлеккә осрағанда, ҡасан тикшерелгән панель баланслы ярҙам/ҡаршы ҡаплау йәки ҡасан факттар цитаталар юҡ.
- Һылтанма AI манифест һәм тәҡдим дәлилдәре өйөмө һәр өҫтөнлөк, тәьмин итеү сәйәсәт жюри автоматлаштырылған һәм кеше контексын күрә, тауыш биргәнсе.

## CLI ҡулланыу

Эш ағымы `cargo xtask` составында:

```bash
cargo xtask ministry-panel synthesize \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --volunteer docs/examples/ministry/volunteer_brief_template.json \
  --ai-manifest docs/examples/ai_moderation_calibration_manifest_202602.json \
  --panel-round RP-2026-05 \
--output artifacts/review_panel/AC-2026-001-RP-2026-05.json
```

Кәрәкле индереүҙәр:

1. `--proposal` – JSON файҙалы йөкләмәләр үтәгән `AgendaProposalV1`. Ярҙамсы резюме генерациялау алдынан схеманы раҫлай.
2. `--volunteer` – JSON массив ирекмәндәр брифтары, улар `docs/source/ministry/volunteer_brief_template.md` X. Теманан тыш яҙмалар автоматик рәүештә иғтибарға алынмай.
.
4. `--panel-round` – Ағымдағы тикшерелгән тур өсөн идентификатор (`RP-YYYY-##`).
. Ҡулланыу `--language` тәҡдим теле һәм `--generated-at` өҫтөндә эшләү өсөн детерминистик Unix ваҡыт маркаһы (миллисекундтар) тәьмин итеү өсөн, ҡасан тарихы кире тултырыу.

Бер тапҡыр үҙ аллы резюме генерацияланған, йүгерергә
[`cargo xtask ministry-panel packet`] (referendum_packet.md) йыйыу өсөн ярҙамсы
тулы референдум досье (`ReferendumPacketV1`). Тәьмин итеү
`--summary-out` пакет командаһына шул уҡ резюме файлы һаҡланасаҡ, ә
уны индереү өсөн пакет объекты өсөн аҫҡы ҡулланыусылар өсөн.

### автоматлаштырыу аша `ministry-transparency ingest`

Командалар, улар инде эшләй ```bash
cargo xtask ministry-transparency ingest \
  --quarter 2026-Q4 \
  --ledger artifacts/ministry/ledger.json \
  --appeals artifacts/ministry/appeals.json \
  --denylist artifacts/ministry/denylist.json \
  --treasury artifacts/ministry/treasury.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --panel-proposal artifacts/ministry/proposal_AC-2026-041.json \
  --panel-ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --panel-summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/ingest.json
``` өсөн квартал һайын дәлилдәр өйөмдәре хәҙер панель резюмеһын шул уҡ торбаға тегә ала:

```bash
cargo xtask ministry-transparency ingest \
  --quarter 2026-Q4 \
  --ledger artifacts/ministry/ledger.json \
  --appeals artifacts/ministry/appeals.json \
  --denylist artifacts/ministry/denylist.json \
  --treasury artifacts/ministry/treasury.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --panel-proposal artifacts/ministry/proposal_AC-2026-041.json \
  --panel-ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --panel-summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/ingest.json
```

Бөтә дүрт `--panel-*` флагтары бергә тәьмин ителергә тейеш (һәм `--volunteer` талап итә). Команда тикшерелгән панель резюме `--panel-summary-out`, һеңдерелгән файҙалы йөк эсендә ингест снимок, һәм теркәү суммаһы шулай аҫҡы инструменталь раҫлай ала дәлилдәр.

## Бәйләнеш һәм етешһеҙлектәр режимдары

`cargo xtask ministry-panel synthesize` резюме яҙғанға тиклем түбәндәге инварианттарҙы үтәй:

- **Баланслы позициялар:** кәмендә бер ярҙам ҡыҫҡа һәм бер ҡаршы ҡыҫҡа булырға тейеш. Яҡтылыҡ юғалған йүгереүҙе тасуири хата менән туҡтата.
- **Цитит ҡаплауы:** өҫтөнлөктәре тик факт рәттәрҙән етештерелә, улар цитаталарҙы үҙ эсенә ала. Һуң цитаталар бер ҡасан да блокировка төҙөү, әммә һәр ҡағылған ҡыҫҡаса `warnings[]` буйынса исемлеккә индерелгән етештереү.
- **Һәр өҫтөнлөклө һылтанмалар:** һәр өҫтөнлөккә һылтанмаларҙы үҙ эсенә ала (а) ирекмәндәр факты рәт (s), (б) AI манифест идентификаторы, һәм (в) тәҡдимдән беренсе дәлилдәр беркетелгән шулай пакет һәр ваҡыт һылтанмалар менән кире ҡул ҡуйылған артефакттар.Әгәр ҙә ниндәй ҙә булһа чек уңышһыҙлыҡҡа осрай, команда нулдән тыш статусы менән сыға һәм проблемалы яҙмала мәрәйҙәр. Уңышлы йүгерә `ReviewPanelSummaryV1` схемаһына тап килгән JSON файлын яҙа һәм идара итеү манифестарында һеңдерелергә мөмкин.

## Сығыш структураһы

`ReviewPanelSummaryV1` йәшәй Norito һәм һәр ҡулланыусы өсөн `iroha_data_model` йәшник аша мөмкин. Төп бүлектәрҙә:

- `overview` – Титул, нейтраль дөйөм хөкөм, һәм ҡарар контексты өсөн сәйәсәт присяжныйҙар пакеты.
- `stance_distribution` – Бер позицияға брифтар һәм факттар рәттәре. Аҫҡа приборҙар таҡталары был уҡыуҙы раҫлау өсөн яҡтыртыу алдынан баҫтырыу.
- `highlights` – Тулы квалификациялы цитаталар менән бер позицияға ике фактҡа тиклем.
- `ai_manifest` – Ҡабатлаусанлыҡ манифестынан алынған метамағлүмәттәр экстракцияланған (нисек UUID, йүгерсе версияһы, сиктәр).
- `volunteer_references` – Пер-ҡыҫҡаса статистика (тел, позиция, рәттәр, рәт-рәттәрҙе цитаталар) аудит өсөн.
- `warnings` – Ирекле форма линт хәбәрҙәре һүрәтләгән әйберҙәрҙе һүрәтләгән (мәҫәлән, цитаталар юҡ факт рәттәре).

## Миҫал

`docs/examples/ministry/review_panel_summary_example.json` ярҙамсы менән етештерелгән тулы өлгө бар. Ул баланслы ярҙам/ҡаршы яҡтыртыу күрһәтә, цитата проводка, асыҡ һылтанмалар, һәм иҫкәрткән ептәр өсөн факт рәттәр, уларҙы пропагандалау мөмкин булмаған өҫтөнлөктәр. Ҡулланғанда, ҡасан оҙайтыу приборҙар таҡтаһы, идара итеү күренә, йәки SDK инструменттар, улар нейтраль резюме ҡулланырға кәрәк.

> **Тип:** генерацияланған резюме менән бер рәттән ҡул ҡуйылған манифест һәм ирекмәндәр ҡыҫҡа һеңдерелгән референдум дәлилдәр өйөмөндә үҙ эсенә ала, шулай итеп, сәйәсәт присяжныйҙары һәр артефакт тикшерә ала һылтанма тикшерелгән панель.