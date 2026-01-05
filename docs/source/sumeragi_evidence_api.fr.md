---
lang: fr
direction: ltr
source: docs/source/sumeragi_evidence_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 575fdc2adfa8ad461ed44529bc0b129fede932d41c3492373e0135457fb538f4
source_last_modified: "2025-12-22T17:40:03.625971+00:00"
translation_last_reviewed: 2026-01-01
---

# Evidence Sumeragi (API d audit)

Endpoints temporaires d audit pour les evidences Sumeragi.

- GET `/v1/sumeragi/evidence/count`
  - Renvoie le nombre d entrees Evidence uniques observees par ce noeud.
  - Reponse (payload Norito): `count: u64`.
  - Definir `Accept: application/json` pour recevoir `{ "count": <u64> }`.
  - Notes:
    - Adosse au store WSV par noeud (`world.consensus_evidence`) persiste avec les codecs Norito.
    - Survit aux redemarrages et alimente `/v1/sumeragi/evidence`; les entrees sont dedupliquees par hash d evidence.
    - Toujours local a chaque validateur (non replique par consensus); l ingestion governance suivra.

- GET `/v1/sumeragi/evidence`
  - Liste les evidences recentes persistees dans le snapshot d audit WSV.
  - Parametres de requete: `limit` (defaut 50, max 1000), `offset` (defaut 0), `kind` (optionnel; un de `DoublePrepare|DoubleCommit|InvalidCommitCertificate|InvalidProposal|Censorship`).
  - Reponse (payload Norito): `(total, Vec<EvidenceRecord>)`.
  - Definir `Accept: application/json` pour recevoir `{ "total": <u64>, "items": [ ... ] }`.
- Les evidences avec un subject height plus ancien que `sumeragi.npos.reconfig.evidence_horizon_blocks`
  (defaut 7200) sont rejetees a l ingress; l acteur journalise le rejet pour aider les operateurs
  a enqueter sur des soumissions perimees.
- POST `/v1/sumeragi/evidence`
  - Soumet une evidence Norito encodee en hex au Sumeragi actor (`ControlFlow::Evidence`).
  - Corps de requete (JSON): `{ "evidence_hex": "<hex string>" }`; la chaine hex represente des octets `ConsensusEvidence` encadres Norito et les espaces sont ignores.
  - Reponse (JSON): `{ "status": "accepted", "kind": "<variant>" }` en succes.
  - La validation couvre l egalite signer/height/view/epoch pour les payloads double-vote, exige des payloads non vides a un seul signataire, applique les quorums de receipt pour les evidences `Censorship` (payloads signes de `TransactionSubmissionReceipt`), et rejette les enregistrements `InvalidProposal` qui n avancent pas height/view ou dont le hash parent diverge du commit certificate embarque.
  - Helper CLI: `iroha sumeragi evidence submit --evidence-hex <hex>` ou `--evidence-hex-file <path>`.

Etat de consensus supplementaire et preuves d execution

- GET `/v1/sumeragi/status` - renvoie un payload `SumeragiStatusWire` encode Norito par defaut. Definir `Accept: application/json` pour recevoir `{ leader_index, view_change_index, highest_qc{height,view,subject_block_hash}, locked_qc{height,view,subject_block_hash}, commit_certificate{height,view,epoch,block_hash,validator_set_hash,validator_set_len,signatures_total}, commit_quorum{height,view,block_hash,signatures_present,signatures_counted,signatures_set_b,signatures_required,last_updated_ms}, view_change_causes{commit_failure_total,quorum_timeout_total,da_gate_total,censorship_evidence_total,missing_payload_total,missing_qc_total,validation_reject_total,last_cause,last_cause_timestamp_ms}, tx_queue{depth,capacity,saturated}, epoch{length_blocks,commit_deadline_offset,reveal_deadline_offset}, membership{height,view,epoch,view_hash}, gossip_fallback_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, validation_reject_total, validation_reject_reason, block_sync_roster{commit_certificate_hint_total,checkpoint_hint_total,commit_certificate_history_total,checkpoint_history_total,roster_sidecar_total,commit_roster_journal_total,drop_missing_total}, pacemaker_backpressure_deferrals_total, commit_pipeline_tick_total, da_reschedule_total, rbc_store{sessions,bytes,pressure_level,backpressure_deferrals_total,evictions_total,recent_evictions[...]}, prf{height,view,epoch_seed}, vrf_penalty_epoch, vrf_committed_no_reveal_total, vrf_no_participation_total, vrf_late_reveals_total, collectors_targeted_current, collectors_targeted_last_per_block, redundant_sends_total, pipeline_conflict_rate_bps, access_set_sources{manifest_hints,entrypoint_hints,prepass_merge,conservative_fallback}, worker_loop{stage,stage_started_ms,last_iteration_ms,queue_depths{vote_rx,block_payload_rx,rbc_chunk_rx,block_rx,consensus_rx,lane_relay_rx,background_rx}} }`. Note: `view_change_causes.da_gate_total` est reserve pour compatibilite et doit rester a zero; les avertissements de disponibilite DA sont exposes via `status.da_gate` et `sumeragi_da_gate_block_total{reason="missing_local_data"}`. `da_reschedule_total` est legacy.
- GET `/v1/sumeragi/qc` - renvoie un snapshot de commit certificate (`SumeragiQcSnapshot`) encode Norito par defaut. Definir `Accept: application/json` pour recevoir `{ highest_qc { height, view, subject_block_hash }, locked_qc { height, view, subject_block_hash } }`.
- GET `/v1/sumeragi/status/sse` - flux SSE du meme payload (cadence ~1s).
- GET `/v1/sumeragi/exec_root/:hash` - renvoie un payload Norito `(block_hash, Option<ExecRoot>)` par defaut. Passer `Accept: application/json` pour recevoir `{ block_hash, exec_root }` (hex) ou `exec_root = null` si absent.
- GET `/v1/sumeragi/exec_qc/:hash` - renvoie un `ExecutionQcRecord` encode Norito pour `:hash` (hash du bloc parent) par defaut. Avec `Accept: application/json` la reponse s etend a:
  - `post_state_root` (hex), `height`, `view`, `epoch`, `signers_bitmap` (hex), `bls_aggregate_signature` (hex).
  - Si absent, renvoie `{ subject_block_hash, exec_qc: null }`.

Exemple (curl)

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

Exemple exec root (curl)

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

Lignes jq rapides (coherence + wrappers)

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

Note
- L egalite cross-height (child vs parent) est appliquee en interne par Sumeragi en mode strict
  (`require_wsv_exec_qc=true`) et n est pas encore exposee via une requete. Les verifications de
  coherence visibles par l operateur ci-dessus confirment que l ExecutionQC du parent correspond
  a son exec_root persiste. Un futur endpoint exposera les roots proposal/header pour permettre
  une comparaison parent-vs-child externe.
