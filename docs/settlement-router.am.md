---
lang: am
direction: ltr
source: docs/settlement-router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 782429c90ac5df034fd7c8ff2c3acf4f9f11348f14f15fcd321f343b22b154b8
source_last_modified: "2025-12-29T18:16:35.914434+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ቆራጥ የሰፈራ ራውተር (NX-3)

** ሁኔታ፡** የተጠናቀቀ (NX-3)  
** ባለቤቶች፡** ኢኮኖሚክስ WG/ኮር ሌጀር WG/ግምጃ ቤት/SRE  
** ወሰን፡** ቀኖናዊ XOR የሰፈራ መንገድ በሁሉም መስመሮች/የመረጃ ቦታዎች ጥቅም ላይ ይውላል። የተላኩ የራውተር ሣጥን፣ የሌይን ደረጃ ደረሰኞች፣ የመጠባበቂያ ሐዲዶች፣ ቴሌሜትሪ እና የኦፕሬተር ማስረጃ ቦታዎች።

# ግቦች
- በነጠላ መስመር እና በNexus ግንባታዎች ላይ የXOR ልወጣን እና ደረሰኝ ማመንጨትን አንድ ማድረግ።
- ኦፕሬተሮች እልባትን በአስተማማኝ ሁኔታ እንዲራመዱ ቆራጥ የሆኑ የፀጉር አስተካካዮችን + ተለዋዋጭ ህዳጎችን በጠባቂ-ሀዲድ ቋት ይተግብሩ።
- ደረሰኞችን፣ ቴሌሜትሪ እና ዳሽቦርዶችን ኦዲተሮች ያለ ሹመት መሳሪያ እንደገና መጫወት ይችላሉ።

## አርክቴክቸር
| አካል | አካባቢ | ኃላፊነት |
|-------------|
| ራውተር primitives | `crates/settlement_router/` | የጥላ ዋጋ ማስያ፣ የፀጉር መቁረጫ ደረጃዎች፣ የመጠባበቂያ ፖሊሲ ረዳቶች፣ የመቋቋሚያ ደረሰኝ አይነት።
| የማስኬጃ ፊት ለፊት | `crates/iroha_core/src/settlement/mod.rs:1` | የራውተር አወቃቀሩን ወደ `SettlementEngine` ያጠቃልላል፣በብሎክ አፈጻጸም ወቅት ጥቅም ላይ የዋለውን `quote` + አከማቸን ያጋልጣል። |
| ውህደትን አግድ | `crates/iroha_core/src/block.rs:120` | `PendingSettlement` መዝገቦችን ያወጣል፣ `LaneSettlementCommitment` በአንድ ሌይን/በመረጃ ቦታ ይሰበስባል፣የሌይን ቋት ሜታዳታን ይተነትናል እና ቴሌሜትሪ ያወጣል። |
| ቴሌሜትሪ እና ዳሽቦርዶች | `crates/iroha_telemetry/src/metrics.rs:4847`, `dashboards/grafana/settlement_router_overview.json:1` | Prometheus/OTLP መለኪያዎች ለጠባቂዎች፣ ልዩነት፣ የፀጉር ማቆሚያዎች፣ የመቀየሪያ ቆጠራዎች; Grafana ሰሌዳ ለ SRE. |
| የማጣቀሻ እቅድ | `docs/source/nexus_fee_model.md:1` | የሰነዶች መቋቋሚያ ደረሰኝ መስኮች በ`LaneBlockCommitment` ውስጥ ቀጥለዋል። |

## ማዋቀር
የራውተር ቁልፎች በ`[settlement.router]` (በ`iroha_config` የተረጋገጠ) ይኖራሉ።

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

በየውሂብ ቦታ ቋት መለያ ውስጥ የሌይን ዲበ ውሂብ ሽቦዎች፡-
- `settlement.buffer_account` - መጠባበቂያውን የያዘ መለያ (ለምሳሌ `buffer::cbdc_treasury`)።
- `settlement.buffer_asset` — ለዋና ክፍል (በተለምዶ `xor#sora`) የተከፈለ የንብረት ትርጉም።
- `settlement.buffer_capacity_micro` - በማይክሮ-XOR (አስርዮሽ ሕብረቁምፊ) ውስጥ የተዋቀረ አቅም።

የሜታዳታ አለመኖር ለዚያ መስመር ቋት ፎቶ ማንሳትን ያሰናክላል (ቴሌሜትሪ ወደ ዜሮ አቅም/ሁኔታ ይመለሳል)።## የመቀየሪያ ቧንቧ
1. **ጥቅስ፡** `SettlementEngine::quote` የተዋቀረውን የኤፒሲሎን + የመተጣጠፍ ህዳግ እና የፀጉር መቆንጠጫ ደረጃን ወደ TWAP ጥቅሶች ይተገብራል፣ `SettlementReceipt` በ `xor_due` እና `xor_after_haircut` እና በፕላስ ይደውሉ `source_id`.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】
2. ** ማጠራቀም: ** በብሎክ አፈፃፀም ወቅት አስፈፃሚው `PendingSettlement` ግቤቶችን ይመዘግባል (የአካባቢው መጠን ፣ TWAP ፣ epsilon ፣ volatility bucket ፣ liquidity profile ፣ oracle timestamp)። `LaneSettlementBuilder` ድምርን ያጠቃለለ እና እገዳውን ከማኅተሙ በፊት በ`(lane, dataspace)` ሜታዳታ ይቀያይራል።
3. **የማቋቋሚያ ቅጽበታዊ ገጽ እይታ፡** የሌይን ሜታዳታ ቋት ካወጀ፣ ግንበኛ `SettlementBufferSnapshot` (የቀረውን ዋና ክፍል፣ አቅም፣ ሁኔታ) የ `BufferPolicy` ጣራዎችን ከውቅረት በመጠቀም ይይዛል።【crates/iroha_core/src/block.rs:20
4. **Commit + Telemetry:** ደረሰኞች እና የተቀያየሩ ማስረጃዎች በ`LaneBlockCommitment` ውስጥ መሬት እና በሁኔታ ቅጽበተ-ፎቶዎች ይገለጣሉ። ቴሌሜትሪ ቋት መለኪያዎችን፣ ልዩነትን (`iroha_settlement_pnl_xor`)፣ የተተገበረ ህዳግ (`iroha_settlement_haircut_bp`)፣ አማራጭ ስዋፕላይን አጠቃቀምን እና በንብረት መቀየሪያ/የጸጉር መቁረጫ ቆጣሪዎችን ይመዘግባል ስለዚህም ዳሽቦርዶች እና ማንቂያዎች ከእገዳው ጋር እንደተመሳሰሉ ይቆያሉ። ይዘቶች።【crates/iroha_core/src/block.rs:298】【crates/iroha_core/src/telemetry.rs:844】
5. **የማስረጃ ቦታዎች፡** `status::set_lane_settlement_commitments` ለሪሌይ/ዲኤ ሸማቾች ቃል ኪዳኖችን ያትማል፣ Grafana ዳሽቦርዶች የ Prometheus ሜትሪክስን ያነባሉ፣ እና ኦፕሬተሮች `ops/runbooks/settlement-buffers.md`ን ከ I10000000042X ጋር ከ I1000000042X ወደ ትራከክሮ ዝግጅቶች ይጠቀማሉ።

## ቴሌሜትሪ እና ማስረጃ
- `iroha_settlement_buffer_xor`፣ `iroha_settlement_buffer_capacity_xor`፣ `iroha_settlement_buffer_status` — የቋት ቅጽበታዊ ሌይን/መረጃ ቦታ (ማይክሮ-XOR + ኮድ የተደረገበት ሁኔታ)።【crates/iroha_telemetry/src/metrics.rs:6212】
- `iroha_settlement_pnl_xor` - በድህረ-ፀጉር መቆረጥ XOR መካከል ያለው ልዩነት ለብሎክ ባች።【crates/iroha_telemetry/src/metrics.rs:6236】
- `iroha_settlement_haircut_bp` — ውጤታማ የኤፒሲሎን/የጸጉር መቆረጥ መነሻ ነጥቦች በቡድን ላይ ተተግብረዋል።【crates/iroha_telemetry/src/metrics.rs:6244】
- `iroha_settlement_swapline_utilisation` - የመለዋወጫ ማስረጃ በሚኖርበት ጊዜ በፈሳሽ ፕሮፋይል የተቀመጠ አማራጭ አጠቃቀም።【crates/iroha_telemetry/src/metrics.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — በሰፈራ ልወጣዎች እና ድምር የፀጉር አስተካካዮች (XOR units) በየመንገድ/ዳታ ቦታ ቆጣሪዎች።
- Grafana ሰሌዳ፡ `dashboards/grafana/settlement_router_overview.json` (የመቋቋሚያ ራስ ክፍል፣ ልዩነት፣ የፀጉር መቆራረጥ) እና የ Alertmanager ደንቦች በNexus ሌይን ማንቂያ ጥቅል ውስጥ የተካተቱ።
- ኦፕሬተር runbook: `ops/runbooks/settlement-buffers.md` (የመሙላት / የማንቂያ የስራ ፍሰት) እና በ `docs/source/nexus_settlement_faq.md` ውስጥ የሚጠየቁ ጥያቄዎች.## ገንቢ እና SRE ማረጋገጫ ዝርዝር
- `[settlement.router]` ዋጋዎችን በ `config/config.json5` (ወይም TOML) ያቀናብሩ እና በ `irohad --version` ምዝግብ ማስታወሻዎች ያረጋግጡ; የመግቢያ ገደቦች `alert > throttle > xor_only > halt` ማሟላታቸውን ያረጋግጡ።
- የሌይን ሜታዳታ ከቋት መለያ/ንብረት/አቅም ጋር ተሞልቷል ስለዚህ የቋት መለኪያዎች የቀጥታ ማከማቻዎችን ያንፀባርቃሉ። ማቋረጦችን መከታተል የሌለባቸው መስመሮችን መተው።
- `settlement_router_*` እና `iroha_settlement_*` መለኪያዎችን በ `dashboards/grafana/settlement_router_overview.json` ይቆጣጠሩ; በስሮትል/XOR-ብቻ/በማቆም ግዛቶች ላይ ማንቂያ።
- `cargo test -p settlement_router` ን ለዋጋ/የፖሊሲ ሽፋን እና በ `crates/iroha_core/src/block.rs` ውስጥ ያሉትን የብሎክ-ደረጃ ማጠቃለያ ሙከራዎችን ያሂዱ።
- በ`docs/source/nexus_fee_model.md` ውስጥ ለውቅረት ለውጦች የአስተዳደር ማፅደቆችን ይመዝግቡ እና ደረጃዎች ወይም ቴሌሜትሪ ወለል ሲቀየሩ `status.md` እንደተዘመነ ያቆዩት።

## የልቀት እቅድ ቅጽበታዊ እይታ
- በእያንዳንዱ ግንባታ ውስጥ ራውተር + ቴሌሜትሪ መርከብ; ምንም ባህሪ በሮች. የሌይን ዲበ ውሂብ የቋት ቅጽበተ-ፎቶዎች ማተምን ይቆጣጠራል።
- ነባሪ ውቅረት ከመንገድ ካርታው እሴቶች ጋር ይዛመዳል (60s TWAP፣ 25bp base epsilon፣ 72h buffer horizon); ለማመልከት በ config በኩል ያስተካክሉ እና `irohad` እንደገና ያስጀምሩ።
- የማስረጃ ጥቅል = የሌይን ማቋቋሚያ ቃል ኪዳኖች + Prometheus ለ`settlement_router_*`/`iroha_settlement_*` ተከታታይ + Grafana ቅጽበታዊ ገጽ እይታ/JSON ለተጎዳው መስኮት ወደ ውጭ መላክ።

## ማስረጃዎች እና ማጣቀሻዎች
- NX-3 የሰፈራ ራውተር ተቀባይነት ማስታወሻዎች: `status.md` (NX-3 ክፍል).
- ኦፕሬተር ወለሎች፡ `dashboards/grafana/settlement_router_overview.json`፣ `ops/runbooks/settlement-buffers.md`።
- የመቀበያ ንድፍ እና የኤፒአይ ወለል፡ `docs/source/nexus_fee_model.md`፣ `/v1/sumeragi/status` -> `lane_settlement_commitments`።