---
lang: pt
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2026-01-03T18:08:01.692664+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha Suíte de 3 bancos

O conjunto de bancada Iroha 3 cronometra os caminhos quentes em que confiamos durante o piqueteamento, taxa
cobrança, verificação de prova, agendamento e pontos de extremidade de prova. Funciona como um
Comando `xtask` com acessórios determinísticos (sementes fixas, material de chave fixa,
e cargas úteis de solicitação estáveis) para que os resultados sejam reproduzíveis entre hosts.

## Executando a suíte

```bash
cargo xtask i3-bench-suite \
  --iterations 64 \
  --sample-count 5 \
  --json-out benchmarks/i3/latest.json \
  --csv-out benchmarks/i3/latest.csv \
  --markdown-out benchmarks/i3/latest.md \
  --threshold benchmarks/i3/thresholds.json \
  --allow-overwrite
```

Bandeiras:

- `--iterations` controla iterações por amostra de cenário (padrão: 64).
- `--sample-count` repete cada cenário para calcular a mediana (padrão: 5).
- `--json-out|--csv-out|--markdown-out` escolha artefatos de saída (todos opcionais).
- `--threshold` compara medianas com os limites da linha de base (definir `--no-threshold`
  pular).
- `--flamegraph-hint` anota o relatório Markdown com o `cargo flamegraph`
  comando para criar o perfil de um cenário.

A cola CI reside em `ci/i3_bench_suite.sh` e o padrão é os caminhos acima; definir
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` para ajustar o tempo de execução nas noites.

## Cenários

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — débito do pagador versus patrocinador
  e rejeição de déficit.
- `staking_bond` / `staking_slash` — fila de ligação/desvinculação com e sem
  cortando.
-`commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` —
  verificação de assinatura em certificados de commit, atestados JDG e ponte
  cargas úteis de prova.
- `commit_cert_assembly` — compilação resumida para certificados de commit.
- `access_scheduler` — agendamento de conjunto de acesso com reconhecimento de conflito.
- `torii_proof_endpoint` — Análise de endpoint à prova de Axum + verificação de ida e volta.

Cada cenário registra a mediana de nanossegundos por iteração, taxa de transferência e um
contador de alocação determinística para regressões rápidas. Os limites vivem em
`benchmarks/i3/thresholds.json`; supere os limites quando o hardware muda e
confirmar o novo artefato junto com um relatório.

## Solução de problemas

- Fixe a frequência/governador da CPU ao coletar evidências para evitar regressões ruidosas.
- Use `--no-threshold` para execuções exploratórias e reative quando a linha de base for
  atualizado.
- Para criar o perfil de um único cenário, defina `--iterations 1` e execute novamente em
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`.