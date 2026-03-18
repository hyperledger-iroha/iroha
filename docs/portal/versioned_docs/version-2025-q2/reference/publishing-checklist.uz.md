---
lang: uz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9be80e0138e1e8aa453c703c53069837b24f29f6b463d14c846a01b015918f24
source_last_modified: "2025-12-29T18:16:35.907815+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Nashr qilish roʻyxati

Dasturchi portalini yangilaganingizda ushbu nazorat roʻyxatidan foydalaning. ni ta'minlaydi
CI qurish, GitHub sahifalarini joylashtirish va qo'lda tutun sinovlari har bir bo'limni qamrab oladi
bir reliz yoki yo'l xaritasi muhim bosqichi qo'nadi oldin.

## 1. Mahalliy tekshirish

- `npm run sync-openapi` (Torii OpenAPI o'zgarganda).
- `npm run build` - `Build on Iroha with confidence` qahramon nusxasini tasdiqlang
  `build/index.html` da paydo bo'ladi.
- `cd build && sha256sum -c checksums.sha256` – nazorat summasi manifestini tekshiring
  qurilgan.
- `npm run start` va jonli qayta yuklash orqali teggan belgini joyida tekshiring
  server.

## 2. So'rovni tekshirish

- `.github/workflows/check-docs.yml` da `docs-portal-build` ishi muvaffaqiyatli bajarilganligini tekshiring.
- `ci/check_docs_portal.sh` ishga tushirilganligini tasdiqlang (CI jurnallari qahramon tutunini tekshirishni ko'rsatadi).
- Manifest (`build/checksums.sha256`) yuklanganligini oldindan ko'rish ish jarayoniga ishonch hosil qiling va
  `sha256sum -c` CI da o'tgan.
- GitHub Pages muhitidan chop etilgan oldindan ko'rish URL manzilini PRga qo'shing
  tavsifi.

## 3. Bo'limni o'chirish

| Bo'lim | Egasi | Tekshirish ro'yxati |
|---------|-------|-----------|
| Bosh sahifa | DevRel | Qahramon nusxasi renderlari, tezkor ishga tushirish kartalari joriy marshrutlarga havola, CTA tugmalari hal qiladi. |
| Norito | Norito WG | Umumiy koʻrinish va ishga tushirish boʻyicha qoʻllanmalar eng soʻnggi CLI bayroqlari va Norito sxema hujjatlariga havola qiladi. |
| SoraFS | Saqlash jamoasi | Tez boshlash tugagunga qadar ishlaydi, manifest hisobot maydonlari hujjatlashtirilgan, simulyatsiya ko'rsatmalarini olish tasdiqlangan. |
| SDK qo'llanmalari | SDK yetakchilari | Rust/Python/JS qo'llanmalari joriy misollarni tuzadi va jonli reposlarga havola qiladi. |
| Malumot | Docs/DevRel | Indeks eng yangi texnik xususiyatlar ro'yxatini beradi, Norito kodek mos yozuvlar `norito.md`. |
| Artifaktni oldindan ko'rish | Docs/DevRel | PRga biriktirilgan `docs-portal-preview` artefakti, tutun tekshiruvi o'tishi, sharhlovchilar bilan ulashilgan havola. |

Har bir qatorni PR ko'rib chiqishingizning bir qismi sifatida belgilang yoki har qanday keyingi vazifalarni qayd qiling
kuzatish aniqligicha qoladi.

## 4. Chiqarish qaydlari

- `https://docs.iroha.tech/` (yoki muhit URL manzilini) qo'shing
  tarqatish ishidan) chiqarish qaydlari va holat yangilanishlarida.
- Har qanday yangi yoki o'zgartirilgan bo'limlarga aniq qo'ng'iroq qiling, shunda quyi oqim guruhlari qaerdaligini bilishadi
  o'zlarining tutun sinovlarini qayta o'tkazish uchun.