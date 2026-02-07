---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SF-6 Security Review
summary: Findings and follow-up items from the independent assessment of keyless signing, proof streaming, and manifest submission pipelines.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-6 Xavfsizlik sharhi

**Baholash oynasi:** 2026-02-10 → 2026-02-18  
**Ko‘rib chiqish bo‘yicha yetakchilar:** Xavfsizlik muhandisligi gildiyasi (`@sec-eng`), Asboblar bo‘yicha ishchi guruhi (`@tooling-wg`)  
**Qoʻllanishi:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), isbotlangan oqim API’lari, Torii manifest bilan ishlash, Sigstore/OIDC integratsiyasi, CI chiqarish ilgaklari.  
**Artifaktlar:**  
- CLI manbasi va testlari (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Torii manifest/dalil ishlov beruvchilar (`crates/iroha_torii/src/sorafs/api.rs`)  
- Chiqarishni avtomatlashtirish (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Deterministik paritet jabduqlar (`crates/sorafs_car/tests/sorafs_cli.rs`, [SoraFS Orchestrator GA Paritet hisoboti](./orchestrator-ga-parity.md))

## Metodologiya

1. **Tahdidlarni modellashtirish ustaxonalari** ishlab chiquvchining ish stantsiyalari, CI tizimlari va Torii tugunlari uchun tajovuzkor imkoniyatlarini xaritalash.  
2. **Kodni koʻrib chiqish** hisobga olish maʼlumotlari yuzasiga (OIDC token almashinuvi, kalitsiz imzo qoʻyish), Norito manifest tekshiruvi va oqimning orqa bosimini isbotlashga qaratilgan.  
3. **Dinamik test** paritet jabduqlar va buyurtma fuzz drayverlari yordamida takrorlangan armatura manifestlari va taqlid qilingan nosozlik rejimlari (tokenni takrorlash, manifestni buzish, kesilgan isbot oqimlari).  
4. **Konfiguratsiya tekshiruvi** `iroha_config` sukut bo'yicha, CLI bayrog'i bilan ishlov berish va deterministik, tekshiriladigan ishga tushirishni ta'minlash uchun reliz skriptlarini tasdiqladi.  
5. **Jarayon intervyu** tuzatish jarayonini, kuchayish yo'llarini va Tooling WG relizlar egalari bilan audit dalillarini to'plashni tasdiqladi.

## Topilmalar xulosasi

| ID | Jiddiylik | Hudud | Topish | Ruxsat |
|----|----------|------|---------|------------|
| SF6-SR-01 | Yuqori | Kalitsiz imzolash | OIDC token auditoriyasi sukut bo'yicha CI shablonlarida yashirin bo'lib, ijarachilar o'rtasida takroriy takrorlash xavfi bor edi. | Chiqarish ilgaklari va CI shablonlarida ([release process](../developer-releases.md), `docs/examples/sorafs_ci.md`) aniq `--identity-token-audience` qo'shildi. Tomoshabinlar o‘tkazib yuborilsa, CI endi ishlamay qoladi. |
| SF6-SR-02 | O'rta | Isbot oqimi | Orqa bosim yo'llari cheksiz abonent buferlarini qabul qildi, bu esa xotiraning tugashini ta'minlaydi. | `sorafs_cli proof stream` chegaralangan kanal o‘lchamlarini deterministik kesish, Norito xulosalarini qayd qilish va oqimni bekor qilish bilan amalga oshiradi; Torii oynasi bog'langan javob qismlariga yangilandi (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | O'rta | Manifest topshirish | `--plan` mavjud bo'lmaganda CLI manifestlarni o'rnatilgan bo'lak rejalarini tekshirmasdan qabul qildi. | Agar `--expect-plan-digest` taqdim etilmasa, `sorafs_cli manifest submit` endi CAR dayjestlarini qayta hisoblab chiqadi va taqqoslaydi, mos kelmaslikni rad etadi va tuzatish bo'yicha maslahatlarni ko'rsatadi. Sinovlar muvaffaqiyat/qobiliyatsizlik holatlarini qamrab oladi (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Past | Audit izi | Relizlar roʻyxatida xavfsizlik tekshiruvi uchun imzolangan tasdiqlash jurnali yoʻq edi. | GA dan oldin koʻrib chiqish eslatmalari xeshlari va kirish chiptasi URL manzilini biriktirishni talab qiluvchi [release process](../developer-releases.md) boʻlimi qoʻshildi. |

Barcha yuqori/o'rta topilmalar ko'rib chiqish oynasida tuzatildi va mavjud paritet jabduqlar orqali tasdiqlandi. Hech qanday yashirin tanqidiy muammolar qolmadi.

## Tekshirish tekshiruvi

- **Kredit ma'lumotlari doirasi:** Standart CI shablonlari endi aniq auditoriya va emitent tasdiqlarini talab qiladi; Agar `--identity-token-audience` `--identity-token-provider` bilan birga bo'lmasa, CLI va bo'shatish yordamchisi tezda ishlamay qoladi.  
- **Deterministik takrorlash:** Yangilangan testlar ijobiy/salbiy manifest yuborish oqimlarini qamrab oladi, bu esa mos kelmaydigan dayjestlar deterministik bo'lmagan nosozliklar bo'lib qolishi va tarmoqqa tegmasdan oldin paydo bo'lishini ta'minlaydi.  
- **Tasdiqlangan oqimning orqa bosimi:** Torii endi PoR/PoTR elementlarini cheklangan kanallar orqali uzatadi va CLI faqat kesilgan kechikish namunalarini + beshta nosozlik misolini saqlab qoladi va deterministik xulosalarni saqlagan holda obunachilarning cheksiz o'sishini oldini oladi.  
- **Kuzatilishi:** Tasdiqlangan oqim hisoblagichlari (`torii_sorafs_proof_stream_*`) va CLI xulosalari operatorlarni tekshirish bo'limlari bilan ta'minlab, bekor qilish sabablarini aniqlaydi.  
- **Hujjatlar:** Ishlab chiquvchi qoʻllanmalari ([ishlab chiquvchilar indeksi](../developer-index.md), [CLI maʼlumotnomasi](../developer-cli.md)) xavfsizlikka sezgir bayroqlar va eskalatsiya ish oqimlarini chaqiradi.

## Nazorat roʻyxati qoʻshimchalarini chiqarish

Reliz menejerlari GA nomzodini ilgari surishda **quyidagi dalillarni ilova qilishi shart:

1. Oxirgi xavfsizlik tekshiruvi eslatmasining xeshi (ushbu hujjat).  
2. Kuzatiladigan tuzatish chiptasiga havola (masalan, `governance/tickets/SF6-SR-2026.md`).  
3. Aniq auditoriya/emitent argumentlarini ko'rsatuvchi `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` chiqishi.  
4. Paritet jabduqdan olingan jurnallar (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Torii reliz eslatmalarida cheklangan isbotlangan oqimli telemetriya hisoblagichlari mavjudligini tasdiqlash.

Yuqoridagi artefaktlarni to'plamaslik GA imzosini bloklaydi.

**Ma’lumot artefakt xeshlari (2026-02-20 ro‘yxatdan o‘tish):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Ajoyib kuzatuvlar

- **Threat modelini yangilash:** Ushbu ko'rib chiqishni har chorakda yoki asosiy CLI bayrog'i qo'shimchalaridan oldin takrorlang.  
- **Fuzzing qamrovi:** Tasdiqlovchi oqimli transport kodlashlari `fuzz/proof_stream_transport` orqali identifikatsiya, gzip, deflate va zstd foydali yuklarini qamrab olgan.  
- **Hodisa repetisiyasi:** Tokenlar kelishuvi va manifest orqaga qaytarilishini taqlid qiluvchi operator mashqini rejalashtiring, bunda hujjatlar amaldagi tartiblarni aks ettiradi.

## Tasdiqlash

- Xavfsizlik muhandisligi gildiyasi vakili: @sec-eng (2026-02-20)  
- Asboblar bo'yicha ishchi guruhi vakili: @tooling-wg (2026-02-20)

Imzolangan tasdiqlarni chiqarish artefakt to‘plami bilan birga saqlang.