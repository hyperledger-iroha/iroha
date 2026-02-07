---
lang: hy
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-19T07:43:34.047491+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ԳՈՐԾԱԿԱԼՆԵՐ Զարգացման աշխատանքային հոսք

Այս տեղեկագիրքը համախմբում է AGENTS-ի ճանապարհային քարտեզի աջակցող պաշտպանիչ բազկաթոռները
նոր բծերը հետևում են նույն լռելյայն դարպասներին:

## Արագ մեկնարկի թիրախներ

- Գործարկեք `make dev-workflow` (փաթաթան `scripts/dev_workflow.sh`-ի շուրջ)՝
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3. `cargo build --workspace --locked`
  4. `cargo test --workspace --locked`
  5. `swift test` `IrohaSwift/`-ից
- `cargo test --workspace`-ը երկարատև է (հաճախ ժամեր): Արագ կրկնությունների համար՝
  օգտագործեք `scripts/dev_workflow.sh --skip-tests` կամ `--skip-swift`, ապա գործարկեք ամբողջական
  հաջորդականությունը մինչև առաքումը:
- Եթե `cargo test --workspace`-ը կանգ է առնում build գրացուցակի կողպեքների վրա, նորից գործարկեք
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (կամ հավաքածու
  `CARGO_TARGET_DIR` դեպի մեկուսացված ճանապարհ)՝ վեճերից խուսափելու համար:
- Բեռնափոխադրումների բոլոր քայլերն օգտագործում են `--locked`՝ պահպանելու պահեստային քաղաքականությունը հարգելու համար
  `Cargo.lock` անձեռնմխելի. Նախընտրում են երկարացնել առկա արկղերը, քան ավելացնել
  աշխատանքային տարածքի նոր անդամներ; հավանություն փնտրեք նոր տուփ ներմուծելուց առաջ:

## Պահակներ- `make check-agents-guardrails` (կամ `ci/check_agents_guardrails.sh`) ձախողվում է, եթե ա
  մասնաճյուղը փոփոխում է `Cargo.lock`-ը, ներկայացնում է աշխատանքային տարածքի նոր անդամներ կամ ավելացնում է նորը
  կախվածություններ. Սցենարը համեմատում է աշխատող ծառը և `HEAD`-ը
  `origin/main` լռելյայն; սահմանել `AGENTS_BASE_REF=<ref>`՝ հիմքը վերացնելու համար:
- `make check-dependency-discipline` (կամ `ci/check_dependency_discipline.sh`)
  տարբերում է `Cargo.toml` կախվածությունը բազայից և ձախողվում է նոր արկղերի վրա. հավաքածու
  `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>`՝ գիտակցելու դիտավորությունը
  լրացումներ։
- `make check-missing-docs` (կամ `ci/check_missing_docs_guard.sh`) արգելափակում է նորը
  `#[allow(missing_docs)]` գրառումներ, դրոշները հպվել են տուփերին (մոտակա `Cargo.toml`)
  որոնց `src/lib.rs`/`src/main.rs` բացակայում են տուփի մակարդակի `//!` փաստաթղթերը և մերժում է նորը
  հանրային իրեր՝ առանց `///` փաստաթղթերի՝ բազային ref-ի համեմատ. հավաքածու
  `MISSING_DOCS_GUARD_ALLOW=1` միայն գրախոսի հաստատմամբ: Պահակը նույնպես
  ստուգում է, որ `docs/source/agents/missing_docs_inventory.{json,md}`-ը թարմ է.
  վերականգնել `python3 scripts/inventory_missing_docs.py`-ով:
- `make check-tests-guard` (կամ `ci/check_tests_guard.sh`) դրոշակներ, որոնց
  փոխված Rust ֆունկցիաները չունեն միավոր-փորձարկման ապացույցներ: Պահակային քարտեզները փոխեցին գծերը
  գործառույթներին, անցնում է, եթե արկղի թեստերը փոխվել են տարբերության մեջ, և այլապես սկանավորում է
  գոյություն ունեցող թեստային ֆայլեր համապատասխանող գործառույթների կանչերի համար, որպեսզի նախկինում գոյություն ունեցող ծածկույթը
  հաշվում; առանց համապատասխան փորձարկումների արկղերը ձախողվելու են: Կոմպլեկտ `TEST_GUARD_ALLOW=1`
  միայն այն դեպքում, երբ փոփոխություններն իսկապես չեզոք են, և վերանայողը համաձայնում է:
- `make check-docs-tests-metrics` (կամ `ci/check_docs_tests_metrics_guard.sh`)
  կիրառում է ճանապարհային քարտեզի քաղաքականությունը, ըստ որի նշաձողերը շարժվում են փաստաթղթերի կողքին,
  թեստեր և չափումներ/վահանակներ: Երբ `roadmap.md` փոխվում է համեմատ
  `AGENTS_BASE_REF`, պահակը ակնկալում է առնվազն մեկ փաստաթղթի փոփոխություն, մեկ թեստային փոփոխություն,
  և մեկ չափման/հեռաչափության/վահանակի փոփոխություն: Կոմպլեկտ `DOC_TEST_METRIC_GUARD_ALLOW=1`
  միայն գրախոսի հաստատմամբ:
- `make check-todo-guard` (կամ `ci/check_todo_guard.sh`) ձախողվում է, երբ TODO մարկերները
  անհետանալ առանց ուղեկցող փաստաթղթերի/թեստերի փոփոխությունների: Ավելացնել կամ թարմացնել ծածկույթը
  TODO-ն լուծելիս կամ սահմանել `TODO_GUARD_ALLOW=1` դիտավորյալ հեռացումների համար:
- `make check-std-only` (կամ `ci/check_std_only.sh`) բլոկներ `no_std`/`wasm32`
  cfgs, որպեսզի աշխատանքային տարածքը մնա միայն `std`-ով: Սահմանել `STD_ONLY_GUARD_ALLOW=1` միայն
  թույլատրված CI փորձեր:
- `make check-status-sync` (կամ `ci/check_status_sync.sh`) բաց է պահում ճանապարհային քարտեզը
  բաժինը զերծ է ավարտված տարրերից և պահանջում է `roadmap.md`/`status.md` մինչև
  փոխեք միասին, որպեսզի պլանը/կարգավիճակը մնա համահունչ. հավաքածու
  `STATUS_SYNC_ALLOW_UNPAIRED=1` միայն հազվագյուտ տառասխալների դեպքում, որոնք ուղղվում են դրանից հետո
  ամրացնելով `AGENTS_BASE_REF`:
- `make check-proc-macro-ui` (կամ `ci/check_proc_macro_ui.sh`) գործարկում է trybuild-ը
  UI հավաքակազմ՝ ածանցյալ/պրոկ-մակրո արկղերի համար: Գործարկեք այն, երբ հպում եք proc-macros-ին
  կայուն պահեք `.stderr` ախտորոշումը և հայտնաբերեք խուճապային միջերեսային ռեգրեսիաներ; հավաքածու
  `PROC_MACRO_UI_CRATES="crate1 crate2"`՝ հատուկ արկղերի վրա կենտրոնանալու համար:
- `make check-env-config-surface` (կամ `ci/check_env_config_surface.sh`) վերակառուցվում է
  env-toggle գույքագրում (`docs/source/agents/env_var_inventory.{json,md}`),
  ձախողվում է, եթե հնացած է, **և** ձախողվում է, երբ հայտնվում են նոր արտադրական նախագիծ
  համեմատած `AGENTS_BASE_REF`-ի հետ (ավտոհայտնաբերված, անհրաժեշտության դեպքում հստակ սահմանվել):
  Թարմացրեք որոնիչը՝ env որոնումները ավելացնելուց/հեռացնելուց հետո
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  օգտագործեք `ENV_CONFIG_GUARD_ALLOW=1` միայն դիտավորյալ env կոճակները փաստաթղթավորելուց հետոմիգրացիայի հետագծում:
- `make check-serde-guard` (կամ `ci/check_serde_guard.sh`) վերածնում է սերդը
  օգտագործման գույքագրումը (`docs/source/norito_json_inventory.{json,md}`) ջերմաստիճանի մեջ
  գտնվելու վայրը, ձախողվում է, եթե կատարված գույքագրումը հնացած է, և մերժում է ցանկացած նոր
  արտադրությունը `serde`/`serde_json` հարվածում է `AGENTS_BASE_REF`-ի համեմատ: Սահմանել
  `SERDE_GUARD_ALLOW=1` միայն CI փորձերի համար՝ միգրացիոն ծրագիր ներկայացնելուց հետո:
- `make guards`-ը պարտադրում է Norito սերիականացման քաղաքականությունը. այն հերքում է նորը
  `serde`/`serde_json` օգտագործումը, ժամանակավոր AoS օգնականները և SCALE կախվածությունները դրսում
  Norito նստարաններ (`scripts/deny_serde_json.sh`,
  `scripts/check_no_direct_serde.sh`, `scripts/deny_handrolled_aos.sh`,
  `scripts/check_no_scale.sh`):
- **Proc-macro UI քաղաքականություն.** յուրաքանչյուր proc-macro crate պետք է առաքի `trybuild`
  ամրագոտի (`tests/ui.rs`՝ անցումային/անհաջող գնդերով) `trybuild-tests`-ի հետևում
  հատկանիշ. Տեղադրեք երջանիկ ուղու նմուշները `tests/ui/pass`-ի տակ, մերժման դեպքերը՝ տակ
  `tests/ui/fail` պարտավորված `.stderr` ելքերով և պահպանել ախտորոշումը
  ոչ խուճապային և կայուն: Թարմացրեք հարմարանքները
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (ըստ ցանկության հետ
  `CARGO_TARGET_DIR=target-codex`՝ գոյություն ունեցող շինությունների խայտառակությունից խուսափելու համար) և
  խուսափեք ծածկույթի կառուցվածքների վրա հույս դնելուց (ակնկալվում են `cfg(not(coverage))` պահակներ):
  Մակրոների համար, որոնք չեն արձակում երկուական մուտքի կետ, նախընտրեք
  `// compile-flags: --crate-type lib` սարքերում` սխալները կենտրոնացված պահելու համար: Ավելացնել
  նոր բացասական դեպքեր, երբ ախտորոշումը փոխվում է:
- CI-ն գործարկում է պաշտպանական գծագրերը `.github/workflows/agents-guardrails.yml`-ի միջոցով
  այնպես որ pull հարցումները արագորեն ձախողվում են, երբ կանոնները խախտվում են:
- Նմուշի կեռիկի կեռիկը (`hooks/pre-commit.sample`) անցնում է պահակապակուց, կախվածություն,
  missing-docs, std-only, env-config և status-sync scripts so contributors
  բռնել քաղաքականության խախտումները CI-ից առաջ: Պահպանեք TODO breadcrumbs ցանկացած միտումնավոր
  հետաձգումներ՝ մեծ փոփոխությունները լուռ հետաձգելու փոխարեն: