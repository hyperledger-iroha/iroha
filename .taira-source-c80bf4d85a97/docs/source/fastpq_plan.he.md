<!-- Hebrew translation of docs/source/fastpq_plan.md -->

---
lang: he
direction: rtl
source: docs/source/fastpq_plan.md
status: complete
translator: manual
---

<div dir="rtl">

# פירוט עבודה לפלובר FASTPQ

המסמך מפרט תכנית שלבים למסירת פלובר FASTPQ-ISI מוכן לפרודקשן ולחיבורו לצינור תזמון data-space. אלא אם צוין אחרת, ההגדרות נורמטיביות. אומדני החוסן מבוססים על גבולות DEEP-FRI בסגנון Cairo, ובדיקות rejection-sampling אוטומטיות ב-CI נכשלות אם ההערכה יורדת מתחת ל-128 ביט.

התכנית מסונכרנת תמיד עם סעיף “FASTPQ Production Track” ב-`roadmap.md`; שינויי סטטוס, שמות שלבים או יעדים חייבים להתעדכן בשני המסמכים יחד.

## בסיסים שהושלמו

### Backend מציין מקום *(הושלם)*
- קידוד Norito דטרמיניסטי עם BLAKE2b.
- הערה היסטורית: backend זמני סיפק ארטיפקטים דטרמיניסטיים מאחורי `fastpq-placeholder`; המסלול הוסר בשלב 6.
- טבלת פרמטרים קנונית מסופקת ב-`fastpq_isi`.

### אב-טיפוס Trace Builder *(הושלם ב-2025-11-09)*
> **סטטוס:** `fastpq_prover` מפרסם עזרי אריזה קנוניים (`pack_bytes`, `PackedBytes`) ו-commitment דטרמיניסטי של Poseidon2 מעל Goldilocks. הקבועים נעולים ל-commit `3f2b7fe` של `ark-poseidon2`. פיקטורות הזהב (`tests/fixtures/packing_roundtrip.json`, `tests/fixtures/ordering_hash.json`) מעגנות כעת את רגרסיית הבדיקות.

- כל שורת trace מכילה:
  - `key_limbs[i]`: מקטעים בגודל 7 בתים (LE) של מפתח קנוני.
  - `value_old_limbs[i]`, `value_new_limbs[i]`: אותה אריזה לערכים לפני/אחרי.
  - עמודות בורר: `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm`.
  - עמודות עזר: `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter`.
  - עמודות נכס: `asset_id_limbs[i]` (7 בתים LE).
  - עמודות SMT לכל רמה: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, ו-`neighbour_leaf` להוכחות אי-חברות.
  - עמודות מטא-נתונים: `dsid`, `slot`.
- **סידור דטרמיניסטי:** מיון יציב לפי `(key_bytes, op_rank, original_index)` ו-commitment עם Poseidon2 תחת תג `fastpq:v1:ordering`. קלט ה-hash מקודד כ-`[domain_len, domain_limbs…, payload_len, payload_limbs…]` (הארכת אורך ל-u64 כדי לשמר סיומות אפס).
- עדי lookup: עבור `s_perm = 1`, מחשבים `perm_hash = Poseidon2(role_id || permission_id || epoch_u64_le)` (מזהים באורך 32 בתים LE, epoch באורך 8 בתים).
- הבטחת אינווריאנטים: בלעדיות הבוררים, שימור נכסים, וקבועי `dsid`/`slot`.
- גודל trace: `N_trace = 2^k` (חסם עליון), וגודל הערכה `N_eval = N_trace * 2^b`.
- Fixtures ובדיקות:
  - בדיקות round-trip לאריזה (`fastpq_prover/tests/packing.rs`, `tests/fixtures/packing_roundtrip.json`).
  - בדיקת יציבות hash הסידור (`tests/fixtures/ordering_hash.json`).
  - Fixtures לבאצ’ים (`trace_transfer.json`, `trace_mint.json`, `trace_duplicate_update.json`).

#### סכמת עמודות AIR
| קבוצה | שמות | תיאור |
|-------|------|--------|
| Activity | `s_active` | 1 לשורות אמתיות, 0 לריפוד. |
| Main | `key_limbs[i]`, `value_old_limbs[i]`, `value_new_limbs[i]` | איברי Goldilocks (LE, ‎7 בתים). |
| Asset | `asset_id_limbs[i]` | מזהה נכס קנוני (LE, ‎7 בתים). |
| Selectors | `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm` | סכום הבוררים שווה ל-`s_active`; `s_perm` מייצג הענקת/שלילת תפקיד. |
| Auxiliary | `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter` | מידע עזר לבקרה ושימור. |
| SMT | `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` | קלט/פלט Poseidon2 לכל רמה + הוכחות אי-חברות. |
| Lookup | `perm_hash` | Hash Poseidon2 לטבלת הרשאות. |

כללי עדכון בכל רמה:
```
if path_bit_ℓ == 0:
    node_out_ℓ = Poseidon2(node_in_ℓ, sibling_ℓ)
else:
    node_out_ℓ = Poseidon2(sibling_ℓ, node_in_ℓ)
```
הכנסה: `(node_in_0 = 0, node_out_0 = value_new)`; מחיקה: `(node_in_0 = value_old, node_out_0 = 0)`. `neighbour_leaf` משמש להוכחת רווח ריק. בסיום מאמתים שה-hash הסופי שווה ל-`old_root`/`new_root`.

## שלבים עתידיים

### שלב 0 — רה-ארגון פרמטרים ותכנון *(בתהליך)*
- הרחבת `StarkParameterSet` עם `trace_log_size`, `trace_root`, `lde_log_size`, `lde_root`, `permutation_size`, `lookup_log_size`, `omega_coset`.
- גזירת גנרטורים באמצעות Poseidon:
  - תג דומיין: `fastpq:v1:domain_roots`.
  - גזירת `(trace_root, lde_root, omega_coset)` לכל סט ואימות פרימיטיביות (`2^log_size`).
  - תיעוד מלא בנספח B ועדכון `fastpq_isi/src/params.rs`.
- החלפת קבועים חקוקים (`GOLDILOCKS_PRIMITIVE_ROOT`) בתוצאות החדשות ב-`fastpq_prover::fft`.  
  פקודת גנרציה: `cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots`
- עדכון בדיקות `find_by_name` והוספת רגרסיות המשוות לטבלה.
- הפעלת הפיצ'ר `fastpq-prover-preview` כדי לאפשר בחירה הדרגתית במתכנן החדש.
- עדכון נספחים, `status.md` ו-`roadmap.md` במקביל.

| פרמטר | trace_log_size | trace_root | lde_log_size | lde_root | permutation_size | lookup_log_size | omega_coset |
|--------|----------------|------------|--------------|----------|------------------|-----------------|-------------|
| `fastpq-lane-balanced` | 14 | `0x79c0_926e_32a5_e647` | 17 | `0x3bdd_b7b6_4404_fc19` | 16,384 | 17 | `0x916f_c4aa_51c6_2011` |
| `fastpq-lane-latency`  | 14 | `0x2853_ec35_498f_58f5` | 18 | `0x74e8_2886_9ab7_331b` | 16,384 | 18 | `0x483d_36dc_097c_9ec2` |

הפיצ'ר `fastpq-prover-preview` מאפשר שילוב אסטרטגי של המתכנן החדש לצד backend מציין המקום.

### שלב 1 — תשתית FFT ולוגריית LDE *(בתהליך)*
- ✅ `Planner::new` מטמון את טבלאות הדומיין מהשלב הקודם (trace ו-LDE) ומבצע אימותי שורש/דו-אדיות לפני שימוש.
- ✅ `fft_columns`/`ifft_columns` מריצים FFT דטרמיניסטי באיטרציה עם Rayon ושומרים על סדר העמודות.
- ✅ `lde_columns` מכפילה את המקדמים בחזקות הקוסאט הקנונית לפני הערכה על הטבלאות המטומנות.
- ✅ Benchmarks של Criterion מודדים את אורך ה-trace הקנוני (2^16 כיום, עם ריצות יומיות על 32k) ומתרחבים אוטומטית לסטים נוספים בעתיד, עם יעד <150ms לעמודה.
- ✅ בדיקות round-trip, דטרמיניזם של טבלאות הדומיין והערכות LDE, ולוגים (`fastpq::planner`) ממשיכים לספק אבחון ביצועים.

### שלב 2 — טרנספורמציות עמודה/lookup *(בתהליך)*
- ✅ `trace_commitment` עובד על מקדמים במקום על שורות גולמיות.
- ✅ קומיטמנט עמודות עם Poseidon סטרימי.
- ✅ קומיטמנט שורות:
  - הערכת העמודות פעם אחת בדומיין LDE ושימוש חוזר להרכבת hash השורות.
  - החלפת `hash_trace_rows` בזרימת Poseidon על `[row_index, column_count, value_0, …]`.
- ✅ עדי lookup:
  - בניית וקטור עדים במרחב המקדמים והערכתו בדומיין LDE עבור הגרנד-פרודקט.
  - יישור `perm_root` לאותו תהליך קומיטמנט.
- בדיקות תאימות מול backend מציין מקום (עם feature flag).
- ✅ עדכון תיעוד/פיקסצ'רים:
  - הדוגמאות ב-`lookup_grand_product.md` ו-`smt_update.md` שודרגו לשיקוף hash של מוצר ה-lookup ועמודות ה-SMT לכל level.
  - נוספו לנספח C תגיות הטרנסקריפט וסדר העמודות.

### שלב 3 — מימוש DEEP-FRI *(לא החל)*
- לולאת קיפול:
  - קיפול פולינומיאלי באריות 8/16.
  - קומיטמנט ל-root כל שכבה עם Poseidon סטרימי ושמירתם בטרנסקריפט.
  - חישוב `U(X) = f(X) + β f(γX^arity + …)` לפי המפרט.
- טרנסקריפט:
  - מימוש `Transcript::append_fr_layer` ו-`challenge_beta`.
  - שמירת ערכי β לשימוש פלובר/וריפייר.
- hashing לשורשי עלים:
  - בניית מסלולי מרקל ב-batch.
  - בדיקות חיוביות/שליליות מול אימפלמנטציית ייחוס בסגנון Winterfell.

### שלב 4 — דגימת שאילתות ואימות *(בתהליך)*
- ✅ Sampler:
  - מימוש Fiat–Shamir על מצב הטרנסקריפט והמונה, עם סינון כפילויות, מיון והגבלת המדגם לאורך הדומיין.
- ✅ אימות:
  - וריפייר בונה מחדש את קומיטמנטי העמודות מתוך פלט ה-Planner ודוחה γ, גרנד-פרודקט או פתיחות שהושחתו – יחד עם בדיקות שליליות תואמות.
- ✅ Cross tests:
  - `crates/fastpq_prover/tests/transcript_replay.rs` צורכת את הפיקצ'ר `tests/fixtures/stage4_balanced_preview.bin` ומוודאת שהטרנסקריפט של Fiat–Shamir משוחזר דטרמיניסטית ב-backend המקדמי.
- עדכון תרשימים ב-`nexus.md`.

### שלב 5 — האצת GPU/SIMD *(בתהליך)*
- יעדים: LDE/NTT, Poseidon2, עצי מרקל, קיפול FRI.
- דטרמיניזם מלא: fast-math כבוי, השוואת תוצאות CPU/CUDA/Metal ב-CI.
- Backend Metal (Apple Silicon):
  - סקריפט ה-build מקמפל את `metal/kernels/ntt_stage.metal` ואת `metal/kernels/poseidon2.metal` בעזרת `xcrun metal`/`xcrun metallib` ומפיק `fastpq.metallib`. ודאו שה-Metal CLI מותקן (`xcode-select --install`, ואם צריך `xcodebuild -downloadComponent MetalToolchain`).【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:189】
  - בנייה ידנית (תואמת ל-`build.rs`) לטובת בדיקות CI או אריזה דטרמיניסטית:
    ```bash
    export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
    xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
    export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
    ```
    לאחר הקימפול מתקבל `FASTPQ_METAL_LIB=<path>` שעליו נשענת הטעינה בזמן ריצה.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:42】
  - בקרוס-קומפילציה ללא כלי Metal ניתן להגדיר `FASTPQ_SKIP_GPU_BUILD=1`; ההתרעה מתועדת והפלנר נשאר במסלול ה-CPU.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
  - ה-host מחשב מראש טבלת twiddle (ערך אחד לכל שלב, ‎8‎ בתים לכל שלב), מאחסן אותן במטמון לפי `(log_len, inverse)` וממחזר את ה-buffer לאורך כל הריצה במקום לבנות אותו מחדש בכל שיגור. דו״ח `fastpq_metal_bench` מוסיף בלוק `twiddle_cache` (hits/misses ו-`before_ms`/`after_ms`) כדי להראות כמה זמן host נחסך יחסית להעלאות הישנות.【crates/fastpq_prover/src/metal.rs:896】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:976】
  - לצורכי ניסוי ב-profiler ניתן להגדיר `FASTPQ_METAL_THREADGROUP=<width>`; ההשקה תכבד את הערך (באילוץ מגבלות ההתקן) ותוציא לוג כדי לעקוב אחר ההגדרה.【crates/fastpq_prover/src/metal.rs:321】
  - ניתן לכוונן את אריח ה-FFT עצמו: ברירת המחדל בוחרת 16 ליינים למסלולים קצרים, 32 כאשר ‎`log_len ≥ 6`‎, 64 כאשר ‎`log_len ≥ 10`‎, 128 כאשר ‎`log_len ≥ 14`‎ ו-256 כאשר ‎`log_len ≥ 18`‎, ומעבירה את עומק האריח מ-5 לשלבים 4 (מ-`log_len ≥ 12`) ואז נשארת ב-12/14/16 שלבים עבור `log_len ≥ 18/20/22` לפני המעבר לקרנל הפוסט-טייל. Override-ים דרך ‎`FASTPQ_METAL_FFT_LANES`‎ (חזקה של 2 בין ‎8‎ ל-‎256‎) ו-`FASTPQ_METAL_FFT_TILE_STAGES` (‎1–16‎) משאירים את הפרופיילינג דטרמיניסטי: הערכים עוברים דרך `FftArgs`, נחתכים לטווח הנתמך ומדווחים ב-log כך שאין צורך לבנות מחדש את ה-metallib.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:120】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:244】
  - גם גודל האצווה של עמודות FFT/IFFT ושל LDE נגזר כעת מרוחב ה-threadgroup שנבחר: ההוסט מכוון לכ־4,096 חוטים לוגיים לכל פקודת GPU, מאחד עד 64 עמודות בכל שיגור באמצעות אריח ה-“circular buffer”, ורק לאחר ש-`log₂(len)` עובר את ‎16/18/20/22‎ מתחילה ירידה של ‎64→32→16→8→4→2→1‎ עמודות כדי לשמור על נצילות גם במסלולים ארוכים. מנגנון ההסתגלות ממשיך להכפיל את רוחב האצווה עד שהמדידות מתקרבות ליעד של ≈2 ms, וכאשר דגימה חוצה את היעד ביותר מ־30% הוא חוצה את מספר העמודות בחצי באופן אוטומטי כך ששינויים ב־lane/tile שמייקרים את הקרנל חוזרים מיידית לטווח הבטוח בלי Override ידני. מסלול ה-Poseidon חולק את אותו מנגנון, ובלוק `metal_heuristics.batch_columns.poseidon` בדוח `fastpq_metal_bench` מתעד את מספר המצבים שנבחרו, ההגבלה המרבית, המדידה האחרונה ודגל ה-override כך שקל לשייך את טלמטריית עומק התור להגדרות Poseidon בפועל. Override דרך `FASTPQ_METAL_FFT_COLUMNS` (1–64) מאפשר לנעול את גודל האצווה ב-FFT, ו-`FASTPQ_METAL_LDE_COLUMNS` (1–64) עושה את אותו הדבר עבור אפיק ה-LDE. דו״ח הבנצ׳מרק מציג את `kernel_profiles.*.columns` בכל לכידה, כך שקל להשוות בין הרצות.【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/metal.rs:1402】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1284】
  - הפעלת multi-queue נעשית אוטומטית ב-Macs עם GPU ייעודי: ההוסט בודק `is_low_power`, `is_headless` ו-`location` כדי לזהות התקנים נשלפים, פותח כברירת מחדל שני `MTLCommandQueue`s, ומתחיל לחלק את אצוות העמודות בסיבוב מרגע שהעומס הכולל חוצה 16 עמודות (מוכפל במקדם הפאן-אאוט). הסמפור מנמיך את רף האכיפה ל-“שני פקודות בכל תור”, וטלמטריית התורים כוללת כעת גם `window_ms` ו-`busy_ratio` עבור הסמפור הכולל ולכל תור ב־`metal_dispatch_queue.queues[*]`, כך שאפשר להראות ששני התורים היו עסוקים יותר מ-‎50 %‎ מאותו חלון זמן. אפשר לאלץ ערכים דטרמיניסטיים דרך `FASTPQ_METAL_QUEUE_FANOUT` (1–4 תורים במקביל) ו-`FASTPQ_METAL_COLUMN_THRESHOLD` (סף עמודות לפני הפעלה), והבדיקות קובעות את שני המשתנים כדי לוודא שהריצות מרובות ה-GPU נשארות דטרמיניסטיות גם בלילה.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:871】
  - זיהוי ה-Metal מנסה כעת קודם כל את `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` (כולל `warm_up` של CoreGraphics במארחים headless) ורק במידה והזרועות האלה נכשלות חוזר ל-`system_profiler`. משתנה הסביבה `FASTPQ_DEBUG_METAL_ENUM=1` מדפיס את הפלט של שתי השיטות כך שאם `FASTPQ_GPU=gpu` עדיין חוזר ל-CPU יש הוכחה למה, ו-`fastpq_metal_bench` מפסיק מיד עם שגיאה כאשר override כזה מוגדר אבל לא נמצאה תאוצה. השילוב מצמצם את מקרי ה-“fallback השקט” שהוזכרו ב-WP2‑E ומוסיף את הלוגים + ההסבר לחבילת הבנצ׳מארק.【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/backend.rs:705】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】
  - מדידות Poseidon כבר לא מתייגות ריצות CPU כ-“GPU”: `hash_columns_gpu` מחזיר אם התאוצה באמת רצה, `measure_poseidon_gpu` משליך את הסמפלים ומתריע כאשר pipeline נופל חזרה ל-CPU, וילד ה-microbench (`FASTPQ_METAL_POSEIDON_MICRO_MODE`) נכשל מיד אם ה-GPU אינו זמין. המשמעות היא שהשדה `gpu_recorded` הופך ל-false בכל ריצת fallback, סיכומי התור ממשיכים להדגים את חלון המדידה, והסיכומים/דשבורדים מתריעים מיידית על הרגרסיה.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1123】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2200】
  - דו״ח `fastpq_metal_bench` מוסיף בלוק `device_profile` עם שם ההתקן, מזהה הרישום, דגלי `low_power`/`headless`, ה-location (built_in/slot/external), אינדיקציה אם מדובר ב‑GPU דיסקרטי, ערך `hw.model` ותווית ה-SoC (למשל “M3 Max”). כך דאשבורדי Stage 7 יכולים למיין לכידות לפי M4/M3 מול GPU דיסקרטי בלי לפענח שמות מארחים, והמידע נשמר לצד טלמטריית התורים כדי שכל חבילת ראיות תראה מאיזה צי נאסף הבנצ׳מרק.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2536】
  - חפיפת ה-FFT בין ה-host ל-GPU משתמשת כעת בצמד באפרים: כל עוד batch ‎n‎ משלים את ה-post-tiling על ה-GPU, ההוסט משטח את batch ‎n + 1‎ לבאפר השני ועוצר רק כאשר חייבים למחזר באפר. המימוש רושם כמה batches שוטחו ומהו היחס בין זמן השיטוח לזמן ההמתנה ל-GPU, ו-`fastpq_metal_bench` מוסיף `column_staging.{batches,flatten_ms,wait_ms,wait_ratio}` כך שניתן להוכיח במסמכי השחרור שה-host לא התבטל בין הדיספאצ'ים. גם מסלול Poseidon משתמש באותו צינור דו־מאגרי, ולכן ריצות `--operation poseidon_hash_columns` מקבלות את אותם נתוני `column_staging` ודלתאות התור ללא אינסטרומנטציה ייעודית.【crates/fastpq_prover/src/metal.rs:1006】【crates/fastpq_prover/src/metal.rs:1076】【crates/fastpq_prover/src/metal.rs:2028】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1778】
- קרנל Poseidon2 עבר לגרסת תפוסה גבוהה: כל threadgroup מעתיק את קבועי הסיבובים ואת מטריצת ה-MDS לזיכרון המקומי, מריץ את הסיבובים בפריסה מלאה ומעבד כמה מצבים לכל ליין כך שכל dispatch כולל לפחות ‎4,096‎ threads לוגיים. אפשר להצמיד את מספר הליינים ואת כמות המצבים לליין עם `FASTPQ_METAL_POSEIDON_LANES` (חזקה של 2 בין ‎32‎–‎256‎) ו-`FASTPQ_METAL_POSEIDON_BATCH` (1–32) ללא קומפילציה מחדש; ההשקות המתורגלות מוזרמות דרך `PoseidonArgs`. ההוסט רושם פעם אחת את `MTLDevice::{is_low_power,is_headless,location}` ומטה באופן אוטומטי GPU‑ים דיסקרטיים לפרופילים מדורגים לפי VRAM (`256×24` ל־≥‎48‎GiB, `256×20` ב־32 GiB, אחרת `256×16`) בזמן שמערכות SoC דלות‑צריכה נשארות על `256×8` (חומרה שמוגבלת ל־128/64 ליינים נשארת עם ‎8/6‎ מצבים לליין), כך שמתקבל עומק מנהרה גדול מ־16 ללא מגע במשתני סביבה. `fastpq_metal_bench` מריץ את עצמו שוב עם `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` ומוסיף בלוק `poseidon_microbench` שמראה את השיפור מול המסלול הסקלרי, ובנוסף נרשמים מוני `poseidon_pipeline` (`chunk_columns`, `pipe_depth`, `fallbacks`) כדי להוכיח את החפיפה על כל לכידה.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/trace.rs:299】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】
  - עומק האריח של ה-LDE מסונכרן עם ההיוריסטיקה של ה-FFT: עבור ‎`log₂(len) ≥ 18`‎ רק 12 שלבים רצים בזיכרון המשותף, ב-‎20‎ יורדים ל-10 שלבים וב-‎22‎ ומעלה ל-8 שלבים, וכל השאר נשלם בקרנל שלאחר האריח. ניתן לאלץ עומק דטרמיניסטי דרך `FASTPQ_METAL_LDE_TILE_STAGES` (1–32); הקריאה לקרנל הפוסט־טיילינג תתרחש רק כשההיוריסטיקה עצרה מוקדם, כך שספירת התורים והטלאמטריה נשארות דטרמיניסטיות.【crates/fastpq_prover/src/metal.rs:827】
  - דוח ה-bench מוסיף כעת `post_tile_dispatches` שמסכם כמה קריאות FFT/IFFT/LDE רצו בקרנל שלאחר האריח (כולל log₂ השלב שממנו ממשיכים). `scripts/fastpq/wrap_benchmark.py` משכפל את הבלוק ל-`benchmarks.post_tile_dispatches` + `post_tile_summary`, ו-`cargo xtask fastpq-bench-manifest` נכשל אם ראיה זו חסרה כך שכל לכידה של ‎20k‎ שורות מוכיחה שהקרנל הרב-שלבי רץ על ה-GPU.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:255】【xtask/src/fastpq.rs:280】
  - כפל ה-coset משולב בתוך שלב ה-FFT האחרון וכבר איננו דורש מעבר נוסף על הבאפר.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:193】
  - מסלול ה-FFT/LDE מבוסס ה-shared-memory נעצר עכשיו לאחר עומק ה-tile, והשלבים שנותרו (כולל הנרמול של IFFT) עוברים לקרנל ייעודי `fastpq_fft_post_tiling`. ה-host של Rust שולח את אותן עמודות גם לפס הזה ורק כאשר `log_len` גדול ממגבלת האריח, כך שטלמטריית עומק התור ו-`kernel_profiles` נשארות דטרמיניסטיות בזמן שה-GPU מריץ את שלבי הסיום על ההתקן.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:654】
  - קרנל ה-LDE מניח שהבאפר של הערכות מאופס מראש ב-host. בעת מיחזור באפרים יש להחזיר אותם לאפס לפני ההרצה.【crates/fastpq_prover/src/metal.rs:233】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:141】
  - רפרנס הקרנלים הרשמי (שם הקרנל, מגבלת threadgroup, מגבלת שלבים והוראות הידור `fastpq.metallib`) נמצא עכשיו ב-`docs/source/fastpq_metal_kernels.md` כדי שמפעילים ואודיטורים יוכלו לאמת את ה-MSL המהודק מול הקוד המקור.【docs/source/fastpq_metal_kernels.md:1】
  - `FASTPQ_METAL_TRACE=1` מפיק לוגי debug על כל dispatch (שם הקרנל, גודל קבוצה, מספר קבוצות, זמן ריצה) כדי לסנכרן עם Metal System Trace.【crates/fastpq_prover/src/metal.rs:346】
  - תור ה־Metal מוגבל ע״י `FASTPQ_METAL_MAX_IN_FLIGHT`, כאשר ברירת המחדל נגזרת ממספר ליבות ה-GPU שהמערכת מדווחת דרך `system_profiler` (עם ריצת נסיגה לאותן מגבלות CPU אם macOS מסרבת לענות) ונחתכת מול סף הפאן-אאוט כדי לשמור על שני תורים עסוקים. הלוג של `fastpq_metal_bench` מוסיף לצד `metal_dispatch_queue` גם בלוק `metal_heuristics` המתעד את ההגבלה שנגזרה ואת רוחב האצווה ל-FFT/LDE (כולל סימון אם Override כפה את הערך), וריצות ממוקדות (`FASTPQ_METAL_TRACE=1 --operation poseidon_hash_columns`) מוסיפות תת-בלוק `metal_dispatch_queue.poseidon` עם הדלתא הייעודית. בנוסף, נתוני הקרנל מתכנסים לבלוק `poseidon_profiles` שמדגיש את זמן/בייט/אוקופנסי של Poseidon כך שהמעקב אחרי צוואר הבקבוק לא דורש ניתוח נוסף. אם גם אחרי ריצת הפרוב מדדי ה-zero-fill או התור חסרים, ההרצה מסנתזת מדידת איפוס מארח דטרמיניסטית ומזריקה אותה לבלוק `zero_fill` כך שהראיות אינן ריקות.【crates/fastpq_prover/src/metal.rs:2056】【crates/fastpq_prover/src/metal.rs:247】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1524】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2078】
  - זיהוי הריצה בודק זמינות דרך `system_profiler`; אם המסגרת, ההתקן או ה-metallib חסרים, הסקריפט מאפס את `FASTPQ_METAL_LIB` והפלנר מדווח על פולבק ל-CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】【crates/fastpq_prover/src/metal.rs:43】
  - צ'קליסט למפעילי Metal:
    1. אשרו שהכלים מותקנים וש-`FASTPQ_METAL_LIB` מצביע על `.metallib` שנבנה (`echo $FASTPQ_METAL_LIB` אמור להחזיר נתיב לאחר `cargo build --features fastpq-gpu`).【crates/fastpq_prover/build.rs:188】
    2. הריצו בדיקות פריטי עם GPU מאולץ: `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release`. הקרנלים יופעלו והמערכת תחזור אוטומטית ל-CPU אם הזיהוי נכשל.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
    3. אספו מדידת Benchmark לראיות: איתרו את קובץ ה-Metal שנבנה (`fd -g 'fastpq.metallib' target/release/build | head -n1`), הגדירו `FASTPQ_METAL_LIB=<הנתיב>` והריצו
       `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json`.
      `fastpq-lane-balanced` מרפד כעת כל ריצה ל-32,768 שורות, כך שה-JSON מציג גם את trace ה-20k המקורי וגם את הדומיין המורחב כדי שמבקרים יוודאו ש-LDE נשאר מתחת ל-950ms (יעד `<1s` על Apple M-series). חריגה מהסף דורשת לבדוק queue depth/zero-fill ולתעד ריצה חדשה. שמרו את ה-JSON/לוג; ה-nightly של macOS מריץ את אותה פקודה ומעלה את הקבצים לצרכים תפעוליים. הפלט כולל כעת `fft_tuning` (מספר הליינים וגבול השלבים), `speedup` לכל פעולה, שדה `zero_fill.{bytes,ms,queue_delta}` ב-LDE כדי לכמת את עלות האיפוס המארח ולתעד את מונה התור (limit/dispatch_count/max_in_flight/busy_ms/overlap_ms), וכן בלוק חדש `kernel_profiles` שמרכז לכל קרנל את יחס האוקופנסי, רוחב הפס המשוער וזמני הריצה כך שדאשבורדים יכולים לאתר רגרסיות GPU ללא עיבוד נוסף.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】 ניתן להוסיף `--trace-auto` (או להגדיר `--trace-output fastpq.trace` ולשנות את `--trace-template` / `--trace-seconds` לפי הצורך) כדי להריץ את הסשן דרך `xcrun xctrace record` (“Metal System Trace” כברירת מחדל) וכך לייצר bundle של Instruments; דוח ה-JSON יסמן זאת בשדות `metal_trace_{template,seconds,output}`.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】
      *(EN note: wrap the JSON עם `python3 scripts/fastpq/wrap_benchmark.py … --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json --sign-output` so the bundle enforces the <950 ms LDE target, the <1 s Poseidon target, embeds the row-usage snapshot, surfaces the `benchmarks.poseidon_microbench` summary, and produces a detached signature for Grafana `fastpq_acceleration` ו-`dashboards/alerts/fastpq_acceleration_rules.yml`.)*
      *(EN follow-up: when you need a standalone Poseidon microbench artefact for dashboards, run
      `python3 scripts/fastpq/export_poseidon_microbench.py --bundle artifacts/fastpq_benchmarks/<metal>.json`
      to drop a summary under `benchmarks/poseidon/poseidon_microbench_<timestamp>.json`; the helper
      exported retroactively. Refresh the aggregated manifest via
      `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`.)*
      הפלט כולל כעת את השדות `speedup.ratio` ו-`speedup.delta_ms` לכל פעולה כדי להוכיח את יתרון ה-GPU מול ה-CPU בלי לנתח שוב את המדידות.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:521】
      ה-wrapper מוסיף גם `zero_fill_hotspots` (בתים, זמן מילוי, GB/s מחושבים + queue_delta) ו-`kernel_profiles` כך שאפשר לזהות צווארי בקבוק של padding, שימוש בתור וגזרת occupancy/רוחב פס של הקרנלים במבט אחד בלי לצלול ל-JSON הגולמי, ואם מספקים `--row-usage` הוא משחיל את ההוכחה לתוך `metadata.row_usage_snapshot`. בנוסף נוצר קובץ חתימה `.json.asc` שמצורף לראיות.【scripts/fastpq/wrap_benchmark.py:1】
    4. צרפו טלמטריית שימוש בשורות מתוך ExecWitness אמיתי כך שהדאשבורד יוכל להשוות בין גאדג׳ט ההעברה למסלול הישן. שלפו את ה-witness דרך Torii
      (`iroha_cli audit witness --binary --out exec.witness`) ופענחו אותו עם
      `iroha_cli audit witness --decode exec.witness` (אם רוצים לאמת את הפרמטר הצפוי, הוסיפו
      `--fastpq-parameter fastpq-lane-balanced`; אצוות FASTPQ נכללות כברירת מחדל; ניתן להשבית אותן
      רק אם צריך פלט מצומצם בעזרת `--no-fastpq-batches`).

       ```bash
       python3 scripts/fastpq/check_row_usage.py \
         --baseline artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json \
         --candidate fastpq_row_usage_2025-05-12.json \
         --max-transfer-ratio-increase 0.005 \
         --max-total-rows-increase 0
       ```

       ב-`scripts/fastpq/examples/` תמצאו דוגמאות JSON לסמוק-טסטים מהירים. מקומית אפשר להריץ
       `make check-fastpq-row-usage` (עוטף את `ci/check_fastpq_row_usage.sh`) ובסביבת ה-CI אותו סקריפט רץ דרך
        `.github/workflows/fastpq-row-usage.yml` כדי להשוות בין קבצי `artifacts/fastpq_benchmarks/fastpq_row_usage_*.json`
        וכך לגלות מיידית כל רגרסיה ב-`transfer_ratio` או במספר השורות הכולל. שימוש ב-`--summary-out <path>` יוצר
        קובץ JSON עם הדלתא (`fastpq_row_usage_summary.json`), שגם מפורסם כארטיפקט ב-CI.
        אם ה-GPU לא מספק טלמטריית zero-fill לאחר הפרוב, ההרצה מסנתזת מדידה על בסיס איפוס מארח ומזריקה אותה לבלוק
        `zero_fill` כך שניתן עדיין להוכיח את עלות ה-padding ואת התקינות של הדוח גם בלי נתוני GPU.
        חבילות Stage 7-3 חייבות גם לעבור את `scripts/fastpq/validate_row_usage_snapshot.py`, שבודק שכל ערך `row_usage`
        כולל את מוני הסלקטורים ושהאינווריאנט `transfer_ratio = transfer_rows / total_rows` נשמר; `ci/check_fastpq_rollout.sh`
        מריץ את הבדיקה אוטומטית כך שחבילות ללא האינווריאנטים האלה נחסמות לפני שהפעלת ה-GPU הופכת לדרישת חובה.【scripts/fastpq/validate_row_usage_snapshot.py:1】【ci/check_fastpq_rollout.sh:1】
    5. אמתו טלמטריה בעזרת `curl` למדד `fastpq_execution_mode_total{backend="metal"}` או באמצעות לוגי `telemetry::fastpq.execution_mode`; הופעה בלתי צפויה של `resolved="cpu"` מסמנת פולבק.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    6. בזמן תחזוקה ניתן להכריח CPU עם `FASTPQ_GPU=cpu` (או דרך `zk.fastpq.execution_mode`), ולאמת שהלוגים מתעדים את הפולבק כדי לשמור את נוהלי SRE מסונכרנים.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
- ראיות מוכנות ל-Metal (לשמור עם כל רילאאוט כדי להוכיח דטרמיניזם, טלמטריה ופולבק):

    | צעד | מטרה | פקודה / ראיה |
    | --- | --- | --- |
    | בניית metallib | לוודא שה-Metal CLI מותקן ולייצר `.metallib` דטרמיניסטי לקומיט הנוכחי | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` → `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` → `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` → `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】
    | בדיקת משתנה סביבה | לתעד ש-`FASTPQ_METAL_LIB` איננו ריק ולכן ה-backend נשאר פעיל | `echo $FASTPQ_METAL_LIB` (צפוי להחזיר נתיב מוחלט; ערך ריק משמעו שה-backend נוטרל).【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
    | בדיקות GPU מאולצות | להוכיח שהקרנלים רצים או שמופיע לוג הפולבק הדטרמיניסטי | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` ולשמור את הפלט שמראה `backend="metal"` או את אזהרת הפולבק.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】
| דגימת Benchmark | ליצור JSON/לוג עם `speedup.*` ונתוני FFT לטובת דאשבורדים | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json` ולשמור את ה-JSON והלוג עם חומרי השחרור (הדוח מציג את ה-trace של 20k שורות ואת הדומיין המרופד ל-32,768 כך שניתן לוודא ש-LDE נשאר מתחת ל-950ms).【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
    | בדיקת טלמטריה | לוודא שמדדי Prometheus ולוגי `telemetry::fastpq.execution_mode` מדווחים `backend="metal"` או מתעדים את הדאונגרייד | `curl -s http://<host>:8180/metrics | rg fastpq_execution_mode_total` + צילום הלוג בזמן האתחול.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    | תרגיל פולבק ל-CPU | לתעד עבור ה-SRE את ההליך הדטרמיניסטי במקרה שצריך לחזור ל-CPU | להריץ עומס קצר עם `FASTPQ_GPU=cpu` או `zk.fastpq.execution_mode = "cpu"` ולשמור את לוג הדאונגרייד.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
    | לכידת Trace (רשות) | להפעיל `FASTPQ_METAL_TRACE=1` כך שעקבות occupancy/lane יישמרו לחקירות עתידיות | להריץ בדיקת פריטי אחת עם `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` ולצרף את ה-trace לחבילת העדכון.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】

    יש לצרף את הראיות לכרטיס השחרור ול-checklist שב-`docs/source/fastpq_migration_guide.md` כדי שסטייג'ינג ופרודקשן יפעלו לפי אותו מתכון.【docs/source/fastpq_migration_guide.md:1】

### אכיפת רשימת הבדיקה לשחרור

כרטיס שחרור FASTPQ נסגר רק לאחר שכל הפריטים הבאים הושלמו והצרופות צורפו:

1. **מדדי הוכחה תת-שניות** — לבדוק את `fastpq_metal_bench_*.json` העדכני ולוודא שערכי `benchmarks.operations` ו-`report.operations` עבור `operation = "lde"` (עומס של ‎20,000‎ שורות המרופד ל־32,768) מציגים `gpu_mean_ms ≤ 950`. חריגה מהתקרה דורשת חקירת zero-fill/queue depth ואיסוף חדש לפני אישור.
2. **מניפסט בנצ' חתום** — לאחר איסוף חבילות Metal/CUDA להריץ  
  `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   ולצרף לכרטיס את `fastpq_bench_manifest.json`, את `fastpq_bench_manifest.sig` ואת טביעת האצבע של המפתח החותם.【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】
3. **צרופות חובה** — להעלות את קובץ ה-JSON מהבנצ' Metal, את לוג ה-stdout (או עקבת Instruments) ואת זוג `fastpq_bench_manifest.{json,sig}` כך שסוקרים יוכלו להשוות מול הדיגסט שנרשם במניפסט.【artifacts/fastpq_benchmarks/README.md:65】

- טלמטריה והסתרת fallback:
  - לוגים ומדדי מצב הרצה (`telemetry::fastpq.execution_mode`,‏ `fastpq_execution_mode_total{backend="metal"|...}`) מציגים את המצב המבוקש מול זה שנפתר בפועל כדי לאתר מעברים שקטים ל-CPU.【crates/fastpq_prover/src/backend.rs:174】【crates/iroha_telemetry/src/metrics.rs:5397】
  - `FASTPQ_GPU={auto,cpu,gpu}` עדיין נתמך; ערכים חריגים מדווחים כאזהרה אך ממשיכים להופיע בטלמטריה.【crates/fastpq_prover/src/backend.rs:308】【crates/fastpq_prover/src/backend.rs:349】
  - בדיקות GPU (`cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu`) חייבות לעבור גם עם CUDA וגם עם Metal; CI מדלג בצורה נשלטת כאשר `metallib` חסרה או שהזיהוי נכשל.【crates/fastpq_prover/src/gpu.rs:49】【crates/fastpq_prover/src/backend.rs:346】
- ✅ רגרסיות SM80:
  - נוספו בדיקות פריות שמפעילות ישירות את קרנלי ה-CUDA עבור FFT/IFFT/LDE כאשר מפעילים את `fastpq-gpu`, ומשוות את הפלט מול המסלול הסקלרי. אם זמן הריצה מחזיר `GPU backend unavailable` הבדיקה מדלגת בצורה מבוקרת, כך שמארחים ללא מאיץ נותרים ירוקים בעוד שרתי SM80 מאמתים שאין סטיות.【crates/fastpq_prover/src/fft.rs:858】【crates/fastpq_prover/src/fft.rs:919】【crates/fastpq_prover/src/fft.rs:1003】
- ✅ Benchmark השוואתי בין CPU ל-GPU:
  - הרתמת Criterion ב-`crates/fastpq_prover/benches/planner.rs` מודדת FFT/IFFT/LDE עבור עקבות 2^13–2^16 בפרופילים `fastpq-lane-balanced` ו-`fastpq-lane-latency`, מתייגת ריצות ללא מאיץ כ-`gpu_fallback` ומשלבת את התוצאות בדוקומנטציה.
  - זמני fallback נשענים על מסלול הפיצול הדטרמיניסטי כך שהתוצאות נשארות זהות גם כאשר הקרנלים נכשלים או לא זמינים.【crates/fastpq_prover/src/fft.rs:132】
- זיהוי ריצה ודיספאץ':
  - הוספת `ExecutionMode { Cpu, Gpu, Auto }`.
  - מפצל עומסים כש-GPU עמוס (רשות).
- ✅ ספוג Poseidon ב-GPU: `PoseidonBackend` מפעיל את ההעתק CUDA (`fastpq_cuda::fastpq_poseidon_permute`) ומדליק אזהרה חד-פעמית לפני מעבר דטרמיניסטי ל-CPU אם הקרנל אינו זמין או נכשל.
- ממשק `PoseidonBackend` להחלפה בין CPU/GPU.

### שלב 6 — מעבר לפרודקשן *(בתהליך)*
- ניהול תצורה:
  - ✅ הוצאת backend המציין מהקוד; הקונסטרקטור הקנוני של `Prover` מפעיל את נתיב הפרודקשן כברירת מחדל, והמעבר CPU/GPU מנוהל דרך `zk.fastpq.execution_mode` או `irohad --fastpq-execution-mode`.
  - ✅ עדכון ספי ביצועים (<1,000ms עבור עקבה של 20,000 שורות; ציון חומרה). רגרסיית הלילה כעת מריצה 20,000 טרנזאקציות סינתטיות עם תקציב של 1,000ms ומתעדת את חומרת הבסיס (CPU:‏ AMD EPYC 7B12‏ 32 ליבות, ‎256 GiB RAM;‏ GPU:‏ NVIDIA A100 40 GB,‏ CUDA 12.2).【crates/fastpq_prover/tests/perf_production.rs:1】【.github/workflows/fastpq-production-perf.yml:19】
- CI:
  - ✅ בדיקת ביצועים בריליס (`FASTPQ_PROOF_ROWS=20000`) כריצת nightly (`.github/workflows/fastpq-production-perf.yml`).
  - שמירת תוצרי ביצועים ופרסומם.
    - ✅ העלאת לוג ביצועים לילי (`fastpq-production-perf-log`) עם סיכום משך וגודל הוכחה.
    - ✅ המרת הלוג ל-NDJSON טלמטרי באמצעות `scripts/fastpq/src/bin/publish_perf.rs` והעלאתו יחד עם הלוג בצעד הלילי לטובת צינור הטלמטריה.【scripts/fastpq/src/bin/publish_perf.rs:1】【.github/workflows/fastpq-production-perf.yml:61】
- תיעוד והרצה:
  - ✅ עדכון `status.md`, `docs/source/nexus.md`, `docs/source/zk/prover_runbook.md` ו-`docs/source/references/configuration.md` כך שיתייחסו ל-backend הפרודקשן ולמדדי הטלמטריה של בחירת מצב ההרצה.
  - ✅ פרסום `docs/source/fastpq_migration_guide.md` עם דגלי בנייה, דרישות חומרה, צעדי פתרון תקלות והוראות פולבק.
- Hardening:
  - ✅ בניות משוחזרות (נעילת toolchain/containers) באמצעות סקריפט ייעודי וקונטיינרים קבועים.【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.cpu:1】【scripts/fastpq/docker/Dockerfile.gpu:1】
  - Fuzzers ל-trace/SMT/lookup.
  - ריצות חוזרות ב-CI על x86_64 ו-ARM64.

### שלב 7 — הקשחה ורולאאוט *(לא החל)*
- הרחבת בדיקות:
  - בדיקות נכונות/שליליות ל-FFT/FRI עם פולינומים אקראיים.
  - אימות כשל עבור שורשים שגויים, טרנסקריפט מעוות, פתיחות שגויות ו-β/γ לא תואמים.
- אינטגרציה:
  - הרצת E2E של IVM/Torii (קלפיות, זרימות כספיות) עם הפלובר החדש.
  - השוואת קומיטמנטים/טרנסקריפטים מול backend מציין מקום לעודד תאימות.
- מדריך רולאאוט:
  - פירוט דגלים, הגדרות, חומרה, פתרון תקלות והוראות fallback.
  - תיעוד שחזור backend הישן לעת תקלה (זמני).
- תיעוד סופי ב-`status.md` עם קישורי ביצועים.
- Alertmanager מוסיף את ההתרעה `FastpqCpuFallbackBurst`, שמתריעה כאשר יותר מ-5% מהבקשות שסומנו כ-`auto|gpu` נפתרות ב-backend של CPU ברצף של לפחות 10 דקות; כאשר היא נורתה יש לאסוף מחדש את ארטיפקטי ה-GPU, לצרף את הראיות לחבילת שלב 7 ולחקור למה ה-GPU נזרק.
- סט ה-SLO מתעדכן גם בהתראה `FastpqQueueDutyCycleDrop`, שממוצעת את `fastpq_metal_queue_ratio{metric="busy"}` בחלון של 15 דקות ומתריעה מיד כשהתורים של Metal לא מצליחים לשמור על ≥50 % תפוסה למרות שעדיין מוזרמים אליהם עומסי GPU. כך הטלמטריה החיה נשארת מסונכרנת עם הראיות מה-benchmark עוד לפני שה-GPU מוגדר כברירת מחדל.【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】

### תוויות מכשירים והסכם התרעות של שלב 7-1

`scripts/fastpq/wrap_benchmark.py` כבר מסתמך על `system_profiler` במארחי macOS ומקליט תגיות חומרה בכל JSON עטוף, כך שטלמטריות הצי ומטריצת הלכידה חולקות את אותו שם בלי להזדקק לגיליונות משלימים. לכידת Metal של ‎20 000‎ שורות כוללת כעת רשומת תוויות כגון:

```json
"labels": {
  "device_class": "apple-m4-pro",
  "chip_family": "m4",
  "chip_bin": "pro",
  "gpu_kind": "integrated",
  "gpu_vendor": "apple",
  "gpu_bus": "builtin",
  "gpu_model": "Apple M4 Pro"
}
```

במערכי Linux אותה עטיפה קוראת את `/proc/cpuinfo`, `nvidia-smi`/`rocm-smi` ו-`lspci` כדי לגזור `cpu_model`, `gpu_model` ו-`device_class` קנוני (למשל `xeon-rtx-sm80` או `neoverse-mi300`). עדיין ניתן לאלץ ערכים ידניים עם `--label`, אך דרישת שלב 7 היא שכל חבילת ראיות תישען על הזיהוי האוטומטי, לכן כשלי תיוג מפילים את החבילה עוד בבדיקת ה-CI.

בזמן הריצה יש להצמיד ל-host את אותן תוויות באמצעות `fastpq.{device_class,chip_family,gpu_kind}`
או המשתנים `FASTPQ_DEVICE_CLASS/FASTPQ_CHIP_FAMILY/FASTPQ_GPU_KIND`. כך סדרות Prometheus כגון
`fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}`
ו-`fastpq_metal_queue_*{...}` משתמשות באותו מזהה שמופיע תחת
`artifacts/fastpq_benchmarks/matrix/devices/*.txt`, והלוח `dashboards/grafana/fastpq_acceleration.json`
יחד עם `dashboards/alerts/fastpq_acceleration_rules.yml` יכולים להציג, לסנן ולהתריע לפי כיתה/משפחה/סוג
בחלוקה אחת לאחת.【crates/iroha_config/src/parameters/user.rs:1224】【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】

טבלת ה-SLO/התרעה הבאה מחברת בין המדדים לבין Gates של שלב 7:

| אות | מקור | יעד / טריגר | אכיפה |
|-----|------|-------------|--------|
| יחס אימוץ GPU | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", device_class="…", chip_family="…", gpu_kind="…", backend="metal"}` | ≥95 % מהרזולוציות לכל שלישיית (class/family/kind) חייבות להסתיים ב-`resolved="gpu"`; Pager אם שלישייה יורדת מתחת ל-50 % במשך ‎15‎ דקות | התראת `FastpqMetalDowngrade` (pager)【dashboards/alerts/fastpq_acceleration_rules.yml:1】 |
| פער backend | Prometheus `fastpq_execution_mode_total{backend="none", device_class="…", chip_family="…", gpu_kind="…"}` | חייב להישאר אפס לכל שלישייה; אזהרה לאחר פרץ מעל ‎10‎ דקות | התראת `FastpqBackendNoneBurst` (אזהרה)【dashboards/alerts/fastpq_acceleration_rules.yml:21】 |
| יחס פולבק ל-CPU | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", backend="cpu", device_class="…", chip_family="…", gpu_kind="…"}` | ≤5 % מהבקשות שמכוונות ל-GPU רשאיות לנחות על CPU לכל שלישייה; Pager אם הערך חוצה 5 % ל-≥10 דקות | התראת `FastpqCpuFallbackBurst` (pager)【dashboards/alerts/fastpq_acceleration_rules.yml:32】 |
| דוּטי-סייקל של תורי Metal | Prometheus `fastpq_metal_queue_ratio{metric="busy", device_class="…", chip_family="…", gpu_kind="…"}` | ממוצע נע של ‎15‎ דקות חייב להישאר ≥50 % כל עוד עומסי GPU ממשיכים להיאסף; להתריע כאשר התפוסה יורדת מתחת ליעד בזמן שהבקשות נמשכות | התראת `FastpqQueueDutyCycleDrop` (אזהרה)【dashboards/alerts/fastpq_acceleration_rules.yml:98】 |
| עומק תור ותקציב zero-fill | חסימות `metal_dispatch_queue` ו-`zero_fill_hotspots` ב-JSON העטוף | `max_in_flight` חייב להישאר לפחות חריץ אחד מתחת ל-`limit` וזמן ה-zero-fill הממוצע של ה-LDE חייב להיות ≤0.40 ms עבור עקבת 20 000 שורות; רגרסיות חוסמות את החבילה | נבדק דרך הפלט של `scripts/fastpq/wrap_benchmark.py` והצמוד ל-`docs/source/fastpq_rollout_playbook.md` |
| מרווח תורים בזמן ריצה | Prometheus `fastpq_metal_queue_depth{metric="limit|max_in_flight", device_class="…", chip_family="…", gpu_kind="…"}` | `limit - max_in_flight ≥ 1` בכל שלישייה; להתריע לאחר ‎10‎ דקות ללא מרווח | התראת `FastpqQueueHeadroomLow` (אזהרה)【dashboards/alerts/fastpq_acceleration_rules.yml:41】 |
| זמן zero-fill בזמן ריצה | Prometheus `fastpq_zero_fill_duration_ms{device_class="…", chip_family="…", gpu_kind="…"}` | הדגימה האחרונה חייבת להישאר ≤0.40 ms (תקציב שלב 7) | התראת `FastpqZeroFillRegression` (pager)【dashboards/alerts/fastpq_acceleration_rules.yml:58】 |

#### רשימת בדיקות לטיפול בהתרעות Stage7-1

ההתרעות הללו מזינות תרגיל on-call סדור שנועד לשמר את הראיות בחבילת ה-rollout:

1. **`FastpqQueueHeadroomLow` (אזהרה).** הריצו שאילתת Prometheus
   `fastpq_metal_queue_depth{metric=~"limit|max_in_flight",device_class="<matrix>"}` וצירפו
   צילום מסך של פאנל “Queue headroom” בדשבורד `fastpq-acceleration`. שמרו את הפלט תחת
   `metrics_headroom.prom` בתוך
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/` יחד עם מזהה ההתרעה כדי להראות שהאזהרה טופלה לפני שהמרווח אזל.【dashboards/grafana/fastpq_acceleration.json:1】
2. **`FastpqZeroFillRegression` (pager).** בדקו
   `fastpq_zero_fill_duration_ms{device_class="<matrix>"}`, וכאשר נדרש הפעילו שוב את
   `scripts/fastpq/wrap_benchmark.py` על קובץ ה-bench האחרון כדי לרענן את
   `zero_fill_hotspots`. צרפו את הפלט, צילומי המסך והקובץ המעודכן לנתיב ה-rollout – אלו
   בדיוק הקבצים ש-`ci/check_fastpq_rollout.sh` מאמת.【scripts/fastpq/wrap_benchmark.py:1】【ci/check_fastpq_rollout.sh:1】
3. **`FastpqCpuFallbackBurst` (pager).** ודאו ש-
   `fastpq_execution_mode_total{requested="gpu",backend="cpu"}` חוצה את סף ה-5%, ואז שלפו
   את יומני `telemetry::fastpq.execution_mode resolved="cpu"`. שמרו את הפלט כ-
   `metrics_cpu_fallback.prom` והוסיפו קטעים רלוונטיים ל-`rollback_drill.log`.
4. **אריזת הראיות.** לאחר שההתראה נרגעת, הריצו מחדש את שלב 7-3 במדריך
   (`Grafana export`, צילום מקבץ ההתרעות, תיעוד Drill) ואשרו את החבילת באמצעות
   `ci/check_fastpq_rollout.sh` לפני צירוף מחדש.【docs/source/fastpq_rollout_playbook.md:114】

מי שמעדיף אוטומציה יכול להריץ
`scripts/fastpq/capture_alert_evidence.sh --device-class <label> --out <bundle-dir>`; הסקריפט
משתמש ב-Prometheus API כדי להשיג את שלוש המדידות ולכתוב קבצי `.prom` מוכנים לצירוף
(`metrics_headroom.prom`,‏ `metrics_zero_fill.prom`,‏ `metrics_cpu_fallback.prom`) תחת תיקיית
ה-rollout שנבחרה.

`ci/check_fastpq_rollout.sh` אוכף את מרווח התורים ואת תקציב ה-zero-fill ישירות. הוא סורק כל לכידת `metal` שמופיעה ב-`fastpq_bench_manifest.json`, קורא את `benchmarks.metal_dispatch_queue.{limit,max_in_flight}` ואת `benchmarks.zero_fill_hotspots[]`, ומפיל את החבילה כשהמרווח יורד מתחת לחריץ אחד או כאשר `mean_ms > 0.40`. כחלק מאותה בדיקה הסקריפט מוודא שלכל לכידה יש `metadata.labels.device_class` ו-`metadata.labels.gpu_kind`, אחרת הבדיקה נכשלת – כך נוצר קו ישר בין חבילות השחרור, מניפת Stage 7-2 והטלמטריה בזמן אמת.【ci/check_fastpq_rollout.sh#L1】

הלוח ב-Grafana המציג את “Latest Benchmark” והתיעוד ב-`docs/source/fastpq_rollout_playbook.md` מצרפים את `device_class`, תקציב ה-zero-fill ותמונת עומק התורים לאותה חבילת ראיות, כך שמשמרות on-call יכולות לחצות בין טלמטריה חיה לבין הרצה שהוכיחה את ה-SLO.

### מדריך טלמטריית צי שלב 7-1

בצעו את הצ’ק-ליסט לפני שמגדירים את נתיב ה-GPU כברירת מחדל כדי לוודא שהטלמטריה וההתרעות משקפות את הראיות מהשחרור:

1. **תייגו את מארחי הלכידה והריצה.** `python3 scripts/fastpq/wrap_benchmark.py` כבר כותב `metadata.labels.device_class/chip_family/gpu_kind` לכל JSON עטוף. שמרו את התוויות מסונכרנות עם `fastpq.{device_class,chip_family,gpu_kind}` (או עם המשתנים `FASTPQ_DEVICE_CLASS/FASTPQ_CHIP_FAMILY/FASTPQ_GPU_KIND=<matrix-label>`) בתוך `iroha_config`, כך ש-`fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}` ו-`fastpq_metal_queue_*{...}` יבטאו בדיוק את אותם שמות שנמצאים תחת `artifacts/fastpq_benchmarks/matrix/devices/*.txt`. כאשר מוסיפים כיתה חדשה, הריצו `scripts/fastpq/capture_matrix.sh --devices artifacts/fastpq_benchmarks/matrix/devices` כדי לרענן את ה-manifest ואת רשימת הכיתות שמשמשת את CI והלוח.
2. **אמתו מחווני תור ויחסי אימוץ.** הפעילו `irohad --features fastpq-gpu` על מארחי ה-Metal וגרדו את נקודת הקצה כדי לוודא שהמדדים זמינים:

   ```bash
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_metal_queue_(ratio|depth)'
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_execution_mode_total'
   ```

   הפקודה הראשונה מוכיחה שהסמפולר מפרסם את `busy`,‏ `overlap`,‏ `limit` ו-`max_in_flight`; השנייה מראה האם כל כיתה נחתמת ב-`backend="metal"` או נופלת ל-`backend="cpu"`. חברו את היעד ל-Prometheus/OTel לפני ייבוא הלוח כך ש-Grafana יקבל מיידית את נוף הצי.
3. **פרסו את הלוח וחבילת ההתרעות.** יבאו את `dashboards/grafana/fastpq_acceleration.json` ל-Grafana (שמרו על משתני Device Class/Chip Family/GPU Kind) וטענו את `dashboards/alerts/fastpq_acceleration_rules.yml` ב-Alertmanager יחד עם פיקסצ’ר הבדיקה שלו. להרצת היחידה השתמשו ב-`promtool test rules dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` כדי לוודא ש-`FastpqMetalDowngrade` ו-`FastpqBackendNoneBurst` עדיין נדלקות בספים המתועדים.
4. **חסמו גרסאות באמצעות חבילת הראיות.** עבודה עם `docs/source/fastpq_rollout_playbook.md` מבטיחה שכל חבילה מכילה לכידות עטופות, יצוא Grafana, חבילת התרעות, הוכחת עומק תור ויומני fallback. `make check-fastpq-rollout` או הפעלה ידנית של `ci/check_fastpq_rollout.sh --bundle <path>` מאמתים את החבילה, מריצים מחדש את בדיקות ההתרעות וחוסמים חתימה כאשר מרווח התורים או תקציב ה-zero-fill נסוגים.
5. **קשרו התרעות לפעולת תיקון.** כש-Alertmanager מזעיק, השתמשו בלוח Grafana ובמונהי Prometheus מהשלב הקודם כדי לאמת האם מדובר ברעב תורים, פולבק ל-CPU או `backend="none"`. העדכונים למסמך זה ול-`docs/source/fastpq_rollout_playbook.md` מתארים את שלבי הטיפול; הוסיפו לטיקט השחרור את מקטעי `fastpq_execution_mode_total`,‏ `fastpq_metal_queue_ratio` ו-`fastpq_metal_queue_depth` הרלוונטיים יחד עם קישורים לפאנל Grafana ולצילום ההתרעה כדי שהמאשרים יראו בדיוק איזה SLO הופעל.

### פאן-אאוט תורי FFT בשלב 7

ה-host של Metal ב-`crates/fastpq_prover/src/metal.rs` כולל כעת `QueuePolicy`
שמייצר כמה תורי פקודה כאשר `Device` מזוהה כ-GPU דיסקרטי. ברירת המחדל
ממשיכה להשתמש בתור יחיד במכשירי Apple Silicon משולבים, אך מארחים עם GPU
דיסקרטי קופצים לשני תורים ויפעילו פאן-אאוט רק כאשר עומס העבודה מכסה לפחות
‎16‎ עמודות. ניתן לשנות את שני החתכים דרך המשתנים
`FASTPQ_METAL_QUEUE_FANOUT` ו-`FASTPQ_METAL_COLUMN_THRESHOLD`, והפ scheduler
מחלק אצוות FFT/LDE בין התורים באופן round-robin בזמן שהקריאה אחרי הטיילינג
נשארת באותו תור כדי לשמור על סדר התלויות.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:772】【crates/fastpq_prover/src/metal.rs:900】

בדיקות העזר מאמתות את ההיוריסטיקות ומגינות על הפרסרים כך שניתן להריץ את
הגארדים ב-CI גם ללא חומרת GPU, והבדיקות שתלויות ב-GPU מכריחות את אותם override-ים
כדי להבטיח שהפאן-אאוט הדטרמיניסטי נשמר גם במעבדות השחזור.【crates/fastpq_prover/src/metal.rs:2163】【crates/fastpq_prover/src/metal.rs:2236】

## פרמטרים וספי ביצועים
| `N_trace` | blowup | FRI arity | שכבות | שאילתות | ביטים מוערכים | גודל הוכחה ≤ | זיכרון ≤ | חביון P95 ≤ |
|-----------|--------|-----------|--------|----------|----------------|--------------|-----------|-------------|
| 2^15 | 8 | 8 | 5 | 52 | ~190 | 300 KB | 1.5 GB | 0.40 s (A100) |
| 2^16 | 8 | 8 | 6 | 58 | ~132 | 420 KB | 2.5 GB | 0.75 s (A100) |
| 2^17 | 16 | 16 | 5 | 64 | ~142 | 550 KB | 3.5 GB | 1.20 s (A100) |

הנגזרות מפורטות בנספח א'. CI נכשל אם המדד יורד מתחת ל-128 ביט.

## סכמת Public IO
| שדה | בתים | קידוד | הערות |
|------|------|--------|--------|
| `dsid` | 16 | UUID LE | מזהה מרחב הנתונים של ה-lane (GLOBAL לליין ברירת מחדל); מתויג ב-`fastpq:v1:dsid`. |
| `slot` | 8 | u64 LE | זמן בננו-שניות. |
| `old_root` | 32 | Poseidon2 LE | שורש SMT לפני הבאטצ’. |
| `new_root` | 32 | Poseidon2 LE | שורש SMT אחרי הבאטצ’. |
| `perm_root` | 32 | Poseidon2 LE | שורש טבלת הרשאות. |
| `tx_set_hash` | 32 | BLAKE2b | hash של מזהי טרנזקציות ממוין. |
| `parameter` | משתנה | UTF-8 | שם סט פרמטרים. |
| `protocol_version`, `params_version` | 2 כל אחד | u16 LE | גרסאות. |
| `ordering_hash` | 32 | Poseidon2 LE | hash של סדר השורות. |

מחיקה: ערכי אפס. מפתחות חסרים: עלה אפס ועוד `neighbour_leaf`.

`FastpqTransitionBatch.public_inputs` הוא הנשׂא הקנוני ל-`dsid` / `slot` ושורשי הקומיטמנט; המטא-דטה שמורה ל-entry hash ולספירת טרנסקריפטים.

## Hashing
- hash הסידור: Poseidon2 (תג `fastpq:v1:ordering`).
- hash ארטיפקט: BLAKE2b על `PublicIO || proof.commitments` עם תג `fastpq:v1:artifact`.

## DoD לכל שלב
- **שלב 0**: שדות חדשים מאוישים; סקריפט גזירה ו-CI מבטיחים התאמה; נספחים/roadmap/`status.md` מעודכנים.
- **שלב 1**: מתכנן עמודות, בדיקות ו-Benchmarks משולבים; FFT/IFFT מוכיחים דטרמיניזם; פיצ'ר `fastpq-prover-preview` מנהל החלפת נתיב.
- **שלב 2**: קומיטמנט Poseidon סטרימי ו-fixtures תאימות; pipeline lookup מעודכן; נספח C מתעד תגי דומיין.
- **שלב 3**: לולאת DEEP-FRI וטרנסקריפט עם בדיקות חיוביות/שליליות; אימפלמנטציית ייחוס בסגנון Winterfell עם בדיקות קרוס; מדדי טלמטריה ללולאות קיפול.
- **שלב 4**: Sampler דטרמיניסטי; וריפייר תואם; בדיקות כשל עבור פתיחות/β/γ; עדכון תרשימי `nexus.md`.
- **שלב 5**: הפעלות GPU שקולות ל-CPU; תצורת מצב ריצה מתועדת; Benchmark GPU ב-nightly.
- **שלב 6**: backend מציין המקום נשמר רק כ-fallback; מדריך הגירה ו-perf harness מעודכנים; ריצות חוזרות מוצלחות.
- **שלב 7**: סוויטות בדיקה מלאות (חיוביות/שליליות); אינטגרציית IVM/Torii מצליחה; צ’ק-ליסט רולאאוט מבוצע ו-`status.md` מעודכן.

בנוסף, סט ה-SLO אוכף כעת גם את יעד הדוּטי־סייקל של Metal (‎≥50%) באמצעות הכלל `FastpqQueueDutyCycleDrop`, שמחשב ממוצע של `fastpq_metal_queue_ratio{metric="busy"}` בחלון מתגלגל של 15 דקות ומתריע כאשר עומסי GPU ממשיכים להישלח אבל תור מסוים אינו שומר על הכיבולת הנדרשת. בכך חוזה הטלמטריה בזמן אמת נשאר מסונכרן עם הראיות מהבנצ'מרקים עוד לפני שהפעלת ערוצי ה-GPU הופכת לדרישת חובה.

---

## סיכום ביקורת ופעולות פתוחות

### חוזקות
- חלוקה מדורגת שמאפשרת רולאאוט חלקי.
- דוקומנטציה מקצה לקצה של קומיטמנטים דטרמיניסטיים וסידור נתונים.
- DoD מוגדר היטב שמבטיח סינכרון בין roadmap, תכנית ומימוש.

### פעולות בעדיפות גבוהה
1. ✅ מתכנן FFT עמודות וסוויטת Criterion (שלב 1) הושלמו; ממשיכים לנטר רגרסיות ביצועים.
2. לחבר את `fastpq-prover-preview` ל-CI nightly ולהפיק מדדי Planner לפני ריצת שלב 2.
3. ליישר את מבחני ההתאמה לפייפליין ה-commitment החדש (נספח C) כדי לסגור פערים מול backend מציין המקום.

### החלטות עיצוב מוסכמות
- מצב ZK נותר כבוי בשלב P1 (correctness-only) וייבחן בעתיד.
- שורש טבלת ההרשאות נגזר ממצב הממשל; הוכחות lookup מתייחסות אליו כקריאה בלבד.
- הוכחות למפתח חסר משתמשות בעלה אפס וב-`neighbour_leaf` מקודד באופן קנוני.
- מחיקה משמעה קביעת ערך העלה לאפס בתוך מרחב המפתחות הקנוני.

## נספח A — גזירת חוסן

נספח זה מפרט כיצד הופקה הטבלה “פרמטרים וספי ביצועים” וכיצד הרנסר של ה-CI מאמת את ההערכות האנליטיות.

### סימון
- `N_trace = 2^{k}` — אורך ה-trace לאחר מיון וריפוד.
- `b` — פקטור ההרחבה (`N_eval = N_trace * b`).
- `r` — ה-arity של FRI (8 או 16 בסטים הקנוניים).
- `ell` — מספר שכבות FRI (`layers` בטבלה).
- `q` — מספר השאילתות שהווריפייר פותח (`queries` בטבלה).
- `rho` — קצב הקוד האפקטיבי בתחילת כל שכבת FRI, כפי שמדווח המתכנן: `max_i(degree_i / domain_i)` על פני אילוצי השורות והעמודות הרלוונטיים.

שדה Goldilocks מספק `|F| = 2^{64} - 2^{32} + 1`; לאורך החישוב נניח `log2 |F| ≈ 64` כדי לחסום התנגשותי Fiat–Shamir.

### גבול אנליטי
ל-DEEP-FRI עם שכבות בקצב קבוע מתקיים

```
p_fri <= \sum_{j=0}^{\ell-1} \rho^{q} = \ell * \rho^{q},
```

משום שהמתכנן שומר על `rho` קבוע לאורך ההקטנות (הדרגה וגודל הדומיין מצטמצמים באותו גורם `r`). התנגשותי Fiat–Shamir תורמות לכל היותר `q / 2^{64}`; גריינדינג מוסיף מרכיב אורתוגונלי של `2^{-g}` (`g ∈ {21, 23}` בקטלוג הקנוני). בטבלה משתקפת תרומת ה-FRI בלבד (`est bits = floor(-log2 p_fri)`), ואילו Fiat–Shamir/גריינדינג מספקים מרווח ביטחון נוסף שמעלה עוד יותר את החוסן הכולל.

### פלטי המתכנן וחישובי שורה
הרצת מתכנן העמודות של שלב 1 על באצ'ים מייצגים מניבה את קצבי הקוד הבאים. הדרגות מתייחסות לדרגת פולינום הקומפוזיציה המקסימלית לפני סיבוב ה-FRI הראשון:

| סט פרמטרים | `N_trace` | `b` | `N_eval` | `rho` (מתכנן) | דרגה אפקטיבית (`rho * N_eval`) | `ell` | `q` | `-log2(\ell * \rho^{q})` |
|-------------|-----------|-----|----------|---------------|----------------------------------|-------|-------|-------------------------|
| באצ' 20k (מאוזן) | `2^15` | 8 | 262144 | 0.077026 | 20192 | 5 | 52 | 190 ביט |
| באצ' 65k (Thruput) | `2^16` | 8 | 524288 | 0.200208 | 104967 | 6 | 58 | 132 ביט |
| באצ' 131k (Latency) | `2^17` | 16 | 2097152 | 0.209492 | 439337 | 5 | 64 | 142 ביט |

דוגמה (באצ' 20k):
1. `N_trace = 2^15`, ולכן `N_eval = 2^15 * 8 = 2^18`.
2. המתכנן מדווח `rho = 0.077026`, ולכן `p_fri = 5 * rho^{52} ≈ 6.4e-58`.
3. `-log2 p_fri = 190` ביט, תואם לטבלה.
4. התנגשותי Fiat–Shamir מוסיפות כ-`2^{-58.3}` וגריינדינג (`g = 23`) מפחית את ההסתברות ב-`2^{-23}`, כך שהחוסן הכולל נשמר מעל 160 ביט.

השורות האחרות נבנות באותו אופן; רק `rho`, `ell` ו-`q` משתנים.

### ריג'קשן-סמפלינג ב-CI
בכל מחזור CI רץ הרנסר הבא כדי לוודא את הגבולות:
1. בחירת סט פרמטרים קנוני וסינתוז `TransitionBatch` אקראי באורך היעד.
2. בניית ה-trace, הזרקת אילוץ שגוי (למשל שינוי תרומת גרנד-פרודקט או אח SMT), וניסיון להפיק הוכחה.
3. הפעלת הווריפייר מחדש עם אתגרי Fiat–Shamir טריים (כולל גריינדינג) ותיעוד האם ההוכחה המזויפת שרדה.
4. חזרה על התהליך עבור 16 384 זרעים לכל סט; המרה לביטסי חוסן באמצעות חסם Clopper–Pearson ב-99%.

התוצאות האמפיריות תואמות את ההערכה האנליטית בטווח ±0.6 ביט לאורך הקטלוג. משימת ה-CI נכשלת אם החסם יורד מתחת ל-128 ביט, כך שכל רגרסיה במתכנן העמודות, בלולאות הקיפול או בהולכת הטרנסקריפט מזוהה בטרם מיזוג.

## נספח B — גזירת שורשי דומיין

גנרטורי תת-החבורה נגזרים דטרמיניסטית מפרמטרי Poseidon2, כך שכל מימוש ישחזר את אותם ערכים.

### תהליך
1. **בחירת seed:** תג UTF-8‏ `fastpq:v1:domain_roots` נספג בספוג Poseidon2 של FASTPQ (רוחב 3, קצב 2, ארבעה סבבי full ו-57 סבבי partial). הקידוד זהה ל-`fastpq_prover::pack_bytes`. התוצאה הראשונה היא הגנרטור הבסיסי `g_base` (כרגע 7).
2. **trace root:** לחשב `trace_root = g_base^{(p-1)/2^{trace_log_size}} mod p` ולהבטיח `trace_root^{2^{trace_log_size}} = 1` תוך בדיקת פרימיטיביות.
3. **lde root:** אותו חישוב עבור `lde_log_size`.
4. **קוסאט:** שלב 0 משתמש בסאב-גרופ הבסיסי ולכן `omega_coset = 1`. בהמשך ניתן להוסיף תג כמו `fastpq:v1:domain_roots:coset` ליצירת קוסאט שאינו טריוויאלי.
5. **גודל פרמוטציה:** שמירת `permutation_size` כערך מפורש כדי למנוע הנחות על ריפוד.

### שחזור ואימות
- להרצה:  
  `cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots`  
  נתמך גם `--format table`, `--seed`, `--filter`.
- בדיקות CI: `fastpq_isi::roots_match_declared_domain` ו-`canonical_sets_meet_security_target` מבטיחות שהקבועים נשארים מסונכרנים.

### ערכי ייחוס
הטבלה בסעיף שלב 0 מציגה את הערכים הנוכחיים (`trace_root`, `lde_root`, `omega_coset`). יש להריץ את הכלי ולהעדכן את הקוד והמסמך בכל הוספת סט פרמטרים חדש.

## נספח C — פרטי צינור הקומיטמנט

### זרימת קומיטמנט Poseidon סטרימינג
שלב 2 מגדיר את הקומיטמנט הדטרמיניסטי ל-trace, המשותף לפלובר ולווריפייר:
1. **קנוניזציה של הטרנזאקציות.** `trace::build_trace` ממיין את באצ' המעברים, מרפד אותו ל-`N_trace = 2^{⌈log₂ rows⌉}`, ומחולל וקטורי עמודות בסדר המופיע להלן.
2. **Hash העמודות.** `trace::column_hashes` סורק את העמודות ברצף, סופג כל אחת לספוג Poseidon2 ייעודי שמוזן בתג `fastpq:v1:trace:column:<name>`. תחת הפיצ'ר `fastpq-prover-preview` אותה רוטינה ממחזרת את מקדמי ה-IFFT/LDE שה-backend זקוק להם, כך שהחישוב נשאר סטרימי וללא העתקת מטריצה.
3. **קיפול מרקל.** `trace::merkle_root` מעלה את Hashי העמודות לעץ Poseidon עם תג `fastpq:v1:trace:node` לצמתים פנימיים, ומשכפל את העלה האחרון כאשר הרמה בעלת מספר אי-זוגי של עלים.
4. **דיג'סט סופי.** `digest::trace_commitment` מוסיף קידומת אורך לדומיין (`fastpq:v1:trace_commitment`), לשם סט הפרמטרים, לממדים המרופדים, ל-digest של העמודות ולשורש המרקל, ואז מחיל `Hash::new` (SHA3-256) כדי לקבל את הקומיטמנט המוטמע ב-`Proof::trace_commitment`.

הווריפייר מחשב מחדש את אותו דיג'סט לפני שהוא מפעיל את טרנסקריפט Fiat–Shamir; כל אי-התאמה מפילה את ההוכחה עוד לפני פתיחת השאילתות.

### סדר העמודות ושמותיהן
העמודות נוצרות בסדר הבא, וזהו הסדר הנאגר ב-hash:
1. דגלי בוררים: `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm`.
2. עמודות איברים (מספר העמודות משתנה, כל אחת מרופדת באפסים): `key_limb_{i}`, `value_old_limb_{i}`, `value_new_limb_{i}`, `asset_id_limb_{i}`.
3. סקלרים עזר: `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter`, `perm_hash`, `neighbour_leaf`, `dsid`, `slot`.
4. עמודות הוכחת ה-SMT לכל רמה `ℓ ∈ [0, 31]`: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`.

מכאן שכל קומיטמנט שמופק ב-`column_hashes` יעדכן את אותו סדר עמודות, וכך מובטח תאום בין backend מציין המקום לבין מימוש ה-STARK המקדמי.

### תגיות הטרנסקריפט
שלב 2 מקבע גם את תגיות Fiat–Shamir כך שההוכחות יישארו דטרמיניסטיות וידידותיות לווריפייר:

| תג | מטרה |
|-----|-------|
| `fastpq:v1:init` | אתחול הטרנסקריפט עם גרסת הפרוטוקול, סט הפרמטרים ו-`PublicIO`. |
| `fastpq:v1:roots` | קומיט לשורש המרקל של ה-trace ולשורש ה-LDE של ה-lookup. |
| `fastpq:v1:gamma` | דגימת אתגר הגרנד-פרודקט של ה-lookups. |
| `fastpq:v1:alpha:<i>` | דגימת אתגרי פולינום הקומפוזיציה (`i = 0,1`). |
| `fastpq:v1:lookup:product` | ספיגת ערך הגרנד-פרודקט המחושב של ה-lookup. |
| `fastpq:v1:beta:<round>` | דגימת אתגרי קיפול FRI לכל סיבוב. |
| `fastpq:v1:fri_layer:<round>` | קומיט לשורש המרקל של כל שכבת FRI. |
| `fastpq:v1:fri:final` | רישום שורש ה-FRI הסופי לפני פתיחת שאילתות. |
| `fastpq:v1:query_index:0` | הפקת אינדקסי השאילתות של הווריפייר באופן דטרמיניסטי. |

תגיות אלו, יחד עם דומייני ה-hash של העמודות לעיל, מרכיבים את “קטלוג הדומיין של הטרנסקריפט” שמוזכר בשלב 2. כל שינוי במחרוזות או בסדרן הוא שבירת תאימות: הפלובר והווריפייר מפיקים ישירות אתגרים מן הטרנסקריפט, וכל שינוי בדומיין יוליד אתגרים שונים.

</div>
