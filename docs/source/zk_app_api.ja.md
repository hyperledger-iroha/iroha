<!-- Japanese translation of docs/source/zk_app_api.md -->

---
lang: ja
direction: ltr
source: docs/source/zk_app_api.md
status: complete
translator: manual
---

# ZK アプリ API: 添付ファイルとプローバーレポート（運用ガイド）

本ドキュメントは、Torii が公開する ZK 添付ファイルおよびバックグラウンドプローバーレポート用エンドポイントを説明します。これらはコンセンサス非依存であり、検証・実行・ブロック形成には影響しません。運用ツールや UI/UX フロー向けです。

主要特性:
- 決定的でフォークしない挙動。ワーカーを無効化してもコンセンサス結果は変わらない。
- フィーチャーフラグ `app_api`（Torii ではデフォルト有効）で保護。
- レート制限および API トークンによる保護が可能。
- 既定で `./storage/torii` 配下に保存。

## 添付ファイル

添付ファイルは証明エンベロープや JSON DTO などのサニタイズ済みアーティファクトを保存します。識別子は決定的に決まります。

エンドポイント:
- `POST /v2/zk/attachments` — 添付ファイルを保存し、`{ id, size, content_type, created_ms }` を返す。
- `GET  /v2/zk/attachments` — 保存済みメタデータを一覧（JSON 配列）。クエリフィルタ: `id`, `content_type`, `since_ms`, `before_ms`, `has_tag=<TAG>`（ZK1 TLV タグ、例: `PROF`, `IPAK`）、`limit`, `offset`, `order=asc|desc`, `ids_only=true`。
- `GET  /v2/zk/attachments/:id` — ID 指定で保存済みバイトを取得。コンテントタイプ維持。
- `DELETE /v2/zk/attachments/:id` — 添付ファイルとメタデータを削除。
- `GET  /v2/zk/attachments/count` — 同フィルタで `{ count }` を取得。
- `GET  /v2/zk/proof/{backend}/{hash}` — バックエンドとハッシュで証明レコードを取得（JSON `{ backend, proof_hash, status, verified_at_height?, vk_ref?, vk_commitment? }`）。
- `GET  /v2/zk/proofs` — 証明レコード一覧。フィルタ: `backend`, `status=Submitted|Verified|Rejected`, `has_tag=<TAG>`（`zk-proof-tags` が必要）, `verified_from_height`, `verified_until_height`, `limit`, `offset`, `order=asc|desc`, `ids_only=true`。
- `GET  /v2/zk/proofs/count` — 証明レコード数 `{ count }` を返す。

詳細:
- ID はサニタイズ後のリクエストボディの Blake2b-32（小文字 hex）。
- `Content-Type` はマジックバイトの sniffing で正規化され、宣言ヘッダは `provenance` に記録。
- サニタイズ: gzip/zstd は `torii.attachments_max_expanded_bytes` と `torii.attachments_max_archive_depth` の範囲で展開し、`torii.attachments_allowed_mime_types` のみ許可。実行モードは `torii.attachments_sanitizer_mode`。旧データ（`provenance` 無し）は配信時に再サニタイズ。
- サイズ上限は `torii.attachments_max_bytes`（既定 4 MiB）。超過時は `413 Payload Too Large`。
- テナント毎の上限: `torii.attachments_per_tenant_max_count`／`_max_bytes`。テナントは `X-API-Token` を `token:<blake2b-32>` にハッシュして決定し、トークンなしは `anon`。上限を越えるアップロードでは最古の添付を削除。単体で上限超過なら即 `413`。
- TTL: `torii.attachments_ttl_secs`（既定 7 日）経過で GC が 60 秒周期で削除。
- ストレージ配置:
  - データ: `storage/torii/zk_attachments/<id>.bin`
  - メタデータ: `storage/torii/zk_attachments/<id>.json`

## バックグラウンドプローバーレポート

バックグラウンドプローバーワーカー（既定無効）は添付ファイルを検証し、`ProofAttachment`/`ProofAttachmentList` をバックエンド検証器で確認した結果をレポートします。ZK1/TLV をトップレベルペイロードとしては受け付けず、タグのみ記録して `ok=false` とします:

- Norito (`application/x-norito`): `ProofAttachment` または `ProofAttachmentList` としてデコードできること。
- JSON (`application/json`, `text/json`): `ProofAttachment` オブジェクト、`ProofAttachmentList`（base64 文字列）、または `ProofAttachment` の配列としてデコードできること。
- その他: JSON → Norito の順でデコードを試み、失敗時は `ok: false` でエラーを記録。

エンドポイント:
- `GET /v2/zk/prover/reports` — レポート一覧（フィルタ: `ok_only`, `failed_only`, `errors_only`, `id`, `content_type`, `has_tag`, `limit`, `since_ms`, `before_ms`, `order`, `offset`, `latest`, `ids_only`, `messages_only`）。
- `GET /v2/zk/prover/reports/:id` — 単一レポート取得。
- `DELETE /v2/zk/prover/reports` — フィルタ条件に一致するレポートを一括削除。戻り値 `{ deleted, ids }`。
- `DELETE /v2/zk/prover/reports/:id` — 単一削除。

プローバーワーカー設定（Torii `config.json`）:
- `torii.zk_prover_enabled` — 有効化（既定 false）。
- `torii.zk_prover_scan_period_secs` — 定期スキャン周期（秒）。
- `torii.zk_prover_reports_ttl_secs` — レポート保持 TTL。
- `torii.zk_prover_max_inflight` / `torii.zk_prover_max_scan_bytes` / `torii.zk_prover_max_scan_millis` — 並列数とスキャン予算。
- `torii.zk_prover_keys_dir` — インライン VK がない場合に読み込む鍵ストア。
- `torii.zk_prover_allowed_backends` / `torii.zk_prover_allowed_circuits` — 許可リスト（prefix マッチ）。

動作:
- 未処理添付ファイルを予算内で取り出し内容を検証。成功時 `ok: true`、失敗時 `ok: false` とエラー文字列。
- レポートは JSON (`storage/torii/zk_prover/reports/<id>.json`) に保存。
- 予算超過分は次サイクルへ持ち越す。

## CLI ワークフロー

`iroha app zk` サブコマンドが添付／レポート操作をラップします。

添付ファイル:
- アップロード: `iroha app zk attach --file myproof.zk1 --content-type application/x-norito`
- 一覧: `iroha app zk list-attachments --limit 10`
- ダウンロード: `iroha app zk download-attachment --id <id> --output proof.zk1`
- 削除: `iroha app zk delete-attachment --id <id>`

レポート:
- 一覧: `iroha app zk list-reports --failed-only`
- 削除: `iroha app zk delete-reports --ids <id1> <id2>`

プローバー操作:
- 起動: `iroha app zk prover start`（バックグラウンドスレッドを生成）
- 停止: `iroha app zk prover stop`
- ステータス: `iroha app zk prover status`

検証スタブ:
- `iroha app zk verify --json <PATH>` / `--norito <PATH>`
- `iroha app zk submit-proof --json <PATH>` / `--norito <PATH>`

## 検証鍵レジストリ（アプリ API）

Torii にはオンチェーンの検証鍵レジストリを扱う仲介エンドポイントがあります。署名付きトランザクションの生成／送信を自動化します。

エンドポイント:
- `POST /v2/zk/vk/register` — `RegisterVerifyingKey` を送信
- `POST /v2/zk/vk/update` — `UpdateVerifyingKey`（version を増分）
- `POST /v2/zk/vk/deprecate` — `DeprecateVerifyingKey`
- `GET  /v2/zk/vk/{backend}/{name}` — レコード取得

- `vk_bytes` (base64) — 検証鍵本体。`commitment_hex` 指定時は一致を検証。`vk_len` を追加すると長さを事前検証できます。
- `commitment_hex` (64 hex) — コミットメントのみを運ぶ場合。鍵本体を省略するため `vk_len` を必須で指定します。

`GET` 応答は次の構造を返します:

```json5
{
  "id": { "backend": "halo2/ipa", "name": "vk_main" },
  "record": {
    "version": 3,
    "circuit_id": "halo2/ipa::transfer_v3",
    "backend": "halo2-ipa-pasta",
    "curve": "pallas",
    "public_inputs_schema_hash": "…",
    "commitment": "…",
    "vk_len": 40960,
    "max_proof_bytes": 8192,
    "gas_schedule_id": "halo2_default",
    "metadata_uri_cid": "ipfs://…",
    "vk_bytes_cid": "ipfs://…",
    "activation_height": 1200,
    "deprecation_height": null,
    "withdraw_height": null,
    "status": "Active",
    "key": { "backend": "halo2/ipa", "bytes_b64": "..." }
  }
}
```

`ids_only=true` の場合は `{ "backend": "...", "name": "..." }` のみを返します。

CLI:
- 登録: `iroha app zk vk register --json ./vk_register.json`
- 更新: `iroha app zk vk update --json ./vk_update.json`
- 廃止: `iroha app zk vk deprecate --json ./vk_deprecate.json`
- 取得: `iroha app zk vk get --backend <backend> --name <name>`

注意:
- コミットメントは `backend || bytes` のドメイン分離ハッシュ。送信時に検証。

### VK レジストリエベント購読

`DataEventFilter.VerifyingKey` を用いて登録／更新／廃止イベントに購読可能。CLI では JSON5 フィルタを指定。

例:

CLI:


### 証明イベント購読

`DataEventFilter.Proof` を使って証明検証イベントに購読可能。CLI プリセットあり。

例: 成功のみを監視
```
iroha ledger trigger register \
  --id proof_successes \
  --filter data \
  --data-proof halo2/ipa:0123abcd... \
  --data-proof-only verified \
  --path ./on_verified.ko
```

## 制限と今後の課題

- プローバーワーカーは証明生成（proving）は行わず、検証のみを実施する（非コンセンサス）。
- レポート保持は手動（API またはファイル削除）。添付ファイルは TTL GC のみ。
- 証明レコードの一覧／集計 API は未実装。将来追加時は添付と類似のフィルタ（`has_tag=<TAG>` を含む）を提供予定。その際は検証時にタグ集合を記録し検索効率を向上させる必要がある。

## ガバナンスエンドポイント（ZK 投票）

ZK 投票関連のエンドポイントは Governance App API を参照（`private_key` がある場合は Torii が署名・送信し、ない場合はスケルトンを返す）:
- `POST /v2/gov/ballots/zk` — `CastZkBallot` スケルトンを返す
- `POST /v2/gov/ballots/zk-v1` — v1 形式 DTO
- `POST /v2/gov/ballots/zk-v1/ballot-proof` — `BallotProof` JSON を直接受け付け（`zk-ballot` フィーチャー）

詳細は `docs/source/governance_api.md` を参照。
