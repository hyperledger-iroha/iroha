---
lang: kk
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

Жол картасының **DOCS-3c** тармағы қаптаманы тексеру парағын ғана қажет етеді: әрбір кейін
SoraFS жариялау біз әзірлеуші порталы екенін үнемі дәлелдеуіміз керек, Байқап көріңіз
прокси және шлюз байланыстары сау болып қалады. Бұл бетте мониторинг құжатталады
[орналастыру нұсқаулығымен](./deploy-guide.md) CI және
қоңырау инженерлері Ops SLO орындау үшін пайдаланатын бірдей тексерулерді орындай алады.

## Құбырларды қорытындылау

1. **Құрастыру және қол қою** – іске қосу үшін [орналастыру нұсқаулығын](./deploy-guide.md) орындаңыз
   `npm run build`, `scripts/preview_wave_preflight.sh` және Sigstore +
   манифестті жіберу қадамдары. Алдын ала ұшу сценарийі `preflight-summary.json` шығарады
   сондықтан әрбір алдын ала қарау құрастыру/сілтеме/зерттеу метадеректерін тасымалдайды.
2. **Тіреуіш және растау** – `sorafs_cli manifest submit`, `cargo xtask soradns-verify-binding`,
   және DNS кесу жоспары басқару үшін детерминирленген артефактілерді қамтамасыз етеді.
3. **Архивтік дәлелдер** – CAR қысқаша мазмұнын, Sigstore бумасын, бүркеншік атын дәлелдеу,
   зонд шығысы және `docs_portal.json` бақылау тақтасының суреттері
   `artifacts/sorafs/<tag>/`.

## Бақылау арналары

### 1. Баспа мониторлары (`scripts/monitor-publishing.mjs`)

Жаңа `npm run monitor:publishing` пәрмені портал зондын орап, Көріп көріңіз
прокси зонды және CI-ға қолайлы бір тексеруге байланыстыру. А қамтамасыз етіңіз
JSON конфигурациясы (CI құпияларында немесе `configs/docs_monitor.json` тексерілген) және іске қосыңыз:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

`--prom-out ../../artifacts/docs_monitor/monitor.prom` қосыңыз (және қосымша
`--prom-job docs-preview`) үшін қолайлы Prometheus мәтіндік пішім көрсеткіштерін шығару үшін
Pushgateway жүктеп салулары немесе қою/өндірістегі тікелей Prometheus сызаттар. The
метрика JSON қорытындысын көрсетеді, осылайша SLO бақылау тақталары мен ескерту ережелері бақыланады
портал, Байқап көру, байланыстыру және DNS денсаулығы дәлелдер бумасын талдаусыз.

Қажетті тұтқалары мен бірнеше байлаулары бар конфигурацияның мысалы:

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
    "samplePath": "/proxy/v1/accounts/<katakana-i105-account-id>/assets?limit=1",
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

Монитор JSON қорытындысын жазады (S3/SoraFS қолайлы) және нөлден басқа кезде шығады.
кез келген зонд сәтсіз аяқталады, бұл оны Cron тапсырмаларына, Buildkite қадамдарына немесе
Alertmanager веб-гуктары. `--evidence-dir` өту сақталады `summary.json`,
`portal.json`, `tryit.json` және `binding.json` және `checksums.sha256`
манифест, сондықтан басқаруды тексерушілер монитор нәтижелерін қажетсіз өзгерте алады
зондтарды қайта іске қосыңыз.

> **TLS қоршауы:** `monitorPortal` сіз орнатпасаңыз, `http://` негізгі URL мекенжайларын қабылдамайды.
> конфигурацияда `allowInsecureHttp: true`. Өндіріс/кезеңдеу зондтарын қосулы ұстаңыз
> HTTPS; қосылу тек жергілікті алдын ала қарау үшін бар.

Әрбір байланыстыру жазбасы түсірілгенге қарсы `cargo xtask soradns-verify-binding` іске қосады
`portal.gateway.binding.json` жинағы (және қосымша `manifestJson`), яғни бүркеншік ат,
дәлелдеу күйі және мазмұн CID жарияланған дәлелдермен сәйкес келеді. The
қосымша `hostname` қорғаушысы бүркеншік атпен алынған канондық хосттың келесіге сәйкес келетінін растайды.
Сіз алға жылжытқыңыз келетін шлюз хостын, DNS үзінділерінен ауытқып кетуді болдырмайды
жазылған байлау.

Қосымша `dns` блок сымдары DOCS-7 SoraDNS шығарылымын бір мониторға қосады.
Әрбір жазба хост атауы/жазба түрінің жұбын шешеді (мысалы
`docs-preview.sora.link` → `docs-preview.sora.link.gw.sora.name` CNAME) және
жауаптардың сәйкестігін растайды `expectedRecords` немесе `expectedIncludes`. Екінші
жоғарыдағы үзіндідегі жазба қатты кодтар арқылы жасалған канондық хэштелген хост атауы
`cargo xtask soradns-hosts --name docs-preview.sora.link`; монитор енді дәлелдейді
адамға қолайлы бүркеншік ат пен канондық хэш (`igjssx53…gw.sora.id`)
бекітілген әдемі хостқа шешіңіз. Бұл DNS жылжыту дәлелдерін автоматты етеді:
HTTP қосылымдары әлі де болса да, хосттың дрейфі болса, монитор істен шығады
дұрыс манифестті бекітіңіз.

### 2. OpenAPI нұсқасы манифест қорғаушысы

DOCS-2b «қол қойылған OpenAPI манифесті» талабы енді автоматтандырылған қорғанысты жібереді:
`ci/check_openapi_spec.sh` `npm run check:openapi-versions` шақырады, ол шақырады
Қайта тексеру үшін `scripts/verify-openapi-versions.mjs`
`docs/portal/static/openapi/versions.json` нақты Torii сипаттамаларымен және
көрсетеді. Күзетші мынаны тексереді:

- `versions.json` тізімінде тізімделген әрбір нұсқаның астында сәйкес каталогы бар
  `static/openapi/versions/`.
- Әрбір жазбаның `bytes` және `sha256` өрістері дискідегі арнайы файлға сәйкес келеді.
- `latest` бүркеншік аты `current` жазбасын көрсетеді (дайджест/өлшем/қолтаңба метадеректері)
  сондықтан әдепкі жүктеп алу дрейфке айналмайды.
- Қол қойылған жазбалар `artifact.path` кері нұсқайтын манифестке сілтеме жасайды.
  бірдей спецификация және қолтаңба/ашық кілт он алтылық мәндері манифестке сәйкес келеді.

Жаңа спецификацияны қайталаған сайын қорғауды жергілікті түрде іске қосыңыз:

```bash
cd docs/portal
npm run check:openapi-versions
```

Сәтсіздік туралы хабарлар ескірген файл туралы кеңесті қамтиды (`npm run sync-openapi -- --latest`)
сондықтан портал үлескерлері суреттерді жаңарту жолын біледі. Күзетшіні іште ұстау
CI қол қойылған манифест пен жарияланған дайджестте портал шығарылымдарына жол бермейді
синхрондаудан шығу.

### 2. Бақылау тақталары және ескертулер

- **`dashboards/grafana/docs_portal.json`** – DOCS-3c үшін негізгі тақта. Панельдер
  трек `torii_sorafs_gateway_refusals_total`, репликация SLA жіберіп алмайды, Байқап көріңіз
  прокси қателері және тексеру кідірісі (`docs.preview.integrity` қабаттасуы). экспорттау
  әр шығарылымнан кейін тақтаға салып, оны операциялық билетке бекітіңіз.
- **Прокси ескертулерін қолданып көріңіз** – Alertmanager ережесі `TryItProxyErrors` іске қосылады
  тұрақты `probe_success{job="tryit-proxy"}` тамшылары немесе
  `tryit_proxy_requests_total{status="error"}` ұштары.
- **Gateway SLO** – `DocsPortal/GatewayRefusals` бүркеншік атпен байланыстыру жалғасуын қамтамасыз етеді
  бекітілген манифест дайджестін жарнамалауға; эскалациялар сілтемесі
  `cargo xtask soradns-verify-binding` CLI транскрипті жариялау кезінде түсірілді.

### 3. Дәлелдеу ізі

Әрбір мониторингті іске қосу керек:

- `monitor-publishing` дәлелдер жинағы (`summary.json`, әр бөлім файлдары және
  `checksums.sha256`).
- `docs_portal` тақтасына арналған Grafana скриншоттары шығару терезесінің үстінде.
- Проксиді өзгерту/қайтару транскрипттерін қолданып көріңіз (`npm run manage:tryit-proxy` журналдары).
- `cargo xtask soradns-verify-binding` бастап бүркеншік аттың растау шығысы.

Оларды `artifacts/sorafs/<tag>/monitoring/` астында сақтаңыз және оларды мына жерде байланыстырыңыз
CI журналдарының мерзімі біткеннен кейін аудит ізі сақталуы үшін шығару мәселесі.

## Операциялық бақылау парағы

1. Қолдану нұсқаулығын 7-қадам арқылы іске қосыңыз.
2. Өндірістік конфигурациямен `npm run monitor:publishing` орындаңыз; мұрағат
   JSON шығысы.
3. Grafana панельдерін түсіріңіз (`docs_portal`, `TryItProxyErrors`,
   `DocsPortal/GatewayRefusals`) және оларды шығару билетіне тіркеңіз.
4. Қайталанатын мониторларды (ұсынылады: әр 15 минут сайын)
   DOCS-3c SLO қақпасын қанағаттандыру үшін бірдей конфигурациялы өндірістік URL мекенжайлары.
5. Оқиғалар кезінде жазу үшін `--json-out` көмегімен монитор пәрменін қайта іске қосыңыз.
   дәлелдемеге дейін/кейін және оны өлгеннен кейін тіркеңіз.

Осы циклден кейін DOCS-3c жабылады: порталды құру ағыны, жариялау құбыры,
және мониторинг стек енді қайталанатын пәрмендері бар бір ойын кітабында тұрады,
үлгі конфигурациялары және телеметриялық ілгектер.