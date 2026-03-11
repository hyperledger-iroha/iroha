---
id: publishing-monitoring
lang: ja
direction: ltr
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

ロードマップ項目 **DOCS-3c** はパッケージングのチェックリストだけでは不十分です。SoraFS 公開後、
開発者ポータル、Try it プロキシ、ゲートウェイのバインディングが健全であることを継続的に証明する必要があります。
このページは、[デプロイガイド](./deploy-guide.md)に付随する監視面を整理し、CI とオンコールが Ops と同じ検証を
実行できるようにします。

## パイプラインの要約

1. **ビルドと署名** - [デプロイガイド](./deploy-guide.md)に従い `npm run build`、
   `scripts/preview_wave_preflight.sh`、Sigstore + manifest 提出手順を実行します。
   preflight スクリプトは `preflight-summary.json` を出力し、各プレビューに build/link/probe のメタデータを付与します。
2. **ピン留めと検証** - `sorafs_cli manifest submit`、`cargo xtask soradns-verify-binding`、
   DNS カットオーバープランにより、ガバナンス向けの決定的な artefacts を提供します。
3. **証跡のアーカイブ** - CAR サマリー、Sigstore バンドル、alias 証明、probe 出力、
   `docs_portal.json` ダッシュボードのスナップショットを `artifacts/sorafs/<tag>/` に保存します。

## 監視チャネル

### 1. 公開モニタ (`scripts/monitor-publishing.mjs`)

新しい `npm run monitor:publishing` コマンドは、ポータル probe、Try it プロキシ probe、
バインディング検証を 1 本の CI 向けチェックにまとめます。JSON config（CI secrets か
`configs/docs_monitor.json` に保存）を用意して実行します。

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

`--prom-out ../../artifacts/docs_monitor/monitor.prom`（必要なら `--prom-job docs-preview`）を付けると、
Prometheus テキスト形式のメトリクスを出力します。Pushgateway や staging/production の直接 scrape に対応します。
メトリクスは JSON サマリーと対応しているため、SLO ダッシュボードやアラートルールがエビデンスバンドルを解析せずに
ポータル、Try it、バインディング、DNS の健全性を追跡できます。

必須ノブと複数 binding を含む config 例:

```json
{
  "portal": {
    "baseUrl": "https://docs-preview.sora.link",
    "paths": ["/", "/devportal/try-it", "/reference/torii-swagger"],
    "expectRelease": "preview-2026-02-14",
    "checkSecurity": true,
    "expectedSecurity": {
      "csp": "default-src 'self'; connect-src https://tryit-preview.sora",
      "permissionsPolicy": "fullscreen=()",
      "referrerPolicy": "strict-origin-when-cross-origin"
    }
  },
  "tryIt": {
    "proxyUrl": "https://tryit-preview.sora",
    "samplePath": "/proxy/v1/accounts/i105.../assets?limit=1",
    "method": "GET",
    "timeoutMs": 7000,
    "token": "${TRYIT_BEARER}",
    "metricsUrl": "https://tryit-preview.sora/metrics"
  },
  "bindings": [
    {
      "label": "portal",
      "bindingPath": "../../artifacts/sorafs/portal.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal.manifest.json"
    },
    {
      "label": "openapi",
      "bindingPath": "../../artifacts/sorafs/openapi.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/openapi.manifest.json"
    },
    {
      "label": "portal-sbom",
      "bindingPath": "../../artifacts/sorafs/portal-sbom.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal-sbom.manifest.json"
    }
  ],

  "dns": [
    {
      "label": "docs-preview CNAME",
      "hostname": "docs-preview.sora.link",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    },
    {
      "label": "docs-preview canonical",
      "hostname": "igjssx53t4ayu3d5qus5o6xtp2f5dvka5rewr6xgscpmh3x4io4q.gw.sora.id",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    }
  ]
}
```

モニタは JSON サマリー（S3/SoraFS 向け）を書き出し、probe が失敗すると non-zero で終了するため、
Cron ジョブ、Buildkite ステップ、Alertmanager webhook に適しています。`--evidence-dir` を付けると
`summary.json`, `portal.json`, `tryit.json`, `binding.json` と `checksums.sha256` が保存され、
ガバナンスレビュアーは probes を再実行せずに diff できます。

> **TLS ガードレール:** `monitorPortal` は `http://` の base URL を拒否します（`allowInsecureHttp: true` を
> config に指定した場合を除く）。production/staging の probes は HTTPS を維持し、ローカル preview 専用の例外にします。

Each binding entry runs `cargo xtask soradns-verify-binding` against the captured
`portal.gateway.binding.json` bundle (and optional `manifestJson`) so alias,
proof status, and content CID stay aligned with the published evidence. The
optional `hostname` guard confirms the alias-derived canonical host matches the
gateway host you intend to promote, preventing DNS cutovers that drift from the
recorded binding.


オプションの `dns` ブロックは DOCS-7 の SoraDNS rollout を同じモニタに統合します。各エントリは
hostname/record-type ペア（例: `docs-preview.sora.link` -> `docs-preview.sora.link.gw.sora.name` の CNAME）を解決し、
`expectedRecords` または `expectedIncludes` に一致することを確認します。上のスニペットの 2 つ目のエントリは
`cargo xtask soradns-hosts --name docs-preview.sora.link` が生成する canonical ハッシュ名を固定しています。
モニタは friendly alias と canonical hash（`igjssx53...gw.sora.id`）の両方が pin した pretty host に解決されることを
証明し、DNS 昇格の証跡を自動化します。どちらかが逸脱すると、HTTP bindings が正しい manifest を付与していても失敗します。

### 2. OpenAPI バージョン manifest ガード

DOCS-2b の「署名済み OpenAPI manifest」要件には自動ガードが追加されました。
`ci/check_openapi_spec.sh` が `npm run check:openapi-versions` を呼び、
`scripts/verify-openapi-versions.mjs` が `docs/portal/static/openapi/versions.json` を
Torii の実 spec/manifest と照合します。ガードの検証項目:

- `versions.json` に載っている各バージョンに対応するディレクトリが `static/openapi/versions/` に存在すること。
- `bytes` と `sha256` がディスク上の spec に一致すること。
- `latest` エイリアスが `current` エントリ（digest/size/signature metadata）を反映し、デフォルトのダウンロードが逸脱しないこと。
- 署名済みエントリが同じ spec を指す `artifact.path` を持ち、署名/公開鍵 hex が manifest と一致すること。

新しい spec をミラーしたらローカルでガードを実行します:

```bash
cd docs/portal
npm run check:openapi-versions
```

失敗メッセージには stale-file のヒント（`npm run sync-openapi -- --latest`）が含まれるため、
スナップショット更新が容易です。CI にガードを入れることで、署名 manifest と公開 digest がずれたままの
ポータルリリースを防ぎます。

### 2. ダッシュボードとアラート

- **`dashboards/grafana/docs_portal.json`** - DOCS-3c の主要ボード。`torii_sorafs_gateway_refusals_total`、
  レプリケーション SLA ミス、Try it プロキシエラー、probe レイテンシ（`docs.preview.integrity` オーバーレイ）を追跡します。
  リリースごとにエクスポートして運用チケットに添付します。
- **Try it プロキシアラート** - Alertmanager ルール `TryItProxyErrors` は
  `probe_success{job="tryit-proxy"}` の継続的な低下や `tryit_proxy_requests_total{status="error"}` のスパイクで発火します。
- **Gateway SLO** - `DocsPortal/GatewayRefusals` は alias binding が pin された manifest digest を広告し続けることを保証し、
  エスカレーションは公開時に取得した `cargo xtask soradns-verify-binding` の CLI transcript にリンクします。

### 3. エビデンストレイル

各モニタリング実行で以下を追加します:

- `monitor-publishing` の証跡バンドル（`summary.json`、セクション別ファイル、`checksums.sha256`）。
- リリース期間の `docs_portal` ボードの Grafana スクリーンショット。
- Try it プロキシの変更/ロールバックの transcript（`npm run manage:tryit-proxy` のログ）。
- `cargo xtask soradns-verify-binding` による alias 検証出力。

これらを `artifacts/sorafs/<tag>/monitoring/` に保存し、リリース issue にリンクして監査ログが CI ログ期限後も残るようにします。

## 運用チェックリスト

1. デプロイガイドを Step 7 まで実行。
2. production 設定で `npm run monitor:publishing` を実行し、JSON 出力をアーカイブ。
3. Grafana パネル（`docs_portal`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`）をキャプチャしてリリースチケットに添付。
4. production URL に同一 config で定期モニタ（推奨: 15 分ごと）を設定し、DOCS-3c の SLO ゲートを満たす。
5. インシデント時は `--json-out` 付きでモニタを再実行し、前後の証跡を postmortem に添付。

このループにより DOCS-3c が完了します。ポータルの build フロー、公開パイプライン、監視スタックが
再現可能なコマンド、サンプル config、テレメトリーフックを備えた 1 つの playbook に統合されます。
