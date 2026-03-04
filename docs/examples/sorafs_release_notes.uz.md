---
lang: uz
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 303a947895c10c7673b98e9187c3431c4012093c69d899252c121b53f9c48bb1
source_last_modified: "2026-01-05T09:28:11.823299+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS CLI va SDK — Relizlar haqida eslatmalar (v0.1.0)

## E'tiborga molik
- `sorafs_cli` endi butun qadoqlash quvurini o'rab oladi (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`) shuning uchun CI yuguruvchilari
  buyurtma yordamchilar o'rniga bitta ikkilik. Yangi kalitsiz imzolash oqimi standart hisoblanadi
  `SIGSTORE_ID_TOKEN`, GitHub Actions OIDC provayderlarini tushunadi va deterministik chiqaradi
  Imzo paketi bilan birga xulosa JSON.
- `sorafs_car` ning bir qismi sifatida ko'p manbali * ko'rsatkichlar paneli* yuboriladi: u normallashadi
  provayder telemetriyasi, qobiliyat jazolarini qo'llaydi, JSON/Norito hisobotlarini davom ettiradi va
  orkestr simulyatorini (`sorafs_fetch`) umumiy ro'yxatga olish dastagi orqali ta'minlaydi.
  `fixtures/sorafs_manifest/ci_sample/` ostidagi armaturalar deterministikni ko'rsatadi
  CI/CD farq qilishi kutilayotgan kirish va chiqishlar.
- Chiqarishni avtomatlashtirish `ci/check_sorafs_cli_release.sh` va kodlangan
  `scripts/release_sorafs_cli.sh`. Har bir nashr endi manifest to'plamini arxivlaydi,
  imzo, `manifest.sign/verify` xulosalari va skorbord snapshoti shunday boshqaruv
  sharhlovchilar quvurni qayta ishga tushirmasdan artefaktlarni kuzatishi mumkin.

## Yangilash bosqichlari
1. Ish joyingizdagi tekislangan qutilarni yangilang:
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. Fmt/clippy/test qamrovini tasdiqlash uchun chiqarish eshigini mahalliy (yoki CIda) qayta ishga tushiring:
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. Belgilangan konfiguratsiya yordamida imzolangan artefaktlar va xulosalarni qayta tiklang:
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   Yangilangan to'plamlarni/dalillarni `fixtures/sorafs_manifest/ci_sample/` ga nusxalash
   kanonik armatura yangilanishlarini chiqarish.

## Tasdiqlash
- Chiqarish eshigi majburiyati: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` darvoza muvaffaqiyatli bo'lgandan so'ng darhol).
- `ci/check_sorafs_cli_release.sh` chiqishi: arxivlangan
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (chiqarish to'plamiga biriktirilgan).
- Manifest to'plami dayjesti: `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`).
- Tasdiqlash xulosasi dayjesti: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`).
- Manifest dayjesti (quyi oqimdagi attestatsiyani o'zaro tekshirish uchun):
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (`manifest.sign.summary.json` dan).

## Operatorlar uchun eslatmalar
- Torii shlyuzi endi `X-Sora-Chunk-Range` qobiliyat sarlavhasini qo'llaydi. Yangilash
  ruxsat etilgan ro'yxatlar, shuning uchun yangi oqim tokenlarini taqdim etuvchi mijozlar qabul qilinadi; eski tokenlar
  diapazonsiz da'vo to'xtatiladi.
- `scripts/sorafs_gateway_self_cert.sh` manifest tekshiruvini birlashtiradi. Yugurayotganda
  o'z-o'zini tasdiqlovchi jabduqlar, yangi yaratilgan manifest to'plamini o'rash uchun taqdim eting
  imzo driftida tezda muvaffaqiyatsizlikka uchraydi.
- Telemetriya asboblar paneli yangi jadval eksportini (`scoreboard.json`) qabul qilishi kerak.
  provayderning muvofiqligi, vazni tayinlanishi va rad etish sabablarini yarashtiring.
- Har bir chiqishda to'rtta kanonik xulosani arxivlang:
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. Boshqaruv chiptalari davomida ushbu aniq fayllarga havola qilinadi
  tasdiqlash.

## Minnatdorchilik
- Saqlash jamoasi - CLI-ni oxirigacha konsolidatsiyalash, chunk-plan renderer va skorbord
  telemetriya sanitariya-tesisat.
- Tooling WG - chiqarish quvuri (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) va deterministik armatura to'plami.
- Gateway Operations - imkoniyatlarni aniqlash, oqim-token siyosatini ko'rib chiqish va yangilangan
  o'z-o'zini sertifikatlash o'yin kitoblari.