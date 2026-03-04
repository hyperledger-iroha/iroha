---
lang: hy
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8599dbc1a8e4fe846965eed90af128deb5950f83dc61838fea583b326b92a011
source_last_modified: "2025-12-29T18:16:35.104300+00:00"
translation_last_reviewed: 2026-02-07
id: incident-runbooks
title: Incident Runbooks & Rollback Drills
sidebar_label: Incident Runbooks
description: Response guides for failed portal deployments, SoraFS replication degradation, analytics outages, and the quarterly rehearsal cadence required by DOCS-9.
translator: machine-google-reviewed
---

## Նպատակը

Ճանապարհային քարտեզի **DOCS-9** կետը պահանջում է գործող գրքույկներ, ինչպես նաև փորձերի պլան, որպեսզի
պորտալի օպերատորները կարող են վերականգնվել առաքման ձախողումներից՝ առանց գուշակելու: Այս նշումը
ընդգրկում է երեք բարձր ազդանշանային միջադեպ՝ ձախողված տեղակայումներ, վերարտադրություն
դեգրադացիա և վերլուծական ընդհատումներ, և փաստում է այդ եռամսյակային վարժությունները
ապացուցել կեղծանունների վերադարձը և սինթետիկ վավերացումը մինչև վերջ:

### Հարակից նյութ

- [`devportal/deploy-guide`](./deploy-guide) - փաթեթավորում, ստորագրում և այլանուն
  խթանման աշխատանքային հոսք:
- [`devportal/observability`](./observability) — թողարկման պիտակներ, վերլուծություններ և
  զոնդերը նշված են ստորև:
- `docs/source/sorafs_node_client_protocol.md`
  և [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — ռեեստրի հեռաչափություն և էսկալացիայի շեմեր:
- `docs/portal/scripts/sorafs-pin-release.sh` և `npm run probe:*` օգնականներ
  նշված ամբողջ ստուգաթերթերում:

### Համատեղ հեռաչափություն և գործիքավորում

| Ազդանշան / Գործիք | Նպատակը |
| -------------- | ------- |
| `torii_sorafs_replication_sla_total` (հանդիպել/բաց թողնված/սպասում է) | Հայտնաբերում է կրկնօրինակման ախոռները և SLA-ի խախտումները: |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Քանակականացնում է հետնահերթության խորությունը և ավարտի ուշացումը տրաժի համար: |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Ցույց է տալիս դարպասի ձախողումները, որոնք հաճախ հետևում են վատ տեղակայմանը: |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Սինթետիկ զոնդեր, որոնք բացում և հաստատում են հետընթացները: |
| `npm run check:links` | Կոտրված կապի դարպաս; օգտագործվում է յուրաքանչյուր մեղմացումից հետո: |
| `sorafs_cli manifest submit … --alias-*` (փաթաթված `scripts/sorafs-pin-release.sh`-ով) | Այլանունների առաջխաղացման/վերադարձի մեխանիզմ: |
| `Docs Portal Publishing` Grafana տախտակ (`dashboards/grafana/docs_portal.json`) | Ագրեգատների մերժում/փոխանուն/TLS/կրկնօրինակման հեռաչափություն: PagerDuty-ի ազդանշանները վկայակոչում են այս վահանակներին: |

## Runbook — Չհաջողված տեղակայում կամ վատ արտեֆակտ

### Գործարկման պայմաններ

- Նախադիտման/արտադրական զոնդերը ձախողվում են (`npm run probe:portal -- --expect-release=…`):
- Grafana ահազանգեր `torii_sorafs_gateway_refusals_total` կամ
  `torii_sorafs_manifest_submit_total{status="error"}` թողարկումից հետո:
- Ձեռնարկի QA-ն անմիջապես հետո նկատում է կոտրված երթուղիները կամ Try-It վստահված անձի ձախողումները
  alias promotion.

### Անմիջական զսպում

1. **Սառեցրեք տեղակայումները.** նշեք CI խողովակաշարը `DEPLOY_FREEZE=1`-ով (GitHub
   աշխատանքային հոսքի մուտքագրում) կամ դադարեցրեք Ջենքինսի աշխատանքը, որպեսզի լրացուցիչ արտեֆակտներ չհեռանան:
2. **Սպանեք արտեֆակտները.** ներբեռնեք ձախողված կառուցվածքի `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, և զոնդը թողարկվի, որպեսզի հետադարձը կարողանա
   հղում ճշգրիտ մարսողություններին:
3. **Տեղեկացնել շահագրգիռ կողմերին.
   իրազեկման համար հերթապահ (հատկապես, երբ `docs.sora`-ը ազդել է):

### Հետադարձ կարգը

1. Բացահայտեք վերջին հայտնի-լավ (LKG) մանիֆեստը: Արտադրության աշխատանքային հոսքը խանութներ
   դրանք `artifacts/devportal/<release>/sorafs/portal.manifest.to`-ի ներքո:
2. Փոխանակման օգնականի հետ կապակցեք կեղծանունը այդ մանիֆեստին.

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. Գրանցեք հետադարձ ամփոփագիրը միջադեպի տոմսում LKG-ի հետ միասին և
   անհաջող մանիֆեստի մարսումներ.

### Վավերացում

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature …` և `sorafs_cli proof verify …`
   (տես տեղակայման ուղեցույցը)՝ հաստատելու համար, որ վերագովազդված մանիֆեստը դեռ համընկնում է
   արխիվացված մեքենան:
4. `npm run probe:tryit-proxy`՝ ապահովելու, որ Try-It բեմադրող վստահված սերվերը վերադարձավ:

### Հետպատահար

1. Վերագործարկեք տեղակայման խողովակաշարը միայն այն բանից հետո, երբ հասկանաք հիմնական պատճառը:
2. Backfill [`devportal/deploy-guide`](./deploy-guide) «Քաղված դասեր»
   գրառումներ նոր գոշերով, եթե այդպիսիք կան:
3. Ֆայլի թերությունները ձախողված թեստային փաթեթի համար (զոնդ, կապի ստուգիչ և այլն):

## Runbook — Replication degradation

### Գործարկման պայմաններ

- Զգուշացում՝ «sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"հանդիպել|բաց թողնված"}), 1) <
  0,95` 10 րոպե:
- `torii_sorafs_replication_backlog_total > 10` 10 րոպեով (տես
  `pin-registry-ops.md`):
- Կառավարությունը հայտնում է, որ թողարկումից հետո կեղծանունների դանդաղ հասանելիությունը կա:

### Տրիաժ

1. Ստուգեք [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) վահանակները՝ հաստատելու համար
   արդյոք հետնաժամկետները տեղայնացված են պահեստավորման դասի կամ մատակարարների նավատորմի մեջ:
2. Ստուգեք Torii տեղեկամատյանները `sorafs_registry::submit_manifest` նախազգուշացումների համար
   որոշել, թե արդյոք ներկայացումները ձախողվում են:
3. Նմուշի կրկնօրինակի առողջությունը `sorafs_cli manifest status --manifest …`-ի միջոցով (ցանկ
   մեկ մատակարարի կրկնօրինակման արդյունքները):

### Մեղմացում

1. Կրկին թողարկեք մանիֆեստը կրկնօրինակների ավելի մեծ քանակով (`--pin-min-replicas 7`)՝ օգտագործելով
   `scripts/sorafs-pin-release.sh` այնպես որ ժամանակացույցը բեռը տարածում է ավելի մեծ տարածքի վրա
   մատակարարի հավաքածու: Գրանցեք նոր մանիֆեստի ամփոփումը միջադեպերի մատյանում:
2. Եթե հետնահերթությունը կապված է մեկ մատակարարի հետ, ժամանակավորապես անջատեք այն
   կրկնօրինակման ժամանակացույց (փաստաթղթավորված `pin-registry-ops.md`-ում) և ներկայացնել նոր
   մանիֆեստ՝ ստիպելով մյուս մատակարարներին թարմացնել կեղծանունը:
3. Երբ կեղծանունների թարմությունն ավելի կարևոր է, քան կրկնօրինակման հավասարությունը, նորից կապեք
   արդեն բեմադրված ջերմ մանիֆեստի այլանունով (`docs-preview`), ապա հրապարակեք
   Հետագա մանիֆեստը, երբ SRE-ը մաքրում է կուտակվածը:

### Վերականգնում և փակում

1. Մոնիտոր `torii_sorafs_replication_sla_total{outcome="missed"}` ապահովելու համար
   հաշվում սարահարթեր.
2. Վերցրեք `sorafs_cli manifest status` ելքը որպես ապացույց, որ յուրաքանչյուր կրկնօրինակը
   հետ համապատասխանության մեջ:
3. Պատկերացրեք կամ թարմացրեք կրկնօրինակման հետմահու հետմահու հաջորդ քայլերով
   (մատակարարի մասշտաբավորում, chunker tuning և այլն):

## Runbook — Վերլուծություն կամ հեռաչափության խափանում

### Գործարկման պայմաններ

- `npm run probe:portal`-ը հաջողվում է, բայց վահանակները դադարում են կուլ տալ
  `AnalyticsTracker` իրադարձություններ >15 րոպեով:
- Գաղտնիության ակնարկը նշում է բաց թողնված իրադարձությունների անսպասելի աճը:
- `npm run probe:tryit-proxy`-ը ձախողվում է `/probe/analytics` ուղիներում:

### Պատասխան

1. Ստուգեք կառուցման ժամանակի մուտքերը՝ `DOCS_ANALYTICS_ENDPOINT` և
   `DOCS_ANALYTICS_SAMPLE_RATE` ձախողված թողարկման արտեֆակտում (`build/release.json`):
2. Կրկին գործարկեք `npm run probe:portal`՝ `DOCS_ANALYTICS_ENDPOINT`-ով մատնացույց անելով դեպի
   բեմադրող կոլեկցիոներ՝ հաստատելու համար, որ որոնիչը դեռևս թողարկում է օգտակար բեռներ:
3. Եթե կոլեկտորները խափանված են, դրեք `DOCS_ANALYTICS_ENDPOINT=""` և վերակառուցեք այնպես, որ
   tracker կարճ միացումներ; գրանցել անջատման պատուհանը միջադեպի ժամանակացույցում:
4. Վավերացրեք `scripts/check-links.mjs` դեռևս մատնահետքերը `checksums.sha256`
   (վերլուծական անջատումները չպետք է *չեն արգելափակեն կայքի քարտեզի վավերացումը):
5. Հենց որ կոլեկցիոները վերականգնվի, գործարկեք `npm run test:widgets`՝ վարժությունը վարելու համար
   Վերահրատարակումից առաջ վերլուծական օգնական միավորի թեստեր:

### Հետպատահար

1. Թարմացրեք [`devportal/observability`](./observability) ցանկացած նոր կոլեկցիոների հետ
   սահմանափակումներ կամ նմուշառման պահանջներ:
2. Ֆայլի կառավարման ծանուցում, եթե վերլուծական տվյալներից դուրս են թողնվել կամ խմբագրվել դրսում
   քաղաքականություն։

## Եռամսյակային ճկունության վարժանքներ

Երկու վարժանքներն էլ կատարեք **յուրաքանչյուր եռամսյակի առաջին երեքշաբթի** (հունվար/ապր/հուլիս/հոկտ.)
կամ ենթակառուցվածքի որևէ լուրջ փոփոխությունից անմիջապես հետո: Պահպանեք արտեֆակտները տակ
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Գայլիկոն | Քայլեր | Ապացույցներ |
| ----- | ----- | -------- |
| Alias ​​rollback rehearsal | 1. Կրկնել «Չհաջողված տեղակայումը» հետադարձ՝ օգտագործելով ամենավերջին արտադրական մանիֆեստը:<br/>2. Կրկին միացրեք արտադրությանը, երբ զոնդերը անցնեն:<br/>3. Գրանցեք `portal.manifest.submit.summary.json` և զոնդավորեք տեղեկամատյանները հորատման թղթապանակում: | `rollback.submit.json`, զոնդի ելք և փորձի թողարկման պիտակ: |
| Սինթետիկ վավերացման աուդիտ | 1. Գործարկեք `npm run probe:portal` և `npm run probe:tryit-proxy` արտադրության և բեմադրության դեմ:<br/>2. Գործարկեք `npm run check:links` և արխիվացրեք `build/link-report.json`:<br/>3. Կցեք Grafana վահանակների սքրինշոթներ/արտահանումներ, որոնք հաստատում են զոնդավորման հաջողությունը: | Զոնդերի մատյաններ + `link-report.json`՝ հղում անելով մանիֆեստի մատնահետքին: |

Բաց թողնված զորավարժությունները փոխանցեք Docs/DevRel մենեջերին և SRE կառավարման վերանայմանը,
քանի որ ճանապարհային քարտեզը պահանջում է դետերմինիստական, եռամսյակային ապացույցներ, որ երկուսն էլ կեղծանունները
հետադարձ և պորտալային զոնդերը մնում են առողջ:

## PagerDuty և հերթապահ համակարգում

- PagerDuty ծառայություն **Docs Portal Publishing**-ը պատկանում է ստեղծվող ահազանգերին
  `dashboards/grafana/docs_portal.json`. Կանոններ `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` և `DocsPortal/TLSExpiry` էջ Docs/DevRel
  առաջնային՝ Storage SRE-ով որպես երկրորդական:
- Երբ էջը տեղադրվում է, ներառեք `DOCS_RELEASE_TAG`-ը, կցեք տուժածների սքրինշոթները
  Grafana վահանակներ և կապի զոնդ/հղման ստուգման ելք նախորդ միջադեպի նշումներում
  մեղմացումը սկսվում է.
- Մեղմացումից հետո (վերադարձ կամ վերաբաշխում), նորից գործարկեք `npm run probe:portal`,
  `npm run check:links` և նկարահանեք թարմ Grafana նկարներ, որոնք ցույց են տալիս չափումները
  վերադառնալ շեմերի սահմաններում: Կցեք բոլոր ապացույցները PagerDuty-ի միջադեպին մինչ այդ
  լուծելով այն։
- Եթե միաժամանակ հնչում են երկու ազդանշաններ (օրինակ՝ TLS ժամկետի ավարտը գումարած հետաձգումը), տրիաժ
  նախ հրաժարվում է (դադարեցնել հրապարակումը), կատարել հետադարձման ընթացակարգը, այնուհետև մաքրել
  TLS/հետապահովված իրեր կամրջի վրա Storage SRE-ով: