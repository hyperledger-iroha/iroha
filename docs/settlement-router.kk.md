---
lang: kk
direction: ltr
source: docs/settlement-router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 782429c90ac5df034fd7c8ff2c3acf4f9f11348f14f15fcd321f343b22b154b8
source_last_modified: "2025-12-29T18:16:35.914434+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Детерминистік есеп айырысу маршрутизаторы (NX-3)

**Күйі:** Аяқталды (NX-3)  
**Меншік иелері:** Экономика ЖТ / Негізгі кітап ЖТ / Қазынашылық / SRE  
**Қолдану аймағы:** Барлық жолақтар/деректер кеңістігі пайдаланатын канондық XOR есеп айырысу жолы. Маршрутизатордың жөнелтілген жәшігі, жолақ деңгейіндегі түбіртектері, буферлік қоршаулар, телеметрия және оператордың куәлік беттері.

## Мақсаттар
- Бір жолақты және Nexus құрылымдары бойынша XOR түрлендіру мен түбіртек генерациясын біріктіріңіз.
- Операторлар есеп айырысуды қауіпсіз түрде жылдамдата алуы үшін детерминирленген шаш қиюларын + құбылмалылық шегін қорғаныс рельстері бар буферлерді қолданыңыз.
- Аудиторлар арнайы құралдарсыз қайталай алатын түбіртектерді, телеметрияны және бақылау тақталарын көрсетіңіз.

## Архитектура
| Құрамдас | Орналасқан жері | Жауапкершілік |
|----------|----------|----------------|
| Маршрутизатордың примитивтері | `crates/settlement_router/` | Көлеңкелі баға калькуляторы, шаш қию деңгейлері, буферлік саясат көмекшілері, есеп айырысу түбіртегінің түрі.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】【crates/settlement_router/rs.1】rc.
| Runtime қасбеті | `crates/iroha_core/src/settlement/mod.rs:1` | Маршрутизатор конфигурациясын `SettlementEngine` ішіне орап, блокты орындау кезінде пайдаланылатын `quote` + аккумуляторды көрсетеді. |
| Блокты біріктіру | `crates/iroha_core/src/block.rs:120` | `PendingSettlement` жазбаларын төгеді, `LaneSettlementCommitment` әр жолға/деректер кеңістігіне біріктіреді, жолақ буферінің метадеректерін талдайды және телеметрияны шығарады. |
| Телеметрия және бақылау тақталары | `crates/iroha_telemetry/src/metrics.rs:4847`, `dashboards/grafana/settlement_router_overview.json:1` | Prometheus/OTLP өлшемдері буферлерге, дисперсияға, шаш қиюларына, түрлендіру сандарына; SRE үшін Grafana тақтасы. |
| Анықтамалық схема | `docs/source/nexus_fee_model.md:1` | Құжаттардың есеп айырысу түбіртегі өрістері `LaneBlockCommitment` ішінде сақталады. |

## Конфигурация
Маршрутизатор тұтқалары `[settlement.router]` астында жұмыс істейді (`iroha_config` арқылы расталған):

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

Деректер кеңістігінің буферінің тіркелгісіндегі жолақты метадеректер сымдары:
- `settlement.buffer_account` — резервті ұстайтын шот (мысалы, `buffer::cbdc_treasury`).
- `settlement.buffer_asset` — актив анықтамасы бос орын үшін дебеттеледі (әдетте `xor#sora`).
- `settlement.buffer_capacity_micro` — micro-XOR (ондық жол) ішінде конфигурацияланған сыйымдылық.

Метадеректер жоқ болса, сол жолақ үшін буферлік суретке түсіруді өшіреді (телеметрия нөлдік сыйымдылыққа/күйге қайтады).## Түрлендіру құбыры
1. **Дәйексөз:** `SettlementEngine::quote` конфигурацияланған epsilon + құбылмалылық шегін және шаш қию деңгейін TWAP тырнақшаларына қолданады, `SettlementReceipt` және `xor_due` және `xor_after_haircut` қоңырау шалушы және уақытты күшейту арқылы қайтарады. `source_id`.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】
2. **Жинақтау:** Блокты орындау кезінде орындаушы `PendingSettlement` жазбаларын (жергілікті сома, TWAP, эпсилон, құбылмалылық шелегі, өтімділік профилі, oracle уақыт белгісі) жазады. `LaneSettlementBuilder` жиынтықтарды біріктіреді және блокты жаппас бұрын `(lane, dataspace)` үшін метадеректерді ауыстырады.【crates/iroha_core/src/settlement/mod.rs:34】【crates/iroha_core/src/block.rs:3460】
3. **Буфердің суреті:** Жолдық метадеректер буферді жарияласа, құрастырушы конфигурациядан `BufferPolicy` шектерін пайдаланып `SettlementBufferSnapshot` (қалған бос орын, сыйымдылық, күй) алады.【crates/iroha_3.
4. **Тапсырыс + телеметрия:** Түбіртектер мен айырбас дәлелдері `LaneBlockCommitment` ішінде орналасады және күй суретіне айналады. Телеметрия буфер көрсеткіштерін, дисперсияны (`iroha_settlement_pnl_xor`), қолданылатын маржаны (`iroha_settlement_haircut_bp`), қосымша своплайнды пайдалануды және әр активке түрлендіру/шаш қию есептегіштерін жазады, осылайша бақылау тақталары мен ескертулер блокпен синхрондалады. мазмұны.【crates/iroha_core/src/block.rs:298】【crates/iroha_core/src/telemetry.rs:844】
5. **Дәлелдер беті:** `status::set_lane_settlement_commitments` реле/DA тұтынушылары үшін міндеттемелерді жариялайды, Grafana бақылау тақталары Prometheus көрсеткіштерін оқиды, ал операторлар `ops/runbooks/settlement-buffers.md`-ті I1000-мен қатар I104-ге дейін пайдаланады. қайта толтыру/дроссель оқиғалары.

## Телеметрия және дәлелдер
- `iroha_settlement_buffer_xor`, `iroha_settlement_buffer_capacity_xor`, `iroha_settlement_buffer_status` — әр жол/деректер кеңістігі үшін буфер суреті (micro-XOR + кодталған күй).【crates/iroha_telemetry/src/metrics.rs:621】
- `iroha_settlement_pnl_xor` — блоктық топтама үшін мерзімі мен шаш қиюдан кейінгі XOR арасындағы айырмашылық анықталды.【crates/iroha_telemetry/src/metrics.rs:6236】
- `iroha_settlement_haircut_bp` — партияға қолданылатын тиімді эпсилон/шаш қиюының негізгі нүктелері.【crates/iroha_telemetry/src/metrics.rs:6244】
- `iroha_settlement_swapline_utilisation` — своп дәлелі болған кезде өтімділік профилі бойынша қосымша пайдалану.【crates/iroha_telemetry/src/metrics.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — есеп айырысуды түрлендіруге және жинақталған шаш қиюға арналған жолақтарға/деректер кеңістігіне есептегіштер (XOR бірліктері).【crates/iroha_telemetry/src/metrics.rs:6260】【crates/iroha:4.cores/irob
- Grafana тақтасы: `dashboards/grafana/settlement_router_overview.json` (буфердегі бос орын, ауытқулар, шаш қиюлары) плюс Nexus жолақ туралы ескерту жинағына енгізілген Alertmanager ережелері.
- Operator runbook: `ops/runbooks/settlement-buffers.md` (қайта толтыру/ескерту жұмыс процесі) және `docs/source/nexus_settlement_faq.md` ішіндегі жиі қойылатын сұрақтар.## Әзірлеуші және SRE бақылау тізімі
- `[settlement.router]` мәндерін `config/config.json5` (немесе TOML) ішінде орнатыңыз және `irohad --version` журналдары арқылы растаңыз; шектердің `alert > throttle > xor_only > halt` сәйкес келетініне көз жеткізіңіз.
- Буферлік есептегіштермен/активпен/сыйымдылықпен жолақ метадеректерін толтырыңыз, осылайша буфер өлшегіштері тірі резервтерді көрсетеді; буферлерді қадағаламайтын жолақтар үшін өрістерді өткізіп жіберіңіз.
- `settlement_router_*` және `iroha_settlement_*` көрсеткіштерін `dashboards/grafana/settlement_router_overview.json` арқылы бақылаңыз; дроссель/тек XOR/тоқтату күйлері туралы ескерту.
- `crates/iroha_core/src/block.rs` ішіндегі баға/саясат қамту және блок-деңгейдегі бар жинақтау сынақтары үшін `cargo test -p settlement_router` іске қосыңыз.
- `docs/source/nexus_fee_model.md` ішіндегі конфигурация өзгерістері үшін басқару мақұлдауларын жазып алыңыз және шекті мәндер немесе телеметрия беттері өзгерген кезде `status.md` жаңартылып отырыңыз.

## Шығарылым жоспарының суреті
- Әрбір құрылыста маршрутизатор + телеметриялық кеме; мүмкіндік қақпалары жоқ. Жолақ метадеректері буфер суреттерінің жариялануын басқарады.
- Әдепкі конфигурация жол картасы мәндеріне сәйкес келеді (60s TWAP, 25bp базалық эпсилон, 72сағ буфер көкжиегі); конфигурация арқылы реттеңіз және қолдану үшін `irohad` қайта іске қосыңыз.
- Дәлелдер жинағы = жолақ бойынша есеп айырысу міндеттемелері + `settlement_router_*`/`iroha_settlement_*` сериялары үшін Prometheus сызат + зардап шеккен терезе үшін Grafana скриншоты/JSON экспорты.

## Дәлелдер мен анықтамалар
- NX-3 есеп айырысу маршрутизаторын қабылдау ескертулері: `status.md` (NX-3 бөлімі).
- Оператор беттері: `dashboards/grafana/settlement_router_overview.json`, `ops/runbooks/settlement-buffers.md`.
- Түбіртек схемасы және API беттері: `docs/source/nexus_fee_model.md`, `/v1/sumeragi/status` -> `lane_settlement_commitments`.