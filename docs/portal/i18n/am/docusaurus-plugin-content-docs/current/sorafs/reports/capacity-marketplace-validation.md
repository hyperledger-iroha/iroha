---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Capacity Marketplace Validation
tags: [SF-2c, acceptance, checklist]
summary: Acceptance checklist covering provider onboarding, dispute workflows, and treasury reconciliation gating the SoraFS capacity marketplace general availability.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS የአቅም የገበያ ቦታ ማረጋገጫ ዝርዝር

**የግምገማ መስኮት፡** 2026-03-18 → 2026-03-24  
**የፕሮግራም ባለቤቶች፡** የማከማቻ ቡድን (`@storage-wg`)፣ የአስተዳደር ምክር ቤት (`@council`)፣ የግምጃ ቤት ማህበር (`@treasury`)  
** ወሰን፡** ለSF-2c GA የሚፈለጉ የአቅራቢዎች የቧንቧ መስመሮች፣ የክርክር ዳኝነት ፍሰቶች እና የግምጃ ቤት ማስታረቅ ሂደቶች።

የገበያ ቦታውን ለውጭ ኦፕሬተሮች ከማስቻሉ በፊት ከዚህ በታች ያለው የማረጋገጫ ዝርዝር መከለስ አለበት። እያንዳንዱ ረድፍ ኦዲተሮች እንደገና ሊጫወቱት ከሚችሉት ወሳኝ ማስረጃዎች (ሙከራዎች፣ ዕቃዎች ወይም ሰነዶች) ጋር ያገናኛል።

## የመቀበል ማረጋገጫ ዝርዝር

### አቅራቢ በመሳፈር ላይ

| አረጋግጥ | ማረጋገጫ | ማስረጃ |
|-------|------------|------|
| መዝገብ ቤት ቀኖናዊ የአቅም መግለጫዎችን ይቀበላል | የውህደት ሙከራ መልመጃዎች `/v2/sorafs/capacity/declare` በመተግበሪያው ኤፒአይ በኩል፣ የፊርማ አያያዝን፣ ሜታዳታ ቀረጻን እና ወደ መስቀለኛ መንገድ መዝገብ ማጥፋት። | `crates/iroha_torii/src/routing.rs:7654` |
| ብልጥ ኮንትራት ያልተዛመደ ክፍያ ውድቅ ያደርጋል | የዩኒት ፈተና የአቅራቢ መታወቂያዎችን እና የጊቢ መስኮችን ከመቀጠልዎ በፊት ከተፈረመው መግለጫ ጋር እንደሚዛመዱ ያረጋግጣል። | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI ቀኖናዊ የመሳፈሪያ ቅርሶችን ያመነጫል | CLI harness የሚወስነውን I18NT0000002X/JSON/Base64 ውጤቶችን ይጽፋል እና የዙር ጉዞዎችን ያረጋግጣል ስለዚህ ኦፕሬተሮች ከመስመር ውጭ መግለጫዎችን እንዲያሳዩ። | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| ኦፕሬተር መመሪያ የመግቢያ የስራ ሂደት እና የአስተዳደር ጥበቃ መንገዶችን ይይዛል | ሰነዱ የማወጃ እቅድን፣ የመመሪያ ነባሮችን እና ለምክር ቤቱ ግምገማ ደረጃዎችን ይዘረዝራል። | `../storage-capacity-marketplace.md` |

### የክርክር አፈታት

| አረጋግጥ | ማረጋገጫ | ማስረጃ |
|-------|------------|------|
| የክርክር መዛግብት በቀኖናዊ የደመወዝ ጭነት መፍቻ | የዩኒት ፈተና አለመግባባቶችን ይመዘግባል፣ የተከማቸ ክፍያን ይፈታዋል እና የሂሳብ መዝገብ አወሳሰንን ለማረጋገጥ በመጠባበቅ ላይ ያለ ሁኔታን ያረጋግጣል። | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI ሙግት ጄኔሬተር ቀኖናዊ ንድፍ ጋር ይዛመዳል | የCLI ፈተና የBase64/Norito ውፅዓቶችን እና የJSON ማጠቃለያዎችን ለ`CapacityDisputeV1` ይሸፍናል፣የማስረጃ ቅርቅቦችን በቁርጠኝነት ያረጋግጣል። | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| የድጋሚ አጫውት ፈተና ሙግት/የቅጣት ውሳኔን ያረጋግጣል | የማረጋገጫ አለመሳካት ቴሌሜትሪ ሁለት ጊዜ በድጋሚ የተጫወተው ተመሳሳይ ደብተር፣ ክሬዲት እና ሙግት ቅጽበታዊ ገጽ እይታዎችን ያዘጋጃል ስለዚህ መቆራረጦች በእኩዮች መካከል የሚወሰኑ ናቸው። | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook ሰነዶች መጨመር እና የመሻር ፍሰት | የክዋኔ መመሪያ የምክር ቤት የስራ ሂደትን፣ የማስረጃ መስፈርቶችን እና የመመለሻ ሂደቶችን ይይዛል። | `../dispute-revocation-runbook.md` |

### የግምጃ ቤት ማስታረቅ

| አረጋግጥ | ማረጋገጫ | ማስረጃ |
|-------|------------|------|
| Ledger accrual የ30-ቀን soak projection ጋር ይዛመዳል | የሶክ ሙከራ አምስት አቅራቢዎችን በ30 የመቋቋሚያ መስኮቶች ላይ ይሸፍናል፣ ይህም የሂሳብ መዝገብ ግቤቶችን ከሚጠበቀው የክፍያ ማጣቀሻ ጋር ይለያል። | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| ደብተር ኤክስፖርት እርቅ በምሽት ተመዝግቧል | `capacity_reconcile.py` የክፍያ ደብተር የሚጠበቁትን ከተፈጸሙት የXOR ማስተላለፍ ኤክስፖርት ጋር ያወዳድራል፣ I18NT0000000X ሜትሪክስ ያወጣል እና የግምጃ ቤት ማረጋገጫ በአለርትማናጀር በኩል። | `scripts/telemetry/capacity_reconcile.py:1`,I18NI0000023X,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| የሂሳብ አከፋፈል ዳሽቦርዶች የወለል ቅጣቶች እና የተጠራቀመ ቴሌሜትሪ | Grafana የማስመጣት ሴራዎች የጂቢሆር ክምችት፣ የስራ ማቆም አድማዎች እና ለጥሪ ታይነት የታሰሩ ዋስትናዎች። | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| የታተመ የሪፖርት መዛግብት የማጥለቅ ዘዴ እና የድጋሚ ትዕዛዞችን | ለኦዲተሮች የዝርዝር ወሰን፣ የአፈጻጸም ትዕዛዞች እና ታዛቢነት መንጠቆዎችን ሪፖርት ያድርጉ። | `./sf2c-capacity-soak.md` |

## የአፈፃፀም ማስታወሻዎች

ከመውጣቱ በፊት የማረጋገጫ ስብስብን እንደገና ያሂዱ፡

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

ኦፕሬተሮች የመሳፈሪያ/የሙግት ጥያቄን በI18NI0000027X እንደገና ማመንጨት እና የተገኘውን JSON/Norito ባይት ከአስተዳደር ትኬት ጋር በማህደር ማስቀመጥ አለባቸው።

## ዘግተህ አጥፋ ቅርሶች

| Artefact | መንገድ | blake2b-256 |
|-------------|--------|
| አቅራቢ የመሳፈሪያ ማረጋገጫ ፓኬት | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| የክርክር አፈታት ማጽደቂያ ፓኬት | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| የግምጃ ቤት ማስታረቅ ማጽደቂያ ፓኬት | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

የተፈረሙትን የእነዚህን ቅርሶች ቅጂዎች ከተለቀቀው ጥቅል ጋር ያከማቹ እና በአስተዳደር ለውጥ መዝገብ ውስጥ ያገናኙዋቸው።

## ማጽደቆች

- የማከማቻ ቡድን መሪ - @storage-tl (2026-03-24)  
- የአስተዳደር ምክር ቤት ፀሐፊ - @ካውንስል-ሰከንድ (2026-03-24)  
- የግምጃ ቤት ስራዎች አመራር - @treasury-ops (2026-03-24)