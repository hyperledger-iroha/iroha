---
lang: he
direction: rtl
source: docs/portal/docs/nexus/confidential-gas-calibration.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Registre de calibration du gas confidentiel
תיאור: אמצעי שחרור איכותיים המאפשרים חסיון לוח שנה גז.
שבלול: /nexus/confidential-gas-calibration
---

# כיול בסיסי של גז סודי

Ce registre suit les sorties validees des benchmarks de calibration du gas confidentiel. תעודת המסמכים הזו היא ללכוד שחרור איכותי עם תקנון ההליך [נכסים סודיים והעברות ZK](./confidential-assets#calibration-baselines--acceptance-gates).

| תאריך (UTC) | להתחייב | פרופיל | `ns/op` | `gas/op` | `ns/gas` | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-ניאון | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | בהמתנה | baseline-simd-neutral | - | - | - | סירוס ביצוע x86_64 planifiee sur l'hote CI `bench-x86-neon0`; כרטיס voir GAS-214. Les תוצאות seront ajoutes une fois la fenetre de bench terminee (רשימת הבדיקה לפני המיזוג לשחרור 2.1). |
| 2026-04-13 | בהמתנה | baseline-avx2 | - | - | - | כיול AVX2 de suivi utilisant le meme commit/build que l'execution neutre; דרוש l'hote `bench-x86-avx2a`. GAS-214 couvre les deux runs avec השוואה delta contre `baseline-neon`. |

`ns/op` agrege la שעון קיר אמצעי לפי הוראת תקן ערך; `gas/op` est la moyenne arithmetique des couts de schema correspondants de `iroha_core::gas::meter_instruction`; `ns/gas` לחלק les nanosecondes sommees par le gas somme sur l'ensemble de neuf הוראות.

*הערה.* L'hote arm64 actuel ne produit pas les resumes `raw.csv` de Criterion par defaut; relancez avec `CRITERION_OUTPUT_TO=csv` או תיקון אחד במעלה הזרם אומנות נימוס ושחרור אפינ que les artefacts דרוש par la checklist d'acceptation soient attaches. Si `target/criterion/` מנקה הדרן לאחר `--save-baseline`, ביצוע ביצוע על לינוקס חם או סדרה של קונסולת מסע בצרור שחרור בואכה הפסקה זמנית. כותרת דה רפרנס, le log console arm64 de la derniere ביצוע se trouve dans `docs/source/confidential_assets_calibration_neon_20251018.log`.

Medianes par instruction de la meme execution (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| הדרכה | אמצע `ns/op` | לוח זמנים `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| הרשמה חשבון | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

לוח הזמנים של הקולונים הוטל על פי `gas::tests::calibration_bench_gas_snapshot` (סה"כ 1,413 הוראות גז על מנת לכלול הוראות) ואקוורה לשינוי עתידי של תיקון מדידה ללא כיול מתקנים.