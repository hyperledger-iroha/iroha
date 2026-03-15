---
lang: am
direction: ltr
source: docs/source/da/rent_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7cdc46bcd87af7924817a94900c8fad2c23570607f4065f19d8a42d259fe83f
source_last_modified: "2026-01-22T14:35:37.691079+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የውሂብ ተገኝነት ኪራይ እና ማበረታቻ ፖሊሲ (DA-7)

_ሁኔታ፡ ረቂቅ - ባለቤቶች፡ ኢኮኖሚክስ WG / የግምጃ ቤት / የማከማቻ ቡድን_

የመንገድ ካርታ ንጥል **DA-7** ለእያንዳንዱ ብሎብ ግልጽ የሆነ በXOR የሚከፈል ኪራይ ያስተዋውቃል
ለNorito ገብቷል፣ እና ለ PDP/PoTR አፈፃፀም የሚሸልሙ ጉርሻዎች እና
egress ደንበኞችን ለማምጣት አገልግሏል። ይህ ሰነድ የመጀመሪያ መለኪያዎችን ይገልጻል ፣
የእነሱ የውሂብ-ሞዴል ውክልና እና በ Torii ጥቅም ላይ የዋለው የስሌቱ የስራ ፍሰት ፣
ኤስዲኬዎች፣ እና የግምጃ ቤት ዳሽቦርዶች።

## የፖሊሲ መዋቅር

መመሪያው እንደ [`DaRentPolicyV1`](/crates/iroha_data_model/src/da/types.rs) ተቀምጧል።
በመረጃ ሞዴል ውስጥ. Torii እና የአስተዳደር መሳሪያዎች ፖሊሲው በ ውስጥ ቀጥሏል።
የNorito ጭነቶች የኪራይ ዋጋዎችን እና የማበረታቻ ደብተሮችን እንደገና ማስላት ይቻላል
በቆራጥነት። መርሃግብሩ አምስት ቁልፎችን ያሳያል-

| መስክ | መግለጫ | ነባሪ |
|-------|-------------|--------|
| `base_rate_per_gib_month` | XOR በየወሩ በማቆየት በጂቢ ይከፍላል። | `250_000` ማይክሮ-XOR (0.25 XOR) |
| `protocol_reserve_bps` | ወደ ፕሮቶኮል መጠባበቂያ (መሰረታዊ ነጥቦች) የተላለፈው የኪራይ ድርሻ። | `2_000` (20%) |
| `pdp_bonus_bps` | የጉርሻ መቶኛ በተሳካ የ PDP ግምገማ። | `500` (5%) |
| `potr_bonus_bps` | የጉርሻ መቶኛ በተሳካ የPoTR ግምገማ። | `250` (2.5%) |
| `egress_credit_per_gib` | ክሬዲት የሚከፈለው አቅራቢው 1ጂቢ የDA ውሂብ ሲያገለግል ነው። | `1_500` ማይክሮ-XOR |

ሁሉም የመሠረት ነጥብ እሴቶች የተረጋገጡት በ`BASIS_POINTS_PER_UNIT` (10000) ነው።
የፖሊሲ ዝማኔዎች በአስተዳደር በኩል መጓዝ አለባቸው፣ እና እያንዳንዱ የTorii መስቀለኛ መንገድ ይህንን ያጋልጣል።
ንቁ ፖሊሲ በ `torii.da_ingest.rent_policy` ውቅር ክፍል በኩል
(`iroha_config`)። ኦፕሬተሮች በ `config.toml` ውስጥ ያሉትን ነባሪዎች መሻር ይችላሉ፡

```toml
[torii.da_ingest.rent_policy]
base_rate_per_gib_month_micro = 250000        # 0.25 XOR/GiB-month
protocol_reserve_bps = 2000                   # 20% protocol reserve
pdp_bonus_bps = 500                           # 5% PDP bonus
potr_bonus_bps = 250                          # 2.5% PoTR bonus
egress_credit_per_gib_micro = 1500            # 0.0015 XOR/GiB egress credit
```

CLI Tooling (`iroha app da rent-quote`) ተመሳሳይ የNorito/JSON ፖሊሲ ግብዓቶችን ይቀበላል
እና ገባሪውን `DaRentPolicyV1` ሳይደርሱ የሚያንፀባርቁ ቅርሶችን ያወጣል።
ወደ Torii ግዛት ተመለስ። ለመግቢያ ሩጫ ጥቅም ላይ የዋለውን የፖሊሲ ቅጽበተ ፎቶ ያቅርቡ
ጥቅስ እንደገና ሊባዛ ይችላል።

### የማያቋርጥ የኪራይ ዋጋ ቅርሶች

`iroha app da rent-quote --gib <size> --months <months> --quote-out <path>` ን ያሂዱ
ሁለቱንም በስክሪኑ ላይ ያለውን ማጠቃለያ እና በቆንጆ-የታተመ JSON አርትዕክት ያውጡ። ፋይሉ
መዛግብት `policy_source`፣ ውስጠ መስመር ያለው `DaRentPolicyV1` ቅጽበታዊ ገጽ እይታ፣ የተሰላ
`DaRentQuote`፣ እና የተገኘ `ledger_projection` (ተከታታይ በ
[`DaRentLedgerProjection`](/crates/iroha_data_model/src/da/types.rs)) ለግምጃ ቤት ዳሽቦርዶች እና ለመጽሐፍ ደብተር ISI ተስማሚ ያደርገዋል
የቧንቧ መስመሮች. `--quote-out` በአንድ ጎጆ ማውጫ ላይ ሲይዝ CLI ማንኛውንም ይፈጥራል
የጎደሉ ወላጆች፣ ስለዚህ ኦፕሬተሮች እንደ ያሉ ቦታዎችን መደበኛ ማድረግ ይችላሉ።
`artifacts/da/rent_quotes/<timestamp>.json` ከሌሎች የ DA ማስረጃ ቅርቅቦች ጋር።
ማጽደቂያዎችን ወይም እርቅን ለመከራየት አርቲፊኬቱን ያያይዙ ስለዚህ XOR
ብልሽት (መሰረታዊ ኪራይ፣ መጠባበቂያ፣ የ PDP/PoTR ጉርሻዎች፣ እና ኢግረስ ክሬዲቶች) ነው።
ሊባዛ የሚችል. በራስ ሰር ለመሻር `--policy-label "<text>"` ይለፉ
የተገኘ `policy_source` መግለጫ (የፋይል ዱካዎች፣ የተከተተ ነባሪ፣ ወዘተ.) ከ
ሰው-ሊነበብ የሚችል መለያ እንደ የአስተዳደር ትኬት ወይም አንጸባራቂ ሃሽ; የ CLI መከርከም
ይህ ዋጋ እና ባዶ/ነጭ ቦታ-ብቻ ሕብረቁምፊዎችን ውድቅ ያደርጋል ስለዚህም የተቀዳው ማስረጃ
ኦዲት ተደርጎ ይቆያል።

```json
{
  "policy_source": "policy JSON `configs/da/rent_policy.json`",
  "gib": 10,
  "months": 3,
  "policy": { "...": "DaRentPolicyV1 fields elided" },
  "quote": { "...": "DaRentQuote breakdown" },
  "ledger_projection": {
    "rent_due": { "micro": 7500000 },
    "protocol_reserve_due": { "micro": 1500000 },
    "provider_reward_due": { "micro": 6000000 },
    "pdp_bonus_pool": { "micro": 375000 },
    "potr_bonus_pool": { "micro": 187500 },
    "egress_credit_per_gib": { "micro": 1500 }
  }
}
```የሂሳብ መዝገብ ትንበያ ክፍል በቀጥታ ወደ DA የኪራይ ደብተር አይኤስአይኤስ ይመገባል፡ እሱ
ለፕሮቶኮል መጠባበቂያ፣ ለአቅራቢ ክፍያዎች፣ እና ለ XOR ዴልታዎች የታሰበውን ይገልጻል
የውርስ ኦርኬስትራ ኮድ ሳያስፈልጋቸው የሁሉም ማረጋገጫ ቦነስ ገንዳዎች።

### የኪራይ ደብተር ዕቅዶችን ማመንጨት

`iroha app da rent-ledger --quote <path> --payer-account <id> --treasury-account <id> --protocol-reserve-account <id> --provider-account <id> --pdp-bonus-account <id> --potr-bonus-account <id> --asset-definition xor#sora` አሂድ
ቀጣይነት ያለው የኪራይ ዋጋን ወደ ተፈጻሚነት ያለው የሂሳብ መዝገብ ማስተላለፍ። ትዕዛዙ
የተከተተውን `ledger_projection` ይተነትናል፣ Norito `Transfer` መመሪያዎችን ያወጣል።
የመነሻ ኪራይን ወደ ግምጃ ቤት የሚሰበስቡ ፣ የተጠባባቂውን / አቅራቢውን ያካሂዳሉ
ክፍሎች፣ እና የ PDP/PoTR ቦነስ ገንዳዎችን በቀጥታ ከከፋዩ ቀድመው ይከፍላሉ። የ
ውፅዓት JSON የጥቅሱን ሜታዳታ ያንፀባርቃል ስለዚህም CI እና የግምጃ ቤት መሥሪያ ቤት ሊረዱ ይችላሉ።
ስለ ተመሳሳይ ቅርስ:

```json
{
  "quote_path": "artifacts/da/rent_quotes/2025-12-07/rent.json",
  "rent_due_micro_xor": 7500000,
  "protocol_reserve_due_micro_xor": 1500000,
  "provider_reward_due_micro_xor": 6000000,
  "pdp_bonus_pool_micro_xor": 375000,
  "potr_bonus_pool_micro_xor": 187500,
  "egress_credit_per_gib_micro_xor": 1500,
  "instructions": [
    { "Transfer": { "...": "payer -> treasury base rent instruction elided" }},
    { "Transfer": { "...": "treasury -> reserve" }},
    { "Transfer": { "...": "treasury -> provider payout" }},
    { "Transfer": { "...": "payer -> PDP bonus escrow" }},
    { "Transfer": { "...": "payer -> PoTR bonus escrow" }}
  ]
}
```

የመጨረሻው `egress_credit_per_gib_micro_xor` መስክ ዳሽቦርድ እና ክፍያ ይፈቅዳል
የጊዜ መርሐግብር አውጪዎች የወጪ ክፍያ ክፍያን ካመጣው የኪራይ ፖሊሲ ጋር ያስተካክላሉ
የፖሊሲ ሒሳብን በስክሪፕት ሙጫ ውስጥ እንደገና ሳታሰላስል ጥቀስ።

## ምሳሌ ጥቅስ

```rust
use iroha_data_model::da::types::DaRentPolicyV1;

// 10 GiB retained for 3 months.
let policy = DaRentPolicyV1::default();
let quote = policy.quote(10, 3).expect("policy validated");

assert_eq!(quote.base_rent.as_micro(), 7_500_000);      // 7.5 XOR total rent
assert_eq!(quote.protocol_reserve.as_micro(), 1_500_000); // 20% reserve
assert_eq!(quote.provider_reward.as_micro(), 6_000_000);  // Direct provider payout
assert_eq!(quote.pdp_bonus.as_micro(), 375_000);          // PDP success bonus
assert_eq!(quote.potr_bonus.as_micro(), 187_500);         // PoTR success bonus
assert_eq!(quote.egress_credit_per_gib.as_micro(), 1_500);
```

ጥቅሱ በTorii ኖዶች፣ ኤስዲኬዎች እና የግምጃ ቤት ሪፖርቶች ላይ ሊባዛ የሚችል ነው ምክንያቱም
ከአድ-ሆክ ሂሳብ ይልቅ ቆራጥ የሆኑ Norito መዋቅሮችን ይጠቀማል። ኦፕሬተሮች ይችላሉ።
የJSON/CBOR ኮድ `DaRentPolicyV1` ከአስተዳደር ሀሳቦች ወይም ኪራይ ጋር ያያይዙ
ለየትኛውም ብልጭልጭነት የትኞቹ መለኪያዎች እንደነበሩ ለማረጋገጥ ኦዲቶች.

## ጉርሻዎች እና መጠባበቂያዎች

- ** የፕሮቶኮል መጠባበቂያ፡** `protocol_reserve_bps` የሚደግፈውን የ XOR መጠባበቂያ ገንዘብ ይሸፍናል
  የአደጋ ጊዜ መልሶ ማባዛት እና ተመላሽ ገንዘቦችን መቀነስ። ግምጃ ቤት ይህንን ባልዲ ይከታተላል
  የመመዝገቢያ ሒሳቦች ከተዋቀረው መጠን ጋር እንደሚዛመዱ ለማረጋገጥ በተናጠል።
- **PDP/PoTR ጉርሻዎች፡** እያንዳንዱ የተሳካ ማረጋገጫ ግምገማ ተጨማሪ ይቀበላል
  ክፍያ ከ `base_rent × bonus_bps` የተገኘ። የዲኤ መርሐግብር አዘጋጅ ማስረጃ ሲያወጣ
  ማበረታቻዎች እንደገና እንዲጫወቱ ደረሰኞች የመሠረት-ነጥብ መለያዎችን ያካትታል።
- ** ኢግረስ ክሬዲት፡** አቅራቢዎች ጂቢን በማንፀባረቅ ያቀረበውን ይመዘግባሉ፣ ይባዛሉ
  `egress_credit_per_gib`፣ እና ደረሰኞችን በ`iroha app da prove-availability` በኩል ያስገቡ።
  የኪራይ ፖሊሲ የጊቢ መጠን ከአስተዳደር ጋር እንዲመሳሰል ያደርገዋል።

## የተግባር ፍሰት

1. ** ማስገቢያ፡** `/v1/da/ingest` ገባሪውን `DaRentPolicyV1` ይጭናል የኪራይ ዋጋ
   በብሎብ መጠን እና ማቆየት ላይ በመመስረት እና ጥቅሱን በ Norito ውስጥ አካትቷል።
   ገላጭ. አስገቢው የኪራይ ሀሽ እና የማጣቀሻ መግለጫ ይፈርማል
   የማከማቻ ትኬት መታወቂያ.
2. **የሂሳብ አያያዝ፡** የግምጃ ቤት ፅሁፎች አንጸባራቂውን መፍታት፣ ይደውሉ
   `DaRentPolicyV1::quote`፣ እና የህዝብ የኪራይ ደብተሮች (ቤዝ ኪራይ፣ መጠባበቂያ፣
   ጉርሻዎች, እና የሚጠበቁ የመውጣት ክሬዲቶች). በተመዘገበ የቤት ኪራይ መካከል ያለ ማንኛውም ልዩነት
   እና እንደገና የተቆጠሩ ጥቅሶች CI ወድቀዋል።
3. **የማስረጃ ሽልማቶች፡** የ PDP/PoTR መርሐግብር አውጪዎች ስኬትን ሲያመለክቱ ደረሰኝ ይለቃሉ።
   አንጸባራቂ መፍጨት፣ የማረጋገጫ አይነት እና የ XOR ጉርሻ የያዘ
   ፖሊሲው. አስተዳደር ተመሳሳዩን ዋጋ እንደገና በማስላት ክፍያዎችን ኦዲት ማድረግ ይችላል።
4. **የኢግረስ ክፍያ፡** ኦርኬስትራዎችን አምጡ የተፈረመ የመውጣት ማጠቃለያዎችን ያስገቡ።
   Torii የጂቢ ቆጠራን በ`egress_credit_per_gib` በማባዛት ክፍያ ይሰጣል
   ከኪራይ መጨናነቅ ጋር የሚቃረን መመሪያ.

## ቴሌሜትሪTorii አንጓዎች የኪራይ አጠቃቀምን በሚከተሉት Prometheus ሜትሪክስ (ስያሜዎች) ያጋልጣሉ።
`cluster`፣ `storage_class`፡

- `torii_da_rent_gib_months_total` - GiB-ወሮች በ `/v1/da/ingest` የተጠቀሱ።
- `torii_da_rent_base_micro_total` — መነሻ ኪራይ (ማይክሮ XOR) በመግቢያው ላይ ተከማችቷል።
- `torii_da_protocol_reserve_micro_total` - የፕሮቶኮል መጠባበቂያ መዋጮዎች።
- `torii_da_provider_reward_micro_total` - የአቅራቢ-ጎን ኪራይ ክፍያዎች።
- `torii_da_pdp_bonus_micro_total` እና `torii_da_potr_bonus_micro_total` -
  የ PDP/PoTR የጉርሻ ገንዳዎች ከመግቢያው ጥቅስ የተገኙ።

የኢኮኖሚክስ ዳሽቦርዶች የሂሳብ ደብተር አይኤስአይኤስ፣ የተጠባባቂ ቧንቧዎች፣
እና የ PDP/PoTR የጉርሻ መርሃ ግብሮች ሁሉም ለእያንዳንዳቸው ከሚተገበሩት የፖሊሲ መለኪያዎች ጋር ይዛመዳሉ
ክላስተር እና የማከማቻ ክፍል. የ SoraFS አቅም ጤና Grafana ሰሌዳ
(`dashboards/grafana/sorafs_capacity_health.json`) አሁን የወሰኑ ፓነሎችን ያቀርባል
ለኪራይ ማከፋፈያ፣ ለ PDP/PoTR የጉርሻ ክምችት፣ እና የጂቢ-ወር ቀረጻ፣ በመፍቀድ
ኢንጂስትን ሲገመግሙ በTorii ክላስተር ወይም ማከማቻ ክፍል የሚጣራ ግምጃ ቤት
መጠን እና ክፍያዎች.

## ቀጣይ እርምጃዎች

- ✅ `/v1/da/ingest` ደረሰኞች አሁን `rent_quote` እና የCLI/SDK ንጣፎች የተጠቀሰውን ያሳያሉ።
  መነሻ ኪራይ፣ የተጠባባቂ ድርሻ እና የ PDP/PoTR ቦነስ አስገቢዎች የXOR ግዴታዎችን ከዚህ በፊት መገምገም ይችላሉ።
  ሸክሞችን መፈጸም.
- የኪራይ ደብተርን ከሚመጣው የDA ዝና/የመጽሐፍ ምግቦች ጋር ያዋህዱ
  ከፍተኛ አቅርቦት አቅራቢዎች ትክክለኛ ክፍያዎችን እየተቀበሉ መሆኑን ለማረጋገጥ።