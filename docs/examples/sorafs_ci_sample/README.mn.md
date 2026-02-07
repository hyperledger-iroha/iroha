---
lang: mn
direction: ltr
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-12-29T18:16:35.082870+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraFS CI дээжийн бэхэлгээ

Энэ лавлах нь дээжээс үүсгэсэн тодорхойлогч олдворуудыг багцалдаг
`fixtures/sorafs_manifest/ci_sample/`-ийн дагуу ачаалал. Багц нь үүнийг харуулж байна
CI ажлын урсгалын хэрэгжүүлдэг төгсгөл хоорондын SoraFS савлагаа, гарын үсэг зурах шугам.

## Олдворын бараа материал

| Файл | Тодорхойлолт |
|------|-------------|
| `payload.txt` | Бэхэлгээний скриптүүдэд ашигласан эх сурвалжийн ачаалал (энгийн текстийн жишээ). |
| `payload.car` | `sorafs_cli car pack`-аас гаргасан CAR архив. |
| `car_summary.json` | Хураангуйг `car pack`-ээр үүсгэсэн, бөөгнөрөл болон метадата. |
| `chunk_plan.json` | Хэсэгчилсэн хүрээ болон үйлчилгээ үзүүлэгчийн хүлээлтийг тодорхойлсон JSON-г авчрах төлөвлөгөө. |
| `manifest.to` | `sorafs_cli manifest build` үйлдвэрлэсэн Norito манифест. |
| `manifest.json` | Дибаг хийхэд зориулсан хүн унших боломжтой манифест дүрслэл. |
| `proof.json` | `sorafs_cli proof verify`-ийн ялгаруулсан PoR хураангуй. |
| `manifest.bundle.json` | `sorafs_cli manifest sign` үүсгэсэн түлхүүргүй гарын үсгийн багц. |
| `manifest.sig` | Манифестт харгалзах салсан Ed25519 гарын үсэг. |
| `manifest.sign.summary.json` | Гарын үсэг зурах явцад гаргасан CLI хураангуй (хэш, багцын мета өгөгдөл). |
| `manifest.verify.summary.json` | `manifest verify-signature`-аас CLI хураангуй. |

Хувилбарын тэмдэглэл болон баримт бичигт иш татсан бүх мэдээллийн эх сурвалжийг эх сурвалжаас авсан
эдгээр файлууд. `ci/check_sorafs_cli_release.sh` ажлын урсгал нь мөн адил сэргээгддэг
олдворууд болон тэдгээрийг үйлдсэн хувилбаруудаас ялгадаг.

## Бэхэлгээний нөхөн сэргэлт

Бэхэлгээний багцыг сэргээхийн тулд хадгалах сангийн үндэсээс доорх тушаалуудыг ажиллуулна уу.
Тэд `sorafs-cli-fixture` ажлын урсгалд ашигладаг алхмуудыг тусгадаг:

```bash
sorafs_cli car pack \
  --input fixtures/sorafs_manifest/ci_sample/payload.txt \
  --car-out fixtures/sorafs_manifest/ci_sample/payload.car \
  --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to \
  --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --car fixtures/sorafs_manifest/ci_sample/payload.car \
  --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig \
  --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)" \
  --issued-at 1700000000 \
  > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b \
  > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

Хэрэв ямар нэгэн алхам нь өөр өөр хэш үүсгэдэг бол бэхэлгээг шинэчлэхээс өмнө судалж үзээрэй.
CI ажлын урсгалууд нь регрессийг илрүүлэхийн тулд детерминистик гаралт дээр тулгуурладаг.

## Ирээдүйн хамрах хүрээ

Замын зургаас нэмэлт chunker профайл болон нотлох форматууд гарч ирснээр,
энэ лавлах дор тэдний каноник бэхэлгээ нэмэгдэх болно (жишээ нь,
`sorafs.sf2@1.0.0` (`fixtures/sorafs_manifest/ci_sample_sf2/`-г үзнэ үү) эсвэл PDP
урсгал нотолгоо). Шинэ профайл бүр ижил бүтэцтэй байх болно - ачаа, машин,
төлөвлөгөө, манифест, нотолгоо, гарын үсгийн олдворууд - иймээс доод урсгалын автоматжуулалт боломжтой
diff нь захиалгат скриптгүйгээр гаргадаг.