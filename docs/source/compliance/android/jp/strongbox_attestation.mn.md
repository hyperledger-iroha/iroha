---
lang: mn
direction: ltr
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2025-12-29T18:16:35.929201+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# StrongBox баталгаажуулалтын нотолгоо — Японд байршуулалт

| Талбай | Үнэ цэнэ |
|-------|-------|
| Үнэлгээний цонх | 2026-02-10 – 2026-02-12 |
| Олдворын байршил | `artifacts/android/attestation/<device-tag>/<date>/` (`docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`-ийн багцын формат) |
| Capture Tooling | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| Шүүгчид | Техник хангамжийн лабораторийн ахлах, дагаж мөрдөх, хууль эрх зүй (JP) |

## 1. Баривчлах журам

1. StrongBox матрицад жагсаасан төхөөрөмж бүр дээр сорилт үүсгэж, баталгаажуулалтын багцыг аваарай:
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. Багцын мета өгөгдлийг (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`) нотлох баримтын мод руу оруулна.
3. Бүх багцыг офлайнаар дахин шалгахын тулд CI туслахыг ажиллуулна уу:
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. Төхөөрөмжийн хураангуй (2026-02-12)

| Төхөөрөмжийн шошго | Загвар / StrongBox | Багцын зам | Үр дүн | Тэмдэглэл |
|------------|-------------------|-------------|--------|-------|
| `pixel6-strongbox-a` | Pixel 6 / Tensor G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ Дамжуулсан (техник хангамжаар дэмжигдсэн) | Сорилттой, үйлдлийн системийн засвар 2025-03-05. |
| `pixel7-strongbox-a` | Pixel 7 / Tensor G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ Давсан | Анхан шатны CI эгнээний нэр дэвшигч; техникийн үзүүлэлт доторх температур. |
| `pixel8pro-strongbox-a` | Pixel 8 Pro / Tensor G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ тэнцсэн (дахин шалгалт) | USB-C төвийг сольсон; Buildkite `android-strongbox-attestation#221` дамжуулсан багцыг барьж авсан. |
| `s23-strongbox-a` | Galaxy S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ Давсан | Нокс аттестатчиллын профайлыг 2026-02-09 импортолсон. |
| `s24-strongbox-a` | Galaxy S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ Давсан | Нокс баталгаажуулалтын профайлыг импортолсон; CI эгнээ одоо ногоон байна. |

Төхөөрөмжийн шошгуудыг `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`-д буулгана.

## 3. Шүүгчийн хяналтын хуудас

- [x] `result.json`-г `strongbox_attestation: true` харуулж байгаа ба гэрчилгээг итгэмжлэгдсэн үндэс рүү гинжийг баталгаажуулна уу.
- [x] Buildkite-н `android-strongbox-attestation#219` (анхны шүүрэлт) болон `#221` (Pixel 8 Pro дахин тест + S24 зураг авалт)-тай таарч байгаа сорилтын байтыг баталгаажуулна уу.
- [x] Техник хангамжийг зассаны дараа Pixel 8 Pro зураг авалтыг дахин ажиллуул (эзэмшигч: Техник хангамжийн лабораторийн ахлагч, 2026-02-13-нд дууссан).
- [x] Нокс профайлын зөвшөөрөл ирсний дараа Galaxy S24-ийн зураг авалтыг дуусгана уу (эзэмшигч: Төхөөрөмжийн лабораторийн үйл ажиллагаа, 2026-02-13-нд дууссан).

## 4. Түгээлт

- Энэ хураангуй болон хамгийн сүүлийн үеийн тайлангийн текст файлыг түншийн нийцлийн багцад хавсаргана (FISC шалгах хуудас §Өгөгдлийн оршин суугч).
- Зохицуулагчийн аудитад хариу өгөхдөө лавлагааны багцын замууд; Шифрлэгдсэн сувгаас гадуур түүхий гэрчилгээг бүү дамжуул.

## 5. Бүртгэлийг өөрчлөх

| Огноо | Өөрчлөх | Зохиогч |
|------|--------|--------|
| 2026-02-12 | Анхны JP багцын зураг авалт + тайлан. | Төхөөрөмжийн лабораторийн үйл ажиллагаа |