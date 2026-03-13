---
lang: dz
direction: ltr
source: docs/settlement-router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 782429c90ac5df034fd7c8ff2c3acf4f9f11348f14f15fcd321f343b22b154b8
source_last_modified: "2025-12-29T18:16:35.914434+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# གཏན་འཇགས་གཞི་ཆགས་ལམ་སྟོན་ (NX-3)

**གནས་སྟངས་:** མཇུག་བསྡུ་ཡོད་པའི་ (NX-3)  
**ཇོ་བདག་:** དཔལ་འབྱོར་ WG / Core Ledger WG / དངུལ་ཁང་ / SRE  
**Scope:** ལམ་ཐིག་/གནད་སྡུད་ས་སྟོང་ཆ་མཉམ་གྱིས་ལག་ལེན་འཐབ་ཡོད་པའི་ ཀེ་ནོ་ནིག་ཨེགསི་ཨོ་ཨར་ གཞིས་ཆགས་འགྲུལ་ལམ་། འགྲུལ་བཞུད་ཀྱི་ཀེརེཊ་དང་ ལམ་རིམ་གྱི་འོང་འབབ་ བཱ་ཕར་སྲུང་སྐྱོབ་པ་ ཊེ་ལི་མི་ཊི་རི་ དེ་ལས་ བཀོལ་སྤྱོད་པའི་སྒྲུབ་བྱེད་ཚུ་ ཁ་ཐོག་ལུ་ བཏང་ཡོདཔ་ཨིན།

## རིལ་ཚང
- ཨེགསི་ཨོ་ཨར་ གཞི་བསྒྱུར་དང་ འཐོབ་ཐངས་ཚུ་ ལམ་ལུགས་གཅིག་ནང་ མཉམ་བསྡོམ་འབད་དེ་ Nexus བཟོ་བསྐྲུན་འབདཝ་ཨིན།
- གཏན་འབེབས་བཟོ་ནིའི་སྐྲ་གཅོད་ནི་དང་ འགྱུར་ལྡོག་ཅན་གྱི་མཐའ་མཚམས་ཚུ་ ལྟ་རྟོག་པ་ཚུ་གིས་ ཉེན་སྲུང་དང་ལྡནམ་སྦེ་ གཞིས་ཆགས་ཚུགས།
- འགྲེམ་སྟོན་འབད་མི་དང་ བརྒྱུད་འཕྲིན་ དེ་ལས་ རྩིས་ཞིབ་པ་ཚུ་གིས་ ལག་ཆས་མ་བཀོད་པར་ ལོག་སྟེ་རྩེད་ཚུགས་པའི་ ཌེཤ་བོརཌ་ཚུ།

## བཟོ༌བཀོད༌རིག༌པ
| ཆ་ཤས་ | ས་གནས་ | འགན་འཁྲི་ |
|---------------------------------------------|
| རའུ་ཊར་གྱི་ གནའ་དུས་ | `crates/settlement_router/` | གྱིབ་མ་གོང་ཚད་རྩིས་འཕྲུལ་ སྐྲ་བརྡུང་བའི་སྲིད་བྱུས་རོགས་སྐྱོར་པ་ གཞིས་ཆགས་ཐོབ་མཁན་ 【ctrats/esttlement_router/src/price.s:1】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】】། |
| གཡོག་བཀོལ། | `crates/iroha_core/src/settlement/mod.rs:1` | བརྡབ་གསིག་ཚུ་ `SettlementEngine` ནང་ལུ་ བཤུབ་སྟེ་ སྡེབ་ཚན་ལག་ལེན་འཐབ་པའི་སྐབས་ ལག་ལེན་འཐབ་མི་ `quote` + བསྡུ་གསོག་འབད་མི་འདི་ ཕྱིར་བཏོན་འབདཝ་ཨིན། |
| བཀག་ཆ་མཉམ་བསྡོམས་ | `crates/iroha_core/src/block.rs:120` | ཆུ་བཏོན་ཐངས་ `PendingSettlement` གིས་ ལམ་ཐིག་རེ་ལུ་ `LaneSettlementCommitment` རེ་ལུ་ ལམ་གྱི་བཱ་ཕར་མེ་ཊ་ཌེ་ཊ་ དབྱེ་དཔྱད་འབདཝ་ཨིནམ་དང་ ཊེ་ལི་མི་ཊི་འདི་ བཏོནམ་ཨིན། |
| ཊེ་ལི་མི་ཊི་རི་ དང་ བརྡ་དོན་བཀོད་སྒྲིག་ཚུ། | `crates/iroha_telemetry/src/metrics.rs:4847`, `dashboards/grafana/settlement_router_overview.json:1` | Prometheus/OTLP མེ་ཊིགསི་ བཱ་ཕར་དང་ ཁྱད་པར་ སྐྲ་གཅོད་ནི་ བསྒྱུར་བཅོས་གྲངས་འབོར།; Grafana འདི་ SRE གི་དོན་ལུ་ཨིན། |
| གཞི་བསྟུན་ལས་འཆར་ | `docs/source/nexus_fee_model.md:1` | ཡིག་ཆ་གཞིས་ཆགས་ཐོབ་པའི་ས་སྒོ་ཚུ་ `LaneBlockCommitment` ནང་ལུ་གནས་ཏེ་ཡོདཔ་ཨིན། |

## རིམ་སྒྲིག་འབད་ནི།
རའུ་ཊར་གྱི་མཛུབ་མོ་འདི་ `[settlement.router]` འོག་ལུ་སྡོད་དོ་ཡོདཔ་ཨིན།

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

གནད་སྡུད་རེ་རེའི་བཱ་ཕར་རྩིས་ཐོ་ནང་ ལེན་མེ་ཊ་ཌེ་ཊ་ གློག་ཐག་ཚུ།
- `settlement.buffer_account` — གསོག་འཇོག་འདི་འཛིན་པའི་རྩིས་ཁྲ་ (དཔེར་ན་ `buffer::cbdc_treasury`).
- `settlement.buffer_asset` — མགུ་ཏོ་ཁང་མིག་གི་དོན་ལུ་ རྒྱུ་དངོས་ངེས་ཚིག་ (སྤྱིར་བཏང་ `xor#sora`) ཨིན།
- `settlement.buffer_capacity_micro` — མའི་ཀོརོ་ཨེགསི་ཨོ་ཨར་ (བཅུ་ཚག་ཡིག་རྒྱུན་) ནང་ རིམ་སྒྲིག་འབད་ཡོད་པའི་ནུས་པ།

མེད་མི་མེ་ཊ་ཌེ་ཊ་གིས་ དེ་གི་དོན་ལུ་ བཱ་ཕར་བཤུད་ཅན་ཚུ་ ལྕོགས་ཅན་བཟོཝ་ཨིན་ (ཊེ་ལི་མི་ཊི་འདི་ ཤོང་ཚད་/གནས་སྟངས་ལུ་ ཀླད་ཀོར་འབད་ཡོད་པའི་ ཀླད་ཀོར་ལུ་ལྷོདཔ་ཨིན།)## བསྒྱུར་བཅོས།
1. **Quote:** `SettlementEngine::quote` རིམ་སྒྲིག་འབད་ཡོད་པའི་ epsilon + བཞུར་བའི་རིམ་པ་ TWAP དང་ TWAP གི་ཚིག་བརྗོད་ཚུ་ `xor_due` དང་ `xor_after_haircut` དང་ མཉམ་དུ་བརྡ་འཕྲིན་ཚུ་ དང་ མཉམ་དུ་བརྡ་འཕྲིན་ཚུ་ ལོག་བཏང་ཡོདཔ་ཨིན། `source_id`.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】
2. **Accumulate:** བཀག་ཆ་བཀོལ་སྤྱོད་ཀྱི་སྐབས་ ལག་ལེན་འཐབ་མཁན་གྱི་དྲན་ཐོ་ `PendingSettlement` ཐོ་བཀོད་ཚུ་ (ས་གནས་ཀྱི་དངུལ་བསྡོམས་ ཊི་ཌབ་ལུ་ཨེ་པི་ ཨིཔ་སི་ལོན་ འགྱུར་ལྡོག་ཅན་གྱི་བཱ་ཀེཊི་ རུ་པི་ཊི་ གསལ་སྡུད་ ཨོར་ཀལ་ཊའིམ་ཊེམ་)། `LaneSettlementBuilder` བཀག་ཆ་མ་ཕྱེ་བའི་ཧེ་མ་ `(lane, dataspace)` རེ་ལུ་ བསྡོམས་རྩིས་དང་ shaptade.【ཀརེ་ཊི་/ཨི་རོ་ཧ་_ཀོར་/སི་ཨར་སི/གཞིས་ཆགས་/མོཌ་:༣༤】།
3. **Buffer snapshot:** གལ་སྲིད་ ལམ་ཐིག་མེ་ཊ་ཌེ་ཊ་གིས་ བཱ་ཕར་ཅིག་གསལ་བསྒྲགས་འབད་བ་ཅིན་ བཟོ་བསྐྲུན་པ་གིས་ `SettlementBufferSnapshot` འཛིན་བཟུང་འབདཝ་ཨིན། (མགུ་ཏོ་ཁང་མིག་དང་ ལྕོགས་གྲུབ་གནས་རིམ་) རིམ་སྒྲིག་ལས་ Grafana ཐེབས་ཚད་ཚུ་ལག་ལེན་འཐབ་ཨིན། 【ཀརཊིསི/རི་ཧ་_ཀོར་/ཨེསི་ཨར་སི་/བི་ལོག/བི་ལོག/བི་ལོག/བི་ལོགསི:༢༠༣1111111111111 བཟོ་བསྐྲུན་པ་གིས་ Grafana ཐེབས་ཚད་ཚུ་ལག་ལེན་འཐབ་ཨིན།
4. **Commit + tenterometry:** `LaneBlockCommitment` ནང་དུ་ལེན་པ་དང་བརྗེ་སོར་གྱི་སྒྲུབ་བྱེད་ཐོབ་ཡོད། ཊེ་ལི་མི་ཊི་དྲན་ཐོ་ཚུ་ བཱ་ཕར་འཇལ་ཚད་ དབྱེ་བ་ (`iroha_settlement_pnl_xor`) འཇུག་སྤྱོད་འབད་མི་ མཐའ་མཚམས་ (`iroha_settlement_haircut_bp`) དང་ གདམ་ཁའི་བརྗེ་སོར་ལག་ལེན་ དེ་ལས་ རྒྱུ་དངོས་རེ་ལུ་ བསྒྱུར་བཅོས་/སྐྲ་གཅོད་པའི་གྱངས་ཁ་ཚུ་ ཌེཤ་བོརཌི་དང་ ཉེན་བརྡ་ཚུ་ བཀག་ཆ་དང་གཅིག་ཁར་ མཉམ་འབྱུང་སྦེ་ སྡོད་དོ་ཡོདཔ་ཨིན། ནང་དོན།【ཀརེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/block.298】【ཀྲེ་རེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/telemetry.rs:844】།
5. **མངོན་གསལ་གྱི་ཁ་ཐོག་:** `status::set_lane_settlement_commitments` རི་ལེ་/ཌི་ཨེ་ ཉོ་སྤྱོད་པ་ཚུ་གི་དོན་ལུ་ ཁས་བླངས་ཚུ་དཔར་བསྐྲུན་འབདཝ་ཨིན། Grafana བརྡ་བཀོད་ཚུ་ Prometheus མེཊི་རི་ལྷག་ཞིནམ་ལས་ བཀོལ་སྤྱོད་པ་ཚུ་གིས་ `ops/runbooks/settlement-buffers.md` འདི་ Prometheus དང་གཅིག་ཁར་ Grafana དང་ཅིག་ཁར་ བརྟག་ཞིབ་ལུ་ལག་ལེན་འཐབ་ཨིན། refill/ཐོར་ཊལ་བྱུང་ལས་ཚུ།

## བརྒྱུད་འཕྲིན་དང་སྒྲུབ་བྱེད།
- `iroha_settlement_buffer_xor`, `iroha_settlement_buffer_capacity_xor`, `iroha_settlement_buffer_status` — བླམ་བནྡེ་/གནས་སྡུད་ས་སྟོང་རེ་ལུ་ 【ཀརོ་-ཧེ་རོ་ཧ་_ཊེ་ལི་མི་ཊི་/ཨེསི་ཨར་སི་/མེ་ཊིག་སི།
- `iroha_settlement_pnl_xor` — བཀག་ཆའི་དོན་ལུ་ དང་ ཧེར་ཀཊ་རྗེས་མའི་ XOR གི་བར་ན་ ཁྱད་པར་མངོན་གསལ་བྱུང་ཡོདཔ།
- `iroha_settlement_haircut_bp` — བེཆ་ལུ་ལག་ལེན་འཐབ་མི་ ནུས་སྟོབས་ཅན་གྱི་ eepsilon/haircut གཞི་རྩའི་གནས་ཚད།
- `iroha_settlement_swapline_utilisation` — བརྗེ་སོར་གྱི་སྒྲུབ་བྱེད་ཡོད་པའི་སྐབས་ གདམ་ཁའི་ལག་ལེན་འཐབ་ཐངས་འདི་ 【crates/iroha_telemetry/src/metrics.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — གཞིས་ཆགས་དང་བསྡོམས་རྩིས་ཀྱི་ སྐྲ་གཅོད་ (XOR ཆ་ཤས་ཚུ་) 【ཀར/ཨི་རོ་ཧ་_ཊེ་ལི་མི་ཊི་རི་/ཨེསི་ཨར་སི་/མེ་ཊིགསི།
- Grafana བཀོད་སྒྲིག་: `dashboards/grafana/settlement_router_overview.json` (buffround, varicy, སྐྲ་གཅོག) དང་ ཉེན་བརྡ་བཏང་མི་གི་ལམ་ལུགས་ Nexus ལམ་ཐིག་ཉེན་བརྡ་ཐུམ་སྒྲིལ་ནང་བཙུགས་ཡོདཔ་ཨིན།
- བཀོལ་སྤྱོད་པ་རང་དེབ་: `ops/runbooks/settlement-buffers.md` (ལཱ་གི་རྒྱུན་རིམ་སླར་འཚལ།) དང་ `docs/source/nexus_settlement_faq.md` ནང་ FAQ.## གོང་འཕེལ་གཏང་མི་དང་ SRE ཞིབ་དཔྱད་ཐོ་ཡིག་།
- Grafana གནས་གོང་ཚུ་ `config/config.json5` (ཡང་ན་ TOML) ནང་ གཞི་སྒྲིག་འབད་ཞིནམ་ལས་ `irohad --version` དྲན་ཐོ་ཚུ་བརྒྱུད་དེ་ བདེན་དཔྱད་འབད། ཐེམ་སྐས་ཚུ་གིས་ `alert > throttle > xor_only > halt` ཚུ་ གྲུབ་ཚུགསཔ་ངེས་གཏན་བཟོ།
- བཱ་ཕར་རྩིས་ཐོ་/རྒྱུ་དངོས་/ལྕོགས་གྲུབ་དང་གཅིག་ཁར་ མི་རློབས་ལམ་ཐིག་མེ་ཊ་ཌེ་ཊ་ དེ་འབདཝ་ལས་ བཱ་ཕར་གཱེགསི་ཚུ་གིས་ ཐད་རི་བ་རི་ གསོག་འཇོག་ཚུ་ བསྟནམ་ཨིན། བཱ་ཕར་ཚུ་ བརྟག་ཞིབ་འབད་མ་དགོ་པའི་ ལམ་ཚུ་གི་དོན་ལུ་ ས་སྒོ་ཚུ་ བཏོན་གཏང་།
- `settlement_router_*` དང་ `iroha_settlement_*` གི་ མེ་ཊིགསི་ཚུ་ `dashboards/grafana/settlement_router_overview.json` བརྒྱུད་དེ་ ལྟ་རྟོག་འབདཝ་ཨིན། ཐོརཊ་ལུ་ཉེན་བརྡ་/ཨེགསི་ཨོ་ཨར་-རྐྱངམ་གཅིག་/ཧརཊ་མངའ་སྡེ་ཚུ།
- གོང་ཚད་/སྲིད་བྱུས་ཁྱབ་ཁོངས་དང་ ད་ལྟོ་ཡོད་པའི་ བཀག་ཆ་གནས་རིམ་གྱི་ བསྡོམས་རྩིས་བརྟག་དཔྱད་ `crates/iroha_core/src/block.rs` གི་དོན་ལུ་ `cargo test -p settlement_router` གཡོག་བཀོལ།
- `docs/source/nexus_fee_model.md` ནང་བསྒྱུར་བཅོས་ཚུ་རིམ་སྒྲིག་འབད་ནིའི་དོན་ལུ་ གཞུང་སྐྱོང་གནང་བ་ཚུ་དང་ ཚད་གཞི་ཡང་ན་ བརྒྱུད་འཕྲིན་ཁ་ཐོག་བསྒྱུར་བཅོས་འགྱོ་བའི་སྐབས་ `status.md` དུས་མཐུན་བཟོ་དགོ།

## ཐོ་འགོད་འཆར་གཞི།
- བཟོ་བསྐྲུན་ག་རའི་ནང་ལུ་ རའུ་ཊར་ + བརྒྱུད་འཕྲིན་གྲུ་གཟིངས། ཁྱད་རྣམ་གྱི་སྒོ་ར་མེད་པ། ལེན་མེ་ཊ་ཌེ་ཊ་གིས་ བཱ་ཕར་པར་ལེན་ཚུ་གིས་ དཔར་བསྐྲུན་འབདཝ་ཨིན་ན་མེན་ན་ ཚད་འཛིན་འབདཝ་ཨིན།
- སྔོན་སྒྲིག་རིམ་སྒྲིག་གིས་ ལམ་གྱི་ས་ཁྲ་གནས་གོང་ཚུ་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན་ (༦༠s TWAP, ༢༥bp གཞི་རྟེན་ epsilon, 72h bubffer གནམ་སྟོང་); འཇུག་སྤྱོད་འབད་ནི་ལུ་ Grafana བརྒྱུད་དེ་ ཊུ་ནའི།
- བདེན་དཔང་བང་སྒྲིག = ལམ་གྱི་གཞིས་ཆགས་ཁས་ལེན་ + `settlement_router_*`/`iroha_settlement_*` རིམ་སྒྲིག་ + Grafana གསལ་གཞི་/JSON གནོད་སྐྱོན་བྱུང་བའི་སྒོ་སྒྲིག་གི་དོན་ལུ་ ཕྱིར་འདྲེན་འབད།

## བདེན་དཔང་དང་ དཔྱད་གཞི།
- NX-3 གཞིས་ཆགས་རའུ་ཊར་ངོས་ལེན་དྲན་ཐོ་: `status.md` (NX-3 དབྱེ་ཚད།).
- བཀོལ་སྤྱོད་པའི་ཕྱི་ངོས་: `dashboards/grafana/settlement_router_overview.json`, `ops/runbooks/settlement-buffers.md`.
- ཐོབ་ཐངས་འཆར་གཞི་དང་ ཨེ་པི་ཨའི་ ཁ་ཐོག་ཚུ་: `docs/source/nexus_fee_model.md`, `/v2/sumeragi/status` -> `lane_settlement_commitments`.