---
lang: am
direction: ltr
source: docs/source/crypto/sm_operator_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dffc2cf6c6e59f54d1fc22136ba93f75466509c699a4361a381bf7e0ce0d1dda
source_last_modified: "2025-12-29T18:16:35.943754+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SM ባህሪ ልቀት እና ቴሌሜትሪ ማረጋገጫ ዝርዝር

ይህ የማረጋገጫ ዝርዝር SRE እና ኦፕሬተር ቡድኖች የSM (SM2/SM3/SM4) ባህሪን እንዲያነቁ ያግዛል።
የኦዲት እና የታዛዥነት በሮች ከተጸዱ በኋላ ደህንነቱ በተጠበቀ ሁኔታ ያስቀምጡ። ይህን ሰነድ ተከተል
በ `docs/source/crypto/sm_program.md` እና በ ውስጥ ካለው የውቅረት አጭር መግለጫ ጋር
በ `docs/source/crypto/sm_compliance_brief.md` ውስጥ ህጋዊ/የመላክ መመሪያ።

## 1. የቅድመ-በረራ ዝግጁነት
- [ ] `sm` እንደ ማረጋገጫ-ብቻ ወይም መፈረም የሚያሳዩትን የስራ ቦታ መልቀቂያ ማስታወሻዎች ያረጋግጡ፣
      በታቀደው ደረጃ ላይ በመመስረት.
- [ ] መርከቦቹ የሚከተሉትን ከሚያካትት ቁርጠኝነት የተገነቡ ሁለትዮሾችን እያሄደ መሆኑን ያረጋግጡ
      SM ቴሌሜትሪ ቆጣሪዎች እና የማዋቀር ቁልፎች። (የታቀደ ልቀት TBD፤ ትራክ
      በታቀደው ትኬት ውስጥ።)
- [ ] `scripts/sm_perf.sh --tolerance 0.25`ን በማቆሚያ መስቀለኛ መንገድ (በዒላማው) ያሂዱ
      አርክቴክቸር) እና የማጠቃለያውን ውጤት በማህደር ያስቀምጡ። ስክሪፕቱ አሁን በራስ-ሰር ይመርጣል
      የ scalar baseline ለማፍጠን ሁነታዎች የንፅፅር ዒላማ
      (`--compare-tolerance` ነባሪዎች ወደ 5.25 SM3 NEON በሚሰራበት ጊዜ);
      ዋናው ወይም ንጽጽር ከሆነ የታቀደውን መርምር ወይም ማገድ
      ጠባቂው አልተሳካም. በሊኑክስ/arch64 Neoverse ሃርድዌር ላይ ሲያነሱ፣ ይለፉ
      `--baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_<mode>.json --write-baseline`
      ወደ ውጭ የተላኩትን `m3-pro-native` ሚዲያዎችን በአስተናጋጁ ቀረጻ ለመፃፍ
      ከማጓጓዙ በፊት.
- [ ] `status.md` እና የታቀዱ ትኬቶቹ የተሟሉ ሰነዶችን መመዝገቡን ያረጋግጡ።
      በሚያስፈልጋቸው ክልሎች ውስጥ የሚሰሩ ማንኛቸውም አንጓዎች (የማሟያ አጭር ይመልከቱ)።
- [ ] አረጋጋጮች የኤስኤምኤስ መፈረሚያ ቁልፎችን የሚያከማቹ ከሆነ የKMS/HSM ዝመናዎችን ያዘጋጁ
      የሃርድዌር ሞጁሎች.

## 2. የውቅረት ለውጦች
1. የSM2 ቁልፍ ክምችት እና ለመለጠፍ ዝግጁ የሆነ ቅንጣቢ ለመፍጠር የ xtask አጋዥን ያሂዱ፡-
   ```bash
   cargo xtask sm-operator-snippet \
     --distid CN12345678901234 \
     --json-out sm2-key.json \
     --snippet-out client-sm2.toml
   ```
   ውጤቶቹን ለመፈተሽ በሚፈልጉበት ጊዜ ወደ stdout በዥረት ለመልቀቅ `--snippet-out -` (እና እንደ አማራጭ `--json-out -`) ይጠቀሙ።
   ዝቅተኛ ደረጃ CLI ትዕዛዞችን በእጅ መንዳት ከመረጡ፣ ተመጣጣኝ ፍሰቱ፡-
   ```bash
   cargo run -p iroha_cli --features sm -- \
     crypto sm2 keygen \
     --distid CN12345678901234 \
     --output sm2-key.json

   cargo run -p iroha_cli --features sm -- \
     crypto sm2 export \
     --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
     --distid CN12345678901234 \
     --snippet-output client-sm2.toml \
     --emit-json --quiet
   ```
   `jq` የማይገኝ ከሆነ `sm2-key.json` ይክፈቱ፣ የ`private_key_hex` እሴት ይቅዱ እና በቀጥታ ወደ ውጭ መላኪያ ትዕዛዙ ያስተላልፉት።
2. የተገኘውን ቅንጣቢ በእያንዳንዱ መስቀለኛ መንገድ ውቅር ላይ ይጨምሩ (እሴቶቹ ለ
   አረጋግጥ-ብቻ ደረጃ; በየአካባቢው ያስተካክሉ እና ቁልፎቹ እንደሚታየው ይደረደራሉ፡
```toml
[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]   # remove "sm2" to stay in verify-only mode
sm2_distid_default = "1234567812345678"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```
3. መስቀለኛ መንገድን እንደገና ያስጀምሩት እና `crypto.sm_helpers_available` እና (የቅድመ-እይታ ጀርባን ካነቁ) `crypto.sm_openssl_preview_enabled` ላይ እንደተጠበቀው ያረጋግጡ፡-
   - `/status` JSON (`"crypto":{"sm_helpers_available":true,"sm_openssl_preview_enabled":true,...}`)።
   - ለእያንዳንዱ መስቀለኛ መንገድ የተሰራው `config.toml`።
4. ከሆነ የኤስኤም ስልተ ቀመሮችን ወደ የፈቃድ ዝርዝር ለመጨመር የገለጻዎች/የዘፍጥረት ግቤቶችን ያዘምኑ
   በታቀደው ልቀት ላይ መፈረም ነቅቷል። `--genesis-manifest-json` ሲጠቀሙ
   ያለ ቅድመ-የተፈረመ የዘፍጥረት ብሎክ፣ `irohad` አሁን የሩጫ ጊዜውን ይዘራል crypto
   ቅጽበታዊ ገጽ እይታ በቀጥታ ከማንፀባረቂያው `crypto` ብሎክ - አንጸባራቂው መሆኑን ያረጋግጡ
   ወደ ፊት ከመሄድዎ በፊት የለውጥ እቅድዎን ያረጋግጡ።## 3. ቴሌሜትሪ እና ክትትል
- የ Prometheus የመጨረሻ ነጥቦችን ያፅዱ እና የሚከተሉት ቆጣሪዎች / መለኪያዎች መኖራቸውን ያረጋግጡ።
  - `iroha_sm_syscall_total{kind="verify"}`
  - `iroha_sm_syscall_total{kind="hash"}`
  - `iroha_sm_syscall_total{kind="seal|open",mode="gcm|ccm"}`
  - `iroha_sm_openssl_preview` (0/1 መለኪያ የቅድመ እይታ መቀያየሪያ ሁኔታን ሪፖርት ያደርጋል)
  - `iroha_sm_syscall_failures_total{kind="verify|hash|seal|open",reason="..."}`
- የ SM2 መፈረም ከነቃ በኋላ መንጠቆ መፈረሚያ መንገድ; ቆጣሪዎችን ያክሉ ለ
  `iroha_sm_sign_total` እና `iroha_sm_sign_failures_total`።
- Grafana ዳሽቦርዶች/ ማንቂያዎችን ይፍጠሩ ለ፡-
  - ውድቀት ቆጣሪዎች ውስጥ ስፒሎች (መስኮት 5 ሜትር).
  - በኤስኤምኤስ syscall throughput ውስጥ ድንገተኛ ጠብታዎች።
  - በአንጓዎች መካከል ያሉ ልዩነቶች (ለምሳሌ፣ ያልተዛመደ ማንቃት)።

## 4. የመልቀቂያ ደረጃዎች
| ደረጃ | ድርጊቶች | ማስታወሻ |
|-------|---------|-------|
| አረጋግጥ-ብቻ | `crypto.default_hash` ወደ `sm3-256` አዘምን፣ `allowed_signing`ን ያለ `sm2` ተወው፣ የማረጋገጫ ቆጣሪዎችን ተቆጣጠር። | ግብ፡ የስምምነት ልዩነትን ሳያጋልጡ የኤስኤም ማረጋገጫ መንገዶችን ይለማመዱ። |
| ድብልቅ ፊርማ አብራሪ | የተገደበ ኤስኤም መፈረም ፍቀድ (የአረጋጋጮች ንዑስ ስብስብ)። የመፈረሚያ ቆጣሪዎችን እና መዘግየትን ይቆጣጠሩ። | የ Ed25519 ውድቀት መገኘቱን ያረጋግጡ። ቴሌሜትሪ አለመመጣጠን ካሳየ ያቁሙ። |
| GA መፈረም | `allowed_signing`ን ለማካተት `sm2`ን ያራዝሙ፣መግለጫዎች/ኤስዲኬዎችን ያዘምኑ እና የመጨረሻውን የሩጫ መጽሐፍ ለማተም። | የተዘጉ የኦዲት ግኝቶች፣ የዘመኑ የተሟሉ ሰነዶች እና የተረጋጋ ቴሌሜትሪ ያስፈልገዋል። |

### ዝግጁነት ግምገማዎች
- **አረጋግጥ-ብቻ ዝግጁነት (SM-RR1)** ልቀትን ኢንጅ፣ ክሪፕቶ ደብሊውጂ፣ ኦፕስ እና ህጋዊ። ያስፈልጋል፡
  - `status.md` ማስታወሻዎች ተገዢነት ፋይል ሁኔታ + OpenSSL provenance.
  - `docs/source/crypto/sm_program.md` / `sm_compliance_brief.md` / ይህ የማረጋገጫ ዝርዝር በመጨረሻው የተለቀቀው መስኮት ውስጥ ተዘምኗል።
  - `defaults/genesis` ወይም አካባቢን-ተኮር አንጸባራቂ ያሳያል `crypto.allowed_signing = ["ed25519","sm2"]` እና `crypto.default_hash = "sm3-256"` (ወይም የማረጋገጫ-ብቻ ልዩነት ያለ `sm2` አሁንም በደረጃ አንድ ከሆነ)።
  - `scripts/sm_openssl_smoke.sh` + `scripts/sm_interop_matrix.sh` ምዝግብ ማስታወሻዎች ከታቀደው ትኬት ጋር ተያይዘዋል።
  - ቴሌሜትሪ ዳሽቦርድ (`iroha_sm_*`) ለተረጋጋ-ግዛት ባህሪ ተገምግሟል።
- ** የአብራሪ ዝግጁነት (SM-RR2) መፈረም።** ተጨማሪ በሮች፡-
  - የ RustCrypto SM ቁልል ተዘግቷል ወይም RFC ለደህንነት የተፈረመ ማካካሻ ሪፖርት።
  - ከዋኝ runbooks (በፋሲሊቲ-ተኮር) የመመለስ/የመመለሻ እርምጃዎችን በመፈረም የዘመኑ።
  - የዘፍጥረት መግለጫዎች ለአብራሪ ስብስብ `allowed_signing = ["ed25519","sm2"]` ያካትታል እና የተፈቀደው ዝርዝር በእያንዳንዱ መስቀለኛ መንገድ ውቅር ውስጥ ይንጸባረቃል።
  - የመውጣት/የመመለሻ እቅድ ተመዝግቧል (`allowed_signing` ወደ Ed25519 ቀይር፣ መግለጫዎችን ወደነበረበት መመለስ፣ ዳሽቦርዶችን ዳግም አስጀምር)።
- ** GA ዝግጁነት (SM-RR3)።** አወንታዊ የአውሮፕላን አብራሪ ሪፖርት፣ ለሁሉም አረጋጋጭ ስልጣኖች የዘመኑ የተሟሉ ሰነዶች፣ የተፈረመ የቴሌሜትሪ መነሻ መስመሮች እና ትኬት መልቀቅ ከ Eng + Crypto WG + Ops/Legal triad ይፈልጋል።## 5. ማሸግ እና ተገዢነት ማረጋገጫ ዝርዝር
- ** Bundle OpenSSL/Tongsuo ቅርሶች።** OpenSSL/Tongsuo 3.0+ የተጋሩ ቤተ-መጻሕፍት (`libcrypto`/`libssl`) በእያንዳንዱ አረጋጋጭ ፓኬጅ ይላኩ ወይም ትክክለኛውን የስርዓት ጥገኝነት ይመዝግቡ። ኦዲተሮች የአቅራቢውን ግንባታ መከታተል እንዲችሉ ስሪቱን ይመዝግቡ፣ ባንዲራዎችን ይገንቡ እና SHA256 ቼኮችን በመልቀቂያ ሰነዱ ውስጥ ይመዝግቡ።
- **በCI ጊዜ ያረጋግጡ።** `scripts/sm_openssl_smoke.sh`ን በእያንዳንዱ የዒላማ መድረክ ላይ ከታሸጉ ቅርሶች ጋር የሚያስፈጽም የCI እርምጃ ያክሉ። የቅድመ እይታ ባንዲራ ከነቃ ስራው መሰናከል አለበት ነገር ግን አቅራቢው መጀመር አይቻልም (የጠፉ ራስጌዎች፣ የማይደገፍ ስልተ-ቀመር ወዘተ)።
- **የታዛዥነት ማስታወሻዎችን ያትሙ።** የመልቀቂያ ማስታወሻዎችን/`status.md` ከተጠቀለለ የአቅራቢ ሥሪት፣የኤክስፖርት መቆጣጠሪያ ማጣቀሻዎች (ጂኤም/ቲ፣ጂቢ/ቲ) እና ለኤስኤም ስልተ ቀመሮች የሚፈለጉትን ማንኛውንም ሥልጣን-ተኮር ሰነዶችን ያዘምኑ።
- **የኦፕሬተር runbook ማሻሻያዎችን።** የማሻሻያ ፍሰቱን ይመዝግቡ፡ አዲሶቹን የተጋሩ ነገሮች ደረጃ ይስጡ፣ እኩዮችን በ `crypto.enable_sm_openssl_preview = true` እንደገና ያስጀምሩ፣ የ`/status` መስክን ያረጋግጡ እና `iroha_sm_openssl_preview` መለኪያ ወደ `true` ይግለጡ ወይም ጥቅሉን ቀድመው ይቀይሩት ወይም እንደገና ይቀይሩት ቴሌሜትሪ በመርከብ ላይ ይለያያል።
- **የማስረጃ ማቆየት።** የOpenSSL/Tongsuo ፓኬጆችን የግንባታ መዝገቦችን በማህደር አስቀምጥ እና የምስክር ወረቀቶችን ከማረጋገጫ ሰጪው ጋር በመሆን ወደፊት የሚደረጉ ኦዲቶች የፕሮቨንስ ሰንሰለቱን እንደገና ማባዛት ይችላሉ።

## 6. የአደጋ ምላሽ
- ** የማረጋገጫ አለመሳካቶች:** ያለ SM ድጋፍ ወደ ግንብ ይመለሱ ወይም `sm2` ያስወግዱ
  ከ `allowed_signing` (እንደ አስፈላጊነቱ `default_hash` በመመለስ) እና ወደ ቀዳሚው ውድቀት
  በመመርመር ላይ እያለ መልቀቅ. ያልተሳኩ የክፍያ ጭነቶችን፣ ንጽጽር ሃሽዎችን እና የመስቀለኛ ምዝግብ ማስታወሻዎችን ያንሱ።
- ** የአፈጻጸም መመለሻዎች፡** የኤስኤም መለኪያዎችን ከEd25519/SHA2 መነሻ መስመሮች ጋር ያወዳድሩ።
  የARM ውስጣዊ መንገድ ልዩነትን የሚያስከትል ከሆነ፣ `crypto.sm_intrinsics = "force-disable"` ያቀናብሩ
  (በመጠባበቅ ላይ ያለ ትግበራ ባህሪ) እና ግኝቶችን ሪፖርት ያድርጉ።
- **የቴሌሜትሪ ክፍተቶች፡** ቆጣሪዎች ከጠፉ ወይም ካልዘመኑ፣ ችግር ያስገቡ
  የተለቀቀው ምህንድስናን በመቃወም; ክፍተቱ እስኪያልቅ ድረስ በሰፊው መልቀቅ አይቀጥሉ
  የሚለው ጥያቄ ተፈቷል።

## 7. የማረጋገጫ ዝርዝር አብነት
- [ ] ውቅረት ተዘጋጅቷል እና አቻ እንደገና ተጀምሯል።
- [] ቴሌሜትሪ ቆጣሪዎች የሚታዩ እና ዳሽቦርዶች ተዋቅረዋል።
- [ ] ተገዢነት/ህጋዊ እርምጃዎች ተመዝግበዋል።
- [ ] የልቀት ደረጃ በCrypto WG/በተለቀቀው TL ጸድቋል።
- [ ] የድህረ ልቀት ግምገማ ተጠናቅቋል እና ግኝቶች ተመዝግበዋል።

ይህንን የማረጋገጫ ዝርዝር በታቀደው ትኬት ውስጥ ያቆዩት እና `status.md` ያዘምኑ
የመርከቦች ሽግግር በደረጃዎች መካከል።