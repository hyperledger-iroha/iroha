<!-- Japanese translation of docs/source/contract_deployment.md -->

---
lang: ja
direction: ltr
source: docs/source/contract_deployment.md
status: complete
translator: manual
---

# コントラクトデプロイ（.to）— API & ワークフロー

ステータス: Torii / CLI / Core の受理テストで検証済み（2025 年 11 月）

## 概要

- Torii へ .to バイトコードを送信する、または `RegisterSmartContractCode` / `RegisterSmartContractBytes` を直接発行することで IVM バイトコードをデプロイします。
- ノードは `code_hash` と正規の ABI ハッシュを再計算し、一致しない場合は決定論的に拒否します。
- マニフェストはオンチェーンの `contract_manifests`、コードバイトは `contract_code` レジストリに保存されます。マニフェストはハッシュ参照のみで小型を維持し、コードは `code_hash` キーで格納されます。
- 保護されたネームスペースは、デプロイ前にガバナンス提案の施行を必要とできます。受理パスは提案ペイロードを参照し、`(namespace, contract_id, code_hash, abi_hash)` の一致を強制します。

## 保存されるアーティファクトと保持

- `RegisterSmartContractCode` は指定 `code_hash` のマニフェストを挿入（上書き）します。同じハッシュが既に存在する場合は置換されます。
- `RegisterSmartContractBytes` は `contract_code[code_hash]` にコンパイル済みコードを保存します。同一ハッシュで既存バイト列がある場合は完全一致が必要で、差異は不変条件違反になります。
- コードサイズ上限はカスタムパラメータ `max_contract_code_bytes`（既定 16 MiB）。大きなアーティファクトを登録する前に `SetParameter(Custom)` トランザクションで更新します。
- 保持期間は無期限です。マニフェストとコードは明示的に削除するまで残り、TTL や GC はありません。

## 受理パイプライン

- 検証時に IVM ヘッダーを解析し、`version_major == 2`、`abi_version == 1` を強制します。未対応バージョンは即拒否となりランタイムトグルは存在しません。
- 既に `code_hash` のマニフェストがある場合、保存済みハッシュと提出プログラムから計算したハッシュが一致するか確認し、差異は `Manifest{Code,Abi}HashMismatch` を投げます。
- 保護ネームスペース宛てトランザクションはメタデータ `gov_namespace` と `gov_contract_id` を含める必要があります。受理パスは施行済み `DeployContract` 提案と照合し、一致がなければ `NotPermitted` で拒否します。

## Torii エンドポイント（feature `app_api`）

- **POST `/v1/contracts/deploy`**  
  リクエスト: `DeployContractDto`（`authority`, `private_key`, `code_b64`）。ハッシュ計算とマニフェスト生成後、`RegisterSmartContractCode` と `RegisterSmartContractBytes` を単一トランザクションで送信します。レスポンス: `{ ok, code_hash_hex, abi_hash_hex }`。
- **POST `/v1/contracts/code`**  
  リクエスト: `RegisterContractCodeDto`（`authority`, `private_key`, `manifest`）。マニフェストのみ登録します。
- **POST `/v1/contracts/instance`**  
  リクエスト: `DeployAndActivateInstanceDto`（`authority`, `private_key`, `namespace`, `contract_id`, `code_b64`, 任意の `manifest` 上書き）。デプロイとアクティベーションを原子的に実行します。レスポンス: `{ ok, namespace, contract_id, code_hash_hex, abi_hash_hex }`。
- **POST `/v1/contracts/instance/activate`**  
  リクエスト: `ActivateInstanceDto`（`authority`, `private_key`, `namespace`, `contract_id`, `code_hash`）。既存アーティファクトを指定インスタンスにバインドします。レスポンス: `{ ok: true }`。
- **GET `/v1/contracts/code/{code_hash}`**  
  `{ manifest: { code_hash, abi_hash } }` を返します。
- **GET `/v1/contracts/code-bytes/{code_hash}`**  
  保存済み `.to` バイト列を Base64 で返します（`{ code_b64 }`）。

全てのコントラクト系エンドポイントは共有レートリミッタ `torii.deploy_rate_per_origin_per_sec` / `torii.deploy_burst_per_origin` を使用します（既定 4 req/s、バースト 8。`X-API-Token`、リモート IP、エンドポイント名の組み合わせでオリジンを識別）。無効化する場合は `null` を指定します。制限に達すると HTTP 429 とともに `torii_contract_throttled_total{endpoint="code|deploy|instance|activate"}` が増加し、ハンドラー内エラーは `torii_contract_errors_total{endpoint=…}` に記録されます。

## ガバナンス統合と保護ネームスペース

- カスタムパラメータ `gov_protected_namespaces`（JSON 配列）を設定すると受理ゲートが有効化されます。Torii `/v1/gov/protected-namespaces` と CLI (`iroha_cli gov protected-set/get`) が補助します。
- `ProposeDeployContract`（または `/v1/gov/proposals/deploy-contract`）で作成される提案は `(namespace, contract_id, code_hash, abi_hash, abi_version)` を記録します。
- 住民投票が可決し `EnactReferendum` で施行されると、同じメタデータとコードを持つデプロイが受理されます。
- トランザクションには `gov_namespace` / `gov_contract_id` のペアを必ず指定します。CLI は `--namespace` / `--contract-id` で自動付与します。

## CLI ヘルパー

- `iroha_cli contract deploy` — Torii デプロイを呼び出し、ハッシュ計算を自動化。
- `iroha_cli contract manifest` — マニフェスト取得。
- `iroha_cli contract code-bytes-get` — 保存済み `.to` をダウンロード。
- `iroha_cli contract instances` — 有効コントラクトインスタンスを一覧表示。
- `iroha_cli gov propose-deploy` ほか各種ガバナンスコマンドで保護ネームスペースワークフローを支援します。

## テストとカバレッジ

- `crates/iroha_core/tests/contract_code_bytes.rs` がコード保存とサイズ上限を検証。
- `crates/iroha_core/tests/gov_enact_deploy.rs` が施行経路を検証し、`gov_protected_gate.rs` が保護ネームスペースの受理を E2E テスト。
- Torii ルートはリクエスト／レスポンスのユニットテスト、CLI は JSON 往復の統合テストを持ちます。

詳細は `docs/source/governance_api.md` を参照してください。
