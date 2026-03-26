---
lang: mn
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ee0a1d26e0c9f3c1ab8908f7eb0dc73049d452e451aff9d2d892c19d733557e
source_last_modified: "2026-01-22T16:26:46.492940+00:00"
translation_last_reviewed: 2026-02-07
id: deploy-guide
title: SoraFS Deployment Guide
sidebar_label: Deployment Guide
description: Promote the developer portal through the SoraFS pipeline with deterministic builds, Sigstore signing, and rollback drills.
translator: machine-google-reviewed
---

## Тойм

Энэхүү тоглоомын дэвтэр нь **DOCS-7** (SoraFS хэвлэл) болон **DOCS-8**-г хөрвүүлдэг.
(CI/CD зүү автоматжуулалт)-ыг хөгжүүлэгчийн порталын үйл ажиллагааны горим болгон хувиргана.
Энэ нь бүтээх / хөвөн үе шат, SoraFS савлагаа, Sigstore тулгуурласан манифестийг хамарна.
гарын үсэг зурах, нэр дэвшүүлэх, баталгаажуулах, буцаах дасгалуудыг хийх тул урьдчилан харах бүр болон
суллах олдвор нь дахин давтагдах, аудит хийх боломжтой.

Урсгал нь таныг `sorafs_cli` хоёртын системтэй (үүнтэй хамт бүтээгдсэн) гэж үздэг.
`--features cli`), пин бүртгэлийн зөвшөөрөлтэй Torii төгсгөлийн цэгт хандах, мөн
OIDC Sigstore-ийн итгэмжлэл. Урт хугацааны нууцыг хадгалах (`IROHA_PRIVATE_KEY`,
`SIGSTORE_ID_TOKEN`, Torii жетонууд) таны CI хадгалах санд; Орон нутгийн гүйлтүүд тэдгээрийг эх сурвалж болгож чадна
бүрхүүлийн экспортоос.

## Урьдчилсан нөхцөл

- `npm` эсвэл `pnpm` бүхий 18.18+ зангилаа.
- `cargo run -p sorafs_car --features cli --bin sorafs_cli`-аас `sorafs_cli`.
- Torii URL, `/v1/sorafs/*` болон эрх мэдлийн бүртгэл/хувийн түлхүүрийг харуулсан URL
  манифест болон бусад нэр оруулах боломжтой.
- OIDC гаргагч (GitHub Actions, GitLab, ажлын ачаалал гэх мэт)
  `SIGSTORE_ID_TOKEN`.
- Нэмэлт: `examples/sorafs_cli_quickstart.sh` хуурай гүйлт болон
  GitHub/GitLab ажлын урсгалын шат дамжлагад зориулсан `docs/source/sorafs_ci_templates.md`.
- Try it OAuth хувьсагчдыг (`DOCS_OAUTH_*`) тохируулаад
  [аюулгүй байдлыг чангатгах хяналтын хуудас](./security-hardening.md) бүтээцийг дэмжихээс өмнө
  лабораторийн гадна. Эдгээр хувьсагч байхгүй үед порталын бүтэц одоо амжилтгүй болно
  эсхүл TTL/санал авах бариулууд цонхны гадна талд унах үед; экспортлох
  `DOCS_OAUTH_ALLOW_INSECURE=1` нь зөвхөн нэг удаагийн орон нутгийн урьдчилан үзэхэд зориулагдсан. -ийг хавсаргана уу
  суллах тасалбарын үзэгний туршилтын нотлох баримт.

## Алхам 0 — Туршиж үзээрэй прокси багцыг аваарай

Netlify эсвэл гарц руу урьдчилан үзэхийг сурталчлахын өмнө Try it прокси дээр тамга дарна уу
эх сурвалжууд болон OpenAPI гарын үсэгтэй манифест дижест нь детерминистик багц болгон:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` прокси/проб/буцах туслахуудыг хуулдаг.
OpenAPI гарын үсгийг шалгаж, `release.json` нэмэх гэж бичнэ
`checksums.sha256`. Энэ багцыг Netlify/SoraFS гарцын урамшуулалд хавсаргана уу
тасалбараар хянагчид яг прокси эх сурвалж болон Torii зорилтот зөвлөмжийг дахин тоглуулах боломжтой
дахин барихгүйгээр. Багц нь үйлчлүүлэгчийн нийлүүлсэн үүргүүд байсан эсэхийг мөн бүртгэнэ
идэвхжүүлсэн (`allow_client_auth`) програмын төлөвлөгөө болон CSP дүрмийг синхрончлолд байлгах.

## Алхам 1 — Порталыг бүтээж, бүрхэх

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

`npm run build` нь `scripts/write-checksums.mjs`-г автоматаар ажиллуулж, дараахийг үүсгэдэг:

- `build/checksums.sha256` — `sha256sum -c`-д тохиромжтой SHA256 манифест.
- `build/release.json` — мета өгөгдөл (`tag`, `generated_at`, `source`)
  CAR/манифест бүр.

Хоёр файлыг CAR-ийн хураангуйтай зэрэгцүүлэн архивлаарай, ингэснээр хянагчдыг урьдчилан харах боломжтой
сэргээн босгохгүйгээр олдворууд.

## Алхам 2 - Статик хөрөнгийг багцлах

CAR савлагчийг Docusaurus гаралтын лавлахын эсрэг ажиллуулна уу. Доорх жишээ
`artifacts/devportal/` дор бүх олдворуудыг бичдэг.

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

JSON-ийн хураангуй хэсэг нь тоолох, задлах, нотлох төлөвлөлтийн зөвлөмжийг агуулдаг.
`manifest build` болон CI хяналтын самбарыг дараа нь дахин ашигладаг.

## Алхам 2b — OpenAPI багц болон SBOM хамтрагчид

DOCS-7 нь портал сайт, OpenAPI хормын хувилбар болон SBOM ачааллыг нийтлэхийг шаарддаг.
Гарцууд нь тодорхой харагдах тул `Sora-Proof`/`Sora-Content-CID`-ийг үдээслэх боломжтой.
олдвор бүрийн гарчиг. Суллах туслах
(`scripts/sorafs-pin-release.sh`) OpenAPI лавлахыг аль хэдийн багцалсан байна
(`static/openapi/`) болон `syft`-ээр ялгардаг SBOM-уудыг тусад нь
`openapi.*`/`*-sbom.*` CAR-ууд болон мета өгөгдлийг бичнэ.
`artifacts/sorafs/portal.additional_assets.json`. Гарын авлагын урсгалыг ажиллуулах үед,
Өөрийн угтвар болон мета өгөгдлийн шошго бүхий ачаа тус бүрийн хувьд 2-4-р алхамуудыг давтана уу
(жишээ нь `--car-out "$OUT"/openapi.car` plus
`--metadata alias_label=docs.sora.link/openapi`). Манифест/алиа бүрийг бүртгээрэй
DNS-г солихын өмнө Torii (сайт, OpenAPI, портал SBOM, OpenAPI SBOM)-д холбоно уу.
Уг гарц нь нийтлэгдсэн бүх олдворыг баталгаажуулах боломжтой.

## Алхам 3 — Манифест бүтээх

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

Хувилбарын цонхондоо пин-бодлогын дарцагуудыг тохируулна уу (жишээ нь, `--pin-storage-class
канаруудад зориулсан халуун`). JSON хувилбар нь нэмэлт боловч кодыг шалгахад тохиромжтой.

## Алхам 4 — Sigstore-ээр гарын үсэг зурна уу

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

Багц нь манифест дижест, chunk digests болон BLAKE3 хэшийг бүртгэдэг.
JWT-г үргэлжлүүлэхгүйгээр OIDC токен. Багцыг хоёуланг нь салгаж хадгал
гарын үсэг; үйлдвэрлэлийн урамшуулал нь огцрохын оронд ижил олдворуудыг дахин ашиглах боломжтой.
Орон нутгийн гүйлтүүд нь үйлчилгээ үзүүлэгчийн тугийг `--identity-token-env` (эсвэл тохируулж) сольж болно.
`SIGSTORE_ID_TOKEN` орчинд) гадны OIDC туслах
жетон.

## Алхам 5 — Бүртгэлийн бүртгэлд оруулна уу

Гарын үсэг зурсан манифестийг (болон хэсэгчилсэн төлөвлөгөөг) Torii руу илгээнэ үү. Үргэлж хураангуй хүсэлт гаргах
Иймээс үүссэн бүртгэлийн бичилт/алиа нотлох баримтыг шалгах боломжтой.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority <katakana-i105-account-id> \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

Урьдчилан үзэх эсвэл канарын нэр (`docs-preview.sora`) гаргахдаа
Өвөрмөц өөр нэртэй илгээх нь QA нь контентыг үйлдвэрлэхээс өмнө баталгаажуулах боломжтой
сурталчилгаа.

Alias холбоход гурван талбар шаардлагатай: `--alias-namespace`, `--alias-name`, болон
`--alias-proof`. Засаглал нотлох багцыг гаргадаг (base64 эсвэл Norito байт)
нэрийн хүсэлтийг зөвшөөрсөн үед; Үүнийг CI нууцад хадгалж, үүнийг a
`manifest submit`-г дуудахын өмнө файл. Та бусад нэрийн тугуудыг тохируулаагүй орхи
DNS-д хүрэлгүйгээр зөвхөн манифестыг бэхлэх зорилготой.

## Алхам 5b — Засаглалын саналыг боловсруулах

Манифест бүр УИХ-ын бэлэн саналтай аялах ёстой бөгөөд ингэснээр ямар ч Сора
иргэн давуу эрхийн үнэмлэх зээлэхгүйгээр өөрчлөлт оруулах боломжтой.
Илгээх/гарын үсэг зурах алхмуудын дараа:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` нь каноник `RegisterPinManifest`-г авдаг.
заавар, бөөгнөрөл, бодлого, бусад нэрийн сануулга. Үүнийг засаглалдаа хавсарга
тасалбар эсвэл Парламентын портал ашиглан төлөөлөгчид дахин барихгүйгээр ачааллыг өөрчлөх боломжтой
олдворууд. Учир нь тушаал нь Torii эрх мэдэл бүхий товчлуур дээр хэзээ ч хүрдэггүй
иргэн орон нутагт саналаа боловсруулж болно.

## Алхам 6 - Нотолгоо болон телеметрийг шалгана уу

Буулгасны дараа тодорхойлогч баталгаажуулах алхмуудыг гүйцэтгэнэ:

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

- `torii_sorafs_gateway_refusals_total` болон шалгах
  Аномалийн хувьд `torii_sorafs_replication_sla_total{outcome="missed"}`.
- `npm run probe:portal`-г ажиллуулж Try-It прокси болон бүртгэгдсэн холбоосуудыг ажиллуулаарай.
  шинээр тогтоогдсон агуулгын эсрэг.
--д тайлбарласан хяналтын нотлох баримтыг авах
  [Хэвлэн нийтлэх ба хяналт](./publishing-monitoring.md) тул DOCS-3c
  ажиглалтын хаалга нь хэвлэн нийтлэх алхмуудын зэрэгцээ хангагдсан байна. Туслагч
  одоо олон `bindings` оруулгуудыг хүлээн авч байна (сайт, OpenAPI, SBOM портал, OpenAPI
  SBOM) ба `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`-ийг зорилтот түвшинд хэрэгжүүлдэг.
  нэмэлт `hostname` хамгаалалтаар дамжуулан хост. Доорх дуудлага нь a хоёуланг нь бичнэ
  ганц JSON хураангуй болон нотлох баримтын багц (`portal.json`, `tryit.json`,
  `binding.json`, болон `checksums.sha256`) хувилбарын лавлах дор:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Алхам 6а - Гарцын гэрчилгээг төлөвлөх

Гарц болохын тулд GAR пакетуудыг үүсгэхээсээ өмнө TLS SAN/challenge төлөвлөгөөг гаргаж аваарай
баг болон DNS зөвшөөрөгчид ижил нотолгоог хянадаг. Шинэ туслах нь толин тусгал
Канон тэмдэгт хостуудыг тоолох замаар DG-3 автоматжуулалтын оролтууд,
pretty-host SAN, DNS-01 шошго, санал болгож буй ACME сорилтууд:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

JSON-г хувилбарын багцын хажууд оруулах (эсвэл өөрчлөлтийн хамт байршуулах
тасалбар) ингэснээр операторууд SAN утгыг Torii-д оруулах боломжтой
`torii.sorafs_gateway.acme` тохиргоо болон GAR хянагчид баталгаажуулах боломжтой
Хост гарал үүслийг дахин ажиллуулахгүйгээр каноник/сайхан зураглал. Нэмэлт нэмэх
Нэг хувилбар дээр дэмжигдсэн дагавар бүрийн `--name` аргументууд.

## Алхам 6б - Каноник хостын зураглалыг гарга

GAR-ийн ачааллыг загварчлахын өмнө тус бүрээр тодорхойлогч хостын зураглалыг тэмдэглэ
бусад нэр. `cargo xtask soradns-hosts` `--name` тус бүрийг каноник болгон хэш болгодог
шошго (`<base32>.gw.sora.id`), шаардлагатай орлуулагч тэмдгийг ялгаруулдаг
(`*.gw.sora.id`), хөөрхөн хостыг (`<alias>.gw.sora.name`) гаргаж авдаг. Тууштай
хувилбарын олдворуудын гаралт, ингэснээр DG-3 хянагч нар зураглалыг ялгаж чадна
GAR мэдүүлгийн хажуугаар:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

`--verify-host-patterns <file>`-г ашиглан GAR эсвэл гарц гарах бүрт хурдан бүтэлгүйтнэ үү
холбох JSON нь шаардлагатай хостуудын аль нэгийг орхигдуулдаг. Туслах нь олон тоо хүлээн авдаг
баталгаажуулах файлууд нь GAR загвар болон загварыг хоёуланг нь зурахад хялбар болгодог
ижил дуудлагад `portal.gateway.binding.json` үдсэн:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

JSON хураангуй болон баталгаажуулалтын бүртгэлийг DNS/гарц солих тасалбарт хавсаргана уу
аудиторууд дахин ажиллуулахгүйгээр каноник, зэрлэг тэмдэгт, хөөрхөн хостуудыг баталгаажуулах боломжтой
үүсмэл скриптүүд. -д шинэ нэр нэмэгдэх бүрд тушаалыг дахин ажиллуул
багцлах тул дараагийн GAR шинэчлэлтүүд нь ижил нотлох баримтыг өвлөн авдаг.

## Алхам 7 - DNS таслах тодорхойлогчийг үүсгэ

Үйлдвэрлэлийн тасалдал нь аудит хийх боломжтой өөрчлөлтийн багцыг шаарддаг. Амжилттай болсны дараа
submission ( alias binding ), туслагч ялгаруулдаг
`artifacts/sorafs/portal.dns-cutover.json`, зураг авалт:- alias холбох мета өгөгдөл (нэрийн зай/нэр/баталгаа, манифест дижест, Torii URL,
  ирүүлсэн эрин үе, эрх мэдэл);
- хувилбарын контекст (шошго, бусад нэр шошго, манифест/CAR зам, багц төлөвлөгөө, Sigstore
  багц);
- баталгаажуулах заагч (шинж команд, бусад нэр + Torii төгсгөлийн цэг); болон
- нэмэлт өөрчлөлтийн хяналтын талбарууд (тасалбарын дугаар, таслах цонх, үйл ажиллагааны холбоо,
  үйлдвэрлэлийн хостын нэр/бүс);
- `Sora-Route-Binding` stapled-ээс авсан маршрутын сурталчилгааны мета өгөгдөл
  толгой хэсэг (каноник хост/CID, толгой + холбох замууд, баталгаажуулах командууд),
  GAR-ийн дэвшлийг баталгаажуулах, нөхөн сэргээх дасгал сургуулилт нь ижил нотолгоонд хамаарна;
- үүсгэсэн маршрутын төлөвлөгөөний олдворууд (`gateway.route_plan.json`,
  толгойн загварууд болон нэмэлт эргүүлэх толгойнууд) тиймээс тасалбар болон CI-г өөрчил
  Хувцасны дэгээ нь DG-3 пакет бүр каноникийг иш татсан эсэхийг шалгах боломжтой
  батлахаас өмнө урамшуулах/буцах төлөвлөгөө;
- нэмэлт кэшийг хүчингүй болгох мета өгөгдөл (цэвэрлэх эцсийн цэг, баталгаажуулах хувьсагч, JSON
  ачаалал, жишээ нь `curl` тушаал); болон
- өмнөх тодорхойлогч руу чиглэсэн буцаах зөвлөмжүүд (таг болон манифестыг суллах
  digest) тул солих тасалбарууд нь тодорхойлогч буцах замыг олж авдаг.

Хувилбар нь кэш цэвэрлэх шаардлагатай үед, хажууд нь каноник төлөвлөгөө үүсгэ
таслах тодорхойлогч:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Үүссэн `portal.cache_plan.json`-г DG-3 багцад хавсаргаснаар операторууд
гаргахдаа детерминист хостууд/замууд (мөн тохирох баталгаажуулах зөвлөмжүүд) байх
`PURGE` хүсэлт. Тодорхойлогчийн нэмэлт кэш мета өгөгдлийн хэсгээс лавлаж болно
Энэ файлыг шууд, өөрчлөлтийг хянах хянагчдыг яг аль дээр нь тохируулж байна
тайрах үед төгсгөлийн цэгүүдийг угаана.

DG-3 багц бүрт урамшуулал + буцаах хяналтын хуудас шаардлагатай. Үүнийг ашиглан үүсгэнэ үү
`cargo xtask soradns-route-plan` тиймээс өөрчлөлтийг хянах хянагч нар яг нарийн зүйлийг хянах боломжтой
өөр нэрээр урьдчилан нислэг хийх, таслах, буцаах алхмууд:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

Гаргасан `gateway.route_plan.json` нь шаталсан канон/хөөрхөн хостуудыг авдаг.
эрүүл мэндийг шалгах сануулга, GAR холболтын шинэчлэлт, кэш цэвэрлэх, буцаах үйлдлүүд.
Өөрчлөлт оруулахаасаа өмнө үүнийг GAR/холбох/таслах олдворуудтай хамт багцлана уу
тасалбар нь скрипттэй ижил алхмууд дээр бэлтгэл хийж, гарын үсэг зурах боломжтой.

`scripts/generate-dns-cutover-plan.mjs` энэ тодорхойлогчийг идэвхжүүлж, ажилладаг
автоматаар `sorafs-pin-release.sh`. Үүнийг сэргээх эсвэл өөрчлөхийн тулд
гараар:

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

Пиг ажиллуулахын өмнө орчны хувьсагчаар нэмэлт мета өгөгдлийг бөглөнө үү
туслагч:

| Хувьсагч | Зорилго |
|----------|---------|
| `DNS_CHANGE_TICKET` | Тодорхойлогчд хадгалагдсан тасалбарын ID. |
| `DNS_CUTOVER_WINDOW` | ISO8601 хайчлах цонх (жишээ нь, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Үйлдвэрлэлийн хостын нэр + эрх мэдэл бүхий бүс. |
| `DNS_OPS_CONTACT` | Дуудлагын нэр эсвэл өргөтгөсөн холбоо барих хаяг. |
| `DNS_CACHE_PURGE_ENDPOINT` | Тодорхойлогч дээр бүртгэгдсэн кэш цэвэрлэх төгсгөлийн цэг. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Цэвэрлэх токен агуулсан Env var (өгөгдмөл нь `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Буцах мета өгөгдлийн өмнөх таслах тодорхойлогч руу очих зам. |

Зөвшөөрөгчид манифестыг баталгаажуулах боломжтой болохын тулд JSON-г DNS өөрчлөлтийн тоймд хавсаргана уу
CI бүртгэлийг хусахгүйгээр дижест, бусад нэр холбох, шалгах командууд.
CLI тугууд `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, `--previous-dns-plan` нь ижил хүчингүй болгодог
CI-ийн гадна туслахыг ажиллуулах үед.

## Алхам 8 — Шийдвэрлэгчийн бүсийн араг ясыг ялгаруулах (заавал биш)

Үйлдвэрлэлийг таслах цонх тодорхой болсон үед хувилбарын скрипт нь ялгаруулж болно
SNS бүсийн араг яс болон шийдэгчийн хэсэг автоматаар. Хүссэн DNS-г дамжуулна уу
орчны хувьсагч эсвэл CLI сонголтоор дамжуулан бүртгэл болон мета өгөгдөл; туслагч
тасалсны дараа шууд `scripts/sns_zonefile_skeleton.py` руу залгана
тодорхойлогч үүсгэгддэг. Дор хаяж нэг A/AAAA/CNAME утга болон GAR-г оруулна уу
digest (Гар үсэг зурсан GAR ачааллын BLAKE3-256). Хэрэв бүс/хостын нэр мэдэгдэж байгаа бол
болон `--dns-zonefile-out`-г орхигдуулсан, туслагч бичнэ
`artifacts/sns/zonefiles/<zone>/<hostname>.json` ба хүн амтай
Шийдвэрлэгчийн хэсэг болгон `ops/soradns/static_zones.<hostname>.json`.

| Хувьсагч / туг | Зорилго |
|----------------|---------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Үүсгэсэн бүсийн араг ясны зам. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Шийдвэрлэгчийн хэсэгчилсэн зам (орхигдсон тохиолдолд анхдагч нь `ops/soradns/static_zones.<hostname>.json`). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | Үүсгэсэн бүртгэлд TTL ашигласан (өгөгдмөл: 600 секунд). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | IPv4 хаягууд (таслалаар тусгаарлагдсан env эсвэл давтагдах боломжтой CLI туг). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | IPv6 хаягууд. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Нэмэлт CNAME зорилт. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | SHA-256 SPKI зүү (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Нэмэлт TXT оруулгууд (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Тооцоолсон бүсийн файлын хувилбарын шошгыг дарна уу. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Таслах цонхыг эхлүүлэхийн оронд `effective_at` цагийн тэмдэглэгээг (RFC3339) хүчээр оруулна уу. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Мета өгөгдөлд бичигдсэн нотлох утгыг дарж бичнэ үү. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Мета өгөгдөлд бүртгэгдсэн CID-г дарна уу. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Guardian хөлдөлт (зөөлөн, хатуу, гэсгээх, хяналт тавих, онцгой байдал). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Асран хамгаалагч/зөвлөлийн тасалбарыг хөлдсөн тухай лавлагаа. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | RFC3339 гэсгээх цагийн тэмдэг. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Нэмэлт царцаах тэмдэглэл (таслалаар тусгаарлагдсан env эсвэл давтагдах туг). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Гарын үсэг зурсан GAR ачааллын BLAKE3-256 дижест (hex). Гарцын холболт байгаа үед шаардлагатай. |

GitHub Үйлдлийн ажлын урсгал нь эдгээр утгыг хадгалах сангийн нууцаас уншдаг тул үйлдвэрлэлийн зүү бүр нь бүсийн артефактуудыг автоматаар ялгаруулдаг. Дараах нууцыг тохируулна уу (мөр нь олон утгатай талбаруудын таслалаар тусгаарлагдсан жагсаалтыг агуулж болно):

| Нууц | Зорилго |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Үйлдвэрлэлийн хостын нэр/бүсийг туслагч руу шилжүүлсэн. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Тодорхойлогчд хадгалагдсан дуудлагын бусад нэр. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Нийтлэх IPv4/IPv6 бичлэгүүд. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Нэмэлт CNAME зорилт. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Base64 SPKI зүү. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Нэмэлт TXT оруулгууд. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Араг ясанд бүртгэгдсэн мета өгөгдлийг царцаах. |
| `DOCS_SORAFS_GAR_DIGEST` | Гарын үсэг зурсан GAR ачааллын зургаан талт кодлогдсон BLAKE3 дижест. |

`.github/workflows/docs-portal-sorafs-pin.yml`-г идэвхжүүлэх үед `dns_change_ticket` болон `dns_cutover_window` оролтуудыг оруулснаар тодорхойлогч/бүсийн файл нь зөв өөрчлөлтийн цонхны мета өгөгдлийг өвлөн авна. Зөвхөн хуурай гүйлт хийх үед тэдгээрийг хоосон орхи.

Ердийн дуудлага (SN-7 эзэмшигчийн runbook-тэй таарч):

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

Туслагч нь солих тасалбарыг TXT оруулга болон автоматаар дамжуулдаг
бусад тохиолдолд таслах цонхны эхлэлийг `effective_at` цагийн тэмдэг болгон өвлөнө.
дарагдсан. Бүрэн үйл ажиллагааны ажлын урсгалыг үзнэ үү
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Нийтийн DNS төлөөлөгчийн тэмдэглэл

Бүсийн араг яс нь зөвхөн тухайн бүсийн эрх мэдэл бүхий бүртгэлийг тодорхойлдог. Та
Бүртгүүлэгч эсвэл DNS дээрээ эцэг эхийн бүсийн NS/DS төлөөллийг тохируулах шаардлагатай хэвээр байна
үйлчилгээ үзүүлэгч, ингэснээр ердийн интернет нэрийн серверүүдийг олж мэдэх боломжтой.

- Апекс/TLD хайчлахын тулд ALIAS/ANAME (үзүүлэгчийн онцлог) ашиглах эсвэл A/AAAA нийтлэх
  ямар ч дамжуулалтын IP хаяг руу чиглэсэн бичлэгүүд.
- Дэд домайнуудын хувьд CNAME-г үүсгэсэн хөөрхөн хост руу нийтлээрэй
  (`<fqdn>.gw.sora.name`).
- Каноник хост (`<hash>.gw.sora.id`) гарцын домэйн дор үлдэнэ.
  таны нийтийн бүсэд нийтлэгдсэнгүй.

### Гарцын толгойн загвар

Байршуулах туслах нь мөн `portal.gateway.headers.txt` болон ялгаруулдаг
`portal.gateway.binding.json`, DG-3-ын шаардлагыг хангасан хоёр олдвор
гарц-агуулгын шаардлага:

- `portal.gateway.headers.txt` нь HTTP толгой блокийг бүрэн агуулж байна (үүнд
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS болон
  `Sora-Route-Binding` тодорхойлогч) ирмэгийн гарцууд бүр дээр бэхлэгдэх ёстой.
  хариу үйлдэл.
- `portal.gateway.binding.json` нь ижил мэдээллийг машинд унших боломжтой хэлбэрээр бичдэг
  хэлбэр нь тасалбарыг солих ба автоматжуулалт нь хост/сид холболтыг ямар ч ялгаагүй
  хусах бүрхүүлийн гаралт.

Тэдгээр нь автоматаар үүсгэгддэг
`cargo xtask soradns-binding-template`
мөн нийлүүлсэн alias, manifest digest болон gateway hostname-ийг авах боломжтой
`sorafs-pin-release.sh` руу. Толгой блокийг дахин үүсгэх эсвэл өөрчлөхийн тулд дараахыг ажиллуулна уу:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Дарахын тулд `--csp-template`, `--permissions-template`, эсвэл `--hsts-template`-ийг дамжуулна
тодорхой байршуулалт нэмэлт шаардлагатай үед анхдагч гарчгийн загварууд
заавар; тэдгээрийг одоо байгаа `--no-*` свичүүдтэй нэгтгэж, толгой хэсгийг буулгана уу
бүхэлд нь.

Толгой хэсгийн хэсгийг CDN өөрчлөлтийн хүсэлтэд хавсаргаж, JSON баримтыг өгнө үү
гарцын автоматжуулалтын дамжуулах хоолой руу оруулснаар бодит хостын сурталчилгаа тохирч байна
нотлох баримтыг суллах.

Хувилбарын скрипт нь баталгаажуулах туслахыг автоматаар ажиллуулдаг тул DG-3 тасалбар
үргэлж сүүлийн үеийн нотлох баримтуудыг оруулдаг. Та үүнийг өөрчлөх болгондоо гараар дахин ажиллуулаарай
JSON-г гараар холбох:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

Тушаал нь холбох багцад авсан `Sora-Proof` ачааллыг баталгаажуулдаг.
`Sora-Route-Binding` мета өгөгдөл нь манифест CID + хостын нэртэй тохирч байгаа эсэхийг баталгаажуулж,
бөгөөд хэрэв толгойн хэсэг эргэвэл хурдан бүтэлгүйтдэг. -ын хажууд байгаа консолын гаралтыг архивлана
CI-ийн гаднах тушаалыг ажиллуулах бүрт бусад байршуулалтын олдворууд DG-3
хянагч нар уг холболтыг таслахаас өмнө баталгаажуулсан гэсэн нотолгоотой.> **DNS тодорхойлогч интеграци:** `portal.dns-cutover.json` одоо
> Эдгээр олдвор руу чиглэсэн `gateway_binding` хэсэг (зам, контент CID,
> нотлох статус болон үсгийн үсгийн загвар) **болон** `route_plan` бадаг
> лавлагаа `gateway.route_plan.json` дээр нэмэх нь үндсэн + буцаах толгой
> загварууд. Эдгээр блокуудыг DG-3 солих тасалбар болгонд оруулаарай, ингэснээр тоймчид боломжтой болно
> яг `Sora-Name/Sora-Proof/CSP` утгуудыг ялгаж, маршрутыг баталгаажуулна уу
> дэмжих/буцах төлөвлөгөө нь угсралтыг нээхгүйгээр нотлох баримтын багцтай таарч байна
> архив.

## Алхам 9 - Нийтлэлийн мониторуудыг ажиллуул

Замын зураглалын даалгавар **DOCS-3c** нь порталыг туршаад үзээрэй гэсэн байнгын нотлох баримтыг шаарддаг
прокси болон гарцын холболтууд гарсны дараа эрүүл хэвээр үлдэнэ. Нэгдсэнийг ажиллуул
7-8-р алхамын дараа шууд хянаж, хуваарьтай датчик руугаа холбоно уу:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` тохиргооны файлыг ачаална (харна уу
  Схемийн хувьд `docs/portal/docs/devportal/publishing-monitoring.md`) ба
  Гурван шалгалтыг гүйцэтгэдэг: портал замын шалгалт + CSP/Зөвшөөрөл-Бодлогын баталгаажуулалт,
  Үүнийг туршаад үзээрэй (заавал `/metrics` төгсгөлийн цэг дээр) болон
  шалгадаг гарцыг холбох баталгаажуулагч (`cargo xtask soradns-verify-binding`).
  хүлээгдэж буй бусад нэр, хост, нотлох статусын эсрэг баригдсан холбох багц,
  болон манифест JSON.
- CI, cron jobs, эсвэл ямар нэгэн датчик амжилтгүй болох үед тушаал нь тэгээс өөр гарна
  runbook операторууд нэр дэвшүүлэхээсээ өмнө хувилбарыг зогсоож болно.
- `--json-out`-ийг давснаар зорилт бүрт JSON ачааллыг нэгтгэн бичнэ.
  статус; `--evidence-dir` нь `summary.json`, `portal.json`, `tryit.json`,
  `binding.json`, `checksums.sha256` тул засаглалын тоймчид ялгах боломжтой
  мониторуудыг дахин ажиллуулахгүйгээр үр дүн гарна. Энэ лавлахыг доор архивлана
  `artifacts/sorafs/<tag>/monitoring/` Sigstore багц болон DNS-ийн хамт
  таслах тодорхойлогч.
- Мониторын гаралтыг оруулах, Grafana экспорт (`dashboards/grafana/docs_portal.json`),
  болон Alertmanager өрөмдлөгийн ID хувилбарын тасалбар нь DOCS-3c SLO байж болно
  дараа нь аудит хийсэн. Тусгай зориулалтын хэвлэлийн монитор тоглоомын ном энд амьдардаг
  `docs/portal/docs/devportal/publishing-monitoring.md`.

Портал шалгалтууд нь HTTPS-г шаарддаг бөгөөд `http://` үндсэн URL-аас татгалздаг.
`allowInsecureHttp` нь дэлгэцийн тохиргоонд тохируулагдсан; үйлдвэрлэл / үе шатыг хадгалах
TLS дээрх зорилтот ба зөвхөн орон нутгийн урьдчилан үзэхийн тулд хүчингүй болгохыг идэвхжүүлнэ.

Нэг удаа Buildkite/cron-д `npm run monitor:publishing`-ээр дамжуулан мониторыг автоматжуулна.
портал шууд ажиллаж байна. Үйлдвэрлэлийн URL-ууд руу чиглэсэн ижил тушаал нь үргэлжилж буй мэдээллийг өгдөг
Хувилбаруудын хооронд SRE/Docs-д тулгуурласан эрүүл мэндийн шалгалт.

## `sorafs-pin-release.sh` ашиглан автоматжуулах

`docs/portal/scripts/sorafs-pin-release.sh` 2-6-р алхамуудыг багтаасан. Энэ нь:

1. `build/` архивыг детерминист tarball болгон,
2. `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   болон `proof verify`,
3. сонголтоор Torii үед `manifest submit` (худалдааны нэрийг оруулах)-г гүйцэтгэнэ
   итгэмжлэлүүд байгаа бөгөөд
4. заавал `artifacts/sorafs/portal.pin.report.json` гэж бичдэг
  `portal.pin.proposal.json`, DNS таслах тодорхойлогч (мэдээлэл илгээсний дараа),
  болон гарц холбох багц (`portal.gateway.binding.json` нэмэх нь
  текстийн толгой блок) тиймээс засаглал, сүлжээ, үйлдлийн багууд ялгаатай байж болно
  CI бүртгэлийг хусахгүйгээр нотлох баримтын багц.

`PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` болон (заавал биш) тохируулах
Скриптийг дуудахын өмнө `PIN_ALIAS_PROOF_PATH`. Хуурай бол `--skip-submit` ашиглана уу
гүйлт; Доор тайлбарласан GitHub ажлын урсгал нь үүнийг `perform_submit`-ээр шилжүүлдэг.
оролт.

## Алхам 8 — OpenAPI үзүүлэлтүүд болон SBOM багцуудыг нийтлэх

DOCS-7 нь аялахын тулд портал бүтээх, OpenAPI техникийн үзүүлэлт, SBOM олдворуудыг шаарддаг.
ижил детерминистик дамжуулах хоолойгоор. Одоо байгаа туслахууд нь гурвыг хамардаг:

1. **Тохиргоог дахин үүсгэж, гарын үсэг зурна уу.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Хадгалахыг хүссэн үедээ `--version=<label>`-ээр дамжуулан гаргах шошгыг дамжуулна уу
   түүхэн агшин зуурын зураг (жишээ нь `2025-q3`). Туслах нь агшин зуурын зургийг бичдэг
   `static/openapi/versions/<label>/torii.json` руу толин тусгал хийнэ
   `versions/current` ба мета өгөгдлийг бүртгэдэг (SHA-256, манифест статус, ба
   шинэчлэгдсэн хугацаа) `static/openapi/versions.json`. Хөгжүүлэгчийн портал
   нь тухайн индексийг уншдаг тул Swagger/RapiDoc самбарууд хувилбар сонгогчийг харуулах боломжтой
   мөн холбогдох тойм/гарын үсгийн мэдээллийг дотор нь харуулах. Орхих
   `--version` нь өмнөх хувилбарын шошгыг хэвээр хадгалж, зөвхөн
   `current` + `latest` заагч.

   Манифест нь SHA-256/BLAKE3 задаргааг авдаг бөгөөд ингэснээр гарц нь үдээстэй болно.
   `Sora-Proof` гарчиг `/reference/torii-swagger`.

2. **CycloneDX SBOMs-г ялгаруулна.** Гаргах шугам нь аль хэдийн syft-д суурилсан байх төлөвтэй байна.
   `docs/source/sorafs_release_pipeline_plan.md`-ийн SBOM. Гаралтыг хадгалах
   Барилгын олдворуудын хажууд:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Ачаа тус бүрийг АВТОМАШИН-д хийнэ.**

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

   Үндсэн сайттай ижил `manifest build` / `manifest sign` алхмуудыг дагана уу.
   Хөрөнгийн өөр нэрийг тааруулах (жишээ нь `docs-openapi.sora` техникийн үзүүлэлт болон
   Гарын үсэг зурсан SBOM багцын хувьд `docs-sbom.sora`). Өөр өөр нэрүүдийг хадгалах
   SoraDNS нотолгоо, GAR болон буцаах тасалбаруудыг яг ачааллын хэмжээнд хүртэл хадгалдаг.

4. **Илгээж, хавсаргана уу.** Одоо байгаа эрх мэдэл + Sigstore багцыг дахин ашиглах, гэхдээ
   Аудиторууд алийг нь хянах боломжтой болохын тулд хувилбарын хяналтын хуудсанд нэрийн tuple-г тэмдэглэ
   Сорагийн нэрийн газрын зураг нь манифест шингэдэг.

Тодорхойлолт/SBOM манифестийг портал бүтээхтэй зэрэгцүүлэн архивлах нь бүх зүйлийг баталгаажуулдаг
суллах тасалбар нь савлагчийг дахин ажиллуулахгүйгээр бүтэн олдворыг агуулна.

### Автоматжуулалтын туслах (CI/багцын скрипт)

`./ci/package_docs_portal_sorafs.sh` 1-8-р алхамуудыг кодчилдог тул замын зураглалын зүйл
**DOCS‑7**-г нэг тушаалаар ашиглаж болно. Туслагч:

- шаардлагатай портал бэлтгэлийг ажиллуулдаг (`npm ci`, OpenAPI/norito sync, виджетийн тестүүд);
- портал, OpenAPI, SBOM CARs + `sorafs_cli`-ээр дамжуулан манифест хосуудыг ялгаруулдаг;
- сонголтоор `sorafs_cli proof verify` (`--proof`) болон Sigstore гарын үсэг зурдаг
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- `artifacts/devportal/sorafs/<timestamp>/` болон доор олдвор бүрийг унагадаг
  `package_summary.json` бичдэг тул CI/release хэрэгсэл нь багцыг залгих боломжтой; болон
- `artifacts/devportal/sorafs/latest`-г шинэчилж, хамгийн сүүлийн гүйлтийг зааж өгнө.

Жишээ (Sigstore + PoR бүхий бүрэн дамжуулах хоолой):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Мэдэх үнэ цэнэтэй тугнууд:

- `--out <dir>` – олдворын үндэсийг хүчингүй болгох (өгөгдмөл тохиргоо нь цагийн тэмдэгтэй хавтаснуудыг хадгалдаг).
- `--skip-build` – одоо байгаа `docs/portal/build`-г дахин ашиглах (CI боломжгүй үед тохиромжтой)
  офлайн толины улмаас дахин бүтээх).
- `--skip-sync-openapi` – `cargo xtask openapi` үед `npm run sync-openapi`-г алгасах
  crates.io руу нэвтрэх боломжгүй.
- `--skip-sbom` – хоёртын файлыг суулгаагүй үед `syft` руу залгахаас зайлсхий.
  скрипт оронд нь анхааруулга хэвлэдэг).
- `--proof` – CAR/манифест хос бүрт `sorafs_cli proof verify`-г ажиллуул. Олон
  файлын ачааллыг CLI-д хэсэгчилсэн төлөвлөгөөний дэмжлэг шаардлагатай хэвээр байгаа тул энэ тугийг үлдээгээрэй
  Хэрэв та `plan chunk count` алдаа гарвал тохиргоог болиулж, нэг удаа гараар баталгаажуулна уу.
  дээд урсгалын хаалганы газар.
- `--sign` – `sorafs_cli manifest sign`-г дуудна. Тэмдэглэгээ өгнө үү
  `SIGSTORE_ID_TOKEN` (эсвэл `--sigstore-token-env`) эсвэл CLI-д үүнийг ашиглан татахыг зөвшөөрнө үү.
  `--sigstore-provider/--sigstore-audience`.

Үйлдвэрлэлийн олдворуудыг тээвэрлэхдээ `docs/portal/scripts/sorafs-pin-release.sh` ашигладаг.
Энэ нь одоо портал, OpenAPI болон SBOM ачааллыг багцалж, манифест бүрт гарын үсэг зурж,
`portal.additional_assets.json`-д нэмэлт хөрөнгийн мета өгөгдлийг бүртгэдэг. Туслагч
CI багцлагчийн ашигладаг нэмэлт товчлуурууд дээр нэмээд шинийг ойлгодог
`--openapi-*`, `--portal-sbom-*`, `--openapi-sbom-*` шилжүүлдэг тул та
олдвор тус бүрд өөр нэр тохируулагчийг оноож, SBOM эх сурвалжийг дарж бичнэ үү
`--openapi-sbom-source`, тодорхой ачааллыг алгасах (`--skip-openapi`/`--skip-sbom`),
мөн `--syft-bin`-тэй анхдагч бус `syft` хоёртын файлыг зааж өгнө.

Скрипт нь түүний ажиллуулж буй тушаал бүрийг харуулдаг; бүртгэлийг хувилбарын тасалбар руу хуулна
`package_summary.json`-ийн хажуугаар тоймчид АВТОМАШИН-ын мэдээллийг ялгаж, төлөвлөх боломжтой
мета өгөгдөл болон Sigstore багц хэшүүдийг тусгай бүрхүүлийн гаралтгүйгээр холбоно.

## Алхам 9 — Гарц + SoraDNS баталгаажуулалт

Таслахыг зарлахаасаа өмнө шинэ нэр нь SoraDNS-ээр шийдэгдэж байгааг батлах хэрэгтэй
гарцууд шинэ нотлох баримтууд:

1. **Сурдаг хаалгыг ажиллуул.** `ci/check_sorafs_gateway_probe.sh` дасгалууд
   `cargo xtask sorafs-gateway-probe`-ийн демо төхөөрөмжүүдийн эсрэг
   `fixtures/sorafs_gateway/probe_demo/`. Бодит байршуулалтын хувьд датчикийг чиглүүлнэ
   зорилтот хостын нэр дээр:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   Сорьц нь `Sora-Name`, `Sora-Proof`, `Sora-Proof-Status` кодыг тайлдаг.
   `docs/source/sorafs_alias_policy.md` ба манифест задлахад амжилтгүй болно,
   TTLs, эсвэл GAR bindings drift.

   Хөнгөн спот шалгалтын хувьд (жишээ нь, зөвхөн холбох багц үед
   өөрчлөгдсөн), `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>` ажиллуулна уу.
   Туслагч нь баригдсан багцыг баталгаажуулдаг бөгөөд суллахад тохиромжтой
   бүрэн шалгалтын өрөмдлөгийн оронд зөвхөн заавал баталгаажуулах шаардлагатай тасалбарууд.

2. **Өрмийн нотлох баримтыг аваарай.** Оператор өрөм эсвэл PagerDuty хуурай гүйлтийн хувьд боож өгнө.
   `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario бүхий шалгалт
   devportal-rollout -- …`. Боодол нь толгой/логуудыг доор хадгалдаг
   `artifacts/sorafs_gateway_probe/<stamp>/`, `ops/drill-log.md` шинэчлэлтүүд болон
   (заавал биш) буцаах дэгээ эсвэл PagerDuty ачааллыг өдөөдөг. Тохируулах
   IP-г хатуу кодлохын оронд SoraDNS замыг баталгаажуулахын тулд `--host docs.sora`.3. **DNS холболтыг баталгаажуулна уу.** Засаглал нь нэрийн баталгааг нийтлэх үед тэмдэглэнэ үү.
   Шинжилгээнд дурдсан GAR файлыг (`--gar`) аваад хувилбарт хавсаргана уу.
   нотлох баримт. Шийдвэрлэгч эзэмшигчид ижил оролтоор дамжуулан тусгах боломжтой
   `tools/soradns-resolver` кэштэй оруулгууд нь шинэ манифестыг хүндэтгэх болно.
   JSON-г хавсаргахаасаа өмнө ажиллуулна уу
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   Иймээс детерминист хостын зураглал, манифест мета өгөгдөл, телеметрийн шошго
   офлайнаар баталгаажуулсан. Туслагч нь `--json-out` хураангуйг хажууд нь гаргаж болно
   GAR гарын үсэг зурсан тул хянагчид хоёртын файлыг нээхгүйгээр баталгаажуулах боломжтой нотлох баримттай болно.
  Шинэ GAR-ийн төслийг боловсруулахдаа илүүд үздэг
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (манифест файл байхгүй үед л `--manifest-cid <cid>` руу буцна уу
  боломжтой). Туслагч одоо CID **болон** BLAKE3 дижестийг шууд эндээс авдаг
  манифест JSON, хоосон зайг багасгаж, давхардсан `--telemetry-label`
  дарцаглаж, шошгыг эрэмбэлж, өгөгдмөл CSP/HSTS/Permissions-Bolicy-г гаргадаг.
  JSON-г бичихээс өмнө загваруудыг ашигласнаар ачаалал хэдий ч тодорхойлогддог
  операторууд өөр өөр бүрхүүлээс шошго авдаг.

4. **Алианы хэмжигдэхүүнийг үзэх.** `torii_sorafs_alias_cache_refresh_duration_ms`-г хадгал
   болон дэлгэцэн дээр `torii_sorafs_gateway_refusals_total{profile="docs"}` байхад
   датчик ажиллаж байна; хоёулаа цувралд багтсан
   `dashboards/grafana/docs_portal.json`.

## Алхам 10 - Хяналт ба нотлох баримтыг багцлах

- **Хяналтын самбар.** Экспортын `dashboards/grafana/docs_portal.json` (порталын SLO),
  `dashboards/grafana/sorafs_gateway_observability.json` (гарцны хоцролт +
  эрүүл мэндийн баталгаа), мөн `dashboards/grafana/sorafs_fetch_observability.json`
  (оркестрийн эрүүл мэнд) хувилбар бүрт. JSON экспортыг хавсаргана уу
  тасалбарыг гаргаснаар тоймчид Prometheus асуултуудыг дахин тоглуулах боломжтой.
- **Архивыг шалгана уу.** `artifacts/sorafs_gateway_probe/<stamp>/`-г git- хавсралтад хадгална уу.
  эсвэл таны нотлох хувин. Туршилтын хураангуй, толгой хэсэг болон PagerDuty-г оруулна уу
  телеметрийн скриптээр авсан ачаалал.
- **Багцыг гаргах.** Портал/SBOM/OpenAPI CAR хураангуй, манифестыг хадгалах
  багцууд, Sigstore гарын үсэг, `portal.pin.report.json`, Турших туршилтын бүртгэлүүд болон
  линк-шалгах тайланг нэг цагийн тэмдэгтэй хавтсанд (жишээ нь,
  `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Өрмийн лог.** Зонд нь өрмийн нэг хэсэг бол зөвшөөр
  `scripts/telemetry/run_sorafs_gateway_probe.sh` `ops/drill-log.md` дээр хавсаргана
  тиймээс ижил нотолгоо нь SNNet-5 эмх замбараагүй байдлын шаардлагыг хангаж байна.
- **Тасалбарын холбоос.** Grafana самбарын ID эсвэл хавсаргасан PNG экспортыг лавлана уу.
  өөрчлөлтийн тасалбар, шалгалтын тайлангийн замын хамт, тиймээс өөрчлөлт-хянагч
  бүрхүүлд нэвтрэхгүйгээр SLO-г шалгаж болно.

## Алхам 11 - Олон эх сурвалжаас авах дасгал болон онооны самбарын нотолгоо

SoraFS дээр нийтлэхэд одоо олон эх сурвалжаас татах нотлох баримт шаардлагатай (DOCS-7/SF-6)
Дээрх DNS/гарц баталгааны хажууд. Манифестыг тогтоосны дараа:

1. **`sorafs_fetch`-г шууд манифестын эсрэг ажиллуулна уу.** Ижил төлөвлөгөө/манифест ашиглана уу.
   2-3-р алхамд үйлдвэрлэсэн олдворууд дээр нэмээд тус бүрт олгосон гарцын итгэмжлэлүүд
   үйлчилгээ үзүүлэгч. Аудиторууд найруулагчийг дахин тоглуулахын тулд гаралт бүрийг үргэлжлүүлээрэй
   шийдвэрийн зам:

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

   - Манифестын иш татсан үйлчилгээ үзүүлэгчийн сурталчилгааг эхлээд татаж аваарай (жишээ нь
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     мөн `--provider-advert name=path`-ээр дамжуулж онооны самбар боломжтой болно
     чадамжийн цонхыг тодорхой хэмжээгээр үнэлэх. Ашиглах
     `--allow-implicit-provider-metadata` **зөвхөн** дотор тоглолтыг дахин тоглуулах үед
     CI; үйлдвэрлэлийн дасгал сургуулилт нь газардсан гарын үсэг зурсан зарыг иш татах ёстой
     зүү.
   - Манифест нь нэмэлт бүс нутгийг дурдах үед тушаалыг давт
     харгалзах үйлчилгээ үзүүлэгч нь тааруулдаг тул кэш/алиас бүр тохирохтой байна
     олдвор авах.

2. **Гаралтыг архивлах.** `scoreboard.json`,
   `providers.ndjson`, `fetch.json`, `chunk_receipts.ndjson`
   нотлох баримт хавтас гаргах. Эдгээр файлууд нь үе тэнгийн жинг авч, дахин оролдоно уу
   засаглалын багцад заавал байх ёстой төсөв, саатлын EWMA болон хэсэг бүрийн орлого
   SF-7-д хадгална.

3. **Телеметрийг шинэчлэх.** Татаж авах гаралтыг **SoraFS Fetch руу импортлох
   Ажиглалт** хяналтын самбар (`dashboards/grafana/sorafs_fetch_observability.json`),
   `torii_sorafs_fetch_duration_ms`/`_failures_total` болон
   хэвийн бус байдлын үйлчилгээ үзүүлэгчийн хүрээний самбар. Grafana самбарын агшин агшныг дараахтай холбоно уу
   онооны самбарын замын хажууд тасалбарыг гарга.

4. **Сэрэмжлүүлгийн дүрмийг дагаж тамхи тат.** `scripts/telemetry/test_sorafs_fetch_alerts.sh`-г ажиллуул.
   хувилбарыг хаахаас өмнө Prometheus дохиоллын багцыг баталгаажуулах. Хавсаргах
   тасалбар руу promtool гаргаснаар DOCS-7 хянагчид лангууг баталгаажуулах боломжтой
   болон удаан нийлүүлэгчийн дохиолол зэвсэгтэй хэвээр байна.

5. **CI руу залгана уу.** Портал зүүний ажлын урсгал нь `sorafs_fetch` алхамыг ардаа байлгадаг.
   `perform_fetch_probe` оролт; тайзны/үйлдвэрлэлийн ажлыг идэвхжүүлэхийн тулд
   авчрах нотлох баримтыг гарын авлагагүйгээр манифест багцын хамт гаргадаг
   хөндлөнгийн оролцоо. Орон нутгийн дасгалууд нь экспортлох замаар ижил скриптийг дахин ашиглах боломжтой
   гарцын токенууд болон `PIN_FETCH_PROVIDERS`-г таслалаар тусгаарласан тохиргоо
   үйлчилгээ үзүүлэгчийн жагсаалт.

## Урамшуулал, ажиглалт, буцаалт

1. **Урамшуулал:** тайз болон үйлдвэрлэлийн нэрсийг тусад нь байлгах. Дэмжих
   `manifest submit`-г ижил манифест/багцаар дахин ажиллуулах, солих
   `--alias-namespace/--alias-name` нь үйлдвэрлэлийн бусад нэрийг заана. Энэ
   QA нь тайзны зүүг зөвшөөрсний дараа дахин бүтээх эсвэл огцрохоос зайлсхийдэг.
2. **Хяналт:** зүү бүртгэлийн хяналтын самбарыг импортлох
   (`docs/source/grafana_sorafs_pin_registry.json`) дээр нэмээд порталын онцлог
   датчик (`docs/portal/docs/devportal/observability.md`-г үзнэ үү). Шалгалтын дүн дээр анхааруулга
   дрифт, амжилтгүй датчик эсвэл дахин оролдлого хийх огцом өсөлт.
3. **Буцах:** буцаах, өмнөх манифестийг дахин илгээх (эсвэл буцаах
   одоогийн бусад нэр) `sorafs_cli manifest submit --alias ... --retire` ашиглан.
   Сүүлд мэдэгдэж байгаа сайн багц болон CAR-ийн хураангуйг үргэлж хадгалаарай
   CI бүртгэлүүд эргэлдэж байвал дахин үүсгэх боломжтой.

## CI ажлын урсгалын загвар

Таны дамжуулах хоолой хамгийн багадаа:

1. Build + Lint (`npm ci`, `npm run build`, хяналтын нийлбэр үүсгэх).
2. Багц (`car pack`) болон манифестуудыг тооцоолох.
3. Ажлын хүрээнд хамаарах OIDC жетон (`manifest sign`) ашиглан гарын үсэг зурна уу.
4. Аудит хийх зорилгоор олдворуудыг (CAR, манифест, багц, төлөвлөгөө, хураангуй) байршуулах.
5. Пин бүртгэлд оруулна уу:
   - Хүсэлт татах → `docs-preview.sora`.
   - Шошго / хамгаалагдсан салбарууд → үйлдвэрлэлийн нэрийн сурталчилгаа.
6. Гарахаасаа өмнө датчик + баталгаа шалгах хаалгыг ажиллуул.

`.github/workflows/docs-portal-sorafs-pin.yml` эдгээр бүх алхмуудыг хооронд нь холбоно
гарын авлагын хувилбаруудад зориулагдсан. Ажлын явц:

- порталыг бүтээх/турших,
- бүтээх ажлыг `scripts/sorafs-pin-release.sh`-ээр багцалж,
- GitHub OIDC ашиглан манифест багцад гарын үсэг зурах/баталгаажуулах,
- CAR/манифест/багц/төлөвлөгөө/баталгааны хураангуйг олдвор болгон байршуулах, мөн
- (заавал биш) нууц байгаа үед манифест + alias заавал илгээнэ.

Ажлыг эхлүүлэхийн өмнө дараах агуулахын нууц/хувьсагчдыг тохируулна уу:

| Нэр | Зорилго |
|------|---------|
| `DOCS_SORAFS_TORII_URL` | `/v1/sorafs/pin/register`-г ил гаргадаг Torii хост. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Илгээлттэй хамт бүртгэгдсэн эрин үеийн танигч. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Манифест илгээх гарын үсэг зурах эрх. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | `perform_submit` `true` үед манифест руу залгасан нэрийн tuple. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Base64-р кодлогдсон хуурамч нэр баталгаажуулах багц (заавал биш; бусад нэрээр холбохыг алгасах). |
| `DOCS_ANALYTICS_*` | Бусад ажлын урсгалд дахин ашигласан аналитик/шинжилгээний төгсгөлийн цэгүүд. |

Үйлдлийн интерфейсээр дамжуулан ажлын урсгалыг идэвхжүүлнэ үү:

1. `alias_label` (жишээ нь, `docs.sora.link`), нэмэлт `proposal_alias`,
   болон нэмэлт `release_tag` дарах.
2. Torii-д хүрэлгүйгээр олдвор үүсгэхийн тулд `perform_submit`-г тэмдэглээгүй орхино уу.
   (хуурай гүйлтэд хэрэгтэй) эсвэл тохируулсан руу шууд нийтлэх боломжтой
   бусад нэр.

`docs/source/sorafs_ci_templates.md` ерөнхий CI туслахуудыг баримтжуулсаар байна
Энэ агуулахаас гадуурх төслүүд, гэхдээ порталын ажлын урсгалыг илүүд үзэх хэрэгтэй
өдөр тутмын хувилбаруудад зориулагдсан.

## Хяналтын хуудас

- [ ] `npm run build`, `npm run test:*`, `npm run check:links` ногоон өнгөтэй.
- [ ] `build/checksums.sha256` болон `build/release.json` олдворуудад баригдсан.
- [ ] `artifacts/` дагуу үүсгэсэн CAR, төлөвлөгөө, манифест, хураангуй.
- [ ] Sigstore багц + бүртгэлтэй хадгалагдсан салангид гарын үсэг.
- [ ] `portal.manifest.submit.summary.json` болон `portal.manifest.submit.response.json`
      мэдүүлэг гарсан үед баригдсан.
- [ ] `portal.pin.report.json` (мөн нэмэлт `portal.pin.proposal.json`)
      CAR/манифест олдворуудын хамт архивлагдсан.
- [ ] `proof verify` болон `manifest verify-signature` логуудыг архивласан.
- [ ] Grafana хяналтын самбар шинэчлэгдсэн + Туршилтыг амжилттай хийсэн.
- [ ] Тэмдэглэлд хавсаргасан буцаах тэмдэглэл (өмнөх манифест ID + бусад нэрийн дижест).
      суллах тасалбар.