---
lang: am
direction: ltr
source: docs/source/crypto/sm_program.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 08e2e1e4a54390d9142d6788aad2385e93282a33423b9fc7f3418e3633f3f86a
source_last_modified: "2026-01-23T23:46:10.134857+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM2/SM3/SM4 የነቃ አርክቴክቸር አጭር ለHyperledger Iroha v2.

# SM ፕሮግራም አርክቴክቸር አጭር መግለጫ

#ዓላማ
ቆራጥ አፈጻጸምን እና ኦዲትነትን በመጠበቅ በIroha v2 ቁልል ላይ የቻይና ብሄራዊ ክሪፕቶግራፊ (SM2/SM3/SM4) ለማስተዋወቅ የቴክኒክ እቅድ፣ የአቅርቦት ሰንሰለት አቀማመጥ እና የአደጋ ድንበሮችን ይግለጹ።

## ወሰን
- ** የጋራ መግባባት-ወሳኝ መንገዶች:** `iroha_crypto`, `iroha`, `irohad`, IVM አስተናጋጅ, Kotodama ውስጣዊ ነገሮች.
- ** የደንበኛ ኤስዲኬዎች እና መጠቀሚያዎች፡** Rust CLI፣ Kagami፣ Python/JS/Swift SDKs፣ ዘፍጥረት መገልገያዎች።
- ** ውቅር እና ተከታታይነት፡** `iroha_config` knobs፣ Norito የውሂብ ሞዴል መለያዎች፣ የሰነድ አያያዝ፣ የመልቲኮድ ማሻሻያ።
**ሙከራ እና ተገዢነት፡** ክፍል/ንብረት/የኢንተርሮፕ ስብስቦች፣ Wycheproof መታጠቂያዎች፣ የአፈጻጸም መገለጫ፣ ኤክስፖርት/የቁጥጥር መመሪያ። *(ሁኔታ፡- RustCrypto-የተደገፈ SM ቁልል ተዋህዷል፤ አማራጭ `sm_proptest` fuzz suite እና OpenSSL የተመጣጣኝ ልጓም ለተራዘመ CI ይገኛል።)*

ከወሰን ውጭ፡ የPQ ስልተ ቀመሮች፣ በስምምነት መንገዶች ውስጥ የማይወሰን አስተናጋጅ ማጣደፍ; wasm/`no_std` ግንባታዎች ጡረታ ወጥተዋል።

## አልጎሪዝም ግብዓቶች እና ማቅረቢያዎች
| አርቲፊሻል | ባለቤት | የሚከፈልበት | ማስታወሻ |
|-------|-------|-----|------|
| SM አልጎሪዝም ባህሪ ንድፍ (`SM-P0`) | Crypto WG | 2025-02 | ባህሪ ጌቲንግ፣ ጥገኝነት ኦዲት፣ የአደጋ መመዝገቢያ። |
| ኮር ዝገት ውህደት (`SM-P1`) | Crypto WG / የውሂብ ሞዴል | 2025-03 | RustCrypto ላይ የተመሰረተ ማረጋገጫ/hash/AEAD ረዳቶች፣ Norito ቅጥያዎች፣ ቋሚዎች። |
| መፈረም + ቪኤም ሲስካልስ (`SM-P2`) | IVM ኮር / ኤስዲኬ ፕሮግራም | 2025-04 | ቆራጥ የመፈረሚያ መጠቅለያዎች፣ syscals፣ Kotodama ሽፋን። |
| አማራጭ አቅራቢ እና ኦፕስ ማንቃት (`SM-P3`) | መድረክ ኦፕስ / አፈጻጸም WG | 2025-06 | OpenSSL/Tongsuo backend፣ ARM intrinsics፣ telemetry፣ documentation። |

## የተመረጡ ቤተ መጻሕፍት
- ** ዋና፡** RustCrypto crates (`sm2`፣ `sm3`፣ `sm4`) በ `rfc6979` ባህሪ የነቃ እና SM3 ከማይታወቅ ነገር ጋር የተያያዘ ነው።
- **አማራጭ FFI፡** OpenSSL 3.x አቅራቢ API ወይም Tongsuo የተመሰከረላቸው ቁልል ወይም የሃርድዌር ሞተሮች ለሚፈልጉ ማሰማራት፤ በባህሪ-የተዘጋ እና በነባሪነት በስምምነት ሁለትዮሽ ውስጥ ተሰናክሏል።### የኮር ቤተ መፃህፍት ውህደት ሁኔታ
- `iroha_crypto::sm` የSM3 hashingን፣ SM2 ማረጋገጫን እና SM4 GCM/CCM ረዳቶችን በተዋሃደ የ`sm` ባህሪ፣ በቋሚ RFC6979 የመፈረሚያ መንገዶችን ለኤስዲኬዎች ያጋልጣል። `Sm2PrivateKey`.【crates/iroha_crypto/src/sm.rs:1049】【crates/iroha_crypto/src/sm.rs:1128】【crates/iroha_crypto/src/sm.rs:1236】
- Norito/Norito-JSON መለያዎች እና መልቲኮዴክ አጋዥዎች የSM2 የህዝብ ቁልፎችን/ፊርማዎችን እና የSM3/SM4 ጭነቶችን ይሸፍናሉ ስለዚህም መመሪያው በተከታታይ የሚወሰን ይሆናል። hosts.【crates/iroha_data_model/src/isi/registry.rs:407】【crates/iroha_data_model/tess/sm_norito_roundtrip.rs:12】
- የታወቁ መልስ ስብስቦች የ RustCrypto ውህደትን (`sm3_sm4_vectors.rs`፣ `sm2_negative_vectors.rs`) ያረጋግጣሉ እና እንደ CI's `sm` ባህሪ ስራዎች አካል ሆነው ይሰራሉ፣ አንጓዎች መፈረም በሚቀጥሉበት ጊዜ የማረጋገጫ ቆራጥነትን ይጠብቃሉ። Ed25519.【crates/iroha_crypto/tests/sm3_sm4_vectors.rs:15】【crates/iroha_crypto/tests/sm2_negative_vectors.rs:1】
- አማራጭ `sm` ባህሪ ግንባታ ማረጋገጫ: `cargo check -p iroha_crypto --features sm --locked` (ቀዝቃዛ 7.9s / ሞቅ 0.23s) እና `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` (1.0s) ሁለቱም ተሳክተዋል; ባህሪውን ማንቃት 11 ሳጥኖችን ይጨምራል (`base64ct` ፣ `ghash` ፣ `opaque-debug` ፣ `pem-rfc7468` ፣ `pkcs8` ፣ `polyval` ፣00 `sm2`፣ `sm3`፣ `sm4`፣ `sm4-gcm`)። ግኝቶች በ`docs/source/crypto/sm_rustcrypto_spike.md` ተመዝግበዋል.【docs/source/crypto/sm_rustcrypto_spike.md:1】
- BouncyCastle/GmSSL አሉታዊ የማረጋገጫ ዕቃዎች በ`crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json` ስር ይኖራሉ፣ ቀኖናዊ ውድቀት ጉዳዮችን (r=0፣ s=0፣ መለየት-መታወቂያ አለመዛመድ፣ የተነካ የህዝብ ቁልፍ) በስፋት ከተሰራጩት ጋር ይቆያሉ አቅራቢዎች።【crates/iroha_crypto/tests/sm2_negative_vectors.rs:1】【crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json:1】
- `sm-ffi-openssl` አሁን የተሸጠውን OpenSSL 3.x toolchain (`openssl` crate `vendored` ባህሪን) ያጠናቅራል ስለዚህ ቅድመ እይታ ይገነባል እና ይሞከራል ስርዓቱ LibreSSL/OpenSSL ኤስኤምኤስ በማይጎድልበት ጊዜም ሁልጊዜ ዘመናዊ የኤስኤምኤስ አቅም ያለው አቅራቢን ያነጣጠረ ነው። አልጎሪዝም።【crates/iroha_crypto/Cargo.toml:59】
- `sm_accel` አሁን AArch64 NEONን በሂደት ያገኝና የ SM3/SM4 መንጠቆቹን በ x86_64/RISC-V መላክ በኩል ይከርክታል የውቅረት ቁልፍ `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`)። የቬክተር የኋላ ጫፎች በማይኖሩበት ጊዜ ላኪው አሁንም በ scalar RustCrypto ዱካ በኩል ስለሚሄድ አግዳሚ ወንበሮች እና የፖሊሲ መቀየሪያዎች በአስተናጋጆች ላይ ያለማቋረጥ ባህሪ እንዲኖራቸው ያደርጋል።【crates/iroha_crypto/src/sm.rs:702】【crates/iroha_crypto/src/sm.rs:733】

### Norito መርሐግብር እና የውሂብ-ሞዴል ወለል| Norito አይነት / ሸማች | ውክልና | ገደቦች እና ማስታወሻዎች |
|----------------------------------|
| `Sm3Digest` (`iroha_crypto::Sm3Digest`) | ባዶ፡ 32-ባይት ብሎብ · JSON፡ አቢይ ሆክስ ገመድ (`"4F4D..."`) | ቀኖናዊ Norito tuple wrapping `[u8; 32]`. JSON/Bare ዲኮዲንግ ርዝማኔዎችን ውድቅ ያደርጋል ≠32. በ`sm_norito_roundtrip::sm3_digest_norito_roundtrip` የተሸፈነ ዙር ጉዞዎች። |
| `Sm2PublicKey` / `Sm2Signature` | መልቲኮዴክ ቅድመ ቅጥያ ብሎብስ (`0x1306` ጊዜያዊ) | የህዝብ ቁልፎች ያልተጨመቁ SEC1 ነጥቦችን ያመለክታሉ; ፊርማዎች `(r∥s)` (እያንዳንዳቸው 32 ባይት) ከDER መተንተን ጠባቂዎች ጋር ናቸው። |
| `Sm4Key` | ባዶ፡ 16-ባይት ብሎብ | ለKotodama/CLI የተጋለጠ ዜሮ ማድረግ። JSON ተከታታይነት ሆን ተብሎ ተትቷል; ኦፕሬተሮች ቁልፎችን በብሎብስ (ኮንትራቶች) ወይም በ CLI hex (`--key-hex`) ማለፍ አለባቸው። |
| `sm4_gcm_seal/open` ኦፔራዎች | የ 4 blobs መካከል Tuple: `(key, nonce, aad, payload)` | ቁልፍ = 16 ባይት; nonce = 12 ባይት; የመለያ ርዝመት በ16 ባይት ተስተካክሏል። `(ciphertext, tag)` ይመልሳል; Kotodama/CLI ሁለቱንም ሄክስ እና ቤዝ64 አጋዥዎችን ያመነጫል።【crates/ivm/tests/sm_syscalls.rs:728】 |
| `sm4_ccm_seal/open` ኦፔራዎች | Tuple of 4 blobs (ቁልፍ፣ ኖንስ፣ አድ፣ ሎድ) + የመለያ ርዝመት በ`r14` | ምንም 7-13 ባይት; የመለያ ርዝመት ∈ {4,6,8,10,12,14,16}። `sm` ባህሪ CCM ከ `sm-ccm` ባንዲራ ጀርባ ያጋልጣል። |
| Kotodama intrinsics (`sm::hash`፣ `sm::seal_gcm`፣ `sm::open_gcm`፣ …) | ካርታ ከላይ ወደ SCALL | የግቤት ማረጋገጫ መስተዋቶች አስተናጋጅ ደንቦች; የተበላሹ መጠኖች `ExecutionError::Type` ያሳድጋሉ። |

ፈጣን ማጣቀሻ አጠቃቀም;
- ** SM3 በኮንትራቶች/በሙከራዎች ውስጥ:** `Sm3Digest::hash(b"...")` (ዝገት) ወይም Kotodama `sm::hash(input_blob)`። JSON 64 ሄክስ ቁምፊዎችን ይጠብቃል።
- **SM4 AEAD በ CLI በኩል፡** `iroha tools crypto sm4 gcm-seal --key-hex <32 hex> --nonce-hex <24 hex> --plaintext-hex …` ሄክስ/base64 የምስጥር ጽሑፍ+መለያ ጥንዶችን ይሰጣል። በሚዛመደው `gcm-open` ዲክሪፕት ያድርጉ።
- **ባለብዙ-ኮዴክ ሕብረቁምፊዎች፡** SM2 ይፋዊ ቁልፎች/ፊርማዎች በ`PublicKey::from_str`/`Signature::from_bytes` ተቀባይነት ካለው ባለብዙ ቤዝ ሕብረቁምፊ ጋር በማጣመር Norito ማኒፌስት እና መለያ መታወቂያ SM ፈራሚዎችን እንዲይዙ ያስችለዋል።

የዳታ ሞዴል ተጠቃሚዎች የSM4 ቁልፎችን እና መለያዎችን እንደ ጊዜያዊ ብሎብስ አድርገው መያዝ አለባቸው። በሰንሰለት ላይ ጥሬ ቁልፎችን በጭራሽ አትቀጥል። ኮንትራቶች ኦዲት ማድረግ በሚያስፈልግበት ጊዜ የምስጢር ጽሑፍ/መለያ ውጤቶች ወይም የተገኙትን (ለምሳሌ የቁልፉ SM3) ብቻ ማከማቸት አለባቸው።

### የአቅርቦት ሰንሰለት እና ፈቃድ አሰጣጥ
| አካል | ፍቃድ | ቅነሳ |
|--------|---------|-----------|
| `sm2`፣ `sm3`፣ `sm4` | Apache-2.0 / MIT | የወራጅ ወንጀሎችን ይከታተሉ፣ የመቆለፊያ ልቀቶች አስፈላጊ ከሆነ አቅራቢ፣ አረጋጋጭ GA ከመፈረሙ በፊት የሶስተኛ ወገን ኦዲት ያስይዙ። |
| `rfc6979` | Apache-2.0 / MIT | ቀድሞውኑ በሌሎች ስልተ ቀመሮች ውስጥ ጥቅም ላይ ውሏል; ከSM3 መፍጨት ጋር የሚወስን `k` ማሰርን ያረጋግጡ። |
| አማራጭ OpenSSL/Tongsuo | Apache-2.0 / BSD-ቅጥ | ከ `sm-ffi-openssl` ባህሪ ጀርባ አቆይ፣ ግልጽ የሆነ የኦፕሬተር መርጦ መግቢያ እና የማሸጊያ ማመሳከሪያ ዝርዝርን ጠይቅ። |### ባህሪ ባንዲራዎች እና ባለቤትነት
| ወለል | ነባሪ | ማቆያ | ማስታወሻ |
|--------|--------|-----------|---|
| `iroha_crypto/sm-core`፣ `sm-ccm`፣ `sm` | ጠፍቷል | Crypto WG | RustCrypto SM primitivesን ያነቃል። `sm` የተረጋገጠ ምስጠራ ለሚያስፈልጋቸው ደንበኞች የCCM ረዳቶችን ያጠባል። |
| `ivm/sm` | ጠፍቷል | IVM ኮር ቡድን | SM syscals (`sm3_hash`፣ `sm2_verify`፣ `sm4_gcm_*`፣ `sm4_ccm_*`) ይገነባል። አስተናጋጅ gating የሚመጣው ከ`crypto.allowed_signing` (የ`sm2` መገኘት) ነው። |
| `iroha_crypto/sm_proptest` | ጠፍቷል | QA / Crypto WG | የተበላሹ ፊርማዎችን/መለያዎችን የሚሸፍን የንብረት-ሙከራ ማሰሪያ። የነቃው በተራዘመ CI ውስጥ ብቻ ነው። |
| `crypto.allowed_signing` + `default_hash` | `["ed25519"]`, `blake2b-256` | WG / ኦፕሬተሮች WG ያዋቅሩ | የ `sm2` እና `sm3-256` ሃሽ መገኘት የኤስኤም ሲስካልስ/ፊርማዎችን ያነቃል። `sm2` ን ማስወገድ ወደ ማረጋገጫ-ብቻ ሁነታ ይመለሳል። |
| አማራጭ `sm-ffi-openssl` (ቅድመ እይታ) | ጠፍቷል | መድረክ ኦፕስ | የቦታ ያዥ ባህሪ ለOpenSSL/Tongsuo አቅራቢ ውህደት; የእውቅና ማረጋገጫ እና ማሸግ SOPs እስከሚወርድ ድረስ እንደተሰናከለ ይቆያል። |

የአውታረ መረብ ፖሊሲ አሁን `network.require_sm_handshake_match` እና ያጋልጣል
`network.require_sm_openssl_preview_match` (ሁለቱም ለ `true` ነባሪ)። የትኛውንም ባንዲራ ማጽዳት ይፈቅዳል
Ed25519-ብቻ ታዛቢዎች ከኤስኤም የነቁ አረጋጋጮች ጋር የሚገናኙበት ድብልቅ ማሰማራት; አለመመጣጠን ናቸው።
በ `WARN` ላይ ገብቷል፣ ነገር ግን የጋራ መግባባት አንጓዎች ድንገተኛ ሁኔታን ለመከላከል ነባሪዎችን ማቆየት አለባቸው
በSM-Aware እና SM- Disabled እኩዮች መካከል ያለው ልዩነት።
CLI እነዚህን መቀያየሪያዎች በ`iroha_cli app sorafs የእጅ መጨባበጥ ዝማኔ በኩል ያደርጋቸዋል።
--ፍቀድ-sm-እጅ መጨባበጥ- አለመዛመድ` and `--ፍቀድ-sm-ክፈት-ቅድመ-እይታ- አለመዛመድ`, or the matching `-- ያስፈልጋል-*`
ጥብቅ አፈፃፀምን ለመመለስ ባንዲራዎች.#### OpenSSL/Tongsuo ቅድመ እይታ (`sm-ffi-openssl`)
- **Scope የጋራ ስምምነት ሁለትዮሾች የ RustCrypto መንገድን መጠቀም መቀጠል አለባቸው; የኤፍኤፍአይ ጀርባ ለጫፍ ማረጋገጫ/አብራሪዎች ለመፈረም በጥብቅ መርጦ ገብቷል።
- ** ቅድመ ሁኔታዎችን ይገንቡ።** ከ`cargo build -p iroha_crypto --features "sm sm-ffi-openssl"` ጋር ያሰባስቡ እና የመሳሪያ ሰንሰለት አገናኞችን ከOpenSSL/Tongsuo 3.0+ (`libcrypto` በSM2/SM3/SM4 ድጋፍ) ያረጋግጡ። የማይንቀሳቀስ ማገናኘት አይበረታታም; በኦፕሬተሩ የሚተዳደሩ ተለዋዋጭ ቤተ-መጻሕፍትን ይመርጣሉ።
- ** የገንቢ ጭስ ሙከራ።** `cargo check -p iroha_crypto --features "sm sm-ffi-openssl"` ን ለማስፈጸም `scripts/sm_openssl_smoke.sh` ን ያሂዱ `cargo test -p iroha_crypto --features "sm sm-ffi-openssl" --test sm_openssl_smoke -- --nocapture`; ረዳቱ የOpenSSL ≥3 ልማት ራስጌዎች በማይገኙበት ጊዜ (ወይም `pkg-config` ሲጎድል) እና ጭስ ውፅዓት ላይ ገንቢዎች የSM2 ማረጋገጫ ወደ ዝገት ትግበራ መሄዱን ወይም መውደቁን እንዲያዩ በራስ-ሰር ይዘላል።
- ** ዝገት ስካፎልዲንግ።** የ`openssl_sm` ሞጁል አሁን SM3 hashing፣ SM2 ማረጋገጫ (ZA prehash + SM2 ECDSA) እና SM4 GCM ኢንክሪፕት/ዲክሪፕት በማድረግ በOpenSSL በኩል የቅድመ እይታ መቀያየርን እና ልክ ያልሆነ የቁልፍ/nonce/መለያ ርዝመትን የሚሸፍኑ የተዋቀሩ ስህተቶች። SM4 CCM ንፁህ-ዝገት-ብቻ እስከ ተጨማሪ FFI ሺምስ መሬት ድረስ ይቆያል።
- ** ባህሪን ዝለል።** OpenSSL ≥3.0 ራስጌዎች ወይም ቤተ-መጻሕፍት በማይኖሩበት ጊዜ የጭስ ሙከራው መዝለል ባነር ያትማል (በ`-- --nocapture`) ነገር ግን አሁንም በተሳካ ሁኔታ ይወጣል ስለዚህ CI የአካባቢ ክፍተቶችን ከእውነተኛ መመለሻዎች መለየት ይችላል።
- ** የሩጫ ጊዜ መከላከያ መንገዶች።** የOpenSSL ቅድመ እይታ በነባሪነት ተሰናክሏል። የFFI ዱካውን ለመጠቀም ከመሞከርዎ በፊት በማዋቀር (`crypto.enable_sm_openssl_preview` / `OpenSslProvider::set_preview_enabled(true)`) ያንቁት። አቅራቢው እስኪመረቅ ድረስ የምርት ስብስቦችን በማረጋገጫ-ብቻ ሁነታ ያቆዩ (`sm2` ከ `allowed_signing` ውጣ)፣ በወሳኙ የRustCrypto ውድቀት ላይ ተመርኩዞ አብራሪዎችን በገለልተኛ አካባቢዎች ይገድቡ።
- **የማሸጊያ ማመሳከሪያዎች።** የአቅራቢውን ሥሪት፣ የመጫኛ ዱካ እና የአቋም መግለጫዎችን በማሰማራት ላይ ይመዝግቡ። ኦፕሬተሮች የተፈቀደውን የOpenSSL/Tongsuo ግንባታን የሚጭኑ፣ በስርዓተ ክወና ትረስት ማከማቻ (ከተፈለገ) እና ከጥገና መስኮቶች ጀርባ ፒን ማሻሻያዎችን የሚጭኑ የመጫኛ ስክሪፕቶችን ማቅረብ አለባቸው።
- **ቀጣይ ደረጃዎች።** የወደፊት እመርታዎች ወሳኙ SM4 CCM FFI ማያያዣዎች፣ CI ጭስ ስራዎች (`ci/check_sm_openssl_stub.sh` ይመልከቱ) እና ቴሌሜትሪ ይጨምራሉ። ሂደትን በSM-P3.1.x በ`roadmap.md` ይከታተሉ።

#### የኮድ ባለቤትነት ቅጽበታዊ ገጽ እይታ
- ** ክሪፕቶ WG: *** `iroha_crypto` ፣ SM መጫዎቻዎች ፣ ተገዢነት ሰነዶች።
- **IVM ኮር፡** syscall ትግበራዎች፣ Kotodama ኢንትሪንሲክስ፣ አስተናጋጅ ጋቲንግ።
- ** አዋቅር WG:** ኩንትኒቲ `crypto.allowed_signing`/`default_hash`፣ ሎሊደስቲየት ምንኒትስት፣ ሂዩት ኬብላህ።
- ** የኤስዲኬ ፕሮግራም፡** SM-aware tooling በመላ CLI/Kagami/ኤስዲኬዎች፣ የጋራ መገልገያዎች።
- **የፕላትፎርም ኦፕስ እና አፈጻጸም WG፡** የማጣደፍ መንጠቆዎች፣ ቴሌሜትሪ፣ ኦፕሬተር ማስቻል።

## ማዋቀር የፍልሰት መጫወቻ መጽሐፍከኤድ25519-ብቻ አውታረ መረቦች ወደ SM የነቁ ማሰማራት የሚሄዱ ኦፕሬተሮች መሆን አለባቸው
የሂደቱን ሂደት ይከተሉ
[`sm_config_migration.md`](sm_config_migration.md)። መመሪያው መገንባትን ይሸፍናል
ማረጋገጫ፣ `iroha_config` ንብርብር (`defaults` → `user` → `actual`) ፣ ዘፍጥረት
በ`kagami` በኩል እንደገና መወለድ (ለምሳሌ `kagami genesis generate --allowed-signing sm2 --default-hash sm3-256`) ፣ የቅድመ-በረራ ማረጋገጫ እና መልሶ መመለስ
ማቀድ ስለዚህ የውቅረት ቅጽበተ-ፎቶዎች እና መግለጫዎች በሁሉም ውስጥ ወጥነት ይኖራቸዋል
መርከቦች.

## የመወሰን ፖሊሲ
- በኤስዲኬዎች ውስጥ ለሚገኙ ሁሉም የSM2 መመዝገቢያ መንገዶች እና አማራጭ አስተናጋጅ ፊርማ RFC6979-የመጡ ያልሆኑትን ማስፈጸም፤ አረጋጋጮች ቀኖናዊ r∥s ኢንኮዲንግ ብቻ ይቀበላሉ።
- የመቆጣጠሪያ-አውሮፕላን ግንኙነት (ዥረት) Ed25519 ይቀራል; አስተዳደር መስፋፋትን ካልፈቀደ በስተቀር SM2 በውሂብ-አውሮፕላን ፊርማዎች ብቻ የተወሰነ።
- ኢንትሪንሲክስ (ARM SM3/SM4) የሚወስን የማረጋገጫ/ሃሽ ኦፕሬሽኖችን ከሩጫ ጊዜ ባህሪ ማወቂያ እና ከሶፍትዌር ውድቀት ጋር የተገደበ።

## Norito እና ኢንኮዲንግ እቅድ
1. አልጎሪዝም ቁጥሮችን በ `iroha_data_model` በ `Sm2PublicKey`፣ `Sm2Signature`፣ `Sm3Digest`፣ `Sm4Key` ያራዝሙ።
2. የ SM2 ፊርማዎችን እንደ ትልቅ-ኤንዲያን ቋሚ ስፋት `r∥s` ድርድሮች (32+32 ባይት) የ DER አሻሚዎችን ለማስወገድ ተከታታይ ማድረግ; በአመቻቾች ውስጥ የሚስተናገዱ ልወጣዎች። * (ተከናውኗል፡ በ`Sm2Signature` ረዳቶች ውስጥ ተተግብሯል፤ Norito/JSON የዙር ጉዞዎች በቦታው።)*
3. መልቲኮዴክ ለዪዎችን (`sm3-256`፣ `sm2-pub`፣ `sm4-key`) መልቲ ፎርማቶችን ከተጠቀሙ፣ መለዋወጫዎችን እና ሰነዶችን ያዘምኑ። *(ሂደት፡ `sm2-pub` ጊዜያዊ ኮድ `0x1306` አሁን በተገኙ ቁልፎች የተረጋገጠ፣የመጨረሻ ምደባ በመጠባበቅ ላይ ያሉ SM3/SM4 ኮዶች፣በ`sm_known_answers.toml` በኩል ክትትል የሚደረግባቸው።)*
4. የ Norito ወርቃማ ሙከራዎችን ያዘምኑ ጉዞዎችን የሚሸፍኑ እና የተበላሹ ኢንኮዲንግ (አጭር/ረዥም r ወይም s፣ ልክ ያልሆኑ የጥምዝ መለኪያዎች)።## አስተናጋጅ እና ቪኤም ውህደት እቅድ (SM-2)
1. ነባሩን GOST hash shim የሚያንጸባርቅ የአስተናጋጅ-ጎን `sm3_hash` syscall ን ይተግብሩ; `Sm3Digest::hash` ን እንደገና መጠቀም እና የመወሰን የስህተት መንገዶችን አጋልጥ። *(የወረደው፡ አስተናጋጁ ብሎብ TLVን ይመልሳል፤ `DefaultHost` ትግበራን እና `sm_syscalls.rs` regression ይመልከቱ።)*
2. ቀኖናዊ አርአዊ ፊርማዎችን የሚቀበል፣ የመለየት መታወቂያዎችን የሚያረጋግጥ እና የመመለሻ ኮዶችን ለመወሰን አለመሳካትን የVM syscall ሰንጠረዥን በ`sm2_verify` ያራዝሙ። *(ተከናውኗል፡ አስተናጋጅ + Kotodama intrinsics return `1/0`፤ regression suite አሁን የተቆራረጡ ፊርማዎችን፣ የተበላሹ የህዝብ ቁልፎችን፣ የብሎብ ቲኤልቪዎችን፣ እና UTF-8/ባዶ/የማይዛመድ Kotodama.*44
3. የ`sm4_gcm_seal`/`sm4_gcm_open` (እና እንደ አማራጭ CCM) syscallsን ከግልጽ ያልሆነ/የመለያ መጠን (RFC 8998) ያቅርቡ። *(ተከናውኗል፡ GCM ቋሚ ባለ 12-ባይት ኖንስ + 16-ባይት መለያዎችን ይጠቀማል፤ CCM 7-13 ባይት ኖንስን ይደግፋል የመለያ ርዝመቶች {4,6,8,10,12,14,16} በ`r14`; `r14`; Kotodama እነዚህን እንደ000000910199 Kotodama. ሽቦ Kotodama የጭስ ኮንትራቶች እና IVM ውህደት ሙከራዎች አወንታዊ እና አሉታዊ ጉዳዮችን የሚሸፍኑ (የተቀየሩ መለያዎች ፣ የተበላሹ ፊርማዎች ፣ የማይደገፉ ስልተ ቀመሮች)። *(በ`crates/ivm/tests/kotodama_sm_syscalls.rs` በማንጸባረቅ አስተናጋጅ ሪግሬሽን ለSM3/SM2/SM4 ተከናውኗል።)*
5. syscall የፈቃድ ዝርዝሮችን፣ ፖሊሲዎችን እና ABI ሰነዶችን (`crates/ivm/docs/syscalls.md`) ያዘምኑ እና አዲሶቹን ግቤቶች ከጨመሩ በኋላ የሃሽ መግለጫዎችን ያድሱ።

### አስተናጋጅ እና ቪኤም ውህደት ሁኔታ
- DefaultHost፣ CoreHost እና WsvHost የSM3/SM2/SM4 ሲሲካል አጋልጠው በ `sm_enabled` ላይ በራቸው፣ የአሂድ ጊዜ ባንዲራ ሲሆን `PermissionDenied` በመመለስ ሐሰት።【crates/ivm/src/host.rs:915】【crates/ivm/src/core_host.rs:833】【crates/ivm/src/mock_wsv.rs:2307】
- `crypto.allowed_signing` gating በቧንቧ መስመር / አስፈፃሚ / ግዛት ውስጥ በክር ነው ስለዚህ የምርት አንጓዎች በማዋቀር በኩል deterministically መርጠዋል; `sm2` በማከል የኤስኤም ረዳት መገኘትን ይቀያይራል::
- የድጋሚ ሽፋን መልመጃዎች ሁለቱንም የነቁ እና የተሰናከሉ መንገዶችን (DefaultHost/CoreHost/WsvHost) ለSM3 hashing፣ SM2 ማረጋገጫ፣ እና SM4 GCM/CCM ማህተም/ክፍት ፍሰት

## የማዋቀር ክሮች
- `crypto.allowed_signing`፣ `crypto.default_hash`፣ `crypto.sm2_distid_default`፣ እና አማራጭ `crypto.enable_sm_openssl_preview` ወደ `iroha_config` ያክሉ። የውሂብ-ሞዴል ባህሪ የቧንቧ መስታዎቶችን ያረጋግጡ (`iroha_data_model` `sm` → `iroha_crypto/sm` ያጋልጣል)።
የመገለጫ/የዘፍጥረት ፋይሎች የተፈቀዱ ስልተ ቀመሮችን እንዲገልጹ የሽቦ ማዋቀር ወደ የመግቢያ ፖሊሲዎች; መቆጣጠሪያ-አውሮፕላን በነባሪነት Ed25519 ይቀራል።### CLI እና ኤስዲኬ ስራ (SM-3)
1. **Torii CLI** (`crates/iroha_cli`)፡ SM2 keygen/import/export (distid aware)፣ SM3 hashing helpers፣ እና SM4 AEAD encrypt/decrypt ትዕዛዞችን ይጨምሩ። በይነተገናኝ ጥያቄዎችን እና ሰነዶችን ያዘምኑ።
2. **የዘፍጥረት መሳርያ** (`xtask`፣ `scripts/`)፡- የተፈቀደላቸው የመፈረሚያ ስልተ ቀመሮችን እና ነባሪ ሃሽዎችን እንዲያውጁ ይፍቀዱ፣ ኤስኤምኤስ ከተዛመደ የማዋቀሪያ ቁልፎች ሳይሰራ ከነቃ በፍጥነት ይወድቁ። *(ተከናውኗል፡ `RawGenesisTransaction` አሁን `crypto` ብሎክን ከ `default_hash`/`allowed_signing`/`sm2_distid_default`፤ `ManifestCrypto::validate` እና I0100000222X እና I0100000222X እና I010000022X እና I010000022X ነባሪዎች/ዘፍጥረት አንጸባራቂ ቅጽበተ-ፎቶውን ያስተዋውቃል።)*
3. **ኤስዲኬ ወለል**፡
   ዝገት (`iroha_client`)፡- የSM2 ፊርማ/ማረጋገጫ አጋዦችን፣ SM3 hashingን፣ SM4 AEAD መጠቅለያዎችን ከሚወስኑ ነባሪዎች ጋር አጋልጥ።
   - Python/JS/Swift: Rust API ን ያንጸባርቁ; ለቋንቋ አቋራጭ ሙከራዎች በ`sm_known_answers.toml` ውስጥ የታቀዱ ዕቃዎችን እንደገና ይጠቀሙ።
4. በCLI/SDK ፈጣን ጅምር ውስጥ SMን ለማንቃት የኦፕሬተር የስራ ፍሰትን ይመዝግቡ እና የJSON/YAML አወቃቀሮች አዲሱን የአልጎሪዝም መለያዎች መቀበላቸውን ያረጋግጡ።

#### የ CLI እድገት
- `cargo run -p iroha_cli --features sm -- crypto sm2 keygen --distid CN12345678901234` አሁን የSM2 ቁልፍ ጥንድ ከ `client.toml` ቅንጥስ (Kotodama፣ `private_key_hex`፣ `distid`) ጋር የሚገልፅ የJSON ክፍያ ጭነት ይለቃል። ትዕዛዙ `--seed-hex`ን ለሚወስን ትውልድ ይቀበላል እና በአስተናጋጆች ጥቅም ላይ የዋለውን የRFC 6979 አመጣጥ ያንጸባርቃል።
- `cargo xtask sm-operator-snippet --distid CN12345678901234` የኪይጅን/የመላክ ፍሰት ይጠቀልላል፣ ተመሳሳይ የ`sm2-key.json`/`client-sm2.toml` ውጤቶችን በአንድ እርምጃ ይጽፋል። `--json-out <path|->`/`--snippet-out <path|->` ፋይሎችን ለመምራት ወይም ወደ stdout በዥረት ለመልቀቅ የ`jq` ጥገኝነትን ለአውቶሜሽን ያስወግዱ።
- `iroha_cli tools crypto sm2 import --private-key-hex <hex> [--distid ...]` ኦፕሬተሮች ከመግባታቸው በፊት የመለየት መታወቂያዎችን ማረጋገጥ እንዲችሉ አሁን ካለው ቁሳቁስ ተመሳሳይ ሜታዳታ ያገኛል።
- `iroha_cli tools crypto sm2 export --private-key-hex <hex> --emit-json` የውቅር ቅንጣቢውን ያትማል (`allowed_signing`/`sm2_distid_default` መመሪያን ጨምሮ) እና እንደ አማራጭ የJSON ቁልፍ ክምችት ለስክሪፕት እንደገና ያወጣል።
- `iroha_cli tools crypto sm3 hash --data <string>` hashes የዘፈቀደ ጭነት; `--data-hex`/`--file` የሁለትዮሽ ግብአቶችን ይሸፍናል እና ትዕዛዙ ሁለቱንም ሄክስ እና ቤዝ64 ለማንፀባረቅ መሳሪያነት ሪፖርት ያደርጋል።
- `iroha_cli tools crypto sm4 gcm-seal --key-hex <KEY> --nonce-hex <NONCE> --plaintext-hex <PT>` (እና `gcm-open`) የአስተናጋጁን SM4-GCM ረዳቶች እና ላዩን `ciphertext_hex`/`tag_hex` ወይም ግልጽ የጽሑፍ ጭነት። `sm4 ccm-seal` / `sm4 ccm-open` ተመሳሳይ UX ለ CCM ያለ ምንም ርዝመት (7-13 ባይት) እና የመለያ ርዝመት (4,6,8,10,12,14,16) መጋገር; ሁለቱም ትዕዛዞች እንደ አማራጭ ጥሬ ባይት ወደ ዲስክ ይለቃሉ።## የሙከራ ስልት
### ክፍል/የሚታወቅ የመልስ ሙከራዎች
- GM/T 0004 & GB/T 32905 ቬክተር ለ SM3 (ለምሳሌ `"abc"`)።
- GM/T 0002 & RFC 8998 ቬክተሮች ለ SM4 (አግድ + GCM/CCM)።
- GM/T 0003/GB/T 32918 ምሳሌዎች ለSM2 (Z-እሴት፣ የፊርማ ማረጋገጫ)፣ አባሪ ምሳሌ 1 ከመታወቂያ `ALICE123@YAHOO.COM` ጋር ጨምሮ።
- ጊዜያዊ የማረጋገጫ ፋይል: `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`.
- Wycheproof-የተገኘ SM2 regression suite (`crates/iroha_crypto/tests/sm2_wycheproof.rs`) አሁን ባለ 52 መያዣ ኮርፐስ ተሸክሞ የሚወስን ቋሚ ዕቃዎችን (አባሪ D፣ ኤስዲኬ ዘሮች) በቢት-ግልብጥብጥ፣ በመልዕክት-ማታፈር እና በተቆራረጠ-ፊርማ አሉታዊ ጎኖች። የጸዳው JSON የሚኖረው በ`crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` ውስጥ ነው፣ እና `sm2_fuzz.rs` በቀጥታ ይጠቀምበታል ስለዚህ ሁለቱም ደስተኛ-መንገድ እና ተንኮለኛ ሁኔታዎች በፉዝ/ንብረት ሩጫዎች ላይ ይሰለፋሉ። 벡터들은 표준 곡선뿐만 아니라 አባሪ 영역도 다루며, 필요 시 내장 Kotodama 백업 루틴이 추적을 완료합니다.
- `cargo xtask sm-wycheproof-sync --input <wycheproof-sm2.json>` (ወይም `--input-url <https://…>`) ማንኛውንም የወራጅ ጠብታ (የጄነሬተር መለያ አማራጭ አማራጭ) ቆርጦ `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json`ን እንደገና ይጽፋል። C2SP ኦፊሴላዊውን ኮርፐስ እስኪያተም ድረስ ሹካዎችን እራስዎ ያውርዱ እና በረዳት በኩል ይመግቡዋቸው; ገምጋሚዎች በልዩነት ላይ እንዲያስቡ ቁልፎችን፣ ቆጠራዎችን እና ባንዲራዎችን መደበኛ ያደርጋል።
- SM2/SM3 Norito ዙር ጉዞዎች በ`crates/iroha_data_model/tests/sm_norito_roundtrip.rs` የተረጋገጠ።
- SM3 አስተናጋጅ syscall regression በ `crates/ivm/tests/sm_syscalls.rs` (SM ባህሪ)።
- SM2 በ `crates/ivm/tests/sm_syscalls.rs` (ስኬት + ውድቀት ጉዳዮች) ውስጥ የሲሲካል ሪግሬሽን ያረጋግጡ።

### የንብረት እና የመመለሻ ሙከራዎች
- ልክ ያልሆኑ ኩርባዎችን፣ ቀኖናዊ ያልሆኑ r/ሰዎችን እና ያልሆኑትን እንደገና ጥቅም ላይ ለማዋል ለSM2 ፕሮፕትስት። * (በ`crates/iroha_crypto/tests/sm2_fuzz.rs` ውስጥ ይገኛል፣ ከ`sm_proptest` በስተጀርባ የተከለለ፣ በ`cargo test -p iroha_crypto --features "sm sm_proptest"` በኩል አንቃ።)*
- Wycheproof SM4 ቬክተር (ብሎክ / AES-ሁነታ) ለተለያዩ ሁነታዎች ተስማሚ; ለ SM2 ተጨማሪዎች ወደ ላይ ይከታተሉ። `sm3_sm4_vectors.rs` አሁን ለጂሲኤም እና ለሲሲኤም ቢት-ግልብጥብጥ ፣የተቆራረጡ መለያዎች እና የምስክሪፕት ቴክስት ልምምዶችን ያደርጋል።

### Interop & Performance
- RustCrypto ↔ OpenSSL/Tongsuo ፓሪቲ ስብስብ ለ SM2 ምልክት/ማረጋገጥ፣ SM3 ዳይጀስት እና SM4 ECB/GCM በ`crates/iroha_crypto/tests/sm_cli_matrix.rs` ውስጥ ይኖራል። በ`scripts/sm_interop_matrix.sh` ጥራ። የ CCM እኩልነት ቬክተሮች አሁን በ `sm3_sm4_vectors.rs` ውስጥ ይሰራሉ; የCLI ማትሪክስ ድጋፍ አንዴ ወደላይ CLIs CCM ረዳቶችን ካጋለጡ በኋላ ይከተላል።
- SM3 NEON አጋዥ አሁን የ Armv8 መጭመቂያ/ፓዲንግ መንገዱን ከጫፍ እስከ ጫፍ በ runtime gating በ `sm_accel::is_sm3_enabled` (ባህሪ + env በSM3/SM4 ላይ ይገለበጣል) ይሰራል። ወርቃማ ዳይጀስት (ዜሮ/`"abc"`/ረጅም-ብሎክ + የዘፈቀደ ርዝመቶች) እና በግዳጅ የሚሰናከሉ ሙከራዎች ከስኬር RustCrypto backend ጋር ተመሳሳይነት ይኖራቸዋል፣ እና የክሪቴሽን ማይክሮ ቤንች (`crates/sm3_neon/benches/digest.rs`) በ AArch64 አስተናጋጆች ላይ scalar vs NEON throughput ን ይይዛል።
- Perf harness mirroring `scripts/gost_bench.sh` Ed25519/SHA-2 vs SM2/SM3/SM4ን ለማነፃፀር እና የመቻቻል ገደቦችን ለማረጋገጥ።#### Arm64 Baseline (የአካባቢው አፕል ሲሊኮን፣ መስፈርት `sm_perf`፣ የታደሰ 2025-12-05)
- `scripts/sm_perf.sh` አሁን የክሪተሪዮን አግዳሚ ወንበርን ያካሂዳል እና በ`crates/iroha_crypto/benches/sm_perf_baseline.json` (በ aarch64 macOS ላይ የተቀዳ፣ መቻቻል 25% በነባሪ፣ የመነሻ ሜታዳታ አስተናጋጁን ሶስት እጥፍ ይይዛል) ያስገድዳል። አዲሱ `--mode` ባንዲራ መሐንዲሶች ስክሪፕቱን ሳያርትዑ scalar vs NEON vs `sm-neon-force` ዳታ ነጥቦችን እንዲይዙ ያስችላቸዋል። የአሁኑ የቀረጻ ቅርቅብ (ጥሬ JSON + ድምር ማጠቃለያ) በ`artifacts/sm_perf/2026-03-lab/m3pro_native/` ስር ይኖራል እና እያንዳንዱን ጭነት በ`cpu_label="m3-pro-native"` ማህተም ያደርጋል።
- የፍጥነት ሁነታዎች አሁን የስክላር መነሻ መስመርን እንደ ንፅፅር ዒላማ በራስ-ሰር ይምረጡ። `scripts/sm_perf.sh` ክሮች ከ`--compare-baseline/--compare-tolerance/--compare-label` እስከ `sm_perf_check`፣የቤንችማርክ ዴልታዎችን ከስክላር ማመሳከሪያው ጋር በማነፃፀር እና መዘግየቱ ከተዋቀረው ገደብ ሲያልፍ አይሳካም። የፐር-ቤንችማርክ መቻቻል ከመነሻው የንፅፅር ጠባቂን ያንቀሳቅሳል (SM3 በ Apple scalar baseline ላይ በ 12% ተሸፍኗል, የ SM3 ማነፃፀር ዴልታ አሁን መጨናነቅን ለማስቀረት እስከ 70% ድረስ በ scalar ማጣቀሻ ላይ ይፈቅዳል); የሊኑክስ መነሻ መስመሮች ከ`neoverse-proxy-macos` ቀረጻ ወደ ውጭ ስለሚላኩ ተመሳሳዩን የንጽጽር ካርታ እንደገና ይጠቀማሉ እና ሚዲያን የሚለያዩ ከሆነ ከባዶ-ሜታል ኒዮቨርስ ሩጫ በኋላ እናጠባባቸዋለን። ጥብቅ ገደቦችን ሲይዙ (ለምሳሌ `--compare-tolerance 0.20`) `--compare-tolerance` በግልፅ ይለፉ እና አማራጭ ማጣቀሻ አስተናጋጆችን ለማብራራት `--compare-label` ይጠቀሙ።
- በ CI ማመሳከሪያ ማሽን ላይ የተመዘገቡት መሰረታዊ መስመሮች በ `crates/iroha_crypto/benches/sm_perf_baseline_aarch64_macos_scalar.json`, `sm_perf_baseline_aarch64_macos_auto.json` እና `sm_perf_baseline_aarch64_macos_neon_force.json` ውስጥ ይኖራሉ. በ`scripts/sm_perf.sh --mode scalar --write-baseline`፣ `--mode auto --write-baseline`፣ ወይም `--mode neon-force --write-baseline` ያድሷቸው (ከመቅረጽ በፊት `SM_PERF_CPU_LABEL` አዘጋጅ) እና የመነጨውን JSON ከሩጫ ምዝግብ ማስታወሻዎች ጋር በማህደር ያስቀምጡ። ገምጋሚዎች እያንዳንዱን ናሙና ኦዲት ማድረግ እንዲችሉ የተዋሃደውን የረዳት ውፅዓት (`artifacts/.../aggregated.json`) ከPR ጋር ያቆዩት። Linux/Neoverse baselines አሁን በ`sm_perf_baseline_aarch64_unknown_linux_gnu_{mode}.json` ውስጥ ይላካሉ፣ ከ`artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/aggregated.json` (ሲፒዩ መለያ `neoverse-proxy-macos`፣ SM3 መቻቻልን 0.70 ለ aarch64 macOS/Linux ያወዳድሩ)። መቻቻልን ለማጥበብ ሲገኝ በባዶ-ሜታል ኒዮቨርስ አስተናጋጆች ላይ እንደገና ይሮጣል።
- ቤዝላይን JSON ፋይሎች አሁን በየቤንችማርክ የጥበቃ መንገዶችን ለማጠናከር አማራጭ `tolerances` ነገር ሊይዙ ይችላሉ። ምሳሌ፡-
  ```json
  {
    "benchmarks": { "...": 12.34 },
    "tolerances": {
      "sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt": 0.08,
      "sm3_vs_sha256_hash/sm3_hash": 0.12
    }
  }
  ```
  `sm_perf_check` እነዚህን ክፍልፋይ ገደቦች (8% እና 12% በምሳሌ) ይተገበራል፣ አለማቀፋዊ CLI መቻቻልን ላልተዘረዘሩ ማመሳከሪያዎች ሲጠቀም።
- የንፅፅር ጠባቂዎች በንፅፅር መነሻ መስመር `compare_tolerances` ን ማክበር ይችላሉ። ቀዳሚውን `tolerances` ለቀጥታ የመነሻ ፍተሻዎች ጥብቅ በማድረግ የላላ ዴልታ ከስካላር ማጣቀሻ (ለምሳሌ `\"sm3_vs_sha256_hash/sm3_hash\": 0.70` በ scalar baseline) ለመፍቀድ ይህንን ይጠቀሙ።- የተረጋገጠው የአፕል ሲሊኮን መነሻ መስመሮች አሁን በኮንክሪት መከላከያ መንገዶች ይላካሉ፡ SM2/SM4 ኦፕሬሽኖች እንደየልዩነቱ ከ12-20% መንሸራተትን ይፈቅዳሉ፣ የSM3/ChaCha ንፅፅር ግን በ8-12% ተቀምጧል። የ scalar baseline `sm3` መቻቻል አሁን ወደ 0.12 ተጠናክሯል; የ `unknown_linux_gnu` ፋይሎች `neoverse-proxy-macos` ኤክስፖርትን ከተመሳሳይ የመቻቻል ካርታ ጋር ያንፀባርቃሉ (SM3 ን ከ0.70 ጋር ይወዳደሩ) እና የሜታዳታ ማስታወሻዎች ለሊኑክስ በር የሚላኩ ባዶ ብረት ኒዮቨርስ ዳግም መሮጥ እስኪገኝ ድረስ ነው።
- SM2 መፈረም፡ 298µs በአንድ ኦፕ (Ed25519፡ 32µs) ⇒ ~9.2× ቀርፋፋ፤ ማረጋገጫ፡ 267µs (Ed25519፡ 41µs) ⇒ ~6.5× ቀርፋፋ።
- SM3 hashing (4KiB ክፍያ)፡ 11.2µs፣ ከSHA-256 ጋር በ11.3µs (≈356MiB/s vs 353MiB/s) ጋር በትክክል ይዛመዳል።
- SM4-GCM ማኅተም/ክፍት (1ኪቢ ጭነት፣ 12-ባይት ምንም): 15.5µs vs ChaCha20-Poly1305 በ1.78µs (≈64ሚቢ/s vs 525MiB/s)።
- የቤንችማርክ ቅርሶች (`target/criterion/sm_perf*`) ለመራባት ተይዘዋል; የሊኑክስ መነሻ መስመሮች ከ`artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (ሲፒዩ መለያ `neoverse-proxy-macos`፣ SM3 መቻቻል 0.70) እና መቻቻልን ለማጥበብ የላቦራቶሪ ጊዜ ከተከፈተ በኋላ በባሬ ሜታል ኒዮቨርስ አስተናጋጆች (`SM-4c.1`) ላይ መታደስ ይችላሉ።

#### የአርክቴክቸር ቀረጻ ዝርዝር
- `scripts/sm_perf_capture_helper.sh` ** በዒላማው ማሽን ላይ ያሂዱ ** (x86_64 የስራ ቦታ፣ የኒዮቨርስ ARM አገልጋይ ወዘተ)። ቀረጻዎቹን ለማተም `--cpu-label <host>` ይለፉ እና (በማትሪክስ ሁነታ ላይ ሲሰሩ) የመነጨውን እቅድ/የላብራቶሪ መርሐግብር ትዕዛዞችን ቀድመው ለመሙላት። ረዳቱ ሞድ-ተኮር ትዕዛዞችን ያትማል፡-
  1. የክሪተሪዮን ስብስብን ከትክክለኛው የባህሪ ስብስብ ጋር ያስፈጽም
  2. ሚዲያን ወደ `crates/iroha_crypto/benches/sm_perf_baseline_${arch}_${os}_${mode}.json` ይፃፉ።
- መጀመሪያ የስክላር መነሻ መስመርን ያንሱ፣ ከዚያ አጋዥውን ለ`auto` (እና `neon-force` በ AArch64 መድረኮች ላይ) ያሂዱ። ገምጋሚዎች የአስተናጋጅ ዝርዝሮችን በJSON ሜታዳታ ውስጥ መፈለግ እንዲችሉ ትርጉም ያለው `SM_PERF_CPU_LABEL` ይጠቀሙ።
- ከእያንዳንዱ ሩጫ በኋላ ጥሬውን `target/criterion/sm_perf*` ማውጫን በማህደር ያስቀምጡ እና ከተፈጠሩት መነሻዎች ጋር በ PR ውስጥ ያካትቱት። ሁለት ተከታታይ ሩጫዎች ሲረጋጉ የየቤንችማርክ መቻቻልን ያጠናክሩ (ለማጣቀሻ ቅርጸት `sm_perf_baseline_aarch64_macos_*.json` ይመልከቱ)።
- በዚህ ክፍል ውስጥ ያሉትን ሚዲያን + መቻቻልን ይመዝግቡ እና አዲስ አርክቴክቸር ሲሸፈን `status.md`/`roadmap.md` ያዘምኑ። የሊኑክስ መነሻ መስመሮች አሁን ከ `neoverse-proxy-macos` ቀረጻ ተረጋግጠዋል (ሜታዳታ ወደ aarch64-unknown-linux-gnu በር መላክን ያስታውሳል); እነዚያ የላብራቶሪ ክፍተቶች ሲገኙ ለመከታተል በባሬ-ሜታል ኒዮቨርስ/x86_64 አስተናጋጆች ላይ እንደገና ይሮጡ።

#### ARMv8 SM3/SM4 intrinsics vs scalar ዱካዎች
`sm_accel` (`crates/iroha_crypto/src/sm.rs:739` ይመልከቱ) በ NEON ለሚደገፉ SM3/SM4 አጋዥዎች የአሂድ ጊዜ መላኪያ ንብርብር ያቀርባል። ባህሪው በሶስት ደረጃዎች ይጠበቃል.| ንብርብር | ቁጥጥር | ማስታወሻ |
|-------|---------|-------|
| ጊዜ ማጠናቀር | `--features sm` (አሁን በ `sm-neon` በራስ-ሰር በ `aarch64` ይጎትታል) ወይም `sm-neon-force` (ሙከራዎች/መመዘኛዎች) | የ NEON ሞጁሎችን ይገነባል እና `sm3-neon`/`sm4-neon` ያገናኛል። |
| የአሂድ ጊዜ በራስ-አግኝ | `sm4_neon::is_supported()` | የAES/PMULL አቻዎችን በሚያጋልጡ ሲፒዩዎች ላይ ብቻ እውነት ነው (ለምሳሌ፡ Apple M-series፣ Neoverse V1/N2)። NEON ወይም FEAT_SM4ን የሚሸፍኑ ቪኤምዎች ወደ ስኬር ኮድ ይመለሳሉ። |
| ኦፕሬተር መሻር | `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`) | ጅምር ላይ በማዋቀር የሚመራ መላኪያ ተተግብሯል፤ የታመኑ አካባቢዎችን ለማሳየት `force-enable` ብቻ ይጠቀሙ እና scalar fallbacks ሲያረጋግጡ `force-disable`ን ይምረጡ። |

** የአፈጻጸም ፖስታ (Apple M3 Pro; በ `sm_perf_baseline_aarch64_macos_{mode}.json` ውስጥ የተመዘገቡ ሚዲያዎች):**

| ሁነታ | SM3 መፍጨት (4KiB) | SM4-GCM ማኅተም (1ኪቢ) | ማስታወሻ |
|-------------|
| Scalar | 11.6µs | 15.9µs | Deterministic RustCrypto መንገድ; በሁሉም ቦታ ጥቅም ላይ የዋለው የ`sm` ባህሪው ተሰብስቧል ነገር ግን NEON አይገኝም። |
| NEON auto | ~ 2.7× ከ scalar ፈጣን | ~ 2.3× ከ scalar ፈጣን | የአሁኑ የ NEON kernels (SM-5a.2c) መርሐ ግብሩን በአንድ ጊዜ አራት ቃላትን ያሰፋሉ እና ባለሁለት ወረፋ ማራገቢያ ይጠቀሙ; ትክክለኛ ሚድያዎች በእያንዳንዱ አስተናጋጅ ይለያያሉ፣ ስለዚህ የመነሻ መስመሩን JSON ዲበ ዳታ ያማክሩ። |
| NEON ኃይል | መስተዋቶች NEON አውቶሞቢል ግን መመለስን ሙሉ በሙሉ ያሰናክላል | ልክ እንደ NEON auto | የአካል ብቃት እንቅስቃሴ በ `scripts/sm_perf.sh --mode neon-force`; ወደ scalar ሁነታ ነባሪ በሆኑ አስተናጋጆች ላይ እንኳን CI ታማኝ ያደርገዋል። |

**ቆራጥነት እና የማሰማራት መመሪያ**
- ውስጣዊ ነገሮች ሊታዩ የሚችሉ ውጤቶችን ፈጽሞ አይለውጡም-`sm_accel` የተፋጠነው መንገድ በማይኖርበት ጊዜ `None` ይመልሳል ስለዚህም ስኬር ረዳቱ ይሰራል። የስምምነት ኮድ መንገዶች ስኬር አተገባበሩ ትክክል እስከሆነ ድረስ ቆራጥ ሆነው ይቆያሉ።
የ NEON መንገድ ጥቅም ላይ መዋሉን በተመለከተ ** አታድርጉ *** የበር የንግድ ሎጂክን አታድርጉ። ማፋጠንን እንደ ጥሩ ፍንጭ ብቻ ይያዙ እና ሁኔታውን በቴሌሜትሪ ብቻ ያጋልጡ (ለምሳሌ፡ `sm_intrinsics_enabled` መለኪያ)።
- ሁልጊዜ የኤስኤምኤስ ኮድን ከተነኩ በኋላ `ci/check_sm_perf.sh` (ወይም `make check-sm-perf`) ያሂዱ ስለዚህ የመመዘኛ ማጥመጃው በእያንዳንዱ የመነሻ መስመር JSON ውስጥ የተካተቱትን መቻቻል በመጠቀም ሁለቱንም scalar እና የተጣደፉ መንገዶችን ያረጋግጣል።
- ቤንችማርክ ሲያደርጉ ወይም ሲያርሙ `crypto.sm_intrinsics` በማጠናቀር ጊዜ ባንዲራዎች ላይ የማዋቀር ቁልፍን ይምረጡ። ከ `sm-neon-force` ጋር እንደገና ማጠናቀር የስክላር ውድቀትን ሙሉ በሙሉ ያሰናክላል፣ `force-enable` ግን የሩጫ ጊዜን ማወቅን ብቻ ያደርገዋል።
- የተመረጠውን ፖሊሲ በመልቀቂያ ማስታወሻዎች ውስጥ ይመዝግቡ፡ የምርት ግንባታዎች ፖሊሲውን በ`Auto` ውስጥ መተው አለባቸው፣ ይህም እያንዳንዱ አረጋጋጭ ሃርድዌር ችሎታዎችን በተናጥል እንዲያገኝ እና አሁንም ተመሳሳይ ሁለትዮሽ ቅርሶችን እያካፈለ ነው።
- በስታቲስቲክስ የተገናኙ የሻጭ ውስጣዊ መረጃዎችን (ለምሳሌ፣ የሶስተኛ ወገን SM4 ቤተ-መጻሕፍት) ተመሳሳይ የመላኪያ እና የፍተሻ ፍሰትን ካላከበሩ በስተቀር ሁለትዮሾችን ከማጓጓዝ ይቆጠቡ - ያለበለዚያ የ perf regressions በመሠረታዊ መሣሪያችን አይያዙም።#### x86_64 Rosetta መነሻ መስመር (Apple M3 Pro፤ 2025-12-01 ተያዘ)
- የመሠረት መስመሮች በ`crates/iroha_crypto/benches/sm_perf_baseline_x86_64_macos_{scalar,auto,neon_force}.json` (cpu_label=`m3-pro-rosetta`) ውስጥ ይኖራሉ፣ በጥሬ + በ `artifacts/sm_perf/2026-03-lab/m3pro_rosetta/` ስር ያሉ ቀረጻዎች።
- በx86_64 ላይ ያለው የፔንችማርክ መቻቻል ወደ 20% ለSM2፣ 15% ለ Ed25519/SHA-256፣ እና 12% ለSM4/ChaCha ተቀናብሯል። `scripts/sm_perf.sh` አሁን የፍጥነት ንፅፅር መቻቻልን ወደ 25% በ AArch64 ባልሆኑ አስተናጋጆች ላይ ነባሪ ያደርገዋል ስለዚህ scalar-vs-auto በ AArch64 ላይ ያለውን 5.25 slack በመተው ወደ ኒዮቨርስ ዳግም መሮጥ እስኪያበቃ ድረስ።

| ቤንችማርክ | Scalar | መኪና | ኒዮን-ኃይል | Auto vs Scalar | ኒዮን vs Scalar | ኒዮን vs ራስ |
|--------|--------|------|------------|
| sm2_vs_ed25519_ምልክት/ed25519_ምልክት |    57.43 |  57.12 |      55.77 |          -0.53% |         -2.88% |        -2.36% |
| sm2_vs_ed25519_ምልክት/sm2_ምልክት |   572.76 | 568.71 |     557.83 |          -0.71% |         -2.61% |        -1.91% |
| sm2_vs_ed25519_አረጋግጥ/አረጋግጥ/ ed25519 |    69.03 |  68.42 |      66.28 |          -0.88% |         -3.97% |        -3.12% |
| sm2_vs_ed25519_አረጋግጥ/አረጋግጥ/sm2 |   521.73 | 514.50 |     502.17 |          -1.38% |         -3.75% |        -2.40% |
| sm3_vs_sha256_hash/sha256_hash |    16.78 |  16.58 |      16፡16 |          -1.19% |         -3.69% |        -2.52% |
| sm3_vs_sha256_hash/sm3_hash |    15.78 |  15.51 |      15.04 |          -1.71% |         -4.69% |        -3.03% |
| sm4_vs_chacha20poly1305_decrypt/chacha20poly1305_ዲክሪፕት |     1.96 |   1.97 |       1.97 |           0.39% |          0.16% |        -0.23% |
| sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt |    16.26 |  16.38 |      16.26 |           0.72% |         -0.01% |        -0.72% |
| sm4_vs_chacha20poly1305_encrypt/chacha20poly1305_encrypt |     1.96 |   2.00 |       1.93 |           2.23% |         -1.14% |        -3.30% |
| sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt |    16.60 |  16.58 |      16.15 |          -0.10% |         -2.66% |        -2.57% |

#### x86_64 / ሌሎች aarch64 ያልሆኑ ኢላማዎች
- የአሁኑ ግንባታዎች አሁንም በ x86_64 ላይ የሚወስነውን RustCrypto scalar መንገድ ብቻ ይላካሉ; `sm` እንደነቃ ያቆዩት ግን ** አትስሩ** የውጭ AVX2/VAES አስኳሎች እስከ SM-4c.1b መሬት ድረስ አያስገቡ። የአሂድ ጊዜ ፖሊሲ መስተዋቶች ARM፡ ነባሪ የ`Auto`፣ ክብር `crypto.sm_intrinsics` እና ተመሳሳይ የቴሌሜትሪ መለኪያዎችን ያርቁ።
- ሊኑክስ/x86_64 ቀረጻዎች ለመመዝገብ ይቀራሉ; ረዳት በዛ ሃርድዌር ላይ እንደገና ተጠቀም እና ሚዲያን ወደ `sm_perf_baseline_x86_64_unknown_linux_gnu_{mode}.json` ከላይ ካለው የ Rosetta መነሻ መስመሮች እና የመቻቻል ካርታ ጋር ጣል።**የተለመዱ ችግሮች**
1. **ምናባዊ የARM ምሳሌዎች፡** ብዙ ደመናዎች NEONን ያጋልጣሉ ነገር ግን `sm4_neon::is_supported()` የሚያጣራውን የSM4/AES ቅጥያዎችን ይደብቃሉ። በእነዚያ አከባቢዎች ውስጥ ያለውን scalar ዱካ ይጠብቁ እና በዚሁ መሰረት የፐርፍ መነሻ መስመሮችን ይያዙ።
2. **በከፊል መሻሮች፡** የ `crypto.sm_intrinsics` እሴቶችን በሩጫ መካከል መቀላቀል ወደ ወጥነት የለሽ የፐርፍ ንባቦች ይመራል። በሙከራ ትኬቱ ውስጥ የታሰበውን መሻር ይመዝግቡ እና አዲስ መነሻ መስመሮችን ከመያዝዎ በፊት ውቅሩን እንደገና ያስጀምሩ።
3. ** CI እኩልነት፡** NEON ንቁ ሆኖ ሳለ አንዳንድ የማክኦኤስ ሯጮች በተቃራኒ ላይ የተመሰረተ የፐርፍ ናሙና አይፈቅዱም። ገምጋሚዎች ሯጩ እነዚያን ቆጣሪዎች ቢደብቃቸውም የተፋጠነው መንገድ መከናወኑን እንዲያረጋግጡ የ`scripts/sm_perf_capture_helper.sh` ውጤቶች ከPRs ጋር እንደተያያዙ ያቆዩ።
4. **የወደፊት ISA ተለዋጮች (SVE/SVE2):** አሁን ያሉት አስኳሎች የኒዮን ሌይን ቅርጾችን ይወስዳሉ። ወደ SVE/SVE2 ከመላክዎ በፊት፣ CI፣ ቴሌሜትሪ እና ኦፕሬተር ማዞሪያዎች እንዲስተካከሉ ለማድረግ `sm_accel::NeonPolicy`ን በልዩ ልዩነት ያስፋፉ።

በSM-5a/SM-4c.1 ስር ክትትል የሚደረግባቸው የእርምጃ እቃዎች CI ለእያንዳንዱ አዲስ አርክቴክቸር የተመጣጠነ ማስረጃዎችን መያዙን ያረጋግጣሉ፣ እና ፍኖተ ካርታው በ 🈺 ላይ ይቆያል Neoverse/x86 baselines እና NEON-vs-scalar tolerances እስኪቀላቀሉ ድረስ።

## ተገዢነት እና የቁጥጥር ማስታወሻዎች

### ደረጃዎች እና መደበኛ ማጣቀሻዎች
- ** ጂኤም/ቲ 0002-2012** (SM4)፣ ** ጂኤም/ቲ 0003-2012** + **ጂቢ/ቲ 32918 ተከታታይ** (SM2)፣ **GM/T 0004-2012** የእኛ መጫዎቻዎች የሚጠቀሙባቸው የKDF ማሰሪያዎች።【docs/source/crypto/sm_vectors.md#L79】
- በ`docs/source/crypto/sm_compliance_brief.md` ውስጥ ያለው የማክበር አጭር ማቋረጫ እነዚህን መመዘኛዎች ከምህንድስና፣ SRE እና ህጋዊ ቡድኖች ከማስመዝገብ/ወደ ውጭ መላክ ኃላፊነቶች ጋር አብሮ ያገናኛል፤ የጂኤም/ቲ ካታሎግ በሚከለስበት ጊዜ ያ አጭር ማዘመን።

### ዋናው ቻይና ተቆጣጣሪ የስራ ፍሰት
1. **የምርት ማቅረቢያ (开发备案):** SM-የነቃ ሁለትዮሽዎችን ከዋናው ቻይና ከማጓጓዝዎ በፊት፣ የቅርስ ዝርዝር መግለጫውን፣ ቆራጥ የግንባታ ደረጃዎችን እና የጥገኝነት ዝርዝሩን ለክልላዊ ክሪፕቶግራፊ አስተዳደር ያስገቡ። የፋይል አብነቶች እና ተገዢነት ማረጋገጫ ዝርዝሩ በ`docs/source/crypto/sm_compliance_brief.md` እና በአባሪዎች ማውጫ (`sm_product_filing_template.md`፣ `sm_sales_usage_filing_template.md`፣ `sm_export_statement_template.md`) ውስጥ ይኖራሉ።
2. **የሽያጭ/የአጠቃቀም ፋይል (销售/使用备案)፡** በኤስኤም የነቁ ኖዶች በባህር ዳርቻ ላይ የሚያሄዱ ኦፕሬተሮች የማሰማራት ወሰን፣ ቁልፍ የአስተዳደር አቀማመጥ እና የቴሌሜትሪ እቅዳቸውን ማስመዝገብ አለባቸው። በሚያስገቡበት ጊዜ የተፈረሙ መግለጫዎችን እና `iroha_sm_*` ሜትሪክ ቅጽበተ-ፎቶዎችን ያያይዙ።
3. **የተረጋገጠ ሙከራ፡** ወሳኝ የመሠረተ ልማት ኦፕሬተሮች የተመሰከረላቸው የላብራቶሪ ሪፖርቶች ሊፈልጉ ይችላሉ። ሊባዙ የሚችሉ የግንባታ ስክሪፕቶችን፣ SBOM ወደ ውጭ የሚላኩ እና የWycheproof/interop artefacts (ከዚህ በታች ይመልከቱ) የታችኛው ተፋሰስ ኦዲተሮች ኮዱን ሳይቀይሩ ቬክተሮችን ማባዛት ይችላሉ።
4. ** የሁኔታ ክትትል:** የተጠናቀቁ መዝገቦችን በመልቀቂያ ትኬት እና `status.md` ውስጥ ይመዝግቡ; የጎደሉ መዝገቦች ከማረጋገጫ-ብቻ ወደ አብራሪዎች መፈረም ማስተዋወቅን አግድ።### ወደ ውጭ መላክ እና ስርጭት አቀማመጥ
- የኤስኤም አቅም ያላቸውን ሁለትዮሾች በ ** US EAR ምድብ 5 ክፍል 2** እና ** የአውሮፓ ህብረት ደንብ 2021/821 አባሪ 1 (5D002)** ስር ቁጥጥር ስር ያሉ ንጥሎች አድርገው ይያዙ። ምንጭ መታተም ለኦፕን-ምንጭ/ENC ቀረጻዎች ብቁ ሆኖ ቀጥሏል፣ነገር ግን የታገዱ መዳረሻዎች እንደገና ማከፋፈል አሁንም የሕግ ግምገማን ይፈልጋል።
- የመልቀቂያ መግለጫዎች የENC/TSU መሠረትን የሚያመለክት ወደ ውጭ መላኪያ መግለጫ ማያያዝ እና የFFI ቅድመ እይታ የታሸገ ከሆነ የOpenSSL/Tongsuo ግንባታ መለያዎችን መዘርዘር አለበት።
- የድንበር ተሻጋሪ ችግሮችን ለማስወገድ ኦፕሬተሮች የባህር ላይ ስርጭት ሲፈልጉ የክልል-አካባቢያዊ ማሸጊያዎችን (ለምሳሌ የሜይንላንድ መስተዋቶች) ይምረጡ።

### ኦፕሬተር ሰነድ እና ማስረጃ
- ይህንን የስነ-ህንፃ አጭር መግለጫ በ`docs/source/crypto/sm_operator_rollout.md` ውስጥ ካለው የታቀዱ ዝርዝር ማመሳከሪያ እና በ`docs/source/crypto/sm_compliance_brief.md` ውስጥ ካለው የማክበር ማዘዣ መመሪያ ጋር ያጣምሩ።
- በ `docs/genesis.md`፣ `docs/genesis.he.md`፣ እና `docs/genesis.ja.md` ላይ የዘፍጥረት/ኦፕሬተር ፈጣን አጀማመርን አቆይ፤ የ SM2/SM3 CLI የስራ ፍሰት ከዋኝ ጋር ፊት ለፊት ያለው የእውነት ምንጭ `crypto` ይገለጣል።
- OpenSSL/Tongsuo provenance፣ `scripts/sm_openssl_smoke.sh` ውፅዓት፣ እና `scripts/sm_interop_matrix.sh` እኩልነት ምዝግብ ማስታወሻዎች ከእያንዳንዱ የመልቀቂያ ጥቅል ጋር ስለዚህ ተገዢነት እና የኦዲት አጋሮች የሚወስኑ ቅርሶች አሏቸው።
- የታዛዥነት ወሰን ሲቀየር (አዲስ ስልጣኖች፣ የፍፃሜ ማጠናቀቂያዎች ወይም ወደ ውጭ የመላክ ውሳኔዎች) የፕሮግራሙ ሁኔታ እንዲታይ ለማድረግ `status.md` ያዘምኑ።
- በ `docs/source/release_dual_track_runbook.md` ውስጥ የተያዙትን የዝግጁነት ግምገማዎችን (`SM-RR1`–`SM-RR3`) ይከተሉ; በማረጋገጫ-ብቻ፣ በፓይለት እና በጂኤ ፊርማ ደረጃዎች መካከል ማስተዋወቅ እዚያ የተዘረዘሩትን ቅርሶች ይፈልጋል።

## Interop አዘገጃጀት

### RustCrypto ↔ OpenSSL/Tongsuo ማትሪክስ
1. OpenSSL/Tongsuo CLIs መኖራቸውን ያረጋግጡ (`IROHA_SM_CLI="openssl /opt/tongsuo/bin/openssl"` ግልጽ የመሳሪያ ምርጫ ይፈቅዳል)።
2. `scripts/sm_interop_matrix.sh` አሂድ; `cargo test -p iroha_crypto --test sm_cli_matrix --features sm`ን ይጠራዋል እና የSM2 ምልክት/ማረጋገጥ፣ SM3 ዲጀስትስ፣ እና SM4 ECB/GCM በእያንዳንዱ አቅራቢ ላይ ይፈስሳል፣የሌለውን CLI በመዝለል።【scripts/sm_interop_matrix.sh#L1】
3. የተገኙትን `target/debug/deps/sm_cli_matrix*.log` ፋይሎችን በተለቀቁት ቅርሶች ያስቀምጡ።

### የኤስኤስኤል ቅድመ እይታ ጭስ (የማሸጊያ በር) ክፈት
1. የOpenSSL ≥3.0 ልማት ራስጌዎችን ይጫኑ እና `pkg-config` ሊያገኛቸው እንደሚችል ያረጋግጡ።
2. `scripts/sm_openssl_smoke.sh` አስፈጽም; ረዳቱ `cargo check`/`cargo test --test sm_openssl_smoke`፣ SM3 hashing፣ SM2 ማረጋገጫ፣ እና SM4-GCM የዙር ጉዞዎችን በኤፍኤፍአይ ጀርባ በኩል ያካሂዳል (የሙከራ ማሰሪያው ቅድመ እይታውን በግልፅ ያሳያል)【scripts/sm_openssl_smoke.【scripts/sm_openssl_smoke.】
3. ማንኛውንም ያለመዝለል አለመሳካትን እንደ መልቀቂያ ማገጃ ይያዙ; ለኦዲት ማስረጃ የኮንሶል ውጤቱን ይያዙ።

### ቆራጥ ቋሚ እድሳት
- የኤስኤምኤስ መጫዎቻዎችን (`sm_vectors.md`፣ `fixtures/sm/…`) ከእያንዳንዱ የማክበር ማቅረቢያ በፊት ያድሱ፣ ከዚያም የፓርቲ ማትሪክስ እና የጭስ ማሰሪያውን እንደገና ያስኪዱ ስለዚህ ኦዲተሮች ከማቅረቡ ጎን ለጎን አዲስ የሚወስኑ ግልባጮች ይቀበላሉ።## የውጪ ኦዲት ዝግጅት
- `docs/source/crypto/sm_audit_brief.md` አውድ፣ ወሰን፣ መርሐግብር እና ዕውቂያዎችን ለውጫዊ ግምገማ ያዘጋጃል።
- የኦዲት ቅርሶች በ`docs/source/crypto/attachments/` (OpenSSL የጭስ መዝገብ፣ የካርጎ ዛፍ ቅጽበታዊ ገጽ እይታ፣ የካርጎ ዲበዳታ ወደ ውጭ መላክ፣የመሳሪያ ስብስብ ፕሮቨንስ) እና `fuzz/sm_corpus_manifest.json` (የተወሰነው የኤስኤም ፉዝ ዘሮች ከነባር ሪግሬሽን ቬክተሮች የተገኘ) ስር ይኖራሉ። በ macOS ላይ የጭስ ማውጫው በአሁኑ ጊዜ የተዘለለ ሩጫን ይመዘግባል ምክንያቱም የስራ ቦታ ጥገኛ ዑደት `cargo check` ይከላከላል; ሊኑክስ ያለ ዑደቱ ይገነባል የቅድመ እይታውን ጀርባ ሙሉ በሙሉ ይጠቀማል።
- በ2026-01-30 ከRFQ መላክ በፊት ለማሰለፍ ወደ Crypto WG፣ Platform Ops፣ Security እና Docs/DevRel ይመራል::

### የኦዲት ተሳትፎ ሁኔታ

- **የቢትስ ዱካ (የሲኤን ክሪፕቶግራፊ ልምምድ)** - የተፈፀመው የስራ መግለጫ በ*2026-02-21**፣መጀመር **2026-02-24**፣ የመስክ ስራ መስኮት **2026-02-24–2026-03-22** የመጨረሻ ሪፖርት ማቅረቢያ **2026-04-15 በየሳምንቱ እሮብ 09:00UTC በየሳምንቱ የፍተሻ ነጥብ ከCrypto WG አመራር እና ከደህንነት ምህንድስና ግንኙነት ጋር። ለዕውቂያዎች፣ ሊቀርቡ የሚችሉ እና የማስረጃ ዓባሪዎችን [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) ይመልከቱ።
- **የኤንሲሲ ቡድን APAC (የድንገተኛ ጊዜ ማስገቢያ) *** - ተጨማሪ ግኝቶች ወይም የቁጥጥር ጥያቄዎች ሁለተኛ አስተያየት የሚፈልጉ ከሆነ የግንቦት 2026 መስኮት እንደ ተከታይ/ትይዩ ግምገማ የተጠበቀ ነው። የተሳትፎ ዝርዝሮች እና የማሳደጊያ መንጠቆዎች በ`sm_audit_brief.md` ውስጥ ካለው የቢትስ መሄጃ መንገድ ጋር ይመዘገባሉ።

## አደጋዎች እና ቅነሳዎች

ሙሉ ምዝገባ፡ ለዝርዝር መረጃ [`sm_risk_register.md`](sm_risk_register.md) ይመልከቱ
የማስቆጠር እድል/ተፅእኖ፣ ቀስቅሴዎችን መከታተል እና የማቋረጥ ታሪክ። የ
ከዚህ በታች ማጠቃለያ ምህንድስናን ለመልቀቅ የወጡትን አርዕስተ ዜናዎች ይከታተላል።
| ስጋት | ከባድነት | ባለቤት | ቅነሳ |
|-------------|-------|-----------|
| ለ RustCrypto SM ሳጥኖች የውጭ ኦዲት እጥረት | ከፍተኛ | Crypto WG | የBits/NCC ቡድን የኮንትራት ዱካ፣የኦዲት ሪፖርት እስኪቀበል ድረስ ማረጋገጥን ብቻ ይያዙ። |
| በመላ ኤስዲኬዎች ላይ ቆራጥ ያልሆኑ ድግግሞሾች | ከፍተኛ | የኤስዲኬ ፕሮግራም ይመራል | በኤስዲኬ CI ላይ መገልገያዎችን ያጋሩ; ቀኖናዊ r∥s ኢንኮዲንግ ማስፈጸም; ክሮስ-ኤስዲኬ ውህደት ሙከራዎችን ያክሉ (በSM-3c ውስጥ ክትትል የሚደረግበት)። |
| ISA-ተኮር ሳንካዎች intrinsics | መካከለኛ | አፈጻጸም WG | Feature-gate intrinsics፣ በARM ላይ የCI ሽፋን ያስፈልጋቸዋል፣ የሶፍትዌር ውድቀትን ይጠብቁ። የሃርድዌር ማረጋገጫ ማትሪክስ በ `sm_perf.md` ውስጥ ተጠብቆ ይቆያል። |
| ተገዢነት አሻሚነት ጉዲፈቻ መዘግየት | መካከለኛ | ሰነዶች እና የህግ ግንኙነት | የታዛዥነት አጭር እና ኦፕሬተር ማረጋገጫ ዝርዝር (SM-6a/SM-6b) ከ GA በፊት ያትሙ። የሕግ ግብአት መሰብሰብ. የማመልከቻ ዝርዝር በ`sm_compliance_brief.md` ተልኳል። |
| የኤፍኤፍአይ የኋላ ተንሸራታች ከአቅራቢዎች ዝመናዎች ጋር | መካከለኛ | መድረክ ኦፕስ | የአቅራቢ ስሪቶችን ይሰኩ፣ የተመጣጣኝ ሙከራዎችን ያክሉ፣ ማሸጊያው እስኪረጋጋ (SM-P3) ድረስ የFFI backend መርጦ መግባቱን ያቆዩ። |## ክፍት ጥያቄዎች / ክትትል
1. በሩስት ውስጥ ከኤስኤምኤል ስልተ ቀመሮች ጋር ልምድ ያላቸውን ገለልተኛ የኦዲት አጋሮችን ይምረጡ።
   - ** መልስ (2026-02-24):** የቢትስ ሲኤን ክሪፕቶግራፊ ልምምድ የመጀመሪያ ደረጃ ኦዲት ተፈራርሟል (እ.ኤ.አ. 2026-02-24 ፣ መላኪያ 2026-04-15) እና የኤንሲሲ ቡድን APAC የግንቦት ድንገተኛ አደጋን ስለሚይዝ ተቆጣጣሪዎች አቃቤ ህግን እንደገና ሳይከፍቱ ሁለተኛ ግምገማ እንዲደረግላቸው ይጠይቃሉ። የተሳትፎ ወሰን፣ አድራሻዎች እና የማረጋገጫ ዝርዝሮች በ[`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) ውስጥ ይኖራሉ እና በ`sm_audit_vendor_landscape.md` ውስጥ ተንጸባርቀዋል።
2. ለኦፊሴላዊ Wycheproof SM2 ዳታ ስብስብ ወደላይ መከታተልን ቀጥል፤ የመሥሪያ ቦታው በአሁኑ ጊዜ የተስተካከለ ባለ 52 ኬዝ ስብስብ (የተወሰነ ቋሚ ዕቃዎች + የተቀናጁ ታምፐር ጉዳዮች) በመላክ ወደ `sm2_wycheproof.rs`/`sm2_fuzz.rs` ይመግባል። ኮርፐሱን በ`cargo xtask sm-wycheproof-sync` አንዴ ወደ ላይኛው JSON መሬት ያዘምኑ።
   - Bouncy ካስል እና GmSSL አሉታዊ የቬክተር ስብስቦችን ይከታተሉ; ወደ `sm2_fuzz.rs` ማስመጣት ፍቃድ አንዴ ከፀደቀ ነባሩን ኮርፐስ ለመጨመር።
3. ለኤስኤም ጉዲፈቻ ክትትል የመነሻ ቴሌሜትሪ (ሜትሪክስ፣ ሎግንግ) ይግለጹ።
4. ለKotodama/VM ተጋላጭነት የSM4 AEAD ነባሪ GCM ወይም CCM መሆኑን ይወስኑ።
5. RustCrypto/OpenSSL ን ለአባሪ ምሳሌ 1 (መታወቂያ `ALICE123@YAHOO.COM`) ይከታተሉ፡ ለታተመው የህዝብ ቁልፍ እና የ`(r, s)` የላይብረሪውን ድጋፍ ያረጋግጡ ስለዚህ መጋጠሚያዎች ወደ መልሶ ማገገሚያ ሙከራዎች እንዲያድጉ።

## የተግባር እቃዎች
- [x] የጥገኝነት ኦዲትን ያጠናቅቁ እና በደህንነት መከታተያ ውስጥ ይያዙ።
- [x] ለRustCrypto SM ሳጥኖች (SM-P0 ክትትል) የኦዲት አጋር ተሳትፎን ያረጋግጡ። ዱካ ኦፍ ቢትስ (ሲኤን ክሪፕቶግራፊ ልምምድ) በ`sm_audit_brief.md` ውስጥ ከተመዘገቡት የመነሻ/የመላኪያ ቀናት ጋር የቀዳሚ ግምገማ ባለቤት ሲሆን NCC ቡድን APAC ተቆጣጣሪን ወይም የአስተዳደር ክትትልን ለማርካት የሜይ 2026 የአደጋ ጊዜ ክፍተትን ይዞ ቆይቷል።
- [x] Wycheproof ሽፋንን ለSM4 CCM ታምፐር ጉዳዮች (SM-4a) ያራዝሙ።
- [x] የመሬት ቀኖናዊ SM2 ፊርማ ዕቃዎች በኤስዲኬዎች እና በሽቦ ወደ CI (SM-3c/SM-1b.1); በ `scripts/check_sm2_sdk_fixtures.py` የተጠበቀ (`ci/check_sm2_sdk_fixtures.sh` ይመልከቱ)።

## ተገዢነት አባሪ (የስቴት የንግድ ክሪፕቶግራፊ)

- ** ምደባ: ** SM2 / SM3 / SM4 መርከብ በቻይና * ግዛት የንግድ ምስጠራ * አገዛዝ (PRC ክሪፕቶግራፊ ህግ, Art.3). እነዚህን ስልተ ቀመሮች በIroha ሶፍትዌር ማጓጓዝ ** ፕሮጀክቱን በዋና/የጋራ (የግዛት-ሚስጥራዊ) ደረጃዎች ውስጥ አያስቀምጠውም ነገር ግን በPRC ማሰማራቶች ውስጥ የሚጠቀሙ ኦፕሬተሮች የንግድ-ክሪፕቶ ፋይል እና የMLPS ግዴታዎችን መከተል አለባቸው።【docs/source/crypto/sm_chinese_crypto_law_brief.
- **የመመዘኛዎች የዘር ሐረግ፡** የሕዝብ ሰነዶችን ከጂኤም/ቲ መግለጫዎች ኦፊሴላዊ የጂቢ/ቲ ልወጣዎች ጋር አሰልፍ፡

| አልጎሪዝም | GB/T ማጣቀሻ | GM/T መነሻ | ማስታወሻ |
|-------------|------------|-------|
| SM2 | GB / T32918 (ሁሉም ክፍሎች) | GM/T0003 | ECC ዲጂታል ፊርማ + የቁልፍ ልውውጥ; Iroha በዋና ኖዶች ውስጥ ማረጋገጫን እና ለኤስዲኬዎች መወሰኛ ፊርማ ያጋልጣል። |
| SM3 | GB/T32905 | GM/T0004 | 256-ቢት ሃሽ; በ scalar እና ARMv8 የተጣደፉ ዱካዎች ላይ የሚወሰን ሃሽንግ። |
| SM4 | GB/T32907 | GM/T0002 | 128-ቢት የማገጃ ሲፈር; Iroha GCM/CCM ረዳቶችን ያቀርባል እና በትግበራዎች ላይ ትልቅ-ኢንዲያን እኩልነትን ያረጋግጣል። |- **የችሎታ መግለጫ፡** Torii `/v1/node/capabilities` የመጨረሻ ነጥብ የሚከተለውን የJSON ቅርፅ ያስተዋውቃል ኦፕሬተሮች እና መሳሪያዎች የኤስኤም ኤን ኤን ኤን ፕረማሲያዊ በሆነ መንገድ ሊጠቀሙበት ይችላሉ።

```json
{
  "supported_abi_versions": [1],
  "default_compile_target": 1,
  "data_model_version": 1,
  "crypto": {
    "sm": {
      "enabled": true,
      "default_hash": "sm3-256",
      "allowed_signing": ["ed25519"],
      "sm2_distid_default": "1234567812345678",
      "openssl_preview": false,
      "acceleration": {
        "scalar": true,
        "neon_sm3": false,
        "neon_sm4": false,
        "policy": "auto"
      }
    }
  }
}
```

የCLI ንኡስ ትዕዛዝ `iroha runtime capabilities` በአገር ውስጥ ተመሳሳይ ክፍያን ያዘጋጃል፣የአንድ መስመር ማጠቃለያን ከJSON የማክበር ማስረጃ መሰብሰብ ጋር በማተም።

- **የሰነድ ማቅረቢያዎች፡** ከላይ ያሉትን ስልተ ቀመሮች/መስፈርቶች የሚለዩ የመልቀቂያ ማስታወሻዎችን እና SBOMዎችን ያትሙ እና ሙሉ ተገዢነትን አጭር (`sm_chinese_crypto_law_brief.md`) ከተለቀቁት ቅርሶች ጋር በማያያዝ ኦፕሬተሮች ከክልላዊ ሰነዶች ጋር እንዲያያይዙት ያድርጉ።
- **የኦፕሬተር እጅ መውጣት፡** MLPS2.0/GB/T39786-2021 የ crypto መተግበሪያ ግምገማዎችን፣ የኤስኤም ቁልፍ አስተዳደር SOPs እና ≥6ዓመት ማስረጃ ማቆየት እንደሚያስፈልጋቸው አሰማሪዎችን አስታውስ። ወደ ኦፕሬተር ማመሳከሪያ ዝርዝሩ በተሟላ መግለጫው ላይ ያመልክቱ።【docs/source/crypto/sm_chinese_crypto_law_brief.md:43】【docs/source/crypto/sm_chinese_crypto_law_brief.md:74】

## የግንኙነት እቅድ
- ** ታዳሚዎች: *** የ Crypto WG ዋና አባላት ፣ የመልቀቂያ ምህንድስና ፣ የደህንነት ግምገማ ቦርድ ፣ የኤስዲኬ ፕሮግራም ይመራል።
- ** ቅርሶች: ** `sm_program.md`, `sm_lock_refresh_plan.md`, `sm_vectors.md`, `sm_wg_sync_template.md`, የመንገድ ካርታ ቅንጭብ (SM-0 .. SM-7a).
- **ሰርጥ፡** ሳምንታዊ የCrypto WG ማመሳሰል አጀንዳ + የተግባር ንጥሎችን ማጠቃለል እና ለመቆለፊያ ማደስ እና የጥገኝነት ቅበላ (ረቂቅ 2025-01-19 ተሰራጭቷል) የሚከታተል ኢሜይል።
- ** ባለቤት: ** Crypto WG እርሳስ (ተወካዩ ተቀባይነት ያለው)።