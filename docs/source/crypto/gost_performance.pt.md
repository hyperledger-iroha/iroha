---
lang: pt
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2026-01-03T18:07:57.084090+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Fluxo de trabalho de desempenho GOST

Esta nota documenta como rastreamos e aplicamos o envelope de desempenho para o
Back-end de assinatura GOST TC26.

## Executando localmente

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

Nos bastidores, ambos os alvos chamam `scripts/gost_bench.sh`, que:

1. Executa `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot`.
2. Executa `gost_perf_check` em relação a `target/criterion`, verificando medianas em relação ao
   linha de base registrada (`crates/iroha_crypto/benches/gost_perf_baseline.json`).
3. Injeta o resumo do Markdown em `$GITHUB_STEP_SUMMARY` quando disponível.

Para atualizar a linha de base após aprovar uma regressão/melhoria, execute:

```bash
make gost-bench-update
```

ou diretamente:

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` executa o bench + checker, substitui o JSON da linha de base e imprime
as novas medianas. Sempre confirme o JSON atualizado junto com o registro de decisão em
`crates/iroha_crypto/docs/gost_backend.md`.

### Medianas de referência atuais

| Algoritmo | Mediana (µs) |
|----------------------|------------|
| ed25519 | 69,67 |
| gosto256_paramset_a | 1136,96 |
| gosto256_paramset_b | 1129,05 |
| gosto256_paramset_c | 1133,25 |
| gosto512_paramset_a | 8944,39 |
| gosto512_paramset_b | 8.963,60 |
| secp256k1 | 160,53 |

## CI

`.github/workflows/gost-perf.yml` usa o mesmo script e também executa o dudect timing guard.
O IC falha quando a mediana medida excede a linha de base em mais do que a tolerância configurada
(20% por padrão) ou quando o protetor de tempo detecta um vazamento, então as regressões são detectadas automaticamente.

## Saída resumida

`gost_perf_check` imprime a tabela de comparação localmente e anexa o mesmo conteúdo a
`$GITHUB_STEP_SUMMARY`, portanto, os logs de tarefas de CI e os resumos de execução compartilham os mesmos números.