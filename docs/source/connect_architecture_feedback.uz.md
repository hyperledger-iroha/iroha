---
lang: uz
direction: ltr
source: docs/source/connect_architecture_feedback.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 097ea58d49f48d059cda762cd719bc62f0b2d6f6ddecedef3f9bac030ae46aec
source_last_modified: "2025-12-29T18:16:35.934098+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! Arxitektura bo'yicha fikr-mulohazalarni tekshirish ro'yxatini ulash

Ushbu nazorat ro'yxati Connect Session Arxitekturasidan ochiq savollarni oladi
Android va JavaScript-dan kiritishni talab qiladigan strawman
2026   fevral. Undan sharhlarni asinxron tarzda to'plash, kuzatish uchun foydalaning
egalik qilish va seminar kun tartibini blokdan chiqarish.

> Status / Eslatmalar ustunida Android va JS yetakchilarining yakuniy javoblari olingan
> 2026       seminar oldidan sinxronlash; qarorlar bo'lsa inline yangi keyingi muammolarni bog'lash
> rivojlanish.

## Seansning hayot aylanishi va tashish

| Mavzu | Android egasi | JS egasi | Holat / Eslatmalar |
|-------|---------------|----------|----------------|
| WebSocket reconnect back-off strategiyasi (eksponensial va chegaralangan chiziqli) | Android Networking TL | JS Lead | ✅ 60 lar bilan chegaralangan jitter bilan eksponensial orqaga qaytish bo'yicha kelishilgan; JS brauzer/tugun pariteti uchun bir xil konstantalarni aks ettiradi. |
| Oflayn bufer sig'imi standart sozlamalari (joriy strawman: 32 kvadrat) | Android Networking TL | JS Lead | ✅ Konfiguratsiyani bekor qilish bilan tasdiqlangan 32 kvadratli standart; Android `ConnectQueueConfig` orqali ishlaydi, JS `window.connectQueueMax` ni hurmat qiladi. |
| Push uslubidagi qayta ulanish bildirishnomalari (FCM/APNS va so'rovga qarshi) | Android Networking TL | JS Lead | ✅ Android hamyon ilovalari uchun ixtiyoriy FCM ilgagini ochadi; JS brauzerni surish cheklovlariga e'tibor berib, eksponensial orqaga qaytish bilan so'rovga asoslangan bo'lib qoladi. |
| Mobil mijozlar uchun ping/pong kadans to'siqlari | Android Networking TL | JS Lead | ✅ Standartlashtirilgan 30s ping, 3× oʻtkazib yuborishga bardoshliligi; Android Doze ta'sirini muvozanatlaydi, JS brauzerni o'chirib qo'ymaslik uchun ≥15 soniyagacha qisadi. |

## Shifrlash va kalitlarni boshqarish

| Mavzu | Android egasi | JS egasi | Holat / Eslatmalar |
|-------|---------------|----------|----------------|
| X25519 asosiy saqlash taxminlari (StrongBox, WebCrypto xavfsiz kontekstlari) | Android Crypto TL | JS Lead | ✅ Android X25519-ni mavjud bo'lganda StrongBox-da saqlaydi (TEE-ga qaytadi); JS, tugundagi mahalliy `iroha_js_host` ko'prigiga qaytib, dApps uchun xavfsiz kontekstli WebCrypto-ni topshiradi. |
| ChaCha20-Poly1305 SDK'lar bo'ylab boshqaruvsiz almashish | Android Crypto TL | JS Lead | ✅ Umumiy `sequence` hisoblagich API-ni 64-bitli himoya va umumiy testlar bilan qabul qiling; JS Rust xatti-harakatlariga mos kelish uchun BigInt hisoblagichlaridan foydalanadi. |
| Uskuna bilan ta'minlangan sertifikatlash foydali yuk sxemasi | Android Crypto TL | JS Lead | ✅ Sxema yakunlandi: `attestation { platform, evidence_b64, statement_hash }`; JS ixtiyoriy (brauzer), Node HSM plaginini ishlatadi. |
| Yo'qolgan hamyonlarni tiklash oqimi (kalitlarni aylantirish) | Android Crypto TL | JS Lead | ✅ Hamyonni aylantirish uchun qoʻl siqish qabul qilindi: dApp `rotate` boshqaruvini chiqaradi, yangi pubkey + imzolangan tasdiq bilan hamyon javoblari; JS WebCrypto materialini darhol qayta kalitlaydi. |

## Ruxsatlar va isbot to'plamlari| Mavzu | Android egasi | JS egasi | Holat / Eslatmalar |
|-------|---------------|----------|----------------|
| GA | uchun minimal ruxsat sxemasi (usullar/hodisalar/resurslar). Android Data Model TL | JS Lead | ✅ GA asosiy: `methods`, `events`, `resources`, `constraints`; JS TypeScript turlarini Rust manifestiga moslashtiradi. |
| Hamyonni rad etish yuki (`reason_code`, mahalliylashtirilgan xabarlar) | Android Networking TL | JS Lead | ✅ Kodlar yakunlandi (`user_declined`, `permissions_mismatch`, `compliance_failed`, `internal_error`) va ixtiyoriy `localized_message`. |
| Isbot to'plamining ixtiyoriy maydonlari (muvofiqlik/KYC qo'shimchalari) | Android Data Model TL | JS Lead | ✅ Barcha SDKlar ixtiyoriy `attachments[]` (Norito `AttachmentRef`) va `compliance_manifest_id` ni qabul qiladi; xulq-atvorni o'zgartirish talab qilinmaydi. |
| Norito JSON sxemasi va ko'prikda yaratilgan tuzilmalar bilan moslashtirish | Android Data Model TL | JS Lead | ✅ Qaror: ko'prik yordamida yaratilgan tuzilmalarni afzal ko'ring; JSON yo'li faqat disk raskadrovka uchun qoladi, JS `Value` adapterini saqlaydi. |

## SDK jabhalari va API shakli

| Mavzu | Android egasi | JS egasi | Holat / Eslatmalar |
|-------|---------------|----------|----------------|
| Yuqori darajadagi asinxron interfeyslar (`Flow`, asinxron iteratorlar) pariteti | Android Networking TL | JS Lead | ✅ Android `Flow<ConnectEvent>` ni ochadi; JS `AsyncIterable<ConnectEvent>` dan foydalanadi; birgalikda `ConnectEventKind` uchun ikkala xarita. |
| Taksonomiyani xaritalashda xatolik (`ConnectError`, yozilgan kichik sinflar) | Android Networking TL | JS Lead | ✅ Toʻlovli yuklanish tafsilotlari bilan {`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`} umumiy raqamni qabul qiling. |
| Parvozdagi belgi so'rovlari uchun bekor qilish semantikasi | Android Networking TL | JS Lead | ✅ `cancelRequest(hash)` boshqaruvi joriy etildi; ikkala SDK ham hamyonni tasdiqlash bo'yicha bekor qilinadigan koroutinlarni/va'dalarni ko'rsatadi. |
| Umumiy telemetriya ilgaklari (hodisalar, o'lchovlarni nomlash) | Android Networking TL | JS Lead | ✅ Metrik nomlari hizalangan: `connect.queue_depth`, `connect.latency_ms`, `connect.reconnects_total`; namuna eksportchilar hujjatlashtirilgan. |

## Oflayn qat'iylik va jurnallar

| Mavzu | Android egasi | JS egasi | Holat / Eslatmalar |
|-------|---------------|----------|----------------|
| Navbatdagi freymlar uchun saqlash formati (ikkilik Norito va JSON) | Android Data Model TL | JS Lead | ✅ Ikkilik Norito (`.to`) hamma joyda saqlang; JS IndexedDB `ArrayBuffer` dan foydalanadi. |
| Jurnalni saqlash siyosati va kattalik chegaralari | Android Networking TL | JS Lead | ✅ Standart saqlash 24 soat va har bir seans uchun 1MiB; `ConnectQueueConfig` orqali sozlash mumkin. |
| Ikkala tomon ham kadrlarni takrorlaganda mojaroni hal qilish | Android Networking TL | JS Lead | ✅ `sequence` + `payload_hash` dan foydalaning; dublikatlar e'tiborga olinmaydi, ziddiyatlar telemetriya hodisasi bilan `ConnectError.Internal` ni ishga tushiradi. |
| Navbat chuqurligi va muvaffaqiyatni takrorlash uchun telemetriya | Android Networking TL | JS Lead | ✅ Emit `connect.queue_depth` o'lchagich va `connect.replay_success_total` hisoblagichi; ikkala SDK ham umumiy Norito telemetriya sxemasiga ulanadi. |

## Implementation Spikes & References- **Rust ko'prigi qurilmalari:** `crates/connect_norito_bridge/src/lib.rs` va tegishli testlar har bir SDK tomonidan ishlatiladigan kanonik kodlash/dekodlash yo'llarini qamrab oladi.
- **Swift demo jabduqlar:** `examples/ios/NoritoDemoXcode/NoritoDemoXcodeTests/ConnectViewModelTests.swift` mashqlari Seans oqimlarini masxara qilingan transportlar bilan ulang.
- **Swift CI gating:** Boshqa SDKlar bilan almashishdan oldin armatura pariteti, asboblar paneli tasmasi va Buildkite `ci/xcframework-smoke:<lane>:device_tag` metamaʼlumotlarini tekshirish uchun Connect artefaktlarini yangilashda `make swift-ci` ni ishga tushiring.
- **JavaScript SDK integratsiya testlari:** `javascript/iroha_js/test/integrationTorii.test.js` Torii ga qarshi ulanish holati/sessiya yordamchilarini tasdiqlaydi.
- **Android mijozning barqarorligi haqida eslatmalar:** `java/iroha_android/README.md:150` navbat/orqaga o'chirish sukut bo'yicha ilhomlantirgan joriy ulanish tajribalarini hujjatlashtiradi.

## Seminarga tayyorgarlik buyumlari

- [x] Android: yuqoridagi har bir jadval qatori uchun nuqta shaxsini tayinlang.
- [x] JS: yuqoridagi har bir jadval qatori uchun nuqta shaxsini tayinlash.
- [x] Mavjud amalga oshirish ko'rsatkichlari yoki tajribalariga havolalarni to'plang.
- [x] 2026-yil fevral kengashigacha ish oldidan ko‘rib chiqishni rejalashtiring (Android TL, JS Lead, Swift Lead bilan 2026-01-29 15:00 UTC uchun band qilingan).
- [x] Qabul qilingan javoblar bilan `docs/source/connect_architecture_strawman.md`-ni yangilang.

## Oldindan o'qilgan paket

- ✅ Toʻplam `artifacts/connect/pre-read/20260129/` ostida yozilgan (somon, SDK qoʻllanmalari va ushbu nazorat roʻyxatini yangilagandan soʻng `make docs-html` orqali yaratilgan).
- 📄 Xulosa + tarqatish bosqichlari `docs/source/project_tracker/connect_architecture_pre_read.md` da ishlaydi; havolani 2026-yil fevraldagi seminar taklifiga va `#sdk-council` eslatmasiga kiriting.
- 🔁 To'plamni yangilayotganda, oldindan o'qilgan eslatma ichidagi yo'l va xeshni yangilang va e'lonni IOS7/AND7 tayyorlik jurnallari ostida `status.md` da arxivlang.