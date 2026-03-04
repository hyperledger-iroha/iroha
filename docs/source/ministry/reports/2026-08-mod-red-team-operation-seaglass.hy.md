---
lang: hy
direction: ltr
source: docs/source/ministry/reports/2026-08-mod-red-team-operation-seaglass.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 64cd1112df2f1fc95571ee4ed269e64bde6bf73bd94b19bbf0eaa80a5b43c219
source_last_modified: "2025-12-29T18:16:35.981105+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill — Operation SeaGlass
summary: Evidence and remediation log for the Operation SeaGlass moderation drill (gateway smuggling, governance replay, alert brownout).
translator: machine-google-reviewed
---

# Red-Team Drill — Operation SeaGlass

- **Գայլիքի ID՝** `20260818-operation-seaglass`
- **Ամսաթիվ և պատուհան:** `2026-08-18 09:00Z – 11:00Z`
- **Սցենարի դաս.** `smuggling`
- **Օպերատորներ:** `Miyu Sato, Liam O'Connor`
- **Վահանակները սառեցված են պարտավորությունից՝ ** `364f9573b`
- **Ապացույցների փաթեթ՝** `artifacts/ministry/red-team/2026-08/operation-seaglass/`
- **SoraFS CID (ըստ ցանկության):** `not pinned (local bundle only)`
- **Ճանապարհային քարտեզի առնչվող տարրեր.

## 1. Նպատակներ և մուտքի պայմաններ

- ** Առաջնային նպատակները **
  - Վավերացրեք հերքման TTL-ի կիրառումը և մուտքի կարանտինը մաքսանենգության փորձի ժամանակ՝ բեռի հեռացման ազդանշանների ժամանակ:
  - Հաստատեք կառավարման վերարտադրման հայտնաբերումը և զգույշ եղեք խափանումների բեռնաթափումը չափավորության մատյանում:
- **Նախադրյալները հաստատված են**
  - `emergency_canon_policy.md` տարբերակ `v2026-08-seaglass`:
  - `dashboards/grafana/ministry_moderation_overview.json` մարսել `sha256:ef5210b5b08d219242119ec4ceb61cb68ee4e42ce2eea8a67991fbff95501cc8`:
  - Չեղյալ համարել լիազորությունները՝ `Kenji Ito (GovOps pager)`:

## 2. Կատարման ժամանակացույց

| Ժամացույց (UTC) | Դերասան | Գործողություն / Հրաման | Արդյունք / Ծանոթագրություններ |
|--------------------------|-----------------|----------------|
| 09:00:12 | Միյու Սատո | Սառեցված վահանակներ/զգուշացումներ `364f9573b`-ում `scripts/ministry/export_red_team_evidence.py --freeze-only`-ի միջոցով | Ելակետային գիծը գրավված և պահվում է `dashboards/` |
| 09:07:44 | Լիամ Օ'Քոնոր | Հրապարակված ժխտողական նկար + GAR-ի փոխարինում `sorafs_cli ... gateway update-denylist --policy-tier emergency`-ով բեմադրելու համար | Պատկերն ընդունված է; վերագրանցման պատուհանը գրանցված է Alertmanager |
| 09:17:03 | Միյու Սատո | Ներարկված մաքսանենգության օգտակար բեռ + կառավարման կրկնություն՝ օգտագործելով `moderation_payload_tool.py --scenario seaglass` | Ահազանգը հնչել է 3 մ12 վրկ-ից հետո; կառավարման վերարտադրումը դրոշակված է |
| 09:31:47 | Լիամ Օ'Քոնոր | Գործարկվեց ապացույցների արտահանում և կնքված մանիֆեստ `seaglass_evidence_manifest.json` | Ապացույցների փաթեթ և հեշեր, որոնք պահվում են `manifests/` |

## 3. Դիտարկումներ և չափումներ

| Մետրական | Թիրախային | Դիտարկված | Անցնել/ձախողել | Ծանոթագրություններ |
|--------|--------|----------|----------|-------|
| Զգուշացման պատասխանի ուշացում | = 0,98 | 0,992 | ✅ | Հայտնաբերվել է և՛ մաքսանենգություն, և՛ վերարտադրող բեռնատարներ |
| Դարպասի անոմալիաների հայտնաբերում | Ահազանգված է | Ահազանգված է + ավտոմատ կարանտին | ✅ | Կարանտինը կիրառվել է մինչև կրկնակի բյուջեի սպառումը |

- `Grafana export:` `artifacts/ministry/red-team/2026-08/operation-seaglass/dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/ministry/red-team/2026-08/operation-seaglass/alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `artifacts/ministry/red-team/2026-08/operation-seaglass/manifests/seaglass_evidence_manifest.json`

## 4. Գտածոներ և վերականգնում

| Խստություն | Գտնելով | Սեփականատեր | Թիրախային ամսաթիվ | Կարգավիճակ / Հղում |
|----------|---------|-------|-------------|--------------|
| Բարձր | Կառավարման վերարտադրման ահազանգը գործարկվել է, բայց SoraFS կնքումը հետաձգվել է 2 մ-ով, երբ սպասողների ցուցակի ձախողումը գործարկվել է | Governance Ops (Լիամ Օ'Քոնոր) | 2026-09-05 | `MINFO-RT-17` բաց — ավելացրեք վերարտադրման կնիքի ավտոմատացում ձախողման ճանապարհին |
| Միջին | Վահանակի սառեցումը ամրացված չէ SoraFS-ին; օպերատորները հենվել են տեղական փաթեթի | Դիտորդականություն (Miyu Sato) | 2026-08-25 | `MINFO-RT-18` բաց — կապում `dashboards/*` դեպի SoraFS՝ ստորագրված CID-ով հաջորդ փորվածքից առաջ |
| Ցածր | CLI մատյանում բաց թողնված Norito մանիֆեստի հեշ առաջին անցումով | Նախարարության օպերացիա (Kenji Ito) | 2026-08-22 | Հորատման ընթացքում ամրագրված; ձևանմուշը թարմացվել է գրանցամատյանում |Փաստաթղթավորեք, թե ինչպես է դրսևորվում չափաբերումը, մերժման կանոնները կամ SDK/գործիքները պետք է փոխվեն: Կցեք GitHub/Jira-ի խնդիրներին և նշեք արգելափակված/ապարգելափակված վիճակները:

## 5. Կառավարում և հաստատումներ

- **Միջադեպի հրամանատարի գրանցում.** `Miyu Sato @ 2026-08-18T11:22Z`
- **Կառավարման խորհրդի վերանայման ամսաթիվը.** `GovOps-2026-08-22`
- **Հետևող ստուգաթերթ.** `[x] status.md updated`, `[x] roadmap row updated`, `[x] transparency packet annotated`

## 6. Կցորդներ

- `[x] CLI logbook (logs/operation_seaglass.log)`
- `[x] Dashboard JSON export`
- `[x] Alertmanager history`
- `[x] SoraFS manifest / CAR`
- `[ ] Override audit log`

Նշեք յուրաքանչյուր կցորդ `[x]`-ով, որը վերբեռնվել է ապացույցների փաթեթում և SoraFS պատկերով:

---

_Վերջին թարմացում՝ 2026-08-18_