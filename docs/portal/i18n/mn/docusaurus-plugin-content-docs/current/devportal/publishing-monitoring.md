---
id: publishing-monitoring
lang: mn
direction: ltr
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Publishing & Monitoring
sidebar_label: Publishing & Monitoring
description: Capture the end-to-end monitoring flow for SoraFS portal releases so DOCS-3c has deterministic probes, telemetry, and evidence bundles.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Замын зургийн **DOCS-3c** зүйл нь савлагааны хяналтын хуудаснаас илүүг шаарддаг: бүрийн дараа
SoraFS нийтлэхдээ бид хөгжүүлэгчийн портал гэдгийг байнга батлах ёстой, Оролдоод үзээрэй
прокси болон гарцын холболтууд эрүүл хэвээр байна. Энэ хуудас нь мониторингийг баримтжуулна
[Байршуулах гарын авлага](./deploy-guide.md) CI болон өөр
Дуудлага хийх инженерүүд нь Ops-ийн SLO-г хэрэгжүүлэхэд ашигладагтай ижил шалгалтуудыг хийж болно.

## Дамжуулах хоолойн тойм

1. **Бүтээж, гарын үсэг зурна уу** – ажиллуулахын тулд [байршуулах гарын авлага](./deploy-guide.md)-ыг дагана уу.
   `npm run build`, `scripts/preview_wave_preflight.sh`, Sigstore +
   ил тод мэдүүлэх алхамууд. Урьдчилсан нислэгийн скрипт нь `preflight-summary.json` ялгаруулдаг
   Тиймээс урьдчилан харах бүр нь бүтээх/холбоос/шинжилгээний мета өгөгдлийг агуулна.
2. **Зохих ба баталгаажуулах** – `sorafs_cli manifest submit`, `cargo xtask soradns-verify-binding`,
   болон DNS таслах төлөвлөгөө нь засаглалын тодорхойлогч артефактуудыг өгдөг.
3. **Архив нотлох баримт** – CAR хураангуй, Sigstore багц, нэрийн баталгаа,
   датчикийн гаралт ба `docs_portal.json` хяналтын самбарын агшин зуурын зургууд
   `artifacts/sorafs/<tag>/`.

## Хяналтын сувгууд

### 1. Хэвлэлийн мониторууд (`scripts/monitor-publishing.mjs`)

Шинэ `npm run monitor:publishing` команд нь портал шалгагчийг ороож, Оролдоод үзээрэй
прокси шалгагч болон баталгаажуулагчийг нэг CI-д ээлтэй чек болгон холбох. хангах a
JSON тохиргоог (CI нууц эсвэл `configs/docs_monitor.json` руу шалгана) хийгээд ажиллуулна уу:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

`--prom-out ../../artifacts/docs_monitor/monitor.prom` (мөн сонголтоор) нэмнэ үү
`--prom-job docs-preview`) -д тохиромжтой Prometheus текст форматын хэмжигдэхүүнийг ялгаруулах
Pushgateway-д байршуулах эсвэл шууд Prometheus үе шат/үйлдвэрлэлийн явцад хусах. The
хэмжүүрүүд нь JSON-ийн хураангуйг тусгадаг тул SLO хяналтын самбар болон дохиоллын дүрмийг хянах боломжтой
портал, Оролдоод үзээрэй, холбох, DNS эрүүл мэндийг нотлох баримтын багцыг задлан шинжлэхгүйгээр.

Шаардлагатай товчлуур болон олон холболттой тохиргооны жишээ:

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
    "samplePath": "/proxy/v2/accounts/i105.../assets?limit=1",
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

Монитор нь JSON хураангуйг бичдэг (S3/SoraFS тохиромжтой) бөгөөд энэ үед тэгээс өөр гарна.
ямар ч датчик амжилтгүй болж, энэ нь Cron ажил, Buildkite алхам, эсвэл
Alertmanager вэб дэгээ. `--evidence-dir`-г давсан хэвээр байна `summary.json`,
`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`-ийн хажууд
манифест, ингэснээр засаглалын тоймчид хяналт шалгалтын үр дүнг шаардлагагүйгээр ялгах боломжтой
датчикуудыг дахин ажиллуул.

> **TLS хашлага:** `monitorPortal` нь таныг тохируулаагүй л бол `http://` үндсэн URL-уудаас татгалздаг.
> тохиргоонд `allowInsecureHttp: true`. Үйлдвэрлэл/үе шатлалын датчикуудыг асаалттай байлга
> HTTPS; Сонголт нь зөвхөн орон нутгийн урьдчилан үзэхэд зориулагдсан.

Заавал оруулах оролт бүр `cargo xtask soradns-verify-binding`-г баригдсан эсрэг ажиллуулдаг
`portal.gateway.binding.json` багц (болон нэмэлт `manifestJson`) нь бусад нэрээр,
нотлох баримтын төлөв, агуулга CID нь нийтлэгдсэн нотлох баримттай нийцэж байна. The
нэмэлт `hostname` хамгаалалт нь бусад нэрээс гаралтай каноник хосттой таарч байгааг баталгаажуулдаг.
Таны сурталчлах гэж буй gateway host-оос DNS таслагдахаас сэргийлнэ
бүртгэгдсэн холболт.

Нэмэлт `dns` блок нь DOCS-7-ийн SoraDNS-ийг ижил монитор руу холбодог.
Оруулга бүр хостын нэр/бичлэгийн төрлийн хосыг шийддэг (жишээ нь
`docs-preview.sora.link` → `docs-preview.sora.link.gw.sora.name` CNAME) болон
хариултууд `expectedRecords` эсвэл `expectedIncludes` таарч байгааг баталгаажуулна. Хоёр дахь нь
Дээрх хэсэг дэх оруулга нь үүсгэсэн каноник хэшлэгдсэн хост нэрийг хатуу кодлодог
`cargo xtask soradns-hosts --name docs-preview.sora.link`; монитор одоо нотолж байна
хүнд ээлтэй нэр болон каноник хэш (`igjssx53…gw.sora.id`)
бэхлэгдсэн хөөрхөн хост руу шийдээрэй. Энэ нь DNS сурталчилгааны нотолгоог автомат болгодог:
HTTP холболтууд хэвээр байгаа ч хостын аль нэг нь зөрөхөд монитор амжилтгүй болно
зөв манифестийг голчил.

### 2. OpenAPI хувилбар манифест хамгаалагч

DOCS-2b-ийн "гарын үсэг зурсан OpenAPI манифест" шаардлага одоо автоматжуулсан хамгаалалтыг нийлүүлж байна:
`ci/check_openapi_spec.sh` нь `npm run check:openapi-versions` руу залгадаг бөгөөд энэ нь дууддаг.
`scripts/verify-openapi-versions.mjs` хөндлөн шалгах
`docs/portal/static/openapi/versions.json` нь бодит Torii үзүүлэлтүүд болон
илэрдэг. Хамгаалагч дараахь зүйлийг баталгаажуулна.

- `versions.json`-д жагсаасан хувилбар бүр доор тохирох лавлахтай байна
  `static/openapi/versions/`.
- Оруулах бүрийн `bytes` болон `sha256` талбарууд нь диск дээрх тусгай файлтай таарч байна.
- `latest` нэр нь `current` оруулгыг тусгадаг (дижест/хэмжээ/гарын үсгийн мета өгөгдөл)
  Тиймээс өгөгдмөл таталт нь дамжих боломжгүй.
- Гарын үсэг зурсан бичилтүүд нь `artifact.path` нь буцаан зааж буй манифестыг иш татдаг.
  ижил үзүүлэлт ба гарын үсэг/нийтийн түлхүүрийн hex утга нь манифесттэй таарч байна.

Шинэ үзүүлэлтийг тусгах бүрдээ хамгаалагчийг дотооддоо ажиллуулна уу:

```bash
cd docs/portal
npm run check:openapi-versions
```

Алдаа дутагдлын мессеж нь хуучирсан файлын зөвлөмжийг агуулдаг (`npm run sync-openapi -- --latest`)
Тиймээс порталын хувь нэмэр оруулагчид агшин зуурын зургийг хэрхэн сэргээхээ мэддэг. Хамгаалагчийг дотор нь байлгах
CI нь гарын үсэг зурсан манифест болон нийтлэгдсэн тойм бүхий портал хувилбараас сэргийлдэг
синк алдагдах.

### 2. Хяналтын самбар ба анхааруулга

- **`dashboards/grafana/docs_portal.json`** – DOCS-3c-ийн үндсэн самбар. Самбар
  зам `torii_sorafs_gateway_refusals_total`, хуулбарлах SLA алдсан, Үүнийг үзээрэй
  прокси алдаа, шалгах саатал (`docs.preview.integrity` давхарлах). -ийг экспортлох
  гаргасны дараа самбар дээр байрлуулж, үйл ажиллагааны тасалбарт хавсаргана.
- **Үүнийг туршаад үзээрэй прокси дохио** – Alertmanager дүрэм `TryItProxyErrors` асаалттай
  тогтвортой `probe_success{job="tryit-proxy"}` дусал эсвэл
  `tryit_proxy_requests_total{status="error"}` үсрэлт.
- **Gateway SLO** – `DocsPortal/GatewayRefusals` нь нэрийн холболтыг үргэлжлүүлнэ
  тээглүүлсэн манифест тоймыг сурталчлах; -тэй холбоотой хурцадмал байдал
  `cargo xtask soradns-verify-binding` CLI хуулбарыг нийтлэх явцад авсан.

### 3. Нотлох баримт

Хяналтын ажил бүрийг хавсаргана:

- `monitor-publishing` нотлох баримтын багц (`summary.json`, хэсэг тус бүрийн файлууд болон
  `checksums.sha256`).
- Хувилбарын цонхон дээрх `docs_portal` самбарт зориулсан Grafana дэлгэцийн агшин.
- Прокси солих/буцах хуулбарыг (`npm run manage:tryit-proxy` бүртгэлүүд) туршаад үзээрэй.
- `cargo xtask soradns-verify-binding`-аас нэрийн баталгаажуулалтын гаралт.

Эдгээрийг `artifacts/sorafs/<tag>/monitoring/` доор хадгалаад, дотор нь холбоно уу
CI бүртгэлийн хугацаа дууссаны дараа аудитын мөр үлдэх болно.

## Үйл ажиллагааны хяналтын хуудас

1. 7-р алхамаар дамжуулан байршуулах гарын авлагыг ажиллуулна уу.
2. Үйлдвэрлэлийн тохиргоотой `npm run monitor:publishing`-ийг гүйцэтгэх; архив
   JSON гаралт.
3. Grafana хавтанг авах (`docs_portal`, `TryItProxyErrors`,
   `DocsPortal/GatewayRefusals`) болон тэдгээрийг суллах тасалбарт хавсаргана уу.
4. Дахин давтагдах мониторуудын хуваарь (зөвлөдөг: 15 минут тутамд)
   DOCS-3c SLO хаалгыг хангахын тулд ижил тохиргоотой үйлдвэрлэлийн URL-ууд.
5. Бэрхшээл гарсан үед `--json-out` ашиглан дэлгэцийн командыг дахин ажиллуулж бичлэг хийнэ үү.
   нотлох баримтын өмнө/дараа болон үхлийн дараах хэсэгт хавсаргана.

Энэ давталтын дараа DOCS-3c хаагдана: портал бүтээх урсгал, нийтлэх дамжуулах хоолой,
болон хяналтын стек нь одоо давтагдах команд бүхий нэг тоглоомын дэвтэрт амьдардаг,
дээжийн тохиргоо, телеметрийн дэгээ.