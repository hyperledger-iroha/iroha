---
lang: hy
direction: ltr
source: docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2025-12-29T18:16:35.067551+00:00"
translation_last_reviewed: 2026-02-07
title: Try It Sandbox Guide
summary: How to run the Torii staging proxy and developer portal sandbox.
translator: machine-google-reviewed
---

Մշակողների պորտալը առաքում է «Փորձեք» վահանակը Torii REST API-ի համար: Այս ուղեցույցը
բացատրում է, թե ինչպես գործարկել օժանդակ պրոքսին և միացնել վահանակը բեմադրությանը
դարպաս՝ առանց հավատարմագրերը բացահայտելու:

## Նախադրյալներ

- Iroha պահեստի վճարում (աշխատանքային տարածքի արմատ):
- Node.js 18.18+ (համապատասխանում է պորտալի բազային):
- Torii վերջնակետը հասանելի է ձեր աշխատակայանից (բեմական կամ տեղական):

## 1. Ստեղծեք OpenAPI լուսանկարը (ըստ ցանկության)

Վահանակը նորից օգտագործում է նույն OpenAPI բեռնվածությունը, ինչպես պորտալի տեղեկատու էջերը: Եթե
դուք փոխել եք Torii երթուղիները, վերականգնեք նկարը.

```bash
cargo xtask openapi
```

Առաջադրանքը գրում է `docs/portal/static/openapi/torii.json`:

## 2. Սկսեք «Փորձեք այն» վստահված անձը

Պահեստի արմատից.

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### Շրջակա միջավայրի փոփոխականներ

| Փոփոխական | Նկարագրություն |
|----------|-------------|
| `TRYIT_PROXY_TARGET` | Torii բազային URL (պարտադիր է): |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Ստորակետերով բաժանված ծագման ցուցակը, որը թույլատրվում է օգտագործել պրոքսի (կանխադրված է `http://localhost:3000`): |
| `TRYIT_PROXY_BEARER` | Լրացուցիչ լռելյայն կրիչի նշանը կիրառվում է բոլոր վստահված հարցումների համար: |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Սահմանեք `1`՝ զանգահարողի `Authorization` վերնագիրը բառացիորեն փոխանցելու համար: |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | Հիշողության արագության սահմանափակիչի կարգավորումներ (կանխադրված՝ 60 հարցում 60 վայրկյանում): |
| `TRYIT_PROXY_MAX_BODY` | Ընդունված առավելագույն հայտի օգտակար բեռը (բայթ, լռելյայն 1 ՄԲ): |
| `TRYIT_PROXY_TIMEOUT_MS` | Torii հարցումների վերին հոսքի ժամանակի ավարտ (կանխադրված 10000 մվ): |

Վստահված անձը բացահայտում է.

- `GET /healthz` — պատրաստության ստուգում:
- `/proxy/*` — վստահված հարցումներ՝ պահպանելով ուղին և հարցումների տողը:

## 3. Գործարկել պորտալը

Առանձին տերմինալում.

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

Այցելեք `http://localhost:3000/api/overview` և օգտագործեք Try It վահանակը: Նույնը
շրջակա միջավայրի փոփոխականները կարգավորում են Swagger UI և RapiDoc ներկառուցումները:

## 4. Գործող միավորի թեստեր

Վստահված անձը բացահայտում է արագ հանգույցի վրա հիմնված թեստային փաթեթը.

```bash
npm run test:tryit-proxy
```

Թեստերը ներառում են հասցեների վերլուծություն, ծագման մշակում, տոկոսադրույքի սահմանափակում և կրող
ներարկում.

## 5. Զոնդերի ավտոմատացում և չափումներ

Օգտագործեք փաթեթավորված զոնդը՝ `/healthz`-ը և նմուշի վերջնական կետը ստուգելու համար.

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Շրջակա միջավայրի բռնակներ.

- `TRYIT_PROXY_SAMPLE_PATH` — կամընտիր Torii երթուղի (առանց `/proxy`) մարզվելու համար:
- `TRYIT_PROXY_SAMPLE_METHOD` — կանխադրված է `GET`; սահմանել `POST`՝ գրելու երթուղիների համար:
- `TRYIT_PROXY_PROBE_TOKEN` — ներարկում է ժամանակավոր կրող նշան նմուշի զանգի համար:
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — անտեսում է լռելյայն 5 վրկ ժամանակի ավարտը:
- `TRYIT_PROXY_PROBE_METRICS_FILE` — Prometheus տեքստային ֆայլի նպատակակետ `probe_success`/`probe_duration_seconds`-ի համար:
- `TRYIT_PROXY_PROBE_LABELS` — ստորակետերով բաժանված `key=value` զույգերը կցված են չափիչներին (կանխադրված են `job=tryit-proxy` և `instance=<proxy URL>`):

Երբ `TRYIT_PROXY_PROBE_METRICS_FILE`-ը դրված է, սցենարը վերագրում է ֆայլը
ատոմային կերպով, այնպես որ ձեր node_exporter/textfile հավաքողը միշտ տեսնում է ամբողջական
օգտակար բեռ. Օրինակ՝

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

Ստացված չափումները փոխանցեք Prometheus-ին և նորից օգտագործեք նմուշի ահազանգը
ծրագրավորող-պորտալի փաստաթղթերը դեպի էջ, երբ `probe_success`-ն իջնում է մինչև `0`:

## 6. Արտադրության կարծրացման ստուգաթերթ

Մինչև տեղական զարգացումից դուրս վստահված անձի հրապարակումը.

- Դադարեցրեք TLS-ը վստահված անձից առաջ (հակադարձ վստահված անձ կամ կառավարվող դարպաս):
- Կազմաձևեք կառուցվածքային անտառահատումները և փոխանցեք դիտարկելիության խողովակաշարերին:
- Պտտեք կրիչի նշանները և պահեք դրանք ձեր գաղտնիքների կառավարիչում:
- Վերահսկեք վստահված անձի `/healthz` վերջնակետը և ընդհանուր հետաձգման չափումները:
- Հավասարեցրեք տոկոսադրույքի սահմանները ձեր Torii բեմականացման քվոտաների հետ; հարմարեցնել `Retry-After`-ը
  վարքագիծը հաճախորդներին շոշափելու համար: