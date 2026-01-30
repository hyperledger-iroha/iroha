---
lang: ja
direction: ltr
source: docs/portal/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d9fe9e2c2763fd83237dc2343fe9b0c6682d662677c6217afa00552444f44283
source_last_modified: "2025-12-27T09:12:05.231822+00:00"
translation_last_reviewed: 2026-01-30
---

# SORA Nexus デベロッパーポータル

このディレクトリには、インタラクティブな開発者ポータルのための Docusaurus ワークスペースがあります。ポータルは Norito ガイド、SDK クイックスタート、`cargo xtask openapi` で生成される OpenAPI リファレンスを集約し、ドキュメントプログラムで使用する SORA Nexus のブランディングでまとめています。

## 前提条件

- Node.js 18.18 以降（Docusaurus v3 の基準）。
- パッケージ管理に Yarn 1.x または npm 9 以上。
- Rust ツールチェーン（OpenAPI 同期スクリプトで使用）。

## ブートストラップ

```bash
cd docs/portal
npm install    # or yarn install
```

## 利用可能なスクリプト

| コマンド | 説明 |
|---------|-------------|
| `npm run start` / `yarn start` | ホットリロード付きのローカル開発サーバーを起動（デフォルトは `http://localhost:3000`）。 |
| `npm run build` / `yarn build` | `build/` に本番ビルドを生成。 |
| `npm run serve` / `yarn serve` | 最新ビルドをローカルで配信（スモークテストに便利）。 |
| `npm run docs:version -- <label>` | 現在のドキュメントを `versioned_docs/version-<label>` にスナップショット（`docusaurus docs:version` のラッパー）。 |
| `npm run sync-openapi` / `yarn sync-openapi` | `cargo xtask openapi` で `static/openapi/torii.json` を再生成（`--mirror=<label>` を渡すと追加のバージョンスナップショットにもコピー）。 |
| `npm run tryit-proxy` | “Try it” コンソールを支えるステージング用プロキシを起動（設定は下記）。 |
| `npm run probe:tryit-proxy` | プロキシに対して `/healthz` + サンプルリクエストのプローブを実行（CI/監視用ヘルパー）。 |
| `npm run manage:tryit-proxy -- <update|rollback>` | バックアップ付きでプロキシの `.env` ターゲットを更新または復元。 |
| `npm run sync-i18n` | 日本語・ヘブライ語・スペイン語・ポルトガル語・フランス語・ロシア語・アラビア語・ウルドゥー語の翻訳スタブを `i18n/` 配下に作成。 |
| `npm run sync-norito-snippets` | 厳選した Kotodama 例のドキュメント＋ダウンロード用スニペットを再生成（開発サーバープラグインから自動実行されます）。 |
| `npm run test:tryit-proxy` | Node のテストランナー（`node --test`）でプロキシのユニットテストを実行。 |

OpenAPI 同期スクリプトは、リポジトリルートから `cargo xtask openapi` を実行できる必要があります。これは `static/openapi/` に決定的な JSON を出力し、Torii ルーターがライブ仕様を公開していることを前提とします（緊急時のプレースホルダー出力に限り `cargo xtask openapi --allow-stub` を使用）。

## ドキュメントのバージョニング & OpenAPI スナップショット

- **ドキュメントのバージョンを切る:** `npm run docs:version -- 2025-q3`（任意のタグ）を実行します。生成された `versioned_docs/version-<label>`、`versioned_sidebars`、`versions.json` をコミットしてください。ナビバーのバージョンドロップダウンに新しいスナップショットが自動で表示されます。
- **OpenAPI アーティファクトの同期:** バージョン作成後、`cargo xtask openapi --sign <path-to-ed25519-key>` でカノニカル仕様とマニフェストを更新し、`npm run sync-openapi -- --version=2025-q3 --mirror=current --latest` で一致するスナップショットを作成します。スクリプトは `static/openapi/versions/2025-q3/torii.json` を書き出し、仕様を `versions/current/torii.json` にミラーし、`versions.json` を更新し、`/openapi/torii.json` を更新し、署名済みの `manifest.json` を各バージョンディレクトリに複製して履歴仕様にも同じ来歴メタデータが残るようにします。`--mirror=<label>` を複数指定すれば、生成した仕様を他の履歴スナップショットにもコピーできます。
- **CI の期待:** ドキュメントに触れるコミットは、（該当する場合）バージョンの追加と OpenAPI スナップショットの更新を含め、Swagger / RapiDoc / Redoc パネルが履歴仕様を問題なく切り替えられるようにします。
- **マニフェストの強制:** `sync-openapi` スクリプトは、署名済み `manifest.json` が欠落・不正・仕様と不一致の場合に失敗し、未署名スナップショットが既定で公開されないようにしています。`cargo xtask openapi --sign <key>` を再実行してカノニカルマニフェストを更新し、再度同期してください。`--allow-unsigned` はローカルプレビュー専用です（CI は `ci/check_openapi_spec.sh` で仕様を再生成し、マニフェストを検証します）。

## 構成

```
docs/portal/
├── docs/                 # ポータル用 Markdown/MDX コンテンツ
├── i18n/                 # sync-i18n が生成するロケール上書き（ja/he）
├── src/                  # React ページ/コンポーネント（プレースホルダー）
├── static/               # 静的アセット（OpenAPI JSON を含む）
├── scripts/              # ヘルパースクリプト（OpenAPI 同期）
├── docusaurus.config.js  # サイト設定
└── sidebars.js           # サイドバー / ナビゲーション
```

### Try it プロキシの設定

“Try it” サンドボックスは `scripts/tryit-proxy.mjs` を介してリクエストを中継します。起動前に環境変数でプロキシを設定してください:

```bash
export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_BEARER="sora-dev-token"          # optional
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run tryit-proxy
```

- `TRYIT_PROXY_LISTEN`（既定 `127.0.0.1:8787`）はバインド先アドレスを制御します。
- `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` はインメモリのレートリミッタ（既定は 60 リクエスト / 60 秒）を設定します。
- `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` を設定すると、既定の Bearer トークンではなく呼び出し元の `Authorization` ヘッダーを転送します。
- `static/openapi/torii.json` が `static/openapi/manifest.json` の署名済みマニフェストと一致しない場合、プロキシは起動を拒否します。`npm run sync-openapi -- --latest` を実行して仕様を更新してください。緊急時のみ `TRYIT_PROXY_ALLOW_STALE_SPEC=1` を設定します（警告を出して起動します）。
- `npm run probe:tryit-proxy` でステージングプロキシのスモークテストを実行し、監視ジョブに組み込みます。`npm run manage:tryit-proxy -- update` で Torii エンドポイントのローテーションを簡素化し、`.env` をバックアップします。
- `TRYIT_PROXY_PROBE_METRICS_FILE` と `TRYIT_PROXY_PROBE_LABELS` を使うと、`probe_success`/`probe_duration_seconds` を Prometheus の textfile 形式で出力でき、node_exporter などに接続できます。
- Prometheus のスクレイプでは `tryit_proxy_request_duration_ms_bucket`/`_count`/`_sum` のヒストグラムが公開されます。ポータルの Grafana ダッシュボードは p95/p99 レイテンシ SLO の追跡に利用します（`dashboards/grafana/docs_portal.json`）。

### OAuth デバイスコードログイン

ポータルを信頼ネットワーク外で公開する場合は OAuth のデバイス認可を設定し、レビューアーが長期 Torii トークンに触れないようにします。`npm run start` または `npm run build` の前に次の変数をエクスポートしてください:

| 変数 | メモ |
|----------|-------|
| `DOCS_OAUTH_DEVICE_CODE_URL` / `DOCS_OAUTH_TOKEN_URL` | デバイス認可フローを実装した OAuth エンドポイント。 |
| `DOCS_OAUTH_CLIENT_ID` | ドキュメントプレビュー用に登録されたクライアント ID。 |
| `DOCS_OAUTH_SCOPE` / `DOCS_OAUTH_AUDIENCE` | 発行トークンを制限するための任意の scope/audience。 |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | ポータルウィジェットが強制する最小ポーリング間隔（既定 5000ms、これ未満は拒否）。 |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` / `DOCS_OAUTH_TOKEN_TTL_SECONDS` | サーバーが `expires_in` を返さない場合のフォールバック有効期限。 |
| `DOCS_OAUTH_ALLOW_INSECURE` | ローカル開発のみ `1` を設定。必要変数が欠けていると本番ビルドは失敗します。 |

例:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
```

ビルドに値が反映されると、Try it コンソールはサインインパネルを表示し、デバイスコードを提示してトークンエンドポイントをポーリングし、Bearer フィールドを自動入力し、期限切れ時にトークンをクリアします。OAuth 設定が不完全、またはトークン/デバイス TTL がセキュリティ予算を逸脱する場合、ポータルは起動を拒否します。匿名アクセスが許容されるローカルプレビューのみ `DOCS_OAUTH_ALLOW_INSECURE=1` を使用してください。プロキシは手動の `X-TryIt-Auth` オーバーライドも受け付けるため、必要に応じて OAuth 変数を外してトークンを貼り付ける運用も可能です。

詳細は [`docs/devportal/security-hardening.md`](docs/devportal/security-hardening.md) を参照してください。

### セキュリティヘッダー

`docusaurus.config.js` は CSP、Trusted Types、Permissions-Policy、Referrer-Policy を決定的に出力し、静的ホストが開発サーバーと同じガードレールを継承できるようにしています。既定ではポータル起点のスクリプトのみ許可し、`connect-src` は設定済みの分析エンドポイントと Try it プロキシに限定されます。本番ビルドは HTTPS 以外の analytics/try-it エンドポイントを拒否します。`http://` ターゲットに対するローカルプレビューでは、`DOCS_SECURITY_ALLOW_INSECURE=1` を明示的に設定してダウングレードを認可してください。

### ローカリゼーションのワークフロー

ポータルは英語をソース言語とし、日本語・ヘブライ語・スペイン語・ポルトガル語・フランス語・ロシア語・アラビア語・ウルドゥー語のロケールを備えます。新しいドキュメントが追加されたら、以下を実行します:

```bash
npm run sync-i18n
```

このスクリプトは、リポジトリ全体の `scripts/sync_docs_i18n.py` と同様の動作で、`i18n/<lang>/docusaurus-plugin-content-docs/current/...` に翻訳スタブを生成します。編集者はプレースホルダーを置き換え、フロントマターのメタデータ（`status`、`translation_last_reviewed` など）を更新します。

既定では、本番ビルドは `docs/i18n/published_locales.json` に列挙されたロケール（現状は英語のみ）しか含めないため、スタブは配布されません。進行中のロケールをローカルでプレビューする場合は `DOCS_I18N_INCLUDE_STUBS=1` を設定します。

### コンテンツマップ（MVP ターゲット）

| セクション | ステータス | メモ |
|---------|--------|-------|
| Norito クイックスタート（`docs/norito/quickstart.md`） | 🟢 Published | Norito ペイロードを送信するための Docker + Kotodama + CLI のエンドツーエンド手順。 |
| Norito Ledger Walkthrough（`docs/norito/ledger-walkthrough.md`） | 🟢 Published | 登録 → mint → transfer を CLI で進め、トランザクション/ステータス確認と SDK パリティのリンクを含む。 |
| SDK レシピ（`docs/sdk/recipes/`） | 🟡 Rolling out | Rust/Python/JS/Swift/Java のレジャーフローが公開済み。残りの SDK も同じパターンに合わせる。 |
| API リファレンス | 🟢 Automated | `yarn sync-openapi` が `static/openapi/torii.json` を公開。DevRel はリリースごとに確認。 |
| Streaming ロードマップ | 🟢 Stubbed | `docs/norito-streaming-roadmap.md` のバックログ統合を参照。 |
| SoraFS マルチソーススケジューリング（`docs/sorafs/provider-advert-multisource.md`） | 🟢 Published | レンジ能力 TLV、ガバナンス検証、CLI フィクスチャ、テレメトリ参照を要約。 |

> 📌 2026-03-05 チェックポイント: クイックスタートと少なくとも 1 つのレシピが公開されたら、この表を削除してポータルのコントリビューターガイドへのリンクに置き換えます。

## CI & デプロイ

- `ci/check_docs_portal.sh` は最初に `node scripts/check-sdk-recipes.mjs` を実行して SDK レシピが正規ソースと一致することを検証し、その後 `npm ci|install`、`npm run build` を実行し、生成された HTML に主要セクション（ホームページ、SoraFS、Norito、publishing checklist）が存在することを確認します。
- `ci/check_openapi_spec.sh` は Torii 仕様を `cargo xtask openapi` で再生成し、`static/openapi/torii.json` と `versions/current/torii.json` と比較し、チェックサムマニフェストを検証して、追跡対象の仕様やハッシュが古い場合は PR を失敗させます。
- `.github/workflows/check-docs.yml` は全 PR でポータルビルドを実行し、コンテンツの退行を検知します。
- `.github/workflows/docs-portal-preview.yml` は PR 向けにサイトをビルドし、`build/checksums.sha256` を出力して `sha256sum -c` で検証し、成果物を `artifacts/preview-site.tar.gz` としてパッケージし、`scripts/generate-preview-descriptor.mjs` でディスクリプタを生成し、静的サイトとメタデータの両方をアップロードします。レビューアーはビルドを再実行せずに正確なスナップショットを監査できます。ワークフローはマニフェスト/アーカイブのダイジェスト（利用可能なら SoraFS バンドルも）を PR コメントに投稿し、Actions ログを開かずに検証結果を確認できます。
- `.github/workflows/docs-portal-deploy.yml` は `main`/`master` へのプッシュで GitHub Pages に静的ビルドを公開し、`github-pages` 環境 URL でプレビューを提供します。デプロイ前にランディングページのスモークチェックを実行します。
- `scripts/sorafs-package-preview.sh` はプレビューサイトを決定的な SoraFS バンドル（CAR、plan、manifest）に変換し、`docs/examples/sorafs_preview_publish.json` の JSON 設定で資格情報を渡すと、マニフェストをステージングの Torii エンドポイントへ送信できます。
- `scripts/preview_verify.sh` は、これまでレビューアーが手動で行っていたチェックサム + ディスクリプタ検証フローを実行します。展開済みの `build/` ディレクトリ（任意でディスクリプタ/アーカイブのパス）を指定して、プレビュー成果物が改ざんされていないことを確認します。
- `scripts/sorafs-pin-release.sh` と `.github/workflows/docs-portal-sorafs-pin.yml` は、本番の SoraFS パイプライン（ビルド/テスト、CAR + マニフェスト生成、Sigstore 署名、検証、任意のエイリアスバインド、ガバナンスレビュー用の成果物アップロード）を自動化します。リリースを昇格する際は `workflow_dispatch` でワークフローをトリガーします。
- `cargo xtask soradns-verify-gar --gar <path> --name <fqdn> [...]` は、Gateway Authorization Records を署名または ops に渡す前に検証します。ヘルパーは正規/整形ホスト、マニフェストメタデータ、テレメトリラベルが決定的なポリシーと一致することを確認し、`--json-out` で DG-3 証跡向けの JSON サマリを出力できます。
- 提出時にはピンヘルパーが `scripts/generate-dns-cutover-plan.mjs` を呼び出し、`artifacts/sorafs/portal.dns-cutover.json` を生成します。`DNS_CHANGE_TICKET`、`DNS_CUTOVER_WINDOW`、`DNS_HOSTNAME`、`DNS_ZONE`、`DNS_OPS_CONTACT`（または対応する `--dns-*` フラグ）を設定し、運用が DNS 切替に必要なメタデータをディスクリプタに含めます。キャッシュ無効化やロールバックが必要な場合は `DNS_CACHE_PURGE_ENDPOINT`、`DNS_CACHE_PURGE_AUTH_ENV`、`DNS_PREVIOUS_PLAN`（または `--cache-purge-*` / `--previous-dns-plan` フラグ）を追加して、ディスクリプタにパージ API 呼び出しと直前のディスクリプタを記録します。ディスクリプタは `Sora-Route-Binding`（ホスト、CID、ヘッダー/バインドパス、検証コマンド）も記載し、GAR 昇格やフォールバック計画のレビューがエッジで配信される正確なヘッダーを参照できるようにします。
- 同じワークフローで `scripts/sns_zonefile_skeleton.py` を介した SNS ゾーンファイルのスケルトン/リゾルバー断片も生成できます。IPv4/IPv6/CNAME/SPKI/TXT メタデータ（`DNS_ZONEFILE_IPV4` のような環境変数または `--dns-zonefile-ipv4` などの CLI フラグ）を渡し、`DNS_GAR_DIGEST` で GAR ダイジェストを指定すると、`artifacts/sns/zonefiles/<zone>/<hostname>.json`（およびリゾルバー断片）を自動生成し、SN-7 証跡をカットオーバーディスクリプタと並べて保存します。
- DNS ディスクリプタは `gateway_binding` セクションに上記成果物（パス、コンテンツ CID、証明ステータス、ヘッダーのテンプレート文字列）を埋め込むため、DG-3 の変更承認にゲートウェイで期待される `Sora-Name/Sora-Proof/CSP/HSTS` バンドルが含まれます。

すべての `npm run build` 実行は `postbuild` フックを起動し、`build/checksums.sha256` を生成します。ビルド後（または CI から成果物をダウンロード後）、`./docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build` を実行してマニフェストを検証します。CI のメタデータバンドルを検証する場合は `--descriptor <path>`/`--archive <path>` を渡し、スクリプトが記録されたダイジェストとファイル名を突き合わせるようにします。
- ローカルプレビューを安全に共有する場合は `npm run serve` を実行してください。このコマンドは `scripts/serve-verified-preview.mjs` をラップしており、`preview_verify.sh` を実行してから `docusaurus serve` を起動します。マニフェスト検証に失敗するか欠落している場合は起動を中止し、CI 外でもプレビューがチェックサムでゲートされるようにします。`npm run serve:verified` のエイリアスも引き続き利用できます。

### プレビュー URL & リリースノート

- パブリックベータのプレビュー: `https://docs.iroha.tech/`
- GitHub は各デプロイごとに **github-pages** 環境の URL でもビルドを公開します。
- ポータルコンテンツに触れる PR には Actions の成果物（`docs-portal-preview`、`docs-portal-preview-metadata`）が含まれ、ビルド済みサイト、チェックサムマニフェスト、圧縮アーカイブ、ディスクリプタが提供されます。レビューアーは `index.html` をローカルで開き、チェックサムを検証してからプレビューを共有できます。ワークフローは各 PR にサマリーコメント（マニフェスト/アーカイブのハッシュと SoraFS ステータス）を投稿し、検証結果のクイックシグナルを提供します。
- プレビューバンドルをダウンロードしたら、`./docs/portal/scripts/preview_verify.sh --build-dir <extracted build> --descriptor <descriptor> --archive <archive>` を実行して、成果物が CI の生成物と一致することを確認してから外部に共有します。
- リリースノートやステータス更新の準備時は、外部レビューアーがリポジトリをクローンせずに最新スナップショットを参照できるよう、プレビュー URL を記載してください。
- プレビューのウェーブ調整は `docs/portal/docs/devportal/preview-invite-flow.md` を使い、`docs/portal/docs/devportal/reviewer-onboarding.md` と組み合わせて、招待・テレメトリ出力・オフボーディングまで同じ証跡を再利用できるようにします。
