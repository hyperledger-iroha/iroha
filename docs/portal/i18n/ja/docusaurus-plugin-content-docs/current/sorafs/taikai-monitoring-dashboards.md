---
id: taikai-monitoring-dashboards
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/taikai-monitoring-dashboards.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

Taikai routing-manifest (TRM) の準備は 2 つの Grafana ボードと付随する alerts に
依存する。このページは `dashboards/grafana/taikai_viewer.json`,
`dashboards/grafana/taikai_cache.json`, `dashboards/alerts/taikai_viewer_rules.yml`
のハイライトをまとめ、レビュアがリポジトリをクローンせずに追えるようにする。

## Viewer dashboard (`taikai_viewer.json`)

- **Live edge とレイテンシ:** パネルは p95/p99 レイテンシヒストグラム
  (`taikai_ingest_segment_latency_ms`, `taikai_ingest_live_edge_drift_ms`) を
  cluster/stream 別に可視化する。p99 > 900 ms または drift > 1.5 s に注意
  (アラート `TaikaiLiveEdgeDrift` が発火)。
- **Segment errors:** `taikai_ingest_segment_errors_total{reason}` を分解し、
  decode failures、lineage replay の試行、manifest mismatch を露出する。
  このパネルが “warning” バンドを超えた場合、SN13-C インシデントに
  スクリーンショットを添付する。
- **Viewer と CEK の健全性:** `taikai_viewer_*` メトリクス由来のパネルが
  CEK rotation 年齢、PQ guard mix、rebuffer counts、alert ロールアップを追跡する。
  CEK パネルは、新しい aliases を承認する前に governance が確認する rotation SLA
  を強制する。
- **Alias テレメトリのスナップショット:** `/status → telemetry.taikai_alias_rotations`
  テーブルがボード上にあり、オペレータが governance evidence を添付する前に
  manifest digests を確認できる。

## Cache dashboard (`taikai_cache.json`)

- **Tier pressure:** パネルは `sorafs_taikai_cache_{hot,warm,cold}_occupancy` と
  `sorafs_taikai_cache_promotions_total` をチャート化する。TRM ローテーションが
  特定 tier を過負荷にしていないか確認する。
- **QoS denials:** `sorafs_taikai_qos_denied_total` は cache pressure により
  throttling が発生したときに表出する。rate がゼロから外れたら drill log に記録。
- **Egress utilisation:** CMAF ウィンドウが回転する際に SoraFS exits が
  Taikai viewers に追従できているか確認する。

## Alerts とエビデンスの取得

- Paging rules は `dashboards/alerts/taikai_viewer_rules.yml` にあり、上記の
  パネルと 1 対 1 で対応する (`TaikaiLiveEdgeDrift`, `TaikaiIngestFailure`,
  `TaikaiCekRotationLag`, proof-health warnings)。すべての production cluster が
  Alertmanager に配線されていることを確認する。
- drills 中に取得した snapshots/スクリーンショットは、spool files と `/status`
  JSON と一緒に `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` に保存する。
  `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor` を使用して共有
  drill log に実行を追記する。
- dashboards が変更されたら、JSON ファイルの SHA-256 digest を portal PR の
  説明に含め、監査人が管理 Grafana フォルダと repo 版を照合できるようにする。

## エビデンス bundle チェックリスト

SN13-C レビューでは、各 drill/インシデントが Taikai anchor runbook に列挙された
同じ artefacts を提供することが期待される。bundle を governance review に備えるため、
以下の順で収集する:

1. 最新の `taikai-anchor-request-*.json`, `taikai-trm-state-*.json`,
   `taikai-lineage-*.json` を `config.da_ingest.manifest_store_dir/taikai/` から
   コピーする。これらの spool artefacts は、どの routing manifest (TRM) と
   lineage window が有効だったかを証明する。ヘルパー
   `cargo xtask taikai-anchor-bundle --spool <dir> --copy-dir <out> --out <out>/anchor_bundle.json [--signing-key <ed25519>]`
   が spool files をコピーし、hashes を出力し、必要に応じて要約を署名する。
2. `/v1/status` の出力を `.telemetry.taikai_alias_rotations[]` にフィルタし、
   spool files の横に保存する。レビュアは `manifest_digest_hex` と window bounds を
   spool state と比較する。
3. 上記のメトリクスに対する Prometheus snapshots をエクスポートし、
   viewer/cache dashboards のスクリーンショットを cluster/stream フィルタ込みで
   取得する。生の JSON/CSV とスクリーンショットを artefact フォルダに置く。
4. `dashboards/alerts/taikai_viewer_rules.yml` のルールに紐づく Alertmanager incident IDs
   (存在する場合) を含め、条件が解消した後に auto-closed されたかを記録する。

すべてを `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` に保存し、drill 監査と
SN13-C governance review が単一アーカイブを取得できるようにする。

## Drill cadence とログ

- Taikai anchor drill は毎月第一火曜日の 15:00 UTC に実施する。
  このスケジュールにより、SN13 governance sync 前にエビデンスを新鮮に保つ。
- 上記 artefacts を取得した後、`scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`
  で共有 ledger に実行を追記する。ヘルパーは `docs/source/sorafs/runbooks-index.md` に
  必要な JSON entry を出力する。
- アーカイブ済み artefacts を runbook index のエントリにリンクし、failed alerts や
  dashboard regressions は 48 時間以内に Media Platform WG/SRE チャンネルでエスカレーションする。
- drill summary スクリーンショット (latency, drift, errors, CEK rotation,
  cache pressure) を spool bundle と一緒に保存し、リハーサル時のダッシュボード挙動を
  オペレータが示せるようにする。

完全な Sev 1 手順と evidence checklist は
[Taikai Anchor Runbook](./taikai-anchor-runbook.md) を参照。このページは
SN13-C が 🈺 を外す前に必要とする dashboard 特有のガイダンスのみをまとめる。
