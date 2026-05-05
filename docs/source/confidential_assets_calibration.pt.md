---
lang: pt
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2026-01-03T18:07:57.759135+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Linhas de base de calibração de gás confidencial

Este livro-razão rastreia os resultados validados da calibração confidencial de gás
benchmarks. Cada linha documenta um conjunto de medidas de qualidade de lançamento capturado com
o procedimento descrito em `docs/source/confidential_assets.md#calibration-baselines--acceptance-gates`.

| Data (UTC) | Confirmar | Perfil | `ns/op` | `gas/op` | `ns/gas` | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| 18/10/2025 | 3c70a7d3 | linha de base-néon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (informações do host); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 28/04/2026 | 8ea9b2a7 | linha de base-neon-20260428 | 4.29e6 | 1.57e2 | 2.73e4 | Darwin 25.0.0 braço64 (`rustc 1.91.0`). Comando: `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`; faça login em `docs/source/confidential_assets_calibration_neon_20260428.log`. As execuções de paridade x86_64 (SIMD-neutro + AVX2) estão programadas para o slot do laboratório de Zurique em 19/03/2026; os artefatos chegarão em `artifacts/confidential_assets_calibration/2026-03-x86/` com comandos correspondentes e serão mesclados na tabela de linha de base uma vez capturados. |
| 28/04/2026 | — | linha de base-simd-neutro | — | — | — | **Dispensado** no Apple Silicon — `ring` impõe NEON para a plataforma ABI, portanto, `RUSTFLAGS="-C target-feature=-neon"` falha antes que o banco possa ser executado (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`). Os dados neutros permanecem bloqueados no host CI `bench-x86-neon0`. |
| 28/04/2026 | — | linha de base-avx2 | — | — | — | **Adiado** até que um executor x86_64 esteja disponível. `arch -x86_64` não pode gerar binários nesta máquina (“Tipo de CPU incorreto no executável”; consulte `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`). O host CI `bench-x86-avx2a` continua sendo a fonte de registro. |

`ns/op` agrega o clock médio por instrução medido pelo Critério;
`gas/op` é a média aritmética dos custos do cronograma correspondentes de
`iroha_core::gas::meter_instruction`; `ns/gas` divide os nanossegundos somados por
o gás somado em todo o conjunto de amostras de nove instruções.

*Nota.* O host arm64 atual não emite resumos do Critério `raw.csv` de
a caixa; execute novamente com `CRITERION_OUTPUT_TO=csv` ou uma correção upstream antes de marcar um
liberação para que os artefatos exigidos pela lista de verificação de aceitação sejam anexados.
Se `target/criterion/` ainda estiver faltando após `--save-baseline`, colete a execução
em um host Linux ou serializar a saída do console no pacote de lançamento como um
paliativo temporário. Para referência, o log do console arm64 da última execução
mora em `docs/source/confidential_assets_calibration_neon_20251018.log`.

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

### 28/04/2026 (Apple Silicon, NEON habilitado)

Latências médias para a atualização de 28/04/2026 (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`):| Instrução | mediana `ns/op` | agendar `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegistrarDomínio | 8.58e6 | 200 | 4.29e4 |
| Registrar conta | 4.40e6 | 200 | 2.20e4 |
| RegistrarAssetDef | 4.23e6 | 200 | 2.12e4 |
| SetAccountKV_small | 3.79e6 | 67 | 5.66e4 |
| GrantAccountRole | 3.60e6 | 96 | 3.75e4 |
| RevogarAccountRole | 3.76e6 | 96 | 3.92e4 |
| ExecuteTrigger_empty_args | 2.71e6 | 224 | 1.21e4 |
| MintAsset | 3.92e6 | 150 | 2.61e4 |
| Transferir ativo | 3.59e6 | 180 | 1.99e4 |

Os agregados `ns/op` e `ns/gas` na tabela acima são derivados da soma de
essas medianas (total `3.85717e7`ns em todo o conjunto de nove instruções e 1.413
unidades de gás).

A coluna de agendamento é imposta por `gas::tests::calibration_bench_gas_snapshot`
(total de 1.413 gases no conjunto de nove instruções) e irá desarmar se patches futuros
alterar a medição sem atualizar os acessórios de calibração.

## Evidência de Telemetria da Árvore de Compromisso (M2.2)

De acordo com a tarefa do roteiro **M2.2**, cada execução de calibração deve capturar o novo
medidores de árvore de compromissos e contadores de despejo para provar que a fronteira Merkle permanece
dentro dos limites configurados:

-`iroha_confidential_tree_commitments{asset_id}`
-`iroha_confidential_tree_depth{asset_id}`
-`iroha_confidential_root_history_entries{asset_id}`
-`iroha_confidential_frontier_checkpoints{asset_id}`
-`iroha_confidential_frontier_last_checkpoint_height{asset_id}`
-`iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
-`iroha_confidential_root_evictions_total{asset_id}`
-`iroha_confidential_frontier_evictions_total{asset_id}`
-`iroha_zk_verifier_cache_events_total{cache,event}`

Registre os valores imediatamente antes e depois da carga de trabalho de calibração. Um
um único comando por ativo é suficiente; exemplo para `4cuvDVPuLBKJyN6dPbRQhmLh68sU`:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="4cuvDVPuLBKJyN6dPbRQhmLh68sU"}'
```

Anexe a saída bruta (ou instantâneo Prometheus) ao tíquete de calibração para que o
O revisor de governança pode confirmar que os limites do histórico raiz e os intervalos dos pontos de verificação estão
honrado. O guia de telemetria em `docs/source/telemetry.md#confidential-tree-telemetry-m22`
expande as expectativas de alerta e os painéis Grafana associados.

Inclua os contadores de cache do verificador no mesmo scrape para que os revisores possam confirmar
a taxa de falta permaneceu abaixo do limite de aviso de 40%:

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

Documente a proporção derivada (`miss / (hit + miss)`) dentro da nota de calibração
para mostrar que os exercícios de modelagem de custos neutros do SIMD reutilizaram caches quentes em vez de
destruindo o registro do verificador Halo2.

## Isenção neutra e AVX2

O SDK Council concedeu uma isenção temporária para o portão PhaseC exigindo
Medições `baseline-simd-neutral` e `baseline-avx2`:

- **SIMD neutro:** No Apple Silicon, o back-end criptográfico `ring` impõe NEON para
  Correção da ABI. Desativando o recurso (`RUSTFLAGS="-C target-feature=-neon"`)
  anula a construção antes que o binário de bancada seja produzido (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`).
- **AVX2:** O conjunto de ferramentas local não pode gerar binários x86_64 (`arch -x86_64 rustc -V`
  → “Tipo de CPU incorreto no executável”; veja
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`).

Até que os hosts CI `bench-x86-neon0` e `bench-x86-avx2a` estejam online, o NEON é executado
acima mais a evidência de telemetria satisfazem os critérios de aceitação da FaseC.
A renúncia é registrada em `status.md` e será revisada assim que o hardware x86 for
disponível.