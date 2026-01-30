---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/nexus/confidential-gas-calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bb4ff674c64756bd438cc5aca9f800c4e1c223954992e0e8c3e305aac746d55
source_last_modified: "2025-11-14T04:43:20.322583+00:00"
translation_last_reviewed: 2026-01-30
---

# קווי בסיס לכיול גז חסוי

יומן זה עוקב אחר הפלטים המאומתים של מדדי כיול הגז החסוי. כל שורה מתעדת סט מדידות ברמת שחרור שנאסף לפי ההליך המתואר ב-[Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates).

| תאריך (UTC) | Commit | פרופיל | `ns/op` | `gas/op` | `ns/gas` | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-neon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pending | baseline-simd-neutral | - | - | - | ריצה ניטרלית x86_64 מתוזמנת על מארח CI `bench-x86-neon0`; ראו כרטיס GAS-214. התוצאות יתווספו לאחר סיום חלון ה-bench (צ'קליסט pre-merge מכוון ל-release 2.1). |
| 2026-04-13 | pending | baseline-avx2 | - | - | - | כיול AVX2 המשך תוך שימוש באותו commit/build של הריצה הניטרלית; דורש מארח `bench-x86-avx2a`. GAS-214 מכסה את שתי הריצות עם השוואת דלתא מול `baseline-neon`. |

`ns/op` מציג את חציון זמן הקיר לכל הוראה שנמדד באמצעות Criterion; `gas/op` הוא הממוצע האריתמטי של עלויות ה-schedule המתאימות מ-`iroha_core::gas::meter_instruction`; ו-`ns/gas` מחלק את סך הננו-שניות בסך הגז על פני סט תשע ההוראות.

*הערה.* מארח arm64 הנוכחי אינו מפיק סיכומי Criterion `raw.csv` כברירת מחדל; הריצו מחדש עם `CRITERION_OUTPUT_TO=csv` או תיקון upstream לפני תגית release כדי לצרף את הארטיפקטים הנדרשים בצ'קליסט הקבלה. אם `target/criterion/` עדיין חסר לאחר `--save-baseline`, אספו את הריצה על מארח Linux או סרייליזו את פלט הקונסול לבנדל ה-release כפתרון זמני. לעיון, לוג הקונסול arm64 מהריצה האחרונה נמצא ב-`docs/source/confidential_assets_calibration_neon_20251018.log`.

חציון לכל הוראה מאותה ריצה (`cargo bench -p iroha_core --bench isi_gas_calibration`):

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

עמודת ה-schedule נאכפת על ידי `gas::tests::calibration_bench_gas_snapshot` (סך 1,413 גז על פני סט תשע ההוראות) ותכשל אם תיקונים עתידיים ישנו את המדידה ללא עדכון fixtures הכיול.
