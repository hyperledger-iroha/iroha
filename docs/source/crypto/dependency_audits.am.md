---
lang: am
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2025-12-29T18:16:35.939138+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ክሪፕቶ ጥገኝነት ኦዲት

## ስትሪቦግ (`streebog` ሣጥን)

- ** በዛፍ ውስጥ ያለው ስሪት: *** `0.11.0-rc.2` በ `vendor/streebog` ስር ይሸጣል (የ`gost` ባህሪ ሲነቃ ጥቅም ላይ ይውላል)።
- ** ሸማች:** `crates/iroha_crypto::signature::gost` (HMAC-Strebog DRBG + የመልእክት hashing)።
- ** ሁኔታ: *** መልቀቅ-እጩ ብቻ። ምንም የRC ያልሆነ ሳጥን በአሁኑ ጊዜ አስፈላጊውን የኤፒአይ ገጽ አያቀርብም፣
  ስለዚህ ለመጨረሻ ጊዜ የሚለቀቀውን ዥረት እየተከታተልን በዛፉ ውስጥ ያለውን ሳጥን ለኦዲትነት እናንጸባርቃለን።
- ** የፍተሻ ነጥቦችን ይገምግሙ: ***
  - የተረጋገጠ የሃሽ ውፅዓት በWycheproof suite እና በ TC26 መጫዎቻዎች በኩል
    `cargo test -p iroha_crypto --features gost` (`crates/iroha_crypto/tests/gost_wycheproof.rs` ይመልከቱ)።
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    ልምምዶች Ed25519/Secp256k1 ከእያንዳንዱ TC26 ኩርባ ጋር ከአሁኑ ጥገኝነት ጋር።
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    አዳዲስ መለኪያዎችን ከተመዘገቡት ሚዲያን ጋር ያወዳድራል (በ CI ውስጥ `--summary-only` ይጠቀሙ፣ ያክሉ
    እንደገና ሲመሰረት `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json`)።
  - `scripts/gost_bench.sh` የቤንች + የፍተሻ ፍሰትን ያጠቃልላል; JSON ን ለማዘመን `--write-baseline` ማለፍ።
    ከጫፍ እስከ ጫፍ የስራ ሂደት `docs/source/crypto/gost_performance.md` ይመልከቱ።
- ** ቅነሳዎች፡** `streebog` የሚጠራው በቆራጥ መጠቅለያዎች ብቻ ሲሆን ቁልፎችን ዜሮ ማድረግ;
  ፈራሚው አስከፊ የሆነ የRNG ውድቀትን ለማስወገድ በስርዓተ ክወና ኢንትሮፒ ያጥርበታል።
- ** ቀጣይ ድርጊቶች: ** የ RustCrypto's streebog `0.11.x` መለቀቅን ይከተሉ; መለያው አንዴ ካረፈ በኋላ ህክምናውን ያዙ
  እንደ መደበኛ ጥገኝነት መጨመር (ቼክ ድምርን ያረጋግጡ፣ ልዩነቱን ይገምግሙ፣ ፕሮቨንሽን ይመዝግቡ፣ እና
  የተሸጠውን መስታወት ይጥሉ).