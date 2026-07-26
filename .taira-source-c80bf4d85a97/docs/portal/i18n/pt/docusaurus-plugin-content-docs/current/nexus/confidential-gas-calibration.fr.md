---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Registro de calibração do gás confidencial
descrição: Medidas de liberação de qualidade que determinam o calendário de gás confidencial.
slug: /nexus/confidential-gas-calibration
---

# Linhas de base de calibração do gás confidencial

Este registro atende às saídas válidas dos benchmarks de calibração do gás confidencial. Cada linha documenta um jogo de medidas de captura de liberação de qualidade com o procedimento descrito em [Ativos Confidenciais e Transferências ZK] (./confidential-assets#calibration-baselines--acceptance-gates).

| Data (UTC) | Confirmar | Perfil | `ns/op` | `gas/op` | `ns/gas` | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| 18/10/2025 | 3c70a7d3 | linha de base-néon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (informações do host); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 12/04/2026 | pendente | linha de base-simd-neutro | - | - | - | Execução neutra x86_64 planejada no hotel CI `bench-x86-neon0`; voir ticket GAS-214. Os resultados serão adicionados quando a janela do banco terminar (a lista de verificação pré-mesclagem está na versão 2.1). |
| 13/04/2026 | pendente | linha de base-avx2 | - | - | - | A calibração AVX2 de suivi usa o meme commit/build que a execução é neutra; solicite o hotel `bench-x86-avx2a`. GAS-214 cobre os dois é executado com comparação delta contra `baseline-neon`. |

`ns/op` agrega o relógio de parede médio por instrução medida por critério; `gas/op` é a média aritmética dos cálculos de programação correspondentes a `iroha_core::gas::meter_instruction`; `ns/gas` divide os nanossegundos somados pelo gás somme no conjunto de novas instruções.

*Nota.* O hotel arm64 atualmente não é produzido nos currículos `raw.csv` de critério por padrão; relancez com `CRITERION_OUTPUT_TO=csv` ou uma correção upstream antes de etiquetar uma liberação para que os artefatos exigidos pela lista de verificação de aceitação sejam anexados. Se `target/criterion/` mais uma vez após `--save-baseline`, capture a execução em um Linux quente ou serialize o console de sortie no pacote de lançamento como provisório temporário. Como título de referência, o log console arm64 da última execução é encontrado em `docs/source/confidential_assets_calibration_neon_20251018.log`.

Mediantes da instrução de execução do meme (`cargo bench -p iroha_core --bench isi_gas_calibration`):

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

A programação de colunas é imposta por `gas::tests::calibration_bench_gas_snapshot` (total de 1.413 gases no conjunto de novas instruções) e ecoa se os corretores futuros modificam a medição sem medir um dia os acessórios de calibração.