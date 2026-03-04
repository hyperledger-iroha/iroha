---
lang: mn
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-19T07:43:34.047491+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# AGENTS хөгжүүлэх ажлын урсгал

Энэхүү runbook нь AGENTS-ийн замын зураг дээрх хувь нэмэр оруулагчийн хамгаалалтын хашлагуудыг нэгтгэсэн болно
шинэ засварууд нь ижил үндсэн хаалгыг дагадаг.

## Түргэн эхлүүлэх зорилтууд

- Гүйцэтгэхийн тулд `make dev-workflow` (`scripts/dev_workflow.sh`-ийн эргэн тойронд боодол) ажиллуулна уу:
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3. `cargo build --workspace --locked`
  4. `cargo test --workspace --locked`
  5. `IrohaSwift/`-с `swift test`
- `cargo test --workspace` урт хугацаанд ажилладаг (ихэвчлэн цаг). Хурдан давталтын хувьд,
  `scripts/dev_workflow.sh --skip-tests` эсвэл `--skip-swift`-г ашиглаад дараа нь бүрэн ажиллуулна уу.
  тээвэрлэлтийн өмнөх дараалал.
- Хэрэв `cargo test --workspace` нь бүтээх лавлах түгжээн дээр зогсвол дахин ажиллуулна уу.
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (эсвэл тохируулсан
  `CARGO_TARGET_DIR` тусгаарлагдсан зам руу) маргаанаас зайлсхийхийн тулд.
- Ачааны бүх шат дамжлага нь хадгалах сангийн бодлогыг хүндэтгэхийн тулд `--locked` ашигладаг.
  `Cargo.lock` хөндөгдөөгүй. Нэмэхийн оронд одоо байгаа хайрцгийг өргөтгөхийг илүүд үзээрэй
  ажлын байрны шинэ гишүүд; шинэ хайрцгийг нэвтрүүлэхээсээ өмнө зөвшөөрөл авах.

## Хамгаалалтын хашлага- Хэрэв `make check-agents-guardrails` (эсвэл `ci/check_agents_guardrails.sh`) амжилтгүй болно
  салбар нь `Cargo.lock`-г өөрчилдөг, ажлын талбарын шинэ гишүүдийг танилцуулж эсвэл шинээр нэмдэг
  хамаарал. Скрипт нь ажлын мод болон `HEAD`-ийг харьцуулдаг
  Анхдагчаар `origin/main`; суурийг дарахын тулд `AGENTS_BASE_REF=<ref>`-г тохируулна уу.
- `make check-dependency-discipline` (эсвэл `ci/check_dependency_discipline.sh`)
  Суурийн эсрэг `Cargo.toml` хамаарлыг ялгаж, шинэ хайрцаг дээр амжилтгүй болно; тогтоосон
  санаатай гэдгийг хүлээн зөвшөөрөхийн тулд `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>`
  нэмэлтүүд.
- `make check-missing-docs` (эсвэл `ci/check_missing_docs_guard.sh`) шинээр блоклодог
  `#[allow(missing_docs)]` оруулгууд, тугнууд хүрсэн хайрцаг (хамгийн ойрын `Cargo.toml`)
  `src/lib.rs`/`src/main.rs` нь хайрцагны түвшний `//!` документ дутуу, шинэ баримтаас татгалздаг
  үндсэн лавлагаатай харьцуулахад `///` баримтгүй нийтийн эд зүйлс; тогтоосон
  `MISSING_DOCS_GUARD_ALLOW=1` зөвхөн хянагчийн зөвшөөрлөөр. Хамгаалагч ч гэсэн
  `docs/source/agents/missing_docs_inventory.{json,md}` шинэхэн эсэхийг шалгана;
  `python3 scripts/inventory_missing_docs.py`-ээр нөхөн сэргээх.
- `make check-tests-guard` (эсвэл `ci/check_tests_guard.sh`) хайрцагны туг
  өөрчлөгдсөн Rust функцуудад нэгж тестийн нотлох баримт дутмаг. Хамгаалалтын газрын зураг шугамыг өөрчилсөн
  функцууд руу, хэрэв крат тест нь дифференциод өөрчлөгдсөн бол тэнцэх ба бусад тохиолдолд сканнердах болно
  функцийн дуудлагыг тааруулахын тулд одоо байгаа туршилтын файлууд тул урьд өмнө байгаа хамрах хүрээ
  тоолох; Тохирох туршилтгүй хайрцагнууд амжилтгүй болно. `TEST_GUARD_ALLOW=1` тохируулна уу
  Өөрчлөлтүүд үнэхээр туршилтаас ангид байж, хянагч зөвшөөрсөн тохиолдолд л.
- `make check-docs-tests-metrics` (эсвэл `ci/check_docs_tests_metrics_guard.sh`)
  чухал үе шатууд нь баримт бичигтэй зэрэгцэн хөдөлдөг замын зураглалын бодлогыг хэрэгжүүлдэг,
  тест, хэмжүүр/хяналтын самбар. Хэзээ `roadmap.md` харьцангуй өөрчлөлт
  `AGENTS_BASE_REF`, хамгаалагч дор хаяж нэг баримтын өөрчлөлт, нэг туршилтын өөрчлөлт,
  мөн нэг хэмжүүр/телеметрийн/хяналтын самбарын өөрчлөлт. `DOC_TEST_METRIC_GUARD_ALLOW=1` тохируулна уу
  зөвхөн шүүмжлэгчийн зөвшөөрлөөр.
- TODO тэмдэглэгээ хийх үед `make check-todo-guard` (эсвэл `ci/check_todo_guard.sh`) амжилтгүй болно
  дагалдах баримт бичиг/туршилтын өөрчлөлтгүйгээр алга болно. Хамрах хүрээ нэмэх эсвэл шинэчлэх
  TODO-г шийдвэрлэх үед, эсвэл санаатайгаар арилгахын тулд `TODO_GUARD_ALLOW=1`-г тохируулна уу.
- `make check-std-only` (эсвэл `ci/check_std_only.sh`) блокууд `no_std`/`wasm32`
  cfg байгаа тул ажлын талбар нь зөвхөн `std` хэвээр үлдэнэ. Зөвхөн `STD_ONLY_GUARD_ALLOW=1`-г тохируулна уу
  зөвшөөрөгдсөн CI туршилтууд.
- `make check-status-sync` (эсвэл `ci/check_status_sync.sh`) замын зураглалыг нээлттэй байлгадаг.
  Энэ хэсэгт дууссан зүйл байхгүй бөгөөд `roadmap.md`/`status.md` шаардлагатай.
  Төлөвлөгөө/төлөв байдал нийцэж байгаа тул хамтдаа өөрчлөх; тогтоосон
  `STATUS_SYNC_ALLOW_UNPAIRED=1` нь зөвхөн төлөв байдлын ховор алдааг зассаны дараа
  `AGENTS_BASE_REF` зүү.
- `make check-proc-macro-ui` (эсвэл `ci/check_proc_macro_ui.sh`) trybuild-г ажиллуулдаг.
  Дерив/прок-макро хайрцагт зориулсан UI иж бүрдэл. Proc-makros-д хүрэх үед үүнийг ажиллуул
  `.stderr` оношилгоог тогтвортой байлгаж, UI регрессийг сандаргах; тогтоосон
  `PROC_MACRO_UI_CRATES="crate1 crate2"` тусгай хайрцагт анхаарлаа хандуулаарай.
- `make check-env-config-surface` (эсвэл `ci/check_env_config_surface.sh`) дахин бүтээдэг
  env-шилжүүлэх бараа материал (`docs/source/agents/env_var_inventory.{json,md}`),
  Хэрэв энэ нь хуучирсан бол бүтэлгүйтдэг, **болон** шинэ үйлдвэрлэлийн env жийргэвч гарч ирэх үед бүтэлгүйтдэг
  `AGENTS_BASE_REF`-тай харьцуулахад (автоматаар илэрсэн; шаардлагатай үед тодорхой тохируулсан).
  Env хайлтуудыг нэмж/хасасны дараа трекерийг сэргээнэ үү
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  `ENV_CONFIG_GUARD_ALLOW=1`-г зөвхөн зориудаар хийсэн env товчлууруудыг баримтжуулсны дараа ашиглана уушилжилт хөдөлгөөн хянагч дээр.
- `make check-serde-guard` (эсвэл `ci/check_serde_guard.sh`) сердийг сэргээдэг.
  хэрэглээний тооллого (`docs/source/norito_json_inventory.{json,md}`) хэмд
  байршил, хэрэв хүлээлгэн өгсөн бараа материал хуучирсан бол бүтэлгүйтэж, шинэ зүйлээс татгалздаг
  үйлдвэрлэл `serde`/`serde_json` `AGENTS_BASE_REF`-тай харьцуулахад хит. Тохируулах
  `SERDE_GUARD_ALLOW=1` нь зөвхөн шилжих төлөвлөгөөг бөглөсний дараа CI туршилтуудад зориулагдсан.
- `make guards` нь Norito цуваачлалын бодлогыг хэрэгжүүлдэг: энэ нь шинийг үгүйсгэдэг
  `serde`/`serde_json` хэрэглээ, түр зуурын AoS туслахууд болон гаднах SCALE хамаарлууд
  Norito вандан сандал (`scripts/deny_serde_json.sh`,
  `scripts/check_no_direct_serde.sh`, `scripts/deny_handrolled_aos.sh`,
  `scripts/check_no_scale.sh`).
- **Proc-makro UI бодлого:** proc-makro хайрцаг бүр `trybuild`-г тээвэрлэх ёстой.
  `trybuild-tests`-ийн ард морины оосор (`tests/ui.rs` дамжуулалт/бүтэлгүйтсэн бөмбөлөгтэй)
  онцлог. Аз жаргалтай замын дээжийг `tests/ui/pass` доор, татгалзсан тохиолдлыг доор байрлуул
  `tests/ui/fail` `.stderr` гаралттай, оношилгоогоо үргэлжлүүлээрэй
  сандрахгүй, тогтвортой. -ээр бэхэлгээг сэргээнэ үү
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (сонголтоор
  `CARGO_TARGET_DIR=target-codex` одоо байгаа барилгуудыг бөглөрөхөөс зайлсхийхийн тулд) болон
  хамрах хүрээний бүтээн байгуулалтад найдахаас зайлсхий (`cfg(not(coverage))` хамгаалалттай байх ёстой).
  Хоёртын нэвтрэх цэгийг ялгаруулдаггүй макросын хувьд илүүд үздэг
  `// compile-flags: --crate-type lib` нь алдааг төвлөрүүлэхийн тулд бэхэлгээнд суулгасан. Нэмэх
  Оношлогоо өөрчлөгдөх бүрт шинэ сөрөг тохиолдол.
- CI нь `.github/workflows/agents-guardrails.yml`-ээр дамжуулан хамгаалалтын скриптүүдийг ажиллуулдаг
  Тиймээс бодлогыг зөрчсөн тохиолдолд татах хүсэлт хурдан бүтэлгүйтдэг.
- Жишээ git hook (`hooks/pre-commit.sample`) нь хамгаалалтын хашлага, хамаарал,
  дутуу-докс, зөвхөн std, env-config, статус-синк скриптүүд нь хувь нэмэр оруулагчид
  Бодлогын зөрчлийг CI-ээс өмнө барих. Ямар ч санаатайгаар TODO талхны үйрмэгийг хадгал
  том өөрчлөлтүүдийг чимээгүй хойшлуулахын оронд дагаж мөрдөх.