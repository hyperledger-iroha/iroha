<!-- Japanese translation of docs/source/sumeragi_evidence_api.md -->

---
lang: ja
direction: ltr
source: docs/source/sumeragi_evidence_api.md
status: complete
translator: manual
---

# Sumeragi エビデンス（監査 API）

Sumeragi エビデンス用の一時的な監査エンドポイント。

- GET `/v1/sumeragi/evidence/count`
  - ノードが観測したユニークな Evidence エントリ数を返します。
  - レスポンス（Norito ペイロード）: `count: u64`
  - `Accept: application/json` を指定すると `{ "count": <u64> }` を取得。
  - 備考:
    - ノードごとの WSV ストア（`world.consensus_evidence`）に保存され、Norito コーデックで永続化。
    - 再起動後も保持され `/v1/sumeragi/evidence` のデータソースになります。エビデンスハッシュで重複排除。
    - 現時点では各バリデータにローカル（コンセンサス複製なし）。ガバナンス取り込みは今後対応。

- GET `/v1/sumeragi/evidence`
  - WSV の監査スナップショットに保存された最近のエビデンスを一覧表示。
  - クエリパラメータ: `limit`（既定 50、最大 1000）、`offset`（既定 0）、`kind`（任意、`DoublePrevote|DoublePrecommit|InvalidQC|InvalidProposal` のいずれか）。
  - レスポンス（Norito ペイロード）: `(total, Vec<EvidenceRecord>)`
  - `Accept: application/json` で `{ "total": <u64>, "items": [ ... ] }` を取得。
  - `sumeragi.npos.reconfig.evidence_horizon_blocks`（既定 7200）より古い高さのエビデンスは取り込み時に破棄され、オペレーターが古い投稿を調査できるようアクターがログを出力。

- POST `/v1/sumeragi/evidence`
  - 16 進 Norito エンコードのエビデンスを Sumeragi アクター（`ControlFlow::Evidence`）に送信。
  - リクエストボディ（JSON）: `{ "evidence_hex": "<hex string>" }`（hex は Norito-framed `ConsensusEvidence` バイト列、空白は無視）。
  - レスポンス（JSON）: `{ "status": "accepted", "kind": "<variant>" }`
  - バリデーションは二重投票ペイロードの署名者／高さ／ビュー／エポック一致を確認し、単一署名ペイロードは空でないことを要求。`Censorship` は署名済み `TransactionSubmissionReceipt` のレシート・クォラムを要求し、`InvalidProposal` は高さ／ビューを前進させないものや埋め込み QC と親ハッシュが一致しないものを拒否。
  - CLI ヘルパー: `iroha sumeragi evidence submit --evidence-hex <hex>` または `--evidence-hex-file <path>`

追加のコンセンサスステータスと実行証跡

- GET `/v1/sumeragi/status` — 既定で Norito エンコードされた `SumeragiStatusWire` を返します。`Accept: application/json` で `{ leader_index, view_change_index, highest_qc{height,view,subject_block_hash}, locked_qc{height,view,subject_block_hash}, commit_certificate{height,view,epoch,block_hash,validator_set_hash,validator_set_len,signatures_total}, commit_quorum{height,view,block_hash,signatures_present,signatures_counted,signatures_set_b,signatures_required,last_updated_ms}, view_change_causes{commit_failure_total,quorum_timeout_total,da_gate_total,censorship_evidence_total,missing_payload_total,missing_qc_total,validation_reject_total,last_cause,last_cause_timestamp_ms}, tx_queue{depth,capacity,saturated}, epoch{length_blocks,commit_deadline_offset,reveal_deadline_offset}, membership{height,view,epoch,view_hash}, gossip_fallback_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, validation_reject_total, validation_reject_reason, block_sync_roster{commit_certificate_hint_total,checkpoint_hint_total,commit_certificate_history_total,checkpoint_history_total,roster_sidecar_total,commit_roster_journal_total,drop_missing_total}, pacemaker_backpressure_deferrals_total, commit_pipeline_tick_total, da_reschedule_total, rbc_store{sessions,bytes,pressure_level,backpressure_deferrals_total,evictions_total,recent_evictions[...]}, prf{height,view,epoch_seed}, vrf_penalty_epoch, vrf_committed_no_reveal_total, vrf_no_participation_total, vrf_late_reveals_total, collectors_targeted_current, collectors_targeted_last_per_block, redundant_sends_total, pipeline_conflict_rate_bps, access_set_sources{manifest_hints,entrypoint_hints,prepass_merge,conservative_fallback}, worker_loop{stage,stage_started_ms,last_iteration_ms,queue_depths{vote_rx,block_payload_rx,rbc_chunk_rx,block_rx,consensus_rx,lane_relay_rx,background_rx}} }` を取得。注: `view_change_causes.da_gate_total` は互換性維持のための予約で通常 0 のままです。DA 可用性の警告は `status.da_gate` と `sumeragi_da_gate_block_total{reason="missing_availability_qc"}` に出ます。`da_reschedule_total` はレガシーです。
- GET `/v1/sumeragi/qc` — 既定で Norito エンコードされた `SumeragiQcSnapshot` を返します。`Accept: application/json` で `{ highest_qc { height, view, subject_block_hash }, locked_qc { height, view, subject_block_hash } }` を取得。
- GET `/v1/sumeragi/status/sse` — 上記ステータスの SSE ストリーム（約 1 秒間隔）。
- GET `/v1/sumeragi/exec_root/:hash` — Norito ペイロード `(block_hash, Option<ExecRoot>)` を返します。`Accept: application/json` で `{ block_hash, exec_root }`（16 進）または `exec_root = null`。
- GET `/v1/sumeragi/exec_qc/:hash` — 指定ハッシュ（親ブロックハッシュ）に対する Norito エンコード済み `ExecutionQcRecord` を返します。`Accept: application/json` を指定すると `post_state_root`（16 進）、`height`、`view`、`epoch`、`signers_bitmap`（16 進）、`bls_aggregate_signature`（16 進）を含む JSON を取得。存在しない場合は `{ subject_block_hash, exec_qc: null }`。

例（curl）

```bash
# HASH を実ブロックの親ハッシュ（16 進 32 バイト）に置き換えてください
HASH=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2

curl -s \
  http://127.0.0.1:8080/v1/sumeragi/exec_qc/$HASH | jq .
```

実行ルート取得例（curl）

```bash
HASH=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2

curl -s \
  http://127.0.0.1:8080/v1/sumeragi/exec_root/$HASH | jq .
```

## jq ワンライナー（整合性チェックと補助）

- 親ブロックの ExecQC と exec_root 一致確認:

```bash
P=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2
QC=$(curl -s http://127.0.0.1:8080/v1/sumeragi/exec_qc/$P | jq -r '.post_state_root // empty')
RT=$(curl -s http://127.0.0.1:8080/v1/sumeragi/exec_root/$P | jq -r '.exec_root // empty')
if [ -z "$QC" ]; then echo "no ExecQC record in WSV for $P"; exit 1; fi
if [ -z "$RT" ]; then echo "no exec_root in WSV for $P"; exit 1; fi
if [ "$QC" = "$RT" ]; then echo "OK: parent ExecQC equals exec_root ($QC)"; else echo "MISMATCH: QC=$QC root=$RT"; fi
```

- シェル補助関数:

```bash
get_exec_qc_root() { curl -s "http://127.0.0.1:8080/v1/sumeragi/exec_qc/$1" | jq -r '.post_state_root // empty'; }
get_exec_root()    { curl -s "http://127.0.0.1:8080/v1/sumeragi/exec_root/$1" | jq -r '.exec_root // empty'; }
check_parent_consistency() {
  local h="$1"; local qc=$(get_exec_qc_root "$h"); local rt=$(get_exec_root "$h");
  if [ -z "$qc" ]; then echo "no ExecQC record for $h"; return 2; fi
  if [ -z "$rt" ]; then echo "no exec_root for $h"; return 3; fi
  if [ "$qc" = "$rt" ]; then echo "OK: $h QC==root ($qc)"; else echo "MISMATCH: QC=$qc root=$rt"; return 1; fi
}
```

備考
- クロス高さの整合（子 vs 親）は Sumeragi の厳格モード（`require_wsv_exec_qc=true`）によって内部的に保証されており、まだクエリから直接確認する手段はありません。上記のチェックにより、親の ExecutionQC レコードと永続化された exec_root が一致するかを確認できます。将来的には提案／ヘッダーのルートを公開し、親子比較を外部から実行できるエンドポイントを追加予定です。
