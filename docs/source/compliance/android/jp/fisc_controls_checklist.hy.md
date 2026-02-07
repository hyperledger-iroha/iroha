---
lang: hy
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2025-12-29T18:16:35.928660+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FISC Security Controls Checklist — Android SDK

| Դաշտային | Արժեք |
|-------|-------|
| Տարբերակ | 0.1 (2026-02-12) |
| Շրջանակ | Android SDK + օպերատորի գործիքավորում, որն օգտագործվում է ճապոնական ֆինանսական տեղակայման մեջ |
| Սեփականատերեր | Համապատասխանություն և իրավական (Daniel Park), Android ծրագրի առաջատար |

## Control Matrix

| FISC Control | Իրականացման մանրամասն | Ապացույցներ / Հղումներ | Կարգավիճակը |
|--------------|----------------------|---------------------|--------|
| **Համակարգի կազմաձևման ամբողջականություն** | `ClientConfig`-ը պարտադրում է մանիֆեստների հեշինգը, սխեմայի վավերացումը և միայն կարդալու գործարկման ժամանակի հասանելիությունը: Կազմաձևման վերաբեռնման ձախողումները թողարկում են `android.telemetry.config.reload` իրադարձություններ, որոնք փաստագրված են runbook-ում: | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ Իրականացվել է |
| **Մուտքի վերահսկում և նույնականացում** | SDK-ն հարգում է Torii TLS քաղաքականությունը և `/v1/pipeline` ստորագրված հարցումները. օպերատորի աշխատանքային հոսքերի տեղեկանք Աջակցման խաղագիրք §4–5՝ ընդլայնման և շրջադարձային մուտքի համար ստորագրված Norito արտեֆակտների միջոցով: | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (շրջանցել աշխատանքային հոսքը): | ✅ Իրականացվել է |
| **Գաղտնագրված բանալիների կառավարում** | StrongBox-ի նախընտրած մատակարարները, ատեստավորման վավերացումը և սարքի մատրիցայի ծածկույթը ապահովում են KMS-ի համապատասխանությունը: Ատեստավորման զրահի ելքերը արխիվացված են `artifacts/android/attestation/` տակ և հետևվում են պատրաստության մատրիցայում: | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ Իրականացվել է |
| **Գրանցում, մոնիտորինգ և պահպանում** | Հեռուստաչափության խմբագրման քաղաքականությունը հաշշում է զգայուն տվյալները, ամրացնում է սարքի հատկանիշները և պարտադրում պահպանումը (7/30/90/365-օրյա պատուհաններ): Աջակցություն Playbook §8-ը նկարագրում է վահանակի շեմերը. `telemetry_override_log.md`-ում գրանցված անտեսումներ: | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ Իրականացվել է |
| **Գործառնությունների և փոփոխությունների կառավարում** | GA-ի անջատման ընթացակարգը (Աջակցող Playbook §7.2) գումարած `status.md` թարմացումները հետևում են թողարկման պատրաստությանը: Թողարկման ապացույցներ (SBOM, Sigstore փաթեթներ)՝ կապված `docs/source/compliance/android/eu/sbom_attestation.md`-ի միջոցով: | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ Իրականացվել է |
| **Դեպքի արձագանքը և հաղորդումը** | Playbook-ը սահմանում է խստության մատրիցան, SLA արձագանքման պատուհաններ և համապատասխանության ծանուցման քայլեր. Հեռաչափության հաղթահարում + քաոսի փորձերն ապահովում են վերարտադրելիություն օդաչուների առաջ: | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ Իրականացվել է |
| **Տվյալների ռեզիդենտություն/տեղայնացում** | Հեռուստաչափական կոլեկտորները JP-ի տեղակայման համար աշխատում են հաստատված Տոկիոյի տարածաշրջանում; StrongBox ատեստավորման փաթեթներ, որոնք պահվում են տարածաշրջանում և հղում են կատարում գործընկեր տոմսերից: Տեղայնացման պլանն ապահովում է, որ փաստաթղթերը հասանելի են ճապոներեն նախքան բետա (AND5): | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 Ընթացքի մեջ է (տեղայնացումը շարունակվում է) |

## Գրախոսի նշումներ

- Ստուգեք սարքի մատրիցային գրառումները Galaxy S23/S24-ի համար՝ նախքան կարգավորվող գործընկերոջ մուտքը (տես `s23-strongbox-a`, `s24-strongbox-a` պատրաստականության փաստաթղթերի տողերը):
- Համոզվեք, որ JP տեղակայումներում հեռաչափական կոլեկտորները կիրառում են DPIA-ում (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`) սահմանված նույն պահման/վերացման տրամաբանությունը:
- Ստացեք հաստատում արտաքին աուդիտորներից, երբ բանկային գործընկերները վերանայեն այս ստուգաթերթը: