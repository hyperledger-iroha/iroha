---
id: reserve-ledger-digest
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


Reserve+Rent ポリシー (ロードマップ項目 **SFM-6**) では、`sorafs reserve` の CLI ヘルパーと
`scripts/telemetry/reserve_ledger_digest.py` トランスレータが提供され、財務の実行で
決定論的な rent/reserve の転送を生成できるようになった。このページは
`docs/source/sorafs_reserve_rent_plan.md` に定義されたワークフローを反映し、
新しい転送フィードを Grafana + Alertmanager に接続して、経済・ガバナンスのレビューが
各請求サイクルを監査できるようにする方法を説明する。

## エンドツーエンドのワークフロー

1. **見積 + 台帳プロジェクション**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

   sorafs reserve ledger \
     --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
     --provider-account <katakana-i105-account-id> \
     --treasury-account <katakana-i105-account-id> \
     --reserve-account <katakana-i105-account-id> \
     --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc \
     --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   ledger ヘルパーは `ledger_projection` ブロック (rent due、reserve shortfall、top-up delta、
   underwriting ブール値) と、財務アカウントとリザーブアカウント間で XOR を移動するための
   Norito `Transfer` ISI を付与する。

2. **ダイジェスト生成 + Prometheus/NDJSON 出力**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   digest ヘルパーは micro-XOR の合計を XOR に正規化し、プロジェクションが underwriting を
   満たすかどうかを記録し、転送フィードのメトリクス `sorafs_reserve_ledger_transfer_xor`
   と `sorafs_reserve_ledger_instruction_total` を出力する。複数の ledger を処理する必要が
   ある場合 (例: プロバイダーのバッチ)、`--ledger`/`--label` のペアを繰り返すと、ヘルパーは
   すべての digest を含む単一の NDJSON/Prometheus ファイルを書き出し、ダッシュボードが
   追加の glue なしにサイクル全体を取り込めるようにする。`--out-prom` ファイルは node-exporter の
   textfile collector を対象にするため、`.prom` を exporter が監視するディレクトリへ置くか、
   Reserve ダッシュボード job が利用するテレメトリバケットへアップロードする。`--ndjson-out` は
   同じ payload をデータパイプラインに供給する。

3. **成果物 + エビデンスの公開**
   - digests を `artifacts/sorafs_reserve/ledger/<provider>/` に保存し、週次の経済レポートから
     Markdown サマリへリンクする。
   - rent の burn-down に JSON digest を添付し (監査人が計算を再現できるようにする)、
     ガバナンスの証跡パケットにチェックサムを含める。
   - digest が top-up または underwriting 逸脱を示す場合は、アラート ID
     (`SoraFSReserveLedgerTopUpRequired`, `SoraFSReserveLedgerUnderwritingBreach`) を参照し、
     適用した転送 ISI を記録する。

## メトリクス → ダッシュボード → アラート

| ソースメトリクス | Grafana パネル | アラート / ポリシーフック | Notes |
|------------------|----------------|---------------------------|-------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | `dashboards/grafana/sorafs_capacity_health.json` の “DA Rent Distribution (XOR/hour)” | 週次の財務ダイジェストを投入する。リザーブフローのスパイクは `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`) に反映される。 |
| `torii_da_rent_gib_months_total` | “Capacity Usage (GiB-months)” (同一ダッシュボード) | 請求されたストレージが XOR 転送と一致することを示すために ledger digest と組み合わせる。 |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | `dashboards/grafana/sorafs_reserve_economics.json` の “Reserve Snapshot (XOR)” + ステータスカード | `SoraFSReserveLedgerTopUpRequired` は `requires_top_up=1` で発火し、`SoraFSReserveLedgerUnderwritingBreach` は `meets_underwriting=0` で発火する。 |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | `dashboards/grafana/sorafs_reserve_economics.json` の “Transfers by Kind”, “Latest Transfer Breakdown”, カバレッジカード | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing`, `SoraFSReserveLedgerTopUpTransferMissing` は、転送フィードが欠落またはゼロにもかかわらず rent/top-up が必要な場合に警告する。カバレッジカードも同じケースで 0% に落ちる。 |

rent サイクルが完了したら Prometheus/NDJSON のスナップショットを更新し、Grafana パネルが
新しい `label` を取り込むことを確認し、スクリーンショット + Alertmanager ID を rent の
ガバナンスパケットに添付する。これにより CLI プロジェクション、テレメトリ、ガバナンス
成果物が **同一** の転送フィードに由来することが証明され、ロードマップの経済ダッシュボードが
Reserve+Rent 自動化と整合する。カバレッジカードは 100% (または 1.0) を示し、rent と
リザーブ top-up の転送が digest に存在すれば新しいアラートは解消する。
