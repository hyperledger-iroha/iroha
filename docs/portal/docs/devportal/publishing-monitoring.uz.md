---
lang: uz
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

“Yo‘l xaritasi” bandi **DOCS-3c** qadoqlash bo‘yicha nazorat ro‘yxatidan ko‘proq narsani talab qiladi: har biridan keyin
SoraFS nashrida biz doimiy ravishda ishlab chiquvchi portal ekanligini isbotlashimiz kerak, sinab ko'ring
proksi-server va shlyuz ulanishlari sog'lom bo'lib qoladi. Ushbu sahifa monitoringni hujjatlashtiradi
[joylashtirish boʻyicha qoʻllanma](./deploy-guide.md) bilan birga boʻlgan sirt
qo'ng'iroq muhandislari Ops SLOni qo'llash uchun foydalanadigan bir xil tekshiruvlarni amalga oshirishi mumkin.

## Quvur quvurlari sarhisobi

1. **Yaratish va imzolash** – ishga tushirish uchun [tartibga solish qo‘llanmasi](./deploy-guide.md)ga amal qiling
   `npm run build`, `scripts/preview_wave_preflight.sh` va Sigstore +
   manifest topshirish bosqichlari. Preflight skripti `preflight-summary.json` ni chiqaradi
   shuning uchun har bir oldindan ko'rish qurilish/bog'lanish/tekshiruv metama'lumotlarini o'z ichiga oladi.
2. **Pin va tasdiqlash** – `sorafs_cli manifest submit`, `cargo xtask soradns-verify-binding`,
   va DNS kesish rejasi boshqaruv uchun deterministik artefaktlarni taqdim etadi.
3. **Arxiv dalillari** – CAR xulosasini, Sigstore to‘plamini, taxallus isbotini,
   zond chiqishi va `docs_portal.json` asboblar panelidagi suratlar
   `artifacts/sorafs/<tag>/`.

## Monitoring kanallari

### 1. Nashriyot monitorlari (`scripts/monitor-publishing.mjs`)

Yangi `npm run monitor:publishing` buyrug'i portal tekshiruvini o'rab oladi, Sinab ko'ring
proksi tekshiruvi va tekshirgichni bitta CI-do'st tekshiruvga ulash. a taqdim eting
JSON konfiguratsiyasi (CI sirlari yoki `configs/docs_monitor.json`da tekshiriladi) va ishga tushiring:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

`--prom-out ../../artifacts/docs_monitor/monitor.prom` qo'shing (va ixtiyoriy
`--prom-job docs-preview`) uchun mos bo'lgan Prometheus matn formatidagi ko'rsatkichlarni chiqaradi.
Pushgateway yuklamalari yoki sahnalashtirish/ishlab chiqarishda to'g'ridan-to'g'ri Prometheus qirqishlar. The
ko'rsatkichlar JSON xulosasini aks ettiradi, shuning uchun SLO asboblar paneli va ogohlantirish qoidalari kuzatilishi mumkin
portal, Sinab ko'ring, bog'lash va DNS salomatligi dalillar to'plamini tahlil qilmasdan.

Kerakli tugmalar va bir nechta ulanishlar bilan namuna konfiguratsiyasi:

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
    "samplePath": "/proxy/v1/accounts/soraカタカナ.../assets?limit=1",
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

Monitor JSON xulosasini yozadi (S3/SoraFS qulay) va qachon noldan farq qiladi
har qanday prob muvaffaqiyatsiz tugadi, bu uni Cron ishlari, Buildkite qadamlari yoki
Alertmanager veb-huklari. `--evidence-dir` o'tish davom etadi `summary.json`,
`portal.json`, `tryit.json` va `binding.json` `checksums.sha256` bilan birga
manifest, shuning uchun boshqaruvni ko'rib chiquvchilar monitor natijalarini shartsiz farqlashlari mumkin
problarni qayta ishga tushiring.

> **TLS himoyasi:** `monitorPortal` `http://` asosiy URL manzillarini siz o‘rnatmasangiz rad etadi
> konfiguratsiyada `allowInsecureHttp: true`. Ishlab chiqarish/sozlash zondlarini yoqing
> HTTPS; kirish faqat mahalliy oldindan ko'rish uchun mavjud.

Har bir majburiy yozuv `cargo xtask soradns-verify-binding` ni qo'lga olinganlarga qarshi ishlaydi
`portal.gateway.binding.json` to'plami (va ixtiyoriy `manifestJson`), shuning uchun taxallus,
isbot holati va kontent CID nashr etilgan dalillarga mos keladi. The
ixtiyoriy `hostname` qo'riqchisi taxallusdan olingan kanonik xost bilan mos kelishini tasdiqlaydi
Siz targ'ib qilmoqchi bo'lgan shlyuz xostingi, bu DNS-dan o'chib ketadigan kesishmalarning oldini oladi
qayd qilingan bog'lanish.

Majburiy emas `dns` blok simlari DOCS-7 ning SoraDNS-ni bir xil monitorga ulaydi.
Har bir yozuv xost nomi/yozuv turi juftligini hal qiladi (masalan
`docs-preview.sora.link` → `docs-preview.sora.link.gw.sora.name` CNAME) va
javoblar `expectedRecords` yoki `expectedIncludes` mosligini tasdiqlaydi. Ikkinchisi
tomonidan ishlab chiqarilgan kanonik xeshlangan xost nomining qattiq kodlari yuqoridagi qismdagi yozuv
`cargo xtask soradns-hosts --name docs-preview.sora.link`; monitor endi isbotlaydi
inson uchun qulay taxallus va kanonik xesh (`igjssx53…gw.sora.id`)
mahkamlangan go'zal xostga qaror qiling. Bu DNS targ'ibot dalillarini avtomatik qiladi:
Agar HTTP ulanishlari hali ham davom etsa ham, agar host drifts bo'lsa, monitor muvaffaqiyatsiz bo'ladi
to'g'ri manifestni belgilang.

### 2. OpenAPI versiyasi manifest himoyasi

DOCS-2b "imzolangan OpenAPI manifest" talabi endi avtomatlashtirilgan qo'riqchini yuboradi:
`ci/check_openapi_spec.sh` `npm run check:openapi-versions` ni chaqiradi, bu esa chaqiradi
O'zaro tekshirish uchun `scripts/verify-openapi-versions.mjs`
`docs/portal/static/openapi/versions.json` haqiqiy Torii xususiyatlariga ega va
namoyon bo'ladi. Qo'riqchi buni tasdiqlaydi:

- `versions.json` ro'yxatida keltirilgan har bir versiyada mos keladigan katalog mavjud
  `static/openapi/versions/`.
- Har bir yozuvning `bytes` va `sha256` maydonlari diskdagi spetsifikatsiya fayliga mos keladi.
- `latest` taxallusi `current` yozuvini aks ettiradi (dijest/hajm/imzo meta-maʼlumotlari)
  shuning uchun standart yuklab olish drift bo'lishi mumkin emas.
- Imzolangan yozuvlar `artifact.path` manifestga ishora qiladi.
  bir xil spetsifikatsiya va imzo/ommaviy kalit hex qiymatlari manifestga mos keladi.

Har safar yangi xususiyatni aks ettirganingizda qo'riqchini mahalliy sifatida ishga tushiring:

```bash
cd docs/portal
npm run check:openapi-versions
```

Muvaffaqiyatsizlik xabarlari eskirgan fayl haqida maslahatni o'z ichiga oladi (`npm run sync-openapi -- --latest`)
shuning uchun portal ishtirokchilari suratlarni qanday yangilashni bilishadi. Qo'riqchini ichkarida ushlab turish
CI imzolangan manifest va chop etilgan dayjestda portal nashrlarini oldini oladi
sinxronlashdan chiqib ketish.

### 2. Boshqaruv paneli va ogohlantirishlar

- **`dashboards/grafana/docs_portal.json`** – DOCS-3c uchun asosiy plata. Panellar
  trek `torii_sorafs_gateway_refusals_total`, replikatsiya SLA o'tkazib yuborilgan, Sinab ko'ring
  proksi-server xatolari va tekshirish kechikishi (`docs.preview.integrity` qoplamasi). ni eksport qiling
  har bir chiqarilgandan keyin taxtani joylashtiring va uni operatsiya chiptasiga biriktiring.
- **Proksi ogohlantirishlarini sinab ko'ring** – Alertmanager qoidasi `TryItProxyErrors` yonadi
  barqaror `probe_success{job="tryit-proxy"}` tomchi yoki
  `tryit_proxy_requests_total{status="error"}` tikanlar.
- **Gateway SLO** – `DocsPortal/GatewayRefusals` taxallus bilan bog‘lanishning davom etishini ta’minlaydi
  mahkamlangan manifest dayjestini reklama qilish; eskalatsiyalar bilan bog'lanadi
  `cargo xtask soradns-verify-binding` CLI transkripti nashr paytida olingan.

### 3. Dalil izi

Har bir monitoring jarayoniga quyidagilar qo'shilishi kerak:

- `monitor-publishing` dalillar to'plami (`summary.json`, har bir bo'lim fayllari va
  `checksums.sha256`).
- Chiqarish oynasi ustidagi `docs_portal` taxtasi uchun Grafana skrinshotlari.
- Proksi-serverni o'zgartirish/qayta tiklash transkriptlarini sinab ko'ring (`npm run manage:tryit-proxy` jurnallari).
- `cargo xtask soradns-verify-binding` dan taxallusni tekshirish chiqishi.

Bularni `artifacts/sorafs/<tag>/monitoring/` ostida saqlang va ularni ulang
CI jurnallari muddati tugagandan keyin audit izi saqlanib qolishi uchun chiqarish muammosi.

## Operatsion nazorat ro'yxati

1. Joylashtirish bo'yicha qo'llanmani 7-qadam orqali ishga tushiring.
2. `npm run monitor:publishing` ni ishlab chiqarish konfiguratsiyasi bilan bajaring; arxiv
   JSON chiqishi.
3. Grafana panellarini suratga oling (`docs_portal`, `TryItProxyErrors`,
   `DocsPortal/GatewayRefusals`) va ularni chiqarish chiptasiga biriktiring.
4. Takroriy monitorlarni (tavsiya etiladi: har 15 daqiqada) belgilab
   DOCS-3c SLO darvozasini qondirish uchun bir xil konfiguratsiyaga ega ishlab chiqarish URL manzillari.
5. Voqea sodir bo'lganda, yozib olish uchun monitor buyrug'ini `--json-out` bilan qayta ishga tushiring.
   dalildan oldin/keyin va uni o'limdan keyin biriktiring.

Ushbu tsikldan so'ng DOCS-3c yopiladi: portalni qurish oqimi, nashr qilish quvuri,
va monitoring to'plami endi takrorlanadigan buyruqlar bilan bitta o'yin kitobida yashaydi,
namuna konfiguratsiyalari va telemetriya ilgaklari.