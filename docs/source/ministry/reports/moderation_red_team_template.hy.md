---
lang: hy
direction: ltr
source: docs/source/ministry/reports/moderation_red_team_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22bfdf5696bf3a58e7899e7d7b2ba77e404a05fa81304f12d6c78eeb1e8035e5
source_last_modified: "2025-12-29T18:16:35.982646+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill Report Template
summary: Copy this file for every MINFO-9 drill to capture metadata, evidence, and remediation actions.
translator: machine-google-reviewed
---

> **Ինչպե՞ս օգտագործել.** կրկնօրինակեք այս ձևանմուշը `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md`-ին յուրաքանչյուր փորվածքից անմիջապես հետո: Պահեք ֆայլերի անունները փոքրատառով, գծիկներով և համահունչ Alertmanager-ում գրանցված հորատման ID-ի հետ:

# Red-Team Drill Report — `<SCENARIO NAME>`

- **Գայլիքի ID՝** `<YYYYMMDD>-<scenario>`
- **Ամսաթիվ և պատուհան:** `<YYYY-MM-DD HH:MMZ – HH:MMZ>`
- **Սցենարի դաս.** `smuggling | bribery | gateway | ...`
- **Օպերատորներ.** `<names / handles>`
- **Վահանակները սառեցված են պարտավորությունից.** `<git SHA>`
- **Ապացույցների փաթեթ՝** `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`
- **SoraFS CID (ըստ ցանկության):** `<cid>`  
- **Ճանապարհային քարտեզի առնչվող տարրեր.** `MINFO-9`, գումարած ցանկացած կապված տոմսեր:

## 1. Նպատակներ և մուտքի պայմաններ

- ** Առաջնային նպատակները **
  - `<e.g. Verify denylist TTL enforcement under smuggling attack>`
- **Նախադրյալները հաստատված են**
  - `emergency_canon_policy.md` տարբերակ `<tag>`
  - `dashboards/grafana/ministry_moderation_overview.json` մարսողություն `<sha256>`
  - Չեղյալ համարել զորավարժությունները՝ `<name>`

## 2. Կատարման ժամանակացույց

| Ժամացույց (UTC) | Դերասան | Գործողություն / Հրաման | Արդյունք / Ծանոթագրություններ |
|--------------------------|-----------------|----------------|
|  |  |  |  |

> Ներառեք Torii հարցումների ID-ներ, բեկորային հեշեր, անտեսման հաստատումներ և Alertmanager հղումներ:

## 3. Դիտարկումներ և չափումներ

| Մետրական | Թիրախային | Դիտարկված | Անցնել/ձախողել | Ծանոթագրություններ |
|--------|--------|----------|----------|-------|
| Զգուշացման արձագանքման ուշացում | `<X> min` | `<Y> min` | ✅/⚠️ |  |
| Չափավորության հայտնաբերման արագություն | `>= <value>` |  |  |  |
| Դարպասի անոմալիաների հայտնաբերում | `Alert fired` |  |  |  |

- `Grafana export:` `artifacts/.../dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/.../alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `<path>`

## 4. Գտածոներ և վերականգնում

| Խստություն | Գտնելով | Սեփականատեր | Թիրախային ամսաթիվ | Կարգավիճակ / Հղում |
|----------|---------|-------|-------------|--------------|
| Բարձր |  |  |  |  |

Փաստաթղթավորեք, թե ինչպես է դրսևորվում չափաբերումը, մերժման կանոնները կամ SDK/գործիքները պետք է փոխվեն: Կցեք GitHub/Jira-ի խնդիրներին և նշեք արգելափակված/ապարգելափակված վիճակները:

## 5. Կառավարում և հաստատումներ

- **Միջադեպի հրամանատարի գրանցում.** `<name / timestamp>`
- **Կառավարման խորհրդի վերանայման ամսաթիվը.** `<meeting id>`
- **Հետևող ստուգաթերթ.** `[ ] status.md updated`, `[ ] roadmap row updated`, `[ ] transparency packet annotated`

## 6. Կցորդներ

- `[ ] CLI logbook (`logs/.md`)`
- `[ ] Dashboard JSON export`
- `[ ] Alertmanager history`
- `[ ] SoraFS manifest / CAR`
- `[ ] Override audit log`

Նշեք յուրաքանչյուր հավելված `[x]`-ով, որը վերբեռնվել է ապացույցների փաթեթում և SoraFS պատկերով:

---

_Վերջին թարմացում՝ {{ ամսաթիվ | լռելյայն («2026-02-20») }}_