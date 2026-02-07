---
lang: he
direction: rtl
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2026-01-03T18:08:01.692664+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha סוויטת 3 ספסלים

חבילת הספסל Iroha 3 פי כמה מהנתיבים החמים עליהם אנו מסתמכים במהלך ההימור, בתשלום
טעינה, אימות הוכחה, תזמון ונקודות קצה הוכחה. זה פועל בתור
פקודת `xtask` עם מתקנים דטרמיניסטיים (זרעים קבועים, חומר מפתח קבוע,
ומטעני בקשות יציבים) כך שניתן לשחזר את התוצאות על פני מארחים.

## הפעלת הסוויטה

```bash
cargo xtask i3-bench-suite \
  --iterations 64 \
  --sample-count 5 \
  --json-out benchmarks/i3/latest.json \
  --csv-out benchmarks/i3/latest.csv \
  --markdown-out benchmarks/i3/latest.md \
  --threshold benchmarks/i3/thresholds.json \
  --allow-overwrite
```

דגלים:

- `--iterations` שולט באיטרציות לכל מדגם תרחיש (ברירת מחדל: 64).
- `--sample-count` חוזר על כל תרחיש כדי לחשב את החציון (ברירת מחדל: 5).
- `--json-out|--csv-out|--markdown-out` בחר חפצי פלט (הכל אופציונלי).
- `--threshold` משווה חציונים מול גבולות קו הבסיס (קבע `--no-threshold`
  לדלג).
- `--flamegraph-hint` מציין את דוח Markdown עם `cargo flamegraph`
  פקודה לפרופיל תרחיש.

דבק CI חי ב-`ci/i3_bench_suite.sh` וברירת המחדל לנתיבים שלמעלה; להגדיר
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` כדי לכוונן את זמן הריצה בשידורי לילה.

## תרחישים

- `fee_payer` / `fee_sponsor` / `fee_insufficient` - חיוב המשלם מול נותן החסות
  ודחיית מחסור.
- `staking_bond` / `staking_slash` — תור איחוי/ללא קשר עם ובלי
  חיתוך.
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` —
  אימות חתימה על אישורי התחייבות, אישורי JDG וגשר
  מטענים הוכחה.
- `commit_cert_assembly` - מכלול עיכול עבור תעודות התחייבות.
- `access_scheduler` - תזמון ערכת גישה מודע לקונפליקט.
- `torii_proof_endpoint` — ניתוח נקודת קצה בהוכחת Axum + אימות הלוך ושוב.

כל תרחיש מתעד חציון ננו-שניות לכל איטרציה, תפוקה ו-a
מונה הקצאה דטרמיניסטית עבור רגרסיות מהירות. ספים חיים ב
`benchmarks/i3/thresholds.json`; גבשושית שם כאשר החומרה משתנה ו
לבצע את החפץ החדש לצד דוח.

## פתרון בעיות

- הצמד את תדר המעבד/מושל בעת איסוף ראיות כדי למנוע רגרסיות רועשות.
- השתמש ב-`--no-threshold` עבור ריצות חקירה, ולאחר מכן הפעל מחדש לאחר שקו הבסיס הוא
  רענן.
- כדי ליצור פרופיל של תרחיש בודד, הגדר את `--iterations 1` והפעל מחדש תחת
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`.