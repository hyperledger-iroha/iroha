---
lang: uz
direction: ltr
source: docs/source/mochi_architecture_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffca282b5a2bb2506f46ac2c7a8985ff2f7d10a46bc999a002277956c9f452b0
source_last_modified: "2026-01-05T09:28:12.023255+00:00"
translation_last_reviewed: 2026-02-07
title: MOCHI Architecture Plan
description: High-level design for the MOCHI local-network GUI supervisor.
translator: machine-google-reviewed
---

# MOCHI arxitektura rejasi


## Maqsadlar

- Bir peerli yoki ko'p peerli (to'rt tugunli BFT) mahalliy tarmoqlarni tezda yuklash.
- `kagami`, `irohad` va qo'llab-quvvatlovchi ikkilik fayllarni qulay GUI ish oqimiga o'rang.
- Torii HTTP/WebSocket so'nggi nuqtalari orqali jonli blok, voqea va holat ma'lumotlarini ko'ring.
- Tranzaktsiyalar va Iroha Maxsus Yo'riqnomalari (ISI) uchun tuzilgan quruvchilarni mahalliy imzolash va topshirish bilan ta'minlash.
- Fayllarni qo'lda tahrir qilmasdan suratlarni, qayta ishlab chiqish oqimlarini va konfiguratsiya sozlamalarini boshqaring.
- Veb ko'rinishi yoki Docker bog'liqligisiz yagona o'zaro faoliyat platforma Rust binari sifatida jo'natish.

## Arxitekturaga umumiy nuqtai

MOCHI yangi `/mochi` katalogida joylashgan ikkita asosiy qutiga bo'lingan (qarang.
Qurilish va foydalanish ko'rsatmalari uchun [MOCHI Quickstart](mochi/quickstart.md):

1. `mochi-core`: konfiguratsiya shablonlari, kalit va genezis materiallarini yaratish, bolalar jarayonlarini nazorat qilish, Torii mijozlarini boshqarish va fayl tizimi holatini boshqarish uchun mas'ul bo'lgan boshsiz kutubxona.
2. `mochi-ui-egui`: `egui`/`eframe` asosida qurilgan ish stoli ilovasi, u foydalanuvchi interfeysini taqdim etadi va `mochi-core` API orqali barcha orkestrlarni topshiradi.

Qo'shimcha old qismlar (masalan, Tauri qobig'i) keyinchalik nazoratchi mantig'ini qayta ishlamasdan `mochi-core` ga ulanishi mumkin.

## Jarayon modeli

- Tengdosh tugunlar alohida `irohad` bolalar jarayonlari sifatida ishlaydi. MOCHI hech qachon tengdoshni kutubxona sifatida bog'lamaydi, beqaror ichki API-lardan qochib, ishlab chiqarishni joylashtirish topologiyalariga mos keladi.
- Genesis va asosiy materiallar `kagami` chaqiruvlari orqali foydalanuvchi tomonidan taqdim etilgan ma'lumotlar (zanjir identifikatori, dastlabki hisoblar, aktivlar) orqali yaratiladi.
- Konfiguratsiya fayllari Torii va P2P portlari, saqlash yo'llari, surat sozlamalari va ishonchli tengdoshlar ro'yxatini to'ldiruvchi TOML shablonlaridan yaratilgan. Yaratilgan konfiguratsiyalar har bir tarmoq ish maydoni katalogi ostida saqlanadi.
- Nazoratchi jarayonning hayot davrlarini kuzatib boradi, jurnallar uchun stdout/stderr oqimlarini yuboradi va sog'liq uchun `/status`, `/metrics` va `/configuration` so'rovlarini oladi.
- Yupqa Torii mijoz qatlami HTTP va WebSocket qo'ng'iroqlarini o'rab oladi va SCALE kodlash/dekodlashni qayta qo'llashdan qochish uchun Iroha Rust mijoz qutilariga tayanadi.

## `mochi-core` tomonidan qo'llab-quvvatlangan foydalanuvchi oqimlari- **Tarmoq yaratish ustasi**: bitta yoki to‘rt tengdoshli profilni tanlang, kataloglarni tanlang va `kagami` raqamiga qo‘ng‘iroq qiling va identifikator va genezisni yarating.
- **Lifecycle Controls**: boshlash, to'xtatish, tengdoshlarni qayta ishga tushirish; sirt jonli ko'rsatkichlari; log dumlarini ochish; ish vaqti konfiguratsiyasining so'nggi nuqtalarini almashtirish (masalan, jurnal darajalari).
- **Bloklash va hodisalar oqimi**: `/block/stream` va `/events` ga obuna bo‘ling, UI panellari uchun xotiradagi buferni saqlang.
- **State Explorer**: Norito tomonidan qoʻllab-quvvatlanadigan `/query` qoʻngʻiroqlarini ishga tushiring va domenlar, hisoblar, aktivlar va aktiv taʼriflarini sahifalash yordamchilari va metamaʼlumotlar xulosalari bilan roʻyxatga oling.
- **Transaction Composer**: zarb qilish/o‘tkazish yo‘riqnomasi qoralamalarini sahnalashtiring, ularni imzolangan tranzaktsiyalarga to‘plang, Norito foydali yukini ko‘rib chiqing, `/transaction` orqali yuboring va natijada sodir bo‘lgan voqealarni kuzatib boring; tonoz imzolash ilgaklari kelajakdagi iteratsiya bo'lib qolmoqda.
- **Snapshots va Re-Genesis**: Kura snapshotlarini eksport/import qilish, doʻkonlarni oʻchirish va tez tiklash uchun genezis materialini qayta tiklash.

## UI qatlami (`mochi-ui-egui`)

- Tashqi ish vaqtlarisiz bitta mahalliy bajariladigan faylni yuborish uchun `egui`/`eframe` dan foydalanadi.
- Tartibga quyidagilar kiradi:
  - **Tarmoq boshqaruv paneli** tengdoshlar kartalari, sog'liq ko'rsatkichlari va tezkor harakatlar.
  - **Bloklar** paneli so'nggi topshiriqlarni uzatadi va balandlikda qidirishga ruxsat beradi.
  - **Voqealar** paneli tranzaksiya holatini xesh yoki hisob bo'yicha filtrlaydi.
  - **State Explorer** sahifalari, Norito natijalari bilan domenlar, hisoblar, aktivlar va aktiv taʼriflari uchun va tekshirish uchun xom axlatlar.
- **Kompozer** shakli toʻplamli yalpiz/oʻtkazish palitralari, navbatni boshqarish (qoʻshish/oʻchirish/tozalash), xom Norito oldindan koʻrish va imzolovchilar ombori tomonidan qoʻllab-quvvatlangan xabarlarni joʻnatish bilan bogʻliq boʻlib, operatorlar ishlab chiqaruvchi va haqiqiy vakolatlar oʻrtasida almashinishi mumkin.
- **Genesis & Snapshots** boshqaruv ko'rinishi.
- **Sozlamalar** ish vaqtini almashtirish va maʼlumotlar katalogi yorliqlari.
- UI kanallar orqali `mochi-core` dan asinxron yangilanishlarga obuna bo'ladi; yadro tuzilgan voqealarni (tengdosh holati, blok sarlavhalari, tranzaksiya yangilanishlari) oqimli `SupervisorHandle` ni ochib beradi.

## Mahalliy rivojlanish eslatmalari

- Ish maydoni konfiguratsiyasi sotuvchi arxivlarini olish o'rniga `zstd-sys` xostiga `libzstd` bilan bog'lanish uchun `ZSTD_SYS_USE_PKG_CONFIG=1` o'rnatadi. Bu pqcrypto-ga bog'liq tuzilmalarni (va MOCHI testlarini) oflayn yoki sinov muhitida ishlashini ta'minlaydi.

## Qadoqlash va tarqatish

- MOCHI to'plamlari (yoki `PATH` da topiladi) `irohad`, `iroha_cli` va `kagami` ikkilik fayllari.
- OpenSSL bog'liqliklarini oldini olish uchun chiquvchi HTTPS uchun `rustls` dan foydalanadi.
- Barcha yaratilgan artefaktlarni maxsus dastur ma'lumotlari ildizida (masalan, `~/.local/share/mochi` yoki platforma ekvivalenti) har bir tarmoq quyi kataloglari bilan saqlaydi. GUI “Finder/Explorer-da ochish” yordamchilarini taqdim etadi.
- Mojarolarni oldini olish uchun tengdoshlarni ishga tushirishdan oldin Torii (8080+) va P2P (1337+) portlarini avtomatik aniqlaydi va zahiraga oladi.

## Kelajakdagi kengaytmalar (MVP imkoniyatlaridan tashqarida)- Muqobil old uchlari (Tauri, CLI boshsiz rejimi) `mochi-core` almashish.
- Tarqalgan test klasterlari uchun ko'p xostli orkestratsiya.
- Konsensus ichki ma'lumotlar uchun vizualizatorlar (Sumeragi davra holatlari, g'iybat vaqtlari).
- Avtomatlashtirilgan vaqtinchalik tarmoq suratlari uchun CI quvurlari bilan integratsiya.
- Maxsus boshqaruv paneli yoki domenga oid inspektorlar uchun plagin tizimi.

## Ma'lumotnomalar

- [Torii Endpoints](https://docs.iroha.tech/reference/torii-endpoints.html)
- [Peer Configuration Parameters](https://docs.iroha.tech/reference/peer-config/params.html)
- [`kagami` ombori hujjatlari](https://github.com/hyperledger-iroha/iroha)
- [Iroha Maxsus ko'rsatmalar](https://iroha-test.readthedocs.io/en/iroha2-dev/references/isi/)