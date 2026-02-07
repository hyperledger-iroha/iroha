---
lang: am
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c2cbb5e965350648a30607bbd0f1588212ee0021b412ec55654993c18cc198e
source_last_modified: "2026-01-28T17:11:30.739162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## የመለያ አድራሻ ተገዢነት ሁኔታ (ADDR-2)

ሁኔታ: ተቀባይነት 2026-03-30  
ባለቤቶች: የውሂብ ሞዴል ቡድን / QA Guild  
የመንገድ ካርታ ማጣቀሻ፡ ADDR-2 — ባለሁለት-ቅርጸት Compliance Suite

### 1. አጠቃላይ እይታ

- ቋሚ: `fixtures/account/address_vectors.json` (IH58 (ተመራጭ) + የታመቀ (`sora`, ሁለተኛ-ምርጥ) + multisig አዎንታዊ / አሉታዊ ጉዳዮች).
- ወሰን፡ ወሳኙ V1 ሸክሞች ስውር-ነባሪ፣ አካባቢያዊ-12፣ ዓለም አቀፍ መዝገብ ቤት እና ባለብዙ ሲግ ተቆጣጣሪዎች ከሙሉ የስህተት ታክሶኖሚ ጋር።
ስርጭት፡ በ Rust data-model፣ Torii፣ JS/TS፣ Swift እና Android SDKs ላይ ተጋርቷል፤ ማንኛውም ሸማች ከተለያየ CI አይሳካም።
- የእውነት ምንጭ፡ ጀነሬተሩ በ`crates/iroha_data_model/src/account/address/compliance_vectors.rs` ውስጥ ይኖራል እና በ`cargo xtask address-vectors` በኩል ተጋልጧል።
### 2. ዳግም መወለድ እና ማረጋገጥ

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

ባንዲራዎች፡

- `--out <path>` — የማስታወቂያ ቅርቅቦችን ሲያመርት አማራጭ መሻር (የ`fixtures/account/address_vectors.json` ነባሪዎች)።
- `--stdout` - ወደ ዲስክ ከመጻፍ ይልቅ JSON ወደ stdout ይልቀቅ።
- `--verify` - የአሁኑን ፋይል አዲስ ከተፈጠረው ይዘት ጋር ያወዳድሩ (በመንሸራተት ላይ በፍጥነት አልተሳካም፣ በ`--stdout` መጠቀም አይቻልም)።

### 3. Artefact ማትሪክስ

| ወለል | ማስፈጸም | ማስታወሻ |
|--------|------------|----|
| ዝገት ውሂብ-ሞዴል | `crates/iroha_data_model/tests/account_address_vectors.rs` | JSON ን ይመረምራል፣ ቀኖናዊ ክፍያዎችን እንደገና ይገነባል፣ እና IH58 (የተመረጡ)/የተጨመቀ (`sora`፣ ሁለተኛ-ምርጥ)/ ቀኖናዊ ልወጣዎች + የተዋቀሩ ስህተቶችን ይፈትሻል። |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | የአገልጋይ ጎን ኮዴኮችን ያረጋግጣል ስለዚህ Torii የተበላሸ IH58 (የተሻለ)/የተጨመቀ (`sora`፣ ሁለተኛ-ምርጥ) በቆራጥነት ይጭናል። |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | መስተዋቶች V1 ቋሚዎች (IH58 ተመራጭ/የተጨመቀ (`sora`) ሁለተኛ-ምርጥ/ሙሉ ስፋት) እና ለእያንዳንዱ አሉታዊ ጉዳይ የNorito-ቅጥ የስህተት ኮዶችን ያስረግጣል። |
| ስዊፍት ኤስዲኬ | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | መልመጃዎች IH58 (ተመራጭ)/የተጨመቀ (`sora`፣ ሁለተኛ-ምርጥ) ዲኮዲንግ፣ ባለብዙ ሲግ ጭነቶች እና በአፕል መድረኮች ላይ የስህተት መጋለጥ። |
| አንድሮይድ ኤስዲኬ | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | የKotlin/Java ማሰሪያዎች ከቀኖናዊው አካል ጋር እንደተጣመሩ መቆየታቸውን ያረጋግጣል። |

### 4. ክትትል እና የላቀ ስራ- ሁኔታን ሪፖርት ማድረግ፡ ይህ ሰነድ ከ`status.md` እና ከመንገድ ካርታው ጋር የተገናኘ ስለሆነ ሳምንታዊ ግምገማዎች የቋሚ ጤናን ማረጋገጥ ይችላሉ።
- የገንቢ ፖርታል ማጠቃለያ፡- **ማጣቀሻ → የመለያ አድራሻ ማክበርን ይመልከቱ** በዶክመንቶች ፖርታል (`docs/portal/docs/reference/account-address-status.md`) ወደ ውጭ ለሚመለከተው ማጠቃለያ።
- Prometheus እና ዳሽቦርዶች፡ የኤስዲኬ ቅጂ ባረጋገጡ ቁጥር ረዳቱን በ`--metrics-out` (እና እንደ አማራጭ `--metrics-label`) ያሂዱ ስለዚህ Prometheus የጽሑፍ ፋይል ሰብሳቢው I10NI36000 ን መውሰድ ይችላል። Grafana ዳሽቦርድ **የመለያ አድራሻ ቋሚ ሁኔታ**(`dashboards/grafana/account_address_fixture_status.json`) ማለፊያ/ውድቀት ቆጠራዎችን በየገጽታ ይሰጣል እና ቀኖናዊ SHA-256 ለኦዲት ማስረጃዎች ይሟገታል። ማንኛውም ዒላማ `0` ሪፖርት ሲያደርግ ማስጠንቀቂያ ይስጡ።
- Torii ሜትሪክስ፡ `torii_address_domain_total{endpoint,domain_kind}` አሁን ለእያንዳንዱ በተሳካ ሁኔታ ለተተነተነ መለያ ቃል በቃል ይወጣል፣ ይህም `torii_address_invalid_total`/`torii_address_local8_total`ን ያሳያል። በማናቸውም የ`domain_kind="local12"` ትራፊክ ላይ ማስጠንቀቂያ ይስጡ እና ቆጣሪዎቹን ወደ SRE `address_ingest` ዳሽቦርድ በማንፀባረቅ የአካባቢ-12 የጡረታ በር ኦዲት ሊደረግ የሚችል ማስረጃ አለው።
- ቋሚ ረዳት፡ `scripts/account_fixture_helper.py` ቀኖናዊውን JSON ያውርዳል ወይም ያረጋግጣል ስለዚህ የኤስዲኬ ልቀት አውቶማቲክ ያለ በእጅ ኮፒ/መለጠፍ ጥቅሉን ፈልጎ እንዲያጣራ/እንደ አማራጭ Prometheus ሜትሪክስ ሲጽፍ። ምሳሌ፡-

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \\
    --target path/to/sdk/address_vectors.json \\
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \\
    --metrics-label android
  ```

  ረዳቱ ዒላማው ሲዛመድ `account_address_fixture_check_status{target="android"} 1` ይጽፋል፣ በተጨማሪም የSHA-256 የምግብ መፈጨትን የሚያጋልጡ `account_address_fixture_remote_info`/`account_address_fixture_local_info` መለኪያዎችን ይጽፋል። የጠፉ ፋይሎች `account_address_fixture_local_missing` ሪፖርት አድርገዋል።
  አውቶሜሽን መጠቅለያ፡ የተጠናከረ የጽሁፍ ፋይል ለመልቀቅ (ነባሪው `artifacts/account_fixture/address_fixture.prom`) ከክሮን/CI ወደ `ci/account_fixture_metrics.sh` ይደውሉ። ተደጋጋሚ የ`--target label=path` ግቤቶችን ማለፍ (በአማራጭ `::https://mirror/...` በእያንዳንዱ ዒላማ ላይ ምንጩን ለመሻር) ስለዚህ Prometheus እያንዳንዱን የኤስዲኬ/CLI ቅጂ የሚሸፍን አንድ ፋይል ይቦጫጭራል። የ GitHub የስራ ፍሰት `address-vectors-verify.yml` ይህንን ረዳት ከቀኖናዊው አካል ጋር በማነፃፀር ለSRE ማስመጫ የ`account-address-fixture-metrics` ቅርስ ሰቅሏል።