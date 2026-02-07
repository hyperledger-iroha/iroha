---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Livro prefeito de calibração de gás confidencial
description: Medições de qualidade de liberação que respaldam o calendário de gás confidencial.
slug: /nexus/confidential-gas-calibration
---

# Linhas base de calibração de gás confidencial

Este registro rastreia os resultados validados dos benchmarks de calibração de gás confidenciais. Cada fila documenta um conjunto de medidas de qualidade de liberação capturadas com o procedimento descrito em [Ativos Confidenciais e Transferências ZK](./confidential-assets#calibration-baselines--acceptance-gates).

| Data (UTC) | Confirmar | Perfil | `ns/op` | `gas/op` | `ns/gas` | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| 18/10/2025 | 3c70a7d3 | linha de base-néon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (informações do host); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 12/04/2026 | pendente | linha de base-simd-neutro | - | - | - | Execução neutra x86_64 programada no host CI `bench-x86-neon0`; ver bilhete GAS-214. Os resultados serão adicionados quando a janela do banco terminar (a lista de verificação pré-mesclagem está na versão 2.1). |
| 13/04/2026 | pendente | linha de base-avx2 | - | - | - | Calibração AVX2 posterior usando o mesmo commit/build que a corrida neutra; requer o host `bench-x86-avx2a`. GAS-214 cubre ambas as corridas com comparação de delta contra `baseline-neon`. |

`ns/op` adiciona a mediana de tempo de pared por instrução medida por Criterion; `gas/op` é a mídia aritmética dos custos de programação correspondentes de `iroha_core::gas::meter_instruction`; `ns/gas` divide os nanosegundos sumados entre o gás sumado no conjunto de novas instruções.

*Nota.* O host arm64 atual não emite currículos `raw.csv` do critério por defeito; Vuelve a executar com `CRITERION_OUTPUT_TO=csv` ou uma correção upstream antes de marcar uma liberação para que os artefatos exigidos pela lista de verificação de aceitação quedenham adjuntos. Se `target/criterion/` continuar faltando após `--save-baseline`, colete a execução em um host Linux ou serialize a saída de console no pacote de lançamento como paliativo temporal. Como referência, o log de console arm64 da última corrida vive em `docs/source/confidential_assets_calibration_neon_20251018.log`.

Medianas por instrução da mesma corrida (`cargo bench -p iroha_core --bench isi_gas_calibration`):

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

A coluna de cronograma é imposta por `gas::tests::calibration_bench_gas_snapshot` (total 1.413 gás no conjunto de novas instruções) e falhará se secas futuras mudarem a medição sem atualizar os dispositivos de calibração.