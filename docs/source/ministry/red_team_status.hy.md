---
lang: hy
direction: ltr
source: docs/source/ministry/red_team_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f4a2e50e18749f64212dee5186bfd3a3d034ac906d1a506d420cdcb1b5117517
source_last_modified: "2025-12-29T18:16:35.979926+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team Status (MINFO-9)
summary: Snapshot of the chaos drill program covering upcoming runs, last completed scenario, and remediation items.
translator: machine-google-reviewed
---

# նախարարության Կարմիր թիմային կարգավիճակ

Այս էջը լրացնում է [Moderation Red-Team Plan] (moderation_red_team_plan.md)
Հետևելով մոտաժամկետ վարժանքների օրացույցին, ապացույցների փաթեթներին և վերականգնմանը
կարգավիճակը։ Թարմացրեք այն յուրաքանչյուր վազքից հետո՝ նկարահանված արտեֆակտների կողքին
`artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`.

## Առաջիկա վարժանքներ

| Ամսաթիվ (UTC) | Սցենար | Սեփականատեր(ներ) | Ապացույցների նախապատրաստում | Ծանոթագրություններ |
|------------|---------|---------|---------------|-------|
| 2026-11-12 | **Operation Blindfold** — Taikai խառը ռեժիմով մաքսանենգության փորձեր` դարպասների վարկանիշի իջեցման փորձերով | Անվտանգության ճարտարագիտություն (Miyu Sato), նախարարության օպերացիա (Liam O'Connor) | `scripts/ministry/scaffold_red_team_drill.py` փաթեթ `docs/source/ministry/reports/red_team/2026-11-operation-blindfold.md` + բեմականացման գրացուցակ `artifacts/ministry/red-team/2026-11/operation-blindfold/` | Զորավարժություններ GAR/Taikai համընկնում և DNS ձախողում; պահանջում է denylist Merkle-ի լուսանկարը մեկնարկից առաջ և `export_red_team_evidence.py` գործարկումը վահանակների նկարահանումից հետո: |

## Վերջին պարապմունքի նկարը

| Ամսաթիվ (UTC) | Սցենար | Ապացույցների փաթեթ | Վերականգնում և Հետագայում |
|------------|---------|---------------|-------------------------|
| 2026-08-18 | **Operation SeaGlass** — Դարպասների մաքսանենգություն, կառավարման վերարտադրում և նախազգուշացման փորձեր | `artifacts/ministry/red-team/2026-08/operation-seaglass/` (Grafana արտահանումներ, Alertmanager տեղեկամատյաններ, `seaglass_evidence_manifest.json`) | **Բաց.** կրկնակի կնիքի ավտոմատացում (`MINFO-RT-17`, սեփականատեր. Governance Ops, ժամկետանց 2026-09-05); փին վահանակի սառեցում SoraFS-ին (`MINFO-RT-18`, դիտարկելիություն, 2026-08-25): **Փակ է.** գրանցամատյանի ձևանմուշը թարմացվել է Norito մանիֆեստի հեշերով: |

## Հետևում և գործիքավորում

- Օգտագործեք `scripts/ministry/moderation_payload_tool.py` ներարկային փաթեթավորման համար
  ծանրաբեռնվածություն և ժխտման կարկատաններ յուրաքանչյուր սցենարի համար:
- Գրանցեք վահանակի/տեղեկամատյանների նկարները `scripts/ministry/export_red_team_evidence.py`-ի միջոցով
  յուրաքանչյուր վարժումից անմիջապես հետո, որպեսզի ապացույցների մանիֆեստը պարունակի ստորագրված հեշեր:
- CI պահակ `ci/check_ministry_red_team.sh`-ը պարտադրում է կատարված վարժության հաշվետվությունները
  չեն պարունակում տեղապահի տեքստ, և որ նշված արտեֆակտները նախկինում գոյություն ունեն
  միաձուլում.

Տես՝ `status.md` (§ *Նախարարության կարմիր թիմի կարգավիճակ*) ուղիղ եթերում հղումով
շաբաթական կոորդինացիոն զանգերում։