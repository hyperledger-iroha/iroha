---
lang: ba
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-19T07:43:34.047491+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# АГЕНТТАР Үҫеш эш ағымы

Был runbook нығытыусы ҡоршауҙарҙы нығыта AGENTS юл картаһы шулай
яңы патчтар шул уҡ ҡапҡалар буйынса үтә.

## Quickstart маҡсаттары

- Run `make dev-workflow` (`scripts/dev_workflow.sh` тирәһендә урау) башҡарыу өсөн:
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3. `cargo build --workspace --locked`
  4. `cargo test --workspace --locked`
  5. `swift test` `IrohaSwift/` .
- `cargo test --workspace` оҙайлы ваҡытҡа (йыш сәғәт). Тиҙ итерацион өсөн,
  ҡулланыу `scripts/dev_workflow.sh --skip-tests` йәки `--skip-swift`, һуңынан тулы йүгерә
  эҙмә-эҙлеклелек ташыу алдынан.
- Әгәр `cargo test --workspace` ларектар төҙөү каталогы йоҙаҡтары, перемость менән .
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (йәки комплект
  `CARGO_TARGET_DIR` айырым юлға тиклем) бәхәстән һаҡланыу өсөн.
- Бөтә йөк аҙымдары ла `--locked` ҡулланып, һаҡлау сәйәсәтен хөрмәт итә.
  `Cargo.lock` тейелмәгән. Өҫтәмә булған йәшниктәрҙе өҫтәү түгел, ә өҫтөнлөк
  яңы эш урыны ағзалары; яңы йәшник индереү алдынан раҫлау эҙләгеҙ.

## Гвардиялар- `make check-agents-guardrails` (йәки `ci/check_agents_guardrails.sh`) уңышһыҙлыҡҡа осрай, әгәр ҙә
  филиалы `Cargo.lock` үҙгәртә, яңы эш урыны ағзалары менән таныштыра, йәки яңы өҫтәй
  бәйлелектәре. Сценарий эш ағасы һәм `HEAD` ҡаршы сағыштырыла.
  `origin/main` ғәҙәттәгесә; базаны өҫтөн ҡуйыу өсөн `AGENTS_BASE_REF=<ref>` ҡуйҙы.
- `make check-dependency-discipline` (йәки `cargo clippy --workspace --all-targets --locked -- -D warnings`)
  диффтар `Cargo.toml` облигацияларға ҡаршы базаға ҡаршы һәм яңы йәшниктәрҙә уңышһыҙлыҡҡа осрай; йыйылма
  `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>` ниәтләп таныу өсөн
  өҫтәмәләр.
- `make check-missing-docs` (йәки `ci/check_missing_docs_guard.sh` X) яңы блоктар
  `#[allow(missing_docs)]` яҙмалары, флагтар йәшниккә ҡағыла (иң яҡын `Cargo.toml`)
  `src/lib.rs`/`src/main.rs` йәшник кимәлендә `//!` docs етешмәй, һәм яңы кире ҡаға.
  йәмәғәт әйберҙәре `///` docs булмаған база ref; йыйылма
  `MISSING_DOCS_GUARD_ALLOW=1` тик рецензент раҫлау менән генә. Һаҡсы шулай уҡ
  `docs/source/agents/missing_docs_inventory.{json,md}` яңы булыуын раҫлай;
  `python3 scripts/inventory_missing_docs.py` менән регенерациялана.
- `make check-tests-guard` X (йәки `ci/check_tests_guard.sh`) флагтар йәшниктәре
  үҙгәртте Rust функциялары берәмек-һынау дәлилдәре юҡ. Һаҡсы карталары һыҙыҡтарын алмаштырҙы .
  функцияларға, әгәр йәшник һынауҙары үҙгәргән дифф, һәм башҡа сканерлау
  ғәмәлдәге һынау файлдары өсөн тап килтереп функцияһы шылтыратыуҙар шулай алдан булған ҡаплау
  һанала; йәшниктәр бер ниндәй ҙә тап килгән һынауҙарһыҙ уңышһыҙлыҡҡа осрай. `TEST_GUARD_ALLOW=1` комплекты
  тик үҙгәрештәр ысынлап та һынау-нейтраль һәм рецензент риза булғанда ғына.
- `make check-docs-tests-metrics` (йәки `ci/check_docs_tests_metrics_guard.sh`)
  юл картаһы сәйәсәтен үтәй, уларҙы документация менән бер рәттән күрһәтә,
  һынауҙар, һәм метрика/приборҙар таҡтаһы. Ҡасан `roadmap.md` үҙгәрә, 2012 йыл.
  `AGENTS_BASE_REF`, һаҡсы көтә, кәмендә бер doc үҙгәреш, бер һынау үҙгәрештәре,
  һәм бер метрика/телеметрия/приборҙар таҡтаһы үҙгәреше. `DOC_TEST_METRIC_GUARD_ALLOW=1` комплекты
  тик рецензент раҫлау менән генә.
- `make check-todo-guard` (йәки `ci/check_todo_guard.sh`) ТОДО маркерҙары ҡасан уңышһыҙлыҡҡа осрай
  юғала, оҙатыусы doc/тестар үҙгәрмәй. Өҫтәү йәки яңыртыу ҡаплау
  ҡасан хәл итеү TODO, йәки `TODO_GUARD_ALLOW=1` өсөн ниәтләп сығарыу өсөн ҡуйылған.
- `make check-std-only` (йәки `ci/check_std_only.sh`) блоктары `no_std`/`scripts/dev_workflow.sh`.
  cfgs шулай эш урыны ҡала `std`-тик. `STD_ONLY_GUARD_ALLOW=1` комплекты өсөн генә .
  санкцияланған CI тәжрибәләре.
- `make check-status-sync` (йәки `ci/check_status_sync.sh`) юл картаһын асыҡ тота.
  18NI000000058X/`status.md` X.
  үҙгәртергә бергә шулай план/статус тура килтереп ҡалырға; йыйылма
  `STATUS_SYNC_ALLOW_UNPAIRED=1` тик һирәк осрай торған статус өсөн генә опечаткалар өсөн 2012 йылдан һуң төҙәтеүҙәр .
  приключать `AGENTS_BASE_REF`.
- `make check-proc-macro-ui` (йәки `ci/check_proc_macro_ui.sh`) трибульд
  UI люкс өсөн алынған/прок-макро йәшниктәр. Йүгереп, ҡасан ҡағыла прок-макрос .
  `.stderr` диагностикаһы тотороҡло һәм тотоу паника UI регрессиялары; йыйылма
  `PROC_MACRO_UI_CRATES="crate1 crate2"` аныҡ йәшниктәргә иғтибар йүнәлтергә.
- `make check-env-config-surface` (йәки `ci/check_env_config_surface.sh`) яңынан төҙөлә
  env-toggle инвентаризацияһы (`docs/source/agents/env_var_inventory.{json,md}` XX).
  уңышһыҙлыҡҡа осрай, әгәр ул иҫке, ** һәм** яңы етештереү env shims барлыҡҡа килгәндә уңышһыҙлыҡҡа осрай
  `AGENTS_BASE_REF` (авто-асыҡланған; кәрәк саҡта асыҡтан-асыҡ ҡуйылған).
  Яңыртыу трекер өҫтәгәндән һуң/юйыу env эҙләүҙәр аша .
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  ҡулланыу `ENV_CONFIG_GUARD_ALLOW=1` тик документлаштырыуҙан һуң ниәтләп env ручкалармиграция трекерында.
- `make check-serde-guard` (йәки `ci/check_serde_guard.sh`) серде регенерациялай
  ҡулланыу инвентаризацияһы (`docs/source/norito_json_inventory.{json,md}`) температураға
  урынлашыу, уңышһыҙлыҡҡа осрай, әгәр ҙә ҡылған инвентарь иҫке, һәм теләһә ниндәй яңы кире ҡаға
  `serde`/`serde_json` етештереү `AGENTS_BASE_REF`-ҡа ҡарағанда. Йыйылма
  `SERDE_GUARD_ALLOW=1` тик CI тәжрибәләре өсөн генә миграция планы тапшырғандан һуң.
- `make guards` Norito сериялаштырыу сәйәсәтен үтәй: ул яңы инҡар итә.
  `serde`/`serde_json` ҡулланыу, махсус AoS ярҙамсылары һәм SCALE бәйлелектәре ситтә
  Norito эскәмйәләре (`scripts/deny_serde_json.sh`,
  `scripts/check_no_direct_serde.sh`, `scripts/deny_handrolled_aos.sh`,
  `scripts/check_no_scale.sh`).
- **Прок-макрослы UI сәйәсәте:** һәр прок-макро йәшник `trybuild`E йөк ташырға тейеш.
  йүгән (`tests/ui.rs` менән үткәреү/уңышһыҙлыҡҡа осраған глобтар) `trybuild-tests`-тән артында
  сифат. Урын бәхетле-юл өлгөләре буйынса `tests/ui/pass`, кире ҡағыу осраҡтары буйынса .
  `tests/ui/fail` менән йөкмәтелгән `.stderr` сығыштары, һәм диагностика һаҡлау
  паникаһыҙ һәм тотороҡло. Яңыртыу ҡоролмалары менән
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (факультатив рәүештә.
  `CARGO_TARGET_DIR=target-codex` clobbering ҡотолоу өсөн булған төҙөүҙәр) һәм
  ҡотолоу өсөн таянып ҡаплау төҙөү (`cfg(not(coverage))` һаҡсылары көтөлә).
  Макрос өсөн, улар бинар инеү нөктәһен сығармай, өҫтөнлөк бирә
  `cargo clippy --workspace --all-targets --locked -- -D warnings` X ҡорамалдарҙа хаталарҙы йүнәлтергә. Ҡушырға
  диагностика үҙгәргән һайын яңы кире осраҡтар.
- CI `cargo build --workspace --locked` аша ҡоршау сценарийҙарын етәкләй.
  тимәк, тартыу запростар тиҙ уңышһыҙлыҡҡа осрай, ҡасан сәйәсәт боҙолған.
- Өлгө git ҡармаҡ (`hooks/pre-commit.sample`) йүгерә ҡоршау, бәйлелек,
  юғалған документтар, std-тик, env-config, һәм статус-синхель сценарийҙары шулай өлөш индереүселәр
  тотоу сәйәсәте боҙоуҙар алдынан CI. ТОДО икмәк өҙөктәрен теләһә ниндәй ниәтле тотоу
  ҙур үҙгәрештәрҙе өнһөҙ кисектереп тормай.