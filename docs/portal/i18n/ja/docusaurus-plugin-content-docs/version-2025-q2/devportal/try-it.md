---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# サンドボックスを試してみる

開発者ポータルにはオプションの「Try it」コンソールが同梱されているため、Torii を呼び出すことができます。
ドキュメントを離れることなくエンドポイントを確認できます。コンソールはリクエストを中継します
バンドルされたプロキシを経由するため、ブラウザは CORS 制限を回避できます。
レート制限と認証を強制します。

## 前提条件

- Node.js 18.18 以降 (ポータルのビルド要件に一致)
- Torii ステージング環境へのネットワーク アクセス
- 実行する予定の Torii ルートを呼び出すことができるベアラー トークン

すべてのプロキシ構成は環境変数を通じて行われます。下の表
最も重要なノブをリストします。

|変数 |目的 |デフォルト |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` |ベース Torii プロキシがリクエストを転送する URL | **必須** |
| `TRYIT_PROXY_LISTEN` |ローカル開発用のリッスン アドレス (形式 `host:port` または `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` |プロキシを呼び出すことができるオリジンのカンマ区切りのリスト | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` |すべてのアップストリーム要求に対して `X-TryIt-Client` に配置される識別子 | `docs-portal` |
| `TRYIT_PROXY_BEARER` |デフォルトのベアラー トークンが Torii に転送されました | _空_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` |エンド ユーザーが `X-TryIt-Auth` 経由で独自のトークンを提供できるようにする | `0` |
| `TRYIT_PROXY_MAX_BODY` |リクエスト本文の最大サイズ (バイト) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` |アップストリームのタイムアウト (ミリ秒) | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` |クライアント IP ごとのレート ウィンドウごとに許可されるリクエスト | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` |レート制限のスライディング ウィンドウ (ミリ秒) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus スタイルのメトリクス エンドポイントのオプションのリッスン アドレス (`host:port` または `[ipv6]:port`) | _空 (無効)_ |
| `TRYIT_PROXY_METRICS_PATH` |メトリクス エンドポイントによって提供される HTTP パス | `/metrics` |

プロキシはまた、`GET /healthz` を公開し、構造化された JSON エラーを返します。
ログ出力からベアラー トークンを編集します。

プロキシをドキュメント ユーザーに公開するときに `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` を有効にすると、Swagger と
RapiDoc パネルは、ユーザーが指定したベアラー トークンを転送できます。プロキシは引き続きレート制限を適用します。
資格情報を編集し、リクエストでデフォルトのトークンが使用されたか、リクエストごとのオーバーライドが使用されたかを記録します。
`TRYIT_PROXY_CLIENT_ID` を、`X-TryIt-Client` として送信するラベルに設定します。
(デフォルトは `docs-portal`)。プロキシは、呼び出し元が指定した値をトリミングして検証します。
`X-TryIt-Client` 値。このデフォルトにフォールバックするため、ステージング ゲートウェイは
ブラウザーのメタデータを関連付けることなく来歴を監査します。

## プロキシをローカルで開始する

ポータルを初めてセットアップするときに依存関係をインストールします。

```bash
cd docs/portal
npm install
```

プロキシを実行し、Torii インスタンスをポイントします。

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

スクリプトはバインドされたアドレスをログに記録し、`/proxy/*` からのリクエストを
設定された Torii 原点。

ソケットをバインドする前に、スクリプトは次のことを検証します。
`static/openapi/torii.json` は、に記録されたダイジェストと一致します。
`static/openapi/manifest.json`。ファイルがドリフトしている場合、コマンドは次のメッセージを表示して終了します。
エラーが発生し、`npm run sync-openapi -- --latest` を実行するように指示されます。エクスポート
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` は緊急オーバーライドの場合のみ。代理人は
警告をログに記録して続行すると、メンテナンス期間中に回復できるようになります。

## ポータル ウィジェットを接続する

開発者ポータルを構築または提供するときに、ウィジェットがアクセスする URL を設定します。
プロキシに使用する必要があります:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

次のコンポーネントは、`docusaurus.config.js` からこれらの値を読み取ります。

- **Swagger UI** — `/reference/torii-swagger` でレンダリングされます。を事前に承認します
  トークンが存在する場合のベアラー スキームは、リクエストに `X-TryIt-Client` をタグ付けします。
  `X-TryIt-Auth` を挿入し、次の場合にプロキシ経由で呼び出しを書き換えます。
  `TRYIT_PROXY_PUBLIC_URL`が設定されます。
- **RapiDoc** — `/reference/torii-rapidoc` でレンダリングされます。トークンフィールドを反映し、
  Swagger パネルと同じヘッダーを再利用し、プロキシをターゲットにします
  URL が設定されると自動的に実行されます。
- **Try it コンソール** — API 概要ページに埋め込まれています。カスタムを送信できます
  リクエスト、ヘッダーの表示、応答本文の検査を行います。

どちらのパネルにも **スナップショット セレクター** が表示されます。
`docs/portal/static/openapi/versions.json`。そのインデックスに次のものを入力します
`npm run sync-openapi -- --version=<label> --mirror=current --latest` だから
レビュアーは過去の仕様間を移動したり、記録された SHA-256 ダイジェストを確認したりできます。
使用する前に、リリース スナップショットに署名されたマニフェストが含まれているかどうかを確認します。
インタラクティブなウィジェット。

ウィジェット内のトークンを変更すると、現在のブラウザ セッションにのみ影響します。の
プロキシは、提供されたトークンを永続化したりログに記録したりすることはありません。

## 有効期間の短い OAuth トークン有効期間の長い Torii トークンをレビュー担当者に配布しないようにするには、「Try it」を配線します。
コンソールから OAuth サーバーに接続します。以下の環境変数が存在する場合
ポータルはデバイス コード ログイン ウィジェットをレンダリングし、有効期間の短いベアラー トークンを作成します。
そしてそれらをコンソール フォームに自動的に挿入します。

|変数 |目的 |デフォルト |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth デバイス認証エンドポイント (`/oauth/device/code`) | _空 (無効)_ |
| `DOCS_OAUTH_TOKEN_URL` | `grant_type=urn:ietf:params:oauth:grant-type:device_code` | を受け入れるトークン エンドポイント_空_ |
| `DOCS_OAUTH_CLIENT_ID` |ドキュメント プレビュー用に登録された OAuth クライアント ID | _空_ |
| `DOCS_OAUTH_SCOPE` |サインイン中に要求されるスペース区切りのスコープ | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` |トークンをバインドするオプションの API オーディエンス | _空_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` |承認を待機するときの最小ポーリング間隔 (ミリ秒) | `5000` (5000ms 未満の値は拒否されます) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` |フォールバック デバイス コードの有効期限ウィンドウ (秒) | `600` (300 秒から 900 秒の間でなければなりません) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` |フォールバック アクセス トークンの有効期間 (秒) | `900` (300 秒から 900 秒の間でなければなりません) |
| `DOCS_OAUTH_ALLOW_INSECURE` | OAuth の適用を意図的にスキップするローカル プレビューの場合は、`1` に設定します。 _設定解除_ |

構成例:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

`npm run start` または `npm run build` を実行すると、ポータルにこれらの値が埋め込まれます。
`docusaurus.config.js`で。ローカル プレビュー中に、「試してみる」カードに
「デバイスコードでサインイン」ボタン。ユーザーは表示されたコードを OAuth に入力します
確認ページ。デバイス フローがウィジェットに成功すると、次のようになります。

- 発行されたベアラー トークンを Try it コンソール フィールドに挿入します。
- 既存の `X-TryIt-Client` および `X-TryIt-Auth` ヘッダーを使用してリクエストをタグ付けします。
- 残りの寿命を表示します。
- トークンの有効期限が切れると自動的にトークンをクリアします。

手動ベアラー入力は引き続き利用可能です。ベアラー入力を行う場合は常に OAuth 変数を省略してください。
レビュー担当者に自分で一時トークンを貼り付けるか、エクスポートするよう強制したい
匿名アクセスが行われる分離されたローカル プレビューの場合は `DOCS_OAUTH_ALLOW_INSECURE=1`
許容されます。 OAuth が構成されていないビルドは、要件を満たすためにすぐに失敗するようになりました。
DOCS-1b ロードマップ ゲート。

📌 [セキュリティ強化と侵入テストのチェックリスト](./security-hardening.md) を確認してください。
ポータルを研究室の外に公開する前に。脅威モデルを文書化します。
CSP/信頼できるタイプのプロファイル、および DOCS-1b をゲートする侵入テストのステップ。

## Norito-RPC サンプル

Norito-RPC リクエストは、JSON ルートと同じプロキシおよび OAuth プラミングを共有します。
彼らは単に `Content-Type: application/x-norito` を設定し、
NRPC仕様に記載されている、事前にエンコードされたNoritoペイロード
(`docs/source/torii/nrpc_spec.md`)。
リポジトリは `fixtures/norito_rpc/` で正規ペイロードを出荷するため、ポータル
作成者、SDK 所有者、およびレビュー担当者は、CI が使用する正確なバイトを再生できます。

### Try It コンソールから Norito ペイロードを送信します

1. `fixtures/norito_rpc/transfer_asset.norito` などのフィクスチャを選択します。これら
   ファイルは生の Norito エンベロープです。これらを Base64 エンコードしないでください**。
2. Swagger または RapiDoc で、NRPC エンドポイントを見つけます (たとえば、
   `POST /v2/pipeline/submit`) を選択し、**Content-Type** セレクターを次のように切り替えます。
   `application/x-norito`。
3. リクエスト本文エディターを **バイナリ** (Swagger の「ファイル」モードまたは
   RapiDoc の「バイナリ/ファイル」セレクター) を使用して、`.norito` ファイルをアップロードします。ウィジェット
   バイトを変更せずにプロキシ経由でストリーミングします。
4. リクエストを送信します。 Torii が `X-Iroha-Error-Code: schema_mismatch` を返す場合、
   バイナリ ペイロードを受け入れるエンドポイントを呼び出していることを確認し、
   `fixtures/norito_rpc/schema_hashes.json` にスキーマ ハッシュが記録されていることを確認します。
   ヒットしている Torii ビルドと一致します。

コンソールは最新のファイルをメモリに保持するため、同じファイルを再送信できます。
異なる認証トークンまたは Torii ホストを実行する際のペイロード。追加
ワークフローに `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` が生成される
NRPC-4 導入計画で参照されている証拠バンドル (ログ + JSON 概要)、
これは、レビュー中に Try It 応答のスクリーンショットを撮ることとうまく組み合わせられます。

### CLI の例 (カール)

同じフィクスチャを `curl` 経由でポータルの外で再生でき、これは便利です
プロキシを検証するとき、またはゲートウェイ応答をデバッグするとき:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl \
  -H "Content-Type: application/x-norito" \
  -H "Authorization: ${TOKEN}" \
  --data-binary @fixtures/norito_rpc/transfer_asset.norito \
  "${TORII}/v2/pipeline/submit"
```

`transaction_fixtures.manifest.json` にリストされているエントリのフィクスチャを交換します。
または、独自のペイロードを `cargo xtask norito-rpc-fixtures` でエンコードします。 Toriiのとき
カナリア モードでは、try-it プロキシで `curl` を指定できます。
(`https://docs.sora.example/proxy/v2/pipeline/submit`) 同じことを実行します
ポータル ウィジェットが使用するインフラストラクチャ。

## 可観測性と操作すべてのリクエストは、メソッド、パス、オリジン、アップストリーム ステータス、および
認証ソース (`override`、`default`、または `client`)。トークンは決して
保存されます - ベアラー ヘッダーと `X-TryIt-Auth` 値の両方が編集される前に編集されます。
ロギング - これにより、心配することなく stdout を中央コレクタに転送できます。
秘密が漏れる。

### ヘルスプローブとアラート

バンドルされたプローブを展開中またはスケジュールに従って実行します。

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

環境ノブ:

- `TRYIT_PROXY_SAMPLE_PATH` — 実行するオプションの Torii ルート (`/proxy` なし)。
- `TRYIT_PROXY_SAMPLE_METHOD` — デフォルトは `GET` です。書き込みルートの場合は `POST` に設定します。
- `TRYIT_PROXY_PROBE_TOKEN` — サンプル呼び出し用の一時ベアラー トークンを挿入します。
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — デフォルトの 5 秒のタイムアウトをオーバーライドします。
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds` のオプションの Prometheus テキスト ファイルの宛先。
- `TRYIT_PROXY_PROBE_LABELS` — メトリックに追加されるコンマ区切りの `key=value` ペア (デフォルトは `job=tryit-proxy` および `instance=<proxy URL>`)。
- `TRYIT_PROXY_PROBE_METRICS_URL` — `TRYIT_PROXY_METRICS_LISTEN` が有効な場合に正常に応答する必要があるオプションのメトリック エンドポイント URL (`http://localhost:9798/metrics` など)。

書き込み可能なファイルにプローブを向けることにより、結果をテキストファイル コレクターにフィードします。
パス (例: `/var/lib/node_exporter/textfile_collector/tryit.prom`) および
カスタム ラベルを追加します。

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

スクリプトはメトリクス ファイルをアトミックに書き換えるため、コレクタは常にメトリクス ファイルを読み取ります。
完全なペイロード。

`TRYIT_PROXY_METRICS_LISTEN` が設定されている場合、設定します
`TRYIT_PROXY_PROBE_METRICS_URL` をメトリクス エンドポイントに送信するため、プローブはすぐに失敗します
スクレープ サーフェスが消えた場合 (たとえば、入力の構成が間違っていたり、欠落している場合)
ファイアウォールのルール)。一般的な運用設定は次のとおりです。
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`。

軽量のアラートを作成するには、プローブを監視スタックに接続します。 Prometheus
2 回連続して失敗した後のページの例:

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

### メトリクスエンドポイントとダッシュボード

前に `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (または任意のホスト/ポートのペア) を設定します。
プロキシを開始して、Prometheus 形式のメトリクス エンドポイントを公開します。パス
デフォルトは `/metrics` ですが、次のようにオーバーライドできます。
`TRYIT_PROXY_METRICS_PATH=/custom`。各スクレイピングはメソッドごとのカウンターを返します
リクエストの合計、レート制限の拒否、アップストリームのエラー/タイムアウト、プロキシの結果、
レイテンシの概要:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Prometheus/OTLP コレクターをメトリック エンドポイントにポイントし、
既存の `dashboards/grafana/docs_portal.json` パネルにより、SRE がテールを観察できるようになります
ログを解析しないと、レイテンシと拒否のスパイクが発生します。プロキシは自動的に
オペレータが再起動を検出できるように、`tryit_proxy_start_timestamp_ms` を公開しています。

### ロールバックの自動化

管理ヘルパーを使用して、ターゲット Torii URL を更新または復元します。スクリプト
以前の設定は `.env.tryit-proxy.bak` に保存されるため、ロールバックは
単一のコマンド。

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

デプロイメントの場合は、env ファイル パスを `--env` または `TRYIT_PROXY_ENV` でオーバーライドします。
設定を別の場所に保存します。