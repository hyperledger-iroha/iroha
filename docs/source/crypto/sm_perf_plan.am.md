---
lang: am
direction: ltr
source: docs/source/crypto/sm_perf_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 493c3c0f6a991b2a5d04f33f97b7e97bff372271c5c57751ff41f5e86d43cbc7
source_last_modified: "2025-12-29T18:16:35.944695+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## የኤስኤም አፈጻጸም ቀረጻ እና መነሻ እቅድ

ሁኔታ: የተረቀቀ - 2025-05-18  
ባለቤቶች፡ የአፈጻጸም WG (ሊድ)፣ Infra Ops (የላብራቶሪ መርሐግብር)፣ QA Guild (CI gating)  
ተዛማጅ የመንገድ ካርታ ተግባራት፡ SM-4c.1a/b፣ SM-5a.3b፣ FASTPQ ደረጃ 7 መሣሪያን አቋራጭ መቅረጽ

### 1. ዓላማዎች
1. Neoverse medians በ `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json` ይመዝግቡ። የአሁኑ መነሻ መስመሮች ከ `neoverse-proxy-macos` ቀረጻ በ`artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (ሲፒዩ መለያ `neoverse-proxy-macos`) በ SM3 ማነፃፀር መቻቻል ለ aarch64 macOS/Linux ወደ 0.70 ተዘርግቷል። ባዶ-ብረት ጊዜ ሲከፈት `scripts/sm_perf_capture_helper.sh --matrix --cpu-label neoverse-n2-b01 --output artifacts/sm_perf/<date>/neoverse-n2-b01` በ Neoverse አስተናጋጅ ላይ እንደገና ያስጀምሩ እና የተዋሃዱ ሚዲያዎችን ወደ መነሻ መስመሮች ያስተዋውቁ።  
2. `ci/check_sm_perf.sh` ሁለቱንም የአስተናጋጅ ክፍሎችን እንዲጠብቅ ተዛማጅ x86_64 ሚዲያዎችን ይሰብስቡ።  
3. የወደፊት የፐርፍ በሮች በጎሳ ዕውቀት ላይ እንዳይመሰረቱ ሊደገም የሚችል የቀረጻ አሰራርን ያትሙ (ትእዛዞች፣ አርቲፊክስ አቀማመጥ፣ ገምጋሚዎች)።

### 2. የሃርድዌር መገኘት
አሁን ባለው የስራ ቦታ የአፕል ሲሊኮን (ማክኦኤስ አርም64) አስተናጋጆች ብቻ ይገኛሉ። የ`neoverse-proxy-macos` ቀረጻው እንደ ጊዜያዊ የሊኑክስ መነሻ መስመር ወደ ውጭ ይላካል፣ ነገር ግን ባዶ-ሜታል ኒዮቨርስ ወይም x86_64 ሚዲያን ለመያዝ አሁንም በ`INFRA-2751` ስር የሚከታተለው የተጋራ ቤተ ሙከራ ሃርድዌር የላብራቶሪ መስኮቱ አንዴ ከተከፈተ በPerformance WG እንዲሄድ ይፈልጋል። የተቀሩት የቀረጻ መስኮቶች አሁን በሥነ ጥበብ ዛፉ ላይ ተመዝግበው ክትትል ይደረግባቸዋል፡-

- Neoverse N2 ባዶ-ሜታል (ቶኪዮ ራክ ቢ) ለ 2026-03-12 ተይዟል። ኦፕሬተሮች ከክፍል 3 ያሉትን ትዕዛዞች እንደገና ይጠቀማሉ እና ቅርሶችን በ `artifacts/sm_perf/2026-03-lab/neoverse-b01/` ስር ያከማቻሉ።
- x86_64 Xeon (Zurich rack D) ለ 2026-03-19 የተያዘው SMT ከተሰናከለ ድምጽን ለመቀነስ; ቅርሶች በ `artifacts/sm_perf/2026-03-lab/xeon-d01/` ስር ያርፋሉ።
- ሁለቱም መሬት ከሮጡ በኋላ ሚዲያን ወደ መነሻ መስመር JSONs ያስተዋውቁ እና የCI በርን በ`ci/check_sm_perf.sh` (የዒላማ መቀየሪያ ቀን፡ 2026-03-25) ያንቁ።

እስከ እነዚያ ቀኖች ድረስ፣ የ macOS arm64 መነሻ መስመሮች ብቻ በአካባቢው ሊታደሱ ይችላሉ።### 3. የመቅረጽ ሂደት
1. **የመሳሪያ ሰንሰለት አመሳስል**  
   ```bash
   rustup override set $(cat rust-toolchain.toml)
   cargo fetch
   ```
2. **የቀረጻ ማትሪክስ ይፍጠሩ** (በአስተናጋጅ)  
   ```bash
   scripts/sm_perf_capture_helper.sh --matrix \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}
   ```
   ረዳቱ አሁን `capture_commands.sh` እና `capture_plan.json` በዒላማው ማውጫ ስር ይጽፋል። ስክሪፕቱ የ`raw/*.json` መቅረጫ መንገዶችን በየሞድ ያዘጋጃል ስለዚህ የላብራቶሪ ቴክኒሻኖች ሩጫውን በቆራጥነት መመደብ ይችላሉ።
3. **ቀረጻዎችን አሂድ**  
   እያንዳንዱን ትዕዛዝ ከ`capture_commands.sh` ያስፈጽም (ወይም ተመጣጣኝውን በእጅ ያሂዱ)፣ እያንዳንዱ ሁነታ የተዋቀረ JSON ብሎብ በ`--capture-json` በኩል እንደሚያወጣ ያረጋግጡ። ሁልጊዜ የአስተናጋጅ መለያን በ`--cpu-label "<model/bin>"` (ወይም `SM_PERF_CPU_LABEL=<label>`) ያቅርቡ ስለዚህ የተቀረጸው ሜታዳታ እና ተከታዩ የመነሻ መስመሮች ሚዲያን ያመረቱትን ትክክለኛ ሃርድዌር ይመዘግባሉ። ረዳቱ ቀድሞውኑ ተገቢውን መንገድ ያቀርባል; በእጅ ለሚሠሩ ሂደቶች ንድፉ የሚከተለው ነው-
   ```bash
   SM_PERF_CAPTURE_LABEL=auto \
   scripts/sm_perf.sh --mode auto \
     --cpu-label "neoverse-n2-lab-b01" \
     --capture-json artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/auto.json
   ```
4. **ውጤቶችን አረጋግጥ**  
   ```bash
   scripts/sm_perf_check \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json
   ```
   በሩጫ መካከል ያለው ልዩነት በ± 3% ውስጥ መቆየቱን ያረጋግጡ። ካልሆነ ፣ የተጎዳውን ሁነታ እንደገና ያስጀምሩ እና በምዝግብ ማስታወሻው ውስጥ እንደገና መሞከሩን ያስተውሉ ።
5. **ሚዲያን ያስተዋውቁ**  
   ሚዲያን ለማስላት እና ወደ የመነሻ መስመር JSON ፋይሎች ለመቅዳት `scripts/sm_perf_aggregate.py` ይጠቀሙ፡
   ```bash
   scripts/sm_perf_aggregate.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json
   ```
   የረዳት ቡድኖቹ በ `metadata.mode` ይቀርጻሉ፣ እያንዳንዱ ስብስብ ይህንን እንደሚጋራ ያረጋግጣል።
   ተመሳሳይ `{target_arch, target_os}` ሶስት እጥፍ፣ እና የJSON ማጠቃለያ ከአንድ ግቤት ጋር ያወጣል።
   በእያንዳንዱ ሁነታ. በመሠረታዊ ፋይሎቹ ውስጥ ማረፍ ያለባቸው ሚዲያን ስር ይኖራሉ
   `modes.<mode>.benchmarks`፣ ተጓዳኝ `statistics` የማገጃ መዝገቦች እያለ
   ሙሉውን የናሙና ዝርዝር፣ ደቂቃ/ከፍተኛ፣ አማካኝ እና የህዝብ ብዛት stdev ለገምጋሚዎች እና CI።
   አንዴ የተዋሃደ ፋይል ካለ፣ የመነሻ መስመር JSONs (በ
   መደበኛ የመቻቻል ካርታ) በ:
   ```bash
   scripts/sm_perf_promote_baseline.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json \
     --out-dir crates/iroha_crypto/benches \
     --target-os unknown_linux_gnu \
     --overwrite
   ```
   ወደ ንዑስ ስብስብ ለመገደብ `--mode` ይሽሩት ወይም `--cpu-label` ለመሰካት
   የተዋሃደ ምንጭ ሲተወው የሲፒዩ ስም ተመዝግቧል።
   አንዴ ሁለቱም አስተናጋጆች በየሥነ ሕንፃው ሲጨርሱ ያዘምኑ፦
   - `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`
   - `sm_perf_baseline_x86_64_unknown_linux_gnu_{scalar,auto}.json` (አዲስ)

   የ `aarch64_unknown_linux_gnu_*` ፋይሎች አሁን `m3-pro-native` ያንፀባርቃሉ
   መቅረጽ (የሲፒዩ መለያ እና የሜታዳታ ማስታወሻዎች ተጠብቀዋል) ስለዚህ `scripts/sm_perf.sh` ይችላል
   ራስ-አግኝ aarch64-ያልታወቀ-linux-gnu አስተናጋጆች ያለ በእጅ ባንዲራዎች። መቼ
   ባሬ-ሜታል ላብራቶሪ አሂድ ተጠናቅቋል፣ እንደገና አስጀምር `scripts/sm_perf.sh --mode 
   --write-baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_.json`
   ጊዜያዊ ሚዲያዎችን ለመፃፍ እና እውነተኛውን ማህተም ለማድረግ ከአዲሱ ቀረጻዎች ጋር
   አስተናጋጅ መለያ.

   > ማጣቀሻ፡ የጁላይ 2025 አፕል ሲሊኮን ቀረጻ (ሲፒዩ መለያ `m3-pro-local`)
   > በ`artifacts/sm_perf/2025-07-lab/takemiyacStudio.lan/{raw,aggregated.json}` ስር ተቀምጧል።
   > Neoverse/x86 artefacts so ገምጋሚዎችን ሲያትሙ ያንን አቀማመጥ ያንጸባርቁት
   > ጥሬ/የተጠቃለሉ ውጤቶችን በቋሚነት ሊለያይ ይችላል።

### 4. Artefact Layout & Sign-off
```
artifacts/sm_perf/
  2025-07-lab/
    neoverse-b01/
      raw/
      aggregated.json
      run-log.md
    neoverse-b02/
      …
    xeon-d01/
    xeon-d02/
```
- `run-log.md` ትዕዛዙን hash፣git revision፣ኦፕሬተር እና ማናቸውንም ያልተለመዱ ነገሮችን ይመዘግባል።
- የተዋሃዱ የJSON ፋይሎች በቀጥታ ወደ መነሻ መስመር ዝመናዎች ይመገባሉ እና በ `docs/source/crypto/sm_perf_baseline_comparison.md` ውስጥ ካለው የአፈጻጸም ግምገማ ጋር ተያይዘዋል።
- QA Guild የመነሻ መስመሮች ከመቀየሩ በፊት ቅርሶቹን ይገመግማል እና በ `status.md` በአፈፃፀም ክፍል ስር ይፈርማል።### 5. CI Gating Timeline
| ቀን | ወሳኝ ምዕራፍ | ድርጊት |
|-------------|----|
| 2025-07-12 | Neoverse ቀረጻዎች ተጠናቅቋል | የ`sm_perf_baseline_aarch64_*` JSON ፋይሎችን ያዘምኑ፣ `ci/check_sm_perf.sh`ን በአገር ውስጥ ያሂዱ፣ PRን ከቅርሶች ጋር ይክፈቱ። |
| 2025-07-24 | x86_64 ቀረጻዎች ተጠናቅቀዋል | አዲስ የመነሻ ፋይሎችን + gating በ `ci/check_sm_perf.sh`; አቋራጭ CI መስመሮችን መጠቀማቸውን ያረጋግጡ። |
| 2025-07-27 | CI ማስፈጸሚያ | በሁለቱም የአስተናጋጅ ክፍሎች ላይ እንዲሰራ የ`sm-perf-gate` የስራ ፍሰትን ያንቁ; ዳግም ግስጋሴዎች ከተዋቀሩ መቻቻል ካለፉ ውህደቶች አይሳኩም። |

### 6. ጥገኛ እና ግንኙነት
- የላብራቶሪ መዳረሻ ለውጦችን በ `infra-ops@iroha.tech` ያስተባብሩ።  
- የአፈጻጸም WG በ `#perf-lab` ቻናል ውስጥ በየቀኑ ማሻሻያዎችን ይለጥፋል ፣ ሲሮጥ።  
- QA Guild የንፅፅር ልዩነትን (`scripts/sm_perf_compare.py`) ያዘጋጃል ስለዚህ ገምጋሚዎች ዴልታዎችን በምስል ማየት ይችላሉ።  
- መሰረታዊ መስመሮች አንዴ ከተዋሃዱ፣ `roadmap.md` (SM-4c.1a/b፣ SM-5a.3b) እና `status.md`ን ከቀረጻ የማጠናቀቂያ ማስታወሻዎች ጋር ያዘምኑ።

በዚህ እቅድ የኤስኤም ማፋጠን ስራ ሊባዙ የሚችሉ ሚዲያዎችን፣ CI gating እና ሊፈለግ የሚችል የማስረጃ ዱካ በማግኘቱ “የተጠባባቂ የላብራቶሪ መስኮቶችን እና ቀረጻ ሚዲያን” የድርጊት ንጥሉን ያረካል።

### 7. CI Gate & የአካባቢ ጭስ

- `ci/check_sm_perf.sh` ቀኖናዊ CI መግቢያ ነጥብ ነው። በ`SM_PERF_MODES` ውስጥ ለእያንዳንዱ ሁነታ ወደ `scripts/sm_perf.sh` ያወጣል (የ `scalar auto neon-force` ነባሪዎች) እና `CARGO_NET_OFFLINE=true` ያዘጋጃል ስለዚህ አግዳሚ ወንበሮች በ CI ምስሎች ላይ በትክክል ይሰራሉ።  
- `.github/workflows/sm-neon-check.yml` አሁን በ macOS arm64 ሯጭ ላይ ያለውን በር ይደውላል ስለዚህ እያንዳንዱ የመሳብ ጥያቄ በአካባቢው ጥቅም ላይ በሚውልበት ተመሳሳይ ረዳት በኩል scalar/auto/neon-force trioን ይለማመዳል። ተጨማሪው ሊኑክስ/ኒዮቨርስ ሌይን x86_64 መሬት ከያዘ እና የኒዮቨርስ ፕሮክሲ መነሻ መስመሮች በባዶ ብረት ሩጫ ከታደሱ በኋላ ይገናኛል።  
- ኦፕሬተሮች የሞድ ዝርዝሩን በአገር ውስጥ መሻር ይችላሉ፡- `SM_PERF_MODES="scalar" bash ci/check_sm_perf.sh` ለፈጣን የጭስ ሙከራ ሩጫውን ወደ አንድ ማለፊያ ያስተካክላል፣ ተጨማሪ ክርክሮች (ለምሳሌ `--tolerance 0.20`) በቀጥታ ወደ `scripts/sm_perf.sh` ይተላለፋሉ።  
- `make check-sm-perf` አሁን ለገንቢ ምቾት በሩን ያጠቃልላል; የ CI ስራዎች ስክሪፕቱን በቀጥታ ሊጠሩት የሚችሉት የማክኦኤስ ገንቢዎች ኢላማውን ሲያደርጉ ነው።  
- የኒዮቨርስ/x86_64 መነሻ መስመሮች አንዴ ካረፉ፣ ያው ስክሪፕት በ `scripts/sm_perf.sh` ውስጥ ባለው የአስተናጋጅ ራስ-ማወቂያ አመክንዮ በኩል ተገቢውን JSON ይወስዳል፣ ስለዚህ በእያንዳንዱ አስተናጋጅ ገንዳ ውስጥ የሚፈለገውን የሞድ ዝርዝር ከማዘጋጀት በዘለለ በስራ ፍሰቶች ውስጥ ምንም ተጨማሪ ሽቦ አያስፈልግም።

### 8. በየሩብ ጊዜ አድስ ረዳት- እንደ `artifacts/sm_perf/2026-Q1/<label>/` ያለ ሩብ ማህተም ያለበትን ማውጫ ለመቅዳት `scripts/sm_perf_quarterly.sh --owner "<name>" --cpu-label "<label>" [--quarter YYYY-QN] [--output-root artifacts/sm_perf]` ን ያሂዱ። ረዳቱ `scripts/sm_perf_capture_helper.sh --matrix` ይጠቀልላል እና `capture_commands.sh`፣ `capture_plan.json` እና `quarterly_plan.json` (ባለቤት + ሩብ ሜታዳታ) ያመነጫል ስለዚህ የላቦራቶሪ ኦፕሬተሮች ያለእጅ የተጻፈ ዕቅድ ሩጫዎችን መርሐግብር ማስያዝ ይችላሉ።
- የተፈጠረውን `capture_commands.sh` በዒላማው አስተናጋጅ ላይ ያስፈጽም, ጥሬ ውጤቶቹን ከ `scripts/sm_perf_aggregate.py --output <dir>/aggregated.json` ጋር ያዋህዱ እና ሚዲያን በ `scripts/sm_perf_promote_baseline.py --out-dir crates/iroha_crypto/benches --overwrite` በኩል ወደ መሰረታዊ JSONs ያስተዋውቁ. መቻቻል አረንጓዴ መቆየቱን ለማረጋገጥ `ci/check_sm_perf.sh`ን እንደገና ያሂዱ።
- ሃርድዌር ወይም የመሳሪያ ሰንሰለቶች ሲቀየሩ፣ በ`docs/source/crypto/sm_perf_baseline_comparison.md` ውስጥ ያሉትን የንፅፅር መቻቻል/ማስታወሻዎችን ያድሱ፣ አዲሶቹ ሚዲያዎች ከተረጋጉ የ`ci/check_sm_perf.sh` መቻቻልን ያጠናክሩ እና ማንኛውንም የዳሽቦርድ/የማስጠንቀቂያ ገደቦችን ከአዲሱ የመነሻ መስመሮች ጋር ያስተካክሉ ስለዚህ የኦፕስ ማንቂያዎች ትርጉም ያለው ሆነው እንዲቆዩ ያድርጉ።
- `quarterly_plan.json`፣ `capture_plan.json`፣ `capture_commands.sh`፣ እና የተዋሃደውን JSON ከመነሻ መስመር ማሻሻያ ጋር ግባ፤ ለመከታተል ተመሳሳይ ቅርሶችን ከሁኔታ/የፍተሻ ካርታ ዝመናዎች ጋር ያያይዙ።