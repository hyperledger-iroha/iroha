---
lang: uz
direction: ltr
source: docs/settlement-router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 782429c90ac5df034fd7c8ff2c3acf4f9f11348f14f15fcd321f343b22b154b8
source_last_modified: "2025-12-29T18:16:35.914434+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Deterministik hisob-kitob marshrutizatori (NX-3)

**Holat:** Tugallandi (NX-3)  
**Egalari:** Iqtisodiyot WG / Asosiy Ledger WG / G'aznachilik / SRE  
**Qoʻllanish doirasi:** Barcha qatorlar/maʼlumotlar boʻshliqlari tomonidan ishlatiladigan kanonik XOR hisob-kitob yoʻli. Yuborilgan marshrutizator qutisi, yo'lak darajasidagi kvitansiyalar, bufer qo'riqlash relslari, telemetriya va operator guvohnomalari.

## Maqsadlar
- Bir qatorli va Nexus tuzilmalari bo'ylab XOR konvertatsiyasi va kvitansiyalarni ishlab chiqarishni birlashtiring.
- Operatorlar xavfsiz tarzda hisob-kitob qilishlari uchun himoyalangan buferlar bilan deterministik soch turmagi + o'zgaruvchanlik chegaralarini qo'llang.
- Auditorlar maxsus asboblarsiz takrorlashi mumkin bo'lgan kvitantsiyalar, telemetriya va asboblar panelini ko'rsating.

## Arxitektura
| Komponent | Manzil | Mas'uliyat |
|-----------|----------|----------------|
| Router primitivlari | `crates/settlement_router/` | Soya narxlari kalkulyatori, sartaroshlik darajalari, bufer siyosati yordamchilari, hisob-kitob kvitansiyasi turi.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】【crates/settlement_router.rs:1】
| Runtime fasad | `crates/iroha_core/src/settlement/mod.rs:1` | Router konfiguratsiyasini `SettlementEngine` ga o'tkazadi, blokni bajarishda ishlatiladigan `quote` + akkumulyatorni ko'rsatadi. |
| Blok integratsiyasi | `crates/iroha_core/src/block.rs:120` | `PendingSettlement` yozuvlarini tozalaydi, `LaneSettlementCommitment` har bir qator/maʼlumotlar maydoni boʻyicha yigʻadi, chiziqli bufer metamaʼlumotlarini tahlil qiladi va telemetriyani chiqaradi. |
| Telemetriya va asboblar paneli | `crates/iroha_telemetry/src/metrics.rs:4847`, `dashboards/grafana/settlement_router_overview.json:1` | Prometheus/OTLP ko'rsatkichlari buferlar, dispersiya, sartaroshlar, konvertatsiyalar soni; SRE uchun Grafana taxtasi. |
| Malumot sxemasi | `docs/source/nexus_fee_model.md:1` | Hujjatlar hisob-kitob kvitansiyasi maydonlari `LaneBlockCommitment` da saqlanib qoldi. |

## Konfiguratsiya
Router tugmalari `[settlement.router]` ostida ishlaydi (`iroha_config` tomonidan tasdiqlangan):

```toml
[settlement.router]
twap_window_seconds = 60      # TWAP window used to derive local→XOR conversions
epsilon_bps = 25              # Base margin added to every quote (basis points)
buffer_alert_pct = 75         # Remaining-buffer % that opens an alert
buffer_throttle_pct = 25      # Remaining-buffer % where throttling begins
buffer_xor_only_pct = 10      # Remaining-buffer % where XOR-only mode is enforced
buffer_halt_pct = 2           # Remaining-buffer % where settlement halts
buffer_horizon_hours = 72     # Horizon (hours) represented by the XOR buffer
```

Har bir maʼlumot fazosi bufer hisobidagi chiziqli metamaʼlumotlar simlari:
- `settlement.buffer_account` — zaxirani saqlaydigan hisob (masalan, `buffer::cbdc_treasury`).
- `settlement.buffer_asset` - aktiv ta'rifi bosh maydoni uchun debetlanadi (odatda `xor#sora`).
- `settlement.buffer_capacity_micro` - micro-XOR (o'nlik qator) da sozlangan sig'im.

Metamaʼlumotlarning yoʻqligi ushbu yoʻlak uchun bufer snapshotini oʻchirib qoʻyadi (temetriya sigʻimi/holati nolga qaytadi).## Konvertatsiya quvuri
1. **Iqtibos:** `SettlementEngine::quote` sozlangan epsilon + volatillik chegarasi va soch turmagi darajasini TWAP kotirovkalari uchun qo‘llaydi, `SettlementReceipt` bilan `xor_due` va `xor_after_haircut` va qo‘ng‘iroq qiluvchini qaytaradi. `source_id`.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】
2. **To'plash:** Blokni bajarish jarayonida ijrochi `PendingSettlement` yozuvlarini (mahalliy miqdor, TWAP, epsilon, o'zgaruvchanlik paqiri, likvidlik profili, oracle vaqt tamg'asi) yozib oladi. Blokni yopishdan oldin `LaneSettlementBuilder` jami yig'adi va metama'lumotlarni har `(lane, dataspace)` uchun almashtiradi.【crates/iroha_core/src/settlement/mod.rs:34】【crates/iroha_core/src/block.rs:3460】
3. **Bufer snapshoti:** Agar chiziq metamaʼlumotlari buferni eʼlon qilsa, quruvchi config.【crates/iroha_3.core-dan `BufferPolicy` chegaralari yordamida `SettlementBufferSnapshot` (qolgan boʻsh joy, sigʻim, holat) oladi.
4. **Tasdiqlash + telemetriya:** Qabul qilish va dalillarni almashtirish `LaneBlockCommitment` ichiga tushadi va holat snapshotlarida aks ettiriladi. Telemetriya bufer o'lchagichlarni, dispersiyani (`iroha_settlement_pnl_xor`), qo'llaniladigan marjani (`iroha_settlement_haircut_bp`), ixtiyoriy almashtirishdan foydalanishni va har bir ob'ektga konvertatsiya/soch kesish hisoblagichlarini yozib oladi, shuning uchun asboblar paneli va ogohlantirishlar blok bilan sinxronlanadi. tarkibi.【crates/iroha_core/src/block.rs:298】【crates/iroha_core/src/telemetry.rs:844】
5. **Dalillar yuzalari:** `status::set_lane_settlement_commitments` o‘rni/DA iste’molchilari uchun majburiyatlarni e’lon qiladi, Grafana asboblar paneli Prometheus ko‘rsatkichlarini o‘qiydi va operatorlar `ops/runbooks/settlement-buffers.md` dan I000 to‘g‘risidagi trek bilan birga `ops/runbooks/settlement-buffers.md` dan foydalanadilar. to'ldirish/bo'lish hodisalari.

## Telemetriya va dalillar
- `iroha_settlement_buffer_xor`, `iroha_settlement_buffer_capacity_xor`, `iroha_settlement_buffer_status` - har bir qator/ma'lumotlar maydoni uchun bufer oniy tasviri (mikro-XOR + kodlangan holat).【crates/iroha_telemetry/src/metrics.rs:621】
- `iroha_settlement_pnl_xor` - blokli partiya uchun soch turmagidan keyingi XOR va muddati o'rtasidagi farqni aniqladi.【crates/iroha_telemetry/src/metrics.rs:6236】
- `iroha_settlement_haircut_bp` - to'plamga qo'llaniladigan samarali epsilon/soch kesish punktlari.【crates/iroha_telemetry/src/metrics.rs:6244】
- `iroha_settlement_swapline_utilisation` - almashtirish dalillari mavjud bo'lganda likvidlik profili bo'yicha ixtiyoriy foydalanish.【crates/iroha_telemetry/src/metrics.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — hisob-kitoblarni konvertatsiya qilish va jamlangan sartaroshlik uchun har bir tarmoqli/ma'lumotlar maydoni hisoblagichlari (XOR birliklari).【crates/iroha_telemetry/src/metrics.rs:6260】【crates/iroha:4】corers/irob
- Grafana doskasi: `dashboards/grafana/settlement_router_overview.json` (bufer bo'shlig'i, farqlar, soch turmagi) va Nexus qatorli ogohlantirish paketiga kiritilgan Alertmanager qoidalari.
- Operator ish kitobi: `ops/runbooks/settlement-buffers.md` (to'ldirish/ogohlantirish ish oqimi) va `docs/source/nexus_settlement_faq.md` da tez-tez so'raladigan savollar.## Dasturchilar va SRE nazorat ro'yxati
- `[settlement.router]` qiymatlarini `config/config.json5` (yoki TOML) da o'rnating va `irohad --version` jurnallari orqali tasdiqlang; chegaralar `alert > throttle > xor_only > halt` ga mos kelishiga ishonch hosil qiling.
- Yo'lak metama'lumotlarini bufer hisobi/aktivi/sig'imi bilan to'ldiring, shunda bufer o'lchagichlar jonli zaxiralarni aks ettiradi; buferlarni kuzatmasligi kerak bo'lgan qatorlar uchun maydonlarni o'tkazib yuboring.
- `settlement_router_*` va `iroha_settlement_*` ko'rsatkichlarini `dashboards/grafana/settlement_router_overview.json` orqali kuzatib boring; gaz kelebeği/faqat XOR/to'xtash holatlarida ogohlantirish.
- `crates/iroha_core/src/block.rs` da narxlash/siyosat qamrovi va mavjud blok-darajali agregatsiya testlari uchun `cargo test -p settlement_router` ni ishga tushiring.
- `docs/source/nexus_fee_model.md` da konfiguratsiya oʻzgarishlari uchun boshqaruv tasdiqlarini yozib oling va chegaralar yoki telemetriya sirtlari oʻzgarganda `status.md` yangilangan holda saqlang.

## Chiqarish rejasining surati
- Har bir qurilishda marshrutizator + telemetriya kemasi; xususiyat eshiklari yo'q. Yoʻlak metamaʼlumotlari bufer lahzalari nashr etilishini nazorat qiladi.
- Standart konfiguratsiya yo'l xaritasi qiymatlariga mos keladi (60s TWAP, 25bp asosiy epsilon, 72h bufer gorizonti); konfiguratsiya orqali sozlang va qo'llash uchun `irohad` ni qayta ishga tushiring.
- Dalillar toʻplami = `settlement_router_*`/`iroha_settlement_*` seriyasi uchun Prometheus qirqish + Prometheus taʼsirlangan oyna uchun skrinshot/JSON eksporti.

## Dalillar va ma'lumotnomalar
- NX-3 hisob-kitob routerini qabul qilish bo'yicha eslatmalar: `status.md` (NX-3 bo'limi).
- Operator sirtlari: `dashboards/grafana/settlement_router_overview.json`, `ops/runbooks/settlement-buffers.md`.
- Qabul qilish sxemasi va API sirtlari: `docs/source/nexus_fee_model.md`, `/v1/sumeragi/status` -> `lane_settlement_commitments`.