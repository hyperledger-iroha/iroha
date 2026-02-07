---
lang: kk
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-19T07:43:34.047491+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# AGENTS әзірлеу жұмыс процесі

Бұл Runbook AGENTS жол картасынан үлес қосушы қоршауларын біріктіреді
жаңа патчтар бірдей әдепкі қақпаларға сәйкес келеді.

## Жылдам бастау мақсаттары

- Орындау үшін `make dev-workflow` (`scripts/dev_workflow.sh` айналасындағы қаптама) іске қосыңыз:
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3. `cargo build --workspace --locked`
  4. `cargo test --workspace --locked`
  5. `swift test`, `IrohaSwift/`
- `cargo test --workspace` ұзақ жұмыс істейді (көбінесе сағат). Жылдам қайталау үшін,
  `scripts/dev_workflow.sh --skip-tests` немесе `--skip-swift` пайдаланыңыз, содан кейін толық іске қосыңыз
  жөнелту алдындағы реттілік.
- Егер `cargo test --workspace` құрастыру каталогының құлыптарында тоқтаса, келесімен қайта іске қосыңыз
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (немесе орнатылған
  `CARGO_TARGET_DIR` оқшауланған жолға) қайшылықты болдырмау үшін.
- Барлық жүк қадамдары репозиторий саясатын сақтау үшін `--locked` пайдаланады.
  `Cargo.lock` қол тимеген. Қолданыстағы жәшіктерді қосудан гөрі кеңейтуді жөн көріңіз
  жаңа жұмыс кеңістігінің мүшелері; жаңа қорапты енгізбес бұрын мақұлдауды сұраңыз.

## Қорғаулар- `make check-agents-guardrails` (немесе `ci/check_agents_guardrails.sh`), егер
  филиал `Cargo.lock` өзгертеді, жаңа жұмыс кеңістігі мүшелерін енгізеді немесе жаңасын қосады
  тәуелділіктер. Сценарий жұмыс ағашын және `HEAD`-ті салыстырады
  Әдепкі бойынша `origin/main`; негізді қайта анықтау үшін `AGENTS_BASE_REF=<ref>` орнатыңыз.
- `make check-dependency-discipline` (немесе `ci/check_dependency_discipline.sh`)
  `Cargo.toml` тәуелділіктерін негізге қарсы ажыратады және жаңа жәшіктерде сәтсіздікке ұшырайды; орнату
  `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>` әдейі мойындау үшін
  толықтырулар.
- `make check-missing-docs` (немесе `ci/check_missing_docs_guard.sh`) жаңа блоктарды
  `#[allow(missing_docs)]` жазбалары, жалаушалар тиген жәшіктер (ең жақын `Cargo.toml`)
  `src/lib.rs`/`src/main.rs` қорап деңгейіндегі `//!` құжаттары жоқ және жаңадан бас тартады
  негізгі сілтемеге қатысты `///` құжаттары жоқ жалпыға ортақ элементтер; орнату
  `MISSING_DOCS_GUARD_ALLOW=1` тек шолушы мақұлдауымен. Күзетші де
  `docs/source/agents/missing_docs_inventory.{json,md}` жаңа екенін тексереді;
  `python3 scripts/inventory_missing_docs.py` көмегімен қалпына келтіріңіз.
- `make check-tests-guard` (немесе `ci/check_tests_guard.sh`) жалаушалары
  өзгерді Rust функцияларында бірлік сынақ дәлелдері жоқ. Күзет карталары сызықтарды өзгертті
  функцияларға, егер дифференциалда жәшік сынақтары өзгерсе, өтеді және басқаша сканерлейді
  сәйкес функция шақыруларына арналған бар сынақ файлдары, сондықтан алдын ала бар қамту
  санау; сәйкес сынақтары жоқ жәшіктер сәтсіз болады. `TEST_GUARD_ALLOW=1` орнатыңыз
  өзгертулер шын мәнінде сынақтан бейтарап болғанда және шолушы келіскенде ғана.
- `make check-docs-tests-metrics` (немесе `ci/check_docs_tests_metrics_guard.sh`)
  кезеңдері құжаттамамен қатар жүретін жол картасы саясатын жүзеге асырады,
  сынақтар және көрсеткіштер/бақылау тақталары. `roadmap.md` қатысты өзгерген кезде
  `AGENTS_BASE_REF`, күзетші кем дегенде бір құжатты, бір сынақты өзгертуді күтеді,
  және бір метрика/телеметрия/бақылау тақтасын өзгерту. `DOC_TEST_METRIC_GUARD_ALLOW=1` орнатыңыз
  тексерушінің рұқсатымен ғана.
- `make check-todo-guard` (немесе `ci/check_todo_guard.sh`) TODO маркерлері сәтсіз аяқталады
  ілеспе құжаттар/сынақ өзгерістерінсіз жоғалады. Қамту аймағын қосыңыз немесе жаңартыңыз
  TODO шешу кезінде немесе әдейі жою үшін `TODO_GUARD_ALLOW=1` орнатыңыз.
- `make check-std-only` (немесе `ci/check_std_only.sh`) блоктары `no_std`/`wasm32`
  cfg файлында жұмыс кеңістігі `std` ғана қалады. `STD_ONLY_GUARD_ALLOW=1` үшін ғана орнатыңыз
  рұқсат етілген CI эксперименттері.
- `make check-status-sync` (немесе `ci/check_status_sync.sh`) жол картасын ашық ұстайды
  бөлімде аяқталған элементтер жоқ және `roadmap.md`/`status.md` талап етеді
  жоспар/мәртебе біркелкі болу үшін бірге өзгерту; орнату
  `STATUS_SYNC_ALLOW_UNPAIRED=1` тек сирек күйдегі қателерді түзетуге арналған
  бекіту `AGENTS_BASE_REF`.
- `make check-proc-macro-ui` (немесе `ci/check_proc_macro_ui.sh`) trybuild іске қосады
  Туынды/прок-макро жәшіктерге арналған UI жиынтықтары. Прок-макростарды түрткен кезде оны іске қосыңыз
  `.stderr` диагностикасын тұрақты ұстау және UI регрессияларының дүрбелеңін ұстау; орнату
  Арнайы жәшіктерге назар аудару үшін `PROC_MACRO_UI_CRATES="crate1 crate2"`.
- `make check-env-config-surface` (немесе `ci/check_env_config_surface.sh`) қайта құрастырады
  env-қосқыштар тізімі (`docs/source/agents/env_var_inventory.{json,md}`),
  егер ол ескірген болса, сәтсіз аяқталады, **және** жаңа өндіріс ортасының жиектері пайда болған кезде сәтсіздікке ұшырайды
  `AGENTS_BASE_REF` қатысты (автоматты түрде анықталады; қажет болғанда нақты орнатылады).
  арқылы env іздеулерін қосқаннан/жоюдан кейін трекерді жаңартыңыз
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  `ENV_CONFIG_GUARD_ALLOW=1` әдейі env тұтқаларын құжаттағаннан кейін ғана пайдаланыңызкөші-қон трекерінде.
- `make check-serde-guard` (немесе `ci/check_serde_guard.sh`) серді қалпына келтіреді
  пайдалану тізімдемесі (`docs/source/norito_json_inventory.{json,md}`) темп
  орны, егер бекітілген түгендеу ескірген болса, сәтсіз аяқталады және кез келген жаңадан бас тартады
  `serde`/`serde_json` өнімі `AGENTS_BASE_REF` қатысты. Орнату
  `SERDE_GUARD_ALLOW=1` тек тасымалдау жоспарын тапсырғаннан кейін CI эксперименттері үшін.
- `make guards` Norito сериялау саясатын жүзеге асырады: ол жаңадан бас тартады
  `serde`/`serde_json` пайдалану, арнайы AoS көмекшілері және сырттағы SCALE тәуелділіктері
  Norito орындықтар (`scripts/deny_serde_json.sh`,
  `scripts/check_no_direct_serde.sh`, `scripts/deny_handrolled_aos.sh`,
  `scripts/check_no_scale.sh`).
- **Proc-macro UI саясаты:** әрбір proc-makro жәшігі `trybuild` жіберуі керек
  `trybuild-tests` артындағы жіп (`tests/ui.rs` өту/сәтсіз глобустары бар)
  ерекшелігі. Бақытты жол үлгілерін `tests/ui/pass` астына, бас тарту жағдайларын астына қойыңыз
  `tests/ui/fail` бекітілген `.stderr` шығыстары бар және диагностиканы сақтаңыз
  дүрбелеңсіз және тұрақты. Арматураларды жаңартыңыз
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (міндетті емес
  `CARGO_TARGET_DIR=target-codex` бар құрылымдарды бұзбау үшін) және
  қамту құрылымына сенбеңіз (`cfg(not(coverage))` күзетшілері күтіледі).
  Екілік енгізу нүктесін шығармайтын макростар үшін артықшылық беріңіз
  Қателерге назар аудару үшін `// compile-flags: --crate-type lib` құрылғыларда. қосу
  диагностика өзгерген сайын жаңа жағымсыз жағдайлар.
- CI қорғаныс сценарийлерін `.github/workflows/agents-guardrails.yml` арқылы басқарады
  сондықтан саясаттар бұзылған кезде тарту сұраулары тез орындалмайды.
- Үлгі git ілгегі (`hooks/pre-commit.sample`) қоршауды, тәуелділікті,
  missing-docs, only std, env-config және status-синхрондау сценарийлері, сондықтан үлес қосушылар
  саясатты бұзуды CI алдында ұстаңыз. Кез келген қасақана TODO нан үгінділерін сақтаңыз
  үлкен өзгерістерді үнсіз кейінге қалдырудың орнына бақылау.