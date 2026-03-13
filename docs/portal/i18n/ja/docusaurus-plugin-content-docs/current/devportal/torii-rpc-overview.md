---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Norito-RPC 概要

Norito-RPC は Torii API 用のバイナリトランスポートです。`/v2/pipeline` と同じ HTTP パスを再利用しますが、スキーマハッシュとチェックサムを含む Norito 形式のペイロードをやり取りします。決定的で検証済みの応答が必要な場合や、pipeline の JSON 応答がボトルネックになる場合に利用してください。

## なぜ切り替えるのか
- CRC64 とスキーマハッシュによる決定的なフレーミングでデコードエラーを減らせます。
- SDK 間で共有される Norito ヘルパーにより、既存のデータモデル型を再利用できます。
- Torii は Norito セッションをテレメトリーで既にタグ付けしているため、提供ダッシュボードで採用状況を監視できます。

## リクエストの送信

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. Norito codec (`iroha_client`、SDK ヘルパー、または `norito::to_bytes`) でペイロードをシリアライズします。
2. `Content-Type: application/x-norito` でリクエストを送信します。
3. `Accept: application/x-norito` を指定して Norito 応答を要求します。
4. 対応する SDK ヘルパーで応答をデコードします。

SDK 別のガイド:
- **Rust**: `iroha_client::Client` は `Accept` ヘッダーを設定すると Norito を自動交渉します。
- **Python**: `iroha_python.norito_rpc` の `NoritoRpcClient` を使用します。
- **Android**: Android SDK の `NoritoRpcClient` と `NoritoRpcRequestOptions` を使用します。
- **JavaScript/Swift**: ヘルパーは `docs/source/torii/norito_rpc_tracker.md` で追跡され、NRPC-3 の一部として提供されます。

## Try It コンソールの例

開発者ポータルには Try It プロキシがあり、レビュー担当者が専用スクリプトを書かずに Norito ペイロードを再生できます。

1. [プロキシを起動](./try-it.md#start-the-proxy-locally)し、`TRYIT_PROXY_PUBLIC_URL` を設定してウィジェットが送信先を把握できるようにします。
2. このページの **Try it** カードか `/reference/torii-swagger` パネルを開き、`POST /v2/pipeline/submit` などのエンドポイントを選択します。
3. **Content-Type** を `application/x-norito` に切り替え、**Binary** エディタを選択し、`fixtures/norito_rpc/transfer_asset.norito` をアップロードします（または `fixtures/norito_rpc/transaction_fixtures.manifest.json` に記載の任意のペイロード）。
4. OAuth device-code ウィジェットまたは手動トークン欄で bearer token を提供します（`TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` を設定するとプロキシは `X-TryIt-Auth` の上書きを許可します）。
5. リクエストを送信し、Torii が `fixtures/norito_rpc/schema_hashes.json` に記載の `schema_hash` を返すことを確認します。一致するハッシュは、ブラウザ/プロキシ経由でも Norito ヘッダーが保持されたことを示します。

ロードマップの証拠として、Try It のスクリーンショットと `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` の実行結果を組み合わせてください。スクリプトは `cargo xtask norito-rpc-verify` をラップし、JSON サマリを `artifacts/norito_rpc/<timestamp>/` に書き込み、ポータルが使用したものと同じ fixtures を取得します。

## トラブルシューティング

| 症状 | 表れる場所 | 想定原因 | 対処 |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Torii 応答 | `Content-Type` ヘッダー不足または誤り | ペイロード送信前に `Content-Type: application/x-norito` を設定してください。 |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Torii 応答の本文/ヘッダー | fixture のスキーマハッシュが Torii ビルドと一致しない | `cargo xtask norito-rpc-fixtures` で fixtures を再生成し、`fixtures/norito_rpc/schema_hashes.json` のハッシュを確認してください。エンドポイントがまだ Norito を有効化していない場合は JSON にフォールバックしてください。 |
| `{"error":"origin_forbidden"}` (HTTP 403) | Try It プロキシ応答 | 要求元が `TRYIT_PROXY_ALLOWED_ORIGINS` にない | ポータルのオリジン（例: `https://docs.devnet.sora.example`）を環境変数に追加してプロキシを再起動してください。 |
| `{"error":"rate_limited"}` (HTTP 429) | Try It プロキシ応答 | IP ごとのクォータが `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` を超過 | 内部負荷テストなら制限を増やすか、ウィンドウがリセットされるまで待ってください（JSON 応答の `retryAfterMs` を参照）。 |
| `{"error":"upstream_timeout"}` (HTTP 504) または `{"error":"upstream_error"}` (HTTP 502) | Try It プロキシ応答 | Torii がタイムアウト、またはプロキシが設定されたバックエンドに到達できない | `TRYIT_PROXY_TARGET` の到達性を確認し、Torii のヘルスをチェックするか、`TRYIT_PROXY_TIMEOUT_MS` を大きくして再試行してください。 |

Try It の追加診断と OAuth のヒントは [`devportal/try-it.md`](./try-it.md#norito-rpc-samples) にあります。

## 追加リソース
- トランスポート RFC: `docs/source/torii/norito_rpc.md`
- エグゼクティブサマリ: `docs/source/torii/norito_rpc_brief.md`
- アクショントラッカー: `docs/source/torii/norito_rpc_tracker.md`
- Try-It プロキシ手順: `docs/portal/docs/devportal/try-it.md`
