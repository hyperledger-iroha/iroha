---
lang: ba
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

Юл картаһы әйбер **DOCS-3c** упаковка тикшерелгән исемлектән күберәк талап итә: һәр һуң
I18NT000000008X баҫтырып сығарыу беҙ даими иҫбатларға тейеш, тип төҙөүсе порталы, Тырышып ҡарағыҙ
прокси, һәм шлюз бәйләүҙәре һау-сәләмәт булып ҡала. Был биттә мониторинг документы .
ер өҫтө, тип оҙатып [йөкмәтке етәксеһе] (./deploy-guide.md) шулай CI һәм өҫтөндә
шылтыратыу инженерҙары шул уҡ тикшерергә мөмкин, тип Ops ҡуллана, SLO үтәү өсөн.

## Торбалы рекап

1. **Төҙөү һәм ҡултамға** – үтәргә [йөкмәткеле етәксе](./deploy-guide.md) йүгерергә .
   `npm run build`, `scripts/preview_wave_preflight.sh`, һәм Sigstore +
   тапшырыу аҙымдарын күрһәтә. Осоу алдынан сценарий I18NI000000018X сығара.
   тимәк, һәр алдан ҡарау тота төҙөү/һылтанма/зонд метамағлүмәттәр.
2. **Пин һәм раҫлау** – I18NI000000019X, `cargo xtask soradns-verify-binding`,
   һәм DNS cutover планы идара итеү өсөн детерминистик артефакттар бирә.
3. **Архив дәлилдәре** – һаҡлау CAR резюме, I18NT00000000003Х өйөм, псевдоним иҫбатлау,
   зонд етештереү, һәм I18NI000000021X приборҙар таҡтаһы снимоктар аҫтында
   `artifacts/sorafs/<tag>/`.

## Мониторинг каналдары

### 1. Баҫма мониторҙар (I18NI000000023X)

Яңы I18NI000000024X командаһы порталь зондты урап, һынап ҡарағыҙ
прокси-зонд, һәм бәйләүсе тикшерелгән бер CI-дуҫ тикшерергә. А тәьмин итеү а .
JSON конфиг (CI серҙәренә йәки I18NI000000025X X) һәм йүгерә:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

Өҫтәү I18NI000000026X (һәм теләк буйынса
I18NI000000027X) I18NT000000000000000000 өсөн яраҡлы текст-формат метрикаларын сығарыу өсөн.
Pushgateway тейәү йәки туранан-тура I18NT0000000001X скраптар сәхнәләштереү/етештереү. 1990 й.
метрика көҙгө JSON резюме шулай SLO приборҙар таҡталары һәм иҫкәртмә ҡағиҙәләрен күҙәтә ала
портал, Һынап ҡарағыҙ, бәйләү, һәм DNS һаулыҡ анализһыҙ дәлилдәр өйөм.

Миҫал өсөн кәрәкле ручкалар һәм күп тапҡыр бәйләүҙәр:

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

Монитор JSON резюмеһы яҙа (S3/I18NT0000000009X дуҫ) һәм нульдән нульдән сыға.
теләһә ниндәй зонд уңышһыҙлыҡҡа осрай, уны Крон эш урындары өсөн яраҡлы итә, Bublekite аҙымдары йәки
Иҫкәртмәнсе webhooks. `--evidence-dir` үткән `summary.json`, 1990 й.
I18NI0000000030X, `tryit.json`, һәм I18NI0000000033X менән бергә I18NI000000032Х.
асыҡлай, шулай идара итеү рецензенттары монитор һөҙөмтәләрен айыра ала
ҡабаттан зондтарҙы яңынан эшләтергә.

> **TLS pergerrail:** I18NI0000000034X I18NI0000000035X база URL-адрестарын кире ҡаға, әгәр һеҙ ҡуймағыҙ.
> I18NI000000036X конфигында. Производство/стужнеж зондтарҙы 2012 йылғы .
> HTTPS; опт-ин тик урындағы алдан ҡарау өсөн бар.

Һәр бәйләүсе инеү I18NI0000000037X ҡаршы әсирлеккә эләккән
I18NI0000000038X өйөмө (һәм опциональ I18NI0000000039X) шулай псевдоним,
Дәлил статусы, һәм йөкмәткеһе CID ҡалыу менән тура килә баҫылған дәлилдәр. 1990 й.
опциональ I18NI000000040X һаҡсы раҫлай псевдоним-алынған канон хост матчтары the
шлюз алып барыусы һеҙ ниәтләйһегеҙ, пропагандалау, DNS өҙөкләндерергә, тип дрейфтан .
теркәлгән бәйләү.

18NI000000041X блок сымдары DOCS-7’s SoraDNS ролл-аут шул уҡ мониторға.
Һәр яҙма хост-нам/яҙма тибындағы парҙы хәл итә (мәҫәлән,
I18NI000000042X → `docs-preview.sora.link.gw.sora.name` CNAME) һәм
яуаптарын раҫлай `expectedRecords` йәки I18NI000000045X. Икенсеһе
Ҡаты кодекс өҫтөндәге өҙөккә инеү канон хешед хост-исеме етештерелгән
I18NI000000046X; монитор хәҙер иҫбатлай
икеһе лә кеше өсөн уңайлы псевдоним һәм канон хеш (I18NI000000047X)
хәл итеү өсөн ҡайнатылған һылыу хужа. Был DNS промоушен дәлилдәр автоматик:
монитор уңышһыҙлыҡҡа осрай, әгәр ҙә йәки хост дрейф, хатта ҡасан HTTP бәйләүҙәр һаман да
штапель уң күренеш.

### 2. I18NT000000000004X версияһы манифест һаҡсыһы

DOCS-2b’s “ҡулға алынған I18NT00000000005X манифест” талап хәҙер автоматлаштырылған һаҡсы ташый:
I18NI000000048X X шылтыратыуҙары I18NI0000000049X, был саҡырыуҙар
I18NI000000050X кросс-тикшерергә
I18NI000000051X менән ысын I18NT0000000010X спецификацияһы һәм
күренә. Һаҡсы быны раҫлай:

- I18NI0000000052Х-ла күрһәтелгән һәр версияһында 1990 йылдарҙа тап килгән каталог бар.
  `static/openapi/versions/`.
- Һәр яҙма’s `bytes` һәм OpenAPI яландары дисктағы спец файлына тап килә.
- I18NI0000000056X псевдонимы I18NI000000057X яҙмаһын көҙгөләй (дайджест/размер/ҡултамсы метамағлүмәттәре)
  тимәк, ғәҙәттәгесә скачать дрейф ала алмай.
- Ҡул ҡуйылған яҙмалар һылтанма манифест, уның I18NI00000000058X мәрәйҙәре кире .
  шул уҡ спец һәм кемдең ҡултамғаһы/йәмәғәт асҡысы гекс ҡиммәттәре манифестҡа тап килә.

Һаҡсы урындағы кимәлдә йүгерергә, ҡасан һеҙ көҙгө яңы спец:

```bash
cd docs/portal
npm run check:openapi-versions
```

Уңышһыҙлыҡ тураһында хәбәрҙәргә иҫке файл кәңәше (I18NI000000059X) инә.
шулай итеп, порталь өлөш индереүселәр нисек яңыртырға белә снимоктар. Һаҡсыны 2018 йылда һаҡлау.
CI портал релиздарына ҡамасаулай, унда ҡул ҡуйылған манифест һәм баҫылған distest
синхронизациянан төшөп.

### 2. Приборҙар таҡталары & иҫкәртмәләр

- **`dashboards/grafana/docs_portal.json`** – DOCS-3c өсөн беренсел таҡта. Панель
  Трек I18NI0000000061X, репликация SLA һағынып, Уны һынап ҡарағыҙ
  прокси хаталары, һәм зонд латентлығы (I18NI0000000062X өҫтәү). Экспорт
  һәр сығарылыштан һуң плата һәм уны операциялар билетына беркетергә.
- **Тырышып ҡарағыҙ, ул прокси-хәрәкәттәр** – Иҫкәртмәнсе ҡағиҙәһе `TryItProxyErrors` 2019 йылда янғындар .
  I18NI000000064X тамсы йәки
  `tryit_proxy_requests_total{status="error"}` шпиктары.
- **Шлюз СЛО** – I18NI000000066XX псевдонимдарҙы бәйләүҙе тәьмин итә
  реклама өсөн булавкаланған асыҡ һеңдерергә; эскалациялар бәйләнеше
  `cargo xtask soradns-verify-binding` CLI стенограммаһы баҫтырып сығарыу ваҡытында төшөрөлгән.

### 3. Дәлилдәр эҙҙәре

Һәр мониторинг йүгерергә тейеш ҡушырға:

- I18NI0000000068X дәлилдәр йыйылмаһы (I18NI000000069X, бер секция файлдары һәм
  `checksums.sha256`).
- I18NI000000071X өсөн плата өсөн Grafana скриншоттары сығарыу тәҙрәһе аша.
- Ул прокси үҙгәрештәр/кире кире транскрипттар (`npm run manage:tryit-proxy` журналдар) һынап ҡарағыҙ.
- Псевдоним тикшерелгән сығыш I18NI000000073X.

Һаҡлау был I18NI000000074X аҫтында һәм уларҙы бәйләү 2019 йылда.
сығарыу мәсьәләһе, шулай итеп, аудит эҙҙәре иҫән ҡала, һуңынан CI журналдар тамамланған.

## Оператив тикшерелгән исемлек

1. Step7 аша таратыу етәксеһен эшләтегеҙ.
2. Етештереүҙең конфигурацияһы менән `npm run monitor:publishing` башҡарырға; архив
   JSON сығыш.
3. I18NT000000007X панелдәре (I18NI000000076X, `TryItProxyErrors`,
   `DocsPortal/GatewayRefusals`) һәм уларҙы сығарыу билетына беркетергә.
4. График ҡабатланған мониторҙар (тәҡдим ителгән: һәр 15минут) күрһәтеп,
   етештереү URL-адрестар менән бер үк конфиг ҡәнәғәтләндерергә DOCS-3c SLO ҡапҡаһы.
5. Инциденттар ваҡытында, яңынан идара итеү монитор командаһы менән I18NI0000000079X теркәү өсөн
   дәлилдәр алдынан/алдан һуң һәм уны беркетергә үлемдән һуңғы.

Был иллюзиянан һуң DOCS-3c ябыла: порталь төҙөү ағымы, баҫтырыу торбаһы,
һәм мониторинг стека хәҙер йәшәй бер playbook менән ҡабатланған командалар,
өлгө конфигурациялары, һәм телеметрия ҡармаҡтары.