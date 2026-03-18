---
lang: uz
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-05T09:28:11.999717+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android relizlar roʻyxati (AND6)

Ushbu nazorat roʻyxati **AND6 — CI & Compliance Hardening** eshiklarini qamrab oladi
`roadmap.md` (§Asosiylik 5). U Android SDK relizlarini Rust bilan moslashtiradi
CI ishlarini, muvofiqlik artefaktlarini talaffuz qilish orqali RFC taxminlarini chiqarib tashlang
GAdan oldin biriktirilishi kerak bo'lgan qurilma-laboratoriya dalillari va kelib chiqish to'plamlari,
LTS yoki tuzatish poyezdi oldinga siljiydi.

Ushbu hujjat bilan birgalikda foydalaning:

- `docs/source/android_support_playbook.md` — chiqarish taqvimi, SLA va
  eskalatsiya daraxti.
- `docs/source/android_runbook.md` - kundalik operatsion kitoblar.
- `docs/source/compliance/android/and6_compliance_checklist.md` - regulyator
  artefakt inventarizatsiyasi.
- `docs/source/release_dual_track_runbook.md` — ikki yoʻlli reliz boshqaruvi.

## 1. Bir qarashda sahna darvozalari

| Bosqich | Kerakli darvozalar | Dalil |
|-------|----------------|----------|
| **T−7 kun (oldindan muzlatish)** | 14 kun davomida tungi `ci/run_android_tests.sh` yashil; `ci/check_android_fixtures.sh`, `ci/check_android_samples.sh` va `ci/check_android_docs_i18n.sh` o'tish; lint/qaramlik skanerlari navbatda. | Buildkite asboblar paneli, armatura farqi hisoboti, skrinshotning namunaviy suratlari. |
| **T−3 kun (RC aksiyasi)** | Qurilma-laboratoriya bandi tasdiqlandi; StrongBox attestatsiyasi CI ishga tushirilishi (`scripts/android_strongbox_attestation_ci.sh`); Rejalashtirilgan uskunada bajariladigan roboelektrik/instrumental to'plamlar; `./gradlew lintRelease ktlintCheck detekt dependencyGuard` toza. | Qurilma matritsasi CSV, attestatsiya toʻplami manifest, Gradle hisobotlari `artifacts/android/lint/<version>/` ostida arxivlangan. |
| **T−1 kun (ketish/yo‘q)** | Telemetriyani tahrirlash holati to'plami yangilandi (`scripts/telemetry/check_redaction_status.py --write-cache`); `and6_compliance_checklist.md` bo'yicha yangilangan muvofiqlik artefaktlari; kelib chiqishi mashqi yakunlandi (`scripts/android_sbom_provenance.sh --dry-run`). | `docs/source/compliance/android/evidence_log.csv`, telemetriya holati JSON, quruq ishga tushirish jurnali. |
| **T0 (GA/LTS kesish)** | `scripts/publish_android_sdk.sh --dry-run` tugallandi; kelib chiqishi + SBOM imzolangan; Eksport qilingan va borish/bermaslik daqiqalariga biriktirilgan nazorat roʻyxati; `ci/sdk_sorafs_orchestrator.sh` tutun ishi yashil. | RFC qo'shimchalarini, Sigstore to'plamini, `artifacts/android/` ostida qabul qilish artefaktlarini chiqaring. |
| **T+1 kun (kesishdan keyingi)** | Tuzatishning tayyorligi tasdiqlandi (`scripts/publish_android_sdk.sh --validate-bundle`); asboblar panelidagi farqlar ko'rib chiqildi (`ci/check_android_dashboard_parity.sh`); dalillar paketi `status.md` ga yuklangan. | Boshqaruv paneli diff eksporti, `status.md` yozuviga havola, arxivlangan nashr paketi. |

## 2. CI va sifat darvozasi matritsasi| Darvoza | Buyruq(lar) / Skript | Eslatmalar |
|------|--------------------|-------|
| Birlik + integratsiya testlari | `ci/run_android_tests.sh` (`ci/run_android_tests.sh` o'raladi) | `artifacts/android/tests/test-summary.json` + test jurnalini chiqaradi. Norito kodek, navbat, StrongBox zaxirasi va Torii mijoz jabduqlari sinovlarini o'z ichiga oladi. Kechasi va teglashdan oldin talab qilinadi. |
| Fikstura pariteti | `ci/check_android_fixtures.sh` (`scripts/check_android_fixtures.py` o'raladi) | Qayta tiklangan Norito moslamalari Rust kanonik to'plamiga mos kelishini ta'minlaydi; darvoza ishlamay qolganda JSON farqini biriktiring. |
| Ilova namunalari | `ci/check_android_samples.sh` | `examples/android/{operator-console,retail-wallet}` quradi va `scripts/android_sample_localization.py` orqali mahalliylashtirilgan skrinshotlarni tasdiqlaydi. |
| Hujjatlar/I18N | `ci/check_android_docs_i18n.sh` | Guards README + mahalliy tezkor ishga tushirish. Hujjat tahrirlari reliz bo'limiga tushganidan keyin yana ishga tushiring. |
| Boshqaruv paneli pariteti | `ci/check_android_dashboard_parity.sh` | CI/eksport qilingan ko'rsatkichlar Rust hamkasblari bilan mos kelishini tasdiqlaydi; T+1 tekshiruvi vaqtida talab qilinadi. |
| SDK qabul qilish tutuni | `ci/sdk_sorafs_orchestrator.sh` | Joriy SDK bilan ko'p manbali Sorafs orkestr bog'lashlarini mashq qiladi. Sahnalashtirilgan artefaktlarni yuklashdan oldin talab qilinadi. |
| Attestatsiyani tekshirish | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | `artifacts/android/attestation/**` ostida StrongBox/TEE attestatsiya to'plamlarini jamlaydi; xulosani GA paketlariga biriktiring. |
| Qurilma-laboratoriya uyasi tekshiruvi | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | Paketlarni chiqarish uchun dalillarni biriktirishdan oldin asboblar to'plamlarini tasdiqlaydi; CI `fixtures/android/device_lab/slot-sample` (temetriya/attestatsiya/navbat/loglar + `sha256sum.txt`) namuna uyasiga qarshi ishlaydi. |

> **Maslahat:** bu ishlarni `android-release` Buildkite quvur liniyasiga qo'shing, shunda
> muzlatish haftalari avtomatik ravishda har bir eshikni bo'shatish shoxchasi uchi bilan qayta ishga tushiradi.

Konsolidatsiyalangan `.github/workflows/android-and6.yml` ishi lintni boshqaradi,
Har bir PR/pushda test-to'plam, attestatsiya-xulosa va qurilma-laboratoriya uyasi tekshiruvlari
Android manbalariga tegish, `artifacts/android/{lint,tests,attestation,device_lab}/` ostida dalillarni yuklash.

## 3. Lint & Dependency Scans

Repo ildizidan `scripts/android_lint_checks.sh --version <semver>` ni ishga tushiring. The
skript bajariladi:

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- Hisobotlar va qaramlik himoyasi natijalari ostida arxivlanadi
  Chiqarish uchun `artifacts/android/lint/<label>/` va `latest/` simli havolasi
  quvurlar.
- Muvaffaqiyatsiz lint topilmalari tuzatishni yoki nashrga kirishni talab qiladi
  Qabul qilingan xavfni hujjatlashtiruvchi RFC (Release Engineering + Program tomonidan tasdiqlangan
  Qo'rg'oshin).
- `dependencyGuardBaseline` qaramlik blokirovkasini qayta tiklaydi; farqni biriktiring
  ketmoq/kelmagan paketga.

## 4. Device Lab & StrongBox qamrovi

1. Pixel + Galaxy qurilmalarini havola qilingan sig'im kuzatuvchisi yordamida zahiraga oling
   `docs/source/compliance/android/device_lab_contingency.md`. Chiqarishlarni bloklaydi
   agar `.
3. Asboblar matritsasini ishga tushiring (qurilmada to'plam/ABI ro'yxatini hujjatlang
   kuzatuvchi). Qayta urinishlar muvaffaqiyatli bo'lsa ham, hodisalar jurnalidagi nosozliklarni yozib oling.
4. Firebase test laboratoriyasiga qaytish zarur bo'lsa, chiptani rasmiylashtiring; chiptani bog'lang
   quyidagi nazorat ro'yxatida.

## 5. Muvofiqlik va telemetriya artefaktlari- Evropa Ittifoqi uchun `docs/source/compliance/android/and6_compliance_checklist.md` ga rioya qiling
  va JP taqdimotlari. `docs/source/compliance/android/evidence_log.csv` yangilang
  xeshlar + Buildkite ish URL manzillari bilan.
- Telemetriyani tahrirlash dalillarini yangilang
  `skriptlar/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`.
  Olingan JSON-ni ostida saqlang
  `artifacts/android/telemetry/<version>/status.json`.
- Sxemaning farqini yozib oling
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  Rust eksportchilari bilan tenglikni isbotlash.

## 6. kelib chiqishi, SBOM va nashriyot

1. Nashr qilish quvurini quriting:

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. SBOM + Sigstore manbasini yarating:

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. `artifacts/android/provenance/<semver>/manifest.json` ni biriktiring va imzolang
   RFC versiyasiga `checksums.sha256`.
4. Haqiqiy Maven omboriga o'tishda, qayta ishga tushiring
   `scripts/publish_android_sdk.sh`, `--dry-run` holda, konsolni suratga oling
   jurnaliga kiring va natijada olingan artefaktlarni `artifacts/android/maven/<semver>` ga yuklang.

## 7. Taqdim etish paketi shabloni

Har bir GA/LTS/tuzatish versiyasi quyidagilarni o'z ichiga olishi kerak:

1. **Toʻldirilgan nazorat roʻyxati** — ushbu fayl jadvalidan nusxa oling, har bir bandga belgi qoʻying va havola qiling
   qo'llab-quvvatlovchi artefaktlarga (Buildkite ishga tushirish, jurnallar, doc diffs).
2. **Qurilma laboratoriyasi dalillari** — attestatsiya hisoboti xulosasi, bron qilish jurnali va
   har qanday favqulodda faollashuvlar.
3. **Telemetriya paketi** — redaktsiya holati JSON, sxema farqi, havola
   `docs/source/sdk/android/telemetry_redaction.md` yangilanishlari (agar mavjud bo'lsa).
4. **Muvofiqlik artefaktlari** — muvofiqlik papkasiga qo‘shilgan/yangilangan yozuvlar
   shuningdek, yangilangan dalillar jurnali CSV.
5. **Provenance bundle** — SBOM, Sigstore imzosi va `checksums.sha256`.
6. **Relizlar sarhisobi** — `status.md` xulosasiga biriktirilgan bir sahifali umumiy koʻrinish
   yuqoridagi (sana, versiya, har qanday bekor qilingan eshiklarning ta'kidlashi).

Paketni `artifacts/android/releases/<version>/` ostida saqlang va unga havola qiling
`status.md` va RFC versiyasida.

- `scripts/run_release_pipeline.py --publish-android-sdk ...` avtomatik ravishda
  oxirgi lint arxivini (`artifacts/android/lint/latest`) va
  muvofiqlik dalillari `artifacts/android/releases/<version>/` tizimiga kiring, shuning uchun
  yuborish paketi har doim kanonik joyga ega.

---

**Eslatma:** yangi CI ishlari, muvofiqlik artefaktlari,
yoki telemetriya talablari qo'shiladi. Yo'l xaritasi AND6 bandi togacha ochiq qoladi
Tekshirish ro'yxati va tegishli avtomatlashtirish ketma-ket ikkita nashr uchun barqarorligini isbotlaydi
poezdlar.