---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ba569c84c112c3a50e489d13dcd17029832aac39c1a6404a07977e326bbe8ffa
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Try It サンドボックス

開発者ポータルには、ドキュメントを離れずに Torii のエンドポイントを呼び出せる "Try it" コンソールが用意されています。コンソールは同梱のプロキシを通してリクエストを中継するため、ブラウザは CORS 制限を回避しつつレート制限と認証を維持できます。

## 前提条件

- Node.js 18.18 以降（ポータル build 要件に一致）
- Torii の staging 環境へのネットワークアクセス
- 実行予定の Torii ルートを呼び出せる bearer token

プロキシの設定はすべて環境変数で行います。重要なノブは次の通りです:

| 変数 | 目的 | デフォルト |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | プロキシが転送する Torii のベース URL | **Required** |
| `TRYIT_PROXY_LISTEN` | ローカル開発の listen アドレス（`host:port` または `[ipv6]:port`） | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | プロキシを呼び出せる origin のカンマ区切りリスト | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | 全ての upstream リクエストに `X-TryIt-Client` で付与する識別子 | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Torii に送るデフォルト bearer token | _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | `X-TryIt-Auth` でユーザーが独自トークンを渡すことを許可 | `0` |
| `TRYIT_PROXY_MAX_BODY` | リクエストボディの最大サイズ（bytes） | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | upstream のタイムアウト（ミリ秒） | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | クライアント IP あたりのレートウィンドウ内許可リクエスト数 | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | レート制限のスライディングウィンドウ（ms） | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus 形式の metrics エンドポイント用の listen アドレス（`host:port` または `[ipv6]:port`） | _empty (disabled)_ |
| `TRYIT_PROXY_METRICS_PATH` | metrics エンドポイントの HTTP パス | `/metrics` |

プロキシは `GET /healthz` も公開し、構造化 JSON エラーを返し、ログ出力から bearer token をマスクします。

ドキュメントユーザーにプロキシを公開する場合は `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` を有効にし、Swagger/RapiDoc がユーザーの bearer token を転送できるようにします。プロキシはレート制限を維持し、資格情報をマスクし、デフォルトトークンかリクエスト毎の上書きかを記録します。`TRYIT_PROXY_CLIENT_ID` を `X-TryIt-Client` として送信したいラベルに設定してください
（デフォルトは `docs-portal`）。プロキシはクライアント提供の `X-TryIt-Client` をトリムして検証し、このデフォルトにフォールバックすることで、staging ゲートウェイがブラウザのメタデータと相関せずに出所を監査できるようにします。

## ローカルでプロキシを起動する

初回セットアップ時に依存関係をインストールします:

```bash
cd docs/portal
npm install
```

プロキシを起動して Torii へ向けます:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

スクリプトはバインドされたアドレスをログし、`/proxy/*` から Torii の origin にリクエストを転送します。

ソケットをバインドする前にスクリプトは
`static/openapi/torii.json` が `static/openapi/manifest.json` に記録された digest と一致するか検証します。ファイルがずれている場合はエラーで終了し、`npm run sync-openapi -- --latest` の実行を促します。`TRYIT_PROXY_ALLOW_STALE_SPEC=1` は緊急時のみ利用してください。プロキシは警告をログし、保守ウィンドウ中に回復できるように動作を継続します。

## ポータルウィジェットを接続する

開発者ポータルをビルドまたはサーブする際に、ウィジェットが使うプロキシ URL を設定します:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

次のコンポーネントは `docusaurus.config.js` から値を読み取ります:

- **Swagger UI** - `/reference/torii-swagger` で表示。トークンがある場合に bearer スキームを pre-authorize し、`X-TryIt-Client` でリクエストにタグ付けし、`X-TryIt-Auth` を挿入し、`TRYIT_PROXY_PUBLIC_URL` が設定されている場合はプロキシ経由に書き換えます。
- **RapiDoc** - `/reference/torii-rapidoc` で表示。トークン欄を反映し、Swagger と同じ headers を再利用し、URL が設定されると自動的にプロキシをターゲットにします。
- **Try it console** - API overview ページに埋め込み。任意のリクエスト送信、headers の表示、レスポンスボディの確認ができます。

両パネルは **snapshot selector** を表示し、`docs/portal/static/openapi/versions.json` を読み取ります。`npm run sync-openapi -- --version=<label> --mirror=current --latest` でインデックスを埋めると、レビュアーは履歴 spec を切り替え、記録済み SHA-256 digest を確認し、インタラクティブウィジェットを使う前に release snapshot が署名済み manifest を持つか確認できます。

どのウィジェットでトークンを変更しても、その影響は現在のブラウザセッションのみで、プロキシはトークンを保存もログもしません。

## 短命な OAuth トークン

長期の Torii トークン配布を避けるために、Try it コンソールを OAuth サーバーに接続します。以下の環境変数がある場合、ポータルは device-code ログインウィジェットを表示し、短命な bearer token を発行してコンソールのフォームに自動注入します。

| 変数 | 目的 | デフォルト |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth Device Authorization エンドポイント (`/oauth/device/code`) | _empty (disabled)_ |
| `DOCS_OAUTH_TOKEN_URL` | `grant_type=urn:ietf:params:oauth:grant-type:device_code` を受け付けるトークンエンドポイント | _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | docs preview 用に登録された OAuth クライアント ID | _empty_ |
| `DOCS_OAUTH_SCOPE` | サインイン時に要求するスペース区切り scopes | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | トークンに紐付ける任意の API audience | _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | 承認待ちの最小 poll 間隔（ms） | `5000`（< 5000 ms は拒否） |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | device-code のフォールバック有効期限（秒） | `600`（300 s から 900 s の範囲） |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | access token のフォールバック有効期限（秒） | `900`（300 s から 900 s の範囲） |
| `DOCS_OAUTH_ALLOW_INSECURE` | OAuth を意図的にスキップするローカル preview で `1` | _unset_ |

Example configuration:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

`npm run start` または `npm run build` を実行すると、ポータルはこれらの値を `docusaurus.config.js` に埋め込みます。ローカル preview では Try it カードに "Sign in with device code" ボタンが表示されます。ユーザーが OAuth 検証ページで表示されたコードを入力し、device flow が成功するとウィジェットは次を行います:

- 発行された bearer token を Try it コンソールのフィールドに注入する
- 既存の `X-TryIt-Client` と `X-TryIt-Auth` の headers を付与する
- 残りの有効期間を表示する
- 期限切れでトークンを自動クリアする

手動の Bearer 入力は残ります。OAuth 変数を省略するとレビュアーに一時トークンの貼り付けを求めることができます。匿名アクセスが許容される隔離されたローカル preview では `DOCS_OAUTH_ALLOW_INSECURE=1` を設定してください。OAuth 未設定のビルドは DOCS-1b のロードマップゲートを満たすために即時失敗します。

Note: ポータルをラボの外に公開する前に [Security hardening & pen-test checklist](./security-hardening.md) を確認してください。脅威モデル、CSP/Trusted Types プロファイル、DOCS-1b をゲートするペンテスト手順が記載されています。

## Norito-RPC サンプル

Norito-RPC リクエストは JSON ルートと同じ proxy/OAuth plumbing を共有します。`Content-Type: application/x-norito` を設定し、NRPC 仕様に記載された Norito の事前エンコード payload を送るだけです（`docs/source/torii/nrpc_spec.md`）。リポジトリには `fixtures/norito_rpc/` 配下に正規の payload があり、ポータル作者、SDK オーナー、レビュアーが CI と同じ bytes を再現できます。

### Try It コンソールから Norito payload を送信

1. `fixtures/norito_rpc/transfer_asset.norito` などの fixture を選びます。これらのファイルは生の Norito エンベロープです。**base64 にしないでください**。
2. Swagger または RapiDoc で NRPC エンドポイント（例: `POST /v2/pipeline/submit`）を選び、**Content-Type** セレクタを `application/x-norito` に切り替えます。
3. リクエストボディエディタを **binary**（Swagger の "File" モード、または RapiDoc の "Binary/File" セレクタ）に切り替え、`.norito` ファイルをアップロードします。ウィジェットは bytes をプロキシ経由で変更せず送信します。
4. リクエストを送信します。Torii が `X-Iroha-Error-Code: schema_mismatch` を返す場合、バイナリ payload を受け付けるエンドポイントか確認し、`fixtures/norito_rpc/schema_hashes.json` に記録された schema hash が対象 Torii build と一致するか確認してください。

コンソールは直近のファイルをメモリに保持するので、異なる認可トークンや Torii ホストを試しながら同じ payload を再送できます。`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` をワークフローに追加すると、NRPC-4 採用計画で参照される証跡バンドル（log + JSON サマリー）が生成され、レビュー時に Try It のレスポンスをスクリーンショットするのと相性が良いです。

### CLI 例 (curl)

同じ fixtures は `curl` でポータル外からも再生できます。プロキシ検証やゲートウェイ応答のデバッグに有用です:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

`transaction_fixtures.manifest.json` にある任意の fixture に差し替えるか、`cargo xtask norito-rpc-fixtures` で独自 payload をエンコードしてください。Torii が canary モードのときは、`curl` を try-it proxy（`https://docs.sora.example/proxy/v2/pipeline/submit`）に向けて、ポータルウィジェットと同じインフラをテストできます。

## Observability と運用

各リクエストは method、path、origin、upstream status、認証ソース（`override`、`default`、`client`）を含めて 1 回だけログされます。トークンは保存されず、bearer headers と `X-TryIt-Auth` はログ前にマスクされるため、stdout を中央コレクタに送っても漏えいを心配する必要はありません。

### Health probe と alerting

デプロイ時またはスケジュールで同梱の probe を実行します:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

環境ノブ:

- `TRYIT_PROXY_SAMPLE_PATH` - `/proxy` を含まない Torii の任意ルート。
- `TRYIT_PROXY_SAMPLE_METHOD` - デフォルトは `GET`。書き込みルートは `POST` に変更。
- `TRYIT_PROXY_PROBE_TOKEN` - サンプル呼び出しに一時的な bearer token を注入。
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - デフォルト 5 s のタイムアウトを上書き。
- `TRYIT_PROXY_PROBE_METRICS_FILE` - `probe_success`/`probe_duration_seconds` 用の Prometheus テキストファイル出力先。
- `TRYIT_PROXY_PROBE_LABELS` - `key=value` のカンマ区切りで metrics に追加（デフォルトは `job=tryit-proxy` と `instance=<proxy URL>`）。
- `TRYIT_PROXY_PROBE_METRICS_URL` - metrics エンドポイントの URL（例: `http://localhost:9798/metrics`）。`TRYIT_PROXY_METRICS_LISTEN` 有効時に成功応答が必要。

textfile collector に流し込むには、probe を書き込み可能なパス（例: `/var/lib/node_exporter/textfile_collector/tryit.prom`）に向け、任意のラベルを追加します:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

スクリプトは metrics ファイルをアトミックに書き換えるため、collector は常に完全な payload を読み取ります。

`TRYIT_PROXY_METRICS_LISTEN` が設定されている場合は、`TRYIT_PROXY_PROBE_METRICS_URL` を metrics エンドポイントに設定して、scrape 面が消えた場合（誤った ingress や firewall ルール不足など）に probe が即座に失敗するようにします。production の典型設定は
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"` です。

軽量なアラートには probe を監視スタックに接続します。2 回連続で失敗したらページングする Prometheus 例:

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### Metrics エンドポイントとダッシュボード

`TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798`（または任意の host/port）を設定してからプロキシを起動すると、Prometheus 形式の metrics エンドポイントが公開されます。パスは既定で `/metrics` ですが、`TRYIT_PROXY_METRICS_PATH=/custom` で変更できます。各 scrape は method 別の合計リクエスト数、レート制限拒否、upstream エラー/タイムアウト、プロキシ結果、レイテンシのサマリーを返します:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Prometheus/OTLP collector を metrics エンドポイントに向け、`dashboards/grafana/docs_portal.json` の既存パネルを再利用することで、SRE はログを解析せずに tail latency や拒否スパイクを監視できます。プロキシは `tryit_proxy_start_timestamp_ms` を自動公開し、再起動検知に使えます。

### ロールバック自動化

管理ヘルパーを使って Torii のターゲット URL を更新または復元できます。スクリプトは前の設定を `.env.tryit-proxy.bak` に保存するため、ロールバックは 1 コマンドで済みます。

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

`--env` または `TRYIT_PROXY_ENV` で env ファイルのパスを上書きできます（別の場所に設定を保存している場合）。
