---
lang: uz
direction: ltr
source: docs/automation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c56bacde8ee42c2427d06038a3a6ca65035d4055c42f6e5ded7e54b33c1fe921
source_last_modified: "2025-12-29T18:16:35.060432+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Hujjatlarni avtomatlashtirish asoslari

Ushbu katalog yo'l xaritasi kabi elementlarni avtomatlashtirish yuzalarini qamrab oladi
AND5/AND6 (Android Developer tajribasi + Relizga tayyorlik) va DA-1
(Ma'lumotlar-mavjudlik tahdidi-modelni avtomatlashtirish) ular qo'ng'iroq qilganda ishora qiladi
tekshirilishi mumkin bo'lgan hujjatlar dalillari. Buyruq havolalarini bosqichma-bosqich va kutilgan
Daraxt ichidagi artefaktlar muvofiqlikni tekshirish uchun zarur shart-sharoitlarni saqlab qoladi
CI quvurlari yoki asboblar paneli oflayn bo'lganda.

## Katalog tartibi

| Yo'l | Maqsad |
|------|---------|
| `docs/automation/android/` | Android hujjatlari va mahalliylashtirishni avtomatlashtirish asoslari (AND5), shu jumladan i18n stub sinxronlash jurnallari, paritet xulosalari va AND6 tizimidan chiqishdan oldin talab qilinadigan SDK nashri dalillari. |
| `docs/automation/da/` | `cargo xtask da-threat-model-report` tomonidan havola qilingan Maʼlumotlar mavjudligi tahdidi modeli avtomatlashtirish natijalari va tungi hujjatlar yangilanadi. |

Har bir kichik katalog dalil bilan birga keltiruvchi buyruqlarni hujjatlashtiradi
biz tekshirishni kutayotgan fayl tartibi (odatda JSON xulosalari, ishga tushirish jurnallari yoki
namoyon bo'ladi). Jamoalar har safar yangi artefaktlarni tegishli papka ostiga tashlaydilar
avtomatlashtirish chop etilgan hujjatlarni sezilarli darajada o'zgartiradi, so'ngra majburiyatga bog'lanadi
tegishli holat/yo‘l xaritasi yozuvidan.

## Foydalanish

1. **Avtomatlashtirishni** quyi katalogda tasvirlangan buyruqlar yordamida ishga tushiring
   README (masalan, `ci/check_android_fixtures.sh` yoki
   `cargo xtask da-threat-model-report`).
2. **Olingan JSON/log artefaktlarini** `artifacts/…` dan nusxa oling.
   ISO-8601 vaqt tamg'asi bilan mos keladigan `docs/automation/<program>/…` papkasi
   auditorlar dalillarni boshqaruv bayonnomalari bilan bog'lashlari uchun fayl nomi.
3. Yo‘l xaritasini yopishda **`status.md`/`roadmap.md`-dagi majburiyatga** havola qiling
   gate, shuning uchun ko'rib chiquvchilar ushbu qaror uchun foydalanilgan avtomatlashtirish asosini tasdiqlashlari mumkin.
4. **Fayllarni engil tuting**. Kutish tuzilgan metadata,
   manifestlar yoki xulosalar - ommaviy ikkilik bloblar emas. Kattaroq axlatxonalar ichida qolishi kerak
   imzolangan ma'lumotnoma bilan ob'ektni saqlash.

Ushbu avtomatlashtirish qaydlarini markazlashtirib, biz “hujjatlar/avtomatlashtirish asoslarini” blokdan chiqaramiz
audit uchun mavjud” sharti AND6 chaqiradi va DA tahdidini beradi
model oqimi tungi hisobotlar va qo'lda spot tekshiruvlar uchun deterministik uy.