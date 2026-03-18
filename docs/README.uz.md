---
lang: uz
direction: ltr
source: docs/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26e6f90205e98b5db87d442eb7e4e7691cce47e1c33ef3d11c9bfba25269294e
source_last_modified: "2026-01-14T17:53:24.552406+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha Hujjatlar

`README.ja.md`](./README.ja.md)

Ish maydoni bir xil kod bazasidan ikkita reliz qatorini jo'natadi: **Iroha 2** (o'z-o'zidan joylashtirilgan joylashtirishlar) va
**Iroha 3 / SORA Nexus** (yagona global Nexus daftar). Ikkalasi ham bir xil Iroha virtual mashinasini (IVM) qayta ishlatadi va
Kotodama asboblar zanjiri, shuning uchun shartnomalar va bayt-kod joylashtirish maqsadlari orasida ko'chma bo'lib qoladi. Hujjatlar amal qiladi
agar boshqacha ko'rsatilmagan bo'lsa, ikkalasiga ham.

[Asosiy Iroha hujjatlarida](https://docs.iroha.tech/) quyidagilarni topasiz:

- [Boshlash uchun qo'llanma](https://docs.iroha.tech/get-started/)
- Rust, Python, Javascript va Java/Kotlin uchun [SDK Tutorials](https://docs.iroha.tech/guide/tutorials/)
- [API ma'lumotnomasi](https://docs.iroha.tech/reference/torii-endpoints.html)

Chiqarish uchun maxsus oq qog'ozlar va texnik xususiyatlar:

- [Iroha 2 Whitepaper](./source/iroha_2_whitepaper.md) — oʻz-oʻzidan joylashtirilgan tarmoq spetsifikatsiyasi.
- [Iroha 3 (SORA Nexus) Whitepaper](./source/iroha_3_whitepaper.md) — Nexus koʻp tarmoqli va maʼlumotlar maydoni dizayni.
- [Ma'lumotlar modeli va ISI Spec (amalga oshirishdan olingan)](./source/data_model_and_isi_spec.md) - teskari ishlab chiqilgan xatti-harakatlar ma'lumotnomasi.
- [ZK konvertlari (Norito)](./source/zk_envelopes.md) — mahalliy IPA/STARK Norito konvertlari va tasdiqlovchi taxminlari.

## Mahalliylashtirish

Yapon (`*.ja.*`), ibroniy (`*.he.*`), ispan (`*.es.*`), portugal
(`*.pt.*`), frantsuz (`*.fr.*`), rus (`*.ru.*`), arab (`*.ar.*`) va urdu
(`*.ur.*`) hujjat stublari har bir ingliz manba fayli yonida joylashgan. Qarang
[`docs/i18n/README.md`](./i18n/README.md) yaratish va yaratish haqida batafsil ma'lumot uchun
tarjimalarni saqlash, shuningdek, yangi tillarni qo'shish bo'yicha ko'rsatmalar
kelajak.

## Asboblar

Ushbu omborda siz Iroha 2 vositalari uchun hujjatlarni topishingiz mumkin:

- [Kagami](../crates/iroha_kagami/README.md)
- [`iroha_derive`](../crates/iroha_derive/) konfiguratsiya tuzilmalari uchun makroslar (`config_base` xususiyatiga qarang)
- [Profillash bosqichlari](./profile_build.md) sekin `iroha_data_model` kompilyatsiya vazifalarini aniqlash uchun

## Swift / iOS SDK havolalari

- [Swift SDK umumiy ko'rinishi](./source/sdk/swift/index.md) — quvur liniyasi yordamchilari, tezlashtirish tugmalari va Connect/WebSocket API.
- [Tezkor ishga tushirishni ulash](./connect_swift_ios.md) — SDK-birinchi koʻrsatma va CryptoKit maʼlumotnomasi.
- [Xcode integratsiya qoʻllanmasi](./connect_swift_integration.md) — ChaChaPoly va ramka yordamchilari bilan NoritoBridgeKit/Connect-ni ilovaga ulash.
- [SwiftUI demo ishtirokchisi uchun qoʻllanma](./norito_demo_contributor.md) — iOS demosini mahalliy Torii tuguniga qarshi ishga tushirish, shuningdek, tezlashtirish qaydlari.
- Swift artefaktlarini yoki Connect o'zgarishlarini nashr qilishdan oldin `make swift-ci` ni ishga tushiring; u armatura pariteti, asboblar paneli tasmasi va Buildkite `ci/xcframework-smoke:<lane>:device_tag` metamaʼlumotlarini tekshiradi.

## Norito (Serializatsiya kodek)

Norito - bu ish maydonini ketma-ketlashtirish kodekidir. Biz `parity-scale-codec` dan foydalanmaymiz
(SCALE). Hujjatlar yoki ko'rsatkichlar SCALE bilan solishtirganda, u faqat uchun
kontekst; barcha ishlab chiqarish yo'llari Norito dan foydalanadi. `norito::codec::{Encode, Decode}`
API'lar xeshlash va simlar uchun sarlavhasiz ("yalang'och") Norito foydali yukini ta'minlaydi.
samaradorlik - bu SCALE emas, Norito.

Oxirgi holat:

- Ruxsat etilgan sarlavha bilan deterministik kodlash/dekodlash (sehrli, versiya, 16 baytli sxema, siqish, uzunlik, CRC64, bayroqlar).
- Ish vaqti bo'yicha tanlangan tezlashtirish bilan CRC64-XZ nazorat summasi:
  - x86_64 PCLMULQDQ (ko'tarishdan kam ko'paytma) + Barrettni qisqartirish, 32 baytdan ortiq bo'laklarga katlanmış.
  - mos buklama bilan aarch64 PMULL.
  - Portativlik uchun 8 ga bo'lingan va bit bo'yicha qayta tiklash.
- Ajratishlarni kamaytirish uchun hosilalar va asosiy turlar tomonidan amalga oshiriladigan kodlangan uzunlik bo'yicha maslahatlar.
- Kattaroq oqim buferlari (64 KiB) va dekodlash vaqtida qo'shimcha CRC yangilanishi.
- ixtiyoriy zstd siqish; GPU tezlashuvi xususiyatga ega va deterministikdir.
- Moslashuvchan yo'lni tanlash: `norito::to_bytes_auto(&T)` raqamlar orasidan tanlaydi
  siqish, CPU zstd yoki GPUdan yuklangan zstd (kompilyatsiya qilingan va mavjud bo'lganda)
  foydali yuk hajmi va keshlangan apparat imkoniyatlariga asoslangan. Tanlov faqat ta'sir qiladi
  ishlash va sarlavhaning `compression` bayti; foydali yuk semantikasi o'zgarmadi.

Parite testlari, ko'rsatkichlar va foydalanish misollari uchun `crates/norito/README.md` ga qarang.

Eslatma: Ba'zi quyi tizim hujjatlari (masalan, IVM tezlashtirish va ZK sxemalari) rivojlanmoqda. Funktsionallik to'liq bo'lmasa, fayllar qolgan ishni va sayohat yo'nalishini chaqiradi.

Holat so‘nggi nuqtasini kodlash qaydlari
- Torii `/status` korpusi ixchamlik uchun sarlavhasiz (“yalangʻoch”) foydali yuk bilan sukut boʻyicha Norito dan foydalanadi. Mijozlar avval Norito kodini dekodlashga harakat qilishlari kerak.
- Serverlar so'ralganda JSONni qaytarishi mumkin; Agar `content-type` `application/json` bo'lsa, mijozlar JSONga qaytadilar.
- Sim formati SCALE emas, Norito. Yalang'och variant uchun `norito::codec::{Encode,Decode}` API ishlatiladi.