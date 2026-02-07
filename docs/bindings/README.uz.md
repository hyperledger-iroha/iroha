---
lang: uz
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf9773ecd75fc31ee89da58a3c5eda846b910eb6e131f1e042b565892e028f16
source_last_modified: "2025-12-29T18:16:35.062011+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SDK majburiy va armatura boshqaruvi

Yo'l xaritasidagi WP1-E "hujjatlar/bog'lashlar" ni saqlash uchun kanonik joy sifatida chaqiradi.
tillararo bog'lanish holati. Ushbu hujjat majburiy inventarni qayd etadi,
qayta tiklash buyruqlari, drift qo'riqchilari va dalillar joylashuvi, shuning uchun GPU pariteti
darvozalari (WP1-E/F/G) va o'zaro SDK kadans kengashi bitta ma'lumotnomaga ega.

## Umumiy to'siqlar
- **Kanonik o'yin kitobi:** `docs/source/norito_binding_regen_playbook.md` ta'riflaydi
  aylanish siyosati, kutilgan dalillar va Android uchun eskalatsiya ish jarayoni,
  Swift, Python va kelajakdagi bog'lanishlar.
- **Norito sxema pariteti:** `scripts/check_norito_bindings_sync.py` (bu orqali chaqiriladi)
  `scripts/check_norito_bindings_sync.sh` va CI da garovli
  `ci/check_norito_bindings_sync.sh`) Rust, Java yoki Python dasturlarini bloklaydi.
  sxema artefaktlari siljishi.
- **Kadens kuzatuvchisi:** `scripts/check_fixture_cadence.py` o'qiydi
  `artifacts/*_fixture_regen_state.json` fayllari va seshanba/juma (Android,
  Python) va Wed (Swift) oynalari, shuning uchun yo'l xaritasi darvozalarida tekshiriladigan vaqt belgilari mavjud.

## Bog'lovchi matritsa

| Bog'lash | Kirish nuqtalari | Fikstura / qayta tiklash buyrug'i | Drift qo'riqchilari | Dalil |
|---------|--------------|-------------------------|--------------|----------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (ixtiyoriy ravishda `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## Bog'lash tafsilotlari

### Android (Java)
Android SDK `java/iroha_android/` ostida ishlaydi va kanonik Norito ni iste'mol qiladi.
`scripts/android_fixture_regen.sh` tomonidan ishlab chiqarilgan armatura. Bu yordamchi eksport qiladi
Rust asboblar zanjiridagi yangi `.norito` bloblari, yangilanishlar
`artifacts/android_fixture_regen_state.json` va kadans metama'lumotlarini qayd qiladi
`scripts/check_fixture_cadence.py` va boshqaruv panellari iste'mol qiladi. Drift - bu
`scripts/check_android_fixtures.py` tomonidan aniqlangan (shuningdek, simli
`ci/check_android_fixtures.sh`) va `java/iroha_android/run_tests.sh` tomonidan, qaysi
JNI ulanishlarini, WorkManager navbatini takrorlashni va StrongBox zaxiralarini mashq qiladi.
Aylanish dalillari, muvaffaqiyatsizlik qaydlari va takroriy transkriptlar ostida yashaydi
`artifacts/android/fixture_runs/`.

### Swift (macOS/iOS)
`IrohaSwift/` `scripts/swift_fixture_regen.sh` orqali bir xil Norito foydali yuklarni aks ettiradi.
Skript aylanish egasi, kadans yorlig'i va manbani qayd qiladi (`live` va `archive`)
`artifacts/swift_fixture_regen_state.json` ichida va metama'lumotni ichiga yuboradi
kadans tekshiruvi. `scripts/swift_fixture_archive.py` saqlovchilarga yutishga imkon beradi
Zang bilan yaratilgan arxivlar; `scripts/check_swift_fixtures.py` va
`ci/check_swift_fixtures.sh` bayt darajasidagi paritet va SLA yosh chegaralarini qo'llaydi.
`scripts/swift_fixture_regen.sh` qo'llanma uchun `SWIFT_FIXTURE_EVENT_TRIGGER` ni qo'llab-quvvatlaydi
aylanishlar. Eskalatsiya ish jarayoni, KPI va asboblar paneli hujjatlashtirilgan
`docs/source/swift_parity_triage.md` va ostidagi kadans qisqartmasi
`docs/source/sdk/swift/`.

### Python
Python mijozi (`python/iroha_python/`) Android qurilmalarini baham ko'radi. Yugurish
`scripts/python_fixture_regen.sh` eng so'nggi `.norito` foydali yuklarini tortib oladi, yangilaydi
`python/iroha_python/tests/fixtures/` va kadans metama'lumotlarini chiqaradi
`artifacts/python_fixture_regen_state.json` bir marta yo'l xaritasidan keyingi birinchi aylanish
qo'lga olinadi. `scripts/check_python_fixtures.py` va
`python/iroha_python/scripts/run_checks.sh` darvoza pytest, mypy, ruff va armatura
mahalliy va CIda paritet. Oxir-oqibat hujjatlar (`docs/source/sdk/python/…`) va
majburiy regen o'yin kitobi Android bilan aylanishlarni qanday muvofiqlashtirishni tasvirlaydi
egalari.

### JavaScript
`javascript/iroha_js/` mahalliy `.norito` fayllariga tayanmaydi, lekin WP1-E treklari
uning chiqarilish dalillari, shuning uchun GPU CI yo'llari to'liq kelib chiqishini meros qilib oladi. Har bir nashr
`npm run release:provenance` orqali kelib chiqishini ushlaydi (quvvatlanadi
`javascript/iroha_js/scripts/record-release-provenance.mjs`), hosil qiladi va ishora qiladi
SBOM toʻplamlari `scripts/js_sbom_provenance.sh` bilan imzolangan quruq ishga tushirishni amalga oshiradi
(`scripts/js_signed_staging.sh`) va registr artefaktini tasdiqlaydi
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. Olingan metama'lumotlar
`artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/` ostidagi yerlar,
`artifacts/js/sbom/` va `artifacts/js/verification/`, deterministik
JS5/JS6 yo'l xaritasi va WP1-F test sinovlari uchun dalillar. Nashriyot o'yin kitobi
`docs/source/sdk/js/` avtomatlashtirishni bir-biriga bog'laydi.