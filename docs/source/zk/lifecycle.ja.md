<!-- Japanese translation of docs/source/zk/lifecycle.md -->

---
lang: ja
direction: ltr
source: docs/source/zk/lifecycle.md
status: complete
translator: manual
---

# 検証キーと証明のライフサイクル

本ドキュメントは、検証鍵 (VK) とゼロ知識証明エンベロープが Iroha v2 の中でどのように流れるかを整理したものです。オンチェーンの状態、Torii のアプリケーションエンドポイント、CLI ヘルパーを 1 つの参照に束ね、オペレーターと SDK 作者が全体像を素早く把握できるようにすることを目的としています。

## ハイレベルな流れ

1. **権威ある VK の作成** – オペレーターまたはコンパイラ成果物が検証鍵と 32 バイトのコミットメントを生成する。VK はバックエンド (`backend::name`) 単位で名前空間化されたバージョン管理が行われる。
2. **Torii を介したアドミッション** – オペレーターは署名済みのガバナンス命令を Torii (`/v1/zk/vk/*`) に送信する。受理されたトランザクションは VK レジストリを登録または更新し、オンチェーンに保存されてピア間で複製される。
3. **コントラクト／ランタイムでの利用** – トランザクションやスマートコントラクトは `(backend, name)` で VK を参照するか、インライン VK ペイロードを埋め込む。実行時に証明検証の過程で VK コミットメントが解決される。
4. **証明検証** – クライアントは `/v1/zk/verify` または `/v1/zk/submit-proof` で証明を送信する。検証はトランザクション実行中の `iroha_core::zk` 内で行われ、成功すると Torii (`/v1/zk/proofs*`) からクエリできる `ProofRecord` が生成される。
5. **バックグラウンド報告** – オプションの Torii プローバーワーカー (`torii.zk_prover_enabled=true`) がアタッチメントをスキャンし、`ProofAttachment` ペイロードを検証してテレメトリを公開する。レポートは設定された TTL 経過後に自動削除される。

## 検証キー

- レジストリエントリはワールドステートの `verifying_keys[(backend, name)]` に保存される。
- 新しい VK を登録する際は、コミットメントがトランザクションに同梱されたハッシュ済みペイロードやインラインバイト列と一致している必要がある。更新ではバージョンを単調に増やさなければならない。

### 関連エンドポイント

- `POST /v1/zk/vk/register` – 署名済みの `RegisterVerifyingKey` 命令を送信する。
- `POST /v1/zk/vk/update` – より高いバージョンの `UpdateVerifyingKey` を送信する。
- `POST /v1/zk/vk/deprecate` – 既存の VK を廃止状態にマークする。
- `GET  /v1/zk/vk` – `backend`、`status`、`name_contains` などのフィルター付きで VK を一覧取得する。
- `GET  /v1/zk/vk/{backend}/{name}` – 単一の VK レコードを取得する。

### CLI ヘルパー

`iroha_cli zk` コマンドは、Torii が期待する JSON DTO を送信する薄いラッパーを提供する。

- `iroha_cli zk vk register --json path/to/register.json`
- `iroha_cli zk vk update --json path/to/update.json`
- `iroha_cli zk vk deprecate --json path/to/deprecate.json`
- `iroha_cli zk vk get --backend halo2/ipa --name vk_transfer`

JSON DTO は `iroha_data_model::proof` のペイロードをそのまま写している。インライン VK のバイト列は Base64 のまま保持され、コミットメントは小文字の 16 進文字列で渡す。

## 証明のライフサイクル

### 提出と検証

- 証明エンベロープは `/v1/zk/verify`（同期）または `/v1/zk/submit-proof`（後で検査する用途）で受け付ける。どちらのエンドポイントも Norito でエンコードしたエンベロープまたは JSON DTO を受理する。
- トランザクション実行中、`iroha_core::smartcontracts::isi::zk::VerifyProof` は証明バイト列をバックエンド名と組み合わせてハッシュ化し、`ProofId` を導出して台帳上の一意性を確保する。
- 検証器はインラインペイロード、参照された `(backend, name)` の組、またはその両方から VK コミットメントを解決する。`debug/*` で登録されたバックエンドは開発用途のため暗号学的チェックをバイパスする。
- 得られた `ProofRecord` には以下が格納される。
  - `backend` と `proof_hash`
  - `status`（`Submitted`、`Verified`、`Rejected`）
  - `verified_at_height`（検証が完了したブロック高）
  - 任意の `vk_ref` と `vk_commitment`
- ZK1/TLV エンベロープは検証時に解析される。既知の 4 バイトタグは遅延記録され、タグベースのクエリに活用される。

### クエリ面

`/v1/zk/proofs` と `/v1/zk/proofs/count` は台帳上のレコードを公開する。

- 利用可能なフィルター: `backend`、`status`、`has_tag`、`offset`、`limit`、`order=asc|desc`、`ids_only`。
- タグフィルタリングは効率的で、検証時にタグがインデックス化され、専用の `(tag → proof ids)` インデックスから提供される。
- `ids_only=true` を指定すると、軽量なページング向けに `{ backend, hash }` オブジェクトのみを返す。
- `/v1/zk/proof/{backend}/{hash}` も直接参照用に利用できる。

### CLI カバレッジ

CLI には `iroha_cli zk proofs` 以下に新しいサブコマンドが追加されている。

- `iroha_cli zk proofs list [--backend halo2/ipa] [--status Verified] [--has-tag PROF] [--limit 20]`
- `iroha_cli zk proofs count [--backend halo2/ipa] [--has-tag IPAK]`
- `iroha_cli zk proofs get --backend halo2/ipa --hash 0123...`

すべてのコマンドは Norito JSON レスポンスを出力する。フィルターは HTTP クエリパラメータと 1 対 1 で対応し、ページネーションのスクリプト化や監視ツールへの連携が容易になる。

## バックグラウンドプローバーとテレメトリ

- `torii.zk_prover_enabled`、`torii.zk_prover_scan_period_secs`、`torii.zk_prover_reports_ttl_secs`、`torii.zk_prover_max_inflight`、`torii.zk_prover_max_scan_bytes`、`torii.zk_prover_max_scan_millis`、`torii.zk_prover_keys_dir`、`torii.zk_prover_allowed_backends`、`torii.zk_prover_allowed_circuits` といった `iroha_config` の設定で制御する。
- アタッチメントは `ProofAttachment`/`ProofAttachmentList`（Norito または JSON）としてデコード可能である必要がある。ZK1/TLV はタグ付けされるがトップレベルのペイロードとしては拒否される。
- バックエンドは prefix マッチの許可リストで制御される（既定 `["halo2/"]`）。`groth16/…` と `stark/…` は本番ビルドでは未サポート。
- 各レポートは `latency_ms = processed_ms - created_ms` を記録し、オペレーターがキュー遅延を把握できるようにする。
- プローバーが公開するテレメトリ:
  - `torii_zk_prover_attachment_bytes`（`content_type` ラベル付きヒストグラム）
  - `torii_zk_prover_latency_ms`（ヒストグラム）
  - `torii_zk_prover_inflight`（ゲージ）と `torii_zk_prover_pending`（ゲージ）
  - `torii_zk_prover_last_scan_bytes` と `torii_zk_prover_last_scan_ms`（ゲージ）
  - `torii_zk_prover_budget_exhausted_total{reason}`（カウンタ）
  - `zk_verify_latency_ms` と `zk_verify_proof_bytes`（`backend` ラベル付きヒストグラム）
- テレメトリは、メトリクス公開を許可するプロファイルで有効化された場合に `/metrics` から取得できる。
- TTL を過ぎたレポートは各スキャンのたびにガーベジコレクションされる。手動削除は `/v1/zk/prover/reports` からも継続して行える。

Nightly Milestone 0 の実行では新しいヒストグラムをスクレイプし、既存の Torii オペレーターダッシュボードと並べてロールアップを公開するため、証明検証レイテンシのリグレッションを迅速に検知できる。
