---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Реестр калибровки конфиденциального газа
description: Измерения уровня релиза, подтверждающие график конфиденциального газа.
slug: /nexus/confidential-gas-calibration
---

# Базовые линии калибровки конфиденциального газа

Este teste fornece resultados comprovados de calibração de calibração de sinal de conexão. Каждая строка документирует набор измерений уровня релиза, собранный по процедуре из [Ativos Confidenciais & ZK Transferências](./confidential-assets#calibration-baselines--acceptance-gates).

| Data (UTC) | Confirmar | Perfil | `ns/op` | `gas/op` | `ns/gas` | Nomeação |
| --- | --- | --- | --- | --- | --- | --- |
| 18/10/2025 | 3c70a7d3 | linha de base-néon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (informações do host); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 12/04/2026 | pendente | linha de base-simd-neutro | - | - | - | Запланированный нейтральный programa x86_64 no CI-хосте `bench-x86-neon0`; sim. bilhete GAS-214. Результаты будут добавлены после завершения окна бенча (pré-mesclagem чеклист ориентирован на релиз 2.1). |
| 13/04/2026 | pendente | linha de base-avx2 | - | - | - | A calibração AVX2 é definida como um commit/build, este e um programa neutro; требуется хост `bench-x86-avx2a`. O GAS-214 é compatível com o programa `baseline-neon`. |

`ns/op` агрегирует медиану relógio de parede em инструкцию, измеренную Critério; `gas/op` - esta aritmética é facilmente encontrada em `iroha_core::gas::meter_instruction`; `ns/gas` fornece um resumo do item de resumo para instruções de operação.

*Exemplo.* O teclado arm64 não é usado Critério `raw.csv`; verifique com `CRITERION_OUTPUT_TO=csv` ou avalie o upstream para obter a confiabilidade da configuração, artefatos e artefatos чеклистом приемки, были приложены. O `target/criterion/` é melhor que o `--save-baseline`, você pode usar o programa Linux ou serializá-lo консоли в релизный бандл как временный paliativo. Para isso, arm64 консольный лог последнего прогона находится в `docs/source/confidential_assets_calibration_neon_20251018.log`.

Medições de instruções de outro programa (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Instrução | mediana `ns/op` | agendar `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegistrarDomínio | 3.46e5 | 200 | 1.73e3 |
| Registrar conta | 3.15e5 | 200 | 1.58e3 |
| RegistrarAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevogarAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| Transferir ativo | 3.68e5 | 180 | 2.04e3 |

Cronograma de coluna обеспечивается `gas::tests::calibration_bench_gas_snapshot` (cerca de 1.413 gás por набору из девяти инструкций) e вызовет ошибку, если Esta configuração permite que a medição seja feita sem a configuração da calibração.