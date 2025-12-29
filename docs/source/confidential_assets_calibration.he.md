<!-- Hebrew translation of docs/source/confidential_assets_calibration.md -->

---
lang: he
direction: rtl
source: docs/source/confidential_assets_calibration.md
status: complete
translator: manual
---

<div dir="rtl">

# בסיסי כיול גז לנכסים חסויים

המחברת מתעדת את פלטי הכיול המאומתים של בנצ׳מרקי הגז עבור יכולות חסויות. כל שורה משקפת מדידה באיכות שחרור שנאספה לפי הנוהל ב-`docs/source/confidential_assets.md#calibration-baselines--acceptance-gates`.

| Date (UTC) | Commit | Profile | `ns/op` | `gas/op` | `ns/gas` | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-neon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (`hostinfo`); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pending | baseline-simd-neutral | — | — | — | הרצה ניטרלית מתוזמנת ל-x86_64 על מארח CI `bench-x86-neon0`; ראו כרטסת GAS-214. התוצאות יתווספו לאחר סיום חלון הבנצ׳ (בדיקת טרום-מיזוג מכוונת לשחרור ‎2.1‎). |
| 2026-04-13 | pending | baseline-avx2 | — | — | — | כיול AVX2 המשך, משתמש באותו קומיט ובנייה כמו הריצה הניטרלית; דורש את `bench-x86-avx2a`. כרטסת GAS-214 מכסה את שתי הריצות ומשווה דלתא מול `baseline-neon`. |

`ns/op` הוא חציון זמן-הקיר לכל הוראה שנמדד ב-Criterion; `gas/op` הוא הממוצע האריתמטי של עלויות הלוח זמנים מ-`iroha_core::gas::meter_instruction`; ‏`ns/gas` מחשב סכום ננו-שניות חלקי סכום הגז עבור תשעת ההוראות במדגם.

*הערה:* מארח arm64 הנוכחי אינו מפיק `raw.csv` של Criterion כברירת מחדל; לפני תג שחרור, הריצו עם `CRITERION_OUTPUT_TO=csv` או החילו תיקון upstream כדי לצרף את הארכיון הנדרש לאימות. אם `target/criterion/` עדיין חסר לאחר `--save-baseline`, אספו את הריצה על מארח Linux או סדרו את פלט המסוף כחלק מחבילת השחרור. יומן המסוף האחרון שמור בקובץ `docs/source/confidential_assets_calibration_neon_20251018.log`.

חציון לכל הוראה מאותה ריצה:

| Instruction | median `ns/op` | schedule `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| RegisterAccount | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

עמודת ה-`gas` נאכפת ב-`gas::tests::calibration_bench_gas_snapshot` (סה״כ 1,413 גז). כל שינוי מדידת גז עתידי יחייב עדכון הפיקסצ׳רים.

</div>
