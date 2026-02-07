---
lang: am
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9df713c3e078ac2ccbd74eb215b91bb80d08306d0ca455dc122fde535601ce8
source_last_modified: "2026-01-18T10:42:52.828202+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito ዥረት

Norito ዥረት የሽቦ ቅርጸቱን፣ የቁጥጥር ፍሬሞችን እና የማጣቀሻ ኮዴክን ይገልጻል።
ለቀጥታ ሚዲያ ፍሰቶች በI18NT0000002X እና SoraNet ላይ ጥቅም ላይ ይውላል። ቀኖናዊው ዝርዝር ውስጥ ይኖራል
`norito_streaming.md` በስራ ቦታ ስር; ይህ ገጽ ቁርጥራጮቹን ያስወግዳል
ኦፕሬተሮች እና የኤስዲኬ ደራሲዎች ከውቅረት ንክኪ ነጥቦች ጎን ለጎን ያስፈልጋቸዋል።

## ሽቦ ቅርጸት እና መቆጣጠሪያ አውሮፕላን

- ** መግለጫዎች እና ክፈፎች።** `ManifestV1` እና I18NI0000005X ክፍሉን ይገልፃሉ።
  የጊዜ መስመር፣ ቁርጥራጭ ገላጭ እና የመንገድ ፍንጭ። የመቆጣጠሪያ ክፈፎች (`KeyUpdate`፣
  `ContentKeyUpdate`፣ እና የcadence ግብረመልስ) ከማኒፌሽኑ ጋር ይኖራሉ
  ተመልካቾች ከመግባታቸው በፊት ቃል ኪዳኖችን ማረጋገጥ ይችላሉ።
- **ቤዝላይን ኮድ
  ቸንክ መታወቂያዎች፣ የጊዜ ማህተም አርቲሜቲክ እና የቁርጠኝነት ማረጋገጫ። አስተናጋጆች መደወል አለባቸው
  `EncodedSegment::verify_manifest` ተመልካቾችን ወይም ቅብብሎሾችን ከማገልገልዎ በፊት።
- ** የባህሪ ቢት።** የአቅም ድርድር `streaming.feature_bits` ያስተዋውቃል
  (ነባሪ I18NI0000012X = የመነሻ ግብረመልስ + የግላዊነት መስመር አቅራቢ) ስለዚህ ቅብብሎሽ እና
  ደንበኞቻቸው ችሎታቸውን ሳይዛመዱ እኩዮችን ውድቅ ማድረግ ይችላሉ።

## ቁልፎች፣ ስብስቦች እና ቃላቶች

- ** የማንነት መስፈርቶች።** የዥረት መቆጣጠሪያ ፍሬሞች ሁል ጊዜ የተፈረሙ ናቸው።
  ኢድ25519. የወሰኑ ቁልፎች በ በኩል ሊቀርቡ ይችላሉ
  `streaming.identity_public_key`/`streaming.identity_private_key`; አለበለዚያ
  የመስቀለኛ መለያው እንደገና ጥቅም ላይ ይውላል.
- ** HPKE suites.** `KeyUpdate` ዝቅተኛውን የጋራ ስብስብ ይመርጣል; ስብስብ #1 ነው።
  አስገዳጅ (`AuthPsk`፣ `Kyber768`፣ `HKDF-SHA3-256`፣ I18NI0000019X)፣ ከ ጋር
  አማራጭ `Kyber1024` ማሻሻያ መንገድ. የ Suite ምርጫ በ ላይ ተከማችቷል።
  ክፍለ ጊዜ እና በእያንዳንዱ ዝመና ላይ የተረጋገጠ።
- **ማሽከርከር።** አታሚዎች የተፈረመ I18NI0000021X በየ64ሚቢ ወይም 5ደቂቃ ይለቃሉ።
  `key_counter` በጥብቅ መጨመር አለበት; እንደገና መመለስ ከባድ ስህተት ነው።
  `ContentKeyUpdate` የተጠቀለለ የቡድን ይዘት ቁልፍ ያሰራጫል።
  የተደራደረው የHPKE ስብስብ፣ እና የጌት ክፍል ዲክሪፕት በ ID + ትክክለኛነት
  መስኮት.
- ** ቅጽበታዊ ገጽ እይታዎች።** `StreamingSession::snapshot_state` እና
  `restore_from_snapshot` `{የክፍለ-ጊዜ_መታወቂያ፣ቁልፍ_ቆጣሪ፣ስብስብ፣ sts_root፣
  cadence ሁኔታ}I18NI0000026Xstreaming.session_store_dir` (ነባሪ
  `./storage/streaming`)። የማጓጓዣ ቁልፎች ወደነበረበት ሲመለሱ ይከሰታሉ
  የክፍለ ጊዜ ሚስጥሮችን አታፍስሱ.

## Runtime ውቅር

- **ቁልፍ ቁሳቁስ።** የተሰጡ ቁልፎችን ያቅርቡ
  `streaming.identity_public_key`/`streaming.identity_private_key` (Ed25519
  multihash) እና አማራጭ Kyber ቁሳዊ በኩል
  `streaming.kyber_public_key`/`streaming.kyber_secret_key`. አራቱም መሆን አለባቸው
  ነባሪዎችን በሚሽርበት ጊዜ መገኘት; `streaming.kyber_suite` ይቀበላል
  `mlkem512|mlkem768|mlkem1024` (ተለዋጭ ስም `kyber512/768/1024`፣ ነባሪ
  `mlkem768`).
- **የኮዴክ መከላከያ መንገዶች** ግንባታው እስካልቻለው ድረስ CABAC እንደተሰናከለ ይቆያል።
  የተጠቀለለ RANS `ENABLE_RANS_BUNDLES=1` ያስፈልገዋል። በ በኩል ማስፈጸም
  `streaming.codec.{entropy_mode,bundle_width,bundle_accel}` እና አማራጭ
  ብጁ ጠረጴዛዎችን ሲያቀርብ `streaming.codec.rans_tables_path`። የተጠቀለለ
- **የሶራኔት መንገዶች።** `streaming.soranet.*` የማይታወቅ መጓጓዣን ይቆጣጠራል።
  `exit_multiaddr` (ነባሪ `/dns/torii/udp/9443/quic`)፣ `padding_budget_ms`
  (ነባሪ 25ms)፣ `access_kind` (`authenticated` vs `read-only`)፣ አማራጭ
  `channel_salt`፣ `provision_spool_dir` (ነባሪ
  `./storage/streaming/soranet_routes`)፣ `provision_spool_max_bytes` (ነባሪ 0፣
  ያልተገደበ)፣ I18NI0000050X (ነባሪ 4) እና
  `provision_queue_capacity` (ነባሪ 256)።
- ** አመሳስል በር።** `streaming.sync` ተንሸራታች ማስፈጸሚያ ለኦዲዮቪዥዋል ይቀይራል።
  ዥረቶች፡ I18NI0000053X፣ `observe_only`፣ `ewma_threshold_ms`፣ እና `hard_cap_ms`
  ክፍሎች በጊዜ መንሸራተት ውድቅ ሲደረግ ያስተዳድሩ።

## ማረጋገጫ እና መጫዎቻዎች

- ቀኖናዊ ዓይነት ትርጓሜዎች እና ረዳቶች ይኖራሉ
  `crates/iroha_crypto/src/streaming.rs`.
- የውህደት ሽፋን የ HPKE እጅ መጨባበጥን፣ የይዘት ቁልፍ ስርጭትን፣
  እና ቅጽበታዊ የህይወት ዑደት (`crates/iroha_crypto/tests/streaming_handshake.rs`)።
  ዥረቱን ለማረጋገጥ `cargo test -p iroha_crypto streaming_handshake`ን ያሂዱ
  ላዩን በአካባቢው.
- ወደ አቀማመጥ፣ የስህተት አያያዝ እና የወደፊት ማሻሻያዎችን በጥልቀት ለመጥለቅ ያንብቡ
  `norito_streaming.md` በማከማቻ ስር.