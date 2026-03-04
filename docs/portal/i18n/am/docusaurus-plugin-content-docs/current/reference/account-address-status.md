---
id: account-address-status
lang: am
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Account address compliance
description: Summary of the ADDR-2 fixture workflow and how SDK teams stay in sync.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ቀኖናዊው ADDR-2 ጥቅል (`fixtures/account/address_vectors.json`) ይቀርጻል።
IH58 (ተመራጭ)፣ የታመቀ (`sora`፣ ሁለተኛ-ምርጥ፣ ግማሽ/ሙሉ ስፋት)፣ ባለብዙ ፊርማ እና አሉታዊ ቋሚዎች።
ማንኛውም የኤስዲኬ + I18NT0000004X ወለል በተመሳሳዩ JSON ላይ ስለሚመረኮዝ ማንኛውንም ኮዴክ ማግኘት እንድንችል
ምርቱን ከመድረሱ በፊት ይንሸራተቱ. ይህ ገጽ የውስጣዊ ሁኔታን አጭር ያንጸባርቃል
(በስር ማከማቻ ውስጥ `docs/source/account_address_status.md`) ስለዚህ ፖርታል
አንባቢዎች በሞኖ-ሪፖ ሳይቆፍሩ የስራ ሂደቱን ማጣቀስ ይችላሉ።

## እንደገና ማመንጨት ወይም ጥቅሉን ያረጋግጡ

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

ባንዲራዎች፡

- `--stdout` - ለማስታወቂያ-ሆክ ፍተሻ stdout JSON ልቀቅ።
- `--out <path>` - ወደተለየ መንገድ ይፃፉ (ለምሳሌ ፣ በአገር ውስጥ ሲለዋወጡ)።
- `--verify` - የስራ ቅጂውን አዲስ ከተፈጠረው ይዘት ጋር ያወዳድሩ (አይቻልም)
  ከ `--stdout` ጋር ይጣመራል።

የ CI የስራ ፍሰት **አድራሻ ቬክተር ድሪፍት** `cargo xtask address-vectors --verify` ይሰራል
በማንኛውም ጊዜ ገምጋሚዎችን ወዲያውኑ ለማስጠንቀቅ ቋሚው፣ ጄነሬተር ወይም ዶክመንቶች ሲቀየሩ።

## ማነው የሚበላው?

| ወለል | ማረጋገጫ |
|--------|-----------|
| ዝገት ውሂብ-ሞዴል | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (አገልጋይ) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| ስዊፍት ኤስዲኬ | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| አንድሮይድ ኤስዲኬ | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

እያንዳንዱ የዙር ጉዞዎች ቀኖናዊ ባይት + IH58 + የተጨመቁ (`sora`፣ ሁለተኛ-ምርጥ) ኢንኮዲንግ እና
የ Norito-style የስህተት ኮዶች ከአሉታዊ ጉዳዮች ጋር መያዛቸውን ያረጋግጣል።

## አውቶማቲክ ይፈልጋሉ?

የልቀት መሣሪያ በረዳት ስክሪፕት ማደስ ይችላል።
ቀኖናዊውን የሚያመጣ ወይም የሚያረጋግጥ `scripts/account_fixture_helper.py`
ቅደም ተከተሎችን ሳይገለብጡ/መለጠፍ

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

ረዳቱ I18NI0000023X መሻርን ወይም `IROHA_ACCOUNT_FIXTURE_URL` ይቀበላል
የአካባቢ ተለዋዋጭ ስለዚህ የኤስዲኬ CI ስራዎች ወደ ተመራጭ መስታወት ሊጠቁሙ ይችላሉ።
`--metrics-out` ሲቀርብ ረዳቱ ይጽፋል
`account_address_fixture_check_status{target=\"…\"}` ከቀኖናዊው ጋር
SHA-256 መፍጨት (`account_address_fixture_remote_info`) ስለዚህ Prometheus የጽሑፍ ፋይል
ሰብሳቢዎች እና Grafana ዳሽቦርድ I18NI0000028X ማረጋገጥ ይችላሉ
እያንዳንዱ ገጽ እንደተመሳሰለ ይቆያል። አንድ ዒላማ `0` ሪፖርት ባደረገ ቁጥር ማስጠንቀቂያ ይስጡ። ለ
ባለብዙ ወለል አውቶሜሽን መጠቅለያውን `ci/account_fixture_metrics.sh` ይጠቀሙ
(የተደጋገመ I18NI0000031X ይቀበላል) ስለዚህ የጥሪ ቡድኖች ማተም ይችላሉ
አንድ የተዋሃደ I18NI0000032X ፋይል ለኖድ-ላኪ የጽሑፍ ፋይል ሰብሳቢ።

## ሙሉውን አጭር መግለጫ ይፈልጋሉ?

ሙሉው ADDR-2 ተገዢነት ሁኔታ (ባለቤቶች፣ የክትትል እቅድ፣ ክፍት የድርጊት እቃዎች)
አብሮ በ `docs/source/account_address_status.md` ውስጥ ይኖራል
ከአድራሻ መዋቅር RFC (`docs/account_structure.md`) ጋር። ይህንን ገጽ እንደ ሀ
ፈጣን የአሠራር አስታዋሽ; ለጥልቅ መመሪያ ወደ ሪፖ ሰነዶች ያስተላልፉ።