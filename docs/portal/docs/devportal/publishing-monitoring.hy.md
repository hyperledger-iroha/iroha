---
lang: hy
direction: ltr
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f68e8cc639bd6a780c33fd14ab4e25df1c6e9381595c7a4c44ff577fea02400d
source_last_modified: "2026-01-22T16:26:46.494851+00:00"
translation_last_reviewed: 2026-02-07
id: publishing-monitoring
title: SoraFS Publishing & Monitoring
sidebar_label: Publishing & Monitoring
description: Capture the end-to-end monitoring flow for SoraFS portal releases so DOCS-3c has deterministic probes, telemetry, and evidence bundles.
translator: machine-google-reviewed
---

Ճանապարհային քարտեզի **DOCS-3c** կետը պահանջում է ավելին, քան փաթեթավորման ստուգաթերթ. ամեն անգամ
SoraFS հրապարակել, մենք պետք է շարունակաբար ապացուցենք, որ մշակողների պորտալը, փորձեք այն
proxy-ը և gateway կապերը մնում են առողջ: Այս էջը փաստում է մոնիտորինգը
մակերեսը, որն ուղեկցում է [տեղակայման ուղեցույցին] (./deploy-guide.md), ուստի CI և
Զանգահարող ինժեներները կարող են իրականացնել նույն ստուգումները, որոնք Ops-ն օգտագործում է SLO-ն կիրառելու համար:

## Խողովակաշարի ամփոփում

1. **Կառուցեք և ստորագրեք** – գործարկելու համար հետևեք [տեղակայման ուղեցույցին] (./deploy-guide.md)
   `npm run build`, `scripts/preview_wave_preflight.sh` և Sigstore +
   մանիֆեստի ներկայացման քայլերը. Նախնական թռիչքի սցենարը թողարկում է `preflight-summary.json`
   այնպես որ յուրաքանչյուր նախադիտում կրում է build/link/probe մետատվյալներ:
2. **Կցեք և հաստատեք** – `sorafs_cli manifest submit`, `cargo xtask soradns-verify-binding`,
   և DNS-ի կտրման պլանը ապահովում է դետերմինիստական արտեֆակտներ կառավարման համար:
3. **Արխիվային ապացույց** – պահեք CAR ամփոփագիրը, Sigstore փաթեթը, կեղծանունի ապացույցը,
   զոնդի ելք, և `docs_portal.json` վահանակի նկարները տակ
   `artifacts/sorafs/<tag>/`.

## Մոնիտորինգի ալիքներ

### 1. Հրապարակող մոնիտորներ (`scripts/monitor-publishing.mjs`)

Նոր `npm run monitor:publishing` հրամանը փաթաթում է պորտալի զոնդը, փորձեք այն
վստահված անձի հետաքննություն և պարտադիր ստուգիչ մեկ CI-ընկերական ստուգման մեջ: Տրամադրել ա
JSON կազմաձև (ստուգվել է CI գաղտնիքներում կամ `configs/docs_monitor.json`) և գործարկել՝

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

Ավելացնել `--prom-out ../../artifacts/docs_monitor/monitor.prom` (և ըստ ցանկության
`--prom-job docs-preview`) արտանետելու Prometheus տեքստային ձևաչափի չափումներ, որոնք հարմար են
Pushgateway-ի վերբեռնումներ կամ ուղղակի Prometheus քերծվածքներ բեմադրության/արտադրության մեջ: Այն
Չափիչները արտացոլում են JSON-ի ամփոփագիրը, որպեսզի կարողանան հետևել SLO վահանակներին և զգուշացման կանոններին
պորտալ, Փորձեք, պարտադիր և DNS առողջություն՝ առանց ապացույցների փաթեթը վերլուծելու:

Կազմաձևի օրինակ՝ անհրաժեշտ բռնակներով և մի քանի կապակցումներով.

```json
{
  "portal": {
    "baseUrl": "https://docs-preview.sora.link",
    "paths": ["/", "/devportal/try-it", "/reference/torii-swagger"],
    "expectRelease": "preview-2026-02-14",
    "checkSecurity": true,
    "expectedSecurity": {
      "csp": "default-src 'self'; connect-src https://tryit-preview.sora",
      "permissionsPolicy": "fullscreen=()",
      "referrerPolicy": "strict-origin-when-cross-origin"
    }
  },
  "tryIt": {
    "proxyUrl": "https://tryit-preview.sora",
    "samplePath": "/proxy/v1/accounts/ih58.../assets?limit=1",
    "method": "GET",
    "timeoutMs": 7000,
    "token": "${TRYIT_BEARER}",
    "metricsUrl": "https://tryit-preview.sora/metrics"
  },
  "bindings": [
    {
      "label": "portal",
      "bindingPath": "../../artifacts/sorafs/portal.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal.manifest.json"
    },
    {
      "label": "openapi",
      "bindingPath": "../../artifacts/sorafs/openapi.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/openapi.manifest.json"
    },
    {
      "label": "portal-sbom",
      "bindingPath": "../../artifacts/sorafs/portal-sbom.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal-sbom.manifest.json"
    }
  ],

  "dns": [
    {
      "label": "docs-preview CNAME",
      "hostname": "docs-preview.sora.link",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    },
    {
      "label": "docs-preview canonical",
      "hostname": "igjssx53t4ayu3d5qus5o6xtp2f5dvka5rewr6xgscpmh3x4io4q.gw.sora.id",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    }
  ]
}
```

Մոնիտորը գրում է JSON ամփոփագիր (S3/SoraFS բարեկամական) և դուրս է գալիս զրոյից, երբ
ցանկացած զոնդ ձախողվում է՝ դարձնելով այն հարմար Cron աշխատանքների, Buildkite քայլերի կամ
Alertmanager webhooks. `--evidence-dir` անցնելը պահպանվում է `summary.json`,
`portal.json`, `tryit.json` և `binding.json` `checksums.sha256`-ի կողքին
դրսևորվում է այնպես, որ կառավարման վերանայողները կարող են տարբերել մոնիտորինգի արդյունքներն առանց դրա
նորից գործարկել զոնդերը:

> **TLS պաշտպանիչ բազրիք.
> `allowInsecureHttp: true` կոնֆիգուրայում: Շարունակեք միացված արտադրական/բեմադրական զոնդերը
> HTTPS; ընտրությունը գործում է բացառապես տեղական նախադիտումների համար:

Յուրաքանչյուր պարտադիր մուտքագրվում է `cargo xtask soradns-verify-binding` գրավվածի դեմ
`portal.gateway.binding.json` փաթեթ (և կամընտիր `manifestJson`) այսպես այլաբանություն,
ապացույցի կարգավիճակը և CID-ի բովանդակությունը համահունչ են հրապարակված ապացույցներին: Այն
կամընտիր `hostname` պահակը հաստատում է, որ կեղծանունից ստացված կանոնական հոսթը համապատասխանում է
gateway host-ը, որը դուք մտադիր եք խթանել՝ կանխելով DNS-ի կտրվածքները, որոնք շեղվում են դրանից
արձանագրված պարտադիր.

Լրացուցիչ `dns` բլոկը միացնում է DOCS-7-ի SoraDNS-ը նույն մոնիտորի մեջ:
Յուրաքանչյուր գրառում լուծում է հյուրընկալողի անուն/գրառման տիպի զույգ (օրինակ՝
`docs-preview.sora.link` → `docs-preview.sora.link.gw.sora.name` CNAME) և
հաստատում է, որ պատասխանները համապատասխանում են `expectedRecords` կամ `expectedIncludes`: Երկրորդը
վերևի հատվածի մուտքը կոշտ կոդավորում է կանոնական հեշավորված հոսթի անունը, որը արտադրվել է
`cargo xtask soradns-hosts --name docs-preview.sora.link`; մոնիտորն այժմ ապացուցում է
և՛ մարդու համար հարմար այլանունը, և՛ կանոնական հեշը (`igjssx53…gw.sora.id`)
վճռել ամրացված գեղեցիկ հյուրընկալողին: Սա ավտոմատացնում է DNS խթանման ապացույցները.
մոնիտորը կձախողվի, եթե հյուրընկալողներից որևէ մեկը շեղվի, նույնիսկ այն դեպքում, երբ HTTP-ը դեռ միանում է
կեռ ճիշտ մանիֆեստը:

### 2. OpenAPI տարբերակի մանիֆեստի պահակ

DOCS-2b-ի «ստորագրված OpenAPI մանիֆեստի» պահանջն այժմ ուղարկում է ավտոմատացված պահակ.
`ci/check_openapi_spec.sh`-ը զանգահարում է `npm run check:openapi-versions`, որը կանչում է
`scripts/verify-openapi-versions.mjs`՝ խաչաձև ստուգելու համար
`docs/portal/static/openapi/versions.json` իրական Torii բնութագրերով և
դրսևորվում է. Պահակը հաստատում է, որ.

- `versions.json`-ում թվարկված յուրաքանչյուր տարբերակ ունի համապատասխան գրացուցակ տակ
  `static/openapi/versions/`.
- Յուրաքանչյուր մուտքի `bytes` և `sha256` դաշտերը համընկնում են սկավառակի վրա տեղադրված հատուկ ֆայլի հետ:
- `latest` կեղծանունը արտացոլում է `current` մուտքը (շարադրանք/չափ/ստորագրության մետատվյալներ)
  այնպես որ լռելյայն ներբեռնումը չի կարող շեղվել:
- Ստորագրված գրառումները հղում են անում մանիֆեստին, որի `artifact.path`-ը ցույց է տալիս դեպի
  նույն հատկանիշը և որի ստորագրության/հրապարակային բանալու վեցանկյուն արժեքները համապատասխանում են մանիֆեստին:

Գործարկեք պահակախումբը տեղում, երբ դուք արտացոլում եք նոր առանձնահատկություն.

```bash
cd docs/portal
npm run check:openapi-versions
```

Անհաջող հաղորդագրությունները ներառում են հնացած ֆայլի հուշում (`npm run sync-openapi -- --latest`)
այնպես որ պորտալի մասնակիցները գիտեն, թե ինչպես թարմացնել նկարները: Պահակին ներս պահելը
CI-ն կանխում է պորտալի թողարկումները, որտեղ ստորագրված մանիֆեստը և հրապարակված ամփոփումը
ընկնել համաժամանակյաց.

### 2. Վահանակներ և ծանուցումներ

- **`dashboards/grafana/docs_portal.json`** – հիմնական տախտակ DOCS-3c-ի համար: Վահանակներ
  Հետևել `torii_sorafs_gateway_refusals_total`, SLA-ի կրկնօրինակումը բացակայում է, փորձեք
  վստահված անձի սխալներ և հետաքննության ուշացում (`docs.preview.integrity` ծածկույթ): Արտահանել
  յուրաքանչյուր թողարկումից հետո նստեք և կցեք այն գործառնական տոմսին:
- **Փորձեք վստահված անձի ծանուցումները** – Գործում է Alertmanager կանոնը `TryItProxyErrors`
  կայուն `probe_success{job="tryit-proxy"}` կաթիլներ կամ
  `tryit_proxy_requests_total{status="error"}` հասկ:
- **Gateway SLO** – `DocsPortal/GatewayRefusals`-ն ապահովում է կեղծանունների կապերի շարունակությունը
  ամրացված մանիֆեստը գովազդելու համար; էսկալացիաները կապում են
  `cargo xtask soradns-verify-binding` CLI սղագրությունը նկարահանվել է հրապարակման ժամանակ:

### 3. Ապացույցների հետք

Մոնիտորինգի յուրաքանչյուր գործարկում պետք է կցվի.

- `monitor-publishing` ապացույցների փաթեթ (`summary.json`, յուրաքանչյուր հատվածի ֆայլեր և
  `checksums.sha256`):
- Grafana սքրինշոթներ `docs_portal` տախտակի համար թողարկման պատուհանի վրա:
- Փորձեք այն վստահված անձի փոփոխության/վերադարձի տառադարձումները (`npm run manage:tryit-proxy` տեղեկամատյաններ):
- Այլանունների ստուգման ելք `cargo xtask soradns-verify-binding`-ից:

Պահեք դրանք `artifacts/sorafs/<tag>/monitoring/`-ի տակ և միացրեք դրանք
թողարկման խնդիր, որպեսզի աուդիտի հետքը պահպանվի CI տեղեկամատյանների ժամկետի ավարտից հետո:

## Գործառնական ստուգաթերթ

1. Գործարկեք տեղակայման ուղեցույցը Քայլ 7-ի միջոցով:
2. Կատարել `npm run monitor:publishing` արտադրական կոնֆիգուրացիայով; արխիվ
   JSON ելքը:
3. Գրավել Grafana վահանակներ (`docs_portal`, `TryItProxyErrors`,
   `DocsPortal/GatewayRefusals`) և կցեք դրանք թողարկման տոմսին:
4. Պլանավորեք կրկնվող մոնիտորները (խորհուրդ է տրվում՝ յուրաքանչյուր 15 րոպեն մեկ), որոնք ուղղված են դեպի
   արտադրության URL-ներ՝ նույն կազմաձևով, որպեսզի բավարարեն DOCS-3c SLO դարպասը:
5. Միջադեպերի ժամանակ կրկին գործարկեք մոնիտորի հրամանը `--json-out`-ով ձայնագրելու համար
   ապացույցներից առաջ/հետո և կցել այն հետմահու:

Այս օղակից հետո փակվում է DOCS-3c-ը. պորտալի կառուցման հոսքը, հրապարակման խողովակաշարը,
և մոնիտորինգի կույտը այժմ ապրում է մեկ գրքում՝ վերարտադրվող հրամաններով,
նմուշի կոնֆիգուրացիաներ և հեռաչափության կեռիկներ: