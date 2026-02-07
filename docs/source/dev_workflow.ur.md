---
lang: ur
direction: rtl
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-18T17:14:31.034360+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ایجنٹوں کی ترقیاتی ورک فلو

یہ رن بک ایجنٹوں کے روڈ میپ سے شراکت دار کے محافظوں کو مستحکم کرتی ہے
نئے پیچ ایک ہی ڈیفالٹ گیٹس کی پیروی کرتے ہیں۔

## کوئیک اسٹارٹ اہداف

- عملدرآمد کے لئے `make dev-workflow` (`scripts/dev_workflow.sh` کے ارد گرد ریپر) چلائیں:
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3. `cargo build --workspace --locked`
  4. `cargo test --workspace --locked`
  5. `swift test` `IrohaSwift/` سے
- `cargo test --workspace` طویل عرصے سے چل رہا ہے (اکثر گھنٹے)۔ فوری تکرار کے لئے ،
  `scripts/dev_workflow.sh --skip-tests` یا `--skip-swift` استعمال کریں ، پھر مکمل چلائیں
  شپنگ سے پہلے ترتیب۔
- اگر `cargo test --workspace` بلڈ ڈائریکٹری کے تالے پر اسٹالز ، کے ساتھ دوبارہ چلائیں
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (یا سیٹ
  `CARGO_TARGET_DIR` ایک الگ تھلگ راستے پر) تنازعہ سے بچنے کے لئے۔
- تمام کارگو اقدامات `--locked` کو برقرار رکھنے کی ذخیرہ پالیسی کا احترام کرنے کے لئے استعمال کرتے ہیں
  `Cargo.lock` اچھوت۔ شامل کرنے کے بجائے موجودہ کریٹس کو بڑھانے کو ترجیح دیں
  ورک اسپیس کے نئے ممبران ؛ نیا کریٹ متعارف کروانے سے پہلے منظوری حاصل کریں۔

## گارڈریلز- `make check-agents-guardrails` (یا `ci/check_agents_guardrails.sh`) ناکام ہوجاتا ہے اگر a
  برانچ `Cargo.lock` میں ترمیم کرتی ہے ، ورک اسپیس کے نئے ممبروں کو متعارف کراتی ہے ، یا نیا شامل کرتی ہے
  انحصار اسکرپٹ میں ورکنگ ٹری اور `HEAD` کے مقابلے میں موازنہ کیا گیا ہے
  `origin/main` کے مطابق بطور ڈیفالٹ ؛ بیس کو اوور رائڈ کرنے کے لئے `AGENTS_BASE_REF=<ref>` سیٹ کریں۔
- `make check-dependency-discipline` (یا `ci/check_dependency_discipline.sh`)
  بیس کے خلاف `Cargo.toml` انحصار میں فرق اور نئے کریٹوں پر ناکام ہوجاتا ہے۔ سیٹ
  `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>` جان بوجھ کر تسلیم کرنا
  اضافے
- `make check-missing-docs` (یا `ci/check_missing_docs_guard.sh`) نئے بلاکس نئے
  `#[allow(missing_docs)]` اندراجات ، جھنڈے چھوئے کریٹ (قریب ترین `Cargo.toml`)
  جس کے `src/lib.rs`/`src/main.rs` میں کریٹ لیول `//!` دستاویزات کی کمی ہے ، اور نئے کو مسترد کرتا ہے
  بیس ریف سے متعلق `///` دستاویزات کے بغیر عوامی اشیاء ؛ سیٹ
  `MISSING_DOCS_GUARD_ALLOW=1` صرف جائزہ لینے والے کی منظوری کے ساتھ۔ گارڈ بھی
  تصدیق کرتا ہے کہ `docs/source/agents/missing_docs_inventory.{json,md}` تازہ ہے۔
  `python3 scripts/inventory_missing_docs.py` کے ساتھ دوبارہ تخلیق کریں۔
- `make check-tests-guard` (یا `ci/check_tests_guard.sh`) جھنڈے والے کریٹس
  زنگ کے افعال میں بدلا ہوا یونٹ ٹیسٹ کے ثبوت کی کمی ہے۔ گارڈ کے نقشوں نے لکیریں تبدیل کیں
  افعال میں ، گزرتا ہے اگر کریٹ ٹیسٹ مختلف میں تبدیل ہوئے ، اور بصورت دیگر اسکین
  ملاپ کے فنکشن کالز کے لئے موجودہ ٹیسٹ فائلیں اتنی پہلے سے موجود کوریج
  گنتی ؛ بغیر کسی مماثل ٹیسٹ کے کریٹس ناکام ہوجائیں گے۔ `TEST_GUARD_ALLOW=1` سیٹ کریں
  صرف اس صورت میں جب تبدیلیاں واقعی ٹیسٹ غیر جانبدار ہوں اور جائزہ لینے والا اس سے اتفاق کرتا ہے۔
- `make check-docs-tests-metrics` (یا `ci/check_docs_tests_metrics_guard.sh`)
  روڈ میپ کی پالیسی کو نافذ کرتا ہے جو سنگ میل دستاویزات کے ساتھ ساتھ حرکت کرتا ہے ،
  ٹیسٹ ، اور میٹرکس/ڈیش بورڈز۔ جب `roadmap.md` کے نسبت تبدیل ہوتا ہے
  `AGENTS_BASE_REF` ، گارڈ کو کم از کم ایک ڈاکٹر کی تبدیلی ، ایک ٹیسٹ میں تبدیلی کی توقع ہے ،
  اور ایک میٹرکس/ٹیلی میٹری/ڈیش بورڈ تبدیلی۔ `DOC_TEST_METRIC_GUARD_ALLOW=1` سیٹ کریں
  صرف جائزہ لینے والے کی منظوری کے ساتھ۔
- `make check-todo-guard` (یا `ci/check_todo_guard.sh`) جب ٹوڈو مارکروں میں ناکام ہوجاتا ہے
  دستاویزات/ٹیسٹوں میں تبدیلی کے بغیر غائب ہوجائیں۔ کوریج کو شامل کریں یا اپ ڈیٹ کریں
  جب کسی ٹوڈو کو حل کریں ، یا جان بوجھ کر ہٹانے کے ل I `TODO_GUARD_ALLOW=1` مرتب کریں۔
- `make check-std-only` (یا `ci/check_std_only.sh`) بلاکس `no_std`/`wasm32`
  CFGs تو ورک اسپیس صرف `std` صرف رہتا ہے۔ صرف `STD_ONLY_GUARD_ALLOW=1` سیٹ کریں
  منظور شدہ CI تجربات۔
- `make check-status-sync` (یا `ci/check_status_sync.sh`) روڈ میپ کو کھلا رکھتا ہے
  مکمل شدہ اشیا سے پاک سیکشن اور `roadmap.md`/`status.md` سے ضرورت ہے
  ایک ساتھ تبدیل کریں لہذا منصوبہ/حیثیت منسلک رہیں۔ سیٹ
  `STATUS_SYNC_ALLOW_UNPAIRED=1` صرف نایاب اسٹیٹس صرف ٹائپو فکس کے بعد
  `AGENTS_BASE_REF` کو پن کرنا۔
- `make check-proc-macro-ui` (یا `ci/check_proc_macro_ui.sh`) ٹربولڈ چلاتا ہے
  UI سویٹس اخذ کردہ/پروک میکرو کریٹس کے لئے۔ پروک میکروز کو چھوتے وقت اسے چلائیں
  `.stderr` تشخیص کو مستحکم رکھیں اور کوچ کو گھبرانے والے UI رجعتیں۔ سیٹ
  `PROC_MACRO_UI_CRATES="crate1 crate2"` مخصوص کریٹوں پر توجہ مرکوز کرنے کے لئے۔
- `make check-env-config-surface` (یا `ci/check_env_config_surface.sh`) دوبارہ تعمیر
  env-togle انوینٹری (`docs/source/agents/env_var_inventory.{json,md}`) ،
  اگر یہ باسی ہے تو ناکام ہوجاتا ہے ، ** اور ** ناکام ہوجاتا ہے جب نئی پروڈکشن ENV شیمس ظاہر ہوتی ہے
  `AGENTS_BASE_REF` (آٹو کا پتہ لگانے والا ؛ ضرورت پڑنے پر واضح طور پر سیٹ کریں) سے متعلق۔
  env تلاش کو شامل کرنے/ہٹانے کے بعد ٹریکر کو ریفریش کریں
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md` ؛
  جان بوجھ کر اینو نوبس کی دستاویز کرنے کے بعد ہی `ENV_CONFIG_GUARD_ALLOW=1` استعمال کریںہجرت ٹریکر میں۔
- `make check-serde-guard` (یا `ci/check_serde_guard.sh`) سیرڈ کو دوبارہ تخلیق کرتا ہے
  استعمال کی انوینٹری (`docs/source/norito_json_inventory.{json,md}`) ایک عارضی میں
  مقام ، اگر پرعزم انوینٹری باسی ہو تو ناکام ہوجاتا ہے ، اور کسی بھی نئے کو مسترد کرتا ہے
  پروڈکشن `serde`/`serde_json` `AGENTS_BASE_REF` کے نسبت سے ٹکرا جاتا ہے۔ سیٹ
  `SERDE_GUARD_ALLOW=1` صرف ہجرت کا منصوبہ داخل کرنے کے بعد CI تجربات کے لئے۔
- `make guards` Norito سیریلائزیشن پالیسی کو نافذ کرتا ہے: یہ نئی سے انکار کرتا ہے
  `serde`/`serde_json` استعمال ، ایڈہاک AOS مددگار ، اور باہر پیمانے پر انحصار
  Norito بنچ (`scripts/deny_serde_json.sh` ،
  `scripts/check_no_direct_serde.sh` ، `scripts/deny_handrolled_aos.sh` ،
  `scripts/check_no_scale.sh`)۔
-** پروک میکرو UI پالیسی: ** ہر پروک میکرو کریٹ کو لازمی طور پر ایک `trybuild` بھیجنا چاہئے
  `trybuild-tests` کے پیچھے کنٹرول (پاس/فیل گلوبز کے ساتھ `tests/ui.rs`)
  خصوصیت `tests/ui/pass` کے تحت ہیپی پاتھ کے نمونے رکھیں ، مسترد ہونے والے معاملات کے تحت
  `tests/ui/fail` پرعزم `.stderr` آؤٹ پٹ کے ساتھ ، اور تشخیص کو برقرار رکھیں
  غیر ہنسی اور مستحکم۔ کے ساتھ فکسچر ریفریش کریں
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (اختیاری کے ساتھ
  `CARGO_TARGET_DIR=target-codex` موجودہ تعمیرات سے بچنے کے لئے) اور
  کوریج بلڈز پر انحصار کرنے سے گریز کریں (`cfg(not(coverage))` گارڈز متوقع ہیں)۔
  میکروز کے لئے جو بائنری انٹری پوائنٹ کو خارج نہیں کرتے ہیں ، ترجیح دیں
  `// compile-flags: --crate-type lib` غلطیوں کو مرکوز رکھنے کے لئے فکسچر میں۔ شامل کریں
  جب بھی تشخیص میں تبدیلی آتی ہے تو نئے منفی معاملات۔
- CI `.github/workflows/agents-guardrails.yml` کے ذریعے گارڈریل اسکرپٹس چلاتا ہے
  لہذا جب پالیسیوں کی خلاف ورزی ہوتی ہے تو پل کی درخواستیں تیزی سے ناکام ہوجاتی ہیں۔
- نمونہ گٹ ہک (`hooks/pre-commit.sample`) گارڈریل ، انحصار ،
  لاپتہ دستاویزات ، صرف ایس ٹی ڈی ، این این وی کنفیگ ، اور اسٹیٹس سنک اسکرپٹ تو شراکت کار
  سی آئی سے پہلے پالیسی کی خلاف ورزیوں کو پکڑیں۔ کسی بھی جان بوجھ کر ٹوڈو بریڈ کرمبس رکھیں
  بڑی تبدیلیوں کو خاموشی سے موخر کرنے کے بجائے فالو اپ۔