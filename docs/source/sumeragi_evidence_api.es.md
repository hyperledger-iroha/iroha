---
lang: es
direction: ltr
source: docs/source/sumeragi_evidence_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 575fdc2adfa8ad461ed44529bc0b129fede932d41c3492373e0135457fb538f4
source_last_modified: "2025-12-22T17:40:03.625971+00:00"
translation_last_reviewed: 2026-01-01
---

# Evidencia de Sumeragi (API de auditoria)

Endpoints temporales de auditoria para evidencia de Sumeragi.

- GET `/v1/sumeragi/evidence/count`
  - Devuelve el numero de entradas Evidence unicas observadas por este nodo.
  - Respuesta (payload Norito): `count: u64`.
  - Establece `Accept: application/json` para recibir `{ "count": <u64> }`.
  - Notas:
    - Respaldado por el store WSV por nodo (`world.consensus_evidence`) persistido con codecs Norito.
    - Sobrevive reinicios y alimenta `/v1/sumeragi/evidence`; las entradas se deduplican por hash de evidence.
    - Aun es local a cada validador (no replicado por consenso); la ingesta de governance vendra despues.

- GET `/v1/sumeragi/evidence`
  - Lista entradas de evidencia recientes persistidas en el snapshot de auditoria WSV.
  - Parametros de query: `limit` (default 50, max 1000), `offset` (default 0), `kind` (opcional; uno de `DoublePrevote|DoublePrecommit|InvalidQC|InvalidProposal|Censorship`).
  - Respuesta (payload Norito): `(total, Vec<EvidenceRecord>)`.
  - Establece `Accept: application/json` para recibir un objeto JSON `{ "total": <u64>, "items": [ ... ] }`.
- La evidencia con subject height mas antiguo que `sumeragi.npos.reconfig.evidence_horizon_blocks`
  (default 7200) se descarta al ingresar; el actor registra el rechazo para ayudar a los operadores
  a investigar envios obsoletos.
- POST `/v1/sumeragi/evidence`
  - Envia evidencia Norito codificada en hex al actor Sumeragi (`ControlFlow::Evidence`).
  - Cuerpo de solicitud (JSON): `{ "evidence_hex": "<hex string>" }`; la cadena hex representa bytes `ConsensusEvidence` con framing Norito y se ignoran los espacios en blanco.
  - Respuesta (JSON): `{ "status": "accepted", "kind": "<variant>" }` en exito.
  - La validacion cubre igualdad de signer/height/view/epoch para payloads de double-vote, requiere payloads de un solo firmante no vacios, aplica quorums de recibos para evidencia `Censorship` (payloads firmados de `TransactionSubmissionReceipt`), y rechaza registros `InvalidProposal` que no avanzan height/view o cuyo hash padre discrepa con el QC embebido.
  - Helper CLI: `iroha sumeragi evidence submit --evidence-hex <hex>` o `--evidence-hex-file <path>`.

Estado adicional de consenso y pruebas de ejecucion

- GET `/v1/sumeragi/status` - devuelve un payload `SumeragiStatusWire` codificado Norito por defecto. Establece `Accept: application/json` para recibir `{ leader_index, view_change_index, highest_qc{height,view,subject_block_hash}, locked_qc{height,view,subject_block_hash}, commit_certificate{height,view,epoch,block_hash,validator_set_hash,validator_set_len,signatures_total}, commit_quorum{height,view,block_hash,signatures_present,signatures_counted,signatures_set_b,signatures_required,last_updated_ms}, view_change_causes{commit_failure_total,quorum_timeout_total,da_gate_total,censorship_evidence_total,missing_payload_total,missing_qc_total,validation_reject_total,last_cause,last_cause_timestamp_ms}, tx_queue{depth,capacity,saturated}, epoch{length_blocks,commit_deadline_offset,reveal_deadline_offset}, membership{height,view,epoch,view_hash}, gossip_fallback_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, validation_reject_total, validation_reject_reason, block_sync_roster{commit_certificate_hint_total,checkpoint_hint_total,commit_certificate_history_total,checkpoint_history_total,roster_sidecar_total,commit_roster_journal_total,drop_missing_total}, pacemaker_backpressure_deferrals_total, commit_pipeline_tick_total, da_reschedule_total, rbc_store{sessions,bytes,pressure_level,backpressure_deferrals_total,evictions_total,recent_evictions[...]}, prf{height,view,epoch_seed}, vrf_penalty_epoch, vrf_committed_no_reveal_total, vrf_no_participation_total, vrf_late_reveals_total, collectors_targeted_current, collectors_targeted_last_per_block, redundant_sends_total, pipeline_conflict_rate_bps, access_set_sources{manifest_hints,entrypoint_hints,prepass_merge,conservative_fallback}, worker_loop{stage,stage_started_ms,last_iteration_ms,queue_depths{vote_rx,block_payload_rx,rbc_chunk_rx,block_rx,consensus_rx,lane_relay_rx,background_rx}} }`. Nota: `view_change_causes.da_gate_total` esta reservado y debe permanecer en cero; las advertencias de disponibilidad DA se exponen via `status.da_gate` y `sumeragi_da_gate_block_total{reason="missing_availability_qc"}`. `da_reschedule_total` no se usa.
- GET `/v1/sumeragi/qc` - devuelve un `SumeragiQcSnapshot` codificado Norito por defecto. Establece `Accept: application/json` para recibir `{ highest_qc { height, view, subject_block_hash }, locked_qc { height, view, subject_block_hash } }`.
- GET `/v1/sumeragi/status/sse` - flujo SSE del mismo payload (cadencia ~1s).
- GET `/v1/sumeragi/exec_root/:hash` - devuelve un payload Norito `(block_hash, Option<ExecRoot>)` por defecto. Pasa `Accept: application/json` para recibir `{ block_hash, exec_root }` (hex) o `exec_root = null` si falta.
- GET `/v1/sumeragi/exec_qc/:hash` - devuelve un `ExecutionQcRecord` codificado Norito para `:hash` (hash de bloque padre) por defecto. Con `Accept: application/json` la respuesta se expande a:
  - `post_state_root` (hex), `height`, `view`, `epoch`, `signers_bitmap` (hex), `bls_aggregate_signature` (hex).
  - Si falta, devuelve `{ subject_block_hash, exec_qc: null }`.

Ejemplo (curl)

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

Ejemplo de exec root (curl)

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

Lineas jq rapidas (consistencia + wrappers)

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
- La igualdad cross-height (hijo vs padre) se aplica internamente en Sumeragi en modo estricto
  (`require_wsv_exec_qc=true`) y aun no se expone directamente via una consulta. Las comprobaciones
  de consistencia visibles para operadores arriba verifican que el registro ExecutionQC del padre
  coincide con su exec_root persistido. Un endpoint futuro expondra roots de proposal/header para
  permitir una comparacion externa padre-vs-hijo.
