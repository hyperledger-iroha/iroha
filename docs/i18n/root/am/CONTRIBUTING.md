---
lang: am
direction: ltr
source: CONTRIBUTING.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71baf5d038cbe6518fd294fcc1b279dff8aaf092e4a83f6159b699a378e51467
source_last_modified: "2025-12-29T18:16:34.772429+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የአስተዋጽኦ መመሪያ

ለIroha 2 ለማበርከት ጊዜ ስለወሰዱ እናመሰግናለን!

እባኮትን እንዴት ማበርከት እንደሚችሉ እና የትኞቹን መመሪያዎች እንዲከተሉ እንደምንጠብቅ ለማወቅ ይህንን መመሪያ ያንብቡ። ይህ ስለ ኮድ እና ሰነዶች መመሪያዎችን እንዲሁም የጊት የስራ ፍሰትን በተመለከተ የእኛን የአውራጃ ስብሰባዎች ያካትታል።

እነዚህን መመሪያዎች ማንበብ በኋላ ጊዜ ይቆጥብልዎታል.

## እንዴት ማበርከት እችላለሁ?

ለፕሮጀክታችን አስተዋፅኦ ማድረግ የሚችሉባቸው ብዙ መንገዶች አሉ፡-

- ሪፖርት ያድርጉ [ሳንካዎች](#reporting-bugs) እና [ተጋላጭነት](#reporting-vulnerabilities)
- [ማሻሻያዎችን ጠቁም](#suggesting-improvements) እና ተግባራዊ ያድርጉ
- [ጥያቄዎችን ይጠይቁ](#asking-questions) እና ከማህበረሰቡ ጋር ይሳተፉ

ለፕሮጀክታችን አዲስ ነገር አለ? [የመጀመሪያውን አስተዋጽዖ ያድርጉ](#your-first-code-contribution)!

### TL;DR

- [ZenHub] (https://app.zenhub.com/workspaces/iroha-v2-60ddb820813b9100181fc060/board?repos=181739240) ያግኙ።
- ሹካ [Iroha](https://github.com/hyperledger-iroha/iroha/tree/main)።
- የመረጡትን ጉዳይ ያስተካክሉ።
- ለኮድ እና ለሰነድ የእኛን [የቅጥ መመሪያዎች](#style-guides) መከተልዎን ያረጋግጡ።
- ጻፍ [ሙከራዎች] (https://doc.rust-lang.org/cargo/commands/cargo-test.html). ሁሉም ማለፋቸውን ያረጋግጡ (`cargo test --workspace`)። የኤስ ኤም ክሪፕቶግራፊ ቁልልን ከነካክ፣ እንዲሁም `cargo test -p iroha_crypto --features "sm sm_proptest"`ን አስኪ አማራጭ ፉዝ/ንብረት ማሰሪያን ለማስፈጸም።
  - ማሳሰቢያ፡ የIVM ፈጻሚን የሚለማመዱ ሙከራዎች `defaults/executor.to` ከሌለ በትንሹ የሚወስን ፈጻሚ ባይት ኮድ በራስ ሰር ያዋህዳሉ። ሙከራዎችን ለማካሄድ ምንም ቅድመ-ደረጃ አያስፈልግም. ለፓሪቲ ቀኖናዊ ባይትኮድ ለማመንጨት የሚከተሉትን ማድረግ ይችላሉ፡-
    - `cargo run --manifest-path scripts/generate_executor_to/Cargo.toml`
    - `cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml`
- ዴሪቭ/ፕሮክ-ማክሮ ሳጥኖችን ከቀየሩ፣ trybuild UI suites ን በ በኩል ያሂዱ
  `make check-proc-macro-ui` (ወይም
  `PROC_MACRO_UI_CRATES="crate1 crate2" make check-proc-macro-ui`) እና አድስ
  `.stderr` መጫዎቻዎች መልእክቶች እንዲረጋጉ ምርመራዎች ሲቀየሩ።
- fmt/clippy/build/test with `--locked` plus `swift test` ለማስፈጸም `make dev-workflow` (በ `scripts/dev_workflow.sh` ዙሪያ መጠቅለያ) ያሂዱ; `cargo test --workspace` ሰዓታትን እንዲወስድ እና `--skip-tests` ለፈጣን የአካባቢ ዑደቶች ብቻ እንዲጠቀም ይጠብቁ። ለሙሉ runbook `docs/source/dev_workflow.md` ይመልከቱ።
- `make check-agents-guardrails` የ `Cargo.lock` አርትዖቶችን እና አዲስ የስራ ቦታ ሳጥኖችን ለማገድ፣ `make check-dependency-discipline` በአዳዲስ ጥገኞች ላይ በግልፅ ካልተፈቀደ በቀር እንዳይሳካ እና `make check-missing-docs` በ `make check-missing-docs` ላይ የሚጠፋውን አዲስ I18NI04X30001 ሳጥኖች፣ ወይም አዲስ ይፋዊ እቃዎች ያለ ዶክ አስተያየት (ጠባቂው `docs/source/agents/missing_docs_inventory.{json,md}` በ `scripts/inventory_missing_docs.py` ያድሳል)። `make check-tests-guard` ጨምር ስለዚህ የተቀየረው ተግባር አይሳካም አሀድ ካልፈተነ በስተቀር (የመስመር ውስጥ `#[cfg(test)]`/`#[test]` ብሎኮች ወይም crate `tests/`፤ ነባር የሽፋን ብዛት) እና `make check-docs-tests-metrics` እና `make check-docs-tests-metrics` ከተደረጉ ለውጦች ጋር የተጣመሩ ናቸው መለኪያዎች / ዳሽቦርዶች. የ TODO ማስፈጸሚያን በI18NI0000142X ያቆዩ ስለዚህ የTODO ማርከሮች ያለ ሰነዶች/ሙከራዎች አይጣሉም። `make check-env-config-surface` env-toggle inventory ያድሳል እና አዲስ ** ምርት ** env shims ከ `AGENTS_BASE_REF` ጋር ሲታዩ አሁን አይሳካም; `ENV_CONFIG_GUARD_ALLOW=1` አዘጋጅ በስደት መከታተያ ውስጥ ሆን ተብሎ የተደረጉ ተጨማሪዎችን ከሰነድ በኋላ ብቻ። `make check-serde-guard` የሴሬድ ክምችትን ያድሳል እና በቆዩ ቅጽበተ-ፎቶዎች ወይም አዲስ ምርት `serde`/`serde_json` hits; `SERDE_GUARD_ALLOW=1` ከጸደቀ የፍልሰት እቅድ ጋር ብቻ አዘጋጅ። በጸጥታ ከማዘግየት ይልቅ በTODO የዳቦ ፍርፋሪ እና የክትትል ትኬቶች በኩል ትልልቅ ማስተላለፎች እንዲታዩ ያድርጉ። `make check-std-only`ን ያሂዱ `no_std`/`wasm32` cfgs እና I18NI0000153X ክፍት እቃዎች ክፍት-ብቻ ሆነው እንዲቀጥሉ እና የመንገድ ካርታ/ሁኔታ በአንድነት መሬት እንደሚቀየር ለማረጋገጥ; `STATUS_SYNC_ALLOW_UNPAIRED=1` አቀናብር ለብርቅ ሁኔታ-ብቻ የትየባ ጥገናዎች `AGENTS_BASE_REF` ከተሰካ በኋላ። ለአንድ ጥሪ፣ ሁሉንም የጥበቃ መንገዶችን አንድ ላይ ለማሄድ `make agents-preflight` ይጠቀሙ።
- ከመግፋቱ በፊት የአካባቢያዊ ተከታታይ ጥበቃዎችን ያሂዱ: `make guards`.
  - ይህ በቀጥታ `serde_json`ን በምርት ኮድ ይከለክላል፣ ከተፈቀደ ዝርዝር ውጭ አዲስ የቀጥታ ሰርዴ ዴፕን ይከለክላል እና ከ`crates/norito` ውጭ የአድሆክ AoS/NCB ረዳቶችን ይከላከላል።
- እንደ አማራጭ ደረቅ-አሂድ I18NT0000003X ባህሪ ማትሪክስ በአካባቢው: `make norito-matrix` (ፈጣን ንዑስ ስብስብ ይጠቀማል).
  - ለሙሉ ሽፋን `scripts/run_norito_feature_matrix.sh` ያለ `--fast` ያሂዱ።
  - በአንድ ጥምር የታችኛው ተፋሰስ ጭስ ለማካተት (ነባሪ crate `iroha_data_model`): `make norito-matrix-downstream` ወይም `scripts/run_norito_feature_matrix.sh --fast --downstream [crate]`.
- ለፕሮክ-ማክሮ ሣጥኖች `trybuild` UI harness (`tests/ui.rs` + I18NI0000169X/`tests/ui/fail`) ይጨምሩ እና ላልተሳኩ ጉዳዮች `.stderr` ምርመራ ያድርጉ። ዲያግኖስቲክስ የተረጋጋ እና የማይደናገጥ ያድርጉ; መገልገያዎችን በ `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` ያድሱ እና በ `cfg(all(feature = "trybuild-tests", not(coverage)))` ይጠብቋቸው።
- እንደ ቅርጸት እና ቅርሶች ዳግም መወለድ ያሉ የቅድመ-ይግባኝ ተግባሮችን ያከናውኑ ([`pre-commit.sample`](./hooks/pre-commit.sample ይመልከቱ))
- ለመከታተል ከ `upstream` ጋር [Hyperledger Iroha ማከማቻ](https://github.com/hyperledger-iroha/iroha) ፣ `git pull -r upstream main` ፣ `git commit -s` ፣ I01000000177X ፣ I0100000177X ፣I01000000177X ፣I0100000177 ጥያቄ](https://github.com/hyperledger-iroha/iroha/compare) ወደ I18NI0000179X ቅርንጫፍ። [የመጎተት ጥያቄ መመሪያዎች](#pull-request-etiquette) መከተሉን ያረጋግጡ።

### ወኪሎች የስራ ፍሰት ፈጣን ጅምር

- `make dev-workflow` (በ `scripts/dev_workflow.sh` ዙሪያ መጠቅለያ ፣ በ `docs/source/dev_workflow.md` ውስጥ የተመዘገበ) ያሂዱ። `cargo fmt --all`፣ `cargo clippy --workspace --all-targets --locked -- -D warnings`፣ `cargo build/test --workspace --locked` (ፈተናዎች ብዙ ሰአታት ሊወስዱ ይችላሉ) እና `swift test` ይጠቀልላል።
- ለፈጣን ድግግሞሽ `scripts/dev_workflow.sh --skip-tests` ወይም I18NI0000188X ይጠቀሙ; የመጎተት ጥያቄን ከመክፈትዎ በፊት ሙሉውን ቅደም ተከተል እንደገና ያስጀምሩ።
- የጥበቃ መንገዶች፡- `Cargo.lock`ን ከመንካት ይቆጠቡ፣ አዲስ የስራ ቦታ አባላትን ከመጨመር፣ አዲስ ጥገኛዎችን ከማስተዋወቅ፣ አዲስ `#[allow(missing_docs)]` ሺምስ መጨመር፣ የክሬት ደረጃ ሰነዶችን መተው፣ ተግባራትን ሲቀይሩ ፈተናዎችን መዝለል፣ የ TODO ምልክቶችን ያለ ሰነዶች/ሙከራዎች መጣል ወይም እንደገና ማስተዋወቅ `no_std`/`wasm32` cfgs ያለፈቃድ። `make check-agents-guardrails` (ወይም `AGENTS_BASE_REF=origin/main bash ci/check_agents_guardrails.sh`) ሲደመር I18NI0000195X፣ `make check-missing-docs` (`docs/source/agents/missing_docs_inventory.{json,md}` ያድሳል)፣ `docs/source/agents/missing_docs_inventory.{json,md}`ን ያሂዱ፣ I18NI000000197Xን ያካሂዱ፣ I18NI000000197Xን ያካሂዱ፣ I18NI000000197Xን ያካሂዱ - ያለ አሃድ ውስጥ የሙከራ ተግባራት ሲቀየሩ አይሳካም። ተግባሩን ማጣቀስ አለበት)፣ `make check-docs-tests-metrics` (የመንገድ ካርታ ለውጦች ሰነዶች/ሙከራዎች/ሜትሪክስ ማሻሻያዎች ሲጎድሉ ይከሽፋል)፣ `make check-todo-guard`፣ `make check-env-config-surface` (ያረጁ ኢንቬንቶሪዎች ወይም አዲስ የምርት env መቀያየሪያዎች ላይ አልተሳካም)፣ በI182NI020000000000000000000000000 `make check-serde-guard` (ያረጁ ሰርዴ ኢንቬንቶሪዎች ወይም አዲስ የምርት serde hits ላይ አልተሳካም፤ በ`SERDE_GUARD_ALLOW=1` በተፈቀደ የፍልሰት ዕቅድ ብቻ መሻር) በአገር ውስጥ ለቅድመ ምልክት፣ `make check-std-only` ለstd-ብቻ ጥበቃ እና አቆይ። `roadmap.md`/`status.md` ከ `make check-status-sync` ጋር በማመሳሰል (`STATUS_SYNC_ALLOW_UNPAIRED=1` ለ ብርቅዬ ሁኔታ ብቻ `AGENTS_BASE_REF` አዘጋጅ)። PR ከመክፈትዎ በፊት ሁሉንም ጠባቂዎች ለማሄድ አንድ ነጠላ ትእዛዝ ከፈለጉ `make agents-preflight` ይጠቀሙ።

### ሳንካዎችን ሪፖርት ማድረግ

*ስህተት* በIroha ውስጥ ያለ ስህተት፣ የንድፍ ጉድለት፣ ውድቀት ወይም ጥፋት ሲሆን ይህም የተሳሳተ፣ ያልተጠበቀ ወይም ያልታሰበ ውጤት ወይም ባህሪ እንዲያመጣ ያደርገዋል።

የIroha ሳንካዎችን በ[GitHub ጉዳዮች](https://github.com/hyperledger-iroha/iroha/issues?q=is%3Aopen+is%3Aissue+label%3ABug) በ`Bug` መለያ እንከታተላለን።

አዲስ እትም ሲፈጥሩ የሚሞሉበት አብነት አለ። ስህተቶችን ሲዘግቡ ማድረግ ያለብዎትን ዝርዝር እነሆ፡-
- [ ] የ `Bug` መለያ ያክሉ
- [ ] ጉዳዩን አብራራ
- [ ] አነስተኛ የሥራ ምሳሌ ያቅርቡ
- [ ] ቅጽበታዊ ገጽ እይታን ያያይዙ

<ዝርዝሮች> <ማጠቃለያ>ዝቅተኛው የስራ ምሳሌ</ ማጠቃለያ>

ለእያንዳንዱ ሳንካ፣ [ቢያንስ የስራ ምሳሌ](https://en.wikipedia.org/wiki/Minimal_working_example) ማቅረብ አለቦት። ለምሳሌ፡-

```
# Minting negative Assets with value spec `Numeric`.

I was able to mint negative values, which shouldn't be possible in Iroha. This is bad because <X>.

# Given

I managed to mint negative values by running
<paste the code here>

# I expected

not to be able to mint negative values

# But, I got

<code showing negative value>

<paste a screenshot>
```

</details>

---
**ማስታወሻ፡** እንደ ጊዜው ያለፈበት ሰነድ፣ በቂ ያልሆነ ሰነድ ወይም የባህሪ ጥያቄዎች ያሉ ጉዳዮች የ`Documentation` ወይም `Enhancement` መለያዎችን መጠቀም አለባቸው። ትኋኖች አይደሉም።

---

### ተጋላጭነቶችን ሪፖርት ማድረግ

የደህንነት ችግሮችን በመከላከል ረገድ ንቁ ብንሆንም፣ ከማድረጋችን በፊት የደህንነት ተጋላጭነት ሊያጋጥሙዎት ይችላሉ።

- ከመጀመሪያው ዋና ዋና መለቀቅ (2.0) በፊት ሁሉም ተጋላጭነቶች እንደ ስህተት ይቆጠራሉ፣ ስለዚህ እንደ ስህተት (ከላይ ያለውን መመሪያ በመከተል) (#reporting-bugs) ለማቅረብ ነፃነት ይሰማዎ።
- ከመጀመሪያው ዋና መለቀቅ በኋላ፣ ተጋላጭነቶችን ለማስገባት እና ሽልማት ለማግኘት የእኛን [bug bounty program](https://hackerone.com/hyperledger) ይጠቀሙ።

: ቃለ አጋኖ፡ ባልተሸፈነ የደህንነት ተጋላጭነት የሚደርሰውን ጉዳት ለመቀነስ ተጋላጭነቱን በተቻለ ፍጥነት ለHyperledger መግለፅ እና **ተመሳሳዩን ተጋላጭነት ለምክንያታዊ ጊዜ በይፋ ከመግለጽ መቆጠብ**።

የእኛን የደህንነት ተጋላጭነቶች አያያዝ በተመለከተ ማንኛቸውም ጥያቄዎች ካሉዎት፣ እባክዎን በRocket.Chat የግል መልእክቶች ውስጥ ካሉት በአሁኑ ጊዜ ንቁ ጠባቂዎችን ለማነጋገር ነፃነት ይሰማዎ።

### ማሻሻያዎችን ይጠቁማል

በ GitHub ላይ [ችግር](https://github.com/hyperledger-iroha/iroha/issues/new) ፍጠር ተገቢ መለያዎች (`Optimization`፣ `Enhancement`) እና እየጠቆምክ ያለውን መሻሻል ግለጽ። ይህንን ሃሳብ ለእኛ ወይም ለሌላ ሰው እንድናዳብር ትተውት ወይም እርስዎ እራስዎ ተግባራዊ ሊያደርጉት ይችላሉ።

ጥቆማውን እራስዎ ለመተግበር ካሰቡ የሚከተሉትን ያድርጉ።

1. የፈጠርከውን ችግር *በእሱ ላይ መስራት ከመጀመርህ በፊት* ስጥ።
2. እርስዎ ባቀረቡት ባህሪ ላይ ይስሩ እና የእኛን [የኮድ እና ሰነዶች መመሪያ](#style-guides) ይከተሉ።
3. የመጎተት ጥያቄን ለመክፈት ዝግጁ በሚሆኑበት ጊዜ [የጎትታ ጥያቄ መመሪያዎችን](#pull-request-etiquette) መከተልዎን ያረጋግጡ እና ቀደም ሲል የተፈጠረውን ችግር እንደመተግበር ምልክት ያድርጉበት፡

   ```
   feat: Description of the feature

   Explanation of the feature

   Closes #1234
   ```

4. የእርስዎ ለውጥ የኤፒአይ ለውጥ የሚፈልግ ከሆነ፣ የ`api-changes` መለያን ይጠቀሙ።

   **ማስታወሻ፡** የ API ለውጦችን የሚሹ ባህሪያት Iroha ላይብረሪ ሰሪዎች ኮዳቸውን እንዲያዘምኑ ስለሚፈልጉ ለመተግበር እና ለማጽደቅ ብዙ ጊዜ ሊወስዱ ይችላሉ።###ጥያቄዎች

ጥያቄ ስህተት ወይም ባህሪ ወይም የማመቻቸት ጥያቄ ያልሆነ ማንኛውም ውይይት ነው።

<ዝርዝሮች> <ማጠቃለያ> ጥያቄን እንዴት እጠይቃለሁ? </ ማጠቃለያ>

እባክዎን ጥያቄዎችዎን ወደ [የእኛ ፈጣን መልእክት መላላኪያ መድረክ](#contact) ይለጥፉ ይህም ሰራተኞች እና የማህበረሰቡ አባላት በጊዜው እንዲረዱዎት።

እርስዎ ከላይ የተጠቀሰው ማህበረሰብ አካል እንደመሆናችሁ መጠን ሌሎችን ለመርዳት ማሰብ አለባችሁ። ለመርዳት ከወሰኑ፣ እባክዎን [በአክብሮት](CODE_OF_CONDUCT.md) ያድርጉት።

</details>

## የእርስዎ የመጀመሪያ ኮድ አስተዋጽዖ

1. በ[ጥሩ-የመጀመሪያ-ጉዳይ](https://github.com/hyperledger-iroha/iroha/labels/good%20first%20issue) መለያ ጉዳዮች መካከል ለጀማሪ ተስማሚ የሆነ ጉዳይ ያግኙ።
2. ለማንም ያልተመደበ መሆኑን በማጣራት በመረጧቸው ጉዳዮች ላይ ማንም እየሰራ እንዳልሆነ ያረጋግጡ።
3. አንድ ሰው በእሱ ላይ እየሰራ መሆኑን ሌሎች እንዲያዩ ጉዳዩን ለራስዎ ይመድቡ።
4. ኮድ መጻፍ ከመጀመርዎ በፊት የእኛን [የዝገት ስታይል መመሪያ](#rust-style-guide) ያንብቡ።
5. ለውጦችዎን ለማድረግ ዝግጁ ሲሆኑ [የመጎተት ጥያቄ መመሪያዎችን](#pull-request-etiquette) ያንብቡ።

## የመጎተት ጥያቄ ስነምግባር

እባክዎን [ሹካ](I18NU0000068X) [ማከማቻ](https://github.com/hyperledger-iroha/iroha/tree/main) እና [የባህሪ ቅርንጫፍ ይፍጠሩ](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) ለእርስዎ አስተዋፅዖ። ከ ** PRs ከሹካዎች** ጋር ሲሰሩ [ይህን መመሪያ](https://help.github.com/articles/checking-out-pull-requests-locally) ይመልከቱ።

#### በኮድ አስተዋፅዖ ላይ በመስራት ላይ፡-
- [የዝገት ስታይል መመሪያ](#rust-style-guide) እና [የሰነድ ዘይቤ መመሪያ](#documentation-style-guide) ተከተል።
- የጻፍከው ኮድ በፈተና የተሸፈነ መሆኑን ያረጋግጡ። ሳንካ ካስተካከሉ፣እባኮትን አነስተኛውን የስራ ምሳሌ ወደ ፈተና ይቀይሩት።
- ዴሪቭ/ፕሮክ-ማክሮ ሳጥኖችን ሲነኩ `make check-proc-macro-ui` (ወይም) ያሂዱ።
  ማጣሪያ በ`PROC_MACRO_UI_CRATES="crate1 crate2"`) ስለዚህ የዩአይ መገልገያዎችን ይሞክሩ
  በማመሳሰል ውስጥ ይቆዩ እና ምርመራዎች የተረጋጋ እንደሆኑ ይቆያሉ።
- አዲስ ይፋዊ ኤፒአይዎችን (የክሬት ደረጃ `//!` እና `///` በአዲስ ንጥሎች ላይ) እና አሂድ
  `make check-missing-docs` የጥበቃ ሀዲዱን ለማረጋገጥ። ዶክመንቶችን ይደውሉ/ይፈትኑሃል
  በመጎተት ጥያቄ መግለጫዎ ውስጥ ተጨምሯል።

### ስራህን በመፈጸም ላይ፡-
- [Git Style Guide](#git-workflow) ተከተል።
- ድርጊቶችዎን ያጥፉ (ከዚህ በፊት) (https://www.git-tower.com/learn/git/faq/git-squash/) ወይም [በውህደቱ ወቅት] (https://rietta.com/blog/github-merge-types/)።
- የፍላጎት ጥያቄዎ በሚዘጋጅበት ወቅት ቅርንጫፍዎ ጊዜው ያለፈበት ከሆነ፣ በ `git pull --rebase upstream main` እንደገና ያዋቅሩት። በአማራጭ፣ ተቆልቋይ ሜኑ ለ`Update branch` ቁልፍ መጠቀም እና የ `Update with rebase` አማራጭን መምረጥ ትችላለህ።

  ይህንን ሂደት ለሁሉም ሰው ቀላል ለማድረግ ፍላጎት ፣ ለጎትት ጥያቄ ከአንድ እፍኝ በላይ ላለማድረግ ይሞክሩ እና የባህሪ ቅርንጫፎችን እንደገና ከመጠቀም ይቆጠቡ።

#### የመሳብ ጥያቄ መፍጠር፡-
- በ[Pull Request Etiquette](#pull-request-etiquette) ክፍል ውስጥ ያለውን መመሪያ በመከተል ተገቢውን የመጎተት ጥያቄ መግለጫ ይጠቀሙ። ከተቻለ ከእነዚህ መመሪያዎች ማፈንገጥን ያስወግዱ።
- በአግባቡ የተቀረጸ [የመጎተት ጥያቄ ርዕስ](#pull-request-titles) ያክሉ።
- ኮድዎ ለመዋሃድ ዝግጁ እንዳልሆነ ከተሰማዎት ግን ጠባቂዎቹ እንዲመለከቱት ከፈለጉ ረቂቅ የመሳብ ጥያቄ ይፍጠሩ።

#### ስራዎን በማዋሃድ;
- የመሳብ ጥያቄ ከመዋሃዱ በፊት ሁሉንም አውቶማቲክ ቼኮች ማለፍ አለበት። ቢያንስ፣ ኮዱ መቀረፅ አለበት፣ ሁሉንም ፈተናዎች በማለፍ፣ እንዲሁም ምንም የላቀ `clippy` ሊንት የለውም።
- የመሳብ ጥያቄ ከንቁ ጠባቂዎች ሁለት ማረጋገጫዎች ከሌለ ሊዋሃድ አይችልም።
- እያንዳንዱ የመሳብ ጥያቄ የኮድ ባለቤቶችን በራስ-ሰር ያሳውቃል። የዘመኑ ጠባቂዎች ዝርዝር በ[MAINTAINERS.md](MAINTAINERS.md) ውስጥ ይገኛል።

#### ሥነ-ምግባርን ይገምግሙ፡-
- ውይይትን በራስዎ አይፍቱ። ገምጋሚው ውሳኔ ይስጥ።
- የግምገማ አስተያየቶችን እውቅና ይስጡ እና ከገምጋሚው ጋር ይሳተፉ (እስማማለሁ፣ አልስማማም፣ ግልጽ ማድረግ፣ ማስረዳት፣ ወዘተ)። አስተያየቶችን ችላ አትበል።
- ለቀላል የኮድ ለውጥ ጥቆማዎች በቀጥታ ከተጠቀሙባቸው ውይይቱን መፍታት ይችላሉ።
- አዳዲስ ለውጦችን በሚገፋፉበት ጊዜ የቀደመውን ቃልዎን ከመፃፍ ይቆጠቡ። ካለፈው ግምገማ በኋላ የተለወጠውን ነገር ያደበዝዛል እና ገምጋሚው ከባዶ እንዲጀምር ያስገድደዋል። ኮሚቶች በራስ-ሰር ከመዋሃዳቸው በፊት ይጨመቃሉ።

### የጥያቄ ርዕሶችን ይጎትቱ

የለውጥ መዝገቦችን ለማመንጨት ሁሉንም የተዋሃዱ የመሳብ ጥያቄዎችን ርዕስ እንተነተናል። እንዲሁም ርዕሱ ኮንቬንሽኑን የሚከተል መሆኑን በ *`check-PR-title`* ቼክ እናረጋግጣለን።

የ*`check-PR-title`* ቼክን ለማለፍ የፑል መጠየቂያ ርዕስ የሚከተሉትን መመሪያዎች ማክበር አለበት፡

<ዝርዝሮች> <ማጠቃለያ> ዝርዝር ርዕስ መመሪያዎችን ለማንበብ ዘርጋ</ ማጠቃለያ>

1. [የተለመደ ግዴታዎች](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers) ቅርጸትን ተከተል።

2. የመጎተት ጥያቄው አንድ ቃል ብቻ ከሆነ፣የPR ርዕስ ከተግባር መልእክት ጋር ተመሳሳይ መሆን አለበት።

</details>

### Git የስራ ፍሰት

- [ፎርክ](I18NU0000081X) [ማጠራቀሚያ](https://github.com/hyperledger-iroha/iroha/tree/main) እና [የባህሪ ቅርንጫፍ ፍጠር](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) ለእርስዎ አስተዋጽዖ።
ሹካዎን ከ[I18NT0000002X Iroha ማከማቻ](https://github.com/hyperledger-iroha/iroha/tree/main) ጋር ለማመሳሰል [የርቀት መቆጣጠሪያውን ያዋቅሩ](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/configuring-a-remote-repository-for-a-fork)።
- [Git Rebase Workflow](https://git-rebase.io/) ተጠቀም። `git pull` ከመጠቀም ይቆጠቡ። በምትኩ `git pull --rebase` ይጠቀሙ።
- የእድገት ሂደቱን ለማቃለል የቀረበውን [git hooks](./hooks/) ይጠቀሙ።

እነዚህን የቃል መመሪያዎች ይከተሉ፡-

- ** እያንዳንዱን ቃል ይውጡ ***። ካላደረጉ፣ [DCO](https://github.com/apps/dco) እንዲዋሃዱ አይፈቅድልዎም።

  `git commit -s`ን በራስ ሰር ለማከል `Signed-off-by: $NAME <$EMAIL>`ን እንደ የቃል ቃልዎ የመጨረሻ መስመር ይጠቀሙ። የእርስዎ ስም እና ኢሜይል በ GitHub መለያዎ ላይ ከተጠቀሰው ጋር አንድ አይነት መሆን አለበት።

  እንዲሁም `git commit -sS` ([የበለጠ ለመረዳት](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits) በመጠቀም ቃል ኪዳንዎን በጂፒጂ ቁልፍ እንዲፈርሙ እናበረታታዎታለን።

  ግዴታዎችን በራስ ሰር ለማቋረጥ [`commit-msg` hook](./hooks/) መጠቀም ይችላሉ።

- መልእክቶችን አስገባ [የተለመደ ግዴታዎች](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers) እና እንደ [የመጎተት መጠየቂያ ርዕሶች](I18NU0000092X) ተመሳሳይ የስያሜ እቅድ መከተል አለባቸው። ይህ ማለት፡-
  - **አሁን ያለውን ጊዜ ተጠቀም** ("ባህሪ አክል" ሳይሆን "የተጨመረ ባህሪ")
  - **አስገዳጅ ስሜትን ተጠቀም** ("ወደ ዶክከር አሰማር..." ሳይሆን "ወደ ዶክከር ያሰማራል...")
- ትርጉም ያለው ቃል ኪዳን ይጻፉ።
- የቃል ቃልን አጭር ለማድረግ ይሞክሩ።
- ረዘም ያለ የቃል መልእክት እንዲኖርዎት ከፈለጉ፡-
  - የመልእክትዎን የመጀመሪያ መስመር በ 50 ቁምፊዎች ወይም ከዚያ በታች ይገድቡ።
  - የቃል ኪዳንዎ የመጀመሪያ መስመር የሰሩት ስራ ማጠቃለያ መያዝ አለበት። ከአንድ በላይ መስመር ከፈለጉ በእያንዳንዱ አንቀጽ መካከል ባዶ መስመር ይተዉ እና ለውጦቹን በመሃል ይግለጹ። የመጨረሻው መስመር መውጫ መሆን አለበት.
- መርሃግብሩን ካሻሻሉ (እቅዱን በ `kagami schema` እና diff በማመንጨት ያረጋግጡ) ሁሉንም ለውጦች በ `[schema]` መልእክት በተለየ ቃል ማድረግ አለብዎት ።
- ትርጉም ባለው ለውጥ አንድ ቃል ለመከተል ይሞክሩ።
  - በአንድ PR ውስጥ ብዙ ጉዳዮችን ካስተካከሉ የተለየ ቃል ይስጧቸው።
  - ቀደም ሲል እንደተገለፀው በ `schema` እና በኤፒአይ ላይ የተደረጉ ለውጦች ከቀሪው ስራዎ ተለይተው በተገቢው ቁርጠኝነት መከናወን አለባቸው።
  - ከተግባራዊነቱ ጋር በተመሳሳይ ቃል ለተግባራዊነት ሙከራዎችን ያክሉ።

## ሙከራዎች እና መመዘኛዎች

- የምንጭ-ኮድ ሙከራዎችን ለማስኬድ፣ [`cargo test`](https://doc.rust-lang.org/cargo/commands/cargo-test.html) በIroha ስር ያስፈጽሙ። ይህ ረጅም ሂደት መሆኑን ልብ ይበሉ.
- መለኪያዎችን ለማስኬድ [`cargo bench`](https://doc.rust-lang.org/cargo/commands/cargo-bench.html) ከ Iroha ስር ያስፈጽሙ። የቤንችማርክ ውጽዓቶችን ለማረም የ`debug_assertions` አካባቢን እንደዚሁ ያዘጋጁ፡ `RUSTFLAGS="--cfg debug_assertions" cargo bench`።
- በአንድ የተወሰነ አካል ላይ እየሰሩ ከሆነ, `cargo test` ን በ [የስራ ቦታ] (https://doc.rust-lang.org/cargo/reference/workspaces.html) ውስጥ ሲያሄዱ, ለዚያ የስራ ቦታ ሙከራዎችን ብቻ እንደሚያካሂድ ያስታውሱ, ይህም አብዛኛውን ጊዜ ምንም [የውህደት ሙከራዎች] (https://www.testingxperts.com/blog/what-is-integration-testing) አያካትትም.
- ለውጦችዎን በትንሹ አውታረ መረብ ላይ መሞከር ከፈለጉ የቀረበው [`docker-compose.yml`](I18NU0000097X) የ 4 Iroha እኩዮችን በዶክ ኮንቴይነሮች ውስጥ መግባባትን እና ከንብረት ስርጭት ጋር የተያያዘ አመክንዮ ይፈጥራል። [`iroha-python`](https://github.com/hyperledger-iroha/iroha-python) ወይም የተካተተውን Iroha ደንበኛ CLIን በመጠቀም ከዚያ አውታረ መረብ ጋር መስተጋብር እንዲፈጥሩ እንመክራለን።
- ያልተሳኩ ሙከራዎችን አያስወግዱ. ችላ የተባሉ ፈተናዎች እንኳን በመጨረሻ በእኛ ቧንቧ ውስጥ ይከናወናሉ.
- ከተቻለ ለውጦችዎን ከማድረግዎ በፊት እና በኋላ ኮድዎን ያመልክቱ ፣ ምክንያቱም ጉልህ የሆነ የአፈፃፀም ውድቀት የነባር ተጠቃሚዎችን ጭነቶች ሊሰብር ይችላል።

### ተከታታይ ጠባቂ ቼኮች

የማጠራቀሚያ ፖሊሲዎችን በአገር ውስጥ ለማረጋገጥ `make guards`ን ያሂዱ፡

- በቀጥታ መካድ-ዝርዝር I18NI0000247X በምርት ምንጮች (`norito::json` ይመርጣል)።
- በቀጥታ I18NI0000249X/`serde_json` ጥገኞችን/ማስመጣቶችን ከተፈቀደ ዝርዝር ውስጥ ከልክል።
- ከ`crates/norito` ውጭ የአድሆክ AoS/NCB ረዳቶችን እንደገና እንዳይገቡ መከላከል።

### የማረም ሙከራዎች

<ዝርዝሮች> <ማጠቃለያ> የምዝግብ ማስታወሻ ደረጃን እንዴት መቀየር እንደሚችሉ ለማወቅ ወይም መዝገቦችን ወደ JSON ለመጻፍ ዘርጋ።</ ማጠቃለያ>

ከፈተናዎችዎ አንዱ ካልተሳካ፣ ከፍተኛውን የምዝግብ ማስታወሻ ደረጃ መቀነስ ይፈልጉ ይሆናል። በነባሪ፣ Iroha የ`INFO` ደረጃ መልዕክቶችን ብቻ ይመዘግባል፣ነገር ግን ሁለቱንም የ`DEBUG` እና `TRACE` ደረጃ ምዝግብ ማስታወሻዎችን የማምረት ችሎታ አለው። ይህ ቅንብር በኮድ ላይ ለተመሰረቱ ሙከራዎች የ`LOG_LEVEL` አካባቢ ተለዋዋጭን በመጠቀም ወይም በተዘረጋ አውታረ መረብ ውስጥ ካሉ እኩዮቹ በአንዱ ላይ የ `/configuration` የመጨረሻ ነጥብን በመጠቀም ሊቀየር ይችላል።በ `stdout` ውስጥ የታተሙ ምዝግብ ማስታወሻዎች በቂ ሲሆኑ፣ `json`-የተቀረጹ ምዝግብ ማስታወሻዎችን ወደ ተለየ ፋይል ለማምረት እና ሁለቱንም [node-bunyan](https://www.npmjs.com/package/bunyan) ወይም [ዝገት-ቡንያን](0XNUMX) በመጠቀም መተንተን የበለጠ አመቺ ሆኖ ሊያገኙት ይችላሉ።

ምዝግብ ማስታወሻዎቹን ለማከማቸት እና ከላይ ያሉትን ጥቅሎች በመጠቀም ለመተንተን የ `LOG_FILE_PATH` አካባቢ ተለዋዋጭ ወደ ተገቢ ቦታ ያቀናብሩ።

</details>

### የቶኪዮ ኮንሶል በመጠቀም ማረም

<ዝርዝሮች> <ማጠቃለያ> Irohaን በቶኪዮ ኮንሶል ድጋፍ እንዴት ማጠናቀር እንደሚቻል ለማወቅ ዘርጋ።</summary>

አንዳንድ ጊዜ [tokio-console](https://github.com/tokio-rs/console) በመጠቀም የቶኪዮ ተግባራትን ለመተንተን ለማረም ጠቃሚ ሊሆን ይችላል።

በዚህ አጋጣሚ Iroha ከቶኪዮ ኮንሶል ድጋፍ ጋር ማጠናቀር አለቦት፡

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
```

የቶኪዮ ኮንሶል ወደብ በ`LOG_TOKIO_CONSOLE_ADDR` የውቅረት መለኪያ (ወይም የአካባቢ ተለዋዋጭ) በኩል ሊዋቀር ይችላል።
ቶኪዮ ኮንሶል ለመጠቀም የምዝግብ ማስታወሻ ደረጃን ይፈልጋል `TRACE`፣በውቅረት መለኪያ ወይም በአከባቢው ተለዋዋጭ I18NI0000262X ሊነቃ ይችላል።

Irohaን ከቶኪዮ ኮንሶል ድጋፍ `scripts/test_env.sh` በመጠቀም የማስኬድ ምሳሌ፡

```bash
# 1. Compile Iroha
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
# 2. Run Iroha with TRACE log level
LOG_LEVEL=TRACE ./scripts/test_env.sh setup
# 3. Access Iroha. Peers will be available on ports 5555, 5556, ...
tokio-console http://127.0.0.1:5555
```

</details>

### መገለጫ

<ዝርዝር> <ማጠቃለያ> Iroha እንዴት ፕሮፋይል እንደሚደረግ ለማወቅ ዘርጋ። </ ማጠቃለያ>

አፈጻጸምን ለማመቻቸት Iroha ፕሮፋይል ማድረግ ጠቃሚ ነው።

የመገለጫ ግንባታዎች በአሁኑ ጊዜ የምሽት የመሳሪያ ሰንሰለት ያስፈልጋቸዋል። አንዱን ለማዘጋጀት Irohaን ከ`profiling` መገለጫ እና ባህሪ ጋር `cargo +nightly` በመጠቀም ያሰባስቡ፡

```bash
RUSTFLAGS="-C force-frame-pointers=on" cargo +nightly -Z build-std build --target your-desired-target --profile profiling --features profiling
```

ከዚያ Iroha ይጀምሩ እና የመረጡትን ፕሮፋይል ከ Iroha ፒዲ ጋር አያይዘው።

በአማራጭ Iroha በፕሮፋይለር ድጋፍ እና ፕሮፋይል I18NT0000029X ውስጠ-ዶከር መገንባት ይቻላል።

```bash
docker build -f Dockerfile.glibc --build-arg="PROFILE=profiling" --build-arg='RUSTFLAGS=-C force-frame-pointers=on' --build-arg='FEATURES=profiling' --build-arg='CARGOFLAGS=-Z build-std' -t iroha:profiling .
```

ለምሳሌ. perf በመጠቀም (በሊኑክስ ላይ ብቻ ይገኛል)

```bash
# to capture profile
sudo perf record -g -p <PID>
# to analyze profile
sudo perf report
```

በIroha ፕሮፋይል ወቅት የፈጻሚውን ፕሮፋይል ለመከታተል፣ ፈጻሚው ሳይነጠቅ ምልክቶችን ማጠናቀር አለበት።
በመሮጥ ሊከናወን ይችላል-

```bash
# compile executor without optimizations
cargo run --bin kagami -- ivm build ./path/to/executor --out-file executor.to
```

የመገለጫ ባህሪን በነቃ I18NT0000031X የመጨረሻ ነጥቡን ለ ppprof መገለጫዎች ያጋልጣል፡

```bash
# profile Iroha for 30 seconds and download the profile data
curl host:port/debug/pprof/profile?seconds=30 -o profile.pb
# analyze profile in browser (required installed go)
go tool pprof -web profile.pb
```

</details>

## የቅጥ መመሪያዎች

ለፕሮጀክታችን የኮድ አስተዋጾ ሲያደርጉ እባክዎ እነዚህን መመሪያዎች ይከተሉ፡-

### Git ስታይል መመሪያ

መጽሐፍ: [git መመሪያዎችን ያንብቡ](#git-workflow)

### የዝገት ዘይቤ መመሪያ

<ዝርዝር> <ማጠቃለያ> :መጽሐፍ፡ የኮድ መመሪያዎችን ያንብቡ</ ማጠቃለያ >

- ኮድ ለመቅረጽ `cargo fmt --all` (እትም 2024) ይጠቀሙ።

የኮድ መመሪያዎች፡-

- በሌላ መልኩ ካልተገለጸ በቀር፣ [ዝገት ምርጥ ልምዶችን](https://github.com/mre/idiomatic-rust) ይመልከቱ።
- የ `mod.rs` ዘይቤን ይጠቀሙ። [በራስ የተሰየሙ ሞጁሎች](https://rust-lang.github.io/rust-clippy/master/) እንደ [`trybuild`](https://crates.io/crates/trybuild) ሙከራዎች ካልሆነ በስተቀር የማይንቀሳቀስ ትንታኔን አያልፍም።
- የጎራ-የመጀመሪያ ሞጁሎችን መዋቅር ይጠቀሙ።

  ምሳሌ፡ `constants::logger` አታድርጉ። በምትኩ፣ ተዋረድን ገልብጥ፣ የሚገለገልበትን ዕቃ አስቀድመህ፡ `iroha_logger::constants`።
- [`expect`](https://learning-rust.github.io/docs/unwrap-and-expect/) ከ `unwrap` ይልቅ ግልጽ በሆነ የስህተት መልእክት ወይም ያለመሳሳት ማረጋገጫ ይጠቀሙ።
- ስህተትን በጭራሽ ችላ አትበል። `panic` ካልቻሉ እና ማገገም ካልቻሉ ቢያንስ በምዝግብ ማስታወሻው ውስጥ መመዝገብ አለበት።
- ከ `panic!` ይልቅ `Result` መመለስን እመርጣለሁ።
- ከቡድን ጋር የተዛመደ ተግባር ከቦታ ፣ በተለይም በተገቢው ሞጁሎች ውስጥ።

  ለምሳሌ፣ ከ `struct` ፍቺዎች እና ከዚያም `impl`s ለእያንዳንዱ ግለሰብ መዋቅር ብሎክ ከመያዝ፣ከዚያ `struct` ጋር የሚዛመዱ I18NI0000278X ዎች ከጎኑ ቢኖሩት ይሻላል።
- ከመተግበሩ በፊት ይግለጹ: `use` መግለጫዎች እና ቋሚዎች ከላይ, ከታች የክፍል ሙከራዎች.
- የመጣው ስም አንድ ጊዜ ብቻ ጥቅም ላይ ከዋለ `use` መግለጫዎችን ለማስወገድ ይሞክሩ። ይህ ኮድዎን ወደ ሌላ ፋይል ማዛወር ቀላል ያደርገዋል።
- `clippy` lints ያለአንዳች ልዩነት ዝም አያድርጉ። ካደረክ፣ ምክንያትህን በአስተያየት (ወይም `expect` መልእክት) አስረዳ።
- አንዱ ካለ `#[outer_attribute]` ወደ `#![inner_attribute]` ይምረጡ።
- የእርስዎ ተግባር ማንኛውንም ግብዓቱን ካልቀየረ (እና ሌላ ማንኛውንም ነገር መለወጥ የለበትም) እንደ `#[must_use]` ምልክት ያድርጉበት።
- ከተቻለ `Box<dyn Error>` ያስወግዱ (ጠንካራ መተየብ እንመርጣለን)።
- ተግባርዎ ገተር/አቀናባሪ ከሆነ `#[inline]` ምልክት ያድርጉበት።
- የእርስዎ ተግባር ገንቢ ከሆነ (ማለትም ከግቤት መለኪያዎች አዲስ እሴት እየፈጠረ እና `default()` ይደውሉ) `#[inline]` ምልክት ያድርጉበት።
- ኮድዎን ከኮንክሪት መረጃ መዋቅሮች ጋር ከማያያዝ ይቆጠቡ; `rustc` አንድ `Vec<InstructionExpr>` ወደ `impl IntoIterator<Item = InstructionExpr>` እና በተገላቢጦሽ ሲፈልግ ለመቀየር በቂ ብልህ ነው።

የስም መመሪያዎች፡-
- በ *ህዝባዊ* መዋቅር፣ ተለዋዋጭ፣ ዘዴ፣ ባህሪ፣ ቋሚ እና ሞጁል ስሞች ውስጥ ሙሉ ቃላትን ብቻ ተጠቀም። ሆኖም፣ አህጽሮተ ቃላት የሚፈቀዱት ከሆነ፡-
  - ስሙ አካባቢያዊ ነው (ለምሳሌ የመዝጊያ ክርክሮች)።
  - ስሙ በሩስት ኮንቬንሽን (ለምሳሌ `len`፣ `typ`) አህጽሮታል።
  - ስሙ ተቀባይነት ያለው ምህጻረ ቃል ነው (ለምሳሌ `tx`, `wsv` ወዘተ); [የፕሮጀክት መዝገበ ቃላት](https://docs.iroha.tech/reference/glossary.html) ለቀኖናዊ አህጽሮተ ቃላት ይመልከቱ።
  - ሙሉው ስም በአካባቢያዊ ተለዋዋጭ (ለምሳሌ `msg <- message`) ይጨልም ነበር።
  - ሙሉ ስሙ በውስጡ ከ5-6 ቃላት በላይ (ለምሳሌ `WorldStateViewReceiverTrait -> WSVRecvTrait`) ኮዱን አስቸጋሪ ያደርገዋል።
- የስያሜ ስምምነቶችን ከቀየሩ አዲሱ የመረጡት ስም ከዚህ በፊት ከነበረው የበለጠ _ይበልጥ ግልጽ መሆኑን ያረጋግጡ።

የአስተያየት መመሪያዎች፡-
- የሰነድ ያልሆኑ አስተያየቶችን በሚጽፉበት ጊዜ, ተግባርዎ * ምን እንደሚሰራ ከመግለጽ ይልቅ አንድን ነገር በተለየ መንገድ ለምን እንደሚሰራ ለማስረዳት ይሞክሩ. ይህ እርስዎን እና የገምጋሚውን ጊዜ ይቆጥባል።
- የፈጠርከውን ችግር እስካልጠቀስክ ድረስ `TODO` ማርከሮችን በኮድ ውስጥ መተው ትችላለህ። ጉዳይ አለመፍጠር ማለት አይዋሃድም ማለት ነው።

የተጣበቁ ጥገኞችን እንጠቀማለን. ለማተም የሚከተሉትን መመሪያዎች ይከተሉ፡-

- ስራዎ በአንድ የተወሰነ ሳጥን ላይ የሚመረኮዝ ከሆነ [`cargo tree`](https://doc.rust-lang.org/cargo/commands/cargo-tree.html) (`bat` ወይም `bat` ወይም `grep` ተጠቀም) አልተጫነም እንደሆነ ይመልከቱ እና ያንን ስሪት ለመጠቀም ይሞክሩ፣ የቅርብ ጊዜው ስሪት።
- ሙሉውን ስሪት "X.Y.Z" በ `Cargo.toml` ይጠቀሙ.
- በተለየ PR ውስጥ የስሪት እብጠቶችን ያቅርቡ።

</details>

### የሰነድ ዘይቤ መመሪያ

<ዝርዝር> <ማጠቃለያ> :መጽሐፍ፡ የሰነድ መመሪያዎችን ያንብቡ</summary>


- የ [`Rust Docs`](https://doc.rust-lang.org/cargo/commands/cargo-doc.html) ቅርጸት ይጠቀሙ።
- የነጠላ መስመር አስተያየት አገባብ ይምረጡ። `///` ከመስመር ሞጁሎች በላይ እና `//!` ፋይል ላይ ለተመሰረቱ ሞጁሎች ተጠቀም።
- ወደ መዋቅር/ሞዱል/ተግባር ሰነዶች ማገናኘት ከቻሉ ያድርጉት።
- የአጠቃቀም ምሳሌ ማቅረብ ከቻሉ ያድርጉት። ይህ [እንዲሁም ፈተና ነው](https://doc.rust-lang.org/rustdoc/documentation-tests.html)።
- አንድ ተግባር ሊሳሳት ወይም ሊደነግጥ የሚችል ከሆነ፣ የሞዳል ግሦችን ያስወግዱ። ምሳሌ፡- ከ`Can possibly fail, if disk IO happens to fail` ይልቅ `Fails if disk IO fails`።
- አንድ ተግባር ከአንድ በላይ በሆነ ምክንያት ሊሳሳት ወይም ሊደነግጥ ከቻለ፣ ከተገቢው `Error` ተለዋጮች (ካለ) ነጥበ ምልክት የተደረገባቸው የውድቀት ሁኔታዎች ዝርዝር ይጠቀሙ።
- ተግባራት * ነገሮችን ያደርጋሉ። የግድ ስሜትን ተጠቀም።
- መዋቅሮች * ነገሮች ናቸው. ወደ ነጥቡ ግባ። ለምሳሌ `Log level for reloading from the environment` ከ `This struct encapsulates the idea of logging levels, and is used for reloading from the environment` የተሻለ ነው።
- መዋቅሮች ሜዳዎች አሏቸው, እነሱም * ነገሮች ናቸው.
- ሞጁሎች * ነገሮችን ይይዛሉ, እና ያንን እናውቃለን. ወደ ነጥቡ ግባ። ምሳሌ፡ ከ`Module which contains logger-related logic` ይልቅ `Logger-related traits.` ይጠቀሙ።


</details>

## ተገናኝ

የእኛ የማህበረሰቡ አባላት በሚከተለው ላይ ንቁ ተሳትፎ ያደርጋሉ፡-

| አገልግሎት | አገናኝ |
|--------------------|
| StackOverflow | https://stackoverflow.com/questions/tagged/hyperledger-iroha |
| የደብዳቤ መላኪያ ዝርዝር | https://lists.lfdecentralizedtrust.org/g/iroha |
| ቴሌግራም | https://t.me/hyperledgeriroha |
| አለመግባባት | https://discord.com/channels/905194001349627914/905205848547155968 |

---