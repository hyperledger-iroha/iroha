<!-- Japanese translation of docs/source/torii/router.md -->

---
lang: ja
direction: ltr
source: docs/source/torii/router.md
status: complete
translator: manual
---

# Torii ルーター構成

Torii の HTTP サーフェスは段階的ビルダーを通じて公開され、関連ルートをグルーピングしつつ共有ステートを一度だけ配線します。`RouterBuilder` ヘルパーは `iroha_torii::router::builder` にあり、`axum::Router<SharedAppState>` をラップします。各 `Torii::add_*_routes` ヘルパーは `&mut RouterBuilder` を受け取り、`RouterBuilder::apply` を介して登録を行います。

## ビルダーパターン

このビルダー API により、従来の「新しい `Router` を返す」パターンは不要になりました。ヘルパー呼び出しごとにルーターを再代入する代わりに、連続してヘルパーを呼び出します。

```rust
let mut builder = RouterBuilder::new(app_state.clone());

// 機能に依存しないグループ
torii.add_sumeragi_routes(&mut builder);
torii.add_telemetry_routes(&mut builder);
torii.add_core_info_routes(&mut builder);

// オプション機能グループ
#[cfg(feature = "connect")]
torii.add_connect_routes(&mut builder);
#[cfg(feature = "app_api")]
torii.add_app_api_routes(&mut builder);

let router = builder.finish();
```

このアプローチにより次の点が保証されます。

- 共有 `AppState` は必要な場合のみクローンされます（`apply_with_state` はステートを必要とするハンドラ用に引き続き利用可能）。
- コンパイル時の機能フラグに依存せず、機能付きヘルパーが決定的に合成されます。
- テストは中間的な `Router` 値を扱わずとも、さまざまな組み合わせを検証できます。

## リグレッションテスト

統合テスト `crates/iroha_torii/tests/router_feature_matrix.rs` は最小構成のインメモリスタックで Torii を構築し、アクティブな機能セットでルーターがビルドできることを確認します。スキーマ／OpenAPI エンドポイントがコンパイルされている場合、テストは生成された仕様をスナップショットと比較することも可能です。

- `IROHA_TORII_OPENAPI_EXPECTED=/path/to/openapi.json` を設定すると、生成されたドキュメントがスナップショットと一致するか検証します。
- `IROHA_TORII_OPENAPI_ACTUAL=/tmp/iroha-openapi.json` を設定すると、現在の出力を書き出し手動確認や diff ツールに利用できます。

OpenAPI エンドポイントが利用できない場合、差分処理は自動的にスキップされます。テストでは機能付きのスモークリクエスト（例: `app_api` 有効時の `/v2/domains`、`connect` 有効時の `/v2/connect/status`）も行い、段階的ビルダーがすべてのルートグループを網羅していることを確認します。

## 移行ノート

Torii を組み込むプロジェクトは、従来の `Router` を直接操作する実装を `RouterBuilder` ヘルパーへ移行してください。`add_*_routes` から変更済みルーターを返す歴史的なパターンは廃止され、クレート内部ではサポートされなくなりました。

## 参考資料

- `crates/iroha_torii/docs/mcp_api.md` — エージェント/ツール統合向け Torii MCP JSON-RPC 契約（`/v2/mcp`）。
