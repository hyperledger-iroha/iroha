---
lang: uz
direction: ltr
source: docs/source/confidential_assets_rotation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd1e43316c492cc96ed107f6318841ad8db160735d4698c4f05562ff6127fda9
source_last_modified: "2026-01-22T14:35:37.492932+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! `roadmap.md:M3` tomonidan havola qilingan maxfiy aktivlarni aylantirish kitobi.

# Maxfiy aktivlarni aylantirish kitobi

Ushbu qo'llanma operatorlar maxfiy aktivni qanday rejalashtirishi va amalga oshirishini tushuntiradi
aylanishlar (parametrlar to'plami, kalitlarni tekshirish va siyosat o'tishlari).
hamyonlarni ta'minlash, Torii mijozlari va mempool qo'riqchilari deterministik bo'lib qoladi.

## Hayotiy tsikl va statuslar

Maxfiy parametrlar to'plami (`PoseidonParams`, `PedersenParams`, tekshirish kalitlari)
ma'lum balandlikda samarali maqomini olish uchun ishlatiladigan panjara va yordamchi yashaydi
`crates/iroha_core/src/state.rs:7540`–`7561`. Ish vaqti yordamchilari kutilmoqda
maqsadli balandlikka erishilgandan so'ng darhol o'tishlar va keyinroq uchun xatoliklarni qayd etish
qayta eshittirishlar (`crates/iroha_core/src/state.rs:6725`–`6765`).

Obyekt siyosatlari oʻrnatilgan
`pending_transition { transition_id, new_mode, effective_height, conversion_window }`
shuning uchun boshqaruv orqali yangilanishlarni rejalashtirish mumkin
`ScheduleConfidentialPolicyTransition` va agar kerak bo'lsa, ularni bekor qiling. Qarang
`crates/iroha_data_model/src/asset/definition.rs:320` va Torii DTO oynalari
(`crates/iroha_torii/src/routing.rs:1539`–`1580`).

## Aylanish ish jarayoni

1. **Yangi parametr toʻplamlarini nashr qilish.** Operatorlar yuboradi
   `PublishPedersenParams`/`PublishPoseidonParams` ko'rsatmalari (CLI)
   `iroha app zk params publish ...`) metama'lumotlarga ega yangi generator to'plamlarini yaratish,
   faollashtirish/bekor qilish oynalari va holat belgilari. Ijrochi rad etadi
   dublikat identifikatorlari, ortib bo'lmaydigan versiyalar yoki boshiga yomon holat o'tishlari
   `crates/iroha_core/src/smartcontracts/isi/world.rs:2499`–`2635` va
   ro'yxatga olish kitobi testlari muvaffaqiyatsizlik rejimlarini qamrab oladi (`crates/iroha_core/tests/confidential_params_registry.rs:93`–`226`).
2. **Roʻyxatdan oʻtish/kalt yangilanishlarini tekshirish.** `RegisterVerifyingKey` backendni qoʻllaydi,
   kalit kirishidan oldin majburiyat va sxema/versiya cheklovlari
   ro'yxatga olish kitobi (`crates/iroha_core/src/smartcontracts/isi/world.rs:2067`–`2137`).
   Kalitni yangilash avtomatik ravishda eski yozuvni bekor qiladi va ichki baytlarni o'chiradi,
   `crates/iroha_core/tests/zk_vk_deprecate_marks_status.rs:1` tomonidan amalga oshirilgan.
3. **Obyekt-siyosat o‘tishlarini rejalashtiring.** Yangi parametr identifikatorlari ishga tushirilgach,
   boshqaruv istalgan bilan `ScheduleConfidentialPolicyTransition` ni chaqiradi
   rejimi, o'tish oynasi va audit xeshi. Ijrochi ziddiyatni rad etadi
   o'tishlar yoki ajoyib shaffof ta'minotga ega aktivlar. kabi testlar
   `crates/iroha_core/tests/confidential_policy_gates.rs:300`–`384` buni tasdiqlang
   bekor qilingan o'tishlar aniq `pending_transition`, esa
   `confidential_policy_transition_reaches_shielded_only_on_schedule` da
   lines385–433 rejalashtirilgan yangilanishlar `ShieldedOnly` ga aynan shu vaqtda oʻtishini tasdiqlaydi.
   samarali balandlik.
4. **Siyosat ilovasi va mempul himoyasi.** Blok ijrochisi kutilayotgan barcha narsalarni tozalaydi.
   Har bir blokning boshida o'tishlar (`apply_policy_if_due`) va chiqaradi
   telemetriya, agar o'tish muvaffaqiyatsiz bo'lsa, operatorlar qayta rejalashtirishlari mumkin. Qabul paytida
   mempul samarali siyosati o'rta blokni o'zgartiradigan operatsiyalarni rad etadi,
   o'tish oynasi bo'ylab deterministik inklyuziyani ta'minlash
   (`docs/source/confidential_assets.md:60`).

## Hamyon va SDK talablari- Swift va boshqa mobil SDK'lar faol siyosatni olish uchun Torii yordamchilarini ochib beradi.
  plus har qanday kutilayotgan o'tish, shuning uchun hamyonlar imzolashdan oldin foydalanuvchilarni ogohlantirishi mumkin. Qarang
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:309` (DTO) va tegishli
  testlar `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift:591`.
- CLI bir xil metama'lumotlarni `iroha ledger assets data-policy get` orqali aks ettiradi (yordamchi
  `crates/iroha_cli/src/main.rs:1497`–`1670`), operatorlarga tekshirishga imkon beradi
  Siyosat/parametr identifikatorlari aktiv taʼrifiga oʻrnatilgan
  blok do'koni.

## Test va telemetriya qamrovi

- `crates/iroha_core/tests/zk_ledger_scaffold.rs:288`–`345` bu siyosatni tasdiqlaydi
  o'tishlar metadata oniy tasvirlariga tarqaladi va qo'llanilganda tozalanadi.
- `crates/iroha_core/tests/zk_dedup.rs:1` `Preverify` keshi ekanligini isbotlaydi
  er-xotin-sarflar rad / ikki-dalil, qaerda aylanish stsenariylari, shu jumladan,
  majburiyatlar farqlanadi.
- `crates/iroha_core/tests/zk_confidential_events.rs` va
  `zk_shield_transfer_audit.rs` qopqog'i uchidan uchigacha qalqon → uzatish → ekrandan chiqarish
  oqimlar, parametr aylanishlari bo'ylab audit izining omon qolishini ta'minlaydi.
- `dashboards/grafana/confidential_assets.json` va
  `docs/source/confidential_assets.md:401` Commitment Tree & ni hujjatlashtiring
  har bir kalibrlash/aylanish ishiga hamroh bo'lgan verifier-kesh o'lchagichlari.

## Runbook egaligi

- **DevRel / Wallet SDK yetakchilari:** SDK snippetlari + ko‘rsatadigan tezkor boshlashlarni saqlang
  kutilayotgan o'tishlarni qanday yuzaga chiqarish va yalpizni qayta o'ynash → uzatish → oshkor qilish
  mahalliy testlar (`docs/source/project_tracker/confidential_assets_phase_c.md:M3.2` ostida kuzatilgan).
- **Program Mgmt / Confidential Assets TL:** o'tish so'rovlarini tasdiqlang, saqlang
  `status.md` bo'lajak aylanishlar bilan yangilanadi va voz kechishlar (agar mavjud bo'lsa) mavjudligiga ishonch hosil qiling.
  kalibrlash kitobi bilan birga qayd etilgan.