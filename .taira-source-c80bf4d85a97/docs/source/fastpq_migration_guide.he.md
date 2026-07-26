---
lang: he
direction: rtl
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-04T10:50:53.613193+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! מדריך העברת ייצור FASTPQ

ספר ריצה זה מתאר כיצד לאמת את מבחן FASTPQ של ייצור Stage6.
הקצה האחורי של מציין המיקום הדטרמיניסטי הוסר כחלק מתוכנית ההעברה הזו.
זה משלים את התוכנית המבוית ב-`docs/source/fastpq_plan.md` ומניח שאתה כבר עוקב
מצב סביבת העבודה ב-`status.md`.

## קהל והיקף
- מפעילי Validator המוציאים את מבחן הייצור בסביבות סטייג'ינג או Mainnet.
- שחרור מהנדסים שיוצרים קבצים בינאריים או קונטיינרים שישלחו עם קצה הייצור.
- צוותי SRE/צפיות חוטים אותות טלמטריה חדשים ומתריעים.

מחוץ לתחום: Kotodama עריכת חוזה ושינויי ABI IVM (ראה `docs/source/nexus.md` עבור
מודל ביצוע).

## מטריצת תכונות
| נתיב | תכונות מטען להפעלת | תוצאה | מתי להשתמש |
| ---- | ---------------------------- | ------ | ----------- |
| מוכיח ייצור (ברירת מחדל) | _אין_ | Stage6 FASTPQ backend עם מתכנן FFT/LDE וצינור DEEP-FRI.【ארגזים/fastpq_prover/src/backend.rs:1144】 | ברירת מחדל עבור כל קבצי הייצור הבינאריים. |
| האצת GPU אופציונלית | `fastpq_prover/fastpq-gpu` | מאפשר גרעיני CUDA/מתכת עם חילופי מעבד אוטומטי.【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 | מארחים עם מאיצים נתמכים. |

## נוהל בנייה
1. **בנייה למעבד בלבד**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   ה-backend של הייצור מורכב כברירת מחדל; אין צורך בתכונות נוספות.

2. **מבנה התומך ב-GPU (אופציונלי)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   תמיכת GPU דורשת ערכת כלים SM80+ CUDA עם `nvcc` זמין במהלך הבנייה.【ארגזים/fastpq_prover/Cargo.toml:11】

3. **בדיקות עצמיות**
   ```bash
   cargo test -p fastpq_prover
   ```
   הפעל את זה פעם אחת לכל גירסה בנייה כדי לאשר את הנתיב Stage6 לפני האריזה.

### הכנת שרשרת כלי מתכת (macOS)
1. התקן את כלי שורת הפקודה Metal לפני הבנייה: `xcode-select --install` (אם חסרים כלי CLI) ו-`xcodebuild -downloadComponent MetalToolchain` כדי להביא את שרשרת הכלים של GPU. סקריפט הבנייה מפעיל ישירות את `xcrun metal`/`xcrun metallib` ויכשל במהירות אם הקבצים הבינאריים נעדרים.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121】
2. כדי לאמת את הצינור לפני CI, אתה יכול לשקף את סקריפט ה-build באופן מקומי:
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   כאשר זה מצליח הבנייה פולטת `FASTPQ_METAL_LIB=<path>`; זמן הריצה קורא את הערך הזה כדי לטעון את metallib באופן דטרמיניסטי.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3. הגדר `FASTPQ_SKIP_GPU_BUILD=1` בעת קומפילציה צולבת ללא שרשרת הכלים מתכת; ה-build מדפיס אזהרה והמתכנן נשאר בנתיב המעבד.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. צמתים חוזרים ל-CPU באופן אוטומטי אם מתכת אינה זמינה (מסגרת חסרה, GPU לא נתמך או `FASTPQ_METAL_LIB` ריק); סקריפט הבנייה מנקה את ה-env var והמתכנן רושם את השדרוג לאחור.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/05.rs:### רשימת בדיקה לפרסום (שלב 6)
שמור את כרטיס השחרור של FASTPQ חסום עד שכל פריט למטה יושלם ומצורף.

1. **מדדי הוכחה של תת-שנייה** — בדוק את ה-`fastpq_metal_bench_*.json` שנלכד לאחרונה
   אשר את הערך `benchmarks.operations` שבו `operation = "lde"` (והמשיקוף
   מדגם `report.operations`) מדווח על `gpu_mean_ms ≤ 950` עבור עומס העבודה של 20000 שורות (32768 מרופד
   שורות). לכידות מחוץ לתקרה דורשות שידורים חוזרים לפני שניתן לחתום על רשימת הבדיקה.
2. **מניפסט חתום** - הפעלה
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   כך שכרטיס השחרור נושא גם את המניפסט וגם את החתימה המנותקת שלו
   (`artifacts/fastpq_bench_manifest.sig`). סוקרים מאמתים את צמד התקציר/חתימה לפני
   קידום מהדורה.【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 מניפסט המטריצה (בנוי
   דרך `scripts/fastpq/capture_matrix.sh`) כבר מקודד את רצפת השורות של 20 אלף
   ניפוי באגים רגרסיה.
3. **קבצים מצורפים של ראיות** - העלה את ה-JSON benchmark של מתכת, יומן סטdout (או מעקב אחר מכשירים),
   פלטי מניפסט CUDA/Metal, והחתימה המנותקת לכרטיס השחרור. הערך ברשימת הבדיקה
   צריך לקשר לכל החפצים בתוספת טביעת האצבע של המפתח הציבורי המשמשת לחתימה, כך שביקורות במורד הזרם
   יכול להפעיל מחדש את שלב האימות.【artifacts/fastpq_benchmarks/README.md:65】### זרימת עבודה של אימות מתכת
1. לאחר בנייה התומכת ב-GPU, אשר את נקודות `FASTPQ_METAL_LIB` ב-`.metallib` (`echo $FASTPQ_METAL_LIB`) כדי שזמן הריצה יוכל לטעון אותו באופן דטרמיניסטי.【crates/fastpq_prover/build.rs:188】
2. הפעל את חבילת הזוגיות עם נתיבי GPU כפויים על:\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. הקצה האחורי יפעיל את ליבות המתכת וירשום חילופי CPU דטרמיניסטים אם הזיהוי נכשל.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
3. צלם דגימת ביצועים עבור לוחות מחוונים:\
   אתר את ספריית המתכת (`fd -g 'fastpq.metallib' target/release/build | head -n1`),
   ייצא אותו דרך `FASTPQ_METAL_LIB`, והפעל\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  פרופיל `fastpq-lane-balanced` הקנוני מרופד כעת כל לכידה ל-32,768 שורות (2¹⁵), כך שה-JSON נושא הן `rows` והן `padded_rows` יחד עם השהיית Metal LDE; הפעל מחדש את הלכידה אם `zero_fill` או הגדרות תור דוחפים את ה-GPU LDE מעבר ליעד של 950ms (<1s) במארחים מסדרת AppleM. שמור את ה-JSON/יומן המתקבל בארכיון לצד עדויות שחרור אחרות; זרימת העבודה הלילית של macOS מבצעת את אותה הפעלה ומעלה את פריטי האמנות שלה לשם השוואה.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  כאשר אתה צריך טלמטריה של פוסידון בלבד (למשל, כדי להקליט מעקב אחר מכשירים), הוסף את `--operation poseidon_hash_columns` לפקודה שלמעלה; הספסל עדיין יכבד את `FASTPQ_GPU=gpu`, יפלוט `metal_dispatch_queue.poseidon`, ויכלול את בלוק `poseidon_profiles` החדש כך שחבילת השחרור מתעדת במפורש את צוואר הבקבוק של פוסידון.
  העדויות כוללות כעת `zero_fill.{bytes,ms,queue_delta}` בתוספת `kernel_profiles` (לכל ליבה
  תפוסה, GB/s משוער וסטטיסטיקות משך זמן) כך שניתן לצייר גרף של יעילות GPU בלי
  עיבוד מחדש של עקבות גולמיות, ובלוק `twiddle_cache` (כניסות/חמצות + `before_ms`/`after_ms`)
  מוכיח שההעלאות של מטמון מטמון בתוקף. `--trace-dir` משיק מחדש את הרתמה מתחת
  `xcrun xctrace record` ו
  מאחסן קובץ `.trace` עם חותמת זמן לצד ה-JSON; אתה עדיין יכול לספק התאמה אישית
  `--trace-output` (עם `--trace-template` / `--trace-seconds` אופציונלי) בעת לכידה ל-
  מיקום/תבנית מותאמים אישית. ה-JSON מתעד `metal_trace_{template,seconds,output}` לצורך ביקורת.【ארגזים/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】לאחר כל לכידה הרץ `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json` כך שהפרסום נושא מטא נתונים מארח (כולל כעת `metadata.metal_trace`) עבור הלוח/חבילת ההתראה Grafana (`dashboards/grafana/fastpq_acceleration.json`, `dashboards/alerts/fastpq_acceleration_rules.yml`). הדוח נושא כעת אובייקט `speedup` לכל פעולה (`speedup.ratio`, `speedup.delta_ms`), העטיפה מעלה את `zero_fill_hotspots` (בתים, חביון, GB/s נגזר, ותור המתכת משטחים דלתות `zero_fill_hotspots` למונה דלתות `zero_fill_hotspots`), `benchmarks.kernel_summary`, שומר על בלוק `twiddle_cache` ללא פגע, מעתיק את הבלוק/סיכום החדש של `post_tile_dispatches` כדי שהבודקים יוכלו להוכיח שהגרעין הרב-מעבר רץ במהלך הלכידה, וכעת מסכם את הראיות של Poseidon microbench 50X לתוך soquote70NI. זמן השהייה scalar-vs-default מבלי לשנות את הדוח הגולמי. שער המניפסט קורא את אותו בלוק ודוחה חבילות ראיות ל-GPU שמשמיטות אותו, מה שמאלץ את האופרטורים לרענן לכידות בכל פעם שדילוג על הנתיב שלאחר הריצוף או מוגדר בצורה שגויה.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732/task/sq.8】】
  ליבת Poseidon2 Metal חולקת את אותם כפתורים: `FASTPQ_METAL_POSEIDON_LANES` (32–256, החזקה של שתיים) ו-`FASTPQ_METAL_POSEIDON_BATCH` (1–32 מצבים לכל נתיב) מאפשרים לך להצמיד את רוחב ההשקה ועבודה לכל נתיב מבלי לבנות מחדש; המארח משחיל את הערכים האלה דרך `PoseidonArgs` לפני כל שיגור. כברירת מחדל, זמן הריצה בודק את `MTLDevice::{is_low_power,is_headless,location}` כדי להטות מעבדי GPU נפרדים לכיוון השקות בשכבות VRAM (`256×24` כאשר ≥48GiB מדווחים, `256×20` ב-32GiB, `256×16`0000, כך שאם לא, נשארים בעוצמה נמוכה. `256×8` (וחלקי נתיב 128/64 ישנים יותר נצמדים ל-8/6 מצבים לכל נתיב), כך שרוב המפעילים לעולם אינם צריכים להגדיר את ה-env vars ידנית. `fastpq_metal_bench` מבצע את עצמו מחדש כעת תחת `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` ופולט בלוק `poseidon_microbench` שמתעד את שני פרופילי ההשקה בתוספת המהירות הנמדדת לעומת הנתיב הסקלרי, כך שחבילות שחרור יכולות להוכיח שהקרנל החדש אכן מכווץ I10000080, והוא כולל את I100NI870, `poseidon_pipeline` לחסום כך שראיות Stage7 לוכדות את כפתורי עומק/חפיפה של נתחים לצד שכבות התפוסה החדשות. השאר את ה-env לא מוגדר לריצות רגילות; הרתמה מנהלת את הביצוע מחדש באופן אוטומטי, מתעדת כשלים אם לכידת הילד לא יכולה לרוץ, ויוצאת מיד כאשר `FASTPQ_GPU=gpu` מוגדר אך אין קצה אחורי של GPU זמין, כך שכשלים שקטים של CPU לעולם לא יתגנבו לביצועים חפצי אמנות.【ארגזים/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【ארגזים/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】העטיפה דוחה לכידת פוסידון שחסרה את הדלתא של `metal_dispatch_queue.poseidon`, המונים המשותפים של `column_staging`, או חסימות הראיות של `poseidon_profiles`/`poseidon_microbench`, כך שהאופרטורים חייבים להיכשל לרענן את כל הלכידה או החפיפה כדי להוכיח את הלכידה החופפת scalar-vs-default speedup.【scripts/fastpq/wrap_benchmark.py:732】 כאשר אתה צריך JSON עצמאי עבור לוחות מחוונים או CI deltas, הפעל `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`; העוזר מקבל הן חפצים עטופים והן לכידות `fastpq_metal_bench*.json` גולמיות, פולט `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` עם תזמוני ברירת המחדל/סקלרים, כוונון מטא נתונים ומהירות מוקלטת.【scripts/fastpq/export_poseidon_microbench.
  סיים את הריצה על ידי ביצוע `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` כך שרשימת הבדיקה של Stage6 תאכוף את תקרת `<1 s` LDE ופולטת חבילת מניפסט/עיכוב חתומה שנשלחת עם הגרסה כרטיס.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
4. ודא טלמטריה לפני השקה: סלסל את נקודת הקצה Prometheus (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) ובדוק יומני `telemetry::fastpq.execution_mode` עבור `resolved="cpu"` בלתי צפויים ערכים.【crates/iroha_telemetry/src/metrics.rs:8887】【ארגזים/fastpq_prover/src/backend.rs:174】
5. תיעד את נתיב החזרה של המעבד על ידי כפייתו בכוונה (`FASTPQ_GPU=cpu` או `zk.fastpq.execution_mode = "cpu"`) כדי שספרי ההשמעות של SRE יישארו מיושרים עם הדטרמיניסטית התנהגות.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】6. כוונון אופציונלי: כברירת מחדל, המארח בוחר 16 נתיבים לעקבות קצרות, 32 למסלולים קצרים, ו-64/128 פעם אחת `log_len ≥ 10/14`, נוחת ב-256 כאשר `log_len ≥ 18`, וכעת הוא שומר את אריח הזיכרון המשותף בחמישה שלבים, 001 ,011 ו-4 שלבים קטנים, 12/14/16 שלבים עבור `log_len ≥ 18/20/22` לפני בעיטת עבודה לגרעין שלאחר הריצוף. ייצא את `FASTPQ_METAL_FFT_LANES` (הספק של שניים בין 8 ל-256) ו/או `FASTPQ_METAL_FFT_TILE_STAGES` (1-16) לפני הפעלת השלבים שלמעלה כדי לעקוף את ההיוריסטיקות הללו. שני גדלי אצווה של עמודות FFT/IFFT ו-LDE נובעים מרוחב קבוצת השרשורים שנפתרו (≈2048 שרשורים לוגיים לשגרה, מוגבלים ל-32 עמודות, וכעת נחלצים מטה דרך 32→16→8→4→2→1 ככל שהתחום גדל) בזמן שנתיב ה-LDE עדיין אוכף את מכסי התחום שלו; הגדר את `FASTPQ_METAL_FFT_COLUMNS` (1–32) להצמדת גודל אצווה דטרמיניסטי ואת `FASTPQ_METAL_LDE_COLUMNS` (1–32) כדי להחיל את אותה עקיפה על שולח LDE כאשר אתה זקוק להשוואות סיביות לסיביות בין מארחים. עומק אריחי ה-LDE משקף גם את היוריסטיות ה-FFT - עקבות עם `log₂ ≥ 18/20/22` פועלות רק 12/10/8 שלבי זיכרון משותף לפני מסירת הפרפרים הרחבים לגרעין שאחרי הריצוף - ואתה יכול לעקוף את המגבלה הזו דרך Prometheus (1–1323X). זמן הריצה משחיל את כל הערכים דרך ה-Args של ליבת Metal, מהדק דחיקות לא נתמכות ומתעד את הערכים שנפתרו כך שניסויים יישארו ניתנים לשחזור מבלי לבנות מחדש את ה-metallib; ה-benchmark JSON מציג הן את הכוונון שנפתר והן את תקציב המילוי האפס של המארח (`zero_fill.{bytes,ms,queue_delta}`) שנלכדו באמצעות נתונים סטטיסטיים של LDE, כך שדלתות התורים קשורות ישירות לכל לכידה, וכעת מוסיף בלוק `column_staging` (אצות מושטחות, flatten_ms, waiter_ms, verlap_ratio_ms, waiters הצג את המארח על ידי צינור מאגר כפול. כאשר ה-GPU מסרב לדווח על טלמטריית מילוי אפס, הרתמה מסנתזת כעת תזמון דטרמיניסטי מניקוי המאגר בצד המארח ומחדיר אותו לבלוק `zero_fill`, כך שעדויות שחרור לעולם לא נשלחות ללא field.【ארגזים/fastpq_prover/src/metal_config.rs:15】【ארגזים/fastpq_prover/src/metal.rs:742】【ארגזים/fastpq_prover/src/bin/fastpq_met al_bench.rs:575】【ארגזים/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【ארגזים/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. שליחת ריבוי תורים מתבצעת אוטומטית במחשבי Mac נפרדים: כאשר `Device::is_low_power()` מחזיר false או שהתקן מתכת מדווח על חריץ/מיקום חיצוני, המארח יוצר מופע של שני `MTLCommandQueue`, מאוורר רק ברגע שעומס העבודה נושא ≥16 עמודות (בגודל של עמודות-u) על פני עמודות-u-bin עקבות ארוכות מעסיקות את שני נתיבי ה-GPU מבלי לפגוע בדטרמיניזם. עוקף את המדיניות עם `FASTPQ_METAL_QUEUE_FANOUT` (1–4 תורים) ו-`FASTPQ_METAL_COLUMN_THRESHOLD` (מינימום עמודות סה"כ לפני המניפה) בכל פעם שאתה צריך לכידות הניתנות לשחזור בין מכונות; מבחני השוויון מאלצים את העקיפות הללו כך שמחשבי Mac מרובי-GPU יישארו מכוסים ונקודת השחרור/הסף שנפתרה נרשמה ליד עומק התור טלמטריה.【ארגזים/fastpq_prover/src/metal.rs:620】【ארגזים/fastpq_prover/src/metal.rs:900】【ארגזים/fastpq_prover/src/metal.rs:2254】### עדות לארכיון
| חפץ | לכידת | הערות |
|--------|--------|-------|
| חבילת `.metallib` | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` ו-`xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` ואחריהם `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` ו-`export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`. | מוכיח ש- Metal CLI/Toolchain הותקן ויצר ספרייה דטרמיניסטית עבור commit זה.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
| תמונת מצב של איכות הסביבה | `echo $FASTPQ_METAL_LIB` לאחר הבנייה; שמור על הנתיב המוחלט עם כרטיס השחרור שלך. | פלט ריק פירושו שמתכת הושבתה; רישום מסמכי הערך שנתיבי GPU נשארים זמינים על חפץ המשלוח.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| יומן זוגיות של GPU | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` ואחסן את קטע הקוד שמכיל את `backend="metal"` או את אזהרת השדרוג לאחור. | מדגים שגרעינים פועלים (או נופלים לאחור באופן דטרמיניסטי) לפני שאתה מקדם את הבנייה.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
| פלט בנצ'מרק | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; לעטוף ולחתום באמצעות `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`. | תקליט ה-JSON העטוף `speedup.ratio`, `speedup.delta_ms`, כוונון FFT, שורות מרופדות (32,768), מועשר `zero_fill`/`kernel_profiles`, ה-I001300 השטוח, ה-I001300X בלוקים `metal_dispatch_queue.poseidon`/`poseidon_profiles` (כשמשתמשים ב-`--operation poseidon_hash_columns`), והמטא-נתונים של המעקב כך שה-GPU LDE ממוצע נשאר ≤950ms ו-Poseidon נשאר <1s; שמור גם את החבילה וגם את חתימת ה-`.json.asc` שנוצרה עם כרטיס השחרור, כך שלוחות מחוונים ומבקרים יוכלו לאמת את החפץ מבלי להפעיל מחדש עומסי עבודה.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
| מניפסט ספסל | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | מאמת את שני חפצי ה-GPU, נכשל אם ממוצע ה-LDE שובר את תקרת `<1 s`, מתעד תקצירי BLAKE3/SHA-256 ופולט מניפסט חתום כך שרשימת הבדיקה לא תוכל להתקדם ללא אימות metrics.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】 |
| חבילת CUDA | הפעל את `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` על מארח המעבדה SM80, עטוף/חתום את ה-JSON לתוך `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json` (השתמש ב-`--label device_class=xeon-rtx-sm80` כדי שלוחות מחוונים יקלוט את המחלקה הנכונה), הוסף את הנתיב ל-`artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt`, ושמור את התאמה `.json`/`.asc` עם חפץ המתכת לפני יצירת המניפסט מחדש. ה-`fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` שנכנס לצ'ק-אין ממחיש את הפורמט המדויק של המבקרים מצפים לו.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】 |
| הוכחת טלמטריה | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` בתוספת יומן `telemetry::fastpq.execution_mode` שנפלט בעת ההפעלה. | מאשר את Prometheus/OTEL חשיפת `device_class="<matrix>", backend="metal"` (או יומן שדרוג לאחור) לפני הפעלת תעבורה.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src:17/backend.| תרגיל מעבד מאולץ | הפעל אצווה קצרה עם `FASTPQ_GPU=cpu` או `zk.fastpq.execution_mode = "cpu"` ולכד את יומן השדרוג לאחור. | שומר על ספרי הפעלה של SRE מיושרים עם נתיב החזרה הדטרמיניסטי למקרה שיש צורך בהחזרה לאחור באמצע ההפצה.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
| לכידת עקבות (אופציונלי) | חזור על בדיקת זוגיות עם `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` ושמור את עקבות המשלוח הנפלטת. | שומר עדויות תפוסה/קבוצת שרשורים עבור סקירות מאוחרות יותר ליצירת פרופיל ללא הפעלה חוזרת של אמות מידה.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

קבצי `fastpq_plan.*` הרב-לשוניים מתייחסים לרשימת הבדיקה הזו, כך שמפעילי הבמה והפקה עוקבים אחר אותו עקבות.【docs/source/fastpq_plan.md:1】

## מבנים הניתנים לשחזור
השתמש בזרימת העבודה של מיכל המוצמד כדי לייצר חפצי Stage6 הניתנים לשחזור:

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

סקריפט העזר בונה את תמונת שרשרת הכלים `rust:1.88.0-slim-bookworm` (ו-`nvidia/cuda:12.2.2-devel-ubuntu22.04` עבור GPU), מריץ את ה-build בתוך הקונטיינר, וכותב `manifest.json`, `sha256s.txt` ואת הקבצים הבינאריים הקומפיליים לפלט היעד מדריך.【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

עקיפות סביבתיות:
- `FASTPQ_RUST_IMAGE`, `FASTPQ_RUST_TOOLCHAIN` - הצמד בסיס/תג חלודה מפורש.
- `FASTPQ_CUDA_IMAGE` - החלף את בסיס ה-CUDA בעת ייצור חפצי GPU.
- `FASTPQ_CONTAINER_RUNTIME` - לכפות זמן ריצה ספציפי; ברירת המחדל `auto` מנסה `FASTPQ_CONTAINER_RUNTIME_FALLBACKS`.
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` - סדר העדפה מופרד בפסיקים לזיהוי אוטומטי בזמן ריצה (ברירת המחדל היא `docker,podman,nerdctl`).

## עדכוני תצורה
1. הגדר את מצב ביצוע זמן הריצה ב-TOML שלך:
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   הערך מנותח דרך `FastpqExecutionMode` ומושחל לתוך הקצה האחורי בעת ההפעלה.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. עוקף בעת ההשקה במידת הצורך:
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   עוקפות CLI גורמות למוטציה בתצורה שנפתרה לפני האתחולים של הצומת.【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. מפתחים יכולים לכפות באופן זמני זיהוי מבלי לגעת בהגדרות על ידי ייצוא
   `FASTPQ_GPU={auto,cpu,gpu}` לפני השקת הבינארי; העקיפה מתועדת והצינור
   עדיין מציג את המצב שנפתר.【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## רשימת רשימת אימות
1. **יומני הפעלה**
   - צפו ל-`FASTPQ execution mode resolved` מהיעד `telemetry::fastpq.execution_mode` עם
     תוויות `requested`, `resolved` ו-`backend`.【crates/fastpq_prover/src/backend.rs:208】
   - בזיהוי GPU אוטומטי יומן משני מ-`fastpq::planner` מדווח על הנתיב הסופי.
   - מתכת מארח את משטח `backend="metal"` כאשר ה-metallib נטען בהצלחה; אם ההידור או הטעינה נכשלים, סקריפט ה-build פולט אזהרה, מנקה את `FASTPQ_METAL_LIB`, והמתכנן מתעד `GPU acceleration unavailable` לפני שהוא נשאר CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover/fastpq_prover】rpq_4prover2. **מדדי Prometheus**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   המונה מוגדל באמצעות `record_fastpq_execution_mode` (כעת מסומן על ידי
   `{device_class,chip_family,gpu_kind}`) בכל פעם שצומת פותר את הביצוע שלו
   מצב.【ארגזים/iroha_telemetry/src/metrics.rs:8887】
   - לכיסוי מתכת אשר
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     גדלים לצד לוחות המחוונים של הפריסה שלך.【ארגזים/iroha_telemetry/src/metrics.rs:5397】
   - צמתי macOS הידור עם `irohad --features fastpq-gpu` חושפים בנוסף
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     ו
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` אז לוחות המחוונים של Stage7
     יכול לעקוב אחר מחזור העבודה ומרווח התורים משריטות חיים של Prometheus.【ארגזים/iroha_telemetry/src/metrics.rs:4436】【ארגזים/irohad/src/main.rs:2345】

3. **ייצוא טלמטריה**
   - OTEL בונה פולט `fastpq.execution_mode_resolutions_total` עם אותן תוויות; להבטיח שלך
     לוחות מחוונים או התראות צופים ב-`resolved="cpu"` בלתי צפוי כאשר GPUs צריכים להיות פעילים.

4. **שפיות הוכחה/אמת**
   - הפעל אצווה קטנה דרך `iroha_cli` או רתמת אינטגרציה ואשר הוכחות לאימות על
     עמית הידור עם אותם פרמטרים.

## פתרון בעיות
- **מצב פתור נשאר CPU במארחי GPU** - בדוק שהבינארי נבנה עם
  `fastpq_prover/fastpq-gpu`, ספריות CUDA נמצאות בנתיב הטעינה, ו-`FASTPQ_GPU` לא מכריח
  `cpu`.
- **מתכת לא זמינה ב-Apple Silicon** - ודא שכלי ה-CLI מותקנים (`xcode-select --install`), הפעל מחדש את `xcodebuild -downloadComponent MetalToolchain`, והבטח שה-build יצר נתיב `FASTPQ_METAL_LIB` לא ריק; ערך ריק או חסר משבית את הקצה האחורי לפי התכנון.【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **`Unknown parameter` שגיאות** - ודא שגם המוכיח וגם המאמת משתמשים באותו קטלוג קנוני
  נפלט על ידי `fastpq_isi`; לא מתאים למשטח כמו `Error::UnknownParameter`.【ארגזים/fastpq_prover/src/proof.rs:133】
- **חזרה לא צפויה של CPU** - בדוק את `cargo tree -p fastpq_prover --features` ו
  אשר ש-`fastpq_prover/fastpq-gpu` קיים בבניית GPU; ודא שספריות `nvcc`/CUDA נמצאות בנתיב החיפוש.
- **מונה טלמטריה חסר** - ודא שהצומת הופעל עם `--features telemetry` (ברירת מחדל)
  ויצוא OTEL (אם מופעל) כולל את הצינור המטרי.【crates/iroha_telemetry/src/metrics.rs:8887】

## הליך נפילה
הקצה האחורי של מציין המיקום הדטרמיניסטי הוסר. אם רגרסיה דורשת החזרה לאחור,
לפרוס מחדש את חפצי השחרור שהיו ידועים בעבר ולחקור לפני ההנפקה מחדש של Stage6
בינאריים. תעדו את החלטת ניהול השינויים והבטיחו שהגלל קדימה יסתיים רק לאחר ה
רגרסיה מובנת.

3. ניטור טלמטריה כדי לוודא ש-`fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` משקף את הצפוי
   ביצוע מציין מקום.

## קו הבסיס של החומרה
| פרופיל | מעבד | GPU | הערות |
| ------- | --- | --- | ----- |
| הפניה (שלב 6) | AMD EPYC7B12 (32 ליבות), 256GiB RAM | NVIDIA A10040GB (CUDA12.2) | אצוות סינתטיות של 20000 שורות חייבות להשלים ≤1000ms.【docs/source/fastpq_plan.md:131】 |
| מעבד בלבד | ≥32 ליבות פיזיות, AVX2 | – | צפו ל-0.9-1.2 שניות ל-20000 שורות; שמור את `execution_mode = "cpu"` לדטרמיניזם. |## מבחני רגרסיה
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (על מארחי GPU)
- בדיקת מתקן זהוב אופציונלי:
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

תיעוד כל חריגה מרשימת הבדיקה הזו בספר ההפעלה שלך ועדכן את `status.md` לאחר
חלון ההגירה הושלם.