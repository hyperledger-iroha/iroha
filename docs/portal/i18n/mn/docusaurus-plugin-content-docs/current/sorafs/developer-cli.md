---
id: developer-cli
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS CLI Cookbook
sidebar_label: CLI Cookbook
description: Task-focused walkthrough of the consolidated `sorafs_cli` surface.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Каноник эх сурвалжийг анхаарна уу
:::

Нэгдсэн `sorafs_cli` гадаргуу (`sorafs_car` хайрцгаар хангагдсан.
`cli` функц идэвхжсэн) SoraFS бэлтгэхэд шаардлагатай алхам бүрийг харуулна.
олдворууд. Нийтлэг ажлын урсгал руу шууд шилжихийн тулд энэ хоолны номыг ашиглана уу; үүнийг хослуул
үйл ажиллагааны контекстэд зориулсан манифест дамжуулах хоолой болон найруулагчийн дэвтэр.

## Багцын ачаалал

`car pack`-г ашиглан тодорхойлогдох АВТОМАШИНЫ архив болон багцын төлөвлөгөөг гарга. The
Хэрэв бариул өгөөгүй бол тушаал нь SF-1 хэсгийг автоматаар сонгоно.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Анхдагч chunker бариул: `sorafs.sf1@1.0.0`.
- Лавлах оруулга нь үг зүйн дарааллаар явагддаг тул шалгах нийлбэр тогтвортой байна
  платформ даяар.
- JSON-ийн хураангуй нь ачааллын хураангуй мэдээлэл, хэсэг бүрийн мета өгөгдөл болон үндэсийг агуулдаг
  Бүртгэл болон найруулагчаар хүлээн зөвшөөрөгдсөн CID.

## Манифестуудыг бүтээх

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- `--pin-*` сонголтуудын зураглал дахь `PinPolicy` талбарт шууд
  `sorafs_manifest::ManifestBuilder`.
- CLI-г SHA3 хэсгийг дахин тооцоолохыг хүсвэл `--chunk-plan` оруулна уу.
  илгээхээс өмнө задлах; эс бөгөөс энэ нь дотор суулгагдсан дижестийг дахин ашигладаг
  хураангуй.
- JSON гаралт нь Norito ачааллыг тусгаж, тухайн үеийн шууд ялгааг харуулдаг.
  тойм.

## Тэмдгүүд нь урт хугацааны түлхүүргүйгээр илэрдэг

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Дотор токен, орчны хувьсагч эсвэл файлд суурилсан эх сурвалжийг хүлээн авна.
- Гарал үүслийн мета өгөгдлийг нэмдэг (`token_source`, `token_hash_hex`, хэсэгчилсэн мэдээлэл)
  `--include-token=true`-ээс бусад тохиолдолд түүхий JWT-г хадгалахгүйгээр.
- CI-д сайн ажилладаг: GitHub Actions OIDC-тай хослуулан тохируулна уу
  `--identity-token-provider=github-actions`.

## Манифестуудыг Torii хаягаар илгээнэ үү

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority soraカタカナ... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Хуваарийн нотолгоонд Norito код тайлж, тэдгээртэй тохирч байгаа эсэхийг шалгана.
  Torii-д нийтлэхээс өмнө манифест дижест.
- Үл тохирох халдлагаас урьдчилан сэргийлэхийн тулд SHA3-ийн багцыг төлөвлөгөөнөөс дахин тооцоолно.
- Хариултын хураангуй нь HTTP статус, толгой хэсэг, бүртгэлийн ачааллыг агуулдаг
  дараа нь аудит хийх.

## Автомашины контент болон нотолгоог баталгаажуулна уу

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- PoR модыг дахин бүтээж, ачааллын дижестийг манифест хураангуйтай харьцуулна.
- Хуулбарлах нотолгоог илгээхэд шаардлагатай тоо, танигчийг бичнэ
  засаглалд.

## Урсгал хамгаалалттай телеметр

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- Дамжуулсан нотлох баримт бүрт NDJSON зүйлийг ялгаруулна (давталтаар тоглуулахыг идэвхгүй болгох
  `--emit-events=false`).
- Амжилт/бүтэлгүйтлийн тоо, хоцрогдлын гистограм, түүвэрлэсэн алдааг нэгтгэдэг.
  хураангуй JSON нь хяналтын самбар нь бүртгэлийг хусахгүйгээр үр дүнг зурах боломжтой.
- Гарц нь алдаа гарсан эсвэл орон нутгийн PoR баталгаажуулалтыг мэдээлэх үед тэгээс өөр гарна
  (`--por-root-hex`-ээр дамжуулан) нотлох баримтыг үгүйсгэдэг. Босгыг тохируулна уу
  `--max-failures` болон `--max-verification-failures` бэлтгэлийн гүйлтэд зориулагдсан.
- Өнөөдөр PoR-ийг дэмждэг; PDP болон PoTR нь SF-13/SF-14 нэг удаа ижил дугтуйг дахин ашигладаг
  газар.
- `--governance-evidence-dir` үзүүлсэн хураангуй, мета өгөгдлийг (цаг хугацааны тэмдэг,
  CLI хувилбар, гарцын URL, манифест дижест) болон манифестын хуулбар
  нийлүүлсэн лавлах, ингэснээр засаглалын пакетууд баталгааны урсгалыг архивлах боломжтой
  гүйлтийг дахин тоглуулахгүйгээр нотлох баримт.

## Нэмэлт лавлагаа

- `docs/source/sorafs_cli.md` - тугны бүрэн баримт бичиг.
- `docs/source/sorafs_proof_streaming.md` — баталгаат телеметрийн схем ба Grafana
  самбарын загвар.
- `docs/source/sorafs/manifest_pipeline.md` - хэсэгчилсэн гүнзгий шумбалт, манифест
  найрлага, АВТО машинтай харьцах.