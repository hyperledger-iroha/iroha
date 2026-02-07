---
id: nexus-routed-trace-audit-2026q1
lang: hy
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: 2026 Q1 routed-trace audit report (B1)
description: Mirror of `docs/source/nexus_routed_trace_audit_report_2026q1.md`, covering the quarterly telemetry rehearsal outcomes.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note Կանոնական աղբյուր
Այս էջը արտացոլում է `docs/source/nexus_routed_trace_audit_report_2026q1.md`: Պահպանեք երկու օրինակները հավասարեցված, մինչև մնացած թարգմանությունները վայրէջք կատարեն:
:::

# 2026 եռամսյակի 1-ին երթուղային հետքի աուդիտի հաշվետվություն (B1)

Ճանապարհային քարտեզի կետ **B1 — Ուղղորդված հետագծային աուդիտներ և հեռաչափության բազային** պահանջում է.
Nexus routed-trace ծրագրի եռամսյակային վերանայում: Այս զեկույցը փաստում է
Q12026 աուդիտի պատուհան (հունվար-մարտ), որպեսզի կառավարման խորհուրդը կարողանա ստորագրել այն
Հեռաչափության կեցվածքը Q2-ի մեկնարկի փորձերից առաջ:

## Շրջանակ և ժամանակացույց

| Հետքի ID | Պատուհան (UTC) | Նպատակային |
|----------|-------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | Ստուգեք գոտի մուտքի հիստոգրամները, հերթերի բամբասանքները և ահազանգերի հոսքը մինչև բազմագոտի միացումը: |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | Վավերացրեք OTLP-ի վերարտադրումը, տարբերվող բոտերի հավասարությունը և SDK-ի հեռաչափության ընդունումը AND4/AND7 հանգուցային կետերից առաջ: |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | Հաստատեք կառավարման կողմից հաստատված `iroha_config` դելտաները և հետ վերադարձի պատրաստակամությունը մինչև RC1 կտրվածքը: |

Յուրաքանչյուր փորձն անցավ արտադրության նման տոպոլոգիայի վրա՝ ուղղորդված հետքով
գործիքավորումը միացված է (`nexus.audit.outcome` հեռաչափություն + Prometheus հաշվիչներ),
Alertmanager-ի կանոնները բեռնված են, և ապացույցներն արտահանվել են `docs/examples/`:

## Մեթոդաբանություն

1. **Հեռաչափության հավաքածու։** Բոլոր հանգույցներն արտանետում էին կառուցվածքային
   `nexus.audit.outcome` իրադարձություն և ուղեկցող չափումներ
   (`nexus_audit_outcome_total*`): Օգնականը
   `scripts/telemetry/check_nexus_audit_outcome.py`-ը հետևել է JSON տեղեկամատյանը,
   վավերացրել է իրադարձության կարգավիճակը և արխիվացրել է օգտակար բեռը տակ
   `docs/examples/nexus_audit_outcomes/`.【scripts/telemetry/check_nexus_audit_outcome.py:1】
2. **Զգուշացման վավերացում։** `dashboards/alerts/nexus_audit_rules.yml` և դրա փորձարկումը
   զրահը ապահովեց զգոն աղմուկի շեմերը և օգտակար բեռնվածքի ձևանմուշները պահպանվեցին
   հետեւողական. CI-ն աշխատում է `dashboards/alerts/tests/nexus_audit_rules.test.yml`-ով
   յուրաքանչյուր փոփոխություն; նույն կանոնները ձեռքով գործադրվեցին յուրաքանչյուր պատուհանի ժամանակ:
3. **Վահանակի ֆիքսում։** Օպերատորներն արտահանել են երթուղավորված հետքի վահանակները
   `dashboards/grafana/soranet_sn16_handshake.json` (ձեռքսեղմման առողջություն) և
   Հեռաչափության ակնարկի վահանակներ՝ հերթի առողջությունը աուդիտի արդյունքների հետ փոխկապակցելու համար:
4. **Գրախոսի նշումներ.** Կառավարության քարտուղարը գրանցել է վերանայողի սկզբնատառերը,
   որոշումը և ցանկացած մեղմացման տոմս [Nexus անցումային նշումներում] (./nexus-transition-notes)
   և կոնֆիգուրացիայի դելտա թրեյքեր (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`):

## Գտածոներ

| Հետքի ID | Արդյունք | Ապացույցներ | Ծանոթագրություններ |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Անցում | Զգուշացում հրդեհի/վերականգնման սքրինշոթներ (ներքին հղում) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` վերարտադրում; Հեռաչափության տարբերությունները գրանցված են [Nexus անցումային նշումներում] (./nexus-transition-notes#quarterly-routed-trace-audit-schedule): | Հերթ ընդունելության P95-ը մնաց 612ms (նպատակը ≤750ms): Հետևողականություն չի պահանջվում: |
| `TRACE-TELEMETRY-BRIDGE` | Անցում | Արխիվացված արդյունքի օգտակար բեռնվածություն `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` գումարած OTLP վերարտադրման հեշը՝ գրանցված `status.md`-ում: | SDK-ի խմբագրման աղերը համապատասխանում էին Rust-ի ելակետին; diff bot-ը հաղորդում է զրոյական դելտաների մասին: |
| `TRACE-CONFIG-DELTA` | Անցում (փակված մեղմացում) | Կառավարման հետագծման մուտքագրում (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS պրոֆիլի մանիֆեստ (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + հեռաչափության փաթեթի մանիֆեստ (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`): | Q2-ի կրկնությունը հեշեց հաստատված TLS պրոֆիլը և հաստատեց զրոյական ստրագլերներ; հեռաչափության մանիֆեստը գրանցում է 912–936 միջակայքը և ծանրաբեռնվածության սերմը `NEXUS-REH-2026Q2`: |

Բոլոր հետքերը արտադրեցին առնվազն մեկ `nexus.audit.outcome` իրադարձություն իրենց շրջանակներում
պատուհաններ, որոնք բավարարում են Alertmanager-ի պահակակետերը (`NexusAuditOutcomeFailure`
մնաց կանաչ եռամսյակի համար):

## Հետևում

- Routed-trace հավելվածը թարմացվել է TLS hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`-ով;
  մեղմացում `NEXUS-421` փակվել է անցումային նշումներում:
- Շարունակեք կցել չմշակված OTLP կրկնօրինակները և Torii տարբեր արտեֆակտները արխիվում
  ամրապնդել հավասարության ապացույցները Android AND4/AND7 ակնարկների համար:
- Հաստատեք, որ առաջիկա `TRACE-MULTILANE-CANARY` փորձերը նույնը նորից կօգտագործեն
  Հեռաչափության օգնական, որպեսզի Q2-ի գրանցումը օգտվի վավերացված աշխատանքային հոսքից:

## Արտեֆակտի ինդեքս

| Ակտիվ | Գտնվելու վայրը |
|-------|-----------|
| Հեռաչափական վավերացնող | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Զգուշացման կանոններ և թեստեր | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Նմուշի արդյունքի ծանրաբեռնվածություն | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Config delta tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Ուղղորդված հետքի ժամանակացույց և նշումներ | [Nexus անցումային նշումներ](./nexus-transition-notes) |

Այս զեկույցը, վերը նշված արտեֆակտները և ազդանշանների/հեռաչափության արտահանումները պետք է լինեն
կցված է կառավարման որոշումների մատյանին՝ եռամսյակի համար B1-ը փակելու համար: