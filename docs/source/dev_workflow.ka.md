---
lang: ka
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-19T07:43:34.047491+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# AGENTS Development Workflow

ეს წარწერის წიგნი აერთიანებს AGENTS-ის საგზაო რუქის კონტრიბუტორს
ახალი პატჩები მიჰყვება იგივე ნაგულისხმევ კარიბჭეებს.

## სწრაფი დაწყების მიზნები

- გაუშვით `make dev-workflow` (შეფუთვა `scripts/dev_workflow.sh`-ის გარშემო) შესასრულებლად:
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3. `cargo build --workspace --locked`
  4. `cargo test --workspace --locked`
  5. `swift test` `IrohaSwift/`-დან
- `cargo test --workspace` არის გრძელვადიანი (ხშირად საათობით). სწრაფი გამეორებისთვის,
  გამოიყენეთ `scripts/dev_workflow.sh --skip-tests` ან `--skip-swift`, შემდეგ გაუშვით სრული
  თანმიმდევრობა გაგზავნამდე.
- თუ `cargo test --workspace` ჩერდება build დირექტორია ბლოკირებზე, ხელახლა გაუშვით
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (ან კომპლექტი
  `CARGO_TARGET_DIR` იზოლირებულ გზაზე) კამათის თავიდან ასაცილებლად.
- ტვირთის ყველა საფეხური იყენებს `--locked` შენახვის საცავის პოლიტიკის პატივისცემას
  `Cargo.lock` ხელუხლებელი. ამჯობინეთ არსებული ყუთების გაფართოება, ვიდრე დამატება
  სამუშაო სივრცის ახალი წევრები; მოიძიეთ თანხმობა ახალი ყუთის შემოღებამდე.

## დაცვა- `make check-agents-guardrails` (ან `ci/check_agents_guardrails.sh`) ვერ ხერხდება, თუ
  ფილიალი ცვლის `Cargo.lock`-ს, წარადგენს სამუშაო სივრცის ახალ წევრებს ან ამატებს ახალს
  დამოკიდებულებები. სკრიპტი ადარებს სამუშაო ხეს და `HEAD`-ს
  ნაგულისხმევად `origin/main`; დააყენეთ `AGENTS_BASE_REF=<ref>` ბაზის გადაფარვის მიზნით.
- `make check-dependency-discipline` (ან `ci/check_dependency_discipline.sh`)
  განასხვავებს `Cargo.toml` დამოკიდებულებებს ფუძესთან და არღვევს ახალ უჯრებზე; კომპლექტი
  `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>` ვაღიაროთ განზრახ
  დამატებები.
- `make check-missing-docs` (ან `ci/check_missing_docs_guard.sh`) ბლოკები ახალი
  `#[allow(missing_docs)]` ჩანაწერები, დროშები შეეხო ყუთებს (უახლოესი `Cargo.toml`)
  რომლის `src/lib.rs`/`src/main.rs` არ აქვს კრატის დონის `//!` დოკუმენტები და უარყოფს ახალს
  საჯარო ნივთები `///` დოკუმენტების გარეშე ბაზის ref-თან შედარებით; კომპლექტი
  `MISSING_DOCS_GUARD_ALLOW=1` მხოლოდ მიმომხილველის თანხმობით. მცველიც
  ამოწმებს, რომ `docs/source/agents/missing_docs_inventory.{json,md}` ახალია;
  რეგენერაცია `python3 scripts/inventory_missing_docs.py`-ით.
- `make check-tests-guard` (ან `ci/check_tests_guard.sh`) დროშის ყუთები, რომელთა
  შეცვლილი Rust ფუნქციებს აკლია ერთეული ტესტის მტკიცებულება. დაცვის რუქებმა შეცვალეს ხაზები
  ფუნქციებზე, გადის, თუ კრატის ტესტები შეიცვალა განსხვავებაში, და სხვაგვარად სკანირებს
  არსებული სატესტო ფაილები შესატყვისი ფუნქციის ზარებისთვის, ასე რომ, წინასწარ არსებული გაშუქება
  ითვლის; ყუთები ყოველგვარი შესატყვისი ტესტის გარეშე ჩავარდება. კომპლექტი `TEST_GUARD_ALLOW=1`
  მხოლოდ მაშინ, როდესაც ცვლილებები ნამდვილად ტესტის ნეიტრალურია და მიმომხილველი ეთანხმება.
- `make check-docs-tests-metrics` (ან `ci/check_docs_tests_metrics_guard.sh`)
  ახორციელებს საგზაო რუქის პოლიტიკას, რომ ეტაპები მოძრაობს დოკუმენტაციის პარალელურად,
  ტესტები და მეტრიკა/დაფები. როდესაც `roadmap.md` იცვლება შედარებით
  `AGENTS_BASE_REF`, მცველი ელის მინიმუმ ერთ დოკუმენტის ცვლილებას, ერთ ტესტის შეცვლას,
  და ერთი მეტრიკის/ტელემეტრიის/დაფის ცვლილება. კომპლექტი `DOC_TEST_METRIC_GUARD_ALLOW=1`
  მხოლოდ მიმომხილველის თანხმობით.
- `make check-todo-guard` (ან `ci/check_todo_guard.sh`) ვერ ხერხდება TODO მარკერების დროს
  გაქრება თანმხლები დოკუმენტების/ტესტების ცვლილებების გარეშე. დაფარვის დამატება ან განახლება
  TODO-ს გადაჭრისას, ან დააყენეთ `TODO_GUARD_ALLOW=1` განზრახ ამოღებისთვის.
- `make check-std-only` (ან `ci/check_std_only.sh`) ბლოკები `no_std`/`wasm32`
  cfgs, ასე რომ სამუშაო სივრცე დარჩეს მხოლოდ `std`. დააყენეთ `STD_ONLY_GUARD_ALLOW=1` მხოლოდ
  სანქცირებული CI ექსპერიმენტები.
- `make check-status-sync` (ან `ci/check_status_sync.sh`) ინახავს საგზაო რუქას ღიად
  განყოფილება დასრულებული ელემენტებისგან თავისუფალი და მოითხოვს `roadmap.md`/`status.md`
  შეცვალეთ ერთად ისე, რომ გეგმა/სტატუსები დარჩეს შესაბამისობაში; კომპლექტი
  `STATUS_SYNC_ALLOW_UNPAIRED=1` მხოლოდ იშვიათ სტატუსზე დაწერილი შეცდომების გამოსწორების შემდეგ
  დამაგრება `AGENTS_BASE_REF`.
- `make check-proc-macro-ui` (ან `ci/check_proc_macro_ui.sh`) აწარმოებს trybuild-ს
  UI კომპლექტები წარმოებული/პროკ-მაკრო კრატებისთვის. გაუშვით პროკ-მაკროსთან შეხებისას
  შეინახეთ `.stderr` დიაგნოსტიკა სტაბილურად და დაიჭირეთ პანიკური ინტერფეისის რეგრესია; კომპლექტი
  `PROC_MACRO_UI_CRATES="crate1 crate2"` კონკრეტულ ყუთებზე ფოკუსირებისთვის.
- `make check-env-config-surface` (ან `ci/check_env_config_surface.sh`) აღდგება
  env-toggle ინვენტარი (`docs/source/agents/env_var_inventory.{json,md}`),
  წარუმატებელია, თუ ის მოძველებულია, ** და ** მარცხდება, როდესაც გამოჩნდება ახალი წარმოების env shims
  `AGENTS_BASE_REF`-თან შედარებით (ავტომატურად გამოვლენილი; მკაფიოდ დაყენებულია საჭიროების შემთხვევაში).
  განაახლეთ ტრეკერი env საძიებლების დამატების/მოხსნის შემდეგ
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  გამოიყენეთ `ENV_CONFIG_GUARD_ALLOW=1` მხოლოდ განზრახ env ღილაკების დოკუმენტაციის შემდეგმიგრაციის ტრეკერში.
- `make check-serde-guard` (ან `ci/check_serde_guard.sh`) აღადგენს სერდს
  გამოიყენეთ ინვენტარი (`docs/source/norito_json_inventory.{json,md}`) ტემ
  მდებარეობა, ვერ ხერხდება, თუ ჩადენილი ინვენტარი მოძველებულია და უარყოფს ნებისმიერ ახალს
  წარმოება `serde`/`serde_json` ხვდება `AGENTS_BASE_REF`-თან შედარებით. კომპლექტი
  `SERDE_GUARD_ALLOW=1` მხოლოდ CI ექსპერიმენტებისთვის მიგრაციის გეგმის წარდგენის შემდეგ.
- `make guards` ახორციელებს Norito სერიიზაციის პოლიტიკას: ის უარყოფს ახალს
  `serde`/`serde_json` გამოყენება, ad-hoc AoS დამხმარეები და SCALE დამოკიდებულებები გარეთ
  Norito სკამები (`scripts/deny_serde_json.sh`,
  `scripts/check_no_direct_serde.sh`, `scripts/deny_handrolled_aos.sh`,
  `scripts/check_no_scale.sh`).
- **Proc-macro UI პოლიტიკა:** ყველა proc-macro crate უნდა გამოგზავნოთ `trybuild`
  აღკაზმულობა (`tests/ui.rs` უღელტეხილით/ჩავარდნის გლობუსებით) `trybuild-tests`-ის უკან
  თვისება. მოათავსეთ ბედნიერი ბილიკის ნიმუშები `tests/ui/pass`-ში, უარყოფის შემთხვევები ქვემოთ
  `tests/ui/fail` ჩადებული `.stderr` გამომავლებით და შეინახეთ დიაგნოსტიკა
  პანიკური და სტაბილური. განაახლეთ მოწყობილობები ერთად
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (სურვილისამებრ
  `CARGO_TARGET_DIR=target-codex` არსებული კონსტრუქციების დაბინძურების თავიდან ასაცილებლად) და
  მოერიდეთ დაფარვის კონსტრუქციებზე დაყრდნობას (მოსალოდნელია `cfg(not(coverage))` მცველები).
  მაკროებისთვის, რომლებიც არ ასხივებენ ბინარულ შესასვლელ წერტილს, უპირატესობა მიანიჭეთ
  `// compile-flags: --crate-type lib` მოწყობილობებში შეცდომების ფოკუსირების შესანარჩუნებლად. დამატება
  ახალი უარყოფითი შემთხვევები, როდესაც დიაგნოზი იცვლება.
- CI მართავს დამცავ სკრიპტებს `.github/workflows/agents-guardrails.yml`-ის საშუალებით
  ასე რომ, pull მოთხოვნები სწრაფად ვერ ხერხდება, როდესაც პოლიტიკა ირღვევა.
- სანიმუშო git hook (`hooks/pre-commit.sample`) ეშვება დამცავი რგოლებით, დამოკიდებულებით,
  missing-docs, std-only, env-config და სტატუსის სინქრონიზაციის სკრიპტები ასე რომ კონტრიბუტორები
  დაიჭირეთ პოლიტიკის დარღვევები CI-მდე. შეინახეთ TODO breadcrumbs ნებისმიერი განზრახ
  შემდგომი დაკვირვებები დიდი ცვლილებების ჩუმად გადადების ნაცვლად.