---
lang: pt
direction: ltr
source: docs/source/sumeragi_randomness_evidence_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9a7b2f030cb798b78947c0d7cb298ccbd8a94a006be2e804f1ed043dc15dabc
source_last_modified: "2025-11-15T08:00:21.780712+00:00"
translation_last_reviewed: 2026-01-01
---

# Runbook de aleatoriedade e evidencia de Sumeragi

Este guia satisfaz o item Milestone A6 do roadmap que exigia procedimentos
atualizados de operador para aleatoriedade VRF e evidencia de slashing. Use-o
junto com {doc}`sumeragi` e {doc}`sumeragi_chaos_performance_runbook` sempre que
preparar um novo build de validador ou capturar artefatos de readiness para governance.


Note: For the v1 release, VRF penalties jail offenders after the activation lag, and consensus slashing is delayed by `sumeragi.npos.reconfig.slashing_delay_blocks` (default 259200 blocks, ~3 days at 1s) so governance can cancel with `CancelConsensusEvidencePenalty` before it applies.

## Escopo e pre-requisitos

- `iroha_cli` configurado para o cluster alvo (ver `docs/source/cli.md`).
- `curl`/`jq` para extrair o payload `/status` do Torii ao preparar entradas.
- Acesso ao Prometheus (ou exports snapshot) para as metricas `sumeragi_vrf_*`.
- Conhecimento da epoca atual e do roster para casar a saida do CLI com o
  snapshot de staking ou manifesto de governance.

## 1. Confirmar selecao de modo e contexto de epoca

1. Execute `iroha --output-format text ops sumeragi params` para provar que o binario carregou
   `sumeragi.consensus_mode="npos"` e registrar `k_aggregators`,
   `redundant_send_r`, o comprimento da epoca e os offsets VRF de commit/reveal.
2. Inspecione a view runtime:

   ```bash
   iroha --output-format text ops sumeragi status
   iroha --output-format text ops sumeragi collectors
   iroha --output-format text ops sumeragi rbc status
   ```

   A linha `status` imprime a tupla leader/view, backlog RBC, retries DA,
   offsets de epoca e deferrals do pacemaker; `collectors` mapeia indices de
   collectors para IDs de peers, mostrando quais validadores carregam tarefas
   de aleatoriedade na altura inspecionada.
3. Capture o numero de epoca que deseja auditar:

   ```bash
   EPOCH=$(curl -s "$TORII/status" | jq '.sumeragi.epoch.height // 0')
   printf "auditing epoch %s\n" "$EPOCH"
   ```

   Guarde o valor (decimal ou com prefixo `0x`) para os comandos VRF abaixo.

## 2. Snapshot de epocas VRF e penalidades

Use os subcomandos dedicados do CLI para obter os registros VRF persistidos de
cada validador:

```bash
iroha --output-format text ops sumeragi vrf-epoch --epoch "$EPOCH"
iroha ops sumeragi vrf-epoch --epoch "$EPOCH" > artifacts/vrf_epoch_${EPOCH}.json

iroha --output-format text ops sumeragi vrf-penalties --epoch "$EPOCH"
iroha ops sumeragi vrf-penalties --epoch "$EPOCH" > artifacts/vrf_penalties_${EPOCH}.json
```

Os resumos mostram se a epoca esta finalizada, quantos participantes enviaram
commits/reveals, o tamanho do roster e a seed derivada. O JSON captura a lista
de participantes, o status de penalidade por signatario e o valor `seed_hex`
usado pelo pacemaker. Compare o numero de participantes com o roster de staking
 e verifique se os arrays de penalidade refletem os alertas disparados durante
os testes de caos (late reveals devem aparecer em `late_reveals`, validadores
forfeited em `no_participation`).

## 3. Monitorar telemetria VRF e alertas

Prometheus expoe os contadores exigidos pelo roadmap:

- `sumeragi_vrf_commits_emitted_total`
- `sumeragi_vrf_reveals_emitted_total`
- `sumeragi_vrf_reveals_late_total`
- `sumeragi_vrf_non_reveal_penalties_total`
- `sumeragi_vrf_non_reveal_by_signer{signer="peer_id"}`
- `sumeragi_vrf_no_participation_total`
- `sumeragi_vrf_no_participation_by_signer{signer="peer_id"}`
- `sumeragi_vrf_rejects_total_by_reason{reason="..."}`

Exemplo de PromQL para o relatorio semanal:

```promql
increase(sumeragi_vrf_non_reveal_by_signer[1w]) > 0
```

Durante os drills de readiness confirme que:

- `sumeragi_vrf_commits_emitted_total` e `..._reveals_emitted_total` aumentam
  em cada bloco dentro das janelas de commit/reveal.
- Cenarios de late-reveal acionam `sumeragi_vrf_reveals_late_total` e limpam a
  entrada correspondente no JSON `vrf_penalties`.
- `sumeragi_vrf_no_participation_total` so aumenta quando voce intencionalmente
  segura commits durante os testes de caos.

O overview do Grafana (`docs/source/grafana_sumeragi_overview.json`) inclui paineis
para cada contador; capture screenshots apos cada execucao e anexe ao pacote de
artefatos referenciado em {doc}`sumeragi_chaos_performance_runbook`.

## 4. Ingestao de evidence e streaming

A evidence de slashing deve ser coletada em cada validador e retransmitida ao Torii.
Use os helpers CLI para demonstrar paridade com os endpoints HTTP documentados em
{doc}`torii/sumeragi_evidence_app_api`:

```bash
# Count and list persisted evidence
iroha --output-format text ops sumeragi evidence count
iroha --output-format text ops sumeragi evidence list --limit 5

# Show JSON for audits
iroha ops sumeragi evidence list --limit 100 > artifacts/evidence_snapshot.json
```

Verifique se o `total` reportado coincide com o widget do Grafana alimentado por
`sumeragi_evidence_records_total`, e confirme que registros mais antigos que
`sumeragi.npos.reconfig.evidence_horizon_blocks` sao rejeitados (o CLI imprime
o motivo). Ao testar alerting, envie um payload valido conhecido via:

```bash
iroha --output-format text ops sumeragi evidence submit --evidence-hex-file fixtures/evidence/double_prevote.hex
```

Monitore `/v2/events/sse` com um stream filtrado para provar que SDKs veem os mesmos
dados: reuse o one-liner de Python de {doc}`torii/sumeragi_evidence_app_api` para
construir o filtro e capture os frames `data:` brutos. Os payloads SSE devem ecoar
o kind de evidence e o signer que apareceu na saida do CLI.

## 5. Empacotamento de evidence e reporting

Para cada rehearsal ou release candidate:

1. Armazene os JSONs do CLI (`vrf_epoch_*.json`, `vrf_penalties_*.json`,
   `evidence_snapshot.json`) no diretorio de artefatos do run (o mesmo root
   usado pelos scripts de caos/performance).
2. Registre os resultados das queries Prometheus ou exports snapshot para os
   contadores listados acima.
3. Anexe a captura SSE e os acknowledgements de alerta ao README de artefatos.
4. Atualize `status.md` e
   `docs/source/project_tracker/npos_sumeragi_phase_a.md` com os caminhos dos
   artefatos e o numero de epoca inspecionado.

Seguir este checklist mantem as provas de aleatoriedade VRF e evidence de slashing
 auditaveis durante o rollout NPoS e entrega aos revisores de governance um rastro
 deterministico ate as metricas capturadas e snapshots do CLI.

## 6. Sinais de troubleshooting

- **Mode selection mismatch** - Se `iroha --output-format text ops sumeragi params` mostra
  `consensus_mode="permissioned"` ou `k_aggregators` difere do manifesto,
  descarte os artefatos capturados, corrija `iroha_config`, reinicie o validador
  e reexecute o fluxo de validacao descrito em {doc}`sumeragi`.
- **Missing commits or reveals** - Uma serie plana de `sumeragi_vrf_commits_emitted_total`
  ou `sumeragi_vrf_reveals_emitted_total` indica que o Torii nao esta encaminhando
  frames VRF. Verifique os logs do validador por erros `handle_vrf_*`, depois
  reenvie o payload manualmente via os helpers POST documentados acima.
- **Unexpected penalties** - Quando `sumeragi_vrf_no_participation_total` sobe,
  confira o arquivo `vrf_penalties_<epoch>.json` para confirmar o signer ID e
  compare com o roster de staking. Penalidades que nao batem com os drills de
  caos indicam clock skew do validador ou protecao de replay do Torii; corrija
  o peer afetado antes de repetir o teste.
- **Evidence ingestion stalls** - Quando `sumeragi_evidence_records_total`
  fica estagnado enquanto os testes de caos emitem falhas, execute
  `iroha ops sumeragi evidence count` em varios validadores e confirme que
  `/v2/sumeragi/evidence/count` corresponde a saida do CLI. Qualquer divergencia
  significa que consumidores SSE/webhook podem estar stale, entao reenvie um
  fixture conhecido e escale para os mantenedores do Torii se o contador nao
  incrementar.
