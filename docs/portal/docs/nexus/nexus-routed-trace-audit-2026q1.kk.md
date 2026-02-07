---
lang: kk
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd9f85b2c7845414c27016f699da179e13c41c9b9e0ce5b178ab88a950744500
source_last_modified: "2025-12-29T18:16:35.142540+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-routed-trace-audit-2026q1
title: 2026 Q1 routed-trace audit report (B1)
description: Mirror of `docs/source/nexus_routed_trace_audit_report_2026q1.md`, covering the quarterly telemetry rehearsal outcomes.
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::ескерту Канондық дереккөз
Бұл бет `docs/source/nexus_routed_trace_audit_report_2026q1.md` көрсетеді. Қалған аудармалар түскенше екі көшірмені де туралаңыз.
:::

№ 2026 1-тоқсан Бағытталған бақылау аудитінің есебі (B1)

Жол картасы тармағы **B1 — Бағытталған бақылау аудиттері және телеметрияның негізгі көрсеткіші**
Nexus маршруттық бақылау бағдарламасының тоқсан сайынғы шолуы. Бұл есеп құжатталады
Q12026 аудит терезесі (қаңтар-наурыз), сондықтан басқару кеңесі қол қоя алады
Q2 ұшыру жаттығуларына дейінгі телеметриялық қалып.

## Ауқым және уақыт шкаласы

| Trace ID | Терезе (UTC) | Мақсаты |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | Көп жолақты қосу алдында жолаққа кіру гистограммаларын, кезек өсектерін және ескерту ағынын тексеріңіз. |
| `TRACE-TELEMETRY-BRIDGE` | 24.02.2026 10:00–10:45 | AND4/AND7 кезеңдерінің алдында OTLP қайталауын, боттардың айырмашылығын және SDK телеметрия қабылдауын растаңыз. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | RC1 кесу алдында басқару мақұлдаған `iroha_config` дельталарын және кері қайтаруға дайындығын растаңыз. |

Әрбір репетиция бағдарланған ізі бар өндіріске ұқсас топологияда орындалды
құралдар қосылған (`nexus.audit.outcome` телеметрия + Prometheus есептегіштері),
Alertmanager ережелері жүктелді және дәлелдер `docs/examples/` ішіне экспортталды.

## Әдістеме

1. **Телеметриялық жинақ.** Барлық түйіндер құрылымды шығарды
   `nexus.audit.outcome` оқиғасы және ілеспе көрсеткіштер
   (`nexus_audit_outcome_total*`). Көмекші
   `scripts/telemetry/check_nexus_audit_outcome.py` JSON журналын қалдырды,
   оқиға күйін растады және пайдалы жүктемені мұрағаттады
   `docs/examples/nexus_audit_outcomes/`.【скрипттер/телеметрия/check_nexus_audit_outcome.py:1】
2. **Ескертуді тексеру.** `dashboards/alerts/nexus_audit_rules.yml` және оның сынағы
   сымдар ескерту шуының шектерін және пайдалы жүк шаблонын сақтауды қамтамасыз етті
   дәйекті. CI `dashboards/alerts/tests/nexus_audit_rules.test.yml` жұмыс істейді
   әрбір өзгеріс; сол ережелер әр терезеде қолмен орындалды.
3. **Бақылау тақтасын түсіру.** Операторлар маршруттық бақылау тақталарын экспорттады
   `dashboards/grafana/soranet_sn16_handshake.json` (қол алысу денсаулығы) және
   Кезектің күйін аудит нәтижелерімен салыстыру үшін телеметрияға шолу бақылау тақталары.
4. **Шолушының ескертпесі.** Басқару хатшысы рецензенттің инициалдарын тіркеді,
   шешім және [Nexus өтпелі жазбалардағы](./nexus-transition-notes) кез келген жеңілдету билеттері
   және конфигурациялық дельта трекері (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Нәтижелер

| Trace ID | Нәтиже | Дәлелдер | Ескертпелер |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | өту | Ескерту өрт/қалпына келтіру скриншоттары (ішкі сілтеме) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` қайталау; [Nexus өтпелі жазбаларда](./nexus-transition-notes#quarterly-routed-trace-audit-schedule) жазылған телеметриялық айырмашылықтар. | Кезекке кіру P95 612 мс қалды (мақсат ≤750 мс). Бақылау қажет емес. |
| `TRACE-TELEMETRY-BRIDGE` | өту | Мұрағатталған нәтиже пайдалы жүктемесі `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` плюс `status.md` ішінде жазылған OTLP қайта ойнату хэші. | SDK редакциясының тұздары Rust бастапқы деңгейіне сәйкес келді; diff боты нөлдік дельталар туралы хабарлады. |
| `TRACE-CONFIG-DELTA` | Өту (жеңілдету жабық) | Басқару трекерінің жазбасы (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS профилінің манифесті (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + телеметриялық бума манифесті (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | 2-тоқсанды қайта орындау бекітілген TLS профилін хэштендірді және расталған нөлдік кедергілер; телеметрия манифесті 912–936 ұяшық ауқымын және `NEXUS-REH-2026Q2` жұмыс жүктемесінің тұқымын жазады. |

Барлық іздер олардың ішінде кем дегенде бір `nexus.audit.outcome` оқиғасын жасады
Alertmanager қоршауларын қанағаттандыратын терезелер (`NexusAuditOutcomeFailure`
тоқсан бойы жасыл болып қалды).

## Бақылау

- `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` TLS хэшімен жаңартылған маршруттық бақылау қосымшасы;
  жеңілдету `NEXUS-421` өтпелі ескертулерде жабылды.
- Мұрағатқа өңделмеген OTLP қайталауларын және Torii diff артефактілерін тіркеуді жалғастырыңыз.
  Android AND4/AND7 шолулары үшін паритет дәлелдерін күшейту.
- Алдағы `TRACE-MULTILANE-CANARY` жаттығулары бірдей қайталанатынын растаңыз
  телеметрия көмекшісі, сондықтан Q2 тіркелу расталған жұмыс үрдісінен пайда көреді.

## Артефакт индексі

| Актив | Орналасқан жері |
|-------|----------|
| Телеметриялық валидатор | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Ескерту ережелері мен сынақтары | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Үлгі нәтиженің пайдалы жүктемесі | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Дельта трекерді теңшеу | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Бағытталған бақылау кестесі және ескертпелер | [Nexus өтпелі жазбалар](./nexus-transition-notes) |

Бұл есеп, жоғарыдағы артефактілер және ескерту/телеметрия экспорты болуы керек
тоқсан үшін B1 жабу үшін басқару шешімдері журналына қоса беріледі.