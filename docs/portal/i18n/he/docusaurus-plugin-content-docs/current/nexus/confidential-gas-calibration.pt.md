---
lang: he
direction: rtl
source: docs/portal/docs/nexus/confidential-gas-calibration.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Livro de calibracao de gas סודי
תיאור: פרסומים רפואיים לשחרור מסמכים או פרסומים סודיים.
שבלול: /nexus/confidential-gas-calibration
---

# קוי הבסיס של כישורי הגז חסויים

Este registro acompanha os resultados validados dos benchmarks de calibracao de gas confidencial. Cada linha documenta um conjunto de medicoes de qualidade de release capturado com o procedimento decrito em [נכסים סודיים והעברות ZK](./confidential-assets#calibration-baselines--acceptance-gates).

| נתונים (UTC) | להתחייב | פרפיל | `ns/op` | `gas/op` | `ns/gas` | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-ניאון | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | בהמתנה | baseline-simd-neutral | - | - | - | Execucao neutra x86_64 programada no host CI `bench-x86-neon0`; ver כרטיס GAS-214. Os resultados serao adicionados quando a janela de bench terminar (רשימת בדיקה לפני מיזוג מירה או גרסה 2.1). |
| 2026-04-13 | בהמתנה | baseline-avx2 | - | - | - | Calibracao AVX2 de acompanhamento usando o mesmo commit/build da execucao neutra; בקש מארח `bench-x86-avx2a`. GAS-214 cobre as duas execucoes com comparacao de delta contra `baseline-neon`. |

`ns/op` agrega a mediana de wall-clock por instrucao medida pelo קריטריון; `gas/op` e a media aritmetica dos custos de schedule correspondentes de `iroha_core::gas::meter_instruction`; `ns/gas` divide os nanosegundos somados pelo gas somado no conjunto de nove instrucos.

*הערה* O host arm64 atual nao emite resumos `raw.csv` do Criterion por padrao; rode novamente com `CRITERION_OUTPUT_TO=csv` או uma correcao במעלה הזרם antes de etiquetar um release para que os artefatos exigidos ela checklist de aceitacao sejam anexados. ראה `target/criterion/` ainda estiver ausente apos `--save-baseline`, עשה ביצוע של לינוקס מארח או סידור של קונסולה ללא חבילה לשחרור como stopgap temporario. Para referencia, o log de console arm64 da ultima execucao fica em `docs/source/confidential_assets_calibration_neon_20251018.log`.

Medianas por instrucao da mesma execucao (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Instrucao | mediana `ns/op` | לוח זמנים `gas` | `ns/gas` |
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

לוח זמנים של קולונה e imposta por `gas::tests::calibration_bench_gas_snapshot` (סה"כ 1,413 גז ללא הוראות חדשות) e vai falhar se patches futuros mudarem o metering sem atualizar os fixtures de calibracao.