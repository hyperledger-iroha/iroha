---
lang: uz
direction: ltr
source: docs/source/kotodama_error_codes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e0e4f16000f6a578fe9c9d6e204c01087e987ac3b46d70537a15b072df48a13
source_last_modified: "2025-12-29T18:16:35.974178+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama kompilyator xato kodlari

Kotodama kompilyatori barqaror xato kodlarini chiqaradi, shuning uchun asboblar va CLI foydalanuvchilari
muvaffaqiyatsizlik sababini tezda tushuning. `koto_compile --explain <code>` dan foydalaning
tegishli maslahatni chop etish uchun.

| Kod | Tavsif | Oddiy tuzatish |
|-------|-------------|-------------|
| `E0001` | Filial maqsadi IVM sakrash kodlash uchun diapazondan tashqarida. | Juda katta funksiyalarni ajrating yoki chiziqni qisqartiring, shunda blokning asosiy masofalari ±1MiB ichida qoladi. |
| `E0002` | Qo'ng'iroq saytlari hech qachon aniqlanmagan funksiyaga havola qiladi. | Qo'ng'iroq qiluvchini olib tashlagan matn terish xatolari, ko'rinish o'zgartirgichlari yoki xususiyat bayroqlarini tekshiring. |
| `E0003` | Bardoshli holat tizimi chaqiruvlari ABI v1 yoqilmagan holda chiqarildi. | `CompilerOptions::abi_version = 1` o'rnating yoki `seiyaku` shartnomasiga `meta { abi_version: 1 }` qo'shing. |
| `E0004` | Obyekt bilan bog'liq tizimlar to'g'ridan-to'g'ri bo'lmagan ko'rsatkichlarni oldi. | `account_id(...)`, `asset_definition(...)` va boshqalardan foydalaning yoki xostning standart sozlamalari uchun 0 sentineldan o'ting. |
| `E0005` | `for`-loop startizer bugungi kunda qo'llab-quvvatlanadiganidan ancha murakkab. | Murakkab o'rnatishni pastadirdan oldin ko'chiring; Hozirda faqat oddiy `let`/ifodani ishga tushiruvchilar qabul qilinadi. |
| `E0006` | `for`-loop qadam bandi bugungi kunda qo'llab-quvvatlanadiganidan ancha murakkab. | Loop hisoblagichini oddiy ifoda bilan yangilang (masalan, `i = i + 1`). |