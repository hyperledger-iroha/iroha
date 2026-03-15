---
lang: uz
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T14:35:37.510319+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! `roadmap.md:M4` tomonidan havola qilingan maxfiy aktivlar auditi va operatsiyalar kitobi.

# Maxfiy aktivlar auditi va operatsiyalar kitobi

Ushbu qo'llanma auditorlar va operatorlar tayanadigan dalillarni birlashtiradi
maxfiy aktivlar oqimini tasdiqlashda. Bu aylanish o'yin kitobini to'ldiradi
(`docs/source/confidential_assets_rotation.md`) va kalibrlash kitobi
(`docs/source/confidential_assets_calibration.md`).

## 1. Tanlangan oshkor qilish va voqealar tasmalari

- Har bir maxfiy ko'rsatma tuzilgan `ConfidentialEvent` foydali yukini chiqaradi
  (`Shielded`, `Transferred`, `Unshielded`) suratga olingan
  `crates/iroha_data_model/src/events/data/events.rs:198` va seriyali
  ijrochilar (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`–`4021`).
  Regressiya to'plami aniq yuklarni amalga oshiradi, shuning uchun auditorlar ishonishlari mumkin
  deterministik JSON sxemalari (`crates/iroha_core/tests/zk_confidential_events.rs:19`–`299`).
- Torii bu hodisalarni standart SSE/WebSocket quvur liniyasi orqali ochib beradi; auditorlar
  `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`) yordamida obuna bo'lish,
  ixtiyoriy ravishda bitta aktiv ta'rifiga ko'ra. CLI misoli:

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "rose#wonderland" } }'
  ```

- Siyosat metama'lumotlari va kutilayotgan o'tishlar orqali mavjud
  `GET /v2/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`), Swift SDK tomonidan aks ettirilgan
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) va hujjatlashtirilgan
  ham maxfiy aktivlar dizayni, ham SDK qo'llanmalari
  (`docs/source/confidential_assets.md:70`, `docs/source/sdk/swift/index.md:334`).

## 2. Telemetriya, asboblar paneli va kalibrlash dalillari

- Ish vaqti ko'rsatkichlari daraxt chuqurligi, majburiyat/chegara tarixi, ildizni olib tashlash
  hisoblagichlar va verifier-kesh urish nisbatlari
  (`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`). Grafana asboblar paneli
  `dashboards/grafana/confidential_assets.json` tegishli panellarni jo'natadi va
  ogohlantirishlar, ish jarayoni `docs/source/confidential_assets.md:401` da hujjatlashtirilgan.
- Imzolangan jurnallar bilan kalibrlash ishlari (NS/op, gaz/op, ns/gas)
  `docs/source/confidential_assets_calibration.md`. Eng yangi Apple Silicon
  NEON ishga tushirilishi arxivlangan
  `docs/source/confidential_assets_calibration_neon_20260428.log` va xuddi shunday
  daftar SIMD-neytral va AVX2 profillari uchun vaqtinchalik voz kechishlarni qayd etadi
  x86 xostlari onlayn bo'ladi.

## 3. Hodisaga javob berish va operator vazifalari

- Aylanish/yangilash tartib-qoidalari mavjud
  `docs/source/confidential_assets_rotation.md`, yangisini qanday sahnalashtirishni o'z ichiga oladi
  parametrlar to'plami, siyosatni yangilashni rejalashtirish va hamyonlar/auditorlarni xabardor qilish. The
  treker (`docs/source/project_tracker/confidential_assets_phase_c.md`) ro'yxatlari
  runbook egalari va mashq kutishlari.
- Ishlab chiqarish mashqlari yoki favqulodda vaziyat oynalari uchun operatorlar dalillarni biriktiradilar
  `status.md` yozuvlari (masalan, ko'p qatorli mashq jurnali) va quyidagilarni o'z ichiga oladi:
  `curl` siyosatga oʻtish isboti, Grafana suratlari va tegishli voqea
  sindiradi, shuning uchun auditorlar yalpiz → uzatish → vaqt jadvallarini ko'rsatishi mumkin.

## 4. Tashqi ko'rib chiqish tezligi

- Xavfsizlikni tekshirish doirasi: maxfiy sxemalar, parametr registrlari, siyosat
  o'tish va telemetriya. Ushbu hujjat va kalibrlash daftarining shakllari
  sotuvchilarga yuborilgan dalillar paketi; ko'rib chiqish jadvali orqali kuzatiladi
  `docs/source/project_tracker/confidential_assets_phase_c.md` da M4.
- Operatorlar `status.md` ni har qanday sotuvchining topilmalari yoki kuzatuvlari bilan yangilab turishi kerak
  harakat elementlari. Tashqi ko'rib chiqish tugaguniga qadar, bu ish kitobi sifatida xizmat qiladi
  operatsion bazaviy auditorlar sinovdan o'tkazishlari mumkin.