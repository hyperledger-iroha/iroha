---
id: deploy-guide
lang: hy
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Deployment Guide
sidebar_label: Deployment Guide
description: Promote the developer portal through the SoraFS pipeline with deterministic builds, Sigstore signing, and rollback drills.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Տեսություն

Այս գրքույկը փոխակերպում է ճանապարհային քարտեզի տարրերը **DOCS-7** (SoraFS հրատարակում) և **DOCS-8**
(CI/CD փին ավտոմատացում) մշակողների պորտալի համար գործող ընթացակարգի մեջ:
Այն ընդգրկում է կառուցման/ծածկույթի փուլը, SoraFS փաթեթավորումը, Sigstore պաշտպանված մանիֆեստը
ստորագրում, alias առաջխաղացում, ստուգում և հետադարձ զորավարժություններ, որպեսզի յուրաքանչյուր նախադիտում և
թողարկման արտեֆակտը վերարտադրելի է և ստուգելի:

Հոսքը ենթադրում է, որ դուք ունեք `sorafs_cli` երկուական (կառուցված
`--features cli`), մուտք դեպի Torii վերջնակետ՝ pin-registry թույլտվություններով, և
OIDC հավատարմագրեր Sigstore-ի համար: Պահպանեք երկարակյաց գաղտնիքները (`IROHA_PRIVATE_KEY`,
`SIGSTORE_ID_TOKEN`, Torii նշաններ) ձեր CI պահոցում; տեղական վազքերը կարող են դրանք աղբյուր լինել
կեղևի արտահանումից։

## Նախադրյալներ

- Հանգույց 18.18+ `npm` կամ `pnpm` հետ:
- `sorafs_cli` `cargo run -p sorafs_car --features cli --bin sorafs_cli`-ից:
- Torii URL, որը բացահայտում է `/v1/sorafs/*`, գումարած հեղինակային հաշիվ/մասնավոր բանալի
  որոնք կարող են ներկայացնել մանիֆեստներ և կեղծանուններ:
- OIDC թողարկող (GitHub Actions, GitLab, աշխատանքային բեռի նույնականացում և այլն)
  `SIGSTORE_ID_TOKEN`.
- Լրացուցիչ. `examples/sorafs_cli_quickstart.sh` չոր վազքի համար և
  `docs/source/sorafs_ci_templates.md` GitHub/GitLab աշխատանքային հոսքի փայտամածների համար:
- Կարգավորեք Try it OAuth փոփոխականները (`DOCS_OAUTH_*`) և գործարկեք
  [անվտանգության կարծրացման ստուգաթերթիկ] (./security-hardening.md) նախքան շինարարությունը խթանելը
  լաբորատորիայից դուրս. Պորտալի կառուցումն այժմ ձախողվում է, երբ այս փոփոխականները բացակայում են
  կամ երբ TTL/քվեարկության բռնակները ընկնում են հարկադիր պատուհաններից դուրս. արտահանում
  `DOCS_OAUTH_ALLOW_INSECURE=1` միայն մեկանգամյա օգտագործման տեղական նախադիտումների համար: Կցեք
  գրիչ-թեստ ապացույցներ ազատման տոմսի համար:

## Քայլ 0 — Գրեք «Փորձեք այն» վստահված անձի փաթեթը

Նախքան Netlify-ի կամ gateway-ի նախադիտումը խթանելը, դրոշմեք «Փորձեք այն» վստահված անձը
աղբյուրները և ստորագրված OpenAPI մանիֆեստը ամփոփվում են դետերմինիստական փաթեթի մեջ.

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs`-ը պատճենում է վստահված անձը/հետաքննությունը/հետադարձ օգնականները,
ստուգում է OpenAPI ստորագրությունը և գրում `release.json` plus
`checksums.sha256`. Կցեք այս փաթեթը Netlify/SoraFS gateway խթանմանը
տոմս, որպեսզի գրախոսները կարողանան վերարտադրել վստահված անձի ճշգրիտ աղբյուրները և Torii թիրախային ակնարկները
առանց վերակառուցման: Փաթեթը նաև արձանագրում է, թե արդյոք հաճախորդի կողմից մատակարարված կրողներ են եղել
միացված է (`allow_client_auth`)՝ թողարկման պլանը և CSP կանոնները համաժամեցված պահելու համար:

## Քայլ 1 - Կառուցեք և ծածկեք պորտալը

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build`-ը ավտոմատ կերպով կատարում է `scripts/write-checksums.mjs`՝ արտադրելով.

- `build/checksums.sha256` — SHA256 մանիֆեստ, որը հարմար է `sha256sum -c`-ի համար:
- `build/release.json` — մետատվյալներ (`tag`, `generated_at`, `source`) ամրացված
  յուրաքանչյուր մեքենա/մանիֆեստ:

Արխիվացրեք երկու ֆայլերը CAR ամփոփագրի կողքին, որպեսզի վերանայողները կարողանան տարբերել նախադիտումը
արտեֆակտներ առանց վերակառուցման.

## Քայլ 2 - Փաթեթավորեք ստատիկ ակտիվները

Գործարկեք CAR փաթեթիչը Docusaurus ելքային գրացուցակի դեմ: Ստորև բերված օրինակը
գրում է բոլոր արտեֆակտները `artifacts/devportal/` տակ:

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
  --input build \
  --car-out "$OUT"/portal.car \
  --plan-out "$OUT"/portal.plan.json \
  --summary-out "$OUT"/portal.car.json \
  --chunker-handle sorafs.sf1@1.0.0
```

Ամփոփիչ JSON-ը ֆիքսում է հատվածների թվերը, ամփոփումները և ապացուցման պլանավորման հուշումները, որ
`manifest build` և CI վահանակները հետագայում նորից օգտագործվեն:

## Քայլ 2բ — Փաթեթ OpenAPI և SBOM ուղեկիցներ

DOCS-7-ը պահանջում է հրապարակել պորտալի կայքը, OpenAPI նկարը և SBOM օգտակար բեռները
ինչպես տարբեր դրսևորումներ, այնպես որ դարպասները կարող են կեռել `Sora-Proof`/`Sora-Content-CID`
վերնագրեր յուրաքանչյուր արտեֆակտի համար: Ազատման օգնականը
(`scripts/sorafs-pin-release.sh`) արդեն փաթեթավորում է OpenAPI գրացուցակը
(`static/openapi/`) և `syft`-ի միջոցով արտանետվող SBOM-ները առանձին
`openapi.*`/`*-sbom.*` ավտոմեքենաներ և գրանցում է մետատվյալները
`artifacts/sorafs/portal.additional_assets.json`. Ձեռքով հոսքը գործարկելիս,
կրկնել 2–4 քայլերը յուրաքանչյուր օգտակար բեռի համար՝ իր նախածանցներով և մետատվյալների պիտակներով
(օրինակ՝ `--car-out "$OUT"/openapi.car` plus
`--metadata alias_label=docs.sora.link/openapi`): Գրանցեք յուրաքանչյուր մանիֆեստ / կեղծանուն
Զույգացրեք Torii-ում (կայք, OpenAPI, պորտալ SBOM, OpenAPI SBOM) նախքան DNS-ն անցնելը
դարպասը կարող է կեռ ապացույցներ ծառայել բոլոր հրապարակված արտեֆակտների համար:

## Քայլ 3 - Կառուցեք մանիֆեստը

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

Կարգավորեք pin-policy դրոշները ձեր թողարկման պատուհանում (օրինակ՝ «--pin-storage-class»
տաք՝ դեղձանիկների համար): JSON տարբերակը կամընտիր է, բայց հարմար է կոդի վերանայման համար:

## Քայլ 4 — Ստորագրեք Sigstore-ով

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

Փաթեթը գրանցում է մանիֆեստի ամփոփումը, հատվածի ամփոփումները և BLAKE3 հեշը
OIDC նշան՝ առանց JWT-ի պահպանման: Պահեք և՛ կապոցը, և՛ անջատված
ստորագրություն; արտադրության խթանումները կարող են կրկին օգտագործել նույն արտեֆակտները՝ հրաժարական տալու փոխարեն:
Տեղական գործարկումները կարող են փոխարինել մատակարարի դրոշակները `--identity-token-env`-ով (կամ սահմանել
`SIGSTORE_ID_TOKEN` շրջակա միջավայրում), երբ արտաքին OIDC օգնականը թողարկում է
նշան.

## Քայլ 5 — Ներկայացրե՛ք փին ռեեստր

Ներկայացրե՛ք ստորագրված մանիֆեստը (և մասնակի պլանը) Torii հասցեին: Միշտ պահանջեք ամփոփագիր
հետևաբար գրանցամատյանում գրանցման/փոխանունի ապացույցը ենթակա է աուդիտի:

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority i105... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

Նախադիտումը կամ դեղձանիկ մականունը (`docs-preview.sora`) հրապարակելիս կրկնել.
ներկայացում եզակի կեղծանունով, որպեսզի QA-ն կարողանա ստուգել բովանդակությունը մինչ արտադրությունը
առաջխաղացում.

Այլանունների կապակցումը պահանջում է երեք դաշտ՝ `--alias-namespace`, `--alias-name` և
`--alias-proof`. Կառավարումը արտադրում է ապացույցների փաթեթը (base64 կամ Norito բայթ)
երբ կեղծանունի հարցումը հաստատվում է. Պահպանեք այն CI գաղտնիքներում և դրեք այն որպես ա
ֆայլ նախքան `manifest submit` կանչելը: Թողեք կեղծանունների դրոշակները, երբ դուք
միայն մտադիր են ամրացնել մանիֆեստը՝ առանց DNS-ի հպման:

## Քայլ 5բ — Ստեղծեք կառավարման առաջարկ

Յուրաքանչյուր մանիֆեստ պետք է ճանապարհորդել խորհրդարանին պատրաստ առաջարկով, որպեսզի ցանկացած Սորա
քաղաքացին կարող է փոփոխություն կատարել առանց արտոնյալ հավատարմագրերի փոխառության։
Ներկայացնել/ստորագրել քայլերից հետո գործարկել՝

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json`-ը գրավում է կանոնական `RegisterPinManifest`-ը
հրահանգներ, կտորների ամփոփում, քաղաքականություն և կեղծանունների հուշում: Կցեք այն կառավարմանը
տոմս կամ խորհրդարանի պորտալ, որպեսզի պատվիրակները կարողանան տարբերել օգտակար բեռը առանց վերակառուցման
արտեֆակտները։ Քանի որ հրամանը երբեք չի դիպչում Torii հեղինակային ստեղնին, ցանկացած
քաղաքացին կարող է առաջարկը մշակել տեղում:

## Քայլ 6 — Ստուգեք ապացույցները և հեռաչափությունը

Ամրացումից հետո կատարեք հաստատման որոշիչ քայլերը.

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- Ստուգեք `torii_sorafs_gateway_refusals_total` և
  `torii_sorafs_replication_sla_total{outcome="missed"}` անոմալիաների համար:
- Գործարկեք `npm run probe:portal`՝ Try-It վստահված անձին և գրանցված հղումները գործարկելու համար
  ընդդեմ նոր ամրացված բովանդակության։
- Վերցրեք մոնիտորինգի ապացույցները, որոնք նկարագրված են
  [Հրապարակում և մոնիտորինգ] (./publishing-monitoring.md) այնպես որ DOCS-3c
  Դիտորդական դարպասը բավարարված է հրապարակման քայլերին զուգահեռ: Օգնականը
  այժմ ընդունում է բազմաթիվ `bindings` գրառումներ (կայք, OpenAPI, պորտալ SBOM, OpenAPI
  SBOM) և կիրառում է `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` թիրախի վրա
  հյուրընկալել կամընտիր `hostname` պահակախմբի միջոցով: Ստորև բերված կոչը գրում է և՛ ա
  մեկ JSON ամփոփագիր և ապացույցների փաթեթ (`portal.json`, `tryit.json`,
  `binding.json` և `checksums.sha256`) թողարկման գրացուցակի տակ.

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Քայլ 6ա — Պլանավորեք դարպասների վկայականները

Նախքան GAR փաթեթներ ստեղծելը, ստացեք TLS SAN/մարտահրավեր ծրագիրը
թիմը և DNS հաստատողները վերանայում են նույն ապացույցները: Նոր օգնականը արտացոլում է
DG-3 ավտոմատացման մուտքերը՝ թվարկելով կանոնական նիշերի հյուրընկալողներ,
գեղեցիկ հյուրընկալող SAN-ներ, DNS-01 պիտակներ և առաջարկվող ACME մարտահրավերներ.

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

Պահպանեք JSON-ը թողարկման փաթեթի կողքին (կամ վերբեռնեք այն փոփոխությամբ
տոմս), որպեսզի օպերատորները կարողանան տեղադրել SAN արժեքները Torii-ի մեջ
`torii.sorafs_gateway.acme` կոնֆիգուրացիան և GAR վերանայողները կարող են հաստատել
կանոնական/գեղեցիկ քարտեզագրումներ՝ առանց հյուրընկալող ածանցյալների վերագործարկման: Ավելացնել լրացուցիչ
`--name` արգումենտներ յուրաքանչյուր վերջածանցի համար, որը գովազդվում է նույն թողարկումում:

## Քայլ 6բ — Ստացեք կանոնական հյուրընկալող քարտեզագրումներ

Նախքան GAR-ի օգտակար բեռների ձևանմուշը, գրանցեք հյուրընկալողի որոշիչ քարտեզագրումը յուրաքանչյուրի համար
կեղծանունը: `cargo xtask soradns-hosts`-ը հեշավորում է յուրաքանչյուր `--name` իր կանոնական
պիտակը (`<base32>.gw.sora.id`), թողարկում է պահանջվող նիշը
(`*.gw.sora.id`) և ստացվում է գեղեցիկ հյուրընկալող (`<alias>.gw.sora.name`): Համառել
թողարկման արտեֆակտների արդյունքը, որպեսզի DG-3 վերանայողները կարողանան տարբերել քարտեզագրումը
GAR ներկայացման հետ մեկտեղ.

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Օգտագործեք `--verify-host-patterns <file>` արագ ձախողման համար, երբ GAR կամ դարպաս կա
binding JSON-ը բաց է թողնում պահանջվող հյուրընկալողներից մեկը: Օգնականն ընդունում է բազմաթիվ
ստուգման ֆայլեր, ինչը հեշտացնում է թե՛ GAR ձևանմուշը, և թե՛ երեսպատումը
կեռ `portal.gateway.binding.json` նույն կոչման մեջ.

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Կցեք ամփոփիչ JSON-ը և հաստատման մատյանը DNS/gateway փոփոխության տոմսին
աուդիտորները կարող են հաստատել կանոնական, վիթխարի և գեղեցիկ հաղորդավարները՝ առանց վերագործարկման
ածանցյալ սցենարները. Նորից գործարկեք հրամանը, երբ նոր անուններ են ավելացվում հրամանին
փաթեթ, ուստի GAR-ի հետագա թարմացումները ժառանգում են նույն ապացույցների հետքը:

## Քայլ 7 — Ստեղծեք DNS անջատիչ նկարագրիչը

Արտադրության կրճատումները պահանջում են ստուգվող փոփոխության փաթեթ: Հաջողությունից հետո
ներկայացում (alias binding), օգնականը արտանետում է
`artifacts/sorafs/portal.dns-cutover.json`, գրավելով.- կեղծանունների պարտադիր մետատվյալներ (անունների տարածություն/անուն/ապացույց, մանիֆեստի ամփոփում, Torii URL,
  ներկայացված դարաշրջան, իշխանություն);
- թողարկման համատեքստ (պիտակ, կեղծանունի պիտակ, մանիֆեստ/CAR ուղիներ, կտոր պլան, Sigstore
  փաթեթ);
- ստուգման ցուցիչներ (զոնդի հրաման, կեղծանուն + Torii վերջնակետ); և
- կամընտիր փոփոխության վերահսկման դաշտեր (տոմսի id, կտրման պատուհան, օպերատիվ կոնտակտ,
  արտադրության հոսթի անունը/գոտի);
- երթուղու առաջխաղացման մետատվյալներ, որոնք ստացվել են կեռավորված `Sora-Route-Binding`-ից
  վերնագիր (կանոնական հոսթ/CID, վերնագիր + կապող ուղիներ, ստուգման հրամաններ),
  ապահովելով, որ GAR-ի առաջխաղացումը և հետադարձ զորավարժությունները վերաբերում են նույն ապացույցներին.
- ստեղծված երթուղու պլանի արտեֆակտները (`gateway.route_plan.json`,
  վերնագրերի ձևանմուշներ և կամընտիր հետադարձ վերնագրեր), այնպես որ փոխեք տոմսերը և CI
  lint hooks-ը կարող է ստուգել, որ յուրաքանչյուր DG-3 փաթեթ հղում է անում կանոնականին
  առաջխաղացման/վերադարձի պլաններ մինչև հաստատումը.
- կամընտիր քեշի անվավերացման մետատվյալներ (մաքրման վերջնակետ, վավերացման փոփոխական, JSON
  օգտակար բեռ և օրինակ `curl` հրաման); և
- հետադարձ ակնարկներ, որոնք ուղղված են նախորդ նկարագրիչին (թողարկեք պիտակը և մանիֆեստը
  digest), այնպես որ փոխեք տոմսերը գրավում են որոշիչ հետադարձ ճանապարհ:

Երբ թողարկումը պահանջում է քեշի մաքրումներ, ստեղծեք կանոնական պլան՝ կողքին
կտրվածքի նկարագրիչ.

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Կցեք ստացված `portal.cache_plan.json`-ը DG-3 փաթեթին, որպեսզի օպերատորներ
թողարկելիս ունենան դետերմինիստական հյուրընկալողներ/ուղիներ (և համապատասխան auth ակնարկներ):
`PURGE` հարցումներ: Նկարագրիչի կամընտիր քեշի մետատվյալների բաժինը կարող է հղում կատարել
այս ֆայլը ուղղակիորեն՝ պահպանելով փոփոխության վերահսկման վերանայողներին համապատասխանեցված կոնկրետ որոնց վրա
վերջնակետերը ողողվում են կտրման ժամանակ:

Յուրաքանչյուր DG-3 փաթեթի կարիք ունի նաև առաջխաղացում + վերադարձի ստուգաթերթ: Ստեղծեք այն միջոցով
`cargo xtask soradns-route-plan`, որպեսզի փոփոխության վերահսկման վերանայողները կարողանան ճշգրիտ հետևել
նախաթռիչք, անջատում և հետադարձ քայլեր ըստ այլանունների՝

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

Արտանետվող `gateway.route_plan.json`-ը գրավում է կանոնական/գեղեցիկ հաղորդավարներ, բեմադրված
առողջության ստուգման հիշեցումներ, GAR պարտադիր թարմացումներ, քեշի մաքրում և հետադարձ գործողություններ:
Փաթեթավորեք այն GAR/կապող/կտրող արտեֆակտներով՝ նախքան փոփոխությունը ներկայացնելը
տոմս, որպեսզի Ops-ը կարողանա կրկնել և ստորագրել նույն սցենարով նախատեսված քայլերը:

`scripts/generate-dns-cutover-plan.mjs`-ը միացնում է այս նկարագրիչը և գործարկում
ավտոմատ կերպով `sorafs-pin-release.sh`-ից: Այն վերականգնելու կամ հարմարեցնելու համար
ձեռքով:

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --out artifacts/sorafs/portal.dns-cutover.json \
  --change-ticket OPS-4821 \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact docs-oncall@sora.link \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

Լրացրեք կամընտիր մետատվյալները շրջակա միջավայրի փոփոխականների միջոցով նախքան փին գործարկելը
օգնական:

| Փոփոխական | Նպատակը |
|----------|---------|
| `DNS_CHANGE_TICKET` | Տոմսի ID-ն պահվում է նկարագրիչում: |
| `DNS_CUTOVER_WINDOW` | ISO8601 կտրող պատուհան (օրինակ՝ `2026-03-21T15:00Z/2026-03-21T15:30Z`): |
| `DNS_HOSTNAME`, `DNS_ZONE` | Արտադրության հոսթի անուն + հեղինակավոր գոտի: |
| `DNS_OPS_CONTACT` | Հերթական փոխանուն կամ էսկալացիոն կոնտակտ: |
| `DNS_CACHE_PURGE_ENDPOINT` | Քեշի մաքրման վերջնակետը գրանցված է նկարագրիչում: |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var, որը պարունակում է մաքրման նշան (կանխադրված է `CACHE_PURGE_TOKEN`): |
| `DNS_PREVIOUS_PLAN` | Հետադարձ մետատվյալների համար ուղի դեպի նախորդ հատման նկարագրիչ: |

Կցեք JSON-ը DNS-ի փոփոխության վերանայմանը, որպեսզի հաստատողները կարողանան հաստատել մանիֆեստը
digests, alias bindings և probe հրամաններ՝ առանց CI տեղեկամատյանները քերելու:
CLI դրոշակներ `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`-ը և `--previous-dns-plan`-ը ապահովում են նույն վերափոխումները
CI-ից դուրս օգնականը գործարկելիս:

## Քայլ 8 — Թողարկեք լուծիչի գոտու ֆայլի կմախքը (ըստ ցանկության)

Երբ հայտնի է արտադրության կտրման պատուհանը, թողարկման սցենարը կարող է թողարկել
SNS zonefile skeleton-ը և լուծիչի հատվածը ինքնաբերաբար: Անցեք ցանկալի DNS-ը
գրառումներ և մետատվյալներ կամ շրջակա միջավայրի փոփոխականների կամ CLI ընտրանքների միջոցով. օգնականը
կզանգահարի `scripts/sns_zonefile_skeleton.py` անմիջապես անջատումից հետո
ստեղծվում է նկարագրիչ: Տրամադրեք առնվազն մեկ A/AAAA/CNAME արժեք և GAR
digest (BLAKE3-256 ստորագրված GAR օգտակար բեռի): Եթե գոտին/հյուրընկալող անունը հայտնի է
իսկ `--dns-zonefile-out`-ը բաց է թողնված, օգնականը գրում է
`artifacts/sns/zonefiles/<zone>/<hostname>.json` և բնակեցնում է
`ops/soradns/static_zones.<hostname>.json` որպես լուծիչի հատված:

| Փոփոխական / դրոշ | Նպատակը |
|-----------------|---------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Ստեղծված zonefile կմախքի ուղին: |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Լուծիչի հատվածի ուղի (կանխադրված է `ops/soradns/static_zones.<hostname>.json`, երբ բաց թողնված է): |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL-ը կիրառվում է ստեղծված գրառումների վրա (կանխադրված՝ 600 վայրկյան): |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | IPv4 հասցեներ (ստորակետերով առանձնացված env կամ կրկնվող CLI դրոշ): |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | IPv6 հասցեներ. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Ընտրովի CNAME թիրախ: |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | SHA-256 SPKI կապում (base64): |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Լրացուցիչ TXT գրառումներ (`key=value`): |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Չեղարկել հաշվարկված zonefile տարբերակի պիտակը: |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Կտրող պատուհանի մեկնարկի փոխարեն հարկադրել `effective_at` ժամադրոշմը (RFC3339): |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Անտեսեք մետատվյալներում գրանցված ապացույցի բառացիությունը: |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Անտեսեք մետատվյալներում գրանցված CID-ը: |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Պահապանի սառեցման վիճակ (փափուկ, կոշտ, հալեցում, մոնիտորինգ, արտակարգ իրավիճակ): |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Պահապան/խորհրդի տոմսերի տեղեկանք սառեցման համար: |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | RFC3339 ժամադրոշմ՝ հալման համար: |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Լրացուցիչ սառեցման նշումներ (ստորակետերով առանձնացված env կամ կրկնվող դրոշակ): |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | BLAKE3-256 digest (վեցանկյուն) ստորագրված GAR օգտակար բեռի: Պահանջվում է, երբ առկա են դարպասների ամրացումներ: |

GitHub Actions-ի աշխատանքային հոսքը կարդում է այս արժեքները պահեստի գաղտնիքներից, այնպես որ յուրաքանչյուր արտադրական փին ավտոմատ կերպով թողարկում է zonefile artefacts: Կարգավորեք հետևյալ գաղտնիքները (տողերը կարող են պարունակել ստորակետերով բաժանված ցուցակներ բազմարժեք դաշտերի համար).

| Գաղտնի | Նպատակը |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Արտադրության հոսթի անունը/գոտին փոխանցվել է օգնականին: |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Նկարագրիչում պահվող զանգի այլանունները: |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | IPv4/IPv6 գրառումները հրապարակելու համար: |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Ընտրովի CNAME թիրախ: |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Base64 SPKI կապում: |
| `DOCS_SORAFS_ZONEFILE_TXT` | Լրացուցիչ TXT գրառումներ: |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Սառեցրեք կմախքի մեջ գրանցված մետատվյալները: |
| `DOCS_SORAFS_GAR_DIGEST` | Վեցանկյուն կոդավորված BLAKE3-ը ստորագրված GAR օգտակար բեռի մասին: |

`.github/workflows/docs-portal-sorafs-pin.yml` գործարկելիս տրամադրեք `dns_change_ticket` և `dns_cutover_window` մուտքերը, որպեսզի նկարագրիչը/գոտի ֆայլը ժառանգի ճիշտ փոփոխության պատուհանի մետատվյալները: Թողեք դրանք դատարկ միայն չոր վազքի ժամանակ:

Տիպիկ կոչում (համապատասխանում է SN-7 սեփականատիրոջ ընթացիկ գրքույկին).

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
  --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
  --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
  --dns-zonefile-ipv4 198.51.100.4 \
  --dns-zonefile-ttl 600 \
  --dns-zonefile-freeze-state soft \
  --dns-zonefile-freeze-ticket SNS-DF-XXXX \
  --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
  --dns-gar-digest <gar-digest-hex> \
  …other flags…
```

Օգնականը ավտոմատ կերպով փոխանցում է փոփոխության տոմսը որպես TXT մուտք և
ժառանգում է կտրման պատուհանի սկիզբը որպես `effective_at` ժամանակի դրոշմակնիք, եթե
գերագնահատված. Գործառնական աշխատանքի ամբողջական ընթացքի համար տե՛ս
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Հանրային DNS պատվիրակության նշում

Zonefile կմախքը սահմանում է միայն հեղինակավոր գրառումներ գոտու համար: Դուք
դեռ պետք է կարգավորել մայր գոտու NS/DS պատվիրակությունը ձեր գրանցամատյանում կամ DNS-ում
մատակարար, որպեսզի սովորական ինտերնետը կարողանա հայտնաբերել անունների սերվերները:

- գագաթային/TLD կտրվածքների համար օգտագործեք ALIAS/ANAME (հատուկ մատակարարին) կամ հրապարակեք A/AAAA
  գրառումներ, որոնք մատնանշում են դարպասի ցանկացած կետի IP-ները:
- Ենթադոմեյնների համար հրապարակեք CNAME ստացված գեղեցիկ հոսթին
  (`<fqdn>.gw.sora.name`):
- Կանոնական սերվերը (`<hash>.gw.sora.id`) մնում է դարպասի տիրույթում և
  չի հրապարակվում ձեր հանրային գոտու ներսում:

### Gateway վերնագրի ձևանմուշ

Տեղակայման օգնականը նաև արտանետում է `portal.gateway.headers.txt` և
`portal.gateway.binding.json`, երկու արտեֆակտ, որոնք բավարարում են DG-3-ի
gateway-content-binding պահանջ.

- `portal.gateway.headers.txt` պարունակում է HTTP վերնագրի ամբողջական բլոկը (ներառյալ
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS և
  `Sora-Route-Binding` նկարագրիչ), որ եզրային դարպասները պետք է ամրացվեն յուրաքանչյուրի վրա
  արձագանք.
- `portal.gateway.binding.json`-ը գրանցում է նույն տեղեկատվությունը մեքենայաընթեռնելի լեզվով
  ձև, այնպես որ փոխեք տոմսերը և ավտոմատացումը կարող է տարբերել հյուրընկալող/cid կապերը առանց
  scraping shell արտադրանքը.

Նրանք ստեղծվում են ավտոմատ կերպով միջոցով
`cargo xtask soradns-binding-template`
և գրավեք տրամադրված այլանունները, ակնառու բովանդակությունը և մուտքի հաղորդավարի անունը
դեպի `sorafs-pin-release.sh`: Վերնագրի բլոկը վերականգնելու կամ հարմարեցնելու համար գործարկեք՝

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Անցեք `--csp-template`, `--permissions-template` կամ `--hsts-template`՝ անտեսելու համար
լռելյայն վերնագրի ձևանմուշները, երբ որոշակի տեղակայման կարիք ունի լրացուցիչ
հրահանգներ; միավորել դրանք գոյություն ունեցող `--no-*` անջատիչների հետ՝ վերնագիր թողնելու համար
ամբողջությամբ.

Կցեք վերնագրի հատվածը CDN փոփոխության հարցումին և սնուցեք JSON փաստաթուղթը
մուտք դեպի դարպասի ավտոմատացման խողովակաշար, որպեսզի իրական հյուրընկալող առաջխաղացումը համապատասխանի դրան
հրապարակել ապացույցներ.

Թողարկման սցենարը ավտոմատ կերպով գործարկում է ստուգման օգնականը, որպեսզի DG-3 տոմսերը
միշտ ներառել վերջին ապացույցները: Վերագործարկեք այն ձեռքով, երբ կսմթեք այն
կապող JSON ձեռքով.

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

Հրամանը վավերացնում է `Sora-Proof` օգտակար բեռը, որը գրավված է պարտադիր փաթեթում,
ապահովում է, որ `Sora-Route-Binding` մետատվյալները համընկնում են մանիֆեստի CID + հոսթի անվան հետ,
և արագ ձախողվում է, եթե որևէ վերնագիր շեղվում է: Արխիվացրեք վահանակի ելքը կողքին
տեղակայման այլ արտեֆակտներ, երբ հրամանը գործարկեք CI-ից դուրս, որպեսզի DG-3
վերանայողներն ունեն ապացույց, որ պարտադիր է վավերացվել նախքան անջատումը:> **DNS նկարագրիչի ինտեգրում.** `portal.dns-cutover.json`-ն այժմ ներկառուցում է
> `gateway_binding` բաժինը, որը ցույց է տալիս այս արտեֆակտները (ուղիներ, բովանդակության CID,
> ապացույցի կարգավիճակը և բառացի վերնագրի ձևանմուշը) **և** `route_plan` տող
> հղում կատարելով `gateway.route_plan.json`-ին, գումարած հիմնական + հետադարձ վերնագիրը
> կաղապարներ: Ներառեք այդ բլոկները DG-3 փոփոխության յուրաքանչյուր տոմսում, որպեսզի վերանայողները կարողանան
> Տարբերեք ճշգրիտ `Sora-Name/Sora-Proof/CSP` արժեքները և հաստատեք, որ երթուղին
> առաջխաղացման/վերադարձի պլանները համընկնում են ապացույցների փաթեթին՝ առանց շինարարությունը բացելու
> արխիվ:

## Քայլ 9 — Գործարկեք հրապարակման մոնիտորները

Ճանապարհային քարտեզի առաջադրանքը **DOCS-3c** պահանջում է շարունակական ապացույցներ, որ պորտալը փորձեք
վստահված անձը և դարպասի կապերը մնում են առողջ թողարկումից հետո: Գործարկել համախմբվածը
վերահսկեք անմիջապես քայլերը 7–8-ից հետո և միացրեք այն ձեր պլանավորված զոնդերի մեջ.

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs`-ը բեռնում է կազմաձևման ֆայլը (տես
  `docs/portal/docs/devportal/publishing-monitoring.md` սխեմայի համար) և
  իրականացնում է երեք ստուգում՝ պորտալի ուղու զոնդեր + CSP/Թույլտվություններ-Քաղաքականության վավերացում,
  Փորձեք պրոքսի զոնդերը (ըստ ցանկության՝ հարվածելով իր `/metrics` վերջնակետին), և
  դարպասի պարտադիր ստուգիչ (`cargo xtask soradns-verify-binding`), որը ստուգում է
  գրավված պարտադիր փաթեթը ակնկալվող կեղծանունի, հաղորդավարի, ապացույցի կարգավիճակի նկատմամբ,
  և դրսևորել JSON:
- Հրամանը դուրս է գալիս ոչ զրոյից, երբ որևէ հետաքննություն ձախողվում է այնպես, որ CI, cron jobs կամ
  runbook-ի օպերատորները կարող են դադարեցնել թողարկումը նախքան այլանունները խթանելը:
- `--json-out` անցնելը գրում է մեկ ամփոփիչ JSON ծանրաբեռնվածություն՝ յուրաքանչյուր թիրախի հետ
  կարգավիճակ; `--evidence-dir`-ն արտանետում է `summary.json`, `portal.json`, `tryit.json`,
  `binding.json` և `checksums.sha256`, որպեսզի կառավարման վերանայողները կարողանան տարբերել
  արդյունքներ՝ առանց մոնիտորները նորից գործարկելու: Արխիվացրեք այս գրացուցակը տակ
  `artifacts/sorafs/<tag>/monitoring/` Sigstore փաթեթի և DNS-ի հետ մեկտեղ
  կտրվածքի նկարագրիչ.
- Ներառեք մոնիտորի ելքը, Grafana արտահանումը (`dashboards/grafana/docs_portal.json`),
  և Alertmanager drill ID-ն թողարկման տոմսում, որպեսզի DOCS-3c SLO-ն կարողանա լինել
  աուդիտի ենթարկվել ավելի ուշ: Նվիրված հրատարակչական մոնիտորի խաղագիրքը ապրում է
  `docs/portal/docs/devportal/publishing-monitoring.md`.

Պորտալի զոնդերը պահանջում են HTTPS և մերժում են `http://` բազային URL-ները, եթե
`allowInsecureHttp`-ը դրված է մոնիտորի կազմաձևում; պահպանել արտադրությունը/բեմականացումը
թիրախներ TLS-ում և միացնել միայն տեղական նախադիտումների անտեսումը:

Ավտոմատացրեք մոնիտորը `npm run monitor:publishing`-ի միջոցով Buildkite/cron-ում մեկ անգամ
պորտալն ուղիղ եթերում է: Նույն հրամանը, որը մատնանշված է արտադրության URL-ների վրա, սնուցում է շարունակականը
առողջության ստուգումներ, որոնց վրա հիմնվում են SRE/Docs-ը թողարկումների միջև:

## Ավտոմատացում `sorafs-pin-release.sh`-ով

`docs/portal/scripts/sorafs-pin-release.sh`-ը ներառում է 2–6 քայլերը: Այն:

1. արխիվացնում է `build/`-ը դետերմինիստական թարբոլի մեջ,
2. աշխատում է `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   և `proof verify`,
3. ընտրովի կատարում է `manifest submit` (ներառյալ alias binding), երբ Torii
   հավատարմագրերը ներկա են, և
4. գրում է `artifacts/sorafs/portal.pin.report.json`, ընտրովի
  `portal.pin.proposal.json`, DNS անջատիչ նկարագրիչ (ներկայացումներից հետո),
  և դարպասի պարտադիր փաթեթը (`portal.gateway.binding.json` գումարած
  տեքստի վերնագրի բլոկ), այնպես որ կառավարման, ցանցային և օպերացիոն թիմերը կարող են տարբերվել
  ապացույցների փաթեթ առանց CI տեղեկամատյանները քերելու:

Սահմանել `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` և (ըստ ցանկության)
`PIN_ALIAS_PROOF_PATH` նախքան սկրիպտը կանչելը: Օգտագործեք `--skip-submit` չորացման համար
վազում; ստորև նկարագրված GitHub աշխատանքային հոսքը փոխում է սա `perform_submit`-ի միջոցով
մուտքագրում.

## Քայլ 8 — Հրապարակեք OpenAPI բնութագրերը և SBOM փաթեթները

DOCS-7-ը պահանջում է պորտալի կառուցում, OpenAPI սպեցիֆիկացիա և SBOM արտեֆակտներ ճանապարհորդելու համար
նույն դետերմինիստական խողովակաշարով։ Առկա օգնականները ներառում են բոլոր երեքը.

1. **Վերականգնել և ստորագրել սպեկտրը։**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Փոխանցեք թողարկման պիտակը `--version=<label>`-ի միջոցով, երբ ցանկանում եք պահպանել
   պատմական պատկեր (օրինակ՝ `2025-q3`): Օգնականը գրում է նկարը
   դեպի `static/openapi/versions/<label>/torii.json`, արտացոլում է այն
   `versions/current` և գրանցում է մետատվյալները (SHA-256, մանիֆեստի կարգավիճակ և
   թարմացված ժամանակացույց) `static/openapi/versions.json`-ում: Մշակողների պորտալը
   կարդում է այդ ինդեքսը, որպեսզի Swagger/RapiDoc վահանակները կարողանան տարբերակի ընտրիչ ներկայացնել
   և ցուցադրել առնչվող ամփոփիչ/ստորագրության տեղեկատվությունը ներդիրում: Բաց թողնելը
   `--version`-ը պահպանում է նախորդ թողարկման պիտակները և միայն թարմացնում է
   `current` + `latest` ցուցիչներ:

   Մանիֆեստը ֆիքսում է SHA-256/BLAKE3 մարսողությունները, որպեսզի դարպասը կարողանա կեռել
   `Sora-Proof` վերնագրեր `/reference/torii-swagger`-ի համար:

2. **Emit CycloneDX SBOMs.** Թողարկման խողովակաշարն արդեն ակնկալում է syft-ի վրա հիմնված
   SBOM-ներ ըստ `docs/source/sorafs_release_pipeline_plan.md`-ի: Պահպանեք ելքը
   շինարարական արտեֆակտների կողքին.

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Փաթեթավորեք յուրաքանչյուր բեռնատար ավտոմեքենայի մեջ:**

   ```bash
   sorafs_cli car pack \
     --input docs/portal/static/openapi \
     --car-out "$OUT"/openapi.car \
     --plan-out "$OUT"/openapi.plan.json \
     --summary-out "$OUT"/openapi.car.json

   sorafs_cli car pack \
     --input "$OUT"/portal.sbom.json \
     --car-out "$OUT"/portal.sbom.car \
     --plan-out "$OUT"/portal.sbom.plan.json \
     --summary-out "$OUT"/portal.sbom.car.json
   ```

   Հետևեք նույն `manifest build` / `manifest sign` քայլերին, ինչպես հիմնական կայքը,
   թյունինգ մականունները մեկ ակտիվի համար (օրինակ՝ `docs-openapi.sora`՝ բնութագրերի և
   `docs-sbom.sora` ստորագրված SBOM փաթեթի համար): Պահպանելով հստակ անուններ
   պահում է SoraDNS-ի ապացույցները, GAR-ները և հետադարձ տոմսերը՝ ճշգրիտ բեռնվածության շրջանակում:

4. **Ներկայացրեք և կապեք։** Կրկին օգտագործեք գոյություն ունեցող հեղինակությունը + Sigstore փաթեթը, բայց
   գրանցեք այլանունների կրկնօրինակը թողարկման ստուգաթերթում, որպեսզի աուդիտորները կարողանան հետևել, թե որին
   Սորա անվան քարտեզները, որոնց մանիֆեստը մարսվում է:

Պորտալի կառուցման հետ մեկտեղ սպեցիֆիկ/SBOM դրսևորումների արխիվացումը ապահովում է ամեն ինչ
թողարկման տոմսը պարունակում է ամբողջական արտեֆակտ հավաքածու՝ առանց փաթեթավորող սարքը նորից գործարկելու:

### Ավտոմատացման օգնական (CI/փաթեթի սցենար)

`./ci/package_docs_portal_sorafs.sh`-ը կոդավորում է քայլերը 1–8, այնպես որ ճանապարհային քարտեզի տարրը
**DOCS‑7**-ը կարող է իրականացվել մեկ հրամանով: Օգնականը.

- գործարկում է անհրաժեշտ պորտալի նախապատրաստումը (`npm ci`, OpenAPI/norito sync, widget-ի թեստեր);
- թողարկում է պորտալը, OpenAPI և SBOM CARs + մանիֆեստի զույգերը `sorafs_cli`-ի միջոցով;
- ընտրովի աշխատում է `sorafs_cli proof verify` (`--proof`) և Sigstore ստորագրում
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- յուրաքանչյուր արտեֆակտ գցում է `artifacts/devportal/sorafs/<timestamp>/`-ի տակ և
  գրում է `package_summary.json`, որպեսզի CI/release tooling-ը կարողանա ներծծել փաթեթը; և
- թարմացնում է `artifacts/devportal/sorafs/latest`-ը, որպեսզի մատնանշի ամենավերջին գործարկումը:

Օրինակ (ամբողջական խողովակաշար Sigstore + PoR-ով):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Դրոշներ, որոնք արժե իմանալ.

- `--out <dir>` – վերացնել արտեֆակտի արմատը (կանխադրված պահում է ժամանակի դրոշմավորված թղթապանակներ):
- `--skip-build` – վերօգտագործեք գոյություն ունեցող `docs/portal/build` (հարմար է, երբ CI-ն չի կարող
  վերակառուցել անցանց հայելիների շնորհիվ):
- `--skip-sync-openapi` – բաց թողնել `npm run sync-openapi`, երբ `cargo xtask openapi`
  չի կարող հասնել crates.io-ին:
- `--skip-sbom` – խուսափեք զանգահարել `syft`, երբ երկուականը տեղադրված չէ (
  սկրիպտը դրա փոխարեն նախազգուշացում է տպում):
- `--proof` – գործարկել `sorafs_cli proof verify` յուրաքանչյուր ավտոմեքենայի/մանիֆեստի զույգի համար: Բազմաթիվ
  ֆայլերի օգտակար բեռները դեռ պահանջում են CLI-ում chunk-plan աջակցություն, այնպես որ թողեք այս դրոշը
  անջատեք, եթե սեղմեք `plan chunk count` սխալները և ստուգեք ձեռքով մեկ անգամ
  վերևում գտնվող դարպասի հողերը.
- `--sign` – կանչել `sorafs_cli manifest sign`: Տրամադրեք նշան
  `SIGSTORE_ID_TOKEN` (կամ `--sigstore-token-env`) կամ թույլ տվեք CLI-ին այն վերցնել՝ օգտագործելով
  `--sigstore-provider/--sigstore-audience`.

Արտադրական արտեֆակտներ առաքելիս օգտագործեք `docs/portal/scripts/sorafs-pin-release.sh`:
Այն այժմ փաթեթավորում է պորտալը, OpenAPI և SBOM բեռները, ստորագրում է յուրաքանչյուր մանիֆեստ և
գրանցում է լրացուցիչ ակտիվների մետատվյալներ `portal.additional_assets.json`-ում: Օգնականը
հասկանում է նույն ընտրովի կոճակները, որոնք օգտագործվում են CI փաթեթավորողի կողմից և նորը
`--openapi-*`, `--portal-sbom-*` և `--openapi-sbom-*` անջատիչներ, որպեսզի կարողանաք
նշանակել alias tuples յուրաքանչյուր արտեֆակտի համար, վերացնել SBOM աղբյուրը միջոցով
`--openapi-sbom-source`, բաց թողնել որոշակի օգտակար բեռներ (`--skip-openapi`/`--skip-sbom`),
և մատնանշեք ոչ լռելյայն `syft` երկուականի վրա `--syft-bin`-ով:

Սցենարը ցուցադրում է յուրաքանչյուր հրաման, որն աշխատում է. պատճենեք գրանցամատյանը թողարկման տոմսի մեջ
`package_summary.json`-ի կողքին, որպեսզի գրախոսները կարողանան տարբերակել CAR-ի ամփոփումները, պլանավորել
մետատվյալներ և Sigstore փաթեթային հեշեր՝ առանց հատուկ ժամանակավոր կեղևի ելքի:

## Քայլ 9 — Gateway + SoraDNS ստուգում

Նախքան անջատման մասին հայտարարելը, ապացուցեք, որ նոր կեղծանունը լուծում է SoraDNS-ի միջոցով և դա
gateways հիմնական թարմ ապացույցներ:

1. **Գործարկեք զոնդի դարպասը։** `ci/check_sorafs_gateway_probe.sh` վարժություններ
   `cargo xtask sorafs-gateway-probe` ընդդեմ ցուցադրական սարքերի
   `fixtures/sorafs_gateway/probe_demo/`. Իրական տեղակայման համար ուղղեք զոնդը
   թիրախային հոսթի անվանման դեպքում՝

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   Զոնդը վերծանում է `Sora-Name`, `Sora-Proof` և `Sora-Proof-Status` մեկ
   `docs/source/sorafs_alias_policy.md` և ձախողվում է, երբ մանիֆեստը մարսվում է,
   TTL-ներ, կամ GAR կապերի դրեյֆ:

   Թեթև տեղում ստուգելու համար (օրինակ, երբ միայն կապող փաթեթը
   փոխվել է), գործարկել `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`:
   Օգնականը վավերացնում է գրավված կապող փաթեթը և հարմար է ազատման համար
   տոմսեր, որոնց համար անհրաժեշտ է միայն պարտադիր հաստատում ամբողջական զոնդային վարժանքի փոխարեն:

2. **Գործեք հորատման ապացույցները:** Օպերատորի փորվածքների կամ PagerDuty չոր վազքի համար, փաթեթավորեք
   զոնդը `scripts/telemetry/run_sorafs_gateway_probe.sh --սցենարով
   devportal-rollout --…`: Փաթաթումը պահպանում է վերնագրերը/տեղեկամատյանները տակ
   `artifacts/sorafs_gateway_probe/<stamp>/`, թարմացումներ `ops/drill-log.md` և
   (ըստ ցանկության) գործարկում է հետադարձ կեռիկներ կամ PagerDuty ծանրաբեռնվածություն: Սահմանել
   `--host docs.sora`՝ հաստատելու SoraDNS ուղին IP կոշտ կոդավորման փոխարեն:3. **Ստուգեք DNS կապերը։** Երբ կառավարումը հրապարակում է կեղծանունի ապացույցը, գրանցեք
   GAR ֆայլը, որը նշված է զոնդում (`--gar`) և կցեք այն թողարկմանը
   ապացույցներ. Լուծիչի սեփականատերերը կարող են արտացոլել նույն մուտքագրումը
   `tools/soradns-resolver` ապահովելու համար, որ պահված գրառումները հարգում են նոր մանիֆեստը:
   JSON-ը կցելուց առաջ գործարկեք
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   Այսպիսով, դետերմինիստական հյուրընկալող քարտեզագրումը, մանիֆեստի մետատվյալները և հեռաչափության պիտակները
   վավերացված անցանց: Օգնականը կարող է հրապարակել `--json-out` ամփոփագիր կողքին
   ստորագրված ԳԱՐ, որպեսզի վերանայողները ունենան ստուգելի ապացույցներ՝ առանց երկուականը բացելու:
  Նոր ԳԱՌ նախագծելիս նախընտրեք
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (վերադարձ դեպի `--manifest-cid <cid>` միայն այն դեպքում, երբ մանիֆեստի ֆայլը չկա
  մատչելի): Օգնողն այժմ բխում է CID **և** BLAKE3 մարսողությունը անմիջապես դրանից
  մանիֆեստը JSON, կրճատում է բաց տարածությունը, կրկնօրինակում է կրկնվող `--telemetry-label`
  դրոշակում է, տեսակավորում է պիտակները և թողարկում լռելյայն CSP/HSTS/Permissions-Policy-ը
  ձևանմուշները նախքան JSON-ը գրելը, որպեսզի բեռնվածությունը որոշիչ մնա նույնիսկ այն ժամանակ, երբ
  օպերատորները տարբեր պատյաններից պիտակներ են վերցնում:

4. **Դիտեք կեղծանունների չափումները։** Պահեք `torii_sorafs_alias_cache_refresh_duration_ms`
   և `torii_sorafs_gateway_refusals_total{profile="docs"}` էկրանին, մինչդեռ
   զոնդն աշխատում է; երկու սերիաներն էլ գծագրված են
   `dashboards/grafana/docs_portal.json`.

## Քայլ 10 — Մոնիտորինգ և ապացույցների փաթեթավորում

- **Dashboards.** Արտահանել `dashboards/grafana/docs_portal.json` (պորտալի SLOs),
  `dashboards/grafana/sorafs_gateway_observability.json` (դարպասի ուշացում +
  առողջության ապացույց), և `dashboards/grafana/sorafs_fetch_observability.json`
  (նվագախմբի առողջություն) յուրաքանչյուր թողարկման համար: Կցեք JSON արտահանումները
  թողարկեք տոմսը, որպեսզի գրախոսները կարողանան կրկնել Prometheus հարցումները:
- **Զոնդի արխիվներ** Պահեք `artifacts/sorafs_gateway_probe/<stamp>/`-ը git-annex-ում
  կամ ձեր ապացույցների դույլը: Ներառեք հետաքննության ամփոփագիրը, վերնագրերը և PagerDuty-ը
  ծանրաբեռնվածություն, որը գրավել է հեռաչափական սցենարը:
- **Թողարկեք փաթեթը։** Պահպանեք պորտալը/SBOM/OpenAPI CAR ամփոփագրերը, մանիֆեստը
  փաթեթներ, Sigstore ստորագրություններ, `portal.pin.report.json`, Try-It probe մատյաններ և
  հղումը ստուգեք հաշվետվությունները մեկ ժամանակի դրոշմված թղթապանակում (օրինակ,
  `artifacts/sorafs/devportal/20260212T1103Z/`):
- **Հորատման մատյան.** Երբ զոնդերը փորվածքի մաս են կազմում, թող
  `scripts/telemetry/run_sorafs_gateway_probe.sh` հավելվածը `ops/drill-log.md`-ին
  այնպես որ նույն ապացույցը բավարարում է SNNet-5 քաոսի պահանջը:
- **Տոմսերի հղումներ** Նշեք Grafana վահանակի ID-ները կամ կցված PNG արտահանումները
  փոփոխության տոմսը, հետաքննության հաշվետվության ուղու հետ միասին, այնպես որ փոփոխության վերանայողներ
  կարող է խաչաձև ստուգել SLO-ները առանց կեղևի հասանելիության:

## Քայլ 11 — Բազմաղբյուրի բեռնման փորվածք և ցուցատախտակի ապացույցներ

SoraFS-ում հրապարակելու համար այժմ պահանջվում է բազմաթիվ աղբյուրներից ստացվող ապացույցներ (DOCS-7/SF-6)
վերը նշված DNS/gateway ապացույցների կողքին: Մանիֆեստն ամրացնելուց հետո՝

1. **Գործարկեք `sorafs_fetch`-ը կենդանի մանիֆեստի դեմ:** Օգտագործեք նույն պլանը/մանիֆեստը
   արտեֆակտներ, որոնք արտադրվել են Քայլ 2–3-ում, գումարած յուրաքանչյուրի համար տրված մուտքի հավատարմագրերը
   մատակարար. Պահպանեք յուրաքանչյուր ելք, որպեսզի աուդիտորները կարողանան վերնվագել նվագախմբին
   որոշման հետքեր.

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$FETCH_OUT"

   cargo run -p sorafs_car --bin sorafs_fetch -- \
     --plan "$OUT/portal.plan.json" \
     --manifest-json "$OUT/portal.manifest.json" \
     --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
     --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
     --scoreboard-out "$FETCH_OUT/scoreboard.json" \
     --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
     --json-out "$FETCH_OUT/fetch.json" \
     --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
     --telemetry-json artifacts/sorafs/provider_telemetry.json \
     --max-peers=3 \
     --retry-budget=4
   ```

   - Սկզբում առբերեք մանիֆեստի կողմից նշված մատակարարի գովազդները (օրինակ
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     և փոխանցեք դրանք `--provider-advert name=path`-ի միջոցով, որպեսզի ցուցատախտակը կարողանա
     որոշիչ կերպով գնահատել պատուհանների հնարավորությունները: Օգտագործեք
     `--allow-implicit-provider-metadata` **միայն** հարմարանքները նորից խաղալիս
     CI; արտադրական վարժանքները պետք է վկայակոչեն ստորագրված գովազդները, որոնք վայրէջք են կատարել
     քորոց.
   - Երբ մանիֆեստը վկայակոչում է լրացուցիչ շրջաններ, կրկնեք հրամանը
     համապատասխան մատակարարը բազմապատկվում է, որպեսզի յուրաքանչյուր քեշ/փոխանուն ունենա համապատասխանություն
     բերել արտեֆակտ:

2. **Արխիվացրեք ելքերը։** Պահպանեք `scoreboard.json`,
   `providers.ndjson`, `fetch.json` և `chunk_receipts.ndjson` տակ
   թողարկել ապացույցների թղթապանակը. Այս ֆայլերը գրավում են գործընկերների կշռումը, նորից փորձեք
   բյուջեն, ուշացման EWMA-ն և յուրաքանչյուր կտորի անդորրագրերը, որոնք պետք է կառավարման փաթեթը
   պահպանել SF-7-ի համար:

3. **Թարմացրեք հեռաչափությունը։** Ներմուծեք առբերման արդյունքները **SoraFS Fetch-ում
   Դիտորդականություն** վահանակ (`dashboards/grafana/sorafs_fetch_observability.json`),
   դիտելով `torii_sorafs_fetch_duration_ms`/`_failures_total` և
   մատակարարի շրջանակի վահանակներ անոմալիաների համար: Կցեք Grafana վահանակի նկարները
   թողարկման տոմսը ցուցատախտակի ուղու կողքին:

4. **Ծխեք զգուշացման կանոնները։** Գործարկեք `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   Prometheus ազդանշանային փաթեթը վավերացնելու համար նախքան թողարկումը փակելը: Կցել
   promtool-ը թողարկվում է տոմսի վրա, որպեսզի DOCS-7 վերանայողները կարողանան հաստատել տաղավարը
   և դանդաղ մատակարարող ահազանգերը շարունակում են զինված մնալ:

5. **Մալարը դեպի CI։** Պորտալի փին աշխատանքային հոսքը հետ է պահում `sorafs_fetch` քայլը
   `perform_fetch_probe` մուտքագրում; միացնել այն բեմադրության/արտադրության համար, որպեսզի
   առբերման ապացույցները արտադրվում են մանիֆեստի փաթեթի կողքին՝ առանց ձեռնարկի
   միջամտություն. Տեղական վարժությունները կարող են նորից օգտագործել նույն սկրիպտը՝ արտահանելով
   Դարպասի նշանները և սահմանելով `PIN_FETCH_PROVIDERS` ստորակետերով բաժանված
   մատակարարների ցուցակը:

## Առաջխաղացում, դիտելիություն և հետադարձ կապ

1. **Առաջիկա.** պահեք առանձին բեմական և արտադրական անուններ: Խթանել ըստ
   կրկին գործարկել `manifest submit`-ը նույն մանիֆեստով/փաթեթով, փոխանակում
   `--alias-namespace/--alias-name`՝ մատնանշելու արտադրական կեղծանունը: Սա
   խուսափում է վերակառուցվելուց կամ հրաժարական տալուց, երբ QA-ն հաստատում է բեմականացման փին:
2. **Մոնիտորինգ.** ներմուծեք pin-registry dashboard-ը
   (`docs/source/grafana_sorafs_pin_registry.json`) գումարած հատուկ պորտալի համար
   զոնդերը (տես `docs/portal/docs/devportal/observability.md`): Զգուշացում ստուգիչ գումարի վերաբերյալ
   drift, ձախողված զոնդեր կամ ապացուցողական կրկնակի ցատկեր:
3. **Վերադարձ.
   ընթացիկ կեղծանունը) օգտագործելով `sorafs_cli manifest submit --alias ... --retire`:
   Միշտ պահեք վերջին հայտնի լավ փաթեթը և CAR-ի ամփոփագիրը, որպեսզի հետադարձ ապացույցները կարողանան
   վերստեղծվել, եթե CI տեղեկամատյանները պտտվեն:

## CI աշխատանքային հոսքի ձևանմուշ

Առնվազն ձեր խողովակաշարը պետք է.

1. Build + lint (`npm ci`, `npm run build`, checksum-ի առաջացում):
2. Փաթեթ (`car pack`) և հաշվարկել մանիֆեստները:
3. Ստորագրեք՝ օգտագործելով աշխատանքի շրջանակով OIDC նշանը (`manifest sign`):
4. Վերբեռնեք արտեֆակտներ (CAR, մանիֆեստ, փաթեթ, պլան, ամփոփագրեր) աուդիտի համար:
5. Ներկայացրեք փին ռեեստր.
   - Քաշեք հարցումներ → `docs-preview.sora`:
   - Պիտակներ / պաշտպանված մասնաճյուղեր → արտադրության այլանունների խթանում:
6. Դուրս գալուց առաջ գործարկեք զոնդերը + ապացույցների ստուգման դարպասները:

`.github/workflows/docs-portal-sorafs-pin.yml`-ը միացնում է այս բոլոր քայլերը միասին
ձեռքով թողարկումների համար: Աշխատանքային ընթացքը.

- կառուցում/փորձարկում է պորտալը,
- փաթեթավորում է կառուցումը `scripts/sorafs-pin-release.sh`-ի միջոցով,
- ստորագրում/հաստատում է մանիֆեստի փաթեթը՝ օգտագործելով GitHub OIDC,
- վերբեռնում է CAR/մանիֆեստը/փաթեթը/պլան/ապացույցը որպես արտեֆակտ և
- (ըստ ցանկության) ներկայացնում է մանիֆեստը + կեղծանունը պարտադիր, երբ առկա են գաղտնիքներ:

Նախքան աշխատանքը գործարկելը կարգավորեք պահեստի հետևյալ գաղտնիքները/փոփոխականները.

| Անունը | Նպատակը |
|------|---------|
| `DOCS_SORAFS_TORII_URL` | Torii հոսթ, որը բացահայտում է `/v1/sorafs/pin/register`-ը: |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Դարաշրջանի նույնացուցիչը գրանցված է ներկայացումներով: |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Մանիֆեստի ներկայացման համար ստորագրող լիազորություն: |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Այլանունների կրկնօրինակը կապված է մանիֆեստին, երբ `perform_submit`-ը `true` է: |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Base64-ով կոդավորված alias proof փաթեթ (ըստ ցանկության, բաց թողնել alias binding): |
| `DOCS_ANALYTICS_*` | Գոյություն ունեցող վերլուծական/հետազոտման վերջնակետեր, որոնք կրկին օգտագործվում են այլ աշխատանքային հոսքերի կողմից: |

Գործարկեք աշխատանքային հոսքը Actions UI-ի միջոցով.

1. Տրամադրել `alias_label` (օրինակ՝ `docs.sora.link`), կամընտիր `proposal_alias`,
   և կամընտիր `release_tag` փոխարինում:
2. `perform_submit`-ը թողեք չնշված՝ արտեֆակտներ ստեղծելու համար՝ առանց Torii-ին դիպչելու:
   (օգտակար չոր գործարկումների համար) կամ թույլ տվեք այն հրապարակել ուղղակիորեն կազմաձևվածում
   կեղծանունը:

`docs/source/sorafs_ci_templates.md`-ը դեռևս փաստաթղթերում է ընդհանուր CI օգնականները
նախագծեր այս պահոցից դուրս, սակայն պորտալի աշխատանքային հոսքը պետք է նախընտրելի լինի
ամենօրյա թողարկումների համար:

## ստուգաթերթ

- [ ] `npm run build`, `npm run test:*` և `npm run check:links` կանաչ են:
- [ ] `build/checksums.sha256` և `build/release.json` գրավված արտեֆակտներում:
- [ ] CAR, պլան, մանիֆեստ և ամփոփում, որը ստեղծվել է `artifacts/`-ի ներքո:
- [ ] Sigstore փաթեթ + անջատված ստորագրություն, որը պահվում է տեղեկամատյաններով:
- [ ] `portal.manifest.submit.summary.json` և `portal.manifest.submit.response.json`
      նկարահանվում է, երբ ներկայացվում են:
- [ ] `portal.pin.report.json` (և կամընտիր `portal.pin.proposal.json`)
      արխիվացված CAR/manifest artefacts-ի կողքին:
- [ ] `proof verify` և `manifest verify-signature` տեղեկամատյանները արխիվացված են:
- [ ] Grafana վահանակները թարմացվել են + «Փորձեք» զոնդերը հաջող են:
- [ ] Հետադարձ նշումներ (նախորդ մանիֆեստի ID + alias digest) կցված են
      թողարկման տոմս.