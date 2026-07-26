---
lang: uz
direction: ltr
source: docs/portal/docs/reference/norito-codec.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito kodek ma'lumotnomasi

Norito - Iroha ning kanonik seriyali qatlami. Har bir simli xabar, diskdagi
foydali yuk va o'zaro komponentli API Norito dan foydalanadi, shuning uchun tugunlar bir xil baytlarga rozi bo'ladi
hatto ular turli xil qurilmalarda ishlaganda ham. Ushbu sahifa harakatlanuvchi qismlarni umumlashtiradi
va `norito.md` da to'liq spetsifikatsiyaga ishora qiladi.

## Asosiy tartib

| Komponent | Maqsad | Manba |
| --- | --- | --- |
| **Sarlavha** | Sehrli/versiya/sxema xesh, CRC64, uzunlik va siqish tegi bilan foydali yuklarni ramkalar; v1 uchun `VERSION_MINOR = 0x00` talab qilinadi va qo'llab-quvvatlanadigan niqobga qarshi sarlavha bayroqlarini tasdiqlaydi (standart `0x00`). | `norito::header` — qarang `norito.md` (“Sarlavha va bayroqlar”, ombor ildizi) |
| **Yalang'och foydali yuk** | Xeshlash/taqqoslash uchun ishlatiladigan deterministik qiymat kodlash. Simli transport har doim sarlavhadan foydalanadi; yalang baytlar faqat ichki. | `norito::codec::{Encode, Decode}` |
| **siqish** | Sarlavhani siqish bayti orqali tanlangan ixtiyoriy Zstd (va eksperimental GPU tezlashuvi). | `norito.md`, "Siqish muzokaralari" |

Joylashtirish bayroqlari reestri (packed-struct, packed-seq, field bitset, compact)
uzunligi) `norito::header::flags` da yashaydi. V1 sukut bo'yicha `0x00` bayroqlari uchun lekin
qo'llab-quvvatlanadigan niqob ichida aniq sarlavha bayroqlarini qabul qiladi; noma'lum bitlar
rad etilgan. `norito::header::Flags` ichki tekshirish uchun saqlanadi va
kelajak versiyalari.

## Yordam oling

`norito_derive` `Encode`, `Decode`, `IntoSchema` va JSON yordamchisini yuboradi.
Asosiy konventsiyalar:

- Derivalar ham AoS, ham paketlangan kod yo'llarini yaratadi; v1 standarti AoS uchun
  tartib (`0x00` bayroqlari), agar sarlavha bayroqlari paketli variantlarga kirmasa.
  Amalga oshirish muddati `crates/norito_derive/src/derive_struct.rs`.
- Tartibga ta'sir qiluvchi xususiyatlar (`packed-struct`, `packed-seq`, `compact-len`)
  sarlavha bayroqlari orqali kirish va tengdoshlar orasida doimiy ravishda kodlangan/dekodlangan bo'lishi kerak.
- JSON yordamchilari (`norito::json`) deterministik Norito tomonidan qo'llab-quvvatlanadigan JSONni ta'minlaydi.
  ochiq API. `norito::json::{to_json_pretty, from_json}` dan foydalaning - hech qachon `serde_json`.

## Multicodec va identifikatorlar jadvallari

Norito multikodek tayinlashlarini `norito::multicodec` da saqlaydi. Malumot
jadval (xeshlar, kalit turlari, foydali yuk deskriptorlari) `multicodec.md` da saqlanadi
ombor ildizida. Yangi identifikator qo'shilganda:

1. `norito::multicodec::registry` ni yangilang.
2. `multicodec.md` da jadvalni kengaytiring.
3. Agar ular xaritani ishlatsa, quyi oqimlarni (Python/Java) qayta tiklang.

## Hujjatlar va jihozlarni qayta tiklash

Portalda nasriy xulosa mavjud bo'lsa, yuqoridagi Markdown-dan foydalaning
manbalar haqiqat manbai sifatida:

- **Xususiyatlar**: `norito.md`
- **Multicode jadvali**: `multicodec.md`
- **Benchmarks**: `crates/norito/benches/`
- **Oltin testlar**: `crates/norito/tests/`

Docusaurus avtomatizatsiyasi ishga tushganda portal yangilanadi.
sinxronlash skripti (`docs/portal/scripts/` da kuzatilgan) bulardan ma'lumotlarni oladi
fayllar. Ungacha, spetsifikatsiya o'zgarganda, bu sahifani qo'lda tekislang.