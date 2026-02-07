---
lang: uz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 38c0cedd4858656db8562c6612f9981df11a1b2292c05908c3671402ee96be9d
source_last_modified: "2026-01-16T16:25:53.031576+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito kodek ma'lumotnomasi

Norito - Iroha ning kanonik ketma-ketlashtirish qatlami. Har bir simli xabar, diskdagi
foydali yuk va o'zaro komponentli API Norito dan foydalanadi, shuning uchun tugunlar bir xil baytlarga rozi bo'ladi
hatto ular turli xil qurilmalarda ishlaganda ham. Ushbu sahifa harakatlanuvchi qismlarni umumlashtiradi
va `norito.md` da to'liq spetsifikatsiyaga ishora qiladi.

## Asosiy tartib

| Komponent | Maqsad | Manba |
| --- | --- | --- |
| **Sarlavha** | Xususiyatlarni (to'plangan tuzilmalar/ketma-ketliklar, ixcham uzunliklar, siqish bayroqlari) muhokama qiladi va CRC64 nazorat summasini o'rnatadi, shuning uchun dekodlashdan oldin foydali yukning yaxlitligi tekshiriladi. | `norito::header` — qarang `norito.md` (“Sarlavha va bayroqlar”, ombor ildizi) |
| **Yalang'och foydali yuk** | Xeshlash/taqqoslash uchun ishlatiladigan deterministik qiymat kodlash. Xuddi shu tartib tashish uchun sarlavha bilan o'ralgan. | `norito::codec::{Encode, Decode}` |
| **Siqish** | `compression` bayroq bayti orqali faollashtirilgan ixtiyoriy Zstd (va eksperimental GPU tezlashuvi). | `norito.md`, "Siqish muzokaralari" |

To'liq bayroq registri (paketlangan-struct, packed-seq, ixcham uzunliklar, siqish)
`norito::header::flags` da yashaydi. `norito::header::Flags` qulaylikni ochib beradi
ish vaqtini tekshirish uchun tekshiruvlar; Zaxiralangan tartib bitlari dekoderlar tomonidan rad etiladi.

## Yordam oling

`norito_derive` `Encode`, `Decode`, `IntoSchema` va JSON yordamchisini yuboradi.
Asosiy konventsiyalar:

- `packed-struct` xususiyati mavjud bo'lganda tuzilmalar/raqamlar to'plangan tartiblarni oladi
  yoqilgan (standart). Amalga oshirish muddati `crates/norito_derive/src/derive_struct.rs`
  va xatti-harakatlar `norito.md` ("Paketlangan sxemalar") da hujjatlashtirilgan.
- Paketli kollektsiyalar v1 da belgilangan kenglikdagi ketma-ketlik sarlavhalari va ofsetlardan foydalanadi; faqat
  har bir qiymat uzunligi prefikslariga `COMPACT_LEN` ta'sir qiladi.
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

- **Spec**: `norito.md`
- **Multicode jadvali**: `multicodec.md`
- **Benchmarks**: `crates/norito/benches/`
- **Oltin testlar**: `crates/norito/tests/`

Docusaurus avtomatizatsiyasi ishga tushganda portal yangilanadi.
sinxronlash skripti (`docs/portal/scripts/` da kuzatilgan) bulardan ma'lumotlarni oladi
fayllar. Ungacha, spetsifikatsiya o'zgarganda, bu sahifani qo'lda tekislang.