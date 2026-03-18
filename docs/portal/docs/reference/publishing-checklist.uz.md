---
lang: uz
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9d7b44d46ef97c20058221aedf1f0b4a27ba85d204c3be4fe4933da31d9e207
source_last_modified: "2025-12-29T18:16:35.160066+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Nashr qilish roʻyxati

Dasturchi portalini yangilaganingizda ushbu nazorat roʻyxatidan foydalaning. ni ta'minlaydi
CI qurish, GitHub sahifalarini joylashtirish va qo'lda tutun sinovlari har bir bo'limni qamrab oladi
bir reliz yoki yo'l xaritasi muhim bosqichi qo'nadi oldin.

## 1. Mahalliy tekshirish

- `npm run sync-openapi -- --version=current --latest` (bir yoki bir nechta qo'shing
  Muzlatilgan surat uchun Torii OpenAPI o'zgarganda `--mirror=<label>` bayroqchalari).
- `npm run build` - `Build on Iroha with confidence` qahramon nusxasini tasdiqlang
  `build/index.html` da paydo bo'ladi.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – tasdiqlang
  nazorat summasi manifesti (yuklab olingan CIni sinovdan o'tkazishda `--descriptor`/`--archive` qo'shing
  artefaktlar).
- `npm run serve` - tekshirish summasi bilan himoyalangan oldindan ko'rish yordamchisini ishga tushiradi.
  `docusaurus serve` ga qo'ng'iroq qilishdan oldin manifest, shuning uchun sharhlovchilar hech qachon ko'rib chiqmaydi
  imzosiz surat (`serve:verified` taxallus aniq qo'ng'iroqlar uchun qoladi).
- `npm run start` va jonli qayta yuklash orqali teggan belgini joyida tekshiring
  server.

## 2. So'rovni tekshirish

- `.github/workflows/check-docs.yml` da `docs-portal-build` ishi muvaffaqiyatli bajarilganligini tekshiring.
- `ci/check_docs_portal.sh` ishlaganligini tasdiqlang (CI jurnallari qahramon tutunini tekshirishni ko'rsatadi).
- Manifest (`build/checksums.sha256`) yuklanganligini oldindan ko'rish ish jarayonini ta'minlang va
  oldindan ko‘rishni tekshirish skripti muvaffaqiyatli bo‘ldi (CI jurnallari
  `scripts/preview_verify.sh` chiqishi).
- GitHub Pages muhitidan chop etilgan oldindan ko'rish URL manzilini PRga qo'shing
  tavsifi.

## 3. Bo'limni o'chirish

| Bo'lim | Egasi | Tekshirish ro'yxati |
|---------|-------|-----------|
| Bosh sahifa | DevRel | Qahramon nusxasi renderlari, tezkor ishga tushirish kartalari joriy marshrutlarga havola, CTA tugmalari hal qiladi. |
| Norito | Norito WG | Umumiy koʻrinish va ishga tushirish boʻyicha qoʻllanmalar eng soʻnggi CLI bayroqlari va Norito sxema hujjatlariga havola qiladi. |
| SoraFS | Saqlash jamoasi | Tez boshlash tugagunga qadar ishlaydi, manifest hisobot maydonlari hujjatlashtirilgan, simulyatsiya ko'rsatmalarini olish tasdiqlangan. |
| SDK qo'llanmalari | SDK yetakchilari | Rust/Python/JS qo'llanmalari joriy misollarni tuzadi va jonli reposlarga havola qiladi. |
| Malumot | Docs/DevRel | Indeksda eng yangi xususiyatlar ro'yxati keltirilgan, Norito kodek mos yozuvlar `norito.md`. |
| Artifaktni oldindan ko'rish | Docs/DevRel | PRga biriktirilgan `docs-portal-preview` artefakti, tutun tekshiruvi oʻtishi, havola sharhlovchilar bilan ulashilgan. |
| Xavfsizlik & Sinab ko'ring sandbox | Docs/DevRel · Xavfsizlik | OAuth qurilma kodiga kirish sozlandi (`DOCS_OAUTH_*`), `security-hardening.md` nazorat roʻyxati bajarildi, CSP/Ishonchli turlar sarlavhalari `npm run build` yoki `npm run probe:portal` orqali tasdiqlandi. |

Har bir qatorni PR ko'rib chiqishingizning bir qismi sifatida belgilang yoki har qanday keyingi vazifalarni qayd qiling
kuzatish aniqligicha qoladi.

## 4. Chiqarish qaydlari

- `https://docs.iroha.tech/` (yoki muhit URL manzilini) qo'shing
  tarqatish ishidan) chiqarish qaydlari va holat yangilanishlarida.
- Har qanday yangi yoki o'zgartirilgan bo'limlarga aniq qo'ng'iroq qiling, shunda quyi oqim guruhlari qaerdaligini bilishadi
  o'zlarining tutun sinovlarini qayta o'tkazish uchun.