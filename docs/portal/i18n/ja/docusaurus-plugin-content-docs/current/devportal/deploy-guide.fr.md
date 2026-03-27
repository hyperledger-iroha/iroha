---
lang: fr
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/devportal/deploy-guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cda4202ed8ea960b6263c4f6f226d1cbda2a980acb932a6b39e122cf32629ea4
source_last_modified: "2026-01-22T15:57:55+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: deploy-guide
lang: ja
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

## 概要

このプレイブックは、ロードマップの **DOCS-7**（SoraFS 公開）と **DOCS-8**（CI/CD ピン自動化）を、開発者ポータルの実行手順に落とし込みます。build/lint フェーズ、SoraFS パッケージ化、Sigstore 署名、エイリアス昇格、検証、ロールバック訓練を含め、すべてのプレビューとリリースを再現可能かつ監査可能にします。

このフローは `sorafs_cli` バイナリ（`--features cli` でビルド済み）、ピンレジストリ権限を持つ Torii エンドポイントへのアクセス、Sigstore 用 OIDC 認証情報がある前提です。長期秘密情報（`IROHA_PRIVATE_KEY`、`SIGSTORE_ID_TOKEN`、Torii トークン）は CI のボールトに保管し、ローカル実行ではシェルの export で読み込んでください。

## 前提条件

- Node 18.18 以上（`npm` または `pnpm`）
- `cargo run -p sorafs_car --features cli --bin sorafs_cli` で取得した `sorafs_cli`
- `/v1/sorafs/*` を公開する Torii URL と、マニフェスト/エイリアスを送信できる権限アカウント/秘密鍵
- `SIGSTORE_ID_TOKEN` を発行できる OIDC 発行元（GitHub Actions、GitLab、Workload Identity など）
- 任意: dry run 用 `examples/sorafs_cli_quickstart.sh`、GitHub/GitLab ワークフローの雛形 `docs/source/sorafs_ci_templates.md`
- Try it OAuth 変数（`DOCS_OAUTH_*`）を設定し、ラボ外に昇格する前に
  [security-hardening checklist](./security-hardening.md) を実行してください。これらの変数が欠落している、または TTL/ポーリングのノブが規定範囲外の場合、ポータルのビルドは失敗します。`DOCS_OAUTH_ALLOW_INSECURE=1` は使い捨てのローカルプレビューにのみ使用し、ペンテスト証跡をリリースチケットに添付してください。

## ステップ 0 - Try it プロキシバンドルを取得

Netlify やゲートウェイにプレビューを昇格する前に、Try it プロキシのソースと署名済み OpenAPI マニフェストのダイジェストを決定論的バンドルにまとめます。

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` は proxy/probe/rollback ヘルパーをコピーし、OpenAPI 署名を検証して `release.json` と `checksums.sha256` を生成します。Netlify/SoraFS ゲートウェイ昇格チケットにこのバンドルを添付すると、レビュアーは再ビルドせずに正確なプロキシソースと Torii ターゲットを再現できます。このバンドルには `allow_client_auth` の有無も記録され、ロールアウト計画と CSP ルールの整合に役立ちます。

## ステップ 1 - ポータルをビルド/リント

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build` は `scripts/write-checksums.mjs` を自動実行し、次を生成します。

- `build/checksums.sha256` - `sha256sum -c` に使える SHA256 マニフェスト
- `build/release.json` - `tag`/`generated_at`/`source` を含むメタデータ（すべての CAR/マニフェストに固定）

これらを CAR サマリーと一緒にアーカイブし、レビュアーがプレビュー成果物を再ビルドなしで比較できるようにします。

## ステップ 2 - 静的アセットをパッケージ化

Docusaurus の出力ディレクトリに CAR パッカーを適用します。例では `artifacts/devportal/` 配下に出力します。

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
  --input build \
  --car-out "$OUT"/portal.car \
  --plan-out "$OUT"/portal.plan.json \
  --summary-out "$OUT"/portal.car.json \
  --chunker-handle sorafs.sf1@1.0.0
```

サマリー JSON にはチャンク数、ダイジェスト、proof 計画のヒントが含まれ、`manifest build` と CI ダッシュボードで再利用されます。

## ステップ 2b - OpenAPI と SBOM のコンパニオンをパッケージ化

DOCS-7 では、ポータルサイト、OpenAPI スナップショット、SBOM ペイロードをそれぞれ独立したマニフェストで公開し、ゲートウェイが各成果物に `Sora-Proof`/`Sora-Content-CID` ヘッダーを付与できるようにする必要があります。リリースヘルパー（`scripts/sorafs-pin-release.sh`）は、OpenAPI ディレクトリ（`static/openapi/`）と `syft` で生成した SBOM を `openapi.*`/`*-sbom.*` の CAR に分けてパッケージ化し、`artifacts/sorafs/portal.additional_assets.json` にメタデータを記録します。手動フローでは、各ペイロードに対してステップ 2〜4 を繰り返し、独自のプレフィックスとメタデータラベルを付けてください（例: `--car-out "$OUT"/openapi.car` と `--metadata alias_label=docs.sora.link/openapi`）。DNS を切り替える前に、サイト、OpenAPI、ポータル SBOM、OpenAPI SBOM の各マニフェスト/エイリアスを Torii に登録し、ゲートウェイがすべての成果物に対して証明書付きヘッダーを返せるようにします。

## ステップ 3 - マニフェストを構築

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

リリースウィンドウに応じて pin-policy フラグを調整します（例: canary には `--pin-storage-class hot`）。JSON 版は任意ですが、コードレビューに便利です。

## ステップ 4 - Sigstore で署名

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

バンドルにはマニフェストダイジェスト、チャンクダイジェスト、OIDC トークンの BLAKE3 ハッシュが記録され、JWT 自体は保存されません。バンドルと分離署名の両方を保管してください。プロダクション昇格時は同じ成果物を再利用でき、再署名の必要がありません。ローカル実行では `--identity-token-env` を使うか、`SIGSTORE_ID_TOKEN` を環境変数で指定します。

## ステップ 5 - ピンレジストリへ送信

署名済みマニフェスト（およびチャンクプラン）を Torii に送信します。監査性確保のため、必ずサマリーの出力を指定してください。

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority <i105-account-id> \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

プレビューやカナリア（`docs-preview.sora`）のロールアウト時は、QA が内容を確認できるように別エイリアスで再送信します。

エイリアスバインディングには `--alias-namespace`、`--alias-name`、`--alias-proof` の 3 つが必要です。Governance が承認時に proof バンドル（base64 または Norito bytes）を発行するので、CI のシークレットとして保存し、`manifest submit` の前にファイルとして用意してください。DNS を触らずにピンだけ行う場合はエイリアス関連のフラグを省略します。

## ステップ 5b - Governance 提案を生成

各マニフェストは Parliament 向け提案を同梱し、特権資格なしで市民が変更を提案できるようにします。submit/sign の後で以下を実行します。

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` には `RegisterPinManifest` 命令、チャンクダイジェスト、ポリシー、エイリアスヒントが含まれます。Governance チケットや Parliament ポータルに添付して、再ビルドなしでペイロードを比較できるようにします。このコマンドは Torii の権限キーに触れないため、誰でもローカルで提案を生成できます。

## ステップ 6 - proof とテレメトリを検証

ピンの後に、決定論的な検証を行います。

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- `torii_sorafs_gateway_refusals_total` と `torii_sorafs_replication_sla_total{outcome="missed"}` を監視します。
- `npm run probe:portal` で Try-It プロキシと記録済みリンクを、新しくピンしたコンテンツに対して試験します。
- [Publishing & Monitoring](./publishing-monitoring.md) に記載の監視エビデンスを収集し、DOCS-3c の観測性ゲートを満たします。ヘルパーは複数の `bindings`（サイト、OpenAPI、ポータル SBOM、OpenAPI SBOM）を受け付け、`hostname` ガードで対象ホストに `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` を強制します。次の実行で JSON サマリーと証跡バンドル（`portal.json`、`tryit.json`、`binding.json`、`checksums.sha256`）をリリースディレクトリに書き出します。

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## ステップ 6a - ゲートウェイ証明書計画

GAR パケット作成前に TLS SAN/challenge 計画を導出し、ゲートウェイチームと DNS 承認者が同じ証跡をレビューできるようにします。新しいヘルパーは DG-3 の入力に合わせ、ワイルドカードの正規ホスト、pretty-host SAN、DNS-01 ラベル、推奨 ACME チャレンジを列挙します。

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

JSON はリリースバンドルにコミットするか、変更チケットに添付します。これにより、Torii の `torii.sorafs_gateway.acme` に SAN 値を貼り付ける際の参照や、GAR レビュー時の正規/pretty マッピング確認が容易になります。同一リリースで複数サフィックスを昇格する場合は `--name` を追加してください。

## ステップ 6b - 正規ホストマッピングを導出

GAR ペイロードをテンプレート化する前に、各エイリアスの決定論的ホストマッピングを記録します。`cargo xtask soradns-hosts` は `--name` をハッシュして正規ラベル（`<base32>.gw.sora.id`）を生成し、必要なワイルドカード（`*.gw.sora.id`）と pretty ホスト（`<alias>.gw.sora.name`）を導出します。出力をリリース成果物に保存してください。

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

`--verify-host-patterns <file>` を使うと、GAR やゲートウェイバインディング JSON が必要なホストを欠いている場合に即失敗させられます。複数ファイルを指定できるため、GAR テンプレートと `portal.gateway.binding.json` を同じ実行で検証できます。

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

JSON サマリーと検証ログを DNS/ゲートウェイ変更チケットに添付し、監査人がホストマッピングを再実行なしで確認できるようにします。新しいエイリアスを追加した場合は再実行し、証跡を更新してください。

## ステップ 7 - DNS カットオーバー記述子を生成

プロダクションカットオーバーには監査可能な変更パケットが必要です。送信（エイリアスバインディング）成功後、ヘルパーは `artifacts/sorafs/portal.dns-cutover.json` を生成し、次を記録します。

- エイリアスバインディングのメタデータ（namespace/name/proof、マニフェストダイジェスト、Torii URL、送信 epoch、権限）
- リリースコンテキスト（タグ、エイリアスラベル、マニフェスト/CAR パス、チャンクプラン、Sigstore バンドル）
- 検証ポインタ（probe コマンド、エイリアス + Torii エンドポイント）
- 任意の変更管理フィールド（チケット ID、カットオーバーウィンドウ、Ops 連絡先、本番ホスト/ゾーン）
- `Sora-Route-Binding` ヘッダー由来のルート昇格メタデータ（正規ホスト/CID、ヘッダー/バインディングのパス、検証コマンド）
- 生成された route-plan 成果物（`gateway.route_plan.json`、ヘッダーテンプレート、任意のロールバックヘッダー）
- キャッシュ無効化メタデータ（パージエンドポイント、認証変数、JSON ペイロード、`curl` 例）
- 以前の記述子へのロールバックヒント（リリースタグ、マニフェストダイジェスト）

キャッシュパージが必要な場合は、カットオーバー記述子と一緒に以下を生成します。

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

`portal.cache_plan.json` を DG-3 パケットに添付し、`PURGE` 実行時のホスト/パスと認証ヒントを一致させます。記述子のキャッシュセクションからこのファイルを参照できます。

DG-3 パケットには昇格+ロールバックのチェックリストも必要です。`cargo xtask soradns-route-plan` で生成します。

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` には正規/pretty ホスト、段階的なヘルスチェック、GAR バインディング更新、キャッシュパージ、ロールバック手順が含まれます。

`scripts/generate-dns-cutover-plan.mjs` がこの記述子を生成し、`sorafs-pin-release.sh` から自動実行されます。手動で再生成する場合は以下を使用します。

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --out artifacts/sorafs/portal.dns-cutover.json \
  --change-ticket OPS-4821 \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact docs-oncall@sora.link \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

ピンヘルパーを実行する前に、任意メタデータを環境変数で設定します。

| 変数 | 用途 |
| --- | --- |
| `DNS_CHANGE_TICKET` | 記述子に保存されるチケット ID。 |
| `DNS_CUTOVER_WINDOW` | ISO8601 カットオーバーウィンドウ（例: `2026-03-21T15:00Z/2026-03-21T15:30Z`）。 |
| `DNS_HOSTNAME`, `DNS_ZONE` | 本番ホスト名と権威ゾーン。 |
| `DNS_OPS_CONTACT` | オンコール連絡先。 |
| `DNS_CACHE_PURGE_ENDPOINT` | 記述子に記録するパージエンドポイント。 |
| `DNS_CACHE_PURGE_AUTH_ENV` | パージトークンを保持する env var（デフォルト: `CACHE_PURGE_TOKEN`）。 |
| `DNS_PREVIOUS_PLAN` | ロールバック用の以前の記述子パス。 |

DNS 変更レビューに JSON を添付し、マニフェストダイジェスト、エイリアスバインディング、probe コマンドを CI ログ無しで検証できるようにします。`--dns-change-ticket`、`--dns-cutover-window`、`--dns-hostname`、`--dns-zone`、`--ops-contact`、`--cache-purge-endpoint`、`--cache-purge-auth-env`、`--previous-dns-plan` の CLI フラグでも同等の設定が可能です。

## ステップ 8 - リゾルバ zonefile のスケルトンを出力（任意）

本番 cutover ウィンドウが確定したら、リリーススクリプトが SNS zonefile スケルトンとリゾルバスニペットを自動生成できます。環境変数または CLI で DNS レコードとメタデータを渡すと、`scripts/sns_zonefile_skeleton.py` が記述子生成直後に実行されます。A/AAAA/CNAME のいずれかと、署名済み GAR の BLAKE3-256 ダイジェストが必要です。ゾーン/ホスト名が既知で `--dns-zonefile-out` を省略した場合、`artifacts/sns/zonefiles/<zone>/<hostname>.json` に出力し、`ops/soradns/static_zones.<hostname>.json` をリゾルバスニペットとして生成します。

| 変数 / フラグ | 用途 |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | 生成する zonefile スケルトンのパス。 |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | リゾルバスニペットのパス（省略時は `ops/soradns/static_zones.<hostname>.json`）。 |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | 生成レコードに適用する TTL（デフォルト: 600 秒）。 |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | IPv4 アドレス（カンマ区切りまたは繰り返し指定）。 |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | IPv6 アドレス。 |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | 任意の CNAME ターゲット。 |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | SHA-256 SPKI ピン（base64）。 |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | 追加 TXT エントリ（`key=value`）。 |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | zonefile バージョンラベルを上書き。 |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | `effective_at` を RFC3339 で指定。 |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | メタデータに記録する proof を上書き。 |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | メタデータに記録する CID を上書き。 |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Guardian freeze 状態（soft, hard, thawing, monitoring, emergency）。 |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | freeze 用チケット参照。 |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | thawing 用 RFC3339 タイムスタンプ。 |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | 追加 freeze ノート。 |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | 署名済み GAR の BLAKE3-256 ダイジェスト（hex）。 |

GitHub Actions ワークフローはこれらの値をリポジトリシークレットから読み込み、プロダクションのピン時に自動で zonefile 成果物を生成します。次のシークレットを設定してください。

| Secret | 用途 |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | production の host/zone。 |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | 記述子に入るオンコール。 |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | 公開する IPv4/IPv6 レコード。 |
| `DOCS_SORAFS_ZONEFILE_CNAME` | 任意の CNAME ターゲット。 |
| `DOCS_SORAFS_ZONEFILE_SPKI` | base64 の SPKI ピン。 |
| `DOCS_SORAFS_ZONEFILE_TXT` | 追加 TXT エントリ。 |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | freeze メタデータ。 |
| `DOCS_SORAFS_GAR_DIGEST` | 署名済み GAR の BLAKE3 ダイジェスト。 |

`.github/workflows/docs-portal-sorafs-pin.yml` の実行時には、`dns_change_ticket` と `dns_cutover_window` を指定し、正しいウィンドウを継承させてください。dry run のみ空にします。

典型的な実行例（SN-7 owner runbook 準拠）:

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
  --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
  --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
  --dns-zonefile-ipv4 198.51.100.4 \
  --dns-zonefile-ttl 600 \
  --dns-zonefile-freeze-state soft \
  --dns-zonefile-freeze-ticket SNS-DF-XXXX \
  --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
  --dns-gar-digest <gar-digest-hex> \
  ...other flags...
```

ヘルパーは変更チケットを TXT エントリとして自動で取り込み、cutover ウィンドウの開始時刻を `effective_at` として継承します（上書きしない限り）。詳細な運用フローは `docs/source/sorafs_gateway_dns_owner_runbook.md` を参照してください。

### ゲートウェイヘッダーテンプレート

デプロイヘルパーは `portal.gateway.headers.txt` と `portal.gateway.binding.json` を出力し、DG-3 の gateway-content-binding 要件を満たします。

- `portal.gateway.headers.txt` には `Sora-Name`/`Sora-Content-CID`/`Sora-Proof`/CSP/HSTS/`Sora-Route-Binding` を含む完全な HTTP ヘッダーブロックが含まれます。
- `portal.gateway.binding.json` は同じ情報を機械可読な形で記録し、変更チケットや自動化で host/cid の比較ができます。

これらは `cargo xtask soradns-binding-template` で自動生成され、`sorafs-pin-release.sh` に渡したエイリアス、マニフェストダイジェスト、ゲートウェイホスト名を記録します。手動で再生成する場合は以下を使用します。

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

`--csp-template`、`--permissions-template`、`--hsts-template` でヘッダーを上書きできます。既存の `--no-*` スイッチと組み合わせてヘッダーを削除することも可能です。

ヘッダースニペットは CDN 変更リクエストに添付し、JSON はゲートウェイ自動化に流し込み、ホスト昇格が証跡と一致するようにします。

リリーススクリプトは検証ヘルパーを自動実行するため、DG-3 チケットに最新証跡が含まれます。バインディング JSON を手動編集した場合は、次を再実行してください。

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

このコマンドは stapled `Sora-Proof` をデコードし、`Sora-Route-Binding` のメタデータが manifest CID と hostname に一致することを検証します。CI 外で実行した場合は出力を成果物に保存してください。

> **DNS 記述子の統合:** `portal.dns-cutover.json` は、`gateway_binding` セクション（パス、content CID、proof ステータス、ヘッダーテンプレート）と、`route_plan` セクション（`gateway.route_plan.json` と主/ロールバックヘッダーテンプレート）を埋め込みます。DG-3 チケットにはこれらを含め、`Sora-Name/Sora-Proof/CSP` の値と昇格/ロールバック計画が一致することを確認できるようにします。

## ステップ 9 - 公開モニタの実行

ロードマップ **DOCS-3c** は、ポータル、Try it プロキシ、ゲートウェイバインディングがリリース後も健全であることを継続的に示す証跡を要求します。ステップ 7〜8 の直後に統合モニタを実行し、定期プローブに組み込んでください。

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` は config を読み込み（スキーマは `docs/portal/docs/devportal/publishing-monitoring.md` を参照）、ポータルのパスプローブ + CSP/Permissions-Policy 検証、Try it プロキシプローブ（任意で `/metrics`）、ゲートウェイバインディング検証（`cargo xtask soradns-verify-binding`）を実行します。
- いずれかのプローブが失敗すると非ゼロ終了し、CI/cron/運用がリリースを止められるようにします。
- `--json-out` はターゲットごとのステータスを含むサマリー JSON を書き出し、`--evidence-dir` は `summary.json`、`portal.json`、`tryit.json`、`binding.json`、`checksums.sha256` を出力します。
- 監視結果、Grafana エクスポート（`dashboards/grafana/docs_portal.json`）、Alertmanager drill ID をリリースチケットに添付し、DOCS-3c の SLO を後で監査できるようにします。

ポータルプローブは HTTPS が必須で、`allowInsecureHttp` が設定されていない限り `http://` を拒否します。staging/production は TLS を使い、ローカルプレビューだけで例外を有効にしてください。

`npm run monitor:publishing` を Buildkite/cron で自動化し、プロダクション URL に向けて定期監視します。

## `sorafs-pin-release.sh` による自動化

`docs/portal/scripts/sorafs-pin-release.sh` はステップ 2〜6 をまとめます。

1. `build/` を決定論的 tarball にアーカイブ
2. `car pack`、`manifest build`、`manifest sign`、`manifest verify-signature`、`proof verify` を実行
3. Torii の認証情報がある場合は `manifest submit`（エイリアスバインディング含む）を実行
4. `artifacts/sorafs/portal.pin.report.json`、任意の `portal.pin.proposal.json`、DNS 記述子、`portal.gateway.binding.json`/ヘッダーブロックを出力

`PIN_ALIAS`、`PIN_ALIAS_NAMESPACE`、`PIN_ALIAS_NAME`、（任意で）`PIN_ALIAS_PROOF_PATH` を設定して実行します。dry run では `--skip-submit` を使用し、GitHub ワークフローの `perform_submit` 入力で切り替えます。

## ステップ 8 - OpenAPI 仕様と SBOM バンドルの公開

DOCS-7 では、ポータルビルド、OpenAPI 仕様、SBOM 成果物を同じ決定論的パイプラインで扱う必要があります。既存ヘルパーで 3 つとも対応できます。

1. **仕様を再生成して署名**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   `--version=<label>` を指定すると履歴スナップショットを残せます（例: `2025-q3`）。`static/openapi/versions/<label>/torii.json` に保存し、`versions/current` を更新し、`static/openapi/versions.json` に SHA-256、マニフェスト状態、更新時刻を記録します。Swagger/RapiDoc はこのインデックスを読み、バージョン選択とダイジェスト/署名の表示を行います。`--version` を省略すると既存ラベルは保持し、`current` と `latest` だけを更新します。

   マニフェストには SHA-256/BLAKE3 のダイジェストが含まれ、`/reference/torii-swagger` に `Sora-Proof` を stapling できます。

2. **CycloneDX SBOM を生成**

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **各ペイロードを CAR にパッケージ**

   ```bash
   sorafs_cli car pack \
     --input docs/portal/static/openapi \
     --car-out "$OUT"/openapi.car \
     --plan-out "$OUT"/openapi.plan.json \
     --summary-out "$OUT"/openapi.car.json

   sorafs_cli car pack \
     --input "$OUT"/portal.sbom.json \
     --car-out "$OUT"/portal.sbom.car \
     --plan-out "$OUT"/portal.sbom.plan.json \
     --summary-out "$OUT"/portal.sbom.car.json
   ```

   サイトと同様に `manifest build`/`manifest sign` を実行し、OpenAPI には `docs-openapi.sora`、SBOM には `docs-sbom.sora` など、成果物ごとに別のエイリアスを割り当てます。これにより SoraDNS/GAR/ロールバックがペイロード単位で切り分けられます。

4. **submit と binding**

   権限と Sigstore バンドルは再利用し、リリースチェックリストにエイリアスタプルを記録して追跡できるようにします。

OpenAPI/SBOM マニフェストをポータルビルドと同じバンドルにまとめれば、チケットに必要な成果物が揃います。

### 自動化ヘルパー（CI/パッケージスクリプト）

`./ci/package_docs_portal_sorafs.sh` はステップ 1〜8 を 1 コマンドで実行できるようにします。内容は以下です。

- ポータル準備（`npm ci`、OpenAPI/Norito 同期、Widget テスト）
- ポータル/OpenAPI/SBOM の CAR とマニフェスト生成
- 任意で `sorafs_cli proof verify`（`--proof`）と Sigstore 署名（`--sign`/`--sigstore-*`）
- `artifacts/devportal/sorafs/<timestamp>/` に成果物を配置し、`package_summary.json` を生成
- `artifacts/devportal/sorafs/latest` を最新実行に更新

例（Sigstore + PoR のフルパイプライン）:

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

主なフラグ:

- `--out <dir>` - 出力ディレクトリを変更
- `--skip-build` - 既存の `docs/portal/build` を使う
- `--skip-sync-openapi` - `cargo xtask openapi` が crates.io に到達できない場合に同期を省略
- `--skip-sbom` - `syft` が無い場合は SBOM 生成をスキップ（警告のみ）
- `--proof` - CAR/マニフェストの検証を実行
- `--sign` - Sigstore 署名を実行

本番向けには `docs/portal/scripts/sorafs-pin-release.sh` を使用してください。ポータル/OpenAPI/SBOM をまとめてパッケージし、各マニフェストを署名して `portal.additional_assets.json` に記録します。`--openapi-*`/`--portal-sbom-*`/`--openapi-sbom-*` でエイリアスや SBOM ソースを調整できます。

このスクリプトは実行コマンドをすべて出力するので、`package_summary.json` と一緒にリリースチケットへ貼り付けてください。

## ステップ 9 - ゲートウェイ + SoraDNS 検証

カットオーバー告知前に、エイリアスが SoraDNS で解決され、ゲートウェイが最新 proof を stapling していることを確認します。

1. **probe gate の実行**

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   probe は `Sora-Name`/`Sora-Proof`/`Sora-Proof-Status` を検証し、マニフェストダイジェスト、TTL、GAR バインディングが一致しない場合に失敗します。

   For lightweight spot checks (for example, when only the binding bundle
   changed), run `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   The helper validates the captured binding bundle and is handy for release
   tickets that only need binding confirmation instead of a full probe drill.

2. **drill 証跡の取得**

   `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- ...` を使うと、ヘッダー/ログを `artifacts/sorafs_gateway_probe/<stamp>/` に保存し、`ops/drill-log.md` を更新します。`--host docs.sora` を指定して SoraDNS 経路を検証してください。

3. **DNS バインディングの検証**

   proof 公開後は GAR を証跡として保存し、`cargo xtask soradns-verify-gar --gar <path> --name <alias> ...` でオフライン検証します。`--json-out` を使えばレビュー用の JSON サマリーも生成できます。

4. **エイリアスメトリクス監視**

   `torii_sorafs_alias_cache_refresh_duration_ms` と `torii_sorafs_gateway_refusals_total{profile="docs"}` を監視してください。

## ステップ 10 - 監視と証跡バンドル

- **ダッシュボード**: `dashboards/grafana/docs_portal.json`、`dashboards/grafana/sorafs_gateway_observability.json`、`dashboards/grafana/sorafs_fetch_observability.json` をエクスポートし、リリースチケットに添付します。
- **probe アーカイブ**: `artifacts/sorafs_gateway_probe/<stamp>/` を保存します。
- **リリースバンドル**: CAR サマリー、マニフェストバンドル、Sigstore 署名、`portal.pin.report.json`、Try-It プローブログなどを 1 つのフォルダにまとめます。
- **drill ログ**: `scripts/telemetry/run_sorafs_gateway_probe.sh` が `ops/drill-log.md` を更新します。
- **チケットリンク**: Grafana のパネル ID や PNG を添付し、SLO を確認できるようにします。

## ステップ 11 - マルチソース fetch drill とスコアボード証跡

SoraFS への公開では、DNS/ゲートウェイ証跡に加えてマルチソース fetch 証跡（DOCS-7/SF-6）が必要です。

1. **`sorafs_fetch` の実行**

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$FETCH_OUT"

   cargo run -p sorafs_car --bin sorafs_fetch -- \
     --plan "$OUT/portal.plan.json" \
     --manifest-json "$OUT/portal.manifest.json" \
     --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
     --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
     --scoreboard-out "$FETCH_OUT/scoreboard.json" \
     --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
     --json-out "$FETCH_OUT/fetch.json" \
     --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
     --telemetry-json artifacts/sorafs/provider_telemetry.json \
     --max-peers=3 \
     --retry-budget=4
   ```

   - 先に provider advert を取得し、`--provider-advert name=path` で渡します。
   - 追加リージョンがある場合は、対応プロバイダで再実行します。

2. **出力のアーカイブ**

   `scoreboard.json`、`providers.ndjson`、`fetch.json`、`chunk_receipts.ndjson` を保存します。

3. **テレメトリ更新**

   `dashboards/grafana/sorafs_fetch_observability.json` を参照し、`torii_sorafs_fetch_duration_ms` を監視します。

4. **アラート検証**

   `scripts/telemetry/test_sorafs_fetch_alerts.sh` を実行し、promtool 出力をチケットへ添付します。

5. **CI への組み込み**

   `perform_fetch_probe` を有効にして staging/production で実行します。

## 昇格・監視・ロールバック

1. **昇格**: staging と production を別エイリアスにし、同じマニフェスト/バンドルで `manifest submit` を再実行して昇格します。
2. **監視**: `docs/source/grafana_sorafs_pin_registry.json` とポータル専用プローブを使用します。
3. **ロールバック**: 以前のマニフェストを再送信するか、`--alias ... --retire` で取り下げます。

## CI ワークフローテンプレート

最低限のパイプラインは以下を含みます。

1. Build + lint（`npm ci`、`npm run build`、チェックサム生成）
2. パッケージ化（`car pack`）とマニフェスト作成
3. OIDC トークンで署名（`manifest sign`）
4. 成果物アップロード（CAR、マニフェスト、バンドル、プラン、サマリー）
5. ピンレジストリへの送信
   - Pull request -> `docs-preview.sora`
   - タグ/保護ブランチ -> production エイリアス
6. probes + proof 検証ゲート

`.github/workflows/docs-portal-sorafs-pin.yml` はこれらを手動リリース用に統合しています。ワークフローは次を行います。

- ポータルの build/test
- `scripts/sorafs-pin-release.sh` によるパッケージ
- GitHub OIDC での署名/検証
- CAR/マニフェスト/バンドル/プラン/サマリーの upload
- secrets がある場合の manifest + alias binding

実行前に以下の secrets/variables を設定してください。

| Name | 用途 |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | `/v1/sorafs/pin/register` を公開する Torii ホスト。 |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | 送信 epoch。 |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | 署名権限。 |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | alias tuple。 |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | base64 の alias proof（任意）。 |
| `DOCS_ANALYTICS_*` | 既存の analytics/probe endpoints。 |

Actions UI から実行する場合は以下を指定します。

1. `alias_label`（例: `docs.sora.link`）、任意の `proposal_alias`、任意の `release_tag` を入力
2. `perform_submit` をオフにすると dry run、オンで直接 publish

`docs/source/sorafs_ci_templates.md` は外部向けの汎用テンプレートを維持していますが、日常のリリースにはポータル用ワークフローを使ってください。

## チェックリスト

- [ ] `npm run build`, `npm run test:*`, `npm run check:links` が成功
- [ ] `build/checksums.sha256` と `build/release.json` を成果物に保存
- [ ] CAR/プラン/マニフェスト/サマリー生成
- [ ] Sigstore バンドルと署名を保存
- [ ] `portal.manifest.submit.summary.json` と `portal.manifest.submit.response.json` を保存
- [ ] `portal.pin.report.json`（任意で `portal.pin.proposal.json`）を保存
- [ ] `proof verify` と `manifest verify-signature` のログを保存
- [ ] Grafana ダッシュボード更新と Try-It プローブ成功
- [ ] ロールバック情報（以前のマニフェスト ID とエイリアスダイジェスト）をチケットに添付
