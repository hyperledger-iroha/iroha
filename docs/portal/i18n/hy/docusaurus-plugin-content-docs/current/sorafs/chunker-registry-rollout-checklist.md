---
id: chunker-registry-rollout-checklist
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Registry Rollout Checklist
sidebar_label: Chunker Rollout Checklist
description: Step-by-step rollout plan for chunker registry updates.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Կանոնական աղբյուր
:::

# SoraFS Ռեեստրի տարածման ստուգաթերթ

Այս ստուգաթերթը ներառում է այն քայլերը, որոնք անհրաժեշտ են նոր chunker պրոֆիլը կամ
մատակարարի ընդունելության փաթեթը վերանայումից մինչև արտադրություն կառավարման ավարտից հետո
կանոնադրությունը վավերացվել է։

> **Շրջանակ.** Կիրառվում է բոլոր թողարկումներին, որոնք փոփոխում են
> `sorafs_manifest::chunker_registry`, մատակարարի ընդունելության ծրարներ կամ
> կանոնական հարմարանքների փաթեթներ (`fixtures/sorafs_chunker/*`):

## 1. Թռիչքից առաջ վավերացում

1. Վերականգնել հարմարանքները և ստուգել դետերմինիզմը.
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Հաստատեք դետերմինիզմի հեշերը
   `docs/source/sorafs/reports/sf1_determinism.md` (կամ համապատասխան պրոֆիլ
   հաշվետվություն) համապատասխանեցնել վերականգնված արտեֆակտներին:
3. Համոզվեք, որ `sorafs_manifest::chunker_registry`-ը կազմում է
   `ensure_charter_compliance()`՝ գործարկելով.
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Թարմացնել առաջարկի դոսյեն.
   - `docs/source/sorafs/proposals/<profile>.json`
   - Խորհրդի արձանագրությունների մուտքագրում `docs/source/sorafs/council_minutes_*.md` տակ
   - Դետերմինիզմի զեկույց

## 2. Կառավարման գրանցում

1. Ներկայացրեք Tooling աշխատանքային խմբի զեկույցը և առաջարկի ամփոփումը Sora-ին
   Խորհրդարանի ենթակառուցվածքների հանձնաժողով.
2. Գրանցեք հաստատման մանրամասները
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Հրապարակել Խորհրդարանի կողմից ստորագրված ծրարը պիտույքների հետ մեկտեղ.
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Ստուգեք, որ ծրարը հասանելի է կառավարման բեռնման օգնականի միջոցով.
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Բեմականացում

Տե՛ս [բեմականացման մանիֆեստի գրքույկը] (./staging-manifest-playbook)
այս քայլերի մանրամասն նկարագրությունը:

1. Տեղադրեք Torii՝ `torii.sorafs` հայտնաբերումը միացված է և մուտքը
   հարկադրումը միացված է (`enforce_admission = true`):
2. Հրել հաստատված մատակարարի ընդունելության ծրարները բեմականացման գրանցամատյան
   գրացուցակը, որը վկայակոչված է `torii.sorafs.discovery.admission.envelopes_dir`-ի կողմից:
3. Ստուգեք, որ մատակարարի գովազդները տարածվում են հայտնաբերման API-ի միջոցով.
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Կիրառեք մանիֆեստի/պլանի վերջնակետերը կառավարման վերնագրերով.
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Հաստատեք հեռաչափության վահանակները (`torii_sorafs_*`) և ահազանգման կանոնները հայտնում են
   նոր պրոֆիլ առանց սխալների:

## 4. Արտադրության տարածում

1. Կրկնեք փուլային քայլերը արտադրության Torii հանգույցների դեմ:
2. Հայտարարեք ակտիվացման պատուհանի մասին (ամսաթիվ/ժամ, արտոնյալ ժամանակաշրջան, վերադարձի պլան):
   օպերատոր և SDK ալիքներ:
3. Միավորել թողարկման PR-ը, որը պարունակում է.
   - Թարմացված հարմարանքները և ծրարը
   - Փաստաթղթերի փոփոխություններ (կանոնադրության հղումներ, դետերմինիզմի զեկույց)
   - Ճանապարհային քարտեզի/կարգավիճակի թարմացում
4. Նշեք թողարկումը և արխիվացրեք ստորագրված արտեֆակտները ծագման համար:

## 5. Հետ-տեղադրման աուդիտ

1. Ձեռք բերեք վերջնական չափումները (բացահայտումների քանակը, հաջողության մակարդակը, սխալը
   հիստոգրամներ) թողարկումից 24 ժամ հետո:
2. Թարմացրեք `status.md`-ը՝ կարճ ամփոփումով և հղումով դետերմինիզմի զեկույցին:
3. Ներկայացրեք ցանկացած հետագա առաջադրանք (օրինակ՝ պրոֆիլի հեղինակային լրացուցիչ ուղեցույց):
   `roadmap.md`.