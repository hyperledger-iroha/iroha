<!-- Hebrew translation of docs/source/crypto/gost_performance.md -->

---
lang: he
direction: rtl
source: docs/source/crypto/gost_performance.md
status: complete
translator: manual
---

<div dir="rtl">

# וורקפלואו ביצועים ל-GOST

מסמך זה מתעד איך אנו עוקבים ואוכפים את מעטפת הביצועים של backend החתימות TC26 GOST.

## הרצה מקומית

```bash
make gost-bench                             # הרצת הבנצ'ים + בדיקת סבילות
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # שינוי סבילות
make gost-dudect                            # בדיקת constant-time
./scripts/update_gost_baseline.sh           # עזר לעדכון הבסיס
```

הפקודות משתמשות ב-`scripts/gost_bench.sh`, המבצע:

1. `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot`.
2. `gost_perf_check` מול `target/criterion`, בהשוואה ל-json הבסיס (`crates/iroha_crypto/benches/gost_perf_baseline.json`).
3. כתיבת סיכום Markdown ל-`$GITHUB_STEP_SUMMARY` אם משתנה זה קיים.

לרענון הבסיס לאחר אישור שינויים:

```bash
make gost-bench-update
```

או ישירות:

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`update_gost_baseline.sh` מריץ את הבנצ'ים, מעדכן את ה-json ומדפיס מדיאנים חדשים. הקפידו לקמבן את ה-json עם רישום ההחלטה ב-`crates/iroha_crypto/docs/gost_backend.md`.

### מדיאנים נוכחיים

| אלגוריתם              | Median (µs) |
|-----------------------|-------------|
| ed25519               | 69.67       |
| gost256_paramset_a    | 1136.96     |
| gost256_paramset_b    | 1129.05     |
| gost256_paramset_c    | 1133.25     |
| gost512_paramset_a    | 8944.39     |
| gost512_paramset_b    | 8963.60     |
| secp256k1             | 160.53      |

## CI

Workflow `gost-perf.yml` מריץ את אותו סקריפט ועוד את dudect להתראה על timing leaks. CI נכשל אם המדיאן חורג מהבסיס מעבר לסבילות (ברירת מחדל 20%) או אם dudect מזהה דליפה.

## סיכום

`gost_perf_check` מציג את ההשוואה מקומית ומוסיף אותה ל-`$GITHUB_STEP_SUMMARY`, כך שסיכומי הריצה מכילים את אותן תוצאות.

</div>
