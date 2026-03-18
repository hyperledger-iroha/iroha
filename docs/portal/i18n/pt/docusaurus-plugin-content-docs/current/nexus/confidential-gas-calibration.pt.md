---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Livro de calibração de gás confidencial
descrição: Medicamentos de qualidade de lançamento que sustentam o cronograma de gás confidencial.
slug: /nexus/confidential-gas-calibration
---

# Baselines de calibração de gás confidencial

Este registro acompanha os resultados validados dos benchmarks de calibração de gás confidencial. Cada linha documenta um conjunto de medicamentos de qualidade de liberação capturados com o procedimento descrito em [Ativos Confidenciais e Transferências ZK](./confidential-assets#calibration-baselines--acceptance-gates).

| Dados (UTC) | Confirmar | Perfil | `ns/op` | `gas/op` | `ns/gas` | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| 18/10/2025 | 3c70a7d3 | linha de base-néon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (informações do host); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 12/04/2026 | pendente | linha de base-simd-neutro | - | - | - | Execução neutra x86_64 programada no host CI `bench-x86-neon0`; ver bilhete GAS-214. Os resultados serão aumentados quando a janela do banco terminar (a checklist pre-merge mira o release 2.1). |
| 13/04/2026 | pendente | linha de base-avx2 | - | - | - | Calibração AVX2 de acompanhamento usando o mesmo commit/build da execução neutra; requer o host `bench-x86-avx2a`. GAS-214 cobre as duas execuções com comparação de delta contra `baseline-neon`. |

`ns/op` adiciona a mediana do relógio de parede por instrução medida pelo Criterion; `gas/op` e a mídia aritmética dos custos de cronograma correspondentes a `iroha_core::gas::meter_instruction`; `ns/gas` divide os nanosegundos somados pelo gás somado no conjunto de nove instruções.

*Nota.* O host arm64 atual não emite resumos `raw.csv` do Criterion por padrão; rode novamente com `CRITERION_OUTPUT_TO=csv` ou uma correção upstream antes de etiquetar um release para que os artefatos exigidos pela checklist de aceitação sejam anexados. Se `target/criterion/` ainda estiver ausente após `--save-baseline`, colete a execução em um host Linux ou serialize a saida do console no bundle de release como stopgap temporário. Para referência, o log do console arm64 da última execução fica em `docs/source/confidential_assets_calibration_neon_20251018.log`.

Medianas por instrução da mesma execução (`cargo bench -p iroha_core --bench isi_gas_calibration`):

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

A coluna cronograma e definida por `gas::tests::calibration_bench_gas_snapshot` (total de 1,413 gás no conjunto de novas instruções) e vai falhar se patches futuros mudarem o medidor sem atualizar os equipamentos de calibração.