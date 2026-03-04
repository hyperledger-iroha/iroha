---
lang: hy
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS metrics & onboarding kit
description: Dashboard, pricing, and automation artifacts referenced by roadmap item SN-8.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SNS չափումներ և ներբեռնման հավաքածու

Ճանապարհային քարտեզի **SN-8** կետը միավորում է երկու խոստում.

1. Հրապարակեք վահանակներ, որոնք բացահայտում են գրանցումները, նորացումները, ARPU-ն, վեճերը և
   սառեցնել պատուհանները `.sora`, `.nexus` և `.dao` համար:
2. Ուղարկեք ներբեռնման փաթեթ, որպեսզի գրանցողներն ու ստյուարդները կարողանան հաղորդագրել DNS, գնագոյացում և
   API-ները հետևողականորեն նախքան որևէ վերջածանցի ակտիվացում:

Այս էջը արտացոլում է սկզբնաղբյուր տարբերակը
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
այնպես որ արտաքին գրախոսները կարող են հետևել նույն ընթացակարգին:

## 1. Մետրային փաթեթ

### Grafana վահանակ և պորտալի ներդրում

- Ներմուծեք `dashboards/grafana/sns_suffix_analytics.json` Grafana (կամ մեկ այլ
  վերլուծական հոսթ) ստանդարտ API-ի միջոցով.

```bash
curl -H "Content-Type: application/json" \
     -H "Authorization: Bearer ${GRAFANA_TOKEN}" \
     -X POST https://grafana.sora.net/api/dashboards/db \
     --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- Նույն JSON-ն ապահովում է այս պորտալի էջի iframe-ը (տես **SNS KPI Dashboard**):
  Ամեն անգամ, երբ հարվածում եք վահանակին, վազեք
  `npm run build && npm run serve-verified-preview` `docs/portal` ներսում մինչև
  հաստատեք և՛ Grafana, և՛ ներկառուցվածի համաժամեցումը:

### Վահանակներ և ապացույցներ

| Վահանակ | Չափիչ | Կառավարման ապացույցներ |
|-------|---------|---------------------|
| Գրանցումներ և նորացումներ | `sns_registrar_status_total` (հաջողություն + նորացման լուծիչի պիտակներ) | Մեկ ածանցի թողունակություն + SLA հետևում: |
| ARPU / զուտ միավորներ | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | Ֆինանսները կարող են համապատասխանեցնել ռեգիստրի մանիֆեստները եկամուտներին: |
| Վեճեր և սառեցումներ | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | Ցույց է տալիս ակտիվ սառեցումները, արբիտրաժային արագությունը և խնամակալի ծանրաբեռնվածությունը: |
| SLA/սխալների տոկոսադրույքներ | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | Կարևորում է API-ի ռեգրեսիաները՝ նախքան դրանք ազդել հաճախորդների վրա: |
| Զանգվածային մանիֆեստի որոնիչ | `sns_bulk_release_manifest_total`, վճարման չափումներ `manifest_id` պիտակներով | Միացնում է CSV կաթիլները հաշվարկային տոմսերին: |

Արտահանել PDF/CSV Grafana-ից (կամ ներկառուցված iframe-ից) ամսական KPI-ի ընթացքում
վերանայել և կցել այն համապատասխան հավելվածի մուտքին,
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Ստյուարդները գրավում են նաև SHA-256-ը
արտահանվող փաթեթի `docs/source/sns/reports/`-ի ներքո (օրինակ,
`steward_scorecard_2026q1.md`), որպեսզի աուդիտները կարողանան կրկնել ապացույցների ուղին:

### Հավելվածի ավտոմատացում

Ստեղծեք հավելվածի ֆայլեր անմիջապես վահանակի արտահանումից, որպեսզի վերանայողները ստանան a
հետևողական մարսողություն.

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

- Օգնականը հեշում է արտահանումը, գրավում է UID/պիտակների/վահանակի քանակը և գրում է
  Markdown հավելվածը `docs/source/sns/reports/.<suffix>/<cycle>.md`-ի ներքո (տես
  `.sora/2026-03` նմուշ, որը կատարվել է այս փաստաթղթի հետ միասին):
- `--dashboard-artifact`-ը պատճենում է արտահանումը
  `artifacts/sns/regulatory/<suffix>/<cycle>/`, ուստի հավելվածը հղում է անում
  կանոնական ապացույցների ուղի; օգտագործեք `--dashboard-label` միայն այն ժամանակ, երբ անհրաժեշտ է մատնացույց անել
  խմբից դուրս արխիվում:
- `--regulatory-entry` միավոր է կառավարող հուշագրում: Օգնականի ներդիրները (կամ
  փոխարինում է) `KPI Dashboard Annex` բլոկ, որը գրանցում է հավելվածի ուղին, վահանակը
  artefact, digest և timestamp, որպեսզի ապացույցները համաժամանակացվեն նորից գործարկումներից հետո:
- `--portal-entry`-ը պահում է Docusaurus պատճենը (`docs/portal/docs/sns/regulatory/*.md`)
  հավասարեցված, որպեսզի վերանայողները ստիպված չլինեն ձեռքով տարբերակել առանձին հավելվածների ամփոփագրերը:
- Եթե բաց եք թողնում `--regulatory-entry`/`--portal-entry`, կցեք ստեղծված ֆայլը
  հուշագրերը ձեռքով և դեռ վերբեռնում են Grafana-ից նկարահանված PDF/CSV նկարները:
- Պարբերական արտահանումների համար նշեք վերջածանց/ցիկլի զույգերը
  `docs/source/sns/regulatory/annex_jobs.json` և գործարկել
  `python3 scripts/run_sns_annex_jobs.py --verbose`. Օգնականը քայլում է ամեն մուտք,
  պատճենում է վահանակի արտահանումը (կանխադրված՝ `dashboards/grafana/sns_suffix_analytics.json`
  երբ չճշտված է), և թարմացնում է հավելվածի բլոկը յուրաքանչյուր կարգավորող մարմնի ներսում (և,
  երբ առկա է, պորտալ) հուշագիր մեկ անցումով:
- Գործարկեք `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (կամ `make check-sns-annex`)՝ ապացուցելու համար, որ աշխատատեղերի ցուցակը մնում է տեսակավորված/չեղյալ, յուրաքանչյուր հուշագիր կրում է համապատասխան `sns-annex` նշիչը, և հավելվածի կոճակը գոյություն ունի: Օգնականը գրում է `artifacts/sns/annex_schedule_summary.json`-ը կառավարման փաթեթներում օգտագործվող տեղային/հեշ ամփոփագրերի կողքին:
Սա հեռացնում է ձեռքով պատճենահանման/տեղադրման քայլերը և պահպանում է SN-8 հավելվածի ապացույցները, մինչդեռ
պահպանության ժամանակացույցը, նշիչը և տեղայնացման շեղումը CI-ում:

## 2. Ներբեռնման հավաքածուի բաղադրիչներ

### վերջածանցային լարեր

- Ռեեստրի սխեման + ընտրիչի կանոններ.
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  և [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md):
- DNS կմախքի օգնական.
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  հետ փորձնական հոսքի գրավել է
  [gateway/DNS runbook] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md):
- Յուրաքանչյուր ռեգիստրատորի գործարկման համար ներկայացրեք կարճ նշում տակ
  `docs/source/sns/reports/` ամփոփում է ընտրիչի նմուշները, GAR ապացույցները և DNS հեշերը:

### Գնային խաբեբա

| Պիտակի երկարությունը | Բազային վճար (USD համարժեք) |
|--------------|---------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6–9 | $12 |
| 10+ | $8 |

Վերջածանցի գործակիցները՝ `.sora` = 1,0×, `.nexus` = 0,8×, `.dao` = 1,3×:  
Ժամկետային բազմապատկիչներ՝ 2 տարի −5%, 5 տարի −12%; շնորհքի պատուհան = 30 օր, մարում
= 60 օր (20% վճար, նվազագույնը $5, առավելագույնը $200): Արձանագրեք բանակցված շեղումները
գրանցման տոմս.

### Պրեմիում աճուրդներ ընդդեմ նորացումների

1. **Պրեմիում լողավազան** — կնքված հայտի հանձնում/բացահայտում (SN-3): Հետևեք հայտերին
   `sns_premium_commit_total` և հրապարակեք մանիֆեստը տակ
   `docs/source/sns/reports/`.
2. **Հոլանդերենը վերաբացվի** — շնորհի + մարման ժամկետը լրանալուց հետո սկսեք 7-օրյա հոլանդական վաճառք
   10×-ով, որը քայքայվում է օրական 15%-ով: Պիտակը դրսևորվում է `manifest_id`-ով, այնպես որ
   վահանակը կարող է առաջընթաց առաջացնել:
3. **Նորացումներ** — մոնիտոր `sns_registrar_status_total{resolver="renewal"}` եւ
   գրավել ինքնաթարմացման ստուգաթերթը (ծանուցումներ, SLA, հետադարձ վճարման ռելսեր)
   գրանցման տոմսի ներսում:

### Մշակողների API-ներ և ավտոմատացում

- API պայմանագրեր՝ [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md):
- Զանգվածային օգնական և CSV սխեմա.
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md):
- Օրինակ հրաման.

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --ndjson artifacts/sns/releases/2026q2/requests.ndjson \
  --submission-log artifacts/sns/releases/2026q2/submissions.log \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token
```

Ներառեք մանիֆեստի ID-ն (`--submission-log` ելք) KPI վահանակի ֆիլտրում
այնպես որ ֆինանսները կարող են հաշտեցնել եկամուտների վահանակները յուրաքանչյուր թողարկման համար:

### Ապացույցների փաթեթ

1. Գրանցման տոմս կոնտակտներով, վերջածանցների շրջանակով և վճարման ռելսերով:
2. DNS/լուծիչ ապացույցներ (zonefile skeletons + GAR proofs):
3. Գնագոյացման աշխատաթերթ + կառավարման կողմից հաստատված ցանկացած անտեսում:
4. API/CLI ծխի փորձարկման արտեֆակտներ (`curl` նմուշներ, CLI տառադարձումներ):
5. KPI վահանակի սքրինշոթ + CSV արտահանում, կցված ամսական հավելվածին:

## 3. Գործարկել ստուգաթերթը

| Քայլ | Սեփականատեր | Արտեֆակտ |
|------|-------|----------|
| Վահանակ ներմուծված | Ապրանքի վերլուծություն | Grafana API պատասխան + վահանակի UID |
| Պորտալի ներդրումը վավերացված է | Փաստաթղթեր/DevRel | `npm run build` տեղեկամատյաններ + նախադիտման սքրինշոթ |
| DNS-ի փորձն ավարտված է | Ցանցային/Օպերատիվ | `sns_zonefile_skeleton.py` ելքեր + runbook մատյան |
| Գրանցման ավտոմատացման չոր գործարկում | Գրանցող Eng | `sns_bulk_onboard.py` ներկայացումների մատյան |
| Ներկայացվել են կառավարման ապացույցներ | Կառավարման խորհուրդ | Հավելվածի հղում + SHA-256 արտահանվող վահանակի |

Լրացրեք ստուգաթերթը նախքան գրանցող կամ վերջածանց ակտիվացնելը: Ստորագրված
փաթեթը մաքրում է SN-8 ճանապարհային քարտեզի դարպասը և աուդիտորներին տալիս է մեկ հղում, երբ
շուկայական գործարկումների վերանայում: