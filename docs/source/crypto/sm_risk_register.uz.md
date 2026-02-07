---
lang: uz
direction: ltr
source: docs/source/crypto/sm_risk_register.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ba5f4fdc9221210a793fd0c2120d8cfb68487d7ddcbe67c208976798446ca5db
source_last_modified: "2025-12-29T18:16:35.945760+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM2/SM3/SM4-ni yoqish uchun SM dasturi xavf registri.

# SM dasturining xavf registri

Oxirgi yangilangan: 2025-03-12.

Ushbu registr `sm_program.md` dagi xulosani kengaytiradi va har bir xavf bilan bog'lanadi.
egalik, monitoring triggerlari va joriy yumshatish holati. Crypto WG
va Core Platform rahbarlari ushbu reestrni haftalik SM kadensiyasida ko'rib chiqadi; o'zgarishlar
bu yerda ham, umumiy yo‘l xaritasida ham o‘z aksini topgan.

## Xavf haqida xulosa

| ID | Xavf | Kategoriya | Ehtimollik | Ta'sir | Jiddiylik | Egasi | Yumshatish | Holati | Triggerlar |
|----|------|----------|-------------|--------|----------|-------|------------|---------|----------|
| R1 | RustCrypto SM qutilari uchun tashqi audit validator GA | imzolashdan oldin bajarilmadi Ta'minot zanjiri | O'rta | Yuqori | Yuqori | Crypto WG | Bits/NCC Group kontrakt izi, hisobot qabul qilinmaguncha faqat tekshirish holatini saqlang | Yumshatish davom etmoqda | SOW auditi 2025-04-15 gacha imzolanmagan yoki audit hisoboti 2025-06-01 da kechiktirilgan |
| R2 | SDKlar bo'yicha deterministik bo'lmagan regressiyalar | Amalga oshirish | O'rta | Yuqori | Yuqori | SDK dasturi yetakchilari | SDK CI boʻylab moslamalarni baham koʻring, kanonik r∥s kodlashni tatbiq eting, SDK oʻrtasida oʻzgartirish testlarini qoʻshing | Monitoring | SM moslamalarisiz CI yoki SDK versiyalarida armatura siljishi aniqlandi |
| R3 | Intrinsicsdagi ISAga xos xatolar (NEON/SIMD) | Ishlash | Past | O'rta | O'rta | Ishlash WG | Xususiyat bayroqlari orqasidagi darvoza ichki xususiyatlari, ARM da CI qamrovini talab qiladi, skalyar qaytarilishni saqlaydi | Yumshatish davom etmoqda | NEON dastgohlari ishlamay qoldi yoki SM perf matritsasida aniqlangan apparat regressiyasi |
| R4 | Muvofiqlik noaniqligi SM qabul qilishni kechiktirish | Boshqaruv | O'rta | O'rta | O'rta | Hujjatlar va yuridik aloqa | Muvofiqlik bo'yicha qisqacha ma'lumotni, operator nazorat ro'yxatini, GA dan oldin yuridik maslahatchi bilan aloqani nashr eting | Yumshatish davom etmoqda | Huquqiy tekshiruv 01.05.2025 dan keyin amalga oshirilmagan yoki nazorat roʻyxati yangilanmagan |
| R5 | Provayder yangilanishlari bilan FFI backend drift | Integratsiya | O'rta | O'rta | O'rta | Platforma operatsiyalari | Provayder versiyalarini mahkamlang, paritet testlarini qo'shing, OpenSSL/Tongsuo-ni oldindan ko'rishga kirishni saqlang | Monitoring | Paket yangilanishi uchuvchi doirasidan tashqarida tenglik yoki oldindan ko'rish yoqilgan holda birlashtirildi |

## Ko'rib chiqish tezligi

- Haftalik Crypto WG sinxronizatsiyasi (kun tartibidagi masala).
- Muvofiqlik holatini tasdiqlash uchun Platform Ops va Docs bilan har oy qo'shma tekshiruv.
- Chiqarishdan oldingi nazorat punkti: xavf registrini muzlatish va GA bilan birga sertifikatlash
  artefaktlar.

## Ro'yxatdan o'tish

| Rol | Vakil | Sana | Eslatmalar |
|------|----------------|------|-------|
| Kripto WG yetakchisi | (fayldagi imzo) | 2025-03-12 | Nashr uchun ma'qullangan va WG backlog bilan bo'lingan. |
| Asosiy platforma yetakchisi | (fayldagi imzo) | 2025-03-12 | Qabul qilingan yumshatishlar va kadansni kuzatish. |

Tarixiy tasdiqlar va yig'ilish bayonnomalari uchun `docs/source/crypto/sm_program.md` ga qarang
(`Communication Plan`) va Crypto WG bilan bog'langan SM kun tartibi arxivi
ish maydoni.