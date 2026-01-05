---
lang: pt
direction: ltr
source: docs/source/sumeragi_aggregators.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee79dba673794a3dd4f888d3daf39163a827443bb22d413ab4d7f2e252762293
source_last_modified: "2025-12-26T13:17:08.872635+00:00"
translation_last_reviewed: 2026-01-01
---

# Roteamento de agregadores Sumeragi

## Resumo

Esta nota captura a estrategia deterministica de roteamento de collectors ("aggregators") usada por Sumeragi apos a atualizacao de equidade da Fase 3. Cada validador calcula a mesma ordenacao de collectors para uma altura e view de bloco dadas. O design elimina a dependencia de aleatoriedade ad hoc e mantem o fan-out normal de votos limitado pela lista de collectors; quando os collectors estao indisponiveis ou o quorum trava, os rebroadcasts de reescalonamento reutilizam alvos de collectors com fallback para a topologia de commit.

## Selecao deterministica

- O novo modulo `sumeragi::collectors` expoe `deterministic_collectors(topology, mode, k, seed, height, view)` que retorna um `Vec<PeerId>` reproduzivel para o par `(height, view)`.
- O modo permissioned rotaciona o conjunto contiguo de collectors de cauda por `height + view`, garantindo que cada collector se torne primario em um calendario round-robin. Isso preserva o comportamento original de proxy-tail enquanto distribui a carga de forma uniforme no segmento de cauda (proxy tail + validadores Set B).
- O modo NPoS continua usando o PRF por epoca, mas o helper agora centraliza o calculo para que cada chamador receba a mesma ordem. A seed e derivada da aleatoriedade de epoca fornecida por `EpochManager`.
- `CollectorPlan` acompanha o consumo dos alvos ordenados e registra se o fallback de gossip foi acionado. As atualizacoes de telemetria (`collect_aggregator_ms`, `sumeragi_redundant_sends_*`, `sumeragi_gossip_fallback_total`) mostram com que frequencia os fallbacks ocorrem e quanto tempo o fan-out redundante leva.

## Objetivos de equidade

1. **Reprodutibilidade:** A mesma topologia de validadores, modo de consenso e tupla `(height, view)` deve levar aos mesmos collectors primarios/secundarios em cada peer. O helper esconde peculiaridades de topologia (proxy tail, validadores Set B) para que a ordem seja portavel entre componentes e testes.
2. **Rotacao:** Em deployments permissioned o collector primario rotaciona a cada bloco (e tambem muda apos um view bump), evitando que um unico validador Set B detenha permanentemente as tarefas de agregacao. A selecao NPoS baseada em PRF ja fornece aleatoriedade e nao e afetada.
3. **Observabilidade:** A telemetria continua reportando atribuicoes por collector e o caminho de fallback emite um aviso quando gossip e acionado para que operadores detectem collectors com mau comportamento.

## Retry e backoff de gossip

- Os validadores mantem um `CollectorPlan` no estado de proposta; o plano registra quantos collectors foram contatados e se o limite de fan-out redundante foi atingido.
- Os planos de collectors sao indexados por `(height, view)` e reinicializados sempre que o assunto muda para que retries de view-change obsoletos nao reutilizem alvos antigos.
- O redundant send (`r`) e aplicado de forma deterministica ao avancar no plano. Quando nao ha collectors disponiveis para a tupla `(height, view)`, os votos retornam para a topologia completa de commit (excluindo o proprio) para evitar deadlock.
- Quando o quorum trava, o caminho de reescalonamento rebroadcast os votos em cache via o plano de collectors, voltando a topologia de commit quando collectors estao vazios, sao apenas locais, ou estao abaixo de quorum. Isso fornece um fallback "gossip" limitado sem pagar o custo de broadcast completo no caminho rapido de estado estavel.
- Cada descarte de proposta por causa do gate de locked commit certificate incrementa `block_created_dropped_by_lock_total`; caminhos de validacao de header que falham elevam `block_created_hint_mismatch_total` e `block_created_proposal_mismatch_total`, ajudando operadores a correlacionar fallbacks repetidos com problemas de correcao do leader. O snapshot `/v1/sumeragi/status` tambem exporta os hashes mais recentes de Highest/Locked commit certificate para que dashboards correlacionem picos de drops com hashes de bloco especificos.

## Resumo da implementacao

- O novo modulo publico `sumeragi::collectors` hospeda `CollectorPlan` e `deterministic_collectors` para que testes a nivel de crate e de integracao possam verificar propriedades de equidade sem instanciar o ator de consenso completo.
- `CollectorPlan` vive no estado de proposta do Sumeragi e e resetado quando o pipeline de propostas termina.
- `Sumeragi` cria planos de collectors via `init_collector_plan` e mira collectors ao emitir votos de availability/precommit. Votos de availability e precommit retornam a topologia de commit quando collectors estao vazios, sao apenas locais ou estao abaixo de quorum, e os rebroadcasts recuam sob as mesmas condicoes.
- Testes unitarios e de integracao validam a rotacao permissioned, o determinismo do PRF e as transicoes de estado de backoff.

## Assinatura de revisao

- Reviewed-by: Consensus WG
- Reviewed-by: Platform Reliability WG
