<!-- Japanese translation of docs/source/governance_api.md -->

---
title: ガバナンスアプリ API — エンドポイント（ドラフト）
lang: ja
direction: ltr
source: docs/source/governance_api.md
status: complete
translator: manual
---

ステータス: 実装タスクに添付するドラフト／スケッチ段階です。実装中に形状が変わる可能性があります。決定論性と RBAC ポリシーは必須要件です。`authority` と `private_key` が提供された場合、Torii はトランザクションを署名・送信します。そうでなければクライアントが命令スケルトンを受け取り、`/transaction` に送信します。

概要
- すべてのエンドポイントは JSON を返します。トランザクションを生成するフローでは `tx_instructions`（命令スケルトンの配列）が応答に含まれます:
  - `wire_id`: 命令種別を示すレジストリ識別子
  - `payload_hex`: Norito でエンコードされたペイロード（16進）
- `authority` と `private_key` が提供された場合（または ballot DTO の `private_key` 指定時）、Torii はトランザクションを署名・送信し、`tx_instructions` は引き続き返します。
- それ以外はクライアントが `authority` と `chain_id` を使って `SignedTransaction` を組み立て、署名後に `/transaction` へ POST します。
- CLI 補助コマンド:
  - `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]`
  - Validates canonical account ids, canonicalizes 32-byte nullifier hints, and merges the hints into `public_inputs_json` (with `--public <path>` for additional overrides).
  - The nullifier is derived from the proof commitment (public input) plus `domain_tag`, `chain_id`, and `election_id`; `--nullifier` is validated against the proof when supplied.
  - When any lock hint is provided, ZK ballots must supply `owner`, `amount`, and `duration_blocks`; partial hints are rejected. When `min_bond_amount > 0`, lock hints are required. Direction remains optional and is treated as a hint only.
  - `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
    - `--lock-amount` / `--lock-duration-blocks` の別名をサポートし、ZK コマンドと同様に fingerprint とヒントをサマリーと JSON へ出力します。

## エンドポイント一覧

### POST `/v1/gov/proposals/deploy-contract`
- リクエスト JSON
  ```json
  {
    "namespace": "apps",
    "contract_id": "my.contract.v1",
    "code_hash": "blake2b32:…" | "…64hex",
    "abi_hash": "blake2b32:…" | "…64hex",
    "abi_version": "1",
    "window": { "lower": 12345, "upper": 12400 },
    "authority": "i105...?",
    "private_key": "…?"
  }
  ```
- レスポンス JSON
  ```json
  { "ok": true, "proposal_id": "…64hex", "tx_instructions": [{ "wire_id": "…", "payload_hex": "…" }] }
  ```
- バリデーション: ノードは指定された `abi_version` に従って `abi_hash` を正規化し、値が一致しない場合に拒否します。`abi_version = "v1"` の場合、期待値は `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))` です。

### Contracts API（デプロイ）
- POST `/v1/contracts/deploy`
  - リクエスト: `{ "authority": "i105...", "private_key": "…", "code_b64": "…" }`
  - 挙動: IVM プログラム本体から `code_hash` を計算し、ヘッダの `abi_version` から `abi_hash` を導出したうえで、`RegisterSmartContractCode`（マニフェスト）と `RegisterSmartContractBytes`（完全な `.to` バイト列）を `authority` の代理で送信します。
  - レスポンス: `{ "ok": true, "code_hash_hex": "…", "abi_hash_hex": "…" }`
  - 関連エンドポイント:
    - GET `/v1/contracts/code/{code_hash}` → 保管済みマニフェストを返す
    - GET `/v1/contracts/code-bytes/{code_hash}` → `{ "code_b64": "…" }` を返す
- POST `/v1/contracts/instance`
  - リクエスト: `{ "authority": "i105...", "private_key": "…", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "…" }`
  - 挙動: バイトコードのデプロイと `(namespace, contract_id)` へのバインドを単一トランザクションで行います（`RegisterSmartContractCode` + `RegisterSmartContractBytes` + `ActivateContractInstance`）。
  - レスポンス: `{ "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "…", "abi_hash_hex": "…" }`

#### コードサイズ上限
- カスタムパラメータ `max_contract_code_bytes`（JSON u64）
  - オンチェーンに保存できるコントラクトコードの最大サイズ（バイト）を制御します。
  - 既定値は 16 MiB。`.to` イメージのサイズが上限を超える `RegisterSmartContractBytes` は不変条件違反として拒否されます。
  - オペレーターは `id = "max_contract_code_bytes"`、数値ペイロードの `SetParameter(Custom)` 命令で調整できます。

### POST `/v1/gov/ballots/zk`
- リクエスト: `{ "authority": "i105...", "private_key": "…?", "chain_id": "…", "election_id": "e1", "proof_b64": "…", "public": { … } }`
- レスポンス: `{ "ok": true, "accepted": true, "tx_instructions": [{ … }] }`
- 備考:
  - 回路の公開入力に `owner`, `amount`, `duration_blocks` が含まれ、構成済みの検証鍵に対して証明が通ると、ノードは `election_id` と `owner` に対するガバナンスロックを新規作成または延長します。方向性（賛否）は公開されず `unknown` として扱われ、金額と有効期限のみ更新されます。再投票は単調増加であり、金額と有効期限は増加方向にしか変えられません（ノードは `max(amount, prev.amount)` と `max(expiry, prev.expiry)` を適用します）。
  - 金額や有効期限を縮小しようとする ZK 再投票は `BallotRejected` で拒否されます。
  - コントラクトは `SubmitBallot` をキューに入れる前に `ZK_VOTE_VERIFY_BALLOT` を呼び出す必要があり、ホストはワンショットラッチを強制します。

### POST `/v1/gov/ballots/plain`
- リクエスト: `{ "authority": "i105...", "private_key": "…?", "chain_id": "…", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }`
- レスポンス: `{ "ok": true, "accepted": true, "tx_instructions": [{ … }] }`
- 備考: 再投票は延長のみ許可され、既存ロックの金額や期限を減らすことはできません。`owner` はトランザクションの `authority` と一致している必要があります。最小学習期間は `conviction_step_blocks` に基づきます。

### POST `/v1/gov/finalize`
- リクエスト: `{ "referendum_id": "r1", "proposal_id": "…64hex", "authority": "i105...?", "private_key": "…?" }`
- レスポンス: `{ "ok": true, "tx_instructions": [{ "wire_id": "…FinalizeReferendum", "payload_hex": "…" }] }`
- オンチェーン効果（現時点のスケルトン）: 承認済みデプロイ提案を実行すると、`code_hash` をキーに期待される `abi_hash` を持つ最小限の `ContractManifest` を登録し、提案を Enacted に設定します。すでに同じ `code_hash` に別の `abi_hash` を持つマニフェストが存在する場合は enact を拒否します。
- 備考:
  - ZK 投票では、`FinalizeElection` を実行する前にコントラクト経路が `ZK_VOTE_VERIFY_TALLY` を呼び出す必要があります。ホストはワンショットラッチを強制します。`FinalizeReferendum` は選挙集計が確定するまで ZK レファレンダムを拒否します。
  - `h_end` の自動クローズは Plain レファレンダムのみ Approved/Rejected を発行します。ZK は集計が確定して `FinalizeReferendum` が実行されるまで Closed のままです。
  - ターンアウト判定は approve+reject のみを使用し、abstain はカウントしません。

### POST `/v1/gov/enact`
- リクエスト: `{ "proposal_id": "…64hex", "preimage_hash": "…64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105...?", "private_key": "…?" }`
- レスポンス: `{ "ok": true, "tx_instructions": [{ "wire_id": "…EnactReferendum", "payload_hex": "…" }] }`
- 備考: `authority`/`private_key` が提供されると Torii が署名・送信し、それ以外は命令スケルトンを返します。`preimage_hash` は任意で現在は情報目的のみ。

### GET `/v1/gov/proposals/{id}`
- パス `{id}`: 提案 ID（16 進文字列 64 桁）
- レスポンス: `{ "found": bool, "proposal": { … }? }`

### GET `/v1/gov/locks/{rid}`
- パス `{rid}`: レファレンダム ID 文字列
- レスポンス: `{ "locks": [ { "owner": "…", "amount": "…", "expires_at": … }, … ] }`

### GET `/v1/gov/locks/summary`
- クエリパラメータ: `rid`（レファレンダム ID）
- レスポンス例: `{ "lock_count": 10, "total_amount": "12345", "expired_locks_now": 2, "last_sweep_height": 98765 }`
- 備考: `last_sweep_height` は期限切れロックを掃除して永続化した最新ブロック高、`expired_locks_now` は現在の高さにおいて期限切れとなるロック数です。

### POST `/v1/gov/ballots/zk-v1`
- リクエスト（v1 形式 DTO）
  ```json
  {
    "authority": "i105...",
    "chain_id": "00000000-0000-0000-0000-000000000000",
    "private_key": "…?",
    "election_id": "ref-1",
    "backend": "halo2/ipa",
    "envelope_b64": "AAECAwQ=",
    "root_hint": "0x…64hex?",
    "owner": "i105...?",
    "amount": "100?",
    "duration_blocks": 6000?,
    "direction": "Aye|Nay|Abstain?",
    "nullifier": "blake2b32:…64hex?"
  }
  ```
- レスポンス: `{ "ok": true, "accepted": true, "tx_instructions": [{ … }] }`

### POST `/v1/gov/ballots/zk-v1/ballot-proof`（feature: `zk-ballot`）
- `BallotProof` JSON を直接受け取り、`CastZkBallot` スケルトンを返します。
- リクエスト
  ```json
  {
    "authority": "i105...",
    "chain_id": "00000000-0000-0000-0000-000000000000",
    "private_key": "…?",
    "election_id": "ref-1",
    "ballot": {
      "backend": "halo2/ipa",
      "envelope_bytes": "AAECAwQ=",
      "root_hint": null,
      "owner": null,
      "nullifier": null,
      "amount": "100",
      "duration_blocks": 6000,
      "direction": "Aye"
    }
  }
  ```
- レスポンス
  ```json
  {
    "ok": true,
    "accepted": true,
    "reason": "build transaction skeleton",
    "tx_instructions": [ { "wire_id": "CastZkBallot", "payload_hex": "…" } ]
  }
  ```
- 備考:
  - サーバーは `root_hint` / `owner` / `amount` / `duration_blocks` / `direction` / `nullifier` の任意値を命令の `public_inputs_json` に写像します。
  - エンベロープバイト列は命令ペイロードで再度 base64 エンコードされます。
  - Torii が提出する場合、応答の `reason` は `submitted transaction` に変わります。
  - このエンドポイントは `zk-ballot` フィーチャが有効な場合のみ利用できます。

#### CastZkBallot 検証フロー
- `CastZkBallot` は渡された base64 証明をデコードし、空または不正形式のペイロードを `BallotRejected`（`invalid or empty proof`）で拒否します。
- `public_inputs_json` が指定されている場合は JSON オブジェクトである必要があり、非オブジェクトは拒否されます。
- ホストはレファレンダムまたはガバナンス既定値から投票用検証鍵（`vk_ballot`）を解決し、レコードが存在し `Active` であり、インラインバイトを保持していることを要求します。
- 保管されている検証鍵バイト列を `hash_vk` で再ハッシュし、コミットメントが一致しない場合は検証前に中断して改ざんを防ぎます（`BallotRejected` with `verifying key commitment mismatch`）。
- 証明バイト列は登録済みバックエンド経由で `zk::verify_backend` に渡されます。無効なトランスクリプトは `BallotRejected`（`invalid proof`）として決定論的に失敗します。
- The proof must expose a ballot commitment and eligibility root as public inputs; the root must match the election’s `eligible_root`, and the derived nullifier must match any provided hint.
- 成功した証明では `BallotAccepted` を発行します。重複ヌリファイア、陳腐化した適格ルート、ロックの後退などは従来どおり拒否理由を返します。

## バリデータの不正行為と共同コンセンサス

### スラッシング／ジャイリングのワークフロー

コンセンサス層はバリデータがプロトコルに違反した際に Norito エンコードされた `Evidence` を生成します。各ペイロードはメモリ上の `EvidenceStore` に記録され、未処理であれば WSV 支援の `consensus_evidence` マップに永続化されます。`sumeragi.npos.reconfig.evidence_horizon_blocks`（既定値 7,200 ブロック）より古いレコードはアーカイブサイズを制御するために拒否されますが、拒否イベントはオペレーター向けにログ出力されます。 Evidence within the horizon also respects `sumeragi.npos.reconfig.activation_lag_blocks` (default `1`) and the slashing delay `sumeragi.npos.reconfig.slashing_delay_blocks` (default `259200`); governance can cancel penalties with `CancelConsensusEvidencePenalty` before slashing applies; the record is marked `penalty_cancelled` and `penalty_cancelled_at_height`.

認識されている違反は `EvidenceKind` と一対一に対応し、識別子（discriminant）はデータモデルによって固定されています。

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidQc,
    EvidenceKind::InvalidProposal,
    EvidenceKind::Censorship,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

...（残りの詳細は実装進行に合わせてドキュメント化される予定です）。