---
lang: am
direction: ltr
source: docs/dependency_audit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb0a770fac1086462d949dbf17dd5a05f133169e57d50b0d90ddb48ae05f2853
source_last_modified: "2026-01-05T09:28:11.822642+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! የጥገኝነት ኦዲት ማጠቃለያ

ቀን፡- 2025-09-01

ወሰን፡ በCargo.toml ፋይሎች ውስጥ የታወጁ እና በCargo.lock ውስጥ የተፈቱ የሁሉም ሳጥኖች የስራ ቦታ-ሰፊ ግምገማ። በRustSec አማካሪ ዲቢ እና በእጅ ግምገማ ለ crate ህጋዊነት እና ለአልጎሪዝም ምርጫዎች "ዋና ክሬት" ምርጫዎች ላይ በካርጎ ኦዲት ተካሂዷል።

መሳሪያዎች/ትእዛዞች ይሰራሉ፡-
- `cargo tree -d --workspace --locked --offline` - የተባዙ ስሪቶች ተፈትሸዋል።
- `cargo audit` – የተቃኘ ካርጎ.ሎክ ለታወቁ ተጋላጭነቶች እና የተቀነጠቁ ሳጥኖች

የደህንነት ምክሮች ተገኝተዋል (አሁን 0 ብልግናዎች፣ 2 ማስጠንቀቂያዎች)፡-
- crossbeam-channel - RUSTSEC-2025-0024
  - ቋሚ: በ `crates/ivm/Cargo.toml` ውስጥ ወደ `0.5.15` ጎድቷል.

  - ቋሚ: `pprof` ወደ `prost-codec` በ `crates/iroha_torii/Cargo.toml` ተገልብጧል።

- ቀለበት - RUSTSEC-2025-0009
  - ቋሚ፡ የተጨማደደ የQUIC/TLS ቁልል (`quinn 0.11`፣ `rustls 0.23`፣ `tokio-rustls 0.26`) እና የ WS ቁልል ወደ `tungstenite/tokio-tungstenite 0.24` የዘመነ። በ`cargo update -p ring --precise 0.17.12` በኩል ወደ `ring 0.17.12` የግዳጅ መቆለፊያ።

የቀሩት ምክሮች: ምንም. ቀሪ ማስጠንቀቂያዎች፡ I18NI0000014X (ያልተጠበቀ)፣ `derivative` (ያልተጠበቀ)።

ህጋዊነት እና "ዋና ክሬት" ግምገማ (ስፖትላይት)፡-
- Hashing: `sha2` (RustCrypto), `blake2` (RustCrypto), `tiny-keccak` (ሰፊ ጥቅም ላይ የዋለ) - ቀኖናዊ ምርጫዎች.
- AEAD/Symmetric: I18NI0000019X, `chacha20poly1305`, `aead` ባህርያት (RustCrypto) - ቀኖናዊ.
- ፊርማዎች / ECC: `ed25519-dalek`, `x25519-dalek` (ዳሌክ ፕሮጀክት), `k256` (RustCrypto), `secp256k1` (libsecp bindings) - ሁሉም ህጋዊ; የወለል ስፋትን ለመቀነስ ነጠላ ሴፕ256k1 ቁልል (`k256` ለንፁህ Rust ወይም I18NI0000027X ለ libsecp) እመርጣለሁ።
- BLS12-381/ZK: `blstrs`, `halo2_*` - በምርት ZK ስነ-ምህዳሮች ውስጥ በስፋት ጥቅም ላይ ይውላል; ህጋዊ.
- PQ: `pqcrypto-dilithium`, `pqcrypto-traits` - ሕጋዊ የማጣቀሻ ሳጥኖች.
- TLS: `rustls`, `tokio-rustls`, I18NI0000034X - ቀኖናዊ ዘመናዊ የዝገት TLS ቁልል.
- ጫጫታ: `snow` - ቀኖናዊ ትግበራ.
- ተከታታይነት፡ I18NI0000036X ለ SCALE ቀኖናዊ ነው። ሰርዴ በስራ ቦታው ላይ ከምርት ጥገኛ ተወግዷል; Norito እያንዳንዱን የሩጫ መንገድ ይሸፍናል/ጸሐፊዎች። ማንኛውም ቀሪ የሰርዴ ማጣቀሻዎች በታሪካዊ ሰነዶች፣ በጠባቂ ስክሪፕቶች ወይም በሙከራ-ብቻ የፈቃድ ዝርዝሮች ይኖራሉ።
- FFI/libs: `libsodium-sys-stable`, `openssl` - ህጋዊ; በምርት ዱካዎች ውስጥ ከOpenSSL ይልቅ Rustlsን ይመርጣሉ (የአሁኑ ኮድ ቀድሞውኑ ይሠራል)።

ምክሮች፡-
- የአድራሻ ማስጠንቀቂያዎች;
  - I18NI0000039Xን በI18NI0000040X/`futures-retry` ወይም በአካባቢያዊ ገላጭ የኋላ ረዳት ለመተካት ያስቡበት።
  - `derivative` ይተካው በእጅ impls ወይም `derive_more` ሲተገበር።
- መካከለኛ፡ የተባዙ አተገባበርን ለመቀነስ በ`k256` ወይም `secp256k1` ላይ አንድ ያድርጉ (ሁለቱንም በትክክል ከተፈለገ ብቻ ይተው)።
- መካከለኛ: ግምገማ I18NI0000046X provenance ለ ZK አጠቃቀም; የሚቻል ከሆነ፣ ትይዩ ስነ-ምህዳሮችን ለመቀነስ ከArkworks/Halo2-native Poseidon ትግበራ ጋር መጣጣምን ያስቡበት።

ማስታወሻዎች፡-
- `cargo tree -d` የሚጠበቁ የተባዙ ዋና ዋና ስሪቶችን ያሳያል (`bitflags` 1/2፣ multiple `ring`) በራሱ የደህንነት ስጋት ሳይሆን የግንባታ ቦታን ይጨምራል።
- ምንም ዓይነት ታይፖስኳት የሚመስሉ ሳጥኖች አልተስተዋሉም; ሁሉም ስሞች እና ምንጮች ለታወቁ የስነ-ምህዳር ሳጥኖች ወይም የውስጥ የስራ ቦታ አባላት ይፈታሉ.
- ለሙከራ፡- BLSን ወደ blstrs-ብቻ ጀርባ ማሸጋገር ለመጀመር የI18NI0000050X ባህሪ `bls-backend-blstrs` ተጨምሯል። የባህሪ/የመቀየሪያ ለውጦችን ለማስቀረት ነባሪው I18NI0000052X ይቀራል። የማጣጣም እቅድ፡
  - በ`crates/iroha_crypto/tests/bls_backend_compat.rs` ውስጥ ቁልፎቹን አንድ ጊዜ የሚያወጡ እና በሁለቱም የጀርባ ክፍሎች ላይ እኩልነትን የሚያረጋግጡ ፣ `SecretKey` ፣ `PublicKey` እና የፊርማ ድምርን የሚሸፍኑ የዙር ጉዞ መሳሪያዎችን ይጨምሩ።

ክትትል (የታቀዱ የስራ እቃዎች)
- የ Serde Guardrails በ CI (`scripts/check_no_direct_serde.sh`, `scripts/deny_serde_json.sh`) ያቆዩት ስለዚህ አዲስ የምርት አጠቃቀምን ማስተዋወቅ አይቻልም።

ለዚህ ኦዲት የተደረገ ሙከራ፡-
- ራን `cargo audit` የቅርብ አማካሪ DB ጋር; አራቱን ምክሮች እና ጥገኛ ዛፎቻቸውን አረጋግጧል.
- ቦታዎችን ለማስተካከል የተጎዱ ሳጥኖችን ቀጥተኛ ጥገኛ መግለጫዎችን ፈልገዋል ።