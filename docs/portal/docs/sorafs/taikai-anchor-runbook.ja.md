---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 50261b1f3173cd3916b29c81e85cc92ed8c14c38a0e0296be38397fe9b5c0596
source_last_modified: "2025-11-21T18:08:23.480735+00:00"
translation_last_reviewed: 2026-01-30
---

# Taikai アンカー観測ランブック

このポータル版は、次の正規ランブックを反映しています。
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md)
SN13-C の routing-manifest (TRM) アンカーをリハーサルする際に使用し、
SoraFS/SoraNet の運用者が spool アーティファクト、Prometheus テレメトリ、
ガバナンス証跡をポータルのプレビュー内で関連付けられるようにします。

## スコープとオーナー

- **プログラム:** SN13-C — Taikai manifests と SoraNS anchors。
- **オーナー:** Media Platform WG, DA Program, Networking TL, Docs/DevRel。
- **目的:** Sev 1/Sev 2 アラート、テレメトリ検証、証跡収集のための
  決定的な playbook を提供し、Taikai routing manifests が aliases を
  ロールフォワードする間の運用を支援します。

## クイックスタート (Sev 1/Sev 2)

1. **spool アーティファクトの取得** — 最新の
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json`,
   `taikai-lineage-*.json` を
   `config.da_ingest.manifest_store_dir/taikai/` からコピーし、
   worker を再起動する前に確保します。
2. **`/status` テレメトリのダンプ** —
   `telemetry.taikai_alias_rotations` 配列を記録して、どの manifest window が
   アクティブかを証明します:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **ダッシュボードとアラートの確認** —
   `dashboards/grafana/taikai_viewer.json` (cluster + stream フィルタ) を読み込み、
   `dashboards/alerts/taikai_viewer_rules.yml` のルールが発火したか確認します
   (`TaikaiLiveEdgeDrift`, `TaikaiIngestFailure`, `TaikaiCekRotationLag`, SoraFS PoR health events)。
4. **Prometheus の確認** — 「Metric reference」セクションのクエリを実行し、
   ingest の latency/drift と alias rotation カウンタが期待通りか確認します。
   `taikai_trm_alias_rotations_total` が複数 window で停止するか、エラーカウンタが
   増加した場合はエスカレーションしてください。

## Metric reference

| メトリクス | 目的 |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | CMAF ingest latency のヒストグラム (cluster/stream 別、目標: p95 < 750 ms, p99 < 900 ms)。 |
| `taikai_ingest_live_edge_drift_ms` | encoder と anchor workers 間の live-edge drift (p99 > 1.5 s が 10 分続くと paging)。 |
| `taikai_ingest_segment_errors_total{reason}` | reason 別エラーカウンタ (`decode`, `manifest_mismatch`, `lineage_replay`, ...)。増加すると `TaikaiIngestFailure` を発火。 |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | `/v2/da/ingest` が新しい TRM を受け取るたびに増加; `rate()` で回転ペースを検証。 |
| `/status → telemetry.taikai_alias_rotations[]` | `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total` と timestamps を含む JSON snapshot (evidence bundles 用)。 |
| `taikai_viewer_*` (rebuffer, CEK rotation age, PQ health, alerts) | viewer 側 KPI。CEK rotation と PQ circuits が anchors 期間中に健全であることを確認。 |

### PromQL snippets

```promql
histogram_quantile(
  0.99,
  sum by (le) (
    rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
)
```

```promql
sum by (reason) (
  rate(taikai_ingest_segment_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
)
```

```promql
rate(
  taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
)
```

## ダッシュボードとアラート

- **Grafana viewer board:** `dashboards/grafana/taikai_viewer.json` — p95/p99 latency、
  live-edge drift、segment errors、CEK rotation age、viewer alerts。
- **Grafana cache board:** `dashboards/grafana/taikai_cache.json` — hot/warm/cold promotions
  と QoS denials (alias windows が回転する際)。
- **Alertmanager rules:** `dashboards/alerts/taikai_viewer_rules.yml` — drift paging、
  ingest failure warnings、CEK rotation lag、SoraFS PoR health penalties/cooldowns。
  各 production cluster に receiver を用意してください。

## Evidence bundle checklist

- Spool artifacts (`taikai-anchor-request-*`, `taikai-trm-state-*`, `taikai-lineage-*`).
- `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>` を実行し、
  pending/delivered envelopes の署名付き JSON inventory を生成し、request/SSM/TRM/lineage
  ファイルを drill bundle にコピーします。デフォルト spool パスは
  `torii.toml` の `storage/da_manifests/taikai` です。
- `/status` snapshot (`telemetry.taikai_alias_rotations` を含む)。
- 事故ウィンドウに対する Prometheus exports (JSON/CSV)。
- フィルタが表示された Grafana スクリーンショット。
- 該当する rule fires を参照する Alertmanager IDs。
- `docs/examples/taikai_anchor_lineage_packet.md` へのリンク (canonical evidence packet の説明)。

## ダッシュボードのミラーリングと drill cadence

SN13-C の要件は、Taikai viewer/cache dashboards がポータルに反映され、
**かつ** anchor evidence drill が予測可能な cadence で実行されることを
示す必要があります。

1. **ポータルミラーリング。** `dashboards/grafana/taikai_viewer.json` または
   `dashboards/grafana/taikai_cache.json` が変更されたら、
   `sorafs/taikai-monitoring-dashboards` (このポータル) に差分を要約し、
   ポータル PR の説明に JSON checksums を記載します。新しいパネル/閾値を
   強調し、管理下の Grafana フォルダと相関できるようにします。
2. **月次ドリル。**
   - 毎月第1火曜日 15:00 UTC に drill を実行し、SN13 ガバナンス sync 前に
     証跡が揃うようにします。
   - spool artifacts、`/status` テレメトリ、Grafana スクリーンショットを
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` に保存します。
   - 実行ログを `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor` で記録します。
3. **レビューと公開。** 48 時間以内に DA Program + NetOps と
   alerts/false positives をレビューし、drill log に follow-up を記録し、
   `docs/source/sorafs/runbooks-index.md` から governance bucket upload をリンクします。

ダッシュボードや drills が遅れると SN13-C は 🈺 を解消できません。cadence
または証跡期待値が変わるたびにこのセクションを更新してください。

## 便利なコマンド

```bash
# Alias rotation telemetry をアーティファクトディレクトリへスナップショット
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# 特定の alias/event の spool entries を列挙
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# spool log から TRM mismatch 理由を確認
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

Taikai anchoring telemetry、dashboards、または governance evidence 要件が変更された
場合は、このポータル版を正規ランブックと同期してください。
