---
lang: uz
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-19T07:43:34.047491+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# AGENTLARni ishlab chiqish jarayoni

Ushbu ish kitobi AGENTS yo'l xaritasidagi hissa qo'shuvchi himoya vositalarini birlashtiradi
yangi yamalar bir xil standart eshiklarni kuzatib boradi.

## Tez boshlash maqsadlari

- Bajarish uchun `make dev-workflow` (`scripts/dev_workflow.sh` atrofidagi o'rash) ni ishga tushiring:
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3. `cargo build --workspace --locked`
  4. `cargo test --workspace --locked`
  5. `swift test`, `IrohaSwift/`
- `cargo test --workspace` uzoq davom etadi (ko'pincha soat). Tez takrorlash uchun,
  `scripts/dev_workflow.sh --skip-tests` yoki `--skip-swift` dan foydalaning, so'ngra to'liq ishga tushiring
  jo'natishdan oldin ketma-ketlik.
- Agar `cargo test --workspace` ma'lumotnoma bloklarida to'xtab qolsa, qayta ishga tushiring.
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (yoki o'rnatilgan
  `CARGO_TARGET_DIR` izolyatsiya qilingan yo'lga).
- Barcha yuk bosqichlari saqlash saqlash siyosatiga rioya qilish uchun `--locked` dan foydalanadi.
  `Cargo.lock` tegilmagan. Qo'shishdan ko'ra mavjud qutilarni kengaytirishni afzal ko'ring
  yangi ish maydoni a'zolari; yangi qutini kiritishdan oldin rozilikni izlang.

## Qo'riqchilar- Agar `make check-agents-guardrails` (yoki `ci/check_agents_guardrails.sh`) bajarilmasa
  filial `Cargo.lock`-ni o'zgartiradi, yangi ish maydoni a'zolarini kiritadi yoki yangilarini qo'shadi
  bog'liqliklar. Skript ishchi daraxt va `HEAD` ni taqqoslaydi
  Sukut bo'yicha `origin/main`; bazani bekor qilish uchun `AGENTS_BASE_REF=<ref>` ni o'rnating.
- `make check-dependency-discipline` (yoki `ci/check_dependency_discipline.sh`)
  `Cargo.toml` bog'liqliklarini bazaga nisbatan farq qiladi va yangi qutilarda muvaffaqiyatsizlikka uchraydi; o'rnatish
  Qasddan tan olish uchun `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>`
  qo'shimchalar.
- `make check-missing-docs` (yoki `ci/check_missing_docs_guard.sh`) yangi bloklarni bloklaydi
  `#[allow(missing_docs)]` yozuvlari, bayroqlar tegilgan qutilar (eng yaqin `Cargo.toml`)
  uning `src/lib.rs`/`src/main.rs` sandiq darajasidagi `//!` hujjatlariga ega emas va yangisini rad etadi
  asosiy referatga nisbatan `///` hujjatlarisiz ommaviy ob'ektlar; o'rnatish
  `MISSING_DOCS_GUARD_ALLOW=1` faqat tekshiruvchi roziligi bilan. Qo'riqchi ham
  `docs/source/agents/missing_docs_inventory.{json,md}` yangi ekanligini tasdiqlaydi;
  `python3 scripts/inventory_missing_docs.py` bilan qayta tiklang.
- `make check-tests-guard` (yoki `ci/check_tests_guard.sh`) qutilari
  o'zgartirilgan Rust funktsiyalarida birlik sinovi dalillari yo'q. Qo'riqchi xaritalari chiziqlarni o'zgartirdi
  funktsiyalarga o'tadi, agar diffda sandiq testlari o'zgargan bo'lsa, o'tadi va aks holda skanerdan o'tadi
  mos keladigan funktsiya chaqiruvlari uchun mavjud sinov fayllari, shuning uchun oldindan mavjud qamrov
  hisoblar; mos keladigan testlarsiz qutilar muvaffaqiyatsiz bo'ladi. `TEST_GUARD_ALLOW=1` o'rnating
  faqat o'zgarishlar haqiqatan ham sinov uchun neytral bo'lsa va sharhlovchi rozi bo'lsa.
- `make check-docs-tests-metrics` (yoki `ci/check_docs_tests_metrics_guard.sh`)
  muhim bosqichlar hujjatlar bilan birga harakatlanadigan yo'l xaritasi siyosatini amalga oshiradi,
  testlar va ko'rsatkichlar / asboblar paneli. `roadmap.md` ga nisbatan o'zgarganda
  `AGENTS_BASE_REF`, qo'riqchi kamida bitta hujjat o'zgarishini, bitta test o'zgarishini kutadi,
  va bitta o'lchov / telemetriya / asboblar panelini o'zgartirish. `DOC_TEST_METRIC_GUARD_ALLOW=1` o'rnating
  faqat sharhlovchining roziligi bilan.
- `make check-todo-guard` (yoki `ci/check_todo_guard.sh`) TODO markerlari ishlamay qoladi
  hujjatlar/testlar oʻzgarishlarisiz yoʻqoladi. Qoplamani qo'shing yoki yangilang
  TODOni hal qilishda yoki qasddan olib tashlash uchun `TODO_GUARD_ALLOW=1` ni o'rnating.
- `make check-std-only` (yoki `ci/check_std_only.sh`) bloklari `no_std`/`wasm32`
  cfgs, shuning uchun ish maydoni faqat `std` bo'lib qoladi. Faqat `STD_ONLY_GUARD_ALLOW=1` uchun o'rnating
  ruxsat etilgan CI tajribalari.
- `make check-status-sync` (yoki `ci/check_status_sync.sh`) yo'l xaritasini ochiq tutadi
  bo'lim tugallangan elementlardan xoli va `roadmap.md`/`status.md` talab qiladi
  birgalikda o'zgartiring, shuning uchun reja/holat bir xil bo'lib qoladi; o'rnatish
  `STATUS_SYNC_ALLOW_UNPAIRED=1` faqat kamdan-kam holatlarda faqat matn terish xatosini tuzatish uchun
  mahkamlash `AGENTS_BASE_REF`.
- `make check-proc-macro-ui` (yoki `ci/check_proc_macro_ui.sh`) trybuild dasturini ishga tushiradi
  Olingan/proc-makros qutilari uchun UI to'plamlari. Proc-makroslarga tegganda uni ishga tushiring
  `.stderr` diagnostikasini barqaror ushlab turing va vahima qo'zg'atuvchi UI regressiyalarini ushlang; o'rnatish
  Muayyan qutilarga e'tibor qaratish uchun `PROC_MACRO_UI_CRATES="crate1 crate2"`.
- `make check-env-config-surface` (yoki `ci/check_env_config_surface.sh`) qayta quriladi
  env-toggle inventarizatsiyasi (`docs/source/agents/env_var_inventory.{json,md}`),
  agar u eskirgan bo'lsa, **va** yangi ishlab chiqarish env shimlari paydo bo'lganda ishlamay qoladi
  `AGENTS_BASE_REF` ga nisbatan (avtomatik ravishda aniqlanadi; kerak bo'lganda aniq o'rnatiladi).
  orqali env qidiruvlarini qo'shish/o'chirishdan so'ng trekerni yangilang
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  `ENV_CONFIG_GUARD_ALLOW=1` dan faqat ataylab env tugmalarini hujjatlashtirgandan keyin foydalaningmigratsiya kuzatuvchisida.
- `make check-serde-guard` (yoki `ci/check_serde_guard.sh`) serdani qayta tiklaydi
  foydalanish inventar (`docs/source/norito_json_inventory.{json,md}`) temp
  joylashuvi, agar topshirilgan inventar eskirgan bo'lsa, muvaffaqiyatsiz bo'ladi va har qanday yangisini rad etadi
  ishlab chiqarish `serde`/`serde_json` `AGENTS_BASE_REF` ga nisbatan xitlar. Oʻrnatish
  `SERDE_GUARD_ALLOW=1` faqat migratsiya rejasini topshirgandan keyin CI tajribalari uchun.
- `make guards` Norito seriyalash siyosatini qo'llaydi: u yangisini rad etadi
  `serde`/`serde_json` foydalanish, maxsus AoS yordamchilari va tashqaridagi SCALE bog'liqliklari
  Norito skameykalar (`scripts/deny_serde_json.sh`,
  `scripts/check_no_direct_serde.sh`, `scripts/deny_handrolled_aos.sh`,
  `scripts/check_no_scale.sh`).
- **Proc-macro UI siyosati:** har bir proc-makros qutisi `trybuild` yuklanishi kerak
  `trybuild-tests` orqasida jabduqlar (`tests/ui.rs` o'tish/qobiliyatsiz globusli)
  xususiyat. Baxtli yo'l namunalarini `tests/ui/pass` ostiga, rad etish holatlari ostida joylashtiring
  Belgilangan `.stderr` chiqishlari bilan `tests/ui/fail` va diagnostikani davom ettiring
  vahima qo'zg'atmaslik va barqaror. bilan jihozlarni yangilang
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (ixtiyoriy
  `CARGO_TARGET_DIR=target-codex` mavjud tuzilmalarni yopishmaslik uchun) va
  qamrovni qurishga tayanmang (`cfg(not(coverage))` soqchilar kutilmoqda).
  Ikkilik kirish nuqtasini chiqarmaydigan makroslar uchun afzallik beriladi
  Xatolarga e'tibor qaratish uchun `// compile-flags: --crate-type lib` moslamalarda. Qo'shish
  diagnostika o'zgarganda yangi salbiy holatlar.
- CI qo'riqchi skriptlarini `.github/workflows/agents-guardrails.yml` orqali boshqaradi
  shuning uchun siyosatlar buzilganda pull so'rovlari tezda muvaffaqiyatsiz bo'ladi.
- Namuna git kancasi (`hooks/pre-commit.sample`) qo'riqchi, qaramlik,
  etishmayotgan hujjatlar, faqat std, env-config va status-sinxronizatsiya skriptlari, shuning uchun hissa qo'shuvchilar
  CIdan oldin siyosat buzilishlarini ushlang. Har qanday qasddan TODO bo'laklarini saqlang
  katta o'zgarishlarni jimgina kechiktirish o'rniga kuzatuvlar.