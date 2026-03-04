---
lang: ba
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

# Көслө Box аттестация дәлилдәре — Япония һүҙҙәре

| Ялан | Ҡиммәте |
|------|-------|
| Баһалау тәҙрәһе | 2026-02-10 – 2026-02-12 |
| Артефакт Урыны | `artifacts/android/attestation/<device-tag>/<date>/` (`docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md` өсөн өйөм форматында) |
| Ҡулға алыу ҡоралы | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| Рецензенттар | Аппарат лабораторияһы етәксеһе, үтәү & Юридик (JP) |

## 1. Ҡулға алыу процедураһы

1. StrongBox матрицаһында күрһәтелгән һәр ҡоролмала һынау генерациялау һәм аттестация пакетын тотоу:
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. Коммит өйөм метамағлүмәттәре (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`) дәлилдәр ағасы.
3. CI ярҙамсыһын офлайн бөтә өйөмдәрҙе яңынан тикшерергә кәрәк:
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. Ҡоролма резюмеһы (2026-02-12)

| Ҡоролма Тег | Модель / Көслө Box | Бундл юл | Һөҙөмтә | Иҫкәрмәләр |
|----------|--------------------------------|--------|-----------|
| `pixel6-strongbox-a` | Пиксель 6 / Тенсор G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ Үткән (аппарат-арҡа) | 2025-03-05 OS патч. |
| `pixel7-strongbox-a` | Пиксель 7 / Тенсор G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ Үткән | Беренсел CI һыҙат кандидаты; температура спец. |
| `pixel8pro-strongbox-a` | Пиксель 8 Pro / Тенсор G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ Үткән (ҡабаттан тест) | USB-C концентраторы алмаштырылған; `android-strongbox-attestation#221` Bublykite үткән өйөмөн төшөргән. |
| `s23-strongbox-a` | Галактика S23 / Snapdragon 8 Ген 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ Үткән | Нокс аттестация профиле 2026-02-09 импортлана. |
| `s24-strongbox-a` | Галактика S24 / Snapdragon 8 Ген 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ Үткән | Нокс аттестация профиле импортланған; CI һыҙат хәҙер йәшел. |

Ҡоролма тегтары картаһы `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`.

## 3. Рецензент тикшерелгән исемлек

- [x] `result.json` тикшерелгән `strongbox_attestation: true` һәм ышаныслы тамырға сертификаттар сылбырын күрһәтә.
- [x] Раҫлау һынауы байттар матч Butarkite эшләй `android-strongbox-attestation#219` (башланғыс һыпыртыу) һәм `#221` (Pixel 8 Pro retest + S24 тотоу).
- [x] Ҡабаттан идара итеү Pixel 8 Pro тотоуҙан һуң аппарат төҙәтеү (хужа: Аппарат лабораторияһы етәксеһе, тамамланы 2026-02-13).
- [x] Тулы Galaxy S24 тотоу бер тапҡыр Нокс профиле раҫлау килә (хужа: Ҡоролма лабораторияһы Ops, тамамланған 2026-02-13).

## 4. Таратыу

- Был резюме беркетергә плюс һуңғы отчет текст файлы партнер үтәү пакеттары (FISC тикшерелгән исемлек §Мәғлүмәттәр резидентлығы).
- Һылтанма өйөмдәре юлдар ҡасан яуап көйләү аудиторҙары; шифрланған каналдарҙан ситтә сеймал сертификаттарын тапшырмағыҙ.

## 5.

| Дата | Үҙгәреш | Автор |
|------|--------|--------|
| 2026-02-12 | Башланғыс JP пакет тотоу + отчет. | Ҡоролма лабораторияһы Ops |