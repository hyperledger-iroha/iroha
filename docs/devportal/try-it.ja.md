---
lang: ja
direction: ltr
source: docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2025-11-15T05:30:33.582253+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/devportal/try-it.md (Try It Sandbox Guide) -->

---
title: Try It サンドボックスガイド
summary: Torii のステージングプロキシと開発者ポータルのサンドボックスを起動する方法。
---

開発者ポータルには Torii REST API 用の “Try it” コンソールが同梱されています。
本ガイドでは、サポート用プロキシを起動し、資格情報を公開せずにステージング
ゲートウェイへコンソールを接続する方法を説明します。

## 前提条件

- Iroha リポジトリのチェックアウト（workspace ルート）。
- Node.js 18.18+（ポータルのベースラインに一致）。
- ワークステーションから到達可能な Torii エンドポイント（staging またはローカル）。

## 1. OpenAPI スナップショットを生成（任意）

コンソールはポータル参照ページと同じ OpenAPI ペイロードを再利用します。
Torii ルートを変更した場合は、スナップショットを再生成してください。

```bash
cargo xtask openapi
```

タスクは `docs/portal/static/openapi/torii.json` を書き込みます。

## 2. Try It プロキシを起動

リポジトリのルートから実行します。

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### 環境変数

| 変数 | 説明 |
|------|------|
| `TRYIT_PROXY_TARGET` | Torii のベース URL（必須）。 |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | プロキシ利用を許可する origin のカンマ区切りリスト（デフォルト: `http://localhost:3000`）。 |
| `TRYIT_PROXY_BEARER` | すべてのプロキシリクエストに適用される任意の bearer トークン。 |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | クライアントの `Authorization` ヘッダーをそのまま転送するには `1` を設定。 |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | メモリ内レートリミッタ設定（デフォルト: 60 リクエスト / 60 秒）。 |
| `TRYIT_PROXY_MAX_BODY` | 受け付ける最大リクエストボディ（バイト、デフォルト 1 MiB）。 |
| `TRYIT_PROXY_TIMEOUT_MS` | Torii の upstream リクエストのタイムアウト（デフォルト 10 000 ms）。 |

プロキシは次を公開します。

- `GET /healthz` — レディネスチェック。
- `/proxy/*` — パスとクエリ文字列を保持したプロキシリクエスト。

## 3. ポータルを起動

別ターミナルで実行します。

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

`http://localhost:3000/api/overview` を開き、Try It コンソールを使用します。
同じ環境変数で Swagger UI と RapiDoc の埋め込み設定も行われます。

## 4. ユニットテストの実行

プロキシには高速な Node ベースのテストスイートがあります。

```bash
npm run test:tryit-proxy
```

テストはアドレス解析、origin 処理、レート制限、bearer 注入をカバーします。

## 5. プローブ自動化とメトリクス

同梱のプローブを使い、`/healthz` とサンプルエンドポイントを検証します。

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

環境設定:

- `TRYIT_PROXY_SAMPLE_PATH` — 任意の Torii ルート（`/proxy` を含めない）を試験。
- `TRYIT_PROXY_SAMPLE_METHOD` — 既定は `GET`。書き込みルートは `POST` を設定。
- `TRYIT_PROXY_PROBE_TOKEN` — サンプル呼び出しに一時 bearer トークンを注入。
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — デフォルト 5 秒のタイムアウトを上書き。
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds` の
  Prometheus textfile 出力先。
- `TRYIT_PROXY_PROBE_LABELS` — `key=value` のカンマ区切りラベル（デフォルト: `job=tryit-proxy` と `instance=<proxy URL>`）。

`TRYIT_PROXY_PROBE_METRICS_FILE` を設定すると、スクリプトはファイルを
アトミックに書き換えるため、node_exporter/textfile collector が常に完全な
ペイロードを取得できます。例:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

生成されたメトリクスを Prometheus に転送し、developer-portal ドキュメントの
サンプルアラートを再利用して `probe_success` が `0` になったら通知します。

## 6. 本番ハードニングチェックリスト

ローカル開発を超えてプロキシを公開する前に:

- TLS をプロキシの前段で終端（リバースプロキシまたはマネージドゲートウェイ）。
- 構造化ログを設定し、観測パイプラインへ転送。
- bearer トークンをローテーションし、シークレットマネージャに保管。
- プロキシの `/healthz` エンドポイントを監視し、レイテンシメトリクスを集約。
- レート制限を Torii staging のクォータに合わせ、クライアントへスロットリングを
  伝えるため `Retry-After` の挙動を調整。
