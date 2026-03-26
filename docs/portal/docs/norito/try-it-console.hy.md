---
lang: hy
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5246118a539e2031dcafb8cf384ac7d20b8abc28b67ee1555e1b1211779fe390
source_last_modified: "2026-01-22T16:26:46.508367+00:00"
translation_last_reviewed: 2026-02-07
title: Norito Try-It Console
description: Use the developer-portal proxy, Swagger, and RapiDoc widgets to send real Torii / Norito-RPC requests directly from the documentation site.
translator: machine-google-reviewed
---

Պորտալը միավորում է երեք ինտերակտիվ մակերեսներ, որոնք փոխանցում են երթևեկությունը Torii-ին.

- **Swagger UI**-ը `/reference/torii-swagger`-ում ներկայացնում է ստորագրված OpenAPI սպեկտրը և ավտոմատ կերպով վերագրում է հարցումները վստահված անձի միջոցով, երբ `TRYIT_PROXY_PUBLIC_URL` կարգավորված է:
- **RapiDoc**-ը `/reference/torii-rapidoc`-ում ցուցադրում է նույն սխեման ֆայլերի վերբեռնումներով և բովանդակության տեսակի ընտրիչներով, որոնք լավ են աշխատում `application/x-norito`-ի համար:
- **Փորձեք այն sandbox** Norito ընդհանուր ակնարկ էջում տրամադրում է թեթև ձև REST ժամանակավոր հարցումների և OAuth սարքի մուտքի համար:

Բոլոր երեք վիջեթները հարցումներ են ուղարկում տեղական **Try-It proxy**-ին (`docs/portal/scripts/tryit-proxy.mjs`): Վստահված սերվերը ստուգում է, որ `static/openapi/torii.json`-ը համապատասխանում է `static/openapi/manifest.json`-ի ստորագրված ամփոփմանը, գործադրում է տոկոսադրույքի սահմանափակիչ, խմբագրում է `X-TryIt-Auth` վերնագրերը տեղեկամատյաններում և պիտակավորում յուրաքանչյուր վերընթաց զանգ I18NI0000000029X-ով, ուստի I18NI00000000029X-ը թույլ է տալիս թրաֆիկը:

## Գործարկեք վստահված անձը

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET`-ը Torii հիմնական URL-ն է, որը ցանկանում եք կիրառել:
- `TRYIT_PROXY_ALLOWED_ORIGINS`-ը պետք է ներառի բոլոր պորտալի սկզբնաղբյուրը (տեղական մշակողի սերվեր, արտադրության հոսթի անունը, նախադիտման URL), որը պետք է ներկառուցի վահանակը:
- `TRYIT_PROXY_PUBLIC_URL`-ը սպառվում է `docusaurus.config.js`-ի կողմից և ներարկվում վիջեթներում `customFields.tryIt`-ի միջոցով:
- `TRYIT_PROXY_BEARER` բեռնվում է միայն `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`; հակառակ դեպքում օգտվողները պետք է տրամադրեն իրենց սեփական նշանը վահանակի կամ OAuth սարքի հոսքի միջոցով:
- `TRYIT_PROXY_CLIENT_ID`-ը սահմանում է `X-TryIt-Client` պիտակը, որն իրականացվում է յուրաքանչյուր հարցում:
  `X-TryIt-Client`-ի մատակարարումը դիտարկիչից թույլատրվում է, բայց արժեքները կրճատված են
  և մերժվում է, եթե դրանք պարունակում են հսկիչ նիշեր:

Գործարկման ժամանակ վստահված անձը գործարկում է `verifySpecDigest` և դուրս է գալիս վերականգնման հուշումով, եթե մանիֆեստը հնացած է: Գործարկեք `npm run sync-openapi -- --latest`՝ նորագույն Torii սպեցիֆիկացիաները ներբեռնելու համար կամ անցեք `TRYIT_PROXY_ALLOW_STALE_SPEC=1`՝ արտակարգ իրավիճակների վերացման համար:

Վստահված անձի թիրախը թարմացնելու կամ ետ վերադարձնելու համար առանց միջավայրի ֆայլերը ձեռքով խմբագրելու, օգտագործեք օգնականը.

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## Միացրեք վիդջեթները

Ծառայել պորտալը վստահված անձի կողմից լսելուց հետո.

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js`-ը բացահայտում է հետևյալ բռնակները.

| Փոփոխական | Նպատակը |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | URL-ը ներարկվել է Swagger-ում, RapiDoc-ում և Try it sandbox-ում: Թողեք չկարգավորված՝ չթույլատրված նախադիտումների ժամանակ վիջեթները թաքցնելու համար: |
| `TRYIT_PROXY_DEFAULT_BEARER` | Հիշողության մեջ պահվող կամընտիր լռելյայն նշան: Պահանջվում է `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` և միայն HTTPS-ով CSP պաշտպանիչ (DOCS-1b), եթե դուք տեղական մակարդակում չեք անցնում `DOCS_SECURITY_ALLOW_INSECURE=1`: |
| `DOCS_OAUTH_*` | Միացրեք OAuth սարքի հոսքը (`OAuthDeviceLogin` բաղադրիչ), որպեսզի վերանայողները կարողանան կարճատև թոքեններ կտրատել՝ առանց պորտալից դուրս գալու: |

Երբ առկա են OAuth փոփոխականները, sandbox-ը ներկայացնում է **Մուտք գործեք սարքի կոդով** կոճակը, որը անցնում է կազմաձևված Auth սերվերի միջով (տե՛ս `config/security-helpers.js`՝ ճշգրիտ ձևի համար): Սարքի հոսքի միջոցով թողարկված նշանները պահվում են միայն բրաուզերի աշխատաշրջանում:

## Norito-RPC օգտակար բեռների ուղարկում

1. Ստեղծեք `.norito` օգտակար բեռ CLI-ով կամ [Norito արագ մեկնարկում] (./quickstart.md) նկարագրված CLI-ով կամ հատվածներով: Վստահված անձը փոխանցում է `application/x-norito` մարմինները անփոփոխ, այնպես որ դուք կարող եք նորից օգտագործել նույն արտեֆակտը, որը կհրապարակեիք `curl`-ով:
2. Բացեք `/reference/torii-rapidoc` (նախընտրելի է երկուական ծանրաբեռնվածության համար) կամ `/reference/torii-swagger`:
3. Բացվող ցանկից ընտրեք ցանկալի Torii նկարը: Ստորագրված են նկարներ; վահանակը ցույց է տալիս `static/openapi/manifest.json`-ում գրանցված մանիֆեստը:
4. Ընտրեք `application/x-norito` բովանդակության տեսակը «Փորձեք» դարակում, սեղմեք **Ընտրեք Ֆայլը** և ընտրեք ձեր օգտակար բեռը: Վստահված անձը վերագրում է հարցումը `/proxy/v1/pipeline/submit`-ին և այն նշում է `X-TryIt-Client=docs-portal-rapidoc`-ով:
5. Norito պատասխանները ներբեռնելու համար սահմանեք `Accept: application/x-norito`: Swagger/RapiDoc-ը ցուցադրում է վերնագրի ընտրիչը նույն դարակում և երկուականը հետ է փոխանցում վստահված անձի միջով:

Միայն JSON երթուղիների համար ներկառուցված «Փորձեք այն» ավազարկղը հաճախ ավելի արագ է. մուտքագրեք ուղին (օրինակ՝ `/v1/accounts/<i105-account-id>/assets`), ընտրեք HTTP մեթոդը, տեղադրեք JSON մարմինը, երբ անհրաժեշտ է, և սեղմեք **Ուղարկել հարցումը**՝ վերնագրերը, տևողությունը և օգտակար բեռները ստուգելու համար:

## Անսարքությունների վերացում

| Ախտանիշ | Հավանական պատճառ | Վերականգնում |
| --- | --- | --- |
| Բրաուզերի վահանակը ցույց է տալիս CORS սխալները կամ ավազարկղը զգուշացնում է, որ վստահված անձի URL-ը բացակայում է: | Proxy-ը չի աշխատում, կամ սկզբնաղբյուրը սպիտակ ցուցակում չէ: | Գործարկեք վստահված սերվերը, համոզվեք, որ `TRYIT_PROXY_ALLOWED_ORIGINS`-ը ծածկում է ձեր պորտալի հոսթը և վերագործարկեք `npm run start`: |
| `npm run tryit-proxy`-ը դուրս է գալիս «դիզեստի անհամապատասխանությամբ»: | Torii OpenAPI փաթեթը փոխվել է հոսանքին հակառակ: | Գործարկեք `npm run sync-openapi -- --latest` (կամ `--version=<tag>`) և նորից փորձեք: |
| Վիջեթները վերադարձնում են `401` կամ `403`: | Նշանները բացակայում են, ժամկետանց կամ անբավարար շրջանակներ: | Օգտագործեք OAuth սարքի հոսքը կամ տեղադրեք վավեր կրող նշան ավազարկղում: Ստատիկ նշանների համար դուք պետք է արտահանեք `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`: |
| `429 Too Many Requests` վստահված անձից: | Մեկ IP տոկոսադրույքի սահմանաչափը գերազանցվել է: | Բարձրացրեք `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` վստահելի միջավայրերի կամ շնչափող փորձարկման սկրիպտների համար: Բոլոր տոկոսադրույքի սահմանաչափի մերժումների ավելացում `tryit_proxy_rate_limited_total`: |

## Դիտարկելիություն

- `npm run probe:tryit-proxy` (`scripts/tryit-proxy-probe.mjs`-ի շուրջը փաթաթվածը) կանչում է `/healthz`, ընտրովի իրականացնում է օրինակելի երթուղի և թողարկում է Prometheus տեքստային ֆայլեր I18NI000000077X/I1807080000000000000000X տեքստային ֆայլերի համար: Կարգավորեք `TRYIT_PROXY_PROBE_METRICS_FILE`՝ ինտեգրվելու համար node_exporter-ին:
- Սահմանեք `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798`-ը, որպեսզի բացահայտի հաշվիչները (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) և հետաձգման հիստոգրամները: `dashboards/grafana/docs_portal.json` տախտակը կարդում է այս չափումները՝ DOCS-SORA SLO-ները կիրառելու համար:
- Runtime տեղեկամատյանները ապրում են stdout-ում: Յուրաքանչյուր գրառում ներառում է հարցման ID-ն, վերին հոսքի կարգավիճակը, նույնականացման աղբյուրը (`default`, `override` կամ `client`) և տևողությունը. գաղտնիքները վերագրվում են արտանետումից առաջ:

Եթե ​​Ձեզ անհրաժեշտ է հաստատել, որ `application/x-norito` օգտակար բեռները հասնում են Torii անփոփոխ, գործարկեք Jest փաթեթը (`npm test -- tryit-proxy`) կամ ստուգեք հարմարանքները `docs/portal/scripts/__tests__/tryit-proxy.test.mjs`-ի ներքո: Ռեգրեսիոն թեստերը ներառում են սեղմված Norito երկուականներ, ստորագրված OpenAPI մանիֆեստներ և վստահված անձի նվազման ուղիները, որպեսզի NRPC-ի թողարկումները պահպանեն մշտական ​​ապացույցների հետքը: