---
lang: pt
direction: ltr
source: docs/source/sumeragi_evidence_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 575fdc2adfa8ad461ed44529bc0b129fede932d41c3492373e0135457fb538f4
source_last_modified: "2025-12-22T17:40:03.625971+00:00"
translation_last_reviewed: 2026-01-01
---

# Evidencia de Sumeragi (API de auditoria)

Endpoints temporarios de auditoria para evidencia de Sumeragi.

- GET `/v1/sumeragi/evidence/count`
  - Retorna o numero de entradas Evidence unicas observadas por este no.
  - Resposta (payload Norito): `count: u64`.
  - Defina `Accept: application/json` para receber `{ "count": <u64> }`.
  - Notas:
    - Apoiado no store WSV por no (`world.consensus_evidence`) persistido com codecs Norito.
    - Sobrevive a reinicios e alimenta `/v1/sumeragi/evidence`; as entradas sao deduplicadas por hash de evidence.
    - Ainda e local a cada validador (nao replicado por consenso); a ingestao de governance vira depois.

- GET `/v1/sumeragi/evidence`
  - Lista entradas de evidencia recentes persistidas no snapshot de auditoria WSV.
  - Parametros de query: `limit` (default 50, max 1000), `offset` (default 0), `kind` (opcional; um de `DoublePrepare|DoubleCommit|InvalidCommitCertificate|InvalidProposal|Censorship`).
  - Resposta (payload Norito): `(total, Vec<EvidenceRecord>)`.
  - Defina `Accept: application/json` para receber um objeto JSON `{ "total": <u64>, "items": [ ... ] }`.
- Evidencia com subject height mais antigo que `sumeragi.npos.reconfig.evidence_horizon_blocks`
  (default 7200) e descartada na entrada; o actor registra a rejeicao para ajudar operadores
  a investigar envios obsoletos.
- POST `/v1/sumeragi/evidence`
  - Envie evidencia Norito codificada em hex para o actor Sumeragi (`ControlFlow::Evidence`).
  - Corpo da requisicao (JSON): `{ "evidence_hex": "<hex string>" }`; a string hex representa bytes `ConsensusEvidence` com framing Norito e espacos em branco sao ignorados.
  - Resposta (JSON): `{ "status": "accepted", "kind": "<variant>" }` em sucesso.
  - A validacao cobre igualdade de signer/height/view/epoch para payloads de double-vote, exige payloads de um so signatario nao vazios, aplica quoruns de receipts para evidencia `Censorship` (payloads assinados de `TransactionSubmissionReceipt`), e rejeita registros `InvalidProposal` que nao avancam height/view ou cujo hash pai diverge do commit certificate embutido.
  - Helper CLI: `iroha sumeragi evidence submit --evidence-hex <hex>` ou `--evidence-hex-file <path>`.

Status adicional de consenso e provas de execucao

- GET `/v1/sumeragi/status` - retorna um payload `SumeragiStatusWire` codificado Norito por padrao. Defina `Accept: application/json` para receber `{ leader_index, view_change_index, highest_qc{height,view,subject_block_hash}, locked_qc{height,view,subject_block_hash}, commit_certificate{height,view,epoch,block_hash,validator_set_hash,validator_set_len,signatures_total}, commit_quorum{height,view,block_hash,signatures_present,signatures_counted,signatures_set_b,signatures_required,last_updated_ms}, view_change_causes{commit_failure_total,quorum_timeout_total,da_gate_total,censorship_evidence_total,missing_payload_total,missing_qc_total,validation_reject_total,last_cause,last_cause_timestamp_ms}, tx_queue{depth,capacity,saturated}, epoch{length_blocks,commit_deadline_offset,reveal_deadline_offset}, membership{height,view,epoch,view_hash}, gossip_fallback_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, validation_reject_total, validation_reject_reason, block_sync_roster{commit_certificate_hint_total,checkpoint_hint_total,commit_certificate_history_total,checkpoint_history_total,roster_sidecar_total,commit_roster_journal_total,drop_missing_total}, pacemaker_backpressure_deferrals_total, commit_pipeline_tick_total, da_reschedule_total, rbc_store{sessions,bytes,pressure_level,backpressure_deferrals_total,evictions_total,recent_evictions[...]}, prf{height,view,epoch_seed}, vrf_penalty_epoch, vrf_committed_no_reveal_total, vrf_no_participation_total, vrf_late_reveals_total, collectors_targeted_current, collectors_targeted_last_per_block, redundant_sends_total, pipeline_conflict_rate_bps, access_set_sources{manifest_hints,entrypoint_hints,prepass_merge,conservative_fallback}, worker_loop{stage,stage_started_ms,last_iteration_ms,queue_depths{vote_rx,block_payload_rx,rbc_chunk_rx,block_rx,consensus_rx,lane_relay_rx,background_rx}} }`. Nota: `view_change_causes.da_gate_total` e reservado para compatibilidade e deve permanecer zero; os avisos de disponibilidade DA aparecem via `status.da_gate` e `sumeragi_da_gate_block_total{reason="missing_local_data"}`. `da_reschedule_total` e legado.
- GET `/v1/sumeragi/qc` - retorna um snapshot de commit certificate (`SumeragiQcSnapshot`) codificado Norito por padrao. Defina `Accept: application/json` para receber `{ highest_qc { height, view, subject_block_hash }, locked_qc { height, view, subject_block_hash } }`.
- GET `/v1/sumeragi/status/sse` - fluxo SSE do mesmo payload (cadencia ~1s).
- GET `/v1/sumeragi/exec_root/:hash` - retorna um payload Norito `(block_hash, Option<ExecRoot>)` por padrao. Passe `Accept: application/json` para receber `{ block_hash, exec_root }` (hex) ou `exec_root = null` se ausente.
- GET `/v1/sumeragi/exec_qc/:hash` - retorna um `ExecutionQcRecord` codificado Norito para `:hash` (hash do bloco pai) por padrao. Com `Accept: application/json` a resposta se expande para:
  - `post_state_root` (hex), `height`, `view`, `epoch`, `signers_bitmap` (hex), `bls_aggregate_signature` (hex).
  - Se ausente, retorna `{ subject_block_hash, exec_qc: null }`.

Exemplo (curl)

```bash
# Replace HASH with a real parent block hash (hex, 32 bytes)
HASH=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2

curl -s \
  http://127.0.0.1:8080/v1/sumeragi/exec_qc/$HASH | jq .

# Example response (when present):
# {
#   "subject_block_hash": "BA6733...F5B2",
#   "post_state_root": "1f9a7d...2c0e",
#   "height": 42,
#   "view": 3,
#   "epoch": 0,
#   "signers_bitmap": "0700",
#   "bls_aggregate_signature": ""
# }
```

Exemplo de exec root (curl)

```bash
# Replace HASH with a real parent block hash (hex, 32 bytes)
HASH=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2

curl -s \
  http://127.0.0.1:8080/v1/sumeragi/exec_root/$HASH | jq .

# Example response (when present):
# {
#   "block_hash": "BA6733...F5B2",
#   "exec_root": "1f9a7d...2c0e"
# }

# When missing:
# {
#   "block_hash": "BA6733...F5B2",
#   "exec_root": null
# }
```

Linhas jq rapidas (consistencia + wrappers)

- Parent consistency check (ExecQC vs exec_root):

```bash
P=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2
QC=$(curl -s http://127.0.0.1:8080/v1/sumeragi/exec_qc/$P | jq -r '.post_state_root // empty')
RT=$(curl -s http://127.0.0.1:8080/v1/sumeragi/exec_root/$P | jq -r '.exec_root // empty')
if [ -z "$QC" ]; then echo "no ExecQC record in WSV for $P"; exit 1; fi
if [ -z "$RT" ]; then echo "no exec_root in WSV for $P"; exit 1; fi
if [ "$QC" = "$RT" ]; then echo "OK: parent ExecQC equals exec_root ($QC)"; else echo "MISMATCH: QC=$QC root=$RT"; fi
```

- Shell helpers you can paste into your terminal:

```bash
get_exec_qc_root() { curl -s "http://127.0.0.1:8080/v1/sumeragi/exec_qc/$1" | jq -r '.post_state_root // empty'; }
get_exec_root()    { curl -s "http://127.0.0.1:8080/v1/sumeragi/exec_root/$1" | jq -r '.exec_root // empty'; }
check_parent_consistency() {
  local h="$1"; local qc=$(get_exec_qc_root "$h"); local rt=$(get_exec_root "$h");
  if [ -z "$qc" ]; then echo "no ExecQC record for $h"; return 2; fi
  if [ -z "$rt" ]; then echo "no exec_root for $h"; return 3; fi
  if [ "$qc" = "$rt" ]; then echo "OK: $h QC==root ($qc)"; else echo "MISMATCH: QC=$qc root=$rt"; return 1; fi
}

# usage:
#   H=...
#   check_parent_consistency "$H"
```

Nota
- A igualdade cross-height (child vs parent) e aplicada internamente pelo Sumeragi em modo estrito
  (`require_wsv_exec_qc=true`) e ainda nao e exposta diretamente via query. As verificacoes de
  consistencia visiveis para operadores acima confirmam que o registro ExecutionQC do pai coincide
  com o exec_root persistido. Um endpoint futuro vai expor roots de proposal/header para permitir
  uma comparacao externa parent-vs-child.
