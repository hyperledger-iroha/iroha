---
lang: uz
direction: ltr
source: docs/portal/docs/norito/overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito Umumiy ko'rinish

Norito - bu Iroha bo'ylab ishlatiladigan ikkilik ketma-ketlik qatlami: u ma'lumotlar qandayligini belgilaydi
tuzilmalar simda kodlanadi, diskda saqlanadi va ular o'rtasida almashinadi
shartnomalar va mezbonlar. Ish joyidagi har bir quti o'rniga Norito ga tayanadi
`serde`, shuning uchun turli xil qurilmalardagi tengdoshlar bir xil baytlarni ishlab chiqaradi.

Ushbu sharh asosiy qismlarni umumlashtiradi va kanonik havolalarga havolalar beradi.

## Bir qarashda arxitektura

- **Sarlavha + foydali yuk** - Har bir Norito xabari xususiyat-muzokaralar bilan boshlanadi
  sarlavha (bayroqlar, nazorat summasi) so'ng yalang'och foydali yuk. Qadoqlangan sxemalar va
  siqishni sarlavha bitlari orqali muhokama qilinadi.
- **Deterministik kodlash** – `norito::codec::{Encode, Decode}` ni amalga oshiradi
  yalang'och kodlash. Xuddi shu tartib foydali yuklarni sarlavhalarga o'rashda qayta ishlatiladi
  xeshlash va imzolash deterministik bo'lib qoladi.
- **Sxema + kelib chiqadi** – `norito_derive` `Encode`, `Decode` va
  `IntoSchema` ilovalari. Paketli tuzilmalar/ketliklar sukut bo'yicha yoqilgan
  va `norito.md` da hujjatlashtirilgan.
- **Multicode registr** – xeshlar, kalit turlari va foydali yuk uchun identifikatorlar
  deskriptorlar `norito::multicodec` da yashaydi. Vakolatli jadval
  `multicodec.md` da saqlanadi.

## Asboblar

| Vazifa | Buyruq / API | Eslatmalar |
| --- | --- | --- |
| Sarlavha/bo'limlarni tekshiring | `ivm_tool inspect <file>.to` | ABI versiyasi, bayroqlar va kirish nuqtalarini ko'rsatadi. |
| Rust | da kodlash/dekodlash `norito::codec::{Encode, Decode}` | Barcha asosiy ma'lumotlar modeli turlari uchun qo'llaniladi. |
| JSON interop | `norito::json::{to_json_pretty, from_json}` | Deterministik JSON Norito qiymatlari bilan quvvatlanadi. |
| Hujjatlarni/xujjatlarni yaratish | `norito.md`, `multicodec.md` | Repo ildizidagi haqiqat manbasi hujjatlari. |

## Ish jarayoni

1. **Trivallarni qoʻshish** – Yangi maʼlumotlar uchun `#[derive(Encode, Decode, IntoSchema)]` ni afzal qiling
   tuzilmalar. Agar zarurat bo'lmasa, qo'lda yozilgan serializatorlardan saqlaning.
2. **Paketlangan sxemalarni tasdiqlang** – `cargo test -p norito` (va oʻramli) dan foydalaning
   yangisini ta'minlash uchun `scripts/run_norito_feature_matrix.sh` dagi xususiyat matritsasi
   sxemalar barqaror bo'lib qoladi.
3. **Hujjatlarni qayta tiklash** – Kodlash o‘zgarganda, `norito.md` va
   multicodec jadvali, keyin portal sahifalarini yangilang (`/reference/norito-codec`
   va bu umumiy ko'rinish).
4. **Norito-birinchi sinovlarni davom ettiring** – Integratsiya testlarida Norito JSON’dan foydalanish kerak
   `serde_json` o'rniga yordamchilar, shuning uchun ular ishlab chiqarish bilan bir xil yo'llardan foydalanadilar.

## Tez havolalar

- Texnik xususiyatlari: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Multicodec topshiriqlari: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Xususiyat matritsasi skripti: `scripts/run_norito_feature_matrix.sh`
- Qadoqlangan tartib misollari: `crates/norito/tests/`

Ushbu umumiy koʻrinishni tezkor ishga tushirish qoʻllanmasi (`/norito/getting-started`) bilan bogʻlang.
Norito dan foydalanadigan bayt kodini kompilyatsiya qilish va ishga tushirish bo'yicha amaliy ko'rsatma
foydali yuklar.