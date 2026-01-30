---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/operations-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b9e6c880827f8eb6333d452ae57dce45e0830adda2435bee1139fc6abb8d0ef6
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
id: operations-playbook
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note 正規ソース
このページは `docs/source/sorafs_ops_playbook.md` にあるランブックを反映しています。Sphinx ドキュメントセットが完全に移行されるまで、両方のコピーを同期してください。
:::

## 主要参照

- 観測性アセット: `dashboards/grafana/` 配下の Grafana ダッシュボードと、`dashboards/alerts/` の Prometheus アラートルールを参照。
- メトリクスカタログ: `docs/source/sorafs_observability_plan.md`。
- オーケストレーターのテレメトリ面: `docs/source/sorafs_orchestrator_plan.md`。

## エスカレーション・マトリクス

| 優先度 | トリガー例 | 主担当オンコール | バックアップ | 注記 |
|--------|------------|------------------|--------------|------|
| P1 | 全体的な gateway 障害、PoR 失敗率 > 5% (15 分)、レプリケーション backlog が 10 分ごとに倍増 | Storage SRE | Observability TL | 影響が 30 分を超える場合はガバナンス評議会を招集。 |
| P2 | 地域 gateway の遅延 SLO 逸脱、SLA 影響なしのオーケストレーター再試行スパイク | Observability TL | Storage SRE | ロールアウトは継続するが新規 manifests をゲート。 |
| P3 | 重要度の低いアラート (manifest の陳腐化、容量 80–90%) | Intake triage | Ops guild | 次の営業日までに対応。 |

## Gateway 障害 / 可用性低下

**検知**

- アラート: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`。
- ダッシュボード: `dashboards/grafana/sorafs_gateway_overview.json`。

**即時対応**

1. リクエストレートのパネルで影響範囲 (単一プロバイダー vs フリート) を確認。
2. マルチプロバイダー環境では、ops 設定 (`docs/source/sorafs_gateway_self_cert.md`) の `sorafs_gateway_route_weights` を切り替えて Torii ルーティングを健全なプロバイダーへ移行。
3. すべてのプロバイダーが影響を受けている場合、CLI/SDK クライアント向けに “direct fetch” フォールバックを有効化 (`docs/source/sorafs_node_client_protocol.md`).

**トリアージ**

- `sorafs_gateway_stream_token_limit` に対する stream token 使用率を確認。
- TLS または admission エラーの有無を gateway ログで確認。
- `scripts/telemetry/run_schema_diff.sh` を実行し、gateway がエクスポートするスキーマが期待バージョンと一致することを確認。

**復旧オプション**

- 影響を受けた gateway プロセスのみを再起動する; 複数プロバイダーが失敗している場合を除き、クラスタ全体の再起動は避ける。
- 飽和が確認された場合、stream token の上限を 10–15% 一時的に引き上げる。
- 安定化後に self-cert を再実行 (`scripts/sorafs_gateway_self_cert.sh`).

**事後対応**

- `docs/source/sorafs/postmortem_template.md` を使って P1 postmortem を作成。
- 手動介入に依存した場合は、フォローアップのカオスドリルを計画。

## 証明失敗スパイク (PoR / PoTR)

**検知**

- アラート: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`。
- ダッシュボード: `dashboards/grafana/sorafs_proof_integrity.json`。
- テレメトリ: `torii_sorafs_proof_stream_events_total` と `provider_reason=corrupt_proof` の `sorafs.fetch.error` イベント。

**即時対応**

1. manifest レジストリをフラグし、新規 manifest の受け入れを停止 (`docs/source/sorafs/manifest_pipeline.md`).
2. Governance に通知して対象プロバイダーのインセンティブを停止。

**トリアージ**

- PoR チャレンジキュー深度を `sorafs_node_replication_backlog_total` と比較。
- 直近デプロイ向けに proof 検証パイプライン (`crates/sorafs_node/src/potr.rs`) を確認。
- オペレーターレジストリと照合してプロバイダーの firmware バージョンを比較。

**復旧オプション**

- 最新 manifest を使って `sorafs_cli proof stream` で PoR リプレイを起動。
- 失敗が継続する場合、ガバナンスレジストリを更新し、オーケストレーターの scoreboards を強制更新してプロバイダーをアクティブセットから除外。

**事後対応**

- 次の本番デプロイ前に PoR カオスドリルを実施。
- postmortem テンプレートで学びを記録し、プロバイダー適格性チェックリストを更新。

## レプリケーション遅延 / Backlog 増加

**検知**

- アラート: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`。`dashboards/alerts/sorafs_capacity_rules.yml` を取り込み、
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  をプロモーション前に実行して、Alertmanager がドキュメントのしきい値を反映することを確認。
- ダッシュボード: `dashboards/grafana/sorafs_capacity_health.json`。
- メトリクス: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`。

**即時対応**

1. backlog の範囲 (単一プロバイダーかフリートか) を確認し、必須でないレプリケーションタスクを停止。
2. backlog が局所的なら、レプリケーションスケジューラで新規オーダーを一時的に別プロバイダーへ割り当て。

**トリアージ**

- backlog を増幅させる可能性のある再試行バーストがないか、オーケストレーターのテレメトリを確認。
- `sorafs_node_capacity_utilisation_percent` によるストレージ headroom を確認。
- 最近の設定変更 (chunk profile 更新、proof cadence) をレビュー。

**復旧オプション**

- `sorafs_cli` の `--rebalance` オプションでコンテンツを再配置。
- 影響プロバイダー向けにレプリケーションワーカーを水平スケール。
- TTL ウィンドウを揃えるため manifest refresh を実行。

**事後対応**

- プロバイダー飽和障害に焦点を当てた容量ドリルを計画。
- `docs/source/sorafs_node_client_protocol.md` のレプリケーション SLA 文書を更新。

## カオスドリルの頻度

- **四半期**: gateway 障害 + オーケストレーター再試行ストームの複合シミュレーション。
- **半年ごと**: 2 プロバイダーに対する PoR/PoTR 失敗注入と復旧。
- **月次スポットチェック**: staging manifests を使ったレプリケーション遅延シナリオ。
- 共有のランブログ (`ops/drill-log.md`) へ以下で記録:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- コミット前に次を実行してログを検証:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- 予定のドリルには `--status scheduled` を使用。完了は `pass`/`fail`、アクションが残る場合は `follow-up`。
- `--log` で出力先を上書きして dry-run や自動検証を実施可能。未指定の場合は `ops/drill-log.md` が更新され続けます。

## Postmortem テンプレート

P1/P2 インシデントおよびカオスドリルの振り返りには `docs/source/sorafs/postmortem_template.md` を使用します。テンプレートはタイムライン、影響の定量化、寄与要因、是正措置、フォローアップ検証タスクをカバーします。
