---
lang: hy
direction: ltr
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b920e21b96436755f7d37f7b5577465cb3e30016d36340c50f7c6f3a9a46919
source_last_modified: "2025-12-29T18:16:35.116499+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Փորձեք ավազատուփը

Մշակողների պորտալը ուղարկում է կամընտիր «Փորձեք այն» վահանակը, որպեսզի կարողանաք զանգահարել Torii
վերջնակետերը՝ առանց փաստաթղթերից դուրս գալու: Վահանակը փոխանցում է հարցումները
փաթեթավորված պրոքսիի միջոցով, որպեսզի բրաուզերները կարողանան շրջանցել CORS-ի սահմանները դեռևս
տոկոսադրույքների սահմանաչափերի և իսկորոշման կիրառում:

## Նախադրյալներ

- Node.js 18.18 կամ ավելի նոր տարբերակ (համապատասխանում է պորտալի կառուցման պահանջներին)
- Ցանցային մուտք դեպի Torii բեմականացման միջավայր
- Կրող նշան, որը կարող է զանգահարել Torii երթուղիները, որոնք դուք նախատեսում եք իրականացնել

Բոլոր վստահված անձի կոնֆիգուրացիան կատարվում է շրջակա միջավայրի փոփոխականների միջոցով: Ստորև բերված աղյուսակը
թվարկում է ամենակարևոր կոճակները.

| Փոփոխական | Նպատակը | Կանխադրված |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Հիմնական Torii URL-ը, որը վստահված անձը ուղարկում է հարցումները | **Պարտադիր** |
| `TRYIT_PROXY_LISTEN` | Լսեք տեղական զարգացման հասցե (ձևաչափ `host:port` կամ `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Ստորակետերով բաժանված ծագման ցուցակ, որը կարող է զանգահարել վստահված անձի | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | `X-TryIt-Client`-ում տեղադրված նույնացուցիչը վերին հոսքի յուրաքանչյուր հարցման համար | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Կանխադրված կրիչի նշանն ուղարկվել է Torii | _դատարկ_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Թույլատրել վերջնական օգտագործողներին մատակարարել իրենց սեփական նշանը `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Հարցման մարմնի առավելագույն չափը (բայթ) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Վերին հոսքի ժամանակի վերջնաժամկետը միլիվայրկյաններով | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Հայտերը թույլատրվում են ըստ տոկոսադրույքի պատուհանի մեկ հաճախորդի IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Սահող պատուհան արագության սահմանափակման համար (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Լսելու կամընտիր հասցե Prometheus ոճի չափման վերջնակետի համար (`host:port` կամ `[ipv6]:port`) | _դատարկ (հաշմանդամ)_ |
| `TRYIT_PROXY_METRICS_PATH` | HTTP ուղին սպասարկվում է չափումների վերջնակետով | `/metrics` |

Վստահված սերվերը նաև բացահայտում է `GET /healthz`-ը, վերադարձնում է JSON-ի կառուցվածքային սխալները և
խմբագրում է կրիչի նշանները տեղեկամատյանների ելքից:

Միացնել `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`-ը փաստաթղթերի օգտատերերի համար պրոքսիի ցուցադրման ժամանակ, որպեսզի Swagger-ը և
RapiDoc վահանակները կարող են փոխանցել օգտվողի կողմից մատակարարվող կրիչի նշանները: Վստահված անձը դեռևս կիրառում է դրույքաչափերի սահմանաչափերը,
խմբագրում է հավատարմագրերը և արձանագրում, թե արդյոք հարցումն օգտագործել է լռելյայն նշանը, թե մեկ հարցման վերագրում:
Սահմանեք `TRYIT_PROXY_CLIENT_ID` այն պիտակի վրա, որը ցանկանում եք ուղարկել որպես `X-TryIt-Client`
(կանխադրված է `docs-portal`): Վստահված սերվերը կրճատում և վավերացնում է զանգահարողի կողմից տրվածը
`X-TryIt-Client` արժեքներ, որոնք հետ են ընկնում այս լռելյայնին, որպեսզի բեմադրող դարպասները կարողանան
աուդիտի ծագումն առանց բրաուզերի մետատվյալների փոխկապակցման:

## Գործարկեք վստահված անձը տեղում

Տեղադրեք կախվածությունները, երբ առաջին անգամ ստեղծեք պորտալը.

```bash
cd docs/portal
npm install
```

Գործարկեք վստահված անձը և ուղղեք այն ձեր Torii օրինակին.

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Սցենարը գրանցում է կապված հասցեն և ուղարկում հարցումները `/proxy/*`-ից դեպի
կազմաձևված Torii ծագում:

Նախքան վարդակից կապելը, սցենարը հաստատում է դա
`static/openapi/torii.json`-ը համապատասխանում է գրանցված ամփոփմանը
`static/openapi/manifest.json`. Եթե ֆայլերը շարժվում են, հրամանը դուրս է գալիս an-ով
սխալ է և հրահանգում է գործարկել `npm run sync-openapi -- --latest`: Արտահանում
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` միայն արտակարգ իրավիճակների վերացման համար; վստահված անձը կամք
մուտքագրեք նախազգուշացում և շարունակեք, որպեսզի կարողանաք վերականգնել սպասարկման պատուհանների ժամանակ:

## Միացրեք պորտալի վիդջեթները

Երբ դուք կառուցում կամ սպասարկում եք մշակողների պորտալը, սահմանեք վիջեթների URL-ը
վստահված անձի համար պետք է օգտագործվի՝

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Հետևյալ բաղադրիչները կարդում են այս արժեքները `docusaurus.config.js`-ից.

- **Swagger UI** — մատուցված է `/reference/torii-swagger`-ում; նախապես թույլատրում է
  կրողի սխեման, երբ առկա է նշան, պիտակավորում է հարցումները `X-TryIt-Client`-ով,
  ներարկում է `X-TryIt-Auth` և վերագրում է զանգերը վստահված անձի միջոցով, երբ
  Սահմանված է `TRYIT_PROXY_PUBLIC_URL`:
- **RapiDoc** — մատուցված է `/reference/torii-rapidoc`; արտացոլում է նշանի դաշտը,
  նորից օգտագործում է նույն վերնագրերը, ինչ Swagger վահանակը և թիրախավորում է վստահված անձին
  ավտոմատ կերպով, երբ URL-ը կազմաձևված է:
- **Փորձեք այն կոնսոլից** — ներկառուցված է API-ի ակնարկ էջում; թույլ է տալիս ուղարկել հատուկ
  հարցումներ, դիտեք վերնագրերը և ստուգեք արձագանքման մարմինները:

Երկու վահանակներն էլ երևում են **պատկերի ընտրիչ**, որը կարդում է
`docs/portal/static/openapi/versions.json`. Լրացրեք այդ ցուցանիշը
`npm run sync-openapi -- --version=<label> --mirror=current --latest` այսպես
վերանայողները կարող են անցնել պատմական բնութագրերի միջև, տես SHA-256-ի գրանցված ամփոփագիրը,
և հաստատեք, թե արդյոք թողարկման նկարը նախքան օգտագործելը ունի ստորագրված մանիֆեստ
ինտերակտիվ վիդջեթներ:

Ցանկացած վիջեթում նշանը փոխելը ազդում է միայն բրաուզերի ընթացիկ աշխատաշրջանի վրա. որ
վստահված անձը երբեք չի պահպանում կամ գրանցում է մատակարարված նշանը:

## Կարճատև OAuth նշաններ

Որպեսզի չտարածեք երկարակյաց Torii ժետոնները գրախոսողներին, միացրեք «Փորձեք այն»
վահանակ ձեր OAuth սերվերին: Երբ առկա են ստորև նշված միջավայրի փոփոխականները
պորտալը տրամադրում է սարքի կոդ մուտքի վիդջեթ, կարճատև կրող նշաններ,
և ավտոմատ կերպով դրանք ներարկում է վահանակի ձևի մեջ:

| Փոփոխական | Նպատակը | Կանխադրված |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth Սարքի թույլտվության վերջնակետ (`/oauth/device/code`) | _դատարկ (հաշմանդամ)_ |
| `DOCS_OAUTH_TOKEN_URL` | Token վերջնական կետ, որն ընդունում է `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _դատարկ_ |
| `DOCS_OAUTH_CLIENT_ID` | OAuth հաճախորդի նույնացուցիչը գրանցված է փաստաթղթերի նախադիտման համար | _դատարկ_ |
| `DOCS_OAUTH_SCOPE` | Մուտքի ժամանակ պահանջվում են տարածքով սահմանազատված շրջանակներ | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Ընտրովի API լսարան՝ նշանը կապելու համար | _դատարկ_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Հաստատման սպասելիս հարցման նվազագույն միջակայքը (ms) | `5000` (<5000ms արժեքները մերժվում են) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Հետադարձ սարք-կոդի ժամկետի լրանալու պատուհան (վայրկյան) | `600` (պետք է մնա 300-ից 900-ականների միջև) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Հետադարձ հասանելիության նշանի կյանքի տևողությունը (վայրկյաններ) | `900` (պետք է մնա 300-ից 900-ականների միջև) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Սահմանել `1`՝ տեղական նախադիտումների համար, որոնք միտումնավոր բաց են թողնում OAuth-ի կիրարկումը | _չսահմանված_ |

Օրինակ կազմաձևում.

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

Երբ գործարկում եք `npm run start` կամ `npm run build`, պորտալը զետեղում է այս արժեքները
`docusaurus.config.js`-ում: Տեղական նախադիտման ժամանակ Try it քարտը ցույց է տալիս a
«Մուտք գործել սարքի կոդով» կոճակը: Օգտագործողները մուտքագրում են ցուցադրված կոդը ձեր OAuth-ում
ստուգման էջ; երբ սարքի հոսքը հաջողվի վիջեթին.

- ներարկում է թողարկված կրիչի նշանը Try it կոնսոլի դաշտում,
- պիտակավորել հարցումները գոյություն ունեցող `X-TryIt-Client` և `X-TryIt-Auth` վերնագրերով,
- ցուցադրում է մնացած կյանքի ժամկետը և
- ավտոմատ կերպով մաքրում է նշանը, երբ այն սպառվում է:

Ձեռքով Bearer մուտքագրումը մնում է հասանելի. ամեն անգամ բաց թողեք OAuth փոփոխականները
ցանկանում են ստիպել վերանայողներին տեղադրել իրենց ժամանակավոր նշանը կամ արտահանել
`DOCS_OAUTH_ALLOW_INSECURE=1` մեկուսացված տեղական նախադիտումների համար, որտեղ անանուն մուտք է գործում
ընդունելի է։ Այժմ առանց OAuth-ի կազմաձևված շինությունները արագ չեն բավարարում
DOCS-1b ճանապարհային քարտեզի դարպաս:

📌 Դիտեք [Անվտանգության կարծրացում և գրիչ-փորձարկման ստուգաթերթը] (./security-hardening.md)
նախքան պորտալը լաբորատորիայից դուրս բացահայտելը. այն փաստագրում է սպառնալիքի մոդելը,
CSP/Trusted Types պրոֆիլը և ներթափանցման փորձարկման քայլերը, որոնք այժմ մուտք են գործում DOCS-1b:

## Norito-RPC նմուշներ

Norito-RPC հարցումները կիսում են նույն պրոքսի և OAuth սանտեխնիկան, ինչ JSON երթուղիները,
նրանք պարզապես դնում են `Content-Type: application/x-norito` և ուղարկում են
նախապես կոդավորված Norito օգտակար բեռ, որը նկարագրված է NRPC-ի բնութագրում
(`docs/source/torii/nrpc_spec.md`):
Պահեստը առաքում է կանոնական բեռներ `fixtures/norito_rpc/` so պորտալի տակ
հեղինակները, SDK-ի սեփականատերերը և գրախոսողները կարող են վերարտադրել ճշգրիտ բայթերը, որոնք օգտագործում է CI-ն:

### Ուղարկեք Norito ծանրաբեռնվածություն Try It վահանակից

1. Ընտրեք այնպիսի սարք, ինչպիսին է `fixtures/norito_rpc/transfer_asset.norito`: Սրանք
   ֆայլերը հում Norito ծրարներ են; մի **մի** base64-կոդավորեք դրանք:
2. Swagger-ում կամ RapiDoc-ում գտնեք NRPC վերջնակետը (օրինակ
   `POST /v1/pipeline/submit`) և միացրեք **Content-Type** ընտրիչը
   `application/x-norito`.
3. Միացրեք հարցումի հիմնական խմբագրիչը **բինար** (Swagger-ի «Ֆայլ» ռեժիմը կամ
   RapiDoc-ի «Բինար/Ֆայլ» ընտրիչը) և վերբեռնեք `.norito` ֆայլը: Վիջեթը
   բայթերը փոխանցում է պրոքսիի միջոցով՝ առանց փոփոխության:
4. Ներկայացրե՛ք հարցումը: Եթե Torii-ը վերադարձնի `X-Iroha-Error-Code: schema_mismatch`,
   ստուգեք, որ զանգում եք վերջնակետ, որն ընդունում է երկուական ծանրաբեռնվածություն և
   հաստատեք, որ `fixtures/norito_rpc/schema_hashes.json`-ում գրանցված սխեմայի հեշը
   համապատասխանում է Torii կառուցվածքին, որը դուք հարվածում եք:

Վահանակը պահում է ամենավերջին ֆայլը հիշողության մեջ, որպեսզի կարողանաք նույնը նորից ուղարկել
ծանրաբեռնվածություն տարբեր թույլտվության նշաններ կամ Torii հոսթեր օգտագործելիս: Ավելացում
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`-ը ձեր աշխատանքային հոսքին արտադրում է
ապացույցների փաթեթը, որը նշված է NRPC-4-ի ընդունման պլանում (տեղեկամատյան + JSON ամփոփում),
որը հիանալի կերպով զուգորդվում է ակնարկների ժամանակ «Փորձիր այն» պատասխանի սքրինշոթինգի հետ:

### CLI օրինակ (գանգուր)

Նույն հարմարանքները կարող են վերարտադրվել պորտալից դուրս `curl`-ի միջոցով, ինչը օգտակար է
վստահված անձի կամ վրիպազերծման դարպասի պատասխանները վավերացնելիս.

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl \
  -H "Content-Type: application/x-norito" \
  -H "Authorization: ${TOKEN}" \
  --data-binary @fixtures/norito_rpc/transfer_asset.norito \
  "${TORII}/v1/pipeline/submit"
```

Փոխարինեք սարքը `transaction_fixtures.manifest.json`-ում նշված ցանկացած մուտքի հետ
կամ կոդավորեք ձեր սեփական ծանրաբեռնվածությունը `cargo xtask norito-rpc-fixtures`-ով: Երբ Torii
Կանարյան ռեժիմում է, կարող եք ուղղել `curl`-ը try-it վստահված անձի վրա
(`https://docs.sora.example/proxy/v1/pipeline/submit`) նույնը վարելու համար
ենթակառուցվածք, որն օգտագործում են պորտալի վիդջեթները:

## Դիտորդականություն և գործառնություններՅուրաքանչյուր հարցում գրանցվում է մեկ անգամ՝ մեթոդով, ուղով, սկզբնավորմամբ, հոսանքին հակառակ կարգավիճակով և
Նույնականացման աղբյուր (`override`, `default` կամ `client`): Նշանները երբեք չեն լինում
պահված. և՛ կրողի վերնագրերը, և՛ `X-TryIt-Auth` արժեքները խմբագրվել են նախկինում
logging — այնպես որ կարող եք stdout-ը փոխանցել կենտրոնական կոլեկցիոներ՝ առանց անհանգստանալու
գաղտնիքների արտահոսք.

### Առողջության հետաքննություն և ահազանգ

Գործարկեք փաթեթավորված զոնդը տեղակայման ժամանակ կամ ըստ ժամանակացույցի.

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Շրջակա միջավայրի բռնակներ.

- `TRYIT_PROXY_SAMPLE_PATH` — կամընտիր Torii երթուղի (առանց `/proxy`) մարզվելու համար:
- `TRYIT_PROXY_SAMPLE_METHOD` — կանխադրված է `GET`; սահմանել `POST`՝ գրելու երթուղիների համար:
- `TRYIT_PROXY_PROBE_TOKEN` — ներարկում է ժամանակավոր կրող նշան նմուշի զանգի համար:
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — անտեսում է լռելյայն 5 վրկ ժամանակի ավարտը:
- `TRYIT_PROXY_PROBE_METRICS_FILE` — կամընտիր Prometheus տեքստային ֆայլի նպատակակետ `probe_success`/`probe_duration_seconds`-ի համար:
- `TRYIT_PROXY_PROBE_LABELS` — ստորակետերով բաժանված `key=value` զույգերը կցված են չափիչներին (կանխադրված են `job=tryit-proxy` և `instance=<proxy URL>`):
- `TRYIT_PROXY_PROBE_METRICS_URL` — չափման կամընտիր վերջնակետի URL (օրինակ՝ `http://localhost:9798/metrics`), որը պետք է հաջողությամբ արձագանքի, երբ `TRYIT_PROXY_METRICS_LISTEN` միացված է:

Արդյունքները փոխանցեք տեքստային ֆայլերի կոլեկցիոներին՝ զոնդն ուղղելով գրվողի վրա
ուղին (օրինակ, `/var/lib/node_exporter/textfile_collector/tryit.prom`) և
ավելացնելով ցանկացած հատուկ պիտակ.

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

Սցենարը վերագրում է չափման ֆայլը ատոմային եղանակով, որպեսզի ձեր կոլեկցիոները միշտ կարդա ա
ամբողջական ծանրաբեռնվածություն.

Երբ `TRYIT_PROXY_METRICS_LISTEN` կազմաձևված է, դրեք
`TRYIT_PROXY_PROBE_METRICS_URL` չափումների վերջնակետին, որպեսզի զոնդն արագ ձախողվի
եթե քերծվածքի մակերեսը անհետանում է (օրինակ՝ սխալ կազմաձևված ներթափանցում կամ բացակայում է):
firewall կանոնները): Տիպիկ արտադրական պարամետրն է
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Թեթև ծանուցման համար զոնդը միացրեք ձեր մոնիտորինգի կույտին: A Prometheus
Օրինակ, որ էջերը երկու անընդմեջ ձախողումներից հետո.

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### Չափման վերջնակետ և վահանակներ

Նախկինում սահմանեք `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (կամ ցանկացած հյուրընկալող/պորտային զույգ):
սկսել վստահված սերվերը՝ բացահայտելու Prometheus ձևաչափով չափման վերջնակետը: Ճանապարհը
լռելյայն է `/metrics`, բայց կարող է անտեսվել միջոցով
`TRYIT_PROXY_METRICS_PATH=/custom`. Յուրաքանչյուր քերծվածք վերադարձնում է հաշվիչներ յուրաքանչյուր մեթոդի համար
հարցումների հանրագումարներ, տոկոսադրույքի սահմանաչափի մերժումներ, վերին հոսքի սխալներ/ժամկետներ, վստահված անձի արդյունքներ,
և հետաձգման ամփոփագրեր.

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Ուղղեք ձեր Prometheus/OTLP կոլեկտորները չափումների վերջնակետում և նորից օգտագործեք
գոյություն ունեցող `dashboards/grafana/docs_portal.json` վահանակներ, որպեսզի SRE-ն կարողանա դիտել պոչը
ուշացումներ և մերժման աճեր՝ առանց տեղեկամատյանների վերլուծության: Վստահված անձը ավտոմատ կերպով
հրատարակում է `tryit_proxy_start_timestamp_ms`՝ օգնելու օպերատորներին հայտնաբերել վերագործարկումները:

### Հետադարձ ավտոմատացում

Օգտագործեք կառավարման օգնականը՝ նպատակային Torii URL-ը թարմացնելու կամ վերականգնելու համար: Սցենարը
պահպանում է նախորդ կոնֆիգուրացիան `.env.tryit-proxy.bak`-ում, որպեսզի հետադարձումները լինեն
մեկ հրաման.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Անտեսեք env ֆայլի ուղին `--env` կամ `TRYIT_PROXY_ENV`, եթե ձեր տեղակայումը
պահպանում է կոնֆիգուրացիան այլուր: