---
lang: am
direction: ltr
source: docs/source/crypto/sm_rustcrypto_spike.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f133d9489c4bcfae2212e6c5dc098f39c3dea3e5cd42855ba76e8c9b73b4d03
source_last_modified: "2025-12-29T18:16:35.946614+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! ማስታወሻዎች ለ RustCrypto SM ውህደት ስፒል።

# RustCrypto SM Spike ማስታወሻዎች

# አላማ
ያንን RustCrypto's `sm2`፣ `sm3`፣ እና `sm4` ሳጥኖችን (በተጨማሪም `rfc6979`፣ `rfc6979`፣ `rfc6979`፣ `ccm`፣ `ccm`፣ ```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
``` አማራጭ ጥገኝነቶችን ማስተዋወቅን ያረጋግጡ። `iroha_crypto` የባህሪ ባንዲራውን ወደ ሰፊው የስራ ቦታ ከመስመሩ በፊት ተቀባይነት ያለው የግንባታ ጊዜዎችን ያሰራጫል።

## የታቀደ ጥገኝነት ካርታ

| Crate | የሚመከር ስሪት | ባህሪያት | ማስታወሻ |
|---
| `sm2` | `0.13` (RustCrypto / ፊርማዎች) | `std` | በ `elliptic-curve` ላይ ይወሰናል; MSRV ከስራ ቦታ ጋር የሚዛመድ መሆኑን ያረጋግጡ። |
| `sm3` | `0.5.0-rc.1` (RustCrypto/hashes) | ነባሪ | የኤፒአይ ትይዩዎች `sha2`፣ ከነባር የ`digest` ባህሪያት ጋር ይዋሃዳል። |
| `sm4` | `0.5.1` (RustCrypto/block-ciphers) | ነባሪ | ከሲፈር ባህሪያት ጋር ይሰራል; የመኢአድ መጠቅለያዎች ወደ በኋላ ሹል ተላለፈ። |
| `rfc6979` | `0.4` | ነባሪ | ቆራጥ ያልሆነ መነሻን እንደገና መጠቀም። |

*ስሪቶቹ ከ2024-12 ጀምሮ አሁን ያሉ ልቀቶችን ያንፀባርቃሉ። ከማረፍዎ በፊት በ`cargo search` ያረጋግጡ።

## ለውጦችን አሳይ (ረቂቅ)

```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```

ክትትል፡ `elliptic-curve` ቀድሞውንም በ`iroha_crypto` (በአሁኑ ጊዜ `0.13.8`) ውስጥ ካሉት ስሪቶች ጋር እንዲመሳሰል ፒን ያድርጉ።

## Spike Checklist
- [x] አማራጭ ጥገኝነቶችን እና ባህሪን ወደ `crates/iroha_crypto/Cargo.toml` ያክሉ።
- [x] ሽቦን ለማረጋገጥ የ `signature::sm` ሞጁል ከ `cfg(feature = "sm")` በቦታ ያዥ ፍጠር።
- [x] ማጠናቀርን ለማረጋገጥ `cargo check -p iroha_crypto --features sm` ን ያሂዱ; የመዝገብ ግንባታ ጊዜ እና አዲስ የጥገኝነት ብዛት (`cargo tree --features sm`)።
- [x] የ std-ብቻውን አቀማመጥ በ `cargo check -p iroha_crypto --features sm --locked` ያረጋግጡ; `no_std` ግንባታዎች ከአሁን በኋላ አይደገፉም።
- [x] የፋይል ውጤቶች (ጊዜዎች፣ የጥገኛ ዛፍ ዴልታ) በ`docs/source/crypto/sm_program.md`።

ለመቅረጽ ## ምልከታዎች
- ተጨማሪ የማጠናቀር ጊዜ ከመነሻ መስመር ጋር።
- የሁለትዮሽ መጠን ተፅእኖ (የሚለካ ከሆነ) ከ `cargo builtinsize` ጋር።
- ማንኛውም MSRV ወይም የባህሪ ግጭቶች (ለምሳሌ ከ`elliptic-curve` ጥቃቅን ስሪቶች ጋር)።
- ማስጠንቀቂያዎች የወጡ (ደህንነቱ ያልተጠበቀ ኮድ፣ const-fn gating) ወደ ላይ የሚለጠፉ መጠገኛዎችን ሊፈልጉ ይችላሉ።

## በመጠባበቅ ላይ ያሉ እቃዎች
- የስራ ቦታ ጥገኝነት ግራፍ ከመጋነንዎ በፊት የCrypto WG ማረጋገጫን ይጠብቁ።
- ለግምገማ ሣጥኖች ሻጭ መደረጉን ያረጋግጡ ወይም በ crates.io ላይ መታመን (መስታወቶች ሊያስፈልጉ ይችላሉ)።
- የማረጋገጫ ዝርዝሩ እንደተጠናቀቀ ምልክት ከማድረግዎ በፊት `Cargo.lock` ማደስን በ`sm_lock_refresh_plan.md` ያስተባብሩ።
- የመቆለፊያ ፋይሉን እና የጥገኝነት ዛፉን ለማደስ ፍቃድ አንዴ ከተሰጠ `scripts/sm_lock_refresh.sh` ይጠቀሙ።

## 2025-01-19 Spike Log
- የተጨመሩ የአማራጭ ጥገኞች (`sm2 0.13`፣ `sm3 0.5.0-rc.1`፣ `sm4 0.5.1`፣ `rfc6979 0.4`) እና `sm` የባህሪ ባንዲራ በ`iroha_crypto`።
- በሚጠናቀርበት ጊዜ የምስጢር ኤፒአይዎችን ለመለማመድ/Stubbed `signature::sm` ሞጁል።
- `cargo check -p iroha_crypto --features sm --locked` አሁን የጥገኝነት ግራፍ ይፈታል ነገር ግን በ `Cargo.lock` የዝማኔ መስፈርት ያቋርጣል። የማጠራቀሚያ ፖሊሲ የፋይል ማረምን ይከለክላል፣ ስለዚህ የተፈቀደውን የመቆለፊያ እድሳት እስክናስተባብር ድረስ የማጠናቀር ሂደቱ በመጠባበቅ ላይ ይቆያል።## 2026-02-12 Spike Log
- የቀደመውን የመቆለፊያ ፋይል ማገጃ ተፈቷል-ጥገኛዎቹ ቀድሞውኑ ተይዘዋል-ስለዚህ `cargo check -p iroha_crypto --features sm --locked` ተሳክቷል (ቀዝቃዛ ግንባታ 7.9s በዴቭ ማክ ላይ ፣ ጭማሪ እንደገና 0.23s)።
- `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` በ 1.0 ዎች ውስጥ ያልፋል ፣ የአማራጭ ባህሪው በ `std`-ብቻ ውቅሮች ውስጥ መዘጋጀቱን ያረጋግጣል (ምንም የ `no_std` ዱካ ይቀራል)።
- ጥገኛ ዴልታ ከ `sm` ባህሪ ጋር የነቃ 11 ሳጥኖችን ያስተዋውቃል: `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `sm2` `primeorder`፣ `sm2`፣ `sm3`፣ `sm4`፣ እና `sm4-gcm`። (`rfc6979` አስቀድሞ የመነሻ ግራፍ አካል ነበር።)
- ጥቅም ላይ ላልዋለ የ NEON ፖሊሲ ረዳቶች የግንባታ ማስጠንቀቂያዎች ይቀጥላሉ; የመለኪያ ማለስለስ አሂድ ጊዜ እነዚያን የኮድ ዱካዎች እንደገና እስኪያነቃቸው ድረስ እንዳለ ይተውት።