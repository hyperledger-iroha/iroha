---
lang: fr
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b49ec68de47965d8176ff0749beacdf1ade448299088d57ddf6bfb2351c42add
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: pin-registry-ops
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note 正規ソース
`docs/source/sorafs/runbooks/pin_registry_ops.md` を反映しています。レガシー Sphinx ドキュメントが退役するまで両方を同期してください。
:::

## 概要

このランブックは、SoraFS の Pin Registry と複製のサービスレベル合意 (SLA) を監視し、トリアージする方法を示します。メトリクスは `iroha_torii` から収集され、Prometheus で `torii_sorafs_*` 名前空間として公開されます。Torii は 30 秒間隔でバックグラウンドに registry 状態をサンプリングするため、オペレータが `/v1/sorafs/pin/*` エンドポイントをポーリングしていない場合でもダッシュボードは最新の状態を保ちます。用意されたダッシュボード (`docs/source/grafana_sorafs_pin_registry.json`) を取り込むことで、以下のセクションに対応した Grafana レイアウトをすぐに利用できます。

## メトリクス参照

| メトリクス | Labels | 説明 |
| --------- | ------ | ---- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | ライフサイクル状態別の on-chain manifests 在庫。 |
| `torii_sorafs_registry_aliases_total` | — | registry に記録されたアクティブな manifest alias の数。 |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | ステータス別の複製オーダー backlog。 |
| `torii_sorafs_replication_backlog_total` | — | `pending` オーダーを反映する簡易ゲージ。 |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA 集計: `met` は期限内完了、`missed` は遅延完了 + 期限切れ、`pending` は未完了オーダー。 |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | 完了レイテンシの集計 (発行から完了までのエポック数)。 |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | 未完了オーダーの余裕時間 (期限 - 発行エポック)。 |

全てのゲージはスナップショット取得ごとにリセットされるため、ダッシュボードは `1m` かそれ以上の頻度でサンプリングしてください。

## Grafana ダッシュボード

ダッシュボード JSON にはオペレータ向けの 7 つのパネルが含まれています。カスタムチャートを作る場合の参考として、以下にクエリを列挙します。

1. **Manifest ライフサイクル** – `torii_sorafs_registry_manifests_total` (`status` でグルーピング)。
2. **Alias カタログ推移** – `torii_sorafs_registry_aliases_total`。
3. **ステータス別オーダーキュー** – `torii_sorafs_registry_orders_total` (`status` でグルーピング)。
4. **Backlog vs 期限切れオーダー** – `torii_sorafs_replication_backlog_total` と `torii_sorafs_registry_orders_total{status="expired"}` を組み合わせて飽和度を可視化。
5. **SLA 成功率** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **レイテンシ vs 期限余裕** – `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` と `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}` を重ねて表示。絶対的な余裕の下限が必要な場合は Grafana の変換で `min_over_time` を追加します。

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **失敗オーダー (1h rate)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## アラート閾値

- **SLA 成功率 < 0.95 が 15 分継続**
  - 閾値: `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - 対応: SRE をページングし、複製 backlog のトリアージを開始。
- **Pending backlog が 10 超**
  - 閾値: `torii_sorafs_replication_backlog_total > 10` が 10 分継続
  - 対応: providers の可用性と Torii の容量スケジューラを確認。
- **期限切れオーダー > 0**
  - 閾値: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - 対応: ガバナンス manifests を確認し、providers の churn を検証。
- **完了 p95 > 期限余裕の平均**
  - 閾値: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - 対応: providers が期限内にコミットしているか確認し、必要なら再割り当て。

### Prometheus ルール例

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS 複製 SLA が目標未達"
          description: "SLA 成功率が 15 分間 95% を下回りました。"

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS 複製 backlog が閾値超過"
          description: "保留中の複製オーダーが設定された backlog 予算を超えました。"

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS 複製オーダーが期限切れ"
          description: "直近 5 分で少なくとも 1 件の複製オーダーが期限切れになりました。"
```

## トリアージ手順

1. **原因を特定**
   - SLA miss が増加し backlog が低い場合は、providers の性能 (PoR 失敗、遅延完了) に注力。
   - backlog が増えて miss が安定している場合は、`/v1/sorafs/pin/*` の admission を確認し、評議会承認待ちの manifests を特定。
2. **providers の状態を検証**
   - `iroha app sorafs providers list` を実行し、広告された能力が複製要件を満たすことを確認。
   - `torii_sorafs_capacity_*` のゲージで provisioned GiB と PoR 成功を確認。
3. **複製の再割り当て**
   - backlog の余裕 (`stat="avg"`) が 5 エポック未満になったら `sorafs_manifest_stub capacity replication-order` で新しいオーダーを発行 (manifest/CAR のパッケージングは `iroha app sorafs toolkit pack`)。
   - alias にアクティブな manifest bindings がない場合 ( `torii_sorafs_registry_aliases_total` の予期せぬ低下) はガバナンスに通知。
4. **結果を記録**
   - SoraFS オペレーションログに、タイムスタンプと影響を受けた manifest digest を記録。
   - 新しい障害モードやダッシュボードが導入されたら runbook を更新。

## ロールアウト計画

本番で alias cache ポリシーを有効化または厳格化する際は、次の段階的手順に従います。

1. **構成の準備**
   - `iroha_config` の `torii.sorafs_alias_cache` (user -> actual) を合意済み TTL と猶予ウィンドウで更新します: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`。デフォルト値は `docs/source/sorafs_alias_policy.md` のポリシーに一致します。
   - SDK には同じ値を設定レイヤに配布します (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` in Rust / NAPI / Python bindings)。クライアント側の enforcement を gateway と一致させるためです。
2. **ステージングでの dry-run**
   - 本番に近いトポロジの staging クラスタに構成変更をデプロイ。
   - `cargo xtask sorafs-pin-fixtures` を実行し、canonical alias fixtures がデコードされ round-trip することを確認。差異があれば upstream drift があるため先に解消します。
   - `/v1/sorafs/pin/{digest}` と `/v1/sorafs/aliases` を、fresh / refresh-window / expired / hard-expired ケースの合成 proof で検証します。HTTP ステータス、ヘッダ (`Sora-Proof-Status`, `Retry-After`, `Warning`)、JSON ボディのフィールドを本 runbook と照合。
3. **本番で有効化**
   - 標準の変更ウィンドウで新しい構成を適用。Torii を先に更新し、その後ノードのログで新ポリシーが確認されたら gateways/SDK サービスを再起動。
   - `docs/source/grafana_sorafs_pin_registry.json` を Grafana にインポート (または既存ダッシュボードを更新) し、alias cache refresh パネルを NOC ワークスペースにピン。
4. **デプロイ後の検証**
   - 30 分間 `torii_sorafs_alias_cache_refresh_total` と `torii_sorafs_alias_cache_age_seconds` を監視。`error`/`expired` のスパイクは refresh ウィンドウと一致すべきで、想定外の増加は alias proof と providers 健全性の確認が必要。
   - クライアント側ログが同じポリシー判断を示すことを確認 (SDK は proof が stale/expired の場合にエラーを出す)。クライアント警告が出ない場合は設定ミスの可能性。
5. **Fallback**
   - alias 発行が遅れ refresh ウィンドウが頻発する場合は、`refresh_window` と `positive_ttl` を増やして一時的に緩和し、再デプロイ。`hard_expiry` は維持し、真に stale な proof は拒否します。
   - テレメトリが `error` カウントの上昇を示し続ける場合は、以前の `iroha_config` スナップショットを復元して戻し、alias 生成遅延の原因調査のためインシデントを起票。

## 関連資料

- `docs/source/sorafs/pin_registry_plan.md` — 実装ロードマップとガバナンス文脈。
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — ストレージワーカー運用。本 playbook を補完します。
