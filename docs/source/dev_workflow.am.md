---
lang: am
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-19T07:43:34.047491+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ወኪሎች ልማት የስራ ፍሰት

ይህ የሩጫ መጽሐፍ የአስተዋጽዖ አበርካቾችን ጥበቃዎች ከኤጀንቶች የመንገድ ካርታ ያጠናክራል።
አዲስ ጥገናዎች ተመሳሳይ ነባሪ በሮች ይከተላሉ።

## Quickstart ኢላማዎች

- ለማስፈጸም `make dev-workflow` (በ`scripts/dev_workflow.sh` አካባቢ መጠቅለያ) ያሂዱ፡
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3. `cargo build --workspace --locked`
  4. `cargo test --workspace --locked`
  5. `swift test` ከ `IrohaSwift/`
- `cargo test --workspace` ረጅም (ብዙውን ጊዜ ሰዓታት) ነው. ለፈጣን ድግግሞሽ ፣
  `scripts/dev_workflow.sh --skip-tests` ወይም `--skip-swift` ይጠቀሙ፣ ከዚያ ሙሉውን ያሂዱ
  ከማጓጓዙ በፊት ቅደም ተከተል.
- `cargo test --workspace` በግንባታ ማውጫ መቆለፊያዎች ላይ ከቆመ፣ በድጋሚ ያሂዱ
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (ወይም አዘጋጅ
  `CARGO_TARGET_DIR` ወደ ገለልተኛ መንገድ) ክርክርን ለማስወገድ።
- ሁሉም የጭነት ደረጃዎች የማጠራቀሚያ ፖሊሲን ለማክበር `--locked` ይጠቀማሉ
  `Cargo.lock` ያልተነካ. ያሉትን ሳጥኖች ከመጨመር ይልቅ ማራዘምን ይምረጡ
  አዲስ የስራ ቦታ አባላት; አዲስ ሣጥን ከማስተዋወቅዎ በፊት ፈቃድ ይጠይቁ።

## መከላከያ መንገዶች- `make check-agents-guardrails` (ወይም `ci/check_agents_guardrails.sh`) ካልተሳካ
  ቅርንጫፍ `Cargo.lock`ን ያስተካክላል፣ አዲስ የስራ ቦታ አባላትን ያስተዋውቃል ወይም አዲስ ይጨምራል
  ጥገኝነቶች. ስክሪፕቱ የሚሠራውን ዛፍ እና `HEAD` ጋር ያነጻጽራል።
  `origin/main` በነባሪ; መሰረቱን ለመሻር `AGENTS_BASE_REF=<ref>` ያዘጋጁ።
- `make check-dependency-discipline` (ወይም `ci/check_dependency_discipline.sh`)
  ከመሠረቱ ላይ `Cargo.toml` ጥገኞችን ይለያል እና በአዳዲስ ሳጥኖች ላይ አልተሳካም; አዘጋጅ
  ሆን ተብሎ እውቅና ለመስጠት `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>`
  ተጨማሪዎች.
- `make check-missing-docs` (ወይም `ci/check_missing_docs_guard.sh`) አዲስ ያግዳል
  `#[allow(missing_docs)]` ግቤቶች፣ ባንዲራዎች የተነኩ ሳጥኖች (በአቅራቢያው `Cargo.toml`)
  የማን `src/lib.rs`/`src/main.rs` የሣጥን ደረጃ `//!` ሰነዶች የጎደለው እና አዲስ ውድቅ ያደርጋል
  ከመሠረታዊ ማጣቀሻ አንጻር ያለ `///` ሰነዶች ያለ የህዝብ እቃዎች; አዘጋጅ
  `MISSING_DOCS_GUARD_ALLOW=1` ከገምጋሚ ማረጋገጫ ጋር ብቻ። ጠባቂውም
  `docs/source/agents/missing_docs_inventory.{json,md}` ትኩስ መሆኑን ያረጋግጣል;
  በ `python3 scripts/inventory_missing_docs.py` እንደገና ማመንጨት።
- `make check-tests-guard` (ወይም `ci/check_tests_guard.sh`) ባንዲራዎች የማን
  የተለወጠ የዝገት ተግባራት የክፍል-ሙከራ ማስረጃ የላቸውም። የጥበቃ ካርታዎች መስመሮችን ቀይረዋል
  ወደ ተግባር፣ የሣጥን ፈተናዎች በዲፍ ውስጥ ከተቀየሩ ያልፋል፣ እና በሌላ መንገድ ይቃኛል።
  ነባር የሙከራ ፋይሎች ለተግባር ጥሪዎች ቅድመ-ነባር ሽፋን
  ይቆጥራል; ምንም ተዛማጅ ሙከራዎች ሳይኖሩባቸው ሳጥኖች አይሳኩም። `TEST_GUARD_ALLOW=1` አዘጋጅ
  ለውጦች በእውነት ፈተና-ገለልተኛ ሲሆኑ እና ገምጋሚው ሲስማማ ብቻ ነው።
- `make check-docs-tests-metrics` (ወይም `ci/check_docs_tests_metrics_guard.sh`)
  ወሳኝ ደረጃዎች ከሰነድ ጎን ለጎን የሚንቀሳቀሱትን የፍኖተ ካርታ ፖሊሲ ያስፈጽማል፣
  ሙከራዎች, እና መለኪያዎች / ዳሽቦርዶች. `roadmap.md` ሲቀየር
  `AGENTS_BASE_REF`፣ ጠባቂው ቢያንስ አንድ የዶክመንት ለውጥ፣ አንድ የሙከራ ለውጥ፣
  እና አንድ ሜትሪክስ/ቴሌሜትሪ/ዳሽቦርድ ለውጥ። `DOC_TEST_METRIC_GUARD_ALLOW=1` አዘጋጅ
  ከገምጋሚው ፈቃድ ጋር ብቻ።
- `make check-todo-guard` (ወይም `ci/check_todo_guard.sh`) የ TODO ማርከሮች አይሳካም
  ሰነዶች/የሙከራ ለውጦችን ሳያካትት ይጠፋል። ሽፋን ያክሉ ወይም ያዘምኑ
  TODO ሲፈታ፣ ወይም ሆን ተብሎ ለሚወገዱ `TODO_GUARD_ALLOW=1` ያዘጋጁ።
- `make check-std-only` (ወይም `ci/check_std_only.sh`) ብሎኮች `no_std`/`wasm32`
  cfgs ስለዚህ የስራ ቦታው `std`-ብቻ ይቆያል። `STD_ONLY_GUARD_ALLOW=1` ብቻ አዘጋጅ
  የተፈቀደ የ CI ሙከራዎች.
- `make check-status-sync` (ወይም `ci/check_status_sync.sh`) የመንገድ ካርታውን ክፍት ያደርገዋል
  ከተጠናቀቁ ዕቃዎች ነፃ ክፍል እና `roadmap.md`/`status.md` ያስፈልገዋል
  ፕላን/ሁኔታ አንድ ላይ እንዲመጣጠን አንድ ላይ መለወጥ; አዘጋጅ
  `STATUS_SYNC_ALLOW_UNPAIRED=1` ለ ብርቅዬ ሁኔታ-ብቻ የትየባ ጥገናዎች በኋላ
  `AGENTS_BASE_REF` መሰካት.
- `make check-proc-macro-ui` (ወይም `ci/check_proc_macro_ui.sh`) trybuild ይሰራል
  የዩአይ ስብስቦች ለዲሪቭ/ፕሮክ-ማክሮ ሳጥኖች። ፕሮክ-ማክሮዎችን ሲነኩ ያሂዱት
  የ `.stderr` መመርመሪያዎች የተረጋጋ እና የሚያስደነግጡ የUI regressions ይያዙ; አዘጋጅ
  `PROC_MACRO_UI_CRATES="crate1 crate2"` በተወሰኑ ሳጥኖች ላይ ለማተኮር።
- `make check-env-config-surface` (ወይም `ci/check_env_config_surface.sh`) እንደገና ይገነባል
  የኢንቭ-መቀያየር ክምችት (`docs/source/agents/env_var_inventory.{json,md}`)፣
  ያረጀ ከሆነ አይሳካም፣ ** እና ** አዲስ ፕሮዳክሽን env shims ሲመጣ ይከሽፋል
  ከ `AGENTS_BASE_REF` ጋር አንጻራዊ (በራስ-የተገኘ፤ አስፈላጊ ሆኖ ሲገኝ በግልጽ ተቀምጧል)።
  የ env ፍለጋዎችን በ በኩል ካከሉ/ ካስወገዱ በኋላ መከታተያውን ያድሱ
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  `ENV_CONFIG_GUARD_ALLOW=1` ሆን ተብሎ የኢንቪ ቁልፎችን ከሰነዱ በኋላ ብቻ ይጠቀሙበስደት መከታተያ ውስጥ.
- `make check-serde-guard` (ወይም `ci/check_serde_guard.sh`) ሰርዴውን ያድሳል
  የአጠቃቀም ክምችት (`docs/source/norito_json_inventory.{json,md}`) ወደ ሙቀት
  መገኛ ቦታ፣ የተፈፀመው ክምችት የቆየ ከሆነ አይሳካም እና አዲስ ማንኛውንም ውድቅ ያደርጋል
  ፕሮዳክሽን `serde`/`serde_json` ከ `AGENTS_BASE_REF` አንጻራዊ ይመታል። አዘጋጅ
  `SERDE_GUARD_ALLOW=1` የፍልሰት እቅድ ካስገባ በኋላ ለ CI ሙከራዎች ብቻ።
- `make guards` የNorito ተከታታይ ፖሊሲን ያስፈጽማል፡ አዲስ ይክዳል
  `serde`/`serde_json` አጠቃቀም፣ ad-hoc AoS ረዳቶች፣ እና SCALE ጥገኞች ውጭ
  የ Norito ወንበሮች (`scripts/deny_serde_json.sh`፣
  `scripts/check_no_direct_serde.sh`፣ `scripts/deny_handrolled_aos.sh`፣
  `scripts/check_no_scale.sh`)።
- **የፕሮክ-ማክሮ ዩአይ ፖሊሲ፡** እያንዳንዱ የፕሮክ-ማክሮ ሳጥን `trybuild` መላክ አለበት።
  መታጠቂያ (`tests/ui.rs` ከ pass/fail globs ጋር) ከ `trybuild-tests` ጀርባ
  ባህሪ. የደስተኛ መንገድ ናሙናዎችን በ `tests/ui/pass` ስር ያስቀምጡ፣ ውድቅ የተደረጉ ጉዳዮችን ስር
  `tests/ui/fail` ከቁርጠኝነት `.stderr` ውጤቶች ጋር፣ እና ምርመራዎችን ያቆዩ
  የማይደናገጥ እና የተረጋጋ. መገልገያዎችን በ ጋር ያድሱ
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (በአማራጭ በ
  `CARGO_TARGET_DIR=target-codex` ያሉትን ግንባታዎች እንዳይዘጉ) እና
  በሽፋን ግንባታዎች ላይ መታመንን ያስወግዱ (`cfg(not(coverage))` ጠባቂዎች ይጠበቃሉ).
  የሁለትዮሽ መግቢያ ነጥብ ለማይለቁ ማክሮዎች፣ ተመራጭ
  ስህተቶችን እንዲያተኩር ለማድረግ `// compile-flags: --crate-type lib` በመሳሪያዎች ውስጥ። አክል
  ምርመራዎች በሚቀየሩበት ጊዜ አዳዲስ አሉታዊ ጉዳዮች።
- CI የጥበቃ ሀዲድ ስክሪፕቶችን በ`.github/workflows/agents-guardrails.yml` በኩል ይሰራል
  ስለዚህ ፖሊሲዎቹ ሲጣሱ የመሳብ ጥያቄዎች በፍጥነት አይሳኩም።
- የናሙና git መንጠቆ (`hooks/pre-commit.sample`) የጥበቃ ሀዲድ ፣ ጥገኝነት ፣
  የጎደሉ ሰነዶች፣ std-only፣ env-config እና የሁኔታ ማመሳሰል ስክሪፕቶች አስተዋጽዖ አበርካቾች
  ከ CI በፊት የፖሊሲ ጥሰቶችን ይያዙ. ለማንኛውም ሆን ተብሎ የTODO የዳቦ ፍርፋሪ ያስቀምጡ
  ትላልቅ ለውጦችን በጸጥታ ከማስተላለፍ ይልቅ ክትትል ማድረግ።