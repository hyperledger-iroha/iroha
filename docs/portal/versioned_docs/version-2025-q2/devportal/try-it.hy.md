---
lang: hy
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-12-29T18:16:35.904297+00:00"
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
| `TRYIT_PROXY_BEARER` | Կանխադրված կրիչի նշանն ուղարկվել է Torii | _դատարկ_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Թույլատրել վերջնական օգտագործողներին մատակարարել իրենց սեփական նշանը `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Հարցման մարմնի առավելագույն չափը (բայթ) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Վերին հոսքի ժամանակի վերջնաժամկետը միլիվայրկյաններով | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Հայտերը թույլատրվում են ըստ տոկոսադրույքի պատուհանի մեկ հաճախորդի IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Սահող պատուհան արագության սահմանափակման համար (ms) | `60000` |

Վստահված սերվերը նաև բացահայտում է `GET /healthz`-ը, վերադարձնում է JSON-ի կառուցվածքային սխալները և
խմբագրում է կրիչի նշանները տեղեկամատյանների ելքից:

## Գործարկեք վստահված անձը տեղում

Տեղադրեք կախվածությունները, երբ առաջին անգամ ստեղծեք պորտալը.

```bash
cd docs/portal
npm install
```

Գործարկեք վստահված անձը և այն ուղղեք ձեր Torii օրինակին.

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Սցենարը գրանցում է կապված հասցեն և ուղարկում հարցումները `/proxy/*`-ից դեպի
կազմաձևված Torii ծագում:

## Միացրեք պորտալի վիդջեթները

Երբ դուք կառուցում կամ սպասարկում եք մշակողների պորտալը, սահմանեք վիջեթների URL-ը
վստահված անձի համար պետք է օգտագործվի՝

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Հետևյալ բաղադրիչները կարդում են այս արժեքները `docusaurus.config.js`-ից.

- **Swagger UI** — մատուցված է `/reference/torii-swagger`-ում; օգտագործում է հարցումը
  interceptor՝ ավտոմատ կերպով կրող նշանները կցելու համար:
- **RapiDoc** — մատուցված է `/reference/torii-rapidoc`; արտացոլում է նշանային դաշտը
  և աջակցում է վստահված անձի դեմ փորձելու հարցումները:
- **Փորձեք այն կոնսոլից** — ներկառուցված է API-ի ակնարկ էջում; թույլ է տալիս ուղարկել հատուկ
  հարցումներ, դիտեք վերնագրերը և ստուգեք արձագանքման մարմինները:

Ցանկացած վիջեթում նշանը փոխելը ազդում է միայն բրաուզերի ընթացիկ աշխատաշրջանի վրա. որ
վստահված անձը երբեք չի պահպանում կամ գրանցում է մատակարարված նշանը:

## Դիտորդականություն և գործառնություններ

Յուրաքանչյուր հարցում գրանցվում է մեկ անգամ՝ մեթոդով, ուղով, սկզբնավորմամբ, հոսանքին հակառակ կարգավիճակով և
Նույնականացման աղբյուր (`override`, `default` կամ `client`): Նշանները երբեք չեն լինում
պահված. և՛ կրողի վերնագրերը, և՛ `X-TryIt-Auth` արժեքները խմբագրվել են նախկինում
logging — այնպես որ կարող եք stdout-ը փոխանցել կենտրոնական կոլեկցիոներ՝ առանց անհանգստանալու
գաղտնիքների արտահոսք.

### Առողջության հետաքննություն և ահազանգԳործարկեք փաթեթավորված զոնդը տեղակայման ժամանակ կամ ըստ ժամանակացույցի.

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