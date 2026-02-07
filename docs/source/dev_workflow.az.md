---
lang: az
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-19T07:43:34.047491+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Agentlərin İnkişafı İş Akışı

Bu runbook AGENTS yol xəritəsindəki töhfəçi qoruyucularını birləşdirir
yeni yamalar eyni standart qapıları izləyir.

## Sürətli başlanğıc hədəfləri

- İcra etmək üçün `make dev-workflow` (`scripts/dev_workflow.sh` ətrafındakı sarğı) işə salın:
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3. `cargo build --workspace --locked`
  4. `cargo test --workspace --locked`
  5. `swift test`, `IrohaSwift/`
- `cargo test --workspace` uzun müddət işləyir (tez-tez saat). Sürətli təkrarlamalar üçün,
  `scripts/dev_workflow.sh --skip-tests` və ya `--skip-swift` istifadə edin, sonra tam
  göndərilmədən əvvəl ardıcıllıqla.
- `cargo test --workspace` quraşdırma kataloq kilidlərində dayanırsa, yenidən işə salın
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (və ya set
  `CARGO_TARGET_DIR` təcrid olunmuş yola) mübahisə etməmək üçün.
- Bütün yük mərhələləri saxlama anbar siyasətinə hörmət etmək üçün `--locked` istifadə edir.
  `Cargo.lock` toxunulmayıb. Əlavə etməkdənsə, mövcud qutuları genişləndirməyə üstünlük verin
  yeni iş sahəsi üzvləri; yeni bir qutu təqdim etməzdən əvvəl təsdiq axtarın.

## Korkuluklar- `make check-agents-guardrails` (və ya `ci/check_agents_guardrails.sh`) uğursuz olarsa
  filial `Cargo.lock`-i dəyişdirir, yeni iş sahəsi üzvləri təqdim edir və ya yeniləri əlavə edir
  asılılıqlar. Skript işləyən ağac və `HEAD` ilə müqayisə edir
  Defolt olaraq `origin/main`; bazanı ləğv etmək üçün `AGENTS_BASE_REF=<ref>` təyin edin.
- `make check-dependency-discipline` (və ya `ci/check_dependency_discipline.sh`)
  `Cargo.toml` asılılıqlarını bazadan fərqləndirir və yeni qutularda uğursuz olur; təyin edin
  qəsdən olduğunu etiraf etmək üçün `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>`
  əlavələr.
- `make check-missing-docs` (və ya `ci/check_missing_docs_guard.sh`) yeni bloklar
  `#[allow(missing_docs)]` girişləri, bayraqlara toxunan qutular (ən yaxın `Cargo.toml`)
  `src/lib.rs`/`src/main.rs`-də sandıq səviyyəli `//!` sənədləri yoxdur və yenisini rədd edir
  əsas referata nisbətən `///` sənədləri olmayan ictimai əşyalar; təyin edin
  `MISSING_DOCS_GUARD_ALLOW=1` yalnız rəyçinin təsdiqi ilə. Mühafizəçi də
  `docs/source/agents/missing_docs_inventory.{json,md}` təzə olduğunu yoxlayır;
  `python3 scripts/inventory_missing_docs.py` ilə bərpa edin.
- `make check-tests-guard` (və ya `ci/check_tests_guard.sh`) bayraqları olan qutular
  dəyişdirilmiş Rust funksiyalarında vahid test sübutu yoxdur. Mühafizə xəritələri xətləri dəyişdirdi
  funksiyalara, diff-də qutu testləri dəyişdikdə keçir və əks halda skan edir
  uyğun funksiya çağırışları üçün mövcud test faylları belə ki, əvvəlcədən mövcud əhatə
  sayır; heç bir uyğun testi olmayan qutular uğursuz olacaq. `TEST_GUARD_ALLOW=1` seçin
  yalnız dəyişikliklər həqiqətən sınaq üçün neytral olduqda və rəyçi razılaşdıqda.
- `make check-docs-tests-metrics` (və ya `ci/check_docs_tests_metrics_guard.sh`)
  mərhələlərin sənədlərlə yanaşı hərəkət etdiyi yol xəritəsi siyasətini tətbiq edir,
  testlər və ölçülər/iş panelləri. `roadmap.md` nisbətən dəyişdikdə
  `AGENTS_BASE_REF`, mühafizəçi ən azı bir sənəd dəyişikliyi, bir sınaq dəyişikliyi gözləyir,
  və bir ölçü/telemetri/ana panel dəyişikliyi. `DOC_TEST_METRIC_GUARD_ALLOW=1` seçin
  yalnız rəyçinin razılığı ilə.
- TODO markerləri uğursuz olduqda `make check-todo-guard` (və ya `ci/check_todo_guard.sh`)
  sənədləri/test dəyişikliklərini müşayiət etmədən yox. Əhatə dairəsini əlavə edin və ya yeniləyin
  TODO həll edərkən və ya qəsdən silmələr üçün `TODO_GUARD_ALLOW=1` təyin edin.
- `make check-std-only` (və ya `ci/check_std_only.sh`) blokları `no_std`/`wasm32`
  cfgs belə iş sahəsi yalnız `std`-də qalır. Yalnız `STD_ONLY_GUARD_ALLOW=1` üçün təyin edin
  sanksiyalaşdırılmış CI təcrübələri.
- `make check-status-sync` (və ya `ci/check_status_sync.sh`) yol xəritəsini açıq saxlayır
  bölmə tamamlanmış elementlərdən azaddır və `roadmap.md`/`status.md` tələb edir
  birlikdə dəyişdirin ki, plan/status uyğun olsun; təyin edin
  `STATUS_SYNC_ALLOW_UNPAIRED=1` yalnız nadir statuslu yazı xətalarını aradan qaldırdıqdan sonra
  sancma `AGENTS_BASE_REF`.
- `make check-proc-macro-ui` (və ya `ci/check_proc_macro_ui.sh`) trybuild-i idarə edir
  Alma/proc-makro qutular üçün UI dəstləri. Proc-makroslara toxunduqda onu işə salın
  `.stderr` diaqnostikasını sabit saxlamaq və UI reqressiyalarını təqib etmək; təyin edin
  Xüsusi qutulara diqqət yetirmək üçün `PROC_MACRO_UI_CRATES="crate1 crate2"`.
- `make check-env-config-surface` (və ya `ci/check_env_config_surface.sh`) yenidən qurur
  env-keçid inventar (`docs/source/agents/env_var_inventory.{json,md}`),
  köhnəldikdə uğursuz olur, **və** yeni istehsal env şimləri görünəndə uğursuz olur
  `AGENTS_BASE_REF`-ə nisbətən (avtomatik aşkarlanır; lazım olduqda açıq şəkildə təyin olunur).
  vasitəsilə env axtarışlarını əlavə etdikdən/çıxardıqdan sonra izləyicini yeniləyin
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  `ENV_CONFIG_GUARD_ALLOW=1`-i yalnız qəsdən env düymələrini sənədləşdirdikdən sonra istifadə edinmiqrasiya izləyicisində.
- `make check-serde-guard` (və ya `ci/check_serde_guard.sh`) serdeni bərpa edir
  İstifadə inventarını (`docs/source/norito_json_inventory.{json,md}`) temp
  yer, qəbul edilmiş inventar köhnədirsə uğursuz olur və hər hansı yenisini rədd edir
  istehsal `serde`/`serde_json`, `AGENTS_BASE_REF`-ə nisbətdə vurur. Set
  `SERDE_GUARD_ALLOW=1` yalnız miqrasiya planını təqdim etdikdən sonra CI təcrübələri üçün.
- `make guards` Norito seriallaşdırma siyasətini tətbiq edir: yenisini rədd edir
  `serde`/`serde_json` istifadəsi, ad-hoc AoS köməkçiləri və xaricində SCALE asılılıqları
  Norito skamyalar (`scripts/deny_serde_json.sh`,
  `scripts/check_no_direct_serde.sh`, `scripts/deny_handrolled_aos.sh`,
  `scripts/check_no_scale.sh`).
- **Proc-makro UI siyasəti:** hər bir proc-makro qutusu `trybuild` göndərməlidir
  `trybuild-tests` arxasında qoşqu (keçmə/uğursuz qlobus ilə `tests/ui.rs`)
  xüsusiyyət. Xoşbəxt yol nümunələrini `tests/ui/pass` altına, rədd hallarını isə altına qoyun
  Sadiq `.stderr` çıxışları ilə `tests/ui/fail` və diaqnostikaya davam edin
  paniksiz və sabitdir. ilə armaturları yeniləyin
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (isteğe bağlı olaraq
  `CARGO_TARGET_DIR=target-codex` mövcud konstruksiyaların tıxanmasının qarşısını almaq üçün) və
  əhatə dairəsi quruluşlarına etibar etməyin (`cfg(not(coverage))` mühafizəçiləri gözlənilir).
  İkili giriş nöqtəsi buraxmayan makrolar üçün üstünlük verin
  Səhvləri diqqət mərkəzində saxlamaq üçün qurğularda `// compile-flags: --crate-type lib`. əlavə et
  diaqnostika dəyişdikdə yeni neqativ hallar.
- CI qoruyucu skriptləri `.github/workflows/agents-guardrails.yml` vasitəsilə idarə edir
  beləliklə, siyasətlər pozulduqda pull sorğuları sürətlə uğursuz olur.
- Nümunə git çəngəl (`hooks/pre-commit.sample`) qoruyucu, asılılıq,
  əskik sənədlər, yalnız std, env-config və status-sinxronizasiya skriptləri
  CI-dən əvvəl siyasət pozuntularını tutun. İstənilən qəsdən TODO çörək qırıntılarını saxlayın
  böyük dəyişiklikləri səssizcə təxirə salmaq yerinə təqiblər.