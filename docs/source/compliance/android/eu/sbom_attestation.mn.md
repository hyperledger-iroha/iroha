---
lang: mn
direction: ltr
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-05T09:28:12.002687+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM & Provenance Attestation — Android SDK

| Талбай | Үнэ цэнэ |
|-------|-------|
| Хамрах хүрээ | Android SDK (`java/iroha_android`) + жишээ програмууд (`examples/android/*`) |
| Ажлын урсгалын эзэмшигч | Хувилбарын инженерчлэл (Алексей Морозов) |
| Хамгийн сүүлд баталгаажуулсан | 2026-02-11 (Buildkite `android-sdk-release#4821`) |

## 1. Үе үеийн ажлын урсгал

Туслах скриптийг ажиллуул (AND6 автоматжуулалтад нэмсэн):

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

Скрипт нь дараахь зүйлийг гүйцэтгэдэг.

1. `ci/run_android_tests.sh` болон `scripts/check_android_samples.sh`-г ажиллуулна.
2. CycloneDX SBOM-уудыг бүтээхийн тулд `examples/android/` доорх Gradle боолтыг дуудна.
   `:android-sdk`, `:operator-console`, болон `:retail-wallet` нийлүүлсэн
   `-PversionName`.
3. SBOM бүрийг каноник нэртэй `artifacts/android/sbom/<sdk-version>/` руу хуулна.
   (`iroha-android.cyclonedx.json` гэх мэт).

## 2. Гарал үүсэл & Гарын үсэг

Ижил скрипт нь SBOM болгонд `cosign sign-blob --bundle <file>.sigstore --yes`-ээр гарын үсэг зурдаг
мөн очих газрын лавлахад `checksums.txt` (SHA-256) ялгаруулдаг. `COSIGN`-г тохируулна уу
хэрэв хоёртын файл нь `$PATH`-ээс гадуур амьдардаг бол орчны хувьсагч. Скрипт дууссаны дараа,
багц/шалгах нийлбэрийн зам дээр нэмээд Buildkite ажиллуулах ID-г бичнэ үү
`docs/source/compliance/android/evidence_log.csv`.

## 3. Баталгаажуулах

Нийтлэгдсэн SBOM-г баталгаажуулахын тулд:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

SHA гаралтыг `checksums.txt`-д заасан утгатай харьцуулна уу. Шүүгчид мөн хараат байдлын дельтануудыг санаатай болгохын тулд SBOM-ийг өмнөх хувилбараас ялгадаг.

## 4. Нотлох баримтын агшин зураг (2026-02-11)

| Бүрэлдэхүүн хэсэг | SBOM | SHA-256 | Sigstore багц |
|----------|------|---------|-----------------|
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | SBOM | хажууд хадгалагдсан `.sigstore` багц
| Операторын консолын жишээ | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| Жижиглэнгийн хэтэвчний дээж | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Buildkite-ээс авсан хэшүүдийг `android-sdk-release#4821` ажиллуулдаг; дээрх баталгаажуулах тушаалаар хуулбарлана.)*

## 5. Гайхалтай ажил

- GA-аас өмнө суллах дамжуулах хоолой доторх SBOM + cosign алхамуудыг автоматжуулах.
- AND6 нь хяналтын хуудсыг дууссан гэж тэмдэглэсний дараа SBOM-уудыг нийтийн олдворын хувин руу толилуулаарай.
- Түншүүдэд зориулсан хувилбарын тэмдэглэлээс SBOM татаж авах байршлуудыг холбохын тулд Docs-тэй хамтран ажиллана уу.