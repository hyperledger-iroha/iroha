---
id: nexus-refactor-plan
lang: uz
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Nexus ledger refactor plan
description: Mirror of `docs/source/nexus_refactor_plan.md`, detailing the phased clean-up work for the Iroha 3 codebase.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
Bu sahifa `docs/source/nexus_refactor_plan.md`ni aks ettiradi. Ko‘p tilli nashr portalga tushmaguncha ikkala nusxani ham bir tekisda saqlang.
:::

# Sora Nexus Ledger Refactor rejasi

Ushbu hujjat Sora Nexus Ledger ("Iroha 3") refaktorining bevosita yo'l xaritasini qamrab oladi. U joriy ombor tartibini va genezis/WSV buxgalteriya hisobi, Sumeragi konsensus, smart-kontrakt triggerlari, oniy tasvir so'rovlari, ko'rsatgich-ABI xost ulanishlari va Norito kodeklarida kuzatilgan regressiyalarni aks ettiradi. Maqsad, barcha tuzatishlarni bitta monolit yamoqqa tushirishga urinmasdan, izchil, sinovdan o'tkaziladigan arxitekturada birlashishdir.

## 0. Boshlovchi tamoyillar
- heterojen qurilmalarda deterministik xatti-harakatlarni saqlash; tezlashuvdan faqat bir xil zaxiralarga ega bo'lgan kirish funksiyasi bayroqlari orqali foydalaning.
- Norito - seriyali qatlam. Har qanday holat/sxema oʻzgarishlari Norito kodlash/dekodlash boʻylab sayohat sinovlari va moslama yangilanishlarini oʻz ichiga olishi kerak.
- Konfiguratsiya `iroha_config` orqali o'tadi (foydalanuvchi → haqiqiy → standart). Ishlab chiqarish yo'llaridan vaqtinchalik muhit o'tish tugmalarini olib tashlang.
- ABI siyosati V1 bo'lib qoladi va muhokama qilinmaydi. Xostlar noma'lum ko'rsatkich turlarini/tizimlarini deterministik ravishda rad etishlari kerak.
- `cargo test --workspace` va oltin testlar (`ivm`, `norito`, `integration_tests`) har bir muhim bosqich uchun asosiy eshik bo'lib qoladi.

## 1. Repozitoriy topologiyasining surati
- `crates/iroha_core`: Sumeragi aktyorlari, WSV, genezis yuklagich, quvurlar (so'rovlar, qoplamalar, zk yo'llari), smart-kontrakt xost elim.
- `crates/iroha_data_model`: zanjirdagi ma'lumotlar va so'rovlar uchun vakolatli sxema.
- `crates/iroha`: CLI, testlar, SDK tomonidan ishlatiladigan mijoz API.
- `crates/iroha_cli`: operator CLI, hozirda `iroha`-da ko'plab API-larni aks ettiradi.
- `crates/ivm`: Kotodama bayt kodli VM, pointer-ABI xost integratsiyasiga kirish nuqtalari.
- `crates/norito`: JSON adapterlari va AoS/NCB backendlari bilan seriyali kodek.
- `integration_tests`: genezis/bootstrap, Sumeragi, triggerlar, sahifalash va h.k.larni qamrab oluvchi oʻzaro komponentli tasdiqlar.
- Hujjatlarda allaqachon Sora Nexus Ledger maqsadlari (`nexus.md`, `new_pipeline.md`, `ivm.md`) tasvirlangan, ammo dastur parchalangan va kodga nisbatan qisman eskirgan.

## 2. Refaktor ustunlari va bosqichlari

### A bosqichi – asoslar va kuzatuvchanlik
1. **WSV Telemetriya + Snapshots**
   - So'rovlar, Sumeragi va CLI tomonidan ishlatiladigan `state` (`WorldStateSnapshot` xususiyati) da kanonik suratga olish APIsini o'rnating.
   - `iroha state dump --format norito` orqali deterministik suratlarni yaratish uchun `scripts/iroha_state_dump.sh` dan foydalaning.
2. **Genesis/Bootstrap Determinizm**
   - Norito quvvatli bitta quvur liniyasi (`iroha_core::genesis`) orqali oqib o'tish uchun refaktor genezisi.
  - Genesis va birinchi blokni takrorlaydigan va arm64/x86_64 (`integration_tests/tests/genesis_replay_determinism.rs` ostida kuzatilgan) bo'ylab bir xil WSV ildizlarini tasdiqlovchi integratsiya/regressiya qamrovini qo'shing.
3. **Kross-kortlarni mahkamlash sinovlari**
   - WSV, quvur liniyasi va ABI invariantlarini bitta jabduqda tekshirish uchun `integration_tests/tests/genesis_json.rs` ni kengaytiring.
  - Sxema driftida vahima qo'yadigan `cargo xtask check-shape` iskalasini joriy qiling (DevEx asboblar to'plamida kuzatilgan; `scripts/xtask/README.md` amal bandiga qarang).

### B bosqichi - WSV va so'rovlar yuzasi
1. **Davlat saqlash operatsiyalari**
   - `state/storage_transactions.rs` ni buyurtma berish va ziddiyatlarni aniqlashni ta'minlovchi tranzaksiya adapteriga aylantiring.
   - Birlik testlari endi aktivlar/dunyo/tetiklar o'zgarishlari muvaffaqiyatsizlikka uchraganini tasdiqlaydi.
2. **Refactor soʻrovi**
   - `crates/iroha_core/src/query/` ostida sahifalash/kursor mantiqini qayta foydalanish mumkin bo'lgan komponentlarga o'tkazing. `iroha_data_model` da Norito tasvirlarini tekislang.
  - Triggerlar, aktivlar va rollar uchun deterministik tartib bilan snapshot so'rovlarini qo'shing (joriy qamrov uchun `crates/iroha_core/tests/snapshot_iterable.rs` orqali kuzatiladi).
3. **Snapshot izchilligi**
   - `iroha ledger query` CLI Sumeragi/fetchers bilan bir xil suratga olish yoʻlidan foydalanishiga ishonch hosil qiling.
   - CLI snapshot regressiya testlari `tests/cli/state_snapshot.rs` ostida ishlaydi (sekin ishlash uchun xususiyatga ega).

### Faza C - Sumeragi quvur liniyasi
1. **Topologiya va davrni boshqarish**
   - `EpochRosterProvider` ni WSV stake snapshotlari tomonidan qoʻllab-quvvatlanadigan ilovalar bilan xususiyatga ajratib oling.
  - `WsvEpochRosterAdapter::from_peer_iter` skameykalar/sinovlar uchun oddiy soxta konstruktorni taklif etadi.
2. **Konsensus oqimini soddalashtirish**
   - `crates/iroha_core/src/sumeragi/*`-ni modullarga qayta tashkil qiling: `consensus` ostida umumiy turdagi `pacemaker`, `aggregation`, `availability`, `witness`.
  - Vaqtinchalik xabar o'tishini terilgan Norito konvertlari bilan almashtiring va ko'rishni o'zgartirish xususiyati testlarini kiriting (Sumeragi xabar almashish to'plamida kuzatilgan).
3. **Line/Proof integratsiyasi**
   - Yo'lakni tasdiqlovchi hujjatlarni DA majburiyatlari bilan moslang va qizil qon tanachalari bir xil bo'lishini ta'minlang.
   - `integration_tests/tests/extra_functional/seven_peer_consistency.rs` end-to-end integratsiya testi endi RBC yoqilgan yo'lni tekshiradi.

### D bosqichi - Aqlli shartnomalar va Pointer-ABI xostlari
1. **Mezbon chegarasi auditi**
   - Pointer tipidagi tekshiruvlarni (`ivm::pointer_abi`) va xost adapterlarini (`iroha_core::smartcontracts::ivm::host`) birlashtirish.
   - Pointer jadvali kutilmalari va xost manifest bog'lashlari `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` va `ivm_host_mapping.rs` tomonidan qoplanadi, ular oltin TLV xaritalarini qo'llaydi.
2. **Trigger Execution Sandbox**
   - Refactor umumiy `TriggerExecutor` orqali ishga tushirish uchun ishga tushiradi, bu gaz, ko'rsatkichni tekshirish va hodisalar jurnalini amalga oshiradi.
  - Muvaffaqiyatsizlik yo'llarini qamrab oluvchi qo'ng'iroq/vaqt triggerlari uchun regressiya testlarini qo'shing (`crates/iroha_core/tests/trigger_failure.rs` orqali kuzatiladi).
3. **CLI & Client Alignment**
   - Driftni oldini olish uchun CLI operatsiyalari (`audit`, `gov`, `sumeragi`, `ivm`) umumiy `iroha` mijoz funksiyalariga tayanishiga ishonch hosil qiling.
   - CLI JSON snapshot testlari `tests/cli/json_snapshot.rs` da ishlaydi; ularni yangilab turing, shuning uchun asosiy buyruq chiqishi kanonik JSON ma'lumotnomasiga mos kelishini davom ettiradi.

### Faza E - Norito Kodekni qattiqlashtirish
1. **Sxema reestri**
   - Asosiy ma'lumotlar turlari uchun kanonik kodlash manbalarini yaratish uchun `crates/norito/src/schema/` ostida Norito sxema registrini yarating.
   - Namunaviy yukni kodlashni tasdiqlovchi hujjat testlari qo'shildi (`norito::schema::SamplePayload`).
2. **Oltin armatura yangilanishi**
   - `crates/norito/tests/*` oltin moslamalarini yangi WSV sxemasiga moslash uchun yangilang.
   - `scripts/norito_regen.sh` `norito_regen_goldens` yordamchisi orqali Norito JSON oltin ranglarini deterministik tarzda qayta tiklaydi.
3. **IVM/Norito integratsiyasi**
   - Kotodama manifest serializatsiyasini Norito orqali oxirigacha tasdiqlang, bu ko'rsatkich ABI metama'lumotlarining izchilligini ta'minlash.
   - `crates/ivm/tests/manifest_roundtrip.rs` manifestlar uchun Norito kodlash/dekodlash paritetini saqlaydi.

## 3. O'zaro bog'liqlik
- **Sinov strategiyasi**: Har bir bosqichda birlik testlari → kassa testlari → integratsiya testlari targ'ib qilinadi. Muvaffaqiyatsiz testlar joriy regressiyalarni ushlaydi; yangi sinovlar ularning qayta tiklanishiga to'sqinlik qiladi.
- **Hujjatlar**: Har bir bosqichdan so'ng, `status.md`-ni yangilang va tugallangan vazifalarni kesish paytida ochiq elementlarni `roadmap.md`-ga aylantiring.
- **Ishlash mezonlari**: `iroha_core`, `ivm` va `norito` da mavjud dastgohlarni saqlab turish; hech qanday regressiyani tasdiqlash uchun refaktordan keyingi asosiy o'lchovlarni qo'shing.
- **Xususiyatlar bayroqlari**: faqat tashqi asboblar zanjirini talab qiladigan orqa uchlar uchun quti darajasidagi o'zgartirishlarni saqlang (`cuda`, `zk-verify-batch`). CPU SIMD yo'llari har doim ish vaqtida quriladi va tanlanadi; qo'llab-quvvatlanmaydigan apparat uchun deterministik skalyar zaxiralarni ta'minlash.

## 4. Darhol keyingi harakatlar
- Faza A iskala (suratli xususiyat + telemetriya simlari) - yo'l xaritasi yangilanishlarida amalga oshirilishi mumkin bo'lgan vazifalarni ko'ring.
- `sumeragi`, `state` va `ivm` uchun yaqinda o'tkazilgan nuqsonlar auditi quyidagi muhim jihatlarni ko'rsatdi:
  - `sumeragi`: o'lik kod imtiyozlari ko'rishni o'zgartirishga qarshi translyatsiyani, VRF takrorlash holatini va EMA telemetriya eksportini himoya qiladi. Faza C konsensus oqimini soddalashtirish va chiziqli/dalil integratsiyani yetkazib berish mumkin bo'lgunga qadar ular yopiq qoladi.
  - `state`: `Cell` tozalash va telemetriya marshruti A faza WSV telemetriya trekiga o'tadi, SoA/parallel qo'llash qaydlari esa C fazali quvur liniyasini optimallashtirish to'plamiga o'tadi.
  - `ivm`: CUDA ekspozitsiyasini, konvertni tekshirishni va Halo2/Metal qamrovi xaritasini D fazasi xost-chegaraviy ishiga va o'zaro faoliyat GPU tezlashtirish mavzusiga o'tkazish; yadrolar tayyor bo'lgunga qadar maxsus GPU zaxirasida qoladi.
- Invaziv kod o'zgarishlarini kiritishdan oldin ushbu rejani ro'yxatdan o'tkazish uchun jamlagan holda jamoalararo RFC tayyorlang.

## 5. Ochiq savollar
- RBC P1 dan keyin ixtiyoriy bo'lib qolishi kerakmi yoki Nexus daftar yo'llari uchun majburiymi? Manfaatdor tomonlarning qarorini talab qiladi.
- Biz P1 da DS birlashma guruhlarini qo'llaymizmi yoki yo'lak isbotlari etgunga qadar ularni o'chirib qo'yamizmi?
- ML-DSA-87 parametrlari uchun kanonik joylashuv qanday? Nomzod: yangi `crates/fastpq_isi` qutisi (yaratilishi kutilmoqda).

---

_Oxirgi yangilangan: 2025-09-12_