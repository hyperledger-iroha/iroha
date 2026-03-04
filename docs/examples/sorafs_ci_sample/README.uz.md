---
lang: uz
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

# SoraFS CI namunaviy moslamalari

Ushbu katalog namunadan yaratilgan deterministik artefaktlarni to'playdi
`fixtures/sorafs_manifest/ci_sample/` ostida foydali yuk. To'plam buni ko'rsatadi
CI ish oqimlari bajaradigan end-to-end SoraFS qadoqlash va imzolash quvuri.

## Artefakt inventarizatsiyasi

| Fayl | Tavsif |
|------|-------------|
| `payload.txt` | Armatura skriptlari tomonidan ishlatiladigan manba yuki (oddiy matn namunasi). |
| `payload.car` | `sorafs_cli car pack` tomonidan chiqarilgan CAR arxivi. |
| `car_summary.json` | Xulosa `car pack` tomonidan yaratilgan va metamaʼlumotlarning parchalarini yozib oladi. |
| `chunk_plan.json` | Bo'lak diapazonlari va provayder kutganlarini tavsiflovchi JSON-ni olib kelish rejasi. |
| `manifest.to` | `sorafs_cli manifest build` tomonidan ishlab chiqarilgan Norito manifest. |
| `manifest.json` | Nosozliklarni tuzatish uchun odam o‘qiy oladigan manifest renderi. |
| `proof.json` | `sorafs_cli proof verify` tomonidan chiqarilgan PoR xulosasi. |
| `manifest.bundle.json` | `sorafs_cli manifest sign` tomonidan yaratilgan kalitsiz imzo toʻplami. |
| `manifest.sig` | Manifestga mos keladigan ajratilgan Ed25519 imzosi. |
| `manifest.sign.summary.json` | Imzolash paytida chiqarilgan CLI xulosasi (xeshlar, toʻplam metamaʼlumotlari). |
| `manifest.verify.summary.json` | `manifest verify-signature` dan CLI xulosasi. |

Chiqarish eslatmalarida va hujjatlarda keltirilgan barcha dayjestlar manbadan olingan
bu fayllar. `ci/check_sorafs_cli_release.sh` ish jarayoni xuddi shunday qayta tiklanadi
artefaktlar va ularni qabul qilingan versiyalardan farq qiladi.

## Armatura regeneratsiyasi

Armatura to'plamini qayta tiklash uchun ombor ildizidan quyidagi buyruqlarni bajaring.
Ular `sorafs-cli-fixture` ish oqimi tomonidan ishlatiladigan qadamlarni aks ettiradi:

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

Agar biron bir qadam turli xil xeshlarni keltirib chiqarsa, armaturani yangilashdan oldin tekshirib ko'ring.
CI ish oqimlari regressiyalarni aniqlash uchun deterministik natijalarga tayanadi.

## Kelajak qamrovi

Qo'shimcha chunker profillari va isbot formatlari yo'l xaritasini tugatgandan so'ng,
ularning kanonik moslamalari ushbu katalog ostida qo'shiladi (masalan,
`sorafs.sf2@1.0.0` (qarang: `fixtures/sorafs_manifest/ci_sample_sf2/`) yoki PDP
oqimli dalillar). Har bir yangi profil bir xil tuzilishga amal qiladi - foydali yuk, CAR,
reja, manifest, dalillar va imzo artefaktlari - shuning uchun quyi oqimdagi avtomatlashtirish mumkin
maxsus skriptsiz diff relizlari.