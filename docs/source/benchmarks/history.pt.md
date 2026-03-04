---
lang: pt
direction: ltr
source: docs/source/benchmarks/history.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aad1366bd823bddaca32dc82573d41ec6572a6d9f969dc1e0c6146ea068e03e
source_last_modified: "2026-01-03T18:08:00.425813+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Histórico de captura de benchmark de GPU (FASTPQ WP5-B)

Este arquivo é gerado por `python3 scripts/fastpq/update_benchmark_history.py`.
Ele satisfaz a entrega do FASTPQ Stage 7 WP5-B rastreando cada GPU empacotada
artefato de referência, o manifesto do microbanco Poseidon e varreduras auxiliares sob
`benchmarks/`. Atualize as capturas subjacentes e execute novamente o script sempre que um novo
agrupar terras ou telemetria precisa de novas evidências.

## Escopo e processo de atualização

- Produzir ou agrupar novas capturas de GPU (via `scripts/fastpq/wrap_benchmark.py`),
  anexe-os à matriz de captura e execute novamente este gerador para atualizar o
  tabelas.
- Quando os dados do microbench Poseidon estiverem presentes, exporte-os com
  `scripts/fastpq/export_poseidon_microbench.py` e reconstrua o manifesto usando
  `scripts/fastpq/aggregate_poseidon_microbench.py`.
- Registre varreduras de limite Merkle armazenando suas saídas JSON em
  `benchmarks/merkle_threshold/`; este gerador lista os arquivos conhecidos para auditorias
  pode fazer referência cruzada à disponibilidade de CPU e GPU.

## Benchmarks de GPU do FASTPQ Estágio 7

| Pacote | Back-end | Modo | Back-end da GPU | GPU disponível | Classe de dispositivo | GPU | LDE ms (CPU/GPU/SU) | Poseidon MS (CPU/GPU/SU) |
|---|---------|------|------------|---------------|-------------|-----|-----------|---------------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | cuda | GPU | cuda-sm80 | sim | xeon-rtx | NVIDIA RTX 6000 Ada | 1512,9/880,7/1,72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | metais | GPU | nenhum | sim | maçã-m4 | GPU Apple de 40 núcleos | 785,6/735,6/1,07 | 1803,8/1897,5/0,95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | metais | GPU | metais | sim | apple-m2-ultra | Apple M2 Ultra | 1581,1/1604,5/0,98 | 3589,9/3697,3/0,97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | metais | GPU | metais | sim | apple-m2-ultra | Apple M2 Ultra | 1804,5/1666,4/1,08 | 3939,5/4083,3/0,96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | metais | GPU | metais | sim | apple-m2-ultra | Apple M2 Ultra | 1804,5/1666,4/1,08 | 3939,5/4083,3/0,96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | abrircl | GPU | abrircl | sim | neoverse-mi300 | AMD Instinto MI300A | 4518,5/688,9/6,56 | 2780,4/905,6/3,07 |

> Colunas: `Backend` é derivado do nome do pacote; `Mode`/`GPU backend`/`GPU available`
> são copiados do bloco `benchmarks` empacotado para expor substitutos de CPU ou GPU ausente
> descoberta (por exemplo, `gpu_backend=none` apesar de `Mode=gpu`). SU = taxa de aceleração (CPU/GPU).

## Instantâneos do Microbanco Poseidon

`benchmarks/poseidon/manifest.json` agrega o Poseidon padrão versus escalar
execuções de microbench exportadas de cada pacote de metal. A tabela abaixo é atualizada por
o script do gerador, para que as revisões de CI e governança possam diferenciar acelerações históricas
sem descompactar os relatórios FASTPQ empacotados.

| Resumo | Pacote | Carimbo de data/hora | MS padrão | MS escalar | Aceleração |
|---------|--------|-----------|------------|-----------|---------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167,7 | 2152,2 | 0,99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | 1990,5 | 1994,5 | 1,00 |

## Varreduras de Limiar MerkleCapturas de referência coletadas via
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
viver sob `benchmarks/merkle_threshold/`. As entradas da lista mostram se o host
dispositivos metálicos expostos durante a varredura; As capturas habilitadas para GPU devem relatar
`metal_available=true`.

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

A captura Apple Silicon (`takemiyacStudio.lan_25.0.0_arm64`) é a linha de base canônica da GPU usada em `docs/source/benchmarks.md`; as entradas do macOS 14 permanecem como linhas de base somente de CPU para ambientes que não podem expor dispositivos Metal.

## Instantâneos de uso de linha

Decodificações de testemunhas capturadas via `scripts/fastpq/check_row_usage.py` comprovam a transferência
eficiência de linha do gadget. Mantenha os artefatos JSON em `artifacts/fastpq_benchmarks/`
e este gerador resumirá as taxas de transferência registradas para os auditores.

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — lotes=2, taxa_de_transferência média=0,629 (mín=0,625, máx=0,633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — lotes=2, taxa_de_transferência média=0,619 (mín=0,613, máx=0,625)