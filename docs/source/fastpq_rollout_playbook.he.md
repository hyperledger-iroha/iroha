---
lang: he
direction: rtl
source: docs/source/fastpq_rollout_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a0c22a213e04a6a8fef94ded6ec0017531737ffd4b9418ec94286bb6759ff8a
source_last_modified: "2026-01-08T09:26:20.579700+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ספר הפעלה של FASTPQ (שלב 7-3)

ספר משחק זה מיישם את דרישת מפת הדרכים Stage7-3: כל שדרוג צי
המאפשר ביצוע FASTPQ GPU חייב לצרף מניפסט ברנצ'מרק שניתן לשחזר,
בשילוב ראיות Grafana, ותרגיל החזרה מתועד. זה משלים
`docs/source/fastpq_plan.md` (יעדים/ארכיטקטורה) ו
`docs/source/fastpq_migration_guide.md` (שלבי שדרוג ברמת הצומת) על ידי מיקוד
ברשימת המשימות להפעלה הפונה למפעיל.

## היקף ותפקידים

- **הנדסת שחרורים / SRE:** לכידת אמת מידה, חתימת מניפסט ו
  ייצוא לוח המחוונים לפני אישור ההשקה.
- **Ops Guild:** מפעילה השקות מבוימות, מקליטת חזרות לחזרה לאחור וחנויות
  חבילת החפצים תחת `artifacts/fastpq_rollouts/<timestamp>/`.
- **ממשל / תאימות:** מאמת שראיות מלוות כל שינוי
  בקשה לפני שברירת המחדל של FASTPQ משתנה עבור צי.

## דרישות חבילת ראיות

כל הגשת השקה חייבת להכיל את החפצים הבאים. צרף את כל הקבצים
לכרטיס השחרור/שדרוג ושמור את החבילה בפנים
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`.| חפץ | מטרה | איך מייצרים |
|--------|--------|----------------|
| `fastpq_bench_manifest.json` | מוכיח שעומס העבודה הקנוני של 20,000 שורות נשאר מתחת לתקרת `<1 s` LDE ומתעד גיבוב עבור כל אמת מידה עטופה.| לכוד ריצות מתכת/CUDA, לעטוף אותן ואז להפעיל:`cargo xtask fastpq-bench-manifest \``  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \``  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \``cargo xtask fastpq-bench-manifest \`Prometheus003003`python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \``  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \`I100NI0pe cap forI100NI00s for (נקודה `--row-usage` ו-`--poseidon-metrics` בתיקי העדים/הגירודים הרלוונטיים). העוזר מטמיע את דגימות `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` המסוננות כך שהראיות של WP2-E.6 זהות על פני Metal ו-CUDA. השתמש ב-`scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` כאשר אתה צריך תקציר מיקרוספסל עצמאי של Poseidon (נתמכים בכניסות עטופות או גולמיות). |
|  |  | **דרישת תווית Stage7:** `wrap_benchmark.py` נכשל כעת, אלא אם הקטע `metadata.labels` שנוצר מכיל גם `device_class` וגם `gpu_kind`. כאשר הזיהוי האוטומטי אינו יכול להסיק מהם (לדוגמה, בעת גלישה על צומת CI מנותק), העבירו דרישות מפורשות כגון `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete`. |
|  |  | **טלמטריית תאוצה:** העטיפה גם לוכדת את `cargo xtask acceleration-state --format json` כברירת מחדל, כותבת `<bundle>.accel.json` ו-`<bundle>.accel.prom` ליד המדד העטוף (עקוף עם דגלים של `--accel-*` או I108NI50X0). מטריצת הלכידה משתמשת בקבצים אלה כדי לבנות `acceleration_matrix.{json,md}` עבור לוחות מחוונים של צי. |
| Grafana ייצוא | מוכיח טלמטריית אימוץ והערות התראות עבור חלון ההשקה.| ייצא את לוח המחוונים של `fastpq-acceleration`:`curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \``  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \``  | jq '.dashboard' \``  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`ציין את הלוח עם זמני התחלה/עצירה של ייצוא. צינור השחרור יכול לעשות זאת באופן אוטומטי באמצעות `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>` (אסימון מסופק באמצעות `GRAFANA_TOKEN`). |
| תמונת מצב התראה | לוכד את כללי ההתראה ששמרו על ההשקה.| העתק את `dashboards/alerts/fastpq_acceleration_rules.yml` (ואת מתקן `tests/`) לתוך החבילה כדי שהבודקים יוכלו להפעיל מחדש את `promtool test rules …`. |
| יומן תרגילי גלגול | מדגים שהמפעילים התאמנו על החזרת ה-CPU הכפויה ואישורי טלמטריה.| השתמש בהליך ב-[Rollback Drills](#rollback-drills) ואחסן את יומני המסוף (`rollback_drill.log`) בתוספת הגרידה של Prometheus (`metrics_rollback.prom`). || `row_usage/fastpq_row_usage_<date>.json` | מתעד את הקצאת השורות של ExecWitness FASTPQ ש-TF-5 עוקב אחר ב-CI ובלוחות מחוונים.| הורד עד חדש מ-Torii, פענח אותו באמצעות `iroha_cli audit witness --decode exec.witness` (אפשר להוסיף את `--fastpq-parameter fastpq-lane-balanced` כדי לקבוע את ערכת הפרמטרים הצפוי; אצווה FASTPQ פולטות כברירת מחדל), והעתק את ה-`row_usage` J740 אל I180NI. שמור על שמות קבצים עם חותמת זמן כדי שהבודקים יוכלו לקשר אותם עם כרטיס ההפצה, ולהפעיל את `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json` (או `make check-fastpq-rollout`) כך ששער Stage7-3 מוודא שכל אצווה מפרסמת את ספירת הבורר ו-`transfer_ratio = transfer_rows / total_rows` מצרף את הראיות לפני כן. |

> **טיפ:** `artifacts/fastpq_rollouts/README.md` מתעד את השם המועדף
> תכנית (`<stamp>/<fleet>/<lane>`) וקבצי הראיות הנדרשים. ה
> תיקיית `<stamp>` חייבת לקודד את `YYYYMMDDThhmmZ` כדי שחפצי אמנות יישארו ניתנים למיון
> ללא התייעצות עם כרטיסים.

## רשימת בדיקות ליצירת עדויות1. **ללכוד מדדי GPU.**
   - הפעל את עומס העבודה הקנוני (20000 שורות לוגיות, 32768 שורות מרופדות) באמצעות
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`.
   - עטפו את התוצאה עם `scripts/fastpq/wrap_benchmark.py` באמצעות `--row-usage <decoded witness>` כך שהחבילה תישא את ראיות הגאדג'ט לצד הטלמטריה של ה-GPU. עברו את `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output` כדי שהעטיפה תיכשל במהירות אם אחד מאיץ ההאצה חורג מהמטרה או אם חסרה טלמטריית התור/פרופיל של Poseidon, וכדי ליצור את החתימה המנותקת.
   - חזור על מארח CUDA כך שהמניפסט יכיל את שתי משפחות ה-GPU.
   - **אל** תפשיט את `benchmarks.metal_dispatch_queue` או
     `benchmarks.zero_fill_hotspots` בלוקים מה-JSON העטוף. שער ה-CI
     (`ci/check_fastpq_rollout.sh`) כעת קורא את השדות הללו ונכשל כאשר עומד בתור
     מרווח הראש יורד מתחת למשבצת אחת או כאשר נקודה חמה של LDE מדווחת על `mean_ms >
     0.40ms`, אוכפת את מגן הטלמטריה Stage7 באופן אוטומטי.
2. **צור את המניפסט.** השתמש ב-`cargo xtask fastpq-bench-manifest …` as
   מוצג בטבלה. אחסן את `fastpq_bench_manifest.json` בחבילת ההפצה.
3. **יצוא Grafana.**
   - הערה על לוח `FASTPQ Acceleration Overview` עם חלון ההפעלה,
     קישור למזהי הפנל הרלוונטיים של Grafana.
   - ייצא את JSON של לוח המחוונים דרך Grafana API (הפקודה למעלה) וכלול
     סעיף `annotations` כדי שהבודקים יוכלו להתאים עקומות אימוץ ל-
     השקה מדורגת.
4. **התראות תמונת מצב.** העתק את כללי ההתראה המדויקים (`dashboards/alerts/…`) בשימוש
   על ידי הפריסה לחבילה. אם כללי Prometheus נדונו, כלול
   ההפרש לעקוף.
5. **Prometheus/OTEL גרידה.** לכיד `fastpq_execution_mode_total{device_class="<matrix>"}` מכל אחד
   מארח (לפני ואחרי הבמה) בתוספת דלפק OTEL
   `fastpq.execution_mode_resolutions_total` והצמד
   `telemetry::fastpq.execution_mode` ערכי יומן. החפצים הללו מוכיחים זאת
   אימוץ ה-GPU יציב וכי נקודות חוזרות מאולצות של מעבד עדיין פולטות טלמטריה.
6. **טלמטריית שימוש בשורות בארכיון.** לאחר פענוח ההפעלה של ExecWitness עבור
   השקה, שחרר את ה-JSON שנוצר תחת `row_usage/` בחבילה. ה-CI
   helper (`ci/check_fastpq_row_usage.sh`) משווה את התמונות האלה מול ה-
   קווי בסיס קנוניים, ו-`ci/check_fastpq_rollout.sh` דורש כעת כל
   חבילה למשלוח קובץ `row_usage` אחד לפחות כדי לשמור עדות TF-5 מצורפת
   לכרטיס השחרור.

## זרימת השקה מדורגת

השתמש בשלושה שלבים דטרמיניסטיים עבור כל צי. התקדמו רק לאחר היציאה
הקריטריונים בכל שלב מתקיימים ומתועדים בצרור הראיות.| שלב | היקף | קריטריוני יציאה | קבצים מצורפים |
|-------|-------|--------------------|
| טייס (P1) | מישור בקרה אחד + צומת מישור נתונים אחד לכל אזור | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` ≥90% במשך 48 שעות, אפס תקריות של Alertmanager ותרגיל חולף לאחור. | חבילה משני המארחים (JSONs ספסל, ייצוא Grafana עם הערת פיילוט, יומני החזרה לאחור). |
| רמפה (P2) | ≥50% מהמאמתים בתוספת נתיב ארכיון אחד לפחות לכל אשכול | ביצוע GPU נמשך במשך 5 ימים, לא יותר מנקודת שדרוג לאחור אחת של יותר מ-10 דקות, ומדדי Prometheus מוכיחים התראת נפילות בתוך שנות ה-60. | ייצוא Grafana מעודכן המציג את ביאור הרמפה, הבדלי גרידה Prometheus, צילום מסך/יומן של Alertmanager. |
| ברירת מחדל (P3) | צמתים שנותרו; FASTPQ מסומן כברירת מחדל ב-`iroha_config` | מניפסט ספסל חתום + יצוא Grafana המתייחס לעקומת האימוץ הסופית, ותרגיל החזרה לאחור מתועד המדגים את החלפת התצורה. | מניפסט סופי, Grafana JSON, יומן החזרה, הפניה לכרטיס לסקירת שינוי תצורה. |

תיעוד כל שלב בקידום בכרטיס ההפצה וקשר ישירות ל-
הערות `grafana_fastpq_acceleration.json` כדי שהבודקים יוכלו לתאם את
ציר הזמן עם הראיות.

## תרגילי החזרה

כל שלב השקה חייב לכלול חזרה חזרה:

1. בחר צומת אחד בכל אשכול ורשום את המדדים הנוכחיים:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2. כפה על מצב מעבד למשך 10 דקות באמצעות כפתור התצורה
   (`zk.fastpq.execution_mode = "cpu"`) או עקיפת הסביבה:
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3. אשר את יומן השדרוג לאחור
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) וגרד
   שוב את נקודת הקצה Prometheus כדי להציג את מרווחי המונה.
4. שחזר את מצב GPU, ודא ש-`telemetry::fastpq.execution_mode` מדווח כעת
   `resolved="metal"` (או `resolved="cuda"/"opencl"` עבור פסים שאינם מתכת),
   אשר שה-Prometheus גרידה מכילה גם את דגימות המעבד וה-GPU ב
   `fastpq_execution_mode_total{backend=…}`, ורשום את הזמן שחלף ל
   איתור/ניקוי.
5. אחסן תמלילי מעטפת, מדדים ואישורי מפעיל כ
   `rollback_drill.log` ו-`metrics_rollback.prom` בחבילת ההפצה. אלה
   קבצים חייבים להמחיש את מחזור השדרוג + השחזור המלא מכיוון
   `ci/check_fastpq_rollout.sh` נכשל כעת בכל פעם שביומן חסר ה-GPU
   שורת השחזור או תמונת המצב של המדדים משמיטים את מונה המעבד או ה-GPU.

יומנים אלו מוכיחים שכל אשכול יכול להתדרדר בחן וכי צוותי SRE
לדעת איך לרדת לאחור באופן דטרמיניסטי אם מנהלי התקנים של GPU או גרעינים נסוגים.

## עדות סתירה במצב מעורב (WP2-E.6)

בכל פעם שמארח צריך GPU FFT/LDE אבל CPU Poseidon hashing (ל-Stage7 <900ms
דרישה), אגד את החפצים הבאים לצד יומני ההחזרה הסטנדרטיים:1. **הבדל תצורה.** בדוק (או צרף) את החלפת המארח-מקומי שקובע
   `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`) בזמן היציאה
   `zk.fastpq.execution_mode` לא נגע. תן שם לתיקון
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`.
2. **גרידה נגד פוסידון.**
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   הלכידה חייבת להראות `path="cpu_forced"` בהגדלה בשלב הנעילה עם
   מונה GPU FFT/LDE עבור אותה סוג מכשיר. קח גרידה שנייה לאחר החזרה
   חזרה למצב GPU כדי שהבודקים יוכלו לראות את קורות החיים של שורה `path="gpu"`.

   העבירו את הקובץ שהתקבל ל-`wrap_benchmark.py --poseidon-metrics …` כך שהמדד העטוף יתעד את אותם מונים בתוך הקטע `poseidon_metrics` שלו; זה שומר על ההשקה של Metal ו-CUDA על זרימת העבודה זהה והופך את הראיות הנפילות לניתנות לביקורת מבלי לפתוח קבצי גרידה נפרדים.
3. **קטע יומן.** העתק את ערכי `telemetry::fastpq.poseidon` המוכיחים את
   פותר התהפך למעבד (`cpu_forced`) לתוך
   `poseidon_fallback.log`, שמירה על חותמות זמן כך שצירי הזמן של Alertmanager יהיו
   בקורלציה לשינוי התצורה.

CI אוכף את בדיקות התור/מילוי אפס היום; ברגע שהשער המעורב נוחת,
`ci/check_fastpq_rollout.sh` גם יעמוד על כך שכל צרור המכיל
`poseidon_fallback.patch` שולח את תמונת המצב התואמת `metrics_poseidon.prom`.
מעקב אחר זרימת עבודה זו שומר על מדיניות החלפה של WP2-E.6 ניתנת לביקורת וקשורה אליה
אותם אוספי ראיות שבהם השתמשו במהלך השקת ברירת המחדל.

## דיווח ואוטומציה

- צרף את כל ספריית `artifacts/fastpq_rollouts/<stamp>/` ל-
  שחרר כרטיס והפנה אליו מ-`status.md` לאחר סגירת ההשקה.
- הפעל את `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` (דרך
  `promtool`) בתוך CI כדי להבטיח חבילות התראה מצורפות עם עדיין השקה
  קומפילציה.
- אמת את החבילה באמצעות `ci/check_fastpq_rollout.sh` (או
  `make check-fastpq-rollout`) ועבור `FASTPQ_ROLLOUT_BUNDLE=<path>` כאשר אתה
  רוצה למקד להשקה בודדת. CI מפעיל את אותו סקריפט באמצעות
  `.github/workflows/fastpq-rollout.yml`, אז חפצים חסרים נכשלים מהר לפני א
  כרטיס שחרור יכול להיסגר. צינור השחרור יכול לאחסן חבילות מאומתות בארכיון
  לצד המניפסטים החתומים במעבר
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>` ל
  `scripts/run_release_pipeline.py`; העוזר משוחזר
  `ci/check_fastpq_rollout.sh` (אלא אם כן מוגדר `--skip-fastpq-rollout-check`) ו
  מעתיק את עץ הספריות לתוך `artifacts/releases/<version>/fastpq_rollouts/…`.
  כחלק משער זה, הסקריפט אוכף את עומק התור של Stage7 ומילוי אפס
  תקציבים על ידי קריאת `benchmarks.metal_dispatch_queue` ו
  `benchmarks.zero_fill_hotspots` מכל `metal` ספסל JSON.

על ידי ביצוע ספר משחק זה נוכל להדגים אימוץ דטרמיניסטי, ספק א
חבילת ראיות יחידה לכל השקה, ולצידן תרגילי החזרה מבוקרים
המדד החתום מופיע.