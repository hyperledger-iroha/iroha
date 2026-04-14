<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi Modelo Formal (TLA+/Apalache)

Este diretório contém um modelo formal limitado para segurança e atividade do caminho de confirmação Sumeragi.

## Escopo

O modelo captura:
- progressão de fase (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- limites de votação e quórum (`CommitQuorum`, `ViewQuorum`),
- quórum de participação ponderado (`StakeQuorum`) para protetores de commit estilo NPoS,
- Causalidade de RBC (`Init -> Chunk -> Ready -> Deliver`) com evidência de cabeçalho/resumo,
- GST e suposições de justiça fracas sobre ações de progresso honestas.

Ele abstrai intencionalmente formatos de fios, assinaturas e detalhes completos de rede.

## Arquivos

- `Sumeragi.tla`: modelo e propriedades do protocolo.
- `Sumeragi_fast.cfg`: conjunto menor de parâmetros compatíveis com CI.
- `Sumeragi_deep.cfg`: maior conjunto de parâmetros de tensão.

## Propriedades

Invariantes:
-`TypeInvariant`
-`CommitImpliesQuorum`
-`CommitImpliesStakeQuorum`
-`CommitImpliesDelivered`
-`DeliverImpliesEvidence`

Propriedade temporal:
- `EventuallyCommit` (`[] (gst => <> committed)`), com codificação de imparcialidade pós-GST
  operacionalmente em `Next` (proteções de tempo limite/preempção de falhas ativadas
  ações de progresso). Isso mantém o modelo verificável com Apalache 0.52.x, que
  não suporta operadores de imparcialidade `WF_` dentro de propriedades temporais verificadas.

## Correndo

Da raiz do repositório:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### Configuração local reproduzível (não é necessário Docker)Instale o conjunto de ferramentas Apalache local fixado usado por este repositório:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

O executor detecta automaticamente esta instalação em:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
Após a instalação, `ci/check_sumeragi_formal.sh` deve funcionar sem env vars extras:

```bash
bash ci/check_sumeragi_formal.sh
```

Se o Apalache não estiver em `PATH`, você poderá:

- defina `APALACHE_BIN` como o caminho do executável ou
- use o fallback Docker (habilitado por padrão quando `docker` está disponível):
  - imagem: `APALACHE_DOCKER_IMAGE` (padrão `ghcr.io/apalache-mc/apalache:latest`)
  - requer um daemon Docker em execução
  - desative o substituto com `APALACHE_ALLOW_DOCKER=0`.

Exemplos:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## Notas

- Este modelo complementa (não substitui) testes de modelo Rust executáveis em
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  e
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- As verificações são delimitadas por valores constantes nos arquivos `.cfg`.
- PR CI executa essas verificações em `.github/workflows/pr.yml` via
  `ci/check_sumeragi_formal.sh`.