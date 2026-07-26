---
lang: uz
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-12-29T18:16:35.929771+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Hisoblash liniyasi (SSC-1)

Hisoblash chizig'i HTTP uslubidagi deterministik qo'ng'iroqlarni qabul qiladi, ularni Kotodama ga ko'rsatadi
kirish punktlari va hisob-kitoblar va boshqaruvni ko'rib chiqish uchun o'lchash/kvitansiyalarni qayd qiladi.
Ushbu RFC manifest sxemasini, qo'ng'iroq/qabul qilish konvertlarini, qum qutisi to'siqlarini,
va birinchi versiya uchun standart konfiguratsiya.

## Manifest

- Sxema: `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`).
- `abi_version` `1` ga mahkamlangan; boshqa versiyadagi manifestlar rad etiladi
  tasdiqlash paytida.
- Har bir marshrut quyidagilarni e'lon qiladi:
  - `id` (`service`, `method`)
  - `entrypoint` (Kotodama kirish nuqtasi nomi)
  - kodek ruxsat etilgan ro'yxati (`codecs`)
  - TTL/gaz/so'rov/javob chegaralari (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - determinizm/bajarish klassi (`determinism`, `execution_class`)
  - SoraFS kirish/model identifikatorlari (`input_limits`, ixtiyoriy `model`)
  - narxlash oilasi (`price_family`) + resurs profili (`resource_profile`)
  - autentifikatsiya siyosati (`auth`)
- Sandbox to'siqlari manifest `sandbox` blokida yashaydi va hamma tomonidan taqsimlanadi
  marshrutlar (rejim/tasodifiylik/saqlash va deterministik bo'lmagan tizim chaqiruvini rad etish).

Misol: `fixtures/compute/manifest_compute_payments.json`.

## Qo'ng'iroqlar, so'rovlar va kvitansiyalar

- Sxema: `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome` ichida
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()` kanonik so'rov xeshini ishlab chiqaradi (sarlavhalar saqlanadi
  deterministik `BTreeMap`da va foydali yuk `payload_hash` sifatida tashiladi).
- `ComputeCall` nom maydoni/marshrut, kodek, TTL/gaz/javob qopqog'ini ushlaydi,
  resurs profili + narxlar oilasi, autentifikatsiya (`Public` yoki UAID bilan bog'langan
  `ComputeAuthn`), determinizm (`Strict` va `BestEffort`), ijro klassi
  maslahatlar (CPU/GPU/TEE), e'lon qilingan SoraFS kirish baytlari/bo'laklari, ixtiyoriy homiy
  byudjet va kanonik so'rov konverti. uchun so'rov xeshi ishlatiladi
  takrorlashni himoya qilish va marshrutlash.
- Marshrutlar ixtiyoriy SoraFS modeliga havolalar va kirish chegaralarini joylashtirishi mumkin
  (inline/bo'lak bosh harflar); manifest sandbox qoidalari darvozasi GPU/TEE maslahatlari.
- `ComputePriceWeights::charge_units` o'lchash ma'lumotlarini hisob-kitob hisobiga aylantiradi
  tsikllar va chiqish baytlari bo'yicha ship-bo'linish orqali birliklar.
- `ComputeOutcome` hisobotlari `Success`, `Timeout`, `OutOfMemory`,
  `BudgetExhausted` yoki `InternalError` va ixtiyoriy ravishda javob xeshlarini o'z ichiga oladi/
  audit uchun o'lchamlar/kodek.

Misollar:
- Qo'ng'iroq qiling: `fixtures/compute/call_compute_payments.json`
- Kvitansiya: `fixtures/compute/receipt_compute_payments.json`

## Sandbox va resurs profillari- `ComputeSandboxRules` sukut bo'yicha ijro rejimini `IvmOnly` ga qulflaydi,
  so'rov xeshidan urug'larning deterministik tasodifiyligi, faqat o'qishga ruxsat beradi SoraFS
  kirish va deterministik bo'lmagan tizim chaqiruvlarini rad etadi. GPU/TEE maslahatlari tomonidan himoyalangan
  `allow_gpu_hints`/`allow_tee_hints` ijroni deterministik saqlash uchun.
- `ComputeResourceBudget` sikllarda, chiziqli xotirada, stekda har bir profil uchun cheklarni o'rnatadi
  hajmi, IO byudjeti va chiqish, shuningdek, GPU maslahatlari va WASI-lite yordamchilari uchun o'tish-o'tish.
- Standartlar ikkita profilni yuboradi (`cpu-small`, `cpu-balanced`).
  `defaults::compute::resource_profiles` deterministik qaytarilishlar bilan.

## Narxlar va hisob-kitob birliklari

- Narxlar oilalari (`ComputePriceWeights`) sikllarni xaritalash va hisoblashga chiqish baytlarini
  birliklar; standart to'lov `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` bilan
  `unit_label = "cu"`. Oilalar manifestlarda `price_family` tomonidan kalit va
  qabul qilinganda amalga oshiriladi.
- O'lchash yozuvlari `charged_units` va xom tsikl/kirish/chiqish/davomiylikni o'z ichiga oladi
  yarashtirish uchun jami. To'lovlar ijro klassi va tomonidan kuchaytiriladi
  determinizm ko'paytmalari (`ComputePriceAmplifiers`) va chegaralangan
  `compute.economics.max_cu_per_call`; chiqish orqali qisiladi
  `compute.economics.max_amplification_ratio` bog'langan javob kuchaytirilishiga.
- Homiy budjetlari (`ComputeCall::sponsor_budget_cu`) ga qarshi amalga oshiriladi
  har bir qo'ng'iroq / kunlik chegaralar; hisoblangan birliklar e'lon qilingan homiy byudjetidan oshmasligi kerak.
- Boshqaruv narxlari yangilanishlarida risklar toifasi chegaralaridan foydalaniladi
  `compute.economics.price_bounds` va asosiy oilalar ichida qayd etilgan
  `compute.economics.price_family_baseline`; foydalanish
  Yangilashdan oldin deltalarni tekshirish uchun `ComputeEconomics::apply_price_update`
  faol oila xaritasi. Torii konfiguratsiya yangilanishlari ishlatiladi
  `ConfigUpdate::ComputePricing` va kiso uni bir xil chegaralar bilan qo'llaydi
  boshqaruv tahrirlarini deterministik saqlang.

## Konfiguratsiya

Yangi hisoblash konfiguratsiyasi `crates/iroha_config/src/parameters` da ishlaydi:

- Foydalanuvchi ko'rinishi: `Compute` (`user.rs`) env bekor qilish bilan:
  - `COMPUTE_ENABLED` (standart `false`)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- Narxlar/iqtisod: `compute.economics` ushlaydi
  `max_cu_per_call`/`max_amplification_ratio`, toʻlovni taqsimlash, homiylik cheklovlari
  (har bir qo'ng'iroq va kunlik CU), narx oilasining asosiy ko'rsatkichlari + xavf sinflari/chegaralari
  boshqaruv yangilanishlari va ijro toifasi multiplikatorlari (GPU/TEE/best-efort).
- Haqiqiy/standart: `actual.rs` / `defaults.rs::compute` tahlil qilingan
  `Compute` sozlamalari (nomlar bo'shliqlari, profillar, narxlar oilalari, sandbox).
- Yaroqsiz konfiguratsiyalar (bo'sh nomlar maydoni, standart profil/oila yo'q, TTL chegarasi
  inversiyalar) tahlil qilish paytida `InvalidComputeConfig` sifatida yuzaga keladi.

## Sinovlar va moslamalar

- Deterministik yordamchilar (`request_hash`, narxlash) va armatura aylanma sayohatlari
  `crates/iroha_data_model/src/compute/mod.rs` (qarang: `fixtures_round_trip`,
  `request_hash_is_stable`, `pricing_rounds_up_units`).
- JSON moslamalari `fixtures/compute/` da ishlaydi va ma'lumotlar modeli tomonidan amalga oshiriladi
  regressiyani qamrab olish uchun testlar.

## SLO jabduqlari va byudjetlari- `compute.slo.*` konfiguratsiyasi shlyuz SLO tugmalarini (parvoz navbati) ochib beradi
  chuqurlik, RPS chegarasi va kechikish maqsadlari) ichida
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. Standart: 32
  parvozda, har bir marshrutda 512 ta navbat, 200 RPS, p50 25ms, p95 75ms, p99 120ms.
- SLO xulosalari va so'rov/chiqishni olish uchun engil dastgoh jabduqlarini ishga tushiring
  snapshot: `cargo run -p xtask --bin compute_gateway -- dastgoh [manifest_path]
  [iteratsiyalar] [bir vaqtning o'zida] [out_dir]` (defaults: `fixtures/compute/manifest_compute_payments.json`,
  128 iteratsiya, parallellik 16, chiqishlar ostida
  `artifacts/compute_gateway/bench_summary.{json,md}`). Skameykadan foydalanadi
  deterministik foydali yuklar (`fixtures/compute/payload_compute_payments.json`) va
  Mashq qilish paytida takroriy to'qnashuvlarni oldini olish uchun har bir so'rov sarlavhalari
  `echo`/`uppercase`/`sha3` kirish nuqtalari.

## SDK/CLI paritetlari

- Kanonik qurilmalar `fixtures/compute/` ostida ishlaydi: manifest, chaqiruv, foydali yuk va
  shlyuz uslubidagi javob/kvitansiya tartibi. Foydali yuk xeshlari qo'ng'iroqqa mos kelishi kerak
  `request.payload_hash`; yordamchi yuk yashaydi
  `fixtures/compute/payload_compute_payments.json`.
- CLI `iroha compute simulate` va `iroha compute invoke` ni yuboradi:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` yashaydi
  ostida regressiya testlari bilan `javascript/iroha_js/src/compute.js`
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift: `ComputeSimulator` bir xil moslamalarni yuklaydi, foydali yuk xeshlarini tasdiqlaydi,
  va kirish nuqtalarini testlar bilan simulyatsiya qiladi
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- CLI/JS/Swift yordamchilarining barchasi bir xil Norito moslamalariga ega, shuning uchun SDKlar
  a tugmachasini bosmasdan so'rovni qurish va hash bilan ishlashni oflayn rejimda tekshirish
  ishlaydigan shlyuz.