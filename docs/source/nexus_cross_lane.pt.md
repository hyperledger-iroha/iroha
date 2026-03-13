---
lang: pt
direction: ltr
source: docs/source/nexus_cross_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6e6f144bf3aef313ba55b539c9e92c827bd626973fe38b557f0b668cc909f589
source_last_modified: "2025-12-13T05:07:11.929584+00:00"
translation_last_reviewed: 2026-01-01
---

# Compromissos cross-lane do Nexus e pipeline de provas

> **Status:** entregavel NX-4 - pipeline de compromissos cross-lane e provas (meta Q4 2025).  
> **Responsaveis:** Nexus Core WG / Cryptography WG / Networking TL.  
> **Itens de roadmap relacionados:** NX-1 (geometria de lanes), NX-3 (settlement router), NX-4 (este documento), NX-8 (scheduler global), NX-11 (conformidade de SDK).

Esta nota descreve como os dados de execucao por lane se tornam um compromisso global verificavel. Ela conecta o settlement router existente (`crates/settlement_router`), o lane block builder (`crates/iroha_core/src/block.rs`), as superficies de telemetria/status, e os hooks LaneRelay/DA planejados que ainda faltam para o roadmap **NX-4**.

## Objetivos

- Produzir um `LaneBlockCommitment` deterministico por lane block que capture settlement, liquidez e dados de variancia sem expor estado privado.
- Reenviar esses compromissos (e suas atestacoes DA) para o anel NPoS global para que o merge ledger ordene, valide e persista atualizacoes cross-lane.
- Expor os mesmos payloads via Torii e telemetria para que operadores, SDKs e auditores possam reproduzir o pipeline sem tooling sob medida.
- Definir as invariantes e bundles de evidencia requeridos para graduar NX-4: provas de lane, atestacoes DA, integracao do merge ledger e cobertura de regressao.

## Componentes e superficies

| Componente | Responsabilidade | Referencias de implementacao |
|-----------|----------------|------------------------------|
| Executor de lane e settlement router | Cotar conversoes XOR, acumular recibos por transacao, aplicar politica de buffer | `crates/iroha_core/src/settlement/mod.rs`, `crates/settlement_router` |
| Lane block builder | Drenar `SettlementAccumulator`s, emitir `LaneBlockCommitment`s junto ao lane block | `crates/iroha_core/src/block.rs:3340-3415` |
| LaneRelay broadcaster | Empacotar QCs de lane + provas DA, difundir via `iroha_p2p`, e alimentar o merge ring | `crates/iroha_core/src/nexus/lane_relay.rs`, `crates/iroha_core/src/sumeragi/main_loop.rs` |
| Global merge ledger | Verificar QCs de lane, reduzir merge hints, persistir compromissos de world-state | `docs/source/merge_ledger.md`, `crates/iroha_core/src/sumeragi/status.rs`, `crates/iroha_core/src/state.rs` |
| Torii status e dashboards | Expor `lane_commitments`, `lane_settlement_commitments`, `lane_relay_envelopes`, gauges do scheduler e paineis Grafana | `crates/iroha_torii/src/routing.rs:16660-16880`, `dashboards/grafana/nexus_lanes.json` |
| Armazenamento de evidencia | Arquivar `LaneBlockCommitment`s, artefatos RBC e snapshots de Alertmanager para auditorias | `docs/settlement-router.md`, `artifacts/nexus/*` (bundle futuro) |

## Estruturas de dados e layout de payload

Os payloads canonicos vivem em `crates/iroha_data_model/src/block/consensus.rs`.

### `LaneSettlementReceipt`

- `source_id` - hash de transacao ou id fornecido pelo caller.
- `local_amount_micro` - debito do token de gas do dataspace.
- `xor_due_micro` / `xor_after_haircut_micro` / `xor_variance_micro` - entradas deterministicas do livro XOR e a margem de seguranca por recibo (`due - after haircut`).
- `timestamp_ms` - timestamp UTC em milissegundos capturado durante o settlement.

Os recibos herdam as regras de cotacao deterministicas de `SettlementEngine` e sao agregados dentro de cada `LaneBlockCommitment`.

### `LaneSwapMetadata`

Metadados opcionais que registram os parametros usados na cotacao:

- `epsilon_bps`, `twap_window_seconds`, `volatility_class`.
- bucket de `liquidity_profile` (Tier1-Tier3).
- string `twap_local_per_xor` para que auditores recomputem conversoes com exatidao.

### `LaneBlockCommitment`

Resumo por lane armazenado com cada bloco:

- Cabecalho: `block_height`, `lane_id`, `dataspace_id`, `tx_count`.
- Totais: `total_local_micro`, `total_xor_due_micro`, `total_xor_after_haircut_micro`, `total_xor_variance_micro`.
- `swap_metadata` opcional.
- Vetor `receipts` ordenado.

Essas structs ja derivam `NoritoSerialize`/`NoritoDeserialize`, entao podem ser transmitidas on-chain, via Torii ou via fixtures sem deriva de esquema.

### `LaneRelayEnvelope`

`LaneRelayEnvelope` (ver `crates/iroha_data_model/src/nexus/relay.rs`) empacota o `BlockHeader`
da lane, um `commit QC (`Qc`)` opcional, um hash opcional de `DaCommitmentBundle`, o
`LaneBlockCommitment` completo e o contador de bytes RBC por lane. O envelope armazena um
`settlement_hash` derivado por Norito (via `compute_settlement_hash`) para que os receptores
validem o payload de settlement antes de encaminha-lo ao merge ledger. Chamadores devem rejeitar
os envelopes quando `verify` falha (mismatch de sujeito QC, mismatch de hash DA ou mismatch de
settlement hash), quando `verify_with_quorum` falha (erros de comprimento do bitmap de
assinantes/quorum), ou quando a assinatura QC agregada nao puder ser verificada contra o roster do
comite por dataspace. A preimagem do QC cobre o hash do lane block mais `parent_state_root` e
`post_state_root`, para que a membresia e a correcao do state-root sejam verificadas juntas.

### Selecao de comite de lane

Os QCs de lane relay sao validados contra um comite por dataspace. O tamanho do comite e `3f+1`,
onde `f` e configurado no catalogo do dataspace (`fault_tolerance`). O pool de validadores sao os
validadores do dataspace: manifests de governanca de lane para lanes admin-managed e registros de
staking de lane publica para lanes stake-elected. A membresia do comite e amostrada de forma
deterministica por epoca usando a semente de epoca VRF vinculada a `dataspace_id` e `lane_id` (estavel
para a epoca). Se o pool for menor que `3f+1`, a finalidade do lane relay pausa ate que o quorum
seja restaurado. Operadores podem estender o pool usando a instrucao multisig admin
`SetLaneRelayEmergencyValidators` (requer `CanManagePeers` e `nexus.lane_relay_emergency.enabled = true`,
que vem desabilitado por padrao). Quando habilitado, a autoridade deve ser uma conta multisig que
atenda aos minimos configurados (`nexus.lane_relay_emergency.multisig_threshold`/`multisig_members`,
por padrao 3-de-5). Os overrides sao armazenados por dataspace, aplicados apenas quando o pool esta
abaixo do quorum, e limpos ao enviar uma lista vazia de validadores. Quando `expires_at_height` esta
definido, a validacao ignora o override assim que o `block_height` do lane relay envelope supera a
altura de expiracao. O contador de telemetria
`lane_relay_emergency_override_total{lane,dataspace,outcome}` registra se o override foi aplicado
(`applied`) ou faltava/expirado/insuficiente/desabilitado durante a validacao.

## Ciclo de vida do compromisso

1. **Cotar e preparar recibos.**  
   A fachada de settlement (`SettlementEngine`, `SettlementAccumulator`) registra um
   `PendingSettlement` por transacao. Cada registro armazena entradas TWAP, perfil de
   liquidez, timestamps e valores XOR para que depois se torne um `LaneSettlementReceipt`.

2. **Selar recibos no bloco.**  
   Durante `BlockBuilder::finalize`, cada par `(lane_id, dataspace_id)` drena seu acumulador. O
   builder instancia um `LaneBlockCommitment`, copia a lista de recibos, acumula totais e
   armazena metadados de swap opcionais (via `SwapEvidence`). O vetor resultante e empurrado para o
   slot de status de Sumeragi (`crates/iroha_core/src/sumeragi/status.rs`) para que Torii e
   telemetria o exponham imediatamente.

3. **Empacotamento de relay e atestacoes DA.**  
   `LaneRelayBroadcaster` agora consome os `LaneRelayEnvelope`s emitidos durante o selamento do
   bloco e os difunde como frames `NetworkMessage::LaneRelay` de alta prioridade. Os envelopes sao
   verificados, deduplicados por `(lane_id,dataspace_id,height,settlement_hash)`, e persistidos no
   snapshot de status de Sumeragi (`/v2/sumeragi/status`) para operadores e auditores. O
   broadcaster continuara evoluindo para anexar artefatos DA (provas de chunk RBC, headers Norito,
   manifests SoraFS/Object) e alimentar o merge ring sem bloqueio de head-of-line.

4. **Ordenacao global e merge ledger.**  
   O anel NPoS valida cada relay envelope: verificar `lane_qc` contra o comite por dataspace,
   recomputar totais de settlement, verificar provas DA, depois alimentar o tip da lane no merge
   ledger descrito em `docs/source/merge_ledger.md`. Quando a entrada de merge e selada, o hash do
   world-state (`global_state_root`) agora compromete cada `LaneBlockCommitment`.

5. **Persistencia e exposicao.**  
   Kura escreve o lane block, a entrada de merge e o `LaneBlockCommitment` de forma atomica para
   que o replay reconstrua a mesma reducao. `/v2/sumeragi/status` expoe:
   - `lane_commitments` (metadata de execucao).
   - `lane_settlement_commitments` (o payload descrito aqui).
   - `lane_relay_envelopes` (headers de relay, QCs, digests DA, settlement hash e contagens de bytes RBC).
  Dashboards (`dashboards/grafana/nexus_lanes.json`) leem as mesmas superficies de telemetria e
  status para mostrar throughput de lane, alertas de disponibilidade DA, volume RBC, deltas de
  settlement e evidencia de relay.

## Regras de verificacao e provas

O merge ring DEVE aplicar o seguinte antes de aceitar um compromisso de lane:

1. **Validade do QC de lane.** Verificar a assinatura BLS agregada sobre a preimagem do voto de
   execucao (hash de bloco, `parent_state_root`, `post_state_root`, altura/vista/epoca,
   `chain_id` e tag de modo) contra o roster do comite por dataspace; garantir que o comprimento
   do bitmap de assinantes corresponde ao comite, que os assinantes mapeiam para indices validos, e
   que a altura do header corresponde a `LaneBlockCommitment.block_height`.
2. **Integridade de recibos.** Recomputar os agregados `total_*` a partir do vetor de recibos;
   rejeitar o compromisso se as somas divergirem ou se os recibos contiverem `source_id`s
   duplicados.
3. **Sanidade de metadados de swap.** Confirmar que `swap_metadata` (se presente) corresponde a
   configuracao de settlement e politica de buffer da lane.
4. **Atestacao DA.** Validar que as provas RBC/SoraFS fornecidas pelo relay geram hash no digest
   embutido e que o conjunto de chunks cobre todo o payload do bloco (`rbc_bytes_total` em
   telemetria deve refletir isso).
5. **Reducao de merge.** Uma vez que as provas por lane passam, incluir o tip da lane na entrada do
   merge ledger e recomputar a reducao Poseidon2 (`reduce_merge_hint_roots`). Qualquer mismatch
   aborta a entrada de merge.
6. **Telemetria e trilha de auditoria.** Incrementar os contadores de auditoria por lane
   (`nexus_audit_outcome_total{lane_id,...}`) e persistir o envelope para que o bundle de evidencia
   contenha tanto a prova quanto a trilha de observabilidade.

## Disponibilidade de dados e observabilidade

- **Metricas:**  
  `nexus_scheduler_lane_teu_*`, `nexus_scheduler_dataspace_*`, `sumeragi_rbc_da_reschedule_total`,
  `da_reschedule_total`, `sumeragi_da_gate_block_total{reason="missing_local_data"}`,
  `lane_relay_invalid_total{error}`, `lane_relay_emergency_override_total{outcome}`, e
  `nexus_audit_outcome_total` ja existem em `crates/iroha_telemetry/src/metrics.rs`. Operadores
  permanecer em zero), e `lane_relay_invalid_total` deve permanecer em zero fora de exercicios
  adversariais.
- **Superficies Torii:**  
  `/v2/sumeragi/status` inclui `lane_commitments`, `lane_settlement_commitments` e snapshots de
  dataspace. `/v2/nexus/lane-config` (planejado) publicara a geometria de `LaneConfig` para que
  clientes possam mapear `lane_id` <-> rotulos de dataspace.
- **Dashboards:**  
  `dashboards/grafana/nexus_lanes.json` traca backlog de lane, sinais de disponibilidade DA e os
  totais de settlement expostos acima. As definicoes de alertas devem paginar quando:
  - `nexus_scheduler_dataspace_age_slots` viola a politica.
  - `sumeragi_da_gate_block_total{reason="missing_local_data"}` aumenta de forma persistente.
  - `total_xor_variance_micro` desvia de normas historicas.
- **Bundles de evidencia:**  
  Cada release deve anexar exportacoes de `LaneBlockCommitment`, snapshots de Grafana/Alertmanager
  e manifests de DA relay sob `artifacts/nexus/cross-lane/<date>/`. O bundle se torna o conjunto
  de prova canonico ao enviar relatorios de readiness NX-4.

## Checklist de implementacao (NX-4)

1. **Servico LaneRelay**
   - Esquema definido em `LaneRelayEnvelope`; broadcaster implementado em
     `crates/iroha_core/src/nexus/lane_relay.rs` e ligado ao selamento de blocos
     (`crates/iroha_core/src/sumeragi/main_loop.rs`), emitindo `NetworkMessage::LaneRelay` com
     deduplicacao por node e persistencia de status.
   - Persistir artefatos de relay para auditorias (`artifacts/nexus/relay/...`).
2. **Hooks de atestacao DA**
   - Integrar provas de chunk RBC / SoraFS com relay envelopes e armazenar metricas resumo em
     `SumeragiStatus`.
   - Expor status DA via Torii e Grafana para operadores.
3. **Validacao de merge ledger**
   - Estender o validador de entradas de merge para exigir relay envelopes, nao headers de lane
     brutos.
   - Adicionar testes de replay (`integration_tests/tests/nexus/*.rs`) que alimentem compromissos
     sinteticos no merge ledger e afirmem reducao deterministica.
4. **Atualizacoes de SDK e tooling**
   - Documentar o layout Norito de `LaneBlockCommitment` para consumidores de SDK
     (`docs/portal/docs/nexus/lane-model.md` ja aponta para ca; estender com snippets de API).
   - Fixtures deterministicas vivem sob `fixtures/nexus/lane_commitments/*.{json,to}`; rodar
     `cargo xtask nexus-fixtures` para regenerar (ou `--verify` para validar) as amostras
     `default_public_lane_commitment` e `cbdc_private_lane_commitment` quando mudancas de esquema
     chegarem.
5. **Observabilidade e runbooks**
   - Ligar o pack de Alertmanager para as novas metricas e documentar o workflow de evidencia em
     `docs/source/runbooks/nexus_cross_lane_incident.md` (seguimento).

Completar o checklist acima, junto com esta especificacao, satisfaz a parte de documentacao do
**NX-4** e desbloqueia o trabalho de implementacao restante.
