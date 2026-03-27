---
lang: am
direction: ltr
source: docs/source/finance/repo_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 42c328443065e102a65180421d515e4e3040a35175c348ea25fd83edab1236b4
source_last_modified: "2026-01-22T16:26:46.567961+00:00"
translation_last_reviewed: 2026-02-07
title: Repo Operations & Evidence Guide
summary: Governance, lifecycle, and audit requirements for repo/reverse-repo flows (roadmap F1).
translator: machine-google-reviewed
---

# ሪፖ ኦፕሬሽኖች እና የማስረጃ መመሪያ (Roadmap F1)

የሪፖ ፕሮግራሙ የሁለትዮሽ እና የሶስትዮሽ ፋይናንስን በቆራጥነት ይከፍታል።
Norito መመሪያዎች፣ CLI/SDK አጋዥ እና ISO 20022 እኩልነት። ይህ ማስታወሻ ይይዛል
የመንገድ ካርታ ወሳኝ ምዕራፍ ለማርካት የሚያስፈልገው የስራ ውል **F1 — repo
የህይወት ዑደት ሰነዶች እና መሳሪያዎች ***። የስራ ፍሰት-ተኮርን ያሟላል።
[`repo_runbook.md`](./repo_runbook.md) በመግለጽ፡-

- የህይወት ዑደት ወለሎች በCLI/SDKs/በአሂድ ጊዜ (`crates/iroha_cli/src/main.rs:3821`፣
  `python/iroha_python/iroha_python_rs/src/lib.rs:2216`፣
  `crates/iroha_core/src/smartcontracts/isi/repo.rs:1`);
- የመወሰን ማረጋገጫ / ማስረጃ ቀረጻ (`integration_tests/tests/repo.rs:1`);
- የሶስትዮሽ ወገን ጥበቃ እና የመተካት ባህሪ; እና
- የአስተዳደር የሚጠበቁ (ባለሁለት-ቁጥጥር, የኦዲት ዱካዎች, የመልሶ ማጫወቻ መጽሐፍት).

## 1. ወሰን እና ተቀባይነት መስፈርቶች

የመንገድ ካርታ ንጥል F1 በአራት ጭብጦች ላይ እንደተዘጋ ይቆያል; ይህ ሰነድ ይዘረዝራል
የሚያስፈልጉ ቅርሶች እና አገናኞች ወደ ኮድ/ሙከራዎች አስቀድመው ያረካቸው፡-

| መስፈርት | ማስረጃ |
|------------|------|
| Repo → በግልባጭ repo → መተካት |. የሚሸፍኑ ቆራጥ የሰፈራ ማረጋገጫዎች `integration_tests/tests/repo.rs` ከጫፍ እስከ ጫፍ ፍሰቶችን፣ የተባዙ መታወቂያ ጠባቂዎችን፣ የኅዳግ ማረጋገጫ ቼኮችን እና በዋስትና የመተካት ስኬት/ውድቀት ጉዳዮችን ይይዛል። ስብስቡ እንደ `cargo test --workspace` አካል ነው የሚሰራው። በ`crates/iroha_core/src/smartcontracts/isi/repo.rs` (`repo_deterministic_lifecycle_proof_matches_fixture`) ቅጽበታዊ ገጽ እይታ ጅማሬ → ህዳግ → መተኪያ ክፈፎች ላይ ኦዲተሮች ቀኖናዊውን የደመወዝ ጭነቶች እንዲለያዩ የሚወስነው የህይወት ኡደት መፍጨት ታጥቆ። |
| የሶስትዮሽ ፓርቲ ሽፋን | የሩጫ ጊዜ ጠባቂ-አዋቂ ፍሰቶችን ያስገድዳል፡ `RepoAgreement::custodian` + `RepoAccountRole::Custodian` ክስተቶች (`crates/iroha_data_model/src/repo.rs:74`፣ `crates/iroha_data_model/src/events/data/events.rs:742`)። |
| የዋስትና ምትክ ፈተናዎች | የተገላቢጦሽ እግር ተገላቢጦሽ በዋስትና ያልተያዙ መተኪያዎችን (`crates/iroha_core/src/smartcontracts/isi/repo.rs:417`) እና የውህደት ሙከራዎችን ከተተካ ጉዞ (`integration_tests/tests/repo.rs:261`) በኋላ የሂሳብ ደብተሩ በትክክል እንደሚጸዳ ያረጋግጣሉ። |
| የኅዳግ-ጥሪ ማስረጃ እና የተሳታፊ ማስፈጸሚያ | `integration_tests/tests/repo.rs::repo_margin_call_enforces_cadence_and_participant_rules` ልምምዶች `RepoMarginCallIsi`፣ ከድነት ጋር የተጣጣመ መርሐግብርን የሚያረጋግጥ፣ ያለጊዜው ጥሪዎችን አለመቀበል እና የተሳታፊ-ብቻ ፍቃድ። |
| በመንግስት የተፈቀዱ runbooks | ይህ መመሪያ እና `repo_runbook.md` የCLI/SDK ሂደቶችን፣ የማጭበርበር/የመመለሻ እርምጃዎችን እና የማስረጃ ቀረጻ መመሪያዎችን ለኦዲት ያቀርባሉ። |

## 2. የህይወት ኡደት ወለሎች

### 2.1 CLI & Norito ግንበኞች

- `iroha app repo initiate|unwind|margin|margin-call` መጠቅለያ `RepoIsi`፣
  `ReverseRepoIsi`፣ እና `RepoMarginCallIsi`
  (`crates/iroha_cli/src/main.rs:3821`)። እያንዳንዱ ንዑስ ትዕዛዝ `--input` / ይደግፋል
  `--output` ስለዚህ ጠረጴዛዎች ለድርብ መጽደቅ የማስተማሪያ ጭነቶችን ደረጃ እንዲያዘጋጁ
  ማስረከብ። ጠባቂ ማዘዋወር የሚገለጸው በ`--custodian` ነው።
- `repo query list|get` ስምምነቶችን ለማንሳት `FindRepoAgreements` ይጠቀማል እና ይችላል
  ለማስረጃ ቅርቅቦች ወደ JSON ቅርሶች ይዛወሩ።
- የ CLI ጭስ በ `crates/iroha_cli/tests/cli_smoke.rs:2637` ያረጋግጣሉ
  ወደ ፋይል የሚለቀቅበት መንገድ ለኦዲተሮች የተረጋጋ ይቆያል።

### 2.2 ኤስዲኬዎች እና አውቶሜሽን መንጠቆዎች- የፓይዘን ማሰሪያዎች `RepoAgreementRecord`፣ `RepoCashLeg`፣
  `RepoCollateralLeg`፣ እና ምቹ ግንበኞች
  (`python/iroha_python/iroha_python_rs/src/lib.rs:2216`) ስለዚህ አውቶማቲክ ማድረግ ይችላል።
  ግብይቶችን ያሰባስቡ እና `next_margin_check_after` በአገር ውስጥ ይገምግሙ።
- JS/Swift ረዳቶች ተመሳሳዩን Norito አቀማመጦችን እንደገና ይጠቀማሉ።
  `javascript/iroha_js/src/instructionBuilders.js` እና
  `IrohaSwift/Sources/IrohaSwift/ConfidentialEncryptedPayload.swift` ለማስታወሻ
  አያያዝ; ኤስዲኬዎች የሪፖ አስተዳደር ቁልፎችን በሚሰሩበት ጊዜ ይህንን ሰነድ መጥቀስ አለባቸው።

### 2.3 የሂሳብ መዝገብ ዝግጅቶች እና ቴሌሜትሪ

እያንዳንዱ የህይወት ኡደት እርምጃ የ `AccountEvent::Repo(...)` መዝገቦችን ያወጣል።
`RepoAccountEvent::{Initiated,Settled,MarginCalled}` የሚጫኑ ጭነቶች ለ
የተሳታፊ ሚና (`crates/iroha_data_model/src/events/data/events.rs:742`)። ግፋ
ግልጽ የሆነ የኦዲት መዝገብ ለማግኘት እነዚያ ክስተቶች በእርስዎ SIEM/Log Aggregator ውስጥ
ለዴስክ ድርጊቶች፣ የኅዳግ ጥሪዎች እና የጥበቃ ማሳወቂያዎች።

### 2.4 የማዋቀር ስርጭት እና ማረጋገጫ

አንጓዎች የዳግም አስተዳደር ቁልፎችን ከ `[settlement.repo]` ስታንዛ ውስጥ ያስገቡ
`iroha_config` (`crates/iroha_config/src/parameters/user.rs:4071`)። ያንን ማከም
ቅንጥስ እንደ የአስተዳደር ማስረጃ ውል - በስሪት ቁጥጥር ውስጥ ያድርጉት
ከሪፖ ፓኬት ጎን ለጎን እና ለውጡን በእርስዎ በኩል ከመግፋትዎ በፊት ሃሽ ያድርጉት
አውቶሜሽን ወይም ConfigMap. አነስተኛ መገለጫ እንደዚህ ይመስላል

```toml
[settlement.repo]
default_haircut_bps = 1500
margin_frequency_secs = 86400
eligible_collateral = ["4fEiy2n5VMFVfi6BzDJge519zAzg", "7dk8Pj8Bqo6XUqch4K2sF8MCM1zd"]

[settlement.repo.collateral_substitution_matrix]
"4fEiy2n5VMFVfi6BzDJge519zAzg" = ["7dk8Pj8Bqo6XUqch4K2sF8MCM1zd", "6zK1LDcJ3FvkpfoZQ8kHUaW6sA7F"]
```

የተግባር ማረጋገጫ ዝርዝር፡

1. ከላይ ያለውን ቅንጣቢ (ወይም የእርስዎን የምርት ልዩነት) ወደ ውቅር ሪፖው ያስገቡ
   `irohad` የሚመገብ እና SHA-256ን በአስተዳደር ፓኬት ውስጥ ይመዘግባል።
   ገምጋሚዎች ለማሰማራት ያቀዱትን ባይት ሊለያዩ ይችላሉ።
2. ለውጡን በመርከብ ላይ አዙረው (የስርዓት ክፍል፣ Kubernetes ConfigMap፣ ወዘተ.)
   እና እያንዳንዱን አንጓ እንደገና ያስጀምሩ. ከታቀዱ በኋላ ወዲያውኑ Torii ን ይያዙ
   የማዋቀር ቅጽበታዊ ገጽ እይታ ለፕሮቬንሽን፡

   ```bash
   curl -sS "${TORII_URL}/v1/configuration" \
     -H "Authorization: Bearer ${TOKEN}" | jq .
   ```

   `ToriiClient.get_configuration()` ለተመሳሳይ በፓይዘን ኤስዲኬ ይገኛል።
   ዓላማ አውቶሜሽን የተተየበ ማስረጃ ሲፈልግ።【python/iroha_python/src/iroha_python/client.py:5791】
3. የሩጫ ጊዜው አሁን የተጠየቀውን የፀጉር አቆራረጥ በመጠየቅ የሚያስፈጽም መሆኑን ያረጋግጡ
   `FindRepoAgreements` (ወይም `iroha app repo margin --agreement-id ...`) እና
   የተከተቱ `RepoGovernance` እሴቶችን በመፈተሽ ላይ። የJSON ምላሾችን ያከማቹ
   በ `artifacts/finance/repo/<agreement>/agreements_after.json`; እነዚያ እሴቶች
   ከ `[settlement.repo]` የተገኙ ናቸው, ስለዚህ መቼ እንደ ሁለተኛ ምስክር ሆነው ያገለግላሉ
   የTorii's `/v1/configuration` ቅጽበታዊ ገጽ እይታ በቂ አይደለም።
4. ሁለቱንም ቅርሶች-የTOML ቅንጭብጭብ እና የTorii/CLI ቅጽበተ-ፎቶዎችን በ
   የአስተዳደር ጥያቄ ከማቅረቡ በፊት የማስረጃ ጥቅል። ኦዲተሮች መቻል አለባቸው
   ቅንጣቢውን ድጋሚ አጫውት፣ ሃሽውን አረጋግጥ፣ እና ከሩጫ ጊዜ እይታ ጋር አገናኘው።

ይህ የስራ ፍሰት የሪፖ ዴስኮች በማስታወቂያ-አከባቢ ተለዋዋጮች ላይ በጭራሽ እንደማይተማመኑ ያረጋግጣል
የማዋቀር መንገድ ቆራጥ ሆኖ ይቆያል፣ እና እያንዳንዱ የአስተዳደር ትኬት እነዚህን ይሸከማል
በመንገድ ካርታ F1 ውስጥ የሚጠበቁ ተመሳሳይ የ`iroha_config` ማረጋገጫዎች ስብስብ።

### 2.5 ቆራጥ የማረጋገጫ ቀበቶ

የአሃዱ ሙከራ `repo_deterministic_lifecycle_proof_matches_fixture` (ተመልከት
`crates/iroha_core/src/smartcontracts/isi/repo.rs`) እያንዳንዱን ደረጃ በተከታታይ ያቀርባል
repo lifecycle ወደ Norito JSON ፍሬም ፣ከቀኖናዊው መግጠሚያ ጋር ያወዳድራል።
`crates/iroha_core/tests/fixtures/repo_lifecycle_proof.json`፣ እና hashes the
ጥቅል (የቋሚ መፍጨት ሂደት በ
`crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`)። በአካባቢው ያሂዱት በ:

```bash
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```ይህ ሙከራ አሁን እንደ ነባሪ `cargo test -p iroha_core` ስብስብ አካል ሆኖ ይሰራል፣ ስለዚህ CI
ቅጽበተ-ፎቶውን በራስ-ሰር ይጠብቃል። የትርጉም ጽሑፎች ወይም ዕቃዎች ሲቀየሩ፣
ሁለቱንም JSON ያድሱ እና በ:

```bash
scripts/regen_repo_proof_fixture.sh
```

ረዳቱ የተሰካውን `rust-toolchain.toml` ቻናል ይጠቀማል፣ መጋጠሚያዎቹን እንደገና ይጽፋል
በ `crates/iroha_core/tests/fixtures/` ስር፣ እና የመወሰኛ መታጠቂያውን እንደገና ያስኬዳል
ስለዚህ ተመዝግቦ የገባው ቅጽበታዊ ገጽ እይታ/መፍጨት ከሩጫ ጊዜ ባህሪ ጋር እንደተስማማ ይቆያል
ኦዲተሮች እንደገና ይጫወታሉ.

### 2.4 Torii ኤፒአይ ወለሎች

- `GET /v1/repo/agreements` ገባሪ ስምምነቶችን ከአማራጭ ገፅ ጋር በማጣራት ይመልሳል
  (`filter={...}`)፣ መደርደር እና የአድራሻ-ቅርጸት መለኪያዎች። ይህንን ለፈጣን ኦዲት ይጠቀሙ ወይም
  ጥሬው የJSON ክፍያ ጭነቶች በቂ ሲሆኑ ዳሽቦርዶች።
- `POST /v1/repo/agreements/query` የተዋቀረውን የመጠይቅ ፖስታ ይቀበላል (ገጽታ፣ ደርድር፣
  `FilterExpr`፣ `fetch_size`) ስለዚህ የታችኛው ተፋሰስ አገልግሎቶች በሒሳብ ደብተር ውስጥ በቁርጠኝነት ገጹን ማግኘት ይችላሉ።
- የጃቫ ስክሪፕት ኤስዲኬ አሁን `listRepoAgreements`፣ `queryRepoAgreements` እና ተደጋጋሚውን ያጋልጣል
  helpers so browser/Node.js tooling ልክ እንደ Rust/Python የተተየቡ DTOዎችን ይቀበላል።

### 2.4 የማዋቀር ነባሪዎች

አንጓዎች `[settlement.repo]` ወደ ውስጥ ያነባሉ።
`iroha_config::parameters::actual::Repo` በሚነሳበት ጊዜ; ማንኛውም repo መመሪያ
መለኪያውን በዜሮ የሚተው ከሱ በፊት ባሉት ነባሪዎች ላይ የተለመደ ነው።
በሰንሰለት ላይ ተመዝግቧል።【crates/iroha_core/src/smartcontracts/isi/repo.rs:40】 ይህ
እያንዳንዱን ኤስዲኬ ሳይነካ አስተዳደር የመነሻ ፖሊሲን ከፍ እንዲያደርግ (ወይም ዝቅ እንዲል) ያስችላል
የጥሪ ጣቢያ፣ የፖሊሲ ለውጡ ሙሉ በሙሉ ከተመዘገበ።

- `default_haircut_bps` - `RepoGovernance::haircut_bps()` ጊዜ ወደ ኋላ የሚመለስ ፀጉር
  ከዜሮ ጋር እኩል ነው። የሩጫ ሰዓቱ ለማቆየት ከጠንካራው 10000bps ጣሪያ ጋር ይጨምረዋል።
  ጤናማ ያዋቅራል።【crates/iroha_core/src/smartcontracts/isi/repo.rs:44】
- `margin_frequency_secs` - ለ `RepoMarginCallIsi` cadence. የዜሮ ጥያቄዎች
  ይህንን እሴት ይውረሱ ፣ ስለሆነም የድጋፍ ማሳጠር ጠረጴዛዎች የበለጠ እንዲለያዩ ያስገድዳቸዋል።
  በተደጋጋሚ በነባሪ።【crates/iroha_core/src/smartcontracts/isi/repo.rs:49】
- `eligible_collateral` - የ `AssetDefinitionId`s አማራጭ የፍቃድ ዝርዝር። መቼ
  ዝርዝሩ ባዶ አይደለም `RepoIsi` ማንኛውንም ቃል ኪዳን ውድቅ በማድረግ ይከላከላል
  ያልተጣራ ቦንዶች ድንገተኛ መሳፈር።【crates/iroha_core/src/smartcontracts/isi/repo.rs:57】
- `collateral_substitution_matrix` - የዋናው መያዣ ካርታ →
  የተፈቀዱ ተተኪዎች. `ReverseRepoIsi` ምትክ የሚቀበለው የ
  ማትሪክስ የተቀዳውን ፍቺ እንደ ቁልፍ እና በእሱ ውስጥ ያለውን ምትክ ይዟል
  የእሴት ድርድር; ያለበለዚያ ፍጥነቱ አልተሳካም ፣ ይህም አስተዳደር እንደፈቀደ ያረጋግጣል
  መሰላል።【ክራተስ/iroha_core/src/smartcontracts/isi/repo.rs:74】

እነዚህ እንቡጦች በመስቀለኛ ውቅረት ውስጥ በ`[settlement.repo]` ስር ይኖራሉ እና ናቸው።
በ`iroha_config::parameters::user::Repo` በኩል የተተነተነ፣ ስለዚህ እነሱ መያዛቸው አለባቸው
እያንዳንዱ የአስተዳደር ማስረጃ ጥቅል።【crates/iroha_config/src/parameters/user.rs:3956】

```toml
[settlement.repo]
default_haircut_bps = 1750
margin_frequency_secs = 43200
eligible_collateral = ["4fEiy2n5VMFVfi6BzDJge519zAzg", "7dk8Pj8Bqo6XUqch4K2sF8MCM1zd"]

[settlement.repo.collateral_substitution_matrix]
"4fEiy2n5VMFVfi6BzDJge519zAzg" = ["7dk8Pj8Bqo6XUqch4K2sF8MCM1zd", "6zK1LDcJ3FvkpfoZQ8kHUaW6sA7F"]
```

** የአስተዳደር ለውጥ ዝርዝር**1. የታቀደውን የTOML ቅንጭብ (መተካት ማትሪክስ ዴልታዎችን ጨምሮ)፣ ሃሽ ደረጃ ይስጡ
   ከSHA-256 ጋር እና ሁለቱንም ቅንጣቢ እና ሃሽ ከአስተዳደር ጋር ያያይዙት።
   ቲኬት ስለዚህ ገምጋሚዎች ባይት በቃል ማባዛት ይችላሉ።
2. ቅንጣቢውን በፕሮፖዛል/ህዝበ ውሳኔ (ለምሳሌ በ
   በአስተዳደር CLI ላይ `--notes` መስክ እና አስፈላጊውን ማጽደቆችን ሰብስብ
   ለ F1. የተፈረመውን የማረጋገጫ ፓኬት ቅንጣቢው በማያያዝ ያቆዩት።
3. ለውጡን በመርከብ ላይ አዙረው፡ `[settlement.repo]`ን አዘምን፣ እያንዳንዱን እንደገና አስጀምር
   መስቀለኛ መንገድ፣ ከዚያ `GET /v1/configuration` ቅጽበተ ፎቶን ያንሱ (ወይም
   `ToriiClient.getConfiguration`) በአንድ አቻ የተተገበሩ እሴቶችን ማረጋገጥ።
4. `integration_tests/tests/repo.rs` ፕላስ እንደገና አሂድ
   `repo_deterministic_lifecycle_proof_matches_fixture` እና ምዝግብ ማስታወሻዎቹን በሚቀጥለው ያከማቹ
   ኦዲተሮች አዲሶቹ ነባሪዎች እንደሚጠብቁ ለማየት ወደ ውቅረት ልዩነት
   ቆራጥነት.

የማትሪክስ ግቤት ከሌለ የሩጫ ጊዜው ንብረቱን የሚቀይሩ ምትክዎችን ውድቅ ያደርጋል
ፍቺ, ምንም እንኳን አጠቃላይ `eligible_collateral` ዝርዝር ቢፈቅድም; መፈጸም
ኦዲተሮች ትክክለኛውን ነገር እንደገና ማባዛት እንዲችሉ የ config ቅጽበተ-ፎቶዎችን ከሪፖ ማስረጃ ጋር ያዋቅሩ
ሪፖ በተያዘበት ጊዜ ፖሊሲ ተፈጻሚ ይሆናል።

### 2.5 የውቅር ማስረጃ እና ተንሸራታች ማወቅ

የNorito/`iroha_config` የቧንቧ ስራ አሁን በስር ያለውን መፍትሄ ያጋልጣል።
`iroha_config::parameters::actual::Repo`፣ ስለዚህ የአስተዳደር እሽጎች ማረጋገጥ አለባቸው
የተተገበሩ እሴቶች በአንድ አቻ - የታቀደው TOML ብቻ አይደለም። የተፈታውን ይያዙ
ውቅር እና ከእያንዳንዱ የታቀደ ልቀት በኋላ መሟጠጡ፡-

1. አወቃቀሩን ከእያንዳንዱ አቻ (`GET /v1/configuration` ወይም
   `ToriiClient.getConfiguration`) እና ሪፖ ስታንዛን ለይ:

   ```bash
   curl -s http://<torii-host>/v1/configuration \
     | jq -cS '.settlement.repo' \
     > artifacts/finance/repo/<agreement-id>/config/repo_config_actual.json
   ```

2. ቀኖናዊውን JSON Hash እና በማስረጃው ውስጥ አስመዝግቡት። መቼ
   መርከቦች ጤናማ ነው ሃሽ ከእኩዮች ጋር መመሳሰል አለበት ምክንያቱም `actual`
   ነባሪዎችን ከደረጃ `[settlement.repo]` ቅንጣቢ ጋር ያጣምራል።

   ```bash
   shasum -a 256 artifacts/finance/repo/<agreement-id>/config/repo_config_actual.json
   ```

3. JSON + hashን ከአስተዳደር ፓኬት ጋር ያያይዙ እና በ ውስጥ ያለውን ግቤት ያንጸባርቁት
   አንጸባራቂ ወደ አስተዳደር DAG ተሰቅሏል። ማንኛውም እኩያ የተለየ ሪፖርት ካደረገ
   መፍጨት፣ መልቀቅን አቁም እና ከዚህ በፊት የውቅረት/ግዛት ድሪፍትን አስታርቁ
   መቀጠል.

### 2.6 የአስተዳደር ማፅደቂያ እና ማስረጃ ጥቅል

የመንገድ ካርታ F1 የሚዘጋው ሪፖ ዴስኮች የተወሰነ ፓኬት ወደ ውስጥ ሲገቡ ብቻ ነው።
አስተዳደር DAG፣ ስለዚህ እያንዳንዱ ለውጥ (አዲስ ፀጉር፣ ጠባቂ ፖሊሲ፣ ወይም መያዣ
ማትሪክስ) ድምጽ ከመቅረቡ በፊት ተመሳሳይ ቅርሶችን መላክ አለበት።【docs/source/governance_playbook.md:1】

** የመቀበያ ፓኬት ***1. ** የመከታተያ አብነት *** - ቅጂ
   `docs/examples/finance/repo_governance_packet_template.md` ወደ ማስረጃዎ
   ማውጫ (ለምሳሌ
   `artifacts/finance/repo/<agreement-id>/packet.md`) እና ሜታዳታውን ይሙሉ
   ቅርሶችን ማሰር ከመጀመርዎ በፊት ያግዱ። አብነት አስተዳደሩን ይጠብቃል
   የፋይል መንገዶችን፣ SHA-256 ጨጓራዎችን በመዘርዘር፣ እና
   የገምጋሚ ምስጋናዎች በአንድ ቦታ።
2. **የመመሪያ ጭነቶች** - ጅምር ደረጃ፣ ንፋስ ፈታ እና የኅዳግ ጥሪ
   መመሪያዎች ከ `iroha app repo ... --output` ጋር ስለዚህ ባለሁለት መቆጣጠሪያ አጽዳቂዎች ግምገማ
   ባይት-ተመሳሳይ ሸክሞች። እያንዳንዱን ፋይል ያሽጉ እና ከስር ያከማቹ
   `artifacts/finance/repo/<agreement-id>/` ከጠረጴዛው የማስረጃ ጥቅል ቀጥሎ
   በዚህ ማስታወሻ ውስጥ ሌላ ቦታ ተጠቅሷል።【crates/iroha_cli/src/main.rs:3821】
3. ** የማዋቀር ልዩነት *** - ትክክለኛውን `[settlement.repo]` TOML ቅንጭብ ያካትቱ
   (ነባሪዎች እና ምትክ ማትሪክስ) እና የእሱ SHA-256። ይህ የትኛውን ያረጋግጣል
   `iroha_config` ማዞሪያዎች ድምጹ ካለፈ በኋላ ንቁ ይሆናሉ
   በመግቢያ ጊዜ የሪፖ መመሪያዎችን መደበኛ የሚያደርጉ የሩጫ መስኮች።【crates/iroha_config/src/parameters/user.rs:3956】
4. ** ቆራጥ ሙከራዎች *** - የቅርብ ጊዜውን ያያይዙ
   `integration_tests/tests/repo.rs` ሎግ እና ውፅዓት ከ
   `repo_deterministic_lifecycle_proof_matches_fixture` ስለዚህ ገምጋሚዎች ያዩታል።
   ከተዘጋጁት መመሪያዎች ጋር የሚዛመድ የህይወት ኡደት ማረጋገጫ ሃሽ።【Integration_tests/tests/repo.rs:1】【crates/iroha_core/src/smartcontracts/isi/repo.rs:1450】
5. ** የክስተት/የቴሌሜትሪ ቅጽበታዊ ገጽ እይታ** - የቅርብ ጊዜውን `AccountEvent::Repo(*)` ወደ ውጭ ይላኩ
   ለጠረጴዛዎቹ ስፋት እና ማንኛውም ዳሽቦርዶች/ሜትሪክስ ምክር ቤቱ የሚፈልጋቸውን መለኪያዎች ይልቀቁ
   አደጋን ለመዳኘት (ለምሳሌ የኅዳግ ተንሸራታች)። ይህ ኦዲተሮችን ተመሳሳይ ይሰጣል
   ግልጽ ያልሆነ መዝገብ ከTorii በኋላ እንደገና ይገነባሉ።【crates/iroha_data_model/src/events/data/events.rs:742】

** ማጽደቅ እና መግባት ***

- በአስተዳደር ትኬት ወይም በሪፈረንደም ውስጥ ያሉትን አርቲፊሻል ሃሽዎች ይመልከቱ እና
  ምክር ቤቱ መደበኛውን ሥነ ሥርዓት መከተል እንዲችል ከተዘጋጀው ፓኬት ጋር ማገናኘት
  የአድ-ሆክ መንገዶችን ሳያሳድዱ በአስተዳደር መጫወቻ መጽሐፍ ውስጥ ተዘርዝሯል።【docs/source/governance_playbook.md:8】
- የትኛዎቹ ባለሁለት መቆጣጠሪያ ፈራሚዎች የታቀዱትን የማስተማሪያ ፋይሎችን እንደገመገሙ እና
  ምስጋናቸውን ከሃሽዎች አጠገብ ያከማቹ; ይህ በሰንሰለት ላይ ያለው ማስረጃ ነው።
  ያ የሪፖ ዴስኮች የ"ሁለት ሰው ህግ" ያረኩ ቢሆንም ምንም እንኳን የሩጫ ሰዓቱ
  የተሳታፊ-ብቻ አፈፃፀምን ያስገድዳል።
- ምክር ቤቱ የአስተዳደር ማፅደቂያ መዝገብ (GAR) ሲያትም፣ ያንጸባርቁት
  በማረጃ መዝገብ ውስጥ የተፈረመ ደቂቃዎች ስለዚህ የወደፊት ምትክ ወይም
  የፀጉር ማስተካከያ ማሻሻያ ትክክለኛውን የውሳኔ ፓኬት እንደገና ከመመለስ ይልቅ ሊጠቅስ ይችላል
  ምክንያታዊነት.

** የድህረ ማጽደቂያ ልቀቶች**1. የተፈቀደውን የ`[settlement.repo]` ውቅረት ይተግብሩ እና እያንዳንዱን መስቀለኛ መንገድ (ወይም ጥቅልል) እንደገና ያስጀምሩ
   በራስ-ሰርዎ በኩል ነው)። ወዲያውኑ `GET /v1/configuration` ይደውሉ እና በማህደር ያስቀምጡ
   ምላሹ በእያንዳንዱ መስቀለኛ መንገድ ስለዚህ የአስተዳደር ጥቅል የትኞቹ እኩዮች እንደተቀበሉ ያሳያል
   ለውጥ።【crates/iroha_torii/src/lib.rs:3225】
2. የመወሰኛ ሪፖ ሙከራዎችን እንደገና ያስኪዱ እና ትኩስ ምዝግብ ማስታወሻዎችን እና ግንባታን ያያይዙ
   ሜታዳታ (ጂት ቁርጠኝነት፣ የመሳሪያ ሰንሰለት) ስለዚህ ኦዲተሮች ሰፈራውን እንደገና ማባዛት ይችላሉ።
   ከታቀዱ በኋላ ማረጋገጫ.
3. የአስተዳደር መከታተያውን በማስረጃ መዝገብ ዱካ፣ ሃሽ እና ያዘምኑ
   የተመልካች ግንኙነት ስለዚህ በኋላ repo ዴስኮች በምትኩ ተመሳሳይ ሂደት ይወርሳሉ ይችላሉ
   የማረጋገጫ ዝርዝሩን እንደገና ማግኘት.

**የመንግስት DAG ህትመት (የሚያስፈልግ)**

1. የማስረጃውን ማውጫ (የማዋቀር ቅንጣቢ፣ የመመሪያ ጭነቶች፣ የማረጋገጫ ምዝግቦች፣
   GAR/ደቂቃ) እና ለአስተዳደር DAG ቧንቧ እንደ ሀ
   `GovernancePayloadKind::PolicyUpdate` ክፍያ ከማብራሪያ ጋር
   `agreement_id`, `iso_week`, እና የታቀደው የፀጉር / የትርፍ ዋጋዎች; የ
   pipeline spec እና CLI ንጣፎች ይኖራሉ
   `docs/source/sorafs_governance_dag_plan.md`.
2. አታሚው የአይፒኤንኤስን ጭንቅላት ካዘመነ በኋላ የማገጃውን CID እና ዋና CID ይቅረጹ
   በአስተዳደር መከታተያ እና በ GAR ማንም ሰው የማይለዋወጥን ማምጣት ይችላል።
   ፓኬት በኋላ. `sorafs governance dag head` እና `sorafs governance dag list`
   ድምጹ ከመከፈቱ በፊት መስቀለኛ መንገድ መያያዙን ያረጋግጡ።
3. የ CAR ፋይልን ያከማቹ ወይም ከሪፖ ማስረጃ ማህደር ቀጥሎ ያለውን ክፍያ ያግዱ
   ኦዲተሮች በሰንሰለት ላይ ያለውን የአስተዳደር ውሳኔ ከትክክለኛው ጋር ማስታረቅ ይችላሉ።
   ከሰንሰለት ውጪ የተፈቀደ ፓኬት።

### 2.7 የህይወት ዑደት ቅጽበተ-ፎቶ አድስ

የሪፖ ትርጉም ሲቀየር (ተመን፣ የሰፈራ ሂሳብ፣ የጥበቃ አመክንዮ፣ ወይም
ነባሪ ውቅር)፣ አስተዳደር እንዲቻል ወሳኙ የህይወት ኡደት ቅጽበታዊ ገጽ እይታን ያድሱ
የማረጋገጫ ማሰሪያውን ሳይገለበጥ አዲሱን መፈጨት ጥቀስ።

1. በተሰካው የመሳሪያ ሰንሰለት ስር ያሉትን እቃዎች ያድሱ፡

   ```bash
   scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
     --bundle-dir artifacts/finance/repo/<agreement>
   ```

   ረዳቱ በቴምፕ ዳይሬክተሩ ውስጥ ደረጃዎችን ያወጣል፣ ክትትል የሚደረግባቸውን ዕቃዎች ያዘምናል።
   በ `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.{json,digest}`፣
   ለማረጋገጫ የማረጋገጫ ሙከራውን እንደገና ያካሂዳል፣ እና (`--bundle-dir` ሲቀናበር)
   `repo_proof_snapshot.json` እና `repo_proof_digest.txt` ወደ ጥቅል ይጥላል
   ለኦዲተሮች ማውጫ.
2. የተከታተሉትን እቃዎች ሳይነኩ ቅርሶችን ወደ ውጭ ለመላክ (ለምሳሌ, ደረቅ ሩጫ
   ማስረጃ) ፣ የኢንቪ ረዳቶችን በቀጥታ ያዘጋጁ፡-

   ```bash
   REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<agreement>/repo_proof_snapshot.json \
   REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<agreement>/repo_proof_digest.txt \
   cargo test -p iroha_core \
     -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
   ```

   `REPO_PROOF_SNAPSHOT_OUT` ያማረውን Norito JSON ከማስረጃው ይቀበላል
   `REPO_PROOF_DIGEST_OUT` አቢይ ሆክስ ዳይጄስት ሲያከማች (ከ
   ለምቾት ሲባል አዲስ መስመርን መከተል)። ረዳቱ መቼ ፋይሎችን ለመፃፍ ፈቃደኛ አይሆንም
   የወላጅ ማውጫው የለም፣ ስለዚህ መጀመሪያ `artifacts/...` ዛፍ ይገንቡ።
3. ሁለቱንም ወደ ውጭ የተላኩ ፋይሎችን ከስምምነቱ ጥቅል ጋር ያያይዙ (§3 ይመልከቱ) እና እንደገና ያድሱ
   አንጸባራቂው በ`scripts/repo_evidence_manifest.py` ስለዚህ የአስተዳደር ፓኬት
   የታደሱትን የማረጋገጫ ቅርሶች በግልፅ ይጠቅሳል። የውስጠ-ግንባታ ዕቃዎች
   ለ CI የእውነት ምንጭ ይቆዩ.

### 2.8 የወለድ ክምችት እና የብስለት አስተዳደር** ቁርጥ ያለ የወለድ ሂሳብ።** `RepoIsi` እና `ReverseRepoIsi` ገንዘቡን ያገኛሉ
ከኤሲቲ/360 አጋዥ በእረፍት ጊዜ ዕዳ አለበት።
`compute_accrued_interest()`【crates/iroha_core/src/smartcontracts/isi/repo.rs:100】
እና የመክፈያ እግሮችን የማይቀበለው በ `expected_cash_settlement()` ውስጥ ያለው ጠባቂ
ከ *ዋና + ወለድ* ያነሰ የሚመለስ።【crates/iroha_core/src/smartcontracts/isi/repo.rs:132】
ረዳቱ `rate_bps` ወደ ባለ አራት አስርዮሽ ክፍልፋይ መደበኛ ያደርገዋል፣ ያባዛዋል።
`elapsed_ms / (360 * 24h)` 18 አስርዮሽ ቦታዎችን በመጠቀም እና በመጨረሻም ወደ
በጥሬ ገንዘብ እግር `NumericSpec` የተገለጸ ልኬት። የአስተዳደር ፓኬጁን ለማቆየት
ሊባዛ የሚችል፣ ረዳቱን የሚመግቡትን አራቱን እሴቶች ይያዙ፡-

1. `cash_leg.quantity` (ዋና)
2. `rate_bps`፣
3. `initiated_timestamp_ms`, እና
4. ሊጠቀሙበት ያሰቡትን የማራገፊያ ጊዜ ማህተም (ለታቀዱ የጂኤል ግቤቶች ይህ ነው።
   ብዙውን ጊዜ `maturity_timestamp_ms` ፣ ግን የአደጋ ጊዜ ነፋሶች ትክክለኛውን ይመዘግባሉ
   `ReverseRepoIsi::settlement_timestamp_ms`).

ቱፕል ከተዘጋጀው የማራገፊያ መመሪያ ጎን ለጎን ያከማቹ እና አጭር ማረጋገጫ ያያይዙ
ቅንጥስ እንደ፡-

```python
from decimal import Decimal
ACT_360_YEAR_MS = 24 * 60 * 60 * 1000 * 360

principal = Decimal("1000")
rate_bps = Decimal("1500")  # 150 bps
elapsed_ms = Decimal(maturity_ms - initiated_ms)
interest = principal * (rate_bps / Decimal(10_000)) * (elapsed_ms / Decimal(ACT_360_YEAR_MS))
expected_cash = principal + interest.quantize(Decimal("0.01"))
```

የተጠጋጋው `expected_cash` በግልባጩ ከተቀመጠው `quantity` ጋር መመሳሰል አለበት
repo መመሪያ. የስክሪፕት ውፅዓት (ወይም ካልኩሌተር የስራ ሉህ) ውስጥ ያቆዩት።
`artifacts/finance/repo/<agreement>/interest.json` ስለዚህ ኦዲተሮች እንደገና ማስላት ይችላሉ።
የእርስዎን የንግድ የተመን ሉህ ሳይተረጉሙ ይሳሉ። ውህደት ስብስብ
ቀድሞውኑ ተመሳሳይ የማይለዋወጥ ሁኔታን ያስፈጽማል
(`repo_roundtrip_transfers_balances_and_clears_agreement`)፣ ግን የops ማስረጃ
የማይጎዱትን ትክክለኛ እሴቶችን መጥቀስ ይኖርበታል።【Integration_tests/tess/repo.rs:1】

** Margin & accrual cadence.** እያንዳንዱ ስምምነት የድጋፍ ሰጪዎችን ያጋልጣል
`RepoAgreement::next_margin_check_after()` እና የተሸጎጠው
`last_margin_check_timestamp_ms`፣ የኅዳግ ጠራርጎ መሆኑን ለማረጋገጥ ጠረጴዛዎች ማንቃት
`RepoMarginCallIsi` ከማቅረባቸው በፊትም ቢሆን በመመሪያው መሰረት መርሐግብር ተይዞላቸው ነበር።
ግብይት።【crates/iroha_data_model/src/repo.rs:113】【crates/iroha_core/src/smartcontracts/isi/repo.rs:557】
እያንዳንዱ የኅዳግ ጥሪ በማስረጃ ጥቅል ውስጥ ሦስት ቅርሶችን ማካተት አለበት፡-

1. `repo margin-call --agreement <id>` JSON ውፅዓት (ወይም ተመጣጣኝ ኤስዲኬ
   ክፍያ)) የስምምነት መታወቂያውን የሚመዘግብ ፣ ለ ጥቅም ላይ የዋለው የማገጃ የጊዜ ማህተም
   ቼክ፣ እና እሱን የቀሰቀሰው ባለስልጣን።【crates/iroha_cli/src/main.rs:3821】
2. የስምምነቱ ቅጽበታዊ ገጽ እይታ (`repo query get --agreement-id <id>`) ተወስዷል
   ከጥሪው በፊት ወዲያውኑ ገምጋሚዎች ግልጽነት ያለው መሆኑን ማረጋገጥ ይችላሉ።
   (`current_timestamp_ms` ከ `next_margin_check_after()` ጋር ያወዳድሩ)።
3. የ`AccountEvent::Repo::MarginCalled` SSE/NDJSON ምግብ ለእያንዳንዱ ሚና የተለቀቀው
   (አስጀማሪ፣ ተጓዳኝ እና እንደ አማራጭ ሞግዚት) ምክንያቱም የሩጫ ሰዓቱ
   ክስተቱን ለእያንዳንዱ ተሳታፊ ያባዛል።【crates/iroha_data_model/src/events/data/events.rs:742】

CI እነዚህን ደንቦች በ በኩል ይጠቀማል
ጥሪዎችን የማይቀበል `repo_margin_call_enforces_cadence_and_participant_rules`
ቀደም ብለው ወይም ካልተፈቀዱ መለያዎች የሚመጡ።【Integration_tests/tests/repo.rs:395】
ያንን ማረጋገጫ በማስረጃ መዝገብ ውስጥ መድገም የመንገድ ካርታውን F1 የሚዘጋው ነው።
የሰነድ ክፍተት፡ የአስተዳደር ገምጋሚዎች ተመሳሳይ የጊዜ ማህተሞችን ማየት ይችላሉ።
በ§2.7 ውስጥ ከተያዘው የመወሰኛ ማረጋገጫ ሃሽ ጋር የሩጫ ጊዜ የታመነ ነው።
እና አንጸባራቂው በ§3.2 ውስጥ ተብራርቷል.

### 2.8 የሶስትዮሽ ወገን ጥበቃ ማረጋገጫ እና ክትትልፍኖተ ካርታ **F1** ዋስትና የቆመበትን የሶስትዮሽ ፓርቲ ጊዜን ይጠራል
ከተጓዳኝ ይልቅ ጠባቂ. የሩጫ ጊዜው የአሳዳጊውን መንገድ ያስገድዳል
ቃል የተገቡ ንብረቶችን ወደ `RepoAgreement::custodian` በማስቀጠል
በሚነሳበት ጊዜ የጠባቂ ሂሳብ እና በመልቀቅ ላይ
ለእያንዳንዱ የህይወት ኡደት እርምጃ `RepoAccountRole::Custodian` ክስተቶች ኦዲተሮች ማየት እንዲችሉ
በእያንዳንዱ የጊዜ ማህተም ላይ ዋስትና የያዙ።【crates/iroha_data_model/src/repo.rs:74】【crates/iroha_core/src/smartcontracts/isi/repo.rs:252】【Integration_tests/tests/repo.rs:951】
ከላይ ከተዘረዘሩት የሁለትዮሽ ማስረጃዎች በተጨማሪ፣ እያንዳንዱ የሶስትዮሽ ፓርቲ ሪፖ መሆን አለበት።
የአስተዳደር ፓኬጁ እንደተጠናቀቀ ከመቆጠሩ በፊት ከዚህ በታች ያሉትን ቅርሶች ይያዙ።

** ተጨማሪ የመመገቢያ መስፈርቶች ***

1. **የጠባቂ እውቅና።** ዴስኮች የተፈረመበት እውቅና ማከማቸት አለባቸው
   እያንዳንዱ ሞግዚት ሪፖ መለያውን ፣ የጥበቃ መስኮቱን ፣ መሄጃውን ያረጋግጣል
   መለያ, እና የሰፈራ SLAs. የተፈረመውን ሰነድ ያያይዙ
   (`artifacts/finance/repo/<agreement>/custodian_ack_<custodian>.md`)
   እና ገምጋሚዎች ያንን ማየት እንዲችሉ በአስተዳደር ፓኬጁ ውስጥ ያመልክቱ
   ሶስተኛው አካል ለተመሳሳይ ባይት ተስማማ/አስጀማሪው/ተቃዋሚው አፀደቀ።
2. ** የጥበቃ ደብተር ቅጽበታዊ ገጽ እይታ።** ጅምር መያዣ ወደ ጠባቂው ያንቀሳቅሳል።
   አካውንት እና መፍታት ወደ አስጀማሪው ይመልሳል; የሚመለከተውን ይያዙ
   `FindAssets` ውፅዓት ለሞግዚት ከእያንዳንዱ እግር በፊት እና በኋላ ስለዚህ ኦዲተሮች
   ሚዛኖቹ ከተዘጋጁት መመሪያዎች ጋር የሚዛመዱ መሆናቸውን ማረጋገጥ ይችላል።【crates/iroha_core/src/smartcontracts/isi/repo.rs:252】【crates/iroha_core/src/smartcontracts/isi/repo.rs:1641】
3. **የክስተት ደረሰኞች።** የ`RepoAccountEvent` ዥረቱን ለሁሉም ሚናዎች እና
   የአሳዳጊውን ጭነት ከአስጀማሪው/የፓርቲ መዛግብት ጋር ያከማቹ።
   የሩጫ ሰዓቱ ለእያንዳንዱ ሚና የተለያዩ ክስተቶችን ያወጣል።
   `RepoAccountRole::{Initiator,Counterparty,Custodian}`, ስለዚህ ጥሬውን በማያያዝ
   SSE ምግብ ሦስቱም ወገኖች ተመሳሳይ የጊዜ ማህተሞችን ማየታቸውን ያረጋግጣል
   የመቋቋሚያ መጠኖች።
4. **ጠባቂ ዝግጁነት ማረጋገጫ ዝርዝር** ሪፖ ማጣቀሻው ሲሰራ
   shims (ለምሳሌ፣ escrow reconciliations ወይም የቁም መመሪያዎች)፣ መዝገብ
   አውቶማቲክ ዕውቂያው እና የስራ ሂደቱን ለመለማመድ ጥቅም ላይ የሚውለው ትዕዛዝ (እንደ
   እንደ `iroha app repo initiate --custodian ... --dry-run`) ስለዚህ ገምጋሚዎች መድረስ ይችላሉ።
   በልምምድ ወቅት ሞግዚት ኦፕሬተሮች.

| ማስረጃ | ትዕዛዝ / መንገድ | ዓላማ |
|-------------|--------|
| ሞግዚት እውቅና (`custodian_ack_<custodian>.md`) | በ`docs/examples/finance/repo_governance_packet_template.md` (`docs/examples/finance/repo_custodian_ack_template.md` እንደ ዘር ተጠቀም) ከተጠቀሰው የተፈረመ ማስታወሻ ጋር አገናኝ። | ንብረቶቹ ከመውሰዳቸው በፊት ሶስተኛው አካል የሪፖ መታወቂያውን፣ የጥበቃ ጥበቃን እና የሰፈራ ሰርጡን መቀበሉን ያሳያል። |
| የጥበቃ ንብረት ቅጽበታዊ እይታ | `iroha json --query FindAssets '{ "id": "...#<custodian>" }' > artifacts/.../assets/custodian_<ts>.json` | `RepoIsi` ኢንኮድ እንዳስቀመጠው ግራ/መመለሱን ያረጋግጣል። |
| ጠባቂ `RepoAccountEvent` ምግብ | `torii-events --account <custodian> --event-type repo > artifacts/.../events/custodian.ndjson` | ለመነሻ፣ ለኅዳግ ጥሪዎች እና ለመዝናናት የሚወጣውን የሩጫ ጊዜ የ`RepoAccountRole::Custodian` ክፍያን ይይዛል። |
| የጥበቃ መሰርሰሪያ መዝገብ | `artifacts/.../governance/drills/<timestamp>-custodian.log` | ሞግዚቱ የመልሶ ማቋቋሚያ ወይም የሰፈራ ስክሪፕቶችን በተለማመዱበት ቦታ የደረቁ ሩጫዎችን ያዘጋጃሉ። |ተመሳሳዩን የሃሽንግ የስራ ፍሰት (`scripts/repo_evidence_manifest.py`) ለ
የአሳዳጊ እውቅና፣ የንብረት ቅጽበተ-ፎቶዎች እና የክስተት ምግቦች የሶስትዮሽ ፓርቲን ያቆያሉ።
ፓኬቶች ሊባዙ የሚችሉ. ብዙ አሳዳጊዎች በአንድ መጽሐፍ ውስጥ ሲሳተፉ ይፍጠሩ
ንዑስ ማውጫዎች በአንድ ሞግዚት ስለዚህ አንጸባራቂው የየትኞቹ ፋይሎች እንደሆኑ ያሳያል
እያንዳንዱ ፓርቲ; የአስተዳደር ትኬቱ እያንዳንዱን ዝርዝር ሃሽ እና የ
የሚዛመደው የእውቅና ፋይል. የሚሸፍነው ውህደት ፈተናዎች
`repo_initiation_with_custodian_routes_collateral` እና
`reverse_repo_with_custodian_emits_events_for_all_parties` ቀድሞውንም ያስፈጽማል
የሩጫ ጊዜ ባህሪ—በማስረጃ ቅርቅቡ ውስጥ ቅርሶቻቸውን ማንጸባረቅ ነው።
ፍኖተ ካርታ **F1** ለሶስትዮሽ ፓርቲ ሁኔታ ዝግጁ የሆኑ ሰነዶችን እንዲልክ ያስችለዋል

### 2.9 የድህረ ማጽደቅ ውቅረት ቅጽበተ-ፎቶዎች

አንዴ አስተዳደር ለውጥን ካፀደቀ እና `[settlement.repo]` ስታንዛ በርቶ
ክላስተር፣ ከእያንዳንዱ እኩያ የተረጋገጠ የውቅር ቅጽበታዊ ገጽ እይታን ያንሱ
ኦዲተሮች የጸደቁት እሴቶች ቀጥታ መሆናቸውን ማረጋገጥ ይችላሉ። Torii ያጋልጣል
ለዚህ ዓላማ `/v1/configuration` መንገድ እና ሁሉም የኤስዲኬዎች ወለል ረዳቶች እንደ
`ToriiClient.getConfiguration`፣ ስለዚህ የቀረጻው የስራ ሂደት ለዴስክ ስክሪፕቶች ይሰራል፣
CI፣ ወይም በእጅ የሚሰራ ኦፕሬተር።【crates/iroha_torii/src/lib.rs:3225】【javascript/iroha_js/src/toriiClient.js:2115】【IrohaSwift/ምንጮች/IrohaSwift/Torii ደንበኛ፡46wi1

1. ወዲያውኑ ለአይ18NI00000198X (ወይም ለኤስዲኬ አጋዥ) ይደውሉ።
   መልቀቅ ። ሙሉውን JSON በስር ይቀጥሉ
   `artifacts/finance/repo/<agreement>/config/peers/<peer-id>.json` እና መዝገብ
   የማገጃው ቁመት/ክላስተር የጊዜ ማህተም በ`config/config_snapshot_index.md`።
   ```bash
   mkdir -p artifacts/finance/repo/<slug>/config/peers
   curl -fsSL https://peer01.example/v1/configuration \
     | jq '.' \
     > artifacts/finance/repo/<slug>/config/peers/peer01.json
   ```
2. እያንዳንዱን ቅጽበታዊ ገጽ እይታ (`sha256sum config/peers/*.json`) Hash እና ቀጣዩን መረጃ ይመዝገቡ
   በአስተዳደር ፓኬት አብነት ውስጥ ለአቻ መታወቂያ። ይህ የትኞቹ እኩዮች እንደሆኑ ያረጋግጣል
   ፖሊሲውን ገብቷል እና የትኛው ቁርጠኝነት/የመሳሪያ ሰንሰለት ቅጽበታዊ ገጽ እይታን ሰርቷል።
3. በእያንዳንዱ ቅጽበታዊ ገጽ እይታ የ`.settlement.repo` ብሎክ ከተዘጋጀው ጋር ያወዳድሩ
   `[settlement.repo]` TOML ቅንጫቢ; ማንኛውንም ተንሸራታች መዝግብ እና እንደገና መሮጥ
   `repo query get --agreement-id <id> --pretty` ስለዚህ የማስረጃ ጥቅል ያሳያል
   ሁለቱም የአሂድ ጊዜ ውቅር እና መደበኛ የ `RepoGovernance` እሴቶች
   በስምምነቱ ተከማችቷል።【crates/iroha_cli/src/main.rs:3821】
4. ቅጽበተ-ፎቶ ፋይሎችን እና የማጠቃለያ መረጃ ጠቋሚውን ከማስረጃው ሰነድ ጋር ያያይዙ (ይመልከቱ
   §3.2) ስለዚህ የአስተዳደር መዝገብ የጸደቀውን ለውጥ ከእውነተኛው እኩያ ጋር ያገናኛል።
   የማዋቀር ባይት. የአስተዳደር አብነት ይህንን ለማካተት ተዘምኗል
   ሠንጠረዥ፣ ስለዚህ እያንዳንዱ የወደፊት ሪፖ ፓኬት ተመሳሳይ ማረጋገጫ ይይዛል።

እነዚህን ቅጽበተ-ፎቶዎች ማንሳት የተጠራው `iroha_config` የሰነድ ክፍተት ይዘጋዋል
በፍኖተ ካርታው ውስጥ፡ ገምጋሚዎች አሁን ደረጃውን የጠበቀ TOML በየባይት ሊለያዩ ይችላሉ።
የአቻ ሪፖርቶች፣ እና ኦዲተሮች ንፅፅርን እንደገና ማካሄድ ይችላሉ።
በምርመራ ላይ.

## 3. ቆራጥ ማስረጃ የስራ ፍሰት1. ** መመሪያውን ይመዝግቡ **
   - በ `iroha app repo ... --output` በኩል የሪፖ/የማራገፍ ክፍያን ይፍጠሩ።
   - `InstructionBox` JSON ስር ያከማቹ
     `artifacts/finance/repo/<agreement-id>/initiation.json`.
2. ** የመመዝገቢያ ሁኔታን ያንሱ ***
   - ከዚህ በፊት `iroha app repo query list --pretty > artifacts/.../agreements.json` ያሂዱ
     እና ከሰፈራ በኋላ ሚዛኖች መሰረታቸውን ለማረጋገጥ።
   - እንደ አማራጭ `FindAssets` በ`iroha json` ወይም በኤስዲኬ አጋዥዎች በኩል በማህደር ለማስቀመጥ ይጠይቁ
     በሪፖ እግር ውስጥ የተነካው የንብረት ሚዛን.
3. ** የክስተት ዥረቶችን ቀጥል ***
   - ለ`AccountEvent::Repo` ከ Torii SSE በላይ ይመዝገቡ ወይም ይጎትቱ እና ያያይዙ
     JSON ወደ የማስረጃ ማውጫው አውጥቷል። ይህ ተንኮለኛውን ያሟላል።
     የመግቢያ አንቀጽ ምክንያቱም ክስተቶቹ በተመለከቱት እኩዮች የተፈረሙ ናቸው
     እያንዳንዱ ለውጥ.
4. ** የመወሰኛ ሙከራዎችን ያካሂዱ ***
   - CI አስቀድሞ `integration_tests/tests/repo.rs` ይሰራል; በእጅ ለመውጣት ፣
     `cargo test -p integration_tests repo::` ን ያስፈጽሙ እና ሎግ ፕላስ በማህደር ያስቀምጡ
     `target/debug/deps/repo-*` JUnit ውፅዓት።
5. **አስተዳደር እና ማዋቀርን ተከታታይ አድርግ**
   - በጊዜው ጥቅም ላይ የዋለውን የ `[settlement.repo]` ውቅር ያረጋግጡ (ወይም አያይዘው)
     የፀጉር አሠራር / ብቁ ዝርዝሮችን ጨምሮ. ይህ የኦዲት ድግግሞሾችን ለማዛመድ ያስችላል
     በ`RepoAgreement` ውስጥ የተመዘገበ የሩጫ መደበኛ አስተዳደር።

### 3.1 የማስረጃ ጥቅል አቀማመጥ

በዚህ ክፍል ውስጥ የተጠሩትን ሁሉንም ቅርሶች በአንድ ስምምነት ስር ያከማቹ
ማውጫ ስለዚህ አስተዳደር አንድ ዛፍ በማህደር ወይም በሃሽ እንዲይዝ። የሚመከረው አቀማመጥ፡-

```
artifacts/finance/repo/<agreement-id>/
├── agreements_before.json
├── agreements_after.json
├── initiation.json
├── unwind.json
├── margin/
│   └── 2026-04-30.json
├── events/
│   └── repo-events.ndjson
├── config/
│   ├── settlement_repo.toml
│   └── peers/
│       ├── peer01.json
│       └── peer02.json
├── repo_proof_snapshot.json
├── repo_proof_digest.txt
└── tests/
    └── repo_lifecycle.log
```

- `agreements_before/after.json` ቀረጻ `repo query list` ውፅዓት ስለዚህ ኦዲተሮች እንዲችሉ
  የሒሳብ መዝገብ ስምምነቱን ማጽደቁን ያረጋግጡ።
- `initiation.json`፣ `unwind.json`፣ እና `margin/*.json` ትክክለኛዎቹ Norito ናቸው።
  ከ `iroha app repo ... --output` ጋር የተደረደሩ የክፍያ ጭነቶች።
- `events/repo-events.ndjson` የ `AccountEvent::Repo(*)` ዥረቱን በድጋሚ ያጫውታል
  `tests/repo_lifecycle.log` የ`cargo test` ማስረጃን ይጠብቃል።
- `repo_proof_snapshot.json` እና `repo_proof_digest.txt` የመጡት ከቅጽበተ-ፎቶ ነው
  በ§2.7 ውስጥ ያለውን ሂደት አድስ እና ገምጋሚዎች የህይወት ኡደትን ሃሽ እንደገና እንዲያሰሉ ያድርጉ
  ማሰሪያውን እንደገና ሳያስኬድ.
- `config/settlement_repo.toml` የ `[settlement.repo]` ቅንጣቢ ይዟል
  ሪፖው ሲተገበር ንቁ የነበረው (የፀጉር መቆረጥ፣ የመተካት ማትሪክስ)።
- `config/peers/*.json` ለእያንዳንዱ እኩያ የ `/v1/configuration` ቅጽበታዊ ገጽ እይታዎችን ይይዛል ፣
  በ TOML እና በሂደት ጊዜ ዋጋዎች መካከል ያለውን ዑደት መዝጋት እኩዮች ሪፖርት
  ከ Torii በላይ።

### 3.2 Hash manifest generation

ገምጋሚዎች ሃሽዎችን ማረጋገጥ እንዲችሉ ለእያንዳንዱ ቅርቅብ የሚወስን ዝርዝር መግለጫ ያያይዙ
ማህደሩን ሳይፈታ. ረዳት በ `scripts/repo_evidence_manifest.py`
የስምምነት ማውጫውን ይመራል፣ `size`፣ `sha256`፣ `blake2b` እና የመጨረሻውን ይመዘግባል።
ለእያንዳንዱ ፋይል የተሻሻለ የጊዜ ማህተም እና የJSON ማጠቃለያ ይጽፋል፡-

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/wonderland-2026q1 \
  --agreement-id 7mxD1tKRyv32je4kZwcWa9wa33bX \
  --output artifacts/finance/repo/wonderland-2026q1/manifest.json \
  --exclude 'scratch/*'
```

ጀነሬተሩ ዱካዎችን በመዝገበ-ቃላት ይመድባል፣ በሚኖርበት ጊዜ የውጤት ፋይሉን ይዘላል
በተመሳሳዩ ማውጫ ውስጥ እና አስተዳደር በቀጥታ መቅዳት የሚችለውን ድምር ያወጣል።
ወደ ለውጥ ቲኬት. `--output` ሲቀር አንጸባራቂ ህትመቶች ወደ
በጠረጴዛ ግምገማዎች ወቅት ለፈጣን ልዩነቶች ምቹ የሆነ stdout።
የጭረት ቁሳቁሶችን ለመተው `--exclude <glob>` ይጠቀሙ (ለምሳሌ `--exclude 'scratch/*' --exclude '*.tmp'`)
ፋይሎችን ከጥቅሉ ውስጥ ሳያንቀሳቅሱ; የግሎብ ንድፍ ሁልጊዜ በ
ከ `--root` አንጻራዊ መንገድ።ምሳሌ አንጸባራቂ (ለአጭሩ የተቆረጠ)

```json
{
  "agreement_id": "7mxD1tKRyv32je4kZwcWa9wa33bX",
  "generated_at": "2026-04-30T11:58:43Z",
  "root": "/var/tmp/repo/wonderland-2026q1",
  "file_count": 5,
  "total_bytes": 1898,
  "files": [
    {
      "path": "agreements_after.json",
      "size": 512,
      "sha256": "6b6ca81b00d0d889272142ce1e6456872dd6b01ce77fcd1905f7374fc7c110cc",
      "blake2b": "5f0c7f03d15cd2a69a120f85df2a4a4a219a716e1f2ec5852a9eb4cdb443cbfe3c1e8cd02b3b7dbfb89ab51a1067f4107be9eab7d5b46a957c07994eb60bb070",
      "modified_at": "2026-04-30T11:42:01Z"
    },
    {
      "path": "initiation.json",
      "size": 274,
      "sha256": "7a1a0ec8c8c5d43485c3fee2455f996191f0e17a9a7d6b25fc47df0ba8de91e7",
      "blake2b": "ce72691b4e26605f2e8a6486d2b43a3c2b472493efd824ab93683a1c1d77e4cff40f5a8d99d138651b93bcd1b1cb5aa855f2c49b5f345d8fac41f5b221859621",
      "modified_at": "2026-04-30T11:39:55Z"
    }
  ]
}
```

ከማስረጃ ጥቅል ቀጥሎ ያለውን አንጸባራቂ ያካትቱ እና የእሱን SHA-256 ሃሽ ያጣቅሱ
በአስተዳደር ፕሮፖዛል ውስጥ ዴስክ፣ ኦፕሬተሮች እና ኦዲተሮች ተመሳሳይ ይጋራሉ።
የመሬት እውነት.

### 3.3 የአስተዳደር ለውጥ ምዝግብ ማስታወሻ እና የድጋሚ ልምምዶች

የፋይናንስ ምክር ቤቱ እያንዳንዱን የድጋሚ ጥያቄ፣ የፀጉር ማስተካከያ ወይም መተካት ይጠብቃል።
ማትሪክስ ለውጥ ሊባዛ የሚችል የአስተዳደር ፓኬት ጋር ሊገናኝ ይችላል
በቀጥታ ከህዝበ ውሳኔ ደቂቃዎች።【docs/source/governance_playbook.md:1】

1. **የአስተዳደር ፓኬጁን ይገንቡ**
   - ለስምምነቱ የማስረጃ ጥቅል ቅዳ
     `artifacts/finance/repo/<agreement-id>/governance/`.
   - `gar.json` (የምክር ቤት ማፅደቂያ መዝገብ)፣ `referendum.md` (ማን ያፀደቀው) ይጨምሩ።
     እና የትኞቹን ሃሽዎች ገምግመዋል) እና `rollback_playbook.md`
     ከ `repo_runbook.md` የተገላቢጦሽ አሰራርን ማጠቃለል
     §§4–5.【docs/source/finance/repo_runbook.md:1】
   - የሚወስነውን አንጸባራቂ ሃሽ ከ§3.2 ኢንች ያንሱ
     `hashes.txt` ስለዚህ ገምጋሚዎች በTorii ግጥሚያ ላይ የሚያዩትን ጭነት ማረጋገጥ ይችላሉ።
     የተደረደሩት ባይት.
2. ** ፓኬጁን በሪፈረንደም አጣቅሱ**
   - `iroha app governance referendum submit` (ወይም ተመጣጣኝ ኤስዲኬ) ሲያሄድ
     አጋዥ) ከ`hashes.txt` በ`--notes` ውስጥ ያለውን አንጸባራቂ ሃሽ ያካትቱ
     ጭነት ስለዚህ GAR ወደ የማይለወጥ ፓኬት ይጠቁማል።
   - በአስተዳደር መከታተያ ወይም በቲኬት መመዝገቢያ ስርዓት ውስጥ ተመሳሳይ ሃሽ ያስገቡ
     የኦዲት ዱካዎች በቅጽበታዊ ገጽ እይታ ዳሽቦርዶች ላይ አይመሰረቱም።
3. ** የሰነድ ልምምዶች እና መልሶ መመለስ**
   - ህዝበ ውሳኔው ካለፈ በኋላ `ops/drill-log.md`ን በሪፖ ያዘምኑ
     የስምምነት መታወቂያ፣ የተሰማራው config hash፣ GAR መታወቂያ እና ኦፕሬተርን ያግኙ
     የሩብ ዓመት የሥልጠና መዝገብ የፋይናንስ ድርጊቶችን ያጠቃልላል።【ops/drill-log.md:1】
   - የመልሶ ማገገሚያ ልምምድ ከሰራ, የተፈረመውን ያያይዙ
     `rollback_playbook.md` እና የ CLI ውፅዓት ከ `iroha app repo unwind` ስር
     `governance/drills/<timestamp>.log` እና ተመሳሳይ በመጠቀም ምክር ቤቱን ያሳውቁ
     በአስተዳደር መጫወቻ መጽሐፍ ውስጥ የተገለጹ እርምጃዎች.

የምሳሌ አቀማመጥ፡-

```
artifacts/finance/repo/<agreement-id>/governance/
├── gar.json
├── hashes.txt
├── referendum.md
├── rollback_playbook.md
└── drills/
    └── 2026-05-12T09-00Z.log
```

GAR፣ ህዝበ ውሳኔ እና ቁፋሮ ቅርሶችን ከህይወት ኡደት ጋር ማቆየት።
ማስረጃዎች እያንዳንዱ የድጋሚ ለውጥ የመንገድ ካርታ F1 አስተዳደርን እንደሚያረካ ዋስትና ይሰጣል
በኋላ ላይ የግዴታ ትኬት ስፔሉንግ ሳያስፈልግ ባር።

### 3.4 የህይወት ዑደት አስተዳደር ማረጋገጫ ዝርዝር

ፍኖተ ካርታ **F1** ለአጀማመር፣ ለማከማቸት/ህዳግ፣ እና የአስተዳደር ሽፋንን ይጠራል
የሶስት-ፓርቲ ንፋስ ፈታ. ከዚህ በታች ያለው ሰንጠረዥ ማጽደቂያዎችን ያጠናክራል ፣ ቆራጥነት
ቅርሶች፣ እና በእያንዳንዱ የህይወት ዑደት ደረጃ ማጣቀሻዎችን በመፈተሽ የፋይናንስ ጠረጴዛዎች ሀ
አንድ ፓኬት በሚሰበሰብበት ጊዜ ነጠላ ዝርዝር.| የህይወት ዑደት ደረጃ | አስፈላጊ ማጽደቅ እና ትኬቶች | ቆራጥ ቅርሶች እና ትዕዛዞች | የተቆራኘ የተሃድሶ ሽፋን |
|--------------------------------|
| ** ተነሳሽነት (የሁለትዮሽ ወይም የሶስትዮሽ ፓርቲ) *** | ባለሁለት መቆጣጠሪያ ምልክት በ`docs/examples/finance/repo_governance_packet_template.md` የተመዘገበ የአስተዳደር ትኬት ከ`[settlement.repo]` ልዩነት እና GAR መታወቂያ ጋር፣የጠባቂ እውቅና `--custodian` ሲዘጋጅ። | መመሪያውን በ`iroha --config client.toml --output repo initiate ...`።የህይወት ዑደቱን ማረጋገጫ ቅጽበታዊ ገጽ እይታ (`REPO_PROOF_*` env vars) እና የጥቅል መግለጫውን ከ`scripts/repo_evidence_manifest.py` ያውጡ።የቅርብ ጊዜውን `FindRepoAgreements`3NIPPET ያያይዙ። (የፀጉር መቆረጥ፣ ብቁ ዝርዝር፣ የመተካት ማትሪክስ)። | `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` (ሁለትዮሽ) እና `integration_tests/tests/repo.rs::repo_roundtrip_with_custodian_routes_collateral` (ትሪ-ፓርቲ) የሩጫ ጊዜው ከተዘጋጁት የጭነት ጭነቶች ጋር እንደሚዛመድ ያረጋግጣሉ። |
| ** የኅዳግ ጥሪ የመሰብሰቢያ ጊዜ** | የዴስክ መሪ + የአደጋ አስተዳዳሪ በአስተዳደር ፓኬት ውስጥ የተመዘገበውን የ cadence መስኮት ያጸድቃል; ቲኬቱ የታቀደውን `RepoMarginCallIsi` ይጠቅሳል። | ወደ `iroha app repo margin-call` ከመደወልዎ በፊት የ`iroha app repo margin --agreement-id` ውፅዓትን ያንሱ፣ የተገኘውን JSON ሃሽ ያድርጉ እና የ`RepoAccountEvent::MarginCalled` SSE ክፍያን በማስረጃ ጥቅል ውስጥ ያስቀምጡ።የ CLI ምዝግብ ማስታወሻን ከመወሰኛ ማረጋገጫ ሃሽ ቀጥሎ ያከማቹ። | `integration_tests/tests/repo.rs::repo_margin_call_enforces_cadence_and_participant_rules` የሩጫ ሰዓቱ ያለጊዜው ጥሪዎችን እና ተሳታፊ ያልሆኑ ግቤቶችን እንደማይቀበል ዋስትና ይሰጣል። |
| ** የዋስትና ምትክ እና ብስለት ፈታ** | የአስተዳደር ለውጥ መዝገብ አስፈላጊውን የ `collateral_substitution_matrix` ግቤቶችን እና የፀጉር አያያዝ ፖሊሲን ይጠቅሳል; የምክር ቤት ደቂቃዎች የመተኪያ ጥንድ SHA-256 hash ይዘረዝራሉ። | የፈታውን እግር በ`iroha app repo unwind --output ... --settlement-timestamp-ms <planned>` ደረጃ ያውጡ ስለዚህ ሁለቱም የACT/360 ስሌት (§2.8) እና የምትክ ክፍያ እንደገና ሊባዙ የሚችሉ ናቸው።የ `[settlement.repo]` TOML ቅንጭብጭብ፣ የመተካት መግለጫ እና የተገኘውን `RepoAccountEvent::Settled` ጨምር። | በ`integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` ውስጥ ያለው የመተካት ዙር ጉዞ በቂ ያልሆነ የተቃርኖ-የጸደቀ የመተካት ፍሰቶችን በመተግበር የስምምነቱ መታወቂያ ቋሚ ሆኖ ይቆያል። |
| ** የአደጋ ጊዜ ማራገፍ/የኋላ መሰርሰሪያ** | የክስተት አዛዥ + የፋይናንስ ምክር ቤት በ `docs/source/finance/repo_runbook.md` (ክፍል 4-5) ላይ እንደተገለጸው መልሶ መመለስን አጽድቆ በ`ops/drill-log.md` ውስጥ ያለውን ግቤት ያዝ። | `iroha app repo unwind`ን በመጠቀም ደረጃውን የጠበቀ የመመለሻ ጭነትን ያስፈጽም፣ የCLI ምዝግብ ማስታወሻዎችን + GAR ማጣቀሻን ከ`governance/drills/<timestamp>.log` ጋር ያያይዙ እና ሁለቱንም `repo_deterministic_lifecycle_proof_matches_fixture` እና `scripts/repo_evidence_manifest.py` ረዳትን ከቁፋሮው በፊት/በኋላ ቁርጠኝነትን ያረጋግጡ። | የደስታ መንገድ ማራገፊያ በ `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` ተሸፍኗል; የቁፋሮውን ደረጃዎች በመከተል የአስተዳደር ቅርሶች በሙከራ ጊዜ ከተተገበሩ ዋስትናዎች ጋር እንዲጣጣሙ ያደርጋቸዋል። |

** የጠረጴዛ የጊዜ መስመር።**1. የመቀበያ አብነት ቅዳ፣ የሜታዳታ እገዳን ሙላ (የስምምነት መታወቂያ፣ GAR ቲኬት፣
   ጠባቂ፣ የማዋቀር ሃሽ) እና የማስረጃ ማውጫውን ይፍጠሩ።
2. እያንዳንዱን መመሪያ (`initiate`፣ `margin-call`፣ `unwind`፣ መተኪያ) በ
   `--output` ሁነታ፣ JSON ን ያሽጉ እና ማፅደቂያዎቹን ከእያንዳንዱ ሃሽ ጎን ያስገቡ።
3. የህይወት ኡደት ማረጋገጫ ቅጽበተ-ፎቶን ያውጡ እና ይህን ካደረጉ በኋላ ወዲያውኑ ይግለጹ
   የአስተዳደር ገምጋሚዎች የምግብ መፍጫ ስርዓቱን በተመሳሳዩ የመጠባበቂያ መሳሪያዎች እንደገና ማስላት ይችላሉ።
4. Mirror `RepoAccountEvent::*` SSE ጭነቶች ለተጎዱ ሂሳቦች እና መጣል
   ወደ ውጭ የተላከው NDJSON በ`artifacts/finance/repo/<agreement-id>/events.ndjson`
   ፓኬጁን ከማቅረቡ በፊት.
5. አንዴ ድምፅ ካለፈ፣ `hashes.txt`ን በGAR መለያ ያዘምኑ።
   ማዋቀር ሃሽ፣ እና የማብራሪያ ቼክተም ምክር ቤቱ የታቀዱ ልቀቶችን መከታተል ይችላል።
   የአካባቢ ስክሪፕቶችን እንደገና ሳያስኬዱ።

### 3.5 የአስተዳደር ፓኬት ፈጣን ጅምር

የመንገድ ካርታ F1 ገምጋሚዎች በሚቆዩበት ጊዜ ሊጠቅሱ የሚችሉትን አጭር የፍተሻ ዝርዝር ጠይቀዋል።
የማስረጃ ጥቅል ማሰባሰብ. የድጋሚ ጥያቄ በቀረበ ቁጥር ከዚህ በታች ያለውን ቅደም ተከተል ይከተሉ
ወይም የፖሊሲ ለውጥ ወደ አስተዳደር እየሄደ ነው፡-

1. **የህይወት ዑደት ማረጋገጫ ቅርሶችን ወደ ውጭ ላክ።**
   ```bash
   mkdir -p artifacts/finance/repo/<slug>
   REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
   REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
   cargo test -p iroha_core \
     -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
   ```
   ወደ ውጭ የተላከው JSON + የምግብ መፍጫ መሣሪያው ስር የተመዘገቡትን እቃዎች ያንጸባርቃል
   `crates/iroha_core/tests/fixtures/`፣ ስለዚህ ገምጋሚዎች የህይወት ዑደቱን እንደገና ማስላት ይችላሉ።
   ፍሬም መላውን ስብስብ እንደገና ሳያስኬድ (§2.7 ይመልከቱ)። መደወልም ይችላሉ።
   `scripts/regen_repo_proof_fixture.sh --bundle-dir artifacts/finance/repo/<slug>`
   ተመሳሳይ ፋይሎችን በአንድ ደረጃ ለማደስ እና ለመቅዳት።
2. **እያንዳንዱን መመሪያ በመድረክ ሃሽ ያድርጉ።** ማነሳሳት/ህዳግ/ ማራገፍ ይፍጠሩ
   ጭነቶች ከ `iroha app repo ... --output` ጋር። ለእያንዳንዱ ፋይል SHA-256 ን ይያዙ
   (በ `hashes/` ስር ያከማቹ) ስለዚህ `docs/examples/finance/repo_governance_packet_template.md`
   የተገመገሙት ጠረጴዛዎች ተመሳሳይ ባይት ሊያመለክት ይችላል.
3. **የደብዳቤ/የማዋቀር ቅጽበታዊ ገጽ እይታዎችን ያስቀምጡ።** `repo query list` ውፅዓት በፊት/በኋላ ይላኩ
   ሰፈራ፣ የሚተገበረውን የ`[settlement.repo]` TOML ብሎክ ጣል፣ እና
   ተዛማጅ የሆነውን `AccountEvent::Repo(*)` SSE ምግብን ያንጸባርቁ
   `artifacts/finance/repo/<slug>/events/repo-events.ndjson`. ከ GAR በኋላ
   ያልፋል፣ `/v1/configuration` ቅጽበተ-ፎቶዎችን በአቻ ይቅረጹ (§2.9) እና ያከማቹ።
   በ`config/peers/` ስር ስለዚህ የአስተዳደር ፓኬት ልቀቱ ስኬታማ መሆኑን ያረጋግጣል።
4. **ማስረጃውን ያመነጩ።**
   ```bash
   python3 scripts/repo_evidence_manifest.py \
     --root artifacts/finance/repo/<slug> \
     --agreement-id <repo-id> \
     --output artifacts/finance/repo/<slug>/manifest.json
   ```
   በአስተዳደር ትኬት ውስጥ ያለውን አንጸባራቂ ሃሽ ወይም የGAR ደቂቃዎችን ያካትቱ
   ኦዲተሮች ጥሬውን ጥቅል ሳያወርዱ ፓኬጁን ሊለያዩ ይችላሉ (§3.2 ይመልከቱ)።
5. ** ፓኬጁን ያሰባስቡ።** አብነቱን ከ
   `docs/examples/finance/repo_governance_packet_template.md`፣ ሜታዳታውን ሙላ፣
   የማረጋገጫ ቅጽበታዊ ገጽ እይታውን ያያይዙት/መፍጨት፣ማሳየት፣ማዋቀር ሃሽ፣ኤስኤስኢ ወደ ውጪ መላክ እና ሙከራ
   ምዝግብ ማስታወሻዎች፣ ከዚያም በሪፈረንደም `--notes` መስክ ውስጥ ያለውን አንጸባራቂ SHA-256 ጥቀስ።
   የተጠናቀቀውን ማርክዳውን ከቅርሶቹ አጠገብ ያከማቹ ስለዚህ መልሶ ማግኘቶች ይወርሳሉ
   ለማጽደቅ የላኩት ትክክለኛ ማስረጃ።

የድጋሚ ጥያቄ ካቀረቡ በኋላ ወዲያውኑ ከላይ ያሉትን ደረጃዎች ማሄድ ማለት ነው።
የአስተዳደር ፓኬት የመጨረሻውን ደቂቃ በማስቀረት ምክር ቤቱ እንደተሰበሰበ ዝግጁ ነው።
hashesን ወይም የክስተት ዥረቶችን እንደገና ለመፍጠር ይቸገራሉ።

## 4. የሶስትዮሽ ወገን ጥበቃ እና መያዣ ምትክ- **ጠባቂዎች፡** `--custodian <account>` መስመሮችን ማለፍ
  ጠባቂ ካዝና; Runtime የመለያ ህልውናን ያስፈጽማል እና ሚና-ታግ ያወጣል።
  ሁኔታዎች ጠባቂዎች እንዲታረቁ (`RepoAccountRole::Custodian`)። ግዛት
  ማሽኑ ጠባቂው ከሁለቱም ወገኖች ጋር የሚዛመዱ ስምምነቶችን ውድቅ ያደርጋል።
- ** የዋስትና መተካት፡** የፈታው እግር ሌላ መያዣ ሊያቀርብ ይችላል።
  በሚተካበት ጊዜ ብዛት/ተከታታይ ከ ** ያላነሰ** እስካልሆነ ድረስ
  ቃል የተገባው መጠን * እና * የመተኪያ ማትሪክስ ጥንድ ይፈቅዳል; `ReverseRepoIsi`
  ሁለቱንም ሁኔታዎች ያስፈጽማል
  (`crates/iroha_core/src/smartcontracts/isi/repo.rs:414`–`437`)። ውህደቱ
  የሙከራ ስብስብ ሁለቱንም ውድቅ መንገዱን እና የተሳካ ምትክን ይጠቀማል
  የማዞሪያ ጉዞ (`integration_tests/tests/repo.rs:261`–`359`)፣ የሪፖ አሃድ ሳለ
  ፈተናዎች አዲሱን የማትሪክስ ፖሊሲ ይሸፍናሉ.
- ** ISO 20022 ካርታ ስራ፡** የ ISO ፖስታዎችን ሲገነቡ ወይም ውጫዊውን ሲያስታርቁ
  ሲስተሞች፣ በመስክ ላይ ያለውን ካርታ እንደገና ይጠቀሙ
  `docs/source/finance/settlement_iso_mapping.md` (`colr.007`፣ `sese.023`፣
  `sese.025`) ስለዚህ የ Norito ክፍያ እና የ ISO ማረጋገጫዎች እንደተመሳሰሉ ይቆያሉ።

## 5. የተግባር ማረጋገጫ ዝርዝሮች

### በየቀኑ ቅድመ-ክፍት

1. በ`iroha app repo query list` በኩል የተቀመጠውን የላቀ ስምምነት ወደ ውጭ ላክ።
2. ከግምጃ ቤት ክምችት ጋር ያወዳድሩ እና ብቁ የሆነ የዋስትና ውቅረት ያረጋግጡ
   ከታቀደው መጽሐፍ ጋር ይዛመዳል.
3. ከ`--output` ጋር መጪ መልሶ ማገገሚያዎችን ያሂዱ እና ድርብ ማረጋገጫዎችን ይሰብስቡ።

### የቀን ክትትል

1. ለጀማሪ/ተቃዋሚ ፓርቲ/አሳዳጊ ለ`AccountEvent::Repo` ይመዝገቡ
   መለያዎች; ያልተጠበቁ ጅምሮች ሲከሰቱ ማስጠንቀቂያ ይስጡ.
2. `iroha app repo margin --agreement-id ID` ይጠቀሙ (ወይም
   `RepoAgreementRecord::next_margin_check_after`) በየሰዓቱ ግልጽነትን ለመለየት
   መንሳፈፍ; ቀስቅሴ `repo margin-call` ጊዜ `is_due = true`.
3. ሁሉንም የኅዳግ ጥሪዎች በኦፕሬተር የመጀመሪያ ፊደላት አስመዝግቡ እና የCLI JSON ውፅዓትን አያይዘው።
   የማስረጃ ማውጫው.

### የቀኑ መጨረሻ + ድህረ ሰፈራ

1. `repo query list` እንደገና ያሂዱ እና ያልተጎዱ ስምምነቶች መወገዳቸውን ያረጋግጡ።
2. የ`RepoAccountEvent::Settled` ክፍያ ጭነቶችን በማህደር ያስቀምጡ እና የጥሬ ገንዘብ/የዋስትና ማረጋገጫን ያረጋግጡ
   በ `FindAssets` በኩል ሚዛኖች.
3. በ `ops/drill-log.md` ውስጥ የድጋሚ ልምምዶች ወይም የአደጋ ሙከራዎች ሲደረጉ የመሰርሰሪያ ግቤት ያስገቡ።
   መሮጥ; ለጊዜ ማህተሞች የ `scripts/telemetry/log_sorafs_drill.sh` ስምምነቶችን እንደገና ይጠቀሙ።

## 6. የማጭበርበር እና የመመለሻ ሂደቶች

- ** ድርብ ቁጥጥር: ** ሁልጊዜ መመሪያዎችን በ `--output` ያመንጩ እና ያከማቹ።
  JSON ለጋራ መፈረም። በሂደት ደረጃ የነጠላ ወገን ማቅረቢያዎችን ውድቅ ያድርጉ
  ምንም እንኳን የሩጫ ጊዜው አስጀማሪውን ስልጣን የሚያስፈጽም ቢሆንም።
- ** ግልጽ ያልሆነ ምዝግብ ማስታወሻ:** የ`RepoAccountEvent` ዥረቱን ወደ እርስዎ ያንጸባርቁት
  SIEM so any forged instruction would be detectable (missing peer signatures).
- ** ወደ ኋላ መመለስ:** ሬፖ ያለጊዜው መቁሰል ካለበት `repo unwind` ያስገቡ
  ከተመሳሳዩ የስምምነት መታወቂያ ጋር እና በአደጋዎ ውስጥ የ `--notes` መስክን ያያይዙ
  በGAR የተፈቀደውን የመልሶ ማጫወቻ መጽሐፍን የሚያመለክት መከታተያ።
- **የማጭበርበር መባባስ፡** ያልተፈቀዱ ድግግሞሾች ከታዩ ጥፋቱን ወደ ውጭ ይላኩ።
  `RepoAccountEvent` ጭነቶች፣ በአስተዳደር ፖሊሲ በኩል ሂሳቦቹን ያቆሙ እና
  ለምክር ቤቱ በሪፖ አስተዳደር SOP ያሳውቁ።

## 7. ሪፖርት ማድረግ እና ክትትል

### 7.1 የግምጃ ቤት ማስታረቅ እና የሂሳብ መዝገብ ማስረጃፍኖተ ካርታ **F1** እና የአለም አቀፉ የሰፈራ ጥበቃ (roadmap.md#L1975-L1978)
የሚወስን የግምጃ ቤት ማረጋገጫዎችን ለማካተት እያንዳንዱን የሪፖ ግምገማ ጠይቅ። አምርት ሀ
ከታች ያለውን የማረጋገጫ ዝርዝር በመከተል በየሩብ ወር ጥቅል።

1. ** ቅጽበታዊ ሒሳቦች።** ኃይል የሚሰጠውን የ`FindAssets` መጠይቁን ይጠቀሙ።
   `iroha ledger asset list` (`crates/iroha_cli/src/main_shared.rs`) ወይም
   የXOR ቀሪ ሒሳቦችን ለ`<i105-account-id>` ለመላክ `iroha_python` አጋዥ፣
   `<i105-account-id>` እና በግምገማው ውስጥ የተሳተፈ እያንዳንዱ የጠረጴዛ መለያ። ማከማቻ
   JSON ስር
   `artifacts/finance/repo/<period>/treasury_assets.json` እና git ይቅረጹ
   መፈጸም/የመሳሪያ ሰንሰለት በተጓዳኝ `README.md`።
2. ** የሒሳብ መመዝገቢያ ግምቶችን አቋርጡ።** እንደገና አሂድ
   `sorafs reserve ledger --quote <...> --json-out ...` እና ውጤቱን መደበኛ ያድርጉት
   በ `scripts/telemetry/reserve_ledger_digest.py` በኩል. የምግብ መፍጫውን በአጠገቡ ያስቀምጡ
   የንብረቱ ቅጽበታዊ ገጽ እይታ ስለዚህ ኦዲተሮች የ XOR ድምርን ከሪፖው ጋር ሊለያዩ ይችላሉ።
   CLI ን እንደገና ሳያጫውቱ የመመዝገቢያ ትንበያ።
3. **የማስታረቂያ ማስታወሻውን ያትሙ።** ዴልታዎችን በ ውስጥ ያጠቃልሉ።
   `artifacts/finance/repo/<period>/treasury_reconciliation.md` በማጣቀስ፡-
   የንብረቱ ቅጽበታዊ ሃሽ፣ የሂሳብ መዝገብ ደብተር ሃሽ እና ስምምነቶች የተሸፈኑ።
   ገምጋሚዎች ማረጋገጥ እንዲችሉ ማስታወሻውን ከፋይናንስ አስተዳደር መከታተያ ጋር ያገናኙ
   የሪፖ ልቀቱን ከማጽደቁ በፊት የግምጃ ቤት ሽፋን።

### 7.2 የመሰርሰሪያ እና የመልሶ ማቋቋም የመለማመጃ ማስረጃ

የመቀበያ መስፈርቶቹ እንዲሁ ደረጃቸውን የጠበቁ መመለሻዎችን እና የአደጋ ልምምዶችን ይፈልጋሉ። እያንዳንዱ
የመሰርሰሪያ ወይም የግርግር ልምምድ የሚከተሉትን ቅርሶች መሰብሰብ አለበት፡-

1. `repo_runbook.md` ክፍሎች4-5 በአደጋው አዛዥ የተፈረመ እና
   የፋይናንስ ምክር ቤት.
2. CLI/SDK ምዝግብ ማስታወሻዎች ለልምምድ (`repo initiate|margin-call|unwind`) እና
   የታደሰ የህይወት ዑደት ማረጋገጫ ቅጽበታዊ ገጽ እይታ እና የማስረጃ ሰነዱ (§§2.7–3.2) ተከማችቷል።
   በ `artifacts/finance/repo/drills/<timestamp>/`.
3. የማስጠንቀቂያ ማናጀር ወይም ፔጀር ግልባጭ የተወጉ ምልክቶችን እና የ
   የእውቅና መንገድ. ግልባጩን ከመሰርሰሪያው ቅርሶች አጠገብ ጣል ያድርጉ እና
   አንዱ ጥቅም ላይ ሲውል የአለርትማኔጀር ጸጥታ መታወቂያውን ያካትቱ።
4. የGAR መታወቂያ፣ የሰነድ መግለጫ ሃሽ እና መሰርሰሪያን የሚያመለክት `ops/drill-log.md` ግቤት
   የወደፊት ኦዲቶች የውይይት ምዝግብ ማስታወሻዎችን ሳይቧጠጡ ልምምዶችን መከታተል እንዲችሉ የጥቅል መንገድ።

### 7.3 የአስተዳደር መከታተያ እና የዶክተሮች ንፅህና አጠባበቅ

- ይህን ሰነድ፣ `repo_runbook.md`፣ እና የፋይናንስ አስተዳደር መከታተያ አስገባ
  CLI/SDK ወይም የአሂድ ጊዜ ባህሪ ሲቀየር መቆለፍ፤ ገምጋሚዎች ይጠብቃሉ
  የመቀበያ ጠረጴዛ ትክክለኛ ሆኖ ለመቆየት.
- ሙሉውን የማስረጃ ጥቅል ያያይዙ (`agreements.json`፣ ደረጃ የተደረገባቸው መመሪያዎች፣ SSE
  የጽሑፍ ግልባጮች፣ የቅጽበታዊ ገጽ እይታን ማዋቀር፣ ማስታረቅ፣ ቅርሶችን መሰርሰሪያ እና ሙከራ
  መዝገቦች) ለእያንዳንዱ የሩብ ዓመት ግምገማ ወደ መከታተያ።
- በማስተባበር ጊዜ `docs/source/finance/settlement_iso_mapping.md` ማጣቀሻ
  ከ ISO ድልድይ ኦፕሬተሮች ጋር ስለዚህ ስርዓት ተሻጋሪ እርቅ ተስተካክሎ ይቆያል።

ይህንን መመሪያ በመከተል ኦፕሬተሮች የፍኖተ ካርታ F1 ተቀባይነት አሞሌን ያረካሉ፡
የመወሰን ማረጋገጫዎች ተይዘዋል, የሶስትዮሽ ፓርቲ እና የመተካት ፍሰቶች ናቸው
በሰነድ የተመዘገቡ እና የአስተዳደር አካሄዶች (ባለሁለት መቆጣጠሪያ + የአደጋ ምዝግብ ማስታወሻ) ናቸው።
በዛፍ ውስጥ የተቀመጠ.