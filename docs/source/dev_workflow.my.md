---
lang: my
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-19T07:43:34.047491+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# AgentS Development Workflow

ဤအပြေးစာအုပ်သည် Agents လမ်းပြမြေပုံမှ ပံ့ပိုးကူညီသူ အကာအကွယ်လမ်းများကို စုစည်းထားသည်။
ဖာထေးမှုအသစ်များသည် တူညီသောမူလဂိတ်များကို လိုက်နာသည်။

## အမြန်စတင်ရန် ပစ်မှတ်များ

- လုပ်ဆောင်ရန် `make dev-workflow` (`scripts/dev_workflow.sh` ပတ်ပတ်လည်) ကို run ပါ။
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3. `cargo build --workspace --locked`
  4. `cargo test --workspace --locked`
  5. `swift test` မှ `IrohaSwift/`
- `cargo test --workspace` သည် ကြာရှည် (မကြာခဏ နာရီ) ဖြစ်သည်။ မြန်မြန်ဆန်ဆန်၊
  `scripts/dev_workflow.sh --skip-tests` သို့မဟုတ် `--skip-swift` ကိုသုံးပါ၊ ထို့နောက် အပြည့်အစုံကို run
  ပို့ဆောင်ခြင်းမပြုမီ sequence
- အကယ်၍ `cargo test --workspace` သည် build directory locks တွင် ရပ်တန့်ပါက၊ ဖြင့် ပြန်ဖွင့်ပါ။
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (သို့မဟုတ် သတ်မှတ်
  `CARGO_TARGET_DIR` သည် သီးခြားလမ်းကြောင်းတစ်ခုသို့) ငြင်းခုံခြင်းကို ရှောင်ရှားရန်။
- သိမ်းဆည်းခြင်းဆိုင်ရာ မူဝါဒကို လေးစားရန် ကုန်စည်တင်သည့် အဆင့်အားလုံးသည် `--locked` ကို အသုံးပြုပါသည်။
  `Cargo.lock` မထိရသေးပါ။ ထည့်မည့်အစား ရှိပြီးသားသေတ္တာများကို တိုးချဲ့ခြင်းကို ဦးစားပေးပါ။
  အလုပ်ခွင်အဖွဲ့ဝင်အသစ်များ၊ သေတ္တာအသစ်တစ်လုံးကို မမိတ်ဆက်မီ အတည်ပြုချက်ရယူပါ။

## အကာအရံများ- `make check-agents-guardrails` (သို့မဟုတ် `ci/check_agents_guardrails.sh`) ပျက်ကွက်ပါက၊
  ဌာနခွဲသည် `Cargo.lock` ကို မွမ်းမံပြင်ဆင်သည်၊ အလုပ်ခွင်အဖွဲ့ဝင်အသစ်များကို မိတ်ဆက်ပေးသည် သို့မဟုတ် အသစ်ထည့်သည်
  မှီခိုမှု။ script သည် အလုပ်လုပ်သောသစ်ပင်နှင့် `HEAD` ကို နှိုင်းယှဉ်ထားသည်။
  မူရင်းအားဖြင့် `origin/main`; အခြေခံကို အစားထိုးရန် `AGENTS_BASE_REF=<ref>` ကို သတ်မှတ်ပါ။
- `make check-dependency-discipline` (သို့မဟုတ် `ci/check_dependency_discipline.sh`)
  အခြေခံနှင့် `Cargo.toml` မှီခိုမှုများ ကွဲပြားပြီး သေတ္တာအသစ်များတွင် ပျက်ကွက်သည်။ သတ်မှတ်
  ရည်ရွယ်ချက်ရှိရှိ အသိအမှတ်ပြုရန် `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>`
  ထပ်လောင်း။
- `make check-missing-docs` (သို့မဟုတ် `ci/check_missing_docs_guard.sh`) တုံးအသစ်များ
  `#[allow(missing_docs)]` ထည့်သွင်းမှုများ၊ ထိထားသော အလံများ (အနီးဆုံး `Cargo.toml`)
  `src/lib.rs`/`src/main.rs` သည် crate-level `//!` docs မရှိ၍ အသစ်ကို ငြင်းပယ်သည်
  `///` docs မပါဘဲ အများသူငှာ ပစ္စည်းများ၊ သတ်မှတ်
  သုံးသပ်သူ၏ခွင့်ပြုချက်ဖြင့်သာ `MISSING_DOCS_GUARD_ALLOW=1`။ ကိုယ်ရံတော်တွေလည်း ပါတယ်။
  `docs/source/agents/missing_docs_inventory.{json,md}` သည် လတ်ဆတ်ကြောင်း အတည်ပြုသည်။
  `python3 scripts/inventory_missing_docs.py` ဖြင့် ပြန်ထုတ်ပါ။
- `make check-tests-guard` (သို့မဟုတ် `ci/check_tests_guard.sh`) အလံပုံးများ
  ပြောင်းလဲထားသော Rust လုပ်ငန်းဆောင်တာများသည် ယူနစ်စမ်းသပ်မှု အထောက်အထားမရှိခြင်း။ အစောင့်မြေပုံများသည် မျဉ်းကြောင်းများပြောင်းသွားသည်။
  လုပ်ငန်းဆောင်တာများဆီသို့၊ diff တွင် crate tests များပြောင်းလဲသွားပါက၊ နှင့် အခြားနည်းဖြင့် scan လုပ်ပါ။
  ကိုက်ညီသည့် လုပ်ဆောင်ချက်ခေါ်ဆိုမှုများအတွက် ရှိပြီးသား စမ်းသပ်ဖိုင်များ ဖြစ်သောကြောင့် နဂိုရှိပြီးသား အကျုံးဝင်ပါသည်။
  အရေအတွက်များ; ကိုက်ညီသောစမ်းသပ်မှုမရှိဘဲသေတ္တာများပျက်လိမ့်မည်။ `TEST_GUARD_ALLOW=1` သတ်မှတ်ပါ။
  အပြောင်းအလဲများသည် အမှန်တကယ် စမ်းသပ်ကြားနေဖြစ်ပြီး ဝေဖန်သုံးသပ်သူ သဘောတူမှသာ၊
- `make check-docs-tests-metrics` (သို့မဟုတ် `ci/check_docs_tests_metrics_guard.sh`)
  မှတ်တမ်းမှတ်ရာများနှင့်အတူ ရွေ့လျားနေသော လမ်းပြမြေပုံပေါ်လစီကို ပြဋ္ဌာန်း၊
  စမ်းသပ်မှုများ၊ တိုင်းတာမှုများ/ဒိုင်ခွက်များ။ `roadmap.md` သည် ပြောင်းလဲသောအခါ
  `AGENTS_BASE_REF`၊ အစောင့်သည် အနည်းဆုံး doc ပြောင်းလဲမှုတစ်ခု၊ စမ်းသပ်မှုတစ်ခု အပြောင်းအလဲကို မျှော်လင့်သည်၊
  နှင့် မက်ထရစ်များ / တယ်လီမီတာ / ဒက်ရှ်ဘုတ် ပြောင်းလဲမှု တစ်ခု။ `DOC_TEST_METRIC_GUARD_ALLOW=1` သတ်မှတ်ပါ။
  သုံးသပ်သူ၏ခွင့်ပြုချက်ဖြင့်သာ။
- TODO အမှတ်အသားများသည် `make check-todo-guard` (သို့မဟုတ် `ci/check_todo_guard.sh`) မအောင်မြင်ပါ။
  ပူးတွဲပါ docs/tests အပြောင်းအလဲများမရှိဘဲ ပျောက်သွားသည်။ အကျုံးဝင်မှုကို ထည့်ပါ သို့မဟုတ် အပ်ဒိတ်လုပ်ပါ။
  TODO ကို ဖြေရှင်းသည့်အခါ သို့မဟုတ် `TODO_GUARD_ALLOW=1` ကို ရည်ရွယ်ချက်ရှိရှိ ဖယ်ရှားခြင်းအတွက် သတ်မှတ်ပါ။
- `make check-std-only` (သို့မဟုတ် `ci/check_std_only.sh`) လုပ်ကွက် `no_std`/`wasm32`
  cfgs ထို့ကြောင့် အလုပ်ခွင်သည် `std`-only ဆက်နေသည်။ `STD_ONLY_GUARD_ALLOW=1` အတွက်သာ သတ်မှတ်ပါ။
  ပိတ်ဆို့အရေးယူထားသော CI စမ်းသပ်မှုများ။
- `make check-status-sync` (သို့မဟုတ် `ci/check_status_sync.sh`) သည် လမ်းပြမြေပုံကို ဖွင့်ပေးသည်
  ပြီးမြောက်သည့် အပိုင်းများ အခမဲ့ဖြစ်ပြီး `roadmap.md`/`status.md` လိုအပ်သည်
  အတူတကွပြောင်းလဲပါ ထို့ကြောင့် အစီအစဉ်/အခြေ အနေ လိုက်လျောညီထွေနေရန်၊ သတ်မှတ်
  `STATUS_SYNC_ALLOW_UNPAIRED=1` ပြီးနောက် ရှားပါးသော အခြေအနေ-သပ်သပ် typo fixes အတွက်သာ
  `AGENTS_BASE_REF` ကို ပင်ထိုးခြင်း။
- `make check-proc-macro-ui` (သို့မဟုတ် `ci/check_proc_macro_ui.sh`) trybuild ကို run သည်
  derive/proc-macro သေတ္တာများအတွက် UI အစုံများ။ proc-macro ကိုထိသောအခါ ၎င်းကိုဖွင့်ပါ။
  `.stderr` ရောဂါရှာဖွေမှုများကို တည်ငြိမ်စေပြီး ထိတ်လန့်နေသော UI ဆုတ်ယုတ်မှုများကို ဖမ်းဆုပ်ပါ။ သတ်မှတ်
  သီးခြားသေတ္တာများပေါ်တွင်အာရုံစိုက်ရန် `PROC_MACRO_UI_CRATES="crate1 crate2"`။
- `make check-env-config-surface` (သို့မဟုတ် `ci/check_env_config_surface.sh`) ပြန်လည်တည်ဆောက်သည်
  env-toggle inventory (`docs/source/agents/env_var_inventory.{json,md}`)၊
  ဟောင်းနွမ်းနေပါက၊ **နှင့်** ထုတ်လုပ်မှုအသစ် env shims ပေါ်လာသောအခါ ပျက်ကွက်သည်။
  `AGENTS_BASE_REF` နှင့် ဆက်စပ်မှု (အလိုအလျောက် ရှာဖွေတွေ့ရှိသည်၊ လိုအပ်သည့်အခါတွင် ပြတ်သားစွာ သတ်မှတ်ပါ)။
  env lookups များကို ထည့်သွင်း/ဖယ်ရှားပြီးနောက် ခြေရာခံကိရိယာကို ပြန်လည်စတင်ပါ။
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  ရည်ရွယ်ချက်ရှိရှိ env ခလုတ်များကို မှတ်တမ်းတင်ပြီးမှသာ `ENV_CONFIG_GUARD_ALLOW=1` ကို အသုံးပြုပါ။migration tracker တွင်။
- `make check-serde-guard` (သို့မဟုတ် `ci/check_serde_guard.sh`) သည် ဆာဒါကို ပြန်လည်ထုတ်ပေးသည်
  အသုံးပြုမှုစာရင်း (`docs/source/norito_json_inventory.{json,md}`) အပူချိန်သို့
  တည်နေရာ၊ ကတိပြုထားသောစာရင်းသည် ပျက်သွားပါက ပျက်ကွက်ပြီး အသစ်ကိုမဆို ငြင်းပယ်ပါ။
  ထုတ်လုပ်မှု `serde`/`serde_json` သည် `AGENTS_BASE_REF` နှင့် ဆက်စပ်မှုရှိသည်။ သတ်မှတ်
  `SERDE_GUARD_ALLOW=1` သည် ရွှေ့ပြောင်းခြင်းအစီအစဉ်ကို တင်ပြပြီးနောက် CI စမ်းသပ်မှုများအတွက်သာ။
- `make guards` သည် Norito အမှတ်စဉ်မူဝါဒကို ကျင့်သုံးသည်- အသစ်ကို ငြင်းပယ်သည်
  `serde`/`serde_json` အသုံးပြုမှု၊ ad-hoc AoS ကူညီပေးသူများ၊ နှင့် အပြင်ဘက် SCALE မှီခိုမှုများ
  Norito ထိုင်ခုံများ (`scripts/deny_serde_json.sh`၊
  `scripts/check_no_direct_serde.sh`, `scripts/deny_handrolled_aos.sh`၊
  `scripts/check_no_scale.sh`)။
- **Proc-macro UI မူဝါဒ-** proc-macro သေတ္တာတိုင်းသည် `trybuild` ကို ပေးပို့ရမည်
  `trybuild-tests` နောက်ကွယ်ရှိ ကြိုး (`tests/ui.rs`)
  ထူးခြားချက်။ ပျော်ရွှင်သောလမ်းကြောင်းနမူနာများကို `tests/ui/pass` အောက်တွင်၊ ငြင်းပယ်ခြင်းဆိုင်ရာကိစ္စရပ်များကို အောက်၌ထားပါ။
  `tests/ui/fail` ကတိကဝတ်ပြုထားသော `.stderr` outputs နှင့်အတူ၊ နှင့်ရောဂါရှာဖွေမှုများကိုစောင့်ရှောက်
  ထိတ်လန့်ခြင်းကင်းပြီး တည်ငြိမ်ခြင်း။ တွဲဖက်ပစ္စည်းများကို ပြန်လည်စတင်ပါ။
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (ရွေးချယ်နိုင်သည်။
  `CARGO_TARGET_DIR=target-codex`) နှင့် ရှိပြီးသား တည်ဆောက်မှုများကို တားဆီးရန်
  လွှမ်းခြုံတည်ဆောက်မှုများကို မှီခိုခြင်းမှ ရှောင်ကြဉ်ပါ (`cfg(not(coverage))` အစောင့်များ မျှော်လင့်ထားသည်)။
  binary entrypoint ကို ထုတ်မပေးသော မက်ခရိုများအတွက်၊ ပိုကြိုက်သည်။
  `// compile-flags: --crate-type lib` အမှားများကို အာရုံစိုက်ထားရန် ထည့်ပါ။
  ရောဂါရှာဖွေမှု ပြောင်းလဲသည့်အခါတိုင်း အပျက်သဘောဆောင်သော အမှုအသစ်များ။
- CI သည် `.github/workflows/agents-guardrails.yml` မှတဆင့် guardrail scripts များကိုလုပ်ဆောင်သည်။
  ထို့ကြောင့် မူဝါဒများကို ချိုးဖောက်သည့်အခါ တောင်းဆိုမှုများ မြန်မြန်ဆန်ဆန် မအောင်မြင်ပါ။
- နမူနာ git ချိတ် (`hooks/pre-commit.sample`) သည် guardrail၊ မှီခိုမှု၊
  ပျောက်ဆုံးနေသော-docs၊ std-only၊ env-config နှင့် status-sync scripts များဖြစ်တာကြောင့် ပံ့ပိုးကူညီသူများ
  CI မတိုင်မီ မူဝါဒချိုးဖောက်မှုများကို ဖမ်းပါ။ ရည်ရွယ်ချက်ရှိရှိ TODO ပေါင်မုန့်ကို သိမ်းထားပါ။
  ကြီးမားသောပြောင်းလဲမှုများကို တိတ်တဆိတ် ရွှေ့ဆိုင်းမည့်အစား နောက်ဆက်တွဲများ။