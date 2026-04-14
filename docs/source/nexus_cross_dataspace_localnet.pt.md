<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus Prova de rede local entre espaços de dados

Este runbook executa a prova de integração Nexus de que:

- inicializa uma rede local de 4 pares com dois espaços de dados privados restritos (`ds1`, `ds2`),
- roteia o tráfego da conta para cada espaço de dados,
- cria um ativo em cada espaço de dados,
- executa liquidação de troca atômica em espaços de dados em ambas as direções,
- comprova a semântica de reversão enviando uma etapa subfinanciada e verificando se os saldos permanecem inalterados.

O teste canônico é:
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## Execução Rápida

Use o script wrapper da raiz do repositório:

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

Comportamento padrão:

- executa apenas o teste de prova entre espaços de dados,
- conjuntos `NORITO_SKIP_BINDINGS_SYNC=1`,
- conjuntos `IROHA_TEST_SKIP_BUILD=1`,
- usa `--test-threads=1`,
- passa `--nocapture`.

## Opções úteis

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` mantém diretórios de pares temporários (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) para análise forense.
- `--all-nexus` executa `mod nexus::` (subconjunto de integração Nexus completo), não apenas o teste de prova.

## Portão CI

Auxiliar de CI:

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

Faça o alvo:

```bash
make check-nexus-cross-dataspace
```

Esta porta executa o wrapper de prova determinístico e falha no trabalho se o atômico entre espaços de dados
cenário de troca regride.

## Comandos Equivalentes Manuais

Teste de prova direcionado:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

Subconjunto Nexus completo:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## Sinais de prova esperados- O teste passa.
- Um aviso esperado aparece para a perna de liquidação subfinanciada que falhou intencionalmente:
  `settlement leg requires 10000 but only ... is available`.
- As declarações de saldo final são bem-sucedidas após:
  - troca direta bem-sucedida,
  - troca reversa bem-sucedida,
  - falha no swap subfinanciado (reversão de saldos inalterados).

## Instantâneo de validação atual

A partir de **19 de fevereiro de 2026**, esse fluxo de trabalho passou com:

- teste direcionado: `1 passed; 0 failed`,
- subconjunto Nexus completo: `24 passed; 0 failed`.