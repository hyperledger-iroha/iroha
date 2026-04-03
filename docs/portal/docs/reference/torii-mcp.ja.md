<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/portal/docs/reference/torii-mcp.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 316a408473f53a9763a18f40d49cfd766b5b93a3611e277e5a761e366e85c082
source_last_modified: "2026-03-15T11:38:44.302824+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
id: torii-mcp
title: Torii MCP API
description: Torii のネイティブ モデル コンテキスト プロトコル ブリッジを使用するためのリファレンス ガイド。
---

Torii は、`/v1/mcp` でネイティブのモデル コンテキスト プロトコル (MCP) ブリッジを公開します。
このエンドポイントにより、エージェントはツールを検出し、JSON-RPC 経由で Torii/Connect ルートを呼び出すことができます。

## 終点の形状

- `GET /v1/mcp` は機能メタデータ (JSON-RPC ラップされていない) を返します。
- `POST /v1/mcp` は JSON-RPC 2.0 リクエストを受け入れます。
- `torii.mcp.enabled = false` の場合、どちらのルートも公開されません。
- `torii.require_api_token` が有効な場合、欠落または無効なトークンは JSON-RPC ディスパッチ前に拒否されます。

## 構成

`torii.mcp` で MCP を有効にします。

```json
{
  "torii": {
    "mcp": {
      "enabled": true,
      "max_request_bytes": 1048576,
      "max_tools_per_list": 500,
      "profile": "read_only",
      "expose_operator_routes": false,
      "allow_tool_prefixes": [],
      "deny_tool_prefixes": [],
      "rate_per_minute": 240,
      "burst": 120,
      "async_job_ttl_secs": 300,
      "async_job_max_entries": 2000
    }
  }
}
```

主な動作:

- `profile` はツールの可視性を制御します (`read_only`、`writer`、`operator`)。
- `allow_tool_prefixes`/`deny_tool_prefixes` は、追加の名前ベースのポリシーを適用します。
- `rate_per_minute`/`burst` は、MCP リクエストにトークン バケット制限を適用します。
- `tools/call_async` からの非同期ジョブ状態は、`async_job_ttl_secs` および `async_job_max_entries` を使用してメモリ内に保持されます。

## 推奨されるクライアント フロー

1. `initialize` に電話します。
2. `tools/list` を呼び出し、`toolsetVersion` をキャッシュします。
3. 通常の操作には `tools/call` を使用します。
4. 長時間の操作には、`tools/call_async` + `tools/jobs/get` を使用します。
5. `listChanged` が `true` の場合、`tools/list` を再実行します。

完全なツール カタログをハードコーディングしないでください。実行時に検出します。

## メソッドとセマンティクス

サポートされている JSON-RPC メソッド:

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

注:- `tools/list` は、`toolset_version` と `toolsetVersion` の両方を受け入れます。
- `tools/jobs/get` は、`job_id` と `jobId` の両方を受け入れます。
- `tools/list.cursor` は数値文字列オフセットです。無効な値は `0` にフォールバックします。
- `tools/call_batch` はアイテムごとのベストエフォートです (1 つの呼び出しが失敗しても、兄弟呼び出しは失敗しません)。
- `tools/call_async` はエンベロープ形状のみを即座に検証します。実行エラーは、ジョブの状態の後半で発生します。
- `jsonrpc` は `"2.0"` である必要があります。省略 `jsonrpc` は互換性のために受け入れられます。

## 認証と転送

MCP ディスパッチは Torii 認証をバイパスしません。呼び出しでは、通常のルート ハンドラーと認証チェックが実行されます。

Torii は、ツールのディスパッチのために受信認証関連ヘッダーを転送します。

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

クライアントは、`arguments.headers` 経由で追加の呼び出しごとのヘッダーを提供することもできます。
`arguments.headers` からの `content-length`、`host`、および `connection` は無視されます。

## エラーモデル

HTTP 層:

- `400` 無効な JSON
- `403` API トークンが JSON-RPC 処理の前に拒否されました
- `413` ペイロードが `max_request_bytes` を超えています
- `429` レート制限あり
- JSON-RPC 応答の場合は `200` (JSON-RPC エラーを含む)

JSON-RPC レイヤー:- トップレベルの `error.data.error_code` は安定しています (例: `invalid_request`、`invalid_params`、`tool_not_found`、`tool_not_allowed`、`job_not_found`、`rate_limited`)。
- ツールの障害は、MCP ツールの結果として `isError = true` および構造化された詳細として表面化します。
- ルートディスパッチされたツールの障害は、HTTP ステータスを `structuredContent.error_code` にマップします (たとえば、`forbidden`、`not_found`、`server_error`)。

## ツールの名前付け

OpenAPI 派生ツールは、安定したルートベースの名前を使用します。

- `torii.<method>_<path...>`
- 例: `torii.get_v1_accounts`

厳選されたエイリアスも `iroha.*` および `connect.*` で公開されます。

## 正規仕様

完全な電信レベルの契約は次の場所で管理されます。

- `crates/iroha_torii/docs/mcp_api.md`

`crates/iroha_torii/src/mcp.rs` または `crates/iroha_torii/src/lib.rs` で動作が変化すると、
同じ変更でその仕様を更新し、キーの使用方法のガイダンスをここに反映します。