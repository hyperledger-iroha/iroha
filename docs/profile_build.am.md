---
lang: am
direction: ltr
source: docs/profile_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-12-29T18:16:35.912559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# መገለጫ `iroha_data_model` ግንባታ

በ`iroha_data_model` ውስጥ ቀርፋፋ የግንባታ ደረጃዎችን ለማግኘት የረዳት ስክሪፕቱን ያሂዱ፡-

```sh
./scripts/profile_build.sh
```

ይህ `cargo build -p iroha_data_model --timings` ያሂዳል እና የጊዜ ሪፖርቶችን ለ`target/cargo-timings/` ይጽፋል።
በአሳሽ ውስጥ `cargo-timing.html` ይክፈቱ እና የትኞቹ ሳጥኖች ወይም የግንባታ ደረጃዎች ብዙ ጊዜ እንደሚወስዱ ለማየት በቆይታ ጊዜ ስራዎችን ይመድቡ።

የማመቻቸት ጥረቶችን በጣም ቀርፋፋ በሆኑ ተግባራት ላይ ለማተኮር ሰዓቱን ተጠቀም።