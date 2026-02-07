---
lang: am
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2025-12-29T18:16:35.937817+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM OpenSSL/Tongsuo Provenance ቅጽበተ ፎቶ
% የተፈጠረ፡ 2026-01-30

# የአካባቢ ማጠቃለያ

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`፡ ሪፖርቶች `LibreSSL 3.3.6` (በማክኦኤስ ላይ በስርዓት የቀረበ TLS Toolkit)።
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`፡ ለትክክለኛው የዝገት ጥገኛ ቁልል (`openssl` crate v0.10.74፣ `openssl-sys` v0.9.x፣ የተሸጠ የOpenSSL 3.00008 በC00X0 ምንጮች በC00X0 `vendored` በ`crates/iroha_crypto/Cargo.toml` ውስጥ የነቃ ቅድመ እይታ ግንባታዎች)።

# ማስታወሻዎች

- ከ LibreSSL ራስጌዎች/ቤተ-መጻሕፍት ጋር የአካባቢ ልማት አካባቢ ግንኙነቶች; የምርት ቅድመ እይታ ግንባታዎች OpenSSL>= 3.0.0 ወይም Tongsuo 8.x መጠቀም አለባቸው። የመጨረሻውን የቅርስ ቅርቅብ በሚያመነጩበት ጊዜ የስርዓት መገልገያ ኪቱን ይተኩ ወይም `OPENSSL_DIR`/`PKG_CONFIG_PATH` ያዘጋጁ።
- ትክክለኛውን የOpenSSL/Tongsuo ታርቦል ሃሽ (`openssl version -v`፣ `openssl version -b`፣ `openssl version -f`) ለመያዝ እና ሊባዛ የሚችለውን የግንባታ ስክሪፕት/ቼክም ለማያያዝ ይህንን ቅጽበታዊ ገጽ እይታ በሚለቀቅበት አካባቢ ውስጥ ያድሱት። ለሽያጭ ግንባታዎች፣ በካርጎ ጥቅም ላይ የዋለውን የ`openssl-src` crate ስሪት/commit ይመዝግቡ (በ`target/debug/build/openssl-sys-*/output` ውስጥ የሚታየው)።
- የApple Silicon አስተናጋጆች የOpenSSL ጭስ ማውጫን ሲሰሩ `RUSTFLAGS=-Aunsafe-code` ያስፈልጋቸዋል ስለዚህ AArch64 SM3/SM4 acceleration stubs (ውስጠ-ቁሳቁሶቹ በማክሮስ ላይ አይገኙም)። `scripts/sm_openssl_smoke.sh` ስክሪፕት CI እና የሀገር ውስጥ ሩጫዎች ወጥነት እንዲኖራቸው ለማድረግ `cargo` ከመጥራቱ በፊት ይህንን ባንዲራ ወደ ውጭ ይልካል።
- የማሸጊያው ቧንቧ ከተሰካ በኋላ ወደ ላይ ያለውን ምንጭ (ለምሳሌ `openssl-src-<ver>.tar.gz` SHA256) ያያይዙ; በCI artefacts ውስጥ ተመሳሳይ ሃሽ ይጠቀሙ።