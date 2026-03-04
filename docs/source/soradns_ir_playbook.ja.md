---
lang: ja
direction: ltr
source: docs/source/soradns_ir_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e9ad416067db38bc5a8f43c654b0094cf9ce8872dd4b919108b71afe22cfcc3e
source_last_modified: "2025-11-14T06:05:41.880329+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/soradns_ir_playbook.md -->

# SoraDNS リゾルバ＆ゲートウェイ インシデント対応プレイブック

このランブックは、resolver バンドルがレジストリ方針から逸脱した場合、
Signed Tree Head 証明が陳腐化した場合、またはゲートウェイが GAR マニフェストと
一致しないコンテンツを配信している場合のインシデントを扱う。DG-5 ロードマップ項目で
参照される transparency tailer、Signed Tree Head レポート CLI、ガバナンスの
エスカレーションループを結び付ける。

## 検知

1. **Prometheus アラート** – `dashboards/alerts/soradns_transparency_rules.yml` が
   `SoradnsProofAgeExceeded` 警告と `SoradnsCidDriftDetected` クリティカルを発報する。
   どちらかが発報したら、Alertmanager のエントリを取得してインシデントチケットに追加する。
2. **透明性レポートの確認** – 週次レポート CLI を以下で実行する:
   `cargo run -p soradns-resolver --bin soradns_transparency_report -- \
   --log /var/log/soradns/resolver.transparency.log \
   --output artifacts/soradns/transparency_report.md \
   --sth-output artifacts/soradns/signed_tree_heads.json \
   --recent-limit 40`。"Recent Events" テーブルが reorg または drift したゾーンを
   強調するので、両方のアーティファクトをチケットに添付する。
3. **ゲートウェイのテレメトリ** – `docs/source/sorafs_gateway_dns_owner_runbook.md` に
   GAR テレメトリプローブが記載されている。インシデントが HTTP ゲートウェイ由来の場合、
   最新の `sorafs_gateway-probe` 出力を取得する。

## 封じ込め

1. **影響を受けた namehash の凍結** – `docs/source/sns/governance_playbook.md` に従い、
   適切なレジストラ操作（`RegisterOfflineVerdictRevocation` または GAR 凍結）を提出する。
2. **Signed Tree Heads の再公開** – レポート CLI が生成した JSON アーティファクトを
   ガバナンス証拠バケットへアップロードし、バリデータと共有して新しい
   `policy_hash_hex` 値を確認できるようにする。
3. **ゲートウェイのロールバック** – `docs/source/sorafs_gateway_dns_owner_runbook.md` の
   GAR ロールバック手順に従い、その後で transparency tailer を再実行して整合性を確認する。

## 報告

1. 次の参照を含む透明性ノートを作成する:
   - `artifacts/soradns/<stamp>/` 配下の Markdown レポートと JSON Signed Tree Head バンドル。
   - アラート健全性と proof-age トレンドを示す Grafana スナップショット。
   - 実施したガバナンス対応（凍結、advisory、または thaw）。
2. `docs/source/reports/soradns_transparency.md` に記載の週次透明性ダイジェストへ
   インシデント概要と是正担当を追記する。
3. 適用できる場合は `ops/drill-log.md` に訓練またはインシデントを記録する。

## チェックリスト

- [ ] Prometheus アラートを確認し、スクリーンショット/ログリンクを記録する。
- [ ] `run_soradns_transparency_tail.sh` を実行してメトリクスアーティファクトを更新する。
- [ ] `soradns_transparency_report` を実行して Markdown + JSON バンドルを生成する。
- [ ] 必要に応じて凍結/ロールバックし、ガバナンスチケット ID を記録する。
- [ ] 週次レポートと透明性ダイジェストをインシデントノートで更新する。
