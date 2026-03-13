---
lang: ba
direction: ltr
source: docs/settlement-router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 782429c90ac5df034fd7c8ff2c3acf4f9f11348f14f15fcd321f343b22b154b8
source_last_modified: "2025-12-29T18:16:35.914434+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Детерминистик ҡасаба маршрутизаторы (NX-3)

**Статус:** Төҙөлгән (NX-3)  
**Хужалар:** Иҡтисад WG / Ядро баш кейеме WG / ҡаҙна / SRE  
**Кәптәс:** Канонлы XOR ҡасаба юлы ҡулланылған бөтә һыҙаттар/мәғлүмәттәр. Таш ебәрелгән маршрутизатор йәшник, һыҙат кимәлендә квитанциялар, буфер һаҡсы рельстар, телеметрия, һәм оператор дәлилдәре өҫтө.

## Маҡсаттар
- XOR конверсия һәм квитанция генерацияһы бер һыҙатлы һәм Nexus төҙөй.
- Ҡулланыу детерминистик стрижка + волатильность сиктәре менән һаҡсы-тимергән буферҙар шулай операторҙар темпта ҡасаба хәүефһеҙ.
- Квитанциялар, телеметрия һәм приборҙар таҡталары, тип аудиторҙар ҡабаттан уйнай ала, инструменталь инструменталь.

## Архитектура
| Компонент | Урыны | Яуаплылыҡ |
|---------|-----------|---------------|
| Маршрут примитивтары | `crates/settlement_router/` | Күләгә-хаҡ калькулятор, сәс ҡырҡыу ярустары, буфер сәйәсәте ярҙамсылары, ҡасаба квитанцияһы тип.【күрһәтеү_маршрут/срк/срк.р. rs:1】【справка_маршрут/срк/срк/срк.р. р.
| Йүгерергә фасад | `crates/iroha_core/src/settlement/mod.rs:1` | Rraps маршрутизатор конфиг `SettlementEngine`, `quote` + аккумуляторы блокировкалау ваҡытында ҡулланыла. |
| Блок интеграцияһы | `crates/iroha_core/src/block.rs:120` | Дренаждар `PendingSettlement` рекордтары, агрегаттар `LaneSettlementCommitment` бер һыҙат/мәғлүмәттәр, парзалар һыҙат буфер метамағлүмәттәре, һәм телеметрия сығара. |
| Телеметрия & приборҙар таҡталары | `crates/iroha_telemetry/src/metrics.rs:4847`, `dashboards/grafana/settlement_router_overview.json:1` X | Prometheus өсөн буферҙар, дисперсия, сәс ҡырҡыу, конверсия һандары; Grafana өсөн плата СРЭ. |
| Һылтанма схемаһы | `docs/source/nexus_fee_model.md:1` | Документтар ҡасабаһы квитанцияһы ятҡылыҡтары `LaneBlockCommitment`-та һаҡланған. |

## Конфигурация
Маршрут руттары `[settlement.router]` аҫтында йәшәй (Prometheus тарафынан раҫланған):

```toml
[settlement.router]
twap_window_seconds = 60      # TWAP window used to derive local→XOR conversions
epsilon_bps = 25              # Base margin added to every quote (basis points)
buffer_alert_pct = 75         # Remaining-buffer % that opens an alert
buffer_throttle_pct = 25      # Remaining-buffer % where throttling begins
buffer_xor_only_pct = 10      # Remaining-buffer % where XOR-only mode is enforced
buffer_halt_pct = 2           # Remaining-buffer % where settlement halts
buffer_horizon_hours = 72     # Horizon (hours) represented by the XOR buffer
``` X

Һылтанма метамағлүмәт сымдары пер-мәғлүмәттәр буфер иҫәбенә:
- `settlement.buffer_account` — резервты тотҡан иҫәп (мәҫәлән, `buffer::cbdc_treasury`).
- `settlement.buffer_asset` — баш бүлмәһе өсөн дебетланған активтар билдәләмәһе (ғәҙәттә `xor#sora`X).
- `settlement.buffer_capacity_micro` — микро-XOR-ҙа конфигурацияланған ҡәҙерле (ун унарлы еп).

Ҡайһы бер осраҡта был һыҙат өсөн буфер өҙөлә (телеметрия нуль ҡөҙрәткә/статусҡа тиклем төшә).## Конверсия торбаһы
1. **Циталы:** `SettlementEngine::quote` конфигурацияланған эпсилон + волатильность маржаһы һәм стрижка ярус TWAP цитаталары, ҡайтарыу `SettlementReceipt` менән `xor_due` һәм `xor_after_haircut` плюс ваҡыт тамғаһы һәм шылтыратыусы-плюс `source_id`.【крос/съезд_маршрут/срк/хаҡ.rs. 1】【кәзит/съезд_маршрут/src/scrut.rct.rs:1】 .
2. **Блокада: блок башҡарыу ваҡытында башҡарыусы яҙмалары `PendingSettlement` яҙмалары (урындағы сумма, TWAP, эпсилон, волатильность биҙрә, ликвидлыҡ профиле, оракул ваҡыт тамғаһы). `LaneSettlementBuilder` агрегаттары дөйөм һәм алмаштырыу метамағлүмәттәр Grafana блокты герметизациялау алдынан.【крат/srha_core/src/scr/mod.s:34【крет/ироха_ядро/src/src.sc.:3460】
**Буфер снимок:** Әгәр ҙә һыҙат метамағлүмәттәре буфер иғлан итһә, төҙөүсе `SettlementBufferSnapshot` ( баш бүлмәһе, ҡәҙерле, статусы) `BufferPolicy` сиктәрен конфиглауҙан тотоп ала.
4. **Ҡоллоҡ + телеметрия:** Квитанциялар һәм своп дәлилдәре ер эсендә `LaneBlockCommitment` һәм көҙгө статус снимоктары. Телеметрия яҙмалары буфер датчиктар, дисперсия (`iroha_settlement_pnl_xor`), ҡулланылған маржа (`iroha_settlement_haircut_bp`), опциональ алмаштырыу утилизацияһы, һәм бер активтарҙы үҙгәртеп ҡороу/сүп-сар өсөн счетчиктар шулай приборҙар таҡтаһы һәм иҫкәртмәләр синхронла Йөкмәткеһе.【крат/ироха_ядро/срк/блок.р.:298】【крат/ироха_ядро/срк/телеметрия.р.:844】
5. **Дәлилдәр өҫтө:** `status::set_lane_settlement_commitments` реле/ДА ҡулланыусылар өсөн йөкләмәләрҙе баҫтырып сығара, Grafana приборҙар таҡталары Prometheus метрикаһын уҡый, ә операторҙар `ops/runbooks/settlement-buffers.md``dashboards/grafana/settlement_router_overview.json` менән бергә ҡуллана. тултырыу/дроссель ваҡиғалар.

## Телеметрия һәм дәлилдәр
- `iroha_settlement_buffer_xor`, `iroha_settlement_buffer_capacity_xor`, `iroha_settlement_buffer_status` — буфер снимок бер һыҙат/мәғлүмәттәр (микро-XOR + кодланған хәл).【крет/ироха_телеметрия/scr/scr/scr.
- `iroha_settlement_pnl_xor` — аңлашылған дисперсия араһында vs vs пост-суск XOR өсөн блок партияһы.【крат/ироха_телеметрия/src/srcs.rs.rs:6236】
- `iroha_settlement_haircut_bp` — һөҙөмтәле эпсилон/стрижка базис пункттары партияға ҡулланыла.【крат/ироха_телеметрия/src/src/src.rs.rs:6244】
- `iroha_settlement_swapline_utilisation` — ликвидацион утилләштереү биҙрә ликвидлыҡ профиле ҡасан своп дәлилдәр бар.【крат/ироха_телеметрия/src/src.rs.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — ҡасаба конверсиялары һәм кумулятив стрижкалар өсөн бер һыҙат/мәғлүмәттәр счетчиктары (XOR Берәмектәр).【крат/ироха_телеметрия/срк/метрия.р.р. 6260】【крат/ироха_ядро/срк/блок.rs:304】
- Grafana платаһы: `dashboards/grafana/settlement_router_overview.json` (буфер баш, дисперсияһы, сәс ҡырҡыу) плюс Alertmanager ҡағиҙәләре Nexus һыҙаттары тураһында иҫкәртмә пакетында индерелгән.
- Оператор runbook: `ops/runbooks/settlement-buffers.md` (баҫтырыу/иҫкәртмә эш ағымы) һәм FAQ `docs/source/nexus_settlement_faq.md`.## Төҙөүсе & SRE тикшерелгән исемлек
- `[settlement.router]` ҡиммәттәре `config/config.json5` (йәки TOML) һәм `irohad --version` журналдары аша раҫлау; тәьмин итеү сиктәрен ҡәнәғәтләндерергә `alert > throttle > xor_only > halt`.
- Буфер иҫәбенә/актив/ҡоролма менән һыҙат метамағлүмәттәре менән бергә шулай буфер датчиктар йәшәү запастарын сағылдыра; буферҙарҙы күҙәтергә тейешле һыҙаттар өсөн баҫыуҙарҙы үткәрмәй.
- `settlement_router_*` һәм `dashboards/grafana/settlement_router_overview.json` аша `iroha_settlement_*` метрикаһы мониторы; уяу дроссель/XOR-тик/туҡталыш хәлдәр.
- Run `cargo test -p settlement_router` өсөн хаҡтар/сәйәсәт ҡаплау һәм ғәмәлдәге блок кимәлендәге агрегация һынауҙары `crates/iroha_core/src/block.rs`.
- `docs/source/nexus_fee_model.md`-та конфиг үҙгәрештәре өсөн идара итеүҙе раҫлау һәм `status.md`X сиктәре йәки телеметрия өҫтө үҙгәргәндә яңыртылғанын һаҡлағыҙ.

## рулет планы Снэпшот
- Маршрут + һәр төҙөүҙә телеметрия карапы; ҡапҡалар юҡ. Лейн метамағлүмәттәре буфер снимоктар баҫтырып сығарыумы-юҡмы икәнлеген контролдә тота.
- Ғәҙәттәгесә конфигурациялау юл картаһы ҡиммәттәренә тап килә (60-сы TWAP, 25-се база эпсилон, 72 сәғәт буфер горизонты); көй аша конфиг һәм перезапуск `irohad` ғариза бирергә.
- Дәлилдәр өйөмө = һыҙаттар иҫәп-хисап йөкләмәләре + Prometheus скреб өсөн Nexus/`iroha_settlement_*` серияһы + Grafana скриншот/JSON экспорты өсөн зыян күргән тәҙрә.

## Дәлилдәр & Һылтанмалар
- NX-3 ҡасаба маршрутизаторы ҡабул итеү иҫкәрмәләре: `status.md`X (NX-3 бүлеге).
- Оператор өҫтө: `dashboards/grafana/settlement_router_overview.json`, `ops/runbooks/settlement-buffers.md`.
- Квитанция схемаһы һәм API өҫтө: `docs/source/nexus_fee_model.md`, `/v2/sumeragi/status` -> `lane_settlement_commitments`.