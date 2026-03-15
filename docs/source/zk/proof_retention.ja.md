---
lang: ja
direction: ltr
source: docs/source/zk/proof_retention.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 16b5b6bd9cf1bfdc901d0aa3c863addd77e033a0e91a8187fa354b4d22058bf9
source_last_modified: "2026-01-22T15:38:30.730892+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/zk/proof_retention.md -->

% 証明保持と剪定

Iroha は監査とリプレイのために、証明検証結果 (backend + hash) のレジストリを保持する。
保持はコンセンサス経路の中で決定的に強制される。

## 設定

`zk` 設定が保持を制御する:

- `zk.proof_history_cap` — backend ごとの最大記録数 (0 = 無制限)
- `zk.proof_retention_grace_blocks` — 年齢ベースの剪定前に保持する最小ブロック数
- `zk.proof_prune_batch` — 1 回の強制パスでの最大削除数 (0 = 無制限)

これらの値は `iroha_config` (user → actual → defaults) から読み取られる。

## 強制

- **挿入時:** `VerifyProof` は新しいエントリを記録した後、証明の backend に対して cap/grace/batch を適用する。
- **手動:** 新しい `PruneProofs` 命令は同じポリシーで全 backend (または指定された単一 backend) を剪定する。CLI ヘルパーを使用する:

  ```bash
  iroha app zk proofs prune --backend halo2/ipa
  ```

両方の経路は `ProofEvent::Pruned` を発行し、backend、削除 ID ( `prune_batch` により上限)、
残件数、cap/grace/batch、高さ、権限、起点 (`Insert` または `Manual`) を監査用に含める。

## 可視化とツール

- ステータスエンドポイント: `GET /v2/proofs/retention` は caps, grace, prune_batch,
  総レコード数、剪定可能数、backend ごとの件数を返す。
- CLI: `iroha app zk proofs retention` (ステータス) と `iroha app zk proofs prune` (手動強制)。
- イベント: SSE/WS フィルタで `DataEvent::Proof(ProofEvent::Pruned)` を購読し、剪定アクティビティを監視する。

剪定は決定的でブロック実行内で行われるため、全ピアが同じ保持集合に収束する。
削除された証明 ID はタグインデックスからも除去される。
