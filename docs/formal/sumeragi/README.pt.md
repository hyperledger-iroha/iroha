<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e89f83a4ce35b7cab8d3bfcee27eafb761f6a281c445a7cae13ae9d228760fe7
source_last_modified: "2026-04-30T20:10:10.884040+00:00"
translation_last_reviewed: 2026-05-01
translator: machine-google-reviewed
---

# Sumeragi Modelo Formal (TLA+/Apalache)

Este diretório contém modelos formais limitados para segurança e vivacidade Sumeragi.

## Escopo

`Sumeragi.tla` captura o caminho de confirmação:
- progressão de fase (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- limites de votação e quórum (`CommitQuorum`, `ViewQuorum`),
- quórum de participação ponderado (`StakeQuorum`) para protetores de commit estilo NPoS,
- Causalidade de RBC (`Init -> Chunk -> Ready -> Deliver`) com evidência de cabeçalho/resumo,
- GST e suposições de justiça fracas sobre ações de progresso honestas.

`SumeragiFrontierRecovery.tla` captura a classe Taira Hang focada em torno de um
bloco de fronteira contígua pendente:
- evidência de voto confirmado abaixo ou no quórum,
- atraso na fila de votação e drenagem local,
- estado de carga útil ausente vs. local,
- propriedade de recuperação de fronteira nova versus obsoleta,
- marcador de reprogramação de quorum/ritmo de janela,
- evidências de fronteiras futuras/novas visões que possam reancorar a fronteira local,
- confirmação determinística pós-GST, retransmissão, rotação de visualização limitada e
  resultados de queda de evidência zero.

Ambos os modelos abstraem intencionalmente formatos de fios, ECDSA/assinatura
verificação e detalhes completos da rede.

## Arquivos- `Sumeragi.tla`: modelo e propriedades do protocolo.
- `Sumeragi_fast.cfg`: conjunto menor de parâmetros compatíveis com CI.
- `Sumeragi_deep.cfg`: maior conjunto de parâmetros de tensão.
- `SumeragiFrontierRecovery.tla`: modelo de recuperação de fronteira focado.
- `SumeragiFrontierRecovery_fast.cfg`: conjunto menor de parâmetros de fronteira compatível com CI.
- `SumeragiFrontierRecovery_deep.cfg`: maior conjunto de limites de backlog/janela/visualização de fronteira maior.
- `SumeragiFrontierRecovery_wide.cfg`: conjunto manual de limite de fronteira mais ampla.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: mutação de proprietário obsoleto com falha esperada.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: mutação na fila de votação com falha esperada.

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

Invariantes de recuperação de fronteira:
-`TypeInvariant`
-`CommitImpliesVoteQuorum`
-`CommitImpliesPayloadAvailability`
-`VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`, que exclui um terminal
  estado pós-GST onde `pending /\ voteBacked /\ ~committed` não tem recuperação,
  confirmação, retransmissão, rotação ou transição de descarte limitado.Propriedade temporal de recuperação de fronteira:
- `PostGstVoteBackedFrontierEventuallyResolves`: após o GST, todos os problemas não resolvidos
  estado de fronteira pendente apoiado por voto eventualmente atinge commit, carga útil
  recuperação, retransmissão de quorum, reancoragem de fronteira futura ou visão limitada
  rotação.
- `RecoveredPayloadEventuallyAdvances`: um estado fronteiriço apoiado pelo voto que
  recuperada, a carga útil não pode permanecer pendente para sempre sem confirmação,
  retransmitir, reancorar ou girar.
- `QuorumRetransmitEventuallyLeavesPending`: assim que a retransmissão de quorum for acionada
  para um estado fronteiriço apoiado por voto, o invólucro pendente deve eventualmente ser eliminado.
- `FutureFrontierEvidenceEventuallyReanchors`: fronteira posterior/evidência de nova visão
  deve limpar o wrapper pendente ou ser consumido como uma reancoragem de fronteira.

## Mapa de suposição

O modelo de fronteira é intencionalmente finito. Estas são a implementação
superfície ele abstrai:| Conceito de modelo | Superfície de implementação |
| --- | --- |
| `pending`, `contiguous`, `payloadState` | Manipulação de `PendingBlock` e verificações de carga útil local em `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`, além de materialização de propriedade BlockCreated/frontier em `proposal_handlers.rs`. |
| `commitVotes`, `queuedVotes` | Contagem de votos confirmados e controle de entrada de votos exercidos por `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` e `reschedule_ignores_quorum_timeout_vote_queue_backlog` em `crates/iroha_core/src/sumeragi/main_loop/tests.rs`. |
| `recoveryOwner` | Estado de proprietário de fronteira ativo/obsoleto em `frontier_slot_has_active_owner_state_for_view(...)`, rendimento de proprietário obsoleto em `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)` e limpeza de substituição em `drop_superseded_contiguous_frontier_owner_state(...)`. |
| `quorumRescheduleArmed`, `quorumWindowAge` | Ritmo de reescalonamento de quórum apoiado por votação em `reschedule_stale_pending_blocks_with_now(...)`; a cobertura de regressão inclui `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned`. |
| `payloadRecovered` | Reparo de carroceria de fronteira exata e admissão de reparo de RBC obsoleto em `request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)` e `stale_frontier_rbc_repair_is_actionable(...)`. |
| `quorumRetransmitted`, `rotated` | Seleção de destino de retransmissão de quorum, `rebroadcast_pending_block_updates(...)`, e chamadas determinísticas de mudança de visualização em `reschedule_stale_pending_blocks_with_now(...)`. |
| `futureFrontierEvidence` | Evidência de quórum de nova visão/fronteira superior futura em `on_pacemaker_propose_ready(...)`, coberta por `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists`. |

## Correndo

Da raiz do repositório:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

O executor define um Apalache `--length` explícito para cada modo:| Modo | Comprimento | Utilização prevista |
| --- | ---: | --- |
| `fast` | 10 | Verificação do caminho de confirmação do CI |
| `deep` | 10 | Verificação maior do caminho de confirmação |
| `frontier-fast` | 10 | Verificação da fronteira CI |
| `frontier-deep` | 12 | Maior controlo fronteiriço |
| `frontier-wide` | 14 | Verificação manual/noturna do estresse nas fronteiras |

`APALACHE_LENGTH=<n>` substitui o padrão por modo ao explorar localmente um
contra-exemplo ou ampliando uma prova limitada.

### Configuração local reproduzível (não é necessário Docker)

Instale o conjunto de ferramentas Apalache local fixado usado por este repositório:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

O executor detecta automaticamente esta instalação em:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
Após a instalação, `ci/check_sumeragi_formal.sh` deve funcionar sem env vars extras:

```bash
bash ci/check_sumeragi_formal.sh
```

As mutações de falha esperada estão intencionalmente fora do IC normal. Eles deveriam
falham no Apalache e são úteis ao alterar o modelo:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Se o Apalache não estiver em `PATH`, você poderá:

- defina `APALACHE_BIN` como o caminho do executável ou
- use o fallback Docker (habilitado por padrão quando `docker` está disponível):
  - imagem: `APALACHE_DOCKER_IMAGE` (padrão `ghcr.io/apalache-mc/apalache:0.52.2`)
  - requer um daemon Docker em execução
  - desative o substituto com `APALACHE_ALLOW_DOCKER=0`.

Exemplos:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## Notas- Este modelo complementa (não substitui) testes de modelo Rust executáveis em
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  e
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- As verificações são limitadas por valores constantes nos arquivos `.cfg`.
- PR CI executa essas verificações em `.github/workflows/pr.yml` via
  `ci/check_sumeragi_formal.sh`.