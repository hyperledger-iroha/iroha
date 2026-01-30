---
lang: ru
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/devportal/observability.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99978cfbbb7852dd4ca6770ffdade28c89fe9d59ecbd3411079fdae261b4aaa3
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/observability.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# ポータルのオブザーバビリティとアナリティクス

DOCS-SORA のロードマップでは、各プレビュー build に対して analytics、synthetic probe、
および broken-link 自動化が必要です。このノートは、訪問者データを漏らさずに
オペレーターが監視を配線できるよう、ポータルに同梱される仕組みを説明します。

## リリースタグ

- ポータルを build する際に `DOCS_RELEASE_TAG=<identifier>` を設定する
  (未設定の場合は `GIT_COMMIT` または `dev` にフォールバック)。値は
  `<meta name="sora-release">` に注入され、probe と dashboard が
  デプロイを区別できます。
- `npm run build` は `build/release.json` を出力します (`scripts/write-checksums.mjs` が作成)。
  タグ、タイムスタンプ、オプションの `DOCS_RELEASE_SOURCE` を記述します。同じファイルが
  プレビューアーティファクトに同梱され、link checker レポートから参照されます。

## プライバシー保護アナリティクス

- `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` を設定して軽量トラッカーを有効化します。
  ペイロードは `{ event, path, locale, release, ts }` を含み、referrer や IP メタデータは含みません。
  可能な限り `navigator.sendBeacon` を使い、ナビゲーションのブロックを回避します。
- `DOCS_ANALYTICS_SAMPLE_RATE` (0-1) でサンプリングを制御します。トラッカーは最後に送信した path を保持し、
  同一ナビゲーションに対する重複イベントを送信しません。
- 実装は `src/components/AnalyticsTracker.jsx` にあり、`src/theme/Root.js` を通じて
  グローバルにマウントされます。

## Synthetic probe

- `npm run probe:portal` は一般的なルート
  (`/`, `/norito/overview`, `/reference/torii-swagger`, など) に GET リクエストを発行し、
  `sora-release` メタタグが `--expect-release` (または `DOCS_RELEASE_TAG`) と一致するか検証します。
  例:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

失敗は path ごとに報告されるため、probe 成功を CD のゲートにしやすくなります。

## Broken-link 自動化

- `npm run check:links` は `build/sitemap.xml` を走査し、各エントリがローカルファイルに
  マップされることを確認します (`index.html` フォールバックをチェック)。そして
  `build/link-report.json` を書き込み、リリースメタデータ、合計、失敗、`checksums.sha256` の
  SHA-256 フィンガープリント (`manifest.id` として公開) を含めることで、各レポートを
  アーティファクトマニフェストに紐付けます。
- ページが欠落している場合は非ゼロで終了するため、CI は古い/壊れたルートでリリースを
  ブロックできます。レポートには試行した候補パスが記載され、ルーティングの退行を
  docs ツリーまで追跡できます。

## Grafana ダッシュボードとアラート

- `dashboards/grafana/docs_portal.json` は Grafana ボード **Docs Portal Publishing** を公開します。
  次のパネルが含まれます:
  - *Gateway Refusals (5m)* は `torii_sorafs_gateway_refusals_total` を `profile`/`reason` で
    スコープし、SRE が不正なポリシープッシュやトークン失敗を検知できるようにします。
  - *Alias Cache Refresh Outcomes* と *Alias Proof Age p90* は `torii_sorafs_alias_cache_*` を追跡し、
    DNS cut over の前に新しい proof が存在することを示します。
  - *Pin Registry Manifest Counts* と *Active Alias Count* の統計は pin-registry のバックログと
    総 alias 数を反映し、ガバナンスが各リリースを監査できるようにします。
  - *Gateway TLS Expiry (hours)* は publishing gateway の TLS 証明書が期限に近づいた際に強調表示します
    (アラート閾値は 72 h)。
  - *Replication SLA Outcomes* と *Replication Backlog* は `torii_sorafs_replication_*` テレメトリーを監視し、
    公開後にすべてのレプリカが GA 基準を満たすことを確認します。
- 組み込みテンプレート変数 (`profile`, `reason`) を使って `docs.sora` の publishing プロファイルに
  フォーカスするか、すべてのゲートウェイのスパイクを調査します。
- PagerDuty ルーティングはダッシュボードパネルを証拠として使用します: `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, `DocsPortal/TLSExpiry` のアラートは対応する系列が閾値を超えると発報されます。
  アラートの runbook をこのページにリンクして、オンコールが正確な Prometheus クエリを再現できるようにします。

## まとめて実行

1. `npm run build` 中に release/analytics の環境変数を設定し、post-build ステップで
   `checksums.sha256`, `release.json`, `link-report.json` を出力させます。
2. プレビューのホスト名に対して `npm run probe:portal` を実行し、`--expect-release` を同じタグに
   接続します。stdout を publishing チェックリスト用に保存します。
3. `npm run check:links` を実行して sitemap の壊れたエントリで早期に失敗させ、生成された JSON レポートを
   プレビューアーティファクトと一緒にアーカイブします。CI は最新レポートを
   `artifacts/docs_portal/link-report.json` に配置するため、ガバナンスは build ログから
   エビデンスバンドルを直接ダウンロードできます。
4. analytics endpoint をプライバシー保護コレクタ (Plausible、self-hosted OTEL ingest など) に
   転送し、サンプリング率をリリースごとに文書化してダッシュボードが正確に解釈できるようにします。
5. CI は既に preview/deploy workflow
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`) にこれらの手順を配線しているため、
   ローカルの dry run は secret 固有の挙動だけを確認すれば十分です。
