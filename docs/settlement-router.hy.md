---
lang: hy
direction: ltr
source: docs/settlement-router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 782429c90ac5df034fd7c8ff2c3acf4f9f11348f14f15fcd321f343b22b154b8
source_last_modified: "2025-12-29T18:16:35.914434+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Որոշիչ հաշվարկային երթուղիչ (NX-3)

** Կարգավիճակ. ** Ավարտված (NX-3)  
**Սեփականատերեր.** Տնտեսական WG / Core Ledger WG / Գանձապետարան / SRE  
**Շրջանակ.** Կանոնական XOR կարգավորման ճանապարհ, որն օգտագործվում է բոլոր գոտիների/տվյալների տարածքների կողմից: Առաքված երթուղիչի տուփ, գծի մակարդակի անդորրագրեր, բուֆերային պաշտպանիչ ռելսեր, հեռաչափություն և օպերատորի ապացույցների մակերեսներ:

## Գոլեր
- Միավորել XOR-ի փոխակերպումը և ստացականների ստեղծումը մեկ երթևեկելի և Nexus շենքերում:
- Կիրառեք դետերմինիստական ​​սանրվածքներ + անկայունության սահմաններ պաշտպանիչ ռելսերով բուֆերներով, որպեսզի օպերատորները կարողանան ապահով կերպով կարգավորել կարգավորումները:
- Բացահայտեք անդորրագրերը, հեռաչափությունը և վահանակները, որոնք աուդիտորները կարող են վերարտադրել առանց պատվիրված գործիքների:

## Ճարտարապետություն
| Բաղադրիչ | Գտնվելու վայրը | Պատասխանատվություն |
|-----------|----------|----------------|
| Ուղղորդիչի պրիմիտիվներ | `crates/settlement_router/` | Ստվերային գնի հաշվիչ, սանրվածքի մակարդակներ, բուֆերային քաղաքականության օգնականներ, հաշվարկների անդորրագրի տեսակը։【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】【crates/settlement_router/rsc
| Runtime ճակատային | `crates/iroha_core/src/settlement/mod.rs:1` | Փաթաթում է երթուղիչի կազմաձևը `SettlementEngine`-ի մեջ, ցուցադրում է `quote` + կուտակիչը, որն օգտագործվում է բլոկի կատարման ժամանակ: |
| Բլոկի ինտեգրում | `crates/iroha_core/src/block.rs:120` | Քամում է `PendingSettlement` գրառումները, ագրեգատում `LaneSettlementCommitment` յուրաքանչյուր գծի/տվյալների տարածության համար, վերլուծում է գծի բուֆերային մետատվյալները և արձակում հեռաչափություն: |
| Հեռաչափություն և վահանակներ | `crates/iroha_telemetry/src/metrics.rs:4847`, `dashboards/grafana/settlement_router_overview.json:1` | Prometheus/OTLP չափումներ բուֆերների, շեղումների, սանրվածքների, փոխակերպումների քանակի համար; Grafana տախտակ SRE-ի համար: |
| Հղման սխեման | `docs/source/nexus_fee_model.md:1` | Փաստաթղթերի հաշվարկման անդորրագրերի դաշտերը պահպանվել են `LaneBlockCommitment`-ում: |

## Կազմաձևում
Ուղղորդիչի բռնակները աշխատում են `[settlement.router]`-ի տակ (վավերացված է `iroha_config`-ի կողմից).

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

Գոտի մետատվյալների լարերը յուրաքանչյուր տվյալների տարածության բուֆերային հաշվում.
- `settlement.buffer_account` - հաշիվ, որը պահում է պահուստը (օրինակ՝ `buffer::cbdc_treasury`):
- `settlement.buffer_asset` - ակտիվի սահմանումը դեբետագրված գլխի համար (սովորաբար `xor#sora`):
- `settlement.buffer_capacity_micro` — կոնֆիգուրացված հզորություն միկրո-XOR-ում (տասնորդական տող):

Բացակայող մետատվյալներն անջատում են բուֆերային նկարահանումը այդ գոտու համար (հեռաչափությունը հետ է ընկնում զրոյական հզորության/կարգավիճակին):## Փոխակերպման խողովակաշար
1. **Մեջբերում.** `SettlementEngine::quote`-ը կիրառում է կազմաձևված էպսիլոն + անկայունության մարժան և սանրվածքի մակարդակը TWAP չակերտների վրա՝ վերադարձնելով `SettlementReceipt`՝ `xor_due`-ով և `xor_due`-ով և Prometheus-ով և Prometheus-ով: `source_id`.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】
2. **Կուտակել.** Բլոկի կատարման ընթացքում կատարողը գրանցում է `PendingSettlement` գրառումներ (տեղական գումար, TWAP, էպսիլոն, անկայունության դույլ, իրացվելիության պրոֆիլ, oracle-ի ժամանակի դրոշմ): `LaneSettlementBuilder`-ը միավորում է հանրագումարները և փոխանակում մետատվյալները մեկ `(lane, dataspace)`-ով մինչև բլոկը կնքելը:【crates/iroha_core/src/settlement/mod.rs:34】【crates/iroha_core/src/block.rs:
3. **Բուֆերային նկար.** Եթե երթուղու մետատվյալները հայտարարում են բուֆեր, ապա շինարարը ֆիքսում է `SettlementBufferSnapshot` (մնացած գլխամաս, հզորություն, կարգավիճակ)՝ օգտագործելով `BufferPolicy` շեմերը՝ կազմաձևից.【crates/rcates:2/0rs:
4. **Պարտավորել + հեռաչափություն.** Անդորրագրերը և փոխանակման ապացույցները հայտնվում են `LaneBlockCommitment`-ի ներսում և արտացոլվում են կարգավիճակի նկարներում: Հեռուստաչափությունը գրանցում է բուֆերաչափերը, շեղումները (`iroha_settlement_pnl_xor`), կիրառվող լուսանցքը (`iroha_settlement_haircut_bp`), ընտրովի փոխարինման գծի օգտագործումը և յուրաքանչյուր ակտիվի փոխակերպման/սանրվածքի հաշվիչները, որպեսզի վահանակներն ու ազդանշանները մնան համաժամանակյա բլոկի հետ: բովանդակություն։【crates/iroha_core/src/block.rs:298】【crates/iroha_core/src/telemetry.rs:844】
5. **Ապացույցների մակերեսները.

## Հեռաչափություն և ապացույցներ
- `iroha_settlement_buffer_xor`, `iroha_settlement_buffer_capacity_xor`, `iroha_settlement_buffer_status` — բուֆերային նկար յուրաքանչյուր գծի/տվյալների տարածության համար (micro-XOR + կոդավորված վիճակ):【crates/iroha_telemetry/src/metrics.rs:2
- `iroha_settlement_pnl_xor` – բացահայտված տարբերություն՝ XOR-ի և հետսանրվածքի XOR-ի միջև բլոկային խմբաքանակի համար:【crates/iroha_telemetry/src/metrics.rs:6236】
- `iroha_settlement_haircut_bp` — արդյունավետ էպսիլոն/սանրվածքի բազային միավորներ, որոնք կիրառվում են խմբաքանակի վրա:【crates/iroha_telemetry/src/metrics.rs:6244】
- `iroha_settlement_swapline_utilisation` — կամընտիր կիրառում, որը կախված է իրացվելիության պրոֆիլից, երբ առկա են փոխանակման ապացույցներ:【crates/iroha_telemetry/src/metrics.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — յուրաքանչյուր գծի/տվյալների տարածության հաշվիչներ հաշվարկային փոխարկումների և կուտակային սանրվածքների համար (XOR միավորներ):
- Grafana տախտակ. `dashboards/grafana/settlement_router_overview.json` (բուֆերային տարածք, դիսպերսիա, սանրվածք) գումարած Alertmanager կանոնները, որոնք ներդրված են Nexus գծի ազդանշանային փաթեթում:
- Օպերատորի մատյան. `ops/runbooks/settlement-buffers.md` (լիցքավորում/զգոն աշխատանքային հոսք) և ՀՏՀ `docs/source/nexus_settlement_faq.md`-ում:## Developer & SRE Checklist
- Սահմանեք `[settlement.router]` արժեքները `config/config.json5`-ում (կամ TOML) և վավերացրեք `irohad --version` տեղեկամատյանների միջոցով; ապահովել, որ շեմերը բավարարում են `alert > throttle > xor_only > halt`-ին:
- Լրացրեք երթուղու մետատվյալները բուֆերային հաշվի/ակտիվների/հզորության հետ, որպեսզի բուֆերաչափերը արտացոլեն կենդանի պահուստները. բաց թողեք այն գոտիների դաշտերը, որոնք չպետք է հետևեն բուֆերներին:
- Դիտեք `settlement_router_*` և `iroha_settlement_*` չափումները `dashboards/grafana/settlement_router_overview.json`-ի միջոցով; զգոն շնչափող/միայն XOR/ կանգառ վիճակների մասին:
- Գործարկեք `cargo test -p settlement_router`՝ գնագոյացման/քաղաքականության ծածկույթի և բլոկի մակարդակի առկա թեստերի համար `crates/iroha_core/src/block.rs`-ում:
- Գրանցեք կառավարման հաստատումները `docs/source/nexus_fee_model.md`-ում կազմաձևման փոփոխությունների համար և թարմացրե՛ք `status.md`-ը, երբ շեմերը կամ հեռաչափական մակերեսները փոխվում են:

## Տարածման պլանի պատկեր
- Երթուղիչ + հեռաչափական նավ յուրաքանչյուր կառուցման մեջ; ոչ մի առանձնահատկություն դարպասներ: Գոտի մետատվյալները վերահսկում են, թե արդյոք բուֆերային նկարների հրապարակումը:
- Կանխադրված կազմաձևը համապատասխանում է ճանապարհային քարտեզի արժեքներին (60-ականների TWAP, 25 bp բազային էպսիլոն, 72 ժամ բուֆերային հորիզոն); կարգավորեք կոնֆիգուրացիայի միջոցով և վերագործարկեք `irohad`՝ կիրառելու համար:
- Ապացույցների փաթեթ = գծի կարգավորման պարտավորություններ + Prometheus քերծվածք `settlement_router_*`/`iroha_settlement_*` սերիայի համար + Grafana սքրինշոթ/JSON արտահանում տուժած պատուհանի համար:

## Ապացույցներ և հղումներ
- NX-3 հաշվարկային երթուղիչի ընդունման նշումներ. `status.md` (NX-3 բաժին):
- Օպերատորի մակերեսները՝ `dashboards/grafana/settlement_router_overview.json`, `ops/runbooks/settlement-buffers.md`:
- Ստացական սխեման և API-ի մակերեսները՝ `docs/source/nexus_fee_model.md`, `/v2/sumeragi/status` -> `lane_settlement_commitments`: