---
lang: ja
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito Try-It コンソール
description: 開発者ポータルのプロキシと Swagger、RapiDoc ウィジェットを使い、ドキュメントサイトから Torii / Norito-RPC の実リクエストを直接送信します。
---

ポータルには Torii にトラフィックを中継する 3 つの対話的な画面が用意されています:

- **Swagger UI** は `/reference/torii-swagger` にあり、署名済み OpenAPI 仕様を表示し、`TRYIT_PROXY_PUBLIC_URL` が設定されているとリクエストをプロキシ経由に自動的に書き換えます。
- **RapiDoc** は `/reference/torii-rapidoc` にあり、`application/x-norito` に適したファイルアップロードとコンテンツタイプ選択を備えた同じスキーマを公開します。
- **Try it sandbox** は Norito 概要ページにあり、アドホックな REST リクエストや OAuth デバイスログイン向けの軽量フォームを提供します。

3 つのウィジェットはすべて **Try-It プロキシ** (`docs/portal/scripts/tryit-proxy.mjs`) にリクエストを送信します。プロキシは `static/openapi/torii.json` が `static/openapi/manifest.json` の署名済みダイジェストと一致することを検証し、レートリミッタを適用し、ログ内の `X-TryIt-Auth` ヘッダーをマスクし、各 upstream 呼び出しに `X-TryIt-Client` を付与して Torii オペレーターがトラフィックの発生元を監査できるようにします。

## プロキシを起動する

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET` は検証したい Torii のベース URL です。
- `TRYIT_PROXY_ALLOWED_ORIGINS` には、コンソールを埋め込むべきポータルの全オリジン（ローカル開発サーバー、プロダクションのホスト名、プレビュー URL）を含める必要があります。
- `TRYIT_PROXY_PUBLIC_URL` は `docusaurus.config.js` によって参照され、`customFields.tryIt` を通じてウィジェットへ注入されます。
- `TRYIT_PROXY_BEARER` は `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` のときだけ読み込まれます。そうでない場合はユーザーがコンソールまたは OAuth デバイスフローで自分のトークンを提供する必要があります。
- `TRYIT_PROXY_CLIENT_ID` は各リクエストに付与される `X-TryIt-Client` タグを設定します。
  ブラウザから `X-TryIt-Client` を送ることは許可されていますが、値はトリムされ
  制御文字を含む場合は拒否されます。

起動時にプロキシは `verifySpecDigest` を実行し、マニフェストが古い場合は対処ヒントとともに終了します。最新の Torii 仕様を取得するには `npm run sync-openapi -- --latest` を実行するか、緊急時は `TRYIT_PROXY_ALLOW_STALE_SPEC=1` を渡します。

環境ファイルを手で編集せずにプロキシのターゲットを更新またはロールバックするには、ヘルパーを使います:

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## ウィジェットを接続する

プロキシが待ち受けたらポータルを起動します:

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` では次の設定を公開しています:

| 変数 | 目的 |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | Swagger、RapiDoc、Try it sandbox に注入される URL。未設定にすると未承認のプレビューでウィジェットを非表示にします。 |
| `TRYIT_PROXY_DEFAULT_BEARER` | メモリに保持されるオプションのデフォルトトークン。`DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` と HTTPS のみの CSP ガード (DOCS-1b) が必要です。ローカルでは `DOCS_SECURITY_ALLOW_INSECURE=1` を渡せます。 |
| `DOCS_OAUTH_*` | OAuth デバイスフロー (`OAuthDeviceLogin` component) を有効にし、レビュアーがポータルを離れずに短命トークンを発行できるようにします。 |

OAuth 変数が設定されている場合、sandbox は **Sign in with device code** ボタンを表示し、設定済みの Auth サーバーを経由します（正確な形は `config/security-helpers.js` を参照）。デバイスフローで発行されたトークンはブラウザのセッションにのみキャッシュされます。

## Norito-RPC ペイロードを送信する

1. CLI または [Norito クイックスタート](./quickstart.md) で紹介しているスニペットで `.norito` ペイロードを作成します。プロキシは `application/x-norito` の本文をそのまま転送するため、`curl` で送る場合と同じ成果物を再利用できます。
2. `/reference/torii-rapidoc`（バイナリペイロード向け）または `/reference/torii-swagger` を開きます。
3. ドロップダウンから目的の Torii スナップショットを選択します。スナップショットは署名済みで、パネルには `static/openapi/manifest.json` に記録されたマニフェストダイジェストが表示されます。
4. "Try it" ドロワーで `application/x-norito` のコンテンツタイプを選択し、**Choose File** をクリックしてペイロードを選びます。プロキシはリクエストを `/proxy/v1/pipeline/submit` に書き換え、`X-TryIt-Client=docs-portal-rapidoc` でタグ付けします。
5. Norito のレスポンスをダウンロードするには `Accept: application/x-norito` を設定します。Swagger/RapiDoc は同じドロワーにヘッダーセレクタを表示し、バイナリをプロキシ経由でストリームします。

JSON のみのルートでは、組み込みの Try it sandbox の方が速い場合があります。パス（例: `/v1/accounts/ih58@wonderland/assets`）を入力し、HTTP メソッドを選択し、必要なら JSON ボディを貼り付け、**Send request** を押してヘッダー、所要時間、ペイロードをその場で確認します。

## トラブルシューティング

| 症状 | 想定される原因 | 対処 |
| --- | --- | --- |
| ブラウザのコンソールに CORS エラーが出る、または sandbox がプロキシ URL 不在を警告する。 | プロキシが起動していない、またはオリジンが許可されていない。 | プロキシを起動し、`TRYIT_PROXY_ALLOWED_ORIGINS` にポータルのホストが含まれていることを確認してから `npm run start` を再実行します。 |
| `npm run tryit-proxy` が “digest mismatch” で終了する。 | Torii の OpenAPI バンドルが上流で変更された。 | `npm run sync-openapi -- --latest`（または `--version=<tag>`）を実行して再試行します。 |
| ウィジェットが `401` または `403` を返す。 | トークンが未指定、期限切れ、または権限不足。 | OAuth デバイスフローを使うか、sandbox に有効な bearer トークンを貼り付けます。静的トークンを使う場合は `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` をエクスポートしてください。 |
| プロキシから `429 Too Many Requests` が返る。 | IP ごとのレート制限を超過した。 | 信頼できる環境では `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` を引き上げるか、テストスクリプトを抑制します。レート制限の拒否はすべて `tryit_proxy_rate_limited_total` を増加させます。 |

## 可観測性

- `npm run probe:tryit-proxy`（`scripts/tryit-proxy-probe.mjs` のラッパー）は `/healthz` を呼び出し、必要に応じてサンプルルートを叩き、`probe_success` / `probe_duration_seconds` 用の Prometheus テキストファイルを生成します。node_exporter に統合するには `TRYIT_PROXY_PROBE_METRICS_FILE` を設定します。
- `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` を設定すると、カウンタ (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) とレイテンシヒストグラムが公開されます。`dashboards/grafana/docs_portal.json` ダッシュボードはこれらのメトリクスを読み取り、DOCS-SORA の SLO を適用します。
- 実行時ログは stdout に出力されます。各エントリにはリクエスト ID、upstream のステータス、認証ソース (`default`、`override`、`client`)、所要時間が含まれ、秘密情報は出力前にマスクされます。

`application/x-norito` のペイロードが Torii に改変なしで到達することを検証する必要がある場合は、Jest スイート (`npm test -- tryit-proxy`) を実行するか、`docs/portal/scripts/__tests__/tryit-proxy.test.mjs` の fixtures を確認してください。回帰テストは圧縮された Norito バイナリ、署名済み OpenAPI マニフェスト、プロキシのダウングレード経路をカバーし、NRPC のロールアウトで恒久的な証跡が残るようにします。
