<!-- Japanese translation of docs/source/torii_contracts_api.md -->

---
lang: ja
direction: ltr
source: docs/source/torii_contracts_api.md
status: complete
translator: manual
---

# Torii Contracts API（マニフェスト & デプロイ）

本ドキュメントは、スマートコントラクトのマニフェストを公開・取得するためのアプリ向け HTTP エンドポイントを説明します。エンドポイントはオンチェーントランザクションや参照専用クエリの薄いラッパーであり、合意セマンティクスはオンチェーンで保たれます。

## エンドポイント

- **POST `/v1/contracts/code`**
  - `RegisterSmartContractCode` 命令を署名付きトランザクションにラップして送信します。
  - リクエストボディ: `RegisterContractCodeDto`（JSON または Norito JSON。後述）。
  - レスポンス: 正常受理時は HTTP `202 Accepted`。その他は標準のトランザクション受理エラー。
  - 認可: `authority` は `CanRegisterSmartContractCode` 権限を保持する必要があります。既定のエグゼキュータではこの権限の付与はジェネシス時のみ許可されています。
  - テレメトリ: ハンドラーエラー時は `torii_contract_errors_total{endpoint="code"}`、レートリミッター作動時は `torii_contract_throttled_total{endpoint="code"}` がインクリメントされます。
  - 失敗レスポンス例:

    | シナリオ | HTTP ステータス | ボディ | 備考 |
    | --- | --- | --- | --- |
    | 正常キュー投入 | `202 Accepted` | 空 | 署名済みペイロードからトランザクションハッシュを取得し、パイプラインで最終結果をポーリングします。 |
    | 権限不足 (`CanRegisterSmartContractCode` 欠如) | `202 Accepted` | 後述のステータス参照 | リクエストはキュー投入されるが、後で `ValidationFail::NotPermitted` により拒否されます。 |
    | `manifest.code_hash` / `manifest.abi_hash` が 32 バイト小文字 hex ではない | `400 Bad Request` | `invalid JSON body: invalid hex string ... manifest.code_hash` など | Norito JSON 抽出時に検証されます。 |
    | トランザクションキュー満杯 | `429 Too Many Requests` | キュー拒否 JSON（`code`, `message`, `queue`, `retry_after_seconds` など） | レスポンスに `Retry-After` とキュー状態ヘッダが付与されます。 |

    権限不足時に `/v1/pipeline/transactions/status` が返す拒否例（読みやすく整形）:

    ```json
    {
      "kind": "Transaction",
      "content": {
        "hash": "…",
        "status": {
          "kind": "Rejected",
          "content": "<base64 TransactionRejectionReason>"
        }
      }
    }
    ```

    Base64 `content` をデコードすると `TransactionRejectionReason::Validation(ValidationFail::NotPermitted("not permitted: CanRegisterSmartContractCode"))` となります。

    キュー拒否レスポンス例:

    ```json
    {
      "code": "queue_full",
      "message": "transaction queue is at capacity",
      "queue": {
        "state": "saturated",
        "queued": 65536,
        "capacity": 65536,
        "saturated": true
      },
      "retry_after_seconds": 1
    }
    ```

- **GET `/v1/contracts/code/{code_hash}`**
  - コンテントアドレス化された `code_hash` によりオンチェーンの `ContractManifest` を取得します。
  - パスパラメータ: 32 バイト hex 文字列。
  - レスポンスボディ: `ContractCodeRecordDto`（JSON）。`manifest` のみを返します。

- **POST `/v1/contracts/deploy`**
  - Base64 の `.to` バイトコード、権限アカウント、秘密鍵を受け取り、`code_hash`（プログラム本体）と `abi_hash`（ヘッダー `abi_version` 由来）を算出します。
  - リクエストボディ: `DeployContractDto`。レスポンスボディ: `DeployContractResponseDto`。
  - 単一トランザクションに `RegisterSmartContractCode`（マニフェスト）と `RegisterSmartContractBytes`（コード保存）をまとめて送信します。
  - バイトコードサイズはカスタムパラメータ `max_contract_code_bytes`（既定 16 MiB）以内である必要があります。より大きなプログラムをアップロードする場合は事前に値を引き上げてください。
  - テレメトリ: ハンドラーエラー時は `torii_contract_errors_total{endpoint="deploy"}`、レートリミッター作動時は `torii_contract_throttled_total{endpoint="deploy"}` がインクリメントされます。
- **POST `/v1/contracts/instance`**
  - Base64 の `.to` バイトコードと対象 `(namespace, contract_id)` を受け取り、デプロイとアクティベーションを一度に行います。
  - `RegisterSmartContractCode`、`RegisterSmartContractBytes`、`ActivateContractInstance` を同一トランザクションで送信します。
  - レスポンスボディ: `{ "ok": true, "namespace": "…", "contract_id": "…", "code_hash_hex": "…", "abi_hash_hex": "…" }`。
  - テレメトリ: ハンドラーエラー時は `torii_contract_errors_total{endpoint="instance"}`、レートリミッター作動時は `torii_contract_throttled_total{endpoint="instance"}` がインクリメントされます。

- **GET `/v1/contracts/code-bytes/{code_hash}`**
  - 指定 `code_hash` のコードバイトを取得します。
  - レスポンスボディ: `{ "code_b64": "…" }`。

- **POST `/v1/contracts/instance/activate`**
  - `(namespace, contract_id)` に `code_hash` をバインドする `ActivateContractInstance` を送信します。
  - リクエストボディ: `ActivateInstanceDto`。レスポンスボディ: `ActivateInstanceResponseDto`。
  - テレメトリ: ハンドラーエラー時は `torii_contract_errors_total{endpoint="activate"}`、レートリミッター作動時は `torii_contract_throttled_total{endpoint="activate"}` がインクリメントされます。

- **GET `/v1/contracts/instances/{ns}`**
  - ネームスペース `ns` における有効なコントラクトインスタンス一覧を返します（ガバナンスの一覧 API と同様の構造）。
  - クエリ: `contains`, `hash_prefix`, `offset`, `limit`, `order`（ガバナンス API と同じ意味）。
  - レスポンス: `{ namespace, instances: [{ contract_id, code_hash_hex }], total, offset, limit }`。

## スキーマ

### RegisterContractCodeDto

署名付きトランザクションにラップしてマニフェストを登録するリクエスト。

```jsonc
{
  "authority": "ih58...",
  "private_key": "ed25519:...",
  "manifest": {
    "code_hash": "0123…cdef",
    "abi_hash": "89ab…7654",
    "compiler_fingerprint": "rustc-1.79 llvm-16",
    "features_bitmap": 0
  }
}
```

- `manifest.code_hash` を指定すると、ノードは `code_hash` キーでマニフェストを保存します。
- `manifest.abi_hash` がある場合、ノードの ABI ポリシーと照合されます。

### DeployContractDto

コンパイル済みバイトコードをアップロードし、Torii にマニフェストとハッシュの計算を任せるリクエスト。

```jsonc
{
  "authority":   "ih58...", // アカウント ID（文字列表現）
  "private_key": "ed25519:0123…",    // ExposedPrivateKey（素の multihash もしくは接頭辞付き）
  "code_b64":    "Base64Payload=="
}
```

- `code_b64` は `abi_version == 1` の IVM プログラムヘッダーにデコードできる必要があります。
- このショートカットではマニフェストをクライアントから送信しません。ノードが内部で生成します。
- デコード後のバイトコード長は `max_contract_code_bytes` 以下でなければなりません。超過するとトランザクション実行時に `InvariantViolation`（`code bytes exceed cap`）で拒否されます。

### DeployContractResponseDto

```jsonc
{
  "ok": true,
  "code_hash_hex": "0123…cdef",
  "abi_hash_hex":  "89ab…7654"
}
```

### JSON の型エンコード

- `Hash`（`code_hash`, `abi_hash` など）は 64 文字の小文字 hex（32 バイト）。
- `AccountId`: canonical IH58 literal (no `@domain` suffix; optional `@<domain>` hint).
- `ExposedPrivateKey` は素の multihash hex 文字列と、アルゴリズム接頭辞付き（例 `ed25519:…`）のどちらも受け付けます。レスポンスでは素の multihash hex に正規化されます。

### GET レスポンス: ContractCodeRecordDto

```jsonc
{
  "manifest": {
    "code_hash": "0123…cdef",
    "abi_hash": "89ab…7654",
    "compiler_fingerprint": "rustc-1.79 llvm-16",
    "features_bitmap": 0
  }
}
```

### DeployAndActivateInstanceDto

デプロイとインスタンスのアクティベーションを同時に行うリクエスト。

```jsonc
{
  "authority":   "ih58...",
  "private_key": "ed25519:…",
  "namespace":   "apps",
  "contract_id": "calc.v1",
  "code_b64":    "…",
  "manifest": {                      // 任意。指定した場合も code_hash / abi_hash はノード側で検証されます
    "compiler_fingerprint": "rustc-1.79 llvm-16",
    "features_bitmap": 0,
    "access_set_hints": {
      "read_keys": ["account:ih58..."],
      "write_keys": ["asset:usd#wonderland"]
    }
  }
}
```

- `manifest.code_hash` を指定した場合は、アップロードしたコードから算出される `code_hash` と一致する必要があります（不一致なら拒否されます）。
- ABI ハッシュもノードが再計算します。`manifest.abi_hash` を指定した場合は同じ値でなければなりません。
- `compiler_fingerprint` や `features_bitmap`、`access_set_hints` はそのままマニフェストに保存されます。

### レスポンス: DeployAndActivateInstanceResponseDto

```jsonc
{
  "ok": true,
  "namespace": "apps",
  "contract_id": "calc.v1",
  "code_hash_hex": "0123…cdef",
  "abi_hash_hex": "89ab…7654"
}
```

### ActivateInstanceDto

既存のマニフェスト／コードハッシュをネームスペース内のコントラクト識別子にバインドするリクエスト。

```jsonc
{
  "authority":   "ih58...",
  "private_key": "ed25519:0123…",
  "namespace":   "apps",
  "contract_id": "calc.v1",
  "code_hash":   "89ab…7654"
}
```

- `code_hash` は 32 バイトの小文字 hex である必要があります。先頭に `0x` が付いていても受け付けられ、取り除かれます。
- 指定した `code_hash` に対応するマニフェストとコードバイトが既にオンチェーンに存在していなければなりません。存在しない場合は後続の検証で拒否されます。
- ガバナンス保護されたネームスペースでは、引き続き `gov_namespace` / `gov_contract_id` メタデータが必須です（CLI ヘルパーが自動設定します）。

### ActivateInstanceResponseDto

```jsonc
{
  "ok": true
}
```

### Norito ペイロード

ここで紹介したすべての DTO は `JsonSerialize` と `NoritoSerialize` の両方を実装しています。クライアントはプレーン JSON か Norito バックエンド JSON のどちらでも送信できます。Kotodama やテストから Norito を出力する場合は、同じフィールド名／エンコードを保った `norito::json::json!` を利用してください。`NoritoJson<T>` エクストラクターが決定論的にデコードします。

### レート制限とテレメトリ

- `torii.deploy_rate_per_origin_per_sec` と `torii.deploy_burst_per_origin` は `/v1/contracts/{code,deploy,instance,instance/activate}` を保護する共通トークンバケットを設定します。既定値は 1 オリジンあたり 1 秒に 4 リクエスト、バースト 8（`X-API-Token`、リモート IP、エンドポイント名の組み合わせで識別）です。
- レートリミッターで拒否されたリクエストは `torii_contract_throttled_total{endpoint}`（`endpoint` は `code` / `deploy` / `instance` / `activate`）をインクリメントします。
- ハンドラー内で発生したエラー（不正ボディ、権限不足、キュー失敗など）は `torii_contract_errors_total{endpoint}` をインクリメントします。キューテレメトリと合わせて監視してください。

## 利用例

マニフェストを登録する:

```bash
curl -s -X POST \
  -H 'Content-Type: application/json' \
  -d '{
        "authority": "ih58...",
        "private_key": "ed25519:…",
        "manifest": { "code_hash": "<32-byte-hex>", "abi_hash": null }
      }' \
  http://127.0.0.1:8080/v1/contracts/code
```

マニフェストの取得:

```bash
curl -s http://127.0.0.1:8080/v1/contracts/code/<32-byte-hex> | jq .
```

デプロイとコードバイト取得:

```bash
curl -s -X POST \
  -H 'Content-Type: application/json' \
  -d '{
        "authority": "ih58...",
        "private_key": "ed25519:…",
        "code_b64": "…"
      }' \
  http://127.0.0.1:8080/v1/contracts/deploy | jq .

curl -s http://127.0.0.1:8080/v1/contracts/code-bytes/<32-byte-hex> | jq .
```

デプロイとアクティベーションを同時に行う:

```bash
curl -s -X POST \
  -H 'Content-Type: application/json' \
  -d '{
        "authority": "ih58...",
        "private_key": "ed25519:…",
        "namespace": "apps",
        "contract_id": "calc.v1",
        "code_b64": "…"
      }' \
  http://127.0.0.1:8080/v1/contracts/instance | jq .
```

既存のアーティファクトをアクティベートする:

```bash
curl -s -X POST \
  -H 'Content-Type: application/json' \
  -d '{
        "authority": "ih58...",
        "private_key": "ed25519:…",
        "namespace": "apps",
        "contract_id": "calc.v1",
        "code_hash": "<32-byte-hex>"
      }' \
  http://127.0.0.1:8080/v1/contracts/instance/activate | jq .
```

### `abi_hash` の算出

マニフェストに `abi_hash` を含めることで、プログラムをノードの IVM ABI ポリシーに結び付けられます。CLI でローカル算出が可能です。

```bash
# ABI v1
iroha ivm abi-hash --policy v1 --uppercase
```

32 バイト hex ダイジェストが出力されるため、`manifest.abi_hash` に設定します。ノードはランタイムポリシーとの一致を確認し、差異があれば受理時に拒否します。

## セキュリティとガバナンス

- `CanRegisterSmartContractCode` を持つアカウントだけがマニフェストを登録できます。既定ではこの権限の付与はガバナンスにより制限されます。
- GET は参照専用で `code_hash` によるコンテントアドレスです。ノードはポリシーに応じたアクセス制御を追加する場合があります。
