---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6404e09aa8f3520328249a1d5c41309b291087908a2a8f5abae3e2fe12de44fb
source_last_modified: "2026-01-05T09:28:11.861409+00:00"
translation_last_reviewed: 2026-02-07
id: deal-engine
title: SoraFS Deal Engine
sidebar_label: Deal Engine
description: Overview of the SF-8 deal engine, Torii integration, and telemetry surfaces.
translator: machine-google-reviewed
---

:::ескерту Канондық дереккөз
:::

# SoraFS Deal Engine

SF-8 жол картасы трегі SoraFS мәміле қозғалтқышын ұсынады,
арасындағы сақтау және алу келісімдерінің детерминирленген есебі
клиенттер мен провайдерлер. Келісімдер Norito пайдалы жүктемелерімен сипатталған
`crates/sorafs_manifest/src/deal.rs`-де анықталған, мәміле шарттарын, облигацияны қамтиды
құлыптау, ықтималдық микротөлемдер және есеп айырысу жазбалары.

Ендірілген SoraFS жұмысшысы (`sorafs_node::NodeHandle`) енді
Әрбір түйін процесі үшін `DealEngine` данасы. Қозғалтқыш:

- `DealTermsV1` көмегімен мәмілелерді растайды және тіркейді;
- репликацияны пайдалану туралы хабарланған кезде XOR-деноминацияланған алымдарды есептейді;
- детерминистикалық көмегімен ықтималдық микротөлем терезелерін бағалайды
  Blake3 негізіндегі іріктеу; және
- басқаруға жарамды бухгалтерлік кітаптың суреттері мен есеп айырысу пайдалы жүктемелерін жасайды
  басып шығару.

Бірлік сынақтары валидацияны, микротөлемді таңдауды және есеп айырысу ағындарын қамтиды
операторлар API интерфейстерін сенімді түрде қолдана алады. Есеп айырысулар қазір шығады
`DealSettlementV1` басқару пайдалы жүктемелері, сымдарды тікелей SF-12 құрылғысына қосу
жариялау құбырын және `sorafs.node.deal_*` OpenTelemetry сериясын жаңартыңыз
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) Torii бақылау тақталары мен SLO үшін
орындау. Кейінгі элементтер аудитордың бастамасымен кесу автоматтандыруына және
бас тарту семантикасын басқару саясатымен үйлестіру.

Пайдалану телеметриясы енді `sorafs.node.micropayment_*` метрика жинағын береді:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, және билет есептегіштері
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Бұл қорытындылар ықтималдықты көрсетеді
лотерея ағыны, осылайша операторлар микротөлемдегі ұтыстар мен несиені ауыстыруды салыстыра алады
есеп айырысу нәтижелерімен.

## Torii интеграциясы

Torii арнайы соңғы нүктелерді көрсетеді, осылайша провайдерлер пайдалану туралы есеп бере алады және
тапсырыс беру сымдарынсыз қызмет ету мерзімі:

- `POST /v2/sorafs/deal/usage` `DealUsageReport` телеметриясын қабылдайды және қайтарады
  детерминирленген бухгалтерлік есеп нәтижелері (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` ағынмен ағымдағы терезені аяқтайды
  нәтижесінде `DealSettlementRecord` base64 кодталған `DealSettlementV1` жанында
  басқару DAG басылымына дайын.
- Torii `/v2/events/sse` арнасы қазір `SorafsGatewayEvent::DealUsage` таратады
  әрбір пайдалануды қорытындылайтын жазбалар (дәуір, өлшенген ГиБ-сағат, билет
  есептегіштер, детерминирленген зарядтар), `SorafsGatewayEvent::DealSettlement`
  канондық есеп айырысу журналының суретін қосатын жазбалар
  Дисктегі басқару артефактісінің BLAKE3 дайджесті/өлшемі/негізі64 және
  `SorafsGatewayEvent::ProofHealth` PDP/PoTR шектері болған сайын ескертеді
  асып кетті (провайдер, терезе, ереуіл/суытатын күй, айыппұл сомасы). Тұтынушылар алады
  жаңа телеметрияға, есеп айырысуларға немесе денсаулық жағдайын растау ескертулеріне сұраусыз әрекет ету үшін провайдер бойынша сүзгіден өткізіңіз.

Екі соңғы нүкте SoraFS квоталық шеңберіне жаңа арқылы қатысады
`torii.sorafs.quota.deal_telemetry` терезесі, операторларға реттеуге мүмкіндік береді
орналастыру үшін рұқсат етілген жіберу жылдамдығы.