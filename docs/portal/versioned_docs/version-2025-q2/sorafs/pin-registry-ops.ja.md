---
lang: ja
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T15:38:30.656337+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: pin-registry-ops-ja
slug: /sorafs/pin-registry-ops-ja
---

:::note 正規ソース
ミラー `docs/source/sorafs/runbooks/pin_registry_ops.md`。リリース間で両方のバージョンを揃えてください。
:::

## 概要

この Runbook では、SoraFS ピン レジストリとそのレプリケーション サービス レベル アグリーメント (SLA) を監視および優先順位付けする方法について説明します。メトリクスは `iroha_torii` から発信され、Prometheus 経由で `torii_sorafs_*` 名前空間にエクスポートされます。 Torii は、バックグラウンドで 30 秒間隔でレジストリの状態をサンプリングするため、オペレーターが `/v1/sorafs/pin/*` エンドポイントをポーリングしていない場合でも、ダッシュボードは最新の状態を維持します。厳選されたダッシュボード (`docs/source/grafana_sorafs_pin_registry.json`) をインポートして、以下のセクションに直接マッピングされるすぐに使用できる Grafana レイアウトを作成します。

## メトリクス参照

|メトリック |ラベル |説明 |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) |ライフサイクル状態ごとのオンチェーンマニフェストインベントリ。 |
| `torii_sorafs_registry_aliases_total` | — |レジストリに記録されているアクティブなマニフェスト エイリアスの数。 |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) |ステータスごとに分割されたレプリケーション注文のバックログ。 |
| `torii_sorafs_replication_backlog_total` | — | `pending` の注文をミラーリングするコンビニエンス ゲージ。 |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA アカウンティング: `met` は期限内に完了した注文をカウントし、`missed` は遅延完了と期限切れを集計し、`pending` は未処理の注文を反映します。 |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) |集計された完了レイテンシ (発行と完了の間のエポック)。 |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) |未決注文のスラック ウィンドウ (期限から発行されたエポックを引いたもの)。 |

すべてのゲージはスナップショットを取得するたびにリセットされるため、ダッシュボードは `1m` のペース以上でサンプリングする必要があります。

## Grafana ダッシュボード

ダッシュボード JSON には、オペレーターのワークフローをカバーする 7 つのパネルが付属しています。カスタム チャートを作成したい場合のクイック リファレンスとして、クエリを以下に示します。

1. **マニフェスト ライフサイクル** – `torii_sorafs_registry_manifests_total` (`status` によってグループ化)。
2. **エイリアス カタログ トレンド** – `torii_sorafs_registry_aliases_total`。
3. **ステータス別の注文キュー** – `torii_sorafs_registry_orders_total` (`status` によってグループ化)。
4. **バックログと期限切れの注文** – `torii_sorafs_replication_backlog_total` と `torii_sorafs_registry_orders_total{status="expired"}` を組み合わせて表面飽和状態にします。
5. **SLA 成功率** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **レイテンシとデッドライン スラック** – オーバーレイ `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` および `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`。絶対的なスラックフロアが必要な場合は、Grafana 変換を使用して `min_over_time` ビューを追加します。次に例を示します。

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **注文ミス (1 時間率)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## アラートしきい値- **15 分間の SLA 成功  0**
  - しきい値: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - アクション: ガバナンス マニフェストを検査して、プロバイダーのチャーンを確認します。
- **完了 p95 > デッドライン スラックの平均**
  - しきい値: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - アクション: プロバイダーが期限前にコミットしていることを確認します。再割り当ての発行を検討してください。

### Prometheus ルールの例

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
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## トリアージ ワークフロー

1. **原因の特定**
   - バックログが低いまま SLA がスパイクに達しない場合は、プロバイダーのパフォーマンス (PoR の失敗、完了の遅れ) に焦点を当てます。
   - 安定したミスでバックログが増加する場合は、入場 (`/v1/sorafs/pin/*`) を検査して、議会の承認を待っているマニフェストを確認します。
2. **プロバイダーのステータスを検証します**
   - `iroha app sorafs providers list` を実行し、アドバタイズされた機能がレプリケーション要件と一致していることを確認します。
   - `torii_sorafs_capacity_*` ゲージをチェックして、プロビジョニングされた GiB と PoR の成功を確認します。
3. **レプリケーションの再割り当て**
   - バックログ スラック (`stat="avg"`) が 5 エポックを下回った場合、`sorafs_manifest_stub capacity replication-order` 経由で新しい注文を発行します (マニフェスト/CAR パッケージ化は `iroha app sorafs toolkit pack` を使用します)。
   - エイリアスにアクティブなマニフェスト バインディングがない場合はガバナンスに通知します (`torii_sorafs_registry_aliases_total` が予期せずドロップします)。
4. **結果を文書化**
   - インシデントのメモをタイムスタンプと影響を受けるマニフェスト ダイジェストとともに SoraFS 操作ログに記録します。
   - 新しい障害モードまたはダッシュボードが導入された場合は、この Runbook を更新します。

## 展開計画

実稼働環境でエイリアス キャッシュ ポリシーを有効にするか強化する場合は、次の段階的な手順に従ってください。1. **構成の準備**
   - `iroha_config` の `torii.sorafs_alias_cache` を、合意された TTL と猶予ウィンドウで更新します (ユーザー→実際): `positive_ttl`、`refresh_window`、`hard_expiry`、`negative_ttl`、`revocation_ttl`、 `rotation_max_age`、`successor_grace`、および `governance_grace`。デフォルトは、`docs/source/sorafs_alias_policy.md` のポリシーと一致します。
   - SDK の場合、クライアントの適用がゲートウェイと一致するように、構成レイヤー (Rust / NAPI / Python バインディングでは `AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)`) を通じて同じ値を配布します。
2. **ステージングでの予行演習**
   - 実稼働トポロジをミラーリングするステージング クラスターに構成変更をデプロイします。
   - `cargo xtask sorafs-pin-fixtures` を実行して、正規のエイリアス フィクスチャが引き続きデコードおよびラウンドトリップしていることを確認します。不一致がある場合は、最初に対処する必要がある上流のマニフェスト ドリフトを意味します。
   - `/v1/sorafs/pin/{digest}` および `/v1/sorafs/aliases` エンドポイントを、新しいケース、リフレッシュ ウィンドウのケース、期限切れのケース、および完全期限切れのケースをカバーする合成証明で実行します。この Runbook に対して HTTP ステータス コード、ヘッダー (`Sora-Proof-Status`、`Retry-After`、`Warning`)、および JSON 本体フィールドを検証します。
3. **運用環境で有効にする**
   - 標準の変更ウィンドウを介して新しい構成をロールアウトします。まずそれを Torii に適用し、ノードがログで新しいポリシーを確認したら、ゲートウェイ/SDK サービスを再起動します。
   - `docs/source/grafana_sorafs_pin_registry.json` を Grafana にインポートし (または既存のダッシュボードを更新し)、エイリアス キャッシュ更新パネルを NOC ワークスペースに固定します。
4. **展開後の検証**
   - `torii_sorafs_alias_cache_refresh_total` および `torii_sorafs_alias_cache_age_seconds` を 30 分間監視します。 `error`/`expired` 曲線のスパイクは、ポリシー更新ウィンドウと相関している必要があります。予想外の増加により、オペレーターは続行する前にエイリアス証明とプロバイダーの健全性を検査する必要があります。
   - クライアント側のログに同じポリシー決定が示されていることを確認します (証明が古いか期限切れになると、SDK によってエラーが表示されます)。クライアント警告がない場合は、構成が間違っていることを示します。
5. **フォールバック**
   - エイリアスの発行が遅れ、更新ウィンドウが頻繁にトリップする場合は、構成で `refresh_window` および `positive_ttl` を増やしてポリシーを一時的に緩和してから、再デプロイします。本当に古い証明が引き続き拒否されるように、`hard_expiry` をそのままの状態に保ちます。
   - テレメトリで `error` カウントの増加が示され続ける場合は、以前の `iroha_config` スナップショットを復元して以前の構成に戻し、その後、インシデントを開いてエイリアス生成の遅延を追跡します。

## 関連資料

- `docs/source/sorafs/pin_registry_plan.md` — 実装ロードマップとガバナンスのコンテキスト。
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — ストレージ ワーカーの操作。このレジストリ プレイブックを補完します。
