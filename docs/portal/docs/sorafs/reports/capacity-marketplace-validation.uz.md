---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ffbb145e1e0aa9dc71bdb6896c4f8be69eb6226194c5c165905af1ac243cc9
source_last_modified: "2025-12-29T18:16:35.199832+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Capacity Marketplace Validation
tags: [SF-2c, acceptance, checklist]
summary: Acceptance checklist covering provider onboarding, dispute workflows, and treasury reconciliation gating the SoraFS capacity marketplace general availability.
translator: machine-google-reviewed
---

# SoraFS Imkoniyatlar bozorini tekshirish ro'yxati

**Ko‘rib chiqish oynasi:** 2026-03-18 → 2026-03-24  
**Dastur egalari:** Saqlash jamoasi (`@storage-wg`), Boshqaruv kengashi (`@council`), G'aznachilik gildiyasi (`@treasury`)  
**Qo‘llanish doirasi:** SF-2c GA uchun zarur bo‘lgan provayder quvurlari, nizolarni ko‘rib chiqish oqimlari va g‘aznachilikni yarashtirish jarayonlari.

Quyidagi nazorat ro'yxati tashqi operatorlar uchun bozorni yoqishdan oldin ko'rib chiqilishi kerak. Har bir qator auditorlar takrorlashi mumkin bo'lgan deterministik dalillarga (testlar, moslamalar yoki hujjatlar) bog'lanadi.

## Qabul qilish ro'yxati

### Provayderni ishga tushirish

| Tekshiring | Tasdiqlash | Dalil |
|-------|------------|----------|
| Registr kanonik sig'im deklaratsiyasini qabul qiladi | Integratsiya testi `/v2/sorafs/capacity/declare` ilova API orqali amalga oshiriladi, imzo bilan ishlov berish, metama'lumotlarni yozib olish va tugun registriga o'tkazishni tekshiradi. | `crates/iroha_torii/src/routing.rs:7654` |
| Smart kontrakt mos kelmaydigan foydali yuklarni rad etadi | Birlik testi davom etishdan oldin provayder identifikatorlari va belgilangan GiB maydonlari imzolangan deklaratsiyaga mos kelishini ta'minlaydi. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI kanonik onboarding artefaktlarini chiqaradi | CLI jabduqlari Norito/JSON/Base64 deterministik natijalarini yozadi va operatorlar deklaratsiyalarni oflayn rejimda ko'rsatishi uchun aylanma safarlarni tasdiqlaydi. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Operator qo'llanmasi qabul ish jarayoni va boshqaruv to'siqlarini qamrab oladi | Hujjatlar deklaratsiya sxemasini, siyosat defoltlarini va kengash uchun ko'rib chiqish bosqichlarini sanab o'tadi. | `../storage-capacity-marketplace.md` |

### Nizolarni hal qilish

| Tekshiring | Tasdiqlash | Dalil |
|-------|------------|----------|
| Eʼtirozli yozuvlar kanonik foydali yuk dayjestida saqlanib qoladi Birlik testi nizoni qayd etadi, saqlangan foydali yukni dekodlaydi va daftar determinizmini kafolatlash uchun kutilayotgan holatni tasdiqlaydi. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI nizo generatori kanonik sxemaga mos keladi | CLI testi `CapacityDisputeV1` uchun Base64/Norito chiqishlari va JSON xulosalarini qamrab oladi, bu esa dalillar to'plamlari xeshini aniq ta'minlaydi. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Takroriy test nizo/jarima determinizmini isbotlaydi | Ikki marta takrorlangan isbot-muvaffaqiyatsiz telemetriya bir xil daftar, kredit va bahsli suratlarni hosil qiladi, shuning uchun slashlar tengdoshlar orasida deterministik bo'ladi. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook hujjatlarini kuchaytirish va bekor qilish oqimi | Operatsion qo'llanma kengash ish jarayonini, dalillar talablarini va orqaga qaytarish tartib-qoidalarini qamrab oladi. | `../dispute-revocation-runbook.md` |

### G'aznachilikni yarashtirish

| Tekshiring | Tasdiqlash | Dalil |
|-------|------------|----------|
| Buxgalteriya hisobi 30 kunlik proektsiyaga mos keladi | Soak testi 30 ta hisob-kitob oynalari bo'ylab beshta provayderni qamrab oladi va buxgalteriya hisobidagi yozuvlarni kutilgan to'lov ma'lumotnomasidan farq qiladi. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Buxgalteriya daftarining eksportini solishtirish kechada qayd etiladi | `capacity_reconcile.py` toʻlov kitobi kutilmalarini bajarilgan XOR transfer eksporti bilan solishtiradi, Prometheus koʻrsatkichlarini chiqaradi va Alertmanager orqali xazina tasdiqlaydi. | `scripts/telemetry/capacity_reconcile.py:1`, `docs/source/sorafs/runbooks/capacity_reconciliation.md:1`, `dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Hisob-kitoblar panelidagi jarimalar va hisoblash telemetriyasi | Grafana import uchastkalari GiB·soat hisoblash, ish tashlash hisoblagichlari va qo'ng'iroq bo'yicha ko'rish uchun bog'langan garov. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Nashr qilingan hisobot arxivlari metodologiyasi va qayta ijro etish buyruqlari | Hisobot tafsilotlari auditorlar uchun ko'lami, bajarish buyruqlari va kuzatuv ilgaklari. | `./sf2c-capacity-soak.md` |

## Ijro qaydlari

Ro'yxatdan o'tishdan oldin tekshirish to'plamini qayta ishga tushiring:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Operatorlar `sorafs_manifest_stub capacity {declaration,dispute}` bilan ishga tushirish/nizo soʻrovining foydali yuklarini qayta yaratishi va natijada olingan JSON/Norito baytlarini boshqaruv chiptasi bilan birga arxivlashi kerak.

## Ro'yxatdan o'tish artefaktlari

| Artefakt | Yo'l | blake2b-256 |
|----------|------|-------------|
| Provayderning ishga kirishini tasdiqlash paketi | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Nizolarni hal qilishni tasdiqlash paketi | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| G'aznachilikni yarashtirishni tasdiqlash paketi | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Ushbu artefaktlarning imzolangan nusxalarini nashr to'plami bilan saqlang va ularni boshqaruvni o'zgartirish yozuviga bog'lang.

## Tasdiqlashlar

- Saqlash guruhi rahbari — @storage-tl (2026-03-24)  
- Boshqaruv kengashi kotibi — @council-sec (2026-03-24)  
- G'aznachilik operatsiyalari rahbari - @treasury-ops (2026-03-24)