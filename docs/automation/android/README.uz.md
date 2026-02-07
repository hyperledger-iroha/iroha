---
lang: uz
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 676798a4cf7c3e7737a0f80640f3f268a2f625f92afdd359ac528881d2aeb046
source_last_modified: "2025-12-29T18:16:35.060950+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Hujjatlarni Avtomatlashtirishning Baseline (AND5)

Yo‘l xaritasining AND5 bandi hujjatlashtirish, mahalliylashtirish va nashr etishni talab qiladi
AND6 (CI & Compliance) ishga tushirilgunga qadar tekshirilishi mumkin bo'lgan avtomatlashtirish. Ushbu jild
AND5/AND6 havolasi bo'lgan buyruqlar, artefaktlar va dalillar tartibini qayd etadi,
olingan rejalarni aks ettiradi
`docs/source/sdk/android/developer_experience_plan.md` va
`docs/source/sdk/android/parity_dashboard_plan.md`.

## Quvurlar va buyruqlar

| Vazifa | Buyruq(lar) | Kutilayotgan artefaktlar | Eslatmalar |
|------|------------|-------------------|-------|
| Mahalliylashtirish stub sinxronlash | `python3 scripts/sync_docs_i18n.py` (ixtiyoriy ravishda har bir ish uchun `--lang <code>` o'tishi) | `docs/automation/android/i18n/<timestamp>-sync.log` ostida saqlangan jurnal fayli va tarjima qilingan stub majburiyatlari | `docs/i18n/manifest.json` ni tarjima qilingan stublar bilan sinxronlashtiradi; jurnal tegilgan til kodlarini va asosiy chiziqda olingan git majburiyatini qayd qiladi. |
| Norito armatura + paritetni tekshirish | `ci/check_android_fixtures.sh` (`python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json` o'raladi) | Yaratilgan JSON xulosasini `docs/automation/android/parity/<stamp>-summary.json` | ga nusxalash `java/iroha_android/src/test/resources` foydali yuklarni, manifest xeshlarini va imzolangan armatura uzunliklarini tasdiqlaydi. Xulosani `artifacts/android/fixture_runs/` ostida kadans dalillari bilan birga ilova qiling. |
| Namuna manifest va nashriyot isboti | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (sinovlar + SBOM + kelib chiqishi) | `docs/automation/android/samples/<version>/` ostida saqlangan `docs/source/sdk/android/samples/` dan olingan manba toʻplami metamaʼlumotlari va `sample_manifest.json` | AND5 namunaviy ilovalarini bog‘lang va avtomatlashtirishni birga chiqaring — beta tekshiruvi uchun yaratilgan manifest, SBOM xesh va kelib chiqish jurnalini yozib oling. |
| Parite boshqaruv paneli tasmasi | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` keyin `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | `metrics.prom` suratini yoki Grafana eksport JSON faylini `docs/automation/android/parity/<stamp>-metrics.prom` ga nusxalash | AND5/AND7 boshqaruvi noto‘g‘ri topshirish hisoblagichlari va telemetriya qabul qilinishini tekshirishi uchun asboblar paneli rejasini ta’minlaydi. |

## Dalillarni olish

1. **Hamma narsani vaqt tamg‘asi bilan belgilang.** UTC vaqt belgilaridan foydalanib fayllarga nom bering
   (`YYYYMMDDTHHMMSSZ`) shunday parite panellari, boshqaruv protokollari va nashr etilgan
   docs bir xil ishga deterministik tarzda murojaat qilishi mumkin.
2. **Ma'lumotnoma topshiriladi.** Har bir jurnal ishga tushirishning git commit xeshini o'z ichiga olishi kerak.
   plus har qanday tegishli konfiguratsiya (masalan, `ANDROID_PARITY_PIPELINE_METADATA`).
   Maxfiylik tahrir qilishni talab qilganda, eslatmani qo'shing va xavfsiz omborga havola qiling.
3. **Minimal kontekstni arxivlash.** Biz faqat tuzilgan xulosalarni tekshiramiz (JSON,
   `.prom`, `.log`). Og'ir artefaktlar (APK to'plamlari, skrinshotlar) ichida qolishi kerak
   `artifacts/` yoki jurnalda qayd etilgan imzolangan xesh bilan ob'ektni saqlash.
4. **Holat yozuvlarini yangilash.** `status.md` da AND5 bosqichlari oldinga siljiganida, iqtibos keltiring
   mos keladigan fayl (masalan, `docs/automation/android/parity/20260324T010203Z-summary.json`)
   Shunday qilib, auditorlar CI jurnallarini qirib tashlamasdan bazani kuzatishi mumkin.

Ushbu tartibdan so'ng mavjud bo'lgan "hujjatlar/avtomatlashtirish asoslari."
audit” sharti, bu AND6 Android hujjatlashtirish dasturini keltirib chiqaradi va saqlaydi
e'lon qilingan rejalar bilan qulflangan.