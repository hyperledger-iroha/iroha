---
lang: mn
direction: ltr
source: docs/settlement-router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 782429c90ac5df034fd7c8ff2c3acf4f9f11348f14f15fcd321f343b22b154b8
source_last_modified: "2025-12-29T18:16:35.914434+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Тодорхойлогч төлбөр тооцооны чиглүүлэгч (NX-3)

**Төлөв:** Дууссан (NX-3)  
**Эзэмшигч:** Эдийн засгийн АХ / Үндсэн дэвтэр АХ / Төрийн сан / SRE  
**Хамрах хүрээ:** Бүх эгнээ/өгөгдлийн орон зайд ашигладаг каноник XOR тооцооны зам. Ачаалагдсан чиглүүлэгчийн хайрцаг, эгнээний түвшний баримт, буфер хамгаалалтын хашлага, телеметр, операторын нотлох гадаргуу.

## Зорилго
- Нэг эгнээ болон Nexus бүтээн байгуулалтууд дээр XOR хөрвүүлэлт болон төлбөрийн баримтыг нэгтгэх.
- Операторууд төлбөр тооцоог аюулгүй хурдасгах боломжтой болохын тулд тодорхой үсний засалт + тогтворгүй байдлын хязгаарыг хамгаалалттай буфер ашиглан хий.
- Аудиторууд захиалгат багаж хэрэгсэлгүйгээр дахин тоглуулах боломжтой төлбөрийн баримт, телеметр, хяналтын самбарыг ил гарга.

## Архитектур
| Бүрэлдэхүүн хэсэг | Байршил | Хариуцлага |
|----------|----------|----------------|
| Чиглүүлэгчийн командууд | `crates/settlement_router/` | Сүүдрийн үнийн тооцоолуур, үс засах түвшин, буфер бодлогын туслахууд, төлбөр тооцооны баримтын төрөл.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】【crates/settlement_router/rs.1】rs/s |
| Runtime fasad | `crates/iroha_core/src/settlement/mod.rs:1` | Чиглүүлэгчийн тохиргоог `SettlementEngine` руу оруулж, блок гүйцэтгэх явцад ашигласан `quote` + аккумляторыг харуулна. |
| Блок нэгтгэх | `crates/iroha_core/src/block.rs:120` | `PendingSettlement` бичлэгийг устгаж, `LaneSettlementCommitment`-ийг эгнээ/өгөгдлийн орон зай болгон нэгтгэж, эгнээний буферийн мета өгөгдлийг задлан шинжилж, телеметрийг ялгаруулдаг. |
| Телеметр ба хяналтын самбар | `crates/iroha_telemetry/src/metrics.rs:4847`, `dashboards/grafana/settlement_router_overview.json:1` | Prometheus/OTLP хэмжигдэхүүн, буфер, хэлбэлзэл, үс засах, хөрвүүлэх тоо; SRE-д зориулсан Grafana самбар. |
| Лавлах схем | `docs/source/nexus_fee_model.md:1` | Баримт бичгийн төлбөр тооцооны талбарууд `LaneBlockCommitment`-д хэвээр байна. |

## Тохиргоо
Чиглүүлэгчийн товчлуурууд нь `[settlement.router]`-ийн дагуу ажилладаг (`iroha_config` баталгаажуулсан):

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

Өгөгдлийн орон зай тус бүрийн буфер бүртгэл дэх эгнээний мета өгөгдлийн утас:
- `settlement.buffer_account` — нөөцийг хадгалдаг данс (жишээ нь, `buffer::cbdc_treasury`).
- `settlement.buffer_asset` — хөрөнгийн тодорхойлолтыг дээд хязгаарт дебит болгосон (ихэвчлэн `xor#sora`).
- `settlement.buffer_capacity_micro` — микро-XOR (аравтын тоо) дахь багтаамжийг тохируулсан.

Мета өгөгдөл байхгүй байгаа нь тухайн эгнээний буфер агшин зуурын зургийг идэвхгүй болгодог (телеметрийн хүчин чадал/төлөв дахин тэг болсон).## Хөрвүүлэх дамжуулах хоолой
1. **Ишлэл:** `SettlementEngine::quote` нь тохируулсан epsilon + тогтворгүй байдлын хязгаар болон үс засах түвшнийг TWAP ишлэлд хэрэглэж, `SettlementReceipt`-ийг `xor_due`, `xor_after_haircut`-тэй залгасан, нэмсэн тоогоор буцаана. `source_id`.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】
2. **Хуримтлуулах:** Блок гүйцэтгэх явцад гүйцэтгэгч `PendingSettlement` оруулгуудыг (локал дүн, TWAP, epsilon, хэлбэлзлийн хувин, хөрвөх чадварын профайл, oracle цагийн тэмдэг) бүртгэдэг. `LaneSettlementBuilder` блокыг битүүмжлэхийн өмнө нийт дүнг нэгтгэж, мета өгөгдлийг `(lane, dataspace)` болгон солино.【crates/iroha_core/src/settlement/mod.rs:34】【crates/iroha_core/src/block.rs:3460】
3. **Буферийн хормын хувилбар:** Хэрэв эгнээний мета өгөгдөл нь буфер гэж зарлавал бүтээгч config.【crates/iroha_3.b.-аас `BufferPolicy` босго ашиглан `SettlementBufferSnapshot` (үлдсэн зай, багтаамж, статус) авдаг.
4. **Commit + телеметр:** Хүлээн авсан баримтууд болон своп нотлох баримтууд нь `LaneBlockCommitment` дотор байрлах ба статусын агшин агшинд тусгагдсан болно. Телеметр нь буфер хэмжигч, хэлбэлзэл (`iroha_settlement_pnl_xor`), ашигласан маржин (`iroha_settlement_haircut_bp`), нэмэлт своплайн ашиглалт, хөрөнгө тус бүрийн хөрвүүлэлт/үс засах тоолуурыг бүртгэдэг тул хяналтын самбар болон анхааруулга нь блоктой синхрон байх болно. агуулга.【crates/iroha_core/src/block.rs:298】【crates/iroha_core/src/telemetry.rs:844】
5. **Нотлох баримтын гадаргуу:** `status::set_lane_settlement_commitments` нь реле/ДА хэрэглэгчдэд зориулсан амлалтуудыг нийтэлдэг, Grafana хяналтын самбар нь Prometheus хэмжигдэхүүнийг уншдаг ба операторууд `ops/runbooks/settlement-buffers.md`-ийг `ops/runbooks/settlement-buffers.md`-ийг I1000-ын хажууд ашигладаг. цэнэглэх/тохируулах үйл явдлууд.

## Телеметр ба нотлох баримт
- `iroha_settlement_buffer_xor`, `iroha_settlement_buffer_capacity_xor`, `iroha_settlement_buffer_status` — эгнээ/өгөгдлийн орон зай тус бүрийн буфер агшин агшин (микро-XOR + кодлогдсон төлөв).【crates/iroha_telemetry/src/metrics.rs:621】
- `iroha_settlement_pnl_xor` — блок багцын хугацаа болон үс тайралтын дараах XOR хоёрын хоорондох зөрүүг ойлгосон.【crates/iroha_telemetry/src/metrics.rs:6236】
- `iroha_settlement_haircut_bp` — багцад хэрэглэсэн үр дүнтэй epsilon/үс засах үндсэн оноо.【crates/iroha_telemetry/src/metrics.rs:6244】
- `iroha_settlement_swapline_utilisation` — своп нотлох баримт байгаа үед хөрвөх чадварын профайлаар тохируулсан нэмэлт хэрэглээ.【crates/iroha_telemetry/src/metrics.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — тооцооны хөрвүүлэлт болон хуримтлагдсан үсний засалт (XOR нэгж).【crates/iroha_telemetry/src/metrics.rs:6260】【crates/iroha:6260】【crates/iroha】rc.3】cores/irob - нэг эгнээ/өгөгдлийн орон зайн тоолуур.
- Grafana самбар: `dashboards/grafana/settlement_router_overview.json` (буферийн зай, зөрүү, үс засах) дээр нь Nexus эгнээний дохиоллын багцад суулгасан Alertmanager дүрэм.
- Операторын runbook: `ops/runbooks/settlement-buffers.md` (дахин дүүргэх/сэрэмжлүүлэх ажлын урсгал) болон `docs/source/nexus_settlement_faq.md` дээрх түгээмэл асуултууд.## Хөгжүүлэгч ба SRE шалгах хуудас
- `[settlement.router]` утгыг `config/config.json5` (эсвэл TOML) дээр тохируулж, `irohad --version` бүртгэлээр баталгаажуулах; босго `alert > throttle > xor_only > halt`-д нийцэж байгаа эсэхийг баталгаажуулах.
- Замын мета өгөгдлийг буфер данс/хөрөнгө/чадавхиар дүүргэх, ингэснээр буфер хэмжигч нь бодит нөөцийг тусгах; буферийг хянах ёсгүй эгнээний талбаруудыг орхи.
- `settlement_router_*` болон `iroha_settlement_*` хэмжигдэхүүнийг `dashboards/grafana/settlement_router_overview.json`-ээр дамжуулан хянах; тохируулагч/зөвхөн XOR/зогсоох төлөвийн дохио.
- `crates/iroha_core/src/block.rs` дээрх үнэ/бодлогын хамрах хүрээ болон одоо байгаа блок түвшний нэгтгэх тестийн хувьд `cargo test -p settlement_router`-г ажиллуул.
- `docs/source/nexus_fee_model.md`-д тохиргооны өөрчлөлтөд зориулсан засаглалын зөвшөөрлийг бүртгэж, босго эсвэл телеметрийн гадаргуу өөрчлөгдөх үед `status.md`-г шинэчлээрэй.

## Дамжуулах төлөвлөгөөний агшин зураг
- Барилга бүрт чиглүүлэгч + телеметрийн хөлөг онгоц; онцлог хаалга байхгүй. Замын мета өгөгдөл нь буфер агшин зуурын зургийг нийтлэх эсэхийг хянадаг.
- Өгөгдмөл тохиргоо нь замын зураглалын утгуудтай таарч байна (60s TWAP, 25bp үндсэн epsilon, 72h bufer horizon); тохиргоогоор дамжуулан тааруулж, хэрэглэхийн тулд `irohad`-г дахин эхлүүлнэ үү.
- Нотлох баримтын багц = эгнээний тооцооны амлалт + `settlement_router_*`/`iroha_settlement_*` цувралын Prometheus хусах + нөлөөлөлд өртсөн цонхны Grafana дэлгэцийн агшин/JSON экспорт.

## Нотлох баримт ба лавлагаа
- NX-3 тооцооны чиглүүлэгчийн хүлээн авах тэмдэглэл: `status.md` (NX-3 хэсэг).
- Операторын гадаргуу: `dashboards/grafana/settlement_router_overview.json`, `ops/runbooks/settlement-buffers.md`.
- Хүлээн авах схем ба API гадаргуу: `docs/source/nexus_fee_model.md`, `/v1/sumeragi/status` -> `lane_settlement_commitments`.